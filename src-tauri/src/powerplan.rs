//! Power Plan Manager — list, activate, create and unlock Windows power schemes.

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
$plans = $lines | Where-Object { $_ -match 'GUID:\s+([\w-]+)\s+\((.+?)\)' } | ForEach-Object {
    [PSCustomObject]@{
        guid   = $Matches[1].Trim().ToLower()
        name   = $Matches[2].Trim()
        active = ($_ -match '\*\s*$')
    }
}
if (-not $plans) { '[]' } else { @($plans) | ConvertTo-Json -Compress }
"#;

    let plans = match ps::run_json(script) {
        Ok(v @ Value::Array(_)) => v,
        Ok(v @ Value::Object(_)) => Value::Array(vec![v]),
        _ => Value::Array(vec![]),
    };

    let has_ultimate = plans
        .as_array()
        .map(|arr| {
            arr.iter().any(|p| {
                p["guid"]
                    .as_str()
                    .unwrap_or("")
                    .contains(&ULTIMATE_GUID[..8])
            })
        })
        .unwrap_or(false);

    json!({
        "plans": plans,
        "ultimateAvailable": has_ultimate,
        "ultimateGuid": ULTIMATE_GUID,
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
    let out = ps::exec("powercfg.exe", &["/setactive", &guid])?;
    let _ = out; // powercfg outputs nothing on success
    Ok(format!("Power plan {guid} activated"))
}

pub fn unlock_ultimate() -> Result<String, String> {
    let script = format!(
        r#"
$existing = powercfg /list 2>$null | Where-Object {{ $_ -match '{ULTIMATE_GUID}' }}
if ($existing) {{
    "Ultimate Performance plan is already available"
}} else {{
    powercfg /duplicatescheme {ULTIMATE_GUID} 2>&1
    "Ultimate Performance plan unlocked — restart powercfg to see it"
}}
"#
    );
    ps::run(&script).map(|s| s.trim().to_string())
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
if ($out -match 'GUID:\s+([\w-]+)') {{
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
