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

fn path(key: &str) -> PathBuf {
    cache_dir().join(format!("{key}.json"))
}

pub fn load(key: &str) -> Option<Value> {
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
    let _ = fs::write(path(key), serde_json::to_string(&env).unwrap_or_default());
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
