//! Per-game profile auto-switcher.
//! Polls running processes every 3 s; on game detect → applies power plan.
//! On game exit → reverts to the plan that was active before.

use crate::gamedb::{self, Game};
use crate::ps;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;
use tauri::Emitter;

/// Guards against `start()` spawning more than one polling thread
/// (e.g. if app setup ever runs twice, or a future reload path calls it again).
static STARTED: AtomicBool = AtomicBool::new(false);

/// Lock helper that recovers from a poisoned mutex instead of panicking.
/// A panic while some other holder was mutating state must not permanently
/// brick every other command that touches `SwitcherState`.
fn lock(state: &SharedState) -> MutexGuard<'_, SwitcherState> {
    state.lock().unwrap_or_else(|e| e.into_inner())
}

// ── Power-plan GUIDs ─────────────────────────────────────────────────────────
pub(crate) const PLAN_BALANCED:          &str = "381b4222-f694-41f0-9685-ff5bb260df2e";
pub(crate) const PLAN_HIGH_PERFORMANCE:  &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";
const PLAN_ULTIMATE:          &str = "e9a42b02-d5df-448d-aa00-03f14749eb61";

// ── Shared state ─────────────────────────────────────────────────────────────
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SwitcherState {
    pub enabled:        bool,
    pub default_preset: String, // "performance" | "balanced" | "quality"
    pub active_game:    Option<String>, // game id
    pub prev_plan_guid: Option<String>, // power plan before game launched
}

pub type SharedState = Arc<Mutex<SwitcherState>>;

pub fn new_state() -> SharedState {
    Arc::new(Mutex::new(SwitcherState {
        enabled:        false,
        default_preset: "performance".into(),
        active_game:    None,
        prev_plan_guid: None,
    }))
}

// ── Background polling thread ────────────────────────────────────────────────
pub fn start(state: SharedState, app: tauri::AppHandle) {
    if STARTED.swap(true, Ordering::SeqCst) {
        return; // already running — never spawn a second poll loop
    }
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(3));

            let (enabled, current_game, preset) = {
                let s = lock(&state);
                (s.enabled, s.active_game.clone(), s.default_preset.clone())
            };
            if !enabled { continue; }

            let running = get_process_names();

            // Detect a known game in the running process list
            let detected = gamedb::get_all().iter().find(|g| {
                g.processes.iter().any(|p| running.contains(&p.to_string()))
            });

            match (detected, current_game) {
                // New game launched
                (Some(game), None) => {
                    let prev = get_active_plan_guid();
                    {
                        let mut s = lock(&state);
                        s.active_game    = Some(game.id.to_string());
                        s.prev_plan_guid = prev.clone();
                    }
                    apply_power_plan_for_preset(game, &preset);
                    let _ = app.emit("game-detected", json!({
                        "id": game.id, "name": game.name,
                        "preset": preset,
                        "powerPlan": plan_label_for_preset(game, &preset),
                    }));
                }

                // Active game no longer running — revert
                (None, Some(id)) => {
                    {
                        let mut s = lock(&state);
                        s.active_game = None;
                    }
                    revert_power_plan(&state);
                    let _ = app.emit("game-exited", json!({ "id": id }));
                }

                // Different game detected (switched without closing first)
                (Some(game), Some(ref cid)) if game.id != cid.as_str() => {
                    {
                        let mut s = lock(&state);
                        s.active_game = Some(game.id.to_string());
                        // keep prev_plan_guid from original game launch
                    }
                    apply_power_plan_for_preset(game, &preset);
                    let _ = app.emit("game-detected", json!({
                        "id": game.id, "name": game.name,
                        "preset": preset,
                        "powerPlan": plan_label_for_preset(game, &preset),
                    }));
                }

                _ => {}
            }
        }
    });
}

// ── Commands ─────────────────────────────────────────────────────────────────

/// Return all games in the database.
pub fn cmd_game_list() -> Value {
    json!(gamedb::get_all())
}

/// Return current switcher status.
pub fn cmd_game_switcher_status(state: &SharedState) -> Value {
    let s = lock(state);
    json!({
        "enabled":       s.enabled,
        "defaultPreset": s.default_preset,
        "activeGame":    s.active_game,
    })
}

/// Enable/disable auto-switcher and optionally change default preset.
pub fn cmd_game_switcher_configure(
    state: &SharedState,
    enabled: bool,
    default_preset: String,
) -> Value {
    let mut s = lock(state);
    s.enabled        = enabled;
    s.default_preset = default_preset;
    json!({ "ok": true })
}

/// Manually apply a preset for a specific game.
pub fn cmd_game_apply_preset(game_id: String, preset: String) -> Value {
    match gamedb::get_all().iter().find(|g| g.id == game_id.as_str()) {
        None => json!({ "error": format!("Unknown game: {game_id}") }),
        Some(game) => {
            apply_power_plan_for_preset(game, &preset);
            let label = plan_label_for_preset(game, &preset);
            json!({ "ok": true, "powerPlan": label, "game": game.name })
        }
    }
}

/// Manually revert to balanced power plan.
pub fn cmd_game_revert(state: &SharedState) -> Value {
    revert_power_plan(state);
    let mut s = lock(state);
    s.active_game = None;
    json!({ "ok": true })
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn get_process_names() -> Vec<String> {
    let Ok(out) = std::process::Command::new("tasklist")
        .args(["/fo", "csv", "/nh"])
        .output() else { return vec![]; };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            line.split(',')
                .next()
                .map(|s| s.trim_matches('"').to_lowercase())
        })
        .collect()
}

pub(crate) fn get_active_plan_guid() -> Option<String> {
    let out = ps::run("powercfg /getactivescheme").ok()?;
    // Output: "Power Scheme GUID: xxxxxxxx-xxxx-... (Name)"
    out.split_whitespace()
        .find(|s| s.contains('-') && s.len() == 36)
        .map(|s| s.to_string())
}

fn set_power_plan(guid: &str) {
    // Try primary GUID; if Ultimate isn't provisioned, fall back to High Performance
    let result = std::process::Command::new("powercfg")
        .args(["/setactive", guid])
        .status();
    if result.map(|s| !s.success()).unwrap_or(true) && guid == PLAN_ULTIMATE {
        let _ = std::process::Command::new("powercfg")
            .args(["/setactive", PLAN_HIGH_PERFORMANCE])
            .status();
    }
}

fn apply_power_plan_for_preset(game: &Game, preset: &str) {
    let plan_key = match preset {
        "balanced" => game.presets.balanced.power_plan,
        "quality"  => game.presets.quality.power_plan,
        _          => game.presets.performance.power_plan,
    };
    let guid = match plan_key {
        "ultimate"         => PLAN_ULTIMATE,
        "high_performance" => PLAN_HIGH_PERFORMANCE,
        _                  => PLAN_BALANCED,
    };
    set_power_plan(guid);
}

fn revert_power_plan(state: &SharedState) {
    let guid = {
        let s = lock(state);
        s.prev_plan_guid.clone().unwrap_or_else(|| PLAN_BALANCED.to_string())
    };
    set_power_plan(&guid);
}

fn plan_label_for_preset(game: &Game, preset: &str) -> &'static str {
    match preset {
        "balanced" => game.presets.balanced.power_plan,
        "quality"  => game.presets.quality.power_plan,
        _          => game.presets.performance.power_plan,
    }
}
