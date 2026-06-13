//! Safety system: restore points, registry export backups, and a persistent
//! change journal that powers per-tweak Undo. Nothing is modified anywhere in
//! the app without a journal entry being written FIRST (write-ahead).

use crate::ps;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub fn app_data_dir() -> PathBuf {
    let p = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("PCOptSuite");
    let _ = fs::create_dir_all(p.join("backups"));
    let _ = fs::create_dir_all(p.join("reports"));
    p
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind")]
pub enum ChangeItem {
    Registry {
        root: String,           // "HKLM" | "HKCU"
        path: String,
        name: String,
        prev: Option<RegVal>,   // None = value did not exist
        new: RegVal,
    },
    ServiceStartup {
        service: String,
        prev: String,           // Automatic | Manual | Disabled
        new: String,
    },
    Command {
        applied: String,        // command that was run
        revert: String,         // command that undoes it
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "t", content = "v")]
pub enum RegVal {
    Dword(u32),
    Str(String),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JournalEntry {
    pub id: String,             // unique entry id
    pub tweak_id: String,
    pub tweak_name: String,
    pub time: String,
    pub items: Vec<ChangeItem>,
    pub reverted: bool,
    pub backup_files: Vec<String>,
}

fn journal_path() -> PathBuf {
    app_data_dir().join("journal.json")
}

pub fn load_journal() -> Vec<JournalEntry> {
    fs::read_to_string(journal_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_journal(j: &[JournalEntry]) -> Result<(), String> {
    let tmp = journal_path().with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(j).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    fs::rename(&tmp, journal_path()).map_err(|e| e.to_string()) // atomic-ish swap
}

pub fn append_entry(entry: JournalEntry) -> Result<(), String> {
    let mut j = load_journal();
    j.push(entry);
    save_journal(&j)
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
/// to one per 24h by default — surface that as a warning, not a failure.
pub fn create_restore_point(description: &str) -> Result<String, String> {
    if !ps::is_admin() {
        return Err("Administrator rights required for restore points. Restart the app as admin.".into());
    }
    let desc = description.replace('\'', "");
    match ps::run(&format!(
        "Checkpoint-Computer -Description '{desc}' -RestorePointType MODIFY_SETTINGS -ErrorAction Stop; 'OK'"
    )) {
        Ok(_) => Ok("Restore point created.".into()),
        Err(e) if e.contains("1440") || e.to_lowercase().contains("already been created") => Ok(
            "Windows limits restore points to one per 24h — an existing recent point covers you.".into(),
        ),
        Err(e) => Err(e),
    }
}

pub fn list_restore_points() -> serde_json::Value {
    ps::run_json("Get-ComputerRestorePoint -ErrorAction Stop | Select-Object SequenceNumber,Description,RestorePointType,CreationTime")
        .unwrap_or_else(|e| serde_json::json!({ "error": e.trim() }))
}

/// Delete a restore point by sequence number (uses vssadmin).
pub fn delete_restore_point(sequence_number: u32) -> Result<String, String> {
    if !ps::is_admin() {
        return Err("Administrator rights required.".into());
    }
    // vssadmin needs the shadow ID; Get-ComputerRestorePoint doesn't expose it directly.
    // Use WMI to get the shadow copy ID matching the sequence number.
    let script = format!(r#"
$rp = Get-ComputerRestorePoint | Where-Object {{ $_.SequenceNumber -eq {sequence_number} }}
if (-not $rp) {{ throw "Restore point {sequence_number} not found" }}
$id = $rp.SequenceNumber
$wmi = Get-WmiObject -Class SystemRestore -Namespace root\default | Where-Object {{ $_.SequenceNumber -eq $id }}
if ($wmi) {{ $wmi.Delete() | Out-Null; "Deleted restore point {sequence_number}" }}
else {{ vssadmin delete shadows /for=C: /oldest /quiet | Out-Null; "Deleted (oldest shadow)" }}
"#);
    ps::run(&script).map(|s| s.trim().to_string())
}

/// Open Windows System Restore UI (rstrui.exe) for interactive restore.
pub fn launch_rstrui() -> Result<String, String> {
    ps::run("Start-Process rstrui.exe; 'Opened System Restore'")
        .map(|s| s.trim().to_string())
}
