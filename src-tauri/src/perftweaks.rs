//! Performance tweaks: Timer Resolution, MSI Mode, Network Adapter, RAM usage (read-only), Pagefile.

use crate::ps;
use serde_json::{json, Value};

// ── Timer Resolution ──────────────────────────────────────────────────────────

pub fn timer_get() -> Value {
    // Use int (not uint), PS [ref] binding works more reliably with signed int.
    // Wrap everything in try-catch so we always return valid JSON.
    let script = r#"
$result = [PSCustomObject]@{
    currentMs        = 15.625
    minMs            = 0.5
    maxMs            = 15.625
    current100ns     = 156250
    globalReqEnabled = $false
    persistent       = $false
}
try {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class NtTimerQ {
    [DllImport("ntdll.dll")] public static extern int NtQueryTimerResolution(out int min, out int max, out int cur);
}
'@ -ErrorAction Stop
    $min = 0; $max = 0; $cur = 0
    [NtTimerQ]::NtQueryTimerResolution([ref]$min, [ref]$max, [ref]$cur) | Out-Null
    if ($cur -gt 0) {
        $result.currentMs    = [math]::Round($cur / 10000.0, 3)
        $result.minMs        = [math]::Round($min / 10000.0, 3)
        $result.maxMs        = [math]::Round($max / 10000.0, 3)
        $result.current100ns = $cur
    }
} catch {}
try {
    $v = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\kernel' `
        -Name 'GlobalTimerResolutionRequests' -ErrorAction Stop).GlobalTimerResolutionRequests
    $result.globalReqEnabled = ($v -eq 1)
} catch {}
try {
    $task = Get-ScheduledTask -TaskPath '\ADHyperOptimize\' -TaskName 'TimerResolution' -ErrorAction Stop
    $result.persistent = ($task.State -eq 'Running')
} catch { $result.persistent = $false }
$result | ConvertTo-Json -Compress
"#;
    ps::run_json(script).unwrap_or_else(|e| json!({ "error": e }))
}

pub fn timer_set(target_100ns: u32) -> Result<String, String> {
    if !matches!(target_100ns, 5000 | 10000) {
        return Err("Unsupported timer resolution; choose 0.5 or 1 ms".into());
    }
    if !ps::is_admin() {
        return Err("Timer task requires administrator rights".into());
    }
    let current = timer_get();
    if let Some(error) = current["error"].as_str() {
        return Err(format!("Cannot inspect existing timer request: {error}"));
    }
    if current["persistent"] == true {
        return Err("Timer request already active; reset it before choosing a new value".into());
    }
    // Steps:
    // 1. Call NtSetTimerResolution now to detect actual hardware minimum
    // 2. Set GlobalTimerResolutionRequests registry key (persists across boots)
    // 3. Encode the helper script as UTF-16LE base64 (-EncodedCommand). No
    //    script file is written: a file in %ProgramData% can be pre-created
    //    and owned by a standard user, which would let them run code as SYSTEM.
    // 4. Register a logon-trigger scheduled task that runs it as SYSTEM
    // 5. Start the task immediately, now the process stays alive and holds the resolution
    // 6. Sleep 800ms so timer_get() called right after sees the updated value
    let script = format!(
        r#"
$ErrorActionPreference = "Stop"
$target = {target_100ns}
$actual = 0
$msg = "Applied"
try {{
    Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class NtTimerS{{[DllImport("ntdll.dll")]public static extern int NtSetTimerResolution(int desired,bool set,out int actual);}}' -ErrorAction Stop
    $status = [NtTimerS]::NtSetTimerResolution($target,$true,[ref]$actual)
    if ($status -ne 0) {{ throw "NtSetTimerResolution failed: $status" }}
    $achievedMs=[math]::Round($actual/10000.0,3)
    if($actual -gt ($target+1000)){{throw "Hardware limit: ${{achievedMs}}ms"}}
    else{{$msg="Set ${{achievedMs}}ms"}}
}}catch{{throw}}
Set-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\kernel' -Name 'GlobalTimerResolutionRequests' -Value 1 -Type DWord -Force -ErrorAction Stop
try{{
    $helper=@'
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class NtTP{{[DllImport("ntdll.dll")]public static extern int NtSetTimerResolution(int d,bool s,out int a);}}' -EA Stop
$a=0;$status=[NtTP]::NtSetTimerResolution({target_100ns},$true,[ref]$a);if($status -ne 0 -or $a -gt ({target_100ns}+1000)){{exit 1}};while($true){{Start-Sleep -Seconds 3600}}
'@
    $enc=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($helper))
    $exe=Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $ta=New-ScheduledTaskAction -Execute $exe -Argument "-NoProfile -WindowStyle Hidden -NonInteractive -EncodedCommand $enc"
    $tt=New-ScheduledTaskTrigger -AtLogOn
    $ts=New-ScheduledTaskSettingsSet -Hidden -ExecutionTimeLimit (New-TimeSpan -Hours 0)
    $tp=New-ScheduledTaskPrincipal -UserId 'SYSTEM' -RunLevel Highest
    Register-ScheduledTask -TaskPath '\ADHyperOptimize\' -TaskName 'TimerResolution' -Action $ta -Trigger $tt -Settings $ts -Principal $tp -Force -ErrorAction Stop|Out-Null
    Start-ScheduledTask -TaskPath '\ADHyperOptimize\' -TaskName 'TimerResolution' -ErrorAction Stop
    Start-Sleep -Milliseconds 800
    $task = Get-ScheduledTask -TaskPath '\ADHyperOptimize\' -TaskName 'TimerResolution' -ErrorAction Stop
    if ($task.State -ne 'Running') {{ throw "Timer helper did not stay running: $($task.State)" }}
}}catch{{throw}}
$msg
"#
    );
    let timer_registry_path = r"SYSTEM\CurrentControlSet\Control\Session Manager\kernel";
    let previous_global_request =
        crate::tweaks::reg_read("HKLM", timer_registry_path, "GlobalTimerResolutionRequests")?;
    let entry_id = crate::safety::append_entry(crate::safety::JournalEntry {
        id: format!(
            "timerRes-{}",
            chrono::Local::now().format("%Y%m%d%H%M%S%6f")
        ),
        tweak_id: "perftweaks:timer".into(),
        tweak_name: "Timer Resolution task".into(),
        origin: None,
        time: chrono::Local::now().to_rfc3339(),
        items: vec![
            crate::safety::ChangeItem::Registry {
                root: "HKLM".into(),
                path: timer_registry_path.into(),
                name: "GlobalTimerResolutionRequests".into(),
                prev: previous_global_request,
                new: crate::safety::RegVal::Dword(1),
            },
            crate::safety::ChangeItem::Command {
                applied: script.clone(),
                revert: timer_revert_command(),
            },
        ],
        reverted: false,
        backup_files: vec![],
        completed: false,
        attempted: 2,
        reverted_items: 0,
        untrusted: false,
    })?;
    let res = ps::run(&script).map(|s| s.trim().to_string());
    crate::safety::with_journal(|entries| {
        let entry = entries
            .iter_mut()
            .find(|entry| entry.id == entry_id)
            .ok_or("Timer journal entry missing")?;
        entry.completed = res.is_ok();
        Ok(())
    })?;
    res
}

pub fn timer_reset() -> Result<String, String> {
    let entry = crate::safety::load_journal()?
        .into_iter()
        .rev()
        .find(|entry| entry.tweak_id == "perftweaks:timer" && !entry.reverted);
    match entry {
        Some(entry) if entry.items.len() > 1 => {
            crate::tweaks::revert_entry(&entry.id)?;
            // Journals written by older versions carry a reset script whose
            // helper path was corrupted (literal TAB), so the SYSTEM-executed
            // helper file survived; remove it explicitly.
            match ps::run(TIMER_LEGACY_CLEANUP_PS) {
                Ok(_) => Ok("Timer request removed and previous state restored".into()),
                Err(e) => Err(format!(
                    "Timer request removed and previous state restored, but the legacy helper \
                     script could not be deleted: {e}"
                )),
            }
        }
        Some(entry) => {
            ps::run(TIMER_RESET_PS)?;
            crate::safety::with_journal(|entries| {
                let saved = entries
                    .iter_mut()
                    .find(|saved| saved.id == entry.id)
                    .ok_or("Legacy timer journal entry missing")?;
                saved.reverted = true;
                saved.reverted_items = saved.items.len();
                Ok(())
            })?;
            Ok("Legacy timer request removed; original registry value was not recorded".into())
        }
        None => {
            let current = timer_get();
            if current["persistent"] != true {
                return Ok("No AD HyperOptimize timer task is active".into());
            }
            ps::run(TIMER_RESET_PS)?;
            Ok("Legacy timer request removed; original registry value was not recorded".into())
        }
    }
}

/// Removes the helper script older versions wrote to
/// %ProgramData%\ADHyperOptimize (executed as SYSTEM, so it must not linger)
/// and the folder once empty. A reparse-point folder is never followed.
macro_rules! timer_legacy_cleanup_ps {
    () => {
        r#"
$legacyDir = Join-Path $env:ProgramData 'ADHyperOptimize'
$legacyItem = Get-Item -LiteralPath $legacyDir -Force -ErrorAction SilentlyContinue
if ($legacyItem -and -not ($legacyItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    $helper = Join-Path $env:ProgramData 'ADHyperOptimize\timerres.ps1'
    if (Test-Path -LiteralPath $helper) { Remove-Item -LiteralPath $helper -Force -ErrorAction Stop }
    if (-not (Get-ChildItem -LiteralPath $legacyDir -Force -ErrorAction Stop)) {
        Remove-Item -LiteralPath $legacyDir -Force -ErrorAction Stop
    }
}
"#
    };
}

const TIMER_LEGACY_CLEANUP_PS: &str = timer_legacy_cleanup_ps!();

/// Undo operations for the persistent timer-resolution task; shared by
/// timer_reset and the journal entry timer_set writes.
/// Journal revert for the timer tweak; statestore re-derives migrated timer
/// entries from this instead of trusting their stored script.
pub(crate) fn timer_revert_command() -> String {
    format!("powershell -Command {TIMER_RESET_PS}")
}

const TIMER_RESET_PS: &str = concat!(
    r#"
# Stop and remove the persistent scheduled task
$task = Get-ScheduledTask -TaskPath '\ADHyperOptimize\' -TaskName 'TimerResolution' -ErrorAction SilentlyContinue
if ($task) {
    if ($task.State -eq 'Running') {
        Stop-ScheduledTask -TaskPath '\ADHyperOptimize\' -TaskName 'TimerResolution' -ErrorAction Stop
    }
    Unregister-ScheduledTask -TaskPath '\ADHyperOptimize\' -TaskName 'TimerResolution' -Confirm:$false -ErrorAction Stop
}"#,
    timer_legacy_cleanup_ps!(),
    r#"# Remove the setting this tool added; the journal restores a prior value afterwards
$kernel = 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\kernel'
if (Get-ItemProperty -Path $kernel -Name 'GlobalTimerResolutionRequests' -ErrorAction SilentlyContinue) {
    Remove-ItemProperty -Path $kernel -Name 'GlobalTimerResolutionRequests' -ErrorAction Stop
}
if (Get-ScheduledTask -TaskPath '\ADHyperOptimize\' -TaskName 'TimerResolution' -ErrorAction SilentlyContinue) {
    throw 'Timer task still exists after reset'
}
"Timer task removed"
"#
);

// ── MSI Mode ─────────────────────────────────────────────────────────────────

// Get-PnpDevice gives a normalized Class + InstanceId regardless of how the
// driver/INF registered the device, far more reliable than manually
// walking Enum\PCI and reading a raw Class string off the instance key.
// InstanceId -> "HKLM:\SYSTEM\CurrentControlSet\Enum\<InstanceId>" is the
// standard, documented mapping (same one pnputil/Device Manager use).
const MSI_LIST_PS: &str = r#"
$results = @()
Get-PnpDevice -PresentOnly -ErrorAction SilentlyContinue |
    Where-Object { $_.Class -in @('Display','Net') -and $_.InstanceId } |
    ForEach-Object {
        $instPath = "HKLM:\SYSTEM\CurrentControlSet\Enum\$($_.InstanceId)"
        $msiKey = Join-Path $instPath 'Device Parameters\Interrupt Management\MessageSignaledInterruptProperties'
        $msiEnabled = $false
        if (Test-Path $msiKey) {
            $mp = Get-ItemProperty $msiKey -ErrorAction SilentlyContinue
            $msiEnabled = ($mp.MSISupported -eq 1)
        }
        $results += [PSCustomObject]@{
            name       = $_.FriendlyName
            type       = if ($_.Class -eq 'Display') { 'GPU' } else { 'NIC' }
            msiEnabled = $msiEnabled
            regPath    = $instPath
        }
    }
if ($results.Count -eq 0) { '[]' } else { @($results) | ConvertTo-Json -Depth 2 -Compress }
"#;

pub fn msi_list() -> Value {
    match ps::run_json(MSI_LIST_PS) {
        Ok(v @ Value::Array(_)) => json!({ "devices": v }),
        Ok(v @ Value::Object(_)) => json!({ "devices": [v] }),
        _ => json!({ "devices": [] }),
    }
}

/// True only when `ps_path` is exactly one of the Display/Net devices the
/// msi_list() enumeration produced. Anything else (storage controllers, USB
/// hosts, chipset devices) is refused: a wrong MSI flag there can stop the
/// system from booting.
fn msi_path_allowed(devices: &Value, ps_path: &str) -> bool {
    let list: Vec<&Value> = match devices {
        Value::Array(a) => a.iter().collect(),
        v @ Value::Object(_) => vec![v],
        _ => return false,
    };
    list.iter().any(|d| {
        matches!(d["type"].as_str(), Some("GPU" | "NIC"))
            && d["regPath"]
                .as_str()
                .is_some_and(|p| p.eq_ignore_ascii_case(ps_path))
    })
}

pub fn msi_set(reg_path: String, enabled: bool) -> Result<String, String> {
    let val = if enabled { 1 } else { 0 };
    // regPath from msi_list() is already "HKLM:\SYSTEM\..." (PS-ready); this
    // replacen is a no-op for that shape, kept only for back-compat with any
    // stale HKEY_LOCAL_MACHINE-prefixed value a client might still send.
    let ps_path = reg_path.replacen("HKEY_LOCAL_MACHINE", "HKLM:", 1);
    // The path is embedded in a single-quoted PS string and can only target
    // device instances the scan produced (HKLM:\SYSTEM\CurrentControlSet\Enum\…).
    // Wildcards are rejected: Set-ItemProperty -Path resolves them at the
    // provider level, so a single '*' would flip MSI mode on EVERY device —
    // disabling it on the boot storage controller bricks the next boot.
    if !ps_path.starts_with(r"HKLM:\SYSTEM\CurrentControlSet\Enum\")
        || !crate::ps::is_safe_ident(&ps_path)
        || ps_path.contains(['*', '?', '[', ']'])
    {
        return Err("Invalid registry path for MSI mode".into());
    }
    let devices =
        ps::run_json(MSI_LIST_PS).map_err(|e| format!("Cannot verify MSI device list: {e}"))?;
    if !msi_path_allowed(&devices, &ps_path) {
        return Err("MSI mode can only be changed for a present display or network adapter".into());
    }
    let script = format!(
        r#"
$msiKey = Join-Path '{ps_path}' 'Device Parameters\Interrupt Management\MessageSignaledInterruptProperties'
if (-not (Test-Path $msiKey)) {{ New-Item -Path $msiKey -Force | Out-Null }}
Set-ItemProperty -Path $msiKey -Name 'MSISupported' -Value {val} -Type DWord -Force
"OK, reboot required to take effect"
"#
    );
    ps::run(&script).map(|_| {
        format!(
            "MSI {}, reboot required",
            if enabled { "enabled" } else { "disabled" }
        )
    })
}

// ── Network Adapter Tweaks ────────────────────────────────────────────────────

pub fn net_adapters() -> Value {
    // No Status filter, include all adapters regardless of link state.
    // Plain foreach + hashtable lookups instead of ForEach-Object with a
    // closure-capturing scriptblock ($gv = { ... }; & $gv ...): if any
    // single adapter's advanced-property fetch threw, the old version's
    // single shared try/catch around the *whole* loop discarded every
    // adapter collected so far, not just the failing one. Each adapter now
    // gets its own try/catch so one bad device can't zero out the list.
    let script = r#"
$adapters = @()
$all = $null
try { $all = Get-NetAdapter -ErrorAction SilentlyContinue } catch {}
foreach ($a in $all) {
    $name = $a.Name
    $kwmap = @{}
    try {
        Get-NetAdapterAdvancedProperty -Name $name -ErrorAction SilentlyContinue | ForEach-Object {
            $kwmap[$_.RegistryKeyword] = $_.DisplayValue
        }
    } catch {}
    $adapters += [PSCustomObject]@{
        name        = $name
        description = $a.InterfaceDescription
        status      = $a.Status
        speedMbps   = if ($a.LinkSpeed) { [math]::Round($a.LinkSpeed / 1e6) } else { 0 }
        intMod      = $kwmap['*InterruptModeration']
        rss         = $kwmap['*RSS']
        tcpOffload  = $kwmap['*TCPChecksumOffloadIPv4']
        lsoV2       = $kwmap['*LsoV2IPv4']
    }
}
if ($adapters.Count -eq 0) { '[]' } else { @($adapters) | ConvertTo-Json -Depth 2 -Compress }
"#;
    match ps::run_json(script) {
        Ok(v @ Value::Array(_)) => json!({ "adapters": v }),
        Ok(v @ Value::Object(_)) => json!({ "adapters": [v] }),
        _ => json!({ "adapters": [] }),
    }
}

pub fn net_tweak(adapter: String, keyword: String, value: u32) -> Result<String, String> {
    let allowed = [
        "*InterruptModeration",
        "*RSS",
        "*TCPChecksumOffloadIPv4",
        "*LsoV2IPv4",
        "*UDPChecksumOffloadIPv4",
        "*IPChecksumOffloadIPv4",
    ];
    if !allowed.contains(&keyword.as_str()) {
        return Err(format!("Disallowed keyword: {keyword}"));
    }
    // Adapter names come from net_adapters(); they never contain path
    // separators, and a name with PS quote chars must never reach the script.
    if !crate::ps::is_safe_regkey_name(&adapter) {
        return Err("Invalid adapter name".into());
    }
    let script = format!(
        "Set-NetAdapterAdvancedProperty -Name '{adapter}' -RegistryKeyword '{keyword}' -RegistryValue {value} -ErrorAction Stop; 'OK'"
    );
    ps::run(&script).map(|_| format!("{adapter}: {keyword} = {value}"))
}

pub fn net_reset_all() -> Result<String, String> {
    // One "OK|name" / "ERR|name|message" line per adapter so a failure on any
    // adapter is reported instead of being swallowed.
    let script = r#"
Get-NetAdapter -Physical -ErrorAction Stop | Where-Object { $_.Status -eq 'Up' } | ForEach-Object {
    $n = $_.Name
    try {
        Reset-NetAdapterAdvancedProperty -Name $n -DisplayName '*' -ErrorAction Stop
        "OK|$n"
    } catch {
        "ERR|$n|$($_.Exception.Message -replace '[\r\n]+', ' ')"
    }
}
"#;
    summarize_net_reset(&ps::run(script)?)
}

fn summarize_net_reset(out: &str) -> Result<String, String> {
    let mut ok = Vec::new();
    let mut failed = Vec::new();
    for line in out.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let mut parts = line.splitn(3, '|');
        match (parts.next(), parts.next(), parts.next()) {
            (Some("OK"), Some(name), _) => ok.push(name.to_string()),
            (Some("ERR"), Some(name), msg) => {
                failed.push(format!("{name}: {}", msg.unwrap_or("unknown error")))
            }
            _ => {}
        }
    }
    if !failed.is_empty() {
        return Err(format!(
            "Reset failed for {} adapter(s): {}{}",
            failed.len(),
            failed.join("; "),
            if ok.is_empty() {
                String::new()
            } else {
                format!(" (reset OK: {})", ok.join(", "))
            }
        ));
    }
    if ok.is_empty() {
        return Ok("No connected physical adapters found; nothing was reset".into());
    }
    Ok(format!(
        "Advanced properties reset to driver defaults: {}",
        ok.join(", ")
    ))
}

// ── RAM usage (read-only) ─────────────────────────────────────────────────────

pub fn ram_info() -> Value {
    // Use sysinfo (already a dep) for reliable cross-version RAM info.
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_memory();
    let total_mb = sys.total_memory() / 1024 / 1024;
    let used_mb = sys.used_memory() / 1024 / 1024;
    let free_mb = sys.free_memory() / 1024 / 1024;

    // Standby + modified from perf counters (best-effort)
    // Win32_PerfFormattedData counter names are locale-independent,
    // unlike Get-Counter's English display paths.
    let standby_mb = ps::run(
        "try { $m = Get-CimInstance Win32_PerfFormattedData_PerfOS_Memory -EA Stop; [math]::Round(($m.StandbyCacheCoreBytes + $m.StandbyCacheNormalPriorityCacheBytes + $m.StandbyCacheReserveBytes)/1MB) } catch { 0 }"
    ).ok().and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(0);

    let modified_mb = ps::run(
        "try { [math]::Round((Get-CimInstance Win32_PerfFormattedData_PerfOS_Memory -EA Stop).ModifiedPageListBytes/1MB) } catch { 0 }"
    ).ok().and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(0);

    json!({
        "totalMb":    total_mb,
        "usedMb":     used_mb,
        "freeMb":     free_mb,
        "standbyMb":  standby_mb,
        "modifiedMb": modified_mb,
    })
}

// The "Flush Standby Cache" action (NtSetSystemInformation
// MemoryPurgeStandbyList) was removed: the standby list is reclaimable cache
// Windows hands to programs on demand, so purging it frees nothing a game
// could not already get, and the next file reads come from disk instead of
// RAM. It changed no persistent setting, so there is nothing to reset.

// ── Pagefile ─────────────────────────────────────────────────────────────────

pub fn pagefile_info() -> Value {
    let script = r#"
$cs  = Get-WmiObject Win32_ComputerSystem -ErrorAction SilentlyContinue
$pfs = Get-WmiObject Win32_PageFileSetting -ErrorAction SilentlyContinue
$pfu = Get-WmiObject Win32_PageFileUsage  -ErrorAction SilentlyContinue
$files = @()
if ($pfs) {
    foreach ($pf in @($pfs)) {
        $u = @($pfu) | Where-Object { $_.Name -eq $pf.Name } | Select-Object -First 1
        $files += [PSCustomObject]@{
            path      = $pf.Name
            initialMb = $pf.InitialSize
            maxMb     = $pf.MaximumSize
            currentMb = if ($u) { $u.CurrentUsage } else { 0 }
            peakMb    = if ($u) { $u.PeakUsage } else { 0 }
        }
    }
}
[PSCustomObject]@{
    autoManaged = [bool]$cs.AutomaticManagedPagefile
    ramGb       = [math]::Round($cs.TotalPhysicalMemory / 1GB, 1)
    files       = $files
} | ConvertTo-Json -Depth 3 -Compress
"#;
    ps::run_json(script).unwrap_or_else(|e| json!({ "error": e }))
}

pub fn pagefile_set_auto() -> Result<String, String> {
    let script = r#"
$cs = Get-WmiObject Win32_ComputerSystem
$cs.AutomaticManagedPagefile = $true
$cs.Put() | Out-Null
"Automatic pagefile management enabled, reboot required"
"#;
    ps::run(script).map(|s| s.trim().to_string())
}

pub fn pagefile_set_custom(path: String, init_mb: u32, max_mb: u32) -> Result<String, String> {
    // Pagefile paths must be local drive paths and must not contain PS quote
    // characters or wildcards, the value is embedded in single-quoted PS
    // strings and used in a `-like '{path}*'` comparison.
    let path = path.trim().to_string();
    let is_local = path.len() >= 3
        && path.as_bytes()[1] == b':'
        && path.as_bytes()[2] == b'\\'
        && path.as_bytes()[0].is_ascii_alphabetic();
    if !is_local || !crate::ps::is_safe_ident(&path) || path.contains(['*', '?', '[', ']']) {
        return Err("Invalid pagefile path".into());
    }
    if max_mb > 0 && init_mb > max_mb {
        return Err("Initial size cannot exceed maximum size".into());
    }
    // Create/update the new pagefile setting BEFORE removing any old entry so
    // the system never ends up without a pagefile when a later step fails.
    let script = format!(
        r#"
$cs = Get-WmiObject Win32_ComputerSystem
$cs.AutomaticManagedPagefile = $false
$cs.Put() | Out-Null
$pf = Get-WmiObject Win32_PageFileSetting -ErrorAction SilentlyContinue |
    Where-Object {{ $_.Name -eq '{path}' }} | Select-Object -First 1
if (-not $pf) {{
    $pf = ([WmiClass]'Win32_PageFileSetting').CreateInstance()
    $pf.Name = '{path}'
}}
$pf.InitialSize = {init_mb}
$pf.MaximumSize = {max_mb}
$pf.Put() | Out-Null
Get-WmiObject Win32_PageFileSetting -ErrorAction SilentlyContinue |
    Where-Object {{ $_.Name -like '{path}*' -and $_.Name -ne '{path}' }} |
    ForEach-Object {{ $_.Delete() }}
"Pagefile: {path} {init_mb}MB-{max_mb}MB, reboot required"
"#
    );
    ps::run(&script).map(|s| s.trim().to_string())
}

pub fn pagefile_disable() -> Result<String, String> {
    let script = r#"
$cs = Get-WmiObject Win32_ComputerSystem
$cs.AutomaticManagedPagefile = $false
$cs.Put() | Out-Null
Get-WmiObject Win32_PageFileSetting -ErrorAction SilentlyContinue | ForEach-Object { $_.Delete() }
"Pagefile disabled, reboot required (only recommended with 32 GB+ RAM)"
"#;
    ps::run(script).map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msi_only_listed_display_or_net_devices_are_allowed() {
        let gpu = r"HKLM:\SYSTEM\CurrentControlSet\Enum\PCI\VEN_10DE&DEV_2684\4&1";
        let devices = json!([
            { "type": "GPU", "regPath": gpu },
            { "type": "NIC", "regPath": r"HKLM:\SYSTEM\CurrentControlSet\Enum\PCI\VEN_8086&DEV_15F3\0" },
        ]);
        assert!(msi_path_allowed(&devices, gpu));
        assert!(msi_path_allowed(&devices, &gpu.to_lowercase()));
        // Boot storage controller: never in the Display/Net enumeration.
        assert!(!msi_path_allowed(
            &devices,
            r"HKLM:\SYSTEM\CurrentControlSet\Enum\PCI\VEN_144D&DEV_A80A\4&2"
        ));
        let single = json!({ "type": "GPU", "regPath": gpu });
        assert!(msi_path_allowed(&single, gpu));
        assert!(!msi_path_allowed(
            &json!({ "type": "Other", "regPath": gpu }),
            gpu
        ));
        assert!(!msi_path_allowed(&Value::Null, gpu));
    }

    #[test]
    fn net_reset_reports_per_adapter_failures() {
        assert!(summarize_net_reset("OK|Ethernet\r\nOK|Wi-Fi\r\n")
            .unwrap()
            .contains("Ethernet, Wi-Fi"));
        let err = summarize_net_reset("OK|Ethernet\nERR|Wi-Fi|Access denied\n").unwrap_err();
        assert!(err.contains("Wi-Fi: Access denied"));
        assert!(err.contains("reset OK: Ethernet"));
        assert!(summarize_net_reset("")
            .unwrap()
            .contains("nothing was reset"));
    }

    #[test]
    fn timer_scripts_never_reference_a_script_file_and_clean_up_legacy_helper() {
        assert!(TIMER_RESET_PS.contains(r"'ADHyperOptimize\timerres.ps1'"));
        assert!(!TIMER_RESET_PS.contains('\t'));
        assert!(TIMER_LEGACY_CLEANUP_PS.contains("ReparsePoint"));
    }
}
