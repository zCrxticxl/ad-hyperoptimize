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

    let has_ultimate = match ps::run_json(script) {
        Ok(Value::Array(ref arr)) => arr.iter().any(|p| {
            p["guid"].as_str().unwrap_or("").contains(&ULTIMATE_GUID[..8])
        }),
        _ => false,
    };

    let plans = match ps::run_json(script) {
        Ok(v @ Value::Array(_)) => v,
        Ok(v @ Value::Object(_)) => Value::Array(vec![v]),
        _ => Value::Array(vec![]),
    };

    json!({
        "plans": plans,
        "ultimateAvailable": has_ultimate,
        "ultimateGuid": ULTIMATE_GUID,
    })
}

pub fn set_active(guid: String) -> Result<String, String> {
    // Validate GUID format (basic)
    if !guid.chars().all(|c| c.is_ascii_hexdigit() || c == '-') || guid.len() < 32 {
        return Err(format!("Invalid GUID: {guid}"));
    }
    let out = ps::exec("powercfg.exe", &["/setactive", &guid])
        .map_err(|e| e)?;
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
    ps::run(&script)
        .map(|s| s.trim().to_string())
        .map_err(|e| e)
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
    ps::exec("powercfg.exe", &["/delete", &guid])
        .map(|_| format!("Plan {guid} deleted"))
        .map_err(|e| e)
}

pub fn create_custom(name: String, base_guid: String) -> Result<String, String> {
    if name.is_empty() { return Err("Plan name cannot be empty".into()); }
    // Duplicate the base plan then rename it
    let safe_name = name.replace('"', "\\\"");
    let script = format!(
        r#"
$out = powercfg /duplicatescheme {base_guid} 2>&1
if ($out -match 'GUID:\s+([\w-]+)') {{
    $newGuid = $Matches[1].Trim()
    powercfg /changename $newGuid "{safe_name}" 2>$null
    "Created: $newGuid"
}} else {{
    throw "Failed to duplicate scheme: $out"
}}
"#
    );
    ps::run(&script)
        .map(|s| s.trim().to_string())
        .map_err(|e| e)
}

pub fn get_plan_details(guid: String) -> Value {
    // Get individual plan settings via powercfg /query
    let script = format!(
        r#"
$q = powercfg /query {guid} 2>$null
$settings = @()
$current = $null
$q | ForEach-Object {{
    if ($_ -match 'Power Setting GUID:\s+([\w-]+)\s+\((.+?)\)') {{
        $current = [PSCustomObject]@{{ guid=$Matches[1]; name=$Matches[2]; ac=$null; dc=$null }}
    }} elseif ($current -and $_ -match 'Current AC Power Setting Index: (0x[\w]+)') {{
        $current.ac = [Convert]::ToInt64($Matches[1], 16)
    }} elseif ($current -and $_ -match 'Current DC Power Setting Index: (0x[\w]+)') {{
        $current.dc = [Convert]::ToInt64($Matches[1], 16)
        $settings += $current; $current = $null
    }}
}}
[PSCustomObject]@{{ guid='{guid}'; settings=@($settings) }} | ConvertTo-Json -Depth 3 -Compress
"#
    );
    ps::run_json(&script).unwrap_or_else(|e| json!({ "error": e }))
}
