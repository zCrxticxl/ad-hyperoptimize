//! Windows Services Manager — list, start/stop, change startup type.
//! Includes a bloat catalog with known safe-to-disable services.

use crate::ps;
use serde_json::{json, Value};

struct BloatEntry {
    name:   &'static str,
    reason: &'static str,
}

static BLOAT: &[BloatEntry] = &[
    BloatEntry { name: "DiagTrack",        reason: "Microsoft Telemetry (Connected User Experiences)" },
    BloatEntry { name: "dmwappushservice", reason: "WAP Push - Telemetry Routing" },
    BloatEntry { name: "WerSvc",           reason: "Windows Error Reporting" },
    BloatEntry { name: "WSearch",          reason: "Windows Search / Indexing (unnecessary if search unused)" },
    BloatEntry { name: "SysMain",          reason: "Superfetch - can cause I/O spikes on SSD" },
    BloatEntry { name: "XblAuthManager",   reason: "Xbox Live Auth - not needed without Xbox login" },
    BloatEntry { name: "XblGameSave",      reason: "Xbox Live Game Save" },
    BloatEntry { name: "XboxNetApiSvc",    reason: "Xbox Live Networking" },
    BloatEntry { name: "XboxGipSvc",       reason: "Xbox Game Input Protocol" },
    BloatEntry { name: "MapsBroker",       reason: "Offline Maps - automatic updates" },
    BloatEntry { name: "PcaSvc",           reason: "Program Compatibility Assistant" },
    BloatEntry { name: "Fax",              reason: "Legacy Fax Service" },
    BloatEntry { name: "stisvc",           reason: "Windows Image Acquisition - only needed for scanners" },
    BloatEntry { name: "TapiSrv",          reason: "Telephony API - Legacy" },
    BloatEntry { name: "lltdsvc",          reason: "Link Layer Topology Discovery" },
    BloatEntry { name: "FDResPub",         reason: "Function Discovery Resource Publication" },
    BloatEntry { name: "SSDPSRV",          reason: "SSDP Discovery (UPnP)" },
    BloatEntry { name: "upnphost",         reason: "UPnP Device Host" },
    BloatEntry { name: "RemoteRegistry",   reason: "Remote Registry Access - security risk" },
    BloatEntry { name: "RetailDemo",       reason: "Retail Demo Mode - not needed on a normal PC" },
    BloatEntry { name: "WMPNetworkSvc",    reason: "Windows Media Player Network Sharing" },
    BloatEntry { name: "wisvc",            reason: "Windows Insider Service" },
    BloatEntry { name: "wuauserv",         reason: "Windows Update - only disable if self-managed" },
];

pub fn list() -> Value {
    let script = r#"
$descMap = @{}
try {
    Get-WmiObject Win32_Service -ErrorAction SilentlyContinue | ForEach-Object {
        if ($_.Description) { $descMap[$_.Name] = $_.Description }
    }
} catch {}
Get-Service -ErrorAction SilentlyContinue | ForEach-Object {
    [PSCustomObject]@{
        name        = $_.Name
        displayName = $_.DisplayName
        status      = $_.Status.ToString()
        startType   = $_.StartType.ToString()
        description = if ($descMap[$_.Name]) { $descMap[$_.Name] } else { '' }
    }
} | ConvertTo-Json -Compress -Depth 2
"#;
    let raw = ps::run(script).unwrap_or_default();
    let services: Vec<Value> = match serde_json::from_str(raw.trim()) {
        Ok(Value::Array(v)) => v,
        Ok(o @ Value::Object(_)) => vec![o],
        _ => return json!({ "services": [], "bloatCount": 0 }),
    };

    // Build bloat lookup
    let bloat_map: std::collections::HashMap<&str, &str> =
        BLOAT.iter().map(|b| (b.name, b.reason)).collect();

    let annotated: Vec<Value> = services.into_iter().map(|mut s| {
        let name = s["name"].as_str().unwrap_or("").to_string();
        if let Some(reason) = bloat_map.get(name.as_str()) {
            s["isBloat"] = json!(true);
            s["bloatReason"] = json!(reason);
        } else {
            s["isBloat"] = json!(false);
        }
        s
    }).collect();

    let bloat_count = annotated.iter().filter(|s| s["isBloat"].as_bool().unwrap_or(false)).count();
    json!({ "services": annotated, "bloatCount": bloat_count })
}

pub fn set_startup(name: String, startup_type: String) -> Result<Value, String> {
    let valid = ["Automatic", "AutomaticDelayedStart", "Manual", "Disabled"];
    if !valid.contains(&startup_type.as_str()) {
        return Err(format!("Invalid startup type: {startup_type}"));
    }
    let script = format!("Set-Service -Name '{name}' -StartupType {startup_type} -ErrorAction Stop");
    ps::run(&script).map_err(|e| e)?;
    Ok(json!({ "ok": true, "name": name, "startupType": startup_type }))
}

pub fn control(name: String, action: String) -> Result<Value, String> {
    let cmd = match action.as_str() {
        "start"   => format!("Start-Service -Name '{name}' -ErrorAction Stop"),
        "stop"    => format!("Stop-Service -Name '{name}' -Force -ErrorAction Stop"),
        "restart" => format!("Restart-Service -Name '{name}' -Force -ErrorAction Stop"),
        _ => return Err(format!("Unknown action: {action}")),
    };
    ps::run(&cmd).map_err(|e| e)?;
    // Return updated status
    let status_script = format!(
        "(Get-Service -Name '{name}' -ErrorAction SilentlyContinue).Status.ToString()"
    );
    let status = ps::run(&status_script).unwrap_or_default().trim().to_string();
    Ok(json!({ "ok": true, "name": name, "action": action, "status": status }))
}
