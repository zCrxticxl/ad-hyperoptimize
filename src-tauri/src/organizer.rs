//! File organizer: sorts the files directly inside one folder into category
//! subfolders (`Downloads\clip.mp4` → `Downloads\Videos\clip.mp4`, optionally
//! `Downloads\Videos\2026-10\clip.mp4`).
//!
//! Safety rules:
//! - Only top-level files of the chosen folder; subfolders are never walked
//!   (that once pulled installed apps out of AppData).
//! - The backend derives every target itself; the renderer only names which
//!   files to move. Nothing is ever overwritten: a taken name gets " (2)".
//! - Hidden/system files, files changed in the last minutes (downloads in
//!   progress), links and protected locations are skipped with a reason.
//! - Every run is logged before the first move; "Move back" uses that log
//!   and skips any file that changed since (moved, deleted, or a new file at
//!   the original path), then removes the folders the run created if empty.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::diskanalyzer::{fmt_size, has_reparse_ancestor, is_protected};

struct Category {
    name: &'static str,
    folder: &'static str,
    icon: &'static str,
    exts: &'static [&'static str],
}

const CATEGORIES: &[Category] = &[
    Category {
        name: "Images",
        folder: "Images",
        icon: "🖼",
        exts: &[
            "jpg", "jpeg", "png", "gif", "bmp", "webp", "svg", "ico", "tiff", "tif", "heic",
            "heif", "raw", "cr2", "nef", "arw", "dng", "psd", "ai", "xcf",
        ],
    },
    Category {
        name: "Videos",
        folder: "Videos",
        icon: "🎬",
        exts: &[
            "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "3gp", "ts",
            "vob", "divx", "rmvb",
        ],
    },
    Category {
        name: "Music",
        folder: "Music",
        icon: "🎵",
        exts: &[
            "mp3", "flac", "wav", "aac", "ogg", "wma", "m4a", "opus", "alac", "aiff", "ape", "mid",
            "midi",
        ],
    },
    Category {
        name: "Documents",
        folder: "Documents",
        icon: "📄",
        exts: &[
            "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "odt", "ods", "odp", "rtf",
            "csv", "md", "epub", "djvu", "pages", "numbers", "key",
        ],
    },
    Category {
        name: "Archives",
        folder: "Archives",
        icon: "📦",
        exts: &[
            "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "iso", "cab", "lzh", "zst", "lz4",
        ],
    },
    Category {
        name: "Code",
        folder: "Code",
        icon: "💻",
        exts: &[
            "py", "js", "ts", "rs", "java", "cpp", "c", "h", "cs", "php", "rb", "go", "sh", "bat",
            "ps1", "json", "xml", "yaml", "yml", "html", "css", "sql", "swift", "kt", "dart",
            "lua", "r", "m",
        ],
    },
    Category {
        name: "Executables",
        folder: "Executables",
        icon: "⚙",
        exts: &["exe", "msi", "apk", "deb", "rpm", "pkg", "dmg"],
    },
    Category {
        name: "Fonts",
        folder: "Fonts",
        icon: "🔤",
        exts: &["ttf", "otf", "woff", "woff2", "fon", "eot"],
    },
    Category {
        name: "3D & CAD",
        folder: "3D",
        icon: "📐",
        exts: &[
            "obj", "fbx", "stl", "3ds", "blend", "dae", "step", "stp", "iges", "dwg", "dxf",
        ],
    },
    Category {
        name: "Torrents",
        folder: "Torrents",
        icon: "🌊",
        exts: &["torrent", "magnet"],
    },
];

/// Files changed more recently than this may still be written (downloads,
/// exports) and are left alone.
const RECENT: Duration = Duration::from_secs(120);
/// Runs kept in the history (older logs are pruned).
const KEEP_RUNS: usize = 20;

fn category_of(name: &str) -> Option<&'static Category> {
    let ext = name.rsplit_once('.')?.1.to_lowercase();
    CATEGORIES.iter().find(|c| c.exts.contains(&ext.as_str()))
}

fn month_of(modified: SystemTime) -> String {
    chrono::DateTime::<chrono::Local>::from(modified)
        .format("%Y-%m")
        .to_string()
}

/// `name.ext` → `name (n).ext`.
fn numbered(name: &str, n: u32) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem} ({n}).{ext}"),
        _ => format!("{name} ({n})"),
    }
}

/// Directory a file goes into: `<folder>\<category>[\<YYYY-MM>]`.
fn target_dir(folder: &Path, cat: &Category, month: Option<&str>) -> PathBuf {
    let d = folder.join(cat.folder);
    match month {
        Some(m) => d.join(m),
        None => d,
    }
}

/// First free name in `dir` for `name`, also avoiding names already handed
/// out in this run (`taken`, lower-cased full paths).
fn free_target(dir: &Path, name: &str, taken: &mut HashSet<String>) -> PathBuf {
    let mut candidate = dir.join(name);
    let mut n = 2;
    while candidate.symlink_metadata().is_ok()
        || taken.contains(&candidate.to_string_lossy().to_lowercase())
    {
        candidate = dir.join(numbered(name, n));
        n += 1;
    }
    taken.insert(candidate.to_string_lossy().to_lowercase());
    candidate
}

#[cfg(windows)]
fn hidden_or_system(m: &std::fs::Metadata, _name: &str) -> bool {
    use std::os::windows::fs::MetadataExt;
    const HIDDEN: u32 = 0x2;
    const SYSTEM: u32 = 0x4;
    m.file_attributes() & (HIDDEN | SYSTEM) != 0
}
#[cfg(not(windows))]
fn hidden_or_system(_m: &std::fs::Metadata, name: &str) -> bool {
    name.starts_with('.')
}

/// One top-level file of the folder, classified.
struct Candidate {
    src: PathBuf,
    name: String,
    size: u64,
    cat: Option<&'static Category>,
    month: String,
    skip: Option<&'static str>,
}

fn scan(folder: &Path, guard: &dyn Fn(&Path) -> bool) -> Result<Vec<Candidate>, String> {
    if guard(folder) || has_reparse_ancestor(folder) {
        return Err("This folder is protected or reached through a link; choose another.".into());
    }
    let rd = std::fs::read_dir(folder).map_err(|e| format!("Cannot read folder: {e}"))?;
    let now = SystemTime::now();
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        let Ok(meta) = path.symlink_metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue; // folders and links are never touched
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let skip = if hidden_or_system(&meta, &name) {
            Some("hidden")
        } else if now.duration_since(modified).unwrap_or_default() < RECENT {
            Some("recent")
        } else if guard(&path) {
            Some("protected")
        } else {
            None
        };
        out.push(Candidate {
            cat: category_of(&name),
            month: month_of(modified),
            src: path,
            name,
            size: meta.len(),
            skip,
        });
    }
    out.sort_by_key(|c| c.name.to_lowercase());
    Ok(out)
}

/// Planned moves (src → dest) for the given candidates.
fn plan<'a>(
    folder: &Path,
    by_month: bool,
    cands: impl Iterator<Item = &'a Candidate>,
) -> Vec<(&'a Candidate, PathBuf)> {
    let mut taken = HashSet::new();
    cands
        .filter_map(|c| {
            let cat = c.cat?;
            let dir = target_dir(folder, cat, by_month.then_some(c.month.as_str()));
            Some((c, free_target(&dir, &c.name, &mut taken)))
        })
        .collect()
}

fn rel(folder: &Path, p: &Path) -> String {
    p.strip_prefix(folder)
        .map(|r| r.to_string_lossy().into_owned())
        .unwrap_or_else(|_| p.to_string_lossy().into_owned())
}

fn preview_in(
    folder: &Path,
    by_month: bool,
    guard: &dyn Fn(&Path) -> bool,
) -> Result<Value, String> {
    let cands = scan(folder, guard)?;
    let movable = cands.iter().filter(|c| c.skip.is_none() && c.cat.is_some());
    let planned = plan(folder, by_month, movable);
    let items: Vec<Value> = planned
        .iter()
        .map(|(c, dest)| {
            let renamed = dest
                .file_name()
                .is_some_and(|n| n.to_string_lossy() != c.name);
            json!({
                "src": c.src.to_string_lossy(),
                "name": c.name,
                "size": c.size,
                "sizeFmt": fmt_size(c.size),
                "cat": c.cat.map(|k| k.folder),
                "target": rel(folder, dest),
                "renamed": renamed,
            })
        })
        .collect();
    let categories: Vec<Value> = CATEGORIES
        .iter()
        .filter_map(|cat| {
            let of: Vec<_> = planned
                .iter()
                .filter(|(c, _)| c.cat.map(|k| k.folder) == Some(cat.folder))
                .collect();
            if of.is_empty() {
                return None;
            }
            let size: u64 = of.iter().map(|(c, _)| c.size).sum();
            Some(json!({
                "id": cat.folder, "name": cat.name, "icon": cat.icon,
                "count": of.len(), "size": size, "sizeFmt": fmt_size(size),
            }))
        })
        .collect();
    let skipped: Vec<Value> = cands
        .iter()
        .filter_map(|c| c.skip.map(|r| json!({ "name": c.name, "reason": r })))
        .collect();
    let uncategorized: Vec<Value> = cands
        .iter()
        .filter(|c| c.skip.is_none() && c.cat.is_none())
        .map(|c| json!({ "name": c.name, "size": c.size, "sizeFmt": fmt_size(c.size) }))
        .collect();
    Ok(json!({
        "folder": folder.to_string_lossy(),
        "byMonth": by_month,
        "items": items,
        "categories": categories,
        "skipped": skipped,
        "uncategorized": uncategorized,
    }))
}

// ── run log ─────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
struct Move {
    from: String,
    to: String,
    #[serde(default)]
    done: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct Run {
    id: String,
    time: String,
    folder: String,
    moves: Vec<Move>,
    /// Folders this run created (removed again by "Move back" when empty).
    created_dirs: Vec<String>,
    #[serde(default)]
    undone: bool,
    #[serde(default)]
    undo_note: Option<String>,
}

fn runs_dir() -> Result<PathBuf, String> {
    crate::safety::state_dir().map(|d| d.join("organizer"))
}

fn valid_run_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn save_run(dir: &Path, run: &Run) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("organizer log: {e}"))?;
    let bytes = serde_json::to_vec_pretty(run).map_err(|e| e.to_string())?;
    crate::safety::write_user_file(&dir.join(format!("{}.json", run.id)), &bytes)
}

fn load_runs(dir: &Path) -> Vec<Run> {
    let mut runs: Vec<Run> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read(e.path()).ok())
        .filter_map(|b| serde_json::from_slice::<Run>(&b).ok())
        .filter(|r| valid_run_id(&r.id))
        .collect();
    runs.sort_by(|a, b| b.id.cmp(&a.id));
    runs
}

fn prune(dir: &Path) {
    for old in load_runs(dir).into_iter().skip(KEEP_RUNS) {
        let _ = std::fs::remove_file(dir.join(format!("{}.json", old.id)));
    }
}

/// A logged move is only acted on when it has the shape this module creates:
/// `from` directly in the run's folder, `to` inside one of its category
/// folders (optionally a `YYYY-MM` subfolder) with the same or a numbered
/// file name.
fn move_shape_ok(folder: &Path, m: &Move) -> bool {
    let from = Path::new(&m.from);
    let to = Path::new(&m.to);
    if from.parent() != Some(folder) {
        return false;
    }
    let Ok(rest) = to.strip_prefix(folder) else {
        return false;
    };
    let parts: Vec<String> = rest
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    if parts.iter().any(|p| p == ".." || p == ".") {
        return false;
    }
    let cat_ok = parts
        .first()
        .is_some_and(|c| CATEGORIES.iter().any(|k| k.folder == c));
    let is_month = |s: &str| {
        let b = s.as_bytes();
        b.len() == 7
            && b[4] == b'-'
            && b.iter()
                .enumerate()
                .all(|(i, c)| i == 4 || c.is_ascii_digit())
    };
    let depth_ok = parts.len() == 2 || (parts.len() == 3 && is_month(&parts[1]));
    let from_name = from
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let to_name = parts.last().cloned().unwrap_or_default();
    let name_ok = to_name == from_name || (2..10_000).any(|n| numbered(&from_name, n) == to_name);
    cat_ok && depth_ok && name_ok
}

fn apply_in(
    folder: &Path,
    by_month: bool,
    srcs: &[String],
    guard: &dyn Fn(&Path) -> bool,
    log_dir: &Path,
) -> Result<Value, String> {
    let wanted: HashSet<String> = srcs.iter().map(|s| s.to_lowercase()).collect();
    let cands = scan(folder, guard)?;
    let chosen = cands.iter().filter(|c| {
        c.skip.is_none()
            && c.cat.is_some()
            && wanted.contains(&c.src.to_string_lossy().to_lowercase())
    });
    let planned = plan(folder, by_month, chosen);
    let ignored = srcs.len().saturating_sub(planned.len());
    if planned.is_empty() {
        return Err(
            "Nothing to move: the selected files are no longer there or are skipped.".into(),
        );
    }

    // Folders that do not exist yet are created by this run.
    let mut created: Vec<String> = Vec::new();
    for (_, dest) in &planned {
        let mut d = dest.parent().map(Path::to_path_buf);
        while let Some(dir) = d {
            if dir.as_path() == folder || dir.symlink_metadata().is_ok() {
                break;
            }
            let s = dir.to_string_lossy().into_owned();
            if !created.contains(&s) {
                created.push(s);
            }
            d = dir.parent().map(Path::to_path_buf);
        }
    }

    let now = chrono::Local::now();
    let mut run = Run {
        id: now.format("%Y%m%d-%H%M%S-%3f").to_string(),
        time: now.to_rfc3339(),
        folder: folder.to_string_lossy().into_owned(),
        moves: planned
            .iter()
            .map(|(c, dest)| Move {
                from: c.src.to_string_lossy().into_owned(),
                to: dest.to_string_lossy().into_owned(),
                done: false,
            })
            .collect(),
        created_dirs: created,
        undone: false,
        undo_note: None,
    };
    // Write-ahead: the log exists before the first file moves.
    save_run(log_dir, &run)?;

    let mut moved = 0usize;
    let mut errors = Vec::new();
    for m in run.moves.iter_mut() {
        let (from, to) = (Path::new(&m.from), Path::new(&m.to));
        let res = (|| {
            let parent = to.parent().unwrap_or(folder);
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            if has_reparse_ancestor(parent) {
                return Err("target is reached through a link".to_string());
            }
            if to.symlink_metadata().is_ok() {
                return Err("target name is taken".to_string()); // never overwrite
            }
            std::fs::rename(from, to).map_err(|e| e.to_string())
        })();
        match res {
            Ok(()) => {
                m.done = true;
                moved += 1;
            }
            Err(e) => errors.push(format!("{}: {e}", m.from)),
        }
    }
    save_run(log_dir, &run)?;
    prune(log_dir);
    Ok(json!({
        "runId": run.id, "moved": moved, "failed": errors.len(),
        "errors": errors, "ignored": ignored,
    }))
}

fn undo_in(run_id: &str, guard: &dyn Fn(&Path) -> bool, log_dir: &Path) -> Result<Value, String> {
    if !valid_run_id(run_id) {
        return Err("unknown run".into());
    }
    let path = log_dir.join(format!("{run_id}.json"));
    let bytes = std::fs::read(&path).map_err(|_| "unknown run".to_string())?;
    let mut run: Run =
        serde_json::from_slice(&bytes).map_err(|e| format!("run log unreadable: {e}"))?;
    if run.undone {
        return Err("This run was already moved back.".into());
    }
    let folder = PathBuf::from(&run.folder);
    if guard(&folder) || has_reparse_ancestor(&folder) {
        return Err("The folder of this run is protected or reached through a link.".into());
    }
    let mut restored = 0usize;
    let mut skipped = Vec::new();
    for m in run.moves.iter().rev() {
        let (from, to) = (Path::new(&m.from), Path::new(&m.to));
        let reason = if !move_shape_ok(&folder, m) {
            Some("foreign")
        } else if guard(to) || has_reparse_ancestor(to) {
            Some("protected")
        } else if !to.symlink_metadata().is_ok_and(|x| x.is_file()) {
            // A move that never happened needs nothing; a done one is gone.
            m.done.then_some("gone")
        } else if from.symlink_metadata().is_ok() {
            Some("occupied")
        } else {
            match std::fs::rename(to, from) {
                Ok(()) => {
                    restored += 1;
                    None
                }
                Err(_) => Some("inUse"),
            }
        };
        if let Some(r) = reason {
            let name = from
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            skipped.push(json!({ "name": name, "reason": r }));
        }
    }
    // Deepest first; remove_dir only succeeds on empty folders.
    let mut dirs = run.created_dirs.clone();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.len()));
    for d in dirs {
        let p = Path::new(&d);
        if p.starts_with(&folder) && p != folder.as_path() && !has_reparse_ancestor(p) {
            let _ = std::fs::remove_dir(p);
        }
    }
    run.undone = true;
    run.undo_note = Some(format!("{restored}/{}", skipped.len()));
    save_run(log_dir, &run)?;
    Ok(json!({ "restored": restored, "skipped": skipped }))
}

fn runs_in(log_dir: &Path) -> Value {
    let runs: Vec<Value> = load_runs(log_dir)
        .iter()
        .map(|r| {
            json!({
                "id": r.id, "time": r.time, "folder": r.folder,
                "moved": r.moves.iter().filter(|m| m.done).count(),
                "undone": r.undone,
            })
        })
        .collect();
    json!({ "runs": runs })
}

// ── public API ──────────────────────────────────────────────────────────────

/// Downloads, Desktop and Documents from the known-folder API (not from
/// environment variables).
pub fn folders() -> Value {
    let list: Vec<Value> = [
        ("downloads", dirs::download_dir()),
        ("desktop", dirs::desktop_dir()),
        ("documents", dirs::document_dir()),
    ]
    .into_iter()
    .filter_map(|(k, p)| {
        p.filter(|p| p.is_dir())
            .map(|p| json!({ "key": k, "path": p.to_string_lossy() }))
    })
    .collect();
    json!({ "folders": list })
}

pub fn preview(folder: String, by_month: bool) -> Result<Value, String> {
    preview_in(Path::new(folder.trim()), by_month, &|p| is_protected(p))
}

pub fn apply(folder: String, by_month: bool, srcs: Vec<String>) -> Result<Value, String> {
    apply_in(
        Path::new(folder.trim()),
        by_month,
        &srcs,
        &|p| is_protected(p),
        &runs_dir()?,
    )
}

pub fn runs() -> Value {
    match runs_dir() {
        Ok(d) => runs_in(&d),
        Err(_) => json!({ "runs": [] }),
    }
}

pub fn undo(run_id: String) -> Result<Value, String> {
    undo_in(&run_id, &|p| is_protected(p), &runs_dir()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh scratch folder in the system temp directory.
    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("adho-organizer-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn old_file(p: &Path, body: &str) {
        std::fs::write(p, body).unwrap();
        let t = SystemTime::now() - Duration::from_secs(3600);
        std::fs::File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    fn allow(_: &Path) -> bool {
        false
    }

    #[test]
    fn numbering_keeps_the_extension() {
        assert_eq!(numbered("clip.mp4", 2), "clip (2).mp4");
        assert_eq!(numbered("README", 3), "README (3)");
        assert_eq!(numbered(".hidden", 2), ".hidden (2)");
    }

    #[test]
    fn preview_skips_recent_and_hidden_and_avoids_name_clashes() {
        let d = scratch("preview");
        old_file(&d.join("a.mp4"), "x");
        old_file(&d.join("b.txt"), "x");
        old_file(&d.join("notes.unknownext"), "x");
        std::fs::write(d.join("fresh.mp4"), "x").unwrap(); // modified now
        std::fs::create_dir_all(d.join("Videos")).unwrap();
        old_file(&d.join("Videos").join("a.mp4"), "existing");
        let v = preview_in(&d, false, &allow).unwrap();
        let items = v["items"].as_array().unwrap();
        let a = items.iter().find(|i| i["name"] == "a.mp4").unwrap();
        assert_eq!(a["renamed"], true);
        assert!(a["target"].as_str().unwrap().ends_with("a (2).mp4"));
        assert!(items.iter().all(|i| i["name"] != "fresh.mp4"));
        assert!(v["skipped"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["name"] == "fresh.mp4" && s["reason"] == "recent"));
        assert_eq!(v["uncategorized"][0]["name"], "notes.unknownext");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn apply_moves_only_selected_files_and_undo_restores_them() {
        let d = scratch("roundtrip");
        let logs = std::env::temp_dir().join(format!("adho-organizer-logs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&logs);
        old_file(&d.join("a.mp4"), "video");
        old_file(&d.join("b.pdf"), "doc");
        old_file(&d.join("c.png"), "img");
        let pick = vec![
            d.join("a.mp4").to_string_lossy().into_owned(),
            d.join("b.pdf").to_string_lossy().into_owned(),
            "/elsewhere/evil.exe".to_string(), // not in the folder: ignored
        ];
        let r = apply_in(&d, true, &pick, &allow, &logs).unwrap();
        assert_eq!(r["moved"], 2);
        assert_eq!(r["ignored"], 1);
        assert!(!d.join("a.mp4").exists());
        assert!(d.join("c.png").exists(), "unselected file stays");
        let videos = d.join("Videos");
        let month = std::fs::read_dir(&videos)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert!(month.join("a.mp4").exists());

        // A new file at an original path is never overwritten by the undo.
        old_file(&d.join("b.pdf"), "new");
        let run_id = r["runId"].as_str().unwrap().to_string();
        assert_eq!(runs_in(&logs)["runs"][0]["moved"], 2);
        let u = undo_in(&run_id, &allow, &logs).unwrap();
        assert_eq!(u["restored"], 1);
        assert_eq!(u["skipped"][0]["reason"], "occupied");
        assert_eq!(std::fs::read_to_string(d.join("a.mp4")).unwrap(), "video");
        assert_eq!(std::fs::read_to_string(d.join("b.pdf")).unwrap(), "new");
        assert!(
            !videos.exists(),
            "empty folders created by the run are removed"
        );
        assert!(
            d.join("Documents").exists(),
            "a folder still holding a file stays"
        );
        assert!(
            undo_in(&run_id, &allow, &logs).is_err(),
            "second undo refused"
        );
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&logs);
    }

    #[test]
    fn foreign_log_entries_are_not_acted_on() {
        let folder = Path::new("/data/Downloads");
        let mv = |from: &str, to: &str| Move {
            from: from.into(),
            to: to.into(),
            done: true,
        };
        assert!(move_shape_ok(
            folder,
            &mv("/data/Downloads/a.mp4", "/data/Downloads/Videos/a (2).mp4")
        ));
        assert!(move_shape_ok(
            folder,
            &mv(
                "/data/Downloads/a.mp4",
                "/data/Downloads/Videos/2026-10/a.mp4"
            )
        ));
        for (from, to) in [
            ("/data/Downloads/a.mp4", "/etc/a.mp4"),
            ("/data/Downloads/a.mp4", "/data/Downloads/NotACat/a.mp4"),
            ("/data/Downloads/a.mp4", "/data/Downloads/Videos/other.mp4"),
            ("/data/Downloads/a.mp4", "/data/Downloads/Videos/x/a.mp4"),
            ("/data/Other/a.mp4", "/data/Downloads/Videos/a.mp4"),
            (
                "/data/Downloads/a.mp4",
                "/data/Downloads/Videos/../../x/a.mp4",
            ),
        ] {
            assert!(!move_shape_ok(folder, &mv(from, to)), "{to}");
        }
    }
}
