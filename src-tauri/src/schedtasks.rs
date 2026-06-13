//! Scheduled Tasks Manager.
//! Lists all Windows Scheduled Tasks and lets the user enable/disable them.
//! Includes a curated list of known background/telemetry tasks with descriptions.

use serde_json::{json, Value};
use std::collections::HashMap;

/// (task_path_lowercase, task_name_lowercase) → reason string.
fn bloat_catalog() -> HashMap<(String, String), &'static str> {
    let entries: &[(&str, &str, &str)] = &[
        // Application Experience
        ("\\microsoft\\windows\\application experience\\", "microsoft compatibility appraiser",
            "Sends app compatibility telemetry to Microsoft. Runs daily and generates disk I/O."),
        ("\\microsoft\\windows\\application experience\\", "programdataupdater",
            "Updates the appcompat telemetry database — pure data collection for Microsoft."),
        ("\\microsoft\\windows\\application experience\\", "startupapptask",
            "Scans startup programs for Microsoft analysis after login."),
        // Autochk
        ("\\microsoft\\windows\\autochk\\", "proxy",
            "Forwards Autochk results (disk errors) to Microsoft (telemetry)."),
        // CEIP
        ("\\microsoft\\windows\\customer experience improvement program\\", "consolidator",
            "CEIP telemetry: Sends system usage data to Microsoft. Runs hourly."),
        ("\\microsoft\\windows\\customer experience improvement program\\", "usbceip",
            "Sends USB device data as part of the Customer Experience Improvement Program."),
        // Disk
        ("\\microsoft\\windows\\diskdiagnostic\\", "microsoft-windows-diskdiagnosticdatacollector",
            "Collects disk diagnostic data and forwards it to Microsoft."),
        ("\\microsoft\\windows\\diskfootprint\\", "diagnostics",
            "Profiles disk I/O patterns in the background — generates load with no user benefit."),
        // WinSAT
        ("\\microsoft\\windows\\maintenance\\", "winsat",
            "Windows System Assessment Tool — runs benchmarks in the background and generates CPU/disk load."),
        // Maps
        ("\\microsoft\\windows\\maps\\", "mapsupdatetask",
            "Automatically downloads offline map updates for the Windows Maps app."),
        ("\\microsoft\\windows\\maps\\", "mapstoasttask",
            "Sends notifications from the Windows Maps app."),
        // NetTrace
        ("\\microsoft\\windows\\nettrace\\", "gathernetworkinfo",
            "Collects detailed network diagnostic data — background overhead with no discernible benefit."),
        // Power Efficiency
        ("\\microsoft\\windows\\power efficiency diagnostics\\", "analyzesystem",
            "Runs power efficiency analysis and sends results to Microsoft."),
        // Family Safety
        ("\\microsoft\\windows\\shell\\", "familysafetyrefreshtask",
            "Updates Family Safety / parental control policies from the Microsoft server."),
        // WER
        ("\\microsoft\\windows\\windows error reporting\\", "queuereporting",
            "Sends queued error reports to Microsoft. Unnecessary when Windows Error Reporting is disabled."),
        // Windows Update
        ("\\microsoft\\windows\\windowsupdate\\", "automatic app update",
            "Automatically updates Store apps in the background — separate from normal Windows Update."),
        ("\\microsoft\\windows\\windowsupdate\\", "scheduled start",
            "Starts Windows Update scans on a fixed schedule."),
        // Workplace Join
        ("\\microsoft\\windows\\workplace join\\", "automatic-device-join",
            "Automatically registers the device in Azure AD / Workplace Join (enterprise feature)."),
        // PushToInstall
        ("\\microsoft\\windows\\pushtoinstall\\", "logincheck",
            "Checks Push-to-Install Store tasks at login — irrelevant for non-enterprise."),
        // MUI language packs
        ("\\microsoft\\windows\\mui\\", "lpremove",
            "Automatically removes unused language packs — may unintentionally delete language packs."),
        // Subscription / license
        ("\\microsoft\\windows\\subscription\\", "enablelicenseacquisition",
            "Attempts to reload Windows activation licenses in the background."),
        ("\\microsoft\\windows\\subscription\\", "licenseacquisition",
            "License reload task — unnecessary on correctly activated systems."),
        // Device Census
        ("\\microsoft\\windows\\device information\\", "device",
            "Sends detailed device inventory data to Microsoft (Device Census)."),
        ("\\microsoft\\windows\\device information\\", "device user",
            "Sends user-related device data to Microsoft."),
        // Feedback
        ("\\microsoft\\windows\\feedback\\siuf\\", "dosiffeedbacktask",
            "Windows Feedback Hub: solicits user feedback in the background."),
        ("\\microsoft\\windows\\feedback\\siuf\\", "dosiffeedbacktasknonujailbreak",
            "Feedback Hub background task for non-Insiders."),
        // Clip
        ("\\microsoft\\windows\\clip\\", "license validation",
            "Microsoft Store license validation — runs periodically in the background."),
    ];

    entries
        .iter()
        .map(|(p, n, r)| ((p.to_string(), n.to_string()), *r))
        .collect()
}

pub fn list() -> Value {
    let catalog = bloat_catalog();

    let script = r#"
$tasks = Get-ScheduledTask -ErrorAction SilentlyContinue | ForEach-Object {
    [PSCustomObject]@{
        Path  = $_.TaskPath
        Name  = $_.TaskName
        State = $_.State.ToString()
    }
}
$tasks | ConvertTo-Json -Compress -Depth 2
"#;

    let raw = match crate::ps::run(script) {
        Ok(s) => s,
        Err(e) => return json!({ "error": e, "tasks": [] }),
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return json!({ "tasks": [] });
    }

    let parsed: Vec<Value> = match serde_json::from_str(trimmed) {
        Ok(Value::Array(v)) => v,
        Ok(single @ Value::Object(_)) => vec![single],
        _ => return json!({ "error": "parse error", "tasks": [] }),
    };

    let tasks: Vec<Value> = parsed
        .into_iter()
        .filter_map(|t| {
            let path = t["Path"].as_str()?.to_string();
            let name = t["Name"].as_str()?.to_string();
            let state = t["State"].as_str().unwrap_or("Unknown").to_string();
            let key = (path.to_lowercase(), name.to_lowercase());
            let reason = catalog.get(&key).copied().unwrap_or("");
            let is_bloat = !reason.is_empty();
            let enabled = matches!(state.as_str(), "Ready" | "Running");
            Some(json!({
                "path":    path,
                "name":    name,
                "state":   state,
                "enabled": enabled,
                "isBloat": is_bloat,
                "reason":  reason,
            }))
        })
        .collect();

    let bloat_count = tasks.iter().filter(|t| t["isBloat"].as_bool().unwrap_or(false)).count();
    json!({ "tasks": tasks, "bloatCount": bloat_count })
}

pub fn toggle(path: String, name: String, enable: bool) -> Result<Value, String> {
    if !crate::ps::is_admin() {
        return Err("Scheduled Tasks ändern benötigt Adminrechte.".into());
    }
    let action = if enable { "Enable-ScheduledTask" } else { "Disable-ScheduledTask" };
    // Escape quotes in path/name
    let safe_path = path.replace('"', "'");
    let safe_name = name.replace('"', "'");
    let script = format!(
        r#"{action} -TaskPath "{safe_path}" -TaskName "{safe_name}" -ErrorAction Stop | Out-Null; "OK""#
    );
    crate::ps::run(&script)
        .map(|_| json!({ "name": name, "path": path, "enabled": enable }))
        .map_err(|e| format!("Toggle fehlgeschlagen: {e}"))
}
