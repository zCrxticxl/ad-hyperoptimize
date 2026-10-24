//! Leftover scanner: settings other "optimizers" leave behind that are known
//! to hurt latency, stability or security, compared with the Windows default.
//! Each finding can be reset to the default; every reset is journaled and can
//! be undone from the change history. Values this app itself set and still
//! tracks in an open journal entry are reported as such and left alone.

use crate::safety::{ChangeItem, RegVal};
use serde_json::{json, Value};

/// How bad a leftover is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Lowers protection against attacks.
    Security,
    /// Known to hurt latency, stability or load times.
    Harmful,
    /// Not the default; no proven benefit, little harm.
    NotDefault,
}

impl Severity {
    fn as_str(self) -> &'static str {
        match self {
            Severity::Security => "security",
            Severity::Harmful => "harmful",
            Severity::NotDefault => "notDefault",
        }
    }
}

/// A registry leftover: `bad` decides from the current value whether it is a
/// leftover; `default` is what the reset writes (None = delete the value).
pub struct RegRule {
    pub id: &'static str,
    pub path: &'static str,
    pub name: &'static str,
    pub severity: Severity,
    pub bad: fn(&RegVal) -> bool,
    pub default: Option<u32>,
}

const MM: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management";
const MMCSS: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile";
const GAMES: &str =
    r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games";

fn dword(v: &RegVal) -> Option<u32> {
    match v {
        RegVal::Dword(d) => Some(*d),
        _ => None,
    }
}

pub static REG_RULES: &[RegRule] = &[
    // FeatureSettingsOverride 3 (with mask 3) turns off Spectre/Meltdown
    // mitigations system-wide (Microsoft's documented "disable" value).
    // Removing the override restores them; the mask alone is harmless
    // (mask 3 with override 0 is the documented "enable" setting).
    RegRule {
        id: "mitigations_off",
        path: MM,
        name: "FeatureSettingsOverride",
        severity: Severity::Security,
        bad: |v| dword(v).is_some_and(|d| d & 3 == 3),
        default: None,
    },
    RegRule {
        id: "large_system_cache",
        path: MM,
        name: "LargeSystemCache",
        severity: Severity::Harmful,
        bad: |v| dword(v).is_some_and(|d| d != 0),
        default: Some(0),
    },
    RegRule {
        id: "paging_executive",
        path: MM,
        name: "DisablePagingExecutive",
        severity: Severity::NotDefault,
        bad: |v| dword(v).is_some_and(|d| d != 0),
        default: Some(0),
    },
    RegRule {
        id: "prefetcher_off",
        path: r"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters",
        name: "EnablePrefetcher",
        severity: Severity::Harmful,
        bad: |v| dword(v) == Some(0),
        default: Some(3),
    },
    RegRule {
        id: "priority_separation",
        path: r"SYSTEM\CurrentControlSet\Control\PriorityControl",
        name: "Win32PrioritySeparation",
        severity: Severity::NotDefault,
        bad: |v| dword(v).is_some_and(|d| d != 2 && d != 0x26),
        default: Some(2),
    },
    RegRule {
        id: "irq8_priority",
        path: r"SYSTEM\CurrentControlSet\Control\PriorityControl",
        name: "IRQ8Priority",
        severity: Severity::NotDefault,
        bad: |_| true,
        default: None,
    },
    RegRule {
        id: "irq16_priority",
        path: r"SYSTEM\CurrentControlSet\Control\PriorityControl",
        name: "IRQ16Priority",
        severity: Severity::NotDefault,
        bad: |_| true,
        default: None,
    },
    RegRule {
        id: "mmcss_responsiveness",
        path: MMCSS,
        name: "SystemResponsiveness",
        severity: Severity::NotDefault,
        bad: |v| dword(v).is_some_and(|d| d != 20),
        default: Some(20),
    },
    RegRule {
        id: "mmcss_throttling",
        path: MMCSS,
        name: "NetworkThrottlingIndex",
        severity: Severity::NotDefault,
        bad: |v| dword(v).is_some_and(|d| d != 10),
        default: Some(10),
    },
    RegRule {
        id: "mmcss_nolazy",
        path: MMCSS,
        name: "NoLazyMode",
        severity: Severity::NotDefault,
        bad: |_| true,
        default: None,
    },
    RegRule {
        id: "mmcss_alwayson",
        path: MMCSS,
        name: "AlwaysOn",
        severity: Severity::NotDefault,
        bad: |_| true,
        default: None,
    },
    RegRule {
        id: "games_gpu_priority",
        path: GAMES,
        name: "GPU Priority",
        severity: Severity::NotDefault,
        bad: |v| dword(v).is_some_and(|d| d != 8),
        default: Some(8),
    },
    RegRule {
        id: "games_priority",
        path: GAMES,
        name: "Priority",
        severity: Severity::NotDefault,
        bad: |v| dword(v).is_some_and(|d| d != 2),
        default: Some(2),
    },
    // TdrLevel 0 turns off GPU timeout recovery: a driver hang freezes the
    // whole PC instead of resetting the driver.
    RegRule {
        id: "tdr_off",
        path: r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers",
        name: "TdrLevel",
        severity: Severity::Harmful,
        bad: |v| dword(v) == Some(0),
        default: None,
    },
    RegRule {
        id: "timer_global",
        path: r"SYSTEM\CurrentControlSet\Control\Session Manager\kernel",
        name: "GlobalTimerResolutionRequests",
        severity: Severity::NotDefault,
        bad: |v| dword(v) == Some(1),
        default: None,
    },
    RegRule {
        id: "svchost_split",
        path: r"SYSTEM\CurrentControlSet\Control",
        name: "SvcHostSplitThresholdInKB",
        severity: Severity::NotDefault,
        bad: |v| dword(v).is_some_and(|d| d != 0x380000),
        default: Some(0x380000),
    },
];

/// BCD elements on {current} whose mere presence is a leftover (the default
/// is "not set"): forced HPET, forced platform tick, disabled dynamic tick,
/// legacy TSC sync policy.
pub const BCD_RULES: &[(&str, &str, Severity)] = &[
    (
        "bcd_useplatformclock",
        "useplatformclock",
        Severity::Harmful,
    ),
    ("bcd_useplatformtick", "useplatformtick", Severity::Harmful),
    (
        "bcd_disabledynamictick",
        "disabledynamictick",
        Severity::NotDefault,
    ),
    ("bcd_tscsyncpolicy", "tscsyncpolicy", Severity::Harmful),
];

/// Core services and their Windows default start type. Disabled is a
/// leftover; some lower security (Windows Update, firewall, UAC prompts).
pub const SERVICE_RULES: &[(&str, &str, Severity)] = &[
    ("Audiosrv", "Automatic", Severity::Harmful),
    ("AudioEndpointBuilder", "Automatic", Severity::Harmful),
    ("Dhcp", "Automatic", Severity::Harmful),
    ("Dnscache", "Automatic", Severity::Harmful),
    ("nsi", "Automatic", Severity::Harmful),
    ("NlaSvc", "Automatic", Severity::Harmful),
    ("BFE", "Automatic", Severity::Security),
    ("mpssvc", "Automatic", Severity::Security),
    ("wuauserv", "Manual", Severity::Security),
    ("BITS", "Manual", Severity::Security),
    ("CryptSvc", "Automatic", Severity::Security),
    ("Appinfo", "Manual", Severity::Security),
    ("EventLog", "Automatic", Severity::Harmful),
    ("Schedule", "Automatic", Severity::Harmful),
    ("Winmgmt", "Automatic", Severity::Harmful),
    ("Power", "Automatic", Severity::Harmful),
    ("Themes", "Automatic", Severity::NotDefault),
    ("FontCache", "Automatic", Severity::NotDefault),
    ("StateRepository", "Manual", Severity::Harmful),
    ("TokenBroker", "Manual", Severity::Harmful),
];

/// Timer and memory tools that keep running in the background.
pub const TOOL_PROCESSES: &[&str] = &[
    "TimerResolution",
    "SetTimerResolutionService",
    "Intelligent standby list cleaner ISLC",
    "MemReduct",
];

/// True when an open journal entry of this app holds a Registry item for
/// that value (the app's own change: undo it there, not here).
fn owned_by_app(journal: &[crate::safety::JournalEntry], path: &str, name: &str) -> bool {
    journal.iter().filter(|e| !e.reverted).any(|e| {
        e.items.iter().any(|i| {
            matches!(i, ChangeItem::Registry { root, path: p, name: n, .. }
            if root == "HKLM" && p.eq_ignore_ascii_case(path) && n.eq_ignore_ascii_case(name))
        })
    })
}

fn open_reset(journal: &[crate::safety::JournalEntry], id: &str) -> Option<String> {
    let tid = format!("leftover:{id}");
    journal
        .iter()
        .rev()
        .find(|e| e.tweak_id == tid && !e.reverted && e.completed)
        .map(|e| e.id.clone())
}

fn reg_display(v: &RegVal) -> String {
    match v {
        RegVal::Dword(d) => format!("{d} (0x{d:X})"),
        RegVal::Str(s) => s.clone(),
        RegVal::Raw { ty, hex } => format!("{ty} {hex}"),
    }
}

#[cfg(windows)]
pub fn scan() -> Value {
    let journal = crate::safety::load_journal().unwrap_or_default();
    let mut found = Vec::new();
    for r in REG_RULES {
        if let Ok(Some(v)) = crate::tweaks::reg_read("HKLM", r.path, r.name) {
            if (r.bad)(&v) {
                found.push(json!({
                    "id": r.id, "kind": "registry", "severity": r.severity.as_str(),
                    "target": format!(r"HKLM\{}\{}", r.path, r.name),
                    "current": reg_display(&v),
                    "default": r.default.map(|d| d.to_string()).unwrap_or_else(|| "–".into()),
                    "ownedByApp": owned_by_app(&journal, r.path, r.name),
                }));
            }
        }
    }
    if let Ok(elems) = crate::bootopt::read_elements("{current}") {
        for (id, el, sev) in BCD_RULES {
            if let Some(v) = elems.get(*el) {
                found.push(json!({
                    "id": id, "kind": "bcd", "severity": sev.as_str(),
                    "target": format!("bcdedit {{current}} {el}"), "current": v, "default": "–",
                    "ownedByApp": false,
                }));
            }
        }
    }
    for (svc, def, sev) in SERVICE_RULES {
        if let Ok(start) = crate::tweaks::service_get_start(svc) {
            if start == "Disabled" {
                found.push(json!({
                    "id": format!("svc_{svc}"), "kind": "service", "severity": sev.as_str(),
                    "target": svc, "current": "Disabled", "default": def,
                    "ownedByApp": journal.iter().filter(|e| !e.reverted).any(|e| e.items.iter().any(|i|
                        matches!(i, ChangeItem::ServiceStartup { service, .. } if service.eq_ignore_ascii_case(svc)))),
                }));
            }
        }
    }
    let mut sys = sysinfo::System::new();
    sys.refresh_processes();
    let tools: Vec<String> = sys
        .processes()
        .values()
        .map(|p| p.name().to_string())
        .filter(|n| {
            let base = n.strip_suffix(".exe").unwrap_or(n);
            TOOL_PROCESSES.iter().any(|t| t.eq_ignore_ascii_case(base))
        })
        .collect();
    for f in found.iter_mut() {
        let id = f["id"].as_str().unwrap_or_default().to_string();
        f["resetEntry"] = json!(open_reset(&journal, &id));
    }
    // Resets that are still open but whose value is now at the default are
    // listed too, so they can be undone from here.
    let mut resets: Vec<Value> = journal
        .iter()
        .filter(|e| e.tweak_id.starts_with("leftover:") && e.completed && !e.reverted)
        .map(|e| json!({ "id": e.tweak_id.trim_start_matches("leftover:"), "entryId": e.id, "name": e.tweak_name }))
        .collect();
    resets.retain(|r| !found.iter().any(|f| f["id"] == r["id"]));
    json!({ "findings": found, "tools": tools, "resets": resets })
}

#[cfg(not(windows))]
pub fn scan() -> Value {
    json!({ "findings": [], "tools": [], "resets": [], "error": "Windows only" })
}

/// Resets one finding to the Windows default, journaled.
#[cfg(windows)]
pub fn reset(id: String) -> Result<Value, String> {
    let journal = crate::safety::load_journal().unwrap_or_default();
    let tid = format!("leftover:{id}");
    if open_reset(&journal, &id).is_some() {
        return Err("Already reset; undo it first to reset again.".into());
    }
    let (name, items): (String, Vec<ChangeItem>) = if let Some(r) =
        REG_RULES.iter().find(|r| r.id == id)
    {
        let prev = crate::tweaks::reg_read("HKLM", r.path, r.name)?;
        if !prev.as_ref().is_some_and(|v| (r.bad)(v)) {
            return Err("Nothing to reset: the value is already at the default.".into());
        }
        if owned_by_app(&journal, r.path, r.name) {
            return Err("This value was set by this app; undo that change instead.".into());
        }
        crate::safety::backup_registry_key("HKLM", r.path)?;
        let item = match r.default {
            Some(d) => ChangeItem::Registry {
                root: "HKLM".into(),
                path: r.path.into(),
                name: r.name.into(),
                prev,
                new: RegVal::Dword(d),
            },
            // Delete = revert of an item whose prev is None; journal it as a
            // command so undo writes the old value back.
            None => {
                let back = match &prev {
                    Some(RegVal::Dword(d)) => format!(
                        "New-Item -Path 'HKLM:\\{}' -Force | Out-Null; New-ItemProperty -Path 'HKLM:\\{}' -Name '{}' -PropertyType DWord -Value {d} -Force | Out-Null",
                        r.path, r.path, r.name
                    ),
                    _ => return Err("Unexpected value type; not reset.".into()),
                };
                ChangeItem::Command {
                    applied: crate::perftweaks::ps_line(&format!(
                        "Remove-ItemProperty -Path 'HKLM:\\{}' -Name '{}' -ErrorAction Stop",
                        r.path, r.name
                    )),
                    revert: crate::perftweaks::ps_line(&back),
                }
            }
        };
        (format!("Leftover reset: {}", r.name), vec![item])
    } else if let Some((_, el, _)) = BCD_RULES.iter().find(|(i, _, _)| *i == id) {
        let elems = crate::bootopt::read_elements("{current}")?;
        let Some(cur) = elems.get(*el) else {
            return Err("Nothing to reset: not set.".into());
        };
        let cur = crate::bootopt::normalize_bcd_value(cur).ok_or("unexpected BCD value")?;
        (
            format!("Leftover reset: BCD {el}"),
            vec![ChangeItem::Command {
                applied: format!("bcdedit /deletevalue {{current}} {el}"),
                revert: format!("bcdedit /set {{current}} {el} {cur}"),
            }],
        )
    } else if let Some((svc, def, _)) = SERVICE_RULES
        .iter()
        .find(|(s, _, _)| format!("svc_{s}") == id)
    {
        let prev = crate::tweaks::service_get_start(svc)?;
        if prev != "Disabled" {
            return Err("Nothing to reset: the service is not disabled.".into());
        }
        (
            format!("Leftover reset: service {svc}"),
            vec![ChangeItem::ServiceStartup {
                service: (*svc).into(),
                prev,
                new: (*def).into(),
            }],
        )
    } else {
        return Err("Unknown finding".into());
    };
    let _engine = crate::tweaks::engine_guard()?;
    let entry = crate::perftweaks::apply_journaled(&tid, &name, items)?;
    Ok(json!({ "ok": true, "entryId": entry }))
}

#[cfg(not(windows))]
pub fn reset(_id: String) -> Result<Value, String> {
    Err("Windows only".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: &str) -> &'static RegRule {
        REG_RULES.iter().find(|r| r.id == id).unwrap()
    }

    #[test]
    fn rules_flag_only_leftover_values() {
        assert!((rule("mitigations_off").bad)(&RegVal::Dword(3)));
        assert!(!(rule("mitigations_off").bad)(&RegVal::Dword(0)));
        assert!((rule("large_system_cache").bad)(&RegVal::Dword(1)));
        assert!(!(rule("priority_separation").bad)(&RegVal::Dword(2)));
        assert!(!(rule("priority_separation").bad)(&RegVal::Dword(0x26)));
        assert!((rule("priority_separation").bad)(&RegVal::Dword(0x28)));
        assert!((rule("prefetcher_off").bad)(&RegVal::Dword(0)));
        assert!(!(rule("prefetcher_off").bad)(&RegVal::Dword(3)));
        assert!((rule("tdr_off").bad)(&RegVal::Dword(0)));
        assert!(!(rule("mmcss_responsiveness").bad)(&RegVal::Dword(20)));
    }

    #[test]
    fn ids_are_unique_and_services_have_defaults() {
        let mut ids: Vec<String> = REG_RULES.iter().map(|r| r.id.to_string()).collect();
        ids.extend(BCD_RULES.iter().map(|b| b.0.to_string()));
        ids.extend(SERVICE_RULES.iter().map(|s| format!("svc_{}", s.0)));
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
        for (_, def, _) in SERVICE_RULES {
            assert!(matches!(*def, "Automatic" | "Manual"));
        }
    }

    #[test]
    fn app_owned_values_are_detected() {
        let e = crate::safety::JournalEntry {
            id: "x".into(),
            tweak_id: "catalog".into(),
            tweak_name: String::new(),
            origin: None,
            time: String::new(),
            items: vec![ChangeItem::Registry {
                root: "HKLM".into(),
                path: MM.into(),
                name: "LargeSystemCache".into(),
                prev: None,
                new: RegVal::Dword(1),
            }],
            reverted: false,
            backup_files: vec![],
            completed: true,
            attempted: 1,
            reverted_items: 0,
            untrusted: false,
        };
        assert!(owned_by_app(&[e], MM, "largesystemcache"));
        assert!(!owned_by_app(&[], MM, "LargeSystemCache"));
    }
}
