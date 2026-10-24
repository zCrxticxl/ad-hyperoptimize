//! Windows Debloater, remove UWP bloatware + disable telemetry/ads/Cortana.

use crate::ps;
use serde_json::{json, Value};

// ── UWP bloatware list ────────────────────────────────────────────────────────

const OPTIONAL_APPS: &[&str] = &[
    "Microsoft.BingNews",
    "Microsoft.BingWeather",
    "Microsoft.BingFinance",
    "Microsoft.BingSports",
    "Microsoft.MicrosoftSolitaireCollection",
    "Microsoft.MixedReality.Portal",
    "Microsoft.People",
    "Microsoft.SkypeApp",
    "Microsoft.Teams",
    // New Teams (Teams 2.x / work or school) ships under a different name.
    "MSTeams",
    "Microsoft.ZuneMusic",
    "Microsoft.ZuneVideo",
    "Microsoft.GetHelp",
    "Microsoft.Getstarted",
    "Microsoft.Microsoft3DViewer",
    "Microsoft.XboxApp",
    "Microsoft.XboxGameOverlay",
    "Microsoft.XboxGamingOverlay",
    "Microsoft.XboxIdentityProvider",
    "king.com.CandyCrushSaga",
    "king.com.CandyCrushFriends",
    "BytedancePte.Ltd.TikTok",
    "SpotifyAB.SpotifyMusic",
    "Disney.37853D22215E2",
    "AmazonVideo.PrimeVideo",
];

/// Catalog entries that are removable on request but must never be
/// recommended or preselected: the Xbox components are needed by Game Pass /
/// Xbox sign-in and Game Bar capture, ZuneMusic is the Windows 11 Media
/// Player, and GetHelp is the inbox troubleshooter host.
const NOT_RECOMMENDED: &[&str] = &[
    "Microsoft.XboxIdentityProvider",
    "Microsoft.XboxGamingOverlay",
    "Microsoft.XboxGameOverlay",
    "Microsoft.ZuneMusic",
    "Microsoft.GetHelp",
];

fn is_recommended(name: &str) -> bool {
    OPTIONAL_APPS.contains(&name) && !NOT_RECOMMENDED.contains(&name)
}

pub fn list_uwp() -> Value {
    let allowed = OPTIONAL_APPS
        .iter()
        .map(|name| format!("'{name}'"))
        .collect::<Vec<_>>()
        .join(",");
    let script = format!(
        r#"
$allowed = @({allowed})
$installed = @(Get-AppxPackage -AllUsers -ErrorAction Stop | Where-Object {{ $_.Name -in $allowed }})
$provisioned = @(Get-AppxProvisionedPackage -Online -ErrorAction Stop | Where-Object {{ $_.DisplayName -in $allowed }})
$rows = foreach ($name in $allowed) {{
    $pkg = $installed | Where-Object {{ $_.Name -eq $name }} | Select-Object -First 1
    $prov = $provisioned | Where-Object {{ $_.DisplayName -eq $name }} | Select-Object -First 1
    if ($pkg -or $prov) {{
        [PSCustomObject]@{{ Name=$name; PackageFullName=$(if ($pkg) {{ $pkg.PackageFullName }} else {{ $null }}); provisioned=[bool]$prov }}
    }}
}}
@{{ apps=@($rows) }} | ConvertTo-Json -Compress -Depth 3
"#
    );
    let mut v = ps::run_json(&script).unwrap_or_else(|e| json!({ "apps": [], "error": e }));
    // ConvertTo-Json collapses a one-element array to an object.
    if let Some(o @ Value::Object(_)) = v.get("apps").cloned() {
        v["apps"] = json!([o]);
    }
    if let Some(apps) = v.get_mut("apps").and_then(Value::as_array_mut) {
        for app in apps {
            let rec = app["Name"].as_str().is_some_and(is_recommended);
            app["recommended"] = json!(rec);
        }
    }
    v
}

pub fn remove_uwp(package_full_name: String) -> Result<String, String> {
    if !ps::is_safe_ident(&package_full_name) {
        return Err("Invalid package name".into());
    }
    let script = format!(
        r#"
$pkg = Get-AppxPackage -AllUsers -ErrorAction Stop | Where-Object {{ $_.PackageFullName -eq '{package_full_name}' }} | Select-Object -First 1
if (-not $pkg) {{ throw 'Package is no longer installed' }}
try {{
    Remove-AppxPackage -Package $pkg.PackageFullName -AllUsers -ErrorAction Stop 2>&1
    "Removed"
}} catch {{
    $allUsersError = $_.Exception.Message
    try {{
        Remove-AppxPackage -Package $pkg.PackageFullName -ErrorAction Stop 2>&1
    }} catch {{
        throw $_
    }}
    # The package is still installed for other users: this is a partial
    # removal and must not be reported as success.
    throw ("PARTIAL: removed for the current user only; removal for all users failed: " + $allUsersError)
}}
"#
    );
    let allowed = OPTIONAL_APPS
        .iter()
        .any(|name| package_full_name.starts_with(&format!("{name}_")));
    if !allowed {
        return Err("Package is not in the optional-app catalog".into());
    }
    ps::run(&script).map(|s| s.trim().to_string())
}

pub fn remove_uwp_provisioned(package_name: String) -> Result<String, String> {
    if !OPTIONAL_APPS.contains(&package_name.as_str()) {
        return Err("Package is not in the optional-app catalog".into());
    }
    let script = format!(
        r#"
$pkg = Get-AppxProvisionedPackage -Online -ErrorAction SilentlyContinue |
    Where-Object {{ $_.DisplayName -eq '{package_name}' }} | Select-Object -First 1
if ($pkg) {{
    Remove-AppxProvisionedPackage -Online -PackageName $pkg.PackageName -ErrorAction Stop | Out-Null
    "Removed provisioned package"
}} else {{ throw 'Package is no longer provisioned' }}
"#
    );
    ps::run(&script).map(|s| s.trim().to_string())
}

// ── System tweaks ─────────────────────────────────────────────────────────────

struct Tweak {
    id: &'static str,
    name: &'static str,
    desc: &'static str,
    cat: &'static str,
    // PowerShell to check current state → should output "1" if tweak is applied
    check: &'static str,
    apply: &'static str,
    // Fallback undo with documented defaults, only used when no captured
    // state exists (e.g. a revert without a matching apply in this install).
    undo: &'static str,
    /// Registry values written by `apply` (root, path, name). Captured before
    /// applying so the undo restores the exact previous value, or removes
    /// the value when it did not exist before (never guesses a default).
    capture: &'static [(&'static str, &'static str, &'static str)],
    /// Service controlled by `apply`, start type and running state are
    /// captured before applying and restored exactly on undo.
    service: Option<&'static str>,
}

static TWEAKS: &[Tweak] = &[
    Tweak {
        id: "telemetry_off",
        name: "Disable Telemetry",
        desc: "Sets AllowTelemetry=1 (Basic), the lowest level Windows Home/Pro honor. Prevents Enhanced/Full diagnostic data upload.",
        cat: "Telemetry",
        check: r#"(Get-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' -Name AllowTelemetry -EA SilentlyContinue).AllowTelemetry -eq 1"#,
        apply: r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection'; if(!(Test-Path $p)){New-Item $p -Force -EA Stop|Out-Null}; Set-ItemProperty $p AllowTelemetry 1 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"Remove-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' AllowTelemetry -EA SilentlyContinue"#,
        capture: &[("HKLM", "SOFTWARE\\Policies\\Microsoft\\Windows\\DataCollection", "AllowTelemetry")],
        service: None,
    },
    Tweak {
        id: "diagtrack_stop",
        name: "Stop Connected User Experiences (DiagTrack)",
        desc: "Stops and disables the telemetry service that sends data to Microsoft.",
        cat: "Telemetry",
        check: r#"(Get-Service DiagTrack -EA SilentlyContinue).StartType -eq 'Disabled'"#,
        apply: r#"Stop-Service DiagTrack -Force -EA SilentlyContinue; Set-Service DiagTrack -StartupType Disabled -EA Stop; 'Applied'"#,
        undo:  r#"Set-Service DiagTrack -StartupType Automatic; Start-Service DiagTrack -EA SilentlyContinue"#,
        capture: &[],
        service: Some("DiagTrack"),
    },
    Tweak {
        id: "activity_history",
        name: "Disable Activity History",
        desc: "Stops Windows from storing and uploading your activity timeline.",
        cat: "Telemetry",
        check: r#"(Get-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name PublishUserActivities -EA SilentlyContinue).PublishUserActivities -eq 0"#,
        apply: r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\System'; if(!(Test-Path $p)){New-Item $p -Force -EA Stop|Out-Null}; Set-ItemProperty $p PublishUserActivities 0 -Type DWord -EA Stop; Set-ItemProperty $p EnableActivityFeed 0 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\System'; Remove-ItemProperty $p PublishUserActivities -EA SilentlyContinue; Remove-ItemProperty $p EnableActivityFeed -EA SilentlyContinue"#,
        capture: &[
            ("HKLM", "SOFTWARE\\Policies\\Microsoft\\Windows\\System", "PublishUserActivities"),
            ("HKLM", "SOFTWARE\\Policies\\Microsoft\\Windows\\System", "EnableActivityFeed"),
        ],
        service: None,
    },
    Tweak {
        id: "advertising_id",
        name: "Disable Advertising ID",
        desc: "Stops apps from using your advertising ID for targeted ads.",
        cat: "Ads & Clutter",
        check: r#"(Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo' -Name Enabled -EA SilentlyContinue).Enabled -eq 0"#,
        apply: r#"$p='HKCU:\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo'; if(!(Test-Path $p)){New-Item $p -Force -EA Stop|Out-Null}; Set-ItemProperty $p Enabled 0 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo' Enabled 1 -Type DWord -EA SilentlyContinue"#,
        capture: &[("HKCU", "Software\\Microsoft\\Windows\\CurrentVersion\\AdvertisingInfo", "Enabled")],
        service: None,
    },
    Tweak {
        id: "bing_search",
        name: "Disable Bing in Start Menu",
        desc: "Removes web/Bing results from Windows Search. Faster, private.",
        cat: "Ads & Clutter",
        check: r#"(Get-ItemProperty 'HKCU:\Software\Policies\Microsoft\Windows\Explorer' -Name DisableSearchBoxSuggestions -EA SilentlyContinue).DisableSearchBoxSuggestions -eq 1"#,
        apply: r#"$p='HKCU:\Software\Policies\Microsoft\Windows\Explorer'; if(!(Test-Path $p)){New-Item $p -Force -EA Stop|Out-Null}; Set-ItemProperty $p DisableSearchBoxSuggestions 1 -Type DWord -EA Stop; Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' BingSearchEnabled 0 -Type DWord -EA SilentlyContinue; 'Applied'"#,
        undo:  r#"Remove-ItemProperty 'HKCU:\Software\Policies\Microsoft\Windows\Explorer' DisableSearchBoxSuggestions -EA SilentlyContinue; Remove-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' BingSearchEnabled -EA SilentlyContinue"#,
        capture: &[
            ("HKCU", "SOFTWARE\\Policies\\Microsoft\\Windows\\Explorer", "DisableSearchBoxSuggestions"),
            ("HKCU", "Software\\Microsoft\\Windows\\CurrentVersion\\Search", "BingSearchEnabled"),
        ],
        service: None,
    },
    Tweak {
        id: "cortana_off",
        name: "Disable Cortana",
        desc: "Prevents Cortana from running. Also reduces Search resource usage.",
        cat: "Privacy",
        check: r#"(Get-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name AllowCortana -EA SilentlyContinue).AllowCortana -eq 0"#,
        apply: r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search'; if(!(Test-Path $p)){New-Item $p -Force -EA Stop|Out-Null}; Set-ItemProperty $p AllowCortana 0 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"Remove-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' AllowCortana -EA SilentlyContinue"#,
        capture: &[("HKLM", "SOFTWARE\\Policies\\Microsoft\\Windows\\Windows Search", "AllowCortana")],
        service: None,
    },
    Tweak {
        id: "app_suggestions",
        name: "Disable App Suggestions / Tips",
        desc: "Removes suggested apps in Start and 'Did you know?' tips.",
        cat: "Ads & Clutter",
        check: r#"$p='HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'; (@('SubscribedContent-338389Enabled','SubscribedContent-338388Enabled','SubscribedContent-353698Enabled','SystemPaneSuggestionsEnabled','SoftLandingEnabled') | ForEach-Object { (Get-ItemProperty $p -Name $_ -EA SilentlyContinue).$_ -eq 0 }) -notcontains $false"#,
        apply: r#"$p='HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'; @('SubscribedContent-338389Enabled','SubscribedContent-338388Enabled','SubscribedContent-353698Enabled','SystemPaneSuggestionsEnabled','SoftLandingEnabled') | ForEach-Object { Set-ItemProperty $p $_ 0 -Type DWord -EA Stop }; 'Applied'"#,
        undo:  r#"$p='HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'; @('SubscribedContent-338389Enabled','SubscribedContent-338388Enabled','SubscribedContent-353698Enabled','SystemPaneSuggestionsEnabled','SoftLandingEnabled') | ForEach-Object { Set-ItemProperty $p $_ 1 -Type DWord -EA SilentlyContinue }"#,
        capture: &[
            ("HKCU", "Software\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager", "SubscribedContent-338389Enabled"),
            ("HKCU", "Software\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager", "SubscribedContent-338388Enabled"),
            ("HKCU", "Software\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager", "SubscribedContent-353698Enabled"),
            ("HKCU", "Software\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager", "SystemPaneSuggestionsEnabled"),
            ("HKCU", "Software\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager", "SoftLandingEnabled"),
        ],
        service: None,
    },
    Tweak {
        id: "xbox_gamebar",
        name: "Disable Xbox Game Bar",
        desc: "Disables Win+G overlay. Reduces background CPU/GPU usage in games.",
        cat: "Gaming",
        check: r#"(Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\GameDVR' -Name AppCaptureEnabled -EA SilentlyContinue).AppCaptureEnabled -eq 0"#,
        apply: r#"Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\GameDVR' AppCaptureEnabled 0 -Type DWord -EA Stop; $p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\GameDVR'; if(!(Test-Path $p)){New-Item $p -Force -EA Stop|Out-Null}; Set-ItemProperty $p AllowGameDVR 0 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\GameDVR' AppCaptureEnabled 1 -Type DWord -EA SilentlyContinue; Remove-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\GameDVR' AllowGameDVR -EA SilentlyContinue"#,
        capture: &[
            ("HKCU", "Software\\Microsoft\\Windows\\CurrentVersion\\GameDVR", "AppCaptureEnabled"),
            ("HKLM", "SOFTWARE\\Policies\\Microsoft\\Windows\\GameDVR", "AllowGameDVR"),
        ],
        service: None,
    },
    Tweak {
        id: "location_off",
        name: "Disable Location Services",
        desc: "Prevents apps from accessing your location via Windows Location API.",
        cat: "Privacy",
        check: r#"(Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\location' -Name Value -EA SilentlyContinue).Value -eq 'Deny'"#,
        apply: r#"Set-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\location' Value 'Deny' -EA Stop; 'Applied'"#,
        undo:  r#"Set-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\location' Value 'Allow' -EA SilentlyContinue"#,
        capture: &[("HKLM", "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\location", "Value")],
        service: None,
    },
    Tweak {
        id: "feedback_off",
        name: "Disable Feedback Requests",
        desc: "Stops Windows from asking for feedback periodically.",
        cat: "Telemetry",
        check: r#"(Get-ItemProperty 'HKCU:\Software\Microsoft\Siuf\Rules' -Name NumberOfSIUFInPeriod -EA SilentlyContinue).NumberOfSIUFInPeriod -eq 0"#,
        apply: r#"$p='HKCU:\Software\Microsoft\Siuf\Rules'; if(!(Test-Path $p)){New-Item $p -Force -EA Stop|Out-Null}; Set-ItemProperty $p NumberOfSIUFInPeriod 0 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"Remove-ItemProperty 'HKCU:\Software\Microsoft\Siuf\Rules' NumberOfSIUFInPeriod -EA SilentlyContinue"#,
        capture: &[("HKCU", "Software\\Microsoft\\Siuf\\Rules", "NumberOfSIUFInPeriod")],
        service: None,
    },
    Tweak {
        id: "error_reporting_off",
        name: "Disable Windows Error Reporting",
        desc: "Stops crash dumps from being sent to Microsoft.",
        cat: "Telemetry",
        check: r#"(Get-Service WerSvc -EA SilentlyContinue).StartType -eq 'Disabled'"#,
        apply: r#"Stop-Service WerSvc -Force -EA SilentlyContinue; Set-Service WerSvc -StartupType Disabled -EA Stop; 'Applied'"#,
        undo:  r#"Set-Service WerSvc -StartupType Manual; Start-Service WerSvc -EA SilentlyContinue"#,
        capture: &[],
        service: Some("WerSvc"),
    },
    Tweak {
        id: "lock_screen_ads",
        name: "Disable Lock Screen Ads / Spotlight",
        desc: "Prevents Windows Spotlight from replacing your lock screen with ads.",
        cat: "Ads & Clutter",
        check: r#"(Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name RotatingLockScreenEnabled -EA SilentlyContinue).RotatingLockScreenEnabled -eq 0"#,
        apply: r#"$p='HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'; Set-ItemProperty $p RotatingLockScreenEnabled 0 -Type DWord -EA Stop; Set-ItemProperty $p RotatingLockScreenOverlayEnabled 0 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"$p='HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'; Set-ItemProperty $p RotatingLockScreenEnabled 1 -Type DWord -EA SilentlyContinue; Set-ItemProperty $p RotatingLockScreenOverlayEnabled 1 -Type DWord -EA SilentlyContinue"#,
        capture: &[
            ("HKCU", "Software\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager", "RotatingLockScreenEnabled"),
            ("HKCU", "Software\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager", "RotatingLockScreenOverlayEnabled"),
        ],
        service: None,
    },
    Tweak {
        id: "taskbar_widgets_off",
        name: "Disable Taskbar News & Widgets",
        desc: "Removes the News & Interests widget panel from the taskbar. Stops background news fetch.",
        cat: "Ads & Clutter",
        check: r#"(Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Feeds' -Name ShellFeedsTaskbarViewMode -EA SilentlyContinue).ShellFeedsTaskbarViewMode -eq 2"#,
        apply: r#"$p='HKCU:\Software\Microsoft\Windows\CurrentVersion\Feeds'; if(!(Test-Path $p)){New-Item $p -Force -EA Stop|Out-Null}; Set-ItemProperty $p ShellFeedsTaskbarViewMode 2 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"Remove-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Feeds' ShellFeedsTaskbarViewMode -EA SilentlyContinue"#,
        capture: &[("HKCU", "Software\\Microsoft\\Windows\\CurrentVersion\\Feeds", "ShellFeedsTaskbarViewMode")],
        service: None,
    },
    Tweak {
        id: "clipboard_sync_off",
        name: "Disable Clipboard History & Cloud Sync",
        desc: "Prevents Windows from storing clipboard history and syncing it across devices via Microsoft account.",
        cat: "Privacy",
        check: r#"(Get-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name AllowClipboardHistory -EA SilentlyContinue).AllowClipboardHistory -eq 0"#,
        apply: r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\System'; if(!(Test-Path $p)){New-Item $p -Force -EA Stop|Out-Null}; Set-ItemProperty $p AllowClipboardHistory 0 -Type DWord -EA Stop; Set-ItemProperty $p AllowCrossDeviceClipboard 0 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\System'; Remove-ItemProperty $p AllowClipboardHistory -EA SilentlyContinue; Remove-ItemProperty $p AllowCrossDeviceClipboard -EA SilentlyContinue"#,
        capture: &[
            ("HKLM", "SOFTWARE\\Policies\\Microsoft\\Windows\\System", "AllowClipboardHistory"),
            ("HKLM", "SOFTWARE\\Policies\\Microsoft\\Windows\\System", "AllowCrossDeviceClipboard"),
        ],
        service: None,
    },
    Tweak {
        id: "gamedvr_off",
        name: "Disable Game DVR (Background Recording)",
        desc: "Stops Windows from silently recording your gameplay in the background. Frees GPU memory and reduces stutters.",
        cat: "Gaming",
        check: r#"(Get-ItemProperty 'HKCU:\System\GameConfigStore' -Name GameDVR_Enabled -EA SilentlyContinue).GameDVR_Enabled -eq 0"#,
        apply: r#"$p='HKCU:\System\GameConfigStore'; Set-ItemProperty $p GameDVR_Enabled 0 -Type DWord -EA Stop; Set-ItemProperty $p GameDVR_FSEBehaviorMode 2 -Type DWord -EA Stop; Set-ItemProperty $p GameDVR_HonorUserFSEBehaviorMode 1 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"$p='HKCU:\System\GameConfigStore'; Set-ItemProperty $p GameDVR_Enabled 1 -Type DWord -EA SilentlyContinue; Remove-ItemProperty $p GameDVR_FSEBehaviorMode -EA SilentlyContinue; Remove-ItemProperty $p GameDVR_HonorUserFSEBehaviorMode -EA SilentlyContinue"#,
        capture: &[
            ("HKCU", "System\\GameConfigStore", "GameDVR_Enabled"),
            ("HKCU", "System\\GameConfigStore", "GameDVR_FSEBehaviorMode"),
            ("HKCU", "System\\GameConfigStore", "GameDVR_HonorUserFSEBehaviorMode"),
        ],
        service: None,
    },
];

/// A Debloater tweak that is no longer offered here: it is obsolete, uses the
/// wrong mechanism, or the same setting has a single home on another page.
/// It is listed only while the Debloater holds its captured pre-apply state,
/// so it can be reset exactly; it can never be applied again, and it is never
/// reset with the guessed-default `undo` (that could undo a change the other
/// page made to the same value).
struct Withdrawn {
    id: &'static str,
    reason: &'static str,
    /// (page, id) of the single source that now offers this setting.
    moved_to: Option<(&'static str, &'static str)>,
}

static WITHDRAWN: &[Withdrawn] = &[
    Withdrawn {
        id: "cortana_off",
        reason: "Microsoft retired the Cortana app; this policy has nothing left to disable.",
        moved_to: None,
    },
    Withdrawn {
        id: "taskbar_widgets_off",
        reason: "This value only hid News and Interests on Windows 10; Windows 11 Widgets ignore it. Optimize offers the working Widgets setting.",
        moved_to: Some(("optimize", "widgets_off")),
    },
    Withdrawn {
        id: "gamedvr_off",
        reason: "Also forced exclusive fullscreen (FSEBehaviorMode = 2), a separate setting with no proven benefit. Optimize offers a DVR switch that changes only recording.",
        moved_to: Some(("optimize", "game_dvr_off")),
    },
    Withdrawn {
        id: "telemetry_off",
        reason: "Same policy value as in the Privacy Center, which is now the single place for it.",
        moved_to: Some(("privacy", "prv_telemetry_basic")),
    },
    Withdrawn {
        id: "diagtrack_stop",
        reason: "Same service setting as in the Privacy Center, which is now the single place for it.",
        moved_to: Some(("privacy", "prv_diagtrack_off")),
    },
    Withdrawn {
        id: "activity_history",
        reason: "Same policy values as in the Privacy Center, which is now the single place for it.",
        moved_to: Some(("privacy", "prv_activity_history_off")),
    },
    Withdrawn {
        id: "advertising_id",
        reason: "Same setting as in the Privacy Center, which is now the single place for it.",
        moved_to: Some(("privacy", "prv_adid_off")),
    },
    Withdrawn {
        id: "app_suggestions",
        reason: "Same Start suggestion values as in the Privacy Center, which is now the single place for them.",
        moved_to: Some(("privacy", "prv_startmenu_suggestions_off")),
    },
    Withdrawn {
        id: "feedback_off",
        reason: "Same setting as in the Privacy Center, which is now the single place for it.",
        moved_to: Some(("privacy", "prv_feedback_never")),
    },
    Withdrawn {
        id: "error_reporting_off",
        reason: "Same service as in the Privacy Center, which is now the single place for it.",
        moved_to: Some(("privacy", "prv_wer_off")),
    },
];

/// Offered tweaks that overlap a setting on another page without being the
/// same mechanism: (id, page, id there). Shown as a cross-reference.
static RELATED: &[(&str, &str, &str)] = &[
    // Settings → Location (ConsentStore) here; the Privacy Center disables the
    // location service (lfsvc) instead.
    ("location_off", "privacy", "prv_location_off"),
];

fn withdrawn(id: &str) -> Option<&'static Withdrawn> {
    WITHDRAWN.iter().find(|w| w.id == id)
}

#[cfg(test)]
pub(crate) fn is_withdrawn(id: &str) -> bool {
    withdrawn(id).is_some()
}

/// Whether the list shows a tweak: offered ones always, withdrawn ones only
/// while the Debloater can reset them from a captured state.
fn listed(id: &str, captured: bool) -> bool {
    withdrawn(id).is_none() || captured
}

pub fn list_tweaks() -> Value {
    // Check all tweak states in a single PS call
    let checks: Vec<String> = TWEAKS
        .iter()
        .map(|t| format!("try{{if({}){{1}}else{{0}}}}catch{{0}}", t.check))
        .collect();
    let script = format!("@({}) | ConvertTo-Json -Compress", checks.join(","));

    let states: Vec<bool> = match ps::run_json(&script) {
        Ok(Value::Array(arr)) => arr.iter().map(|v| v.as_i64().unwrap_or(0) == 1).collect(),
        Ok(v) => vec![v.as_i64().unwrap_or(0) == 1],
        _ => vec![false; TWEAKS.len()],
    };

    let mut tweaks = Vec::new();
    let mut withdrawn_active = Vec::new();
    for (i, t) in TWEAKS.iter().enumerate() {
        let live = states.get(i).copied().unwrap_or(false);
        // A tweak we successfully applied always has a captured state file.
        // The live registry check can be flaky on some systems (value
        // read-back differs from what we wrote), so treat "we applied it
        // in this app" as applied too, revert uses that exact state file,
        // so the status never wrongly flips back to "not applied".
        let captured = has_captured_state(t.id);
        if !listed(t.id, captured) {
            continue;
        }
        let related = RELATED
            .iter()
            .find(|(id, _, _)| *id == t.id)
            .map(|(_, page, id)| json!({ "page": page, "id": id }));
        let mut entry = json!({
            "id":      t.id,
            "name":    t.name,
            "desc":    t.desc,
            "cat":     t.cat,
            "applied": live || captured,
            "related": related,
        });
        match withdrawn(t.id) {
            Some(w) => {
                entry["withdrawn"] = json!(true);
                entry["reason"] = json!(w.reason);
                entry["movedTo"] = json!(w
                    .moved_to
                    .map(|(page, id)| json!({ "page": page, "id": id })));
                withdrawn_active.push(entry);
            }
            None => tweaks.push(entry),
        }
    }

    json!({ "tweaks": tweaks, "withdrawn": withdrawn_active })
}

// ── Exact undo: capture pre-apply state, restore it on revert ─────────────────

/// Lives in the protected state folder: restore_script runs elevated.
fn state_path(id: &str) -> Result<std::path::PathBuf, String> {
    Ok(crate::safety::state_dir()?
        .join("debloater")
        .join(format!("{id}.json")))
}

/// The captured items of a state file. `$out | ConvertTo-Json` wrote a
/// single captured item as a bare object instead of a one-element array;
/// files written that way (every tweak with one value or only a service)
/// must still restore exactly instead of falling back to the guessed `undo`.
fn state_items(state: &serde_json::Value) -> Vec<&serde_json::Value> {
    match state {
        serde_json::Value::Array(items) => items.iter().collect(),
        serde_json::Value::Object(_) => vec![state],
        _ => Vec::new(),
    }
}

/// A migrated legacy state file is accepted only if every captured item is
/// one this tweak's catalog entry declares (statestore.rs): restore_script
/// would otherwise write attacker-chosen registry values with admin rights.
pub(crate) fn state_is_valid(id: &str, state: &serde_json::Value) -> bool {
    let Some(t) = TWEAKS.iter().find(|t| t.id == id) else {
        return false;
    };
    if !(state.is_array() || state.is_object()) {
        return false;
    }
    state_items(state)
        .iter()
        .all(|it| match it["kind"].as_str() {
            Some("reg") => t.capture.iter().any(|(root, path, name)| {
                it["root"].as_str() == Some(*root)
                    && it["path"].as_str() == Some(*path)
                    && it["name"].as_str() == Some(*name)
            }),
            Some("service") => t.service.is_some() && it["name"].as_str() == t.service,
            _ => false,
        })
}

/// Capture registry values + service state listed in `t.capture`/`t.service`
/// as JSON, ready to be stored next to the applied tweak.
fn capture_state(t: &Tweak) -> Result<serde_json::Value, String> {
    let items = t
        .capture
        .iter()
        .map(|(root, path, name)| format!("@{{root='{root}';path='{path}';name='{name}'}}"))
        .collect::<Vec<_>>()
        .join(",");
    let svc_block = match t.service {
        Some(svc) => format!(
            r#"
$svc = Get-Service -Name '{svc}' -EA SilentlyContinue
if ($svc) {{
    $out += @{{ kind='service'; name='{svc}'; startType=$svc.StartType.ToString(); status=$svc.Status.ToString() }}
}}"#
        ),
        None => String::new(),
    };
    let script = format!(
        r#"
$out = @()
$items = @({items})
foreach ($i in $items) {{
    try {{
        $v = (Get-ItemProperty "$($i.root):\$($i.path)" -Name $i.name -ErrorAction Stop).$($i.name)
        $out += @{{ kind='reg'; root=$i.root; path=$i.path; name=$i.name;
                    value=$v; isNumber=($v -is [int32] -or $v -is [int64] -or $v -is [uint32]) }}
    }} catch {{
        $out += @{{ kind='reg'; root=$i.root; path=$i.path; name=$i.name; value=$null; isNumber=$false }}
    }}
}}
{svc_block}
ConvertTo-Json -InputObject @($out) -Compress -Depth 4
"#
    );
    ps::run_json(&script)
}

/// Build the PowerShell that restores a captured state exactly: registry
/// values back to their previous value (or removed if they were absent),
/// service back to its previous start type and running state.
fn restore_script(state: &serde_json::Value) -> String {
    // Every interpolated field is single-quote-escaped and numbers are taken
    // only as integers: this script runs elevated, so a tampered state file
    // must never be able to break out of the quoted PS strings.
    let q = crate::ps::escape_single;
    // Each step runs on its own and reports its own error, so one failure
    // does not hide the others and the message says what failed.
    let step = |what: &str, body: String| {
        format!("try {{ {body} }} catch {{ $e += '{what}: ' + $_.Exception.Message }}")
    };
    let mut steps = Vec::new();
    {
        for it in state_items(state) {
            match it["kind"].as_str() {
                Some("reg") => {
                    let root = q(it["root"].as_str().unwrap_or("HKCU"));
                    let path = q(it["path"].as_str().unwrap_or(""));
                    let name = q(it["name"].as_str().unwrap_or(""));
                    let key = format!("{root}:\\{path}");
                    let what = format!("{key}\\{name}");
                    // A captured value implies its key existed: recreate it
                    // if something removed it since.
                    let ensure_key =
                        format!("if (!(Test-Path '{key}')) {{ New-Item '{key}' -Force -EA Stop | Out-Null }}; ");
                    if it["value"].is_null() {
                        steps.push(step(
                            &what,
                            format!("Remove-ItemProperty '{key}' '{name}' -EA SilentlyContinue"),
                        ));
                    } else if it["isNumber"].as_bool().unwrap_or(false) {
                        let Some(v) = it["value"].as_i64() else {
                            steps.push(format!("$e += '{what}: captured value is not an integer'"));
                            continue;
                        };
                        steps.push(step(
                            &what,
                            format!(
                                "{ensure_key}Set-ItemProperty '{key}' '{name}' ([int]({v})) -Type DWord -EA Stop"
                            ),
                        ));
                    } else {
                        let v = q(it["value"].as_str().unwrap_or(""));
                        steps.push(step(
                            &what,
                            format!("{ensure_key}Set-ItemProperty '{key}' '{name}' '{v}' -EA Stop"),
                        ));
                    }
                }
                Some("service") => {
                    let svc = q(it["name"].as_str().unwrap_or(""));
                    // Whitelist instead of escaping: StartType has exactly
                    // three valid values.
                    let start_type = it["startType"].as_str().unwrap_or("Manual");
                    let start = if matches!(start_type, "Automatic" | "Manual" | "Disabled") {
                        start_type
                    } else {
                        "Manual"
                    };
                    let mut body =
                        format!("Set-Service -Name '{svc}' -StartupType {start} -EA Stop");
                    match it["status"].as_str() {
                        Some("Running") => body.push_str(&format!(
                            "; Start-Service -Name '{svc}' -EA SilentlyContinue"
                        )),
                        Some("Stopped") => body.push_str(&format!(
                            "; Stop-Service -Name '{svc}' -Force -EA SilentlyContinue"
                        )),
                        _ => {}
                    }
                    steps.push(step(&format!("service {svc}"), body));
                }
                _ => {}
            }
        }
    }
    if steps.is_empty() {
        return String::new();
    }
    format!(
        "$e = @(); {}; if ($e.Count) {{ [Console]::Error.WriteLine($e -join ' | '); exit 1 }}",
        steps.join("; ")
    )
}

pub fn apply_tweak(id: String) -> Result<String, String> {
    if let Some(w) = withdrawn(&id) {
        return Err(format!(
            "'{id}' is no longer offered by the Debloater: {} Only reset is still available.",
            w.reason
        ));
    }
    let t = TWEAKS
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("Unknown tweak: {id}"))?;
    // Capture before applying, the undo restores exactly this.
    let state = capture_state(t)?;
    // Write-ahead: persist the captured state BEFORE applying, so a partial
    // apply (or a crash mid-apply) is still undoable via revert_tweak. An
    // existing state file is kept: it holds the original pre-apply values,
    // a re-apply must not overwrite it with already-applied ones.
    let path = state_path(id.as_str())?;
    let wrote_state = if path.exists() {
        false
    } else {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, serde_json::to_string(&state).unwrap_or_default())
            .map_err(|e| format!("saving tweak state failed: {e}"))?;
        true
    };
    if let Err(e) = ps::run(t.apply) {
        if !wrote_state {
            return Err(e);
        }
        // Undo whatever part of the apply landed; only drop the state file
        // once that restore succeeded, otherwise keep it for a manual undo.
        let script = restore_script(&state);
        let restored = script.trim().is_empty() || ps::run(&script).is_ok();
        if restored {
            let _ = std::fs::remove_file(&path);
            return Err(format!("{e} (changes rolled back)"));
        }
        return Err(format!(
            "{e} (rollback failed; captured state kept, use Revert to undo)"
        ));
    }
    Ok(format!("Applied: {}", t.name))
}

/// Whether an exact pre-apply state was captured for this tweak.
pub(crate) fn has_captured_state(id: &str) -> bool {
    state_path(id).is_ok_and(|p| p.exists())
}

pub fn revert_tweak(id: String) -> Result<String, String> {
    let t = TWEAKS
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("Unknown tweak: {id}"))?;
    let path = state_path(id.as_str())?;
    match std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
    {
        // Exact undo from the captured pre-apply state.
        Some(state) => {
            let script = restore_script(&state);
            let res = if script.trim().is_empty() {
                ps::run(t.undo)
            } else {
                ps::run(&script)
            };
            match res {
                Ok(_) => {
                    // Only drop the captured state once the exact restore
                    // succeeded, deleting it on failure would silently
                    // downgrade a retry to the guessed-default static undo.
                    let _ = std::fs::remove_file(&path);
                    Ok(format!("Reverted: {}", t.name))
                }
                Err(e) => Err(format!(
                    "revert failed: {e} (captured state kept for retry)"
                )),
            }
        }
        // A withdrawn tweak's value may now be owned by another page (same
        // registry value); the guessed default would undo that page's change.
        None if withdrawn(&id).is_some() => Err(format!(
            "No captured state for {id}: the Debloater did not change this setting, so it does not reset it."
        )),
        // No captured state (apply happened outside this app or the state was
        // lost), fall back to the documented-default undo.
        None => ps::run(t.undo).map(|_| format!("Reverted: {}", t.name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_packages_are_never_recommended() {
        for n in NOT_RECOMMENDED {
            assert!(OPTIONAL_APPS.contains(n), "{n} missing from catalog");
            assert!(!is_recommended(n), "{n}");
        }
        assert!(is_recommended("Microsoft.BingNews"));
        assert!(!is_recommended("Microsoft.WindowsStore"));
    }

    #[test]
    fn both_teams_generations_are_listed() {
        assert!(OPTIONAL_APPS.contains(&"Microsoft.Teams"));
        assert!(OPTIONAL_APPS.contains(&"MSTeams"));
    }

    #[test]
    fn restore_script_reports_each_failure_and_recreates_keys() {
        let state = serde_json::json!([
            { "kind": "reg", "root": "HKLM", "path": "SOFTWARE\\Policies\\X", "name": "A", "value": 0, "isNumber": true },
            { "kind": "reg", "root": "HKCU", "path": "Software\\Y", "name": "B", "value": null, "isNumber": false },
            { "kind": "service", "name": "WerSvc", "startType": "Manual", "status": "Stopped" }
        ]);
        let s = restore_script(&state);
        assert!(s.starts_with("$e = @();"));
        assert!(s.contains("New-Item 'HKLM:\\SOFTWARE\\Policies\\X' -Force"));
        assert!(s.contains(
            "Set-ItemProperty 'HKLM:\\SOFTWARE\\Policies\\X' 'A' ([int](0)) -Type DWord"
        ));
        assert!(s.contains("Remove-ItemProperty 'HKCU:\\Software\\Y' 'B'"));
        assert!(s.contains("Set-Service -Name 'WerSvc' -StartupType Manual"));
        assert_eq!(s.matches("try {").count(), 3, "one try per step");
        assert!(s.contains("[Console]::Error.WriteLine"));
        assert_eq!(restore_script(&serde_json::json!([])), "");
    }

    /// Regression (Cortana reset "exit code 1"): a state with one captured
    /// value was stored as a bare object, restore_script ignored it and the
    /// guessed `undo` ran instead.
    #[test]
    fn single_item_state_restores_exactly() {
        let state = serde_json::json!({
            "kind": "reg", "root": "HKLM",
            "path": "SOFTWARE\\Policies\\Microsoft\\Windows\\Windows Search",
            "name": "AllowCortana", "value": null, "isNumber": false
        });
        let s = restore_script(&state);
        assert!(
            s.contains("Remove-ItemProperty 'HKLM:\\SOFTWARE\\Policies\\Microsoft\\Windows\\Windows Search' 'AllowCortana'"),
            "{s}"
        );
        assert!(state_is_valid("cortana_off", &state));
        let svc = serde_json::json!({ "kind": "service", "name": "WerSvc", "startType": "Manual", "status": "Stopped" });
        assert!(restore_script(&svc).contains("Set-Service -Name 'WerSvc' -StartupType Manual"));
        assert!(state_is_valid("error_reporting_off", &svc));
        assert!(!state_is_valid("cortana_off", &serde_json::json!("x")));
    }

    /// Only parses the generated script (nothing is executed).
    #[cfg(windows)]
    #[test]
    fn restore_script_parses_in_powershell() {
        let state = serde_json::json!([
            { "kind": "reg", "root": "HKLM", "path": "SOFTWARE\\Policies\\X", "name": "A", "value": 0, "isNumber": true },
            { "kind": "reg", "root": "HKCU", "path": "Software\\Y", "name": "B", "value": "text", "isNumber": false },
            { "kind": "reg", "root": "HKCU", "path": "Software\\Y", "name": "C", "value": null, "isNumber": false },
            { "kind": "service", "name": "WerSvc", "startType": "Manual", "status": "Running" }
        ]);
        let script = restore_script(&state);
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

    #[test]
    fn restore_script_never_embeds_a_non_integer_as_number() {
        let state = serde_json::json!([
            { "kind": "reg", "root": "HKCU", "path": "Software\\Y", "name": "B", "value": "$(calc)", "isNumber": true },
            { "kind": "reg", "root": "HKCU", "path": "Software\\Y'; calc; '", "name": "C", "value": "x", "isNumber": false }
        ]);
        let s = restore_script(&state);
        assert!(!s.contains("([int]("), "{s}");
        assert!(s.contains("captured value is not an integer"));
        // Quotes in a path are doubled, so they cannot end the literal.
        assert!(s.contains("Software\\Y''; calc; ''"), "{s}");
    }

    #[test]
    fn withdrawn_tweaks_exist_and_point_to_a_live_single_source() {
        for w in WITHDRAWN {
            assert!(
                TWEAKS.iter().any(|t| t.id == w.id),
                "{} not in catalog",
                w.id
            );
            assert!(!w.reason.is_empty());
            if let Some((page, target)) = w.moved_to {
                match page {
                    "privacy" => assert!(
                        crate::privacy::is_offered(target),
                        "{}: {target} is not offered in the Privacy Center",
                        w.id
                    ),
                    "optimize" => {
                        assert!(crate::optregistry::meta(target).is_some(), "{target}");
                        assert!(
                            !crate::optregistry::is_retired(target),
                            "{target} is retired"
                        );
                    }
                    other => panic!("unknown page {other}"),
                }
            }
        }
        for (id, page, target) in RELATED {
            assert!(TWEAKS.iter().any(|t| t.id == *id), "{id}");
            assert!(withdrawn(id).is_none(), "{id} is withdrawn and related");
            assert_eq!(*page, "privacy");
            assert!(crate::privacy::is_offered(target), "{target}");
        }
    }

    #[test]
    fn requested_cleanup_is_withdrawn() {
        for id in [
            "cortana_off",
            "taskbar_widgets_off",
            "gamedvr_off",
            "telemetry_off",
            "advertising_id",
            "app_suggestions",
        ] {
            assert!(withdrawn(id).is_some(), "{id}");
        }
        // Still offered: no equivalent elsewhere.
        for id in [
            "location_off",
            "clipboard_sync_off",
            "bing_search",
            "xbox_gamebar",
        ] {
            assert!(withdrawn(id).is_none(), "{id}");
        }
    }

    #[test]
    fn withdrawn_tweaks_are_listed_only_with_a_captured_state() {
        assert!(!listed("cortana_off", false));
        assert!(listed("cortana_off", true));
        assert!(!listed("telemetry_off", false));
        assert!(listed("location_off", false));
        assert!(listed("location_off", true));
    }

    #[test]
    fn withdrawn_tweaks_cannot_be_applied() {
        for w in WITHDRAWN {
            let err = apply_tweak(w.id.to_string()).unwrap_err();
            assert!(err.contains("no longer offered"), "{}: {err}", w.id);
        }
    }

    #[test]
    fn gamedvr_reset_restores_the_fse_values_it_changed() {
        // Old applies also wrote the FSE values; their captured originals must
        // stay restorable (and accepted on state migration).
        let t = TWEAKS.iter().find(|t| t.id == "gamedvr_off").unwrap();
        for name in [
            "GameDVR_FSEBehaviorMode",
            "GameDVR_HonorUserFSEBehaviorMode",
        ] {
            assert!(t.capture.iter().any(|(_, _, n)| *n == name), "{name}");
        }
    }
}
