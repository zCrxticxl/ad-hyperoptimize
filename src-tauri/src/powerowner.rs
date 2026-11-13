//! Persistent ownership for overlapping Quick Boost power-plan requests.
#[cfg(test)]
mod tests {
    use super::State;
    use super::{acquire_at, release_at};
    use crate::gameprofile::{PLAN_BALANCED, PLAN_HIGH_PERFORMANCE};

    #[test]
    fn failed_apply_and_restore_keep_durable_recovery_for_retry() {
        let dir = std::env::temp_dir().join(format!("ad-power-test-{}", rand::random::<u64>()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("state.json");
        assert!(acquire_at(
            &path,
            "A",
            || Ok(PLAN_BALANCED.into()),
            |_| Err("powercfg failed".into())
        )
        .is_err());
        let saved = std::fs::read(&path).unwrap();
        assert!(release_at(
            &path,
            "A",
            || Ok(PLAN_HIGH_PERFORMANCE.into()),
            |_| Err("restore failed".into())
        )
        .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), saved);
        release_at(
            &path,
            "A",
            || Ok(PLAN_HIGH_PERFORMANCE.into()),
            |plan| {
                assert_eq!(plan, PLAN_BALANCED);
                Ok(())
            },
        )
        .unwrap();
        assert!(!path.exists());
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn failed_journal_write_never_changes_plan_and_user_plan_is_preserved() {
        let dir = std::env::temp_dir().join(format!("ad-power-write-{}", rand::random::<u64>()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("state.json");
        std::fs::create_dir(path.with_extension("tmp")).unwrap();
        // A non-existent parent cannot be persisted; no system mutation may follow.
        assert!(acquire_at(
            &dir.join("missing/state.json"),
            "A",
            || Ok(PLAN_BALANCED.into()),
            |_| panic!("must persist before mutation")
        )
        .is_err());
        acquire_at(&path, "A", || Ok(PLAN_BALANCED.into()), |_| Ok(())).unwrap();
        release_at(
            &path,
            "A",
            || Ok("custom-plan".into()),
            |_| panic!("must preserve user change"),
        )
        .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn release_order_preserves_baseline_and_surviving_owner_after_reload() {
        for order in [["A", "B"], ["B", "A"]] {
            let mut state = State {
                baseline: "original".into(),
                owners: vec![],
            };
            state.acquire("A");
            state.acquire("B");
            state.acquire("A");
            assert_eq!(state.owners.len(), 2);
            let mut state: State =
                serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
            state.release(order[0]);
            assert_eq!(state.owners, vec![order[1]]);
            assert_eq!(state.baseline, "original");
            state.release(order[1]);
            assert!(state.owners.is_empty());
            assert_eq!(state.baseline, "original");
        }
    }
}
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
static LOCK: Mutex<()> = Mutex::new(());
#[derive(Serialize, Deserialize)]
struct State {
    baseline: String,
    owners: Vec<String>,
}

impl State {
    fn acquire(&mut self, owner: &str) {
        if !self.owners.iter().any(|s| s == owner) {
            self.owners.push(owner.into());
        }
    }
    fn release(&mut self, owner: &str) {
        self.owners.retain(|s| s != owner);
    }
}

pub fn acquire(owner: &str) -> Result<(), String> {
    let _guard = LOCK.lock().map_err(|_| "power ownership lock poisoned")?;
    let path = crate::safety::state_dir()?.join("quick-power.json");
    acquire_at(
        &path,
        owner,
        || {
            crate::gameprofile::get_active_plan_guid()
                .ok_or_else(|| "Cannot read active power plan".into())
        },
        |guid| crate::ps::exec("powercfg.exe", &["/setactive", guid]).map(|_| ()),
    )
}

fn acquire_at(
    path: &std::path::Path,
    owner: &str,
    read_plan: impl FnOnce() -> Result<String, String>,
    set_plan: impl FnOnce(&str) -> Result<(), String>,
) -> Result<(), String> {
    let mut state: State = if path.exists() {
        serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
    } else {
        State {
            baseline: read_plan()?,
            owners: vec![],
        }
    };
    state.acquire(owner);
    crate::safety::write_user_file(
        path,
        &serde_json::to_vec(&state).map_err(|e| e.to_string())?,
    )?;
    set_plan(crate::gameprofile::PLAN_HIGH_PERFORMANCE)
}

pub fn release(owner: &str) -> Result<(), String> {
    let _guard = LOCK.lock().map_err(|_| "power ownership lock poisoned")?;
    let path = crate::safety::state_dir()?.join("quick-power.json");
    release_at(
        &path,
        owner,
        || {
            crate::gameprofile::get_active_plan_guid()
                .ok_or_else(|| "Cannot read active power plan".into())
        },
        |guid| crate::ps::exec("powercfg.exe", &["/setactive", guid]).map(|_| ()),
    )
}

fn release_at(
    path: &std::path::Path,
    owner: &str,
    read_plan: impl FnOnce() -> Result<String, String>,
    set_plan: impl FnOnce(&str) -> Result<(), String>,
) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let mut state: State =
        serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if !state.owners.iter().any(|s| s == owner) {
        return Ok(());
    }
    state.release(owner);
    if state.owners.is_empty() {
        if !crate::ps::is_guid(&state.baseline) {
            return Err("Invalid baseline power plan".into());
        }
        let active = read_plan()?;
        if active.eq_ignore_ascii_case(crate::gameprofile::PLAN_HIGH_PERFORMANCE) {
            set_plan(&state.baseline)?;
        }
        std::fs::remove_file(path).map_err(|e| e.to_string())
    } else {
        crate::safety::write_user_file(
            path,
            &serde_json::to_vec(&state).map_err(|e| e.to_string())?,
        )
    }
}
