//! Power Plan Manager, list, activate, create and unlock Windows power schemes.

use crate::ps;
use serde_json::{json, Value};

/// GUID of the hidden Ultimate Performance plan (built into Windows 10/11 Pro+).
const ULTIMATE_GUID: &str = "e9a42b02-d5df-448d-aa00-03f14749eb61";

pub fn list_plans() -> Value {
    // Parse `powercfg /list` which outputs:
    //   Power Scheme GUID: <guid>  (<name>) *
    // The * marks the active scheme.
    let script = r#"
$lines = powercfg /list 2>$null
$plans = $lines | ForEach-Object {
    if ($_ -match '([0-9a-fA-F]{8}(?:-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12})\s+\((.+?)\)') {
        [PSCustomObject]@{
            guid   = $Matches[1].ToLower()
            name   = $Matches[2].Trim()
            active = ($_ -match '\*\s*$')
        }
    }
}
if (-not $plans) { '[]' } else { @($plans) | ConvertTo-Json -Compress }
"#;

    let mut plans = match ps::run_json(script) {
        Ok(v @ Value::Array(_)) => v,
        Ok(v @ Value::Object(_)) => Value::Array(vec![v]),
        Ok(_) => {
            return json!({ "error": "Power plan list returned an unexpected shape", "plans": [] })
        }
        Err(error) => return json!({ "error": error, "plans": [] }),
    };

    let ultimate = plans.as_array().and_then(|arr| {
        arr.iter().find(|p| {
            let name = p["name"].as_str().unwrap_or("").to_lowercase();
            p["guid"].as_str() == Some(ULTIMATE_GUID)
                || name.contains("ultimate performance")
                || name.contains("ultimative leistung")
        })
    });
    let ultimate_available = ultimate.is_some();
    let ultimate_guid = ultimate
        .and_then(|p| p["guid"].as_str())
        .unwrap_or(ULTIMATE_GUID)
        .to_string();
    if let Some(arr) = plans.as_array_mut() {
        for plan in arr {
            plan["isUltimate"] = json!(plan["guid"].as_str() == Some(ultimate_guid.as_str()));
        }
    }

    json!({
        "plans": plans,
        "ultimateAvailable": ultimate_available,
        "ultimateGuid": ultimate_guid,
    })
}

/// Canonical 36-char GUID shape: 8-4-4-4-12 hex digits with dashes.
fn is_guid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b[8] == b'-'
        && b[13] == b'-'
        && b[18] == b'-'
        && b[23] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| matches!(i, 8 | 13 | 18 | 23) || c.is_ascii_hexdigit())
}

pub fn set_active(guid: String) -> Result<String, String> {
    if !is_guid(&guid) {
        return Err(format!("Invalid GUID: {guid}"));
    }
    ps::exec("powercfg.exe", &["/setactive", &guid])?;
    let plans = list_plans();
    if let Some(error) = plans["error"].as_str() {
        return Err(format!("Cannot verify active power plan: {error}"));
    }
    let active = plans["plans"].as_array().is_some_and(|entries| {
        entries.iter().any(|entry| {
            entry["guid"].as_str() == Some(guid.to_lowercase().as_str()) && entry["active"] == true
        })
    });
    if !active {
        return Err(format!("Power plan {guid} was not active after /setactive"));
    }
    Ok(format!("Power plan {guid} activated"))
}

pub fn unlock_ultimate() -> Result<String, String> {
    let existing = list_plans();
    if let Some(error) = existing["error"].as_str() {
        return Err(format!("Cannot inspect power plans: {error}"));
    }
    if existing["ultimateAvailable"] == true {
        let guid = existing["ultimateGuid"]
            .as_str()
            .ok_or("Ultimate plan GUID missing")?;
        set_active(guid.to_string())?;
        return Ok("Ultimate Performance plan activated".into());
    }
    let output = ps::exec("powercfg.exe", &["/duplicatescheme", ULTIMATE_GUID])?;
    let created = list_plans();
    if let Some(error) = created["error"].as_str() {
        return Err(format!("Cannot verify created power plan: {error}"));
    }
    let guid = created["ultimateGuid"]
        .as_str()
        .ok_or_else(|| format!("Ultimate plan not found after creation: {output}"))?;
    if created["ultimateAvailable"] != true {
        return Err(format!("Ultimate plan not found after creation: {output}"));
    }
    set_active(guid.to_string())?;
    Ok(format!(
        "Ultimate Performance plan created and activated: {guid}"
    ))
}

pub fn delete_plan(guid: String) -> Result<String, String> {
    // Protect built-in plans + Ultimate
    let builtin = [
        "381b4222-f694-41f0-9685-ff5bb260df2e", // Balanced
        "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c", // High performance
        "a1841308-3541-4fab-bc81-f71556f20b4a", // Power saver
        ULTIMATE_GUID,
    ];
    let g = guid.to_lowercase();
    if builtin.contains(&g.as_str()) {
        return Err("Cannot delete a built-in power plan.".into());
    }
    let plans = list_plans();
    if let Some(error) = plans["error"].as_str() {
        return Err(format!(
            "Cannot verify power plan type before deletion: {error}"
        ));
    }
    if plans["ultimateGuid"].as_str() == Some(g.as_str()) && plans["ultimateAvailable"] == true {
        return Err("Cannot delete the Ultimate Performance plan.".into());
    }
    ps::exec("powercfg.exe", &["/delete", &guid]).map(|_| format!("Plan {guid} deleted"))
}

pub fn create_custom(name: String, base_guid: String) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Plan name cannot be empty".into());
    }
    if name.len() > 128 {
        return Err("Plan name too long (max 128 chars)".into());
    }
    // The name is embedded in a double-quoted PS string: reject every char
    // that could terminate or escape the quote (`"`, `$`, backtick, `'`).
    if !ps::is_safe_ident(name) {
        return Err("Plan name contains unsupported characters".into());
    }
    if !is_guid(&base_guid) {
        return Err(format!("Invalid base plan GUID: {base_guid}"));
    }
    let script = format!(
        r#"
$out = powercfg /duplicatescheme {base_guid} 2>&1
if ($out -match '([0-9a-fA-F]{{8}}(?:-[0-9a-fA-F]{{4}}){{3}}-[0-9a-fA-F]{{12}})') {{
    $newGuid = $Matches[1].Trim()
    powercfg /changename $newGuid "{name}" 2>$null
    "Created: $newGuid"
}} else {{
    throw "Failed to duplicate scheme: $out"
}}
"#
    );
    ps::run(&script).map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::is_guid;

    #[test]
    fn guid_validation() {
        assert!(is_guid("381b4222-f694-41f0-9685-ff5bb260df2e"));
        assert!(is_guid("E9A42B02-D5DF-448D-AA00-03F14749EB61"));
        assert!(!is_guid(""));
        assert!(!is_guid("381b4222-f694-41f0-9685"));
        assert!(!is_guid("381b4222f69441f09685ff5bb260df2e"));
        assert!(!is_guid("381b4222-f694-41f0-9685-ff5bb260df2e; calc"));
        assert!(!is_guid("381b4222_f694_41f0_9685_ff5bb260df2e"));
        assert!(!is_guid("g81b4222-f694-41f0-9685-ff5bb260df2e"));
    }
}
