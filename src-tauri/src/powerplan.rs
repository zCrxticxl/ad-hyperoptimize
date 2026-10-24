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

    let remembered = read_remembered_ultimate();
    let ultimate = plans
        .as_array()
        .and_then(|arr| find_ultimate(arr, remembered.as_deref()));
    let ultimate_available = ultimate.is_some();
    let ultimate_guid = ultimate.unwrap_or_else(|| ULTIMATE_GUID.to_string());
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

/// File in the app data dir holding the GUID `/duplicatescheme` returned
/// when this app unlocked Ultimate Performance. Plan names are localized,
/// so the GUID is the only locale-independent way to find the copy again.
fn ultimate_marker() -> Result<std::path::PathBuf, String> {
    Ok(crate::safety::state_dir()?.join("ultimate_plan_guid.txt"))
}

fn read_remembered_ultimate() -> Option<String> {
    let s = std::fs::read_to_string(ultimate_marker().ok()?).ok()?;
    let g = s.trim().to_lowercase();
    is_guid(&g).then_some(g)
}

/// Ultimate plan among `plans`: the original GUID, then the copy this app
/// remembered (only if it still exists), then English/German name matching
/// for copies created before the GUID was remembered.
fn find_ultimate(plans: &[Value], remembered: Option<&str>) -> Option<String> {
    let guid_of = |p: &Value| p["guid"].as_str().map(str::to_lowercase);
    plans
        .iter()
        .find(|p| guid_of(p).as_deref() == Some(ULTIMATE_GUID))
        .or_else(|| {
            remembered.and_then(|r| plans.iter().find(|p| guid_of(p).as_deref() == Some(r)))
        })
        .or_else(|| {
            plans.iter().find(|p| {
                let name = p["name"].as_str().unwrap_or("").to_lowercase();
                name.contains("ultimate performance") || name.contains("ultimative leistung")
            })
        })
        .and_then(guid_of)
}

/// The new scheme GUID from `/duplicatescheme` output (localized text, but
/// the GUID token itself is not). The source GUID is skipped.
fn parse_duplicated_guid(out: &str) -> Option<String> {
    out.split(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .map(str::to_lowercase)
        .find(|t| is_guid(t) && t != ULTIMATE_GUID)
}

/// Plan names are embedded in a PowerShell string: reject quotes (ASCII and
/// the typographic U+2018..U+201F that PowerShell also treats as quotes),
/// `$`, backtick and control chars.
fn validate_plan_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Plan name cannot be empty".into());
    }
    if name.len() > 128 {
        return Err("Plan name too long (max 128 chars)".into());
    }
    if !ps::is_safe_ident(name) {
        return Err("Plan name contains unsupported characters".into());
    }
    Ok(name)
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
    let guid = parse_duplicated_guid(&output)
        .ok_or_else(|| format!("Ultimate plan GUID not found in powercfg output: {output}"))?;
    // Remember the copy so later clicks reuse it on any display language
    // instead of duplicating again.
    let remember =
        ultimate_marker().and_then(|p| std::fs::write(p, &guid).map_err(|e| e.to_string()));
    set_active(guid.clone())?;
    if let Err(e) = remember {
        return Ok(format!(
            "Ultimate Performance plan created and activated: {guid} (could not remember it: {e})"
        ));
    }
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
    if !is_guid(&g) {
        return Err(format!("Invalid GUID: {guid}"));
    }
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
    // Deleting a custom plan is otherwise final: export it first, and do not
    // delete without that backup. `powercfg /import <file>` restores it.
    let dir = crate::safety::state_dir()?.join("powerplans");
    std::fs::create_dir_all(&dir).map_err(|e| format!("creating backup folder: {e}"))?;
    let backup = dir.join(format!(
        "{g}-{}.pow",
        chrono::Local::now().format("%Y%m%d%H%M%S")
    ));
    let backup_str = backup.to_string_lossy().into_owned();
    ps::exec("powercfg.exe", &["/export", &backup_str, &g])
        .map_err(|e| format!("Backup of plan {g} failed, nothing deleted: {}", e.trim()))?;
    ps::exec("powercfg.exe", &["/delete", &g])
        .map(|_| format!("Plan {g} deleted (backup: {backup_str}; restore with powercfg /import)"))
}

pub fn create_custom(name: String, base_guid: String) -> Result<String, String> {
    let name = validate_plan_name(&name)?;
    if !is_guid(&base_guid) {
        return Err(format!("Invalid base plan GUID: {base_guid}"));
    }
    // Validated above and additionally single-quoted (quote_single doubles
    // ASCII and typographic single quotes), so nothing in it is expanded.
    let quoted = ps::quote_single(name);
    let script = format!(
        r#"
$out = powercfg /duplicatescheme {base_guid} 2>&1
if ($LASTEXITCODE -eq 0 -and "$out" -match '([0-9a-fA-F]{{8}}(?:-[0-9a-fA-F]{{4}}){{3}}-[0-9a-fA-F]{{12}})') {{
    $newGuid = $Matches[1].Trim()
    $rename = powercfg /changename $newGuid {quoted} 2>&1
    $rc = $LASTEXITCODE
    if ($rc -ne 0) {{
        # Don't leave an unnamed duplicate behind.
        powercfg /delete $newGuid 2>&1 | Out-Null
        $cleanup = if ($LASTEXITCODE -eq 0) {{ 'the copy was removed' }} else {{ "the copy $newGuid could not be removed" }}
        throw "Renaming the new plan failed (exit $rc): $rename; $cleanup"
    }}
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
    use super::{find_ultimate, is_guid, parse_duplicated_guid, validate_plan_name, ULTIMATE_GUID};
    use serde_json::json;

    #[test]
    fn ultimate_copy_is_found_by_remembered_guid_on_any_locale() {
        let copy = "11111111-2222-3333-4444-555555555555";
        let plans = vec![
            json!({ "guid": "381b4222-f694-41f0-9685-ff5bb260df2e", "name": "Équilibré" }),
            json!({ "guid": copy, "name": "Performances optimales" }),
        ];
        assert_eq!(find_ultimate(&plans, Some(copy)).as_deref(), Some(copy));
        assert_eq!(find_ultimate(&plans, None), None);
        // Remembered copy was deleted: no false positive.
        assert_eq!(find_ultimate(&plans[..1], Some(copy)), None);
        let named = vec![json!({ "guid": copy, "name": "Ultimate Performance" })];
        assert_eq!(find_ultimate(&named, None).as_deref(), Some(copy));
        let orig = vec![json!({ "guid": ULTIMATE_GUID, "name": "x" })];
        assert_eq!(find_ultimate(&orig, None).as_deref(), Some(ULTIMATE_GUID));
    }

    #[test]
    fn duplicated_guid_is_parsed_locale_independent() {
        let en = "Power Scheme GUID: 11111111-2222-3333-4444-555555555555  (Ultimate Performance)";
        let de =
            "GUID des Energieschemas: 11111111-2222-3333-4444-555555555555  (Ultimative Leistung)";
        for out in [en, de] {
            assert_eq!(
                parse_duplicated_guid(out).as_deref(),
                Some("11111111-2222-3333-4444-555555555555")
            );
        }
        assert_eq!(
            parse_duplicated_guid(&format!("GUID: {ULTIMATE_GUID}")),
            None
        );
        assert_eq!(parse_duplicated_guid("error"), None);
    }

    #[test]
    fn plan_names_reject_ascii_and_typographic_quotes() {
        assert_eq!(validate_plan_name("  Gaming  "), Ok("Gaming"));
        for bad in [
            "a'b",
            "a\"b",
            "a$b",
            "a`b",
            "a\u{2018}b",
            "a\u{2019}b",
            "a\u{201C}b",
            "a\u{201D}b",
            "a\u{201E}b",
            "a\u{201F}b",
            "",
            "   ",
        ] {
            assert!(validate_plan_name(bad).is_err(), "{bad:?}");
        }
        assert!(validate_plan_name(&"x".repeat(129)).is_err());
    }

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
