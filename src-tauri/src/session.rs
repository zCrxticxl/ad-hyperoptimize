//! Stutter log: records what the PC does once per second while you play
//! (background processes by CPU and disk, DPC time, hard page faults, CPU
//! limit, GPU throttle reasons) and, with a frame-time capture of the same
//! session, shows which of these coincide with stutters noticeably more often
//! than during the rest of the session. Read-only; nothing is changed.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Hard cap: 2 hours at one sample per second.
const MAX_SAMPLES: usize = 7200;
const TOP_PER_SAMPLE: usize = 8;

// Thresholds for "something notable happened in this second". They flag,
// they do not prove a cause; the analysis compares against the whole session.
const PROC_CPU_PCT: f32 = 8.0; // % of the whole machine
const PROC_DISK_MBS: f64 = 20.0;
const DPC_PCT: f64 = 3.0;
const HARD_FAULTS_PER_S: f64 = 100.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Proc {
    pub name: String,
    /// Share of the whole machine (all cores), %.
    pub cpu: f32,
    pub disk_mbs: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Sample {
    /// Unix time, ms, at the end of the second this sample covers.
    pub t: i64,
    pub cpu: f32,
    pub cpu_limit: Option<f64>,
    pub dpc: Option<f64>,
    pub hard_faults: Option<f64>,
    pub gpu_util: Option<f64>,
    pub gpu_temp: Option<f64>,
    pub gpu_reasons: Vec<String>,
    pub top: Vec<Proc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    pub id: String,
    pub started: String,
    pub samples: Vec<Sample>,
}

type Running = (
    std::thread::JoinHandle<()>,
    Arc<AtomicBool>,
    Arc<Mutex<Recording>>,
);

#[derive(Default)]
pub struct SessionState {
    current: Mutex<Option<Running>>,
}

fn sessions_dir() -> std::path::PathBuf {
    crate::safety::app_data_dir().join("sessions")
}

fn valid_id(id: &str) -> bool {
    id.starts_with("sess-")
        && id.len() <= 40
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn path_for(id: &str) -> Result<std::path::PathBuf, String> {
    if !valid_id(id) {
        return Err("Invalid session id.".into());
    }
    Ok(sessions_dir().join(format!("{id}.json")))
}

pub fn start(st: &SessionState) -> Result<Value, String> {
    let mut slot = st.current.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((h, _, rec)) = slot.as_ref() {
        if !h.is_finished() {
            let id = rec.lock().unwrap_or_else(|e| e.into_inner()).id.clone();
            return Ok(json!({ "running": true, "id": id }));
        }
    }
    // A loop that ended on its own (cap reached) is saved before a new start.
    if let Some(prev) = slot.take() {
        finish(prev)?;
    }
    let now = chrono::Local::now();
    let rec = Arc::new(Mutex::new(Recording {
        id: format!("sess-{}", now.format("%Y%m%d-%H%M%S")),
        started: now.to_rfc3339(),
        samples: Vec::new(),
    }));
    let flag = Arc::new(AtomicBool::new(true));
    let (run, out) = (flag.clone(), rec.clone());
    let handle = std::thread::Builder::new()
        .spawn(move || record(run, out))
        .map_err(|e| format!("failed to start the recorder: {e}"))?;
    let id = rec.lock().unwrap_or_else(|e| e.into_inner()).id.clone();
    *slot = Some((handle, flag, rec));
    Ok(json!({ "running": true, "id": id }))
}

fn record(running: Arc<AtomicBool>, out: Arc<Mutex<Recording>>) {
    use sysinfo::System;
    let mut sys = System::new_all();
    let cores = sys.cpus().len().max(1) as f32;
    let limit = crate::pdh::PerfCounter::new(crate::pdh::CPU_PERFORMANCE_LIMIT);
    let dpc = crate::pdh::PerfCounter::new(crate::pdh::CPU_DPC_TIME);
    let faults = crate::pdh::PerfCounter::new(crate::pdh::HARD_FAULTS);
    sys.refresh_cpu();
    sys.refresh_processes();
    let mut last = std::time::Instant::now();
    let mut gpu: Option<crate::throttle::GpuSample> = None;
    let mut tick: u64 = 0;
    while running.load(Ordering::SeqCst) {
        for _ in 0..10 {
            if !running.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let secs = last.elapsed().as_secs_f64().max(0.1);
        last = std::time::Instant::now();
        sys.refresh_cpu();
        sys.refresh_processes();
        // nvidia-smi costs ~0.1-0.3 s: every other second is enough.
        if tick % 2 == 0 {
            gpu = crate::throttle::gpu_sample();
        }
        tick += 1;
        let mut procs: Vec<Proc> = sys
            .processes()
            .values()
            .map(|p| {
                let d = p.disk_usage();
                Proc {
                    name: p.name().to_string(),
                    cpu: p.cpu_usage() / cores,
                    disk_mbs: (d.read_bytes + d.written_bytes) as f64 / 1_048_576.0 / secs,
                }
            })
            .filter(|p| p.cpu >= 0.5 || p.disk_mbs >= 1.0)
            .collect();
        procs.sort_by(|a, b| {
            b.cpu
                .partial_cmp(&a.cpu)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut top: Vec<Proc> = procs.iter().take(5).cloned().collect();
        procs.sort_by(|a, b| {
            b.disk_mbs
                .partial_cmp(&a.disk_mbs)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for p in procs.into_iter().take(3) {
            if top.len() < TOP_PER_SAMPLE && !top.iter().any(|t| t.name == p.name) {
                top.push(p);
            }
        }
        let round1 = |v: f64| (v * 10.0).round() / 10.0;
        let sample = Sample {
            t: chrono::Utc::now().timestamp_millis(),
            cpu: sys.global_cpu_info().cpu_usage(),
            cpu_limit: limit.as_ref().and_then(|c| c.read()).map(round1),
            dpc: dpc.as_ref().and_then(|c| c.read()).map(round1),
            hard_faults: faults.as_ref().and_then(|c| c.read()).map(round1),
            gpu_util: gpu.as_ref().and_then(|g| g.util_pct),
            gpu_temp: gpu.as_ref().and_then(|g| g.temp_c),
            gpu_reasons: gpu
                .as_ref()
                .map(|g| g.reasons.iter().map(|r| r.to_string()).collect())
                .unwrap_or_default(),
            top: top
                .into_iter()
                .map(|p| Proc {
                    cpu: (p.cpu * 10.0).round() / 10.0,
                    disk_mbs: round1(p.disk_mbs),
                    ..p
                })
                .collect(),
        };
        let mut r = out.lock().unwrap_or_else(|e| e.into_inner());
        r.samples.push(sample);
        if r.samples.len() >= MAX_SAMPLES {
            return;
        }
    }
}

fn finish((handle, flag, rec): Running) -> Result<Value, String> {
    flag.store(false, Ordering::SeqCst);
    let _ = handle.join();
    let r = rec.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if r.samples.len() < 5 {
        return Ok(
            json!({ "saved": false, "reason": "Recording too short (under 5 seconds); not saved." }),
        );
    }
    let path = path_for(&r.id)?;
    std::fs::create_dir_all(sessions_dir()).map_err(|e| format!("creating data folder: {e}"))?;
    crate::safety::write_user_file(
        &path,
        serde_json::to_string(&r).unwrap_or_default().as_bytes(),
    )?;
    Ok(json!({ "saved": true, "id": r.id, "summary": summary(&r) }))
}

pub fn stop(st: &SessionState) -> Result<Value, String> {
    let mut slot = st.current.lock().unwrap_or_else(|e| e.into_inner());
    match slot.take() {
        Some(running) => finish(running),
        None => Ok(json!({ "saved": false, "reason": "No recording is running." })),
    }
}

pub fn status(st: &SessionState) -> Value {
    let slot = st.current.lock().unwrap_or_else(|e| e.into_inner());
    match slot.as_ref() {
        Some((h, _, rec)) => {
            let r = rec.lock().unwrap_or_else(|e| e.into_inner());
            json!({
                "running": !h.is_finished(),
                "capReached": r.samples.len() >= MAX_SAMPLES,
                "id": r.id,
                "seconds": r.samples.len(),
                "last": r.samples.last(),
            })
        }
        None => json!({ "running": false }),
    }
}

fn summary(r: &Recording) -> Value {
    json!({
        "id": r.id,
        "started": r.started,
        "seconds": r.samples.len(),
    })
}

fn load(id: &str) -> Result<Recording, String> {
    let raw =
        std::fs::read_to_string(path_for(id)?).map_err(|_| "Recording not found.".to_string())?;
    serde_json::from_str(&raw).map_err(|e| format!("The recording could not be read: {e}"))
}

pub fn list() -> Value {
    let mut out: Vec<Value> = std::fs::read_dir(sessions_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let id = name.strip_suffix(".json")?;
            load(id).ok().map(|r| summary(&r))
        })
        .collect();
    out.sort_by(|a, b| b["id"].as_str().cmp(&a["id"].as_str()));
    json!({ "sessions": out })
}

pub fn delete(id: String) -> Result<Value, String> {
    std::fs::remove_file(path_for(&id)?).map_err(|_| "Recording not found.".to_string())?;
    Ok(json!({ "ok": true }))
}

/// Notable conditions of one second, as stable keys ("proc:name:cpu",
/// "proc:name:disk", "dpc", "hardFaults", "cpuLimit", "gpu:reason").
pub fn conditions(s: &Sample, game: Option<&str>) -> Vec<String> {
    let mut c = Vec::new();
    for p in &s.top {
        if game.is_some_and(|g| p.name.eq_ignore_ascii_case(g)) {
            continue;
        }
        if p.cpu >= PROC_CPU_PCT {
            c.push(format!("proc:{}:cpu", p.name));
        }
        if p.disk_mbs >= PROC_DISK_MBS {
            c.push(format!("proc:{}:disk", p.name));
        }
    }
    if s.dpc.is_some_and(|v| v >= DPC_PCT) {
        c.push("dpc".into());
    }
    if s.hard_faults.is_some_and(|v| v >= HARD_FAULTS_PER_S) {
        c.push("hardFaults".into());
    }
    if crate::throttle::cpu_limited(s.cpu_limit) {
        c.push("cpuLimit".into());
    }
    for r in &s.gpu_reasons {
        c.push(format!("gpu:{r}"));
    }
    c
}

/// The process that used the most CPU over the session (likely the game).
pub fn busiest_process(samples: &[Sample]) -> Option<String> {
    let mut sum: std::collections::HashMap<&str, f32> = std::collections::HashMap::new();
    for s in samples {
        for p in &s.top {
            *sum.entry(p.name.as_str()).or_default() += p.cpu;
        }
    }
    sum.into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(n, _)| n.to_string())
}

/// Indices of stutter frames: at least twice the median of the previous 120
/// frames and at least 8 ms above it (a visible hitch, not normal jitter).
pub fn stutter_frames(ft: &[f64]) -> Vec<usize> {
    const WINDOW: usize = 120;
    let mut out = Vec::new();
    let mut win: Vec<f64> = Vec::with_capacity(WINDOW);
    for (i, &f) in ft.iter().enumerate() {
        if i >= 30 {
            let start = i.saturating_sub(WINDOW);
            win.clear();
            win.extend_from_slice(&ft[start..i]);
            win.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let median = win[win.len() / 2];
            if f >= 2.0 * median && f - median >= 8.0 {
                out.push(i);
            }
        }
    }
    out
}

fn condition_counts<'a>(
    samples: impl Iterator<Item = &'a Sample>,
    game: Option<&str>,
) -> std::collections::BTreeMap<String, usize> {
    let mut m = std::collections::BTreeMap::new();
    for s in samples {
        for c in conditions(s, game) {
            *m.entry(c).or_insert(0) += 1;
        }
    }
    m
}

/// Analysis of a recording, optionally with a frame-time capture whose last
/// frame ended at `capture_end_ms` (the CSV file's modification time).
pub fn analyze_recording(
    r: &Recording,
    capture: Option<(&crate::frametime::Capture, i64)>,
) -> Value {
    let samples = &r.samples;
    let game = match capture {
        Some((c, _)) => c.app.clone(),
        None => busiest_process(samples),
    };
    let game_ref = game.as_deref();
    let Some((cap, end_ms)) = capture else {
        let counts = condition_counts(samples.iter(), game_ref);
        let mut events: Vec<Value> = counts
            .into_iter()
            .map(|(k, n)| json!({ "condition": k, "seconds": n }))
            .collect();
        events.sort_by(|a, b| b["seconds"].as_u64().cmp(&a["seconds"].as_u64()));
        return json!({
            "id": r.id, "started": r.started, "seconds": samples.len(),
            "game": game, "gameGuessed": true, "events": events,
        });
    };
    let total_ms: f64 = cap.frametimes_ms.iter().sum();
    let start_ms = end_ms as f64 - total_ms;
    // Samples inside the capture window (each covers the second before t).
    let in_window: Vec<usize> = samples
        .iter()
        .enumerate()
        .filter(|(_, s)| (s.t as f64) > start_ms && (s.t as f64) - 1000.0 < end_ms as f64)
        .map(|(i, _)| i)
        .collect();
    if in_window.len() < 5 {
        return json!({
            "id": r.id, "error": "The capture does not overlap this recording. Record and capture the same session (the CSV file's time is used to line them up).",
        });
    }
    let stutters = stutter_frames(&cap.frametimes_ms);
    let mut cum = vec![0.0; cap.frametimes_ms.len()];
    let mut acc = 0.0;
    for (i, f) in cap.frametimes_ms.iter().enumerate() {
        acc += f;
        cum[i] = acc;
    }
    let mut stutter_samples: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    for &i in &stutters {
        let at = start_ms + cum[i];
        // The first sample at or after the stutter (within 1.5 s) covers it.
        if let Some(&si) = in_window
            .iter()
            .find(|&&si| (samples[si].t as f64) >= at && (samples[si].t as f64) - at <= 1500.0)
        {
            stutter_samples.insert(si);
        }
    }
    let all = condition_counts(in_window.iter().map(|&i| &samples[i]), game_ref);
    let during = condition_counts(stutter_samples.iter().map(|&i| &samples[i]), game_ref);
    let n_all = in_window.len() as f64;
    let n_st = stutter_samples.len() as f64;
    let mut findings: Vec<Value> = during
        .iter()
        .map(|(k, &n)| {
            let base = *all.get(k).unwrap_or(&n) as f64 / n_all;
            let rate = n as f64 / n_st.max(1.0);
            let lift = if base > 0.0 { rate / base } else { 0.0 };
            json!({
                "condition": k,
                "stutterSeconds": n,
                "sessionSeconds": all.get(k).copied().unwrap_or(n),
                "rateDuringStutter": (rate * 1000.0).round() / 10.0,
                "rateOverall": (base * 1000.0).round() / 10.0,
                "lift": (lift * 10.0).round() / 10.0,
                // Needs repeats and a clearly higher rate than usual.
                "likely": n >= 2 && lift >= 2.0,
            })
        })
        .collect();
    findings.sort_by(|a, b| {
        b["likely"].as_bool().cmp(&a["likely"].as_bool()).then(
            b["lift"]
                .as_f64()
                .partial_cmp(&a["lift"].as_f64())
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    json!({
        "id": r.id, "started": r.started, "seconds": samples.len(),
        "game": game, "gameGuessed": false,
        "capture": {
            "frames": cap.frametimes_ms.len(),
            "seconds": (total_ms / 100.0).round() / 10.0,
            "stutters": stutters.len(),
            "stutterSeconds": stutter_samples.len(),
            "overlapSeconds": in_window.len(),
        },
        "findings": findings,
    })
}

pub fn analyze(
    id: String,
    csv: Option<String>,
    capture_end_ms: Option<i64>,
) -> Result<Value, String> {
    let r = load(&id)?;
    match (csv, capture_end_ms) {
        (Some(csv), Some(end)) => {
            let cap = crate::frametime::parse_capture(&csv)?;
            Ok(analyze_recording(&r, Some((&cap, end))))
        }
        _ => Ok(analyze_recording(&r, None)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(t: i64, top: Vec<(&str, f32, f64)>) -> Sample {
        Sample {
            t,
            top: top
                .into_iter()
                .map(|(n, c, d)| Proc {
                    name: n.into(),
                    cpu: c,
                    disk_mbs: d,
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn conditions_skip_the_game_and_flag_spikes() {
        let s = Sample {
            dpc: Some(4.0),
            cpu_limit: Some(70.0),
            gpu_reasons: vec!["thermal".into()],
            ..sample(
                0,
                vec![
                    ("Game.exe", 40.0, 0.0),
                    ("MsMpEng.exe", 12.0, 30.0),
                    ("calm.exe", 1.0, 0.0),
                ],
            )
        };
        let c = conditions(&s, Some("game.exe"));
        assert_eq!(
            c,
            vec![
                "proc:MsMpEng.exe:cpu",
                "proc:MsMpEng.exe:disk",
                "dpc",
                "cpuLimit",
                "gpu:thermal"
            ]
        );
    }

    #[test]
    fn stutter_detection_needs_a_visible_hitch() {
        let mut ft = vec![7.0; 300];
        ft[100] = 30.0; // hitch
        ft[200] = 12.0; // within normal jitter: not counted
        assert_eq!(stutter_frames(&ft), vec![100]);
    }

    #[test]
    fn analysis_ranks_conditions_that_coincide_with_stutters() {
        // 60 s capture at ~143 FPS ending at t = 60_000; stutters at 10 s, 30 s, 50 s.
        let mut ft = Vec::new();
        for sec in 0..60 {
            for i in 0..143 {
                let hitch = i == 70 && sec % 20 == 10;
                ft.push(if hitch { 40.0 } else { 7.0 });
            }
        }
        let total: f64 = ft.iter().sum();
        let end = total.round() as i64;
        let cap = crate::frametime::Capture {
            app: Some("Game.exe".into()),
            other_apps: vec![],
            frametimes_ms: ft,
        };
        // OneDrive spikes exactly in the stutter seconds; Discord all the time.
        let samples: Vec<Sample> = (1..=61)
            .map(|s| {
                let mut top = vec![("Game.exe", 40.0, 0.0), ("Discord.exe", 9.0, 0.0)];
                if s % 20 == 11 {
                    top.push(("OneDrive.exe", 15.0, 0.0));
                }
                sample(s * 1000, top)
            })
            .collect();
        let r = Recording {
            id: "sess-test".into(),
            started: String::new(),
            samples,
        };
        let a = analyze_recording(&r, Some((&cap, end)));
        assert_eq!(a["capture"]["stutters"], 3);
        let top = &a["findings"][0];
        assert_eq!(top["condition"], "proc:OneDrive.exe:cpu");
        assert_eq!(top["likely"], true);
        let discord = a["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["condition"] == "proc:Discord.exe:cpu")
            .unwrap();
        assert_eq!(
            discord["likely"], false,
            "present all the time, not specific to stutters"
        );
    }

    #[test]
    fn ids_are_validated() {
        assert!(valid_id("sess-20261024-181500"));
        assert!(!valid_id("../x"));
        assert!(!valid_id("sess-..\\x"));
    }
}
