//! Boot Optimizer — Event Log boot times (Event 100), BCD read/write, tweak catalog.

use crate::ps;
use serde_json::{json, Value};

// ── BCD tweak catalog ────────────────────────────────────────────────────────

struct BcdTweak {
    id:          &'static str,
    name:        &'static str,
    description: &'static str,
    impact:      &'static str,
    risk:        &'static str, // Low | Medium | High
    /// bcdedit command to apply (executed via PowerShell)
    apply_cmd:   &'static str,
    /// bcdedit command to revert
    revert_cmd:  &'static str,
    /// How to check if applied: PowerShell expression returning "true"/"false"
    check_expr:  &'static str,
}

static TWEAKS: &[BcdTweak] = &[
    BcdTweak {
        id:          "bcd_timeout_0",
        name:        "Boot-Menü Timeout auf 0",
        description: "Setzt den Boot-Menü-Timeout auf 0 Sekunden — überspringt das Auswahlmenü sofort, wenn nur ein OS vorhanden ist.",
        impact:      "Spart 3–5 s bei jedem Booten",
        risk:        "Low",
        apply_cmd:   "bcdedit /timeout 0",
        revert_cmd:  "bcdedit /timeout 30",
        check_expr:  r#"(bcdedit /enum '{bootmgr}' 2>$null | Select-String 'timeout\s+0$').Count -gt 0"#,
    },
    BcdTweak {
        id:          "bcd_no_bootlog",
        name:        "Boot-Log deaktivieren",
        description: "Deaktiviert das Schreiben der NTOSKRNL-Boot-Log-Datei (ntbtlog.txt). Nur für Diagnose benötigt.",
        impact:      "Minimal, reduziert IO beim Start",
        risk:        "Low",
        apply_cmd:   "bcdedit /set '{current}' bootlog no",
        revert_cmd:  "bcdedit /set '{current}' bootlog yes",
        check_expr:  r#"(bcdedit /enum '{current}' 2>$null | Select-String 'bootlog\s+No').Count -gt 0"#,
    },
    BcdTweak {
        id:          "bcd_no_bootux",
        name:        "Boot-Fortschrittsbalken deaktivieren",
        description: "Schaltet den animierten Windows-Ladebalken ab (quietboot). Spart etwas GPU-Initialisierungszeit.",
        impact:      "~0.1–0.3 s",
        risk:        "Low",
        apply_cmd:   "bcdedit /set '{current}' quietboot yes",
        revert_cmd:  "bcdedit /deletevalue '{current}' quietboot",
        check_expr:  r#"(bcdedit /enum '{current}' 2>$null | Select-String 'quietboot\s+Yes').Count -gt 0"#,
    },
    BcdTweak {
        id:          "bcd_numproc_all",
        name:        "Alle CPU-Kerne beim Boot nutzen",
        description: "Hebt ggf. gesetzte msconfig-Prozessorzahl-Begrenzung auf. Standard ist 0 (= alle Kerne).",
        impact:      "Parallelisierung der Treiberinitialisierung",
        risk:        "Low",
        apply_cmd:   "bcdedit /deletevalue '{current}' numproc 2>$null; $true",
        revert_cmd:  "$true",
        check_expr:  r#"(bcdedit /enum '{current}' 2>$null | Select-String 'numproc').Count -eq 0"#,
    },
    BcdTweak {
        id:          "bcd_standard_policy",
        name:        "Boot-Policy auf Standard setzen",
        description: "Setzt bootmenupolicy auf 'Standard' (modernes Boot-Menü). 'Legacy' erzwingt das alte F8-Menü und verlangsamt den Start.",
        impact:      "Schnellerer Übergang vom Bootloader",
        risk:        "Low",
        apply_cmd:   "bcdedit /set '{default}' bootmenupolicy standard",
        revert_cmd:  "bcdedit /set '{default}' bootmenupolicy legacy",
        check_expr:  r#"(bcdedit /enum '{default}' 2>$null | Select-String 'bootmenupolicy\s+Standard').Count -gt 0"#,
    },
    BcdTweak {
        id:          "bcd_no_nx_alwaysoff",
        name:        "DEP: OptOut-Modus",
        description: "Setzt DEP auf OptOut (Standard-Sicherheitsniveau, nicht deaktiviert). Verhindert langsamere AlwaysOn-Überprüfungen bei Programmen ohne DEP-Opt-in.",
        impact:      "Minimal",
        risk:        "Medium",
        apply_cmd:   "bcdedit /set '{current}' nx OptOut",
        revert_cmd:  "bcdedit /set '{current}' nx OptIn",
        check_expr:  r#"(bcdedit /enum '{current}' 2>$null | Select-String 'nx\s+OptOut').Count -gt 0"#,
    },
    BcdTweak {
        id:          "bcd_disable_integrity",
        name:        "Driver-Signaturen: TESTSIGNING aus",
        description: "Stellt sicher, dass TESTSIGNING deaktiviert ist (unsignierte Treiber blockiert). Schützt Bootzeit vor rogue-Treibern.",
        impact:      "Sicherheits-Hygiene",
        risk:        "Low",
        apply_cmd:   "bcdedit /set testsigning off",
        revert_cmd:  "bcdedit /set testsigning on",
        check_expr:  r#"(bcdedit /enum 2>$null | Select-String 'testsigning\s+No').Count -gt 0"#,
    },
];

// ── PowerShell helpers ───────────────────────────────────────────────────────

/// Query Event Log for Kernel-Boot Event ID 100 (BootDuration in ms).
/// Returns last N entries.
/// Returns (events, log_was_disabled). If the log was disabled we just enabled it
/// but won't have history yet — the caller should surface this to the user.
fn query_boot_events(limit: usize) -> (Vec<Value>, bool) {
    let script = format!(r#"
$logName = 'Microsoft-Windows-Diagnostics-Performance/Operational'
$wasDisabled = $false
$logInfo = Get-WinEvent -ListLog $logName -ErrorAction SilentlyContinue
if ($logInfo -and -not $logInfo.IsEnabled) {{
    $wasDisabled = $true
    wevtutil sl $logName /e:true 2>$null
}}
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
"#, limit = limit);

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
/// Uses /enum {current} + /enum {bootmgr} — works without admin for read.
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

/// Check applied status for all tweaks — one PS call, one variable per tweak.
fn check_statuses() -> std::collections::HashMap<String, bool> {
    // Each tweak gets its own $var assignment; collect into a hashtable for JSON.
    let assignments: Vec<String> = TWEAKS.iter().map(|t| {
        // Replace hyphens so the variable name is valid PS identifier
        let var = t.id.replace('-', "_");
        format!("${var} = try {{ [bool]({}) }} catch {{ $false }};", t.check_expr)
    }).collect();
    let entries: Vec<String> = TWEAKS.iter().map(|t| {
        let var = t.id.replace('-', "_");
        format!("'{}' = ${};", t.id, var)
    }).collect();
    let script = format!(
        "{} $r = @{{{}}}; $r | ConvertTo-Json -Compress",
        assignments.join(" "),
        entries.join(" ")
    );
    let raw = ps::run(&script).unwrap_or_default();
    let v: Value = serde_json::from_str(raw.trim()).unwrap_or(json!({}));
    TWEAKS.iter().map(|t| {
        let applied = v[t.id].as_bool().unwrap_or(false);
        (t.id.to_string(), applied)
    }).collect()
}

// ── public API ───────────────────────────────────────────────────────────────

pub fn scan() -> Value {
    let (boot_events, log_was_disabled) = query_boot_events(20);
    let bcd      = read_bcd();
    let statuses = check_statuses();

    let last_boot_ms = boot_events.first()
        .and_then(|e| e["BootDurationMs"].as_i64())
        .filter(|&ms| ms > 0)
        .unwrap_or(-1);

    let tweaks: Vec<Value> = TWEAKS.iter().map(|t| {
        let applied = *statuses.get(t.id).unwrap_or(&false);
        json!({
            "id":          t.id,
            "name":        t.name,
            "description": t.description,
            "impact":      t.impact,
            "risk":        t.risk,
            "applied":     applied,
        })
    }).collect();

    json!({
        "bootEvents":      boot_events,
        "lastBootMs":      last_boot_ms,
        "logWasDisabled":  log_was_disabled,
        "bcd":             bcd,
        "tweaks":          tweaks,
    })
}

pub fn apply_tweak(id: String) -> Result<Value, String> {
    let t = TWEAKS.iter().find(|t| t.id == id)
        .ok_or_else(|| format!("Unknown tweak: {id}"))?;
    ps::run(t.apply_cmd).map_err(|e| e)?;
    Ok(json!({ "ok": true, "id": id }))
}

pub fn revert_tweak(id: String) -> Result<Value, String> {
    let t = TWEAKS.iter().find(|t| t.id == id)
        .ok_or_else(|| format!("Unknown tweak: {id}"))?;
    ps::run(t.revert_cmd).map_err(|e| e)?;
    Ok(json!({ "ok": true, "id": id }))
}
