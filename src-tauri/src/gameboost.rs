//! Game Booster — one-click system tuning when launching a game.
//! Kills background bloat, boosts target process, reverts everything after.

use crate::ps;
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
