//! App Uninstaller, list installed programs, launch uninstaller, scan/clean leftovers.

use crate::ps;
use serde_json::{json, Value};

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
        $dedup = $_.DisplayName.ToLower().Trim()
        if (-not $seen[$dedup]) {
            $seen[$dedup] = $true
            $apps += [PSCustomObject]@{
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
    match ps::run_json(script) {
        Ok(v @ Value::Array(_)) => json!({ "apps": v }),
        Ok(v @ Value::Object(_)) => json!({ "apps": [v] }),
        _ => json!({ "apps": [] }),
    }
}

pub fn uninstall_app(uninstall_string: String) -> Result<String, String> {
    // Detect MSI vs EXE and launch appropriately, detached so UI stays responsive.
    let safe = uninstall_string.replace('\'', "''");
    let script = format!(
        r#"
$us = '{safe}'
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

pub fn scan_leftovers(app_name: String, publisher: String, install_location: String) -> Value {
    let safe_name = app_name.replace('\'', "''");
    let safe_pub = publisher.replace('\'', "''");
    let safe_loc = install_location.replace('\'', "''");
    let script = format!(
        r#"
$name = '{safe_name}'
$pub  = '{safe_pub}'
$loc  = '{safe_loc}'
$found = @()
$seen  = @{{}}

function Add($type, $path) {{
    if ($seen[$path]) {{ return }}
    $seen[$path] = $true
    $script:found += [PSCustomObject]@{{ type=$type; path=$path }}
}}

# Keywords to search for
$kws = @($name)
if ($pub -and $pub -ne '') {{ $kws += $pub }}

# File system
$dirs = @($env:APPDATA, $env:LOCALAPPDATA, $env:PROGRAMDATA,
          $env:ProgramFiles, ${{env:ProgramFiles(x86)}}) | Where-Object {{ $_ -and (Test-Path $_) }}
foreach ($dir in $dirs) {{
    foreach ($kw in $kws) {{
        if ($kw.Length -lt 3) {{ continue }}
        Get-ChildItem $dir -Directory -ErrorAction SilentlyContinue |
            Where-Object {{ $_.Name -like "*$kw*" }} |
            ForEach-Object {{ Add 'folder' $_.FullName }}
    }}
}}

# Install location (if still exists after uninstall)
if ($loc -and $loc.Length -gt 3 -and (Test-Path $loc)) {{ Add 'folder' $loc }}

# Registry
$regBases = @('HKCU:\Software', 'HKLM:\Software', 'HKLM:\Software\WOW6432Node')
foreach ($base in $regBases) {{
    foreach ($kw in $kws) {{
        if ($kw.Length -lt 3) {{ continue }}
        $sub = Join-Path $base $kw
        if (Test-Path $sub) {{ Add 'registry' $sub }}
    }}
}}

if ($found.Count -eq 0) {{ '[]' }} else {{ @($found) | ConvertTo-Json -Compress }}
"#
    );
    match ps::run_json(&script) {
        // Drop anything clean_leftovers would refuse so the UI only ever
        // offers cleanable leftovers, never a dead-end. The filter mirrors
        // the cleaner's predicate exactly (under-base AND not protected).
        Ok(v @ Value::Array(_)) => {
            let cleanable: Vec<Value> = v
                .as_array()
                .unwrap()
                .iter()
                .filter(|it| leftover_cleanable(it))
                .cloned()
                .collect();
            json!({ "leftovers": cleanable })
        }
        Ok(v @ Value::Object(_)) => {
            if leftover_cleanable(&v) {
                json!({ "leftovers": [v] })
            } else {
                json!({ "leftovers": [] })
            }
        }
        _ => json!({ "leftovers": [] }),
    }
}

/// A leftover is only offered when clean_leftovers would accept it: folders
/// must live strictly under a scanned base AND not be a protected system
/// location. Registry items are always cleanable (structurally allowlisted
/// below). This predicate mirrors clean_leftovers so the UI can never show an
/// item that the cleaner would refuse.
fn leftover_cleanable(it: &serde_json::Value) -> bool {
    if it["type"].as_str() == Some("folder") {
        let p = it["path"].as_str().unwrap_or("");
        leftover_is_under_base(p) && !crate::diskanalyzer::is_protected(std::path::Path::new(p))
    } else {
        true
    }
}

/// Directory bases the leftover scanner enumerates (see scan_leftovers).
/// clean_leftovers refuses to touch anything outside these.
const LEFTOVER_BASES: &[&str] = &[
    "APPDATA",
    "LOCALAPPDATA",
    "PROGRAMDATA",
    "ProgramFiles",
    "ProgramFiles(x86)",
];

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
    LEFTOVER_BASES.iter().any(|b| {
        std::env::var_os(b)
            .map(|base| {
                let base = std::path::PathBuf::from(base);
                p != base && p.starts_with(&base)
            })
            .unwrap_or(false)
    })
}

/// A registry leftover path is only deletable below the bases the scanner
/// enumerates: `HKCU:\Software\…`, `HKLM:\Software\…` and
/// `HKLM:\Software\WOW6432Node\…` (at least one component below the base).
fn leftover_reg_is_safe(path: &str) -> bool {
    if path.len() > 1024 {
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
        .any(|c| c.is_empty() || *c == "." || *c == "..")
    {
        return false;
    }
    if !comps[0].eq_ignore_ascii_case("software") {
        return false;
    }
    if comps
        .get(1)
        .map(|c| c.eq_ignore_ascii_case("wow6432node"))
        .unwrap_or(false)
    {
        comps.len() >= 3
    } else {
        comps.len() >= 2
    }
}

pub fn clean_leftovers(paths: Vec<String>) -> Result<String, String> {
    let mut cleaned = 0usize;
    let mut errors = Vec::<String>::new();

    for path in &paths {
        let is_reg = path.starts_with("HKCU:") || path.starts_with("HKLM:");
        if is_reg {
            // Renderer input, structural allowlist before Remove-Item:
            // only keys the leftover scanner itself could have found.
            if !leftover_reg_is_safe(path) {
                errors.push(format!("{path}: refusing unsafe registry path"));
                continue;
            }
            let safe = path.replace('\'', "''");
            match ps::run(&format!(
                "Remove-Item -Path '{safe}' -Recurse -Force -ErrorAction Stop; 'ok'"
            )) {
                Ok(_) => cleaned += 1,
                Err(e) => errors.push(format!("{path}: {e}")),
            }
        } else {
            // Filesystem, same allowlist: must live under a scanned base
            // dir, must not be a protected system location, no traversal.
            if !leftover_is_under_base(path)
                || crate::diskanalyzer::is_protected(std::path::Path::new(path))
            {
                errors.push(format!("{path}: refusing unsafe path"));
                continue;
            }
            let p = std::path::Path::new(path);
            let ok = if p.is_dir() {
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

    if errors.is_empty() {
        Ok(format!("{cleaned} items removed"))
    } else {
        Ok(format!(
            "{cleaned} removed, {} failed: {}",
            errors.len(),
            errors.join("; ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{leftover_is_under_base, leftover_reg_is_safe};
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
    fn fs_leftovers_only_under_base_dirs() {
        let base = std::env::temp_dir().join("adho-leftover-test-base");
        std::fs::create_dir_all(&base).unwrap();
        std::env::set_var("LOCALAPPDATA", &base);

        let sub = base.join("MyApp");
        std::fs::create_dir_all(&sub).unwrap();
        assert!(leftover_is_under_base(sub.to_str().unwrap()));

        // the base itself and outside paths are rejected
        assert!(!leftover_is_under_base(base.to_str().unwrap()));
        assert!(!leftover_is_under_base("/etc"));
        assert!(!leftover_is_under_base(&format!(
            "{}\\..\\escape",
            base.display()
        )));

        let escaped = PathBuf::from(format!("{}\\..\\escape", base.display()));
        assert!(!leftover_is_under_base(escaped.to_str().unwrap()));

        std::env::remove_var("LOCALAPPDATA");
        std::fs::remove_dir_all(&base).unwrap();
    }
}
