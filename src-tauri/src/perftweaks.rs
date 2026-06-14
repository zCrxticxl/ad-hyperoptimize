//! Performance tweaks: Timer Resolution, MSI Mode, Network Adapter, RAM Standby, Pagefile.

use crate::ps;
use serde_json::{json, Value};

// ── Timer Resolution ──────────────────────────────────────────────────────────

pub fn timer_get() -> Value {
    // Use int (not uint) — PS [ref] binding works more reliably with signed int.
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
    // Steps:
    // 1. Call NtSetTimerResolution now to detect actual hardware minimum
    // 2. Set GlobalTimerResolutionRequests registry key (persists across boots)
    // 3. Write a PS1 helper script to ProgramData
    // 4. Register a logon-trigger scheduled task that runs it as SYSTEM
    // 5. Start the task immediately — now the process stays alive and holds the resolution
    // 6. Sleep 800ms so timer_get() called right after sees the updated value
    let script = format!(r#"
$target = {target_100ns}
$actual = 0
$msg = "Applied"
try {{
    Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class NtTimerS{{[DllImport("ntdll.dll")]public static extern int NtSetTimerResolution(int desired,bool set,out int actual);}}' -ErrorAction Stop
    [NtTimerS]::NtSetTimerResolution($target,$true,[ref]$actual)|Out-Null
    $achievedMs=[math]::Round($actual/10000.0,3)
    if($actual -gt ($target+1000)){{$msg="Hardware limit: ${{achievedMs}}ms (0.5ms not supported on this CPU)"}}
    else{{$msg="Set ${{achievedMs}}ms"}}
}}catch{{$msg="Applied (NtDll unavailable)"}}
try{{Set-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\kernel' -Name 'GlobalTimerResolutionRequests' -Value 1 -Type DWord -Force}}catch{{}}
$dir="$env:ProgramData\ADHyperOptimize"
$ps1="$dir\timerres.ps1"
try{{
    New-Item -Path $dir -ItemType Directory -Force|Out-Null
    @'
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class NtTP{{[DllImport("ntdll.dll")]public static extern int NtSetTimerResolution(int d,bool s,out int a);}}' -EA SilentlyContinue
$a=0;[NtTP]::NtSetTimerResolution({target_100ns},$true,[ref]$a)|Out-Null;while($true){{Start-Sleep -Seconds 3600}}
'@|Set-Content -Path $ps1 -Encoding UTF8
    $ta=New-ScheduledTaskAction -Execute 'powershell.exe' -Argument "-WindowStyle Hidden -NonInteractive -ExecutionPolicy Bypass -File `"$ps1`""
    $tt=New-ScheduledTaskTrigger -AtLogOn
    $ts=New-ScheduledTaskSettingsSet -Hidden -ExecutionTimeLimit (New-TimeSpan -Hours 0)
    $tp=New-ScheduledTaskPrincipal -UserId 'SYSTEM' -RunLevel Highest
    Register-ScheduledTask -TaskName 'ADHyperOptimize\TimerResolution' -Action $ta -Trigger $tt -Settings $ts -Principal $tp -Force|Out-Null
    Start-ScheduledTask -TaskPath '\ADHyperOptimize\' -TaskName 'TimerResolution' -EA SilentlyContinue
    Start-Sleep -Milliseconds 800
}}catch{{}}
$msg
"#);
    ps::run(&script).map(|s| s.trim().to_string()).map_err(|e| e)
}

pub fn timer_reset() -> Result<String, String> {
    let script = r#"
# Stop and remove the persistent scheduled task
try { Stop-ScheduledTask  -TaskPath '\ADHyperOptimize\' -TaskName 'TimerResolution' -EA SilentlyContinue } catch {}
try { Unregister-ScheduledTask -TaskPath '\ADHyperOptimize\' -TaskName 'TimerResolution' -Confirm:$false -EA SilentlyContinue } catch {}
# Kill any lingering timerres.ps1 powershell processes
Get-CimInstance Win32_Process -Filter "Name='powershell.exe'" -EA SilentlyContinue | Where-Object {
    $_.CommandLine -like '*timerres*'
} | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -EA SilentlyContinue }
# Remove registry key
try { Remove-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\kernel' -Name 'GlobalTimerResolutionRequests' -EA SilentlyContinue } catch {}
# Release NtSetTimerResolution in this process
try {
    Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class NtTimerR{[DllImport("ntdll.dll")]public static extern int NtSetTimerResolution(int desired,bool set,out int actual);}' -EA Stop
    $actual = 0
    [NtTimerR]::NtSetTimerResolution(156250,$false,[ref]$actual)|Out-Null
} catch {}
"Timer reset to default"
"#;
    ps::run(script).map(|s| s.trim().to_string()).map_err(|e| e)
}

// ── MSI Mode ─────────────────────────────────────────────────────────────────

pub fn msi_list() -> Value {
    // Search Enum\PCI directly — works regardless of class GUID location.
    let script = r#"
$results = @()
$pciBase = 'HKLM:\SYSTEM\CurrentControlSet\Enum\PCI'
if (Test-Path $pciBase) {
    Get-ChildItem $pciBase -ErrorAction SilentlyContinue | ForEach-Object {
        $devFolder = $_.PSPath
        Get-ChildItem $devFolder -ErrorAction SilentlyContinue | ForEach-Object {
            $instPath = $_.PSPath
            $props = Get-ItemProperty $instPath -ErrorAction SilentlyContinue
            $cls = $props.Class
            if ($cls -notin @('Display','Net')) { return }
            $name = if ($props.FriendlyName) { $props.FriendlyName }
                    elseif ($props.DeviceDesc) { $props.DeviceDesc } else { $null }
            if (-not $name) { return }
            $msiKey = Join-Path $instPath 'Device Parameters\Interrupt Management\MessageSignaledInterruptProperties'
            $msiEnabled = $false
            if (Test-Path $msiKey) {
                $mp = Get-ItemProperty $msiKey -ErrorAction SilentlyContinue
                $msiEnabled = ($mp.MSISupported -eq 1)
            }
            $results += [PSCustomObject]@{
                name       = $name
                type       = if ($cls -eq 'Display') { 'GPU' } else { 'NIC' }
                msiEnabled = $msiEnabled
                regPath    = $instPath -replace 'Microsoft\.PowerShell\.Core\\Registry::',''
            }
        }
    }
}
if ($results.Count -eq 0) { '[]' } else { @($results) | ConvertTo-Json -Depth 2 -Compress }
"#;
    match ps::run_json(script) {
        Ok(v @ Value::Array(_)) => json!({ "devices": v }),
        Ok(v @ Value::Object(_)) => json!({ "devices": [v] }),
        _ => json!({ "devices": [] }),
    }
}

pub fn msi_set(reg_path: String, enabled: bool) -> Result<String, String> {
    let val = if enabled { 1 } else { 0 };
    // reg_path is like HKEY_LOCAL_MACHINE\SYSTEM\... — convert to HKLM: for PS
    let ps_path = reg_path.replacen("HKEY_LOCAL_MACHINE", "HKLM:", 1);
    let script = format!(
        r#"
$msiKey = Join-Path '{ps_path}' 'Device Parameters\Interrupt Management\MessageSignaledInterruptProperties'
if (-not (Test-Path $msiKey)) {{ New-Item -Path $msiKey -Force | Out-Null }}
Set-ItemProperty -Path $msiKey -Name 'MSISupported' -Value {val} -Type DWord -Force
"OK — reboot required to take effect"
"#
    );
    ps::run(&script)
        .map(|_| format!("MSI {} — reboot required", if enabled { "enabled" } else { "disabled" }))
        .map_err(|e| e)
}

// ── Network Adapter Tweaks ────────────────────────────────────────────────────

pub fn net_adapters() -> Value {
    // No Status filter — include all physical adapters regardless of link state.
    let script = r#"
$adapters = @()
try {
    $all = Get-NetAdapter -ErrorAction SilentlyContinue | Where-Object {
        $_.HardwareInterface -eq $true -or $_.PhysicalMediaType -notin @('Unspecified','802.3')
    }
    # Fallback: just get everything if the above is empty
    if (-not $all) { $all = Get-NetAdapter -ErrorAction SilentlyContinue }
    $all | ForEach-Object {
        $n = $_.Name
        $props = Get-NetAdapterAdvancedProperty -Name $n -ErrorAction SilentlyContinue
        $gv = { param($kw) ($props | Where-Object { $_.RegistryKeyword -eq $kw } | Select-Object -First 1).DisplayValue }
        $adapters += [PSCustomObject]@{
            name        = $n
            description = $_.InterfaceDescription
            status      = $_.Status
            speedMbps   = if ($_.LinkSpeed) { [math]::Round($_.LinkSpeed / 1e6) } else { 0 }
            intMod      = & $gv '*InterruptModeration'
            rss         = & $gv '*RSS'
            tcpOffload  = & $gv '*TCPChecksumOffloadIPv4'
            lsoV2       = & $gv '*LsoV2IPv4'
        }
    }
} catch {}
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
    let script = format!(
        "Set-NetAdapterAdvancedProperty -Name '{adapter}' -RegistryKeyword '{keyword}' -RegistryValue {value} -ErrorAction Stop; 'OK'"
    );
    ps::run(&script)
        .map(|_| format!("{adapter}: {keyword} = {value}"))
        .map_err(|e| e)
}

// Disable all optimisable properties on all active physical adapters in one shot
pub fn net_tweak_all_gaming() -> Result<String, String> {
    let script = r#"
$changed = 0
Get-NetAdapter -Physical | Where-Object { $_.Status -eq 'Up' } | ForEach-Object {
    $n = $_.Name
    $props = Get-NetAdapterAdvancedProperty -Name $n -ErrorAction SilentlyContinue
    $keywords = $props.RegistryKeyword
    foreach ($kw in @('*InterruptModeration','*RSS','*TCPChecksumOffloadIPv4','*LsoV2IPv4','*UDPChecksumOffloadIPv4','*IPChecksumOffloadIPv4')) {
        if ($keywords -contains $kw) {
            try {
                Set-NetAdapterAdvancedProperty -Name $n -RegistryKeyword $kw -RegistryValue 0 -ErrorAction Stop
                $changed++
            } catch {}
        }
    }
}
"$changed properties updated"
"#;
    ps::run(script)
        .map(|s| s.trim().to_string())
        .map_err(|e| e)
}

pub fn net_reset_all() -> Result<String, String> {
    let script = r#"
Get-NetAdapter -Physical | Where-Object { $_.Status -eq 'Up' } | ForEach-Object {
    Reset-NetAdapterAdvancedProperty -Name $_.Name -DisplayName '*' -ErrorAction SilentlyContinue
}
"All adapter advanced properties reset to driver defaults"
"#;
    ps::run(script)
        .map(|s| s.trim().to_string())
        .map_err(|e| e)
}

// ── RAM Standby Cleaner ───────────────────────────────────────────────────────

pub fn ram_info() -> Value {
    // Use sysinfo (already a dep) for reliable cross-version RAM info.
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_memory();
    let total_mb = sys.total_memory() / 1024 / 1024;
    let used_mb  = sys.used_memory()  / 1024 / 1024;
    let free_mb  = sys.free_memory()  / 1024 / 1024;

    // Standby + modified from perf counters (best-effort)
    let standby_mb = ps::run(
        "try { [math]::Round((Get-Counter '\\Memory\\Standby Cache Total Bytes' -EA Stop).CounterSamples[0].CookedValue/1MB) } catch { 0 }"
    ).ok().and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(0);

    let modified_mb = ps::run(
        "try { [math]::Round((Get-Counter '\\Memory\\Modified Page List Bytes' -EA Stop).CounterSamples[0].CookedValue/1MB) } catch { 0 }"
    ).ok().and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(0);

    json!({
        "totalMb":    total_mb,
        "usedMb":     used_mb,
        "freeMb":     free_mb,
        "standbyMb":  standby_mb,
        "modifiedMb": modified_mb,
    })
}

pub fn ram_flush_standby() -> Result<String, String> {
    // NtSetSystemInformation(80 = SystemMemoryListInformation, payload=4 = MemoryPurgeStandbyList)
    let script = r#"
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class RamFlush {
    [DllImport("ntdll.dll")] public static extern uint NtSetSystemInformation(int cls, IntPtr buf, int len);
    public static uint PurgeStandby() {
        var p = Marshal.AllocHGlobal(4);
        Marshal.WriteInt32(p, 4);
        try { return NtSetSystemInformation(80, p, 4); }
        finally { Marshal.FreeHGlobal(p); }
    }
}
'@
$r = [RamFlush]::PurgeStandby()
if ($r -eq 0) { "Standby list flushed successfully" } else { "Flush result: 0x$($r.ToString('X8'))" }
"#;
    ps::run(script)
        .map(|s| s.trim().to_string())
        .map_err(|e| e)
}

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
"Automatic pagefile management enabled — reboot required"
"#;
    ps::run(script)
        .map(|s| s.trim().to_string())
        .map_err(|e| e)
}

pub fn pagefile_set_custom(path: String, init_mb: u32, max_mb: u32) -> Result<String, String> {
    let script = format!(
        r#"
$cs = Get-WmiObject Win32_ComputerSystem
$cs.AutomaticManagedPagefile = $false
$cs.Put() | Out-Null
Get-WmiObject Win32_PageFileSetting -ErrorAction SilentlyContinue |
    Where-Object {{ $_.Name -like '{path}*' }} |
    ForEach-Object {{ $_.Delete() }}
$pf = ([WmiClass]'Win32_PageFileSetting').CreateInstance()
$pf.Name        = '{path}'
$pf.InitialSize = {init_mb}
$pf.MaximumSize = {max_mb}
$pf.Put() | Out-Null
"Pagefile: {path} {init_mb}MB–{max_mb}MB — reboot required"
"#
    );
    ps::run(&script)
        .map(|s| s.trim().to_string())
        .map_err(|e| e)
}

pub fn pagefile_disable() -> Result<String, String> {
    let script = r#"
$cs = Get-WmiObject Win32_ComputerSystem
$cs.AutomaticManagedPagefile = $false
$cs.Put() | Out-Null
Get-WmiObject Win32_PageFileSetting -ErrorAction SilentlyContinue | ForEach-Object { $_.Delete() }
"Pagefile disabled — reboot required (only recommended with 32 GB+ RAM)"
"#;
    ps::run(script)
        .map(|s| s.trim().to_string())
        .map_err(|e| e)
}
