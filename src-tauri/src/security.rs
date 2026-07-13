//! Security & anomaly inspection. Mostly read-only (Defender/firewall/
//! autoruns/hosts surfacing), but unsigned drivers can be disabled or
//! removed directly via pnputil.exe (see disable/enable/remove_unsigned_driver).

use crate::ps;
use serde_json::{json, Value};
use std::fs;

fn sec(script: &str) -> Value {
    ps::run_json(script).unwrap_or_else(|e| json!({ "error": e.trim() }))
}

pub fn scan() -> Value {
    json!({
        "defender": sec(
            "Get-MpComputerStatus -ErrorAction Stop | Select-Object AMServiceEnabled,RealTimeProtectionEnabled,AntivirusEnabled,AntivirusSignatureLastUpdated,QuickScanEndTime,IsTamperProtected"
        ),
        "firewall": sec(
            "Get-NetFirewallProfile | Select-Object Name,Enabled,DefaultInboundAction,DefaultOutboundAction"
        ),
        "unsigned_drivers": unsigned_drivers(),
        "autoruns": autoruns(),
        "hosts": hosts_entries(),
        "suspicious_processes": sec(
            // Userland processes running from Temp/AppData\\Local\\Temp — classic persistence red flag.
            "Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -match '\\\\Temp\\\\|\\\\AppData\\\\Local\\\\Temp' } | Select-Object Name,ProcessId,ExecutablePath"
        ),
        "uac": sec(
            "Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies\\System' | Select-Object EnableLUA,ConsentPromptBehaviorAdmin"
        ),
        "secure_boot": match ps::run("Confirm-SecureBootUEFI -ErrorAction Stop") {
            Ok(s) => json!(s.trim().eq_ignore_ascii_case("true")),
            Err(_) => json!("unavailable (legacy BIOS or non-admin)"),
        },
    })
}

fn unsigned_drivers() -> Value {
    // Win32_PnPSignedDriver.DeviceID is the PnP *device instance ID* (e.g.
    // "PCI\VEN_10DE&DEV_...\4&1a2b3c4d&0&0008") — unlike driverquery's
    // DeviceName, this uniquely identifies one physical/virtual device, which
    // is required to safely target disable/remove actions at exactly the
    // flagged device and nothing else.
    let script = r#"
Get-CimInstance Win32_PnPSignedDriver -ErrorAction SilentlyContinue |
    Where-Object { $_.IsSigned -eq $false -and $_.DeviceID } |
    Select-Object DeviceName, DeviceID, Manufacturer, DeviceClass |
    Sort-Object DeviceName
"#;
    let to_item = |d: &Value| {
        json!({
            "device": d["DeviceName"].as_str().unwrap_or("Unknown driver"),
            "manufacturer": d["Manufacturer"].as_str().unwrap_or(""),
            "deviceClass": d["DeviceClass"].as_str().unwrap_or(""),
            "deviceId": d["DeviceID"].as_str().unwrap_or(""),
        })
    };
    match ps::run_json(script) {
        Ok(Value::Array(arr)) => {
            let items: Vec<Value> = arr.iter().map(to_item).collect();
            json!({ "count": items.len(), "items": items })
        }
        Ok(v @ Value::Object(_)) => json!({ "count": 1, "items": [to_item(&v)] }),
        Ok(_) => json!({ "count": 0, "items": [] }),
        Err(e) => json!({ "error": e.trim() }),
    }
}

/// Run pnputil.exe directly (no shell/string interpolation — args go straight
/// to argv, so a device ID containing `&`/`\`/spaces can't break out or
/// inject anything). Treats reboot-pending exit codes as success.
fn pnputil(args: &[&str]) -> Result<String, String> {
    use std::process::Command;
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    let mut cmd = Command::new("pnputil.exe");
    cmd.args(args);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let out = cmd.output().map_err(|e| format!("pnputil spawn: {e}"))?;
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    match code {
        0 => Ok(if stdout.is_empty() { "Done.".into() } else { stdout }),
        3010 | 1641 => Ok(format!("{} (restart required to finish)", if stdout.is_empty() { "Done." } else { &stdout })),
        _ => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(if !stderr.is_empty() { stderr } else if !stdout.is_empty() { stdout } else { format!("pnputil exited with code {code}") })
        }
    }
}

fn require_device_id(device_id: &str) -> Result<(), String> {
    if device_id.trim().is_empty() {
        Err("Missing device instance ID".into())
    } else {
        Ok(())
    }
}

/// Reversible: device stays installed but stops loading/binding. Safe first
/// step — can be undone with enable_unsigned_driver.
pub fn disable_unsigned_driver(device_id: String) -> Result<String, String> {
    require_device_id(&device_id)?;
    pnputil(&["/disable-device", &device_id])
}

/// Undo for disable_unsigned_driver.
pub fn enable_unsigned_driver(device_id: String) -> Result<String, String> {
    require_device_id(&device_id)?;
    pnputil(&["/enable-device", &device_id])
}

/// Uninstalls the device + driver package. If the hardware is still
/// physically/logically present, Windows PnP will typically re-enumerate and
/// reinstall a driver for it (on its own or after a rescan/reboot) — this is
/// the same "repair by reinstall" path Device Manager's own Uninstall-device
/// button uses, not a guaranteed permanent removal.
pub fn remove_unsigned_driver(device_id: String) -> Result<String, String> {
    require_device_id(&device_id)?;
    pnputil(&["/remove-device", &device_id])
}

/// Launch a Defender quick scan detached (survives this process).
pub fn defender_quick_scan() -> Result<String, String> {
    ps::run(
        "Start-Process powershell -WindowStyle Hidden -ArgumentList '-NoProfile','-Command','Start-MpScan -ScanType QuickScan'; 'OK'",
    )
    .map(|_| "Defender Quick Scan started — result will appear in Windows Security Center.".into())
}

fn autoruns() -> Value {
    json!({
        "hklm_run": sec("Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run' -ErrorAction Stop | Select-Object * -ExcludeProperty PS*"),
        "hkcu_run": sec("Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run' -ErrorAction Stop | Select-Object * -ExcludeProperty PS*"),
        "startup_folder": sec("Get-ChildItem ([Environment]::GetFolderPath('Startup')) -ErrorAction Stop | Select-Object Name,FullName"),
        "tasks_nonms": sec("Get-ScheduledTask | Where-Object { $_.TaskPath -notlike '\\Microsoft\\*' } | Select-Object TaskName,TaskPath,State | Select-Object -First 60"),
        "winlogon_shell": sec("Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon' | Select-Object Shell,Userinit"),
    })
}

const HO_PREFIX: &str = "# [ADHYPER] ";

fn hosts_path() -> String {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    format!("{windir}\\System32\\drivers\\etc\\hosts")
}

fn hosts_entries() -> Value {
    match fs::read_to_string(hosts_path()) {
        Ok(s) => {
            let mut active: Vec<String> = Vec::new();
            let mut ho_disabled: Vec<String> = Vec::new();
            for line in s.lines() {
                let t = line.trim();
                if t.starts_with(HO_PREFIX) {
                             ho_disabled.push(t[HO_PREFIX.len()..].to_string());
                } else if !t.is_empty() && !t.starts_with('#') {
                    active.push(t.to_string());
                }
            }
            json!({ "active": active, "disabled": ho_disabled })
        }
        Err(e) => json!({ "error": e.to_string() }),
    }
}

pub fn hosts_list_all() -> Value {
    hosts_entries()
}

pub fn hosts_disable_entries(entries: Vec<String>) -> Result<String, String> {
    let path = hosts_path();
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut count = 0u32;
    for entry in &entries {
        for line in &mut lines {
            let t = line.trim();
            if t == entry.trim() && !t.starts_with('#') {
                *line = format!("{HO_PREFIX}{}", t);
                count += 1;
            }
        }
    }
    let new_content = lines.join("\r\n") + "\r\n";
    fs::write(&path, new_content).map_err(|e| e.to_string())?;
    Ok(format!("Disabled {count} host entries"))
}

pub fn hosts_enable_entries(entries: Vec<String>) -> Result<String, String> {
    let path = hosts_path();
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut count = 0u32;
    for entry in &entries {
        for line in &mut lines {
            let t = line.trim();
            let prefixed = format!("{HO_PREFIX}{}", entry.trim());
            if t == prefixed.trim() {
                *line = entry.trim().to_string();
                count += 1;
            }
        }
    }
    let new_content = lines.join("\r\n") + "\r\n";
    fs::write(&path, new_content).map_err(|e| e.to_string())?;
    Ok(format!("Enabled {count} host entries"))
}

// ── Scheduled task disable/enable ─────────────────────────────────────────────

pub fn disable_scheduled_task(task_path: String, task_name: String) -> Result<String, String> {
    ps::run(&format!(
        "Disable-ScheduledTask -TaskPath '{}' -TaskName '{}' -ErrorAction Stop | Out-Null; 'Disabled: {}{}'",
        task_path, task_name, task_path, task_name
    ))
    .map(|s| s.trim().to_string())
}

pub fn enable_scheduled_task(task_path: String, task_name: String) -> Result<String, String> {
    ps::run(&format!(
        "Enable-ScheduledTask -TaskPath '{}' -TaskName '{}' -ErrorAction Stop | Out-Null; 'Enabled: {}{}'",
        task_path, task_name, task_path, task_name
    ))
    .map(|s| s.trim().to_string())
}

// ── Defender toggles ──────────────────────────────────────────────────────────

pub fn defender_set_realtime(enabled: bool) -> Result<String, String> {
    let cmd = if enabled {
        "Set-MpPreference -DisableRealtimeMonitoring $false -ErrorAction Stop; 'Real-Time Protection enabled'"
    } else {
        "Set-MpPreference -DisableRealtimeMonitoring $true -ErrorAction Stop; 'Real-Time Protection disabled'"
    };
    ps::run(cmd).map(|s| s.trim().to_string())
}

pub fn defender_set_cloud(enabled: bool) -> Result<String, String> {
    let val = if enabled { "2" } else { "0" };
    ps::run(&format!(
        "Set-MpPreference -MAPSReporting {} -ErrorAction Stop; '{}'",
        val,
        if enabled { "Cloud Protection enabled" } else { "Cloud Protection disabled" }
    ))
    .map(|s| s.trim().to_string())
}
