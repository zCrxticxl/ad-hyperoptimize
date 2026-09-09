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
            "Updates the appcompat telemetry database, pure data collection for Microsoft."),
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
            "Profiles disk I/O patterns in the background, generates load with no user benefit."),
        // WinSAT
        ("\\microsoft\\windows\\maintenance\\", "winsat",
            "Windows System Assessment Tool, runs benchmarks in the background and generates CPU/disk load."),
        // Maps
        ("\\microsoft\\windows\\maps\\", "mapsupdatetask",
            "Automatically downloads offline map updates for the Windows Maps app."),
        ("\\microsoft\\windows\\maps\\", "mapstoasttask",
            "Sends notifications from the Windows Maps app."),
        // NetTrace
        ("\\microsoft\\windows\\nettrace\\", "gathernetworkinfo",
            "Collects detailed network diagnostic data, background overhead with no discernible benefit."),
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
            "Automatically updates Store apps in the background, separate from normal Windows Update."),
        ("\\microsoft\\windows\\windowsupdate\\", "scheduled start",
            "Starts Windows Update scans on a fixed schedule. Effect is limited on Win11: the Update Orchestrator and trigger-start of wuauserv run scans independently of this task."),
        // Workplace Join
        ("\\microsoft\\windows\\workplace join\\", "automatic-device-join",
            "Automatically registers the device in Azure AD / Workplace Join (enterprise feature)."),
        // PushToInstall
        ("\\microsoft\\windows\\pushtoinstall\\", "logincheck",
            "Checks Push-to-Install Store tasks at login, irrelevant for non-enterprise."),
        // MUI language packs
        ("\\microsoft\\windows\\mui\\", "lpremove",
            "Automatically removes unused language packs, may unintentionally delete language packs."),
        // Subscription / license
        ("\\microsoft\\windows\\subscription\\", "enablelicenseacquisition",
            "Attempts to reload Windows activation licenses in the background."),
        ("\\microsoft\\windows\\subscription\\", "licenseacquisition",
            "License reload task, unnecessary on correctly activated systems."),
        // Device Census
        ("\\microsoft\\windows\\device information\\", "device",
            "Sends detailed device inventory data to Microsoft (Device Census)."),
        ("\\microsoft\\windows\\device information\\", "device user",
            "Sends user-related device data to Microsoft."),
        // Siuf tasks were removed from Windows 11 25H2; still present on Windows 10 and 24H2.
        ("\\microsoft\\windows\\feedback\\siuf\\", "dmclient",
            "Feedback/telemetry client task (Siuf): uploads user feedback diagnostics in the background."),
        ("\\microsoft\\windows\\feedback\\siuf\\", "dmclientonscenariodownload",
            "Feedback/telemetry client task (Siuf, scenario download): downloads feedback-scenario payloads."),
        // Clip
        ("\\microsoft\\windows\\clip\\", "license validation",
            "Microsoft Store license validation, runs periodically in the background."),
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
    $created = $null
    $ds = "$($_.Date)"
    if ($ds) { try { $created = [datetime]::Parse($ds).ToUniversalTime().ToString("o") } catch { } }
    $lastRun = $null
    $nextRun = $null
    try {
        $info = $_ | Get-ScheduledTaskInfo -ErrorAction Stop
        if ($info.LastRunTime -and $info.LastRunTime.Year -gt 1601) { $lastRun = $info.LastRunTime.ToUniversalTime().ToString("o") }
        if ($info.NextRunTime -and $info.NextRunTime.Year -gt 1601) { $nextRun = $info.NextRunTime.ToUniversalTime().ToString("o") }
    } catch { }
    $exec = ""
    try { $exec = "$($_.Actions[0].Execute)" } catch { }
    [PSCustomObject]@{
        Path    = $_.TaskPath
        Name    = $_.TaskName
        State   = $_.State.ToString()
        Author  = "$($_.Author)"
        Created = $created
        LastRun = $lastRun
        NextRun = $nextRun
        Exec    = $exec
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
            // All inbox Windows tasks live under \Microsoft\; anything else
            // (vendor folders or root-level entries) is third-party installed.
            let origin = if path.to_lowercase().starts_with("\\microsoft\\") {
                "windows"
            } else {
                "thirdparty"
            };
            Some(json!({
                "path":    path,
                "name":    name,
                "state":   state,
                "enabled": enabled,
                "isBloat": is_bloat,
                "reason":  reason,
                "origin":  origin,
                "author":  t["Author"].as_str().unwrap_or(""),
                "created": t["Created"].as_str(),
                "lastRun": t["LastRun"].as_str(),
                "nextRun": t["NextRun"].as_str(),
                "exec":    t["Exec"].as_str().unwrap_or(""),
            }))
        })
        .collect();

    let bloat_count = tasks
        .iter()
        .filter(|t| t["isBloat"].as_bool().unwrap_or(false))
        .count();
    json!({ "tasks": tasks, "bloatCount": bloat_count })
}

pub fn toggle(path: String, name: String, enable: bool) -> Result<Value, String> {
    if !crate::ps::is_admin() {
        return Err("Scheduled Tasks ändern benötigt Adminrechte.".into());
    }
    // Path/name are embedded in single-quoted PS strings and come from the
    // renderer, reject anything that could terminate the quotes.
    // Task paths/names never contain wildcards; blocking them here keeps
    // wildcard-expanding cmdlets (Test-Path, schtasks quoting) safe.
    if !crate::ps::is_safe_ident(&path)
        || !crate::ps::is_safe_ident(&name)
        || path.contains(['*', '?', '[', ']'])
        || name.contains(['*', '?', '[', ']'])
    {
        return Err("Invalid task path or name".into());
    }
    let action = if enable {
        "Enable-ScheduledTask"
    } else {
        "Disable-ScheduledTask"
    };
    let script = format!(
        r#"{action} -TaskPath '{path}' -TaskName '{name}' -ErrorAction Stop | Out-Null; "OK""#
    );
    crate::ps::run(&script)
        .map(|_| json!({ "name": name, "path": path, "enabled": enable }))
        .map_err(|e| format!("Toggle fehlgeschlagen: {e}"))
}
