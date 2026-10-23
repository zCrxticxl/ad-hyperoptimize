//! Windows Services Manager, list, start/stop, change startup type.
//! Includes a bloat catalog with known safe-to-disable services.

use crate::ps;
use serde_json::{json, Value};

struct BloatEntry {
    name: &'static str,
    reason: &'static str,
}

static BLOAT: &[BloatEntry] = &[
    BloatEntry {
        name: "DiagTrack",
        reason: "Microsoft Telemetry (Connected User Experiences)",
    },
    BloatEntry {
        name: "dmwappushservice",
        reason: "WAP Push - Telemetry Routing",
    },
    BloatEntry {
        name: "MapsBroker",
        reason: "Offline Maps - automatic updates",
    },
    BloatEntry {
        name: "Fax",
        reason: "Legacy Fax Service",
    },
    BloatEntry {
        name: "RemoteRegistry",
        reason: "Remote Registry Access - security risk",
    },
    BloatEntry {
        name: "RetailDemo",
        reason: "Retail Demo Mode - not needed on a normal PC",
    },
    BloatEntry {
        name: "wisvc",
        reason: "Windows Insider Service",
    },
];

pub fn list() -> Value {
    let script = r#"
$descMap = @{}
$pathMap = @{}
try {
    Get-CimInstance Win32_Service -ErrorAction SilentlyContinue | ForEach-Object {
        if ($_.Description) { $descMap[$_.Name] = $_.Description }
        if ($_.PathName)    { $pathMap[$_.Name] = $_.PathName }
    }
} catch {}
Get-Service -ErrorAction Stop | ForEach-Object {
    [PSCustomObject]@{
        name        = $_.Name
        displayName = $_.DisplayName
        status      = $_.Status.ToString()
        startType   = $_.StartType.ToString()
        description = if ($descMap[$_.Name]) { $descMap[$_.Name] } else { '' }
        path        = if ($pathMap[$_.Name]) { $pathMap[$_.Name] } else { '' }
    }
} | ConvertTo-Json -Compress -Depth 2
"#;
    let raw = match ps::run(script) {
        Ok(raw) => raw,
        Err(error) => return json!({ "services": [], "bloatCount": 0, "error": error }),
    };
    let services: Vec<Value> = match serde_json::from_str(raw.trim()) {
        Ok(Value::Array(v)) => v,
        Ok(o @ Value::Object(_)) => vec![o],
        _ => {
            return json!({ "services": [], "bloatCount": 0, "error": "Service list could not be parsed" })
        }
    };

    // Build bloat lookup
    let bloat_map: std::collections::HashMap<&str, &str> =
        BLOAT.iter().map(|b| (b.name, b.reason)).collect();

    let annotated: Vec<Value> = services
        .into_iter()
        .map(|mut s| {
            let name = s["name"].as_str().unwrap_or("").to_string();
            if let Some(reason) = bloat_map.get(name.as_str()) {
                s["isBloat"] = json!(true);
                s["bloatReason"] = json!(reason);
            } else {
                s["isBloat"] = json!(false);
            }
            // Origin: executables outside the Windows directory belong to
            // installed software. No path (rare) counts as inbox.
            let raw_path = s["path"]
                .as_str()
                .unwrap_or("")
                .trim_matches('"')
                .to_lowercase();
            let normalized = raw_path.replace("%systemroot%", "c:\\windows");
            s["origin"] = json!(
                if normalized.starts_with("c:\\windows\\") || normalized.is_empty() {
                    "windows"
                } else {
                    "thirdparty"
                }
            );
            s
        })
        .collect();

    let bloat_count = annotated
        .iter()
        .filter(|s| s["isBloat"].as_bool().unwrap_or(false))
        .count();
    json!({ "services": annotated, "bloatCount": bloat_count })
}

pub fn set_startup(name: String, startup_type: String) -> Result<Value, String> {
    let valid = ["Automatic", "AutomaticDelayedStart", "Manual", "Disabled"];
    if !valid.contains(&startup_type.as_str()) {
        return Err(format!("Invalid startup type: {startup_type}"));
    }
    // Service names come from the renderer and are embedded in a single-quoted
    // PS string. Wildcards are never valid service names and would be
    // resolved by wildcard-aware cmdlets downstream.
    if !ps::is_safe_ident(&name) || name.contains(['*', '?', '[', ']']) {
        return Err(format!("Invalid service name: {name}"));
    }
    if !ps::is_admin() {
        return Err("Changing service startup requires administrator rights".into());
    }
    let previous = ps::run(&format!(
        "(Get-Service -Name '{name}' -ErrorAction Stop).StartType.ToString()"
    ))?;
    let previous = previous.trim();
    if previous == startup_type {
        return Ok(
            json!({ "ok": true, "name": name, "startupType": startup_type, "alreadySet": true }),
        );
    }
    let service_registry = format!(r"SYSTEM\CurrentControlSet\Services\{name}");
    let delayed_before = crate::tweaks::reg_read("HKLM", &service_registry, "DelayedAutoStart")?;
    let entry_id = crate::safety::append_entry(crate::safety::JournalEntry {
        id: format!(
            "service-{name}-{}",
            chrono::Local::now().format("%Y%m%d%H%M%S%6f")
        ),
        tweak_id: format!("service:{name}"),
        tweak_name: format!("Service startup: {name}"),
        origin: None,
        time: chrono::Local::now().to_rfc3339(),
        items: vec![
            crate::safety::ChangeItem::Registry {
                root: "HKLM".into(),
                path: service_registry,
                name: "DelayedAutoStart".into(),
                prev: delayed_before,
                new: crate::safety::RegVal::Dword(0),
            },
            crate::safety::ChangeItem::ServiceStartup {
                service: name.clone(),
                prev: previous.into(),
                new: startup_type.clone(),
            },
        ],
        reverted: false,
        backup_files: vec![],
        completed: false,
        attempted: 2,
        reverted_items: 0,
    })?;
    let script =
        format!("Set-Service -Name '{name}' -StartupType {startup_type} -ErrorAction Stop");
    let result = ps::run(&script).and_then(|_| {
        let actual = ps::run(&format!(
            "(Get-Service -Name '{name}' -ErrorAction Stop).StartType.ToString()"
        ))?;
        if actual.trim() == startup_type {
            Ok(())
        } else {
            Err(format!("Service {name} startup remains {}", actual.trim()))
        }
    });
    crate::safety::with_journal(|entries| {
        let entry = entries
            .iter_mut()
            .find(|entry| entry.id == entry_id)
            .ok_or("Service journal entry missing")?;
        entry.completed = result.is_ok();
        Ok(())
    })?;
    result?;
    Ok(json!({ "ok": true, "name": name, "startupType": startup_type, "entryId": entry_id }))
}

pub fn control(name: String, action: String) -> Result<Value, String> {
    let cmd = match action.as_str() {
        "start" => format!("Start-Service -Name '{name}' -ErrorAction Stop"),
        "stop" => format!("Stop-Service -Name '{name}' -Force -ErrorAction Stop"),
        "restart" => format!("Restart-Service -Name '{name}' -Force -ErrorAction Stop"),
        _ => return Err(format!("Unknown action: {action}")),
    };
    if !ps::is_safe_ident(&name) || name.contains(['*', '?', '[', ']']) {
        return Err(format!("Invalid service name: {name}"));
    }
    ps::run(&cmd)?;
    // Return updated status
    let status_script =
        format!("(Get-Service -Name '{name}' -ErrorAction SilentlyContinue).Status.ToString()");
    let status = ps::run(&status_script)
        .unwrap_or_default()
        .trim()
        .to_string();
    Ok(json!({ "ok": true, "name": name, "action": action, "status": status }))
}

#[cfg(test)]
mod tests {
    use super::BLOAT;

    #[test]
    fn critical_windows_services_are_not_bulk_candidates() {
        for name in ["wuauserv", "WSearch", "SysMain", "PcaSvc", "stisvc"] {
            assert!(!BLOAT.iter().any(|entry| entry.name == name), "{name}");
        }
    }
}
