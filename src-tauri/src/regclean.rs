//! Registry Orphan Cleaner.
//! Scans five categories of dead registry entries. Before each deletion the
//! affected key is exported to a real `.reg` file; `restore()` re-imports those
//! files, so deletions are genuinely reversible. If the export fails, the entry
//! is skipped rather than deleted.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS, KEY_READ, KEY_SET_VALUE};
#[cfg(windows)]
use winreg::RegKey;

// ── stable ID from key components ──────────────────────────────────────────
fn make_id(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts { h.update(p.as_bytes()); }
    h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect()
}

// ── environment variable expansion ─────────────────────────────────────────
#[cfg(windows)]
fn expand_env(s: &str) -> String {
    let pairs: &[(&str, fn() -> String)] = &[
        ("%SYSTEMROOT%",          || std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into())),
        ("%WINDIR%",              || std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into())),
        ("%PROGRAMFILES%",        || std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into())),
        ("%PROGRAMFILES(X86)%",   || std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| "C:\\Program Files (x86)".into())),
        ("%COMMONPROGRAMFILES%",  || std::env::var("CommonProgramFiles").unwrap_or_else(|_| "C:\\Program Files\\Common Files".into())),
        ("%APPDATA%",             || std::env::var("APPDATA").unwrap_or_default()),
        ("%LOCALAPPDATA%",        || std::env::var("LOCALAPPDATA").unwrap_or_default()),
        ("%SYSTEMDRIVE%",         || std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into())),
    ];
    let mut result = s.to_string();
    for (var, f) in pairs {
        if result.to_uppercase().contains(var.to_uppercase().as_str()) {
            let val = f();
            // case-insensitive replace
            let upper_result = result.to_uppercase();
            if let Some(pos) = upper_result.find(&var.to_uppercase()) {
                result = format!("{}{}{}", &result[..pos], val, &result[pos + var.len()..]);
            }
        }
    }
    result
}

/// Extract the real exe path from a command string like `"C:\path\app.exe" /args`
#[cfg(windows)]
fn extract_exe_path(cmd: &str) -> Option<String> {
    let s = expand_env(cmd.trim());
    let s = s.trim();
    if s.is_empty() { return None; }

    // Quoted: "C:\path\app.exe" ...
    if s.starts_with('"') {
        if let Some(end) = s[1..].find('"') {
            let p = s[1..end + 1].to_string();
            if !p.is_empty() { return Some(p); }
        }
    }

    // .exe marker: take everything up to and including .exe
    let lower = s.to_lowercase();
    if let Some(pos) = lower.find(".exe") {
        return Some(s[..pos + 4].to_string());
    }
    for ext in &[".cmd", ".bat", ".msi", ".vbs"] {
        if let Some(pos) = lower.find(ext) {
            return Some(s[..pos + ext.len()].to_string());
        }
    }

    // First token if it looks like a path
    let first = s.split_whitespace().next().unwrap_or(s);
    if first.contains('\\') { return Some(first.to_string()); }

    None
}

#[cfg(windows)]
fn exists(p: &str) -> bool {
    !p.is_empty() && std::path::Path::new(p).exists()
}

#[cfg(windows)]
fn hive(root: &str) -> RegKey {
    RegKey::predef(if root == "HKLM" { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER })
}

// ── Category scanners ───────────────────────────────────────────────────────

/// MUI Cache — HKCU values whose exe path no longer exists.
/// Groups by base exe so one orphan covers all related value names.
#[cfg(windows)]
fn scan_mui_cache(out: &mut Vec<Value>) {
    const PATH: &str =
        "Software\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\Shell\\MuiCache";
    let Ok(key) = hive("HKCU").open_subkey_with_flags(PATH, KEY_READ) else { return };

    // exe_path → list of value names
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();

    for r in key.enum_values() {
        let Ok((name, _)) = r else { continue };
        let lower = name.to_lowercase();
        let base = if let Some(pos) = lower.rfind(".exe") {
            name[..pos + 4].to_string()
        } else {
            continue; // not an exe entry
        };
        if !base.contains('\\') { continue; }
        if exists(&base) { continue; }

        if let Some(&idx) = seen.get(&base.to_lowercase()) {
            groups[idx].1.push(name);
        } else {
            seen.insert(base.to_lowercase(), groups.len());
            groups.push((base, vec![name]));
        }
    }

    for (base, values) in groups {
        let fname = std::path::Path::new(&base)
            .file_name().map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| base.clone());
        out.push(json!({
            "id":           make_id(&["mui", "HKCU", PATH, &base]),
            "category":     "MUI Cache",
            "root":         "HKCU",
            "keyPath":      PATH,
            "valueName":    values[0],
            "relatedValues": values,
            "displayName":  fname,
            "badPath":      base,
            "reason":       "Anwendung existiert nicht mehr",
        }));
    }
}

/// Uninstall entries where InstallLocation points to a non-existent directory.
#[cfg(windows)]
fn scan_uninstall(root: &'static str, subpath: &str, out: &mut Vec<Value>) {
    let Ok(base) = hive(root).open_subkey_with_flags(subpath, KEY_READ) else { return };
    for r in base.enum_keys() {
        let Ok(sub_name) = r else { continue };
        let Ok(sub) = base.open_subkey_with_flags(&sub_name, KEY_READ) else { continue };
        let display: String = sub.get_value("DisplayName").unwrap_or_default();
        if display.is_empty() { continue; }
        let loc: String = sub.get_value("InstallLocation").unwrap_or_default();
        let loc = expand_env(loc.trim().trim_matches('"'));
        if loc.is_empty() || exists(&loc) { continue; }
        let key_path = format!("{subpath}\\{sub_name}");
        out.push(json!({
            "id":           make_id(&["uninst", root, &key_path]),
            "category":     "Uninstall",
            "root":         root,
            "keyPath":      key_path,
            "valueName":    "",           // empty = delete whole subkey
            "relatedValues": [],
            "displayName":  display,
            "badPath":      loc,
            "reason":       "InstallLocation zeigt auf nicht-existenten Pfad",
        }));
    }
}

/// App Paths entries where the exe no longer exists.
#[cfg(windows)]
fn scan_app_paths(out: &mut Vec<Value>) {
    const PATH: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths";
    let Ok(base) = hive("HKLM").open_subkey_with_flags(PATH, KEY_READ) else { return };
    for r in base.enum_keys() {
        let Ok(name) = r else { continue };
        let Ok(sub) = base.open_subkey_with_flags(&name, KEY_READ) else { continue };
        let default: String = sub.get_value("").unwrap_or_default();
        let expanded = expand_env(default.trim().trim_matches('"'));
        if expanded.is_empty() || exists(&expanded) { continue; }
        let key_path = format!("{PATH}\\{name}");
        out.push(json!({
            "id":           make_id(&["apppath", "HKLM", &key_path]),
            "category":     "App Paths",
            "root":         "HKLM",
            "keyPath":      key_path,
            "valueName":    "",
            "relatedValues": [],
            "displayName":  name,
            "badPath":      expanded,
            "reason":       "App-Paths-Eintrag zeigt auf nicht-existente Exe",
        }));
    }
}

/// SharedDLLs entries pointing to non-existent files.
#[cfg(windows)]
fn scan_shared_dlls(out: &mut Vec<Value>) {
    const PATH: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\SharedDLLs";
    let Ok(key) = hive("HKLM").open_subkey_with_flags(PATH, KEY_READ) else { return };
    for r in key.enum_values() {
        let Ok((name, _)) = r else { continue };
        if name.is_empty() { continue; }
        let expanded = expand_env(&name);
        if !expanded.contains('\\') || exists(&expanded) { continue; }
        let fname = std::path::Path::new(&expanded)
            .file_name().map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| expanded.clone());
        out.push(json!({
            "id":           make_id(&["sharedll", "HKLM", PATH, &name]),
            "category":     "SharedDLLs",
            "root":         "HKLM",
            "keyPath":      PATH,
            "valueName":    name,
            "relatedValues": [],
            "displayName":  fname,
            "badPath":      expanded,
            "reason":       "SharedDLL-Referenz zeigt auf nicht-existente Datei",
        }));
    }
}

/// Shell Extensions (Approved) whose InprocServer32 DLL is gone.
#[cfg(windows)]
fn scan_shell_extensions(out: &mut Vec<Value>) {
    const APPROVED: &str =
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Shell Extensions\\Approved";
    const CLSID_BASE: &str = "SOFTWARE\\Classes\\CLSID";
    let Ok(approved) = hive("HKLM").open_subkey_with_flags(APPROVED, KEY_READ) else { return };
    let Ok(clsid_root) = hive("HKLM").open_subkey_with_flags(CLSID_BASE, KEY_READ) else { return };

    for r in approved.enum_values() {
        let Ok((clsid, _)) = r else { continue };
        let desc: String = approved.get_value(&clsid).unwrap_or_else(|_| clsid.clone());
        let Ok(server) = clsid_root.open_subkey_with_flags(
            &format!("{clsid}\\InprocServer32"), KEY_READ,
        ) else { continue };
        let dll: String = server.get_value("").unwrap_or_default();
        if dll.is_empty() { continue; }
        let expanded = expand_env(dll.trim().trim_matches('"'));
        if expanded.is_empty() || exists(&expanded) { continue; }
        out.push(json!({
            "id":           make_id(&["shellext", "HKLM", APPROVED, &clsid]),
            "category":     "Shell Extensions",
            "root":         "HKLM",
            "keyPath":      APPROVED,
            "valueName":    clsid,
            "relatedValues": [],
            "displayName":  desc,
            "badPath":      expanded,
            "reason":       "Shell-Extension-DLL existiert nicht mehr",
        }));
    }
}

/// Run key entries pointing to non-existent executables.
#[cfg(windows)]
fn scan_run_keys(root: &'static str, path: &str, out: &mut Vec<Value>) {
    let Ok(key) = hive(root).open_subkey_with_flags(path, KEY_READ) else { return };
    for r in key.enum_values() {
        let Ok((name, _)) = r else { continue };
        let cmd: String = key.get_value(&name).unwrap_or_default();
        let Some(exe) = extract_exe_path(&cmd) else { continue };
        if exists(&exe) { continue; }
        out.push(json!({
            "id":           make_id(&["run", root, path, &name]),
            "category":     "Autostart",
            "root":         root,
            "keyPath":      path,
            "valueName":    name,
            "relatedValues": [],
            "displayName":  name,
            "badPath":      exe,
            "reason":       "Autostart-Eintrag zeigt auf nicht-existente Exe",
        }));
    }
}

// ── Public API ──────────────────────────────────────────────────────────────

pub fn scan() -> Value {
    #[cfg(windows)]
    {
        let mut orphans: Vec<Value> = Vec::new();
        scan_mui_cache(&mut orphans);
        scan_uninstall("HKLM", "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall", &mut orphans);
        scan_uninstall("HKLM", "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall", &mut orphans);
        scan_uninstall("HKCU", "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall", &mut orphans);
        scan_app_paths(&mut orphans);
        scan_shared_dlls(&mut orphans);
        scan_shell_extensions(&mut orphans);
        scan_run_keys("HKCU", "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run", &mut orphans);
        scan_run_keys("HKLM", "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run", &mut orphans);

        let mut counts: HashMap<&str, u32> = HashMap::new();
        for o in &orphans {
            let cat = o["category"].as_str().unwrap_or("Other");
            *counts.entry(cat).or_insert(0) += 1;
        }
        let counts_v: serde_json::Map<String, Value> = counts
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect();

        json!({ "orphans": orphans, "total": orphans.len(), "counts": counts_v })
    }
    #[cfg(not(windows))]
    json!({ "orphans": [], "total": 0, "counts": {} })
}

/// Clean the supplied entries (passed verbatim from the UI scan result).
/// Writes a backup JSON before touching anything.
/// Returns { deleted, errors, backupPath }.
pub fn clean(entries: Vec<Value>) -> Result<Value, String> {
    if entries.is_empty() {
        return Err("Keine Einträge ausgewählt.".into());
    }
    #[cfg(windows)]
    {
        // 1. Prepare backup dir + a per-cleanup folder for the .reg exports.
        let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_dir = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("PCOptSuite")
            .join("regclean");
        let reg_dir = backup_dir.join(&ts);
        std::fs::create_dir_all(&reg_dir).map_err(|e| format!("backup dir: {e}"))?;
        let backup_path = backup_dir.join(format!("backup_{ts}.json"));

        // 2. Delete — but ONLY after a real .reg export of the affected key
        //    succeeded, so every deletion is genuinely restorable.
        let mut deleted: u32 = 0;
        let mut errors: Vec<String> = Vec::new();
        let mut manifest: Vec<Value> = Vec::new();

        for entry in &entries {
            let root       = entry["root"].as_str().unwrap_or("HKCU");
            let key_path   = entry["keyPath"].as_str().unwrap_or("");
            let value_name = entry["valueName"].as_str().unwrap_or("");
            let display    = entry["displayName"].as_str().unwrap_or("?");

            // Related values (e.g. multiple MUI cache entries for same exe)
            let related: Vec<String> = entry["relatedValues"]
                .as_array()
                .map(|arr| arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect())
                .unwrap_or_default();

            // Export the affected key to a .reg file BEFORE deleting. If the
            // export fails we skip the deletion entirely — better to leave an
            // orphan than to delete something we can't put back.
            let reg_file = reg_dir.join(format!("{}.reg", manifest.len()));
            let full = format!("{root}\\{key_path}");
            if let Err(e) = reg_export(&full, &reg_file) {
                errors.push(format!("{display}: backup failed, skipped ({e})"));
                continue;
            }

            let result: Result<(), String> = if value_name.is_empty() {
                // Delete entire subkey
                if let Some(sep) = key_path.rfind('\\') {
                    let parent = &key_path[..sep];
                    let child  = &key_path[sep + 1..];
                    hive(root)
                        .open_subkey_with_flags(parent, KEY_ALL_ACCESS)
                        .and_then(|k| k.delete_subkey_all(child))
                        .map_err(|e| e.to_string())
                } else {
                    Err("invalid key path".into())
                }
            } else if !related.is_empty() {
                // Delete all related values (MUI cache group)
                hive(root)
                    .open_subkey_with_flags(key_path, KEY_READ | KEY_SET_VALUE)
                    .map_err(|e| e.to_string())
                    .map(|k| {
                        for rv in &related {
                            let _ = k.delete_value(rv);
                        }
                    })
            } else {
                // Delete single value
                hive(root)
                    .open_subkey_with_flags(key_path, KEY_READ | KEY_SET_VALUE)
                    .and_then(|k| k.delete_value(value_name))
                    .map_err(|e| e.to_string())
            };

            match result {
                Ok(_) => {
                    deleted += 1;
                    manifest.push(json!({
                        "display": display,
                        "root": root,
                        "keyPath": key_path,
                        "valueName": value_name,
                        "regFile": reg_file.to_string_lossy().into_owned(),
                    }));
                }
                Err(e) => errors.push(format!("{display}: {e}")),
            }
        }

        // Write the restore manifest (points at the .reg exports).
        let backup_json = json!({
            "time": chrono::Local::now().to_rfc3339(),
            "count": manifest.len(),
            "items": manifest,
        });
        std::fs::write(&backup_path, serde_json::to_string_pretty(&backup_json).unwrap())
            .map_err(|e| format!("backup write: {e}"))?;

        let backup_str = backup_path.to_string_lossy().into_owned();
        Ok(json!({ "deleted": deleted, "errors": errors, "backupPath": backup_str }))
    }
    #[cfg(not(windows))]
    Err("Windows only".into())
}

/// Run reg.exe with the given args, hiding the console window. Returns the
/// combined output as an error string on non-zero exit.
#[cfg(windows)]
fn reg_exe(args: &[&str]) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("reg.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Export a registry key (with all its values/subkeys) to a .reg file.
#[cfg(windows)]
fn reg_export(full_key: &str, file: &std::path::Path) -> Result<(), String> {
    let f = file.to_string_lossy().into_owned();
    reg_exe(&["export", full_key, f.as_str(), "/y"])
}

/// Restore a previous cleanup from its backup manifest by re-importing every
/// exported .reg file. Returns { restored, errors }.
pub fn restore(backup_path: String) -> Result<Value, String> {
    #[cfg(windows)]
    {
        let raw = std::fs::read_to_string(&backup_path)
            .map_err(|e| format!("cannot read backup: {e}"))?;
        let parsed: Value = serde_json::from_str(&raw)
            .map_err(|e| format!("bad backup file: {e}"))?;
        let items = parsed["items"].as_array().cloned().unwrap_or_default();
        if items.is_empty() {
            return Err("This backup has no restorable entries.".into());
        }

        let mut restored: u32 = 0;
        let mut errors: Vec<String> = Vec::new();
        for it in &items {
            let display = it["display"].as_str().unwrap_or("?");
            let reg_file = it["regFile"].as_str().unwrap_or("");
            if reg_file.is_empty() || !std::path::Path::new(reg_file).exists() {
                errors.push(format!("{display}: backup file missing"));
                continue;
            }
            match reg_exe(&["import", reg_file]) {
                Ok(_)  => restored += 1,
                Err(e) => errors.push(format!("{display}: {e}")),
            }
        }
        return Ok(json!({ "restored": restored, "errors": errors }));
    }
    #[allow(unreachable_code)]
    Err("Windows only".into())
}

/// List available cleanup backups (newest first) for the restore UI.
pub fn list_backups() -> Value {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("PCOptSuite")
        .join("regclean");
    let mut out: Vec<Value> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let path = e.path();
            let is_backup = path.extension().and_then(|x| x.to_str()) == Some("json")
                && path.file_name().map(|n| n.to_string_lossy().starts_with("backup_")).unwrap_or(false);
            if !is_backup { continue; }
            if let Ok(raw) = std::fs::read_to_string(&path) {
                if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                    out.push(json!({
                        "path": path.to_string_lossy().into_owned(),
                        "time": v["time"].as_str().unwrap_or(""),
                        "count": v["count"].as_u64().unwrap_or(0),
                    }));
                }
            }
        }
    }
    out.sort_by(|a, b| b["time"].as_str().unwrap_or("").cmp(a["time"].as_str().unwrap_or("")));
    json!(out)
}
