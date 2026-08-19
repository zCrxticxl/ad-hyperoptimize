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
    if package_name.contains('\'') {
        return Err("Invalid name".into());
    }
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
    id: &'static str,
    name: &'static str,
    desc: &'static str,
    cat: &'static str,
    // PowerShell to check current state → should output "1" if tweak is applied
    check: &'static str,
    apply: &'static str,
    // Fallback undo with documented defaults — only used when no captured
    // state exists (e.g. a revert without a matching apply in this install).
    undo: &'static str,
    /// Registry values written by `apply` (root, path, name). Captured before
    /// applying so the undo restores the exact previous value — or removes
    /// the value when it did not exist before (never guesses a default).
    capture: &'static [(&'static str, &'static str, &'static str)],
    /// Service controlled by `apply` — start type and running state are
    /// captured before applying and restored exactly on undo.
    service: Option<&'static str>,
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
        check: r#"(Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name SubscribedContent-338389Enabled -EA SilentlyContinue).'SubscribedContent-338389Enabled' -eq 0"#,
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

    let tweaks: Vec<Value> = TWEAKS
        .iter()
        .enumerate()
        .map(|(i, t)| {
            json!({
                "id":      t.id,
                "name":    t.name,
                "desc":    t.desc,
                "cat":     t.cat,
                "applied": states.get(i).copied().unwrap_or(false),
            })
        })
        .collect();

    json!({ "tweaks": tweaks })
}

// ── Exact undo: capture pre-apply state, restore it on revert ─────────────────

fn state_path(id: &str) -> std::path::PathBuf {
    crate::safety::app_data_dir()
        .join("debloater")
        .join(format!("{id}.json"))
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
$out | ConvertTo-Json -Compress -Depth 4
"#
    );
    ps::run_json(&script)
}

/// Build the PowerShell that restores a captured state exactly: registry
/// values back to their previous value (or removed if they were absent),
/// service back to its previous start type and running state.
fn restore_script(state: &serde_json::Value) -> String {
    let mut lines = Vec::new();
    if let Some(arr) = state.as_array() {
        for it in arr {
            match it["kind"].as_str() {
                Some("reg") => {
                    let root = it["root"].as_str().unwrap_or("HKCU");
                    let path = it["path"].as_str().unwrap_or("");
                    let name = it["name"].as_str().unwrap_or("");
                    if it["value"].is_null() {
                        lines.push(format!(
                            "Remove-ItemProperty '{root}:\\{path}' '{name}' -EA SilentlyContinue"
                        ));
                    } else if it["isNumber"].as_bool().unwrap_or(false) {
                        lines.push(format!(
                            "Set-ItemProperty '{root}:\\{path}' '{name}' ([int]({v})) -Type DWord -EA Stop",
                            v = it["value"]
                        ));
                    } else {
                        // Escape single quotes so a captured value can never
                        // terminate the quoted PS string.
                        let v = it["value"].as_str().unwrap_or("").replace('\'', "''");
                        lines.push(format!(
                            "Set-ItemProperty '{root}:\\{path}' '{name}' '{v}' -EA Stop"
                        ));
                    }
                }
                Some("service") => {
                    let svc = it["name"].as_str().unwrap_or("");
                    let start = it["startType"].as_str().unwrap_or("Manual");
                    lines.push(format!(
                        "Set-Service -Name '{svc}' -StartupType {start} -EA Stop"
                    ));
                    match it["status"].as_str() {
                        Some("Running") => {
                            lines.push(format!("Start-Service -Name '{svc}' -EA SilentlyContinue"))
                        }
                        Some("Stopped") => lines.push(format!(
                            "Stop-Service -Name '{svc}' -Force -EA SilentlyContinue"
                        )),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
    lines.join("; ")
}

pub fn apply_tweak(id: String) -> Result<String, String> {
    let t = TWEAKS
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("Unknown tweak: {id}"))?;
    // Capture before applying — the undo restores exactly this.
    let state = capture_state(t)?;
    ps::run(t.apply)?;
    let dir = state_path(id.as_str()).parent().unwrap().to_path_buf();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(
        state_path(id.as_str()),
        serde_json::to_string(&state).unwrap_or_default(),
    )
    .map_err(|e| format!("saving tweak state failed: {e}"))?;
    Ok(format!("Applied: {}", t.name))
}

pub fn revert_tweak(id: String) -> Result<String, String> {
    let t = TWEAKS
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("Unknown tweak: {id}"))?;
    let path = state_path(id.as_str());
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
                    // succeeded — deleting it on failure would silently
                    // downgrade a retry to the guessed-default static undo.
                    let _ = std::fs::remove_file(&path);
                    Ok(format!("Reverted: {}", t.name))
                }
                Err(e) => Err(format!(
                    "revert failed: {e} (captured state kept for retry)"
                )),
            }
        }
        // No captured state (apply happened outside this app or the state was
        // lost) — fall back to the documented-default undo.
        None => ps::run(t.undo).map(|_| format!("Reverted: {}", t.name)),
    }
}
