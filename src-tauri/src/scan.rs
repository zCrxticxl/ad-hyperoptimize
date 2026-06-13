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
            "Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,BuildNumber,OSArchitecture,InstallDate,LastBootUpTime,TotalVisibleMemorySize,FreePhysicalMemory"
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
        "Get-WinEvent -FilterHashtable @{LogName='Microsoft-Windows-Diagnostics-Performance/Operational'; Id=100} -MaxEvents 10 -ErrorAction Stop | ForEach-Object { $x=[xml]$_.ToXml(); @{ time=$_.TimeCreated.ToString('s'); bootMs=[int64]$x.Event.EventData.Data[0].'#text' } }"
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
            "Get-ChildItem 'C:\\Windows\\Minidump' -ErrorAction Stop | Select-Object Name,Length,LastWriteTime"
        )
    })
}

/// DISM/SFC component health (read-only checks; repairs are user-triggered).
pub fn component_health() -> Value {
    json!({
        "dism_check": match ps::exec("DISM.exe", &["/Online", "/Cleanup-Image", "/CheckHealth"]) {
            Ok(s) => json!(s.trim()),
            Err(e) => json!({ "error": e.trim() }),
        },
        "note": "Run 'sfc /scannow' and 'DISM /Online /Cleanup-Image /RestoreHealth' from the Repair page (admin required)."
    })
}

/// DNS benchmark: time lookups against popular resolvers.
pub fn dns_benchmark() -> Value {
    section(
        "$servers=@{'Current'=$null;'Cloudflare 1.1.1.1'='1.1.1.1';'Google 8.8.8.8'='8.8.8.8';'Quad9 9.9.9.9'='9.9.9.9'}; \
         $servers.GetEnumerator() | ForEach-Object { \
           $t=Measure-Command { try { if ($_.Value) { Resolve-DnsName example.com -Server $_.Value -ErrorAction Stop | Out-Null } else { Resolve-DnsName example.com -ErrorAction Stop | Out-Null } } catch {} }; \
           @{ server=$_.Key; ms=[math]::Round($t.TotalMilliseconds,1) } }"
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
