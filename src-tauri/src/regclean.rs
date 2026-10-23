//! Registry Orphan Cleaner.
//! Scans five categories of dead registry entries. Before each deletion the
//! affected key is exported to a real `.reg` file; `restore()` re-imports those
//! files, so deletions are genuinely reversible. If the export fails, the entry
//! is skipped rather than deleted.

use serde_json::{json, Value};

#[cfg(windows)]
use sha2::{Digest, Sha256};
#[cfg(windows)]
use std::collections::HashMap;

#[cfg(windows)]
use winreg::enums::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS, KEY_READ, KEY_SET_VALUE,
};
#[cfg(windows)]
use winreg::RegKey;

// ── stable ID from key components ──────────────────────────────────────────
#[cfg(windows)]
fn make_id(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p.as_bytes());
    }
    h.finalize()
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Registry key paths are built by the scanners only; reject anything a
/// compromised renderer could smuggle in (traversal, empty parts). Quotes
/// are fine here: the path reaches winreg/reg.exe as an argument, never a
/// shell, and MuiCache value names are full exe paths that may contain `'`.
fn is_valid_key_path(p: &str) -> bool {
    if p.is_empty() || p.len() > 1024 {
        return false;
    }
    if p.starts_with('\\') || p.ends_with('\\') {
        return false;
    }
    p.split('\\')
        .all(|c| !c.is_empty() && c != "." && c != "..")
}

fn is_valid_root(root: &str) -> bool {
    root == "HKLM" || root == "HKCU"
}

/// Structural check for a single scan entry, runs on every platform so the
/// same validation guards the Windows deletion path and stays unit-testable.
fn entry_is_valid(entry: &Value) -> Result<(), String> {
    let root = entry["root"].as_str().unwrap_or("");
    let key_path = entry["keyPath"].as_str().unwrap_or("");
    if !is_valid_root(root) {
        return Err("invalid root".into());
    }
    if !is_valid_key_path(key_path) {
        return Err("invalid key path".into());
    }
    let bad_value = entry["relatedValues"]
        .as_array()
        .map(|arr| {
            arr.iter().any(|v| {
                let s = v.as_str().unwrap_or("");
                s.is_empty() || s.len() > 1024 || s.chars().any(char::is_control)
            })
        })
        .unwrap_or(false);
    if bad_value {
        return Err("invalid related value".into());
    }
    Ok(())
}

// ── environment variable expansion ─────────────────────────────────────────

/// Expand every `%NAME%` reference with `lookup(NAME)`, the same contract as
/// `ExpandEnvironmentStringsW` (unknown variables stay verbatim). Variable
/// names are matched case-insensitively by the lookup, like Windows does.
/// Pure so it is unit-testable on every platform.
fn expand_env_with(s: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match (!name.is_empty()).then(|| lookup(name)).flatten() {
                    Some(val) => {
                        out.push_str(&val);
                        rest = &after[end + 1..];
                    }
                    None => {
                        // Not a variable: keep the first `%` and rescan from
                        // the second one (it may open a real reference).
                        out.push('%');
                        rest = after;
                    }
                }
            }
            None => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Expand `%VAR%` references against the process environment. No
/// `windows`/`winapi` crate is a dependency, so `ExpandEnvironmentStringsW`
/// is not reachable; scanning for any `%NAME%` and resolving it through the
/// process environment gives the same result for every defined variable
/// (ProgramData, USERPROFILE, CommonProgramFiles(x86), ProgramW6432, …).
/// std's env lookup is case-insensitive on Windows. A few well-known roots
/// keep a hard fallback in case the elevated environment lacks them.
#[cfg(windows)]
fn expand_env(s: &str) -> String {
    expand_env_with(s, |name| {
        std::env::var(name).ok().or_else(|| {
            let fallback = match name.to_ascii_uppercase().as_str() {
                "SYSTEMROOT" | "WINDIR" => "C:\\Windows",
                "SYSTEMDRIVE" => "C:",
                "PROGRAMFILES" | "PROGRAMW6432" => "C:\\Program Files",
                "PROGRAMFILES(X86)" => "C:\\Program Files (x86)",
                "PROGRAMDATA" | "ALLUSERSPROFILE" => "C:\\ProgramData",
                "COMMONPROGRAMFILES" | "COMMONPROGRAMW6432" => "C:\\Program Files\\Common Files",
                "COMMONPROGRAMFILES(X86)" => "C:\\Program Files (x86)\\Common Files",
                "PUBLIC" => "C:\\Users\\Public",
                _ => return None,
            };
            Some(fallback.to_string())
        })
    })
}

/// True for a fully qualified Windows path (`X:\…` or `\\server\…`); relative
/// names (`ctfmon.exe`, `shell32.dll`) are resolved through the search path
/// by Windows and cannot be checked for existence here.
fn is_qualified_path(p: &str) -> bool {
    let b = p.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'))
        || p.starts_with("\\\\")
}

/// Parse the program a command line runs, after env expansion. Returns the
/// target whose existence decides whether the entry is an orphan:
/// * `"C:\path\app.exe" /args` → the quoted path;
/// * `C:\Program Files\App\app.exe /args` → tokens up to the first one that
///   ends in `.exe` (unquoted paths with spaces);
/// * otherwise the first whitespace token;
/// * `rundll32[.exe] <dll>,Entry` → the DLL, since rundll32 itself always
///   exists.
///
/// Returns `None` when the target is not a fully qualified path (bare names
/// resolve via the search path and cannot be judged missing).
fn parse_command_target(cmd: &str) -> Option<String> {
    let s = cmd.trim();
    if s.is_empty() {
        return None;
    }
    let (first, rest): (String, &str) = if let Some(stripped) = s.strip_prefix('"') {
        let end = stripped.find('"')?;
        (stripped[..end].to_string(), &stripped[end + 1..])
    } else {
        let mut consumed = 0usize;
        let mut found: Option<usize> = None;
        for (i, ch) in s.char_indices() {
            if ch.is_whitespace() {
                let tok = &s[..i];
                if tok.to_ascii_lowercase().ends_with(".exe") {
                    found = Some(i);
                    break;
                }
            }
            consumed = i + ch.len_utf8();
        }
        let end = match found {
            Some(i) => i,
            None if s.to_ascii_lowercase().ends_with(".exe") => consumed,
            None => s.find(char::is_whitespace).unwrap_or(s.len()),
        };
        (s[..end].to_string(), &s[end..])
    };
    let first = first.trim().to_string();
    if first.is_empty() {
        return None;
    }

    let file_name = first
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(&first)
        .to_ascii_lowercase();
    if file_name == "rundll32.exe" || file_name == "rundll32" {
        let r = rest.trim_start();
        let dll = if let Some(q) = r.strip_prefix('"') {
            &q[..q.find('"')?]
        } else {
            r.split(',').next().unwrap_or("").trim()
        };
        let dll = dll.trim();
        return is_qualified_path(dll).then(|| dll.to_string());
    }

    is_qualified_path(&first).then_some(first)
}

/// Extract the real target path from a command string (see
/// [`parse_command_target`]), after expanding environment variables.
#[cfg(windows)]
fn extract_exe_path(cmd: &str) -> Option<String> {
    parse_command_target(&expand_env(cmd.trim()))
}

#[cfg(windows)]
fn exists(p: &str) -> bool {
    !p.is_empty() && std::path::Path::new(p).exists()
}

/// True when `p` is on a drive letter whose root currently exists. An
/// unmounted/removable drive must never make an entry look orphaned.
#[cfg(windows)]
fn drive_exists(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() >= 2
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && std::path::Path::new(&format!("{}:\\", b[0] as char)).exists()
}

#[cfg(windows)]
fn hive(root: &str) -> RegKey {
    RegKey::predef(if root == "HKLM" {
        HKEY_LOCAL_MACHINE
    } else {
        HKEY_CURRENT_USER
    })
}

// ── Category scanners ───────────────────────────────────────────────────────

/// MUI Cache, HKCU values whose exe path no longer exists.
/// Groups by base exe so one orphan covers all related value names.
#[cfg(windows)]
fn scan_mui_cache(out: &mut Vec<Value>) {
    const PATH: &str =
        "Software\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\Shell\\MuiCache";
    let Ok(key) = hive("HKCU").open_subkey_with_flags(PATH, KEY_READ) else {
        return;
    };

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
        if !base.contains('\\') {
            continue;
        }
        if exists(&base) {
            continue;
        }

        if let Some(&idx) = seen.get(&base.to_lowercase()) {
            groups[idx].1.push(name);
        } else {
            seen.insert(base.to_lowercase(), groups.len());
            groups.push((base, vec![name]));
        }
    }

    for (base, values) in groups {
        let fname = std::path::Path::new(&base)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
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
    let Ok(base) = hive(root).open_subkey_with_flags(subpath, KEY_READ) else {
        return;
    };
    for r in base.enum_keys() {
        let Ok(sub_name) = r else { continue };
        let Ok(sub) = base.open_subkey_with_flags(&sub_name, KEY_READ) else {
            continue;
        };
        let display: String = sub.get_value("DisplayName").unwrap_or_default();
        if display.is_empty() {
            continue;
        }
        let loc: String = sub.get_value("InstallLocation").unwrap_or_default();
        let loc = expand_env(loc.trim().trim_matches('"'));
        // A missing folder on an unmounted/removable drive is not an orphan.
        if loc.is_empty() || exists(&loc) || !drive_exists(&loc) {
            continue;
        }
        // The uninstaller itself must be gone too (on a present drive). An
        // unverifiable command (MsiExec, bare names) never counts as missing.
        let uninst: String = sub.get_value("UninstallString").unwrap_or_default();
        match extract_exe_path(&uninst) {
            Some(target) if drive_exists(&target) && !exists(&target) => {}
            _ => continue,
        }
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
    let Ok(base) = hive("HKLM").open_subkey_with_flags(PATH, KEY_READ) else {
        return;
    };
    for r in base.enum_keys() {
        let Ok(name) = r else { continue };
        let Ok(sub) = base.open_subkey_with_flags(&name, KEY_READ) else {
            continue;
        };
        let default: String = sub.get_value("").unwrap_or_default();
        let expanded = expand_env(default.trim().trim_matches('"'));
        if expanded.is_empty() || exists(&expanded) {
            continue;
        }
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
    let Ok(key) = hive("HKLM").open_subkey_with_flags(PATH, KEY_READ) else {
        return;
    };
    for r in key.enum_values() {
        let Ok((name, _)) = r else { continue };
        if name.is_empty() {
            continue;
        }
        let expanded = expand_env(&name);
        if !expanded.contains('\\') || exists(&expanded) {
            continue;
        }
        let fname = std::path::Path::new(&expanded)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
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
    let Ok(approved) = hive("HKLM").open_subkey_with_flags(APPROVED, KEY_READ) else {
        return;
    };
    let Ok(clsid_root) = hive("HKLM").open_subkey_with_flags(CLSID_BASE, KEY_READ) else {
        return;
    };

    for r in approved.enum_values() {
        let Ok((clsid, _)) = r else { continue };
        let desc: String = approved.get_value(&clsid).unwrap_or_else(|_| clsid.clone());
        let Ok(server) =
            clsid_root.open_subkey_with_flags(format!("{clsid}\\InprocServer32"), KEY_READ)
        else {
            continue;
        };
        let dll: String = server.get_value("").unwrap_or_default();
        if dll.is_empty() {
            continue;
        }
        let expanded = expand_env(dll.trim().trim_matches('"'));
        if expanded.is_empty() || exists(&expanded) {
            continue;
        }
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
    let Ok(key) = hive(root).open_subkey_with_flags(path, KEY_READ) else {
        return;
    };
    for r in key.enum_values() {
        let Ok((name, _)) = r else { continue };
        let cmd: String = key.get_value(&name).unwrap_or_default();
        let Some(exe) = extract_exe_path(&cmd) else {
            continue;
        };
        if exists(&exe) {
            continue;
        }
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
        scan_uninstall(
            "HKLM",
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
            &mut orphans,
        );
        scan_uninstall(
            "HKLM",
            "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
            &mut orphans,
        );
        scan_uninstall(
            "HKCU",
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
            &mut orphans,
        );
        scan_app_paths(&mut orphans);
        scan_shared_dlls(&mut orphans);
        scan_shell_extensions(&mut orphans);
        scan_run_keys(
            "HKCU",
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
            &mut orphans,
        );
        scan_run_keys(
            "HKLM",
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
            &mut orphans,
        );

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

/// The fields that decide what a cleanup deletes. A renderer-supplied entry
/// is only acted on when all of them equal a freshly scanned entry.
fn same_target(a: &Value, b: &Value) -> bool {
    ["id", "root", "keyPath", "valueName", "relatedValues"]
        .iter()
        .all(|k| a[*k] == b[*k])
}

/// Map each renderer-supplied entry to the matching entry of a fresh backend
/// scan. Entries the scanners did not produce right now (forged, stale, or
/// tampered key path / value name) are rejected with an error.
fn match_scanned<'a>(entries: &[Value], scanned: &'a [Value]) -> (Vec<&'a Value>, Vec<String>) {
    let mut ok: Vec<&Value> = Vec::new();
    let mut errors = Vec::new();
    for entry in entries {
        let display = entry["displayName"].as_str().unwrap_or("?");
        if let Err(m) = entry_is_valid(entry) {
            errors.push(format!("{display}: {m}"));
            continue;
        }
        match scanned.iter().find(|s| same_target(entry, s)) {
            Some(s) if !ok.iter().any(|o| std::ptr::eq(*o, s)) => ok.push(s),
            Some(_) => {} // duplicate selection
            None => errors.push(format!("{display}: not an orphan in a fresh scan, skipped")),
        }
    }
    (ok, errors)
}

/// A restorable backup session: every key is exported to its own `.reg`
/// file under `regclean/<ts>/`, and `finish` writes the `backup_<ts>.json`
/// manifest that `restore()`/`list_backups()` understand. Shared with the
/// app uninstaller's leftover cleanup so every registry deletion is
/// restorable from the same UI.
#[cfg(windows)]
pub(crate) struct RegBackup {
    reg_dir: std::path::PathBuf,
    backup_path: std::path::PathBuf,
    manifest: Vec<Value>,
    exports: usize,
}

#[cfg(windows)]
impl RegBackup {
    pub(crate) fn new() -> Result<Self, String> {
        let ts = chrono::Local::now().format("%Y%m%d_%H%M%S%6f").to_string();
        let backup_dir = crate::safety::state_dir()?.join("regclean");
        let reg_dir = backup_dir.join(&ts);
        std::fs::create_dir_all(&reg_dir).map_err(|e| format!("backup dir: {e}"))?;
        Ok(Self {
            reg_dir,
            backup_path: backup_dir.join(format!("backup_{ts}.json")),
            manifest: Vec::new(),
            exports: 0,
        })
    }

    /// Export `root\key_path` (root = "HKLM"/"HKCU") to a fresh `.reg` file.
    pub(crate) fn export(
        &mut self,
        root: &str,
        key_path: &str,
    ) -> Result<std::path::PathBuf, String> {
        if !is_valid_root(root) || !is_valid_key_path(key_path) {
            return Err("invalid key".into());
        }
        let reg_file = self.reg_dir.join(format!("{}.reg", self.exports));
        self.exports += 1;
        reg_export(&format!("{root}\\{key_path}"), &reg_file)?;
        Ok(reg_file)
    }

    /// Record a completed deletion whose key was exported to `reg_file`.
    pub(crate) fn record(
        &mut self,
        display: &str,
        root: &str,
        key_path: &str,
        value_name: &str,
        reg_file: &std::path::Path,
    ) {
        self.manifest.push(json!({
            "display": display,
            "root": root,
            "keyPath": key_path,
            "valueName": value_name,
            "regFile": reg_file.to_string_lossy().into_owned(),
        }));
    }

    pub(crate) fn deleted(&self) -> usize {
        self.manifest.len()
    }

    /// Write the restore manifest; returns its path.
    pub(crate) fn finish(self) -> Result<String, String> {
        let backup_json = json!({
            "time": chrono::Local::now().to_rfc3339(),
            "count": self.manifest.len(),
            "items": self.manifest,
        });
        std::fs::write(
            &self.backup_path,
            serde_json::to_string_pretty(&backup_json).unwrap(),
        )
        .map_err(|e| format!("backup write: {e}"))?;
        Ok(self.backup_path.to_string_lossy().into_owned())
    }
}

/// Delete `root\key_path` with all subkeys via the registry API (no shell,
/// no wildcard expansion).
#[cfg(windows)]
pub(crate) fn delete_key_tree(root: &str, key_path: &str) -> Result<(), String> {
    if !is_valid_root(root) || !is_valid_key_path(key_path) {
        return Err("invalid key".into());
    }
    let Some(sep) = key_path.rfind('\\') else {
        return Err("invalid key path".into());
    };
    hive(root)
        .open_subkey_with_flags(&key_path[..sep], KEY_ALL_ACCESS)
        .and_then(|k| k.delete_subkey_all(&key_path[sep + 1..]))
        .map_err(|e| e.to_string())
}

/// Clean the selected entries. The renderer's entries only select: every
/// one must exactly match an entry of a fresh backend scan, and deletion
/// acts on that scanned entry, never on renderer data. Each affected key is
/// exported to a `.reg` file before anything is deleted.
/// Returns { deleted, errors, backupPath }.
pub fn clean(entries: Vec<Value>) -> Result<Value, String> {
    if entries.is_empty() {
        return Err("Keine Einträge ausgewählt.".into());
    }

    let fresh = scan();
    let scanned: Vec<Value> = fresh["orphans"].as_array().cloned().unwrap_or_default();
    let (valid, mut errors) = match_scanned(&entries, &scanned);
    if valid.is_empty() {
        return Ok(json!({ "deleted": 0, "errors": errors, "backupPath": "" }));
    }

    #[cfg(windows)]
    {
        let mut backup = RegBackup::new()?;

        for entry in &valid {
            let root = entry["root"].as_str().unwrap_or("");
            let key_path = entry["keyPath"].as_str().unwrap_or("");
            let value_name = entry["valueName"].as_str().unwrap_or("");
            let display = entry["displayName"].as_str().unwrap_or("?");

            // Related values (e.g. multiple MUI cache entries for same exe)
            let related: Vec<String> = entry["relatedValues"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            // Export the affected key to a .reg file BEFORE deleting. If the
            // export fails we skip the deletion entirely, better to leave an
            // orphan than to delete something we can't put back.
            let reg_file = match backup.export(root, key_path) {
                Ok(f) => f,
                Err(e) => {
                    errors.push(format!("{display}: backup failed, skipped ({e})"));
                    continue;
                }
            };

            let result: Result<(), String> = if value_name.is_empty() {
                // Whole subkey: only scanner-produced Uninstall / App Paths
                // keys reach here (see match_scanned).
                delete_key_tree(root, key_path)
            } else if !related.is_empty() {
                // Delete all related values (MUI cache group); a value that
                // cannot be deleted must surface instead of silently
                // counting the group as cleaned.
                hive(root)
                    .open_subkey_with_flags(key_path, KEY_READ | KEY_SET_VALUE)
                    .map_err(|e| e.to_string())
                    .and_then(|k| {
                        let mut first_err = None;
                        for rv in &related {
                            if let Err(e) = k.delete_value(rv) {
                                first_err.get_or_insert(e.to_string());
                            }
                        }
                        match first_err {
                            Some(e) => Err(e),
                            None => Ok(()),
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
                Ok(_) => backup.record(display, root, key_path, value_name, &reg_file),
                Err(e) => errors.push(format!("{display}: {e}")),
            }
        }

        let deleted = backup.deleted();
        let backup_str = backup.finish()?;
        Ok(json!({ "deleted": deleted, "errors": errors, "backupPath": backup_str }))
    }
    #[cfg(not(windows))]
    {
        let _ = valid;
        Err("Windows only".into())
    }
}

/// Run reg.exe with the given args, hiding the console window. Returns the
/// combined output as an error string on non-zero exit.
#[cfg(windows)]
fn reg_exe(args: &[&str]) -> Result<(), String> {
    // ps::exec: CREATE_NO_WINDOW + hard timeout — the raw Command::output()
    // here had none, so a hung reg.exe wedged the whole cleaning pipeline.
    crate::ps::exec("reg.exe", args).map(|_| ())
}

/// Export a registry key (with all its values/subkeys) to a .reg file.
#[cfg(windows)]
fn reg_export(full_key: &str, file: &std::path::Path) -> Result<(), String> {
    let f = file.to_string_lossy().into_owned();
    reg_exe(&["export", full_key, f.as_str(), "/y"])
}

/// Restore a previous cleanup from its backup manifest by re-importing every
/// exported .reg file. Returns { restored, errors }.
///
/// Only manifests inside the protected state folder's `regclean` directory
/// are accepted, and every referenced `.reg` file must live there too,
/// otherwise this command would be an arbitrary registry-import primitive.
/// Backups left in the old user-writable folders are deliberately not
/// restorable: anyone could have planted a manifest and `.reg` file there.
pub fn restore(backup_path: String) -> Result<Value, String> {
    #[cfg(windows)]
    {
        let allowed_dirs = [crate::safety::state_dir()?.join("regclean")];
        let allowed_dirs: Vec<_> = allowed_dirs
            .iter()
            .filter_map(|dir| std::fs::canonicalize(dir).ok())
            .collect();
        let backup_canon =
            std::fs::canonicalize(&backup_path).map_err(|e| format!("cannot read backup: {e}"))?;
        if !allowed_dirs.iter().any(|dir| backup_canon.starts_with(dir)) {
            return Err("backup manifest must live in the app backup folder.".into());
        }

        let raw = std::fs::read_to_string(&backup_canon)
            .map_err(|e| format!("cannot read backup: {e}"))?;
        let parsed: Value =
            serde_json::from_str(&raw).map_err(|e| format!("bad backup file: {e}"))?;
        let items = parsed["items"].as_array().cloned().unwrap_or_default();
        if items.is_empty() {
            return Err("This backup has no restorable entries.".into());
        }

        let mut restored: u32 = 0;
        let mut errors: Vec<String> = Vec::new();
        for it in &items {
            let display = it["display"].as_str().unwrap_or("?");
            let reg_file = it["regFile"].as_str().unwrap_or("");
            let Ok(reg_canon) = std::fs::canonicalize(reg_file) else {
                errors.push(format!("{display}: backup file missing"));
                continue;
            };
            if !allowed_dirs.iter().any(|dir| reg_canon.starts_with(dir)) {
                errors.push(format!("{display}: backup file outside app folder"));
                continue;
            }
            match reg_exe(&["import", reg_canon.to_string_lossy().as_ref()]) {
                Ok(_) => restored += 1,
                Err(e) => errors.push(format!("{display}: {e}")),
            }
        }
        Ok(json!({ "restored": restored, "errors": errors }))
    }
    #[cfg(not(windows))]
    {
        let _ = backup_path;
        Err("Windows only".into())
    }
}

/// List available cleanup backups (newest first) for the restore UI.
pub fn list_backups() -> Value {
    // Only the protected state folder: restore refuses anything else.
    let current = match crate::safety::state_dir() {
        Ok(dir) => dir.join("regclean"),
        Err(_) => return json!([]),
    };
    let dirs = vec![current.clone(), current.join("regclean")];
    let mut out: Vec<Value> = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let path = e.path();
            let is_backup = path.extension().and_then(|x| x.to_str()) == Some("json")
                && path
                    .file_name()
                    .map(|n| n.to_string_lossy().starts_with("backup_"))
                    .unwrap_or(false);
            if !is_backup {
                continue;
            }
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
    out.sort_by(|a, b| {
        b["time"]
            .as_str()
            .unwrap_or("")
            .cmp(a["time"].as_str().unwrap_or(""))
    });
    json!(out)
}

#[cfg(test)]
mod tests {
    use super::{
        expand_env_with, is_valid_key_path, is_valid_root, match_scanned, parse_command_target,
    };
    use serde_json::json;

    fn env(name: &str) -> Option<String> {
        match name.to_ascii_uppercase().as_str() {
            "PROGRAMDATA" => Some("C:\\ProgramData".into()),
            "USERPROFILE" => Some("C:\\Users\\me".into()),
            "COMMONPROGRAMFILES(X86)" => Some("C:\\Program Files (x86)\\Common Files".into()),
            "WINDIR" => Some("C:\\Windows".into()),
            _ => None,
        }
    }

    #[test]
    fn expand_env_handles_any_variable() {
        assert_eq!(
            expand_env_with("%ProgramData%\\x", env),
            "C:\\ProgramData\\x"
        );
        assert_eq!(
            expand_env_with("%userprofile%\\a;%USERPROFILE%\\b", env),
            "C:\\Users\\me\\a;C:\\Users\\me\\b"
        );
        assert_eq!(
            expand_env_with("%CommonProgramFiles(x86)%\\y.dll", env),
            "C:\\Program Files (x86)\\Common Files\\y.dll"
        );
        // Unknown variables and stray percent signs stay verbatim.
        assert_eq!(expand_env_with("%NOPE%\\x", env), "%NOPE%\\x");
        assert_eq!(
            expand_env_with("50% off %windir%", env),
            "50% off C:\\Windows"
        );
        assert_eq!(expand_env_with("100%", env), "100%");
        assert_eq!(expand_env_with("%%", env), "%%");
        assert_eq!(
            expand_env_with("Müller %WINDIR%", env),
            "Müller C:\\Windows"
        );
    }

    #[test]
    fn command_target_parsing() {
        let p = |s: &str| parse_command_target(s);
        assert_eq!(
            p("\"C:\\Program Files\\App\\app.exe\" /min"),
            Some("C:\\Program Files\\App\\app.exe".into())
        );
        assert_eq!(
            p("C:\\Program Files\\App\\app.exe /min"),
            Some("C:\\Program Files\\App\\app.exe".into())
        );
        assert_eq!(p("C:\\Tools\\x.exe"), Some("C:\\Tools\\x.exe".into()));
        // The first .exe wins, an .exe in the arguments is not the target.
        assert_eq!(
            p("C:\\a\\launcher.exe --run C:\\b\\other.exe"),
            Some("C:\\a\\launcher.exe".into())
        );
        // rundll32 targets the DLL, not rundll32 itself.
        assert_eq!(
            p("C:\\Windows\\System32\\rundll32.exe \"C:\\Program Files\\X\\x.dll\",Start"),
            Some("C:\\Program Files\\X\\x.dll".into())
        );
        assert_eq!(
            p("rundll32.exe C:\\Program Files\\X\\x.dll,Start"),
            Some("C:\\Program Files\\X\\x.dll".into())
        );
        assert_eq!(p("rundll32 shell32.dll,Control_RunDLL"), None);
        // Bare names / MsiExec can't be judged missing.
        assert_eq!(p("ctfmon.exe"), None);
        assert_eq!(p("MsiExec.exe /X{1234}"), None);
        assert_eq!(p(""), None);
        assert_eq!(p("\"unterminated"), None);
        assert_eq!(
            p("\\\\server\\share\\a.exe"),
            Some("\\\\server\\share\\a.exe".into())
        );
    }

    #[test]
    fn clean_only_acts_on_freshly_scanned_entries() {
        let scanned = vec![json!({
            "id": "abc", "root": "HKCU",
            "keyPath": "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
            "valueName": "Old", "relatedValues": [], "displayName": "Old",
        })];
        // Exact match (display name may differ, it is not a target field).
        let mut ok = scanned[0].clone();
        ok["displayName"] = json!("renamed");
        // Forged: same id but whole-key delete of an arbitrary path.
        let mut forged = scanned[0].clone();
        forged["keyPath"] = json!("SOFTWARE\\Microsoft");
        forged["valueName"] = json!("");
        let mut other_value = scanned[0].clone();
        other_value["valueName"] = json!("SecurityHealth");
        let (valid, errors) = match_scanned(&[ok.clone(), ok, forged, other_value], &scanned);
        assert_eq!(valid.len(), 1);
        assert_eq!(valid[0]["valueName"], "Old");
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn key_path_validation() {
        for p in [
            "Software\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\Shell\\MuiCache",
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{A2B3C4D5}",
            "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Run",
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\My App.exe",
            "SOFTWARE\\Classes\\O'Brien Corp\\App",
        ] {
            assert!(is_valid_key_path(p), "should be valid: {p}");
        }
        for p in [
            "",
            "\\SOFTWARE\\X",
            "SOFTWARE\\X\\",
            "SOFTWARE\\..\\X",
            "SOFTWARE\\.\\X",
            "SOFTWARE\\\\X",
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run\\..\\..\\RunOnce",
        ] {
            assert!(!is_valid_key_path(p), "should be invalid: {p}");
        }
    }

    #[test]
    fn root_validation() {
        assert!(is_valid_root("HKLM"));
        assert!(is_valid_root("HKCU"));
        assert!(!is_valid_root(""));
        assert!(!is_valid_root("HKCR"));
        assert!(!is_valid_root("HKU"));
        assert!(!is_valid_root("HKLM\\SOFTWARE"));
    }
}
