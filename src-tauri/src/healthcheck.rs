//! SFC + DISM health check runner.
//! All checks require admin. SFC can take 5-15 min; DISM RestoreHealth even longer.

use crate::ps;
use serde_json::{json, Value};

/// Run a single health check and return full output + parsed result.
pub fn run(kind: String) -> Result<Value, String> {
    if !ps::is_admin() {
        return Err("Administrator-Rechte erforderlich. App als Administrator starten.".into());
    }

    let (label, args): (&str, Vec<&str>) = match kind.as_str() {
        "sfc" => ("SFC /scannow", vec!["sfc", "/scannow"]),
        "dism_check" => (
            "DISM CheckHealth",
            vec!["DISM", "/Online", "/Cleanup-Image", "/CheckHealth"],
        ),
        "dism_scan" => (
            "DISM ScanHealth",
            vec!["DISM", "/Online", "/Cleanup-Image", "/ScanHealth"],
        ),
        "dism_restore" => (
            "DISM RestoreHealth",
            vec!["DISM", "/Online", "/Cleanup-Image", "/RestoreHealth"],
        ),
        "dism_component" => (
            "DISM ComponentCleanup",
            vec![
                "DISM",
                "/Online",
                "/Cleanup-Image",
                "/StartComponentCleanup",
            ],
        ),
        _ => return Err(format!("Unknown check kind: {kind}")),
    };

    // Run via cmd to capture output properly (SFC writes Unicode via kernel)
    let script = if kind == "sfc" {
        // SFC needs special handling, its output is UTF-16 via kernel driver.
        // Reading CBS.log gives the actual result.
        r#"
$job = Start-Process -FilePath 'sfc.exe' -ArgumentList '/scannow' -Wait -PassThru -NoNewWindow 2>$null
$cbsPath = "$env:SystemRoot\Logs\CBS\CBS.log"
$summary = ''
if (Test-Path $cbsPath) {
    $lines = Get-Content $cbsPath -Tail 50 -ErrorAction SilentlyContinue
    $summary = ($lines | Where-Object { $_ -match 'SFC' -or $_ -match 'Integrity' -or $_ -match 'repair' }) -join "`n"
}
if ($summary) { $summary } else { "SFC abgeschlossen (Exit: $($job.ExitCode)). Log: $cbsPath" }
"#.to_string()
    } else {
        // DISM output is localized; the exit code is not. Append a marker the
        // parser reads instead of trusting English phrases.
        format!(
            "& {} 2>&1 | Out-String; \"DISM_EXIT:$($LASTEXITCODE)\"",
            args.join(" ")
        )
    };

    // SFC /scannow and DISM checks/repairs legitimately take minutes, the
    // short 30s command timeout would kill them mid-operation, so use the
    // long (20 min) timeout here.
    let output = ps::run_long(&script)?;
    let clean: String = output
        .lines()
        .map(|l| l.rsplit('\r').next().unwrap_or("").to_string())
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    let result = parse_result(&kind, &clean);

    Ok(json!({
        "kind":   kind,
        "label":  label,
        "output": clean,
        "result": result,
    }))
}

/// Locale-independent DISM verdict: "DISM_EXIT:<code>" from the script.
fn dism_exit(lo: &str) -> Option<i64> {
    let idx = lo.find("dism_exit:")?;
    let rest = &lo[idx + "dism_exit:".len()..];
    let tok: String = rest
        .chars()
        .skip_while(|c| !c.is_ascii_digit() && *c != '-')
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    tok.parse().ok()
}

fn parse_result(kind: &str, output: &str) -> &'static str {
    let lo = output.to_lowercase();
    match kind {
        "sfc" => {
            if lo.contains("no integrity violations")
                || lo.contains("did not find any integrity violations")
            {
                "clean"
            } else if lo.contains("found corrupt files and successfully repaired") {
                "repaired"
            } else if lo.contains("found corrupt files but was unable to fix") {
                "corrupt"
            } else {
                "unknown"
            }
        }
        "dism_check" | "dism_scan" => match dism_exit(&lo) {
            Some(0) => "clean",
            Some(_) => "corrupt",
            // Legacy fallback for output without the marker.
            None => {
                if lo.contains("no component store corruption detected")
                    || lo.contains("the operation completed successfully")
                {
                    "clean"
                } else if lo.contains("component store is repairable")
                    || lo.contains("corruption was detected")
                {
                    "corrupt"
                } else {
                    "unknown"
                }
            }
        },
        "dism_restore" => match dism_exit(&lo) {
            Some(0) => "repaired",
            Some(_) => "error",
            None => {
                if lo.contains("the restore operation completed successfully")
                    || lo.contains("the operation completed successfully")
                {
                    "repaired"
                } else if lo.contains("the source files could not be found") {
                    "error"
                } else {
                    "unknown"
                }
            }
        },
        "dism_component" => match dism_exit(&lo) {
            Some(0) => "clean",
            Some(_) => "unknown",
            None if lo.contains("the operation completed successfully") => "clean",
            _ => "unknown",
        },
        _ => "unknown",
    }
}
