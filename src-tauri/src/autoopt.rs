//! Auto-Optimizer — scans all modules, surfaces safe unapplied tweaks,
//! applies selected ones in sequence after creating a restore point.

use crate::{safety, tweaks, privacy, debloater};
use serde_json::{json, Value};

/// A recommendation surfaced to the UI.
/// `module` controls which apply/revert command to call.
#[derive(serde::Serialize)]
#[allow(dead_code)]
pub struct Rec {
    pub id:          String,
    pub module:      String,
    pub category:    String,
    pub name:        String,
    pub description: String,
    pub impact:      String,
    pub risk:        String,
    pub applied:     bool,
}

const SAFE_TWEAK_IDS: &[&str] = &[
    "game_dvr_off",
    "gamebar_popups_off",
    "transparency_off",
    "start_websearch_off",
    "lockscreen_ads_off",
    "widgets_off",
    "delivery_opt_off",
    "wer_off",
    "activity_history_off",
    "ntfs_last_access_off",
    "ntfs_8dot3_off",
    "xbox_services_off",
    "remote_registry_off",
    "cortana_off",
    "edge_prelaunch_off",
    "auto_play_off",
    "svc_fax_off",
    "svc_geolocation_off",
    "svc_wmp_network_off",
    "svc_maps_off",
    "svc_link_tracking_off",
    "svc_wisvc_off",
    "usb_suspend_off",
    "menu_delay_fast",
    "nagle_off",
    "tcp_timed_wait_low",
    "irp_stack_high",
    "qos_bandwidth_free",
    "tcp_autotuning_normal",
    "net_rss_on",
];

const SAFE_PRIVACY_IDS: &[&str] = &[
    "advertising_id",
    "telemetry_minimal",
    "app_diagnostics",
    "inking_typing",
    "feedback_frequency",
    "content_suggestions",
    "background_apps_off",
    "location_off",
];

const SAFE_DEBLOAT_IDS: &[&str] = &[
    "telemetry_tasks",
    "ceip",
    "wer_service",
];

pub fn scan() -> Value {
    let mut recs: Vec<Value> = Vec::new();

    let tweak_list = tweaks::list_with_status();
    if let Some(arr) = tweak_list.as_array() {
        for t in arr {
            let id = t["id"].as_str().unwrap_or("");
            if !SAFE_TWEAK_IDS.contains(&id) { continue; }
            let applied = t["status"].as_str() == Some("applied");
            recs.push(json!({
                "id":          id,
                "module":      "tweak",
                "category":    t["category"].as_str().unwrap_or("General"),
                "name":        t["name"].as_str().unwrap_or(id),
                "description": t["description"].as_str().unwrap_or(""),
                "impact":      t.get("impact").and_then(|v| v.as_str()).unwrap_or(""),
                "risk":        "safe",
                "applied":     applied,
            }));
        }
    }

    let priv_scan = privacy::scan();
    if let Some(arr) = priv_scan["tweaks"].as_array() {
        for p in arr {
            let id = p["id"].as_str().unwrap_or("");
            if !SAFE_PRIVACY_IDS.contains(&id) { continue; }
            let applied = p["applied"].as_bool().unwrap_or(false);
            recs.push(json!({
                "id":          id,
                "module":      "privacy",
                "category":    p["category"].as_str().unwrap_or("Privacy"),
                "name":        p["name"].as_str().unwrap_or(id),
                "description": p["description"].as_str().unwrap_or(""),
                "impact":      "",
                "risk":        "safe",
                "applied":     applied,
            }));
        }
    }

    let deb_list = debloater::list_tweaks();
    if let Some(arr) = deb_list.as_array() {
        for d in arr {
            let id = d["id"].as_str().unwrap_or("");
            if !SAFE_DEBLOAT_IDS.contains(&id) { continue; }
            let applied = d["applied"].as_bool().unwrap_or(false);
            recs.push(json!({
                "id":          id,
                "module":      "debloater_tweak",
                "category":    d["category"].as_str().unwrap_or("Telemetry"),
                "name":        d["name"].as_str().unwrap_or(id),
                "description": d["description"].as_str().unwrap_or(""),
                "impact":      "",
                "risk":        "safe",
                "applied":     applied,
            }));
        }
    }

    let total   = recs.len();
    let pending = recs.iter().filter(|r| !r["applied"].as_bool().unwrap_or(false)).count();
    json!({ "recs": recs, "total": total, "pending": pending })
}

pub fn apply_selected(items: Vec<Value>) -> Value {
    let rp = safety::create_restore_point("AD HyperOptimize Auto-Optimizer")
        .unwrap_or_else(|e| format!("Restore point skipped: {}", e));

    let mut results: Vec<Value> = Vec::new();

    for item in &items {
        let id     = item["id"].as_str().unwrap_or("").to_string();
        let module = item["module"].as_str().unwrap_or("").to_string();
        let name   = item["name"].as_str().unwrap_or(&id).to_string();

        let result: Result<String, String> = match module.as_str() {
            "tweak"           => tweaks::apply(&id).map(|_| format!("OK: {}", name)).map_err(|e| e),
            "privacy"         => privacy::apply(id.clone()).map(|_| format!("OK: {}", name)).map_err(|e| format!("{:?}", e)),
            "debloater_tweak" => debloater::apply_tweak(id.clone()),
            other             => Err(format!("Unknown module: {}", other)),
        };

        results.push(json!({
            "id":   id,
            "name": name,
            "ok":   result.is_ok(),
            "msg":  result.unwrap_or_else(|e| e),
        }));
    }

    let ok_count  = results.iter().filter(|r| r["ok"].as_bool().unwrap_or(false)).count();
    let err_count = results.len() - ok_count;

    json!({
        "restore_point": rp,
        "results":       results,
        "ok":            ok_count,
        "errors":        err_count,
    })
}

pub fn score() -> Value {
    let data    = scan();
    let total   = data["total"].as_u64().unwrap_or(1).max(1);
    let pending = data["pending"].as_u64().unwrap_or(0);
    let applied = total - pending;
    let pct     = (applied * 100 / total) as u32;
    json!({ "score": pct, "applied": applied, "total": total, "pending": pending })
}
