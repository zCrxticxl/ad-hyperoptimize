//! Startup manager. Enumerates Run keys (HKCU/HKLM/Wow6432Node) and Startup
//! folders, and toggles entries exactly like Task Manager does, via the
//! StartupApproved registry values (REG_BINARY: first byte 0x02 = enabled,
//! 0x03 = disabled). Nothing is ever deleted; toggles are fully reversible.

use serde_json::{json, Value};
#[cfg(windows)]
use std::fs;
#[cfg(windows)]
use std::path::PathBuf;

#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
#[cfg(windows)]
use winreg::{RegKey, RegValue};

const APPROVED_RUN: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";
const APPROVED_RUN32: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run32";
const APPROVED_FOLDER: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\StartupFolder";

/// Hive and StartupApproved key of a startup scope.
pub(crate) fn approved_location(scope: &str) -> Option<(&'static str, &'static str)> {
    Some(match scope {
        "hkcu_run" => ("HKCU", APPROVED_RUN),
        "hklm_run" => ("HKLM", APPROVED_RUN),
        "hklm_run32" => ("HKLM", APPROVED_RUN32),
        "folder_user" => ("HKCU", APPROVED_FOLDER),
        "folder_common" => ("HKLM", APPROVED_FOLDER),
        _ => return None,
    })
}

#[cfg(windows)]
fn hive(root: &str) -> RegKey {
    RegKey::predef(if root == "HKLM" {
        HKEY_LOCAL_MACHINE
    } else {
        HKEY_CURRENT_USER
    })
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
fn run_key_entries(
    root: &str,
    run_path: &str,
    approved_path: &str,
    scope: &str,
) -> Result<Vec<Value>, String> {
    let key = match hive(root).open_subkey_with_flags(run_path, KEY_READ) {
        Ok(key) => key,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Cannot read {root}/{run_path}: {error}")),
    };
    let mut entries = Vec::new();
    for value in key.enum_values() {
        let (name, _) =
            value.map_err(|error| format!("Cannot enumerate {root}/{run_path}: {error}"))?;
        let command: String = key
            .get_value(&name)
            .map_err(|error| format!("Cannot read startup value {name}: {error}"))?;
        entries.push(json!({
            "name": name,
            "command": command,
            "scope": scope,
            "enabled": approved_state(root, approved_path, &name),
            "kind": "registry",
        }));
    }
    Ok(entries)
}

#[cfg(windows)]
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
        let mut errors = Vec::new();
        for result in [
            run_key_entries(
                "HKCU",
                "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                APPROVED_RUN,
                "hkcu_run",
            ),
            run_key_entries(
                "HKLM",
                "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                APPROVED_RUN,
                "hklm_run",
            ),
            run_key_entries(
                "HKLM",
                "Software\\Wow6432Node\\Microsoft\\Windows\\CurrentVersion\\Run",
                APPROVED_RUN32,
                "hklm_run32",
            ),
        ] {
            match result {
                Ok(found) => items.extend(found),
                Err(error) => errors.push(error),
            }
        }
        for (dir, scope) in startup_folders() {
            let root = if scope == "folder_common" {
                "HKLM"
            } else {
                "HKCU"
            };
            match fs::read_dir(&dir) {
                Ok(rd) => {
                    for e in rd {
                        let e = match e {
                            Ok(entry) => entry,
                            Err(error) => {
                                errors.push(format!("Cannot enumerate {}: {error}", dir.display()));
                                continue;
                            }
                        };
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
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => errors.push(format!("Cannot read {}: {error}", dir.display())),
            }
        }
        json!({ "items": items, "errors": errors })
    }
    #[cfg(not(windows))]
    json!({ "items": [] })
}

/// Toggle exactly like Task Manager: 12-byte REG_BINARY in StartupApproved.
pub fn toggle(scope: String, name: String, enable: bool) -> Result<Value, String> {
    #[cfg(windows)]
    {
        let (root, approved) =
            approved_location(&scope).ok_or_else(|| format!("unknown scope '{scope}'"))?;
        if root == "HKLM" && !crate::ps::is_admin() {
            return Err("Systemweite Autostart-Einträge brauchen Adminrechte.".into());
        }
        let exists = match scope.as_str() {
            "hkcu_run" | "hklm_run" | "hklm_run32" => {
                let run_path = if scope == "hklm_run32" {
                    r"Software\Wow6432Node\Microsoft\Windows\CurrentVersion\Run"
                } else {
                    r"Software\Microsoft\Windows\CurrentVersion\Run"
                };
                hive(root)
                    .open_subkey_with_flags(run_path, KEY_READ)
                    .and_then(|key| key.get_raw_value(&name))
                    .is_ok()
            }
            "folder_user" | "folder_common" => startup_folders()
                .into_iter()
                .find(|(_, folder_scope)| *folder_scope == scope)
                .is_some_and(|(dir, _)| dir.join(&name).exists()),
            _ => false,
        };
        if !exists {
            return Err(format!("Autostart-Eintrag nicht gefunden: {scope}/{name}"));
        }
        let previous = crate::tweaks::reg_read(root, approved, &name)?;
        if approved_state(root, approved, &name) == enable {
            return Ok(json!({ "name": name, "enabled": enable, "alreadySet": true }));
        }
        let mut bytes = vec![0u8; 12];
        bytes[0] = if enable { 0x02 } else { 0x03 };
        if !enable {
            // FILETIME of "now" in the trailing 8 bytes, like Task Manager.
            let ft = (chrono::Utc::now().timestamp() as u64 + 11_644_473_600) * 10_000_000;
            bytes[4..12].copy_from_slice(&ft.to_le_bytes());
        }
        let encoded = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let entry_id = crate::safety::append_entry(crate::safety::JournalEntry {
            id: format!("startup-{}", chrono::Local::now().format("%Y%m%d%H%M%S%6f")),
            tweak_id: format!("startup:{scope}:{name}"),
            tweak_name: format!("Startup entry: {name}"),
            origin: None,
            time: chrono::Local::now().to_rfc3339(),
            items: vec![crate::safety::ChangeItem::Registry {
                root: root.into(),
                path: approved.into(),
                name: name.clone(),
                prev: previous,
                new: crate::safety::RegVal::Raw {
                    ty: "REG_BINARY".into(),
                    hex: encoded,
                },
            }],
            reverted: false,
            backup_files: vec![],
            completed: false,
            attempted: 1,
            reverted_items: 0,
            untrusted: false,
        })?;
        let (key, _) = hive(root)
            .create_subkey(approved)
            .map_err(|e| format!("open StartupApproved: {e}"))?;
        key.set_raw_value(
            &name,
            &RegValue {
                bytes,
                vtype: winreg::enums::RegType::REG_BINARY,
            },
        )
        .map_err(|e| format!("set state: {e}"))?;
        let changed = approved_state(root, approved, &name) == enable;
        crate::safety::with_journal(|entries| {
            let entry = entries
                .iter_mut()
                .find(|entry| entry.id == entry_id)
                .ok_or("Startup journal entry missing")?;
            entry.completed = changed;
            Ok(())
        })?;
        if !changed {
            return Err(format!("Startup state for {name} was not updated"));
        }
        Ok(json!({ "name": name, "enabled": enable, "entryId": entry_id }))
    }
    #[cfg(not(windows))]
    {
        let _ = (scope, name, enable);
        Err("Windows only".into())
    }
}
