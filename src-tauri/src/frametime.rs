//! Real game performance measurement from frame-time captures.
//!
//! The app does not capture frames itself: PresentMon, CapFrameX and NVIDIA
//! FrameView already do that reliably and export CSV. This module imports
//! those CSVs, computes the usual metrics and compares runs "before" and
//! "after" a change, taking run-to-run variation into account. Nothing is
//! executed; the CSV is only parsed.

use serde_json::{json, Value};
use std::fs;

/// Upper bound for one imported CSV (a 5-minute PresentMon capture at 500 FPS
/// is roughly 30 MB with all columns).
const MAX_CSV_BYTES: usize = 64 * 1024 * 1024;
const MIN_FRAMES: usize = 100;
const MIN_SECONDS: f64 = 5.0;
/// Runs per state before a comparison is made at all.
pub const MIN_RUNS: usize = 3;

/// Frame-time columns in ms, in order of preference: PresentMon 1.x,
/// CapFrameX and FrameView use MsBetweenPresents; PresentMon 2.x FrameTime.
const FRAMETIME_COLUMNS: &[&str] = &["msbetweenpresents", "frametime", "ms between presents"];
const APP_COLUMNS: &[&str] = &["application", "processname", "process name"];

#[derive(Debug, Clone, PartialEq)]
pub struct Capture {
    /// Process the frames belong to (the one with the most frames).
    pub app: Option<String>,
    /// Other processes found in the file and left out.
    pub other_apps: Vec<String>,
    pub frametimes_ms: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stats {
    pub frames: usize,
    pub seconds: f64,
    pub avg_fps: f64,
    /// 1000 / 99th-percentile frame time.
    pub low1_fps: f64,
    /// 1000 / 99.9th-percentile frame time.
    pub low01_fps: f64,
    pub median_ms: f64,
    pub p99_ms: f64,
    pub stdev_ms: f64,
}

fn split_row(line: &str, sep: char) -> Vec<String> {
    line.split(sep)
        .map(|c| c.trim().trim_matches('"').trim().to_string())
        .collect()
}

/// Parses a PresentMon / CapFrameX / FrameView CSV. The header may be preceded
/// by comment or metadata lines; comma and semicolon separators are accepted
/// (semicolon files may use a decimal comma).
pub fn parse_capture(csv: &str) -> Result<Capture, String> {
    if csv.len() > MAX_CSV_BYTES {
        return Err("The file is larger than 64 MB.".into());
    }
    let lines: Vec<&str> = csv.lines().collect();
    let mut header = None;
    for (i, line) in lines.iter().enumerate().take(200) {
        let sep = if line.matches(';').count() > line.matches(',').count() {
            ';'
        } else {
            ','
        };
        let cols: Vec<String> = split_row(line, sep)
            .into_iter()
            .map(|c| c.to_ascii_lowercase())
            .collect();
        if let Some(ft) = FRAMETIME_COLUMNS
            .iter()
            .find_map(|name| cols.iter().position(|c| c == name))
        {
            let app = APP_COLUMNS
                .iter()
                .find_map(|name| cols.iter().position(|c| c == name));
            header = Some((i, sep, ft, app));
            break;
        }
    }
    let (start, sep, ft_col, app_col) = header.ok_or(
        "No frame-time column found. Export a CSV from PresentMon, CapFrameX or FrameView (column MsBetweenPresents or FrameTime).",
    )?;

    let mut rows: Vec<(Option<String>, f64)> = Vec::new();
    for line in &lines[start + 1..] {
        if line.trim().is_empty() {
            continue;
        }
        let cells = split_row(line, sep);
        let Some(raw) = cells.get(ft_col) else {
            continue;
        };
        let raw = if sep == ';' {
            raw.replace(',', ".")
        } else {
            raw.clone()
        };
        let Ok(ms) = raw.parse::<f64>() else {
            continue;
        };
        // Drop garbage; keep real hitches (up to 1 s).
        if !ms.is_finite() || ms <= 0.0 || ms > 1000.0 {
            continue;
        }
        let app = app_col
            .and_then(|c| cells.get(c))
            .filter(|a| !a.is_empty())
            .cloned();
        rows.push((app, ms));
    }

    // A capture of several processes: keep the one with the most frames.
    let mut counts: Vec<(Option<String>, usize)> = Vec::new();
    for (app, _) in &rows {
        match counts.iter_mut().find(|(a, _)| a == app) {
            Some((_, n)) => *n += 1,
            None => counts.push((app.clone(), 1)),
        }
    }
    counts.sort_by_key(|a| std::cmp::Reverse(a.1));
    let main = counts.first().and_then(|(a, _)| a.clone());
    let other_apps = counts
        .iter()
        .skip(1)
        .filter_map(|(a, _)| a.clone())
        .collect();
    let frametimes_ms = rows
        .into_iter()
        .filter(|(a, _)| *a == main)
        .map(|(_, ms)| ms)
        .collect();
    Ok(Capture {
        app: main,
        other_apps,
        frametimes_ms,
    })
}

/// Nearest-rank percentile of ascending sorted values.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

pub fn stats(frametimes_ms: &[f64]) -> Result<Stats, String> {
    let n = frametimes_ms.len();
    let total_ms: f64 = frametimes_ms.iter().sum();
    if n < MIN_FRAMES || total_ms / 1000.0 < MIN_SECONDS {
        return Err(format!(
            "Capture too short: {n} frames over {:.1} s. Record at least {MIN_SECONDS:.0} s (60 s or more gives stable numbers).",
            total_ms / 1000.0
        ));
    }
    let mut sorted = frametimes_ms.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let mean = total_ms / n as f64;
    let var = frametimes_ms
        .iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f64>()
        / n as f64;
    let p99 = percentile(&sorted, 99.0);
    Ok(Stats {
        frames: n,
        seconds: total_ms / 1000.0,
        avg_fps: 1000.0 * n as f64 / total_ms,
        low1_fps: 1000.0 / p99,
        low01_fps: 1000.0 / percentile(&sorted, 99.9),
        median_ms: percentile(&sorted, 50.0),
        p99_ms: p99,
        stdev_ms: var.sqrt(),
    })
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn stats_json(s: &Stats) -> Value {
    json!({
        "frames": s.frames, "seconds": round2(s.seconds),
        "avgFps": round2(s.avg_fps), "low1Fps": round2(s.low1_fps), "low01Fps": round2(s.low01_fps),
        "medianMs": round2(s.median_ms), "p99Ms": round2(s.p99_ms), "stdevMs": round2(s.stdev_ms),
    })
}

/// Verdict for one metric across runs of two states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Fewer than MIN_RUNS runs in a state.
    TooFewRuns,
    /// The difference is within run-to-run variation.
    WithinNoise,
    Better,
    Worse,
}

impl Verdict {
    fn as_str(self) -> &'static str {
        match self {
            Self::TooFewRuns => "tooFewRuns",
            Self::WithinNoise => "withinNoise",
            Self::Better => "better",
            Self::Worse => "worse",
        }
    }
}

fn mean_sd(xs: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let sd = if xs.len() > 1 {
        (xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt()
    } else {
        0.0
    };
    (mean, sd)
}

/// Compares a metric (higher is better) between "before" and "after" runs.
/// A change counts only if it exceeds twice the standard error of the
/// difference (about a 95 % interval) and at least 1 % of the baseline, so
/// normal run-to-run variation is never reported as a gain.
pub fn compare_metric(before: &[f64], after: &[f64]) -> (Verdict, Value) {
    if before.len() < MIN_RUNS || after.len() < MIN_RUNS {
        return (
            Verdict::TooFewRuns,
            json!({ "runsBefore": before.len(), "runsAfter": after.len() }),
        );
    }
    let (mb, sb) = mean_sd(before);
    let (ma, sa) = mean_sd(after);
    let diff = ma - mb;
    let noise = 2.0 * (sb.powi(2) / before.len() as f64 + sa.powi(2) / after.len() as f64).sqrt();
    let threshold = noise.max(0.01 * mb.abs());
    let verdict = if diff.abs() <= threshold {
        Verdict::WithinNoise
    } else if diff > 0.0 {
        Verdict::Better
    } else {
        Verdict::Worse
    };
    (
        verdict,
        json!({
            "before": round2(mb), "after": round2(ma), "sdBefore": round2(sb), "sdAfter": round2(sa),
            "diff": round2(diff), "diffPct": if mb != 0.0 { round2(diff / mb * 100.0) } else { 0.0 },
            "noise": round2(threshold), "runsBefore": before.len(), "runsAfter": after.len(),
        }),
    )
}

// ── Stored runs ──────────────────────────────────────────────────────────────

fn runs_path() -> std::path::PathBuf {
    crate::safety::app_data_dir().join("fps_runs.json")
}

fn load_runs() -> Result<Vec<Value>, String> {
    match fs::read_to_string(runs_path()) {
        Err(_) => Ok(Vec::new()),
        Ok(s) => serde_json::from_str(&s).map_err(|e| {
            format!("The saved measurements could not be read ({e}); the file was left unchanged.")
        }),
    }
}

fn save_runs(runs: &[Value]) -> Result<(), String> {
    let path = runs_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("creating data folder: {e}"))?;
    }
    crate::safety::write_user_file(
        &path,
        serde_json::to_string_pretty(runs)
            .unwrap_or_default()
            .as_bytes(),
    )
    .map_err(|e| format!("saving measurements: {e}"))
}

fn clean_text(s: &str, max: usize) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Imports one capture as a run of `game` in `state` ("before" | "after").
pub fn import(
    csv: String,
    file_name: String,
    game: String,
    state: String,
    note: String,
) -> Result<Value, String> {
    if !matches!(state.as_str(), "before" | "after") {
        return Err("State must be 'before' or 'after'.".into());
    }
    let game = clean_text(&game, 80);
    if game.is_empty() {
        return Err("Enter the game or scene this capture belongs to.".into());
    }
    let capture = parse_capture(&csv)?;
    let s = stats(&capture.frametimes_ms)?;
    let now = chrono::Local::now();
    let run = json!({
        "id": format!("fps-{}", now.format("%Y%m%d%H%M%S%6f")),
        "time": now.to_rfc3339(),
        "game": game,
        "state": state,
        "note": clean_text(&note, 200),
        "file": clean_text(&file_name, 200),
        "app": capture.app,
        "otherApps": capture.other_apps,
        "stats": stats_json(&s),
    });
    let mut runs = load_runs()?;
    runs.push(run.clone());
    save_runs(&runs)?;
    Ok(run)
}

pub fn list() -> Value {
    match load_runs() {
        Ok(runs) => json!({ "runs": runs }),
        Err(e) => json!({ "runs": [], "error": e }),
    }
}

pub fn delete(id: String) -> Result<Value, String> {
    let mut runs = load_runs()?;
    let before = runs.len();
    runs.retain(|r| r["id"].as_str() != Some(id.as_str()));
    if runs.len() == before {
        return Err("Measurement not found.".into());
    }
    save_runs(&runs)?;
    Ok(json!({ "ok": true }))
}

/// Before/after comparison of all runs of one game.
pub fn compare_runs(runs: &[Value], game: &str) -> Value {
    let pick = |state: &str, metric: &str| -> Vec<f64> {
        runs.iter()
            .filter(|r| r["game"].as_str() == Some(game) && r["state"].as_str() == Some(state))
            .filter_map(|r| r["stats"][metric].as_f64())
            .collect()
    };
    let metrics: Vec<Value> = ["avgFps", "low1Fps", "low01Fps"]
        .iter()
        .map(|m| {
            let (v, detail) = compare_metric(&pick("before", m), &pick("after", m));
            json!({ "metric": m, "verdict": v.as_str(), "detail": detail })
        })
        .collect();
    json!({ "game": game, "minRuns": MIN_RUNS, "metrics": metrics })
}

pub fn compare(game: String) -> Result<Value, String> {
    Ok(compare_runs(&load_runs()?, &game))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn presentmon_csv(app_frames: &[(&str, f64, usize)]) -> String {
        let mut s = String::from(
            "Application,ProcessID,SwapChainAddress,Runtime,SyncInterval,PresentFlags,Dropped,TimeInSeconds,MsBetweenPresents\n",
        );
        for (app, ms, n) in app_frames {
            for _ in 0..*n {
                s.push_str(&format!("{app},1234,0x1,DXGI,0,0,0,1.0,{ms}\n"));
            }
        }
        s
    }

    #[test]
    fn presentmon_csv_keeps_the_main_process() {
        let csv = presentmon_csv(&[("RainbowSix.exe", 4.0, 2000), ("dwm.exe", 16.6, 300)]);
        let c = parse_capture(&csv).unwrap();
        assert_eq!(c.app.as_deref(), Some("RainbowSix.exe"));
        assert_eq!(c.other_apps, vec!["dwm.exe".to_string()]);
        assert_eq!(c.frametimes_ms.len(), 2000);
    }

    #[test]
    fn capframex_and_semicolon_files_parse() {
        let csv = "// CapFrameX export\n\"MsBetweenPresents\";\"Other\"\n".to_string()
            + &"4,5;1\n".repeat(1500);
        let c = parse_capture(&csv).unwrap();
        assert_eq!(c.frametimes_ms.len(), 1500);
        assert!((c.frametimes_ms[0] - 4.5).abs() < 1e-9);
        // PresentMon 2.x column name.
        let csv2 = "Application,FrameTime\n".to_string() + &"game.exe,8\n".repeat(1000);
        assert_eq!(parse_capture(&csv2).unwrap().frametimes_ms.len(), 1000);
    }

    #[test]
    fn files_without_frame_times_are_rejected() {
        assert!(parse_capture("a,b,c\n1,2,3\n").is_err());
    }

    #[test]
    fn metrics_follow_their_definitions() {
        // 1978 frames at 4 ms (250 FPS) and 22 at 20 ms: the slowest 1 %
        // (20 frames) are all 20 ms frames.
        let mut ft = vec![4.0; 1978];
        ft.extend(vec![20.0; 22]);
        let s = stats(&ft).unwrap();
        let total = 1978.0 * 4.0 + 22.0 * 20.0;
        assert!((s.avg_fps - 1000.0 * 2000.0 / total).abs() < 1e-6);
        assert_eq!(s.p99_ms, 20.0);
        assert!((s.low1_fps - 50.0).abs() < 1e-9);
        assert!((s.low01_fps - 50.0).abs() < 1e-9);
        assert_eq!(s.median_ms, 4.0);
    }

    #[test]
    fn short_captures_are_rejected() {
        assert!(stats(&[4.0; 50]).is_err());
        assert!(stats(&[1.0; 1000]).is_err()); // 1 s
    }

    #[test]
    fn noise_is_never_reported_as_a_gain() {
        // ±2 FPS run-to-run variation, +1 FPS difference.
        let before = [200.0, 204.0, 202.0];
        let after = [201.0, 205.0, 203.0];
        assert_eq!(compare_metric(&before, &after).0, Verdict::WithinNoise);
        // Clear, consistent gain.
        let after = [220.0, 221.0, 219.0];
        assert_eq!(compare_metric(&before, &after).0, Verdict::Better);
        assert_eq!(compare_metric(&after, &before).0, Verdict::Worse);
        // Below 1 % of the baseline is never a result, even with no variation.
        assert_eq!(
            compare_metric(&[200.0; 3], &[201.0; 3]).0,
            Verdict::WithinNoise
        );
    }

    #[test]
    fn comparison_needs_enough_runs() {
        assert_eq!(
            compare_metric(&[200.0, 201.0], &[230.0, 231.0, 232.0]).0,
            Verdict::TooFewRuns
        );
    }

    #[test]
    fn runs_are_compared_per_game() {
        let run = |game: &str, state: &str, fps: f64| json!({ "game": game, "state": state, "stats": { "avgFps": fps, "low1Fps": fps / 2.0, "low01Fps": fps / 3.0 } });
        let mut runs = Vec::new();
        for fps in [200.0, 201.0, 199.0] {
            runs.push(run("R6", "before", fps));
            runs.push(run("R6", "after", fps + 30.0));
            runs.push(run("Other", "after", 10.0));
        }
        let c = compare_runs(&runs, "R6");
        assert_eq!(c["metrics"][0]["verdict"], "better");
        assert_eq!(c["metrics"][0]["detail"]["runsAfter"], 3);
    }
}
