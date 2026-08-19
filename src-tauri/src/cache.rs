//! Persistent scan cache. Heavy scans (hardware, security, cleanup, analysis)
//! are written to %APPDATA%\PCOptSuite\cache\<key>.json and served instantly
//! until the user explicitly forces a refresh.
//!
//! Envelope shape returned to the UI:
//!   { "fromCache": bool, "time": rfc3339, "data": <payload> }

use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

fn cache_dir() -> PathBuf {
    let p = crate::safety::app_data_dir().join("cache");
    let _ = fs::create_dir_all(&p);
    p
}

/// Cache keys are file names joined into the cache dir. Only allow a strict
/// alphanumeric/`_`/`-`/`.` set so a key can never traverse out of the cache
/// directory (all current callers pass compile-time constants; this is
/// defense-in-depth against future dynamic keys).
fn is_safe_cache_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn path(key: &str) -> PathBuf {
    cache_dir().join(format!("{key}.json"))
}

pub fn load(key: &str) -> Option<Value> {
    if !is_safe_cache_key(key) {
        return None;
    }
    let v: Value = serde_json::from_str(&fs::read_to_string(path(key)).ok()?).ok()?;
    if v.get("time").is_some() && v.get("data").is_some() {
        Some(v)
    } else {
        None
    }
}

pub fn store(key: &str, data: Value) -> Value {
    let env = json!({
        "fromCache": false,
        "time": chrono::Local::now().to_rfc3339(),
        "data": data,
    });
    // An invalid key is never persisted (avoids writing outside the cache
    // dir); the in-memory envelope is still returned so callers work.
    if is_safe_cache_key(key) {
        let _ = fs::write(path(key), serde_json::to_string(&env).unwrap_or_default());
    }
    env
}

/// Serve cache unless `force`; compute + persist otherwise.
pub fn get_or(key: &str, force: bool, f: impl FnOnce() -> Value) -> Value {
    if !force {
        if let Some(mut hit) = load(key) {
            hit["fromCache"] = json!(true);
            return hit;
        }
    }
    store(key, f())
}

/// Raw cached payload (no envelope) for internal consumers like analysis.
pub fn data_or(key: &str, force: bool, f: impl FnOnce() -> Value) -> Value {
    get_or(key, force, f)["data"].clone()
}

#[cfg(test)]
mod tests {
    use super::is_safe_cache_key;

    #[test]
    fn cache_keys_accept_plain_names() {
        assert!(is_safe_cache_key("scan"));
        assert!(is_safe_cache_key("analysis_v2"));
        assert!(is_safe_cache_key("cleanup"));
        assert!(is_safe_cache_key("a.b-c_d"));
    }

    #[test]
    fn cache_keys_reject_traversal() {
        assert!(!is_safe_cache_key(""));
        assert!(!is_safe_cache_key("../evil"));
        assert!(!is_safe_cache_key(r"..\evil"));
        assert!(!is_safe_cache_key("a/b"));
        assert!(!is_safe_cache_key(r"a\b"));
        assert!(!is_safe_cache_key("a:b"));
        assert!(!is_safe_cache_key("a b"));
        assert!(!is_safe_cache_key(&"x".repeat(65)));
    }
}
