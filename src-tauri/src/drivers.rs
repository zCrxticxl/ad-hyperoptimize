//! Driver Manager — list, flag, and update installed device drivers.

use crate::ps;
use serde_json::{json, Value};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Windows inbox/system drivers — date is meaningless, they're managed by WU.
fn is_inbox(manufacturer: &str, device_name: &str) -> bool {
    let m = manufacturer.to_lowercase();
    let n = device_name.to_lowercase();
    m == "microsoft" || m.starts_with("(standard ") ||
    n.contains("audio endpoint") || n.contains("system firmware") ||
    n.contains("composite bus") || n.contains("microsoft basic") ||
    n.contains("remote desktop") || n.contains("volume manager")
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
    if cls.contains("audio") || cls.contains("sound") || cls.contains("media") {
        if mfr.contains("realtek") {
            return DriverInfo {
                winget_id: Some("Realtek.RealtekHighDefinitionAudioDriver"),
                vendor_url: "https://www.realtek.com/en/component/zoo/category/pc-audio-codecs-high-definition-audio-codecs-software",
                label: "Realtek Audio Driver",
            };
        }
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
    Select-Object DeviceName, DeviceClass, DriverVersion, Manufacturer, IsSigned,
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
    let enriched: Vec<Value> = raw.into_iter().map(|mut d| {
        let cls = d["DeviceClass"].as_str().unwrap_or("").to_string();
        let mfr = d["Manufacturer"].as_str().unwrap_or("").to_string();
        let name = d["DeviceName"].as_str().unwrap_or("").to_string();
        let inbox = is_inbox(&mfr, &name);
        let info = driver_info(&cls, &mfr);
        d["wingetId"]   = json!(info.winget_id);
        d["vendorUrl"]  = json!(info.vendor_url);
        d["vendorLabel"]= json!(info.label);
        d["inbox"]      = json!(inbox);
        // inbox drivers shouldn't be flagged as "old"
        if inbox { d["old"] = json!(false); }
        d
    }).collect();

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
    let script = format!(
        r#"
$out = winget upgrade --id '{package_id}' --source winget --accept-source-agreements 2>&1 | Out-String
if ($out -match 'No applicable update') {{
    [PSCustomObject]@{{ available=$false; current=''; newVersion='' }}
}} elseif ($out -match '{package_id}\s+([\S]+)\s+([\S]+)') {{
    [PSCustomObject]@{{ available=$true; current=$Matches[1]; newVersion=$Matches[2] }}
}} else {{
    # try install to see if it's not installed at all
    $show = winget show --id '{package_id}' --source winget --accept-source-agreements 2>&1 | Out-String
    if ($show -match 'Version:\s+([\S]+)') {{
        [PSCustomObject]@{{ available=$true; current='unknown'; newVersion=$Matches[1] }}
    }} else {{
        [PSCustomObject]@{{ available=$false; current=''; newVersion='' }}
    }}
}}
"#
    );
    match ps::run_json(&script) {
        Ok(v) => v,
        Err(e) => json!({ "available": false, "error": e }),
    }
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
    // Try upgrade first, fall back to install
    let script = format!(
        r#"
$upg = winget upgrade --id '{package_id}' --source winget --accept-package-agreements --accept-source-agreements 2>&1
if ($upg -match 'No applicable update|already installed') {{
    $inst = winget install --id '{package_id}' --source winget --accept-package-agreements --accept-source-agreements 2>&1
    ($inst -join "`n").Trim()
}} else {{
    ($upg -join "`n").Trim()
}}
"#
    );
    ps::run(&script).map(|s| s.trim().to_string())
}

/// Open vendor download page in default browser.
pub fn open_vendor_url(url: String) -> Result<String, String> {
    // Whitelist URL prefixes
    let trusted = [
        "https://www.nvidia.com/", "https://www.amd.com/",
        "https://www.intel.com/", "https://www.realtek.com/",
        "https://www.mediatek.com/", "https://www.catalog.update.microsoft.com/",
    ];
    if !trusted.iter().any(|p| url.starts_with(p)) {
        return Err("URL not trusted".into());
    }
    ps::run(&format!("Start-Process '{url}'")).map(|_| format!("Opened {url}"))
}

/// Update driver directly via Windows Device Manager (devcon-style via PnPUtil).
pub fn update_via_pnputil(device_name: String) -> Result<String, String> {
    if device_name.contains('\'') || device_name.contains('"') {
        return Err("Invalid device name".into());
    }
    // pnputil /scan-devices forces WU to search for updated drivers for all devices
    let script = r#"
$r = pnputil /scan-devices 2>&1
if ($LASTEXITCODE -eq 0) { "Driver scan via Windows Update triggered. Open Windows Update to install." }
else { "Error: " + ($r -join ' ') }
"#;
    ps::run(script).map(|s| s.trim().to_string())
}

pub fn open_device_manager() -> Result<String, String> {
    ps::run("Start-Process devmgmt.msc").map(|_| "Opened Device Manager".into())
}

pub fn open_windows_update() -> Result<String, String> {
    ps::run("Start-Process 'ms-settings:windowsupdate-action'").map(|_| "Opened Windows Update".into())
}
