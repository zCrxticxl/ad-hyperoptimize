//! Windows Debloater — remove UWP bloatware + disable telemetry/ads/Cortana.

use crate::ps;
use serde_json::{json, Value};

// ── UWP bloatware list ────────────────────────────────────────────────────────

pub fn list_uwp() -> Value {
    let script = r#"
$keep = @(
    'Microsoft.WindowsStore','Microsoft.Windows.Photos','Microsoft.WindowsCalculator',
    'Microsoft.Windows.SoundRecorder','Microsoft.WindowsNotepad',
    'Microsoft.MicrosoftEdge','Microsoft.MicrosoftEdge.Stable',
    'Microsoft.Paint','Microsoft.ScreenSketch','Microsoft.WindowsTerminal',
    'Microsoft.Windows.Narrator','Microsoft.Windows.Magnifier',
    'Microsoft.WindowsAlarms','Microsoft.WindowsCamera',
    'Microsoft.Windows.Search','Microsoft.WindowsSecurity',
    'Microsoft.WindowsStore','Microsoft.StorePurchaseApp',
    'Microsoft.Xbox.TCUI'
)
$apps = Get-AppxPackage -AllUsers -ErrorAction SilentlyContinue |
    Where-Object { $_.IsFramework -eq $false -and $_.SignatureKind -ne 'System' } |
    Where-Object { $_.Name -notin $keep } |
    Select-Object Name, PackageFullName, Publisher, PackageFamilyName,
        @{n='sizeMb';e={0}}, @{n='removable';e={$true}} |
    Sort-Object Name
$apps | ConvertTo-Json -Compress -Depth 3
"#;
    match ps::run_json(script) {
        Ok(Value::Array(arr)) => json!({ "apps": arr }),
        Ok(v @ Value::Object(_)) => json!({ "apps": [v] }),
        _ => json!({ "apps": [] }),
    }
}

pub fn remove_uwp(package_full_name: String) -> Result<String, String> {
    // Sanitize — only allow package name characters
    if package_full_name.contains('\'') || package_full_name.contains('"') {
        return Err("Invalid package name".into());
    }
    let script = format!(
        r#"
try {{
    Remove-AppxPackage -Package '{package_full_name}' -AllUsers -ErrorAction Stop 2>&1
    "Removed"
}} catch {{
    try {{
        Remove-AppxPackage -Package '{package_full_name}' -ErrorAction Stop 2>&1
        "Removed (current user)"
    }} catch {{
        throw $_
    }}
}}
"#
    );
    ps::run(&script).map(|s| s.trim().to_string())
}

pub fn remove_uwp_provisioned(package_name: String) -> Result<String, String> {
    if package_name.contains('\'') { return Err("Invalid name".into()); }
    let script = format!(
        r#"
$pkg = Get-AppxProvisionedPackage -Online -ErrorAction SilentlyContinue |
    Where-Object {{ $_.DisplayName -eq '{package_name}' }} | Select-Object -First 1
if ($pkg) {{
    Remove-AppxProvisionedPackage -Online -PackageName $pkg.PackageName -ErrorAction Stop | Out-Null
    "Removed provisioned package"
}} else {{ "Not found as provisioned" }}
"#
    );
    ps::run(&script).map(|s| s.trim().to_string())
}

// ── System tweaks ─────────────────────────────────────────────────────────────

struct Tweak {
    id:    &'static str,
    name:  &'static str,
    desc:  &'static str,
    cat:   &'static str,
    // PowerShell to check current state → should output "1" if tweak is applied
    check: &'static str,
    apply: &'static str,
    undo:  &'static str,
}

static TWEAKS: &[Tweak] = &[
    Tweak {
        id: "telemetry_off",
        name: "Disable Telemetry",
        desc: "Sets AllowTelemetry=0 (Security level). Stops diagnostic data upload.",
        cat: "Telemetry",
        check: r#"(Get-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' -Name AllowTelemetry -EA SilentlyContinue).AllowTelemetry -eq 0"#,
        apply: r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection'; if(!(Test-Path $p)){New-Item $p -Force -EA Stop|Out-Null}; Set-ItemProperty $p AllowTelemetry 0 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"Remove-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' AllowTelemetry -EA SilentlyContinue"#,
    },
    Tweak {
        id: "diagtrack_stop",
        name: "Stop Connected User Experiences (DiagTrack)",
        desc: "Stops and disables the telemetry service that sends data to Microsoft.",
        cat: "Telemetry",
        check: r#"(Get-Service DiagTrack -EA SilentlyContinue).StartType -eq 'Disabled'"#,
        apply: r#"Stop-Service DiagTrack -Force -EA SilentlyContinue; Set-Service DiagTrack -StartupType Disabled -EA Stop; 'Applied'"#,
        undo:  r#"Set-Service DiagTrack -StartupType Automatic; Start-Service DiagTrack -EA SilentlyContinue"#,
    },
    Tweak {
        id: "activity_history",
        name: "Disable Activity History",
        desc: "Stops Windows from storing and uploading your activity timeline.",
        cat: "Telemetry",
        check: r#"(Get-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name PublishUserActivities -EA SilentlyContinue).PublishUserActivities -eq 0"#,
        apply: r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\System'; if(!(Test-Path $p)){New-Item $p -Force -EA Stop|Out-Null}; Set-ItemProperty $p PublishUserActivities 0 -Type DWord -EA Stop; Set-ItemProperty $p EnableActivityFeed 0 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\System'; Remove-ItemProperty $p PublishUserActivities -EA SilentlyContinue; Remove-ItemProperty $p EnableActivityFeed -EA SilentlyContinue"#,
    },
    Tweak {
        id: "advertising_id",
        name: "Disable Advertising ID",
        desc: "Stops apps from using your advertising ID for targeted ads.",
        cat: "Privacy",
        check: r#"(Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo' -Name Enabled -EA SilentlyContinue).Enabled -eq 0"#,
        apply: r#"$p='HKCU:\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo'; if(!(Test-Path $p)){New-Item $p -Force -EA Stop|Out-Null}; Set-ItemProperty $p Enabled 0 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo' Enabled 1 -Type DWord -EA SilentlyContinue"#,
    },
    Tweak {
        id: "bing_search",
        name: "Disable Bing in Start Menu",
        desc: "Removes web/Bing results from Windows Search. Faster, private.",
        cat: "Privacy",
        check: r#"(Get-ItemProperty 'HKCU:\Software\Policies\Microsoft\Windows\Explorer' -Name DisableSearchBoxSuggestions -EA SilentlyContinue).DisableSearchBoxSuggestions -eq 1"#,
        apply: r#"$p='HKCU:\Software\Policies\Microsoft\Windows\Explorer'; if(!(Test-Path $p)){New-Item $p -Force -EA Stop|Out-Null}; Set-ItemProperty $p DisableSearchBoxSuggestions 1 -Type DWord -EA Stop; Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' BingSearchEnabled 0 -Type DWord -EA SilentlyContinue; 'Applied'"#,
        undo:  r#"Remove-ItemProperty 'HKCU:\Software\Policies\Microsoft\Windows\Explorer' DisableSearchBoxSuggestions -EA SilentlyContinue; Remove-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' BingSearchEnabled -EA SilentlyContinue"#,
    },
    Tweak {
        id: "cortana_off",
        name: "Disable Cortana",
        desc: "Prevents Cortana from running. Also reduces Search resource usage.",
        cat: "Privacy",
        check: r#"(Get-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name AllowCortana -EA SilentlyContinue).AllowCortana -eq 0"#,
        apply: r#"$p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search'; if(!(Test-Path $p)){New-Item $p -Force -EA Stop|Out-Null}; Set-ItemProperty $p AllowCortana 0 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"Remove-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' AllowCortana -EA SilentlyContinue"#,
    },
    Tweak {
        id: "app_suggestions",
        name: "Disable App Suggestions / Tips",
        desc: "Removes suggested apps in Start and 'Did you know?' tips.",
        cat: "Privacy",
        check: r#"(Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name SubscribedContent-338389Enabled -EA SilentlyContinue).'SubscribedContent-338389Enabled' -eq 0"#,
        apply: r#"$p='HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'; @('SubscribedContent-338389Enabled','SubscribedContent-338388Enabled','SubscribedContent-353698Enabled','SystemPaneSuggestionsEnabled','SoftLandingEnabled') | ForEach-Object { Set-ItemProperty $p $_ 0 -Type DWord -EA Stop }; 'Applied'"#,
        undo:  r#"$p='HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'; @('SubscribedContent-338389Enabled','SubscribedContent-338388Enabled','SubscribedContent-353698Enabled','SystemPaneSuggestionsEnabled','SoftLandingEnabled') | ForEach-Object { Set-ItemProperty $p $_ 1 -Type DWord -EA SilentlyContinue }"#,
    },
    Tweak {
        id: "xbox_gamebar",
        name: "Disable Xbox Game Bar",
        desc: "Disables Win+G overlay. Reduces background CPU/GPU usage in games.",
        cat: "Gaming",
        check: r#"(Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\GameDVR' -Name AppCaptureEnabled -EA SilentlyContinue).AppCaptureEnabled -eq 0"#,
        apply: r#"Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\GameDVR' AppCaptureEnabled 0 -Type DWord -EA Stop; $p='HKLM:\SOFTWARE\Policies\Microsoft\Windows\GameDVR'; if(!(Test-Path $p)){New-Item $p -Force -EA Stop|Out-Null}; Set-ItemProperty $p AllowGameDVR 0 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\GameDVR' AppCaptureEnabled 1 -Type DWord -EA SilentlyContinue; Remove-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\GameDVR' AllowGameDVR -EA SilentlyContinue"#,
    },
    Tweak {
        id: "location_off",
        name: "Disable Location Services",
        desc: "Prevents apps from accessing your location via Windows Location API.",
        cat: "Privacy",
        check: r#"(Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\location' -Name Value -EA SilentlyContinue).Value -eq 'Deny'"#,
        apply: r#"Set-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\location' Value 'Deny' -EA Stop; 'Applied'"#,
        undo:  r#"Set-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\location' Value 'Allow' -EA SilentlyContinue"#,
    },
    Tweak {
        id: "feedback_off",
        name: "Disable Feedback Requests",
        desc: "Stops Windows from asking for feedback periodically.",
        cat: "Telemetry",
        check: r#"(Get-ItemProperty 'HKCU:\Software\Microsoft\Siuf\Rules' -Name NumberOfSIUFInPeriod -EA SilentlyContinue).NumberOfSIUFInPeriod -eq 0"#,
        apply: r#"$p='HKCU:\Software\Microsoft\Siuf\Rules'; if(!(Test-Path $p)){New-Item $p -Force -EA Stop|Out-Null}; Set-ItemProperty $p NumberOfSIUFInPeriod 0 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"Remove-ItemProperty 'HKCU:\Software\Microsoft\Siuf\Rules' NumberOfSIUFInPeriod -EA SilentlyContinue"#,
    },
    Tweak {
        id: "error_reporting_off",
        name: "Disable Windows Error Reporting",
        desc: "Stops crash dumps from being sent to Microsoft.",
        cat: "Telemetry",
        check: r#"(Get-Service WerSvc -EA SilentlyContinue).StartType -eq 'Disabled'"#,
        apply: r#"Stop-Service WerSvc -Force -EA SilentlyContinue; Set-Service WerSvc -StartupType Disabled -EA Stop; 'Applied'"#,
        undo:  r#"Set-Service WerSvc -StartupType Manual; Start-Service WerSvc -EA SilentlyContinue"#,
    },
    Tweak {
        id: "lock_screen_ads",
        name: "Disable Lock Screen Ads / Spotlight",
        desc: "Prevents Windows Spotlight from replacing your lock screen with ads.",
        cat: "Privacy",
        check: r#"(Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name RotatingLockScreenEnabled -EA SilentlyContinue).RotatingLockScreenEnabled -eq 0"#,
        apply: r#"$p='HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'; Set-ItemProperty $p RotatingLockScreenEnabled 0 -Type DWord -EA Stop; Set-ItemProperty $p RotatingLockScreenOverlayEnabled 0 -Type DWord -EA Stop; 'Applied'"#,
        undo:  r#"$p='HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'; Set-ItemProperty $p RotatingLockScreenEnabled 1 -Type DWord -EA SilentlyContinue; Set-ItemProperty $p RotatingLockScreenOverlayEnabled 1 -Type DWord -EA SilentlyContinue"#,
    },
];

pub fn list_tweaks() -> Value {
    // Check all tweak states in a single PS call
    let checks: Vec<String> = TWEAKS
        .iter()
        .map(|t| format!("try{{if({}){{1}}else{{0}}}}catch{{0}}", t.check))
        .collect();
    let script = format!(
        "@({}) | ConvertTo-Json -Compress",
        checks.join(",")
    );

    let states: Vec<bool> = match ps::run_json(&script) {
        Ok(Value::Array(arr)) => arr
            .iter()
            .map(|v| v.as_i64().unwrap_or(0) == 1)
            .collect(),
        Ok(v) => vec![v.as_i64().unwrap_or(0) == 1],
        _ => vec![false; TWEAKS.len()],
    };

    let tweaks: Vec<Value> = TWEAKS
        .iter()
        .enumerate()
        .map(|(i, t)| json!({
            "id":      t.id,
            "name":    t.name,
            "desc":    t.desc,
            "cat":     t.cat,
            "applied": states.get(i).copied().unwrap_or(false),
        }))
        .collect();

    json!({ "tweaks": tweaks })
}

pub fn apply_tweak(id: String) -> Result<String, String> {
    let t = TWEAKS.iter().find(|t| t.id == id)
        .ok_or_else(|| format!("Unknown tweak: {id}"))?;
    ps::run(t.apply).map(|_| format!("Applied: {}", t.name))
}

pub fn revert_tweak(id: String) -> Result<String, String> {
    let t = TWEAKS.iter().find(|t| t.id == id)
        .ok_or_else(|| format!("Unknown tweak: {id}"))?;
    ps::run(t.undo).map(|_| format!("Reverted: {}", t.name))
}
