//! Pre-game check: system activity that competes with a game right now
//! (Windows Update installing, Defender scanning, search indexing, .NET
//! optimization after updates, cloud sync, updaters), and a reversible
//! "pause until I'm done" for the parts Windows lets an app pause safely.
//!
//! Pausing never touches Windows Update or Defender settings: a running
//! Defender scan can be cancelled (Windows schedules it again), the search
//! indexer service is stopped and restarted afterwards, OneDrive is closed
//! and started again. Everything else is only reported.

use serde_json::{json, Value};

/// (process name without .exe, activity key). Matched case-insensitively.
const ACTIVITY: &[(&str, &str)] = &[
    ("TiWorker", "windowsUpdate"),
    ("TrustedInstaller", "windowsUpdate"),
    ("MoUsoCoreWorker", "windowsUpdate"),
    ("wuauclt", "windowsUpdate"),
    ("MsMpEng", "defender"),
    ("MpCmdRun", "defender"),
    ("SearchIndexer", "indexing"),
    ("SearchProtocolHost", "indexing"),
    ("SearchFilterHost", "indexing"),
    ("mscorsvw", "dotnet"),
    ("ngen", "dotnet"),
    ("CompatTelRunner", "compat"),
    ("DismHost", "maintenance"),
    ("mrt", "maintenance"),
    ("OneDrive", "onedrive"),
    ("GoogleDriveFS", "sync"),
    ("Dropbox", "sync"),
    ("GoogleUpdate", "updater"),
    ("MicrosoftEdgeUpdate", "updater"),
    ("AdobeARM", "updater"),
];

/// A process counts as busy above this share of the whole machine or this
/// disk throughput (MB/s), measured over ~1.5 s.
const BUSY_CPU_PCT: f32 = 2.0;
const BUSY_DISK_MBS: f64 = 5.0;

/// Activities the app can pause and resume.
pub const PAUSABLE: &[&str] = &["defender", "indexing", "onedrive"];

pub fn activity_of(process: &str) -> Option<&'static str> {
    let base = process.strip_suffix(".exe").unwrap_or(process);
    ACTIVITY
        .iter()
        .find(|(p, _)| p.eq_ignore_ascii_case(base))
        .map(|(_, a)| *a)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Busy {
    pub process: String,
    pub activity: &'static str,
    pub cpu: f32,
    pub disk_mbs: f64,
}

pub fn is_busy(cpu: f32, disk_mbs: f64) -> bool {
    cpu >= BUSY_CPU_PCT || disk_mbs >= BUSY_DISK_MBS
}

/// Groups busy processes by activity: [{ activity, pausable, processes }].
pub fn group(busy: &[Busy]) -> Vec<Value> {
    let mut seen: Vec<&str> = Vec::new();
    for b in busy {
        if !seen.contains(&b.activity) {
            seen.push(b.activity);
        }
    }
    seen.into_iter()
        .map(|a| {
            json!({
                "activity": a,
                "pausable": PAUSABLE.contains(&a),
                "processes": busy.iter().filter(|b| b.activity == a).map(|b| json!({
                    "name": b.process,
                    "cpu": (b.cpu * 10.0).round() / 10.0,
                    "diskMbs": (b.disk_mbs * 10.0).round() / 10.0,
                })).collect::<Vec<_>>(),
            })
        })
        .collect()
}

pub fn scan() -> Value {
    use sysinfo::System;
    let mut sys = System::new_all();
    let cores = sys.cpus().len().max(1) as f32;
    sys.refresh_processes();
    let t = std::time::Instant::now();
    std::thread::sleep(std::time::Duration::from_millis(1500));
    sys.refresh_processes();
    let secs = t.elapsed().as_secs_f64();
    let mut busy: Vec<Busy> = sys
        .processes()
        .values()
        .filter_map(|p| {
            let activity = activity_of(p.name())?;
            let d = p.disk_usage();
            let b = Busy {
                process: p.name().to_string(),
                activity,
                cpu: p.cpu_usage() / cores,
                disk_mbs: (d.read_bytes + d.written_bytes) as f64 / 1_048_576.0 / secs,
            };
            is_busy(b.cpu, b.disk_mbs).then_some(b)
        })
        .collect();
    busy.sort_by(|a, b| a.activity.cmp(b.activity));
    json!({ "activities": group(&busy), "paused": paused_state() })
}

fn state_path() -> std::path::PathBuf {
    crate::safety::app_data_dir().join("pregame_paused.json")
}

fn paused_state() -> Value {
    std::fs::read_to_string(state_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}

#[cfg(windows)]
fn onedrive_exe() -> Option<String> {
    let local = std::env::var("LOCALAPPDATA").ok()?;
    let p = format!(r"{local}\Microsoft\OneDrive\OneDrive.exe");
    std::path::Path::new(&p).is_file().then_some(p)
}

/// Pauses the chosen activities; writes what to undo before acting.
#[cfg(windows)]
pub fn pause(activities: Vec<String>) -> Result<Value, String> {
    if !paused_state().is_null() {
        return Err("Already paused. Resume first.".into());
    }
    let mut done = serde_json::Map::new();
    let mut notes = Vec::new();
    // Write-ahead: the resume plan is stored before anything is stopped.
    let mut plan = serde_json::Map::new();
    if activities.iter().any(|a| a == "indexing") {
        let state =
            crate::ps::run("(Get-Service -Name WSearch -ErrorAction Stop).Status.ToString()")?;
        plan.insert(
            "indexingWasRunning".into(),
            json!(state.trim() == "Running"),
        );
    }
    if activities.iter().any(|a| a == "onedrive") {
        plan.insert("onedrive".into(), json!(onedrive_exe()));
    }
    std::fs::create_dir_all(crate::safety::app_data_dir()).map_err(|e| e.to_string())?;
    crate::safety::write_user_file(
        &state_path(),
        Value::Object(plan.clone()).to_string().as_bytes(),
    )?;

    for a in &activities {
        match a.as_str() {
            "defender" => {
                let exe = format!(
                    r"{}\Windows Defender\MpCmdRun.exe",
                    std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into())
                );
                let r = crate::ps::exec(&exe, &["-Cancel"]);
                done.insert("defender".into(), json!(r.is_ok()));
                notes.push(match r {
                    Ok(_) => "Defender scan cancelled; Windows runs it again later.".to_string(),
                    Err(e) => format!("Defender scan could not be cancelled: {}", e.trim()),
                });
            }
            "indexing" if plan["indexingWasRunning"] == json!(true) => {
                let r = crate::ps::run("Stop-Service -Name WSearch -Force -ErrorAction Stop");
                done.insert("indexing".into(), json!(r.is_ok()));
                if let Err(e) = r {
                    notes.push(format!("Search indexing could not be paused: {}", e.trim()));
                }
            }
            "onedrive" => {
                if let Some(exe) = plan["onedrive"].as_str() {
                    let r = crate::ps::exec(exe, &["/shutdown"]);
                    done.insert("onedrive".into(), json!(r.is_ok()));
                }
            }
            _ => {}
        }
    }
    Ok(json!({ "ok": true, "done": done, "notes": notes }))
}

/// Restores what `pause` stopped and clears the state.
#[cfg(windows)]
pub fn resume() -> Result<Value, String> {
    let plan = paused_state();
    if plan.is_null() {
        return Ok(json!({ "ok": true, "nothing": true }));
    }
    let mut errors = Vec::new();
    if plan["indexingWasRunning"] == json!(true) {
        if let Err(e) = crate::ps::run("Start-Service -Name WSearch -ErrorAction Stop") {
            errors.push(format!("search indexing: {}", e.trim()));
        }
    }
    if let Some(exe) = plan["onedrive"].as_str() {
        // Through explorer.exe so OneDrive runs as the user, not elevated.
        let explorer = std::path::Path::new(&crate::ps::windows_dir()).join("explorer.exe");
        if let Err(e) = std::process::Command::new(explorer).arg(exe).spawn() {
            errors.push(format!("OneDrive: {e}"));
        }
    }
    if !errors.is_empty() {
        return Err(format!(
            "Resume incomplete ({}); try again.",
            errors.join("; ")
        ));
    }
    let _ = std::fs::remove_file(state_path());
    Ok(json!({ "ok": true }))
}

#[cfg(not(windows))]
pub fn pause(_activities: Vec<String>) -> Result<Value, String> {
    Err("Windows only".into())
}

#[cfg(not(windows))]
pub fn resume() -> Result<Value, String> {
    Err("Windows only".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_processes_to_activities() {
        assert_eq!(activity_of("TiWorker.exe"), Some("windowsUpdate"));
        assert_eq!(activity_of("msmpeng.exe"), Some("defender"));
        assert_eq!(activity_of("mscorsvw"), Some("dotnet"));
        assert_eq!(activity_of("RainbowSix.exe"), None);
        assert_eq!(activity_of("OneDriveSetup.exe"), None, "exact names only");
    }

    #[test]
    fn groups_by_activity_and_marks_pausable() {
        let b = |p: &str, a: &'static str| Busy {
            process: p.into(),
            activity: a,
            cpu: 5.0,
            disk_mbs: 0.0,
        };
        let g = group(&[
            b("MsMpEng.exe", "defender"),
            b("TiWorker.exe", "windowsUpdate"),
            b("TrustedInstaller.exe", "windowsUpdate"),
        ]);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0]["activity"], "defender");
        assert_eq!(g[0]["pausable"], true);
        assert_eq!(g[1]["activity"], "windowsUpdate");
        assert_eq!(g[1]["pausable"], false);
        assert_eq!(g[1]["processes"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn busy_threshold() {
        assert!(is_busy(2.5, 0.0));
        assert!(is_busy(0.0, 6.0));
        assert!(!is_busy(0.5, 1.0));
    }
}
