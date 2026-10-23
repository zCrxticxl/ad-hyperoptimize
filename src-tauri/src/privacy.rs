//! Privacy Center, catalog of Windows privacy tweaks with apply/revert/check.
//! All via registry + service control. Check status in a single PS call.

use crate::ps;
use crate::safety;
use serde_json::{json, Value};

struct PrivacyTweak {
    id: &'static str,
    name: &'static str,
    category: &'static str,
    description: &'static str,
    risk: &'static str,
    apply: &'static str, // PS script
    /// PS expression returning $true when the privacy tweak IS applied.
    check: &'static str,
}

// Helper macro so long registry paths stay readable.
#[allow(unused_macros)]
macro_rules! regget {
    ($path:expr, $name:expr) => {
        concat!(
            "(Get-ItemProperty -Path '",
            $path,
            "' -Name '",
            $name,
            "' -ErrorAction SilentlyContinue).'",
            $name,
            "'"
        )
    };
}

static TWEAKS: &[PrivacyTweak] = &[
    // ── Telemetry ────────────────────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_telemetry_basic",
        name:        "Set Telemetry to Minimum",
        category:    "Telemetry",
        description: "Sets AllowTelemetry to 1 (Basic), lowest value for Home/Pro. Prevents sending Enhanced/Full diagnostic data.",
        risk:        "Low",
        apply:       r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name AllowTelemetry -Value 1 -Type DWord; Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\DataCollection' -Name AllowTelemetry -Value 1 -Type DWord -ErrorAction SilentlyContinue"#,
        check:       r#"$v = (Get-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' -ErrorAction SilentlyContinue).AllowTelemetry; $null -ne $v -and $v -le 1"#,
    },
    PrivacyTweak {
        id:          "prv_diagtrack_off",
        name:        "Disable DiagTrack Service",
        category:    "Telemetry",
        description: "Stops and disables the Connected User Experiences and Telemetry service. Prevents periodic upload of diagnostic data.",
        risk:        "Low",
        apply:       "Stop-Service -Name DiagTrack -Force -ErrorAction SilentlyContinue; Set-Service -Name DiagTrack -StartupType Disabled -ErrorAction SilentlyContinue",
        check:       "(Get-Service -Name DiagTrack -ErrorAction SilentlyContinue).StartType -eq 'Disabled'",
    },
    PrivacyTweak {
        id:          "prv_ceip_off",
        name:        "Disable CEIP (Customer Experience Program)",
        category:    "Telemetry",
        description: "Disables the Customer Experience Improvement Program (SQMClient).",
        risk:        "Low",
        apply:       r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\SQMClient\Windows'; if(!(Test-Path $p)){New-Item -Path $p -Force -ErrorAction SilentlyContinue|Out-Null}; Set-ItemProperty -Path $p -Name CEIPEnable -Value 0 -Type DWord -ErrorAction SilentlyContinue; 'OK'"#,
        check:       "(Get-ItemProperty 'HKLM:\\SOFTWARE\\Policies\\Microsoft\\SQMClient\\Windows' -ErrorAction SilentlyContinue).CEIPEnable -eq 0",
    },
    PrivacyTweak {
        id:          "prv_wer_off",
        name:        "Disable Windows Error Reporting",
        category:    "Telemetry",
        description: "Prevents crash dumps and error reports from being sent to Microsoft.",
        risk:        "Low",
        apply:       r#"$p='HKLM:\SOFTWARE\Microsoft\Windows\Windows Error Reporting'; Set-ItemProperty -Path $p -Name Disabled -Value 1 -Type DWord; Stop-Service -Name WerSvc -Force -ErrorAction SilentlyContinue; Set-Service -Name WerSvc -StartupType Disabled -ErrorAction SilentlyContinue"#,
        check:       "(Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\Windows Error Reporting' -ErrorAction SilentlyContinue).Disabled -eq 1",
    },
    PrivacyTweak {
        id:          "prv_dmwap_off",
        name:        "Disable WAP Push Service",
        category:    "Telemetry",
        description: "Disables dmwappushservice, used for telemetry routing, not needed for normal user applications.",
        risk:        "Low",
        apply:       "Stop-Service -Name dmwappushservice -Force -ErrorAction SilentlyContinue; Set-Service -Name dmwappushservice -StartupType Disabled -ErrorAction SilentlyContinue",
        check:       "(Get-Service -Name dmwappushservice -ErrorAction SilentlyContinue).StartType -eq 'Disabled'",
    },

    // ── Werbung & Personalisierung ────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_adid_off",
        name:        "Disable Advertising ID",
        category:    "Advertising",
        description: "Disables the Windows Advertising ID, prevents cross-app tracking for personalized ads.",
        risk:        "Low",
        apply:       r#"$p='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\AdvertisingInfo'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name Enabled -Value 0 -Type DWord"#,
        check:       "(Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\AdvertisingInfo' -ErrorAction SilentlyContinue).Enabled -eq 0",
    },
    PrivacyTweak {
        id:          "prv_tailored_off",
        name:        "Disable Tailored Experiences",
        category:    "Advertising",
        description: "Prevents use of diagnostic data for personalized tips, offers, and recommendations.",
        risk:        "Low",
        apply:       r#"$p='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Privacy'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name TailoredExperiencesWithDiagnosticDataEnabled -Value 0 -Type DWord"#,
        check:       "(Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Privacy' -ErrorAction SilentlyContinue).TailoredExperiencesWithDiagnosticDataEnabled -eq 0",
    },
    PrivacyTweak {
        id:          "prv_startmenu_suggestions_off",
        name:        "Disable Start Menu Suggestions",
        category:    "Advertising",
        description: "Removes app suggestions, sponsored content, and tips from the Start menu.",
        risk:        "Low",
        apply:       r#"$p='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'; Set-ItemProperty -Path $p -Name SystemPaneSuggestionsEnabled -Value 0 -Type DWord -ErrorAction SilentlyContinue; Set-ItemProperty -Path $p -Name SoftLandingEnabled -Value 0 -Type DWord -ErrorAction SilentlyContinue; Set-ItemProperty -Path $p -Name SubscribedContent-338388Enabled -Value 0 -Type DWord -ErrorAction SilentlyContinue; Set-ItemProperty -Path $p -Name SubscribedContent-338389Enabled -Value 0 -Type DWord -ErrorAction SilentlyContinue; Set-ItemProperty -Path $p -Name SubscribedContent-338393Enabled -Value 0 -Type DWord -ErrorAction SilentlyContinue"#,
        check:       "(Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager' -ErrorAction SilentlyContinue).SystemPaneSuggestionsEnabled -eq 0",
    },
    PrivacyTweak {
        id:          "prv_app_launch_tracking_off",
        name:        "Disable App Launch Tracking",
        category:    "Advertising",
        description: "Prevents Windows from tracking launched apps (used for Start menu sorting and advertising).",
        risk:        "Low",
        apply:       r#"$p='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Advanced'; Set-ItemProperty -Path $p -Name Start_TrackProgs -Value 0 -Type DWord -ErrorAction SilentlyContinue"#,
        check:       "(Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced' -ErrorAction SilentlyContinue).Start_TrackProgs -eq 0",
    },

    // ── Suche & Cortana ───────────────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_bing_search_off",
        name:        "Disable Bing Search in Start Menu",
        category:    "Search",
        description: "Prevents local Start menu searches from being forwarded to Bing / the internet.",
        risk:        "Low",
        apply:       r#"$p='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Search'; Set-ItemProperty -Path $p -Name BingSearchEnabled -Value 0 -Type DWord -ErrorAction SilentlyContinue; Set-ItemProperty -Path $p -Name CortanaConsent -Value 0 -Type DWord -ErrorAction SilentlyContinue"#,
        check:       "(Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Search' -ErrorAction SilentlyContinue).BingSearchEnabled -eq 0",
    },
    PrivacyTweak {
        id:          "prv_search_highlights_off",
        name:        "Disable Search Highlights",
        category:    "Search",
        description: "Removes trending and highlighted content from Windows Search.",
        risk:        "Low",
        apply:       r#"$p='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\SearchSettings'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name IsDynamicSearchBoxEnabled -Value 0 -Type DWord -ErrorAction SilentlyContinue"#,
        check:       "(Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\SearchSettings' -ErrorAction SilentlyContinue).IsDynamicSearchBoxEnabled -eq 0",
    },

    // ── Aktivitätsverlauf ─────────────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_activity_history_off",
        name:        "Disable Activity History",
        category:    "Activity History",
        description: "Disables Activity Feed, publishing, and upload. Prevents saving activities for the Timeline.",
        risk:        "Low",
        apply:       r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\System'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name EnableActivityFeed -Value 0 -Type DWord; Set-ItemProperty -Path $p -Name PublishUserActivities -Value 0 -Type DWord; Set-ItemProperty -Path $p -Name UploadUserActivities -Value 0 -Type DWord"#,
        check:       "(Get-ItemProperty 'HKLM:\\SOFTWARE\\Policies\\Microsoft\\Windows\\System' -ErrorAction SilentlyContinue).EnableActivityFeed -eq 0",
    },

    // ── Eingabe & Handschrift ─────────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_ink_personalization_off",
        name:        "Disable Typing / Handwriting Personalization",
        category:    "Input",
        description: "Prevents collection of typing and handwriting data used to improve autocorrect and Cortana.",
        risk:        "Low",
        apply:       r#"$p='HKCU:\SOFTWARE\Microsoft\InputPersonalization'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name RestrictImplicitInkCollection -Value 1 -Type DWord; Set-ItemProperty -Path $p -Name RestrictImplicitTextCollection -Value 1 -Type DWord; $p2='HKCU:\SOFTWARE\Microsoft\InputPersonalization\TrainedDataStore'; if(!(Test-Path $p2)){New-Item -Path $p2 -Force|Out-Null}; Set-ItemProperty -Path $p2 -Name HarvestContacts -Value 0 -Type DWord"#,
        check:       "(Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\InputPersonalization' -ErrorAction SilentlyContinue).RestrictImplicitInkCollection -eq 1",
    },

    // ── Standort ──────────────────────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_location_off",
        name:        "Disable Location Service",
        category:    "Location",
        description: "Disables the Windows Location Service (lfsvc). Apps can no longer retrieve GPS/network location.",
        risk:        "Medium",
        apply:       r#"Stop-Service -Name lfsvc -Force -ErrorAction SilentlyContinue; Set-Service -Name lfsvc -StartupType Disabled -ErrorAction SilentlyContinue; $p='HKLM:\SYSTEM\CurrentControlSet\Services\lfsvc\Service\Configuration'; if(Test-Path $p){ Set-ItemProperty -Path $p -Name Status -Value 0 -Type DWord -ErrorAction SilentlyContinue }"#,
        check:       "(Get-Service -Name lfsvc -ErrorAction SilentlyContinue).StartType -eq 'Disabled'",
    },

    // ── Feedback ──────────────────────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_feedback_never",
        name:        "Disable Feedback Requests",
        category:    "Feedback",
        description: "Sets feedback frequency to Never, prevents Windows feedback dialogs from appearing.",
        risk:        "Low",
        apply:       r#"$p='HKCU:\SOFTWARE\Microsoft\Siuf\Rules'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name NumberOfSIUFInPeriod -Value 0 -Type DWord; Remove-ItemProperty -Path $p -Name PeriodInNanoSeconds -ErrorAction SilentlyContinue"#,
        check:       "(Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\Siuf\\Rules' -ErrorAction SilentlyContinue).NumberOfSIUFInPeriod -eq 0",
    },

    // ── Delivery Optimization ─────────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_delivery_opt_off",
        name:        "Disable Delivery Optimization P2P",
        category:    "Network",
        description: "Prevents Windows from uploading update files to other PCs on the internet. Downloads from Microsoft still work.",
        risk:        "Low",
        apply:       r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name DODownloadMode -Value 0 -Type DWord"#,
        check:       "(Get-ItemProperty 'HKLM:\\SOFTWARE\\Policies\\Microsoft\\Windows\\DeliveryOptimization' -ErrorAction SilentlyContinue).DODownloadMode -eq 0",
    },

    // ── Maps ──────────────────────────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_maps_update_off",
        name:        "Disable Automatic Map Updates",
        category:    "Network",
        description: "Prevents Windows from downloading offline map data in the background.",
        risk:        "Low",
        apply:       r#"$p='HKLM:\SYSTEM\Maps'; if(Test-Path $p){ Set-ItemProperty -Path $p -Name AutoUpdateEnabled -Value 0 -Type DWord -ErrorAction SilentlyContinue }"#,
        check:       "(Get-ItemProperty 'HKLM:\\SYSTEM\\Maps' -ErrorAction SilentlyContinue).AutoUpdateEnabled -eq 0",
    },
];

// ── status check ─────────────────────────────────────────────────────────────

/// Checks may be statement lists (`$v = ...; <expr>`), so they are wrapped in
/// a subexpression `$( )`: plain `( )` only accepts a single expression and
/// made the whole scan script fail to parse.
fn status_script() -> String {
    let assignments: Vec<String> = TWEAKS
        .iter()
        .map(|t| {
            let var = t.id.replace('-', "_");
            format!(
                "${var} = try {{ [bool]$({}) }} catch {{ $false }};",
                t.check
            )
        })
        .collect();
    let entries: Vec<String> = TWEAKS
        .iter()
        .map(|t| {
            let var = t.id.replace('-', "_");
            format!("'{}' = ${};", t.id, var)
        })
        .collect();
    format!(
        "{} $r = @{{{}}}; $r | ConvertTo-Json -Compress",
        assignments.join(" "),
        entries.join(" ")
    )
}

fn check_one_script(t: &PrivacyTweak) -> String {
    format!("$ErrorActionPreference = 'Stop'; [bool]$({})", t.check)
}

fn check_statuses() -> Result<std::collections::HashMap<String, bool>, String> {
    let raw = ps::run(&status_script())?;
    let v: Value = serde_json::from_str(raw.trim())
        .map_err(|error| format!("Privacy scan could not be parsed: {error}"))?;
    Ok(TWEAKS
        .iter()
        .map(|t| (t.id.to_string(), v[t.id].as_bool().unwrap_or(false)))
        .collect())
}

// ── public API ────────────────────────────────────────────────────────────────

fn registry_targets(id: &str) -> &'static [(&'static str, &'static str, &'static str)] {
    match id {
        "prv_telemetry_basic" => &[
            (
                r"HKLM",
                r"SOFTWARE\Policies\Microsoft\Windows\DataCollection",
                r"AllowTelemetry",
            ),
            (
                r"HKLM",
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\DataCollection",
                r"AllowTelemetry",
            ),
        ],
        "prv_ceip_off" => &[(
            r"HKLM",
            r"SOFTWARE\Policies\Microsoft\SQMClient\Windows",
            r"CEIPEnable",
        )],
        "prv_wer_off" => &[(
            r"HKLM",
            r"SOFTWARE\Microsoft\Windows\Windows Error Reporting",
            r"Disabled",
        )],
        "prv_adid_off" => &[(
            r"HKCU",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\AdvertisingInfo",
            r"Enabled",
        )],
        "prv_tailored_off" => &[(
            r"HKCU",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Privacy",
            r"TailoredExperiencesWithDiagnosticDataEnabled",
        )],
        "prv_startmenu_suggestions_off" => &[
            (
                r"HKCU",
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\ContentDeliveryManager",
                r"SystemPaneSuggestionsEnabled",
            ),
            (
                r"HKCU",
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\ContentDeliveryManager",
                r"SoftLandingEnabled",
            ),
            (
                r"HKCU",
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\ContentDeliveryManager",
                r"SubscribedContent-338388Enabled",
            ),
            (
                r"HKCU",
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\ContentDeliveryManager",
                r"SubscribedContent-338389Enabled",
            ),
            (
                r"HKCU",
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\ContentDeliveryManager",
                r"SubscribedContent-338393Enabled",
            ),
        ],
        "prv_app_launch_tracking_off" => &[(
            r"HKCU",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
            r"Start_TrackProgs",
        )],
        "prv_bing_search_off" => &[
            (
                r"HKCU",
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Search",
                r"BingSearchEnabled",
            ),
            (
                r"HKCU",
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Search",
                r"CortanaConsent",
            ),
        ],
        "prv_search_highlights_off" => &[(
            r"HKCU",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\SearchSettings",
            r"IsDynamicSearchBoxEnabled",
        )],
        "prv_activity_history_off" => &[
            (
                r"HKLM",
                r"SOFTWARE\Policies\Microsoft\Windows\System",
                r"EnableActivityFeed",
            ),
            (
                r"HKLM",
                r"SOFTWARE\Policies\Microsoft\Windows\System",
                r"PublishUserActivities",
            ),
            (
                r"HKLM",
                r"SOFTWARE\Policies\Microsoft\Windows\System",
                r"UploadUserActivities",
            ),
        ],
        "prv_ink_personalization_off" => &[
            (
                r"HKCU",
                r"SOFTWARE\Microsoft\InputPersonalization",
                r"RestrictImplicitInkCollection",
            ),
            (
                r"HKCU",
                r"SOFTWARE\Microsoft\InputPersonalization",
                r"RestrictImplicitTextCollection",
            ),
            (
                r"HKCU",
                r"SOFTWARE\Microsoft\InputPersonalization\TrainedDataStore",
                r"HarvestContacts",
            ),
        ],
        "prv_location_off" => &[(
            r"HKLM",
            r"SYSTEM\CurrentControlSet\Services\lfsvc\Service\Configuration",
            r"Status",
        )],
        "prv_feedback_never" => &[
            (
                r"HKCU",
                r"SOFTWARE\Microsoft\Siuf\Rules",
                r"NumberOfSIUFInPeriod",
            ),
            (
                r"HKCU",
                r"SOFTWARE\Microsoft\Siuf\Rules",
                r"PeriodInNanoSeconds",
            ),
        ],
        "prv_delivery_opt_off" => &[(
            r"HKLM",
            r"SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization",
            r"DODownloadMode",
        )],
        "prv_maps_update_off" => &[(r"HKLM", r"SYSTEM\Maps", r"AutoUpdateEnabled")],
        "prv_diagtrack_off" | "prv_dmwap_off" => &[],
        _ => &[],
    }
}

fn service_target(id: &str) -> Option<&'static str> {
    match id {
        "prv_diagtrack_off" => Some("DiagTrack"),
        "prv_wer_off" => Some("WerSvc"),
        "prv_dmwap_off" => Some("dmwappushservice"),
        "prv_location_off" => Some("lfsvc"),
        _ => None,
    }
}

fn capture_changes(id: &str) -> Result<Vec<safety::ChangeItem>, String> {
    let mut items = Vec::new();
    if let Some(service) = service_target(id) {
        let state = ps::run(&format!(
            "$s = Get-Service -Name '{service}' -ErrorAction Stop; $s.StartType.ToString(); $s.Status.ToString()"
        ))?;
        let mut lines = state.lines();
        let startup = lines.next().ok_or("Service startup type missing")?.trim();
        let running = lines
            .next()
            .is_some_and(|status| status.trim() == "Running");
        if running {
            items.push(safety::ChangeItem::Command {
                applied: String::new(),
                revert: format!(
                    "powershell -Command Start-Service -Name '{service}' -ErrorAction Stop"
                ),
            });
        }
        items.push(safety::ChangeItem::ServiceStartup {
            service: service.into(),
            prev: startup.into(),
            new: "Disabled".into(),
        });
    }
    for (root, path, name) in registry_targets(id) {
        items.push(safety::ChangeItem::Registry {
            root: (*root).into(),
            path: (*path).into(),
            name: (*name).into(),
            prev: crate::tweaks::reg_read(root, path, name)?,
            // Record the value apply actually writes (some items set 1, not
            // 0). A value apply deletes (expected None) has no RegVal form;
            // it keeps the Dword(0) placeholder, undo only uses `prev`.
            new: safety::RegVal::Dword(expected_dword(id, name).unwrap_or(0)),
        });
    }
    Ok(items)
}

fn expected_dword(id: &str, name: &str) -> Option<u32> {
    if id == "prv_feedback_never" && name == "PeriodInNanoSeconds" {
        return None;
    }
    if id == "prv_telemetry_basic"
        || id == "prv_wer_off"
        || (id == "prv_ink_personalization_off" && name != "HarvestContacts")
    {
        Some(1)
    } else {
        Some(0)
    }
}

fn verify_applied(t: &PrivacyTweak) -> Result<(), String> {
    if !check_one(t)? {
        return Err(format!(
            "Privacy setting {} did not reach the requested state",
            t.id
        ));
    }
    for (root, path, name) in registry_targets(t.id) {
        let current = crate::tweaks::reg_read(root, path, name)?;
        let expected = expected_dword(t.id, name);
        let matches = match (current, expected) {
            (Some(safety::RegVal::Dword(value)), Some(want)) => value == want,
            (None, None) => true,
            _ => false,
        };
        if !matches {
            return Err(format!(
                "Privacy registry value not updated: {root}/{path}/{name}"
            ));
        }
    }
    if let Some(service) = service_target(t.id) {
        let result = ps::run(&format!(
            "$s = Get-Service -Name '{service}' -ErrorAction Stop; $s.StartType.ToString(); $s.Status.ToString()"
        ))?;
        let mut lines = result.lines();
        if lines.next().map_or(true, |mode| mode.trim() != "Disabled")
            || lines
                .next()
                .map_or(true, |status| status.trim() != "Stopped")
        {
            return Err(format!("Privacy service {service} is still active"));
        }
    }
    Ok(())
}

fn check_one(t: &PrivacyTweak) -> Result<bool, String> {
    let output = ps::run(&check_one_script(t))?;
    match output.trim().to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("Unexpected privacy status for {}: {other}", t.id)),
    }
}

pub fn scan() -> Value {
    let statuses = match check_statuses() {
        Ok(statuses) => statuses,
        Err(error) => return json!({ "error": error, "tweaks": [], "applied": 0, "total": 0 }),
    };
    let journal = match safety::load_journal() {
        Ok(entries) => entries,
        Err(error) => return json!({ "error": error, "tweaks": [], "applied": 0, "total": 0 }),
    };
    let tweaks: Vec<Value> = TWEAKS
        .iter()
        .map(|t| {
            json!({
                "id":          t.id,
                "name":        t.name,
                "category":    t.category,
                "description": t.description,
                "risk":        t.risk,
                "applied":     statuses.get(t.id).copied().unwrap_or(false),
                "canRevert":   journal.iter().any(|entry| entry.tweak_id == format!("privacy:{}", t.id) && !entry.reverted),
            })
        })
        .collect();
    let applied = tweaks
        .iter()
        .filter(|t| t["applied"].as_bool().unwrap_or(false))
        .count();
    json!({ "tweaks": tweaks, "applied": applied, "total": tweaks.len() })
}

pub fn apply(id: String) -> Result<Value, String> {
    let t = TWEAKS
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("Unknown tweak: {id}"))?;
    // Write-ahead journal so privacy tweaks participate in the app-wide Undo
    // (the audit flagged these as mutating without any journal entry).
    let changes = capture_changes(&id)?;
    let entry_id = safety::append_entry(safety::JournalEntry {
        id: format!(
            "privacy-{id}-{}",
            chrono::Local::now().format("%Y%m%d%H%M%S%6f")
        ),
        tweak_id: format!("privacy:{id}"),
        tweak_name: t.name.to_string(),
        origin: None,
        time: chrono::Local::now().to_rfc3339(),
        items: changes,
        reverted: false,
        backup_files: vec![],
        completed: false,
        attempted: 0,
        reverted_items: 0,
        untrusted: false,
    })?;
    safety::with_journal(|entries| {
        if let Some(entry) = entries.iter_mut().find(|entry| entry.id == entry_id) {
            entry.attempted = entry.items.len();
        }
        Ok(())
    })?;
    let res = ps::run(t.apply).and_then(|_| verify_applied(t));
    let _ = safety::with_journal(|j| {
        if let Some(en) = j.iter_mut().find(|en| en.id == entry_id) {
            en.completed = res.is_ok();
        }
        Ok(())
    });
    res?;
    Ok(json!({ "ok": true, "id": id, "entryId": entry_id }))
}

pub fn revert(id: String) -> Result<Value, String> {
    if !TWEAKS.iter().any(|t| t.id == id) {
        return Err(format!("Unknown tweak: {id}"));
    }
    let entry = safety::load_journal()?
        .into_iter()
        .rev()
        .find(|entry| entry.tweak_id == format!("privacy:{id}") && !entry.reverted)
        .ok_or_else(|| format!("No saved previous state for {id}; refusing a blind reset"))?;
    crate::tweaks::revert_entry(&entry.id)?;
    Ok(json!({ "ok": true, "id": id, "entryId": entry.id }))
}

#[cfg(test)]
mod tests {
    use super::{expected_dword, registry_targets, service_target, TWEAKS};

    /// Regression: a statement-list check inside `[bool]( )` made the whole
    /// scan script unparsable, so the Privacy Center showed a parser error.
    /// Only parses the generated scripts (nothing is executed).
    #[cfg(windows)]
    #[test]
    fn generated_status_scripts_parse() {
        let mut scripts = vec![super::status_script()];
        scripts.extend(TWEAKS.iter().map(super::check_one_script));
        for script in scripts {
            let out = std::process::Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "$t = $null; $e = $null; \
                     [void][System.Management.Automation.Language.Parser]::ParseInput($env:ADHO_SCRIPT, [ref]$t, [ref]$e); \
                     $e.Count",
                ])
                .env("ADHO_SCRIPT", &script)
                .output()
                .unwrap();
            let errors = String::from_utf8_lossy(&out.stdout).trim().to_string();
            assert_eq!(errors, "0", "script does not parse: {script}");
        }
    }

    #[test]
    fn telemetry_check_requires_the_value_to_exist() {
        let t = TWEAKS
            .iter()
            .find(|t| t.id == "prv_telemetry_basic")
            .unwrap();
        // `$null -le 1` is $true in PowerShell; a missing value must not
        // count as applied.
        assert!(t.check.contains("$null -ne $v"));
    }

    #[test]
    fn journal_records_the_value_apply_writes() {
        for (_, _, name) in registry_targets("prv_telemetry_basic") {
            assert_eq!(expected_dword("prv_telemetry_basic", name), Some(1));
        }
    }

    #[test]
    fn every_privacy_tweak_has_an_exact_undo_target() {
        for tweak in TWEAKS {
            assert!(
                !registry_targets(tweak.id).is_empty() || service_target(tweak.id).is_some(),
                "missing snapshot targets for {}",
                tweak.id
            );
        }
    }
}
