//! Driver Manager, list, flag, and update installed device drivers.

use crate::ps;
use serde_json::{json, Value};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Date Microsoft stamps on every inbox driver so that any vendor driver
/// outranks it; it says nothing about the driver's age.
const INBOX_DRIVER_DATE: &str = "2006-06-21";

/// Windows inbox/system drivers, date is meaningless, they're managed by WU.
/// The provider is not localized ("Microsoft"); the manufacturer is
/// ("(Standard)", "(Standardsystemgeräte)", ...).
fn is_inbox(provider: &str, date: &str, manufacturer: &str, device_name: &str) -> bool {
    let m = manufacturer.to_lowercase();
    let n = device_name.to_lowercase();
    provider.eq_ignore_ascii_case("microsoft")
        || date == INBOX_DRIVER_DATE
        || m == "microsoft"
        || m.starts_with("(standard")
        || m.starts_with("(standard ")
        || n.contains("audio endpoint")
        || n.contains("system firmware")
        || n.contains("composite bus")
        || n.contains("microsoft basic")
        || n.contains("remote desktop")
        || n.contains("volume manager")
}

struct DriverInfo {
    winget_id: Option<&'static str>,
    vendor_url: &'static str,
    label: &'static str,
}

fn driver_info(device_class: &str, manufacturer: &str) -> DriverInfo {
    let cls = device_class.to_lowercase();
    let mfr = manufacturer.to_lowercase();

    if cls.contains("display") || cls.contains("video") {
        if mfr.contains("nvidia") {
            return DriverInfo {
                winget_id: Some("Nvidia.GeForce.GameReadyDriver"),
                vendor_url: "https://www.nvidia.com/Download/index.aspx",
                label: "NVIDIA Driver Download",
            };
        }
        if mfr.contains("amd") || mfr.contains("advanced micro") {
            return DriverInfo {
                winget_id: Some("AMD.Software.Adrenalin.Edition"),
                vendor_url: "https://www.amd.com/en/support",
                label: "AMD Driver Download",
            };
        }
        if mfr.contains("intel") {
            return DriverInfo {
                winget_id: Some("Intel.IntelDriverAndSupportAssistant"),
                vendor_url: "https://www.intel.com/content/www/us/en/download-center/home.html",
                label: "Intel Download Center",
            };
        }
    }
    if (cls.contains("audio") || cls.contains("sound") || cls.contains("media"))
        && mfr.contains("realtek")
    {
        return DriverInfo {
                winget_id: Some("Realtek.RealtekHighDefinitionAudioDriver"),
                vendor_url: "https://www.realtek.com/en/component/zoo/category/pc-audio-codecs-high-definition-audio-codecs-software",
                label: "Realtek Audio Driver",
            };
    }
    if cls.contains("net") || cls.contains("bluetooth") {
        if mfr.contains("intel") {
            return DriverInfo {
                winget_id: Some("Intel.IntelDriverAndSupportAssistant"),
                vendor_url: "https://www.intel.com/content/www/us/en/download-center/home.html",
                label: "Intel Download Center",
            };
        }
        if mfr.contains("realtek") {
            return DriverInfo {
                winget_id: Some("Realtek.RealtekDriverInstallationProgram"),
                vendor_url: "https://www.realtek.com/en/component/zoo/category/network-interface-controllers-10-100-1000m-gigabit-ethernet-pci-express-software",
                label: "Realtek NIC Driver",
            };
        }
        if mfr.contains("mediatek") || mfr.contains("ralink") {
            return DriverInfo {
                winget_id: None,
                vendor_url: "https://www.mediatek.com/products/home-networking",
                label: "MediaTek Downloads",
            };
        }
    }
    if mfr.contains("intel") {
        return DriverInfo {
            winget_id: Some("Intel.IntelDriverAndSupportAssistant"),
            vendor_url: "https://www.intel.com/content/www/us/en/download-center/home.html",
            label: "Intel Download Center",
        };
    }
    DriverInfo {
        winget_id: None,
        vendor_url: "https://www.catalog.update.microsoft.com/Home.aspx",
        label: "Windows Update Catalog",
    }
}

// ── public API ────────────────────────────────────────────────────────────────

pub fn list_drivers() -> Value {
    let script = r#"
$cutoff = (Get-Date).AddYears(-2)
$skip_mfr = @('Microsoft','(Standard disk drives)','(Standard SATA AHCI Controller)')
$drivers = Get-WmiObject Win32_PnPSignedDriver -ErrorAction SilentlyContinue |
    Where-Object { $_.DeviceName -and $_.DriverVersion -and
                   $_.DeviceClass -notmatch '^(Processor|System|Computer|Volume|SCSIAdapter)$' -and
                   $_.DeviceName -notmatch '^(Remote|Composite|Microsoft|Generic|Unknown)' } |
    Select-Object DeviceName, DeviceClass, DriverVersion, Manufacturer, DriverProviderName, IsSigned,
                  @{n='dateStr';e={
                      if($_.DriverDate) {
                          [Management.ManagementDateTimeConverter]::ToDateTime($_.DriverDate).ToString('yyyy-MM-dd')
                      } else { '' }
                  }},
                  @{n='old';e={
                      if($_.DriverDate) {
                          [Management.ManagementDateTimeConverter]::ToDateTime($_.DriverDate) -lt $cutoff
                      } else { $false }
                  }} |
    Sort-Object DeviceClass, DeviceName
$drivers | ConvertTo-Json -Compress -Depth 3
"#;
    let raw = match ps::run_json(script) {
        Ok(Value::Array(arr)) => arr,
        Ok(v @ Value::Object(_)) => vec![v],
        _ => vec![],
    };

    // Enrich with winget ID + vendor URL + inbox flag in Rust (no PS overhead)
    let enriched: Vec<Value> = raw
        .into_iter()
        .map(|mut d| {
            let cls = d["DeviceClass"].as_str().unwrap_or("").to_string();
            let mfr = d["Manufacturer"].as_str().unwrap_or("").to_string();
            let name = d["DeviceName"].as_str().unwrap_or("").to_string();
            let provider = d["DriverProviderName"].as_str().unwrap_or("");
            let date = d["dateStr"].as_str().unwrap_or("");
            let inbox = is_inbox(provider, date, &mfr, &name);
            let info = driver_info(&cls, &mfr);
            d["wingetId"] = json!(info.winget_id);
            d["vendorUrl"] = json!(info.vendor_url);
            d["vendorLabel"] = json!(info.label);
            d["inbox"] = json!(inbox);
            // inbox drivers shouldn't be flagged as "old"
            if inbox {
                d["old"] = json!(false);
            }
            d
        })
        .collect();

    json!({ "drivers": enriched })
}

/// Check a specific winget package ID for available update.
/// More reliable than parsing the full upgrade list.
pub fn check_winget_package(package_id: String) -> Value {
    let allowed = [
        "Nvidia.GeForce.GameReadyDriver",
        "AMD.Software.Adrenalin.Edition",
        "Intel.IntelDriverAndSupportAssistant",
        "Realtek.RealtekHighDefinitionAudioDriver",
        "Realtek.RealtekDriverInstallationProgram",
    ];
    if !allowed.contains(&package_id.as_str()) {
        return json!({ "available": false, "error": "not whitelisted" });
    }
    let winget = ps::quote_single(&ps::winget_exe());
    let pid_q = ps::quote_single(&package_id);
    let script = format!(
        r#"
$pidEsc = {pid_q}
$out = & {winget} upgrade --id $pidEsc --exact --source winget --accept-source-agreements 2>&1 | Out-String
$exitCode = $LASTEXITCODE
if ($out -match [regex]::Escape($pidEsc) + '\s+([^\s]+)\s+([^\s]+)') {{
    [PSCustomObject]@{{ available=$true; current=$Matches[1]; newVersion=$Matches[2] }}
}} elseif ($exitCode -ne 0 -and $exitCode -ne -1978335212) {{
    [PSCustomObject]@{{ available=$false; current=''; newVersion=''; error=($out.Trim()) }}
}} else {{
    [PSCustomObject]@{{ available=$false; current=''; newVersion='' }}
}}
"#
    );
    match ps::run_json_long(&script) {
        Ok(v) => v,
        Err(e) => json!({ "available": false, "error": e }),
    }
}

/// winget APPINSTALLER_CLI_ERROR_* codes (as signed 32-bit exit codes).
/// 0x8A150014: no installed package matched (upgrade) / no package found.
const WINGET_NO_PACKAGE_FOUND: i64 = -1978335212;
/// 0x8A15002B: installed, no newer version available.
const WINGET_UPDATE_NOT_APPLICABLE: i64 = -1978335189;
/// 0x8A150061: install of a package that is already installed.
const WINGET_ALREADY_INSTALLED: i64 = -1978335135;

/// Exit codes of the upgrade-or-install step that mean "done". "No package
/// found" is not among them: it used to count as success, so installing a
/// driver package that was not installed reported success and did nothing.
fn winget_install_ok(code: i64) -> bool {
    matches!(
        code,
        0 | WINGET_UPDATE_NOT_APPLICABLE | WINGET_ALREADY_INSTALLED
    )
}

/// Install or upgrade a whitelisted driver package via winget.
pub fn install_via_winget(package_id: String) -> Result<String, String> {
    let allowed = [
        "Nvidia.GeForce.GameReadyDriver",
        "AMD.Software.Adrenalin.Edition",
        "Intel.IntelDriverAndSupportAssistant",
        "Realtek.RealtekHighDefinitionAudioDriver",
        "Realtek.RealtekDriverInstallationProgram",
    ];
    if !allowed.contains(&package_id.as_str()) {
        return Err(format!("Package ID not whitelisted: {package_id}"));
    }
    // Try upgrade first; only "no installed package" (exit code, not the
    // localized text) falls back to install. Every branch reports the real
    // winget exit code so failures are never presented as success.
    let winget = ps::quote_single(&ps::winget_exe());
    let script = format!(
        r#"
$upg = & {winget} upgrade --id '{package_id}' --exact --source winget --accept-package-agreements --accept-source-agreements 2>&1
$upgCode = $LASTEXITCODE
if ($upgCode -eq {WINGET_NO_PACKAGE_FOUND}) {{
    $inst = & {winget} install --id '{package_id}' --exact --source winget --accept-package-agreements --accept-source-agreements 2>&1
    $instCode = $LASTEXITCODE
    "EXIT:$instCode`n" + ($inst -join "`n")
}} else {{
    "EXIT:$upgCode`n" + ($upg -join "`n")
}}
"#
    );
    ps::run_long(&script)
        .map(|s| {
            let out = s.trim();
            let exit_ok = out
                .lines()
                .next()
                .and_then(|l| l.strip_prefix("EXIT:"))
                .and_then(|c| c.trim().parse::<i64>().ok())
                .is_some_and(winget_install_ok);
            if exit_ok {
                Ok(out
                    .lines()
                    .skip(1)
                    .collect::<Vec<_>>()
                    .join("\n")
                    .trim()
                    .to_string())
            } else {
                Err(out.to_string())
            }
        })
        .and_then(|r| r)
}

/// Open vendor download page in default browser.
pub fn open_vendor_url(url: String) -> Result<String, String> {
    // Whitelist URL prefixes + reject PS quote characters (the url is
    // embedded in a single-quoted Start-Process argument).
    let trusted = [
        "https://www.nvidia.com/",
        "https://www.amd.com/",
        "https://www.intel.com/",
        "https://www.realtek.com/",
        "https://www.mediatek.com/",
        "https://www.catalog.update.microsoft.com/",
    ];
    if !trusted.iter().any(|p| url.starts_with(p)) || !ps::is_safe_ident(&url) {
        return Err("URL not trusted".into());
    }
    // Through explorer.exe (like cmd_open_path): the running shell opens the
    // browser unelevated. Start-Process here started it with the admin token.
    // explorer's exit code is meaningless, so the child is not waited on.
    std::process::Command::new(std::path::Path::new(&ps::windows_dir()).join("explorer.exe"))
        .arg(&url)
        .spawn()
        .map(|_| format!("Opened {url}"))
        .map_err(|e| format!("explorer: {e}"))
}

/// Rescan hardware (`pnputil /scan-devices`): Windows re-enumerates devices
/// and binds drivers already in the driver store. It does not search Windows
/// Update; newer drivers come from Windows Update or the vendor.
pub fn update_via_pnputil() -> Result<String, String> {
    let (status, stdout, stderr) = ps::exec_capture("pnputil.exe", &["/scan-devices"])?;
    if status.success() {
        Ok("Hardware rescan done: new or changed devices were detected and drivers already on this PC were bound. Newer drivers come from Windows Update or the vendor.".into())
    } else {
        let detail = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        Err(format!("pnputil /scan-devices failed: {detail}"))
    }
}

pub fn open_device_manager() -> Result<String, String> {
    ps::run("Start-Process devmgmt.msc").map(|_| "Opened Device Manager".into())
}

pub fn open_windows_update() -> Result<String, String> {
    std::process::Command::new(std::path::Path::new(&ps::windows_dir()).join("explorer.exe"))
        .arg("ms-settings:windowsupdate-action")
        .spawn()
        .map(|_| "Opened Windows Update".into())
        .map_err(|e| format!("explorer: {e}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn inbox_drivers_are_recognized_on_localized_systems() {
        // From a German Windows 11: TPM, SATA AHCI and WinUsb are inbox
        // drivers with the fixed 2006-06-21 date.
        assert!(super::is_inbox(
            "Microsoft",
            "2006-06-21",
            "(Standard)",
            "Trusted Platform Module 2.0"
        ));
        assert!(super::is_inbox(
            "",
            "2006-06-21",
            "Standard SATA AHCI Controller",
            "Standard SATA AHCI Controller"
        ));
        assert!(super::is_inbox(
            "Microsoft",
            "2024-01-01",
            "WinUsb Device",
            "WinUsb Device"
        ));
        assert!(!super::is_inbox(
            "WireGuard LLC",
            "2021-10-13",
            "WireGuard LLC",
            "Tailscale Tunnel"
        ));
        assert!(!super::is_inbox(
            "HyperX",
            "2023-10-20",
            "HyperX",
            "DTS HPXV2 Service for HyperX"
        ));
    }

    use super::*;

    #[test]
    fn missing_package_is_not_reported_as_installed() {
        assert!(winget_install_ok(0));
        assert!(winget_install_ok(WINGET_UPDATE_NOT_APPLICABLE));
        assert!(winget_install_ok(WINGET_ALREADY_INSTALLED));
        assert!(!winget_install_ok(WINGET_NO_PACKAGE_FOUND));
        assert!(!winget_install_ok(1));
    }
}
