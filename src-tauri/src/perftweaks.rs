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
    // The script can fail after the registry write or the task registration
    // ("did not stay running"): restore the captured state instead of
    // leaving an open, half-applied entry.
    let msg = match ps::run(&script) {
        Ok(out) => out.trim().to_string(),
        Err(e) => {
            return match crate::tweaks::revert_entry(&entry_id) {
                Ok(_) => Err(format!("{e}; all changes were rolled back")),
                Err(re) => Err(format!(
                    "{e}; rollback incomplete ({re}); restore token: {entry_id}"
                )),
            }
        }
    };
    crate::safety::with_journal(|entries| {
        let entry = entries
            .iter_mut()
            .find(|entry| entry.id == entry_id)
            .ok_or("Timer journal entry missing")?;
        entry.completed = true;
        Ok(())
    })?;
    Ok(msg)
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

// ── Journal plumbing for MSI / NIC / page file ───────────────────────────────
// Every change below is captured first and written ahead to the journal, the
// same write-ahead → apply → undo pattern the tweak engine uses: a failure
// rolls back what was captured, and Undo restores the exact earlier state
// (including "value not set") instead of writing the opposite value.

use crate::safety::{ChangeItem, JournalEntry};

/// Open (applied, not undone) entry of `tweak_id`.
pub(crate) fn open_entry(tweak_id: &str) -> Option<JournalEntry> {
    crate::safety::load_journal()
        .unwrap_or_default()
        .into_iter()
        .rev()
        .find(|e| e.tweak_id == tweak_id && !e.reverted && (e.completed || e.attempted > 0))
}

/// Journal `items` write-ahead, apply them in order and mark the entry
/// complete. On a failure the applied part is rolled back.
pub(crate) fn apply_journaled(
    tweak_id: &str,
    name: &str,
    items: Vec<ChangeItem>,
) -> Result<String, String> {
    let entry_id = crate::safety::append_entry(JournalEntry {
        id: format!("perf-{}", chrono::Local::now().format("%Y%m%d%H%M%S%6f")),
        tweak_id: tweak_id.into(),
        tweak_name: name.into(),
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
    for (idx, item) in items.iter().enumerate() {
        crate::safety::with_journal(|j| {
            let entry = j
                .iter_mut()
                .find(|e| e.id == entry_id)
                .ok_or("journal entry missing")?;
            entry.attempted = idx + 1;
            Ok(())
        })?;
        if let Err(e) = crate::tweaks::apply_item(item) {
            return match crate::tweaks::rollback_entry_items(&entry_id, &items[..=idx]) {
                Ok(()) => Err(format!("{e}; changes were rolled back")),
                Err(re) => Err(format!(
                    "{e}; rollback incomplete ({re}); undo it from the change history"
                )),
            };
        }
    }
    crate::safety::with_journal(|j| {
        let entry = j
            .iter_mut()
            .find(|e| e.id == entry_id)
            .ok_or("journal entry missing")?;
        entry.completed = true;
        Ok(())
    })?;
    Ok(entry_id)
}

/// A PowerShell expression as a journal command line (run by
/// `tweaks::run_cmdline` through `ps::run`, never re-split).
pub(crate) fn ps_line(script: &str) -> String {
    format!("powershell -Command {script}")
}

/// Open entries of this page (MSI, NIC, page file), for its Undo list. The
/// timer has its own reset button.
pub fn open_entries() -> Value {
    let list: Vec<Value> = crate::safety::load_journal()
        .unwrap_or_default()
        .iter()
        .filter(|e| {
            e.tweak_id.starts_with("perftweaks:")
                && e.tweak_id != "perftweaks:timer"
                && e.completed
                && !e.reverted
        })
        .map(|e| json!({ "id": e.id, "name": e.tweak_name, "time": e.time }))
        .collect();
    json!({ "entries": list })
}

/// Undo a change of this page; only entries of this page are accepted.
pub fn undo(entry_id: String) -> Result<Value, String> {
    let known = crate::safety::load_journal()
        .unwrap_or_default()
        .iter()
        .any(|e| e.id == entry_id && e.tweak_id.starts_with("perftweaks:"));
    if !known {
        return Err("unknown entry".into());
    }
    crate::tweaks::revert_entry(&entry_id)
}

// ── Interrupt affinity ───────────────────────────────────────────────────────

// Per-device "Affinity Policy" (documented Interrupt Affinity Policy keys):
// DevicePolicy 4 = IrqPolicySpecifiedProcessors with the processor mask in
// AssignmentSetOverride (KAFFINITY, 8 bytes on x64). Takes effect after the
// device restarts (reboot). Systems with more than 64 logical processors use
// processor groups and are not supported here.
const IRQ_SUBKEY: &str = r"Device Parameters\Interrupt Management\Affinity Policy";

const IRQ_LIST_PS: &str = r#"
$cpus = [Environment]::ProcessorCount
$devs = @(Get-PnpDevice -PresentOnly -ErrorAction SilentlyContinue |
    Where-Object { $_.InstanceId -like 'PCI\*' -and $_.Class -in @('Display','Net','USB') } |
    ForEach-Object {
        $inst = "HKLM:\SYSTEM\CurrentControlSet\Enum\$($_.InstanceId)"
        $ap = Get-ItemProperty (Join-Path $inst 'Device Parameters\Interrupt Management\Affinity Policy') -ErrorAction SilentlyContinue
        $mask = $null
        if ($ap -and $ap.AssignmentSetOverride) { $mask = ($ap.AssignmentSetOverride | ForEach-Object { $_.ToString('x2') }) -join '' }
        [PSCustomObject]@{
            name = $_.FriendlyName
            type = switch ($_.Class) { 'Display' { 'GPU' } 'Net' { 'NIC' } default { 'USB' } }
            regPath = $inst
            policy = if ($ap) { $ap.DevicePolicy } else { $null }
            mask = $mask
        } })
@{ cpus = $cpus; devices = $devs } | ConvertTo-Json -Depth 3 -Compress
"#;

/// Little-endian KAFFINITY hex for one logical processor.
pub fn affinity_hex(core: u32) -> String {
    (1u64 << core)
        .to_le_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Logical processors set in a little-endian mask hex string.
pub fn cores_of_mask(hex: &str) -> Vec<u32> {
    let bytes: Vec<u8> = (0..hex.len() / 2)
        .filter_map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok())
        .collect();
    let mut cores = Vec::new();
    for (i, b) in bytes.iter().enumerate() {
        for bit in 0..8 {
            if b & (1 << bit) != 0 {
                cores.push((i * 8 + bit) as u32);
            }
        }
    }
    cores
}

pub fn irq_list() -> Value {
    let v = ps::run_json(IRQ_LIST_PS).unwrap_or(Value::Null);
    let devices: Vec<Value> = match &v["devices"] {
        Value::Array(a) => a.clone(),
        o @ Value::Object(_) => vec![o.clone()],
        _ => vec![],
    }
    .into_iter()
    .map(|mut d| {
        let inst = d["regPath"]
            .as_str()
            .unwrap_or_default()
            .trim_start_matches(r"HKLM:\")
            .to_string();
        d["cores"] = json!(d["mask"].as_str().map(cores_of_mask));
        d["openEntry"] = json!(open_entry(&format!("perftweaks:irq:{inst}")).map(|e| e.id));
        d
    })
    .collect();
    json!({ "cpus": v["cpus"], "devices": devices })
}

/// Pins a device's interrupts to one logical processor (`core`), or with
/// `None` undoes the app's earlier change. Journaled; reboot required.
pub fn irq_set(reg_path: String, core: Option<u32>) -> Result<String, String> {
    if !reg_path.starts_with(r"HKLM:\SYSTEM\CurrentControlSet\Enum\PCI\")
        || !crate::ps::is_safe_ident(&reg_path)
        || reg_path.contains(['*', '?', '[', ']'])
        || reg_path.split('\\').any(|seg| seg == ".." || seg == ".")
    {
        return Err("Invalid device path".into());
    }
    let list = ps::run_json(IRQ_LIST_PS).map_err(|e| format!("Cannot verify device list: {e}"))?;
    let known = match &list["devices"] {
        Value::Array(a) => a.iter().any(|d| {
            d["regPath"]
                .as_str()
                .is_some_and(|p| p.eq_ignore_ascii_case(&reg_path))
        }),
        o @ Value::Object(_) => o["regPath"]
            .as_str()
            .is_some_and(|p| p.eq_ignore_ascii_case(&reg_path)),
        _ => false,
    };
    if !known {
        return Err("Only a present GPU, network or USB controller can be changed".into());
    }
    let cpus = list["cpus"].as_u64().unwrap_or(0) as u32;
    let instance = &reg_path[r"HKLM:\".len()..];
    let tweak_id = format!("perftweaks:irq:{instance}");
    let _engine = crate::tweaks::engine_guard()?;
    if let Some(open) = open_entry(&tweak_id) {
        drop(_engine);
        if core.is_none() {
            crate::tweaks::revert_entry(&open.id)?;
            return Ok("Interrupt affinity restored, reboot required".into());
        }
        return Err("An affinity change for this device is still open; undo it first".into());
    }
    let core = core.ok_or("Nothing to undo for this device")?;
    if cpus == 0 || cpus > 64 {
        return Err("Only systems with 1 to 64 logical processors are supported".into());
    }
    if core >= cpus {
        return Err(format!("Processor {core} does not exist (0..{})", cpus - 1));
    }
    let key = format!(r"{instance}\{IRQ_SUBKEY}");
    let policy_prev = crate::tweaks::reg_read("HKLM", &key, "DevicePolicy")?;
    let mask_prev = crate::tweaks::reg_read("HKLM", &key, "AssignmentSetOverride")?;
    apply_journaled(
        &tweak_id,
        &format!("Interrupts to CPU {core} ({instance})"),
        vec![
            ChangeItem::Registry {
                root: "HKLM".into(),
                path: key.clone(),
                name: "AssignmentSetOverride".into(),
                prev: mask_prev,
                new: crate::safety::RegVal::Raw {
                    ty: "REG_BINARY".into(),
                    hex: affinity_hex(core),
                },
            },
            ChangeItem::Registry {
                root: "HKLM".into(),
                path: key,
                name: "DevicePolicy".into(),
                prev: policy_prev,
                new: crate::safety::RegVal::Dword(4),
            },
        ],
    )?;
    Ok(format!("Interrupts pinned to CPU {core}, reboot required"))
}

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

const MSI_SUBKEY: &str =
    r"Device Parameters\Interrupt Management\MessageSignaledInterruptProperties";

pub fn msi_set(reg_path: String, enabled: bool) -> Result<String, String> {
    let val = if enabled { 1 } else { 0 };
    // regPath from msi_list() is already "HKLM:\SYSTEM\..." (PS-ready); this
    // replacen is a no-op for that shape, kept only for back-compat with any
    // stale HKEY_LOCAL_MACHINE-prefixed value a client might still send.
    let ps_path = reg_path.replacen("HKEY_LOCAL_MACHINE", "HKLM:", 1);
    // Only device instances the scan produced (HKLM:\SYSTEM\CurrentControlSet\Enum\…);
    // wildcards and quote characters are refused.
    if !ps_path.starts_with(r"HKLM:\SYSTEM\CurrentControlSet\Enum\")
        || !crate::ps::is_safe_ident(&ps_path)
        || ps_path.contains(['*', '?', '[', ']'])
        || ps_path.split('\\').any(|seg| seg == ".." || seg == ".")
    {
        return Err("Invalid registry path for MSI mode".into());
    }
    let devices =
        ps::run_json(MSI_LIST_PS).map_err(|e| format!("Cannot verify MSI device list: {e}"))?;
    if !msi_path_allowed(&devices, &ps_path) {
        return Err("MSI mode can only be changed for a present display or network adapter".into());
    }
    let _engine = crate::tweaks::engine_guard()?;
    let instance = &ps_path[r"HKLM:\".len()..];
    let tweak_id = format!("perftweaks:msi:{instance}");
    let key = format!(r"{instance}\{MSI_SUBKEY}");
    if let Some(open) = open_entry(&tweak_id) {
        // Switching back is the undo of the open change, not a second change
        // on top of it (which would capture this app's own value as "before").
        let back = open.items.iter().any(|i| {
            // "Not set" is the driver default, not 0: only an explicit
            // earlier value counts as switching back.
            matches!(i, ChangeItem::Registry { prev: Some(crate::safety::RegVal::Dword(v)), .. } if *v == val)
        });
        drop(_engine);
        if back {
            crate::tweaks::revert_entry(&open.id)?;
            return Ok("MSI change undone, reboot required".into());
        }
        return Err("An MSI change for this device is still open; undo it first".into());
    }
    let prev = crate::tweaks::reg_read("HKLM", &key, "MSISupported")?;
    apply_journaled(
        &tweak_id,
        &format!(
            "MSI mode {} ({instance})",
            if enabled { "on" } else { "off" }
        ),
        vec![ChangeItem::Registry {
            root: "HKLM".into(),
            path: key,
            name: "MSISupported".into(),
            prev,
            new: crate::safety::RegVal::Dword(val),
        }],
    )?;
    Ok(format!(
        "MSI {}, reboot required",
        if enabled { "enabled" } else { "disabled" }
    ))
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
    # State from RegistryValue (locale-independent: "0"/"1"/"3"); the
    # localized DisplayValue is only shown next to it.
    $kwmap = @{}
    $txtmap = @{}
    try {
        Get-NetAdapterAdvancedProperty -Name $name -ErrorAction SilentlyContinue | ForEach-Object {
            $kwmap[$_.RegistryKeyword] = ($_.RegistryValue -join ',')
            $txtmap[$_.RegistryKeyword] = $_.DisplayValue
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
        intModText     = $txtmap['*InterruptModeration']
        rssText        = $txtmap['*RSS']
        tcpOffloadText = $txtmap['*TCPChecksumOffloadIPv4']
        lsoV2Text      = $txtmap['*LsoV2IPv4']
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

const NET_KEYWORDS: &[&str] = &[
    "*InterruptModeration",
    "*RSS",
    "*TCPChecksumOffloadIPv4",
    "*LsoV2IPv4",
    "*UDPChecksumOffloadIPv4",
    "*IPChecksumOffloadIPv4",
];

fn net_set_line(adapter: &str, keyword: &str, value: &str) -> String {
    ps_line(&format!(
        "Set-NetAdapterAdvancedProperty -Name {} -RegistryKeyword {} -RegistryValue {} -ErrorAction Stop",
        ps::quote_single(adapter),
        ps::quote_single(keyword),
        ps::quote_single(value)
    ))
}

pub fn net_tweak(adapter: String, keyword: String, value: u32) -> Result<String, String> {
    if !NET_KEYWORDS.contains(&keyword.as_str()) {
        return Err(format!("Disallowed keyword: {keyword}"));
    }
    // Adapter names come from net_adapters(); they never contain path
    // separators, and a name with PS quote chars must never reach the script.
    if !crate::ps::is_safe_regkey_name(&adapter) {
        return Err("Invalid adapter name".into());
    }
    let _engine = crate::tweaks::engine_guard()?;
    let tweak_id = format!("perftweaks:nic:{adapter}:{keyword}");
    let new = value.to_string();
    if let Some(open) = open_entry(&tweak_id) {
        // Setting the value from before the open change is its undo.
        let back = open.items.iter().any(|i| {
            matches!(i, ChangeItem::Command { revert, .. } if *revert == net_set_line(&adapter, &keyword, &new))
        });
        drop(_engine);
        if back {
            crate::tweaks::revert_entry(&open.id)?;
            return Ok(format!("{adapter}: {keyword} restored"));
        }
        return Err(format!(
            "A change of {keyword} on {adapter} is still open; undo it first"
        ));
    }
    let prev = ps::run(&format!(
        "(Get-NetAdapterAdvancedProperty -Name {} -RegistryKeyword {} -ErrorAction Stop).RegistryValue -join ','",
        ps::quote_single(&adapter),
        ps::quote_single(&keyword)
    ))
    .map_err(|e| format!("Cannot read {keyword} on {adapter}: {}", e.trim()))?;
    let prev = prev.trim().to_string();
    if prev.is_empty() || !prev.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "{keyword} on {adapter} has an unexpected value ('{prev}'); nothing was changed"
        ));
    }
    if prev == new {
        return Ok(format!("{adapter}: {keyword} is already {new}"));
    }
    apply_journaled(
        &tweak_id,
        &format!("NIC {adapter}: {keyword} {prev} → {new}"),
        vec![ChangeItem::Command {
            applied: net_set_line(&adapter, &keyword, &new),
            revert: net_set_line(&adapter, &keyword, &prev),
        }],
    )?;
    Ok(format!("{adapter}: {keyword} = {value}"))
}

/// Restores every captured advanced property of one adapter, then restarts
/// it once. A property that cannot be restored fails the undo (it can be
/// retried) instead of being skipped silently.
fn net_restore_script(adapter: &str, props: &[(String, Vec<String>)]) -> String {
    let name = ps::quote_single(adapter);
    let mut script = String::from("$e=@(); ");
    for (keyword, values) in props {
        let list = values
            .iter()
            .map(|v| ps::quote_single(v))
            .collect::<Vec<_>>()
            .join(",");
        script.push_str(&format!(
            "try {{ Set-NetAdapterAdvancedProperty -Name {name} -RegistryKeyword {} -RegistryValue @({list}) -NoRestart -ErrorAction Stop }} catch {{ $e += {} }}; ",
            ps::quote_single(keyword),
            ps::quote_single(keyword)
        ));
    }
    script.push_str(&format!(
        "Restart-NetAdapter -Name {name} -Confirm:$false -ErrorAction SilentlyContinue; if ($e.Count) {{ throw ('not restored: ' + ($e -join ', ')) }}"
    ));
    script
}

/// Advanced properties of one adapter: (keyword, registry values).
type NetProps = Vec<(String, Vec<String>)>;

/// Parses the capture of `net_reset_all`: adapters with their properties.
fn parse_net_capture(v: &Value) -> Vec<(String, NetProps)> {
    let rows: Vec<&Value> = match v {
        Value::Array(a) => a.iter().collect(),
        o @ Value::Object(_) => vec![o],
        _ => vec![],
    };
    rows.iter()
        .filter_map(|r| {
            let name = r["name"].as_str()?.to_string();
            let props = match &r["props"] {
                Value::Array(a) => a.iter().collect::<Vec<_>>(),
                o @ Value::Object(_) => vec![o],
                _ => vec![],
            };
            let props = props
                .iter()
                .filter_map(|p| {
                    let k = p["k"].as_str()?.to_string();
                    let vals: Vec<String> = match &p["v"] {
                        Value::Array(a) => a
                            .iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect(),
                        Value::String(s) => vec![s.clone()],
                        _ => vec![],
                    };
                    (!vals.is_empty()).then_some((k, vals))
                })
                .collect();
            Some((name, props))
        })
        .collect()
}

pub fn net_reset_all() -> Result<String, String> {
    let _engine = crate::tweaks::engine_guard()?;
    if let Some(open) = open_entry("perftweaks:nicreset") {
        return Err(format!(
            "The adapter reset from {} is still open; undo it first",
            open.time
        ));
    }
    // Capture every advanced property first, so the reset can be undone.
    let captured = ps::run_json(
        r#"
@(Get-NetAdapter -Physical -ErrorAction Stop | Where-Object { $_.Status -eq 'Up' } | ForEach-Object {
    $n = $_.Name
    [PSCustomObject]@{
        name  = $n
        props = @(Get-NetAdapterAdvancedProperty -Name $n -ErrorAction Stop |
            Where-Object { $_.RegistryKeyword } |
            ForEach-Object { [PSCustomObject]@{ k = $_.RegistryKeyword; v = @($_.RegistryValue | ForEach-Object { [string]$_ }) } })
    }
})
"#,
    )
    .map_err(|e| format!("Cannot capture adapter settings, nothing was reset: {e}"))?;
    let adapters = parse_net_capture(&captured);
    if adapters.is_empty() {
        return Ok("No connected physical adapters found; nothing was reset".into());
    }
    let names: Vec<String> = adapters.iter().map(|(n, _)| n.clone()).collect();
    let items = adapters
        .iter()
        .map(|(name, props)| ChangeItem::Command {
            applied: ps_line(&format!(
                "Reset-NetAdapterAdvancedProperty -Name {} -DisplayName '*' -ErrorAction Stop",
                ps::quote_single(name)
            )),
            revert: ps_line(&net_restore_script(name, props)),
        })
        .collect();
    apply_journaled(
        "perftweaks:nicreset",
        &format!("NIC reset to driver defaults ({})", names.join(", ")),
        items,
    )?;
    Ok(format!(
        "Advanced properties reset to driver defaults: {}",
        names.join(", ")
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

/// Page-file configuration as captured before a change.
struct PagefileState {
    auto: bool,
    files: Vec<(String, u32, u32)>,
}

fn pagefile_capture() -> Result<PagefileState, String> {
    let v = ps::run_json(
        r#"
$cs = Get-WmiObject Win32_ComputerSystem -ErrorAction Stop
[PSCustomObject]@{
    auto  = [bool]$cs.AutomaticManagedPagefile
    files = @(Get-WmiObject Win32_PageFileSetting -ErrorAction Stop | ForEach-Object {
        [PSCustomObject]@{ name = $_.Name; init = [int]$_.InitialSize; max = [int]$_.MaximumSize } })
}
"#,
    )
    .map_err(|e| format!("Cannot read the page-file configuration, nothing was changed: {e}"))?;
    let files = match &v["files"] {
        Value::Array(a) => a.iter().collect::<Vec<_>>(),
        o @ Value::Object(_) => vec![o],
        _ => vec![],
    };
    let mut out = Vec::new();
    for f in files {
        let name = f["name"].as_str().unwrap_or("").to_string();
        if !pagefile_path_ok(&name) {
            return Err(format!(
                "Unexpected page-file entry '{name}'; nothing was changed"
            ));
        }
        out.push((
            name,
            f["init"].as_u64().unwrap_or(0) as u32,
            f["max"].as_u64().unwrap_or(0) as u32,
        ));
    }
    Ok(PagefileState {
        auto: v["auto"].as_bool().ok_or("page-file state unreadable")?,
        files: out,
    })
}

fn pagefile_path_ok(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && b[2] == b'\\'
        && crate::ps::is_safe_ident(path)
        && !path.contains(['*', '?', '[', ']'])
}

/// Script that sets exactly `state`: entries not in it are removed.
fn pagefile_state_script(state: &PagefileState) -> String {
    let mut s = String::from(
        "$cs = Get-WmiObject Win32_ComputerSystem -ErrorAction Stop; $cs.AutomaticManagedPagefile = $false; $cs.Put() | Out-Null; ",
    );
    // Create/update the wanted entries BEFORE removing others, so the system
    // never ends up without a page file when a later step fails.
    for (name, init, max) in &state.files {
        let q = ps::quote_single(name);
        s.push_str(&format!(
            "$pf = Get-WmiObject Win32_PageFileSetting -ErrorAction Stop | Where-Object {{ $_.Name -eq {q} }} | Select-Object -First 1; \
             if (-not $pf) {{ $pf = ([WmiClass]'Win32_PageFileSetting').CreateInstance(); $pf.Name = {q} }}; \
             $pf.InitialSize = {init}; $pf.MaximumSize = {max}; $pf.Put() | Out-Null; "
        ));
    }
    let keep = state
        .files
        .iter()
        .map(|(n, _, _)| ps::quote_single(n))
        .collect::<Vec<_>>()
        .join(",");
    s.push_str(&format!(
        "$keep = @({keep}); Get-WmiObject Win32_PageFileSetting -ErrorAction Stop | Where-Object {{ $keep -notcontains $_.Name }} | ForEach-Object {{ $_.Delete() }}; "
    ));
    if state.auto {
        s.push_str("$cs = Get-WmiObject Win32_ComputerSystem -ErrorAction Stop; $cs.AutomaticManagedPagefile = $true; $cs.Put() | Out-Null; ");
    }
    s.push_str("'OK'");
    s
}

fn pagefile_change(
    label: &str,
    target: impl FnOnce(&PagefileState) -> PagefileState,
) -> Result<String, String> {
    let _engine = crate::tweaks::engine_guard()?;
    if let Some(open) = open_entry("perftweaks:pagefile") {
        return Err(format!(
            "The page-file change from {} is still open; undo it first",
            open.time
        ));
    }
    let before = pagefile_capture()?;
    let target = target(&before);
    apply_journaled(
        "perftweaks:pagefile",
        &format!("Page file: {label}"),
        vec![ChangeItem::Command {
            applied: ps_line(&pagefile_state_script(&target)),
            revert: ps_line(&pagefile_state_script(&before)),
        }],
    )?;
    Ok(format!("Page file: {label}, reboot required"))
}

pub fn pagefile_set_auto() -> Result<String, String> {
    pagefile_change("managed by Windows", |_| PagefileState {
        auto: true,
        files: vec![],
    })
}

pub fn pagefile_set_custom(path: String, init_mb: u32, max_mb: u32) -> Result<String, String> {
    let path = path.trim().to_string();
    if !pagefile_path_ok(&path) {
        return Err("Invalid pagefile path".into());
    }
    if max_mb > 0 && init_mb > max_mb {
        return Err("Initial size cannot exceed maximum size".into());
    }
    let label = format!("{path} {init_mb}-{max_mb} MB");
    // Page files on other drives stay as they are.
    pagefile_change(&label, |before| {
        let mut files: Vec<(String, u32, u32)> = before
            .files
            .iter()
            .filter(|(n, _, _)| !n.eq_ignore_ascii_case(&path))
            .cloned()
            .collect();
        files.push((path.clone(), init_mb, max_mb));
        PagefileState { auto: false, files }
    })
}

pub fn pagefile_disable() -> Result<String, String> {
    pagefile_change("disabled", |_| PagefileState {
        auto: false,
        files: vec![],
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn affinity_masks() {
        assert_eq!(super::affinity_hex(0), "0100000000000000");
        assert_eq!(super::affinity_hex(9), "0002000000000000");
        assert_eq!(super::cores_of_mask("0002000000000000"), vec![9]);
        assert_eq!(super::cores_of_mask("0300"), vec![0, 1]);
    }

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
    fn net_capture_is_parsed_and_restored_quoted() {
        let v = json!([{ "name": "Ethernet", "props": [
            { "k": "*RSS", "v": ["1"] },
            { "k": "NetworkAddress", "v": "00'11" },
            { "k": "*Empty", "v": [] }
        ]}]);
        let parsed = parse_net_capture(&v);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].1.len(), 2, "empty values are not restored");
        let script = net_restore_script(&parsed[0].0, &parsed[0].1);
        assert!(script.contains("-RegistryKeyword '*RSS' -RegistryValue @('1')"));
        assert!(script.contains("@('00''11')"), "{script}");
        assert!(script.contains("throw"));
        // A single adapter arrives as a bare object.
        assert_eq!(parse_net_capture(&v[0]).len(), 1);
    }

    #[test]
    fn pagefile_script_sets_exactly_the_captured_state() {
        let state = PagefileState {
            auto: false,
            files: vec![(r"D:\pagefile.sys".into(), 1024, 4096)],
        };
        let s = pagefile_state_script(&state);
        assert!(s.contains(r"$pf.Name = 'D:\pagefile.sys'"));
        assert!(s.contains("$pf.InitialSize = 1024; $pf.MaximumSize = 4096"));
        assert!(s.contains(r"$keep = @('D:\pagefile.sys')"));
        assert!(!s.contains("AutomaticManagedPagefile = $true"));
        let auto = pagefile_state_script(&PagefileState {
            auto: true,
            files: vec![],
        });
        assert!(auto.contains("$keep = @()"));
        assert!(auto.contains("AutomaticManagedPagefile = $true"));
        assert!(pagefile_path_ok(r"C:\pagefile.sys"));
        assert!(!pagefile_path_ok(r"C:\page'file.sys"));
        assert!(!pagefile_path_ok(r"\\server\share\pagefile.sys"));
        assert!(!pagefile_path_ok(r"C:\*.sys"));
    }

    #[test]
    fn timer_scripts_never_reference_a_script_file_and_clean_up_legacy_helper() {
        assert!(TIMER_RESET_PS.contains(r"'ADHyperOptimize\timerres.ps1'"));
        assert!(!TIMER_RESET_PS.contains('\t'));
        assert!(TIMER_LEGACY_CLEANUP_PS.contains("ReparsePoint"));
    }
}
