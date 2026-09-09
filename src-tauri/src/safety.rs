//! Safety system: restore points, registry export backups, and a persistent
//! change journal that powers per-tweak Undo. Nothing is modified anywhere in
//! the app without a journal entry being written FIRST (write-ahead).

use crate::ps;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

static JOURNAL_LOCK: Mutex<()> = Mutex::new(());

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
}

fn default_completed() -> bool {
    true
}
fn default_all_attempted() -> usize {
    usize::MAX
}

fn journal_path() -> PathBuf {
    app_data_dir().join("journal.json")
}

pub fn load_journal() -> Result<Vec<JournalEntry>, String> {
    let p = journal_path();
    if !p.exists() {
        return Ok(Vec::new());
    }
    // A broken or unreadable journal must never be silently replaced by an
    // empty one: the next save would destroy the recovery history.
    let s = fs::read_to_string(&p).map_err(|e| format!("journal read: {e}"))?;
    serde_json::from_str(&s).map_err(|e| format!("journal parse: {e}"))
}

pub fn save_journal(j: &[JournalEntry]) -> Result<(), String> {
    let tmp = journal_path().with_extension("json.tmp");
    fs::write(
        &tmp,
        serde_json::to_string_pretty(j).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(&tmp, journal_path()).map_err(|e| e.to_string()) // atomic-ish swap
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
    let r = f(&mut j)?;
    save_journal(&j)?;
    Ok(r)
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

/// Newest `.reg` backup for a registry key, or None. Used by the force-revert
/// path to restore prior state instead of blindly deleting values.
pub fn latest_backup_for(root: &str, path: &str) -> Option<String> {
    let safe = path.replace(['\\', '/'], "_");
    let prefix = format!("{root}_{safe}_");
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for entry in fs::read_dir(app_data_dir().join("backups")).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix) && name.ends_with(".reg") {
            let modified = entry.metadata().and_then(|m| m.modified()).ok()?;
            let better = match &best {
                Some((t, _)) => modified > *t,
                None => true,
            };
            if better {
                best = Some((modified, entry.path().to_string_lossy().into_owned()));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Export a registry key with reg.exe before touching it. Returns backup path.
pub fn backup_registry_key(root: &str, path: &str) -> Result<String, String> {
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let safe = path.replace(['\\', '/'], "_");
    let file = app_data_dir()
        .join("backups")
        .join(format!("{root}_{safe}_{ts}.reg"));
    let full = format!("{root}\\{path}");
    ps::exec("reg.exe", &["export", &full, &file.to_string_lossy(), "/y"])?;
    Ok(file.to_string_lossy().into_owned())
}

/// Create a System Restore point. Requires admin; Windows throttles creation
/// to one per 24h by default, surface that as a warning, not a failure.
pub fn create_restore_point(description: &str) -> Result<String, String> {
    if !ps::is_admin() {
        return Err(
            "Administrator rights required for restore points. Restart the app as admin.".into(),
        );
    }
    // PowerShell doubles '' inside single-quoted strings; stripping changed
    // the user's description text.
    let desc = description.replace('\'', "''");
    match ps::run(&format!(
        "Checkpoint-Computer -Description '{desc}' -RestorePointType MODIFY_SETTINGS -ErrorAction Stop; 'OK'"
    )) {
        Ok(_) => Ok("Restore point created.".into()),
        Err(e) if e.contains("1440") || e.to_lowercase().contains("already been created") => Ok(
            "Windows limits restore points to one per 24h, an existing recent point covers you.".into(),
        ),
        Err(e) => Err(e),
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
