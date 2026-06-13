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
    .map(|_| "Defender Quick-Scan gestartet — Ergebnis erscheint im Windows-Sicherheit-Center.".into())
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
                    ho_disabled.push(t[HO_PREFIX.len()..].to_string());
                } else if !t.is_empty() && !t.starts_with('#') {
                    active.push(t.to_string());
                }
            }
            let count = active.len();
            let ho_count = ho_disabled.len();
            let active_preview: Vec<&str> = active.iter().take(20).map(|s| s.as_str()).collect();
            let ho_preview: Vec<&str> = ho_disabled.iter().take(20).map(|s| s.as_str()).collect();
            json!({
                "count": count,
                "hoDisabledCount": ho_count,
                "activeEntries": active_preview,
                "hoDisabledEntries": ho_preview,
            })
        }
        Err(e) => json!({ "error": e.to_string() }),
    }
}

pub fn hosts_list_all() -> Value {
    match fs::read_to_string(hosts_path()) {
        Ok(s) => {
            let mut active: Vec<String> = Vec::new();
            let mut ho_disabled: Vec<String> = Vec::new();
            for line in s.lines() {
                let t = line.trim();
                if t.starts_with(HO_PREFIX) {
                    ho_disabled.push(t[HO_PREFIX.len()..].to_string());
                } else if !t.is_empty() && !t.starts_with('#') {
                    active.push(t.to_string());
                }
            }
            json!({ "active": active, "hoDisabled": ho_disabled })
        }
        Err(e) => json!({ "error": e.to_string() }),
    }
}

pub fn hosts_disable_entries(entries: Vec<String>) -> Result<String, String> {
    let path = hosts_path();
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let set: std::collections::HashSet<&str> = entries.iter().map(|s| s.as_str()).collect();
    let new_content = content
        .lines()
        .map(|line| {
            let t = line.trim();
            if !t.starts_with('#') && !t.is_empty() && set.contains(t) {
                format!("{HO_PREFIX}{t}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n");
    fs::write(&path, new_content).map_err(|e| e.to_string())?;
    Ok(format!("{} entries disabled", entries.len()))
}

pub fn hosts_enable_entries(entries: Vec<String>) -> Result<String, String> {
    let path = hosts_path();
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let set: std::collections::HashSet<&str> = entries.iter().map(|s| s.as_str()).collect();
    let new_content = content
        .lines()
        .map(|line| {
            let t = line.trim();
            if t.starts_with(HO_PREFIX) {
                let inner = &t[HO_PREFIX.len()..];
                if set.contains(inner) {
                    return inner.to_string();
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\r\n");
    fs::write(&path, new_content).map_err(|e| e.to_string())?;
    Ok(format!("{} entries re-enabled", entries.len()))
}
