//! Process manager: live list, kill, priority and CPU-affinity control.
//! Critical system processes are protected from kill at the backend level.

use crate::ps;
use serde_json::{json, Value};
use std::time::Duration;
use sysinfo::{Pid, System};

const PROTECTED: &[&str] = &[
    "system", "registry", "smss.exe", "csrss.exe", "wininit.exe", "winlogon.exe",
    "services.exe", "lsass.exe", "svchost.exe", "dwm.exe", "fontdrvhost.exe",
    "memory compression",
];

fn is_protected(name: &str) -> bool {
    PROTECTED.contains(&name.to_lowercase().as_str())
}

pub fn list() -> Value {
    let mut sys = System::new_all();
    sys.refresh_processes();
    std::thread::sleep(Duration::from_millis(300));
    sys.refresh_processes();

    let ncpu = sys.cpus().len().max(1);
    let mut procs: Vec<Value> = sys
        .processes()
        .values()
        .map(|p| {
            json!({
                "pid": p.pid().as_u32(),
                "name": p.name(),
                // normalize: sysinfo reports % of one core
                "cpu": ((p.cpu_usage() / ncpu as f32) * 10.0).round() / 10.0,
                "memMb": p.memory() / 1_048_576,
                "path": p.exe().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default(),
                "protected": is_protected(p.name()),
            })
        })
        .collect();
    procs.sort_by(|a, b| {
        b["cpu"].as_f64().partial_cmp(&a["cpu"].as_f64()).unwrap_or(std::cmp::Ordering::Equal)
    });
    json!({ "coreCount": ncpu, "processes": procs })
}

pub fn kill(pid: u32) -> Result<Value, String> {
    let mut sys = System::new();
    sys.refresh_processes();
    let p = sys.process(Pid::from_u32(pid)).ok_or("Process not found (already terminated?)")?;
    if is_protected(p.name()) {
        return Err(format!("'{}' ist ein geschützter Systemprozess — Beenden würde Windows crashen.", p.name()));
    }
    if p.kill() {
        Ok(json!({ "killed": pid }))
    } else {
        Err("Beenden fehlgeschlagen (Zugriff verweigert — als Admin starten?)".into())
    }
}

const PRIORITIES: &[&str] = &["Idle", "BelowNormal", "Normal", "AboveNormal", "High", "RealTime"];

pub fn set_priority(pid: u32, priority: String) -> Result<Value, String> {
    if !PRIORITIES.contains(&priority.as_str()) {
        return Err(format!("ungültige Priorität '{priority}'"));
    }
    ps::run(&format!(
        "(Get-Process -Id {pid} -ErrorAction Stop).PriorityClass = '{priority}'; 'OK'"
    ))?;
    Ok(json!({ "pid": pid, "priority": priority }))
}

/// `mask` = bitmask of allowed cores (bit 0 = core 0, …).
pub fn set_affinity(pid: u32, mask: u64) -> Result<Value, String> {
    if mask == 0 {
        return Err("mindestens ein Kern muss ausgewählt sein".into());
    }
    ps::run(&format!(
        "(Get-Process -Id {pid} -ErrorAction Stop).ProcessorAffinity = [IntPtr]{mask}; 'OK'"
    ))?;
    Ok(json!({ "pid": pid, "mask": mask }))
}

// ---------- permanent priority via IFEO PerfOptions ----------
// Documented Windows mechanism: HKLM\...\Image File Execution Options\<exe>\
// PerfOptions\CpuPriorityClass. Applies to every future start of the exe.
// RealTime is intentionally not supported here (not honored + dangerous).

#[cfg(windows)]
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
#[cfg(windows)]
use winreg::RegKey;

#[cfg(windows)]
const IFEO: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Image File Execution Options";

fn prio_to_dword(p: &str) -> Option<u32> {
    Some(match p {
        "Idle" => 1,
        "BelowNormal" => 5,
        "Normal" => 2,
        "AboveNormal" => 6,
        "High" => 3,
        _ => return None,
    })
}

fn dword_to_prio(d: u32) -> &'static str {
    match d {
        1 => "Idle",
        5 => "BelowNormal",
        2 => "Normal",
        6 => "AboveNormal",
        3 => "High",
        _ => "?",
    }
}

/// All exes with a permanent CpuPriorityClass override.
pub fn perm_list() -> Value {
    #[cfg(windows)]
    {
        let mut out = serde_json::Map::new();
        if let Ok(ifeo) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(IFEO, KEY_READ) {
            for exe in ifeo.enum_keys().filter_map(|k| k.ok()) {
                if let Ok(po) = ifeo.open_subkey_with_flags(format!("{exe}\\PerfOptions"), KEY_READ) {
                    if let Ok(d) = po.get_value::<u32, _>("CpuPriorityClass") {
                        out.insert(exe.to_lowercase(), json!(dword_to_prio(d)));
                    }
                }
            }
        }
        Value::Object(out)
    }
    #[cfg(not(windows))]
    json!({})
}

/// Set permanent priority for an exe name (e.g. "chrome.exe") and apply it to
/// all currently running instances too.
pub fn perm_set(exe: String, priority: String) -> Result<Value, String> {
    #[cfg(windows)]
    {
        if !exe.to_lowercase().ends_with(".exe") || exe.contains(['\\', '/']) {
            return Err("erwarte einen Exe-Namen wie 'chrome.exe'".into());
        }
        if !ps::is_admin() {
            return Err("Dauerhafte Priorität braucht Adminrechte (HKLM/IFEO).".into());
        }
        let d = prio_to_dword(&priority)
            .ok_or("ungültige Priorität (RealTime ist dauerhaft nicht erlaubt)")?;
        let (key, _) = RegKey::predef(HKEY_LOCAL_MACHINE)
            .create_subkey(format!("{IFEO}\\{exe}\\PerfOptions"))
            .map_err(|e| format!("IFEO: {e}"))?;
        key.set_value("CpuPriorityClass", &d).map_err(|e| e.to_string())?;
        // Best effort: also apply to running instances right now.
        let base = exe.trim_end_matches(".exe").trim_end_matches(".EXE");
        let _ = ps::run(&format!(
            "Get-Process -Name '{base}' -ErrorAction SilentlyContinue | ForEach-Object {{ $_.PriorityClass = '{priority}' }}"
        ));
        Ok(json!({ "exe": exe, "priority": priority, "permanent": true }))
    }
    #[cfg(not(windows))]
    {
        let _ = (exe, priority);
        Err("Windows only".into())
    }
}

pub fn perm_remove(exe: String) -> Result<Value, String> {
    #[cfg(windows)]
    {
        if !ps::is_admin() {
            return Err("Adminrechte erforderlich.".into());
        }
        let ifeo = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(IFEO, KEY_READ | winreg::enums::KEY_SET_VALUE)
            .map_err(|e| e.to_string())?;
        ifeo.delete_subkey_all(format!("{exe}\\PerfOptions")).map_err(|e| e.to_string())?;
        let _ = ifeo.delete_subkey(&exe); // remove now-empty parent; ignore if it has other values
        Ok(json!({ "exe": exe, "removed": true }))
    }
    #[cfg(not(windows))]
    {
        let _ = exe;
        Err("Windows only".into())
    }
}

pub fn get_detail(pid: u32) -> Value {
    ps::run_json(&format!(
        "Get-Process -Id {pid} -ErrorAction Stop | Select-Object Id,ProcessName,PriorityClass,@{{n='affinity';e={{[int64]$_.ProcessorAffinity}}}},StartTime,Path"
    ))
    .unwrap_or_else(|e| json!({ "error": e.trim() }))
}
