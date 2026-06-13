//! Privacy Center — catalog of Windows privacy tweaks with apply/revert/check.
//! All via registry + service control. Check status in a single PS call.

use crate::ps;
use serde_json::{json, Value};

struct PrivacyTweak {
    id:          &'static str,
    name:        &'static str,
    category:    &'static str,
    description: &'static str,
    risk:        &'static str,
    apply:       &'static str, // PS script
    revert:      &'static str, // PS script
    /// PS expression returning $true when the privacy tweak IS applied.
    check:       &'static str,
}

// Helper macro so long registry paths stay readable.
macro_rules! regget {
    ($path:expr, $name:expr) => {
        concat!(
            "(Get-ItemProperty -Path '", $path,
            "' -Name '", $name,
            "' -ErrorAction SilentlyContinue).'", $name, "'"
        )
    };
}

static TWEAKS: &[PrivacyTweak] = &[
    // ── Telemetry ────────────────────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_telemetry_basic",
        name:        "Telemetrie auf Minimum",
        category:    "Telemetrie",
        description: "Setzt AllowTelemetry auf 1 (Basic) — niedrigster Wert für Home/Pro. Verhindert das Senden von Enhanced/Full-Diagnosedaten.",
        risk:        "Low",
        apply:       r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name AllowTelemetry -Value 1 -Type DWord; Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\DataCollection' -Name AllowTelemetry -Value 1 -Type DWord -ErrorAction SilentlyContinue"#,
        revert:      r#"Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' -Name AllowTelemetry -ErrorAction SilentlyContinue"#,
        check:       r#"(Get-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' -ErrorAction SilentlyContinue).AllowTelemetry -le 1"#,
    },
    PrivacyTweak {
        id:          "prv_diagtrack_off",
        name:        "DiagTrack-Dienst deaktivieren",
        category:    "Telemetrie",
        description: "Stoppt und deaktiviert den 'Connected User Experiences and Telemetry'-Dienst. Verhindert das regelmäßige Hochladen von Diagnosedaten.",
        risk:        "Low",
        apply:       "Stop-Service -Name DiagTrack -Force -ErrorAction SilentlyContinue; Set-Service -Name DiagTrack -StartupType Disabled -ErrorAction SilentlyContinue",
        revert:      "Set-Service -Name DiagTrack -StartupType Automatic -ErrorAction SilentlyContinue; Start-Service -Name DiagTrack -ErrorAction SilentlyContinue",
        check:       "(Get-Service -Name DiagTrack -ErrorAction SilentlyContinue).StartType -eq 'Disabled'",
    },
    PrivacyTweak {
        id:          "prv_ceip_off",
        name:        "CEIP (Customer Experience) deaktivieren",
        category:    "Telemetrie",
        description: "Deaktiviert das Kundenerfahrungsverbesserungs-Programm (SQMClient).",
        risk:        "Low",
        apply:       r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\SQMClient\Windows'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name CEIPEnable -Value 0 -Type DWord"#,
        revert:      r#"Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\SQMClient\Windows' -Name CEIPEnable -ErrorAction SilentlyContinue"#,
        check:       "(Get-ItemProperty 'HKLM:\\SOFTWARE\\Policies\\Microsoft\\SQMClient\\Windows' -ErrorAction SilentlyContinue).CEIPEnable -eq 0",
    },
    PrivacyTweak {
        id:          "prv_wer_off",
        name:        "Windows-Fehlerberichterstattung deaktivieren",
        category:    "Telemetrie",
        description: "Verhindert das Senden von Crash-Dumps und Fehlerberichten an Microsoft.",
        risk:        "Low",
        apply:       r#"$p='HKLM:\SOFTWARE\Microsoft\Windows\Windows Error Reporting'; Set-ItemProperty -Path $p -Name Disabled -Value 1 -Type DWord; Stop-Service -Name WerSvc -Force -ErrorAction SilentlyContinue; Set-Service -Name WerSvc -StartupType Disabled -ErrorAction SilentlyContinue"#,
        revert:      r#"Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\Windows Error Reporting' -Name Disabled -Value 0 -Type DWord; Set-Service -Name WerSvc -StartupType Manual -ErrorAction SilentlyContinue"#,
        check:       "(Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\Windows Error Reporting' -ErrorAction SilentlyContinue).Disabled -eq 1",
    },
    PrivacyTweak {
        id:          "prv_dmwap_off",
        name:        "WAP Push Service deaktivieren",
        category:    "Telemetrie",
        description: "Deaktiviert dmwappushservice — wird für Telemetrie-Routing genutzt, nicht für normale Benutzeranwendungen.",
        risk:        "Low",
        apply:       "Stop-Service -Name dmwappushservice -Force -ErrorAction SilentlyContinue; Set-Service -Name dmwappushservice -StartupType Disabled -ErrorAction SilentlyContinue",
        revert:      "Set-Service -Name dmwappushservice -StartupType Manual -ErrorAction SilentlyContinue",
        check:       "(Get-Service -Name dmwappushservice -ErrorAction SilentlyContinue).StartType -eq 'Disabled'",
    },

    // ── Werbung & Personalisierung ────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_adid_off",
        name:        "Werbe-ID deaktivieren",
        category:    "Werbung",
        description: "Deaktiviert die Windows Advertising ID — verhindert app-übergreifendes Tracking für personalisierte Werbung.",
        risk:        "Low",
        apply:       r#"$p='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\AdvertisingInfo'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name Enabled -Value 0 -Type DWord"#,
        revert:      r#"Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\AdvertisingInfo' -Name Enabled -Value 1 -Type DWord"#,
        check:       "(Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\AdvertisingInfo' -ErrorAction SilentlyContinue).Enabled -eq 0",
    },
    PrivacyTweak {
        id:          "prv_tailored_off",
        name:        "Maßgeschneiderte Erlebnisse deaktivieren",
        category:    "Werbung",
        description: "Verhindert die Nutzung von Diagnosedaten für personalisierte Tipps, Angebote und Empfehlungen.",
        risk:        "Low",
        apply:       r#"$p='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Privacy'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name TailoredExperiencesWithDiagnosticDataEnabled -Value 0 -Type DWord"#,
        revert:      r#"Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Privacy' -Name TailoredExperiencesWithDiagnosticDataEnabled -Value 1 -Type DWord"#,
        check:       "(Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Privacy' -ErrorAction SilentlyContinue).TailoredExperiencesWithDiagnosticDataEnabled -eq 0",
    },
    PrivacyTweak {
        id:          "prv_startmenu_suggestions_off",
        name:        "Start-Menü Vorschläge deaktivieren",
        category:    "Werbung",
        description: "Entfernt App-Vorschläge, gesponserte Inhalte und 'Tipps' aus dem Startmenü.",
        risk:        "Low",
        apply:       r#"$p='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'; Set-ItemProperty -Path $p -Name SystemPaneSuggestionsEnabled -Value 0 -Type DWord -ErrorAction SilentlyContinue; Set-ItemProperty -Path $p -Name SoftLandingEnabled -Value 0 -Type DWord -ErrorAction SilentlyContinue; Set-ItemProperty -Path $p -Name SubscribedContent-338388Enabled -Value 0 -Type DWord -ErrorAction SilentlyContinue; Set-ItemProperty -Path $p -Name SubscribedContent-338389Enabled -Value 0 -Type DWord -ErrorAction SilentlyContinue; Set-ItemProperty -Path $p -Name SubscribedContent-338393Enabled -Value 0 -Type DWord -ErrorAction SilentlyContinue"#,
        revert:      r#"$p='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'; Set-ItemProperty -Path $p -Name SystemPaneSuggestionsEnabled -Value 1 -Type DWord -ErrorAction SilentlyContinue; Set-ItemProperty -Path $p -Name SoftLandingEnabled -Value 1 -Type DWord -ErrorAction SilentlyContinue"#,
        check:       "(Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager' -ErrorAction SilentlyContinue).SystemPaneSuggestionsEnabled -eq 0",
    },
    PrivacyTweak {
        id:          "prv_app_launch_tracking_off",
        name:        "App-Start-Tracking deaktivieren",
        category:    "Werbung",
        description: "Verhindert, dass Windows gestartete Apps trackt (für Sortierung im Startmenü und Werbezwecke).",
        risk:        "Low",
        apply:       r#"$p='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Advanced'; Set-ItemProperty -Path $p -Name Start_TrackProgs -Value 0 -Type DWord -ErrorAction SilentlyContinue"#,
        revert:      r#"Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -Name Start_TrackProgs -Value 1 -Type DWord -ErrorAction SilentlyContinue"#,
        check:       "(Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced' -ErrorAction SilentlyContinue).Start_TrackProgs -eq 0",
    },

    // ── Suche & Cortana ───────────────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_bing_search_off",
        name:        "Bing-Suche in Startmenü deaktivieren",
        category:    "Suche",
        description: "Verhindert, dass lokale Suchanfragen im Startmenü an Bing / das Internet weitergeleitet werden.",
        risk:        "Low",
        apply:       r#"$p='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Search'; Set-ItemProperty -Path $p -Name BingSearchEnabled -Value 0 -Type DWord -ErrorAction SilentlyContinue; Set-ItemProperty -Path $p -Name CortanaConsent -Value 0 -Type DWord -ErrorAction SilentlyContinue"#,
        revert:      r#"$p='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Search'; Set-ItemProperty -Path $p -Name BingSearchEnabled -Value 1 -Type DWord -ErrorAction SilentlyContinue; Set-ItemProperty -Path $p -Name CortanaConsent -Value 1 -Type DWord -ErrorAction SilentlyContinue"#,
        check:       "(Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Search' -ErrorAction SilentlyContinue).BingSearchEnabled -eq 0",
    },
    PrivacyTweak {
        id:          "prv_search_highlights_off",
        name:        "Suchhervorhebungen deaktivieren",
        category:    "Suche",
        description: "Entfernt 'Interessante Momente' und trendende Inhalte aus der Windows-Suche.",
        risk:        "Low",
        apply:       r#"$p='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\SearchSettings'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name IsDynamicSearchBoxEnabled -Value 0 -Type DWord -ErrorAction SilentlyContinue"#,
        revert:      r#"Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\SearchSettings' -Name IsDynamicSearchBoxEnabled -Value 1 -Type DWord -ErrorAction SilentlyContinue"#,
        check:       "(Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\SearchSettings' -ErrorAction SilentlyContinue).IsDynamicSearchBoxEnabled -eq 0",
    },

    // ── Aktivitätsverlauf ─────────────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_activity_history_off",
        name:        "Aktivitätsverlauf deaktivieren",
        category:    "Aktivitätsverlauf",
        description: "Deaktiviert Activity Feed, Publishing und Upload. Verhindert Speichern der Aktivitäten für die Timeline.",
        risk:        "Low",
        apply:       r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\System'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name EnableActivityFeed -Value 0 -Type DWord; Set-ItemProperty -Path $p -Name PublishUserActivities -Value 0 -Type DWord; Set-ItemProperty -Path $p -Name UploadUserActivities -Value 0 -Type DWord"#,
        revert:      r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\System'; Remove-ItemProperty -Path $p -Name EnableActivityFeed -ErrorAction SilentlyContinue; Remove-ItemProperty -Path $p -Name PublishUserActivities -ErrorAction SilentlyContinue; Remove-ItemProperty -Path $p -Name UploadUserActivities -ErrorAction SilentlyContinue"#,
        check:       "(Get-ItemProperty 'HKLM:\\SOFTWARE\\Policies\\Microsoft\\Windows\\System' -ErrorAction SilentlyContinue).EnableActivityFeed -eq 0",
    },

    // ── Eingabe & Handschrift ─────────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_ink_personalization_off",
        name:        "Eingabe-/Handschrift-Personalisierung deaktivieren",
        category:    "Eingabe",
        description: "Verhindert das Sammeln von Tipp- und Handschriftdaten zur Verbesserung von Autocorrect und Cortana.",
        risk:        "Low",
        apply:       r#"$p='HKCU:\SOFTWARE\Microsoft\InputPersonalization'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name RestrictImplicitInkCollection -Value 1 -Type DWord; Set-ItemProperty -Path $p -Name RestrictImplicitTextCollection -Value 1 -Type DWord; $p2='HKCU:\SOFTWARE\Microsoft\InputPersonalization\TrainedDataStore'; if(!(Test-Path $p2)){New-Item -Path $p2 -Force|Out-Null}; Set-ItemProperty -Path $p2 -Name HarvestContacts -Value 0 -Type DWord"#,
        revert:      r#"$p='HKCU:\SOFTWARE\Microsoft\InputPersonalization'; Set-ItemProperty -Path $p -Name RestrictImplicitInkCollection -Value 0 -Type DWord; Set-ItemProperty -Path $p -Name RestrictImplicitTextCollection -Value 0 -Type DWord"#,
        check:       "(Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\InputPersonalization' -ErrorAction SilentlyContinue).RestrictImplicitInkCollection -eq 1",
    },

    // ── Standort ──────────────────────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_location_off",
        name:        "Standortdienst deaktivieren",
        category:    "Standort",
        description: "Deaktiviert den Windows-Standortdienst (lfsvc). Apps können keinen GPS/Netzwerk-Standort mehr abrufen.",
        risk:        "Medium",
        apply:       r#"Stop-Service -Name lfsvc -Force -ErrorAction SilentlyContinue; Set-Service -Name lfsvc -StartupType Disabled -ErrorAction SilentlyContinue; $p='HKLM:\SYSTEM\CurrentControlSet\Services\lfsvc\Service\Configuration'; if(Test-Path $p){ Set-ItemProperty -Path $p -Name Status -Value 0 -Type DWord -ErrorAction SilentlyContinue }"#,
        revert:      "Set-Service -Name lfsvc -StartupType Manual -ErrorAction SilentlyContinue",
        check:       "(Get-Service -Name lfsvc -ErrorAction SilentlyContinue).StartType -eq 'Disabled'",
    },

    // ── Feedback ──────────────────────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_feedback_never",
        name:        "Feedback-Anfragen deaktivieren",
        category:    "Feedback",
        description: "Setzt die Feedback-Häufigkeit auf 'Nie' — verhindert Windows-Feedback-Dialoge.",
        risk:        "Low",
        apply:       r#"$p='HKCU:\SOFTWARE\Microsoft\Siuf\Rules'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name NumberOfSIUFInPeriod -Value 0 -Type DWord; Remove-ItemProperty -Path $p -Name PeriodInNanoSeconds -ErrorAction SilentlyContinue"#,
        revert:      r#"Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Siuf\Rules' -Name NumberOfSIUFInPeriod -ErrorAction SilentlyContinue"#,
        check:       "(Get-ItemProperty 'HKCU:\\SOFTWARE\\Microsoft\\Siuf\\Rules' -ErrorAction SilentlyContinue).NumberOfSIUFInPeriod -eq 0",
    },

    // ── Delivery Optimization ─────────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_delivery_opt_off",
        name:        "Delivery Optimization P2P deaktivieren",
        category:    "Netzwerk",
        description: "Verhindert, dass Windows Update-Dateien an andere PCs im Internet hochlädt (P2P-Verteilung). Downloads von Microsoft bleiben möglich.",
        risk:        "Low",
        apply:       r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization'; if(!(Test-Path $p)){New-Item -Path $p -Force|Out-Null}; Set-ItemProperty -Path $p -Name DODownloadMode -Value 0 -Type DWord"#,
        revert:      r#"Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization' -Name DODownloadMode -ErrorAction SilentlyContinue"#,
        check:       "(Get-ItemProperty 'HKLM:\\SOFTWARE\\Policies\\Microsoft\\Windows\\DeliveryOptimization' -ErrorAction SilentlyContinue).DODownloadMode -eq 0",
    },

    // ── Maps ──────────────────────────────────────────────────────────────────
    PrivacyTweak {
        id:          "prv_maps_update_off",
        name:        "Automatische Karten-Updates deaktivieren",
        category:    "Netzwerk",
        description: "Verhindert, dass Windows im Hintergrund Offline-Kartendaten herunterlädt.",
        risk:        "Low",
        apply:       r#"$p='HKLM:\SYSTEM\Maps'; if(Test-Path $p){ Set-ItemProperty -Path $p -Name AutoUpdateEnabled -Value 0 -Type DWord -ErrorAction SilentlyContinue }"#,
        revert:      r#"$p='HKLM:\SYSTEM\Maps'; if(Test-Path $p){ Set-ItemProperty -Path $p -Name AutoUpdateEnabled -Value 1 -Type DWord -ErrorAction SilentlyContinue }"#,
        check:       "(Get-ItemProperty 'HKLM:\\SYSTEM\\Maps' -ErrorAction SilentlyContinue).AutoUpdateEnabled -eq 0",
    },
];

// ── status check ─────────────────────────────────────────────────────────────

fn check_statuses() -> std::collections::HashMap<String, bool> {
    let assignments: Vec<String> = TWEAKS.iter().map(|t| {
        let var = t.id.replace('-', "_");
        format!("${var} = try {{ [bool]({}) }} catch {{ $false }};", t.check)
    }).collect();
    let entries: Vec<String> = TWEAKS.iter().map(|t| {
        let var = t.id.replace('-', "_");
        format!("'{}' = ${};", t.id, var)
    }).collect();
    let script = format!(
        "{} $r = @{{{}}}; $r | ConvertTo-Json -Compress",
        assignments.join(" "), entries.join(" ")
    );
    let raw = ps::run(&script).unwrap_or_default();
    let v: Value = serde_json::from_str(raw.trim()).unwrap_or(json!({}));
    TWEAKS.iter().map(|t| (t.id.to_string(), v[t.id].as_bool().unwrap_or(false))).collect()
}

// ── public API ────────────────────────────────────────────────────────────────

pub fn scan() -> Value {
    let statuses = check_statuses();
    let tweaks: Vec<Value> = TWEAKS.iter().map(|t| {
        json!({
            "id":          t.id,
            "name":        t.name,
            "category":    t.category,
            "description": t.description,
            "risk":        t.risk,
            "applied":     statuses.get(t.id).copied().unwrap_or(false),
        })
    }).collect();
    let applied = tweaks.iter().filter(|t| t["applied"].as_bool().unwrap_or(false)).count();
    json!({ "tweaks": tweaks, "applied": applied, "total": tweaks.len() })
}

pub fn apply(id: String) -> Result<Value, String> {
    let t = TWEAKS.iter().find(|t| t.id == id)
        .ok_or_else(|| format!("Unknown tweak: {id}"))?;
    ps::run(t.apply).map_err(|e| e)?;
    Ok(json!({ "ok": true, "id": id }))
}

pub fn revert(id: String) -> Result<Value, String> {
    let t = TWEAKS.iter().find(|t| t.id == id)
        .ok_or_else(|| format!("Unknown tweak: {id}"))?;
    ps::run(t.revert).map_err(|e| e)?;
    Ok(json!({ "ok": true, "id": id }))
}
