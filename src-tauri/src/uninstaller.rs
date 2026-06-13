//! App Uninstaller — list installed programs, launch uninstaller, scan/clean leftovers.

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
    # EXE uninstaller — parse quoted path + args
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
    ps::run(&script)
        .map(|s| s.trim().to_string())
        .map_err(|e| e)
}

pub fn scan_leftovers(app_name: String, publisher: String, install_location: String) -> Value {
    let safe_name = app_name.replace('\'', "''");
    let safe_pub  = publisher.replace('\'', "''");
    let safe_loc  = install_location.replace('\'', "''");
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
        Ok(v @ Value::Array(_)) => json!({ "leftovers": v }),
        Ok(v @ Value::Object(_)) => json!({ "leftovers": [v] }),
        _ => json!({ "leftovers": [] }),
    }
}

pub fn clean_leftovers(paths: Vec<String>) -> Result<String, String> {
    let mut cleaned = 0usize;
    let mut errors  = Vec::<String>::new();

    for path in &paths {
        let is_reg = path.starts_with("HKCU:") || path.starts_with("HKLM:")
            || path.starts_with("HKEY_");
        if is_reg {
            let safe = path.replace('\'', "''");
            match ps::run(&format!(
                "Remove-Item -Path '{safe}' -Recurse -Force -ErrorAction Stop; 'ok'"
            )) {
                Ok(_) => cleaned += 1,
                Err(e) => errors.push(format!("{path}: {e}")),
            }
        } else {
            let p = std::path::Path::new(path);
            let ok = if p.is_dir() {
                std::fs::remove_dir_all(p).is_ok()
            } else {
                std::fs::remove_file(p).is_ok()
            };
            if ok { cleaned += 1; } else { errors.push(path.clone()); }
        }
    }

    if errors.is_empty() {
        Ok(format!("{cleaned} items removed"))
    } else {
        Ok(format!("{cleaned} removed, {} failed: {}", errors.len(), errors.join("; ")))
    }
}
