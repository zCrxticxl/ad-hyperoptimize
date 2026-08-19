//! Game Booster, one-click system tuning when launching a game.
//! Kills background bloat, boosts target process, reverts everything after.

use crate::gameprofile::{get_active_plan_guid, PLAN_BALANCED, PLAN_HIGH_PERFORMANCE};
use crate::ps;
use crate::safety::{self, ChangeItem, JournalEntry};
use crate::tweaks;
use serde_json::{json, Value};

// Processes considered safe to suspend/kill during gaming
const KILL_CANDIDATES: &[&str] = &[
    "Discord",
    "Spotify",
    "OneDrive",
    "GoogleDriveFS",
    "Dropbox",
    "Teams",
    "slack",
    "zoom",
    "skype",
    "lync",
    "chrome",
    "msedge",
    "firefox",
    "brave",
    "opera",
    "AdobeUpdater",
    "AdobeIPCBroker",
    "Creative Cloud",
    "EpicGamesLauncher",
    "GalaxyClient",
    "upc",
    "SearchApp",
    "SearchHost",
    "Widgets",
    "WidgetService",
    "PhoneExperienceHost",
    "YourPhone",
    "WinStore.App",
    "MicrosoftEdgeUpdate",
];

pub fn list_background_procs() -> Value {
    let candidates_json = KILL_CANDIDATES
        .iter()
        .map(|s| format!("'{s}'"))
        .collect::<Vec<_>>()
        .join(",");

    let script = format!(
        r#"
$candidates = @({candidates_json})
$procs = Get-Process -ErrorAction SilentlyContinue |
    Where-Object {{
        $n = $_.Name
        $candidates | Where-Object {{ $n -like "*$_*" }}
    }} |
    Select-Object Id, Name,
        @{{n='memMb';e={{[math]::Round($_.WorkingSet64/1MB,1)}}}}
$procs | ConvertTo-Json -Compress -Depth 2
"#
    );

    match ps::run_json(&script) {
        Ok(Value::Array(arr)) => json!({ "procs": arr }),
        Ok(v @ Value::Object(_)) => json!({ "procs": [v] }),
        _ => json!({ "procs": [] }),
    }
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

pub fn boost_process(pid: u32) -> Result<String, String> {
    let script = format!(
        r#"
$p = Get-Process -Id {pid} -ErrorAction Stop
$p.PriorityClass = [System.Diagnostics.ProcessPriorityClass]::High
"Boosted PID {pid} ($($p.Name)) to High priority"
"#
    );
    ps::run(&script).map(|s| s.trim().to_string())
}

pub fn kill_background(pids: Vec<u32>) -> Result<String, String> {
    if pids.is_empty() {
        return Ok("Nothing to kill".into());
    }
    let pid_list = pids
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let script = format!(
        r#"
$killed = 0
@({pid_list}) | ForEach-Object {{
    try {{
        Stop-Process -Id $_ -Force -ErrorAction Stop
        $killed++
    }} catch {{ }}
}}
"$killed processes terminated"
"#
    );
    ps::run(&script).map(|s| s.trim().to_string())
}

pub fn boost_start(pid: u32) -> Result<String, String> {
    // Capture the pre-boost state so boost_stop can restore it exactly
    // instead of forcing the hardcoded Balanced plan (H9).
    let prev_plan = get_active_plan_guid().unwrap_or_else(|| PLAN_BALANCED.to_string());
    let prev_toast = ps::run(
        "(Get-ItemProperty 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Notifications\\Settings' -Name NOC_GLOBAL_SETTING_TOASTS_ENABLED -ErrorAction SilentlyContinue).NOC_GLOBAL_SETTING_TOASTS_ENABLED",
    )
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty());

    let script = format!(
        r#"
$errors = @()

# 1. Boost target process
try {{
    $p = Get-Process -Id {pid} -EA Stop
    $p.PriorityClass = [System.Diagnostics.ProcessPriorityClass]::High
}} catch {{ $errors += "Priority: $_" }}

# 2. Enable Game Mode
try {{
    Set-ItemProperty 'HKCU:\Software\Microsoft\GameBar' AutoGameModeEnabled 1 -Type DWord -EA SilentlyContinue
}} catch {{}}

# 3. Set power plan to High Performance or Ultimate
try {{
    $up = (powercfg /list | Select-String 'e9a42b02').ToString().Trim()
    if ($up -match 'GUID:\s+([\w-]+)') {{
        powercfg /setactive $Matches[1]
    }} else {{
        powercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c
    }}
}} catch {{ $errors += "Power: $_" }}

# 4. Disable notifications
try {{
    Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings' NOC_GLOBAL_SETTING_TOASTS_ENABLED 0 -Type DWord -EA SilentlyContinue
}} catch {{}}

if ($errors) {{ "Boosted with warnings: " + ($errors -join '; ') }}
else {{ "Game boost active for PID {pid}" }}
"#
    );
    save_boost_state(&prev_plan, prev_toast.as_deref())?;
    ps::run(&script).map(|s| s.trim().to_string())
}

/// Persist the pre-boost state for exact restore in boost_stop.
fn save_boost_state(prev_plan: &str, prev_toast: Option<&str>) -> Result<(), String> {
    let dir = crate::safety::app_data_dir().join("gameboost");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let v = json!({ "prevPlan": prev_plan, "prevToast": prev_toast });
    std::fs::write(
        dir.join("boost-state.json"),
        serde_json::to_string(&v).unwrap_or_default(),
    )
    .map_err(|e| format!("saving boost state failed: {e}"))
}

/// Read (without consuming) the pre-boost state; `None` when no state was
/// saved (boost_stop without a matching boost_start).
fn read_boost_state() -> Option<(String, Option<String>)> {
    let path = crate::safety::app_data_dir()
        .join("gameboost")
        .join("boost-state.json");
    let raw = std::fs::read_to_string(&path).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    let plan = v["prevPlan"].as_str()?.to_string();
    let toast = v["prevToast"].as_str().map(str::to_string);
    Some((plan, toast))
}

fn remove_boost_state() {
    let path = crate::safety::app_data_dir()
        .join("gameboost")
        .join("boost-state.json");
    let _ = std::fs::remove_file(&path);
}

pub fn boost_stop() -> Result<String, String> {
    let (prev_plan, prev_toast) =
        read_boost_state().unwrap_or_else(|| (PLAN_BALANCED.to_string(), None));

    // The plan guid is embedded in a PowerShell script, only ever a real
    // GUID (or the hardcoded Balanced fallback). A corrupted state file must
    // not be able to inject script, so anything else degrades to Balanced.
    let prev_plan = if crate::ps::is_guid(&prev_plan) {
        prev_plan
    } else {
        PLAN_BALANCED.to_string()
    };

    // Restoring a value we previously changed must succeed; a failure is
    // reported honestly instead of printing "settings restored" anyway.
    // A captured DWORD value is a plain number, anything else is rejected so
    // a tampered registry value can never inject script; we fall back to
    // ensuring the property is absent.
    let toast_restore = match prev_toast.as_deref() {
        Some(v) if !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()) => format!(
            "Set-ItemProperty 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Notifications\\Settings' NOC_GLOBAL_SETTING_TOASTS_ENABLED {v} -Type DWord -EA Stop"
        ),
        _ => "Remove-ItemProperty 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Notifications\\Settings' NOC_GLOBAL_SETTING_TOASTS_ENABLED -EA SilentlyContinue".to_string(),
    };

    let script = format!(
        r#"
$errs = @()
powercfg /setactive {prev_plan} 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {{ $errs += 'power plan restore failed (exit ' + $LASTEXITCODE + ')' }}
try {{ {toast_restore} }} catch {{ $errs += ('toast restore failed: ' + $_.Exception.Message) }}
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
        Err(e) => Err(e),
    }
}

pub fn set_gpu_max_perf(enable: bool) -> Result<String, String> {
    let val = if enable { 1 } else { 0 };
    let script = format!(
        r#"
$path = 'HKLM:\SYSTEM\CurrentControlSet\Control\GraphicsDrivers'
Set-ItemProperty $path HwSchMode {val} -Type DWord -EA SilentlyContinue
"GPU scheduling preference updated (restart may be required)"
"#
    );
    ps::run(&script).map(|s| s.trim().to_string())
}

// ── Quick Boost, one-click safe combo with journal-backed undo ─────────────
// Unlike boost_start/boost_stop above (one-way apply, hardcoded global
// revert), every change Quick Boost makes is captured as a ChangeItem before
// it is applied, written to the same write-ahead journal the rest of the app
// uses (safety.rs / tweaks.rs), and returned to the caller as a single
// "restore token" (the journal entry id). That token undoes exactly this
// invocation via `tweaks::revert_entry`, so boosting two different games at
// once still gives each one its own independent undo.

const HAGS_PATH: &str = "HKLM:\\SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers";

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
    let base = name.trim_end_matches(".exe").trim_end_matches(".EXE");
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

/// Bitmask covering the first half of logical cores. Windows/PowerShell have
/// no simple cross-vendor API for the real P-core/E-core split on hybrid
/// CPUs; "first half" is the closest safe heuristic without pulling in extra
/// topology APIs, and it's a no-op-equivalent (full mask) on symmetric CPUs
/// with <=2 cores.
fn perf_core_mask(core_count: usize) -> u64 {
    let half = (core_count / 2).clamp(1, 63);
    (1u64 << half) - 1
}

/// Best-effort, non-reversible cleanup of known overlay/background bloat.
/// Returns the names of whatever got killed so the UI can tell the user
/// (these are ordinary user apps, Discord, Spotify, browsers, relaunching
/// them is on the user, which is the accepted trade-off for "low risk").
fn kill_known_overlays() -> Vec<String> {
    let procs = list_background_procs();
    let arr = procs["procs"].as_array().cloned().unwrap_or_default();
    let pids: Vec<u32> = arr
        .iter()
        .filter_map(|p| p["Id"].as_u64().map(|x| x as u32))
        .collect();
    let names: Vec<String> = arr
        .iter()
        .filter_map(|p| p["Name"].as_str().map(str::to_string))
        .collect();
    if !pids.is_empty() {
        let _ = kill_background(pids);
    }
    names
}

/// Apply the safe Quick Boost combo to `process_name`: HIGH priority, affinity
/// pinned to the (heuristic) performance cores, High Performance power plan,
/// HAGS on, known overlays/background bloat killed. Snapshots everything
/// reversible into one journal entry first; returns its id as the restore
/// token plus what was killed.
pub fn quick_boost_start(process_name: String) -> Result<Value, String> {
    let pid = find_pid_by_name(&process_name)?;

    // ---- snapshot current state ----
    let prev_priority = ps::run(&format!(
        "(Get-Process -Id {pid} -ErrorAction Stop).PriorityClass.ToString()"
    ))
    .map(|s| s.trim().to_string())
    .unwrap_or_else(|_| "Normal".into());

    let prev_affinity = ps::run(&format!(
        "[int64](Get-Process -Id {pid} -ErrorAction Stop).ProcessorAffinity"
    ))
    .ok()
    .and_then(|s| s.trim().parse::<u64>().ok());

    let prev_plan = get_active_plan_guid().unwrap_or_else(|| PLAN_BALANCED.to_string());

    let prev_hags = ps::run(&format!(
        "(Get-ItemProperty -Path '{HAGS_PATH}' -Name HwSchMode -ErrorAction SilentlyContinue).HwSchMode"
    ))
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty());

    let core_count: usize = ps::run("[Environment]::ProcessorCount")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(8);
    let mask = perf_core_mask(core_count);

    // ---- build reversible items (write-ahead, nothing applied yet) ----
    // Command items are executed via tweaks::apply_item → run_cmdline, which
    // spawns the first whitespace token as an executable. PowerShell
    // expressions must therefore be wrapped in a powershell.exe invocation
    // (the expression is one -Command argument, so quotes/$ stay literal).
    //
    // The HAGS item writes to HKLM, only ever plan it when elevated, so an
    // unelevated Quick Boost doesn't journal a change it cannot make. All
    // items use -EA Stop: a step that cannot actually be performed fails the
    // boost with rollback instead of silently recording "applied".
    let mut items = vec![
        ChangeItem::Command {
            applied: ps_expr(format!("(Get-Process -Id {pid} -ErrorAction Stop).PriorityClass = 'High'")),
            revert: ps_expr(format!(
                "Get-Process -Id {pid} -ErrorAction SilentlyContinue | ForEach-Object {{ $_.PriorityClass = '{prev_priority}' }}"
            )),
        },
        ChangeItem::Command {
            applied: ps_expr(format!("(Get-Process -Id {pid} -ErrorAction Stop).ProcessorAffinity = [IntPtr]{mask}")),
            revert: ps_expr(format!(
                "Get-Process -Id {pid} -ErrorAction SilentlyContinue | ForEach-Object {{ $_.ProcessorAffinity = [IntPtr]{m} }}",
                m = prev_affinity.map_or(-1i64, |v| v as i64),
            )),
        },
        ChangeItem::Command {
            applied: format!("powercfg /setactive {PLAN_HIGH_PERFORMANCE}"),
            revert: format!("powercfg /setactive {prev_plan}"),
        },
    ];
    if crate::ps::is_admin() {
        items.push(ChangeItem::Command {
            applied: ps_expr(format!(
                "Set-ItemProperty -Path '{HAGS_PATH}' -Name HwSchMode -Value 2 -Type DWord -EA Stop"
            )),
            revert: match &prev_hags {
                Some(v) => ps_expr(format!(
                    "Set-ItemProperty -Path '{HAGS_PATH}' -Name HwSchMode -Value {v} -Type DWord -EA Stop"
                )),
                None => ps_expr(format!(
                    "Remove-ItemProperty -Path '{HAGS_PATH}' -Name HwSchMode -EA Stop"
                )),
            },
        });
    }

    // ---- write-ahead journal entry; its id is the restore token ----
    let entry_id = format!(
        "quickBoost-{pid}-{}",
        chrono::Local::now().format("%Y%m%d%H%M%S")
    );
    safety::append_entry(JournalEntry {
        id: entry_id.clone(),
        tweak_id: "quickBoost".into(),
        tweak_name: format!("Quick Boost ({process_name})"),
        time: chrono::Local::now().to_rfc3339(),
        items: items.clone(),
        reverted: false,
        backup_files: vec![],
    })?;

    // ---- apply; roll back whatever already succeeded if one step fails ----
    // The write-ahead entry is marked reverted so the UI never offers an undo
    // for a boost that failed and was rolled back (mirrors tweaks::apply).
    let mut done: Vec<&ChangeItem> = Vec::new();
    for item in &items {
        if let Err(e) = tweaks::apply_item(item) {
            for d in done.iter().rev() {
                let _ = tweaks::revert_item(d);
            }
            let _ = safety::with_journal(|j| {
                if let Some(en) = j.iter_mut().find(|en| en.id == entry_id) {
                    en.reverted = true;
                }
                Ok(())
            });
            return Err(format!("Quick Boost failed ({e}); changes rolled back"));
        }
        done.push(item);
    }

    // ---- best-effort, non-reversible overlay cleanup ----
    let killed = kill_known_overlays();

    Ok(json!({
        "restoreToken": entry_id,
        "pid": pid,
        "killedBackground": killed,
    }))
}

/// Undo a single Quick Boost invocation via its restore token.
pub fn quick_boost_revert(restore_token: String) -> Result<Value, String> {
    tweaks::revert_entry(&restore_token)
}
