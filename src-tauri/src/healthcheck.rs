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
        // sfc.exe writes its console output as UTF-16LE even into a pipe, so
        // it is read with an explicit Unicode decoder (the generic UTF-8 path
        // turns it into NUL-separated garbage). The verdict comes from this
        // stdout plus sfc's exit code, NOT from CBS.log: the filtered log
        // lines never contain the summary sentence.
        r#"
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = "$env:SystemRoot\System32\sfc.exe"
$psi.Arguments = '/scannow'
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.CreateNoWindow = $true
$psi.StandardOutputEncoding = [System.Text.Encoding]::Unicode
$p = [System.Diagnostics.Process]::Start($psi)
$out = $p.StandardOutput.ReadToEnd()
$p.WaitForExit()
$out
"SFC_EXIT:$($p.ExitCode)"
"#
        .to_string()
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
    marker_exit(lo, "dism_exit:")
}

/// Exit code appended by the script as `<marker><code>` (lowercased input).
fn marker_exit(lo: &str, marker: &str) -> Option<i64> {
    let idx = lo.find(marker)?;
    let rest = &lo[idx + marker.len()..];
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
            // sfc stdout summary sentence (English/German), checked from
            // most to least specific.
            if lo.contains("did not find any integrity violations")
                || lo.contains("no integrity violations")
                || lo.contains("keine integritätsverletzungen")
            {
                "clean"
            } else if lo.contains("found corrupt files and successfully repaired")
                || lo.contains("gefunden und erfolgreich repariert")
            {
                "repaired"
            } else if lo.contains("found corrupt files but was unable to fix")
                || lo.contains("konnte jedoch einige")
                || lo.contains("beschädigte dateien gefunden")
            {
                "corrupt"
            } else if lo.contains("could not perform the requested operation")
                || lo.contains("konnte den angeforderten vorgang nicht")
                || lo.contains("there is a system repair pending")
                || lo.contains("systemreparatur aus")
            {
                "error"
            } else {
                // Unrecognized language: a non-zero exit with no recognized
                // summary is still an honest failure; exit 0 alone does not
                // tell clean from repaired, so it stays unknown.
                match marker_exit(&lo, "sfc_exit:") {
                    Some(c) if c != 0 => "error",
                    _ => "unknown",
                }
            }
        }
        "dism_check" | "dism_scan" => {
            if lo.contains("component store is repairable")
                || lo.contains("corruption was detected")
                || lo.contains("komponentenspeicher kann repariert werden")
                || lo.contains("komponentenspeicher ist reparierbar")
            {
                "corrupt"
            } else if lo.contains("no component store corruption detected")
                || lo.contains("keine beschädigung des komponentenspeichers erkannt")
            {
                "clean"
            } else if dism_exit(&lo).is_some_and(|code| code != 0) {
                "error"
            } else {
                "unknown"
            }
        }
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

#[cfg(test)]
mod tests {
    use super::parse_result;

    #[test]
    fn sfc_verdict_from_stdout() {
        assert_eq!(
            parse_result(
                "sfc",
                "Verification 100% complete.\nWindows Resource Protection did not find any integrity violations.\nSFC_EXIT:0"
            ),
            "clean"
        );
        assert_eq!(
            parse_result(
                "sfc",
                "Der Windows-Ressourcenschutz hat keine Integritätsverletzungen gefunden.\nSFC_EXIT:0"
            ),
            "clean"
        );
        assert_eq!(
            parse_result(
                "sfc",
                "Windows Resource Protection found corrupt files and successfully repaired them.\nSFC_EXIT:0"
            ),
            "repaired"
        );
        assert_eq!(
            parse_result(
                "sfc",
                "Der Windows-Ressourcenschutz hat beschädigte Dateien gefunden und erfolgreich repariert.\nSFC_EXIT:0"
            ),
            "repaired"
        );
        assert_eq!(
            parse_result(
                "sfc",
                "Der Windows-Ressourcenschutz hat beschädigte Dateien gefunden, konnte jedoch einige nicht reparieren.\nSFC_EXIT:0"
            ),
            "corrupt"
        );
        assert_eq!(
            parse_result(
                "sfc",
                "Windows Resource Protection could not perform the requested operation.\nSFC_EXIT:1"
            ),
            "error"
        );
        assert_eq!(parse_result("sfc", "???\nSFC_EXIT:5"), "error");
        assert_eq!(parse_result("sfc", "???\nSFC_EXIT:0"), "unknown");
    }

    #[test]
    fn dism_success_does_not_hide_repairable_store() {
        assert_eq!(parse_result("dism_scan", "The component store is repairable\nThe operation completed successfully.\nDISM_EXIT:0"), "corrupt");
        assert_eq!(
            parse_result(
                "dism_scan",
                "Der Komponentenspeicher ist reparierbar.\nDISM_EXIT:0"
            ),
            "corrupt"
        );
        assert_eq!(
            parse_result(
                "dism_scan",
                "The operation completed successfully.\nDISM_EXIT:0"
            ),
            "unknown"
        );
    }
}
