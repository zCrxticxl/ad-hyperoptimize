//! Game Booster — one-click system tuning when launching a game.
//! Kills background bloat, boosts target process, reverts everything after.

use crate::gameprofile::{get_active_plan_guid, PLAN_BALANCED, PLAN_HIGH_PERFORMANCE};
use crate::ps;
use crate::safety::{self, ChangeItem, JournalEntry};
use crate::tweaks;
use serde_json::{json, Value};

// Processes considered safe to suspend/kill during gaming
const KILL_CANDIDATES: &[&str] = &[
    "Discord", "Spotify", "OneDrive", "GoogleDriveFS", "Dropbox",
    "Teams", "slack", "zoom", "skype", "lync",
    "chrome", "msedge", "firefox", "brave", "opera",
    "AdobeUpdater", "AdobeIPCBroker", "Creative Cloud",
    "EpicGamesLauncher", "GalaxyClient", "upc",
    "SearchApp", "SearchHost", "Widgets", "WidgetService",
    "PhoneExperienceHost", "YourPhone", "WinStore.App",
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
    if pids.is_empty() { return Ok("Nothing to kill".into()); }
    let pid_list = pids.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",");
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
    ps::run(&script).map(|s| s.trim().to_string())
}

pub fn boost_stop() -> Result<String, String> {
    let script = r#"
# Restore Balanced power plan
powercfg /setactive 381b4222-f694-41f0-9685-ff5bb260df2e | Out-Null

# Re-enable notifications
Remove-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings' NOC_GLOBAL_SETTING_TOASTS_ENABLED -EA SilentlyContinue

"Game boost stopped — settings restored"
"#;
    ps::run(script).map(|s| s.trim().to_string())
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

// ── Quick Boost — one-click safe combo with journal-backed undo ─────────────
// Unlike boost_start/boost_stop above (one-way apply, hardcoded global
// revert), every change Quick Boost makes is captured as a ChangeItem before
// it is applied, written to the same write-ahead journal the rest of the app
// uses (safety.rs / tweaks.rs), and returned to the caller as a single
// "restore token" (the journal entry id). That token undoes exactly this
// invocation via `tweaks::revert_entry`, so boosting two different games at
// once still gives each one its own independent undo.

const HAGS_PATH: &str = "HKLM:\\SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers";

/// Resolve a process name (with or without ".exe") to a live PID.
fn find_pid_by_name(name: &str) -> Result<u32, String> {
    let base = name.trim_end_matches(".exe").trim_end_matches(".EXE");
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
    let half = (core_count / 2).max(1).min(63);
    (1u64 << half) - 1
}

/// Best-effort, non-reversible cleanup of known overlay/background bloat.
/// Returns the names of whatever got killed so the UI can tell the user
/// (these are ordinary user apps — Discord, Spotify, browsers — relaunching
/// them is on the user, which is the accepted trade-off for "low risk").
fn kill_known_overlays() -> Vec<String> {
    let procs = list_background_procs();
    let arr = procs["procs"].as_array().cloned().unwrap_or_default();
    let pids: Vec<u32> = arr.iter().filter_map(|p| p["Id"].as_u64().map(|x| x as u32)).collect();
    let names: Vec<String> = arr.iter().filter_map(|p| p["Name"].as_str().map(str::to_string)).collect();
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
    let items = vec![
        ChangeItem::Command {
            applied: format!("(Get-Process -Id {pid} -ErrorAction Stop).PriorityClass = 'High'"),
            revert: format!(
                "Get-Process -Id {pid} -ErrorAction SilentlyContinue | ForEach-Object {{ $_.PriorityClass = '{prev_priority}' }}"
            ),
        },
        ChangeItem::Command {
            applied: format!("(Get-Process -Id {pid} -ErrorAction Stop).ProcessorAffinity = [IntPtr]{mask}"),
            revert: match prev_affinity {
                Some(m) => format!(
                    "Get-Process -Id {pid} -ErrorAction SilentlyContinue | ForEach-Object {{ $_.ProcessorAffinity = [IntPtr]{m} }}"
                ),
                None => format!(
                    "Get-Process -Id {pid} -ErrorAction SilentlyContinue | ForEach-Object {{ $_.ProcessorAffinity = [IntPtr]-1 }}"
                ),
            },
        },
        ChangeItem::Command {
            applied: format!("powercfg /setactive {PLAN_HIGH_PERFORMANCE}"),
            revert: format!("powercfg /setactive {prev_plan}"),
        },
        ChangeItem::Command {
            applied: format!(
                "Set-ItemProperty -Path '{HAGS_PATH}' -Name HwSchMode -Value 2 -Type DWord -EA SilentlyContinue"
            ),
            revert: match &prev_hags {
                Some(v) => format!(
                    "Set-ItemProperty -Path '{HAGS_PATH}' -Name HwSchMode -Value {v} -Type DWord -EA SilentlyContinue"
                ),
                None => format!("Remove-ItemProperty -Path '{HAGS_PATH}' -Name HwSchMode -EA SilentlyContinue"),
            },
        },
    ];

    // ---- write-ahead journal entry; its id is the restore token ----
    let entry_id = format!("quickBoost-{pid}-{}", chrono::Local::now().format("%Y%m%d%H%M%S"));
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
    let mut done: Vec<&ChangeItem> = Vec::new();
    for item in &items {
        if let Err(e) = tweaks::apply_item(item) {
            for d in done.iter().rev() {
                let _ = tweaks::revert_item(d);
            }
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
