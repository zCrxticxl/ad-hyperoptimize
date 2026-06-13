//! Read-only security & anomaly inspection. This module never removes
//! anything — it surfaces findings for the user to act on.

use crate::ps;
use serde_json::{json, Value};
use std::fs;

fn sec(script: &str) -> Value {
    ps::run_json(script).unwrap_or_else(|e| json!({ "error": e.trim() }))
}

pub fn scan() -> Value {
    json!({
        "defender": sec(
            "Get-MpComputerStatus -ErrorAction Stop | Select-Object AMServiceEnabled,RealTimeProtectionEnabled,AntivirusEnabled,AntivirusSignatureLastUpdated,QuickScanEndTime,IsTamperProtected"
        ),
        "firewall": sec(
            "Get-NetFirewallProfile | Select-Object Name,Enabled,DefaultInboundAction,DefaultOutboundAction"
        ),
        "unsigned_drivers": unsigned_drivers(),
        "autoruns": autoruns(),
        "hosts": hosts_entries(),
        "suspicious_processes": sec(
            // Userland processes running from Temp/AppData\\Local\\Temp — classic persistence red flag.
            "Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -match '\\\\Temp\\\\|\\\\AppData\\\\Local\\\\Temp' } | Select-Object Name,ProcessId,ExecutablePath"
        ),
        "uac": sec(
            "Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies\\System' | Select-Object EnableLUA,ConsentPromptBehaviorAdmin"
        ),
        "secure_boot": match ps::run("Confirm-SecureBootUEFI -ErrorAction Stop") {
            Ok(s) => json!(s.trim().eq_ignore_ascii_case("true")),
            Err(_) => json!("unavailable (legacy BIOS or non-admin)"),
        },
    })
}

fn unsigned_drivers() -> Value {
    match ps::exec("driverquery.exe", &["/si", "/fo", "csv", "/nh"]) {
        Ok(csv) => {
            // columns: DeviceName, InfName, IsSigned, Manufacturer
            let mut agg: std::collections::HashMap<String, (u32, String)> = Default::default();
            for l in csv.lines() {
                let cols: Vec<&str> = l.split("\",\"").collect();
                if cols.len() >= 3 && cols[2].to_uppercase().contains("FALSE") {
                    let device = cols[0].trim_matches('"').to_string();
                    let manu = cols.get(3).map(|s| s.trim_matches('"')).unwrap_or("").to_string();
                    let e = agg.entry(device).or_insert((0, manu));
                    e.0 += 1;
                }
            }
            let total: u32 = agg.values().map(|(c, _)| c).sum();
            let mut items: Vec<Value> = agg
                .into_iter()
                .map(|(device, (count, manufacturer))| {
                    json!({ "device": device, "count": count, "manufacturer": manufacturer })
                })
                .collect();
            items.sort_by(|a, b| b["count"].as_u64().cmp(&a["count"].as_u64()));
            json!({ "count": total, "items": items })
        }
        Err(e) => json!({ "error": e.trim() }),
    }
}

/// Launch a Defender quick scan detached (survives this process).
pub fn defender_quick_scan() -> Result<String, String> {
    ps::run(
        "Start-Process powershell -WindowStyle Hidden -ArgumentList '-NoProfile','-Command','Start-MpScan -ScanType QuickScan'; 'OK'",
    )
    .map(|_| "Defender Quick Scan started — result will appear in Windows Security Center.".into())
}

fn autoruns() -> Value {
    json!({
        "hklm_run": sec("Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run' -ErrorAction Stop | Select-Object * -ExcludeProperty PS*"),
        "hkcu_run": sec("Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run' -ErrorAction Stop | Select-Object * -ExcludeProperty PS*"),
        "startup_folder": sec("Get-ChildItem ([Environment]::GetFolderPath('Startup')) -ErrorAction Stop | Select-Object Name,FullName"),
        "tasks_nonms": sec("Get-ScheduledTask | Where-Object { $_.TaskPath -notlike '\\Microsoft\\*' -and $_.State -ne 'Disabled' } | Select-Object TaskName,TaskPath,State | Select-Object -First 40"),
        "winlogon_shell": sec("Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon' | Select-Object Shell,Userinit"),
    })
}

const HO_PREFIX: &str = "# [ADHYPER] ";

fn hosts_path() -> String {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    format!("{windir}\\System32\\drivers\\etc\\hosts")
}

fn hosts_entries() -> Value {
    match fs::read_to_string(hosts_path()) {
        Ok(s) => {
            let mut active: Vec<String> = Vec::new();
            let mut ho_disabled: Vec<String> = Vec::new();
            for line in s.lines() {
                let t = line.trim();
                if t.starts_with(HO_PREFIX) {
                    // disabled by us: "# [ADHYPER] 0.0.0.0 tracker.example.com"
                    ho_disabled.push(t.trim_start_matches(HO_PREFIX).to_string());
                } else if !t.is_empty() && !t.starts_with('#') {
                    active.push(t.to_string());
                }
            }
            json!({ "active": active, "disabled": ho_disabled })
        }
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// Returns all hosts entries: managed (ADHYPER-prefixed) + rest
pub fn hosts_list_all() -> serde_json::Value {
    hosts_entries()
}

/// Disable (comment out) entries by prepending HO_PREFIX
pub fn hosts_disable_entries(entries: Vec<String>) -> Result<String, String> {
    let path = hosts_path();
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut changed = 0usize;
    for entry in &entries {
        let target = entry.trim();
        for line in lines.iter_mut() {
            let t = line.trim();
            if t == target {
                *line = format!("{HO_PREFIX}{target}");
                changed += 1;
                break;
            }
        }
    }
    fs::write(&path, lines.join("\n")).map_err(|e| e.to_string())?;
    Ok(format!("Disabled {changed} entries"))
}

/// Re-enable entries previously disabled by hosts_disable_entries
pub fn hosts_enable_entries(entries: Vec<String>) -> Result<String, String> {
    let path = hosts_path();
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut changed = 0usize;
    for entry in &entries {
        let target = entry.trim();
        let prefixed = format!("{HO_PREFIX}{target}");
        for line in lines.iter_mut() {
            if line.trim() == prefixed.trim() {
                *line = target.to_string();
                changed += 1;
                break;
            }
        }
    }
    fs::write(&path, lines.join("\n")).map_err(|e| e.to_string())?;
    Ok(format!("Enabled {changed} entries"))
}
