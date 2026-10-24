//! Protected storage for privileged state.
//!
//! The undo journal, registry backups and the state files of the debloater,
//! game booster and power-plan helpers are *executed or imported with admin
//! rights* on undo. They used to live in `%APPDATA%\PCOptSuite`, which any
//! unelevated process of the same user can write: malware without admin could
//! plant a revert command and get it run elevated on the next Undo.
//!
//! An elevated app now keeps that state in `%ProgramData%\ADHyperOptimize`,
//! owned by Administrators with a protected DACL (SYSTEM and Administrators
//! full control, Authenticated Users read-only). A pre-existing folder that is
//! not already locked down that way may contain planted files, so it is moved
//! aside instead of being "fixed" in place.
//!
//! Data written by earlier versions (or by unelevated sessions, which keep
//! using `%APPDATA%`) is migrated once, item by item: registry/service steps
//! are plain data and re-validated, revert *commands* are only carried over
//! when they match a shape this app generates; anything else marks the entry
//! untrusted, and its commands are never run automatically.

use crate::safety::{ChangeItem, JournalEntry};
use std::path::{Path, PathBuf};

const SID_SYSTEM: &str = "S-1-5-18";
const SID_ADMINS: &str = "S-1-5-32-544";

/// Rights that let a principal change the folder or plant content in it:
/// WriteData/CreateFiles, AppendData, WriteExtendedAttributes, DeleteSubdirs,
/// WriteAttributes, Delete, WRITE_DAC, WRITE_OWNER, GENERIC_ALL, GENERIC_WRITE.
const WRITE_MASK: u32 =
    0x2 | 0x4 | 0x10 | 0x40 | 0x100 | 0x1_0000 | 0x4_0000 | 0x8_0000 | 0x1000_0000 | 0x4000_0000;

#[derive(Debug, PartialEq)]
struct AclReport {
    owner: String,
    protected: bool,
    /// (allow, sid, rights mask)
    rules: Vec<(bool, String, u32)>,
}

/// Parses the `OWNER|…`, `PROTECTED|…`, `RULE|Allow|<sid>|<rights>` lines
/// produced by `acl_report_script`. SIDs and numeric rights are locale-free.
fn parse_acl_report(out: &str) -> Option<AclReport> {
    let mut owner = None;
    let mut protected = None;
    let mut rules = Vec::new();
    for line in out.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let parts: Vec<&str> = line.split('|').collect();
        match parts.as_slice() {
            ["OWNER", sid] => owner = Some(sid.to_string()),
            ["PROTECTED", p] => protected = Some(p.eq_ignore_ascii_case("true")),
            ["RULE", kind, sid, rights] => {
                let mask = rights.parse::<i64>().ok()? as u32;
                rules.push((kind.eq_ignore_ascii_case("allow"), sid.to_string(), mask));
            }
            _ => return None,
        }
    }
    Some(AclReport {
        owner: owner?,
        protected: protected?,
        rules,
    })
}

/// Strict = owned by SYSTEM/Administrators, no inherited rules, and nobody
/// else is granted any right that could modify the folder or its contents.
fn acl_is_strict(r: &AclReport) -> bool {
    let trusted = |sid: &str| sid == SID_SYSTEM || sid == SID_ADMINS;
    r.protected
        && trusted(&r.owner)
        && r.rules
            .iter()
            .all(|(allow, sid, mask)| !allow || trusted(sid) || mask & WRITE_MASK == 0)
}

// ── Revert-command validation (pure) ─────────────────────────────────────────

fn is_guid_or_current(s: &str) -> bool {
    s == "scheme_current" || crate::ps::is_guid(s)
}

/// Strips `powershell -Command ` and one pair of legacy wrapping quotes.
fn ps_script(cmd: &str) -> Option<&str> {
    let rest = cmd.strip_prefix("powershell -Command ")?.trim();
    Some(
        if rest.len() >= 2
            && rest.starts_with('"')
            && rest.ends_with('"')
            && !rest[1..rest.len() - 1].contains('"')
        {
            &rest[1..rest.len() - 1]
        } else {
            rest
        },
    )
}

/// Single-quoted argument between `prefix` and `suffix`; rejects anything a
/// validator below would not have produced.
fn quoted_between<'a>(s: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    s.strip_prefix(prefix)?.strip_suffix(suffix)
}

fn trusted_powercfg(cmd: &str) -> bool {
    let t: Vec<&str> = cmd.split_whitespace().collect();
    match t.as_slice() {
        ["powercfg", "/setactive", g] => is_guid_or_current(g),
        ["powercfg", "/h", "on" | "off"] => true,
        ["powercfg", "/setacvalueindex" | "/setdcvalueindex", g, sub, set, n] => {
            is_guid_or_current(g)
                && crate::ps::is_guid(sub)
                && crate::ps::is_guid(set)
                && n.parse::<u32>().is_ok()
        }
        _ => false,
    }
}

/// The captured power-index revert (tweaks::capture_revert), current or
/// legacy (quoted, without the trailing exit) form. Returns the current form.
fn trusted_power_index(cmd: &str) -> Option<String> {
    let script = ps_script(cmd)?;
    let t: Vec<&str> = script.split_whitespace().collect();
    if t.len() < 6 || t[0] != "powercfg" || t[1] != "/setacvalueindex" {
        return None;
    }
    let (g, sub, set) = (t[2], t[3], t[4]);
    let ac = t[5].strip_suffix(';')?;
    let dc_pos = t.iter().position(|x| *x == "/setdcvalueindex")?;
    let dc = t.get(dc_pos + 4)?.strip_suffix(';')?;
    if !(crate::ps::is_guid(g)
        && crate::ps::is_guid(sub)
        && crate::ps::is_guid(set)
        && ac.parse::<u32>().is_ok()
        && dc.parse::<u32>().is_ok())
    {
        return None;
    }
    let current = crate::tweaks::power_index_revert(g, sub, set, ac, dc);
    let legacy = format!(
        "powercfg /setacvalueindex {g} {sub} {set} {ac}; if ($LASTEXITCODE -eq 0) {{ powercfg /setdcvalueindex {g} {sub} {set} {dc}; if ($LASTEXITCODE -eq 0) {{ powercfg /setactive scheme_current }} }}"
    );
    let norm = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let script = norm(script);
    let current_script = norm(ps_script(&current)?);
    (script == current_script || script == norm(&legacy)).then_some(current)
}

fn trusted_netsh(cmd: &str) -> bool {
    matches!(
        cmd,
        "netsh int tcp set global autotuninglevel=normal"
            | "netsh int tcp set global autotuninglevel=disabled"
            | "netsh int tcp set global autotuninglevel=highlyrestricted"
            | "netsh int tcp set global autotuninglevel=restricted"
            | "netsh int tcp set global autotuninglevel=experimental"
            | "netsh int tcp set global rss=enabled"
            | "netsh int tcp set global rss=disabled"
    )
}

/// Re-enabling the task this entry is about. Disabling a task (the revert of
/// an "enable") is never taken from the user-writable folder: it could turn
/// off any task, Defender's scans included.
fn trusted_sched_task(tweak_id: &str, cmd: &str) -> bool {
    let Some(script) = ps_script(cmd) else {
        return false;
    };
    let Some(args) = quoted_between(
        script,
        "Enable-ScheduledTask -TaskPath '",
        "' -ErrorAction Stop | Out-Null",
    ) else {
        return false;
    };
    let Some((path, name)) = args.split_once("' -TaskName '") else {
        return false;
    };
    crate::schedtasks::is_valid_task_ident(path)
        && crate::schedtasks::is_valid_task_ident(name)
        && tweak_id == format!("task:{path}{name}")
}

/// Restarting the service of the privacy item this entry is about.
fn trusted_service_start(tweak_id: &str, cmd: &str) -> bool {
    let declared = tweak_id
        .strip_prefix("privacy:")
        .and_then(crate::privacy::service_target);
    ps_script(cmd)
        .and_then(|s| quoted_between(s, "Start-Service -Name '", "' -ErrorAction Stop"))
        .is_some_and(|svc| declared == Some(svc))
}

/// Only reverts that turn Defender protection back ON are carried over; one
/// that switches it off is exactly what planted data would ask for.
fn trusted_defender(tweak_id: &str, cmd: &str) -> bool {
    matches!(
        (tweak_id, ps_script(cmd)),
        (
            "defenderRealtime",
            Some("Set-MpPreference -DisableRealtimeMonitoring $false -ErrorAction Stop"),
        ) | (
            "defenderCloud",
            Some(
                "Set-MpPreference -MAPSReporting 1 -ErrorAction Stop"
                    | "Set-MpPreference -MAPSReporting 2 -ErrorAction Stop"
            ),
        )
    )
}

/// The element of the boot tweak this entry is about, and never a value that
/// lowers boot security (test signing on, DEP always off).
fn trusted_bcdedit(tweak_id: &str, cmd: &str) -> bool {
    let Some((target, element)) = tweak_id
        .strip_prefix("bcd:")
        .and_then(crate::bootopt::declared_element)
    else {
        return false;
    };
    let t: Vec<&str> = cmd.split_whitespace().collect();
    let value = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric());
    let weakens = |v: &str| {
        let v = v.to_ascii_lowercase();
        (element == "testsigning" && !matches!(v.as_str(), "no" | "off"))
            || (element == "nx" && v == "alwaysoff")
    };
    match t.as_slice() {
        ["bcdedit", "/set", tg, el, v] => {
            *tg == target && *el == element && value(v) && !weakens(v)
        }
        ["bcdedit", "/deletevalue", tg, el] => *tg == target && *el == element,
        _ => false,
    }
}

/// The revert command to keep for a migrated Command item, or None when it
/// does not match anything this app generates (the entry becomes untrusted).
/// `static_reverts` are the catalog's fixed revert strings for `tweak_id`.
fn trusted_revert(tweak_id: &str, revert: &str, static_reverts: &[&str]) -> Option<String> {
    if tweak_id == "perftweaks:timer" {
        // Re-derived from code: older journals stored a reset script that
        // could not even delete the legacy helper file.
        return Some(crate::perftweaks::timer_revert_command());
    }
    if static_reverts.contains(&revert) {
        return Some(revert.to_string());
    }
    if trusted_powercfg(revert)
        || trusted_netsh(revert)
        || trusted_sched_task(tweak_id, revert)
        || trusted_service_start(tweak_id, revert)
        || trusted_defender(tweak_id, revert)
        || trusted_bcdedit(tweak_id, revert)
    {
        return Some(revert.to_string());
    }
    trusted_power_index(revert)
}

/// Whether the feature that recorded `tweak_id` can write this registry
/// value. Undo writes the captured previous value elevated, so a migrated
/// item is only data when its target is one this app itself declares;
/// otherwise a planted entry could set any HKLM value (IFEO, Run, policies).
fn registry_declared(tweak_id: &str, root: &str, path: &str, name: &str) -> bool {
    let eq = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let listed = |targets: &[(&str, &str, &str)]| {
        targets
            .iter()
            .any(|(r, p, n)| eq(r, root) && eq(p, path) && eq(n, name))
    };
    if let Some(id) = tweak_id.strip_prefix("privacy:") {
        return listed(crate::privacy::registry_targets(id));
    }
    if let Some(id) = tweak_id.strip_prefix("gpu:") {
        return crate::gputweaks::declares_registry(id, root, path, name);
    }
    if tweak_id.starts_with("nvCtrlPanel:") {
        return crate::gputweaks::nv_panel_declares(root, path, name);
    }
    if tweak_id == "perftweaks:timer" {
        return listed(&[(
            "HKLM",
            r"SYSTEM\CurrentControlSet\Control\Session Manager\kernel",
            "GlobalTimerResolutionRequests",
        )]);
    }
    if let Some(rest) = tweak_id.strip_prefix("startup:") {
        // startup:<scope>:<value name>; the value name may contain ':'.
        return rest.split_once(':').is_some_and(|(scope, entry_name)| {
            crate::startup::approved_location(scope)
                .is_some_and(|(r, p)| eq(r, root) && eq(p, path) && entry_name == name)
        });
    }
    // Service-manager entries (service:<name>) can target any service and
    // are therefore never trusted from the user-writable folder.
    listed(&crate::tweaks::declared_registry(tweak_id))
}

/// Whether the feature that recorded `tweak_id` changes this service.
fn service_declared(tweak_id: &str, service: &str) -> bool {
    if let Some(id) = tweak_id.strip_prefix("privacy:") {
        return crate::privacy::service_target(id).is_some_and(|s| s.eq_ignore_ascii_case(service));
    }
    if let Some(id) = tweak_id.strip_prefix("gpu:") {
        return crate::gputweaks::declares_service(id, service);
    }
    crate::tweaks::declared_services(tweak_id)
        .iter()
        .any(|s| s.eq_ignore_ascii_case(service))
}

/// Validates one legacy entry in place. Returns true when it is fully trusted.
fn sanitize_entry(entry: &mut JournalEntry) -> bool {
    let static_reverts: Vec<&'static str> = crate::tweaks::static_cmd_reverts(&entry.tweak_id);
    let mut trusted = true;
    for item in &mut entry.items {
        match item {
            ChangeItem::Registry {
                root, path, name, ..
            } => {
                if !registry_declared(&entry.tweak_id, root, path, name) {
                    trusted = false;
                }
            }
            ChangeItem::ServiceStartup { service, .. } => {
                if !crate::ps::is_safe_ident(service) || !service_declared(&entry.tweak_id, service)
                {
                    trusted = false;
                }
            }
            ChangeItem::Command { revert, .. } => {
                match trusted_revert(&entry.tweak_id, revert, &static_reverts) {
                    Some(cmd) => *revert = cmd,
                    None => trusted = false,
                }
            }
        }
    }
    entry.untrusted = !trusted;
    trusted
}

/// Merges legacy entries (by id) into `secure`, sanitizing each. Returns the
/// number of entries added and how many of them are untrusted.
fn merge_legacy(secure: &mut Vec<JournalEntry>, legacy: Vec<JournalEntry>) -> (usize, usize) {
    let (mut added, mut untrusted) = (0, 0);
    for mut entry in legacy {
        if secure.iter().any(|e| e.id == entry.id) {
            continue;
        }
        if !sanitize_entry(&mut entry) {
            untrusted += 1;
        }
        secure.push(entry);
        added += 1;
    }
    (added, untrusted)
}

// ── Windows: folder resolution, ACL enforcement, migration ───────────────────

/// `%ProgramData%` resolved from HKLM (admin-writable only). The environment
/// variable is not used: an elevated process inherits the user's environment,
/// and a user-level override would redirect privileged state.
#[cfg(windows)]
pub fn program_data_dir() -> Result<PathBuf, String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    let hklm = winreg::RegKey::predef(HKEY_LOCAL_MACHINE);
    let system_root: String = hklm
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .and_then(|k| k.get_value("SystemRoot"))
        .map_err(|e| format!("reading SystemRoot: {e}"))?;
    let raw: String = hklm
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList")
        .and_then(|k| k.get_value("ProgramData"))
        .map_err(|e| format!("reading ProgramData location: {e}"))?;
    resolve_program_data(&system_root, &raw)
}

/// Expands `%SystemDrive%` (the only variable Windows uses here) from the
/// drive of SystemRoot; anything else unexpected is refused.
fn resolve_program_data(system_root: &str, raw: &str) -> Result<PathBuf, String> {
    let drive = system_root
        .get(..2)
        .filter(|d| d.as_bytes()[0].is_ascii_alphabetic() && d.as_bytes()[1] == b':')
        .ok_or("unexpected SystemRoot")?;
    let lower = raw.to_ascii_lowercase();
    let expanded = match lower.find("%systemdrive%") {
        Some(i) => format!(
            "{}{}{}",
            &raw[..i],
            drive,
            &raw[i + "%systemdrive%".len()..]
        ),
        None => raw.to_string(),
    };
    let b = expanded.as_bytes();
    if expanded.contains('%') || b.len() < 4 || !b[0].is_ascii_alphabetic() || &b[1..3] != b":\\" {
        return Err(format!("unexpected ProgramData location: {raw}"));
    }
    Ok(PathBuf::from(expanded))
}

#[cfg(windows)]
fn is_plain_dir(dir: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    std::fs::symlink_metadata(dir)
        .is_ok_and(|m| m.is_dir() && m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0)
}

#[cfg(windows)]
fn acl_report(dir: &Path) -> Result<AclReport, String> {
    let q = crate::ps::quote_single(&dir.to_string_lossy());
    let out = crate::ps::run(&format!(
        "$a = Get-Acl -LiteralPath {q}; \
         'OWNER|' + $a.GetOwner([System.Security.Principal.SecurityIdentifier]).Value; \
         'PROTECTED|' + $a.AreAccessRulesProtected; \
         foreach ($r in $a.GetAccessRules($true, $true, [System.Security.Principal.SecurityIdentifier])) \
         {{ 'RULE|' + $r.AccessControlType + '|' + $r.IdentityReference.Value + '|' + [int64]$r.FileSystemRights }}"
    ))?;
    parse_acl_report(&out).ok_or_else(|| format!("unreadable ACL report: {out}"))
}

/// Administrators own it; only SYSTEM and Administrators may write;
/// Authenticated Users may read (the unelevated UI shows history).
#[cfg(windows)]
fn harden(dir: &Path) -> Result<(), String> {
    let d = dir.to_string_lossy();
    crate::ps::exec("icacls.exe", &[&d, "/setowner", "*S-1-5-32-544"])?;
    crate::ps::exec(
        "icacls.exe",
        &[
            &d,
            "/grant:r",
            "*S-1-5-18:(OI)(CI)F",
            "*S-1-5-32-544:(OI)(CI)F",
            "*S-1-5-11:(OI)(CI)RX",
        ],
    )?;
    crate::ps::exec("icacls.exe", &[&d, "/inheritance:r"])?;
    Ok(())
}

#[cfg(windows)]
fn quarantine(dir: &Path) -> Result<(), String> {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let target = dir.with_file_name(format!(
        "{name}.untrusted-{}",
        chrono::Local::now().format("%Y%m%d%H%M%S")
    ));
    std::fs::rename(dir, &target)
        .map_err(|e| format!("moving untrusted {} aside: {e}", dir.display()))
}

/// Makes `dir` an admin-only folder. An existing folder is used only if it is
/// already strict; otherwise it is moved aside (its content may be planted)
/// and a fresh one is created, hardened and checked to be still empty.
#[cfg(windows)]
pub fn ensure_secure_dir(dir: &Path) -> Result<(), String> {
    for _ in 0..3 {
        match std::fs::create_dir(dir) {
            Ok(()) => {
                harden(dir)?;
                // Between create and harden the folder briefly inherited the
                // permissive ProgramData ACL; anything inside now was planted.
                let empty = std::fs::read_dir(dir)
                    .map_err(|e| e.to_string())?
                    .next()
                    .is_none();
                if empty && is_plain_dir(dir) && acl_is_strict(&acl_report(dir)?) {
                    return Ok(());
                }
                quarantine(dir)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if is_plain_dir(dir) && acl_report(dir).is_ok_and(|r| acl_is_strict(&r)) {
                    return Ok(());
                }
                quarantine(dir)?;
            }
            Err(e) => return Err(format!("creating {}: {e}", dir.display())),
        }
    }
    Err(format!(
        "could not establish a protected state folder at {}",
        dir.display()
    ))
}

#[cfg(windows)]
fn stamp() -> String {
    chrono::Local::now().format("%Y%m%d%H%M%S").to_string()
}

/// One-time (per legacy file) migration from the user-writable folder.
/// Failures are logged and leave the legacy data in place; they never block
/// the app.
#[cfg(windows)]
pub fn migrate_legacy(legacy: &Path, secure: &Path) {
    if let Err(e) = migrate_journal(legacy, secure) {
        eprintln!("ad-hyperoptimize: journal migration skipped: {e}");
    }
    migrate_debloater(legacy, secure);
    migrate_small_files(legacy, secure);
}

#[cfg(windows)]
fn migrate_journal(legacy: &Path, secure: &Path) -> Result<(), String> {
    let old = legacy.join("journal.json");
    if !old.exists() {
        return Ok(());
    }
    let legacy_entries = crate::safety::load_journal_at(&old)?;
    let new = secure.join("journal.json");
    let mut entries = if new.exists() {
        crate::safety::load_journal_at(&new)?
    } else {
        Vec::new()
    };
    let (added, untrusted) = merge_legacy(&mut entries, legacy_entries);
    crate::safety::save_journal_at(&new, &entries)?;
    std::fs::rename(
        &old,
        legacy.join(format!("journal.migrated-{}.json", stamp())),
    )
    .map_err(|e| format!("archiving legacy journal: {e}"))?;
    eprintln!("ad-hyperoptimize: migrated {added} journal entries ({untrusted} untrusted)");
    Ok(())
}

/// Debloater state files are restore data for a known tweak: each captured
/// registry value / service must be one the catalog declares for that id.
#[cfg(windows)]
fn migrate_debloater(legacy: &Path, secure: &Path) {
    let src = legacy.join("debloater");
    let Ok(read) = std::fs::read_dir(&src) else {
        return;
    };
    let dst = secure.join("debloater");
    let _ = std::fs::create_dir_all(&dst);
    for file in read.flatten() {
        let path = file.path();
        let Some(id) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
            continue;
        };
        // Write the bytes that were validated: copying the path again would
        // follow a symlink swapped in after the check (the legacy folder is
        // user-writable) and copy an admin-only file into a readable folder.
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let valid = serde_json::from_str::<serde_json::Value>(&content)
            .is_ok_and(|v| crate::debloater::state_is_valid(&id, &v));
        if valid && !dst.join(file.file_name()).exists() {
            let _ = std::fs::write(dst.join(file.file_name()), &content);
        }
    }
    let _ = std::fs::rename(&src, legacy.join(format!("debloater.migrated-{}", stamp())));
}

/// Small single-value state: only copied when it parses as the expected data.
#[cfg(windows)]
fn migrate_small_files(legacy: &Path, secure: &Path) {
    let guid_list = |s: &str| s.trim().split('|').all(crate::ps::is_guid);
    let boost_state = |s: &str| {
        serde_json::from_str::<serde_json::Value>(s).is_ok_and(|v| {
            v["prevPlan"].as_str().is_some_and(crate::ps::is_guid)
                && (v["prevToast"].is_null()
                    || v["prevToast"]
                        .as_str()
                        .is_some_and(|t| t.parse::<u32>().is_ok()))
        })
    };
    type Validator<'a> = &'a dyn Fn(&str) -> bool;
    let files: [(&str, Validator); 3] = [
        ("ultimate_plan_guid.txt", &guid_list),
        (r"gameprofile\prev-plan.txt", &guid_list),
        (r"gameboost\boost-state.json", &boost_state),
    ];
    for (rel, valid) in files {
        let src = legacy.join(rel);
        let Ok(content) = std::fs::read_to_string(&src) else {
            continue;
        };
        let dst = secure.join(rel);
        if valid(&content) && !dst.exists() {
            if let Some(dir) = dst.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&dst, &content);
        }
        let _ = std::fs::rename(&src, src.with_extension(format!("migrated-{}", stamp())));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: &str = "2a737441-1930-4402-8d77-b2bebba308a3";
    const S: &str = "48e6b7a6-50f5-4782-a5d4-53bb8f07e226";

    fn report(owner: &str, protected: bool, rules: &[(&str, &str, i64)]) -> String {
        let mut s = format!("OWNER|{owner}\nPROTECTED|{protected}\n");
        for (kind, sid, rights) in rules {
            s.push_str(&format!("RULE|{kind}|{sid}|{rights}\n"));
        }
        s
    }

    /// Reads (never changes) real ACLs: the report must parse, and a folder
    /// under the user's temp dir (inherited, user-owned) is not strict.
    #[cfg(windows)]
    #[test]
    fn real_acl_reports_parse() {
        let tmp = std::env::temp_dir().join("adho-acl-probe");
        std::fs::create_dir_all(&tmp).unwrap();
        let r = acl_report(&tmp);
        std::fs::remove_dir_all(&tmp).unwrap();
        assert!(!acl_is_strict(&r.unwrap()));
        let pd = program_data_dir().unwrap();
        assert!(pd.is_dir(), "{}", pd.display());
    }

    /// End-to-end check of the elevated path on scratch folders only
    /// (`%ProgramData%\ADHO-selftest-<ts>*`, removed afterwards). Needs an
    /// elevated run: `cargo test --locked -- --ignored secure_dir_end_to_end`.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn secure_dir_end_to_end() {
        assert!(crate::ps::is_admin(), "run elevated");
        let base = program_data_dir().unwrap();
        let name = format!(
            "ADHO-selftest-{}",
            chrono::Local::now().format("%Y%m%d%H%M%S%3f")
        );
        let dir = base.join(&name);
        let cleanup = || {
            for e in std::fs::read_dir(&base).unwrap().flatten() {
                if e.file_name().to_string_lossy().starts_with(&name) {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        };
        let result = std::panic::catch_unwind(|| {
            // 1. Fresh folder: created, hardened, strict.
            ensure_secure_dir(&dir).unwrap();
            assert!(acl_is_strict(&acl_report(&dir).unwrap()));
            std::fs::write(dir.join("journal.json"), "[]").unwrap();
            // 2. Existing strict folder is reused with its content.
            ensure_secure_dir(&dir).unwrap();
            assert!(dir.join("journal.json").exists());
            // 3. A weak pre-existing folder (inherited ProgramData ACL, as an
            //    attacker or an old version would leave it) with planted
            //    content is moved aside and replaced by an empty strict one.
            std::fs::remove_dir_all(&dir).unwrap();
            std::fs::create_dir(&dir).unwrap();
            std::fs::write(dir.join("journal.json"), "planted").unwrap();
            assert!(!acl_is_strict(&acl_report(&dir).unwrap()));
            ensure_secure_dir(&dir).unwrap();
            assert!(acl_is_strict(&acl_report(&dir).unwrap()));
            assert!(std::fs::read_dir(&dir).unwrap().next().is_none());
            let quarantined = std::fs::read_dir(&base).unwrap().flatten().any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(&format!("{name}.untrusted-"))
            });
            assert!(quarantined);
        });
        cleanup();
        result.unwrap();
    }

    /// Dry run of the journal migration against a copy of a real journal
    /// (`ADHO_LEGACY_JOURNAL=<path>`); prints what would be untrusted.
    #[test]
    #[ignore]
    fn dry_run_migration_of_real_journal() {
        let path = std::env::var("ADHO_LEGACY_JOURNAL").expect("ADHO_LEGACY_JOURNAL");
        let legacy = crate::safety::load_journal_at(Path::new(&path)).unwrap();
        let total = legacy.len();
        let mut merged = Vec::new();
        let (added, untrusted) = merge_legacy(&mut merged, legacy);
        println!("entries={total} added={added} untrusted={untrusted}");
        for e in merged.iter().filter(|e| e.untrusted) {
            println!("UNTRUSTED reverted={} [{}]", e.reverted, e.tweak_id);
        }
    }

    #[test]
    fn strict_acl_is_accepted() {
        let r = parse_acl_report(&report(
            SID_ADMINS,
            true,
            &[
                ("Allow", SID_SYSTEM, 2032127),
                ("Allow", SID_ADMINS, 2032127),
                ("Allow", "S-1-5-11", 1179817), // ReadAndExecute, Synchronize
            ],
        ))
        .unwrap();
        assert!(acl_is_strict(&r));
    }

    #[test]
    fn weak_acls_are_rejected() {
        // Default ProgramData inheritance: Users may create files/folders.
        let inherited = parse_acl_report(&report(
            SID_ADMINS,
            false,
            &[("Allow", SID_ADMINS, 2032127), ("Allow", "S-1-5-32-545", 6)],
        ))
        .unwrap();
        assert!(!acl_is_strict(&inherited));
        // Protected, but a user is granted GENERIC_ALL (negative int form too).
        let user_full = parse_acl_report(&report(
            SID_ADMINS,
            true,
            &[("Allow", "S-1-5-21-1-2-3-1001", 268435456)],
        ))
        .unwrap();
        assert!(!acl_is_strict(&user_full));
        // Owned by a normal user (attacker pre-created the folder).
        let user_owned = parse_acl_report(&report(
            "S-1-5-21-1-2-3-1001",
            true,
            &[("Allow", SID_ADMINS, 2032127)],
        ))
        .unwrap();
        assert!(!acl_is_strict(&user_owned));
        // GENERIC_WRITE as a negative int32 must still count as write.
        let generic = parse_acl_report(&report(
            SID_ADMINS,
            true,
            &[("Allow", "S-1-1-0", -1073741824)],
        ))
        .unwrap();
        assert!(!acl_is_strict(&generic));
        assert_eq!(parse_acl_report("garbage"), None);
    }

    #[test]
    fn program_data_is_resolved_from_registry_values() {
        assert_eq!(
            resolve_program_data(r"C:\WINDOWS", r"%SystemDrive%\ProgramData").unwrap(),
            PathBuf::from(r"C:\ProgramData")
        );
        assert!(resolve_program_data(r"C:\WINDOWS", r"%USERPROFILE%\x").is_err());
        assert!(resolve_program_data(r"C:\WINDOWS", r"\\server\share").is_err());
    }

    #[test]
    fn generated_revert_shapes_are_trusted() {
        for (tweak, cmd) in [
            ("x", "powercfg /setactive 381b4222-f694-41f0-9685-ff5bb260df2e"),
            ("x", "powercfg /h on"),
            ("x", "netsh int tcp set global autotuninglevel=normal"),
            (
                "task:\\Opera GX scheduled Autoupdate 1786662128",
                "powershell -Command Enable-ScheduledTask -TaskPath '\\' -TaskName 'Opera GX scheduled Autoupdate 1786662128' -ErrorAction Stop | Out-Null",
            ),
            (
                "privacy:prv_diagtrack_off",
                "powershell -Command Start-Service -Name 'DiagTrack' -ErrorAction Stop",
            ),
            (
                "defenderCloud",
                "powershell -Command Set-MpPreference -MAPSReporting 2 -ErrorAction Stop",
            ),
            (
                "defenderRealtime",
                "powershell -Command Set-MpPreference -DisableRealtimeMonitoring $false -ErrorAction Stop",
            ),
            ("bcd:bcd_standard_policy", "bcdedit /set {default} bootmenupolicy Standard"),
            ("bcd:bcd_numproc_all", "bcdedit /deletevalue {current} numproc"),
            ("bcd:bcd_disable_integrity", "bcdedit /set {current} testsigning no"),
            ("bcd:bcd_no_nx_alwaysoff", "bcdedit /set {current} nx OptIn"),
        ] {
            assert_eq!(trusted_revert(tweak, cmd, &[]).as_deref(), Some(cmd), "{cmd}");
        }
        // Legacy quoted power-index form is upgraded to the current form.
        let legacy = format!(
            "powershell -Command \"powercfg /setacvalueindex {G} {S} {S} 1; if ($LASTEXITCODE -eq 0) {{ powercfg /setdcvalueindex {G} {S} {S} 1; if ($LASTEXITCODE -eq 0) {{ powercfg /setactive scheme_current }} }}\""
        );
        assert_eq!(
            trusted_revert("usb_suspend_off", &legacy, &[]),
            Some(crate::tweaks::power_index_revert(G, S, S, "1", "1"))
        );
        // Catalog static reverts are trusted by exact match only.
        assert!(trusted_revert("t", "powercfg /setactive scheme_current", &[]).is_some());
        assert!(trusted_revert("t", "something odd", &["something odd"]).is_some());
    }

    #[test]
    fn planted_commands_are_not_trusted() {
        for cmd in [
            "powershell -Command Start-Process calc",
            "cmd /c whoami",
            "powercfg /setactive 381b4222-f694-41f0-9685-ff5bb260df2e & calc",
            "powershell -Command Start-Service -Name 'Spooler' -ErrorAction Stop",
            "powershell -Command Enable-ScheduledTask -TaskPath '\\' -TaskName 'x'; calc; '' -ErrorAction Stop | Out-Null",
            "bcdedit /set {current} testsigning on;calc",
            "netsh int tcp set global autotuninglevel=normal; calc",
            // Old privacy items were never runnable (no powershell prefix).
            "Set-ItemProperty -Path 'HKCU:\\Software\\X' -Name Y -Value 1 -Type DWord",
        ] {
            assert_eq!(trusted_revert("x", cmd, &[]), None, "{cmd}");
        }
        // Valid shapes, but not tied to the entry's own tweak, or lowering
        // security when run elevated from planted data.
        for (tweak, cmd) in [
            ("bcd:bcd_disable_integrity", "bcdedit /set {current} testsigning on"),
            ("bcd:bcd_disable_integrity", "bcdedit /set {current} testsigning yes"),
            ("bcd:bcd_no_nx_alwaysoff", "bcdedit /set {current} nx AlwaysOff"),
            ("bcd:bcd_timeout_0", "bcdedit /set {current} nointegritychecks on"),
            ("bcd:bcd_timeout_0", "bcdedit /set {current} timeout 5"),
            ("x", "bcdedit /deletevalue {current} numproc"),
            (
                "defenderRealtime",
                "powershell -Command Set-MpPreference -DisableRealtimeMonitoring $true -ErrorAction Stop",
            ),
            (
                "defenderCloud",
                "powershell -Command Set-MpPreference -MAPSReporting 0 -ErrorAction Stop",
            ),
            (
                "x",
                "powershell -Command Set-MpPreference -MAPSReporting 2 -ErrorAction Stop",
            ),
            (
                "task:\\Microsoft\\Windows\\Windows Defender\\Windows Defender Scheduled Scan",
                "powershell -Command Disable-ScheduledTask -TaskPath '\\Microsoft\\Windows\\Windows Defender\\' -TaskName 'Windows Defender Scheduled Scan' -ErrorAction Stop | Out-Null",
            ),
            (
                "task:\\Other",
                "powershell -Command Enable-ScheduledTask -TaskPath '\\' -TaskName 'x' -ErrorAction Stop | Out-Null",
            ),
            (
                "privacy:prv_wer_off",
                "powershell -Command Start-Service -Name 'DiagTrack' -ErrorAction Stop",
            ),
        ] {
            assert_eq!(trusted_revert(tweak, cmd, &[]), None, "{tweak} {cmd}");
        }
    }

    #[test]
    fn merge_marks_entries_with_unknown_commands_untrusted() {
        let entry = |id: &str, revert: &str| JournalEntry {
            id: id.into(),
            tweak_id: "auto_play_off".into(),
            tweak_name: "x".into(),
            origin: None,
            time: String::new(),
            items: vec![
                ChangeItem::Registry {
                    root: "HKCU".into(),
                    path: r"Software\Microsoft\Windows\CurrentVersion\Explorer\AutoplayHandlers"
                        .into(),
                    name: "DisableAutoplay".into(),
                    prev: None,
                    new: crate::safety::RegVal::Dword(1),
                },
                ChangeItem::Command {
                    applied: String::new(),
                    revert: revert.into(),
                },
            ],
            reverted: false,
            backup_files: vec![],
            completed: true,
            attempted: 2,
            reverted_items: 0,
            untrusted: false,
        };
        let mut secure = vec![entry("keep", "powercfg /h on")];
        let legacy = vec![
            entry("keep", "cmd /c evil"), // duplicate id: secure copy wins
            entry("ok", "powercfg /h off"),
            entry("bad", "cmd /c evil"),
        ];
        assert_eq!(merge_legacy(&mut secure, legacy), (2, 1));
        let get = |id: &str| secure.iter().find(|e| e.id == id).unwrap();
        assert!(!get("keep").untrusted);
        assert!(!get("ok").untrusted);
        assert!(get("bad").untrusted);
    }

    fn single_item_entry(tweak_id: &str, item: ChangeItem) -> JournalEntry {
        JournalEntry {
            id: "e".into(),
            tweak_id: tweak_id.into(),
            tweak_name: "x".into(),
            origin: None,
            time: String::new(),
            items: vec![item],
            reverted: false,
            backup_files: vec![],
            completed: true,
            attempted: 1,
            reverted_items: 0,
            untrusted: false,
        }
    }

    fn reg(root: &str, path: &str, name: &str) -> ChangeItem {
        ChangeItem::Registry {
            root: root.into(),
            path: path.into(),
            name: name.into(),
            prev: Some(crate::safety::RegVal::Str("payload".into())),
            new: crate::safety::RegVal::Dword(1),
        }
    }

    fn svc(name: &str) -> ChangeItem {
        ChangeItem::ServiceStartup {
            service: name.into(),
            prev: "Disabled".into(),
            new: "Manual".into(),
        }
    }

    #[test]
    fn planted_registry_and_service_targets_are_not_trusted() {
        let ifeo =
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\sethc.exe";
        let approved = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
        for (tweak, item) in [
            // Any HKLM value under a known tweak id.
            ("game_dvr_off", reg("HKLM", ifeo, "Debugger")),
            ("privacy:prv_telemetry_basic", reg("HKLM", ifeo, "Debugger")),
            ("gpu:x", reg("HKLM", ifeo, "Debugger")),
            ("nvCtrlPanel:x", reg("HKLM", ifeo, "Debugger")),
            (
                "perftweaks:timer",
                reg(
                    "HKLM",
                    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
                    "x",
                ),
            ),
            // Startup entry: the value name must be the recorded entry.
            ("startup:hkcu_run:Steam", reg("HKCU", approved, "Other")),
            ("startup:bogus:Steam", reg("HKCU", approved, "Steam")),
            ("unknown_tweak", reg("HKCU", r"Software\X", "Y")),
            // Services: only the ones the feature declares.
            ("game_dvr_off", svc("WinDefend")),
            ("privacy:prv_diagtrack_off", svc("WinDefend")),
            ("service:wuauserv", svc("wuauserv")),
        ] {
            let mut e = single_item_entry(tweak, item);
            assert!(!sanitize_entry(&mut e), "{tweak}");
            assert!(e.untrusted, "{tweak}");
        }
    }

    #[test]
    fn declared_registry_and_service_targets_stay_trusted() {
        let approved = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
        for (tweak, item) in [
            ("startup:hkcu_run:Steam", reg("HKCU", approved, "Steam")),
            (
                "perftweaks:timer",
                reg(
                    "HKLM",
                    r"SYSTEM\CurrentControlSet\Control\Session Manager\kernel",
                    "GlobalTimerResolutionRequests",
                ),
            ),
            (
                "privacy:prv_telemetry_basic",
                reg(
                    "HKLM",
                    r"SOFTWARE\Policies\Microsoft\Windows\DataCollection",
                    "AllowTelemetry",
                ),
            ),
            ("privacy:prv_diagtrack_off", svc("DiagTrack")),
            ("svc_fax_off", svc("Fax")),
        ] {
            let mut e = single_item_entry(tweak, item);
            assert!(sanitize_entry(&mut e), "{tweak}");
        }
        // Every catalog registry target of every tweak is accepted.
        for id in crate::tweaks::catalog_ids() {
            for (r, p, n) in crate::tweaks::declared_registry(id) {
                assert!(registry_declared(id, r, p, n), "{id} {p}\\{n}");
            }
        }
    }
}
