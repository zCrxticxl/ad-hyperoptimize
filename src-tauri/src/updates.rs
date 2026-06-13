//! Update engine.
//!
//! Apps:    winget (built into Windows 10 1809+/11). List + selective or
//!          bulk silent upgrades.
//! Drivers: Windows Update Agent COM API — WHQL-signed drivers straight from
//!          Microsoft. Deliberately NO third-party driver sources: that whole
//!          ecosystem is scamware. GPU drivers get a vendor hint instead,
//!          since vendor packages (NVIDIA/AMD) are newer than WU's.

use crate::ps;
use serde_json::{json, Value};

// ---------------- apps (winget) ----------------

/// Installed-software info from the registry Uninstall keys, used to enrich
/// winget rows with publisher + install location.
fn registry_apps() -> Vec<Value> {
    let script = r#"
$paths = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
         'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
         'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
Get-ItemProperty $paths -ErrorAction SilentlyContinue |
  Where-Object DisplayName |
  ForEach-Object {
    $loc = $_.InstallLocation
    if (-not $loc -and $_.DisplayIcon) { $loc = Split-Path ($_.DisplayIcon -split ',')[0] -ErrorAction SilentlyContinue }
    @{ name = $_.DisplayName; publisher = $_.Publisher; location = $loc }
  }
"#;
    match ps::run_json(script) {
        Ok(Value::Array(a)) => a,
        Ok(v) if v.is_object() => vec![v],
        _ => vec![],
    }
}

/// Parse `winget upgrade` fixed-width table. Locale-independent: columns are
/// located by header word offsets (char-based). Carriage-return progress
/// spinner output is stripped, footer sentences and secondary sections are
/// rejected by id sanity checks.
pub fn scan_app_updates() -> Result<Value, String> {
    let raw = ps::exec(
        "winget.exe",
        &["upgrade", "--accept-source-agreements", "--disable-interactivity"],
    )
    .map_err(|e| if e.is_empty() { "winget not available on this system".into() } else { e })?;

    // winget redraws progress with '\r' — keep only the final segment per line.
    let lines: Vec<String> = raw
        .lines()
        .map(|l| l.rsplit('\r').next().unwrap_or("").to_string())
        .collect();

    let is_dash = |l: &str| {
        let t = l.trim();
        t.len() >= 10 && t.chars().all(|c| c == '-')
    };
    let dash_idx = lines
        .iter()
        .position(|l| is_dash(l))
        .ok_or("could not locate winget table (no updates available, or unexpected output)")?;
    if dash_idx == 0 {
        return Err("unexpected winget output".into());
    }
    let header: Vec<char> = lines[dash_idx - 1].chars().collect();

    // Column start offsets = char index of each header word start.
    let mut cols: Vec<usize> = Vec::new();
    let mut prev_space = true;
    for (i, ch) in header.iter().enumerate() {
        if !ch.is_whitespace() && prev_space {
            cols.push(i);
        }
        prev_space = ch.is_whitespace();
    }
    if cols.len() < 4 {
        return Err("unexpected winget header".into());
    }

    let slice = |chars: &[char], idx: usize| -> String {
        let start = cols[idx].min(chars.len());
        let end = if idx + 1 < cols.len() { cols[idx + 1].min(chars.len()) } else { chars.len() };
        chars[start..end].iter().collect::<String>().trim().to_string()
    };

    // winget ids are single tokens: "Publisher.Name", "ARP\..." or Store ids.
    let id_ok = |id: &str| {
        !id.is_empty()
            && !id.contains(' ')
            && id.chars().any(|c| c.is_ascii_alphanumeric())
    };

    let reg = registry_apps();
    let find_reg = |name: &str| -> (Value, Value) {
        let prefix = name.split('…').next().unwrap_or("").trim().to_lowercase();
        if prefix.len() < 3 {
            return (Value::Null, Value::Null);
        }
        for r in &reg {
            let dn = r["name"].as_str().unwrap_or("").to_lowercase();
            if dn.starts_with(&prefix) || prefix.starts_with(&dn) || dn.contains(&prefix) {
                return (r["publisher"].clone(), r["location"].clone());
            }
        }
        (Value::Null, Value::Null)
    };

    let mut apps = Vec::new();
    for line in &lines[dash_idx + 1..] {
        if is_dash(line) {
            break; // secondary section (e.g. pinned packages) — stop
        }
        let chars: Vec<char> = line.chars().collect();
        if chars.len() < cols[1] + 1 {
            if apps.is_empty() { continue } else { break } // footer / blank
        }
        let name = slice(&chars, 0);
        let id = slice(&chars, 1);
        let version = slice(&chars, 2);
        let available = slice(&chars, 3);
        let source = if cols.len() > 4 { slice(&chars, 4) } else { String::new() };
        if !id_ok(&id) || available.is_empty() || name.is_empty() {
            continue; // footer sentence fragments, progress junk
        }
        let (publisher, location) = find_reg(&name);
        apps.push(json!({
            "name": name,
            "id": id,
            "version": version,
            "available": available,
            "source": source,
            "publisher": publisher,
            "location": location,
            // Truncated ids (winget ellipsis) can't be targeted individually.
            "targetable": !id.contains('…'),
        }));
    }
    Ok(json!({ "count": apps.len(), "apps": apps }))
}

/// Upgrade one app by exact id, or everything when `id` is None.
/// Silent, license-accepting. Can run for many minutes — async command.
pub fn update_apps(id: Option<String>) -> Result<Value, String> {
    let mut args: Vec<String> = vec!["upgrade".into()];
    match &id {
        Some(pkg) => {
            args.push("--id".into());
            args.push(pkg.clone());
            args.push("--exact".into());
        }
        None => args.push("--all".into()),
    }
    // --disable-interactivity suppresses UAC on some packages → omit for per-app updates.
    let extra: &[&str] = if id.is_some() {
        &["--silent", "--accept-source-agreements", "--accept-package-agreements"]
    } else {
        &["--silent", "--accept-source-agreements", "--accept-package-agreements", "--disable-interactivity"]
    };
    args.extend(extra.iter().map(|s| s.to_string()));
    let argrefs: Vec<&str> = args.iter().map(String::as_str).collect();

    // winget writes errors to stdout (not stderr) — capture both.
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    let mut cmd = std::process::Command::new("winget.exe");
    cmd.args(&argrefs);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

    let out = cmd.output().map_err(|e| format!("winget spawn: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.rsplit('\r').next().unwrap_or("").to_string())
        .collect::<Vec<_>>()
        .join("\n");

    if !out.status.success() {
        // Extract last meaningful lines from stdout as the error message
        let err_lines: Vec<&str> = stdout.lines()
            .filter(|l| !l.trim().is_empty())
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let msg = if err_lines.is_empty() {
            format!("winget exited with code {}", out.status)
        } else {
            err_lines.join("\n")
        };
        return Err(msg);
    }

    let tail: String = stdout.lines()
        .filter(|l| !l.trim().is_empty())
        .rev().take(20).collect::<Vec<_>>()
        .into_iter().rev().collect::<Vec<_>>()
        .join("\n");
    Ok(json!({ "ok": true, "log": tail }))
}

// ---------------- drivers (Windows Update Agent) ----------------

/// Search Windows Update for missing driver updates (read-only).
pub fn scan_driver_updates() -> Value {
    let script = r#"
$session = New-Object -ComObject Microsoft.Update.Session
$searcher = $session.CreateUpdateSearcher()
try {
  $result = $searcher.Search("IsInstalled=0 and Type='Driver'")
  $result.Updates | ForEach-Object {
    @{ title = $_.Title
       driverModel = $_.DriverModel
       manufacturer = $_.DriverManufacturer
       verDate = if ($_.DriverVerDate) { $_.DriverVerDate.ToString('yyyy-MM-dd') } else { $null }
       downloaded = $_.IsDownloaded
       sizeMb = [math]::Round($_.MaxDownloadSize/1MB,1) }
  }
} catch { @{ error = $_.Exception.Message } }
"#;
    match ps::run_json(script) {
        Ok(Value::Null) => json!({ "count": 0, "drivers": [] }),
        Ok(v) => {
            let arr = if v.is_array() { v.as_array().unwrap().clone() } else { vec![v] };
            if arr.len() == 1 && arr[0].get("error").is_some() {
                return json!({ "error": arr[0]["error"] });
            }
            json!({ "count": arr.len(), "drivers": arr })
        }
        Err(e) => json!({ "error": e.trim() }),
    }
}

/// Download + install ALL missing WU driver updates. Admin required.
/// Returns per-update result codes and whether a reboot is needed.
pub fn install_driver_updates() -> Result<Value, String> {
    if !ps::is_admin() {
        return Err("Driver installation needs administrator rights. Restart the app as admin.".into());
    }
    let script = r#"
$session = New-Object -ComObject Microsoft.Update.Session
$searcher = $session.CreateUpdateSearcher()
$result = $searcher.Search("IsInstalled=0 and Type='Driver'")
if ($result.Updates.Count -eq 0) {
  @{ installed = 0; rebootRequired = $false; overall = 'NothingToDo'; results = @() }
} else {
$coll = New-Object -ComObject Microsoft.Update.UpdateColl
foreach ($u in $result.Updates) {
  if (-not $u.EulaAccepted) { $u.AcceptEula() | Out-Null }
  $coll.Add($u) | Out-Null
}
$downloader = $session.CreateUpdateDownloader(); $downloader.Updates = $coll
$downloader.Download() | Out-Null
$installer = $session.CreateUpdateInstaller(); $installer.Updates = $coll
$ir = $installer.Install()
$codes = @('NotStarted','InProgress','Succeeded','SucceededWithErrors','Failed','Aborted')
$results = @()
for ($i = 0; $i -lt $coll.Count; $i++) {
  $results += @{ title = $coll.Item($i).Title; result = $codes[$ir.GetUpdateResult($i).ResultCode] }
}
@{ installed = $coll.Count; rebootRequired = [bool]$ir.RebootRequired; overall = $codes[$ir.ResultCode]; results = $results }
}
"#;
    ps::run_json(script)
}

/// Detect GPU vendor for the "get your GPU driver from the vendor" hint.
pub fn gpu_vendor_hint() -> Value {
    let gpus = ps::run("(Get-CimInstance Win32_VideoController).Name -join ';'").unwrap_or_default();
    let g = gpus.to_lowercase();
    let (vendor, url) = if g.contains("nvidia") {
        ("NVIDIA", "https://www.nvidia.com/Download/index.aspx")
    } else if g.contains("amd") || g.contains("radeon") {
        ("AMD", "https://www.amd.com/en/support")
    } else if g.contains("intel") || g.contains("arc") {
        ("Intel", "https://www.intel.com/content/www/us/en/download-center/home.html")
    } else {
        ("unknown", "")
    };
    json!({ "gpus": gpus.trim(), "vendor": vendor, "url": url })
}
