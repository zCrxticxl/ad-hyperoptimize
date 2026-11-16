//! Session-oriented, drift-checked recovery of typed journal values.
//! Command-only records stay visible but are never executed by this path.
use crate::safety::{self, ChangeItem, JournalEntry, RegVal};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cell::RefCell;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub label: String,
}

thread_local! {
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
}

pub struct SessionScope(Option<Session>);
impl SessionScope {
    pub fn new(label: &str) -> Self {
        Self(SESSION.with(|s| s.replace(Some(new_session(label)))))
    }
}
impl Drop for SessionScope {
    fn drop(&mut self) {
        SESSION.with(|s| s.replace(self.0.take()));
    }
}
fn new_session(label: &str) -> Session {
    Session {
        id: format!("session-{:032x}", rand::random::<u128>()),
        label: label.to_string(),
    }
}
pub fn current_session(label: &str) -> Session {
    SESSION.with(|s| s.borrow().clone().unwrap_or_else(|| new_session(label)))
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value")]
enum Setting {
    Registry(Option<RegVal>),
    Service(String),
}

fn values(item: &ChangeItem) -> Option<(Setting, Setting)> {
    match item {
        ChangeItem::Registry { prev, new, .. } => Some((
            Setting::Registry(prev.clone()),
            Setting::Registry(Some(new.clone())),
        )),
        ChangeItem::ServiceStartup { prev, new, .. } => Some((
            Setting::Service(prev.clone()),
            Setting::Service(new.clone()),
        )),
        _ => None,
    }
}
fn read(item: &ChangeItem) -> Result<Setting, String> {
    match item {
        #[cfg(windows)]
        ChangeItem::Registry {
            root, path, name, ..
        } => crate::tweaks::reg_read(root, path, name).map(Setting::Registry),
        #[cfg(windows)]
        ChangeItem::ServiceStartup { service, .. } => {
            let path = format!(r"SYSTEM\CurrentControlSet\Services\{service}");
            let mode = match crate::tweaks::reg_read("HKLM", &path, "Start")? {
                None => "__ABSENT__",
                Some(RegVal::Dword(2)) => {
                    match crate::tweaks::reg_read("HKLM", &path, "DelayedAutostart")? {
                        Some(RegVal::Dword(1)) => "AutomaticDelayedStart",
                        Some(RegVal::Dword(0)) | None => "Automatic",
                        _ => return Err("Unknown delayed-start value".into()),
                    }
                }
                Some(RegVal::Dword(3)) => "Manual",
                Some(RegVal::Dword(4)) => "Disabled",
                _ => return Err("Unsupported service startup value".into()),
            };
            Ok(Setting::Service(mode.into()))
        }
        _ => Err("No typed state reader for this operation".into()),
    }
}
fn resource(item: &ChangeItem) -> Option<String> {
    match item {
        ChangeItem::Registry {
            root, path, name, ..
        } => Some(format!("reg:{root}\\{path}\\{name}").to_ascii_lowercase()),
        ChangeItem::ServiceStartup { service, .. } => {
            Some(format!("service:{service}").to_ascii_lowercase())
        }
        _ => None,
    }
}
fn remaining(entry: &JournalEntry) -> usize {
    let attempted = entry.attempted.min(entry.items.len());
    attempted.saturating_sub(entry.reverted_items)
}
fn later_owner(journal: &[JournalEntry], entry_id: &str, item: &ChangeItem) -> bool {
    journal
        .iter()
        .skip_while(|e| e.id != entry_id)
        .skip(1)
        .any(|e| {
            !e.reverted
                && e.items[..remaining(e)]
                    .iter()
                    .any(|other| overlaps(item, other))
        })
}

fn overlaps(a: &ChangeItem, b: &ChangeItem) -> bool {
    // An opaque later command can own a registry/service resource too. Until
    // it has a typed adapter, an older session must not assume independence.
    let (Some(a_key), Some(b_key)) = (resource(a), resource(b)) else {
        return true;
    };
    if a_key == b_key {
        return true;
    }
    let service_registry = |service: &str, item: &ChangeItem| {
        matches!(item, ChangeItem::Registry { root, path, name, .. }
            if root.eq_ignore_ascii_case("HKLM")
            && path.eq_ignore_ascii_case(&format!(r"SYSTEM\CurrentControlSet\Services\{service}"))
            && (name.eq_ignore_ascii_case("Start") || name.eq_ignore_ascii_case("DelayedAutostart")))
    };
    match (a, b) {
        (ChangeItem::ServiceStartup { service, .. }, item)
        | (item, ChangeItem::ServiceStartup { service, .. }) => service_registry(service, item),
        _ => false,
    }
}

fn paired_delay(items: &[ChangeItem], index: usize) -> bool {
    if index == 0 {
        return false;
    }
    matches!((&items[index], &items[index - 1]),
        (ChangeItem::ServiceStartup { service, .. }, ChangeItem::Registry { root, path, name, .. })
        if root.eq_ignore_ascii_case("HKLM")
            && path.eq_ignore_ascii_case(&format!(r"SYSTEM\CurrentControlSet\Services\{service}"))
            && name.eq_ignore_ascii_case("DelayedAutostart"))
}

fn decision(before: &Setting, target: &Setting, current: &Setting, overlap: bool) -> &'static str {
    if overlap {
        "newerChange"
    } else if current == before {
        "alreadyOriginal"
    } else if current == target {
        "ready"
    } else {
        "conflict"
    }
}

fn item_view(entry: &JournalEntry, index: usize, journal: &[JournalEntry]) -> Value {
    let item = &entry.items[index];
    let pair = values(item);
    let current = pair.as_ref().map(|_| read(item));
    let status = if entry.reverted
        || (index >= remaining(entry) && index < entry.attempted.min(entry.items.len()))
    {
        "restored"
    } else if index >= entry.attempted.min(entry.items.len()) {
        "notAttempted"
    } else if entry.untrusted {
        "untrusted"
    } else {
        match (&pair, &current) {
            (Some((before, target)), Some(Ok(now))) => {
                decision(before, target, now, later_owner(journal, &entry.id, item))
            }
            (Some(_), Some(Err(_))) => "unreadable",
            _ => "unsupported",
        }
    };
    json!({
        "resource": resource(item).unwrap_or_else(|| match item { ChangeItem::QuickPower { .. } => "Power-plan ownership".into(), _ => "Command / dedicated recovery".into() }),
        "before": pair.as_ref().map(|p| &p.0), "target": pair.as_ref().map(|p| &p.1),
        "current": current.as_ref().and_then(|r| r.as_ref().ok()),
        "error": current.as_ref().and_then(|r| r.as_ref().err()), "status": status,
    })
}

fn session_id(entry: &JournalEntry) -> &str {
    entry
        .session
        .as_ref()
        .map(|s| s.id.as_str())
        .unwrap_or(&entry.id)
}

pub fn timeline() -> Result<Value, String> {
    // Read-only snapshot, also available to an unelevated broker UI. Undo
    // re-reads under the engine lock; a preview is never an authorization.
    let journal = safety::load_journal()?;
    let mut sessions: Vec<Value> = Vec::new();
    for entry in &journal {
        let id = session_id(entry);
        let pos = match sessions.iter().position(|s| s["id"] == id) {
            Some(pos) => pos,
            None => {
                sessions.push(json!({ "id": id, "label": entry.session.as_ref().map(|s| s.label.as_str()).unwrap_or(&entry.tweak_name), "time": entry.time, "legacy": entry.session.is_none(), "entries": [] }));
                sessions.len() - 1
            }
        };
        let items: Vec<_> = (0..entry.items.len())
            .map(|i| item_view(entry, i, &journal))
            .collect();
        let status = if entry.reverted {
            "restored"
        } else if entry.reverted_items > 0 {
            "partial"
        } else if !entry.completed {
            "incomplete"
        } else {
            "applied"
        };
        sessions[pos]["entries"]
            .as_array_mut()
            .ok_or("invalid session entries")?
            .push(json!({
                "id": entry.id, "name": entry.tweak_name, "status": status,
                "restored": entry.reverted_items.min(entry.attempted.min(entry.items.len())),
                "attempted": entry.attempted.min(entry.items.len()), "items": items,
            }));
    }
    sessions.reverse();
    Ok(json!({ "sessions": sessions }))
}

fn restore_steps(
    entry: &mut JournalEntry,
    journal: &[JournalEntry],
    mut read_state: impl FnMut(&ChangeItem) -> Result<Setting, String>,
    mut restore: impl FnMut(&ChangeItem) -> Result<(), String>,
    mut persist: impl FnMut(&JournalEntry) -> Result<(), String>,
) -> Result<(), String> {
    if entry.untrusted {
        return Err("untrusted".into());
    }
    while remaining(entry) > 0 {
        let end = remaining(entry);
        let start = if paired_delay(&entry.items, end - 1) {
            end - 2
        } else {
            end - 1
        };
        // Delayed-auto startup and its flag form one state. Verify both after
        // both writes; the intermediate state need not equal the final state.
        for item in entry.items[start..end].iter().rev() {
            let (before, target) = values(item).ok_or("unsupported")?;
            let now = read_state(item)?;
            match decision(
                &before,
                &target,
                &now,
                later_owner(journal, &entry.id, item),
            ) {
                "ready" | "alreadyOriginal" => {}
                other => return Err(other.into()),
            }
        }
        for item in entry.items[start..end].iter().rev() {
            let (before, target) = values(item).ok_or("unsupported")?;
            let now = read_state(item)?;
            match decision(&before, &target, &now, false) {
                "ready" => restore(item)?,
                "alreadyOriginal" => {}
                other => return Err(other.into()),
            }
        }
        for item in &entry.items[start..end] {
            if read_state(item)? != values(item).ok_or("unsupported")?.0 {
                return Err("verificationFailed".into());
            }
        }
        entry.reverted_items += end - start;
        persist(entry)?;
    }
    entry.reverted = true;
    persist(entry)
}

pub fn undo(id: &str) -> Result<Value, String> {
    if !crate::broker::valid_entry_id(id) {
        return Err("Invalid session ID".into());
    }
    if !crate::ps::is_admin() {
        return Err("Administrator rights required for recovery".into());
    }
    let _engine = crate::tweaks::engine_guard()?;
    let mut journal = safety::load_journal()?;
    let ids: Vec<_> = journal
        .iter()
        .filter(|e| session_id(e) == id && !e.reverted)
        .map(|e| e.id.clone())
        .rev()
        .collect();
    if ids.is_empty() {
        return Err("No open changes in this session".into());
    }
    let mut results = Vec::new();
    for entry_id in ids {
        let index = journal
            .iter()
            .position(|e| e.id == entry_id)
            .ok_or("Entry disappeared")?;
        let mut entry = journal[index].clone();
        let result = restore_steps(
            &mut entry,
            &journal,
            read,
            crate::tweaks::revert_item,
            |progress| {
                safety::with_journal(|stored| {
                    let e = stored
                        .iter_mut()
                        .find(|e| e.id == progress.id)
                        .ok_or("Entry disappeared")?;
                    e.reverted_items = progress.reverted_items;
                    e.reverted = progress.reverted;
                    Ok(())
                })
            },
        );
        results.push(json!({ "id": entry_id, "ok": result.is_ok(), "error": result.err() }));
        // Reload durable progress even if a write or verification failed.
        journal = safety::load_journal()?;
    }
    Ok(json!({ "results": results }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry() -> JournalEntry {
        serde_json::from_value(json!({ "id":"test", "tweak_id":"test", "tweak_name":"test", "time":"test", "items":[
            { "kind":"Registry", "root":"HKCU", "path":"test", "name":"a", "prev":{"t":"Dword","v":0}, "new":{"t":"Dword","v":1} },
            { "kind":"Registry", "root":"HKCU", "path":"test", "name":"b", "prev":null, "new":{"t":"Dword","v":1} }
        ], "attempted":2, "reverted":false, "backup_files":[] })).unwrap()
    }
    #[test]
    fn sessions_are_explicit_nested_and_legacy_is_ungrouped() {
        assert!(entry().session.is_none());
        let scope = SessionScope::new("batch");
        let first = current_session("a");
        assert_eq!(first.id, current_session("b").id);
        {
            let _nested = SessionScope::new("nested");
            assert_ne!(first.id, current_session("c").id);
        }
        assert_eq!(first.id, current_session("a").id);
        drop(scope);
        assert_ne!(first.id, current_session("a").id);
    }
    #[test]
    fn drift_type_changes_and_newer_owners_block_writes() {
        let before = Setting::Registry(None);
        let target = Setting::Registry(Some(RegVal::Dword(1)));
        assert_eq!(decision(&before, &target, &target, false), "ready");
        assert_eq!(
            decision(&before, &target, &before, false),
            "alreadyOriginal"
        );
        assert_eq!(decision(&before, &target, &target, true), "newerChange");
        assert_eq!(
            decision(
                &before,
                &target,
                &Setting::Registry(Some(RegVal::Str("1".into()))),
                false
            ),
            "conflict"
        );
        let mut e = entry();
        let journal = vec![e.clone()];
        let result = restore_steps(
            &mut e,
            &journal,
            |_| Ok(Setting::Registry(Some(RegVal::Dword(2)))),
            |_| panic!("must not write"),
            |_| panic!("must not advance"),
        );
        assert_eq!(result.unwrap_err(), "conflict");
        assert_eq!(e.reverted_items, 0);
    }
    #[test]
    fn partial_recovery_persists_each_step_and_retries_without_rewriting_original() {
        let mut e = entry();
        let journal = vec![e.clone()];
        let mut saved = vec![];
        let result = restore_steps(
            &mut e,
            &journal,
            |item| {
                if let ChangeItem::Registry { name, .. } = item {
                    if name == "b" {
                        return Ok(Setting::Registry(None));
                    }
                }
                Err("read failed".into())
            },
            |_| panic!("already original"),
            |e| {
                saved.push(e.reverted_items);
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(saved, vec![1]);
        assert!(!e.reverted);
        restore_steps(
            &mut e,
            &journal,
            |_| Ok(Setting::Registry(Some(RegVal::Dword(0)))),
            |_| panic!("already original"),
            |_| Ok(()),
        )
        .unwrap();
        assert!(e.reverted);
        assert_eq!(e.reverted_items, 2);
    }
    #[test]
    fn newer_open_resource_blocks_older_session_even_with_same_value() {
        let older = entry();
        let mut newer = entry();
        newer.id = "newer".into();
        let mut journal = vec![older.clone(), newer];
        assert!(later_owner(&journal, "test", &older.items[0]));
        journal[1].reverted = true;
        assert!(!later_owner(&journal, "test", &older.items[0]));
    }

    #[test]
    fn failed_write_or_readback_never_marks_a_step_restored() {
        let mut e = entry();
        let journal = vec![e.clone()];
        let expected = Setting::Registry(Some(RegVal::Dword(1)));
        let result = restore_steps(
            &mut e,
            &journal,
            |_| Ok(expected.clone()),
            |_| Err("write failed".into()),
            |_| panic!("must not persist success"),
        );
        assert_eq!(result.unwrap_err(), "write failed");
        assert_eq!(e.reverted_items, 0);
        let result = restore_steps(
            &mut e,
            &journal,
            |_| Ok(expected.clone()),
            |_| Ok(()),
            |_| panic!("must verify first"),
        );
        assert_eq!(result.unwrap_err(), "verificationFailed");
        assert!(!e.reverted);
    }

    #[test]
    fn delayed_service_and_registry_flag_restore_as_one_verified_unit() {
        let mut e = entry();
        e.items = vec![
            ChangeItem::Registry {
                root: "HKLM".into(),
                path: r"SYSTEM\CurrentControlSet\Services\Test".into(),
                name: "DelayedAutostart".into(),
                prev: None,
                new: RegVal::Dword(1),
            },
            ChangeItem::ServiceStartup {
                service: "Test".into(),
                prev: "Automatic".into(),
                new: "AutomaticDelayedStart".into(),
            },
        ];
        let journal = vec![e.clone()];
        let delay = RefCell::new(Some(RegVal::Dword(1)));
        let mut writes = 0;
        let mut saves = vec![];
        restore_steps(
            &mut e,
            &journal,
            |item| {
                Ok(match item {
                    ChangeItem::Registry { .. } => Setting::Registry(delay.borrow().clone()),
                    // Set-Service Automatic leaves the delayed flag intact.
                    _ => Setting::Service(
                        if delay.borrow().as_ref() == Some(&RegVal::Dword(1)) {
                            "AutomaticDelayedStart"
                        } else {
                            "Automatic"
                        }
                        .into(),
                    ),
                })
            },
            |item| {
                writes += 1;
                if let ChangeItem::Registry { prev, .. } = item {
                    *delay.borrow_mut() = prev.clone();
                }
                Ok(())
            },
            |progress| {
                saves.push((progress.reverted_items, progress.reverted));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(writes, 2);
        assert_eq!(saves, vec![(2, false), (2, true)]);
        assert!(delay.borrow().is_none());
    }

    #[test]
    fn opaque_later_commands_and_service_registry_aliases_block_ownership() {
        let older = entry();
        let mut newer = entry();
        newer.id = "opaque".into();
        newer.items = vec![ChangeItem::Command {
            applied: "command".into(),
            revert: "revert".into(),
        }];
        assert!(later_owner(
            &[older.clone(), newer],
            "test",
            &older.items[0]
        ));
        let service = ChangeItem::ServiceStartup {
            service: "Test".into(),
            prev: "Manual".into(),
            new: "Automatic".into(),
        };
        let start = ChangeItem::Registry {
            root: "HKLM".into(),
            path: r"SYSTEM\CurrentControlSet\Services\test".into(),
            name: "Start".into(),
            prev: None,
            new: RegVal::Dword(2),
        };
        assert!(overlaps(&service, &start));
        assert!(overlaps(&start, &service));
    }

    #[cfg(windows)]
    #[test]
    fn isolated_registry_recovery_detects_drift_and_restores_exact_absence() {
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        let path = format!(
            r"Software\ADHyperOptimizeTests\recovery-{:016x}",
            rand::random::<u64>()
        );
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = root.create_subkey(&path).unwrap();
        let result = std::panic::catch_unwind(|| {
            key.set_value("test", &2u32).unwrap();
            let mut e = entry();
            e.items = vec![ChangeItem::Registry {
                root: "HKCU".into(),
                path: path.clone(),
                name: "test".into(),
                prev: None,
                new: RegVal::Dword(1),
            }];
            e.attempted = 1;
            let journal = vec![e.clone()];
            assert_eq!(
                restore_steps(&mut e, &journal, read, crate::tweaks::revert_item, |_| Ok(
                    ()
                ))
                .unwrap_err(),
                "conflict"
            );
            assert_eq!(key.get_value::<u32, _>("test").unwrap(), 2);
            key.set_value("test", &1u32).unwrap();
            let mut progress = vec![];
            restore_steps(&mut e, &journal, read, crate::tweaks::revert_item, |e| {
                progress.push((e.reverted_items, e.reverted));
                Ok(())
            })
            .unwrap();
            assert_eq!(
                crate::tweaks::reg_read("HKCU", &path, "test").unwrap(),
                None
            );
            assert_eq!(progress, vec![(1, false), (1, true)]);
        });
        drop(key);
        root.delete_subkey_all(&path).unwrap();
        result.unwrap();
    }
}
