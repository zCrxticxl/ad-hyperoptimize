//! Auto-Optimizer, scans all modules, surfaces safe unapplied tweaks,
//! applies selected ones in sequence after creating a restore point.

use crate::{debloater, privacy, safety, tweaks};
use serde_json::{json, Value};

/// A recommendation surfaced to the UI.
/// `module` controls which apply/revert command to call.
#[derive(serde::Serialize)]
#[allow(dead_code)]
pub struct Rec {
    pub id: String,
    pub module: String,
    pub category: String,
    pub name: String,
    pub description: String,
    pub impact: String,
    pub risk: String,
    pub applied: bool,
}

const SAFE_TWEAK_IDS: &[&str] = &["game_dvr_off", "game_mode_on"];

const SAFE_PRIVACY_IDS: &[&str] = &[];

const SAFE_DEBLOAT_IDS: &[&str] = &[];

pub fn scan() -> Value {
    let mut recs: Vec<Value> = Vec::new();

    let tweak_list = tweaks::list_with_status();
    if let Some(arr) = tweak_list.as_array() {
        for t in arr {
            let id = t["id"].as_str().unwrap_or("");
            if !SAFE_TWEAK_IDS.contains(&id) {
                continue;
            }
            let applied = t["status"].as_str() == Some("applied");
            recs.push(json!({
                "id":          id,
                "module":      "tweak",
                "category":    t["category"].as_str().unwrap_or("General"),
                "name":        t["name"].as_str().unwrap_or(id),
                "description": t["description"].as_str().unwrap_or(""),
                "impact":      t.get("impact").and_then(|v| v.as_str()).unwrap_or(""),
                // Propagate the real catalog risk (Low/Medium/High) so the
                // Auto-Optimizer can never present a Medium tweak as "safe".
                "risk":        t["risk"].as_str().unwrap_or("Low"),
                "applied":     applied,
            }));
        }
    }

    let priv_scan = privacy::scan();
    if let Some(arr) = priv_scan["tweaks"].as_array() {
        for p in arr {
            let id = p["id"].as_str().unwrap_or("");
            if !SAFE_PRIVACY_IDS.contains(&id) {
                continue;
            }
            let applied = p["applied"].as_bool().unwrap_or(false);
            recs.push(json!({
                "id":          id,
                "module":      "privacy",
                "category":    p["category"].as_str().unwrap_or("Privacy"),
                "name":        p["name"].as_str().unwrap_or(id),
                "description": p["description"].as_str().unwrap_or(""),
                "impact":      "",
                "risk":        p["risk"].as_str().unwrap_or("Low"),
                "applied":     applied,
            }));
        }
    }

    let deb_list = debloater::list_tweaks();
    if let Some(arr) = deb_list.as_array() {
        for d in arr {
            let id = d["id"].as_str().unwrap_or("");
            if !SAFE_DEBLOAT_IDS.contains(&id) {
                continue;
            }
            let applied = d["applied"].as_bool().unwrap_or(false);
            recs.push(json!({
                "id":          id,
                "module":      "debloater_tweak",
                "category":    d["category"].as_str().unwrap_or("Telemetry"),
                "name":        d["name"].as_str().unwrap_or(id),
                "description": d["description"].as_str().unwrap_or(""),
                "impact":      "",
                "risk":        "Low",
                "applied":     applied,
            }));
        }
    }

    let total = recs.len();
    let pending = recs
        .iter()
        .filter(|r| !r["applied"].as_bool().unwrap_or(false))
        .count();
    json!({ "recs": recs, "total": total, "pending": pending })
}

pub fn apply_selected(items: Vec<Value>) -> Value {
    let rp = safety::create_restore_point("AD HyperOptimize Auto-Optimizer")
        .unwrap_or_else(|e| format!("Restore point skipped: {}", e));

    let mut results: Vec<Value> = Vec::new();

    for item in &items {
        let id = item["id"].as_str().unwrap_or("").to_string();
        let module = item["module"].as_str().unwrap_or("").to_string();
        let name = item["name"].as_str().unwrap_or(&id).to_string();

        let allowed = match module.as_str() {
            "tweak" => SAFE_TWEAK_IDS.contains(&id.as_str()),
            "privacy" => SAFE_PRIVACY_IDS.contains(&id.as_str()),
            "debloater_tweak" => SAFE_DEBLOAT_IDS.contains(&id.as_str()),
            _ => false,
        };
        let result: Result<String, String> = if !allowed {
            Err(format!("Not recommended: {module}/{id}"))
        } else {
            match module.as_str() {
                "tweak" => tweaks::apply(&id).map(|_| format!("OK: {}", name)),
                "privacy" => privacy::apply(id.clone()).map(|_| format!("OK: {}", name)),
                "debloater_tweak" => debloater::apply_tweak(id.clone()),
                other => Err(format!("Unknown module: {}", other)),
            }
        };

        results.push(json!({
            "id":   id,
            "name": name,
            "ok":   result.is_ok(),
            "msg":  result.unwrap_or_else(|e| e),
        }));
    }

    let ok_count = results
        .iter()
        .filter(|r| r["ok"].as_bool().unwrap_or(false))
        .count();
    let err_count = results.len() - ok_count;

    json!({
        "restore_point": rp,
        "results":       results,
        "ok":            ok_count,
        "errors":        err_count,
    })
}

pub fn score() -> Value {
    let data = scan();
    let total = data["total"].as_u64().unwrap_or(1).max(1);
    let pending = data["pending"].as_u64().unwrap_or(0);
    let applied = total - pending;
    let pct = (applied * 100 / total) as u32;
    json!({ "score": pct, "applied": applied, "total": total, "pending": pending })
}
