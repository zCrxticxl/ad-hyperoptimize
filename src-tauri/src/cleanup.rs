//! Cleanup engine. Scan is read-only; deletion only runs on categories the
//! user explicitly selected, only inside hardcoded whitelisted roots, and
//! skips anything locked or younger than `min_age_hours`.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

struct Category {
    id: &'static str,
    name: &'static str,
    note: &'static str,
    min_age_hours: u64,
}

const CATS: &[Category] = &[
    Category {
        id: "win_temp",
        name: "Windows Temp",
        note: "C:\\Windows\\Temp, safe to clear; in-use files are skipped automatically.",
        min_age_hours: 24,
    },
    Category {
        id: "user_temp",
        name: "User Temp",
        note: "%TEMP%, safe to clear; in-use files are skipped automatically.",
        min_age_hours: 24,
    },
    Category {
        id: "wu_cache",
        name: "Windows Update download cache",
        note: "SoftwareDistribution\\Download, already-installed update payloads.",
        min_age_hours: 72,
    },
    Category {
        id: "dx_shader",
        name: "DirectX shader cache",
        note: "Rebuilt on demand; first game launches recompile shaders (brief stutter once).",
        min_age_hours: 0,
    },
    Category {
        id: "thumb_cache",
        name: "Thumbnail cache",
        note: "Explorer rebuilds thumbnails as folders are opened.",
        min_age_hours: 0,
    },
    Category {
        id: "crash_dumps",
        name: "Crash dumps (WER)",
        note: "Old crash reports. Keep if you're actively debugging a problem.",
        min_age_hours: 0,
    },
    Category {
        id: "chrome_cache",
        name: "Chrome HTTP cache",
        note: "Close Chrome first. Pages reload slower once.",
        min_age_hours: 0,
    },
    Category {
        id: "edge_cache",
        name: "Edge HTTP cache",
        note: "Close Edge first. Pages reload slower once.",
        min_age_hours: 0,
    },
];

/// Cleanup roots of a category: (path, per-user). Machine roots are built
/// from the Windows API / HKLM, never from the environment: the elevated app
/// inherits `%WINDIR%`/`%TEMP%` from its caller, and `HKCU\Environment`
/// (user-writable) could point "User Temp" at `C:\Windows\System32`.
fn cat_roots(id: &str) -> Vec<(PathBuf, bool)> {
    let machine = |p: PathBuf| (p, false);
    let user = |p: PathBuf| (p, true);
    match id {
        "win_temp" | "wu_cache" => cat_paths(id).into_iter().map(machine).collect(),
        "crash_dumps" => {
            let mut v = cat_paths(id);
            let queue = v.pop();
            v.into_iter().map(user).chain(queue.map(machine)).collect()
        }
        _ => cat_paths(id).into_iter().map(user).collect(),
    }
}

fn cat_paths(id: &str) -> Vec<PathBuf> {
    let local = dirs::cache_dir().unwrap_or_default(); // %LOCALAPPDATA%
    let windir = crate::ps::windows_dir();
    #[cfg(windows)]
    let program_data = crate::statestore::program_data_dir().unwrap_or_default();
    #[cfg(not(windows))]
    let program_data = PathBuf::from("C:\\ProgramData");
    match id {
        "win_temp" => vec![PathBuf::from(format!("{windir}\\Temp"))],
        "user_temp" => vec![std::env::temp_dir()],
        "wu_cache" => vec![PathBuf::from(format!(
            "{windir}\\SoftwareDistribution\\Download"
        ))],
        "dx_shader" => vec![
            local.join("D3DSCache"),
            local.join("NVIDIA\\DXCache"),
            local.join("AMD\\DxCache"),
        ],
        "thumb_cache" => vec![local.join("Microsoft\\Windows\\Explorer")],
        "crash_dumps" => vec![
            local.join("CrashDumps"),
            program_data.join("Microsoft\\Windows\\WER\\ReportQueue"),
        ],
        "chrome_cache" => vec![local.join("Google\\Chrome\\User Data\\Default\\Cache")],
        "edge_cache" => vec![local.join("Microsoft\\Edge\\User Data\\Default\\Cache")],
        _ => vec![],
    }
}

fn walk(root: &Path, min_age: Duration, files: &mut Vec<(PathBuf, u64)>) {
    // Iterative BFS: an explicit stack instead of recursion, so hostile
    // deep temp trees cannot overflow the call stack.
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if crate::diskanalyzer::is_reparse_point(&dir) {
            continue;
        }
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        let now = SystemTime::now();
        for entry in rd.flatten() {
            let p = entry.path();
            if crate::diskanalyzer::is_reparse_point(&p) {
                continue;
            }
            let Ok(md) = p.symlink_metadata() else {
                continue;
            };
            if md.is_dir() {
                stack.push(p);
            } else if md.is_file() {
                let old_enough = md
                    .modified()
                    .ok()
                    .and_then(|m| now.duration_since(m).ok())
                    .map(|age| age >= min_age)
                    .unwrap_or(false);
                if old_enough || min_age.is_zero() {
                    files.push((p, md.len()));
                }
            }
        }
    }
}

/// True when `root` (canonical) lies strictly inside `home` (canonical).
fn is_inside(root: &Path, home: &Path) -> bool {
    root != home && root.starts_with(home)
}

/// A per-user root must resolve to a folder inside this user's profile (the
/// profile path comes from the account, not from redirectable variables);
/// machine roots are fixed. Anything else is never walked or cleaned.
fn root_allowed(root: &Path, per_user: bool) -> bool {
    if !per_user {
        return true;
    }
    let Some(home) = dirs::home_dir() else {
        return false;
    };
    match (fs::canonicalize(root), fs::canonicalize(home)) {
        (Ok(r), Ok(h)) => is_inside(&r, &h) && !crate::diskanalyzer::is_protected(root),
        _ => false,
    }
}

fn collect(id: &str, min_age_hours: u64) -> Vec<(PathBuf, u64)> {
    let mut files = Vec::new();
    for (root, per_user) in cat_roots(id) {
        if root.exists() && root_allowed(&root, per_user) {
            walk(&root, Duration::from_secs(min_age_hours * 3600), &mut files);
        }
    }
    files
}

/// Read-only scan: what could be freed, per category.
pub fn scan() -> Value {
    let out: Vec<Value> = CATS
        .iter()
        .map(|c| {
            let files = collect(c.id, c.min_age_hours);
            let bytes: u64 = files.iter().map(|(_, s)| s).sum();
            json!({
                "id": c.id, "name": c.name, "note": c.note,
                "fileCount": files.len(), "bytes": bytes,
                "paths": cat_paths(c.id).iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!(out)
}

/// Delete files in the selected categories. Locked/in-use files are skipped.
pub fn clean(category_ids: Vec<String>) -> Value {
    let mut freed: u64 = 0;
    let mut deleted = 0usize;
    let mut skipped = 0usize;
    for id in &category_ids {
        let Some(c) = CATS.iter().find(|c| c.id == id) else {
            continue;
        };
        for (root, per_user) in cat_roots(c.id) {
            if !root.exists() || !root_allowed(&root, per_user) {
                continue;
            }
            let mut files = Vec::new();
            walk(
                &root,
                Duration::from_secs(c.min_age_hours * 3600),
                &mut files,
            );
            for (path, size) in files {
                match safe_delete::delete_file_under(&path, &root) {
                    Ok(()) => {
                        freed += size;
                        deleted += 1;
                    }
                    // locked, permission, or the path no longer resolves to
                    // a plain file inside the root: never force
                    Err(_) => skipped += 1,
                }
            }
            // Sweep now-empty subdirectories (ignore failures).
            let _ = remove_empty_dirs(&root, false);
        }
    }
    json!({ "freedBytes": freed, "deleted": deleted, "skippedInUse": skipped })
}

/// Race-free single-file deletion for the elevated cleaner.
///
/// Roots such as `C:\Windows\Temp` are writable by unprivileged users, so a
/// path-based check followed by `remove_file` is a TOCTOU window: a local
/// user can swap a parent directory for a junction between the check and
/// the delete and make the elevated process delete a protected file. Here
/// the file is opened ONCE by handle (`FILE_FLAG_OPEN_REPARSE_POINT`, so a
/// final-component link is opened itself, never followed), every check runs
/// on that handle, and the delete is issued on the same handle:
/// 1. `GetFileInformationByHandle`: must be a plain, single-link file (no
///    reparse point, no directory, no hard link to another location);
/// 2. `GetFinalPathNameByHandleW`: the object actually opened must still lie
///    strictly under the canonical cleanup root (catches junctions planted in
///    any intermediate component);
/// 3. `SetFileInformationByHandle(FileDispositionInfo)`: deletes exactly the
///    object that was verified.
///
/// The Win32 calls are declared directly (same pattern as the PDH bindings
/// in monitor.rs): Cargo.toml has no `windows`/`windows-sys` dependency.
#[cfg(windows)]
mod safe_delete {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    type Handle = *mut c_void;
    const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;

    const DELETE: u32 = 0x0001_0000;
    const FILE_READ_ATTRIBUTES: u32 = 0x0000_0080;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_SHARE_DELETE: u32 = 0x4;
    const OPEN_EXISTING: u32 = 3;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x0000_0010;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    /// FILE_INFO_BY_HANDLE_CLASS::FileDispositionInfo
    const FILE_DISPOSITION_INFO_CLASS: i32 = 4;
    /// FILE_NAME_NORMALIZED | VOLUME_NAME_DOS
    const FINAL_PATH_FLAGS: u32 = 0;

    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    /// BY_HANDLE_FILE_INFORMATION
    #[repr(C)]
    #[derive(Default)]
    struct ByHandleFileInformation {
        file_attributes: u32,
        creation_time: FileTime,
        last_access_time: FileTime,
        last_write_time: FileTime,
        volume_serial_number: u32,
        file_size_high: u32,
        file_size_low: u32,
        number_of_links: u32,
        file_index_high: u32,
        file_index_low: u32,
    }

    /// FILE_DISPOSITION_INFO (`BOOLEAN DeleteFile`)
    #[repr(C)]
    struct FileDispositionInfo {
        delete_file: u8,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *mut c_void,
            disposition: u32,
            flags: u32,
            template: Handle,
        ) -> Handle;
        fn GetFileInformationByHandle(h: Handle, info: *mut ByHandleFileInformation) -> i32;
        fn GetFinalPathNameByHandleW(h: Handle, buf: *mut u16, len: u32, flags: u32) -> u32;
        fn SetFileInformationByHandle(h: Handle, class: i32, info: *const c_void, size: u32)
            -> i32;
        fn CloseHandle(h: Handle) -> i32;
    }

    struct Owned(Handle);
    impl Drop for Owned {
        fn drop(&mut self) {
            // SAFETY: self.0 is a valid handle returned by CreateFileW.
            unsafe { CloseHandle(self.0) };
        }
    }

    fn wide(p: &Path) -> Vec<u16> {
        p.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    fn final_path(h: &Owned) -> Option<String> {
        let mut buf = vec![0u16; 1024];
        loop {
            // SAFETY: buf is a writable buffer of buf.len() u16s.
            let n = unsafe {
                GetFinalPathNameByHandleW(h.0, buf.as_mut_ptr(), buf.len() as u32, FINAL_PATH_FLAGS)
            } as usize;
            if n == 0 {
                return None;
            }
            if n < buf.len() {
                return Some(String::from_utf16_lossy(&buf[..n]));
            }
            if n > 32_768 {
                return None;
            }
            buf.resize(n + 1, 0);
        }
    }

    /// Normalise a `\\?\C:\…` final/canonical path for comparison.
    fn norm(p: &str) -> String {
        p.strip_prefix(r"\\?\")
            .unwrap_or(p)
            .trim_end_matches('\\')
            .to_lowercase()
    }

    /// True when `final_path` lies strictly below `root` (both as returned
    /// by GetFinalPathNameByHandleW / canonicalize).
    pub(super) fn is_strictly_under(final_path: &str, root: &str) -> bool {
        let (f, r) = (norm(final_path), norm(root));
        // A `\\?\UNC\…` root or target never qualifies.
        if f.starts_with("unc\\") || r.starts_with("unc\\") || r.is_empty() {
            return false;
        }
        f.len() > r.len() + 1 && f.starts_with(&r) && f.as_bytes()[r.len()] == b'\\'
    }

    pub fn delete_file_under(path: &Path, root: &Path) -> Result<(), String> {
        let root_canon = std::fs::canonicalize(root).map_err(|e| format!("root: {e}"))?;
        let root_canon = root_canon.to_string_lossy().into_owned();
        let name = wide(path);
        // SAFETY: name is NUL-terminated; all other pointers are null/valid.
        let raw = unsafe {
            CreateFileW(
                name.as_ptr(),
                DELETE | FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
                std::ptr::null_mut(),
            )
        };
        if raw == INVALID_HANDLE_VALUE || raw.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let h = Owned(raw);

        let mut info = ByHandleFileInformation::default();
        // SAFETY: info is a valid, writable BY_HANDLE_FILE_INFORMATION.
        if unsafe { GetFileInformationByHandle(h.0, &mut info) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if info.file_attributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) != 0 {
            return Err("not a plain file".into());
        }
        if info.number_of_links != 1 {
            return Err("hard-linked file".into());
        }

        let fp = final_path(&h).ok_or("cannot resolve final path")?;
        if !is_strictly_under(&fp, &root_canon) {
            return Err(format!("{fp} is outside the cleanup root"));
        }

        let disp = FileDispositionInfo { delete_file: 1 };
        // SAFETY: disp is a valid FILE_DISPOSITION_INFO of the given size.
        let ok = unsafe {
            SetFileInformationByHandle(
                h.0,
                FILE_DISPOSITION_INFO_CLASS,
                &disp as *const FileDispositionInfo as *const c_void,
                std::mem::size_of::<FileDispositionInfo>() as u32,
            )
        };
        if ok == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
}

/// Non-Windows builds (unit tests on dev hosts): plain path-based delete
/// with the reparse-ancestor guard; the elevated TOCTOU threat only exists
/// on Windows.
#[cfg(not(windows))]
mod safe_delete {
    use std::path::Path;

    pub fn delete_file_under(path: &Path, root: &Path) -> Result<(), String> {
        if !path.starts_with(root) || crate::diskanalyzer::has_reparse_ancestor(path) {
            return Err("outside root or behind a link".into());
        }
        std::fs::remove_file(path).map_err(|e| e.to_string())
    }
}

fn remove_empty_dirs(dir: &Path, remove_self: bool) -> std::io::Result<bool> {
    if crate::diskanalyzer::is_reparse_point(dir) {
        return Ok(false);
    }
    let mut empty = true;
    for entry in fs::read_dir(dir)? {
        let p = entry?.path();
        if crate::diskanalyzer::is_reparse_point(&p) {
            empty = false;
            continue;
        }
        if p.is_dir() {
            if !remove_empty_dirs(&p, true).unwrap_or(false) {
                empty = false;
            }
        } else {
            empty = false;
        }
    }
    if empty && remove_self {
        fs::remove_dir(dir)?;
    }
    Ok(empty)
}

#[cfg(all(test, windows))]
mod tests {
    use super::walk;
    use std::fs;
    use std::process::Command;
    use std::time::Duration;

    #[test]
    fn cleanup_scan_does_not_follow_junctions() {
        let root = std::env::temp_dir().join(format!(
            "ad-hyperoptimize-cleanup-test-{}",
            rand::random::<u64>()
        ));
        let scan_root = root.join("scan");
        let outside = root.join("outside");
        let junction = scan_root.join("junction");
        fs::create_dir_all(&scan_root).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("keep.txt"), "keep").unwrap();

        let output = Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .output()
            .unwrap();
        if !output.status.success() {
            fs::remove_dir_all(&root).unwrap();
            panic!(
                "junction creation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let mut files = Vec::new();
        walk(&scan_root, Duration::ZERO, &mut files);
        fs::remove_dir(&junction).unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn final_path_containment() {
        use super::safe_delete::is_strictly_under;
        let root = r"\\?\C:\Windows\Temp";
        assert!(is_strictly_under(r"\\?\C:\Windows\Temp\a.tmp", root));
        assert!(is_strictly_under(r"\\?\C:\WINDOWS\TEMP\sub\a.tmp", root));
        assert!(!is_strictly_under(r"\\?\C:\Windows\Temp", root));
        assert!(!is_strictly_under(r"\\?\C:\Windows\TempX\a.tmp", root));
        assert!(!is_strictly_under(
            r"\\?\C:\Windows\System32\drivers\x.sys",
            root
        ));
        assert!(!is_strictly_under(r"\\?\UNC\srv\share\a", root));
        assert!(!is_strictly_under(r"\\?\C:\a", ""));
    }

    #[test]
    fn handle_delete_stays_inside_root_and_refuses_junction_escape() {
        use super::safe_delete::delete_file_under;
        let base = std::env::temp_dir().join(format!(
            "ad-hyperoptimize-safedel-test-{}",
            rand::random::<u64>()
        ));
        let root = base.join("root");
        let outside = base.join("outside");
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let inside_file = root.join("sub").join("old.tmp");
        fs::write(&inside_file, "x").unwrap();
        let keep = outside.join("keep.txt");
        fs::write(&keep, "keep").unwrap();

        // A plain file inside the root is deleted.
        delete_file_under(&inside_file, &root).unwrap();
        assert!(!inside_file.exists());

        // A file reached through a junction inside the root resolves
        // outside it and must be refused.
        let junction = root.join("j");
        let ok = Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .output()
            .unwrap()
            .status
            .success();
        let via_junction = junction.join("keep.txt");
        let escaped = ok && delete_file_under(&via_junction, &root).is_ok();
        // The junction itself is a reparse point, never deleted as a file.
        let junction_deleted = ok && delete_file_under(&junction, &root).is_ok();
        let keep_exists = keep.exists();
        if ok {
            fs::remove_dir(&junction).unwrap();
        }
        // A file outside the root is refused even by direct path.
        let direct = delete_file_under(&keep, &root).is_ok();
        fs::remove_dir_all(&base).unwrap();

        assert!(ok, "junction creation failed");
        assert!(!escaped, "deleted a file through a junction");
        assert!(!junction_deleted, "deleted a junction as a file");
        assert!(keep_exists);
        assert!(!direct);
    }
}

#[cfg(test)]
mod root_tests {
    use super::{cat_paths, cat_roots, is_inside, CATS};
    use std::path::Path;

    #[test]
    fn per_user_roots_must_stay_inside_the_profile() {
        let home = Path::new("Users").join("me");
        let temp = home.join("AppData").join("Local").join("Temp");
        assert!(is_inside(&temp, &home));
        assert!(!is_inside(&home, &home));
        assert!(!is_inside(&Path::new("Windows").join("System32"), &home));
        assert!(!is_inside(
            &Path::new("Users").join("me2").join("Temp"),
            &home
        ));
    }

    #[test]
    fn every_category_root_is_classified() {
        for c in CATS {
            assert_eq!(cat_roots(c.id).len(), cat_paths(c.id).len(), "{}", c.id);
        }
        // Machine roots: Windows Temp, the update cache and the WER queue.
        let machine = |id: &str| cat_roots(id).iter().filter(|(_, u)| !u).count();
        assert_eq!(machine("win_temp"), 1);
        assert_eq!(machine("wu_cache"), 1);
        assert_eq!(machine("crash_dumps"), 1);
        assert_eq!(machine("user_temp"), 0);
    }
}
