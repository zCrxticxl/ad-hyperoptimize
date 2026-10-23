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
pub(crate) const PLAN_BALANCED: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";
pub(crate) const PLAN_HIGH_PERFORMANCE: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";

// ── Shared state ─────────────────────────────────────────────────────────────
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SwitcherState {
    pub enabled: bool,
    pub default_preset: String, // "performance" | "balanced" | "quality"
    pub active_game: Option<String>, // game id
    pub prev_plan_guid: Option<String>, // power plan before game launched
}

pub type SharedState = Arc<Mutex<SwitcherState>>;

pub fn new_state() -> SharedState {
    Arc::new(Mutex::new(SwitcherState {
        enabled: false,
        default_preset: "performance".into(),
        active_game: None,
        prev_plan_guid: None,
    }))
}

// ── Background polling thread ────────────────────────────────────────────────
pub fn start(state: SharedState, app: tauri::AppHandle) {
    if STARTED.swap(true, Ordering::SeqCst) {
        return; // already running, never spawn a second poll loop
    }
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(3));

            let (enabled, current_game, preset) = {
                let s = lock(&state);
                (s.enabled, s.active_game.clone(), s.default_preset.clone())
            };
            if !enabled {
                continue;
            }

            let running = get_process_names();

            // Detect a known game in the running process list
            let detected = gamedb::get_all()
                .iter()
                .find(|g| g.processes.iter().any(|p| running.contains(&p.to_string())));

            match (detected, current_game) {
                // New game launched
                (Some(game), None) => {
                    {
                        let mut s = lock(&state);
                        s.active_game = Some(game.id.to_string());
                    }
                    let result = record_prev_plan(Some(&state))
                        .and_then(|_| apply_power_plan_for_preset(game, &preset));
                    emit_detected(&app, game, &preset, result);
                }

                // Active game no longer running, revert
                (None, Some(id)) => {
                    {
                        let mut s = lock(&state);
                        s.active_game = None;
                    }
                    let r = revert_power_plan(&state);
                    let _ = app.emit("game-exited", json!({ "id": id, "error": r.err() }));
                }

                // Different game detected (switched without closing first)
                (Some(game), Some(ref cid)) if game.id != cid.as_str() => {
                    {
                        let mut s = lock(&state);
                        s.active_game = Some(game.id.to_string());
                        // keep the previous plan recorded at the original launch
                    }
                    let result = apply_power_plan_for_preset(game, &preset);
                    emit_detected(&app, game, &preset, result);
                }

                _ => {}
            }
        }
    });
}

fn emit_detected(
    app: &tauri::AppHandle,
    game: &Game,
    preset: &str,
    result: Result<String, String>,
) {
    let (plan, error) = match result {
        Ok(label) => (label, None),
        Err(e) => (plan_label_for_preset(game, preset).to_string(), Some(e)),
    };
    let _ = app.emit(
        "game-detected",
        json!({
            "id": game.id, "name": game.name,
            "preset": preset,
            "powerPlan": plan,
            "error": error,
        }),
    );
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
    s.enabled = enabled;
    s.default_preset = default_preset;
    json!({ "ok": true })
}

/// Manually apply a preset for a specific game. The plan active before the
/// first manual/automatic apply is recorded (and persisted) so revert restores
/// the user's real plan rather than a guessed Balanced.
pub fn cmd_game_apply_preset(game_id: String, preset: String) -> Value {
    match gamedb::get_all().iter().find(|g| g.id == game_id.as_str()) {
        None => json!({ "error": format!("Unknown game: {game_id}") }),
        Some(game) => {
            match record_prev_plan(None).and_then(|_| apply_power_plan_for_preset(game, &preset)) {
                Ok(label) => json!({ "ok": true, "powerPlan": label, "game": game.name }),
                Err(e) => json!({ "ok": false, "error": e, "game": game.name }),
            }
        }
    }
}

/// Manually revert to the power plan that was active before the preset was
/// applied. With nothing recorded this is an honest no-op.
pub fn cmd_game_revert(state: &SharedState) -> Value {
    let r = revert_power_plan(state);
    let mut s = lock(state);
    s.active_game = None;
    match r {
        Ok(Some(guid)) => json!({ "ok": true, "restoredPlan": guid }),
        Ok(None) => {
            json!({ "ok": true, "note": "no previous power plan recorded; nothing to revert" })
        }
        Err(e) => json!({ "ok": false, "error": e }),
    }
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn get_process_names() -> Vec<String> {
    // ps::exec: CREATE_NO_WINDOW + timeout — a raw Command here flashed a
    // console window every poll interval.
    let Ok(out) = ps::exec("tasklist.exe", &["/fo", "csv", "/nh"]) else {
        return vec![];
    };
    out.lines()
        .filter_map(|line| {
            let first = if let Some(stripped) = line.strip_prefix('"') {
                // tasklist /fo csv quotes fields containing commas
                stripped.split('"').next()
            } else {
                line.split(',').next()
            };
            first.map(|s| s.trim_matches('"').to_lowercase())
        })
        .collect()
}

pub(crate) fn get_active_plan_guid() -> Option<String> {
    let out = ps::run("powercfg /getactivescheme").ok()?;
    // Output: "Power Scheme GUID: xxxxxxxx-xxxx-... (Name)", the header is
    // localized; the GUID itself is not.
    out.split_whitespace()
        .find(|s| ps::is_guid(s))
        .map(|s| s.to_string())
}

/// All scheme GUIDs listed by `powercfg /list` (header/names are localized,
/// GUIDs are not).
fn parse_listed_guids(out: &str) -> Vec<String> {
    out.split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == '*')
        .filter(|t| ps::is_guid(t))
        .map(|t| t.to_ascii_lowercase())
        .collect()
}

/// Pick an installed Ultimate Performance scheme. `registry` holds
/// `guid|FriendlyName` lines from HKLM\...\Power\User\PowerSchemes. The
/// FriendlyName of every Ultimate scheme (the hidden template e9a42b02… and
/// each `-duplicatescheme` copy, which gets its own GUID) is the
/// locale-independent resource reference `@…\powrprof.dll,-19,…`. Only a
/// scheme that `powercfg /list` actually lists is installed/activatable.
fn pick_ultimate(listed: &[String], registry: &str) -> Option<String> {
    registry.lines().find_map(|line| {
        let (guid, friendly) = line.trim().split_once('|')?;
        let guid = guid.trim().to_ascii_lowercase();
        let is_ultimate = friendly.to_ascii_lowercase().contains("powrprof.dll,-19,");
        (ps::is_guid(&guid) && is_ultimate && listed.contains(&guid)).then_some(guid)
    })
}

/// GUID of an installed Ultimate Performance scheme, or None when the user
/// has never provisioned one (the template GUID itself is not activatable).
pub(crate) fn resolve_ultimate_plan() -> Option<String> {
    let listed = parse_listed_guids(&ps::exec("powercfg.exe", &["/list"]).ok()?);
    let registry = ps::run(
        "Get-ChildItem 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\Power\\User\\PowerSchemes' -ErrorAction SilentlyContinue | ForEach-Object { $_.PSChildName + '|' + (Get-ItemProperty -LiteralPath $_.PSPath -ErrorAction SilentlyContinue).FriendlyName }",
    )
    .ok()?;
    pick_ultimate(&listed, &registry)
}

fn set_power_plan(guid: &str) -> Result<(), String> {
    ps::exec("powercfg.exe", &["/setactive", guid])
        .map(|_| ())
        .map_err(|e| format!("powercfg /setactive {guid} failed: {}", e.trim()))
}

/// Apply the preset's plan; returns the label of the plan actually applied.
fn apply_power_plan_for_preset(game: &Game, preset: &str) -> Result<String, String> {
    let (guid, label) = match plan_label_for_preset(game, preset) {
        "ultimate" => match resolve_ultimate_plan() {
            Some(g) => (g, "ultimate".to_string()),
            None => (
                PLAN_HIGH_PERFORMANCE.to_string(),
                "high_performance (Ultimate Performance is not installed)".to_string(),
            ),
        },
        "high_performance" => (PLAN_HIGH_PERFORMANCE.to_string(), "high_performance".into()),
        _ => (PLAN_BALANCED.to_string(), "balanced".into()),
    };
    set_power_plan(&guid)?;
    note_applied_plan(&guid);
    Ok(label)
}

// ── Previous-plan persistence ────────────────────────────────────────────────
// The plan active before the first game apply is written to disk, so a crash
// or restart while a game plan is active never strands the user on High
// Performance: revert reads it back.

fn prev_plan_path() -> std::path::PathBuf {
    crate::safety::app_data_dir()
        .join("gameprofile")
        .join("prev-plan.txt")
}

/// The record is `<previous plan>` or `<previous plan>|<plan the app applied>`.
/// Knowing the applied plan lets a stale record be detected: if the active
/// plan is no longer the one the app set, the user changed it since, and
/// restoring the old record would silently override their choice.
fn load_prev_record() -> Option<(String, Option<String>)> {
    let raw = std::fs::read_to_string(prev_plan_path()).ok()?;
    parse_prev_record(&raw)
}

fn parse_prev_record(raw: &str) -> Option<(String, Option<String>)> {
    let mut parts = raw.trim().splitn(2, '|');
    let prev = parts.next()?.trim().to_string();
    if !ps::is_guid(&prev) {
        return None;
    }
    let applied = parts
        .next()
        .map(|a| a.trim().to_string())
        .filter(|a| ps::is_guid(a));
    Some((prev, applied))
}

fn write_prev_record(prev: &str, applied: Option<&str>) -> Result<(), String> {
    let path = prev_plan_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let body = match applied {
        Some(a) => format!("{prev}|{a}"),
        None => prev.to_string(),
    };
    std::fs::write(&path, body).map_err(|e| format!("saving previous plan failed: {e}"))
}

/// True when a record exists whose applied plan is no longer active.
fn record_is_stale(applied: Option<&str>, active: Option<&str>) -> bool {
    matches!((applied, active), (Some(a), Some(now)) if !a.eq_ignore_ascii_case(now))
}

fn note_applied_plan(guid: &str) {
    if let Some((prev, _)) = load_prev_record() {
        let _ = write_prev_record(&prev, Some(guid));
    }
}

fn clear_prev_plan() {
    let _ = std::fs::remove_file(prev_plan_path());
}

/// Record the currently active plan as the one to restore, unless one is
/// already recorded (a second apply must not overwrite the user's original
/// plan with an already-applied game plan). A stale record (the user switched
/// plans after the app's apply) is replaced by the current plan.
fn record_prev_plan(state: Option<&SharedState>) -> Result<(), String> {
    let active = get_active_plan_guid();
    let prev = match load_prev_record() {
        Some((g, applied)) if !record_is_stale(applied.as_deref(), active.as_deref()) => g,
        _ => {
            let g = active.ok_or("could not read the active power plan")?;
            write_prev_record(&g, None)?;
            g
        }
    };
    if let Some(state) = state {
        lock(state).prev_plan_guid = Some(prev);
    }
    Ok(())
}

/// Restore the recorded previous plan. Ok(None) when nothing was recorded, or
/// when the user switched plans after the app's apply (their choice wins).
/// The record is only cleared after the restore succeeded.
fn revert_power_plan(state: &SharedState) -> Result<Option<String>, String> {
    let record = load_prev_record();
    if let Some((_, applied)) = &record {
        if record_is_stale(applied.as_deref(), get_active_plan_guid().as_deref()) {
            lock(state).prev_plan_guid = None;
            clear_prev_plan();
            return Ok(None);
        }
    }
    let recorded = lock(state)
        .prev_plan_guid
        .clone()
        .filter(|g| ps::is_guid(g))
        .or(record.map(|(g, _)| g));
    let Some(guid) = recorded else {
        return Ok(None);
    };
    set_power_plan(&guid)?;
    lock(state).prev_plan_guid = None;
    clear_prev_plan();
    Ok(Some(guid))
}

fn plan_label_for_preset(game: &Game, preset: &str) -> &'static str {
    match preset {
        "balanced" => game.presets.balanced.power_plan,
        "quality" => game.presets.quality.power_plan,
        _ => game.presets.performance.power_plan,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST_DE: &str = "Bestehende Energieschemen (* Aktiv)\n-----------------------------------\nGUID des Energieschemas: 381b4222-f694-41f0-9685-ff5bb260df2e  (Ausbalanciert)\nGUID des Energieschemas: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c  (Höchstleistung) *\nGUID des Energieschemas: d1a87a2d-32c1-4791-aad7-5902b0d8a2d5  (Ultimative Leistung)\n";

    const REGISTRY: &str = "381b4222-f694-41f0-9685-ff5bb260df2e|@C:\\WINDOWS\\system32\\powrprof.dll,-15,Balanced\n8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c|@C:\\WINDOWS\\system32\\powrprof.dll,-13,High performance\nd1a87a2d-32c1-4791-aad7-5902b0d8a2d5|@C:\\WINDOWS\\system32\\powrprof.dll,-19,Ultimate Performance\ne9a42b02-d5df-448d-aa00-03f14749eb61|@C:\\WINDOWS\\system32\\powrprof.dll,-19,Ultimate Performance\n";

    #[test]
    fn prev_record_parsing_and_staleness() {
        let hp = PLAN_HIGH_PERFORMANCE;
        let bal = PLAN_BALANCED;
        assert_eq!(parse_prev_record(bal), Some((bal.to_string(), None)));
        assert_eq!(
            parse_prev_record(&format!(
                "{bal}|{hp}
"
            )),
            Some((bal.to_string(), Some(hp.to_string())))
        );
        assert_eq!(parse_prev_record("garbage"), None);
        // The app's plan is still active: not stale, restore is correct.
        assert!(!record_is_stale(Some(hp), Some(hp)));
        // The user switched plans after the apply: stale, do not restore.
        assert!(record_is_stale(Some(hp), Some(bal)));
        // Legacy record without the applied plan: keep previous behavior.
        assert!(!record_is_stale(None, Some(bal)));
    }

    #[test]
    fn parses_guids_from_localized_list() {
        let g = parse_listed_guids(LIST_DE);
        assert_eq!(g.len(), 3);
        assert!(g.contains(&PLAN_HIGH_PERFORMANCE.to_string()));
    }

    #[test]
    fn picks_installed_ultimate_copy_not_template() {
        let listed = parse_listed_guids(LIST_DE);
        assert_eq!(
            pick_ultimate(&listed, REGISTRY).as_deref(),
            Some("d1a87a2d-32c1-4791-aad7-5902b0d8a2d5")
        );
    }

    #[test]
    fn no_ultimate_when_only_template_exists() {
        let listed = vec![PLAN_BALANCED.to_string(), PLAN_HIGH_PERFORMANCE.to_string()];
        assert_eq!(pick_ultimate(&listed, REGISTRY), None);
        assert_eq!(pick_ultimate(&listed, "garbage|x\n|\n"), None);
    }
}
