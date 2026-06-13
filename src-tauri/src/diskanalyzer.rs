//! Disk Analyzer: largest files + folders, duplicate detection, temp-file age.
//! All filesystem walks are iterative (no recursion) to avoid stack overflow.

use rayon::prelude::*;
use sha2::{Digest, Sha256};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

// ── constants ───────────────────────────────────────────────────────────────

/// Directory names (lower-case) that are always skipped during walks.
const SKIP_DIRS: &[&str] = &[
    "$recycle.bin",
    "system volume information",
    "winsxs",      // Windows component store — huge, untouchable
    "softwaredistribution",
    ".git",
    "node_modules",
];

/// Minimum file size for duplicate detection (100 KB — hashing tiny files wastes time).
const DUP_MIN_BYTES: u64 = 100 * 1024;

/// Maximum files to walk per drive scan (safety valve for drives with millions of files).
const MAX_FILES: usize = 2_000_000;

// ── internal types ──────────────────────────────────────────────────────────

struct FileInfo {
    path:     PathBuf,
    name:     String,
    size:     u64,
    ext:      String,
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
    if b >= GB       { format!("{:.2} GB", b / GB) }
    else if b >= MB  { format!("{:.1} MB", b / MB) }
    else if b >= KB  { format!("{:.0} KB", b / KB) }
    else             { format!("{bytes} B") }
}

/// Iterative BFS walk. Skips symlinks and SKIP_DIRS.
fn walk(root: &Path) -> Vec<FileInfo> {
    let mut stack = vec![root.to_path_buf()];
    let mut out   = Vec::new();

    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            // Use symlink_metadata to avoid following links
            let Ok(meta) = entry.metadata() else { continue };
            let path = entry.path();
            let lname = path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            if SKIP_DIRS.iter().any(|&s| lname == s) { continue; }

            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                if out.len() >= MAX_FILES { continue; }
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let ext = path
                    .extension()
                    .map(|e| e.to_string_lossy().to_uppercase().to_owned())
                    .unwrap_or_default();
                out.push(FileInfo { path, name, size: meta.len(), ext, modified: modified_secs(&meta) });
            }
        }
    }
    out
}

fn hash_file(path: &Path) -> Option<String> {
    let mut f   = std::fs::File::open(path).ok()?;
    let mut h   = Sha256::new();
    let mut buf = vec![0u8; 65_536];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 { break; }
        h.update(&buf[..n]);
    }
    Some(format!("{:x}", h.finalize()))
}

// ── file operations ─────────────────────────────────────────────────────────

/// Delete a list of file/folder paths. Returns {deleted, failed, errors}.
pub fn delete_items(paths: Vec<String>) -> Value {
    let mut deleted = 0u32;
    let mut errors: Vec<String> = Vec::new();

    for raw in &paths {
        let p = Path::new(raw);
        let res = if p.is_dir() {
            std::fs::remove_dir_all(p)
        } else {
            std::fs::remove_file(p)
        };
        match res {
            Ok(_)  => deleted += 1,
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
    std::fs::copy(src, dst)?;
    Ok(())
}

/// Recursively copy a directory tree.
fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    let rd = std::fs::read_dir(src)?;
    for entry in rd.flatten() {
        let m = entry.metadata()?;
        let d = dst.join(entry.file_name());
        if m.is_dir() {
            copy_dir(&entry.path(), &d)?;
        } else {
            copy_file(&entry.path(), &d)?;
        }
    }
    Ok(())
}

/// Move a list of paths to dest_dir. Handles cross-drive (copy+delete fallback).
pub fn move_items(paths: Vec<String>, dest_dir: String) -> Value {
    let dest_root = PathBuf::from(&dest_dir);
    let mut moved = 0u32;
    let mut errors: Vec<String> = Vec::new();

    for raw in &paths {
        let src = PathBuf::from(raw);
        let Some(fname) = src.file_name() else { continue };
        let dst = dest_root.join(fname);

        // Ensure dest directory exists
        if let Err(e) = std::fs::create_dir_all(&dest_root) {
            errors.push(format!("{}: mkdir {e}", raw));
            continue;
        }

        // Try atomic rename first (same drive = instant)
        let res = std::fs::rename(&src, &dst).or_else(|_| {
            // Cross-drive: copy then remove
            if src.is_dir() {
                copy_dir(&src, &dst).and_then(|_| { std::fs::remove_dir_all(&src) })
            } else {
                copy_file(&src, &dst).and_then(|_| { std::fs::remove_file(&src) })
            }
        });

        match res {
            Ok(_)  => moved += 1,
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
    let list: Vec<Value> = parsed.into_iter().map(|d| {
        let used  = d["Used"].as_i64().unwrap_or(0).max(0) as u64;
        let free  = d["Free"].as_i64().unwrap_or(0).max(0) as u64;
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
    }).collect();
    json!({ "drives": list })
}

/// Walk a path and return the top-N largest files + top-20 largest folders.
pub fn scan_largest(path: String, limit: usize) -> Value {
    let root = PathBuf::from(&path);
    let files = walk(&root);

    let total_size:  u64   = files.iter().map(|f| f.size).sum();
    let file_count:  usize = files.len();

    // ── Top N files ─────────────────────────────────────────────────────────
    let mut sorted_refs: Vec<&FileInfo> = files.iter().collect();
    sorted_refs.sort_unstable_by(|a, b| b.size.cmp(&a.size));
    let top_files: Vec<Value> = sorted_refs.iter().take(limit).map(|f| {
        json!({
            "path":     f.path.to_string_lossy(),
            "name":     f.name,
            "size":     f.size,
            "sizeFmt":  fmt_size(f.size),
            "ext":      f.ext,
            "modified": f.modified,
        })
    }).collect();

    // ── Folder sizes (accumulate bottom-up) ─────────────────────────────────
    let mut dir_sizes: HashMap<PathBuf, u64> = HashMap::new();
    for f in &files {
        if let Some(p) = f.path.parent() {
            *dir_sizes.entry(p.to_path_buf()).or_insert(0) += f.size;
        }
    }
    // Sort by depth desc so children propagate into parents correctly.
    let mut dir_keys: Vec<PathBuf> = dir_sizes.keys().cloned().collect();
    dir_keys.sort_unstable_by_key(|p| std::cmp::Reverse(p.components().count()));
    for dir in &dir_keys {
        let sz = dir_sizes[dir];
        if let Some(parent) = dir.parent() {
            if parent.starts_with(&root) {
                *dir_sizes.entry(parent.to_path_buf()).or_insert(0) += sz;
            }
        }
    }
    // Top 20 subdirs (exclude root itself)
    let root_norm = root.clone();
    let mut dir_vec: Vec<(&PathBuf, u64)> = dir_sizes.iter()
        .filter(|(p, _)| **p != root_norm && p.starts_with(&root_norm))
        .map(|(p, &s)| (p, s))
        .collect();
    dir_vec.sort_unstable_by(|a, b| b.1.cmp(&a.1));
    let top_folders: Vec<Value> = dir_vec.iter().take(20).map(|(p, size)| {
        let name = p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.to_string_lossy().into_owned());
        // Compute relative display path
        let rel = p.strip_prefix(&root_norm)
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

/// Find duplicate files (two-pass: group by size → hash within groups).
pub fn scan_duplicates(path: String) -> Value {
    let root = PathBuf::from(&path);
    let files = walk(&root);

    // Group by size — skip tiny files
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for f in files {
        if f.size >= DUP_MIN_BYTES {
            by_size.entry(f.size).or_default().push(f.path);
        }
    }

    // Keep only size groups with 2+ files
    let candidates: Vec<(u64, Vec<PathBuf>)> = by_size
        .into_iter()
        .filter(|(_, v)| v.len() >= 2)
        .collect();

    if candidates.is_empty() {
        return json!({ "groups": [], "totalWasted": 0, "totalWastedFmt": "0 B", "checked": 0 });
    }

    // Flatten to (path, size) for rayon parallel hashing
    let flat: Vec<(PathBuf, u64)> = candidates
        .iter()
        .flat_map(|(sz, paths)| paths.iter().map(move |p| (p.clone(), *sz)))
        .collect();
    let total_checked = flat.len();

    // Hash in parallel
    let hashed: Vec<(String, PathBuf, u64, u64)> = flat
        .par_iter()
        .filter_map(|(path, size)| {
            let modified = path.metadata()
                .ok()
                .map(|m| modified_secs(&m))
                .unwrap_or(0);
            hash_file(path).map(|h| (h, path.clone(), *size, modified))
        })
        .collect();

    // Group by hash
    let mut by_hash: HashMap<String, Vec<(PathBuf, u64, u64)>> = HashMap::new();
    for (hash, path, size, modified) in hashed {
        by_hash.entry(hash).or_default().push((path, size, modified));
    }

    // Build output groups (2+ files = real duplicates)
    let mut groups: Vec<Value> = by_hash
        .into_iter()
        .filter(|(_, v)| v.len() >= 2)
        .map(|(hash, files)| {
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
        b["wasted"].as_u64().unwrap_or(0).cmp(&a["wasted"].as_u64().unwrap_or(0))
    });

    let total_wasted: u64 = groups.iter().map(|g| g["wasted"].as_u64().unwrap_or(0)).sum();

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
        std::env::var("SystemRoot").ok().map(|r| format!("{r}\\Temp")),
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
        ("Heute (<24h)",         86_400),
        ("Diese Woche (<7d)",    7 * 86_400),
        ("Dieser Monat (<30d)", 30 * 86_400),
        ("Dieses Jahr (<365d)",365 * 86_400),
        ("Älter als 1 Jahr",    u64::MAX),
    ];

    let mut counts = vec![0u64; BUCKETS.len()];
    let mut sizes  = vec![0u64; BUCKETS.len()];

    for dir in &dirs {
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        for entry in rd.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() { continue; }
            let age = now.saturating_sub(modified_secs(&meta));
            let sz  = meta.len();
            for (i, (_, thresh)) in BUCKETS.iter().enumerate() {
                if age < *thresh {
                    counts[i] += 1;
                    sizes[i]  += sz;
                    break;
                }
            }
        }
    }

    let total_count: u64 = counts.iter().sum();
    let total_size:  u64 = sizes.iter().sum();

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
