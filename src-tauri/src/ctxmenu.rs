//! Context menu cleaner — disable/enable Windows right-click bloat entries.
//! Disabling renames the registry key (appends "-bak") so it's fully reversible.

use crate::ps;
use serde_json::{json, Value};
use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

/// Curated list of known Windows context menu bloat.
/// `path` = registry key that controls the entry.
/// Disabling: rename key leaf to "<leaf>-bak"
/// Enabling:  rename back
struct CtxEntry {
    name:   &'static str,
    desc:   &'static str,
    path:   &'static str,   // full registry path (HKLM: or HKCU: prefix)
    admin:  bool,
}

static ENTRIES: &[CtxEntry] = &[
    CtxEntry { name: "Give access to / Share",        desc: "Network sharing submenu on files and folders",                              path: r"HKLM:\SOFTWARE\Classes\*\shellex\ContextMenuHandlers\Sharing",                                              admin: true  },
    CtxEntry { name: "Cast to Device",                desc: "Streams media to a Chromecast or DLNA device",                              path: r"HKLM:\SOFTWARE\Classes\*\shellex\ContextMenuHandlers\{7AD84985-87B4-4a16-BE58-8B72A5B390F7}",             admin: true  },
    CtxEntry { name: "Previous Versions",             desc: "Restore from shadow copies / File History",                                  path: r"HKLM:\SOFTWARE\Classes\AllFilesystemObjects\shellex\ContextMenuHandlers\{596AB062-B4D2-4215-9F74-E9109B0A8153}", admin: true  },
    CtxEntry { name: "Scan with Microsoft Defender",  desc: "Adds Defender scan to every right-click",                                   path: r"HKLM:\SOFTWARE\Classes\*\shellex\ContextMenuHandlers\EPP",                                                  admin: true  },
    CtxEntry { name: "Include in Library",            desc: "Adds folders to Windows libraries",                                         path: r"HKLM:\SOFTWARE\Classes\Folder\shellex\ContextMenuHandlers\Library Location",                                admin: true  },
    CtxEntry { name: "Pin to Start",                  desc: "Pin to Start menu option on apps",                                          path: r"HKLM:\SOFTWARE\Classes\*\shell\pintostartscreen",                                                            admin: true  },
    CtxEntry { name: "Pin to Taskbar",                desc: "Pin to Taskbar option on shortcuts/EXEs",                                   path: r"HKLM:\SOFTWARE\Classes\*\shell\taskbarpin",                                                                  admin: true  },
    CtxEntry { name: "Edit with Paint 3D",            desc: "Adds Paint 3D to image file right-click",                                   path: r"HKLM:\SOFTWARE\Classes\SystemFileAssociations\.bmp\Shell\3D Edit",                                          admin: true  },
    CtxEntry { name: "Open with (extra submenu)",     desc: "Secondary Open With submenu for all files",                                  path: r"HKLM:\SOFTWARE\Classes\*\shellex\ContextMenuHandlers\Open With",                                             admin: true  },
    CtxEntry { name: "OneDrive - Sync status",        desc: "OneDrive file sync overlay and context menu",                               path: r"HKCU:\Software\Classes\*\shellex\ContextMenuHandlers\FileSyncEx",                                            admin: false },
    CtxEntry { name: "OneDrive - Folder sync",        desc: "OneDrive folder sync status icons",                                         path: r"HKCU:\Software\Classes\Folder\shellex\ContextMenuHandlers\FileSyncEx",                                       admin: false },
    CtxEntry { name: "Send to - Bluetooth",           desc: "Bluetooth option in Send To submenu",                                       path: r"HKLM:\SOFTWARE\Classes\AllFilesystemObjects\shellex\ContextMenuHandlers\SendTo",                             admin: true  },
    CtxEntry { name: "Restore to previous version",   desc: "File-level shadow copy restore (duplicate of Previous Versions)",           path: r"HKLM:\SOFTWARE\Classes\AllFilesystemObjects\shellex\ContextMenuHandlers\{450D8FBA-AD25-11D0-98A8-0800361B1103}", admin: true  },
    CtxEntry { name: "Windows Ink Workspace",         desc: "Annotate in Windows Ink when right-clicking images",                        path: r"HKLM:\SOFTWARE\Classes\*\shell\ms-penworkspace",                                                             admin: true  },
    CtxEntry { name: "Print to PDF (printto verb)",   desc: "Print to PDF option shown when right-clicking documents",                   path: r"HKLM:\SOFTWARE\Classes\*\shell\printto",                                                                     admin: true  },
];

/// Parse a registry path string like `HKLM:\SOFTWARE\...` into (root_key, subkey, leaf).
fn open_parent(path: &str) -> Option<(RegKey, String)> {
    // path format: HKLM:\sub\key  or  HKCU:\sub\key
    let (hive, rest) = if let Some(r) = path.strip_prefix(r"HKLM:\") {
        (HKEY_LOCAL_MACHINE, r)
    } else if let Some(r) = path.strip_prefix(r"HKCU:\") {
        (HKEY_CURRENT_USER, r)
    } else if let Some(r) = path.strip_prefix(r"HKCR:\") {
        (HKEY_CLASSES_ROOT, r)
    } else {
        return None;
    };
    // Map HKLM:\SOFTWARE\Classes\* → HKEY_CLASSES_ROOT
    // (PowerShell HKLM:\SOFTWARE\Classes is same as HKCR:\)
    let (root, subpath) = if rest.starts_with(r"SOFTWARE\Classes\") {
        (RegKey::predef(HKEY_CLASSES_ROOT), rest.trim_start_matches(r"SOFTWARE\Classes\"))
    } else {
        (RegKey::predef(hive), rest)
    };
    Some((root, subpath.to_string()))
}

fn reg_key_exists(path: &str) -> bool {
    let Some((root, sub)) = open_parent(path) else { return false };
    root.open_subkey(&sub).is_ok()
}

pub fn list_entries() -> Value {
    let entries: Vec<Value> = ENTRIES
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let bak_path = format!("{}-bak", e.path);
            let exists   = reg_key_exists(e.path);
            let disabled = reg_key_exists(&bak_path);
            let present  = exists || disabled;
            let enabled  = exists;
            json!({
                "idx":     i,
                "name":    e.name,
                "desc":    e.desc,
                "path":    e.path,
                "admin":   e.admin,
                "present": present,
                "enabled": enabled,
            })
        })
        .collect();

    json!({ "entries": entries })
}

pub fn toggle_entry(path: String, enable: bool) -> Result<String, String> {
    let bak = format!("{path}-bak");
    let script = if enable {
        format!(
            r#"
if (Test-Path '{bak}') {{
    Rename-Item -Path '{bak}' -NewName ('{path}' | Split-Path -Leaf) -Force -ErrorAction Stop
    "Enabled"
}} elseif (Test-Path '{path}') {{ "Already enabled" }}
else {{ "Key not found" }}
"#
        )
    } else {
        let leaf_bak = format!(
            "{}-bak",
            path.split('\\').last().unwrap_or("key")
        );
        format!(
            r#"
if (Test-Path '{path}') {{
    Rename-Item -Path '{path}' -NewName '{leaf_bak}' -Force -ErrorAction Stop
    "Disabled"
}} elseif (Test-Path '{bak}') {{ "Already disabled" }}
else {{ "Key not found — may not be installed" }}
"#
        )
    };
    ps::run(&script)
        .map(|s| s.trim().to_string())
        .map_err(|e| e)
}

pub fn disable_all_bloat() -> Result<String, String> {
    let mut disabled = 0usize;
    let mut errors   = Vec::<String>::new();

    for e in ENTRIES {
        if !e.admin {
            // HKCU entries — don't need admin, always attempt
        }
        match toggle_entry(e.path.to_string(), false) {
            Ok(s) if s == "Disabled" => disabled += 1,
            Ok(_) => {}
            Err(err) => errors.push(format!("{}: {err}", e.name)),
        }
    }

    if errors.is_empty() {
        Ok(format!("{disabled} entries disabled"))
    } else {
        Ok(format!("{disabled} disabled, {} errors: {}", errors.len(), errors.join("; ")))
    }
}

pub fn enable_all() -> Result<String, String> {
    let mut enabled = 0usize;
    for e in ENTRIES {
        if let Ok(s) = toggle_entry(e.path.to_string(), true) {
            if s == "Enabled" { enabled += 1; }
        }
    }
    Ok(format!("{enabled} entries re-enabled"))
}
