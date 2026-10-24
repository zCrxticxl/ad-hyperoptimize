//! Disk Analyzer: largest files + folders, duplicate detection, temp-file age.
//! All filesystem walks are iterative (no recursion) to avoid stack overflow.

use rayon::prelude::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tauri::Emitter as _;

// ── constants ───────────────────────────────────────────────────────────────

/// Directory names (lower-case) that are always skipped during walks.
const SKIP_DIRS: &[&str] = &[
    "$recycle.bin",
    "system volume information",
    "winsxs", // Windows component store, huge, untouchable
    "softwaredistribution",
    ".git",
    "node_modules",
    "target", // Rust build artifacts
    "__pycache__",
    ".cargo",
];

/// Minimum file size for duplicate detection (100 KB, hashing tiny files wastes time).
const DUP_MIN_BYTES: u64 = 100 * 1024;

/// Maximum files to walk per drive scan (safety valve for drives with millions of files).
const MAX_FILES: usize = 400_000;

// ── internal types ──────────────────────────────────────────────────────────

struct FileInfo {
    path: PathBuf,
    name: String,
    size: u64,
    ext: String,
    modified: u64, // Unix secs
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn modified_secs(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn fmt_size(bytes: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

/// Iterative BFS walk. Skips symlinks and SKIP_DIRS.
#[cfg(windows)]
pub(crate) fn is_reparse_point(p: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    p.symlink_metadata()
        .map(|m| m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
        .unwrap_or(false)
}
#[cfg(not(windows))]
pub(crate) fn is_reparse_point(_p: &Path) -> bool {
    false
}

pub(crate) fn has_reparse_ancestor(path: &Path) -> bool {
    path.ancestors().any(is_reparse_point)
}

fn walk(root: &Path) -> Vec<FileInfo> {
    let mut stack = vec![root.to_path_buf()];
    let mut out = Vec::new();

    while let Some(dir) = stack.pop() {
        if out.len() >= MAX_FILES {
            break;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            // symlink_metadata: do NOT follow symlinks/junctions, a reparse
            // point inside the scanned tree must never pull system paths in.
            let Ok(meta) = entry.path().symlink_metadata() else {
                continue;
            };
            let path = entry.path();
            let lname = path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            if SKIP_DIRS.iter().any(|&s| lname == s) {
                continue;
            }

            if meta.is_dir() {
                // A junction's symlink_metadata reports is_dir()==true; the
                // reparse attribute is what actually distinguishes it.
                if !is_reparse_point(&path) {
                    stack.push(path);
                }
            } else if meta.is_file() {
                if out.len() >= MAX_FILES {
                    // Cap reached: the collected set cannot grow anymore, so
                    // stop walking instead of enumerating the rest of the
                    // drive for nothing.
                    break;
                }
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let ext = path
                    .extension()
                    .map(|e| e.to_string_lossy().to_uppercase().to_owned())
                    .unwrap_or_default();
                out.push(FileInfo {
                    path,
                    name,
                    size: meta.len(),
                    ext,
                    modified: modified_secs(&meta),
                });
            }
        }
    }
    out
}

/// Bytes hashed in the cheap first duplicate pass.
const DUP_PREFIX_BYTES: u64 = 64 * 1024;

fn hash_file(path: &Path) -> Option<String> {
    hash_file_limited(path, u64::MAX)
}

/// SHA-256 of the first `limit` bytes of `path` (the whole file when it is
/// shorter).
fn hash_file_limited(path: &Path, limit: u64) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?.take(limit);
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 65_536];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Some(format!("{:x}", h.finalize()))
}

// ── file operations ─────────────────────────────────────────────────────────

/// True when a path must never be deleted/moved by the disk analyzer.
///
/// Paths produced by the scan are absolute `X:\…` paths without `.`/`..`
/// segments. Everything else, relative paths, drive-relative paths, `..`
/// traversal, system roots, the app's own data directory and whole user
/// profiles, is rejected. String-based so it behaves identically on Linux
/// (unit tests) and Windows (runtime).
pub(crate) fn is_protected(path: &Path) -> bool {
    is_protected_inner(path, false)
}

/// Destination-side variant: allows bare drive roots (`D:\`) because the UI
/// offers them as move targets, but still rejects every protected location.
fn is_protected_dest(path: &Path) -> bool {
    is_protected_inner(path, true)
}

fn is_protected_inner(path: &Path, allow_root: bool) -> bool {
    // The string check alone is bypassable by aliases that resolve to a
    // protected location (junctions/symlinks, 8.3 names, case/normalisation
    // tricks). When the path exists, resolve it to its canonical long path
    // and check that too; the canonical form already expands 8.3 aliases, so
    // legitimate names like `Report~1.docx` are not refused. A path that
    // does not exist cannot be resolved, so 8.3-looking segments are refused.
    match std::fs::canonicalize(path) {
        Ok(canon) => match strip_verbatim_prefix(&canon) {
            Some(c) => {
                is_protected_str(path, allow_root, false)
                    || is_protected_str(Path::new(&c), allow_root, false)
            }
            None => true, // resolves to a UNC/device path, never a scan target
        },
        Err(_) => is_protected_str(path, allow_root, true),
    }
}

/// `std::fs::canonicalize` returns `\\?\C:\…` on Windows; strip it back to
/// `C:\…`. `\\?\UNC\…` and other verbatim device forms yield `None`.
fn strip_verbatim_prefix(p: &Path) -> Option<String> {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if rest.len() >= 2 && rest.as_bytes()[1] == b':' => Some(rest.to_string()),
        Some(_) => None,
        None => Some(s.into_owned()),
    }
}

fn is_protected_str(path: &Path, allow_root: bool, reject_short_names: bool) -> bool {
    let raw = path.to_string_lossy().replace('/', "\\");

    // `.` / `..` segments never come from the scanner.
    if raw.split('\\').any(|seg| seg == ".." || seg == ".") {
        return true;
    }

    // NTFS stream / attribute syntax (`C:\Windows::$INDEX_ALLOCATION`,
    // `file:stream`): no `:` may follow the drive letter.
    if raw.char_indices().any(|(i, c)| c == ':' && i != 1) {
        return true;
    }

    // 8.3 short names (`C:\PROGRA~1`, `C:\WINDOW~1`) alias long names the
    // prefix list below would otherwise miss. Only applied to paths that
    // could not be canonicalized (see is_protected_inner).
    if reject_short_names
        && raw.split('\\').any(|seg| {
            let b = seg.as_bytes();
            b.windows(2).any(|w| w[0] == b'~' && w[1].is_ascii_digit())
        })
    {
        return true;
    }

    // Win32 silently strips trailing dots/spaces from every path component
    // ("C:\Windows " opens C:\Windows). Normalize so such forms cannot
    // bypass the checks below.
    let norm: String = raw
        .split('\\')
        .map(|seg| seg.trim_end_matches([' ', '.']))
        .collect::<Vec<_>>()
        .join("\\");
    let norm_upper = norm.to_uppercase();

    // Only absolute drive paths (`X:\…`) are acceptable; anything else is
    // relative or drive-relative and cannot come from a scan.
    let b = norm.as_bytes();
    let is_abs_win = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\';
    if !is_abs_win {
        return true;
    }
    if b.len() == 3 {
        return !allow_root; // bare drive root (C:\)
    }

    // System-critical + app-owned locations (prefix match on the path
    // relative to the drive root, case-insensitive).
    const PROTECTED_PREFIXES: &[&str] = &[
        "\\WINDOWS",             // system32, winsxs, …
        "\\PROGRAM FILES",       // covers "Program Files\…"
        "\\PROGRAM FILES (X86)", // covers "Program Files (x86)\…"
        "\\PROGRAMDATA",
        "\\$RECYCLE.BIN",
        "\\SYSTEM VOLUME INFORMATION",
        "\\PERFLOGS",
        "\\RECOVERY",
        "\\WINDOWS.OLD",
        "\\EFI",
        "\\BOOT",
        "\\CONFIG.MSI",
        "\\PCOPTSUITE", // journal / cache / backups, never self-delete
    ];
    // `X:\…` → keep the backslash: index 2 is `\`, so [2..] is `\…`.
    let rest = &norm_upper[2..];
    if PROTECTED_PREFIXES
        .iter()
        .any(|p| rest == *p || rest.starts_with(&format!("{p}\\").to_string()))
    {
        return true;
    }

    // Whole user profiles and their data roots: `\Users`, `\Users\<name>`,
    // `\Users\<name>\AppData`, `…\AppData\Roaming|Local|LocalLow`, deleting
    // any of these is never a scan-level action, yet the folder scan
    // surfaces exactly these roots. Deeper paths (e.g. Temp files) stay
    // user-controllable.
    let comps: Vec<&str> = norm[3..].split('\\').filter(|s| !s.is_empty()).collect();
    if comps.is_empty() {
        return true;
    }
    if comps.len() == 1 && comps[0].eq_ignore_ascii_case("users") {
        return true;
    }
    if comps.len() == 2 && comps[0].eq_ignore_ascii_case("users") {
        return true;
    }
    if comps.len() >= 3
        && comps[0].eq_ignore_ascii_case("users")
        && comps[2].eq_ignore_ascii_case("appdata")
        && (comps.len() == 3
            || (comps.len() == 4
                && matches!(
                    comps[3].to_uppercase().as_str(),
                    "ROAMING" | "LOCAL" | "LOCALLOW"
                )))
    {
        return true;
    }
    // The app's own data dir lives under `\Users\<name>\AppData\…\PCOptSuite`
    // (Roaming/Local), never self-delete journal, cache or backups.
    if comps[0].eq_ignore_ascii_case("users")
        && comps.iter().any(|c| c.eq_ignore_ascii_case("pcoptsuite"))
    {
        return true;
    }

    false
}

/// Delete a list of file/folder paths. Returns {deleted, failed, errors}.
pub fn delete_items(paths: Vec<String>) -> Value {
    let mut deleted = 0u32;
    let mut errors: Vec<String> = Vec::new();

    for raw in &paths {
        let p = Path::new(raw);
        if is_protected(p) {
            errors.push(format!("{raw}: path is protected"));
            continue;
        }
        if has_reparse_ancestor(p) {
            errors.push(format!(
                "{raw}: refusing to delete a symlink/junction — delete its target explicitly instead"
            ));
            continue;
        }
        let res = if p.is_dir() {
            std::fs::remove_dir_all(p)
        } else {
            std::fs::remove_file(p)
        };
        match res {
            Ok(_) => deleted += 1,
            Err(e) => errors.push(format!("{}: {}", raw, e)),
        }
    }

    json!({
        "deleted": deleted,
        "failed":  errors.len(),
        "errors":  errors,
    })
}

/// Copy a single file, creating parent directories as needed.
fn copy_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut input = std::fs::File::open(src)?;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dst)?;
    let result = std::io::copy(&mut input, &mut output).and_then(|_| output.flush());
    if result.is_err() {
        drop(output);
        let _ = std::fs::remove_file(dst);
    }
    result.map(|_| ())
}

/// Copy a directory tree iteratively (explicit stack, no recursion).
fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir(dst)?;
    let result = (|| {
        let mut stack = vec![(src.to_path_buf(), dst.to_path_buf())];
        while let Some((s, d)) = stack.pop() {
            let rd = std::fs::read_dir(&s)?;
            for entry in rd.flatten() {
                if is_reparse_point(&entry.path()) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "refusing to copy a symlink/junction",
                    ));
                }
                let m = entry.path().symlink_metadata()?;
                let target = d.join(entry.file_name());
                if m.is_dir() {
                    std::fs::create_dir(&target)?;
                    stack.push((entry.path(), target));
                } else {
                    copy_file(&entry.path(), &target)?;
                }
            }
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(dst);
    }
    result
}

/// Move a list of paths to dest_dir. Handles cross-drive (copy+delete fallback).
pub fn move_items(paths: Vec<String>, dest_dir: String) -> Value {
    let dest_root = PathBuf::from(&dest_dir);
    if is_protected_dest(&dest_root) {
        return json!({
            "moved": 0,
            "failed": paths.len(),
            "errors": paths.iter().map(|p| format!("{p}: destination is protected")).collect::<Vec<_>>(),
        });
    }
    let mut moved = 0u32;
    let mut errors: Vec<String> = Vec::new();

    for raw in &paths {
        let src = PathBuf::from(raw);
        if is_protected(&src) {
            errors.push(format!("{raw}: path is protected"));
            continue;
        }
        let Some(fname) = src.file_name() else {
            continue;
        };
        let dst = dest_root.join(fname);

        if has_reparse_ancestor(&src) || has_reparse_ancestor(&dst) {
            errors.push(format!(
                "{raw}: refusing to move through a symlink/junction"
            ));
            continue;
        }
        if src.is_dir() {
            if let Ok(source) = src.canonicalize() {
                if dest_root
                    .ancestors()
                    .find_map(|ancestor| ancestor.canonicalize().ok())
                    .is_some_and(|ancestor| ancestor.starts_with(&source))
                {
                    errors.push(format!("{raw}: destination is inside the source"));
                    continue;
                }
            }
        }
        // Validate the destination before creating it: a rejected move must
        // not leave directories inside the source or beyond a junction.
        if let Err(e) = std::fs::create_dir_all(&dest_root) {
            errors.push(format!("{}: mkdir {e}", raw));
            continue;
        }
        if dst.symlink_metadata().is_ok() {
            errors.push(format!("{raw}: destination already exists"));
            continue;
        }

        // Try atomic rename first (same drive = instant)
        let res: Result<(), std::io::Error> = std::fs::rename(&src, &dst).or_else(|_| {
            // Cross-drive: copy then remove
            if src.is_dir() {
                copy_dir(&src, &dst).and_then(|_| std::fs::remove_dir_all(&src))
            } else {
                copy_file(&src, &dst).and_then(|_| std::fs::remove_file(&src))
            }
        });

        match res {
            Ok(_) => moved += 1,
            Err(e) => errors.push(format!("{}: {e}", raw)),
        }
    }

    json!({
        "moved":  moved,
        "failed": errors.len(),
        "errors": errors,
    })
}

// ── public API ──────────────────────────────────────────────────────────────

/// List all filesystem drives with usage stats.
pub fn drives() -> Value {
    let script = r#"
Get-PSDrive -PSProvider FileSystem -ErrorAction SilentlyContinue |
  Where-Object { $_.Root -ne $null -and $_.Root.Trim() -ne '' } |
  ForEach-Object {
    [PSCustomObject]@{
      Name  = $_.Name
      Root  = $_.Root
      Used  = if($_.Used  -ne $null){[long]$_.Used }else{0}
      Free  = if($_.Free  -ne $null){[long]$_.Free }else{0}
    }
  } | ConvertTo-Json -Compress
"#;
    let raw = crate::ps::run(script).unwrap_or_default();
    let parsed: Vec<Value> = match serde_json::from_str(raw.trim()) {
        Ok(Value::Array(v)) => v,
        Ok(o @ Value::Object(_)) => vec![o],
        _ => return json!({ "drives": [] }),
    };
    let list: Vec<Value> = parsed
        .into_iter()
        .map(|d| {
            let used = d["Used"].as_i64().unwrap_or(0).max(0) as u64;
            let free = d["Free"].as_i64().unwrap_or(0).max(0) as u64;
            let total = used + free;
            json!({
                "name":    d["Name"],
                "root":    d["Root"],
                "used":    used,
                "free":    free,
                "total":   total,
                "pct":     if total > 0 { (used as f64 / total as f64 * 100.0) as u32 } else { 0 },
                "usedFmt": fmt_size(used),
                "freeFmt": fmt_size(free),
                "totFmt":  fmt_size(total),
            })
        })
        .collect();
    json!({ "drives": list })
}

// ── helpers for scan_largest ────────────────────────────────────────────────

fn build_top_files(files: &[FileInfo], limit: usize) -> Vec<Value> {
    let mut refs: Vec<&FileInfo> = files.iter().collect();
    refs.sort_unstable_by_key(|r| std::cmp::Reverse(r.size));
    refs.iter()
        .take(limit)
        .map(|f| {
            json!({
                "path":     f.path.to_string_lossy(),
                "name":     f.name,
                "size":     f.size,
                "sizeFmt":  fmt_size(f.size),
                "ext":      f.ext,
                "modified": f.modified,
            })
        })
        .collect()
}

fn build_largest_result(files: &[FileInfo], root: &Path, limit: usize) -> Value {
    let total_size: u64 = files.iter().map(|f| f.size).sum();
    let file_count: usize = files.len();
    let top_files = build_top_files(files, limit);

    // Folder sizes (accumulate bottom-up)
    let mut dir_sizes: HashMap<PathBuf, u64> = HashMap::new();
    for f in files {
        if let Some(p) = f.path.parent() {
            *dir_sizes.entry(p.to_path_buf()).or_insert(0) += f.size;
        }
    }
    let mut dir_keys: Vec<PathBuf> = dir_sizes.keys().cloned().collect();
    dir_keys.sort_unstable_by_key(|p| std::cmp::Reverse(p.components().count()));
    for dir in &dir_keys {
        let sz = dir_sizes[dir];
        if let Some(parent) = dir.parent() {
            if parent.starts_with(root) {
                *dir_sizes.entry(parent.to_path_buf()).or_insert(0) += sz;
            }
        }
    }
    let mut dir_vec: Vec<(&PathBuf, u64)> = dir_sizes
        .iter()
        .filter(|(p, _)| **p != root && p.starts_with(root))
        .map(|(p, &s)| (p, s))
        .collect();
    dir_vec.sort_unstable_by_key(|d| std::cmp::Reverse(d.1));
    let top_folders: Vec<Value> = dir_vec.iter().take(20).map(|(p, size)| {
        let name = p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.to_string_lossy().into_owned());
        let rel = p.strip_prefix(root)
            .map(|r| r.to_string_lossy().into_owned())
            .unwrap_or_else(|_| p.to_string_lossy().into_owned());
        json!({
            "path":    p.to_string_lossy(),
            "name":    name,
            "relPath": rel,
            "size":    *size,
            "sizeFmt": fmt_size(*size),
            "pct":     if total_size > 0 { (*size as f64 / total_size as f64 * 100.0) as u32 } else { 0 },
        })
    }).collect();

    json!({
        "files":        top_files,
        "folders":      top_folders,
        "fileCount":    file_count,
        "totalSize":    total_size,
        "totalSizeFmt": fmt_size(total_size),
        "capped":       file_count >= MAX_FILES,
    })
}

/// Walk a path, stream partial results via Tauri events every ~1.5 s,
/// and return the full top-N largest files + top-20 largest folders.
pub fn scan_largest(path: String, limit: usize, app: Option<tauri::AppHandle>) -> Value {
    use std::time::Instant;

    let root = PathBuf::from(&path);
    let mut stack: Vec<PathBuf> = vec![root.clone()];
    let mut files: Vec<FileInfo> = Vec::new();
    let mut last_emit = Instant::now();

    'walk: while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            // symlink_metadata: never descend through junctions/symlinks.
            let Ok(meta) = entry.path().symlink_metadata() else {
                continue;
            };
            let fpath = entry.path();
            let lname = fpath
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if SKIP_DIRS.iter().any(|&s| lname == s) {
                continue;
            }

            if meta.is_dir() {
                // Same rule as walk(): a junction reports is_dir() too, and
                // descending into it double-counts sizes or loops forever
                // (a junction to an ancestor; MAX_FILES only counts files).
                if !is_reparse_point(&fpath) {
                    stack.push(fpath);
                }
            } else if meta.is_file() {
                if files.len() >= MAX_FILES {
                    break 'walk;
                }
                let name = fpath
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let ext = fpath
                    .extension()
                    .map(|e| e.to_string_lossy().to_uppercase().to_owned())
                    .unwrap_or_default();
                files.push(FileInfo {
                    path: fpath,
                    name,
                    size: meta.len(),
                    ext,
                    modified: modified_secs(&meta),
                });

                // Stream partial results every 1.5 s
                if let Some(ref handle) = app {
                    if last_emit.elapsed().as_millis() >= 1500 {
                        let partial = build_top_files(&files, limit);
                        let _ = handle.emit(
                            "disk-scan-progress",
                            json!({
                                "scanned": files.len(),
                                "files":   partial,
                                "done":    false,
                            }),
                        );
                        last_emit = Instant::now();
                    }
                }
            }
        }
    }

    let result = build_largest_result(&files, &root, limit);

    // Final event so frontend knows scan is complete
    if let Some(ref handle) = app {
        let _ = handle.emit(
            "disk-scan-progress",
            json!({
                "scanned": files.len(),
                "done":    true,
            }),
        );
    }

    result
}

/// Find duplicate files (two-pass: group by size → hash within groups).
pub fn scan_duplicates(path: String) -> Value {
    let root = PathBuf::from(&path);
    let files = walk(&root);

    // Group by size, skip tiny files
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for f in files {
        if f.size >= DUP_MIN_BYTES {
            by_size.entry(f.size).or_default().push(f.path);
        }
    }

    // Keep only size groups with 2+ files
    let candidates: Vec<(u64, Vec<PathBuf>)> =
        by_size.into_iter().filter(|(_, v)| v.len() >= 2).collect();

    if candidates.is_empty() {
        return json!({ "groups": [], "totalWasted": 0, "totalWastedFmt": "0 B", "checked": 0 });
    }

    // Flatten to (path, size) for rayon parallel hashing
    let flat: Vec<(PathBuf, u64)> = candidates
        .iter()
        .flat_map(|(sz, paths)| paths.iter().map(move |p| (p.clone(), *sz)))
        .collect();
    let total_checked = flat.len();

    // Pass 1: hash only a 64 KiB prefix of every size-matched file (cheap),
    // in parallel. Only (size, prefix) collisions can be duplicates.
    let prefixed: Vec<(String, PathBuf, u64)> = flat
        .par_iter()
        .filter_map(|(path, size)| {
            hash_file_limited(path, DUP_PREFIX_BYTES).map(|h| (h, path.clone(), *size))
        })
        .collect();
    let mut by_prefix: HashMap<(u64, String), Vec<PathBuf>> = HashMap::new();
    for (h, path, size) in prefixed {
        by_prefix.entry((size, h)).or_default().push(path);
    }
    let collisions: Vec<(String, PathBuf, u64)> = by_prefix
        .into_iter()
        .filter(|(_, v)| v.len() >= 2)
        .flat_map(|((size, h), v)| v.into_iter().map(move |p| (h.clone(), p, size)))
        .collect();

    // Pass 2: full hash for prefix collisions only. A file no larger than
    // the prefix was already hashed completely in pass 1.
    let hashed: Vec<(String, PathBuf, u64, u64)> = collisions
        .par_iter()
        .filter_map(|(prefix_hash, path, size)| {
            let modified = path.metadata().ok().map(|m| modified_secs(&m)).unwrap_or(0);
            let full = if *size <= DUP_PREFIX_BYTES {
                Some(prefix_hash.clone())
            } else {
                hash_file(path)
            };
            full.map(|h| (h, path.clone(), *size, modified))
        })
        .collect();

    // Group by (size, full hash); value = (path, size, modified)
    type DupFiles = Vec<(PathBuf, u64, u64)>;
    let mut by_hash: HashMap<(u64, String), DupFiles> = HashMap::new();
    for (hash, path, size, modified) in hashed {
        by_hash
            .entry((size, hash))
            .or_default()
            .push((path, size, modified));
    }

    // Build output groups (2+ files = real duplicates)
    let mut groups: Vec<Value> = by_hash
        .into_iter()
        .filter(|(_, v)| v.len() >= 2)
        .map(|((_, hash), files)| {
            let size   = files[0].1;
            let wasted = size * (files.len() as u64 - 1);
            // Sort: oldest first (original), newest last (likely the copy)
            let mut sorted = files;
            sorted.sort_by_key(|(_, _, m)| *m);
            let file_list: Vec<Value> = sorted.iter().map(|(p, _, m)| json!({
                "path":     p.to_string_lossy(),
                "name":     p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                "modified": m,
            })).collect();
            json!({
                "hash":       &hash[..16],
                "size":       size,
                "sizeFmt":    fmt_size(size),
                "count":      sorted.len(),
                "wasted":     wasted,
                "wastedFmt":  fmt_size(wasted),
                "files":      file_list,
            })
        })
        .collect();

    groups.sort_unstable_by(|a, b| {
        b["wasted"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["wasted"].as_u64().unwrap_or(0))
    });

    let total_wasted: u64 = groups
        .iter()
        .map(|g| g["wasted"].as_u64().unwrap_or(0))
        .sum();

    json!({
        "groups":          groups,
        "totalWasted":     total_wasted,
        "totalWastedFmt":  fmt_size(total_wasted),
        "checked":         total_checked,
    })
}

/// Temp-file age breakdown across all known temp directories.
pub fn scan_temp_age() -> Value {
    let dirs: Vec<PathBuf> = [
        std::env::var("TEMP").ok(),
        std::env::var("TMP").ok(),
        Some(format!("{}\\Temp", crate::ps::windows_dir())),
    ]
    .into_iter()
    .flatten()
    .map(PathBuf::from)
    .filter(|p| p.exists())
    .collect::<std::collections::HashSet<_>>() // dedup (TEMP and TMP often same)
    .into_iter()
    .collect();

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    const BUCKETS: &[(&str, u64)] = &[
        ("Today (<24h)", 86_400),
        ("This Week (<7d)", 7 * 86_400),
        ("This Month (<30d)", 30 * 86_400),
        ("This Year (<365d)", 365 * 86_400),
        ("Older than 1 Year", u64::MAX),
    ];

    let mut counts = vec![0u64; BUCKETS.len()];
    let mut sizes = vec![0u64; BUCKETS.len()];

    for dir in &dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let age = now.saturating_sub(modified_secs(&meta));
            let sz = meta.len();
            for (i, (_, thresh)) in BUCKETS.iter().enumerate() {
                if age < *thresh {
                    counts[i] += 1;
                    sizes[i] += sz;
                    break;
                }
            }
        }
    }

    let total_count: u64 = counts.iter().sum();
    let total_size: u64 = sizes.iter().sum();

    let buckets: Vec<Value> = BUCKETS.iter().enumerate().map(|(i, (label, _))| json!({
        "label":   label,
        "count":   counts[i],
        "size":    sizes[i],
        "sizeFmt": fmt_size(sizes[i]),
        "pct":     if total_count > 0 { (counts[i] as f64 / total_count as f64 * 100.0) as u32 } else { 0 },
    })).collect();

    json!({
        "buckets":        buckets,
        "totalCount":     total_count,
        "totalSize":      total_size,
        "totalSizeFmt":   fmt_size(total_size),
        "dirs":           dirs.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {

    use super::{is_protected, is_protected_dest};
    use std::path::Path;

    #[test]
    fn protected_paths_are_rejected() {
        for p in [
            r"C:\",
            r"C:\\",
            r"E:\",
            r"C:\Windows",
            r"C:\Windows\System32\drivers\evil.sys",
            r"C:\Windows ",
            r"C:\Windows.",
            r"C:\Windows \System32",
            r"C:\Windows \ ",
            r"C:\Program Files\App",
            r"C:\Program Files (x86)\App",
            r"C:\ProgramData\X",
            r"C:\$Recycle.Bin\S-1-5-21\file",
            r"C:\System Volume Information\restore",
            r"C:\Users",
            r"C:\Users\Alice",
            r"C:\Users\Alice\AppData",
            r"C:\Users\Alice\AppData\Roaming",
            r"C:\Users\Alice\AppData\Local",
            r"C:\Users\Alice\AppData\LocalLow",
            r"C:\Users\Alice\AppData\Roaming\PCOptSuite\journal.json",
            r"C:\PCOptSuite\cache",
            r"C:\Windows.OLD",
            r"C:\EFI\Boot",
            r"C:\RECOVERY\X",
            r"relative\path",
            r"C:relative",
            r"\\.\C:\Windows",
            r"C:\Users\Alice\Downloads\..\Windows",
            r"..\..\Windows\System32",
        ] {
            assert!(is_protected(Path::new(p)), "should be protected: {p}");
        }
    }

    #[test]
    fn short_name_stream_and_boot_aliases_are_rejected() {
        for p in [
            // 8.3 short names alias protected long names.
            r"C:\PROGRA~1\App",
            r"C:\PROGRA~2",
            r"C:\WINDOW~1\System32",
            r"C:\Users\ALICE~1\AppData",
            r"D:\data\LONGFO~1\x",
            // NTFS stream / attribute syntax.
            r"C:\Windows::$INDEX_ALLOCATION",
            r"C:\Windows:$I30:$INDEX_ALLOCATION",
            r"C:\Users\Alice\Downloads\file.zip:Zone.Identifier",
            r"D:\x:y",
            // Boot files and the MSI rollback folder.
            r"C:\Boot",
            r"C:\Boot\BCD",
            r"C:\boot\x",
            r"C:\Config.Msi",
            r"C:\Config.Msi\abc.rbf",
        ] {
            assert!(is_protected(Path::new(p)), "should be protected: {p}");
            assert!(
                is_protected_dest(Path::new(p)),
                "dest should be protected: {p}"
            );
        }
        // A tilde not followed by a digit is an ordinary name.
        assert!(!is_protected(Path::new(r"D:\Games\~backup\save.dat")));
        assert!(!is_protected(Path::new(r"D:\Games\v1~beta\save.dat")));
        assert!(!is_protected(Path::new(r"D:\Bootleg\x")));
    }

    #[test]
    fn verbatim_prefix_is_stripped_only_for_drive_paths() {
        use super::strip_verbatim_prefix;
        assert_eq!(
            strip_verbatim_prefix(Path::new(r"\\?\C:\Windows")).as_deref(),
            Some(r"C:\Windows")
        );
        assert_eq!(
            strip_verbatim_prefix(Path::new(r"C:\Users\x")).as_deref(),
            Some(r"C:\Users\x")
        );
        assert_eq!(strip_verbatim_prefix(Path::new(r"\\?\UNC\srv\share")), None);
        assert_eq!(strip_verbatim_prefix(Path::new(r"\\?\Volume{x}\a")), None);
    }

    #[cfg(windows)]
    #[test]
    fn existing_paths_are_checked_by_canonical_long_name() {
        // A real 8.3/case alias of a protected directory is caught by the
        // canonical check as well as the string filter.
        let windir = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        assert!(is_protected(Path::new(&windir.to_lowercase())));
        assert!(is_protected(Path::new(&format!(r"{windir}\System32"))));
        // A temp dir under the user profile stays allowed after canonicalisation.
        let tmp = std::env::temp_dir().join("adho-protect-canon");
        std::fs::create_dir_all(&tmp).unwrap();
        // A real file whose long name merely contains `~<digit>`.
        let tilde = tmp.join("Report~1.txt");
        std::fs::write(&tilde, b"x").unwrap();
        let canon_ok = !is_protected(&tmp);
        let tilde_ok = !is_protected(&tilde);
        std::fs::remove_dir_all(&tmp).unwrap();
        // Existing paths are judged by their canonical long form, so an 8.3
        // temp dir (C:\Users\ADRIA~1\...) and `~1` file names are allowed.
        assert!(canon_ok, "temp dir should be allowed: {}", tmp.display());
        assert!(
            tilde_ok,
            "existing ~1 file should be allowed: {}",
            tilde.display()
        );
    }

    #[test]
    fn user_data_paths_are_allowed() {
        for p in [
            r"C:\Users\Alice\Downloads\file.zip",
            r"C:\Users\Alice\Documents\folder",
            r"C:\Users\Alice\AppData\Local\Temp\old.log",
            r"C:\Users\Alice\AppData\Local\Temp",
            r"C:\Users\Alice\AppData\Roaming\SomeApp\cache",
            r"D:\Games\save.dat",
            r"D:\Downloads\Images\photo.jpg",
            r"Z:\backup\2024\archive.7z",
        ] {
            assert!(!is_protected(Path::new(p)), "should be allowed: {p}");
        }
    }

    #[test]
    fn drive_roots_are_allowed_as_move_destinations() {
        for p in [r"C:\", r"D:\", r"Z:\"] {
            assert!(
                is_protected(Path::new(p)),
                "delete target must be protected: {p}"
            );
            assert!(
                !is_protected_dest(Path::new(p)),
                "move destination must be allowed: {p}"
            );
        }
        assert!(is_protected_dest(Path::new(r"C:\Windows")));
    }

    #[cfg(windows)]
    #[test]
    fn move_refuses_existing_destination_and_self_nesting() {
        use super::move_items;
        use std::fs;

        let root = std::env::temp_dir().join(format!(
            "ad-hyperoptimize-move-test-{}",
            rand::random::<u64>()
        ));
        let source = root.join("source");
        let destination = root.join("destination");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&destination).unwrap();
        let source_file = source.join("same.txt");
        let destination_file = destination.join("same.txt");
        fs::write(&source_file, "source").unwrap();
        fs::write(&destination_file, "destination").unwrap();

        let result = move_items(
            vec![source_file.to_string_lossy().into_owned()],
            destination.to_string_lossy().into_owned(),
        );
        assert_eq!(result["moved"], 0);
        assert_eq!(fs::read_to_string(&source_file).unwrap(), "source");
        assert_eq!(
            fs::read_to_string(&destination_file).unwrap(),
            "destination"
        );

        let nested = source.join("nested");
        let result = move_items(
            vec![source.to_string_lossy().into_owned()],
            nested.to_string_lossy().into_owned(),
        );
        assert_eq!(result["moved"], 0);
        assert!(source_file.exists());
        assert!(!nested.exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn junction_is_detected_as_reparse_point() {
        use super::is_reparse_point;
        use std::fs;
        use std::process::Command;

        let root = std::env::temp_dir().join(format!(
            "ad-hyperoptimize-junction-test-{}",
            rand::random::<u64>()
        ));
        let target = root.join("target");
        let junction = root.join("junction");
        fs::create_dir_all(&target).unwrap();
        let output = Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .output()
            .unwrap();
        if !output.status.success() {
            fs::remove_dir_all(&root).unwrap();
            panic!(
                "junction creation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert!(is_reparse_point(&junction));
        fs::remove_dir(&junction).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }
}
