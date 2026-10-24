//! Boot Optimizer, Event Log boot times (Event 100), BCD read/write, tweak catalog.

use crate::ps;
use serde_json::{json, Value};

// ── BCD tweak catalog ────────────────────────────────────────────────────────
//
// Every tweak names one BCD element on one object and the value it wants
// (None = the element is deleted). Apply captures the element's current value
// from `bcdedit /enum <object> /v` first and journals an exact revert
// (`/set` back to the captured value, or `/deletevalue` if it was absent).
// Element names in bcdedit output are never localized; boolean VALUES are
// ("Yes"/"Ja", "No"/"Nein"), so they are normalized before comparison.

struct BcdTweak {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    impact: &'static str,
    risk: &'static str, // Low | Medium | High
    /// BCD object the element lives on ({bootmgr}, {current}, {default}).
    target: &'static str,
    /// Untranslated BCD element name as printed by `bcdedit /enum /v`.
    element: &'static str,
    /// Desired value (`bcdedit /set` syntax), None = delete the element.
    desired: Option<&'static str>,
}

static TWEAKS: &[BcdTweak] = &[
    BcdTweak {
        id: "bcd_timeout_0",
        name: "Set Boot Menu Timeout to 0",
        description: "Sets the boot menu timeout to 0 seconds, skips the selection menu instantly when only one OS is present.",
        impact: "Saves 3-5 s on every boot",
        risk: "Low",
        target: "{bootmgr}",
        element: "timeout",
        desired: Some("0"),
    },
    BcdTweak {
        id: "bcd_numproc_all",
        name: "Alle CPU-Kerne beim Boot nutzen",
        description: "Hebt ggf. gesetzte msconfig-Prozessorzahl-Begrenzung auf. Standard ist 0 (= alle Kerne).",
        impact: "Parallelisierung der Treiberinitialisierung",
        risk: "Low",
        target: "{current}",
        element: "numproc",
        desired: None,
    },
    BcdTweak {
        id: "bcd_standard_policy",
        name: "Boot-Policy auf Standard setzen",
        description: "Sets bootmenupolicy to 'standard' (modern boot menu). 'legacy' forces the old F8 menu and slows startup.",
        impact: "Faster bootloader transition",
        risk: "Low",
        target: "{default}",
        element: "bootmenupolicy",
        desired: Some("standard"),
    },
    BcdTweak {
        id: "bcd_disable_integrity",
        name: "Driver-Signaturen: TESTSIGNING aus",
        description: "Ensures TESTSIGNING is disabled (unsigned drivers blocked). Protects boot time from rogue drivers.",
        impact: "Sicherheits-Hygiene",
        risk: "Low",
        target: "{current}",
        element: "testsigning",
        desired: Some("off"),
    },
];

// Retired tweaks: no longer offered for apply, but still revertable for
// machines where an older version applied them.
// - bcd_no_bootlog: `bootlog no` is already the Windows default (no
//   ntbtlog.txt is written unless bootlog is enabled), so it did nothing.
// - bcd_no_bootux: `quietboot` only hides the boot animation; it has no
//   measurable boot-time effect.
// - bcd_no_nx_alwaysoff: set DEP to OptOut, which is NOT the client default
//   (OptIn) and is not "the standard security level" as its description
//   claimed. Reverting restores the Windows client default `nx OptIn`.
struct RetiredBcdTweak {
    id: &'static str,
    name: &'static str,
    target: &'static str,
    element: &'static str,
    /// Value an old apply set (normalized), used to detect "still applied".
    applied_value: &'static str,
    /// Exact bcdedit argv that reverts it.
    revert_args: &'static [&'static str],
}

static RETIRED: &[RetiredBcdTweak] = &[
    RetiredBcdTweak {
        id: "bcd_no_bootlog",
        name: "Disable Boot Log (retired)",
        target: "{current}",
        element: "bootlog",
        applied_value: "no",
        revert_args: &["/deletevalue", "{current}", "bootlog"],
    },
    RetiredBcdTweak {
        id: "bcd_no_bootux",
        name: "Disable Boot Progress Bar (retired)",
        target: "{current}",
        element: "quietboot",
        applied_value: "yes",
        revert_args: &["/deletevalue", "{current}", "quietboot"],
    },
    RetiredBcdTweak {
        id: "bcd_no_nx_alwaysoff",
        name: "DEP: OptOut-Modus (retired)",
        target: "{current}",
        element: "nx",
        applied_value: "optout",
        revert_args: &["/set", "{current}", "nx", "OptIn"],
    },
];

// ── PowerShell helpers ───────────────────────────────────────────────────────

/// Query Event Log for Kernel-Boot Event ID 100 (BootDuration in ms).
/// Returns last N entries.
/// Returns (events, log_was_disabled). If the log was disabled we just enabled it
/// but won't have history yet, the caller should surface this to the user.
fn query_boot_events(limit: usize) -> (Vec<Value>, bool) {
    let script = format!(
        r#"
$logName = 'Microsoft-Windows-Diagnostics-Performance/Operational'
$wasDisabled = $false
$logInfo = Get-WinEvent -ListLog $logName -ErrorAction SilentlyContinue
# Read-only: the log stays as the user configured it. Enabling a
# disabled diagnostic log is a persistent system change with no undo.
$wasDisabled = $logInfo -and -not $logInfo.IsEnabled
$events = Get-WinEvent -FilterHashtable @{{LogName=$logName; Id=100}} `
    -MaxEvents {limit} -ErrorAction SilentlyContinue
$results = @()
if ($events) {{
    foreach ($e in $events) {{
        $xml  = [xml]$e.ToXml()
        $data = @{{}}
        foreach ($d in $xml.Event.EventData.Data) {{ $data[$d.Name] = $d.'#text' }}
        $bd  = if ($data['BootDuration'])          {{ [int]$data['BootDuration'] }}          else {{ 0 }}
        $mp  = if ($data['MainPathBootTime'])       {{ [int]$data['MainPathBootTime'] }}       else {{ 0 }}
        $pb  = if ($data['BootPostBootTime'])       {{ [int]$data['BootPostBootTime'] }}       else {{ 0 }}
        $sd  = if ($data['SystemDriveInitialization']) {{ [int]$data['SystemDriveInitialization'] }} else {{ 0 }}
        $results += [PSCustomObject]@{{
            TimeCreated       = $e.TimeCreated.ToString('o')
            BootDurationMs    = $bd
            MainPathBootMs    = $mp
            BootPostBootMs    = $pb
            SystemDriveInitMs = $sd
        }}
    }}
}}
[PSCustomObject]@{{
    WasDisabled = $wasDisabled
    Events      = $results
}} | ConvertTo-Json -Compress -Depth 4
"#,
        limit = limit
    );

    let raw = ps::run(&script).unwrap_or_default();
    let v: Value = serde_json::from_str(raw.trim()).unwrap_or(json!({}));
    let was_disabled = v["WasDisabled"].as_bool().unwrap_or(false);
    let events = match v.get("Events") {
        Some(Value::Array(a)) => a.clone(),
        Some(o @ Value::Object(_)) => vec![o.clone()],
        _ => vec![],
    };
    (events, was_disabled)
}

/// Read relevant BCD values via bcdedit.
/// Uses /enum {current} + /enum {bootmgr}, works without admin for read.
fn read_bcd() -> Value {
    let script = r#"
$out = @{}
# Try both current loader and boot manager
$targets = @('{current}', '{bootmgr}', '{default}')
foreach ($t in $targets) {
    $lines = bcdedit /enum $t 2>$null
    if (-not $lines) { continue }
    foreach ($line in $lines) {
        if ($line -match '^\s*(\w+)\s{2,}(.+)$') {
            $key = $Matches[1].Trim(); $val = $Matches[2].Trim()
            if ($key -ne 'identifier' -and -not $out.ContainsKey($key)) {
                $out[$key] = $val
            }
        }
    }
}
if ($out.Count -eq 0) { Write-Output '{}'; return }
$out | ConvertTo-Json -Compress
"#;
    ps::run(script)
        .ok()
        .and_then(|s| serde_json::from_str(s.trim()).ok())
        .unwrap_or(json!({}))
}

// ── BCD element parsing ──────────────────────────────────────────────────────

/// Parse `bcdedit /enum <object> /v` output into lowercase element → value.
/// Element lines start at column 0 with the (untranslated) element name,
/// followed by 2+ spaces and the value; headers, separators and indented
/// continuation lines are skipped.
fn parse_bcd_elements(out: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for line in out.lines() {
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let Some(pos) = line.find("  ") else {
            continue;
        };
        let key = &line[..pos];
        let val = line[pos..].trim();
        if key.is_empty()
            || val.is_empty()
            || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            continue;
        }
        map.entry(key.to_ascii_lowercase())
            .or_insert_with(|| val.to_string());
    }
    map
}

/// Normalize a BCD value for comparison/write-back: localized booleans
/// (English/German) and on/off map to "yes"/"no"; everything else is
/// lowercased. Returns None for values that are unsafe to pass back to
/// bcdedit (anything but ASCII alphanumerics).
fn normalize_bcd_value(v: &str) -> Option<String> {
    let t = v.trim().to_ascii_lowercase();
    let t = match t.as_str() {
        "yes" | "ja" | "on" | "true" => "yes".to_string(),
        "no" | "nein" | "off" | "false" => "no".to_string(),
        _ => t,
    };
    (!t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric())).then_some(t)
}

/// True when `current` (None = element absent) equals the desired state.
fn is_at_desired(current: Option<&str>, desired: Option<&str>) -> bool {
    match (current, desired) {
        (None, None) => true,
        (Some(c), Some(d)) => normalize_bcd_value(c) == normalize_bcd_value(d),
        _ => false,
    }
}

/// Build the bcdedit argv that restores `prev` (None = delete the element).
fn restore_args(target: &str, element: &str, prev: Option<&str>) -> Result<Vec<String>, String> {
    match prev {
        None => Ok(vec!["/deletevalue".into(), target.into(), element.into()]),
        Some(v) => {
            let n = normalize_bcd_value(v)
                .ok_or_else(|| format!("cannot capture current {element} value '{v}'"))?;
            Ok(vec!["/set".into(), target.into(), element.into(), n])
        }
    }
}

fn read_elements(target: &str) -> Result<std::collections::HashMap<String, String>, String> {
    ps::exec("bcdedit.exe", &["/enum", target, "/v"]).map(|o| parse_bcd_elements(&o))
}

/// Check applied status for all tweaks. Unreadable BCD (non-admin) = false.
fn check_statuses() -> std::collections::HashMap<String, bool> {
    let mut cache: std::collections::HashMap<
        &str,
        Option<std::collections::HashMap<String, String>>,
    > = std::collections::HashMap::new();
    let mut out = std::collections::HashMap::new();
    for t in TWEAKS {
        let elems = cache
            .entry(t.target)
            .or_insert_with(|| read_elements(t.target).ok());
        let applied = elems
            .as_ref()
            .is_some_and(|m| is_at_desired(m.get(t.element).map(String::as_str), t.desired));
        out.insert(t.id.to_string(), applied);
    }
    for r in RETIRED {
        let elems = cache
            .entry(r.target)
            .or_insert_with(|| read_elements(r.target).ok());
        let applied = elems.as_ref().is_some_and(|m| {
            m.get(r.element)
                .and_then(|v| normalize_bcd_value(v))
                .is_some_and(|v| v == r.applied_value)
        });
        out.insert(r.id.to_string(), applied);
    }
    out
}

// ── public API ───────────────────────────────────────────────────────────────

pub fn scan() -> Value {
    let (boot_events, log_was_disabled) = query_boot_events(20);
    let bcd = read_bcd();
    let statuses = check_statuses();

    let last_boot_ms = boot_events
        .first()
        .and_then(|e| e["BootDurationMs"].as_i64())
        .filter(|&ms| ms > 0)
        .unwrap_or(-1);

    let mut tweaks: Vec<Value> = TWEAKS
        .iter()
        .map(|t| {
            let applied = *statuses.get(t.id).unwrap_or(&false);
            json!({
                "id":          t.id,
                "name":        t.name,
                "description": t.description,
                "impact":      t.impact,
                "risk":        t.risk,
                "applied":     applied,
            })
        })
        .collect();
    // Retired tweaks are only listed while still applied, so they can be
    // reverted; they can never be applied again.
    for r in RETIRED {
        if *statuses.get(r.id).unwrap_or(&false) {
            tweaks.push(json!({
                "id":          r.id,
                "name":        r.name,
                "description": "Retired: this setting had no benefit (or weakened DEP). Revert restores the Windows default.",
                "impact":      "None",
                "risk":        "Low",
                "applied":     true,
                "retired":     true,
            }));
        }
    }

    json!({
        "bootEvents":      boot_events,
        "lastBootMs":      last_boot_ms,
        "logWasDisabled":  log_was_disabled,
        "bcd":             bcd,
        "tweaks":          tweaks,
    })
}

/// (BCD object, element) a current or retired tweak changes.
pub(crate) fn declared_element(id: &str) -> Option<(&'static str, &'static str)> {
    TWEAKS
        .iter()
        .find(|t| t.id == id)
        .map(|t| (t.target, t.element))
        .or_else(|| {
            RETIRED
                .iter()
                .find(|r| r.id == id)
                .map(|r| (r.target, r.element))
        })
}

fn tweak_journal_id(id: &str) -> String {
    format!("bcd:{id}")
}

pub fn apply_tweak(id: String) -> Result<Value, String> {
    if RETIRED.iter().any(|r| r.id == id) {
        return Err(format!(
            "{id} has been retired and can no longer be applied"
        ));
    }
    let t = TWEAKS
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("Unknown tweak: {id}"))?;
    if !ps::is_admin() {
        return Err("Changing boot configuration requires administrator rights".into());
    }

    // Capture the current value first; without it there is no exact undo.
    let elems = read_elements(t.target)
        .map_err(|e| format!("reading BCD {} failed: {}", t.target, e.trim()))?;
    let prev = elems.get(t.element).map(String::as_str);
    if is_at_desired(prev, t.desired) {
        return Ok(json!({ "ok": true, "id": id, "alreadySet": true }));
    }
    let revert = restore_args(t.target, t.element, prev)?;
    let applied: Vec<String> = match t.desired {
        Some(v) => vec!["/set".into(), t.target.into(), t.element.into(), v.into()],
        None => vec!["/deletevalue".into(), t.target.into(), t.element.into()],
    };

    let _engine = crate::tweaks::engine_guard()?;
    let entry_id = crate::safety::append_entry(crate::safety::JournalEntry {
        id: format!(
            "bcd-{id}-{}",
            chrono::Local::now().format("%Y%m%d%H%M%S%6f")
        ),
        tweak_id: tweak_journal_id(&id),
        tweak_name: format!("Boot: {}", t.name),
        origin: None,
        time: chrono::Local::now().to_rfc3339(),
        items: vec![crate::safety::ChangeItem::Command {
            // run_cmdline splits on whitespace and execs bcdedit directly
            // (argv, no shell), so the braces stay literal.
            applied: format!("bcdedit {}", applied.join(" ")),
            revert: format!("bcdedit {}", revert.join(" ")),
        }],
        reverted: false,
        backup_files: vec![],
        completed: false,
        attempted: 1,
        reverted_items: 0,
        untrusted: false,
    })?;

    let args: Vec<&str> = applied.iter().map(String::as_str).collect();
    // bcdedit exits non-zero on failure (access denied, unknown element);
    // re-read afterwards so a silent no-op is reported too.
    let result = ps::exec("bcdedit.exe", &args).and_then(|_| {
        let now = read_elements(t.target)?;
        if is_at_desired(now.get(t.element).map(String::as_str), t.desired) {
            Ok(())
        } else {
            Err(format!("{} did not change", t.element))
        }
    });
    crate::safety::with_journal(|entries| {
        let entry = entries
            .iter_mut()
            .find(|e| e.id == entry_id)
            .ok_or("BCD journal entry missing")?;
        entry.completed = result.is_ok();
        Ok(())
    })?;
    result.map_err(|e| format!("Applying {id} failed: {}", e.trim()))?;
    Ok(json!({ "ok": true, "id": id, "entryId": entry_id }))
}

pub fn revert_tweak(id: String) -> Result<Value, String> {
    if let Some(r) = RETIRED.iter().find(|r| r.id == id) {
        if !ps::is_admin() {
            return Err("Changing boot configuration requires administrator rights".into());
        }
        ps::exec("bcdedit.exe", r.revert_args)
            .map_err(|e| format!("Reverting {id} failed: {}", e.trim()))?;
        return Ok(json!({ "ok": true, "id": id }));
    }
    if !TWEAKS.iter().any(|t| t.id == id) {
        return Err(format!("Unknown tweak: {id}"));
    }
    // Exact undo from the journal (restores the captured previous value).
    let jid = tweak_journal_id(&id);
    let entry = crate::safety::load_journal()?
        .into_iter()
        .rev()
        .find(|e| e.tweak_id == jid && !e.reverted)
        .ok_or_else(|| {
            format!(
                "No recorded previous value for {id} (applied by an older version or outside the app); nothing was changed"
            )
        })?;
    crate::tweaks::revert_entry(&entry.id)?;
    Ok(json!({ "ok": true, "id": id, "entryId": entry.id }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENUM_DE: &str = "Windows-Startladeprogramm\n-------------------------\nidentifier              {9dea862c-5cdd-4e70-acc1-f32b344d4795}\ndevice                  partition=C:\ndescription             Windows 11\nlocale                  de-DE\nbootlog                 Nein\nnx                      OptIn\nbootmenupolicy          Standard\ntestsigning             Ja\nnumproc                 4\ntimeout                 30\ndisplayorder            {current}\n                        {a1b2}\n";

    #[test]
    fn parses_localized_enum_output() {
        let m = parse_bcd_elements(ENUM_DE);
        assert_eq!(
            m.get("bootmenupolicy").map(String::as_str),
            Some("Standard")
        );
        assert_eq!(m.get("testsigning").map(String::as_str), Some("Ja"));
        assert_eq!(m.get("numproc").map(String::as_str), Some("4"));
        assert_eq!(m.get("timeout").map(String::as_str), Some("30"));
        assert!(!m.contains_key("windows-startladeprogramm"));
        assert!(!m.contains_key("-------------------------"));
    }

    #[test]
    fn normalizes_localized_booleans() {
        assert_eq!(normalize_bcd_value("Ja").as_deref(), Some("yes"));
        assert_eq!(normalize_bcd_value("Yes").as_deref(), Some("yes"));
        assert_eq!(normalize_bcd_value("Nein").as_deref(), Some("no"));
        assert_eq!(normalize_bcd_value("off").as_deref(), Some("no"));
        assert_eq!(normalize_bcd_value("Legacy").as_deref(), Some("legacy"));
        assert_eq!(normalize_bcd_value("a b"), None);
        assert_eq!(normalize_bcd_value("{x}"), None);
    }

    #[test]
    fn desired_state_comparison() {
        assert!(is_at_desired(Some("Nein"), Some("off")));
        assert!(!is_at_desired(Some("Ja"), Some("off")));
        assert!(is_at_desired(None, None));
        assert!(!is_at_desired(Some("4"), None));
        assert!(is_at_desired(Some("0"), Some("0")));
        assert!(!is_at_desired(None, Some("0")));
    }

    #[test]
    fn revert_restores_captured_value_exactly() {
        assert_eq!(
            restore_args("{bootmgr}", "timeout", Some("30")).unwrap(),
            vec!["/set", "{bootmgr}", "timeout", "30"]
        );
        assert_eq!(
            restore_args("{current}", "numproc", None).unwrap(),
            vec!["/deletevalue", "{current}", "numproc"]
        );
        assert_eq!(
            restore_args("{current}", "testsigning", Some("Ja")).unwrap(),
            vec!["/set", "{current}", "testsigning", "yes"]
        );
        assert!(restore_args("{current}", "x", Some("a;b")).is_err());
    }

    #[test]
    fn retired_tweaks_are_not_active() {
        for r in RETIRED {
            assert!(!TWEAKS.iter().any(|t| t.id == r.id), "{}", r.id);
        }
        let nx = RETIRED
            .iter()
            .find(|r| r.id == "bcd_no_nx_alwaysoff")
            .unwrap();
        assert_eq!(nx.revert_args, &["/set", "{current}", "nx", "OptIn"]);
    }
}
