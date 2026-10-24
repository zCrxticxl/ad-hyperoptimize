//! App Uninstaller, list installed programs, launch uninstaller, scan/clean leftovers.

use crate::ps;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

pub fn list_apps() -> Value {
    let script = r#"
$keys = @(
    'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
)
$seen = @{}
$apps = @()
foreach ($k in $keys) {
    Get-ItemProperty $k -ErrorAction SilentlyContinue |
    Where-Object {
        $_.DisplayName -and $_.DisplayName.Trim() -ne '' -and
        -not $_.SystemComponent -and
        ($_.UninstallString -or $_.QuietUninstallString)
    } | ForEach-Object {
        $dedup = "$($_.PSPath.ToLower().Trim())"
        if (-not $seen[$dedup]) {
            $seen[$dedup] = $true
                $hive = if ($k -like '*HKCU*') { 'HKCU' } else { 'HKLM' }
            $apps += [PSCustomObject]@{
                hive            = $hive
                name            = $_.DisplayName.Trim()
                publisher       = if ($_.Publisher)       { $_.Publisher.Trim() }      else { '' }
                version         = if ($_.DisplayVersion)  { $_.DisplayVersion.Trim() } else { '' }
                installDate     = if ($_.InstallDate)     { $_.InstallDate }            else { '' }
                installLocation = if ($_.InstallLocation) { $_.InstallLocation.Trim() } else { '' }
                uninstallString = if ($_.QuietUninstallString) { $_.QuietUninstallString } else { $_.UninstallString }
                sizeMb          = if ($_.EstimatedSize -gt 0) { [math]::Round($_.EstimatedSize / 1024) } else { 0 }
            }
        }
    }
}
$apps | Sort-Object name | ConvertTo-Json -Compress -Depth 2
"#;
    let apps = match ps::run_json(script) {
        Ok(v @ Value::Array(_)) => v,
        Ok(v @ Value::Object(_)) => json!([v]),
        _ => json!([]),
    };
    remember_app_hives(&apps);
    json!({ "apps": apps })
}

pub fn uninstall_app(uninstall_string: String, from_hkcu: bool) -> Result<String, String> {
    // Detect MSI vs EXE and launch appropriately, detached so UI stays responsive.
    //
    // Never elevate an HKCU command. Re-read HKLM here: the renderer's hive
    // flag and uninstall string are untrusted input, even after a list scan.
    if from_hkcu {
        return Err("Per-user apps must be uninstalled from Windows Settings > Apps.".into());
    }
    let listed = list_apps();
    let trusted = listed["apps"].as_array().is_some_and(|apps| {
        apps.iter().any(|app| {
            app["hive"] == "HKLM"
                && app["uninstallString"].as_str() == Some(uninstall_string.as_str())
        })
    });
    if !trusted {
        return Err("Uninstaller is not a currently registered machine-wide app.".into());
    }
    let quoted = ps::quote_single(&uninstall_string);
    let script = format!(
        r#"
$us = {quoted}
if ($us -imatch '^MsiExec') {{
    $args = ($us -ireplace '^MsiExec\.exe\s*','').Trim()
    # Switch /I (install) to /X (uninstall) just in case
    $args = $args -ireplace '^/I','/X'
    Start-Process 'MsiExec.exe' -ArgumentList $args
}} else {{
    # EXE uninstaller, parse quoted path + args
    if ($us -match '^"(.+?)"\s*(.*)$') {{
        $exe  = $Matches[1]
        $rest = $Matches[2]
        Start-Process $exe -ArgumentList $rest
    }} else {{
        Start-Process 'cmd.exe' -ArgumentList '/C',$us
    }}
}}
"Uninstaller launched"
"#
    );
    ps::run(&script).map(|s| s.trim().to_string())
}

/// Hives each DisplayName (lower-cased) was seen in by [`list_apps`] during
/// this session: `(hklm, hkcu)`. Lets the leftover scan decide whether an
/// app is machine-wide even after its Uninstall key is already gone.
static APP_HIVES: Mutex<Option<HashMap<String, (bool, bool)>>> = Mutex::new(None);

/// Exact leftover paths a backend scan offered. `clean_leftovers` only acts
/// on these, never on arbitrary renderer-supplied paths.
static OFFERED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn remember_app_hives(apps: &Value) {
    let Some(arr) = apps.as_array() else { return };
    let mut guard = APP_HIVES.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    for app in arr {
        let Some(name) = app["name"].as_str() else {
            continue;
        };
        let e = map.entry(name.to_lowercase()).or_insert((false, false));
        match app["hive"].as_str() {
            Some("HKLM") => e.0 = true,
            Some("HKCU") => e.1 = true,
            _ => {}
        }
    }
}

/// HKLM leftovers are only offered for an app whose entry came from HKLM.
/// DisplayName in HKCU is user-writable: a per-user entry named like a
/// machine-wide key ("Microsoft") must never make HKLM\Software\<name>
/// deletable, so any HKCU entry with the same name also disables HKLM.
/// Unknown apps (never listed) get no HKLM leftovers.
fn hklm_leftovers_allowed(hives: Option<(bool, bool)>) -> bool {
    matches!(hives, Some((true, false)))
}

/// Names of shared/system locations directly below `Software` or a
/// user/program data root. A leftover name equal to one of these is never
/// offered, whatever DisplayName an app (possibly a spoofed HKCU entry) has.
const RESERVED_LEFTOVER_NAMES: &[&str] = &[
    "microsoft",
    "classes",
    "policies",
    "clients",
    "wow6432node",
    "registeredapplications",
    "odbc",
    "windows",
    "windows nt",
    "common files",
    "microsoft shared",
    "windowsapps",
    "modifiablewindowsapps",
    "packages",
    "windows defender",
    "windowspowershell",
    "internet explorer",
    "dotnet",
    "reference assemblies",
    "msbuild",
    "pcoptsuite",
];

/// A DisplayName is only usable as a leftover folder/key name when it is a
/// single, literal path component: no separators, no wildcard characters
/// (`* ? [ ]` would be expanded by PowerShell providers), no device/stream
/// syntax, not `.`/`..`, no trailing dot/space (Win32 strips them), and not
/// a reserved shared location.
fn leftover_name_ok(name: &str) -> bool {
    name.chars().count() >= 3
        && name.len() <= 256
        && name.trim() == name
        && !name.ends_with('.')
        && !name.chars().any(|c| {
            c.is_control()
                || matches!(
                    c,
                    '\\' | '/' | ':' | '*' | '?' | '[' | ']' | '"' | '<' | '>' | '|'
                )
        })
        && !RESERVED_LEFTOVER_NAMES.contains(&name.to_lowercase().as_str())
}

pub fn scan_leftovers(app_name: String, publisher: String, install_location: String) -> Value {
    let _ = publisher;
    let name = app_name.trim().to_string();
    if !leftover_name_ok(&name) {
        return json!({ "leftovers": [] });
    }

    // Refresh the hive map from the live registry, then decide on HKLM.
    remember_app_hives(&list_apps()["apps"]);
    let hives = APP_HIVES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|m| m.get(&name.to_lowercase()).copied());
    let allow_hklm = hklm_leftovers_allowed(hives);

    let mut found: Vec<Value> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut add = |kind: &str, path: String| {
        if seen.insert(path.to_lowercase()) {
            found.push(json!({ "type": kind, "path": path }));
        }
    };

    // File system: exact `<base>\<name>` folders only (no enumeration, no
    // wildcard semantics; the name is a validated single component).
    for base in leftover_bases() {
        let dir = base.join(&name);
        if dir.is_dir() {
            add("folder", dir.to_string_lossy().into_owned());
        }
    }

    // Install location (if it still exists after uninstall) and its leaf is
    // exactly the app name.
    let loc = install_location
        .trim()
        .trim_matches('"')
        .trim_end_matches('\\');
    let loc_path = std::path::Path::new(loc);
    if loc.len() > 3
        && loc_path.is_dir()
        && loc_path
            .file_name()
            .is_some_and(|leaf| leaf.to_string_lossy().eq_ignore_ascii_case(&name))
    {
        add("folder", loc.to_string());
    }

    // Registry: exact `Software\<name>` keys via the registry API.
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
        use winreg::RegKey;
        let mut bases: Vec<(&str, winreg::HKEY, &str)> =
            vec![("HKCU", HKEY_CURRENT_USER, "Software")];
        if allow_hklm {
            bases.push(("HKLM", HKEY_LOCAL_MACHINE, "Software"));
            bases.push(("HKLM", HKEY_LOCAL_MACHINE, "Software\\WOW6432Node"));
        }
        for (label, hkey, sub) in bases {
            let key = format!("{sub}\\{name}");
            if RegKey::predef(hkey)
                .open_subkey_with_flags(&key, KEY_READ)
                .is_ok()
            {
                add("registry", format!("{label}:\\{key}"));
            }
        }
    }
    #[cfg(not(windows))]
    let _ = allow_hklm;

    // Drop anything clean_leftovers would refuse so the UI only ever offers
    // cleanable leftovers, never a dead-end. The filter mirrors the
    // cleaner's predicate exactly.
    let cleanable: Vec<Value> = found.into_iter().filter(leftover_cleanable).collect();
    {
        let mut offered = OFFERED.lock().unwrap_or_else(|e| e.into_inner());
        let set = offered.get_or_insert_with(HashSet::new);
        for it in &cleanable {
            if let Some(p) = it["path"].as_str() {
                set.insert(p.to_string());
            }
        }
    }
    json!({ "leftovers": cleanable })
}

/// A leftover is only offered when clean_leftovers would accept it: folders
/// must live strictly under a scanned base AND not be a protected system
/// location; registry keys must pass the structural allowlist.
fn leftover_cleanable(it: &serde_json::Value) -> bool {
    let p = it["path"].as_str().unwrap_or("");
    if it["type"].as_str() == Some("folder") {
        leftover_is_under_base(p) && !crate::diskanalyzer::is_protected(std::path::Path::new(p))
    } else {
        leftover_reg_is_safe(p)
    }
}
/// Directory bases the leftover scanner enumerates (see scan_leftovers).
/// clean_leftovers refuses to touch anything outside these. Resolved without
/// the process environment: the elevated app inherits it from the caller, and
/// `ProgramFiles=C:\Users\<other>` would otherwise make another profile's
/// folders deletable. Machine folders come from HKLM (admin-writable), the
/// per-user ones from the known-folder API.
#[cfg(windows)]
fn leftover_bases() -> Vec<std::path::PathBuf> {
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    let program_files = hklm
        .open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion")
        .ok()
        .map(|k| {
            ["ProgramFilesDir", "ProgramFilesDir (x86)"]
                .iter()
                .filter_map(|v| k.get_value::<String, _>(v).ok())
                .filter(|p| !p.contains('%'))
                .map(std::path::PathBuf::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    [dirs::data_dir(), dirs::data_local_dir()]
        .into_iter()
        .flatten()
        .chain(crate::statestore::program_data_dir().ok())
        .chain(program_files)
        .collect()
}

/// Non-Windows builds (unit tests) read the same names from the environment.
#[cfg(not(windows))]
fn leftover_bases() -> Vec<std::path::PathBuf> {
    [
        "APPDATA",
        "LOCALAPPDATA",
        "PROGRAMDATA",
        "ProgramFiles",
        "ProgramFiles(x86)",
    ]
    .iter()
    .filter_map(std::env::var_os)
    .map(std::path::PathBuf::from)
    .collect()
}

/// True when `path` is a subdirectory/file strictly below one of the
/// leftover base dirs (never the base itself, never a traversal component).
fn leftover_is_under_base(path: &str) -> bool {
    let p = std::path::Path::new(path);
    if !p.is_absolute() {
        return false;
    }
    if p.components().any(|c| {
        matches!(
            c,
            std::path::Component::ParentDir | std::path::Component::CurDir
        )
    }) {
        return false;
    }
    leftover_bases()
        .iter()
        .any(|base| p != base.as_path() && p.starts_with(base))
}

/// A registry leftover path is only deletable below the bases the scanner
/// enumerates: `HKCU:\Software\…`, `HKLM:\Software\…` and
/// `HKLM:\Software\WOW6432Node\…` (at least one component below the base).
/// Wildcard characters (`* ? [ ]`) are rejected outright: PowerShell
/// providers would expand them (`Micro*` → `HKLM:\Software\Microsoft`), and
/// the first component below the base must not be a reserved shared key.
fn leftover_reg_is_safe(path: &str) -> bool {
    if path.len() > 1024 || path.contains(['*', '?', '[', ']']) {
        return false;
    }
    let rest = match path
        .strip_prefix("HKCU:\\")
        .or_else(|| path.strip_prefix("HKLM:\\"))
    {
        Some(r) => r,
        None => return false,
    };
    let comps: Vec<&str> = rest.split('\\').collect();
    if comps
        .iter()
        .any(|c| c.is_empty() || *c == "." || *c == ".." || c.chars().any(char::is_control))
    {
        return false;
    }
    if !comps[0].eq_ignore_ascii_case("software") {
        return false;
    }
    let app_idx = if comps
        .get(1)
        .map(|c| c.eq_ignore_ascii_case("wow6432node"))
        .unwrap_or(false)
    {
        2
    } else {
        1
    };
    match comps.get(app_idx) {
        Some(app) => !RESERVED_LEFTOVER_NAMES.contains(&app.to_lowercase().as_str()),
        None => false,
    }
}

/// Split a validated `HKCU:\…` / `HKLM:\…` leftover path into the
/// `(root, key_path)` form the registry API and `reg export` use.
fn split_reg_leftover(path: &str) -> Option<(&'static str, &str)> {
    if let Some(r) = path.strip_prefix("HKCU:\\") {
        Some(("HKCU", r))
    } else {
        path.strip_prefix("HKLM:\\").map(|r| ("HKLM", r))
    }
}

pub fn clean_leftovers(paths: Vec<String>) -> Result<String, String> {
    let mut cleaned = 0usize;
    let mut errors = Vec::<String>::new();
    #[cfg(windows)]
    let mut backup: Option<crate::regclean::RegBackup> = None;

    for path in &paths {
        // Only paths a backend scan offered in this session are acted on;
        // the renderer can select among them but never add new ones.
        let offered = OFFERED
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|s| s.contains(path));
        if !offered {
            errors.push(format!("{path}: not offered by a leftover scan"));
            continue;
        }
        let is_reg = path.starts_with("HKCU:") || path.starts_with("HKLM:");
        if is_reg {
            // Structural allowlist: only keys the leftover scanner itself
            // could have found, no wildcard characters.
            let Some((root, key_path)) =
                split_reg_leftover(path).filter(|_| leftover_reg_is_safe(path))
            else {
                errors.push(format!("{path}: refusing unsafe registry path"));
                continue;
            };
            #[cfg(windows)]
            {
                // Export to the app's regclean backup folder first (same
                // restorable manifest format as the Registry Cleaner); no
                // export, no deletion. The delete goes through the registry
                // API: literal key names, no provider wildcard expansion.
                if backup.is_none() {
                    match crate::regclean::RegBackup::new() {
                        Ok(b) => backup = Some(b),
                        Err(e) => {
                            errors.push(format!("{path}: backup unavailable, skipped ({e})"));
                            continue;
                        }
                    }
                }
                let b = backup.as_mut().expect("backup initialised above");
                let reg_file = match b.export(root, key_path) {
                    Ok(f) => f,
                    Err(e) => {
                        errors.push(format!("{path}: backup failed, skipped ({e})"));
                        continue;
                    }
                };
                match crate::regclean::delete_key_tree(root, key_path) {
                    Ok(()) => {
                        b.record(path, root, key_path, "", &reg_file);
                        cleaned += 1;
                    }
                    Err(e) => errors.push(format!("{path}: {e}")),
                }
            }
            #[cfg(not(windows))]
            {
                let _ = (root, key_path);
                errors.push(format!("{path}: registry cleanup is Windows only"));
            }
        } else {
            // Filesystem, same allowlist: must live under a scanned base
            // dir, must not be a protected system location, no traversal.
            if !leftover_is_under_base(path)
                || crate::diskanalyzer::is_protected(std::path::Path::new(path))
                || crate::diskanalyzer::has_reparse_ancestor(std::path::Path::new(path))
            {
                errors.push(format!("{path}: refusing unsafe path"));
                continue;
            }
            let p = std::path::Path::new(path);
            // A junction/symlink must never be recursed into; deleting the
            // link itself is always safe.
            let ok = if crate::diskanalyzer::is_reparse_point(p) {
                std::fs::remove_file(p).is_ok()
            } else if p.is_dir() {
                std::fs::remove_dir_all(p).is_ok()
            } else {
                std::fs::remove_file(p).is_ok()
            };
            if ok {
                cleaned += 1;
            } else {
                errors.push(path.clone());
            }
        }
    }

    #[cfg(windows)]
    let backup_note = match backup {
        Some(b) if b.deleted() > 0 => match b.finish() {
            Ok(p) => format!(" (registry backup: {p})"),
            Err(e) => {
                errors.push(format!("registry backup manifest: {e}"));
                String::new()
            }
        },
        _ => String::new(),
    };
    #[cfg(not(windows))]
    let backup_note = String::new();

    if errors.is_empty() {
        Ok(format!("{cleaned} items removed{backup_note}"))
    } else {
        Err(format!(
            "{cleaned} removed, {} failed: {}{backup_note}",
            errors.len(),
            errors.join("; ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        hklm_leftovers_allowed, leftover_is_under_base, leftover_name_ok, leftover_reg_is_safe,
        split_reg_leftover,
    };
    use std::path::PathBuf;

    #[test]
    fn reg_leftovers_only_below_scanned_bases() {
        assert!(leftover_reg_is_safe(r"HKCU:\Software\MyApp"));
        assert!(leftover_reg_is_safe(r"HKCU:\Software\MyApp\Sub"));
        assert!(leftover_reg_is_safe(r"HKLM:\Software\MyApp"));
        assert!(leftover_reg_is_safe(r"HKLM:\Software\WOW6432Node\MyApp"));
        // bases themselves, never deletable
        assert!(!leftover_reg_is_safe(r"HKCU:\Software"));
        assert!(!leftover_reg_is_safe(r"HKLM:\Software"));
        assert!(!leftover_reg_is_safe(r"HKLM:\Software\WOW6432Node"));
        // wrong hives / prefixes / traversal / empty segments
        assert!(!leftover_reg_is_safe(r"HKCR:\Software\X"));
        assert!(!leftover_reg_is_safe(r"HKEY_LOCAL_MACHINE:\Software\X"));
        assert!(!leftover_reg_is_safe(r"HKCU:\Windows\X"));
        assert!(!leftover_reg_is_safe(r"HKCU:\Software\..\Windows"));
        assert!(!leftover_reg_is_safe(r"HKCU:\Software\MyApp\"));
        assert!(!leftover_reg_is_safe(r"HKCU:\Software\My App\..\System"));
        assert!(!leftover_reg_is_safe(
            &("HKCU:\\Software\\".to_string() + &"A".repeat(1200))
        ));
    }

    #[test]
    fn reg_leftovers_reject_wildcards_and_reserved_keys() {
        // `Micro*` resolved to HKLM:\Software\Microsoft via provider globbing.
        assert!(!leftover_reg_is_safe(r"HKLM:\Software\Micro*"));
        assert!(!leftover_reg_is_safe(r"HKLM:\Software\Micro?oft"));
        assert!(!leftover_reg_is_safe(r"HKLM:\Software\[M]icrosoft"));
        assert!(!leftover_reg_is_safe(r"HKCU:\Software\App]"));
        assert!(!leftover_reg_is_safe(r"HKLM:\Software\Microsoft"));
        assert!(!leftover_reg_is_safe(r"HKLM:\Software\microsoft\Windows"));
        assert!(!leftover_reg_is_safe(r"HKLM:\Software\Classes"));
        assert!(!leftover_reg_is_safe(r"HKLM:\Software\Policies"));
        assert!(!leftover_reg_is_safe(
            r"HKLM:\Software\WOW6432Node\Microsoft"
        ));
        assert!(!leftover_reg_is_safe("HKCU:\\Software\\a\u{0}b"));
        assert!(leftover_reg_is_safe(r"HKLM:\Software\Microsoft Edge Tools"));
    }

    #[test]
    fn reg_leftover_split() {
        assert_eq!(
            split_reg_leftover(r"HKCU:\Software\App"),
            Some(("HKCU", r"Software\App"))
        );
        assert_eq!(
            split_reg_leftover(r"HKLM:\Software\WOW6432Node\App"),
            Some(("HKLM", r"Software\WOW6432Node\App"))
        );
        assert_eq!(split_reg_leftover(r"HKCR:\x"), None);
    }

    #[test]
    fn leftover_names_are_single_literal_components() {
        assert!(leftover_name_ok("Discord"));
        assert!(leftover_name_ok("Mozilla Firefox"));
        assert!(leftover_name_ok("Müller Tool"));
        for bad in [
            "",
            "ab",
            "Micro*",
            "App?",
            "[App]",
            r"a\b",
            "a/b",
            "C:x",
            "..",
            "App.",
            " App",
            "App ",
            "Microsoft",
            "WOW6432Node",
            "Common Files",
            "a|b",
            "a\nb",
        ] {
            assert!(!leftover_name_ok(bad), "should reject {bad:?}");
        }
    }

    #[test]
    fn hklm_leftovers_only_for_machine_wide_apps() {
        assert!(hklm_leftovers_allowed(Some((true, false))));
        // A same-named HKCU entry (user-writable) disables HKLM leftovers.
        assert!(!hklm_leftovers_allowed(Some((true, true))));
        assert!(!hklm_leftovers_allowed(Some((false, true))));
        assert!(!hklm_leftovers_allowed(None));
    }

    #[test]
    fn fs_leftovers_only_under_base_dirs() {
        // Linux: bases come from the environment; Windows: from the known
        // folders / HKLM (the environment is ignored there).
        #[cfg(not(windows))]
        let tmp = {
            let base = std::env::temp_dir().join("adho-leftover-test-base");
            std::fs::create_dir_all(&base).unwrap();
            std::env::set_var("LOCALAPPDATA", &base);
            base
        };
        let base = super::leftover_bases()
            .into_iter()
            .next()
            .expect("at least one base");

        assert!(leftover_is_under_base(base.join("MyApp").to_str().unwrap()));
        // the base itself and outside paths are rejected
        assert!(!leftover_is_under_base(base.to_str().unwrap()));
        #[cfg(not(windows))]
        assert!(!leftover_is_under_base("/etc"));
        #[cfg(windows)]
        assert!(!leftover_is_under_base(r"C:\Windows\System32\drivers"));
        let escaped = PathBuf::from(format!("{}\\..\\escape", base.display()));
        assert!(!leftover_is_under_base(escaped.to_str().unwrap()));

        #[cfg(not(windows))]
        {
            std::env::remove_var("LOCALAPPDATA");
            std::fs::remove_dir_all(&tmp).unwrap();
        }
    }
}
