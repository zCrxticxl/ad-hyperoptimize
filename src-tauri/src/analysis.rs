//! Rule-based analysis engine: turns raw scan data into prioritized,
//! human-readable findings with severity scores and linked tweaks.
//! Deliberately deterministic & explainable — every finding states its
//! evidence. (An LLM hook could be layered on top; nothing here pretends.)

use serde_json::{json, Value};

struct Finding {
    severity: u8, // 1 info, 2 low, 3 medium, 4 high, 5 critical
    title: String,
    detail: String,
    recommendation: String,
    tweak_ids: Vec<&'static str>,
}

fn f(severity: u8, title: &str, detail: String, rec: &str, tweaks: Vec<&'static str>) -> Finding {
    Finding { severity, title: title.into(), detail, recommendation: rec.into(), tweak_ids: tweaks }
}

pub fn analyze(scan: &Value, security: &Value, cleanup: &Value) -> Value {
    let mut findings: Vec<Finding> = Vec::new();

    // RAM pressure
    if let (Some(total), Some(free)) = (
        scan["os"]["TotalVisibleMemorySize"].as_u64(),
        scan["os"]["FreePhysicalMemory"].as_u64(),
    ) {
        let used_pct = 100.0 - (free as f64 / total as f64 * 100.0);
        if used_pct > 85.0 {
            findings.push(f(4, "High memory pressure",
                format!("{used_pct:.0}% of {:.1} GB RAM in use at scan time. Sustained >85% causes paging and stutter.", total as f64 / 1_048_576.0),
                "Review top memory processes on the Monitor page; consider disabling UWP background apps.",
                vec!["background_apps_off"]));
        }
    }

    // System drive nearly full
    if let Some(vols) = scan["volumes"].as_array() {
        for v in vols {
            if v["DriveLetter"].as_str() == Some("C") {
                if let (Some(size), Some(rem)) = (v["Size"].as_f64(), v["SizeRemaining"].as_f64()) {
                    let free_pct = rem / size * 100.0;
                    if free_pct < 10.0 {
                        findings.push(f(4, "System drive almost full",
                            format!("C: has {free_pct:.1}% free. Below ~10% Windows update and paging performance degrade."),
                            "Run Cleanup; consider disabling hibernation to reclaim hiberfil.sys.",
                            vec!["hibernate_off"]));
                    }
                }
            }
        }
    }

    // HDD as a system disk
    if let Some(disks) = scan["disks"].as_array() {
        if disks.iter().any(|d| d["MediaType"].as_str() == Some("HDD")) {
            findings.push(f(3, "Mechanical HDD detected",
                "An HDD is present. If Windows boots from it, this is the single biggest bottleneck — no software tweak comes close to an SSD upgrade.".into(),
                "If C: is on the HDD, an SSD migration outperforms every optimization in this app combined. Do NOT disable SysMain on HDD systems.",
                vec![]));
        }
    }

    // SMART health
    if let Some(disks) = scan["disks"].as_array() {
        for d in disks {
            if let Some(h) = d["HealthStatus"].as_str() {
                if h != "Healthy" {
                    findings.push(f(5, "Disk health warning",
                        format!("'{}' reports HealthStatus={h}. Back up data NOW.", d["FriendlyName"].as_str().unwrap_or("?")),
                        "Back up immediately and check SMART details; plan replacement.",
                        vec![]));
                }
            }
        }
    }

    // Startup bloat
    if let Some(items) = scan["startup_items"].as_array() {
        if items.len() > 10 {
            findings.push(f(3, "Heavy startup load",
                format!("{} programs launch at login. Each adds boot time and idle RAM.", items.len()),
                "Disable nonessential entries (Task Manager → Startup). Apply the startup-delay tweak for faster time-to-desktop.",
                vec!["startup_delay_off"]));
        }
    }

    // Device manager problems
    if let Some(devs) = scan["drivers_problem"].as_array() {
        if !devs.is_empty() {
            findings.push(f(3, "Devices with driver problems",
                format!("{} device(s) report ConfigManager error codes (missing/broken drivers).", devs.len()),
                "Open Device Manager and update or reinstall the flagged drivers.",
                vec![]));
        }
    }

    // Defender / firewall off
    if security["defender"]["RealTimeProtectionEnabled"].as_bool() == Some(false) {
        findings.push(f(5, "Real-time antivirus protection is OFF",
            "Microsoft Defender real-time protection is disabled and no equivalent appears active.".into(),
            "Re-enable real-time protection unless a third-party AV is intentionally handling it.",
            vec![]));
    }
    if let Some(profiles) = security["firewall"].as_array() {
        for p in profiles {
            if p["Enabled"].as_bool() == Some(false) || p["Enabled"].as_i64() == Some(0) {
                findings.push(f(4, "Firewall profile disabled",
                    format!("Firewall profile '{}' is disabled.", p["Name"].as_str().unwrap_or("?")),
                    "Re-enable the firewall profile unless intentionally managed elsewhere.",
                    vec![]));
            }
        }
    }

    // Unsigned drivers
    if let Some(c) = security["unsigned_drivers"]["count"].as_u64() {
        if c > 0 {
            findings.push(f(3, "Unsigned drivers present",
                format!("{c} unsigned driver(s) loaded. Often legacy hardware tools, but a common rootkit vector."),
                "Review the list on the Security page; update or remove unknown ones.",
                vec![]));
        }
    }

    // Hosts file hijack indicators
    if let Some(c) = security["hosts"]["count"].as_u64() {
        if c > 5 {
            findings.push(f(3, "Non-default hosts file entries",
                format!("{c} active hosts entries. Legit for ad-blocking/dev work; also a classic hijack vector."),
                "Verify each entry on the Security page is one you added.",
                vec![]));
        }
    }

    // Processes running from Temp
    if let Some(arr) = security["suspicious_processes"].as_array() {
        if !arr.is_empty() {
            findings.push(f(4, "Processes executing from Temp directories",
                format!("{} process(es) run from a Temp folder — unusual for legitimate software.", arr.len()),
                "Investigate these on the Security page; scan with Defender if unrecognized.",
                vec![]));
        }
    }

    // VBS / Memory Integrity — informational FPS tradeoff (user decides; we never toggle security)
    if let Some(svcs) = scan["vbs"]["SecurityServicesRunning"].as_array() {
        if svcs.iter().any(|v| v.as_i64() == Some(2)) {
            findings.push(f(1, "Memory Integrity (HVCI) is active",
                "Virtualization-based security with HVCI is running. It's a genuine security protection but costs roughly 3–8% performance in CPU-bound games on some systems.".into(),
                "Informational only — this app will not change security protections. If you want maximum FPS and accept the tradeoff, toggle it yourself: Windows Security → Device Security → Core Isolation. Benchmark before/after.",
                vec![]));
        }
    }

    // Cleanup potential
    if let Some(cats) = cleanup.as_array() {
        let total: u64 = cats.iter().filter_map(|c| c["bytes"].as_u64()).sum();
        if total > 2_000_000_000 {
            findings.push(f(2, "Significant reclaimable disk space",
                format!("{:.1} GB of caches/temp files can be safely removed.", total as f64 / 1e9),
                "Run the Cleanup page (review categories first).",
                vec![]));
        }
    }

    findings.sort_by(|a, b| b.severity.cmp(&a.severity));
    let health: i64 = 100 - findings.iter().map(|x| match x.severity { 5 => 25i64, 4 => 15, 3 => 8, 2 => 3, _ => 1 }).sum::<i64>().min(95);

    let summary = if findings.is_empty() {
        "No significant issues detected. System configuration looks healthy.".to_string()
    } else {
        let top = &findings[0];
        format!(
            "{} issue(s) found. Highest priority: {} — {} Health score reflects weighted severity; address top items first for the biggest real-world gain.",
            findings.len(), top.title, top.recommendation
        )
    };

    json!({
        "healthScore": health,
        "summary": summary,
        "findings": findings.iter().map(|x| json!({
            "severity": x.severity, "title": x.title, "detail": x.detail,
            "recommendation": x.recommendation, "tweakIds": x.tweak_ids,
        })).collect::<Vec<_>>(),
    })
}
