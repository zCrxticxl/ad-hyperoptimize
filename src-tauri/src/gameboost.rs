//! Game Booster, one-click system tuning when launching a game.
//! Kills background bloat, boosts target process, reverts everything after.

use crate::gameprofile::{get_active_plan_guid, resolve_ultimate_plan, PLAN_HIGH_PERFORMANCE};
use crate::ps;
use crate::safety::{self, ChangeItem, JournalEntry};
use crate::tweaks;
use serde_json::{json, Value};

// Processes considered safe to terminate during gaming. Matched by EXACT
// process name (case-insensitive, without ".exe"), never by substring: a
// substring match on "msedge" hit msedgewebview2 (this app's own WebView2),
// and "upc" killed Ubisoft Connect, which Rainbow Six Siege needs to run.
// Game launchers, anti-cheat, and voice/call apps (Discord, Teams, Zoom,
// Skype) are deliberately absent: killing them breaks the game or the
// user's voice chat.
const KILL_CANDIDATES: &[&str] = &[
    "Spotify",
    "OneDrive",
    "GoogleDriveFS",
    "Dropbox",
    "slack",
    "chrome",
    "msedge",
    "firefox",
    "brave",
    "opera",
    "AdobeUpdater",
    "AdobeIPCBroker",
    "Creative Cloud",
    "SearchApp",
    "SearchHost",
    "Widgets",
    "WidgetService",
    "PhoneExperienceHost",
    "YourPhone",
    "WinStore.App",
    "MicrosoftEdgeUpdate",
];

/// Processes that are never terminated, even if a future candidate list or a
/// frontend-supplied PID points at them: this app's WebView2 host, the audio
/// engine, and the anti-cheat services games require.
const PROTECTED_PROCS: &[&str] = &[
    "msedgewebview2",
    "audiodg",
    "BEService",
    "BEService_x64",
    "BEDaisy",
    "vgc",
    "vgtray",
    "vgk",
];

/// True for a process name that must never be killed. EasyAntiCheat ships
/// several executables (EasyAntiCheat, EasyAntiCheat_EOS, …), so it is
/// matched by prefix.
fn is_protected_proc(name: &str) -> bool {
    let base = strip_exe(name);
    base.to_ascii_lowercase().starts_with("easyanticheat")
        || PROTECTED_PROCS.iter().any(|p| p.eq_ignore_ascii_case(base))
}

fn strip_exe(name: &str) -> &str {
    let n = name.trim();
    if n.len() > 4 && n[n.len() - 4..].eq_ignore_ascii_case(".exe") {
        &n[..n.len() - 4]
    } else {
        n
    }
}

/// Exact (case-insensitive) candidate match that also honours the protected
/// list.
fn is_kill_candidate(name: &str) -> bool {
    let base = strip_exe(name);
    !is_protected_proc(base) && KILL_CANDIDATES.iter().any(|c| c.eq_ignore_ascii_case(base))
}

pub fn list_background_procs() -> Value {
    let candidates_json = KILL_CANDIDATES
        .iter()
        .map(|s| format!("'{s}'"))
        .collect::<Vec<_>>()
        .join(",");

    // -contains is an exact, case-insensitive equality test (no wildcards).
    let script = format!(
        r#"
$candidates = @({candidates_json})
$procs = Get-Process -ErrorAction SilentlyContinue |
    Where-Object {{ $candidates -contains $_.Name }} |
    Select-Object Id, Name,
        @{{n='memMb';e={{[math]::Round($_.WorkingSet64/1MB,1)}}}}
$procs | ConvertTo-Json -Compress -Depth 2
"#
    );

    let arr = match ps::run_json(&script) {
        Ok(Value::Array(arr)) => arr,
        Ok(v @ Value::Object(_)) => vec![v],
        _ => vec![],
    };
    // Defence in depth: re-apply the exact match + protected list in Rust.
    let arr: Vec<Value> = arr
        .into_iter()
        .filter(|p| p["Name"].as_str().is_some_and(is_kill_candidate))
        .collect();
    json!({ "procs": arr })
}

pub fn list_running_games() -> Value {
    // Look for processes that look like games (large working set, GPU user, not system)
    let script = r#"
$procs = Get-Process -ErrorAction SilentlyContinue |
    Where-Object {
        $_.WorkingSet64 -gt 200MB -and
        $_.MainWindowTitle -ne '' -and
        $_.Name -notmatch '^(svchost|explorer|dwm|winlogon|csrss|lsass|services|wininit|audiodg|RuntimeBroker|ShellExperienceHost|SearchIndexer|MsMpEng)$'
    } |
    Select-Object Id, Name, MainWindowTitle,
        @{n='memMb';e={[math]::Round($_.WorkingSet64/1MB,0)}},
        @{n='priority';e={$_.PriorityClass.ToString()}}
$procs | ConvertTo-Json -Compress -Depth 2
"#;
    match ps::run_json(script) {
        Ok(Value::Array(arr)) => json!({ "games": arr }),
        Ok(v @ Value::Object(_)) => json!({ "games": [v] }),
        _ => json!({ "games": [] }),
    }
}

/// Terminate frontend-selected background processes. The PIDs are NOT
/// trusted: each one is re-resolved to its process name on the backend and
/// only killed when that name exactly matches a kill candidate and is not
/// protected. The kill itself re-checks the name, so a PID reused between
/// the lookup and the kill is never hit. Returns one line per PID.
pub fn kill_background(pids: Vec<u32>) -> Result<String, String> {
    if pids.is_empty() {
        return Ok("Nothing to kill".into());
    }
    let pid_list = pids
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let names = ps::run_json(&format!(
        r#"
@({pid_list}) | ForEach-Object {{
    $p = Get-Process -Id $_ -ErrorAction SilentlyContinue
    [pscustomobject]@{{ Id = $_; Name = $(if ($p) {{ $p.Name }} else {{ '' }}) }}
}}
"#
    ))?;
    let rows = match names {
        Value::Array(a) => a,
        v @ Value::Object(_) => vec![v],
        _ => vec![],
    };

    let mut results = Vec::new();
    let mut killed = 0usize;
    for pid in &pids {
        let name = rows
            .iter()
            .find(|r| r["Id"].as_u64() == Some(u64::from(*pid)))
            .and_then(|r| r["Name"].as_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            results.push(format!("{pid}: not running"));
            continue;
        }
        if is_protected_proc(&name) {
            results.push(format!("{pid} ({name}): refused, protected process"));
            continue;
        }
        if !is_kill_candidate(&name) {
            results.push(format!(
                "{pid} ({name}): refused, not a background candidate"
            ));
            continue;
        }
        // `name` passed is_kill_candidate, so it is one of the constant
        // candidate names (safe to embed in a single-quoted PS string).
        let r = ps::run(&format!(
            "$p = Get-Process -Id {pid} -ErrorAction Stop; if ($p.Name -ne '{name}') {{ throw 'PID was reused by another process' }}; Stop-Process -Id {pid} -Force -ErrorAction Stop"
        ));
        match r {
            Ok(_) => {
                killed += 1;
                results.push(format!("{pid} ({name}): terminated"));
            }
            Err(e) => results.push(format!("{pid} ({name}): failed: {}", e.trim())),
        }
    }
    Ok(format!(
        "{killed} of {} processes terminated\n{}",
        pids.len(),
        results.join("\n")
    ))
}

fn boost_state_path() -> Result<std::path::PathBuf, String> {
    crate::safety::state_dir().map(|d| d.join("gameboost").join("boost-state.json"))
}

/// Activate `guid` via powercfg; a non-zero exit is a real error.
fn set_active_plan(guid: &str) -> Result<(), String> {
    ps::exec("powercfg.exe", &["/setactive", guid])
        .map(|_| ())
        .map_err(|e| format!("powercfg /setactive {guid} failed: {}", e.trim()))
}

pub fn boost_start(pid: u32) -> Result<String, String> {
    // A second boost must not overwrite the saved pre-boost state with
    // already-boosted values (the stop would then "restore" the boost).
    if boost_state_path()?.exists() {
        return Err("a boost is already active; stop it first".into());
    }

    // Capture the pre-boost state so boost_stop can restore it exactly
    // instead of forcing the hardcoded Balanced plan (H9).
    let prev_plan =
        get_active_plan_guid().ok_or("could not read the active power plan; boost not started")?;
    let prev_toast = ps::run(
        "(Get-ItemProperty 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Notifications\\Settings' -Name NOC_GLOBAL_SETTING_TOASTS_ENABLED -ErrorAction SilentlyContinue).NOC_GLOBAL_SETTING_TOASTS_ENABLED",
    )
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty());
    // Game Mode is switched on below; its previous value (None = not set,
    // the Windows default) is restored on stop.
    let prev_game_mode = match tweaks::reg_read("HKCU", GAME_BAR_KEY, "AutoGameModeEnabled")? {
        None => None,
        Some(crate::safety::RegVal::Dword(v)) => Some(v),
        Some(_) => return Err("unexpected AutoGameModeEnabled type; boost not started".into()),
    };

    // Write-ahead: persist the pre-boost state BEFORE changing anything, so
    // a partial apply (or a crash mid-apply) is still restorable by stop.
    save_boost_state(&prev_plan, prev_toast.as_deref(), prev_game_mode)?;

    // Power plan: an installed Ultimate Performance scheme if present,
    // otherwise High Performance. A failure here aborts and restores.
    let (target_plan, plan_label) = match resolve_ultimate_plan() {
        Some(g) => (g, "Ultimate Performance"),
        None => (PLAN_HIGH_PERFORMANCE.to_string(), "High Performance"),
    };
    if let Err(e) = set_active_plan(&target_plan) {
        return Err(match boost_stop() {
            Ok(_) => format!("Game boost failed ({e}); settings restored"),
            Err(re) => format!("Game boost failed ({e}); restore incomplete ({re})"),
        });
    }

    let script = format!(
        r#"
$skipped = @()

# Boost target process. Anti-cheat protected games (BattlEye, EAC, Vanguard)
# deny priority changes; that is a skipped step, not a failed boost.
try {{
    $p = Get-Process -Id {pid} -EA Stop
    $p.PriorityClass = [System.Diagnostics.ProcessPriorityClass]::High
}} catch {{ $skipped += "priority ($($_.Exception.Message))" }}

# Enable Game Mode
try {{
    Set-ItemProperty 'HKCU:\Software\Microsoft\GameBar' AutoGameModeEnabled 1 -Type DWord -EA Stop
}} catch {{ $skipped += "Game Mode" }}

# Disable notifications
try {{
    Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings' NOC_GLOBAL_SETTING_TOASTS_ENABLED 0 -Type DWord -EA Stop
}} catch {{ $skipped += "notifications" }}

if ($skipped) {{ "Game boost active for PID {pid} ({plan_label}); skipped: " + ($skipped -join '; ') }}
else {{ "Game boost active for PID {pid} ({plan_label})" }}
"#
    );
    match ps::run(&script) {
        Ok(out) => Ok(out.trim().to_string()),
        Err(e) => Err(match boost_stop() {
            Ok(_) => format!("Game boost failed ({}); settings restored", e.trim()),
            Err(re) => format!(
                "Game boost failed ({}); restore incomplete ({re})",
                e.trim()
            ),
        }),
    }
}

const GAME_BAR_KEY: &str = r"Software\Microsoft\GameBar";

/// Persist the pre-boost state for exact restore in boost_stop.
fn save_boost_state(
    prev_plan: &str,
    prev_toast: Option<&str>,
    prev_game_mode: Option<u32>,
) -> Result<(), String> {
    let path = boost_state_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let v =
        json!({ "prevPlan": prev_plan, "prevToast": prev_toast, "prevGameMode": prev_game_mode });
    std::fs::write(&path, serde_json::to_string(&v).unwrap_or_default())
        .map_err(|e| format!("saving boost state failed: {e}"))
}

/// Pre-boost state as saved by boost_start.
struct BoostState {
    plan: String,
    toast: Option<String>,
    /// None: not recorded (state written by an older version, leave Game
    /// Mode alone); Some(None): the value did not exist; Some(Some(v)): v.
    game_mode: Option<Option<u32>>,
}

fn parse_boost_state(v: &Value) -> BoostState {
    BoostState {
        plan: v["prevPlan"].as_str().unwrap_or_default().to_string(),
        toast: v["prevToast"].as_str().map(str::to_string),
        game_mode: match v.get("prevGameMode") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()).map(Some),
        },
    }
}

/// Read (without consuming) the pre-boost state; `None` when no state was
/// saved or the file is unreadable.
fn read_boost_state() -> Option<BoostState> {
    let raw = std::fs::read_to_string(boost_state_path().ok()?).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    Some(parse_boost_state(&v))
}

/// A boost whose pre-boost state is still on disk, also one started before
/// the app was restarted.
pub fn boost_active() -> bool {
    boost_state_path().is_ok_and(|p| p.exists())
}

fn remove_boost_state() {
    if let Ok(path) = boost_state_path() {
        let _ = std::fs::remove_file(path);
    }
}

pub fn boost_stop() -> Result<String, String> {
    // No recorded state means there is nothing we changed to restore. Never
    // guess a "Balanced" plan the user may never have had.
    let Some(BoostState {
        plan: prev_plan,
        toast: prev_toast,
        game_mode: prev_game_mode,
    }) = read_boost_state()
    else {
        if boost_state_path()?.exists() {
            // Unparseable state can never be restored; drop it so a new boost
            // is possible, and say honestly that nothing was restored.
            remove_boost_state();
            return Err(
                "boost state file was unreadable; power plan and notifications were not restored"
                    .into(),
            );
        }
        return Ok("No active game boost; nothing to restore".into());
    };

    // The plan guid is embedded in a script, so only a real GUID is accepted.
    // A corrupted value is reported, never replaced by a guessed default.
    let plan_valid = crate::ps::is_guid(&prev_plan);
    let plan_restore = if plan_valid {
        format!(
            "powercfg /setactive {prev_plan} 2>&1 | Out-Null\nif ($LASTEXITCODE -ne 0) {{ $errs += 'power plan restore failed (exit ' + $LASTEXITCODE + ')' }}"
        )
    } else {
        "$errs += 'saved power plan is invalid; plan not restored'".to_string()
    };

    // Restoring a value we previously changed must succeed; a failure is
    // reported honestly instead of printing "settings restored" anyway.
    // A captured DWORD value is a plain number, anything else is rejected so
    // a tampered registry value can never inject script; we fall back to
    // ensuring the property is absent.
    let toast_restore = match prev_toast.as_deref().map(str::trim) {
        Some(v) if !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()) => format!(
            "Set-ItemProperty 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Notifications\\Settings' NOC_GLOBAL_SETTING_TOASTS_ENABLED {v} -Type DWord -EA Stop"
        ),
        _ => "Remove-ItemProperty 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Notifications\\Settings' NOC_GLOBAL_SETTING_TOASTS_ENABLED -EA SilentlyContinue".to_string(),
    };

    let game_mode_restore = match prev_game_mode {
        None => String::new(),
        Some(Some(v)) => format!(
            "try {{ Set-ItemProperty 'HKCU:\\Software\\Microsoft\\GameBar' AutoGameModeEnabled {v} -Type DWord -EA Stop }} catch {{ $errs += ('Game Mode restore failed: ' + $_.Exception.Message) }}"
        ),
        Some(None) => "Remove-ItemProperty 'HKCU:\\Software\\Microsoft\\GameBar' AutoGameModeEnabled -EA SilentlyContinue".to_string(),
    };

    let script = format!(
        r#"
$errs = @()
{plan_restore}
try {{ {toast_restore} }} catch {{ $errs += ('toast restore failed: ' + $_.Exception.Message) }}
{game_mode_restore}
if ($errs.Count) {{ Write-Error ('Game boost stopped, but: ' + ($errs -join '; ')) }} else {{ 'Game boost stopped, settings restored' }}
"#
    );
    match ps::run(&script) {
        // State is only consumed once the exact restore succeeded, so a retry
        // after a failure still restores the captured values (never defaults).
        Ok(s) => {
            remove_boost_state();
            Ok(s.trim().to_string())
        }
        Err(e) => {
            // An invalid saved plan can never be restored; drop the state so
            // the user is not stuck with a boost that can't be stopped.
            if !plan_valid {
                remove_boost_state();
            }
            Err(e)
        }
    }
}

// ── Quick Boost, one-click safe combo with journal-backed undo ─────────────
// Unlike boost_start/boost_stop above (one-way apply, hardcoded global
// revert), every change Quick Boost makes is captured as a ChangeItem before
// it is applied, written to the same write-ahead journal the rest of the app
// uses (safety.rs / tweaks.rs), and returned to the caller as a single
// "restore token" (the journal entry id). That token undoes exactly this
// invocation via `tweaks::revert_entry`, so boosting two different games at
// once still gives each one its own independent undo.
//
// Deliberately NOT part of Quick Boost (removed after audit):
// - CPU affinity to "the first half of logical CPUs": on SMT CPUs that
//   halves the real cores, on hybrid CPUs it can drop P-cores.
// - HAGS toggle: needs a reboot to take effect and persists; it belongs to
//   the `hags_on` catalog tweak.
// - Killing overlays/background apps: non-reversible, and it used to run
//   unconditionally. quick_boost_start has no option to opt in, so the step
//   is gone; the user can still kill selected apps via kill_background.

/// Wrap a PowerShell expression so it runs as an executable command line -
/// `tweaks::apply_item` executes Command items via run_cmdline. run_cmdline
/// detects the `powershell … -Command <expr>` form and runs `<expr>` as a
/// single `-Command` argument (via ps::run), so `$`/quotes/spacing inside
/// stay literal script text.
fn ps_expr(expr: String) -> String {
    format!("powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command {expr}")
}

/// Resolve a process name (with or without ".exe") to a live PID.
fn find_pid_by_name(name: &str) -> Result<u32, String> {
    let base = strip_exe(name);
    // Embedded in a single-quoted PS string, reject anything that could
    // terminate the quote (process names are plain file names in practice).
    if !crate::ps::is_safe_ident(base) {
        return Err(format!("invalid process name '{name}'"));
    }
    let out = ps::run(&format!(
        "(Get-Process -Name '{base}' -ErrorAction Stop | Select-Object -First 1 -ExpandProperty Id)"
    ))?;
    out.trim()
        .parse::<u32>()
        .map_err(|_| format!("process '{name}' not found or not running"))
}

/// Whitelist a captured .NET ProcessPriorityClass name; the value is
/// interpolated into a revert script.
fn valid_priority(s: &str) -> bool {
    matches!(
        s,
        "Normal" | "Idle" | "BelowNormal" | "AboveNormal" | "High" | "RealTime"
    )
}

/// Apply the safe Quick Boost combo to `process_name`: HIGH priority (when
/// the process allows it) and the High Performance power plan. Snapshots
/// everything into one journal entry first; returns its id as the restore
/// token plus any skipped steps.
pub fn quick_boost_start(process_name: String) -> Result<Value, String> {
    let pid = find_pid_by_name(&process_name)?;
    let mut skipped: Vec<String> = Vec::new();

    // ---- snapshot current state ----
    // Anti-cheat protected games (BattlEye, EAC, Vanguard) deny reading or
    // changing the priority class. Probe with a no-op write of the current
    // value: if that is denied, the priority step is skipped instead of
    // failing (and rolling back) the whole boost.
    let captured_priority = ps::run(&format!(
        "$p = Get-Process -Id {pid} -ErrorAction Stop; $c = $p.PriorityClass; $p.PriorityClass = $c; $c.ToString()"
    ))
    .map(|s| s.trim().to_string());
    let prev_priority = match captured_priority {
        Ok(p) if valid_priority(&p) => Some(p),
        Ok(_) => {
            skipped.push("priority (unrecognised current priority)".into());
            None
        }
        Err(e) => {
            skipped.push(format!(
                "priority (access denied, likely anti-cheat protected: {})",
                e.trim()
            ));
            None
        }
    };

    let prev_plan = get_active_plan_guid()
        .ok_or("could not read the active power plan; Quick Boost not started")?;

    // ---- build reversible items (write-ahead, nothing applied yet) ----
    // Command items are executed via tweaks::apply_item → run_cmdline, which
    // spawns the first whitespace token as an executable. PowerShell
    // expressions must therefore be wrapped in a powershell.exe invocation
    // (the expression is one -Command argument, so quotes/$ stay literal).
    // All items use -EA Stop: a step that cannot actually be performed fails
    // the boost with rollback instead of silently recording "applied".
    let mut items = Vec::new();
    if let Some(prev_priority) = &prev_priority {
        items.push(ChangeItem::Command {
            applied: ps_expr(format!(
                "(Get-Process -Id {pid} -ErrorAction Stop).PriorityClass = 'High'"
            )),
            revert: ps_expr(format!(
                "Get-Process -Id {pid} -ErrorAction SilentlyContinue | ForEach-Object {{ $_.PriorityClass = '{prev_priority}' }}"
            )),
        });
    }
    items.push(ChangeItem::Command {
        applied: format!("powercfg /setactive {PLAN_HIGH_PERFORMANCE}"),
        revert: format!("powercfg /setactive {prev_plan}"),
    });

    // ---- write-ahead journal entry; its id is the restore token ----
    let entry_id = format!(
        "quickBoost-{pid}-{}",
        chrono::Local::now().format("%Y%m%d%H%M%S")
    );
    let entry_id = safety::append_entry(JournalEntry {
        id: entry_id,
        tweak_id: "quickBoost".into(),
        tweak_name: format!("Quick Boost ({process_name})"),
        origin: None,
        time: chrono::Local::now().to_rfc3339(),
        items: items.clone(),
        reverted: false,
        backup_files: vec![],
        completed: false,
        attempted: 0,
        reverted_items: 0,
        untrusted: false,
    })?;

    // ---- apply; roll back whatever already succeeded if one step fails ----
    // The write-ahead entry is marked reverted so the UI never offers an undo
    // for a boost that failed and was rolled back (mirrors tweaks::apply).
    // The failing action itself is rolled back too: a killed command can have
    // partially mutated already.
    for (idx, item) in items.iter().enumerate() {
        safety::with_journal(|j| {
            let entry = j
                .iter_mut()
                .find(|entry| entry.id == entry_id)
                .ok_or("journal entry missing")?;
            entry.attempted = idx + 1;
            Ok(())
        })?;
        if let Err(e) = tweaks::apply_item(item) {
            return match tweaks::rollback_entry_items(&entry_id, &items[..=idx]) {
                Ok(()) => Err(format!("Quick Boost failed ({e}); changes rolled back")),
                Err(re) => Err(format!(
                    "Quick Boost failed ({e}); rollback incomplete ({re}); restore token: {entry_id}"
                )),
            };
        }
    }

    safety::with_journal(|j| {
        let entry = j
            .iter_mut()
            .find(|entry| entry.id == entry_id)
            .ok_or("journal entry missing")?;
        entry.completed = true;
        Ok(())
    })?;

    Ok(json!({
        "restoreToken": entry_id,
        "pid": pid,
        // Kept for frontend compatibility; Quick Boost no longer kills
        // anything (see the comment above).
        "killedBackground": Vec::<String>::new(),
        "skipped": skipped,
    }))
}

/// Undo a single Quick Boost invocation via its restore token.
pub fn quick_boost_revert(restore_token: String) -> Result<Value, String> {
    tweaks::revert_entry(&restore_token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_match_exactly_not_by_substring() {
        assert!(is_kill_candidate("msedge"));
        assert!(is_kill_candidate("MSEDGE.exe"));
        assert!(is_kill_candidate("Spotify"));
        assert!(!is_kill_candidate("msedgewebview2"));
        assert!(!is_kill_candidate("chromedriver"));
        assert!(!is_kill_candidate("upc"));
        assert!(!is_kill_candidate("UplayWebCore"));
        assert!(!is_kill_candidate("Discord"));
        assert!(!is_kill_candidate("EpicGamesLauncher"));
        assert!(!is_kill_candidate("GalaxyClient"));
        assert!(!is_kill_candidate("Teams"));
        assert!(!is_kill_candidate(""));
    }

    #[test]
    fn protected_processes_are_never_candidates() {
        for p in [
            "msedgewebview2",
            "audiodg.exe",
            "BEService",
            "BEDaisy",
            "EasyAntiCheat",
            "EasyAntiCheat_EOS.exe",
            "vgc",
            "vgtray",
        ] {
            assert!(is_protected_proc(p), "{p} must be protected");
            assert!(!is_kill_candidate(p), "{p} must not be killable");
        }
        for c in KILL_CANDIDATES {
            assert!(!is_protected_proc(c), "{c} overlaps the protected list");
        }
    }

    #[test]
    fn candidate_names_are_safe_to_embed() {
        for c in KILL_CANDIDATES {
            assert!(crate::ps::is_safe_ident(c), "{c}");
        }
    }

    #[test]
    fn strip_exe_handles_case_and_short_names() {
        assert_eq!(strip_exe("Game.EXE"), "Game");
        assert_eq!(strip_exe("game.exe"), "game");
        assert_eq!(strip_exe(".exe"), ".exe");
        assert_eq!(strip_exe("vgc"), "vgc");
    }

    #[test]
    fn boost_state_keeps_absent_and_legacy_game_mode_apart() {
        let st = |v: Value| parse_boost_state(&v).game_mode;
        // Written by an older version: Game Mode is left alone on stop.
        assert_eq!(st(json!({ "prevPlan": "x" })), None);
        // The value did not exist before the boost: it is removed again.
        assert_eq!(
            st(json!({ "prevPlan": "x", "prevGameMode": null })),
            Some(None)
        );
        assert_eq!(
            st(json!({ "prevPlan": "x", "prevGameMode": 0 })),
            Some(Some(0))
        );
    }

    #[test]
    fn priority_whitelist() {
        assert!(valid_priority("High"));
        assert!(valid_priority("Idle"));
        assert!(!valid_priority("High'; rm"));
        assert!(!valid_priority(""));
    }
}
