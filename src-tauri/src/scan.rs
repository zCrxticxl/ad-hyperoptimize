//! Full system analysis. Every section is fetched independently and failures
//! degrade to `{"error": "..."}` instead of aborting the whole scan.

use crate::ps;
use serde_json::{json, Value};

fn section(script: &str) -> Value {
    match ps::run_json(script) {
        Ok(v) => v,
        Err(e) => json!({ "error": e.trim() }),
    }
}

pub fn full_scan() -> Value {
    json!({
        "timestamp": chrono::Local::now().to_rfc3339(),
        "os": section(
            "$os = Get-CimInstance Win32_OperatingSystem; \
             $perf = Get-CimInstance Win32_PerfFormattedData_PerfOS_Memory; \
             [PSCustomObject]@{ \
               Caption=$os.Caption; Version=$os.Version; BuildNumber=$os.BuildNumber; \
               OSArchitecture=$os.OSArchitecture; InstallDate=$os.InstallDate; \
               LastBootUpTime=$os.LastBootUpTime; TotalVisibleMemorySize=$os.TotalVisibleMemorySize; \
               FreePhysicalMemory=$os.FreePhysicalMemory; AvailableBytes=$perf.AvailableBytes }"
        ),
        "cpu": section(
            "Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors,MaxClockSpeed,CurrentClockSpeed,L2CacheSize,L3CacheSize,LoadPercentage,VirtualizationFirmwareEnabled"
        ),
        "gpu": section(
            "Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion,DriverDate,AdapterRAM,VideoModeDescription,CurrentRefreshRate,Status"
        ),
        "ram_modules": section(
            "Get-CimInstance Win32_PhysicalMemory | Select-Object Manufacturer,PartNumber,Capacity,Speed,ConfiguredClockSpeed,DeviceLocator,SMBIOSMemoryType"
        ),
        "board": section(
            "Get-CimInstance Win32_BaseBoard | Select-Object Manufacturer,Product,Version,SerialNumber"
        ),
        "bios": section(
            "Get-CimInstance Win32_BIOS | Select-Object Manufacturer,SMBIOSBIOSVersion,ReleaseDate"
        ),
        "disks": section(
            "Get-PhysicalDisk | Select-Object FriendlyName,MediaType,BusType,Size,HealthStatus,OperationalStatus,SpindleSpeed,FirmwareVersion"
        ),
        "volumes": section(
            "Get-Volume | Where-Object DriveLetter | Select-Object DriveLetter,FileSystemLabel,FileSystem,Size,SizeRemaining,HealthStatus"
        ),
        "smart": section(
            "Get-PhysicalDisk | Get-StorageReliabilityCounter | Select-Object DeviceId,Temperature,Wear,ReadErrorsTotal,WriteErrorsTotal,PowerOnHours"
        ),
        "battery": section(
            "Get-CimInstance Win32_Battery | Select-Object Name,EstimatedChargeRemaining,BatteryStatus,DesignVoltage"
        ),
        "network_adapters": section(
            "Get-NetAdapter | Where-Object Status -eq 'Up' | Select-Object Name,InterfaceDescription,LinkSpeed,MacAddress,DriverVersion"
        ),
        "startup_items": section(
            "Get-CimInstance Win32_StartupCommand | Select-Object Name,Command,Location,User"
        ),
        "services_running": section(
            "Get-Service | Where-Object Status -eq 'Running' | Select-Object Name,DisplayName,StartType | Sort-Object Name"
        ),
        "scheduled_tasks_count": section(
            "@{ total = (Get-ScheduledTask | Measure-Object).Count; ready = (Get-ScheduledTask | Where-Object State -eq 'Ready' | Measure-Object).Count }"
        ),
        "drivers_problem": section(
            "Get-CimInstance Win32_PnPEntity | Where-Object { $_.ConfigManagerErrorCode -ne 0 } | Select-Object Name,DeviceID,ConfigManagerErrorCode"
        ),
        "hotfixes_recent": section(
            "Get-HotFix | Sort-Object InstalledOn -Descending -ErrorAction SilentlyContinue | Select-Object -First 10 HotFixID,Description,InstalledOn"
        ),
        "power_plan": section(
            "powercfg /getactivescheme | Out-String"
        ),
        "vbs": section(
            "Get-CimInstance -Namespace root\\Microsoft\\Windows\\DeviceGuard Win32_DeviceGuard -ErrorAction Stop | Select-Object SecurityServicesRunning,VirtualizationBasedSecurityStatus"
        ),
        "thermal": section(
            "Get-CimInstance -Namespace root/wmi MSAcpi_ThermalZoneTemperature -ErrorAction Stop | Select-Object InstanceName,CurrentTemperature"
        )
    })
}

/// Boot time analysis via Event Log (event 100 of Diagnostics-Performance).
pub fn boot_analysis() -> Value {
    section(
        // By name: Data[0] is BootTsVersion (always 2), the duration is BootTime.
        "Get-WinEvent -FilterHashtable @{LogName='Microsoft-Windows-Diagnostics-Performance/Operational'; Id=100} -MaxEvents 10 -ErrorAction Stop | ForEach-Object { $x=[xml]$_.ToXml(); $b=($x.Event.EventData.Data | Where-Object { $_.Name -eq 'BootTime' } | Select-Object -First 1).'#text'; @{ time=$_.TimeCreated.ToString('s'); bootMs=[int64]$b } }"
    )
}

/// Recent critical/error events for correlation + BSOD (41/1001).
pub fn event_log_summary() -> Value {
    json!({
        "critical_system": section(
            "Get-WinEvent -FilterHashtable @{LogName='System'; Level=1,2; StartTime=(Get-Date).AddDays(-7)} -MaxEvents 50 -ErrorAction Stop | Select-Object TimeCreated,Id,ProviderName,LevelDisplayName,Message"
        ),
        "bsod": section(
            "Get-WinEvent -FilterHashtable @{LogName='System'; Id=41,1001} -MaxEvents 10 -ErrorAction Stop | Select-Object TimeCreated,Id,ProviderName"
        ),
        "minidumps": section(
            "Get-ChildItem (Join-Path $env:SystemRoot 'Minidump') -ErrorAction Stop | Select-Object Name,Length,LastWriteTime"
        )
    })
}

/// DISM/SFC component health (read-only checks; repairs are user-triggered).
pub fn component_health() -> Value {
    json!({
        "dism_check": match ps::exec_long("DISM.exe", &["/Online", "/Cleanup-Image", "/CheckHealth"]) {
            Ok(s) => json!(s.trim()),
            Err(e) => json!({ "error": e.trim() }),
        },
        "note": "Run 'sfc /scannow' and 'DISM /Online /Cleanup-Image /RestoreHealth' from the Repair page (admin required)."
    })
}

/// DNS benchmark: time lookups against popular resolvers.
pub fn dns_benchmark() -> Value {
    // Every server, the current one included, is queried directly: without
    // -Server, Resolve-DnsName answers from the local cache and "Current"
    // looked unrealistically fast. A failed query is reported as an error,
    // not as its (short) time; the time is the median of three queries.
    section(
        "$cur = (Get-DnsClientServerAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | \
           Where-Object { $_.ServerAddresses } | Select-Object -First 1).ServerAddresses | Select-Object -First 1; \
         $servers = [ordered]@{}; if ($cur) { $servers[\"Current ($cur)\"] = $cur }; \
         $servers['Cloudflare 1.1.1.1']='1.1.1.1'; $servers['Google 8.8.8.8']='8.8.8.8'; $servers['Quad9 9.9.9.9']='9.9.9.9'; \
         $servers.GetEnumerator() | ForEach-Object { \
           $name = $_.Key; $srv = $_.Value; $times = @(); $err = $null; \
           foreach ($i in 1..3) { \
             try { $t = Measure-Command { Resolve-DnsName example.com -Server $srv -DnsOnly -NoHostsFile -ErrorAction Stop | Out-Null }; $times += $t.TotalMilliseconds } \
             catch { $err = $_.Exception.Message } \
           }; \
           if ($times.Count -eq 0) { @{ server=$name; ms=$null; error=$err } } \
           else { $s = @($times | Sort-Object); @{ server=$name; ms=[math]::Round($s[[int][math]::Floor(($s.Count - 1) / 2)],1) } } }"
    )
}

/// Network latency / packet loss to a well-known host.
pub fn network_diag() -> Value {
    section(
        "$r = Test-Connection 1.1.1.1 -Count 8 -ErrorAction SilentlyContinue; \
         if ($r) { $lat = $r | Measure-Object -Property Latency -Average -Maximum -Minimum -ErrorAction SilentlyContinue; \
           if (-not $lat.Average) { $lat = $r | Measure-Object -Property ResponseTime -Average -Maximum -Minimum } \
           @{ sent=8; received=($r|Measure-Object).Count; lossPct=[math]::Round((8-($r|Measure-Object).Count)/8*100,1); avgMs=[math]::Round($lat.Average,1); maxMs=$lat.Maximum; minMs=$lat.Minimum } } \
         else { @{ error='no replies (offline or ICMP blocked)' } }"
    )
}
