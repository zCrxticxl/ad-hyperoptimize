//! Security & anomaly inspection. Mostly read-only (Defender/firewall/
//! autoruns/hosts surfacing), but unsigned drivers can be disabled or
//! removed directly via pnputil.exe (see disable/enable/remove_unsigned_driver).

use crate::ps;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

fn sec(script: &str) -> Value {
    ps::run_json(script).unwrap_or_else(|e| json!({ "error": e.trim() }))
}

// ── hosts file backup ─────────────────────────────────────────────────────────

/// The hosts file is modified in place. Immediately before EVERY write the
/// current file is copied into the app data dir, so a failed or corrupted
/// write is restored to the state right before that write. (A one-time
/// snapshot went stale: restoring it silently discarded every hosts change
/// made since, by this app or anything else.)
fn hosts_backup() -> Result<PathBuf, String> {
    let backup = crate::safety::state_dir()?.join("hosts").join("hosts.bak");
    if let Some(parent) = backup.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::copy(hosts_path(), &backup).map_err(|e| format!("hosts backup failed: {e}"))?;
    Ok(backup)
}

fn hosts_restore(backup: &Path) -> Result<(), String> {
    if backup.exists() {
        fs::copy(backup, hosts_path()).map_err(|e| e.to_string())?;
        Ok(())
    } else {
        // Honest failure: the caller must not report "restored from backup"
        // when there was nothing to restore from.
        Err(format!("no hosts backup found at {}", backup.display()))
    }
}

pub fn scan() -> Value {
    json!({
        "defender": sec(
            // MAPSReporting (cloud-delivered protection) is a preference, not a
            // status field: it only exists on Get-MpPreference.
            "$s = Get-MpComputerStatus -ErrorAction Stop | Select-Object AMServiceEnabled,RealTimeProtectionEnabled,AntivirusEnabled,AntivirusSignatureLastUpdated,QuickScanEndTime,IsTamperProtected; $s | Add-Member -NotePropertyName MAPSReporting -NotePropertyValue ((Get-MpPreference -ErrorAction SilentlyContinue).MAPSReporting) -Force; $s"
        ),
        "firewall": sec(
            "Get-NetFirewallProfile | Select-Object Name,Enabled,DefaultInboundAction,DefaultOutboundAction"
        ),
        "unsigned_drivers": unsigned_drivers(),
        "autoruns": autoruns(),
        "hosts": hosts_entries(),
        "suspicious_processes": sec(
            // Userland processes running from Temp/AppData\\Local\\Temp, classic persistence red flag.
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
    // "PCI\VEN_10DE&DEV_...\4&1a2b3c4d&0&0008"), unlike driverquery's
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

/// Run pnputil.exe directly (no shell/string interpolation, args go straight
/// to argv, so a device ID containing `&`/`\`/spaces can't break out or
/// inject anything). Treats reboot-pending exit codes as success.
fn pnputil(args: &[&str]) -> Result<String, String> {
    // exec_capture: long timeout + whole-tree kill on timeout; pnputil driver
    // ops (scan/delete) legitimately take minutes.
    let (status, stdout, stderr) = crate::ps::exec_capture("pnputil.exe", args)?;
    let code = status.code().unwrap_or(-1);
    let stdout = stdout.trim().to_string();
    match code {
        0 => Ok(if stdout.is_empty() {
            "Done.".into()
        } else {
            stdout
        }),
        3010 | 1641 => Ok(format!(
            "{} (restart required to finish)",
            if stdout.is_empty() { "Done." } else { &stdout }
        )),
        _ => {
            let stderr = stderr.trim().to_string();
            Err(if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("pnputil exited with code {code}")
            })
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
/// step, can be undone with enable_unsigned_driver.
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
/// reinstall a driver for it (on its own or after a rescan/reboot), this is
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
    .map(|_| "Defender Quick Scan started, result will appear in Windows Security Center.".into())
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
                    ho_disabled.push(t.strip_prefix(HO_PREFIX).unwrap().to_string());
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

/// Serializes hosts-file mutations: two concurrent disable/enable calls would
/// otherwise read-modify-write the same file and lose entries.
static HOSTS_LOCK: Mutex<()> = Mutex::new(());

pub fn hosts_disable_entries(entries: Vec<String>) -> Result<String, String> {
    if !crate::ps::is_admin() {
        return Err("Administrator rights required to edit the hosts file.".into());
    }
    let _g = HOSTS_LOCK.lock().map_err(|e| format!("hosts lock: {e}"))?;
    let path = hosts_path();
    let backup = hosts_backup()?;
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
    fs::write(&path, new_content).map_err(|e| {
        let restore = hosts_restore(&backup)
            .map(|_| "hosts restored from backup".to_string())
            .unwrap_or_else(|re| format!("restore also failed: {re}"));
        format!("write failed ({e}); {restore}")
    })?;
    Ok(format!("Disabled {count} host entries"))
}

pub fn hosts_enable_entries(entries: Vec<String>) -> Result<String, String> {
    if !crate::ps::is_admin() {
        return Err("Administrator rights required to edit the hosts file.".into());
    }
    let _g = HOSTS_LOCK.lock().map_err(|e| format!("hosts lock: {e}"))?;
    let path = hosts_path();
    let backup = hosts_backup()?;
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
    fs::write(&path, new_content).map_err(|e| {
        let restore = hosts_restore(&backup)
            .map(|_| "hosts restored from backup".to_string())
            .unwrap_or_else(|re| format!("restore also failed: {re}"));
        format!("write failed ({e}); {restore}")
    })?;
    Ok(format!("Enabled {count} host entries"))
}

// ── Scheduled task disable/enable ─────────────────────────────────────────────
// Delegates to schedtasks::toggle so this entry point gets the same admin
// check, validation (quotes, wildcards, typographic quotes) and journal-
// backed undo instead of a second, weaker implementation.

pub fn disable_scheduled_task(task_path: String, task_name: String) -> Result<String, String> {
    crate::schedtasks::toggle(task_path, task_name, false).map(|_| "OK".to_string())
}

pub fn enable_scheduled_task(task_path: String, task_name: String) -> Result<String, String> {
    crate::schedtasks::toggle(task_path, task_name, true).map(|_| "OK".to_string())
}

// ── Defender toggles ──────────────────────────────────────────────────────────

/// Tamper Protection locks Defender settings: Set-MpPreference then fails
/// with an obscure Win32 error. Detect it up front so the user gets an
/// actionable message instead. A failed probe is treated as "not locked" -
/// the subsequent Set-MpPreference error still surfaces honestly.
fn tamper_protection_active() -> bool {
    ps::run("(Get-MpComputerStatus -ErrorAction Stop).IsTamperProtected")
        .map(|s| s.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn ensure_defender_unlocked() -> Result<(), String> {
    if tamper_protection_active() {
        return Err(
            "Tamper Protection is enabled, Defender settings are locked. \
                    Disable \"Tamper Protection\" in Windows Security → Virus & threat \
                    protection → Manage settings first."
                .into(),
        );
    }
    Ok(())
}

/// Apply a Defender preference change with a write-ahead journal entry whose
/// revert restores the value captured before the change.
fn journaled_defender_change(
    tweak_id: &str,
    tweak_name: &str,
    apply: &str,
    revert: &str,
    done_msg: &str,
) -> Result<String, String> {
    use crate::safety::{self, ChangeItem, JournalEntry};
    let entry_id = safety::append_entry(JournalEntry {
        id: format!(
            "{tweak_id}-{}",
            chrono::Local::now().format("%Y%m%d%H%M%S%6f")
        ),
        tweak_id: tweak_id.into(),
        tweak_name: tweak_name.into(),
        origin: None,
        time: chrono::Local::now().to_rfc3339(),
        items: vec![ChangeItem::Command {
            applied: format!("powershell -Command {apply}"),
            revert: format!("powershell -Command {revert}"),
        }],
        reverted: false,
        backup_files: vec![],
        completed: false,
        attempted: 1,
        reverted_items: 0,
        untrusted: false,
    })?;
    let result = ps::run(apply);
    safety::with_journal(|entries| {
        let entry = entries
            .iter_mut()
            .find(|e| e.id == entry_id)
            .ok_or("Defender journal entry missing")?;
        entry.completed = result.is_ok();
        // Set-MpPreference -ErrorAction Stop is all-or-nothing: on failure
        // nothing changed, so the entry must not offer an undo.
        entry.reverted = result.is_err();
        Ok(())
    })?;
    result.map(|_| done_msg.to_string())
}

/// Parse the captured DisableRealtimeMonitoring preference.
fn parse_ps_bool(s: &str) -> Option<bool> {
    match s.trim() {
        t if t.eq_ignore_ascii_case("true") => Some(true),
        t if t.eq_ignore_ascii_case("false") => Some(false),
        _ => None,
    }
}

/// Parse the captured MAPSReporting preference (0 = off, 1 = basic,
/// 2 = advanced). Only these values are ever written back.
fn parse_maps_reporting(s: &str) -> Option<u8> {
    match s.trim() {
        "0" => Some(0),
        "1" => Some(1),
        "2" => Some(2),
        _ => None,
    }
}

pub fn defender_set_realtime(enabled: bool) -> Result<String, String> {
    if !enabled {
        ensure_defender_unlocked()?;
    }
    let prev_disabled = ps::run("(Get-MpPreference -ErrorAction Stop).DisableRealtimeMonitoring")
        .ok()
        .and_then(|s| parse_ps_bool(&s))
        .ok_or("could not read the current Real-Time Protection setting; nothing changed")?;
    let msg = if enabled {
        "Real-Time Protection enabled"
    } else {
        "Real-Time Protection disabled"
    };
    if prev_disabled != enabled {
        // Already in the requested state; nothing to change or journal.
        return Ok(msg.into());
    }
    let ps_bool = |b: bool| if b { "$true" } else { "$false" };
    journaled_defender_change(
        "defenderRealtime",
        if enabled {
            "Defender: Real-Time Protection on"
        } else {
            "Defender: Real-Time Protection off"
        },
        &format!(
            "Set-MpPreference -DisableRealtimeMonitoring {} -ErrorAction Stop",
            ps_bool(!enabled)
        ),
        &format!(
            "Set-MpPreference -DisableRealtimeMonitoring {} -ErrorAction Stop",
            ps_bool(prev_disabled)
        ),
        msg,
    )
}

pub fn defender_set_cloud(enabled: bool) -> Result<String, String> {
    if !enabled {
        ensure_defender_unlocked()?;
    }
    let prev = ps::run("(Get-MpPreference -ErrorAction Stop).MAPSReporting")
        .ok()
        .and_then(|s| parse_maps_reporting(&s))
        .ok_or("could not read the current Cloud Protection setting; nothing changed")?;
    let msg = if enabled {
        "Cloud Protection enabled"
    } else {
        "Cloud Protection disabled"
    };
    // Already in the requested state. Enabling keeps a user's existing
    // non-zero level (basic stays basic).
    if (prev != 0) == enabled {
        return Ok(msg.into());
    }
    let val = if enabled { 2 } else { 0 };
    journaled_defender_change(
        "defenderCloud",
        if enabled {
            "Defender: Cloud Protection on"
        } else {
            "Defender: Cloud Protection off"
        },
        &format!("Set-MpPreference -MAPSReporting {val} -ErrorAction Stop"),
        // Restores the captured level instead of a hardcoded 2.
        &format!("Set-MpPreference -MAPSReporting {prev} -ErrorAction Stop"),
        msg,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_captured_defender_values() {
        assert_eq!(parse_ps_bool("True\r\n"), Some(true));
        assert_eq!(parse_ps_bool("false"), Some(false));
        assert_eq!(parse_ps_bool(""), None);
        assert_eq!(parse_maps_reporting("1\n"), Some(1));
        assert_eq!(parse_maps_reporting("2"), Some(2));
        assert_eq!(parse_maps_reporting("3"), None);
        assert_eq!(parse_maps_reporting("0; calc"), None);
    }
}
