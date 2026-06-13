//! Cleanup engine. Scan is read-only; deletion only runs on categories the
//! user explicitly selected, only inside hardcoded whitelisted roots, and
//! skips anything locked or younger than `min_age_hours`.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

struct Category {
    id: &'static str,
    name: &'static str,
    note: &'static str,
    min_age_hours: u64,
}

const CATS: &[Category] = &[
    Category { id: "win_temp", name: "Windows Temp", note: "C:\\Windows\\Temp — safe to clear; in-use files are skipped automatically.", min_age_hours: 24 },
    Category { id: "user_temp", name: "User Temp", note: "%TEMP% — safe to clear; in-use files are skipped automatically.", min_age_hours: 24 },
    Category { id: "wu_cache", name: "Windows Update download cache", note: "SoftwareDistribution\\Download — already-installed update payloads.", min_age_hours: 72 },
    Category { id: "dx_shader", name: "DirectX shader cache", note: "Rebuilt on demand; first game launches recompile shaders (brief stutter once).", min_age_hours: 0 },
    Category { id: "thumb_cache", name: "Thumbnail cache", note: "Explorer rebuilds thumbnails as folders are opened.", min_age_hours: 0 },
    Category { id: "crash_dumps", name: "Crash dumps (WER)", note: "Old crash reports. Keep if you're actively debugging a problem.", min_age_hours: 0 },
    Category { id: "chrome_cache", name: "Chrome HTTP cache", note: "Close Chrome first. Pages reload slower once.", min_age_hours: 0 },
    Category { id: "edge_cache", name: "Edge HTTP cache", note: "Close Edge first. Pages reload slower once.", min_age_hours: 0 },
];

fn cat_paths(id: &str) -> Vec<PathBuf> {
    let local = dirs::cache_dir().unwrap_or_default(); // %LOCALAPPDATA%
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    match id {
        "win_temp" => vec![PathBuf::from(format!("{windir}\\Temp"))],
        "user_temp" => vec![std::env::temp_dir()],
        "wu_cache" => vec![PathBuf::from(format!("{windir}\\SoftwareDistribution\\Download"))],
        "dx_shader" => vec![local.join("D3DSCache"), local.join("NVIDIA\\DXCache"), local.join("AMD\\DxCache")],
        "thumb_cache" => vec![local.join("Microsoft\\Windows\\Explorer")],
        "crash_dumps" => vec![local.join("CrashDumps"), PathBuf::from("C:\\ProgramData\\Microsoft\\Windows\\WER\\ReportQueue")],
        "chrome_cache" => vec![local.join("Google\\Chrome\\User Data\\Default\\Cache")],
        "edge_cache" => vec![local.join("Microsoft\\Edge\\User Data\\Default\\Cache")],
        _ => vec![],
    }
}

fn walk(dir: &Path, min_age: Duration, files: &mut Vec<(PathBuf, u64)>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let now = SystemTime::now();
    for entry in rd.flatten() {
        let p = entry.path();
        let Ok(md) = entry.metadata() else { continue };
        if md.is_dir() {
            walk(&p, min_age, files);
        } else if md.is_file() {
            let old_enough = md
                .modified()
                .ok()
                .and_then(|m| now.duration_since(m).ok())
                .map(|age| age >= min_age)
                .unwrap_or(false);
            if old_enough || min_age.is_zero() {
                files.push((p, md.len()));
            }
        }
    }
}

fn collect(id: &str, min_age_hours: u64) -> Vec<(PathBuf, u64)> {
    let mut files = Vec::new();
    for root in cat_paths(id) {
        if root.exists() {
            walk(&root, Duration::from_secs(min_age_hours * 3600), &mut files);
        }
    }
    files
}

/// Read-only scan: what could be freed, per category.
pub fn scan() -> Value {
    let out: Vec<Value> = CATS
        .iter()
        .map(|c| {
            let files = collect(c.id, c.min_age_hours);
            let bytes: u64 = files.iter().map(|(_, s)| s).sum();
            json!({
                "id": c.id, "name": c.name, "note": c.note,
                "fileCount": files.len(), "bytes": bytes,
                "paths": cat_paths(c.id).iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!(out)
}

/// Delete files in the selected categories. Locked/in-use files are skipped.
pub fn clean(category_ids: Vec<String>) -> Value {
    let mut freed: u64 = 0;
    let mut deleted = 0usize;
    let mut skipped = 0usize;
    for id in &category_ids {
        let Some(c) = CATS.iter().find(|c| c.id == id) else { continue };
        for (path, size) in collect(c.id, c.min_age_hours) {
            match fs::remove_file(&path) {
                Ok(_) => {
                    freed += size;
                    deleted += 1;
                }
                Err(_) => skipped += 1, // locked or permission — never force
            }
        }
        // Sweep now-empty subdirectories (ignore failures).
        for root in cat_paths(c.id) {
            let _ = remove_empty_dirs(&root, false);
        }
    }
    json!({ "freedBytes": freed, "deleted": deleted, "skippedInUse": skipped })
}

fn remove_empty_dirs(dir: &Path, remove_self: bool) -> std::io::Result<bool> {
    let mut empty = true;
    for entry in fs::read_dir(dir)? {
        let p = entry?.path();
        if p.is_dir() {
            if !remove_empty_dirs(&p, true).unwrap_or(false) {
                empty = false;
            }
        } else {
            empty = false;
        }
    }
    if empty && remove_self {
        fs::remove_dir(dir)?;
    }
    Ok(empty)
}
