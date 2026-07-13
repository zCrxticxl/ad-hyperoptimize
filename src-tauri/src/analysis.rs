//! Rule-based analysis engine: turns raw scan data into prioritized,
//! human-readable findings with severity scores and linked tweaks.
//! Deliberately deterministic & explainable — every finding states its
//! evidence. (An LLM hook could be layered on top; nothing here pretends.)
//!
//! Each finding carries a stable `code` + raw `params` alongside the
//! English `title`/`detail`/`recommendation`/`summary` strings. The English
//! strings remain the source of truth for the JSON export and HTML report
//! (report.rs, Rust-side, no i18n). The live Dashboard UI instead localizes
//! via `code`+`params` through src/localize.ts (see `localizeFinding`),
//! so findings/summary now respect the selected UI language there.

use chrono::Datelike;
use serde_json::{json, Value};

/// WMI returns a single object (not array) when there's only 1 result.
/// This normalizes to a slice of refs so callers don't need to handle both.
fn wmi_arr(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(a) => a.iter().collect(),
        Value::Object(_) => vec![v],
        _ => vec![],
    }
}

struct Finding {
    severity: u8, // 1 info, 2 low, 3 medium, 4 high, 5 critical
    code: &'static str,
    title: String,
    detail: String,
    recommendation: String,
    tweak_ids: Vec<&'static str>,
    params: Value,
}

fn f(severity: u8, code: &'static str, title: &str, detail: String, rec: &str, tweaks: Vec<&'static str>, params: Value) -> Finding {
    Finding { severity, code, title: title.into(), detail, recommendation: rec.into(), tweak_ids: tweaks, params }
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
            let total_gb = total as f64 / 1_048_576.0;
            findings.push(f(4, "high_mem_pressure", "High memory pressure",
                format!("{used_pct:.0}% of {total_gb:.1} GB RAM in use at scan time. Sustained >85% causes paging and stutter."),
                "Review top memory processes on the Monitor page; consider disabling UWP background apps.",
                vec!["background_apps_off"],
                json!({ "pct": (used_pct).round(), "totalGb": (total_gb * 10.0).round() / 10.0 })));
        }
    }

    // System drive nearly full
    if let Some(vols) = scan["volumes"].as_array() {
        for v in vols {
            if v["DriveLetter"].as_str() == Some("C") {
                if let (Some(size), Some(rem)) = (v["Size"].as_f64(), v["SizeRemaining"].as_f64()) {
                    let free_pct = rem / size * 100.0;
                    if free_pct < 10.0 {
                        findings.push(f(4, "sys_drive_full", "System drive almost full",
                            format!("C: has {free_pct:.1}% free. Below ~10% Windows update and paging performance degrade."),
                            "Run Cleanup; consider disabling hibernation to reclaim hiberfil.sys.",
                            vec!["hibernate_off"],
                            json!({ "freePct": (free_pct * 10.0).round() / 10.0 })));
                    }
                }
            }
        }
    }

    // HDD as a system disk
    if let Some(disks) = scan["disks"].as_array() {
        if disks.iter().any(|d| d["MediaType"].as_str() == Some("HDD")) {
            findings.push(f(3, "mechanical_hdd", "Mechanical HDD detected",
                "An HDD is present. If Windows boots from it, this is the single biggest bottleneck — no software tweak comes close to an SSD upgrade.".into(),
                "If C: is on the HDD, an SSD migration outperforms every optimization in this app combined. Do NOT disable SysMain on HDD systems.",
                vec![],
                json!({})));
        }
    }

    // SMART health
    if let Some(disks) = scan["disks"].as_array() {
        for d in disks {
            if let Some(h) = d["HealthStatus"].as_str() {
                if h != "Healthy" {
                    let name = d["FriendlyName"].as_str().unwrap_or("?");
                    findings.push(f(5, "disk_health_warning", "Disk health warning",
                        format!("'{name}' reports HealthStatus={h}. Back up data NOW."),
                        "Back up immediately and check SMART details; plan replacement.",
                        vec![],
                        json!({ "name": name, "health": h })));
                }
            }
        }
    }

    // Startup bloat
    if let Some(items) = scan["startup_items"].as_array() {
        if items.len() > 10 {
            findings.push(f(3, "heavy_startup", "Heavy startup load",
                format!("{} programs launch at login. Each adds boot time and idle RAM.", items.len()),
                "Disable nonessential entries (Task Manager → Startup). Apply the startup-delay tweak for faster time-to-desktop.",
                vec!["startup_delay_off"],
                json!({ "count": items.len() })));
        }
    }

    // Device manager problems
    if let Some(devs) = scan["drivers_problem"].as_array() {
        if !devs.is_empty() {
            findings.push(f(3, "driver_problems", "Devices with driver problems",
                format!("{} device(s) report ConfigManager error codes (missing/broken drivers).", devs.len()),
                "Open Device Manager and update or reinstall the flagged drivers.",
                vec![],
                json!({ "count": devs.len() })));
        }
    }

    // Defender / firewall off
    if security["defender"]["RealTimeProtectionEnabled"].as_bool() == Some(false) {
        findings.push(f(5, "defender_off", "Real-time antivirus protection is OFF",
            "Microsoft Defender real-time protection is disabled and no equivalent appears active.".into(),
            "Re-enable real-time protection unless a third-party AV is intentionally handling it.",
            vec![],
            json!({})));
    }
    if let Some(profiles) = security["firewall"].as_array() {
        for p in profiles {
            if p["Enabled"].as_bool() == Some(false) || p["Enabled"].as_i64() == Some(0) {
                let name = p["Name"].as_str().unwrap_or("?");
                findings.push(f(4, "firewall_disabled", "Firewall profile disabled",
                    format!("Firewall profile '{name}' is disabled."),
                    "Re-enable the firewall profile unless intentionally managed elsewhere.",
                    vec![],
                    json!({ "name": name })));
            }
        }
    }

    // Unsigned drivers
    if let Some(c) = security["unsigned_drivers"]["count"].as_u64() {
        if c > 0 {
            findings.push(f(3, "unsigned_drivers", "Unsigned drivers present",
                format!("{c} unsigned driver(s) loaded. Often legacy hardware tools, but a common rootkit vector."),
                "Review the list on the Security page; update or remove unknown ones.",
                vec![],
                json!({ "count": c })));
        }
    }

    // Hosts file hijack indicators
    if let Some(c) = security["hosts"]["count"].as_u64() {
        if c > 5 {
            findings.push(f(3, "hosts_entries", "Non-default hosts file entries",
                format!("{c} active hosts entries. Legit for ad-blocking/dev work; also a classic hijack vector."),
                "Verify each entry on the Security page is one you added.",
                vec![],
                json!({ "count": c })));
        }
    }

    // Processes running from Temp
    if let Some(arr) = security["suspicious_processes"].as_array() {
        if !arr.is_empty() {
            findings.push(f(4, "temp_processes", "Processes executing from Temp directories",
                format!("{} process(es) run from a Temp folder — unusual for legitimate software.", arr.len()),
                "Investigate these on the Security page; scan with Defender if unrecognized.",
                vec![],
                json!({ "count": arr.len() })));
        }
    }

    // VBS / Memory Integrity — informational FPS tradeoff
    if let Some(svcs) = scan["vbs"]["SecurityServicesRunning"].as_array() {
        if svcs.iter().any(|v| v.as_i64() == Some(2)) {
            findings.push(f(1, "hvci_active", "Memory Integrity (HVCI) is active",
                "Virtualization-based security with HVCI is running. It's a genuine security protection but costs roughly 3–8% performance in CPU-bound games on some systems.".into(),
                "Informational only — this app will not change security protections. If you want maximum FPS and accept the tradeoff, toggle it yourself: Windows Security → Device Security → Core Isolation. Benchmark before/after.",
                vec![],
                json!({})));
        }
    }

    // Cleanup potential
    if let Some(cats) = cleanup.as_array() {
        let total: u64 = cats.iter().filter_map(|c| c["bytes"].as_u64()).sum();
        if total > 2_000_000_000 {
            let gb = total as f64 / 1e9;
            findings.push(f(2, "reclaimable_space", "Significant reclaimable disk space",
                format!("{gb:.1} GB of caches/temp files can be safely removed."),
                "Run the Cleanup page (review categories first).",
                vec![],
                json!({ "gb": (gb * 10.0).round() / 10.0 })));
        }
    }

    // Long uptime
    if let Some(last_boot) = scan["os"]["LastBootUpTime"].as_str() {
        if let Ok(boot_time) = chrono::DateTime::parse_from_rfc3339(last_boot)
            .or_else(|_| chrono::DateTime::parse_from_str(last_boot, "%Y%m%d%H%M%S%.f%z"))
        {
            let hours = (chrono::Local::now().timestamp() - boot_time.timestamp()) / 3600;
            if hours > 168 {
                findings.push(f(2, "long_uptime", "System not rebooted in over 7 days",
                    format!("Last reboot was {hours} hours ago. Long uptimes accumulate memory leaks, pending updates, and degraded performance."),
                    "Reboot regularly — weekly is ideal for a gaming/workstation system.",
                    vec![],
                    json!({ "hours": hours })));
            }
        }
    }

    // Windows too old / no recent hotfixes
    if let Some(hotfixes) = scan["hotfixes_recent"].as_array() {
        if hotfixes.is_empty() {
            findings.push(f(3, "no_updates", "No Windows updates detected",
                "No recent hotfixes found. The system may be missing security patches or feature updates.".into(),
                "Run Windows Update and check for pending updates.",
                vec![],
                json!({})));
        }
    }

    // Non-power-plan (not High Performance or Ultimate)
    if let Some(plan) = scan["power_plan"].as_str() {
        let plan_lower = plan.to_lowercase();
        if !plan_lower.contains("high performance") && !plan_lower.contains("ultimate") && !plan_lower.contains("h\u{00f6}chstleistung") {
            let plan_trimmed = plan.trim();
            findings.push(f(2, "suboptimal_power_plan", "Suboptimal power plan active",
                format!("Active power plan: '{plan_trimmed}'. Balanced/Power Saver plans throttle CPU frequency and increase latency."),
                "Switch to High Performance or Ultimate Performance on the Power Plan page.",
                vec![],
                json!({ "plan": plan_trimmed })));
        }
    }

    // SMART wear / read errors
    {
        for s in wmi_arr(&scan["smart"]) {
            let wear = s["Wear"].as_u64().unwrap_or(0);
            let read_err = s["ReadErrorsTotal"].as_u64().unwrap_or(0);
            if wear > 90 {
                findings.push(f(4, "ssd_wear", "SSD nearing end of life",
                    format!("Drive wear indicator is at {wear}%. NVMe/SSD drives have a limited write endurance — beyond 90% the risk of data loss increases."),
                    "Back up critical data immediately and plan drive replacement.",
                    vec![],
                    json!({ "wear": wear })));
            }
            if read_err > 100 {
                findings.push(f(3, "drive_read_errors", "Drive read errors detected",
                    format!("{read_err} total read errors on a drive. May indicate failing sectors or cable issues."),
                    "Run chkdsk /r and monitor closely; back up data.",
                    vec![],
                    json!({ "count": read_err })));
            }
        }
    }

    // Too many running services
    if let Some(svcs) = scan["services_running"].as_array() {
        if svcs.len() > 150 {
            findings.push(f(2, "many_services", "High number of running background services",
                format!("{} services currently running. Many are Windows bloat or third-party telemetry that increase RAM usage and boot time.", svcs.len()),
                "Review and disable unnecessary services on the Services page.",
                vec![],
                json!({ "count": svcs.len() })));
        }
    }

    // RAM speed low (DDR4 < 2666)
    {
        let speeds: Vec<u64> = wmi_arr(&scan["ram_modules"]).into_iter()
            .filter_map(|m| m["ConfiguredClockSpeed"].as_u64())
            .filter(|&s| s > 0)
            .collect();
        if !speeds.is_empty() {
            let avg = speeds.iter().sum::<u64>() / speeds.len() as u64;
            if avg < 2666 {
                findings.push(f(2, "low_ram_speed", "RAM running below DDR4 baseline speed",
                    format!("Average configured RAM speed: {avg} MHz. Modern DDR4 should run at >=2666 MHz; lower speeds bottleneck CPU-bound workloads."),
                    "Check XMP/EXPO is enabled in BIOS — most RAM ships with a default 2133 MHz profile but supports higher speeds.",
                    vec![],
                    json!({ "avg": avg })));
            }
        }
    }

    // GPU driver outdated (> 2 years old)
    {
        for gpu in wmi_arr(&scan["gpu"]) {
            if let Some(date_str) = gpu["DriverDate"].as_str() {
                if date_str.len() >= 8 {
                    if let Ok(year) = date_str[0..4].parse::<i32>() {
                        let current_year = chrono::Local::now().year();
                        if current_year - year >= 2 {
                            let date_short = &date_str[0..8];
                            findings.push(f(2, "gpu_driver_old", "GPU driver is over 2 years old",
                                format!("GPU driver date: {date_short}. Outdated drivers miss performance optimizations, game-specific fixes, and may cause crashes."),
                                "Update GPU drivers from the Drivers page or manufacturer website.",
                                vec![],
                                json!({ "date": date_short })));
                        }
                    }
                }
            }
        }
    }

    // Thermal zone high (WMI returns tenths of Kelvin)
    {
        for zone in wmi_arr(&scan["thermal"]) {
            if let Some(temp_raw) = zone["CurrentTemperature"].as_u64() {
                let celsius = (temp_raw / 10).saturating_sub(273);
                if celsius > 90 {
                    findings.push(f(4, "critical_temp", "Critical CPU/system temperature detected",
                        format!("Thermal zone reports {celsius}\u{00b0}C. Sustained temperatures above 90\u{00b0}C cause thermal throttling and reduce hardware lifespan."),
                        "Clean dust from CPU/case fans, reapply thermal paste, ensure airflow is unobstructed. Check HW Monitor page for details.",
                        vec![],
                        json!({ "celsius": celsius })));
                } else if celsius > 80 {
                    findings.push(f(3, "elevated_temp", "Elevated system temperature",
                        format!("Thermal zone reports {celsius}\u{00b0}C. This is above the ideal operating range and may cause instability under load."),
                        "Check cooling solution. Consider reapplying thermal paste or adding case fans.",
                        vec![],
                        json!({ "celsius": celsius })));
                }
            }
        }
    }

    // Battery health (laptops)
    if let Some(bat) = scan["battery"].as_object() {
        if let Some(charge) = bat.get("EstimatedChargeRemaining").and_then(|v| v.as_u64()) {
            if charge < 20 {
                findings.push(f(2, "battery_low", "Battery critically low",
                    format!("Battery at {charge}%. Running on low battery forces CPU/GPU power limits, causing significant performance drops."),
                    "Plug in the charger or adjust Windows power settings for battery-saving mode.",
                    vec![],
                    json!({ "charge": charge })));
            }
        }
    }

    // Low RAM
    if let Some(total) = scan["os"]["TotalVisibleMemorySize"].as_u64() {
        let ram_gb = total as f64 / 1_048_576.0;
        if ram_gb < 16.0 {
            findings.push(f(2, "low_ram_total", "Low total RAM for modern workloads",
                format!("{ram_gb:.1} GB RAM installed. 16 GB is the practical minimum for gaming + browser + background apps without constant paging."),
                "Consider a RAM upgrade. In the meantime, enable automatic pagefile management to prevent out-of-memory crashes.",
                vec![],
                json!({ "gb": (ram_gb * 10.0).round() / 10.0 })));
        }
    }

    findings.sort_by(|a, b| b.severity.cmp(&a.severity));
    let health: i64 = 100 - findings.iter().map(|x| match x.severity {
        5 => 25i64, 4 => 15, 3 => 8, 2 => 3, _ => 1
    }).sum::<i64>().min(95);

    // English fallback summary — used by report.rs (no i18n there) and as a
    // safety net if the frontend ever fails to localize. The live Dashboard
    // UI builds its own localized summary from `findings[0].code`/`params`
    // instead of using this string (see src/localize.ts::localizeFinding and
    // Dashboard.tsx).
    let summary = if findings.is_empty() {
        "No significant issues detected. System configuration looks healthy.".to_string()
    } else {
        let top = &findings[0];
        format!(
            "{} issue(s) found. Highest priority: {} -- {} Health score reflects weighted severity; address top items first for the biggest real-world gain.",
            findings.len(), top.title, top.recommendation
        )
    };

    json!({
        "healthScore": health,
        "summary": summary,
        "findings": findings.iter().map(|x| json!({
            "severity": x.severity, "code": x.code, "title": x.title, "detail": x.detail,
            "recommendation": x.recommendation, "tweakIds": x.tweak_ids, "params": x.params,
        })).collect::<Vec<_>>(),
    })
}
