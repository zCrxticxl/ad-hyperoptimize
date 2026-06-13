//! Hardware monitoring: CPU/GPU temperatures, SSD S.M.A.R.T., fan speeds.

use crate::ps;
use serde_json::{json, Value};

pub fn temps() -> Value {
    let script = r#"
$out = @{}

# --- CPU thermal zones (ACPI WMI) ---
try {
    $zones = Get-WmiObject MSAcpi_ThermalZoneTemperature -Namespace "root/wmi" -ErrorAction Stop
    $out['cpuZones'] = @($zones | ForEach-Object {
        [PSCustomObject]@{
            name  = $_.InstanceName -replace 'ACPI\\ThermalZone\\','' -replace '_\d+$',''
            tempC = [math]::Round($_.CurrentTemperature / 10.0 - 273.15, 1)
        }
    })
} catch { $out['cpuZones'] = @(); $out['cpuError'] = $_.Exception.Message }

# --- GPU: NVIDIA via nvidia-smi ---
try {
    $nv = & nvidia-smi --query-gpu=name,temperature.gpu,utilization.gpu,memory.used,memory.total,power.draw,clocks.current.graphics `
        --format=csv,noheader,nounits 2>$null
    if ($nv) {
        $out['gpus'] = @($nv -split "`n" | Where-Object { $_.Trim() } | ForEach-Object {
            $p = $_ -split ',\s*'
            [PSCustomObject]@{
                vendor      = 'NVIDIA'
                name        = $p[0].Trim()
                tempC       = if ($p[1] -match '^\d') { [int]$p[1] } else { $null }
                utilPct     = if ($p[2] -match '^\d') { [int]$p[2] } else { $null }
                memUsedMb   = if ($p[3] -match '^\d') { [int]$p[3] } else { $null }
                memTotalMb  = if ($p[4] -match '^\d') { [int]$p[4] } else { $null }
                powerW      = if ($p[5] -match '[\d.]+') { [math]::Round([double]($p[5] -replace '[^\d.]',''), 1) } else { $null }
                clockMhz    = if ($p[6] -match '^\d') { [int]$p[6] } else { $null }
            }
        })
    }
} catch {}

# --- GPU: AMD via WMI (fallback) ---
if (-not $out['gpus']) {
    try {
        $amdT = Get-WmiObject -Namespace "root\wmi" -Class "AMD_SMBios_Data" -ErrorAction Stop
        if ($amdT) { $out['gpus'] = @([PSCustomObject]@{ vendor='AMD'; name='AMD GPU'; tempC=$null }) }
    } catch {}
}

# --- CPU package temp via CIM (works on some systems) ---
try {
    $cimT = Get-CimInstance -Namespace "root/wmi" -ClassName "MSAcpi_ThermalZoneTemperature" -ErrorAction Stop
    if ($cimT -and $out['cpuZones'].Count -eq 0) {
        $out['cpuZones'] = @($cimT | ForEach-Object {
            [PSCustomObject]@{ name='CPU'; tempC=[math]::Round($_.CurrentTemperature/10.0-273.15,1) }
        })
    }
} catch {}

$out | ConvertTo-Json -Depth 4 -Compress
"#;
    ps::run_json(script).unwrap_or_else(|e| json!({ "error": e }))
}

pub fn smart() -> Value {
    let script = r#"
$disks = @()
try {
    $physical = Get-PhysicalDisk -ErrorAction Stop
    $reliability = $physical | Get-StorageReliabilityCounter -ErrorAction SilentlyContinue
    foreach ($d in $physical) {
        $rc = $reliability | Where-Object { $_.PSComputerName -eq $d.PSComputerName -or $true } |
            Select-Object -First 1  # best effort match
        $rc2 = ($physical | Where-Object { $_.UniqueId -eq $d.UniqueId } |
            Get-StorageReliabilityCounter -ErrorAction SilentlyContinue)
        if ($rc2) { $rc = $rc2 }
        $health = switch ($d.HealthStatus) {
            'Healthy'  { 'good' }
            'Warning'  { 'warning' }
            'Unhealthy'{ 'bad' }
            default    { $d.HealthStatus }
        }
        $disks += [PSCustomObject]@{
            name         = $d.FriendlyName
            type         = $d.MediaType     # SSD / HDD / SCM / Unspecified
            bus          = $d.BusType
            health       = $health
            sizeGb       = [math]::Round($d.Size / 1GB)
            tempC        = if ($rc -and $rc.Temperature -gt 0) { $rc.Temperature } else { $null }
            wearPct      = if ($rc -and $rc.Wear -ne $null) { $rc.Wear } else { $null }
            powerOnHours = if ($rc) { $rc.PowerOnHours } else { $null }
            readErrors   = if ($rc) { $rc.ReadErrorsTotal } else { $null }
            writeErrors  = if ($rc) { $rc.WriteErrorsTotal } else { $null }
            uncorrected  = if ($rc) { $rc.ReadErrorsUncorrected } else { $null }
        }
    }
} catch { $disks += [PSCustomObject]@{ error = $_.Exception.Message } }
if ($disks.Count -eq 0) { '[]' } else { @($disks) | ConvertTo-Json -Depth 2 -Compress }
"#;
    match ps::run_json(script) {
        Ok(v @ Value::Array(_)) => json!({ "disks": v }),
        Ok(v @ Value::Object(_)) => json!({ "disks": [v] }),
        _ => json!({ "disks": [] }),
    }
}

pub fn fans() -> Value {
    let script = r#"
$fans = @()
# WMI fan speed (rarely available, but try)
try {
    $wmi = Get-WmiObject Win32_Fan -ErrorAction Stop
    $fans = @($wmi | ForEach-Object {
        [PSCustomObject]@{ name=$_.Name; rpm=[int]$_.DesiredSpeed }
    })
} catch {}
# CIM fallback
if ($fans.Count -eq 0) {
    try {
        $cim = Get-CimInstance -ClassName CIM_Fan -ErrorAction Stop
        $fans = @($cim | ForEach-Object { [PSCustomObject]@{ name=$_.Name; rpm=[int]$_.DesiredSpeed } })
    } catch {}
}
if ($fans.Count -eq 0) { '[]' } else { @($fans) | ConvertTo-Json -Compress }
"#;
    match ps::run_json(script) {
        Ok(v @ Value::Array(_)) => json!({ "fans": v }),
        Ok(v @ Value::Object(_)) => json!({ "fans": [v] }),
        _ => json!({ "fans": [] }),
    }
}

pub fn full() -> Value {
    json!({
        "temps": temps(),
        "smart": smart(),
        "fans":  fans(),
    })
}
