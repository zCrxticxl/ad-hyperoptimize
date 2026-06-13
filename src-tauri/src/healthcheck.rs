//! SFC + DISM health check runner.
//! All checks require admin. SFC can take 5-15 min; DISM RestoreHealth even longer.

use crate::ps;
use serde_json::{json, Value};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Run a single health check and return full output + parsed result.
pub fn run(kind: String) -> Result<Value, String> {
    if !ps::is_admin() {
        return Err("Administrator-Rechte erforderlich. App als Administrator starten.".into());
    }

    let (label, args): (&str, Vec<&str>) = match kind.as_str() {
        "sfc" => ("SFC /scannow", vec!["sfc", "/scannow"]),
        "dism_check"   => ("DISM CheckHealth",   vec!["DISM", "/Online", "/Cleanup-Image", "/CheckHealth"]),
        "dism_scan"    => ("DISM ScanHealth",    vec!["DISM", "/Online", "/Cleanup-Image", "/ScanHealth"]),
        "dism_restore" => ("DISM RestoreHealth", vec!["DISM", "/Online", "/Cleanup-Image", "/RestoreHealth"]),
        "dism_component" => ("DISM ComponentCleanup", vec!["DISM", "/Online", "/Cleanup-Image", "/StartComponentCleanup"]),
        _ => return Err(format!("Unknown check kind: {kind}")),
    };

    // Run via cmd to capture output properly (SFC writes Unicode via kernel)
    let script = if kind == "sfc" {
        // SFC needs special handling — its output is UTF-16 via kernel driver.
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
        format!("& {} 2>&1 | Out-String", args.join(" "))
    };

    let output = ps::run(&script).map_err(|e| e)?;
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

fn parse_result(kind: &str, output: &str) -> &'static str {
    let lo = output.to_lowercase();
    match kind {
        "sfc" => {
            if lo.contains("no integrity violations") || lo.contains("did not find any integrity violations") {
                "clean"
            } else if lo.contains("found corrupt files and successfully repaired") {
                "repaired"
            } else if lo.contains("found corrupt files but was unable to fix") {
                "corrupt"
            } else {
                "unknown"
            }
        }
        "dism_check" | "dism_scan" => {
            if lo.contains("no component store corruption detected") {
                "clean"
            } else if lo.contains("component store is repairable") || lo.contains("corruption was detected") {
                "corrupt"
            } else if lo.contains("the operation completed successfully") {
                "clean"
            } else {
                "unknown"
            }
        }
        "dism_restore" => {
            if lo.contains("the restore operation completed successfully") || lo.contains("the operation completed successfully") {
                "repaired"
            } else if lo.contains("the source files could not be found") {
                "error"
            } else {
                "unknown"
            }
        }
        "dism_component" => {
            if lo.contains("the operation completed successfully") { "clean" } else { "unknown" }
        }
        _ => "unknown",
    }
}
