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
        # StartType cannot express delayed start; read the service's flag so
        # the list matches what set_startup compares against.
        startType   = $(
            $t = $_.StartType.ToString()
            if ($t -eq 'Automatic') {
                $d = (Get-ItemProperty -LiteralPath ('HKLM:\SYSTEM\CurrentControlSet\Services\' + $_.Name) -Name DelayedAutostart -ErrorAction SilentlyContinue).DelayedAutostart
                if ($d -eq 1) { $t = 'AutomaticDelayedStart' }
            }
            $t
        )
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

/// Services Windows (or this app's safety story) depends on. The backend
/// refuses to disable or stop them no matter what the renderer sends;
/// Manual/Automatic remain allowed (they never break the system).
const PROTECTED_SERVICES: &[&str] = &[
    "wuauserv",
    "UsoSvc",
    "BITS",
    "WinDefend",
    "WdNisSvc",
    "mpssvc",
    "BFE",
    "WlanSvc",
    "Audiosrv",
    "AudioEndpointBuilder",
    "RpcSs",
    "RpcEptMapper",
    "DcomLaunch",
    "Dnscache",
    "SecurityHealthService",
    "TrustedInstaller",
    "EventLog",
    "Schedule",
    "LSM",
    "Winmgmt",
    "CryptSvc",
    "nsi",
    "Dhcp",
    "BrokerInfrastructure",
    "gpsvc",
    "ProfSvc",
    "CoreMessagingRegistrar",
    "StateRepository",
    "AppXSvc",
];

const DELAYED_AUTO: &str = "AutomaticDelayedStart";

fn is_protected_service(name: &str) -> bool {
    PROTECTED_SERVICES
        .iter()
        .any(|p| p.eq_ignore_ascii_case(name.trim()))
}

/// Service names come from the renderer and are embedded in a single-quoted
/// PS string. Wildcards are never valid service names and would be resolved
/// by wildcard-aware cmdlets downstream.
fn valid_service_name(name: &str) -> bool {
    ps::is_safe_ident(name) && !name.contains(['*', '?', '[', ']'])
}

/// Check a requested startup change against the deny-list (pure, testable).
fn check_startup_request(name: &str, startup_type: &str) -> Result<(), String> {
    if !matches!(
        startup_type,
        "Automatic" | DELAYED_AUTO | "Manual" | "Disabled"
    ) {
        return Err(format!("Invalid startup type: {startup_type}"));
    }
    if !valid_service_name(name) {
        return Err(format!("Invalid service name: {name}"));
    }
    if startup_type == "Disabled" && is_protected_service(name) {
        return Err(format!(
            "{name} is a critical Windows service and cannot be disabled here"
        ));
    }
    Ok(())
}

/// Current startup type. PowerShell 5.1 reports delayed-auto services as
/// plain "Automatic"; the DelayedAutostart flag distinguishes them so the
/// journal records (and undo restores) the exact previous mode.
fn current_startup(name: &str) -> Result<String, String> {
    ps::run(&format!(
        "$s = Get-Service -Name '{name}' -ErrorAction Stop; $t = [string]$s.StartType; \
         if ($t -eq 'Automatic') {{ \
           $d = (Get-ItemProperty -LiteralPath ('HKLM:\\SYSTEM\\CurrentControlSet\\Services\\' + $s.ServiceName) -Name DelayedAutostart -ErrorAction SilentlyContinue).DelayedAutostart; \
           if ($d -eq 1) {{ $t = '{DELAYED_AUTO}' }} \
         }}; $t"
    ))
    .map(|s| s.trim().to_string())
}

pub fn set_startup(name: String, startup_type: String) -> Result<Value, String> {
    check_startup_request(&name, &startup_type)?;
    if !ps::is_admin() {
        return Err("Changing service startup requires administrator rights".into());
    }
    // Capture BEFORE any journal write: a service that cannot be queried
    // leaves no entry behind.
    let previous = current_startup(&name)?;
    if previous == startup_type {
        return Ok(
            json!({ "ok": true, "name": name, "startupType": startup_type, "alreadySet": true }),
        );
    }

    // Items are applied through the shared tweak engine: ServiceStartup uses
    // Set-Service for Automatic/Manual/Disabled and `sc.exe config <name>
    // start= delayed-auto` for AutomaticDelayedStart (Set-Service in
    // Windows PowerShell 5.1 rejects that value). The DelayedAutostart flag
    // is journaled only when it is actually changed (target delayed), with
    // the real new value.
    let mut items = Vec::new();
    if startup_type == "Automatic" && previous == DELAYED_AUTO {
        // Set-Service -StartupType Automatic leaves DelayedAutostart=1, so the
        // service would stay delayed and verification would fail.
        let service_registry = format!(r"SYSTEM\CurrentControlSet\Services\{name}");
        let prev = crate::tweaks::reg_read("HKLM", &service_registry, "DelayedAutostart")?;
        items.push(crate::safety::ChangeItem::Registry {
            root: "HKLM".into(),
            path: service_registry,
            name: "DelayedAutostart".into(),
            prev,
            new: crate::safety::RegVal::Dword(0),
        });
    }
    if startup_type == DELAYED_AUTO {
        let service_registry = format!(r"SYSTEM\CurrentControlSet\Services\{name}");
        let prev = crate::tweaks::reg_read("HKLM", &service_registry, "DelayedAutostart")?;
        items.push(crate::safety::ChangeItem::Registry {
            root: "HKLM".into(),
            path: service_registry,
            name: "DelayedAutostart".into(),
            prev,
            new: crate::safety::RegVal::Dword(1),
        });
    }
    items.push(crate::safety::ChangeItem::ServiceStartup {
        service: name.clone(),
        prev: previous.clone(),
        new: startup_type.clone(),
    });

    let _engine = crate::tweaks::engine_guard()?;
    let entry_id = crate::safety::append_entry(crate::safety::JournalEntry {
        id: format!(
            "service-{name}-{}",
            chrono::Local::now().format("%Y%m%d%H%M%S%6f")
        ),
        tweak_id: format!("service:{name}"),
        tweak_name: format!("Service startup: {name}"),
        origin: None,
        time: chrono::Local::now().to_rfc3339(),
        items: items.clone(),
        reverted: false,
        backup_files: vec![],
        completed: false,
        attempted: 0,
        reverted_items: 0,
        untrusted: false,
    })?;

    let mut failure = None;
    for (idx, item) in items.iter().enumerate() {
        crate::safety::with_journal(|entries| {
            let entry = entries
                .iter_mut()
                .find(|entry| entry.id == entry_id)
                .ok_or("Service journal entry missing")?;
            entry.attempted = idx + 1;
            Ok(())
        })?;
        if let Err(e) = crate::tweaks::apply_item(item) {
            failure = Some((idx, e));
            break;
        }
    }
    if failure.is_none() {
        match current_startup(&name) {
            Ok(actual) if actual == startup_type => {}
            Ok(actual) => {
                failure = Some((
                    items.len() - 1,
                    format!("Service {name} startup remains {actual}"),
                ))
            }
            Err(e) => failure = Some((items.len() - 1, e)),
        }
    }

    if let Some((idx, e)) = failure {
        // Roll back what was attempted; the entry ends up reverted, so no
        // bogus undo is offered for a change that never took effect. A failed
        // startup change that left the service as it was (e.g. access denied
        // by the service's own ACL) is not rolled back: retrying the same
        // refused call would only fail again and strand the entry.
        let startup_unchanged =
            matches!(items[idx], crate::safety::ChangeItem::ServiceStartup { .. })
                && current_startup(&name).is_ok_and(|now| now == previous);
        let attempted = if startup_unchanged {
            &items[..idx]
        } else {
            &items[..=idx]
        };
        return match crate::tweaks::rollback_entry_items(&entry_id, attempted) {
            Ok(()) => Err(format!("{e}; changes rolled back")),
            Err(re) => Err(format!(
                "{e}; rollback incomplete ({re}); restore token: {entry_id}"
            )),
        };
    }

    crate::safety::with_journal(|entries| {
        let entry = entries
            .iter_mut()
            .find(|entry| entry.id == entry_id)
            .ok_or("Service journal entry missing")?;
        entry.completed = true;
        Ok(())
    })?;
    Ok(json!({ "ok": true, "name": name, "startupType": startup_type, "entryId": entry_id }))
}

/// Check a requested control action against the deny-list (pure, testable).
fn check_control_request(name: &str, action: &str) -> Result<(), String> {
    if !matches!(action, "start" | "stop" | "restart") {
        return Err(format!("Unknown action: {action}"));
    }
    if !valid_service_name(name) {
        return Err(format!("Invalid service name: {name}"));
    }
    if action == "stop" && is_protected_service(name) {
        return Err(format!(
            "{name} is a critical Windows service and cannot be stopped here"
        ));
    }
    Ok(())
}

pub fn control(name: String, action: String) -> Result<Value, String> {
    check_control_request(&name, &action)?;
    if !ps::is_admin() {
        return Err("Controlling services requires administrator rights".into());
    }
    let cmd = match action.as_str() {
        "start" => format!("Start-Service -Name '{name}' -ErrorAction Stop"),
        "stop" => format!("Stop-Service -Name '{name}' -Force -ErrorAction Stop"),
        _ => format!("Restart-Service -Name '{name}' -Force -ErrorAction Stop"),
    };
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
    use super::*;

    #[test]
    fn critical_windows_services_are_not_bulk_candidates() {
        for name in ["wuauserv", "WSearch", "SysMain", "PcaSvc", "stisvc"] {
            assert!(!BLOAT.iter().any(|entry| entry.name == name), "{name}");
        }
    }

    #[test]
    fn protected_services_cannot_be_disabled_or_stopped() {
        for name in [
            "wuauserv",
            "WINDEFEND",
            "rpcss",
            "Audiosrv",
            "AppXSvc",
            "BFE",
        ] {
            assert!(check_startup_request(name, "Disabled").is_err(), "{name}");
            assert!(check_control_request(name, "stop").is_err(), "{name}");
            // Manual/Automatic and start/restart stay allowed.
            assert!(check_startup_request(name, "Manual").is_ok(), "{name}");
            assert!(check_startup_request(name, "Automatic").is_ok(), "{name}");
            assert!(check_control_request(name, "start").is_ok(), "{name}");
        }
    }

    #[test]
    fn bloat_catalog_does_not_overlap_deny_list() {
        for b in BLOAT {
            assert!(!is_protected_service(b.name), "{}", b.name);
            assert!(check_startup_request(b.name, "Disabled").is_ok());
        }
    }

    #[test]
    fn rejects_invalid_names_and_types() {
        assert!(check_startup_request("Fax", "Boot").is_err());
        assert!(check_startup_request("Fa*", "Manual").is_err());
        assert!(check_startup_request("a'b", "Manual").is_err());
        assert!(check_startup_request("Fax", DELAYED_AUTO).is_ok());
        assert!(check_control_request("Fax", "pause").is_err());
        assert!(check_control_request("Fax?", "stop").is_err());
    }
}
