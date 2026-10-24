//! Safety system: restore points, registry export backups, and a persistent
//! change journal that powers per-tweak Undo. Nothing is modified anywhere in
//! the app without a journal entry being written FIRST (write-ahead).

use crate::ps;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static JOURNAL_LOCK: Mutex<()> = Mutex::new(());
static STATE_DIR: OnceLock<Result<PathBuf, String>> = OnceLock::new();

/// Where privileged state lives: the journal, registry backups and state files
/// that undo executes or imports with admin rights. Elevated, that is an
/// admin-only folder under %ProgramData% (see statestore.rs); data left in the
/// user-writable %APPDATA% folder is migrated into it once, validated.
/// Unelevated sessions keep using %APPDATA%: nothing they record can run with
/// more rights than they have, and an elevated session only takes it over via
/// that validation. With the `broker` feature the unelevated UI reads the
/// protected folder instead, which the elevated helper writes. Fails closed: if the protected folder can't be
/// established, privileged operations fail instead of using a writable one.
pub fn state_dir() -> Result<PathBuf, String> {
    STATE_DIR.get_or_init(resolve_state_dir).clone()
}

fn resolve_state_dir() -> Result<PathBuf, String> {
    #[cfg(windows)]
    if ps::is_admin() {
        let dir = protected_state_dir()?;
        crate::statestore::ensure_secure_dir(&dir)?;
        crate::statestore::migrate_legacy(&app_data_dir(), &dir);
        return Ok(dir);
    }
    // Broker mode: the elevated helper records journal and captured states in
    // the protected folder, so the unelevated UI must read the same folder
    // (Authenticated Users may read it, only SYSTEM/Administrators write) or
    // status, undo and history would not match what was applied. The UI never
    // creates, hardens or migrates it; the broker does that on first use.
    #[cfg(all(windows, feature = "broker"))]
    return protected_state_dir();
    #[allow(unreachable_code)]
    Ok(app_data_dir())
}

#[cfg(windows)]
fn protected_state_dir() -> Result<PathBuf, String> {
    Ok(crate::statestore::program_data_dir()?.join("ADHyperOptimize"))
}

pub fn app_data_dir() -> PathBuf {
    // Under elevation the CWD can be System32; a "." fallback would silently
    // relocate the whole journal/backup pipeline there. Fall back to
    // %APPDATA%, then the home directory, then temp — all guaranteed writable.
    let base = dirs::data_dir()
        .or_else(|| std::env::var("APPDATA").ok().map(PathBuf::from))
        .or_else(dirs::home_dir)
        .unwrap_or_else(std::env::temp_dir);
    let p = base.join("PCOptSuite");
    if let Err(e) = fs::create_dir_all(p.join("backups"))
        .map_err(|e| e.to_string())
        .and_then(|_| fs::create_dir_all(p.join("reports")).map_err(|e| e.to_string()))
    {
        eprintln!(
            "ad-hyperoptimize: cannot create data dir {}: {e}",
            p.display()
        );
    }
    p
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind")]
pub enum ChangeItem {
    Registry {
        root: String, // "HKLM" | "HKCU"
        path: String,
        name: String,
        prev: Option<RegVal>, // None = value did not exist
        new: RegVal,
    },
    ServiceStartup {
        service: String,
        prev: String, // Automatic | Manual | Disabled
        new: String,
    },
    Command {
        applied: String, // command that was run
        revert: String,  // command that undoes it
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "t", content = "v")]
pub enum RegVal {
    Dword(u32),
    Str(String),
    /// Faithful capture of an exotic registry type (REG_BINARY, REG_QWORD,
    /// REG_EXPAND_SZ, REG_MULTI_SZ, …). `ty` mirrors the winreg RegType name,
    /// `hex` is the raw little-endian payload. Only ever produced by capture
    /// and only ever written back verbatim, so undo preserves the original
    /// representation instead of degrading it to REG_SZ or deletion.
    Raw {
        ty: String,
        hex: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JournalEntry {
    pub id: String, // unique entry id
    pub tweak_id: String,
    pub tweak_name: String,
    #[serde(default)]
    pub origin: Option<String>,
    pub time: String,
    pub items: Vec<ChangeItem>,
    pub reverted: bool,
    pub backup_files: Vec<String>,
    /// False between the write-ahead append and full apply success. An
    /// interrupted apply must never count as "applied" in the UI.
    #[serde(default = "default_completed")]
    pub completed: bool,
    /// How many items apply actually attempted (crash safety: undo only
    /// touches what was attempted). usize::MAX = legacy/unknown = all.
    #[serde(default = "default_all_attempted")]
    pub attempted: usize,
    /// How many items (counted from the END of items) a partial revert has
    /// already restored, so a retry resumes exactly where it stopped.
    #[serde(default)]
    pub reverted_items: usize,
    /// Set when the entry was migrated from the user-writable legacy folder
    /// and one of its revert commands is not a shape this app generates: those
    /// commands are never run automatically (see statestore.rs).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub untrusted: bool,
}

fn default_completed() -> bool {
    true
}
fn default_all_attempted() -> usize {
    usize::MAX
}

fn journal_path() -> Result<PathBuf, String> {
    Ok(state_dir()?.join("journal.json"))
}

pub fn load_journal() -> Result<Vec<JournalEntry>, String> {
    load_journal_at(&journal_path()?)
}

pub(crate) fn load_journal_at(p: &Path) -> Result<Vec<JournalEntry>, String> {
    if !p.exists() {
        return Ok(Vec::new());
    }
    // A broken or unreadable journal must never be silently replaced by an
    // empty one: the next save would destroy the recovery history.
    let s = fs::read_to_string(p).map_err(|e| format!("journal read: {e}"))?;
    serde_json::from_str(&s).map_err(|e| format!("journal parse: {e}"))
}

pub fn save_journal(j: &[JournalEntry]) -> Result<(), String> {
    save_journal_at(&journal_path()?, j)
}

pub(crate) fn save_journal_at(p: &Path, j: &[JournalEntry]) -> Result<(), String> {
    let tmp = p.with_extension("json.tmp");
    fs::write(
        &tmp,
        serde_json::to_string_pretty(j).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(&tmp, p).map_err(|e| e.to_string()) // atomic-ish swap
}

/// Serializes every journal read-modify-write pair (apply, revert, undo,
/// appends). Without this, two near-simultaneous commands could interleave
/// load→modify→save and silently drop entries.
pub fn with_journal<R>(
    f: impl FnOnce(&mut Vec<JournalEntry>) -> Result<R, String>,
) -> Result<R, String> {
    let _g = JOURNAL_LOCK
        .lock()
        .map_err(|e| format!("journal lock: {e}"))?;
    let mut j = load_journal()?;
    let r = f(&mut j);
    save_journal(&j)?;
    r
}

/// Appends an entry and returns the ACTUAL stored id: collisions (two applies
/// of the same tweak within one second) get a numeric suffix so every entry
/// keeps an unambiguous restore token. The journal is pruned on append so it
/// stays bounded: beyond 500 entries the oldest already-reverted ones are
/// dropped first; entries with undo state are never dropped.
pub fn append_entry(entry: JournalEntry) -> Result<String, String> {
    with_journal(|j| {
        let mut id = entry.id.clone();
        let mut n = 1;
        while j.iter().any(|e| e.id == id) {
            n += 1;
            id = format!("{}-{n}", entry.id);
        }
        let mut owned = entry;
        owned.id = id.clone();
        j.push(owned);

        const MAX_ENTRIES: usize = 500;
        if j.len() > MAX_ENTRIES {
            // Oldest-first, but only fully reverted AND completed entries are
            // expendable; open undo state always survives.
            let mut reverted_idx: Vec<usize> = j
                .iter()
                .enumerate()
                .filter(|(_, e)| e.reverted && e.completed)
                .map(|(i, _)| i)
                .collect();
            reverted_idx.sort_unstable();
            let excess = j.len() - MAX_ENTRIES;
            let drop: std::collections::HashSet<usize> =
                reverted_idx.into_iter().take(excess).collect();
            if drop.len() == excess {
                let mut i = 0;
                j.retain(|_| {
                    let keep = !drop.contains(&i);
                    i += 1;
                    keep
                });
            }
        }
        Ok(id)
    })
}

/// File-name prefix of the `.reg` backups of one key.
fn backup_prefix(root: &str, path: &str) -> String {
    format!("{root}_{}_", path.replace(['\\', '/'], "_"))
}

/// True for `<prefix><YYYYMMDD-HHMMSS>.reg` of exactly this key. A plain
/// prefix match also hit backups of subkeys (`Key\Sub` → `Key_Sub_…`), and
/// force-revert then imported the wrong key.
fn is_backup_of(file_name: &str, root: &str, path: &str) -> bool {
    file_name
        .strip_prefix(&backup_prefix(root, path))
        .and_then(|rest| rest.strip_suffix(".reg"))
        .is_some_and(|ts| {
            let b = ts.as_bytes();
            b.len() == 15
                && b[8] == b'-'
                && b.iter()
                    .enumerate()
                    .all(|(i, c)| i == 8 || c.is_ascii_digit())
        })
}

/// Newest `.reg` backup for a registry key, or None. Used by the force-revert
/// path to restore prior state instead of blindly deleting values.
pub fn latest_backup_for(root: &str, path: &str) -> Option<String> {
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for entry in fs::read_dir(state_dir().ok()?.join("backups"))
        .ok()?
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_backup_of(&name, root, path) {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
            continue;
        };
        if best.as_ref().map_or(true, |(t, _)| modified > *t) {
            best = Some((modified, entry.path().to_string_lossy().into_owned()));
        }
    }
    best.map(|(_, p)| p)
}

/// Export a registry key with reg.exe before touching it. Returns backup path.
pub fn backup_registry_key(root: &str, path: &str) -> Result<String, String> {
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let dir = state_dir()?.join("backups");
    fs::create_dir_all(&dir).map_err(|e| format!("creating backup folder: {e}"))?;
    let file = dir.join(format!("{}{ts}.reg", backup_prefix(root, path)));
    let full = format!("{root}\\{path}");
    ps::exec("reg.exe", &["export", &full, &file.to_string_lossy(), "/y"])?;
    Ok(file.to_string_lossy().into_owned())
}

/// Create a System Restore point. Requires admin; Windows throttles creation
/// to one per 24h by default, surface that as a warning, not a failure.
#[derive(Debug, PartialEq, Eq)]
enum RestorePointOutcome {
    Created { sequence_number: u32 },
    CoveredByRecent { sequence_number: u32 },
    Missing,
}

struct RestorePointObservation {
    before_sequence: Option<u32>,
    after_sequence: Option<u32>,
    after_description: Option<String>,
    recent_point_exists: bool,
}

fn classify_restore_point(
    observation: &RestorePointObservation,
    description: &str,
) -> RestorePointOutcome {
    match observation.after_sequence {
        Some(sequence_number)
            if Some(sequence_number) != observation.before_sequence
                && observation.after_description.as_deref() == Some(description) =>
        {
            RestorePointOutcome::Created { sequence_number }
        }
        Some(sequence_number)
            if Some(sequence_number) == observation.before_sequence
                && observation.recent_point_exists =>
        {
            RestorePointOutcome::CoveredByRecent { sequence_number }
        }
        _ => RestorePointOutcome::Missing,
    }
}

fn newest_restore_point() -> Result<(Option<u32>, Option<String>, bool), String> {
    let value = ps::run_json(
        "$p = Get-ComputerRestorePoint -ErrorAction Stop | Sort-Object SequenceNumber -Descending | Select-Object -First 1; \
         if ($p) { \
           $recent = try { [Management.ManagementDateTimeConverter]::ToDateTime($p.CreationTime) -ge (Get-Date).AddHours(-24) } catch { $false }; \
           [PSCustomObject]@{ sequence = [uint32]$p.SequenceNumber; description = [string]$p.Description; recent = [bool]$recent } \
         } else { [PSCustomObject]@{ sequence = $null; description = $null; recent = $false } }",
    )?;
    let sequence = value["sequence"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok());
    let description = value["description"].as_str().map(str::to_owned);
    let recent = value["recent"].as_bool().unwrap_or(false);
    Ok((sequence, description, recent))
}

pub fn create_restore_point(description: &str) -> Result<String, String> {
    if !ps::is_admin() {
        return Err(
            "Administrator rights required for restore points. Restart the app as admin.".into(),
        );
    }
    let before_sequence = newest_restore_point()?.0;
    // PowerShell doubles '' inside single-quoted strings; stripping changed
    // the user's description text.
    let desc = ps::escape_single(description);
    let result = ps::run(&format!(
        "Checkpoint-Computer -Description '{desc}' -RestorePointType MODIFY_SETTINGS -ErrorAction Stop; 'OK'"
    ));
    if let Err(e) = &result {
        if !e.contains("1440") && !e.to_lowercase().contains("already been created") {
            return Err(e.clone());
        }
    }
    let (after_sequence, after_description, recent_point_exists) = newest_restore_point()?;
    let observation = RestorePointObservation {
        before_sequence,
        after_sequence,
        after_description,
        recent_point_exists,
    };
    match classify_restore_point(&observation, description) {
        RestorePointOutcome::Created { .. } => Ok("Restore point created.".into()),
        RestorePointOutcome::CoveredByRecent { .. } => Ok(
            "Windows limits restore points to one per 24h; an existing recent point covers you."
                .into(),
        ),
        RestorePointOutcome::Missing => {
            Err("Windows did not create a restore point, and no recent point was found.".into())
        }
    }
}

pub fn list_restore_points() -> serde_json::Value {
    ps::run_json("Get-ComputerRestorePoint -ErrorAction Stop | Select-Object SequenceNumber,Description,RestorePointType,CreationTime")
        .unwrap_or_else(|e| serde_json::json!({ "error": e.trim() }))
}

/// Delete a restore point by sequence number.
///
/// `SystemRestore.DeleteRestorePoint(SequenceNumber)` is the documented WMI
/// API for exactly this. The old code called `.Delete()` on the *class*
/// object (which always fails) and then fell back to `vssadmin … /oldest`,
/// deleting the oldest shadow copy instead of the selected point.
pub fn delete_restore_point(sequence_number: u32) -> Result<String, String> {
    if !ps::is_admin() {
        return Err("Administrator rights required.".into());
    }
    let script = format!(
        r#"
$rp = Get-ComputerRestorePoint | Where-Object {{ $_.SequenceNumber -eq {sequence_number} }}
if (-not $rp) {{ throw "Restore point {sequence_number} not found" }}
$result = ([wmiclass]'root\default:SystemRestore').DeleteRestorePoint({sequence_number})
if ($result.ReturnValue -ne 0) {{ throw "DeleteRestorePoint failed (code $($result.ReturnValue))" }}
"Deleted restore point {sequence_number}"
"#
    );
    ps::run(&script).map(|s| s.trim().to_string())
}

/// Open Windows System Restore UI (rstrui.exe) for interactive restore.
pub fn launch_rstrui() -> Result<String, String> {
    ps::run("Start-Process rstrui.exe; 'Opened System Restore'").map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::{classify_restore_point, RestorePointObservation, RestorePointOutcome};

    #[test]
    fn backups_match_their_own_key_only() {
        let key = r"SOFTWARE\Policies\Microsoft\Windows\Windows Search";
        let own = "HKLM_SOFTWARE_Policies_Microsoft_Windows_Windows Search_20261024-101500.reg";
        assert!(super::is_backup_of(own, "HKLM", key));
        // Subkey backup shares the prefix.
        assert!(!super::is_backup_of(
            "HKLM_SOFTWARE_Policies_Microsoft_Windows_Windows Search_Sub_20261024-101500.reg",
            "HKLM",
            key
        ));
        assert!(!super::is_backup_of(own, "HKCU", key));
        assert!(!super::is_backup_of(
            "HKLM_SOFTWARE_Policies_Microsoft_Windows_Windows Search_20261024-101500.txt",
            "HKLM",
            key
        ));
        // The name written by backup_registry_key is recognised.
        let written = format!("{}20261024-101500.reg", super::backup_prefix("HKLM", key));
        assert!(super::is_backup_of(&written, "HKLM", key));
    }

    #[test]
    fn classifies_created_restore_point_when_matching_sequence_changes() {
        let observation = RestorePointObservation {
            before_sequence: Some(41),
            after_sequence: Some(42),
            after_description: Some("AD HyperOptimize".into()),
            recent_point_exists: true,
        };

        let outcome = classify_restore_point(&observation, "AD HyperOptimize");

        assert_eq!(
            outcome,
            RestorePointOutcome::Created {
                sequence_number: 42
            }
        );
    }

    #[test]
    fn classifies_first_matching_restore_point_as_created() {
        let observation = RestorePointObservation {
            before_sequence: None,
            after_sequence: Some(1),
            after_description: Some("AD HyperOptimize".into()),
            recent_point_exists: true,
        };

        let outcome = classify_restore_point(&observation, "AD HyperOptimize");

        assert_eq!(outcome, RestorePointOutcome::Created { sequence_number: 1 });
    }

    #[test]
    fn classifies_unchanged_recent_restore_point_as_throttled() {
        let observation = RestorePointObservation {
            before_sequence: Some(42),
            after_sequence: Some(42),
            after_description: Some("Windows Update".into()),
            recent_point_exists: true,
        };

        let outcome = classify_restore_point(&observation, "AD HyperOptimize");

        assert_eq!(
            outcome,
            RestorePointOutcome::CoveredByRecent {
                sequence_number: 42
            }
        );
    }

    #[test]
    fn classifies_unchanged_stale_restore_point_as_missing() {
        let observation = RestorePointObservation {
            before_sequence: Some(42),
            after_sequence: Some(42),
            after_description: Some("Old restore point".into()),
            recent_point_exists: false,
        };

        let outcome = classify_restore_point(&observation, "AD HyperOptimize");

        assert_eq!(outcome, RestorePointOutcome::Missing);
    }

    #[test]
    fn rejects_sequence_change_from_unrelated_restore_point() {
        let observation = RestorePointObservation {
            before_sequence: Some(42),
            after_sequence: Some(43),
            after_description: Some("Another application".into()),
            recent_point_exists: true,
        };

        let outcome = classify_restore_point(&observation, "AD HyperOptimize");

        assert_eq!(outcome, RestorePointOutcome::Missing);
    }
}
