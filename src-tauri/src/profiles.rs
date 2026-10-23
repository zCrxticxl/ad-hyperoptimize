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
        desc: "Game AND encoder both need CPU: no aggressive foreground boost, DVR off (conflicts with OBS capture), MMCSS, no throttling of background encoder.",
        tweaks: &[
            "power_high_performance",
            "game_dvr_off",
            "game_mode_on",
            "mmcss_gaming",
            "power_throttling_off",
            "content_suggestions_off",
        ],
    },
    Profile {
        id: "workstation",
        name: "Workstation / Productivity",
        desc: "Fast boot, snappy UI, less telemetry/ads and background noise, without gaming scheduler changes.",
        tweaks: &[
            "power_high_performance",
            "startup_delay_off",
            "menu_delay_fast",
            "telemetry_minimal",
            "advertising_id_off",
            "content_suggestions_off",
            "power_throttling_off",
        ],
    },
];
// Deliberately NOT in any default profile (audit): fse_optimizations_off
// (fullscreen optimizations are required for modern flip-model presentation
// and Auto HDR; disabling them regresses many current titles) and
// background_apps_off (breaks notifications/sync of Store apps such as
// Mail, Teams, Phone Link). Both remain available individually.

/// Tweaks whose benefit depends on the hardware: they are applied by a
/// profile only when hwprofile has no risk verdict for this machine.
const HW_GATED: &[&str] = &[
    "priority_separation",
    "mmcss_games_task",
    "power_throttling_off",
];

/// Why `id` should be skipped on this hardware, from hwprofile's
/// `tweakRisks` map (None = no verdict, or not a gated tweak).
fn hw_skip_reason(id: &str, tweak_risks: &Value) -> Option<String> {
    if !HW_GATED.contains(&id) {
        return None;
    }
    let r = tweak_risks.get(id)?;
    Some(format!(
        "skipped on this hardware: {}",
        r["title"].as_str().unwrap_or("hardware risk verdict")
    ))
}

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
    let journal = safety::load_journal().unwrap_or_default();
    let out: Vec<Value> = PROFILES
        .iter()
        .map(|p| {
            let tws: Vec<Value> = p
                .tweaks
                .iter()
                .map(|id| {
                    let (name, st, undo) =
                        status
                            .get(*id)
                            .cloned()
                            .unwrap_or(("?".into(), "unknown".into(), false));
                    json!({ "id": id, "name": name, "status": st, "undoable": undo })
                })
                .collect();
            let applied = tws.iter().filter(|t| t["status"] == "applied").count();
            let origin = format!("profile:{}", p.id);
            let owned_count = journal
                .iter()
                .filter(|entry| entry.origin.as_deref() == Some(origin.as_str()) && !entry.reverted)
                .count();
            json!({
                "id": p.id, "name": p.name, "desc": p.desc,
                "tweaks": tws, "appliedCount": applied, "ownedCount": owned_count,
            })
        })
        .collect();
    json!(out)
}

fn bench_suite() -> Value {
    json!({ "cpu": bench::cpu(), "memory": bench::memory(), "disk": bench::disk() })
}

/// Apply a whole profile. Optionally benchmark before/after.
/// Per-tweak failures don't abort the rest, they're reported.
pub fn apply(profile_id: &str, with_bench: bool) -> Result<Value, String> {
    let p = PROFILES
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| format!("unbekanntes Profil '{profile_id}'"))?;

    // A failed restore point does not abort (every tweak is still journaled
    // and individually undoable), but it is flagged prominently.
    let (restore_msg, restore_failed) =
        match safety::create_restore_point(&format!("AD HyperOptimize Profil: {}", p.name)) {
            Ok(m) => (m, false),
            Err(e) => (format!("Restore Point FEHLGESCHLAGEN: {e}"), true),
        };

    // Hardware verdicts are only needed (and only computed) when the profile
    // contains a hardware-gated tweak.
    let tweak_risks = if p.tweaks.iter().any(|t| HW_GATED.contains(t)) {
        crate::hwprofile::hw_profile()["tweakRisks"].clone()
    } else {
        Value::Null
    };

    let before = if with_bench {
        Some(bench_suite())
    } else {
        None
    };

    let status = tweak_status_map();
    let mut applied = Vec::new();
    let mut skipped = Vec::new();
    let mut failed = Vec::new();
    let mut hw_skipped = Vec::new();
    for id in p.tweaks {
        if let Some(reason) = hw_skip_reason(id, &tweak_risks) {
            hw_skipped.push(json!({ "id": id, "reason": reason }));
            continue;
        }
        match status.get(*id).map(|s| s.1.as_str()) {
            Some("applied") => skipped.push(json!(id)),
            _ => match tweaks::apply(id) {
                Ok(result) => {
                    if let Some(entry_id) = result["entryId"].as_str() {
                        let tag = safety::with_journal(|journal| {
                            let entry = journal
                                .iter_mut()
                                .find(|entry| entry.id == entry_id)
                                .ok_or("profile entry missing from journal")?;
                            entry.origin = Some(format!("profile:{}", p.id));
                            Ok(())
                        });
                        match tag {
                            Ok(()) => applied.push(json!(id)),
                            Err(e) => {
                                let rollback = tweaks::revert_entry(entry_id).err();
                                failed.push(
                                    json!({ "id": id, "error": e, "rollbackError": rollback }),
                                );
                            }
                        }
                    } else {
                        failed.push(json!({ "id": id, "error": "missing journal entry ID" }));
                    }
                }
                Err(e) => failed.push(json!({ "id": id, "error": e })),
            },
        }
    }

    let after = if with_bench {
        Some(bench_suite())
    } else {
        None
    };

    // Every attempted tweak failed: that is an error, not a result.
    if applied.is_empty() && !failed.is_empty() {
        return Err(format!(
            "Profil '{}' fehlgeschlagen: alle {} Tweaks sind fehlgeschlagen ({}). {}",
            p.id,
            failed.len(),
            failed
                .iter()
                .map(|f| format!(
                    "{}: {}",
                    f["id"].as_str().unwrap_or("?"),
                    f["error"].as_str().unwrap_or("?")
                ))
                .collect::<Vec<_>>()
                .join("; "),
            restore_msg
        ));
    }

    Ok(json!({
        "profile": p.id,
        "restorePoint": restore_msg,
        "restorePointFailed": restore_failed,
        "warning": if restore_failed {
            Some("Kein Wiederherstellungspunkt erstellt: Änderungen sind nur über die Undo-Historie rückgängig zu machen.")
        } else {
            None
        },
        "applied": applied,
        "skipped": skipped,
        "hwSkipped": hw_skipped,
        "failed": failed,
        "benchBefore": before,
        "benchAfter": after,
        "benchNote": "Synthetische Benchmarks zeigen Scheduler-/Latenz-Tweaks kaum, der echte Effekt zeigt sich in Frametimes im Spiel. Disk/CPU-Deltas unter ~3% sind Messrauschen.",
    }))
}

/// Revert only journal entries written by this profile.
pub fn revert(profile_id: &str) -> Result<Value, String> {
    let p = PROFILES
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| format!("unbekanntes Profil '{profile_id}'"))?;
    let mut reverted = Vec::new();
    let mut failed = Vec::new();
    let origin = format!("profile:{}", p.id);
    let entries: Vec<_> = safety::load_journal()?
        .into_iter()
        .filter(|entry| entry.origin.as_deref() == Some(origin.as_str()) && !entry.reverted)
        .map(|entry| (entry.id, entry.tweak_id))
        .collect();
    for (entry_id, tweak_id) in entries.iter().rev() {
        match tweaks::revert_entry(entry_id) {
            Ok(_) => reverted.push(json!(tweak_id)),
            Err(e) => failed.push(json!({ "id": tweak_id, "error": e })),
        }
    }
    Ok(json!({ "profile": p.id, "reverted": reverted, "skipped": [], "failed": failed }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removed_tweaks_are_not_in_default_profiles() {
        for p in PROFILES {
            for bad in ["fse_optimizations_off", "background_apps_off"] {
                assert!(!p.tweaks.contains(&bad), "{} contains {bad}", p.id);
            }
        }
    }

    #[test]
    fn hw_gate_only_skips_gated_tweaks_with_a_verdict() {
        let risks = json!({
            "priority_separation": { "severity": "warning", "title": "Low core count" },
            "hags_on": { "severity": "warning", "title": "Old GPU" },
        });
        assert!(hw_skip_reason("priority_separation", &risks).is_some());
        assert!(hw_skip_reason("mmcss_games_task", &risks).is_none());
        // Not gated: profiles never consult the verdict for it.
        assert!(hw_skip_reason("hags_on", &risks).is_none());
        assert!(hw_skip_reason("power_throttling_off", &Value::Null).is_none());
    }
}
