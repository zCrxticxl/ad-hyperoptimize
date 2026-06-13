//! Startup manager. Enumerates Run keys (HKCU/HKLM/Wow6432Node) and Startup
//! folders, and toggles entries exactly like Task Manager does — via the
//! StartupApproved registry values (REG_BINARY: first byte 0x02 = enabled,
//! 0x03 = disabled). Nothing is ever deleted; toggles are fully reversible.

use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
#[cfg(windows)]
use winreg::{RegKey, RegValue};

#[cfg(windows)]
const APPROVED_RUN: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";
#[cfg(windows)]
const APPROVED_RUN32: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run32";
#[cfg(windows)]
const APPROVED_FOLDER: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\StartupFolder";

#[cfg(windows)]
fn hive(root: &str) -> RegKey {
    RegKey::predef(if root == "HKLM" { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER })
}

/// StartupApproved state: missing value or first byte 0x02/0x06 → enabled.
#[cfg(windows)]
fn approved_state(root: &str, approved_path: &str, name: &str) -> bool {
    let Ok(key) = hive(root).open_subkey_with_flags(approved_path, KEY_READ) else {
        return true;
    };
    match key.get_raw_value(name) {
        Ok(v) => v.bytes.first().map(|b| b & 0x01 == 0).unwrap_or(true),
        Err(_) => true,
    }
}

#[cfg(windows)]
fn run_key_entries(root: &str, run_path: &str, approved_path: &str, scope: &str) -> Vec<Value> {
    let Ok(key) = hive(root).open_subkey_with_flags(run_path, KEY_READ) else {
        return vec![];
    };
    key.enum_values()
        .filter_map(|r| r.ok())
        .map(|(name, _)| {
            let cmd: String = key.get_value(&name).unwrap_or_default();
            json!({
                "name": name,
                "command": cmd,
                "scope": scope,
                "enabled": approved_state(root, approved_path, &name),
                "kind": "registry",
            })
        })
        .collect()
}

fn startup_folders() -> Vec<(PathBuf, &'static str)> {
    let mut v = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        v.push((
            PathBuf::from(appdata).join("Microsoft\\Windows\\Start Menu\\Programs\\Startup"),
            "folder_user",
        ));
    }
    if let Ok(pd) = std::env::var("ProgramData") {
        v.push((
            PathBuf::from(pd).join("Microsoft\\Windows\\Start Menu\\Programs\\Startup"),
            "folder_common",
        ));
    }
    v
}

pub fn list() -> Value {
    #[cfg(windows)]
    {
        let mut items = Vec::new();
        items.extend(run_key_entries(
            "HKCU",
            "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            APPROVED_RUN,
            "hkcu_run",
        ));
        items.extend(run_key_entries(
            "HKLM",
            "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            APPROVED_RUN,
            "hklm_run",
        ));
        items.extend(run_key_entries(
            "HKLM",
            "Software\\Wow6432Node\\Microsoft\\Windows\\CurrentVersion\\Run",
            APPROVED_RUN32,
            "hklm_run32",
        ));
        for (dir, scope) in startup_folders() {
            let root = if scope == "folder_common" { "HKLM" } else { "HKCU" };
            if let Ok(rd) = fs::read_dir(&dir) {
                for e in rd.flatten() {
                    let fname = e.file_name().to_string_lossy().into_owned();
                    if fname.to_lowercase() == "desktop.ini" {
                        continue;
                    }
                    items.push(json!({
                        "name": fname,
                        "command": e.path().to_string_lossy(),
                        "scope": scope,
                        "enabled": approved_state(root, APPROVED_FOLDER, &fname),
                        "kind": "folder",
                    }));
                }
            }
        }
        json!({ "items": items })
    }
    #[cfg(not(windows))]
    json!({ "items": [] })
}

/// Toggle exactly like Task Manager: 12-byte REG_BINARY in StartupApproved.
pub fn toggle(scope: String, name: String, enable: bool) -> Result<Value, String> {
    #[cfg(windows)]
    {
        let (root, approved) = match scope.as_str() {
            "hkcu_run" => ("HKCU", APPROVED_RUN),
            "hklm_run" => ("HKLM", APPROVED_RUN),
            "hklm_run32" => ("HKLM", APPROVED_RUN32),
            "folder_user" => ("HKCU", APPROVED_FOLDER),
            "folder_common" => ("HKLM", APPROVED_FOLDER),
            _ => return Err(format!("unknown scope '{scope}'")),
        };
        if root == "HKLM" && !crate::ps::is_admin() {
            return Err("Systemweite Autostart-Einträge brauchen Adminrechte.".into());
        }
        let (key, _) = hive(root)
            .create_subkey(approved)
            .map_err(|e| format!("open StartupApproved: {e}"))?;
        let mut bytes = vec![0u8; 12];
        bytes[0] = if enable { 0x02 } else { 0x03 };
        if !enable {
            // FILETIME of "now" in the trailing 8 bytes, like Task Manager.
            let ft = (chrono::Utc::now().timestamp() as u64 + 11_644_473_600) * 10_000_000;
            bytes[4..12].copy_from_slice(&ft.to_le_bytes());
        }
        key.set_raw_value(
            &name,
            &RegValue { bytes, vtype: winreg::enums::RegType::REG_BINARY },
        )
        .map_err(|e| format!("set state: {e}"))?;
        Ok(json!({ "name": name, "enabled": enable }))
    }
    #[cfg(not(windows))]
    {
        let _ = (scope, name, enable);
        Err("Windows only".into())
    }
}
