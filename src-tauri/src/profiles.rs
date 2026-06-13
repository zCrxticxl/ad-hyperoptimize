//! One-click optimization profiles: curated tweak bundles with optional
//! before/after benchmarking and bundle-wide revert. Reuses the tweak
//! engine, so every change is still journaled and individually undoable.

use crate::{bench, safety, tweaks};
use serde_json::{json, Value};

struct Profile {
    id: &'static str,
    name: &'static str,
    desc: &'static str,
    tweaks: &'static [&'static str],
}

const PROFILES: &[Profile] = &[
    Profile {
        id: "esports",
        name: "Esports / Competitive",
        desc: "Maximum responsiveness: High Performance power, Game DVR off, MMCSS scheduler, foreground boost, 1:1 mouse, no power throttling. Reboot recommended after.",
        tweaks: &[
            "power_high_performance",
            "game_dvr_off",
            "game_mode_on",
            "fse_optimizations_off",
            "mmcss_gaming",
            "mmcss_games_task",
            "priority_separation",
            "mouse_accel_off",
            "power_throttling_off",
        ],
    },
    Profile {
        id: "streaming",
        name: "Streaming / OBS",
        desc: "Game AND encoder both need CPU: no aggressive foreground boost, DVR off (conflicts with OBS capture), MMCSS, no throttling of background encoder, fewer background apps.",
        tweaks: &[
            "power_high_performance",
            "game_dvr_off",
            "game_mode_on",
            "mmcss_gaming",
            "power_throttling_off",
            "background_apps_off",
            "content_suggestions_off",
        ],
    },
    Profile {
        id: "workstation",
        name: "Workstation / Productivity",
        desc: "Fast boot, snappy UI, less telemetry/ads and background noise — without gaming scheduler changes.",
        tweaks: &[
            "power_high_performance",
            "startup_delay_off",
            "menu_delay_fast",
            "telemetry_minimal",
            "advertising_id_off",
            "content_suggestions_off",
            "background_apps_off",
            "power_throttling_off",
        ],
    },
];

fn tweak_status_map() -> std::collections::HashMap<String, (String, String, bool)> {
    let mut m = std::collections::HashMap::new();
    if let Value::Array(arr) = tweaks::list_with_status() {
        for t in arr {
            m.insert(
                t["id"].as_str().unwrap_or("").to_string(),
                (
                    t["name"].as_str().unwrap_or("").to_string(),
                    t["status"].as_str().unwrap_or("unknown").to_string(),
                    t["undoable"].as_bool().unwrap_or(false),
                ),
            );
        }
    }
    m
}

pub fn list() -> Value {
    let status = tweak_status_map();
    let out: Vec<Value> = PROFILES
        .iter()
        .map(|p| {
            let tws: Vec<Value> = p
                .tweaks
                .iter()
                .map(|id| {
                    let (name, st, undo) = status
                        .get(*id)
                        .cloned()
                        .unwrap_or(("?".into(), "unknown".into(), false));
                    json!({ "id": id, "name": name, "status": st, "undoable": undo })
                })
                .collect();
            let applied = tws.iter().filter(|t| t["status"] == "applied").count();
            json!({
                "id": p.id, "name": p.name, "desc": p.desc,
                "tweaks": tws, "appliedCount": applied,
            })
        })
        .collect();
    json!(out)
}

fn bench_suite() -> Value {
    json!({ "cpu": bench::cpu(), "memory": bench::memory(), "disk": bench::disk() })
}

/// Apply a whole profile. Optionally benchmark before/after.
/// Per-tweak failures don't abort the rest — they're reported.
pub fn apply(profile_id: &str, with_bench: bool) -> Result<Value, String> {
    let p = PROFILES
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| format!("unbekanntes Profil '{profile_id}'"))?;

    let restore_msg = safety::create_restore_point(&format!("AD HyperOptimize Profil: {}", p.name))
        .unwrap_or_else(|e| format!("Restore Point übersprungen: {e}"));

    let before = if with_bench { Some(bench_suite()) } else { None };

    let status = tweak_status_map();
    let mut applied = Vec::new();
    let mut skipped = Vec::new();
    let mut failed = Vec::new();
    for id in p.tweaks {
        match status.get(*id).map(|s| s.1.as_str()) {
            Some("applied") => skipped.push(json!(id)),
            _ => match tweaks::apply(id) {
                Ok(_) => applied.push(json!(id)),
                Err(e) => failed.push(json!({ "id": id, "error": e })),
            },
        }
    }

    let after = if with_bench { Some(bench_suite()) } else { None };

    Ok(json!({
        "profile": p.id,
        "restorePoint": restore_msg,
        "applied": applied,
        "skipped": skipped,
        "failed": failed,
        "benchBefore": before,
        "benchAfter": after,
        "benchNote": "Synthetische Benchmarks zeigen Scheduler-/Latenz-Tweaks kaum — der echte Effekt zeigt sich in Frametimes im Spiel. Disk/CPU-Deltas unter ~3% sind Messrauschen.",
    }))
}

/// Revert every undoable tweak of a profile (reverse order).
pub fn revert(profile_id: &str) -> Result<Value, String> {
    let p = PROFILES
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| format!("unbekanntes Profil '{profile_id}'"))?;
    let mut reverted = Vec::new();
    let mut skipped = Vec::new();
    let mut failed = Vec::new();
    for id in p.tweaks.iter().rev() {
        match tweaks::revert(id) {
            Ok(_) => reverted.push(json!(id)),
            Err(e) if e.contains("nothing to undo") => skipped.push(json!(id)),
            Err(e) => failed.push(json!({ "id": id, "error": e })),
        }
    }
    Ok(json!({ "profile": p.id, "reverted": reverted, "skipped": skipped, "failed": failed }))
}
