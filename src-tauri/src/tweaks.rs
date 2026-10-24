//! Safe optimization engine. Declarative tweak catalog; every tweak knows how
//! to describe itself, detect its current state, apply with write-ahead
//! journaling + registry backups, and revert exactly.
//!
//! Catalog policy: only measurable, documented, reversible tweaks. No
//! "registry cleaning", no myths.

use crate::safety::{self, ChangeItem, JournalEntry, RegVal};
use serde::Serialize;
use serde_json::{json, Value};

#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE};
#[cfg(windows)]
use winreg::RegKey;

#[derive(Clone, Serialize)]
pub enum Action {
    RegSet {
        root: &'static str,
        path: &'static str,
        name: &'static str,
        value: RegVal,
    },
    Service {
        name: &'static str,
        target: &'static str,
    },
    Cmd {
        apply: &'static str,
        revert: &'static str,
        /// When set, the *pre-apply* state is captured and the revert command
        /// is built from it at apply time (exact undo instead of a hardcoded
        /// default). When the capture fails the apply is refused; `revert`
        /// is only the force-revert default for uncaptured commands.
        show: Option<ShowCapture>,
    },
}

/// How a `Cmd` action captures its pre-apply state to build an exact revert.
#[derive(Clone, Serialize)]
pub enum ShowCapture {
    /// `powercfg /getactivescheme`, revert reactivates the captured scheme.
    PowerScheme,
    /// `Get-NetTCPSetting AutoTuningLevelLocal`, revert maps the enum value
    /// (English on every locale) back to the netsh token.
    TcpAutotuning,
    /// Registry `HibernateEnabled`, revert restores hibernation exactly.
    HibernateState,
    /// Registry `Tcpip\Parameters\EnableRSS` (what `netsh rss=` writes),
    /// locale-independent unlike parsing `netsh int tcp show global` output.
    RssEnabled,
    /// `powercfg /query` current AC/DC values for one setting; the query
    /// labels are localized but the `0x…` index values are not.
    PowerSettingIndex {
        subgroup: &'static str,
        setting: &'static str,
    },
    /// Snapshots the per-interface TcpAckFrequency/TCPNoDelay state before
    /// mutation; the generated revert restores exactly the captured values
    /// and never touches adapters added afterwards.
    NagleInterfaces,
}

/// Extract the active power scheme GUID from `powercfg /getactivescheme`
/// output. The header text is localized; the GUID itself is not.
#[cfg(any(windows, test))]
fn parse_power_scheme(out: &str) -> Option<String> {
    out.split_whitespace()
        .find(|t| crate::ps::is_guid(t))
        .map(|g| format!("powercfg /setactive {g}"))
}

/// Map `Get-NetTCPSetting` AutoTuningLevelLocal enum output (English on every
/// locale) to the corresponding netsh token.
#[cfg(any(windows, test))]
fn parse_tcp_autotuning(out: &str) -> Option<String> {
    let v = out.trim().to_ascii_lowercase();
    match v.as_str() {
        "disabled" | "highlyrestricted" | "restricted" | "normal" | "experimental" => {
            Some(format!("netsh int tcp set global autotuninglevel={v}"))
        }
        _ => None,
    }
}

/// Extract the current AC/DC index pair from `powercfg /query <scheme>
/// <subgroup> <setting>` output. Only `0x`-prefixed 8-digit hex tokens are
/// considered (labels are localized), and the LAST pair wins: some settings
/// print Minimum/Maximum/Increment rows before the current AC/DC values.
#[cfg(any(windows, test))]
fn parse_power_indexes(out: &str) -> Option<(u32, u32)> {
    let hexes: Vec<u32> = out
        .lines()
        .filter_map(|l| {
            l.split_whitespace()
                .find(|t| t.starts_with("0x") && t.len() == 10)
        })
        .map(|t| u32::from_str_radix(&t[2..], 16).ok())
        .collect::<Option<Vec<_>>>()?;
    match hexes.as_slice() {
        [.., ac, dc] => Some((*ac, *dc)),
        _ => None,
    }
}

/// Pull the raw GUID out of `powercfg /getactivescheme` output.
#[cfg(any(windows, test))]
fn parse_active_scheme_guid(out: &str) -> Option<String> {
    out.split_whitespace()
        .find(|t| crate::ps::is_guid(t))
        .map(|s| s.to_string())
}

/// Enumerates the per-interface Nagle state as `GUID|ack|nodelay` lines
/// ("A" marks an absent value). Locale-independent: only the key GUID and
/// numeric values are read.
#[cfg(any(windows, test))]
const NAGLE_SNAPSHOT_PS: &str = r#"
Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces' | ForEach-Object {
  $p = Get-ItemProperty -Path $_.PSPath
  $ack = if ($p.PSObject.Properties['TcpAckFrequency']) { $p.TcpAckFrequency } else { 'A' }
  $nd  = if ($p.PSObject.Properties['TCPNoDelay'])    { $p.TCPNoDelay }    else { 'A' }
  "$($_.PSChildName)|$ack|$nd"
}"#;

/// Builds a revert script from a NAGLE_SNAPSHOT_PS capture. Every step is
/// baked from the captured state: pre-existing values are restored verbatim,
/// absent values get a conditional delete (only removed if still holding the
/// value we wrote), and any step failure fails the whole undo.
#[cfg(any(windows, test))]
fn build_nagle_revert(out: &str) -> Option<String> {
    let mut steps: Vec<String> = Vec::new();
    for line in out.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let mut parts = line.split('|');
        let key = parts.next()?;
        let ack = parts.next()?;
        let nd = parts.next()?;
        // Interface subkeys are named `{GUID}` (with braces).
        let bare = key.strip_prefix('{').and_then(|k| k.strip_suffix('}'));
        if parts.next().is_some() || !bare.is_some_and(crate::ps::is_guid) {
            return None;
        }
        let path = format!(
            "HKLM:\\SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters\\Interfaces\\{key}"
        );
        for (name, captured) in [("TcpAckFrequency", ack), ("TCPNoDelay", nd)] {
            if captured == "A" {
                steps.push(format!(
                    "$v = (Get-ItemProperty -Path '{path}' -Name '{name}' -EA SilentlyContinue).'{name}'; \
                     if ($v -eq 1) {{ Remove-ItemProperty -Path '{path}' -Name '{name}' -EA Stop }} else {{ if ($null -ne $v) {{ exit 1 }} }}"
                ));
            } else if captured != "1" {
                // Only a plain DWORD may be baked into the script text.
                captured.parse::<u32>().ok()?;
                steps.push(format!(
                    "Set-ItemProperty -Path '{path}' -Name '{name}' -Value {captured} -Type DWord -EA Stop"
                ));
            }
        }
    }
    if steps.is_empty() {
        return Some("powershell -Command exit 0".into());
    }
    // No outer quotes: run_cmdline hands everything after ` -Command ` to
    // PowerShell as the script, and a quoted script is just a string literal
    // that PowerShell echoes instead of executing.
    Some(format!("powershell -Command {}", steps.join("; ")))
}

/// Exact revert for a captured power-setting index. One PowerShell wrapper so
/// run_cmdline executes all three steps in order and aborts on the first
/// failure; the final setactive makes the restored indexes effective
/// immediately. Also used by statestore to recognise migrated entries.
pub(crate) fn power_index_revert(
    scheme: &str,
    subgroup: &str,
    setting: &str,
    ac: &str,
    dc: &str,
) -> String {
    format!(
        "powershell -Command powercfg /setacvalueindex {scheme} {subgroup} {setting} {ac}; \
         if ($LASTEXITCODE -eq 0) {{ powercfg /setdcvalueindex {scheme} {subgroup} {setting} {dc}; \
         if ($LASTEXITCODE -eq 0) {{ powercfg /setactive scheme_current }} }}; \
         exit $LASTEXITCODE"
    )
}

/// The catalog's fixed revert commands for a tweak (used to recognise
/// migrated journal entries as app-generated).
pub(crate) fn static_cmd_reverts(tweak_id: &str) -> Vec<&'static str> {
    catalog()
        .into_iter()
        .filter(|t| t.id == tweak_id)
        .flat_map(|t| t.actions)
        .filter_map(|a| match a {
            Action::Cmd { revert, .. } => Some(revert),
            _ => None,
        })
        .collect()
}

/// Registry values the catalog entry `tweak_id` writes (root, path, name).
pub(crate) fn declared_registry(tweak_id: &str) -> Vec<(&'static str, &'static str, &'static str)> {
    catalog()
        .into_iter()
        .filter(|t| t.id == tweak_id)
        .flat_map(|t| t.actions)
        .filter_map(|a| match a {
            Action::RegSet {
                root, path, name, ..
            } => Some((root, path, name)),
            _ => None,
        })
        .collect()
}

/// Services whose startup type the catalog entry `tweak_id` changes.
pub(crate) fn declared_services(tweak_id: &str) -> Vec<&'static str> {
    catalog()
        .into_iter()
        .filter(|t| t.id == tweak_id)
        .flat_map(|t| t.actions)
        .filter_map(|a| match a {
            Action::Service { name, .. } => Some(name),
            _ => None,
        })
        .collect()
}

/// Undo never runs any step of an untrusted (migrated, unrecognised) entry:
/// its registry targets, services and commands all came from a folder any
/// unelevated process could write, and the undo runs elevated.
fn refuse_untrusted(entry: &JournalEntry, upto: usize) -> Result<(), String> {
    if !entry.untrusted || upto == 0 {
        return Ok(());
    }
    let steps: Vec<String> = entry.items[..upto]
        .iter()
        .map(|i| match i {
            ChangeItem::Registry {
                root,
                path,
                name,
                prev,
                ..
            } => match prev {
                Some(v) => format!("set {root}\\{path}\\{name} = {v:?}"),
                None => format!("delete {root}\\{path}\\{name}"),
            },
            ChangeItem::ServiceStartup { service, prev, .. } => {
                format!("service {service} startup = {prev}")
            }
            ChangeItem::Command { revert, .. } => revert.clone(),
        })
        .collect();
    Err(format!(
        "This undo entry was recorded in the old, user-writable data folder and could not be \
         verified as one this app generates, so it is not run automatically. Review it and \
         undo it manually only if it is what you expect: {}",
        steps.join(" | ")
    ))
}

/// Run a capture and build the exact revert command for it.
#[cfg(windows)]
fn capture_revert(cap: &ShowCapture) -> Option<String> {
    match cap {
        ShowCapture::PowerScheme => {
            // ps::exec returns the captured output; run_cmdline discards it.
            let out = crate::ps::exec("powercfg.exe", &["/getactivescheme"]).ok()?;
            parse_power_scheme(&out)
        }
        ShowCapture::TcpAutotuning => {
            // Get-NetTCPSetting is a PowerShell cmdlet, not an executable -
            // it must run through ps::run (run_cmdline would try to spawn it
            // directly and fail).
            let out = crate::ps::run(
                "Get-NetTCPSetting -SettingName Internet | Select-Object -ExpandProperty AutoTuningLevelLocal",
            )
            .ok()?;
            parse_tcp_autotuning(out.as_str())
        }
        ShowCapture::HibernateState => {
            let reg = reg_read(
                "HKLM",
                "SYSTEM\\CurrentControlSet\\Control\\Power",
                "HibernateEnabled",
            )
            .ok()?;
            let drive = crate::ps::windows_dir()
                .get(..2)
                .unwrap_or("C:")
                .to_string();
            let file = std::path::Path::new(&format!("{drive}\\")).join("hiberfil.sys");
            hibernate_revert(reg.as_ref(), std::fs::symlink_metadata(file).is_ok())
        }
        ShowCapture::RssEnabled => {
            let prev = reg_read(
                "HKLM",
                "SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters",
                "EnableRSS",
            )
            .ok()
            .flatten()?;
            match prev {
                RegVal::Dword(1) => Some("netsh int tcp set global rss=enabled".into()),
                RegVal::Dword(0) => Some("netsh int tcp set global rss=disabled".into()),
                _ => None,
            }
        }
        ShowCapture::PowerSettingIndex { subgroup, setting } => {
            // Bind the restore to the scheme that was active at capture time:
            // writing back via `scheme_current` after the user switched plans
            // would corrupt the other scheme's independent values.
            let scheme_out = crate::ps::exec("powercfg.exe", &["/getactivescheme"]).ok()?;
            let guid = parse_active_scheme_guid(&scheme_out)?;
            let query_out =
                crate::ps::exec("powercfg.exe", &["/query", &guid, subgroup, setting]).ok()?;
            parse_power_indexes(&query_out).map(|(ac, dc)| {
                power_index_revert(&guid, subgroup, setting, &ac.to_string(), &dc.to_string())
            })
        }
        ShowCapture::NagleInterfaces => {
            let out = crate::ps::run(NAGLE_SNAPSHOT_PS).ok()?;
            build_nagle_revert(&out)
        }
    }
}

#[derive(Clone, Serialize)]
pub struct Tweak {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub rationale: &'static str,
    pub impact: &'static str,
    pub risk: &'static str, // Low | Medium | High (High = advanced / situational)
    pub requires_admin: bool,
    pub reversible: bool,
    pub actions: Vec<Action>,
}

pub fn catalog() -> Vec<Tweak> {
    vec![
        Tweak {
            id: "power_high_performance",
            name: "High Performance power plan",
            category: "Power",
            description: "Switches the active power plan to High Performance.",
            rationale: "Balanced lowers clocks at idle; High Performance keeps them higher, which can reduce ramp-up latency in games and interactive work. Not measured by this app.",
            impact: "May lower input/scheduling latency; higher idle power draw (relevant on laptops).",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![Action::Cmd {
                apply: "powercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c",
                revert: "powercfg /setactive 381b4222-f694-41f0-9685-ff5bb260df2e",
                show: Some(ShowCapture::PowerScheme),
            }],
        },
        Tweak {
            id: "game_dvr_off",
            name: "Disable Game DVR / background recording",
            category: "Gaming",
            description: "Turns off Xbox Game DVR background capture.",
            rationale: "Background capture continuously encodes frames, which costs GPU/CPU time and can introduce frametime spikes.",
            impact: "May steady frametimes on weaker GPUs; not measured by this app.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKCU", path: "System\\GameConfigStore", name: "GameDVR_Enabled", value: RegVal::Dword(0) },
                Action::RegSet { root: "HKCU", path: "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\GameDVR", name: "AppCaptureEnabled", value: RegVal::Dword(0) },
            ],
        },
        Tweak {
            id: "mmcss_gaming",
            name: "MMCSS gaming scheduler profile",
            category: "Gaming",
            description: "Sets SystemResponsiveness=10 and disables network throttling (NetworkThrottlingIndex=0xFFFFFFFF).",
            rationale: "MMCSS reserves 20% CPU for background tasks by default and throttles network packets during multimedia playback; both are conservative for gaming rigs.",
            impact: "May smooth frametimes under load; slightly less CPU reserved for background services.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKLM", path: "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile", name: "SystemResponsiveness", value: RegVal::Dword(10) },
                Action::RegSet { root: "HKLM", path: "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile", name: "NetworkThrottlingIndex", value: RegVal::Dword(0xFFFF_FFFF) },
            ],
        },
        Tweak {
            id: "startup_delay_off",
            name: "Remove artificial startup app delay",
            category: "Startup",
            description: "Sets Explorer Serialize\\StartupDelayInMSec to 0.",
            rationale: "Windows staggers startup apps by several seconds after login; removing the delay makes the desktop usable sooner.",
            impact: "Faster time-to-desktop after login.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Serialize", name: "StartupDelayInMSec", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "visual_fx_performance",
            name: "Visual effects: best performance",
            category: "Memory",
            description: "Sets the visual-effects preference to 'Adjust for best performance'.",
            rationale: "Disables shell animations, shadows and smoothing. On any GPU from the last decade DWM composites these effects at negligible cost, so this is a preference, not a performance fix; it can still feel snappier on very old iGPUs or in remote sessions.",
            impact: "No measurable gain on modern hardware; mostly cosmetic.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            // VisualFXSetting alone only selects the radio button in the
            // dialog; the effects themselves are the UserPreferencesMask bits,
            // which the dialog writes when "best performance" is confirmed.
            // Both are captured and restored exactly. Effective after sign-out.
            actions: vec![
                Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\VisualEffects", name: "VisualFXSetting", value: RegVal::Dword(2) },
                Action::RegSet {
                    root: "HKCU",
                    path: "Control Panel\\Desktop",
                    name: "UserPreferencesMask",
                    value: RegVal::Raw { ty: "REG_BINARY".into(), hex: "9012038010000000".into() },
                },
            ],
        },
        Tweak {
            id: "telemetry_minimal",
            name: "Minimize telemetry (policy + DiagTrack)",
            category: "Privacy",
            description: "Sets AllowTelemetry to 1 (Basic/Required, the lowest level Windows Home/Pro actually honor) and disables the Connected User Experiences (DiagTrack) service.",
            rationale: "Reduces background data collection, disk writes and network chatter from the telemetry pipeline. The often-copied value 0 (Security) is silently ignored on Home and Pro — only Enterprise/Education honor it — so 1 is the honest minimum.",
            impact: "Less background I/O; some diagnostic data unavailable to Microsoft support.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKLM", path: "SOFTWARE\\Policies\\Microsoft\\Windows\\DataCollection", name: "AllowTelemetry", value: RegVal::Dword(1) },
                Action::Service { name: "DiagTrack", target: "Disabled" },
            ],
        },
        Tweak {
            id: "advertising_id_off",
            name: "Disable advertising ID",
            category: "Privacy",
            description: "Turns off the per-user advertising identifier.",
            rationale: "Stops apps from using your advertising ID for cross-app profiling. Zero performance cost, pure privacy win.",
            impact: "Privacy improvement; no performance change.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\Windows\\CurrentVersion\\AdvertisingInfo", name: "Enabled", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "sysmain_off",
            name: "Disable SysMain/Superfetch (SSD systems)",
            category: "Services",
            description: "Sets the SysMain service to Disabled.",
            rationale: "SysMain's prefetching helps HDDs; on SSDs it adds background I/O and RAM pressure with negligible benefit. Only apply if your system drive is an SSD.",
            impact: "Less background disk activity on SSDs, but app launches can get slower without prefetch. Do NOT apply on HDD systems. Advanced: undo if launches feel slower.",
            risk: "High",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "SysMain", target: "Disabled" }],
        },
        Tweak {
            id: "hibernate_off",
            name: "Disable hibernation (free hiberfil.sys)",
            category: "Storage",
            description: "Runs 'powercfg /h off', deleting hiberfil.sys.",
            rationale: "hiberfil.sys reserves ~40% of RAM size on disk. Desktops that never hibernate get that space back. Also disables Fast Startup.",
            impact: "Frees several GB on the system drive; hibernate/Fast Startup unavailable until reverted.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Cmd {
                apply: "powercfg /h off",
                revert: "powercfg /h on",
                show: Some(ShowCapture::HibernateState),
            }],
        },
        Tweak {
            id: "menu_delay_fast",
            name: "Faster menu response",
            category: "Responsiveness",
            description: "Reduces MenuShowDelay from 400ms to 100ms.",
            rationale: "Pure artificial delay before menus open; lowering it makes the shell feel faster at zero cost. (Don't set 0, hover menus become twitchy.)",
            impact: "UI feels snappier.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKCU", path: "Control Panel\\Desktop", name: "MenuShowDelay", value: RegVal::Str("100".into()) }],
        },
        Tweak {
            id: "content_suggestions_off",
            name: "Disable tips, suggestions & Start ads",
            category: "Privacy",
            description: "Turns off Content Delivery Manager suggestion surfaces.",
            rationale: "Suggestion content is fetched and rendered in the background and occasionally installs sponsored apps.",
            impact: "Cleaner Start menu, less background fetching.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager", name: "SubscribedContent-338389Enabled", value: RegVal::Dword(0) },
                Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager", name: "SoftLandingEnabled", value: RegVal::Dword(0) },
            ],
        },
        Tweak {
            id: "background_apps_off",
            name: "Disable UWP background apps (global)",
            category: "Background",
            description: "Prevents Store apps from running in the background for this user.",
            rationale: "Stops Store apps from idling in the background. Suspended UWP apps already use almost no CPU, and Windows 11 ignores this global switch for many apps, so the effect is about notifications/privacy, not speed.",
            impact: "No measurable performance gain on typical systems. Some app notifications, live tiles and background sync stop.",
            risk: "Medium",
            requires_admin: false,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\Windows\\CurrentVersion\\BackgroundAccessApplications", name: "GlobalUserDisabled", value: RegVal::Dword(1) }],
        },
        Tweak {
            id: "mouse_accel_off",
            name: "Disable mouse acceleration (Enhance Pointer Precision)",
            category: "Gaming",
            description: "Sets MouseSpeed/Threshold1/Threshold2 to 0.",
            rationale: "Acceleration makes cursor distance depend on speed, bad for aim consistency. Esports-standard tweak.",
            impact: "1:1 mouse input. Cursor feel changes; takes adjustment.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKCU", path: "Control Panel\\Mouse", name: "MouseSpeed", value: RegVal::Str("0".into()) },
                Action::RegSet { root: "HKCU", path: "Control Panel\\Mouse", name: "MouseThreshold1", value: RegVal::Str("0".into()) },
                Action::RegSet { root: "HKCU", path: "Control Panel\\Mouse", name: "MouseThreshold2", value: RegVal::Str("0".into()) },
            ],
        },
        Tweak {
            id: "game_mode_on",
            name: "Enable Windows Game Mode",
            category: "Gaming",
            description: "Turns on Game Mode (auto-prioritizes the game, defers Windows Update scans while playing).",
            rationale: "Game Mode steers scheduler attention to the foreground game and suppresses update activity during play. Microsoft's own supported gaming optimization.",
            impact: "Fewer background interruptions while gaming.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\GameBar", name: "AllowAutoGameMode", value: RegVal::Dword(1) },
                Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\GameBar", name: "AutoGameModeEnabled", value: RegVal::Dword(1) },
            ],
        },
        Tweak {
            id: "fse_optimizations_off",
            name: "Disable Fullscreen Optimizations (FSE behavior)",
            category: "Gaming",
            description: "Forces classic exclusive-fullscreen behavior instead of Windows' borderless-fullscreen wrapper.",
            rationale: "Fullscreen Optimizations route games through DWM composition. On many systems classic exclusive fullscreen gives lower input latency and steadier frametimes. DXGIHonorFSEWindowsCompatible is deliberately NOT set: it can break ICC color profiles in exclusive fullscreen.",
            impact: "May lower input lag in fullscreen games on affected systems; Alt-Tab becomes slower.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKCU", path: "System\\GameConfigStore", name: "GameDVR_FSEBehaviorMode", value: RegVal::Dword(2) },
                Action::RegSet { root: "HKCU", path: "System\\GameConfigStore", name: "GameDVR_HonorUserFSEBehaviorMode", value: RegVal::Dword(1) },
                Action::RegSet { root: "HKCU", path: "System\\GameConfigStore", name: "GameDVR_EFSEFeatureFlags", value: RegVal::Dword(0) },
            ],
        },
        Tweak {
            id: "hags_on",
            name: "Hardware-Accelerated GPU Scheduling (HAGS)",
            category: "Gaming",
            description: "Sets HwSchMode=2, letting the GPU manage its own scheduling queue. REBOOT REQUIRED.",
            rationale: "Offloads frame scheduling from the CPU to a dedicated GPU scheduler, which can reduce latency on modern GPUs (NVIDIA 10-series+/AMD RDNA+), and is required for DLSS 3 Frame Generation.",
            impact: "May slightly lower latency on modern GPUs. Requires reboot. On very old GPUs/drivers it can hurt, benchmark before/after.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers", name: "HwSchMode", value: RegVal::Dword(2) }],
        },
        Tweak {
            id: "hvci_off",
            name: "Turn off Memory Integrity (HVCI)",
            category: "Gaming",
            description: "Sets Core isolation > Memory integrity (HypervisorEnforcedCodeIntegrity) to off. RESTART REQUIRED. Lowers security: kernel code integrity is no longer enforced by the hypervisor.",
            rationale: "Hypervisor-enforced code integrity checks kernel memory through the hypervisor, which costs CPU time. Independent tests measured a few percent lower average FPS with it on, and much more in some CPU-bound games; others show no difference.",
            impact: "May raise FPS in CPU-bound games, depending on the game; measure before and after. Windows Security will report Memory integrity as off.",
            risk: "High",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SYSTEM\\CurrentControlSet\\Control\\DeviceGuard\\Scenarios\\HypervisorEnforcedCodeIntegrity", name: "Enabled", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "priority_separation",
            name: "Foreground priority boost (Win32PrioritySeparation 0x26)",
            category: "Gaming",
            description: "Sets Win32PrioritySeparation to 0x26: short, variable quanta with a 3:1 foreground boost.",
            rationale: "0x26 decodes to short / variable / 3:1, which is exactly what the Windows client default (2) already resolves to. The widely shared claim that it gives short FIXED quanta is wrong for this value. Writing it explicitly only matters if another tool changed the value before.",
            impact: "No measurable change on a default Windows client install; it only restores client scheduling if the value was altered.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SYSTEM\\CurrentControlSet\\Control\\PriorityControl", name: "Win32PrioritySeparation", value: RegVal::Dword(0x26) }],
        },
        Tweak {
            id: "power_throttling_off",
            name: "Disable Power Throttling (EcoQoS)",
            category: "Gaming",
            description: "Globally disables Windows power throttling of background processes. REBOOT RECOMMENDED.",
            rationale: "EcoQoS forces processes onto efficiency states/E-cores; it sometimes misclassifies game launchers, overlays, OBS or anti-cheat helpers, starving them mid-game.",
            impact: "No process gets force-throttled. Higher idle power draw and heat; Advanced: do NOT use on laptops (battery life and thermals suffer). Helps only where a specific helper process was misclassified.",
            risk: "High",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SYSTEM\\CurrentControlSet\\Control\\Power\\PowerThrottling", name: "PowerThrottlingOff", value: RegVal::Dword(1) }],
        },
        Tweak {
            id: "mmcss_games_task",
            name: "MMCSS 'Games' task: maximum priority",
            category: "Gaming",
            description: "Raises the multimedia scheduler's Games profile: CPU Priority 6 (default 2) and Scheduling Category High (default Medium).",
            rationale: "Only threads that explicitly register with MMCSS for the 'Games' task (AvSetMmThreadCharacteristics) inherit this profile; most games never do, so many titles are unaffected. Only CPU Priority and Scheduling Category are read by Windows; the widely-copied 'GPU Priority 8' and 'SFIO Priority High' values are documented as unused by Microsoft, so they are deliberately not written.",
            impact: "Games registered with MMCSS run in the High scheduling band with a higher base thread priority; most games already get this from the built-in profile, so gains are game-dependent.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKLM", path: "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile\\Tasks\\Games", name: "Priority", value: RegVal::Dword(6) },
                Action::RegSet { root: "HKLM", path: "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile\\Tasks\\Games", name: "Scheduling Category", value: RegVal::Str("High".into()) },
            ],
        },
        Tweak {
            id: "mpo_off",
            name: "Disable Multiplane Overlay (MPO)",
            category: "Gaming",
            description: "Sets DWM OverlayTestMode=5, disabling multiplane overlays. REBOOT REQUIRED. NOTE: Windows 11 25H2 reportedly ignores this value; on 25H2+ the effect may not apply.",
            rationale: "MPO causes stutter, flicker and black-screen glitches on a range of GPU/monitor combos, both NVIDIA and AMD have historically recommended this exact toggle as the workaround. Confirmed working through Windows 11 24H2.",
            impact: "Fixes MPO-related flicker/stutter if present. Slightly higher DWM GPU load and possibly higher latency in windowed/borderless games. Advanced: only apply if you see flicker/stutter; undo if no change.",
            risk: "High",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SOFTWARE\\Microsoft\\Windows\\Dwm", name: "OverlayTestMode", value: RegVal::Dword(5) }],
        },
        Tweak {
            id: "gamebar_popups_off",
            name: "Disable Game Bar Popups & Start Panel",
            category: "Gaming",
            description: "Disables Game Bar start panel and Nexus overlay hints.",
            rationale: "Game Bar popups interrupt games and cost focus/frames on startup.",
            impact: "No more Game Bar popups; Win+G still works manually.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\GameBar", name: "ShowStartupPanel", value: RegVal::Dword(0) },
                Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\GameBar", name: "UseNexusForGameBarEnabled", value: RegVal::Dword(0) },
            ],
        },
        Tweak {
            id: "transparency_off",
            name: "Disable Transparency Effects",
            category: "Responsiveness",
            description: "Disables acrylic/blur transparency in taskbar, Start menu, and windows.",
            rationale: "Blur effects cost DWM GPU time, which can matter on iGPUs and weaker cards.",
            impact: "Lower DWM GPU load; UI looks more plain.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKCU", path: "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize", name: "EnableTransparency", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "min_animate_off",
            name: "Disable Window Minimize Animation",
            category: "Responsiveness",
            description: "Disables the minimize/maximize animation.",
            rationale: "Pure waiting animation (~200ms per action). Without it, window handling feels instant.",
            impact: "Snappier window behavior.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKCU", path: "Control Panel\\Desktop\\WindowMetrics", name: "MinAnimate", value: RegVal::Str("0".into()) }],
        },
        Tweak {
            id: "start_websearch_off",
            name: "Disable Web Search in Start Menu",
            category: "Privacy",
            description: "Start menu search shows only local results (no Bing suggestions).",
            rationale: "Every keystroke in Start search otherwise goes to Bing, adds latency and data leakage.",
            impact: "Faster, purely local Start menu search.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKCU", path: "Software\\Policies\\Microsoft\\Windows\\Explorer", name: "DisableSearchBoxSuggestions", value: RegVal::Dword(1) }],
        },
        Tweak {
            id: "lockscreen_ads_off",
            name: "Disable Lock Screen Spotlight / Ads",
            category: "Privacy",
            description: "Disables Windows Spotlight overlays and Fun Facts on the lock screen.",
            rationale: "Spotlight downloads content in the background and shows sponsored hints.",
            impact: "Static lock screen, no background fetching.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager", name: "RotatingLockScreenOverlayEnabled", value: RegVal::Dword(0) },
                Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager", name: "SubscribedContent-338387Enabled", value: RegVal::Dword(0) },
            ],
        },
        Tweak {
            id: "widgets_off",
            name: "Disable Widgets / News & Interests (Policy)",
            category: "Background",
            description: "Disables the Widget Board (Win11) via the Dsh policy and hides the widget taskbar button. NOTE: on recent builds Microsoft's UCPD driver protects the per-user TaskbarDa value, so the taskbar-button write may be blocked; the machine Dsh policy is what reliably disables Widgets.",
            rationale: "The widget process (msedgewebview2) runs permanently, fetches news, and costs RAM + network.",
            impact: "Permanently saves RAM and background traffic. Widget button disappears.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKLM", path: "SOFTWARE\\Policies\\Microsoft\\Dsh", name: "AllowNewsAndInterests", value: RegVal::Dword(0) },
                Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced", name: "TaskbarDa", value: RegVal::Dword(0) },
            ],
        },
        Tweak {
            id: "delivery_opt_off",
            name: "Disable Update P2P (Delivery Optimization)",
            category: "Network",
            description: "Sets DODownloadMode=0, updates come only from Microsoft, no peer-to-peer.",
            rationale: "Windows otherwise uploads updates to other PCs (upload bandwidth!) and keeps a disk cache.",
            impact: "No upload consumption from Windows Update; slightly slower update downloads possible.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SOFTWARE\\Policies\\Microsoft\\Windows\\DeliveryOptimization", name: "DODownloadMode", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "wer_off",
            name: "Disable Windows Error Reporting",
            category: "Background",
            description: "Disables automatic crash report collection and upload.",
            rationale: "WER writes crash dumps to disk and uploads them, I/O spikes exactly when the system is already struggling.",
            impact: "No more WER dumps/uploads. Downside: less crash diagnostic data if you debug yourself.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SOFTWARE\\Microsoft\\Windows\\Windows Error Reporting", name: "Disabled", value: RegVal::Dword(1) }],
        },
        Tweak {
            id: "activity_history_off",
            name: "Disable Activity History (Timeline)",
            category: "Privacy",
            description: "Stops logging app/document activity (PublishUserActivities=0).",
            rationale: "Every opened app/file is otherwise written to the activity database, background I/O and data collection.",
            impact: "Fewer background writes; Win+Tab Timeline stays empty.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SOFTWARE\\Policies\\Microsoft\\Windows\\System", name: "PublishUserActivities", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "usb_suspend_off",
            name: "Disable USB Selective Suspend",
            category: "Gaming",
            description: "Prevents Windows from putting USB devices to sleep during use.",
            rationale: "Selective Suspend causes USB audio dropouts, micro-stutters with mice/headsets, and controller reconnects.",
            impact: "Only helps if you actually have USB dropouts/reconnects; otherwise no measurable change. Higher power draw (noticeable on laptop battery). Applies to the active power plan only. Advanced.",
            risk: "High",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::Cmd {
                    apply: "powercfg /setacvalueindex scheme_current 2a737441-1930-4402-8d77-b2bebba308a3 48e6b7a6-50f5-4782-a5d4-53bb8f07e226 0",
                    revert: "powercfg /setacvalueindex scheme_current 2a737441-1930-4402-8d77-b2bebba308a3 48e6b7a6-50f5-4782-a5d4-53bb8f07e226 1",
                    show: Some(ShowCapture::PowerSettingIndex {
                        subgroup: "2a737441-1930-4402-8d77-b2bebba308a3",
                        setting: "48e6b7a6-50f5-4782-a5d4-53bb8f07e226",
                    }),
                },
                Action::Cmd { apply: "powercfg /setactive scheme_current", revert: "powercfg /setactive scheme_current", show: None },
            ],
        },
        Tweak {
            id: "fast_startup_off",
            name: "Disable Fast Startup",
            category: "Power",
            description: "Disables hiberboot, Shutdown performs a full cold shutdown.",
            rationale: "Fast Startup freezes the kernel instead of restarting: driver issues survive reboots, dual-boot setups get corrupted, uptime grows indefinitely. A real cold boot fixes more than it costs.",
            impact: "Boot takes longer (a full cold boot), but with a fresh kernel state every time. Hibernate itself remains available.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Power", name: "HiberbootEnabled", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "disable_paging_executive",
            name: "Keep Kernel in RAM (DisablePagingExecutive, Legacy)",
            category: "Memory",
            description: "Prevents kernel code and drivers from being paged out to the swap file. LEGACY: Microsoft states Windows 8+ manages this efficiently on its own; modern sources find no measurable benefit.",
            rationale: "Historically (Vista/7-era, ≤4-8 GB RAM) this avoided slow read-back of paged kernel code. On current Windows with 16+ GB RAM it only pins kernel memory, which can increase memory pressure under load instead of helping.",
            impact: "No measurable gain expected on modern systems; marginally higher RAM usage. Kept for older low-RAM machines where the classic advice still applies.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Memory Management", name: "DisablePagingExecutive", value: RegVal::Dword(1) }],
        },
        Tweak {
            id: "core_parking_off",
            name: "Disable CPU Core Parking",
            category: "Gaming",
            description: "Keeps all CPU cores permanently active (Core Parking Min Cores = 100%). REBOOT RECOMMENDED.",
            rationale: "Windows parks cores at idle to save power; unparking is fast on modern CPUs, so gains are situational. WARNING: on AMD dual-CCD X3D CPUs (7950X3D/7900X3D/9950X3D/9900X3D) the AMD driver uses core parking to keep games on the V-Cache CCD, and on Intel hybrid CPUs (12th gen and newer) parking steers work between P- and E-cores; forcing every core unparked can make games slower there.",
            impact: "Situational: may reduce rare unpark latency on older CPUs; can hurt on dual-CCD X3D and Intel hybrid CPUs. Higher idle power draw. Applies to the active power plan only. Advanced: benchmark before/after.",
            risk: "High",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::Cmd {
                    apply: "powercfg /setacvalueindex scheme_current 54533251-82be-4824-96c1-47b60b740d00 0cc5b647-c1df-4637-891a-dec35c318583 100",
                    revert: "powercfg /setacvalueindex scheme_current 54533251-82be-4824-96c1-47b60b740d00 0cc5b647-c1df-4637-891a-dec35c318583 0",
                    show: Some(ShowCapture::PowerSettingIndex {
                        subgroup: "54533251-82be-4824-96c1-47b60b740d00",
                        setting: "0cc5b647-c1df-4637-891a-dec35c318583",
                    }),
                },
                Action::Cmd { apply: "powercfg /setactive scheme_current", revert: "powercfg /setactive scheme_current", show: None },
            ],
        },
        Tweak {
            id: "ntfs_last_access_off",
            name: "Disable NTFS Last-Access Timestamp",
            category: "Storage",
            description: "Prevents NTFS from writing the last-accessed timestamp on every read.",
            rationale: "Since Windows 10 1803 last-access updates are system-managed and already disabled by default on volumes larger than 128 GB, and NTFS only flushes them lazily (about once per hour per file). Setting 1 makes it user-managed and always off.",
            impact: "No measurable gain on most systems (already off by default on large volumes). Tools relying on last-access times lose that data.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet {
                root: "HKLM",
                path: "SYSTEM\\CurrentControlSet\\Control\\FileSystem",
                name: "NtfsDisableLastAccessUpdate",
                value: RegVal::Dword(1),
            }],
        },
        Tweak {
            id: "ntfs_8dot3_off",
            name: "Disable 8.3 Short Names (DOS compatibility)",
            category: "Storage",
            description: "Stops automatic generation of 8.3 short names (e.g. PROGRA~1) for new files.",
            rationale: "On every file creation NTFS checks if an 8.3 short name must be generated, unnecessary overhead; no modern program needs this legacy compatibility.",
            impact: "Can speed up file creation in very large directories; no 8.3 names for newly created files.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet {
                root: "HKLM",
                path: "SYSTEM\\CurrentControlSet\\Control\\FileSystem",
                name: "NtfsDisable8dot3NameCreation",
                value: RegVal::Dword(1),
            }],
        },
        Tweak {
            id: "xbox_services_off",
            name: "Set Xbox Background Services to Manual",
            category: "Services",
            description: "Sets XblAuthManager, XblGameSave, and XboxNetApiSvc to Manual.",
            rationale: "These services are already Manual (trigger-start) by default and only run when an Xbox / Game Pass app needs them, so there is nothing to gain at idle.",
            impact: "No measurable performance change. If something set them to Automatic, this restores the on-demand default.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::Service { name: "XblAuthManager", target: "Manual" },
                Action::Service { name: "XblGameSave", target: "Manual" },
                Action::Service { name: "XboxNetApiSvc", target: "Manual" },
            ],
        },
        Tweak {
            id: "remote_registry_off",
            name: "Disable Remote Registry",
            category: "Services",
            description: "Sets the RemoteRegistry service to Disabled.",
            rationale: "Allows external programs to access the registry over the network, a known attack vector that home users never need.",
            impact: "Security hardening only, no performance change (the service is already Disabled on Home and idle on Pro).",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "RemoteRegistry", target: "Disabled" }],
        },
        Tweak {
            id: "cortana_off",
            name: "Disable Cortana (Policy, Legacy)",
            category: "Privacy",
            description: "Sets AllowCortana=0 via policy. LEGACY: Cortana was removed from Windows 11 starting with 24H2 — this policy is a no-op there and only matters on Windows 10 and Windows 11 up to 23H2.",
            rationale: "On supported versions the policy stops Cortana from running as a background process collecting voice metrics. On 24H2+ there is nothing left to disable; kept only so users on older builds still get the effect.",
            impact: "Windows 10 / Win11 up to 23H2: no Cortana background process. Win11 24H2+: no effect either way.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SOFTWARE\\Policies\\Microsoft\\Windows\\Windows Search", name: "AllowCortana", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "edge_prelaunch_off",
            name: "Disable Edge Preloading (Startup Boost)",
            category: "Startup",
            description: "Sets the Chromium Edge Startup Boost policy (Software\\Policies\\Microsoft\\Edge\\StartupBoostEnabled=0) so Edge does not silently preload in the background.",
            rationale: "Edge keeps a background process resident from boot via Startup Boost, costing RAM even when Edge is never used. The old EdgeHTML 'AllowPrelaunch' policy no longer affects modern Edge.",
            impact: "Less RAM usage after boot; Edge opens slightly slower on first launch. Requires admin (machine policy).",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKLM", path: "SOFTWARE\\Policies\\Microsoft\\Edge", name: "StartupBoostEnabled", value: RegVal::Dword(0) },
            ],
        },
        Tweak {
            id: "search_highlights_off",
            name: "Disable Search Highlights & Trending Content",
            category: "Privacy",
            description: "Disables dynamic search highlights in the Windows search box (IsDynamicSearchBoxEnabled=0) and hides the Windows 10 News and Interests taskbar feed (ShellFeedsTaskbarViewMode=2; no effect on Windows 11).",
            rationale: "The search box downloads new highlight content daily and shows it on open, background traffic and distraction with no value.",
            impact: "No network fetching by the search box; calmer, faster search box.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\Windows\\CurrentVersion\\SearchSettings", name: "IsDynamicSearchBoxEnabled", value: RegVal::Dword(0) },
                Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\Windows\\CurrentVersion\\Feeds", name: "ShellFeedsTaskbarViewMode", value: RegVal::Dword(2) },
            ],
        },
        Tweak {
            id: "auto_play_off",
            name: "Disable AutoPlay",
            category: "Privacy",
            description: "Prevents Windows from auto-running USB drives or media when plugged in.",
            rationale: "AutoPlay is a classic attack vector for autorun malware. No modern system needs auto-starting unknown devices.",
            impact: "Security improvement; USB devices and disc media no longer open automatically.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\AutoplayHandlers", name: "DisableAutoplay", value: RegVal::Dword(1) }],
        },
        Tweak {
            id: "svc_fax_off",
            name: "Disable Fax Service",
            category: "Services",
            description: "Sets the Fax service to Disabled.",
            rationale: "Virtually no one uses Windows Fax. The service loads drivers and waits for fax lines, pure waste.",
            impact: "One fewer starting service. Fax functionality removed (reversible).",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "Fax", target: "Disabled" }],
        },
        Tweak {
            id: "svc_geolocation_off",
            name: "Set Location Service (lfsvc) to Manual",
            category: "Privacy",
            description: "Sets the Geolocation service to Manual.",
            rationale: "lfsvc continuously sends location requests when active. Apps needing GPS/location start it themselves on demand.",
            impact: "Privacy only, no performance change (lfsvc is trigger-started and idle when unused). Maps/weather apps can still start the service.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "lfsvc", target: "Manual" }],
        },
        Tweak {
            id: "svc_wmp_network_off",
            name: "Disable Windows Media Player Network Sharing",
            category: "Services",
            description: "Sets WMPNetworkSvc to Disabled.",
            rationale: "Shares media over DLNA/UPnP on the home network, hardly anyone uses this through Windows itself. Unnecessary service + network listener.",
            impact: "No more DLNA streaming via Windows Media Player.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "WMPNetworkSvc", target: "Disabled" }],
        },
        Tweak {
            id: "svc_maps_off",
            name: "Disable Offline Maps Manager",
            category: "Services",
            description: "Sets MapsBroker to Disabled.",
            rationale: "Manages Windows offline map downloads. Anyone not using Windows Maps never needs this service.",
            impact: "No automatic map updates; Windows Maps offline functionality unavailable.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "MapsBroker", target: "Disabled" }],
        },
        Tweak {
            id: "svc_link_tracking_off",
            name: "Disable Distributed Link Tracking Client",
            category: "Services",
            description: "Sets TrkWks to Disabled.",
            rationale: "Tracks NTFS links across network drives (legacy enterprise feature). On home systems it runs completely for nothing.",
            impact: "Broken shortcuts over network drives won't be auto-repaired, affects home users virtually never.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "TrkWks", target: "Disabled" }],
        },
        Tweak {
            id: "svc_wisvc_off",
            name: "Disable Windows Insider Service",
            category: "Services",
            description: "Sets wisvc to Disabled.",
            rationale: "Only needed for the Windows Insider Program. Pure overhead for non-Insiders.",
            impact: "No effect for non-Insiders; Insider Program can no longer be activated.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "wisvc", target: "Disabled" }],
        },
        Tweak {
            id: "svc_diag_tracking_off",
            name: "Disable Diagnostics Hub Standard Collector (Visual Studio)",
            category: "Services",
            description: "Sets diagnosticshub.standardcollector.service (Microsoft Diagnostics Hub Standard Collector) to Disabled. This is NOT the DiagTrack telemetry service.",
            rationale: "This Manual-start service collects ETW traces for Visual Studio / developer diagnostics tools. It only runs when a profiler requests it, so disabling it changes nothing at idle.",
            impact: "No performance or telemetry change; Visual Studio diagnostic tools stop working.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "diagnosticshub.standardcollector.service", target: "Disabled" }],
        },
        Tweak {
            id: "wsearch_manual",
            name: "Windows Search indexer: Manual start",
            category: "Services",
            description: "Sets the WSearch service to Manual.",
            rationale: "The indexer can cause sustained disk/CPU churn. Manual keeps search functional (slower, non-indexed) while stopping background indexing.",
            impact: "Less background I/O while indexing; Start-menu, Outlook and Explorer search become slow and incomplete. Advanced.",
            risk: "High",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "WSearch", target: "Manual" }],
        },
        Tweak {
            id: "nagle_off",
            name: "Disable Nagle's Algorithm",
            category: "Network",
            description: "Sets TcpAckFrequency=1 and TCPNoDelay=1 on all network adapters, eliminating Nagle's packet-coalescing delay.",
            rationale: "Nagle only coalesces small TCP sends. Almost all online games use UDP, and games that use TCP already set TCP_NODELAY on their own sockets, so these per-interface values do not change in-game ping.",
            impact: "No measurable effect on game latency in practice; may matter only for old TCP-based games that do not set TCP_NODELAY.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::Cmd {
                    apply: r"powershell -NoProfile -WindowStyle Hidden -Command $errs = 0; Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces' | ForEach-Object { try { Set-ItemProperty -Path $_.PSPath -Name TcpAckFrequency -Value 1 -Type DWord -Force -EA Stop; Set-ItemProperty -Path $_.PSPath -Name TCPNoDelay -Value 1 -Type DWord -Force -EA Stop } catch { $errs++ } }; if ($errs -gt 0) { exit 1 }",
                    // Only removes values that still hold OUR written value (1),
                    // so an adapter with pre-existing custom settings keeps them.
                    // Fallback for the force-revert path only; the precise
                    // journal undo uses the NagleInterfaces capture instead.
                    revert: r"powershell -NoProfile -WindowStyle Hidden -Command $errs = 0; Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces' | ForEach-Object { $p = Get-ItemProperty -Path $_.PSPath; if ($p.PSObject.Properties['TcpAckFrequency'] -and $p.TcpAckFrequency -eq 1) { try { Remove-ItemProperty -Path $_.PSPath -Name TcpAckFrequency -EA Stop } catch { $errs++ } }; if ($p.PSObject.Properties['TCPNoDelay'] -and $p.TCPNoDelay -eq 1) { try { Remove-ItemProperty -Path $_.PSPath -Name TCPNoDelay -EA Stop } catch { $errs++ } } }; if ($errs -gt 0) { exit 1 }",
                    show: Some(ShowCapture::NagleInterfaces),
                },
            ],
        },
        Tweak {
            id: "tcp_timed_wait_low",
            name: "Reduce TCP TIME_WAIT Delay",
            category: "Network",
            description: "Sets TCPTimedWaitDelay to 30 seconds (default: 120). Frees up ports faster after connections close.",
            rationale: "Only relevant for servers or tools that open thousands of short outbound TCP connections per minute; games use few long-lived (mostly UDP) connections and never exhaust the ephemeral port range.",
            impact: "No measurable effect for desktop/gaming use. Kept for heavy connection-churn workloads.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKLM", path: "SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters", name: "TCPTimedWaitDelay", value: RegVal::Dword(30) },
            ],
        },
        Tweak {
            id: "irp_stack_high",
            name: "Increase Network IRP Stack Size",
            category: "Network",
            description: "Sets IRPStackSize to 32 for the Server service (default: 15). Allows more simultaneous I/O request packets.",
            rationale: "IRPStackSize is read only by the LanmanServer (file-sharing server) service; it has nothing to do with game, voice or streaming traffic. Raising it historically fixed 'not enough server storage' errors on shared folders.",
            impact: "No effect on gaming/network latency. Only relevant if this PC shares folders and logs IRP stack errors.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKLM", path: "SYSTEM\\CurrentControlSet\\Services\\LanmanServer\\Parameters", name: "IRPStackSize", value: RegVal::Dword(32) },
            ],
        },
        Tweak {
            id: "qos_bandwidth_free",
            name: "Set QoS NonBestEffortLimit to 0",
            category: "Network",
            description: "Sets NonBestEffortLimit to 0. Note: this does NOT free up 20% bandwidth, that is a long-standing myth.",
            rationale: "The '20% reserved bandwidth' claim is false. Per Microsoft, the QoS Packet Scheduler only limits bandwidth for apps that actively use the QoS API; if none do, 100% is already available to normal traffic. Setting NonBestEffortLimit=0 therefore has no measurable effect for typical users. Included only for completeness, expect no real-world gain.",
            impact: "No measurable change for normal use. The reserved bandwidth is already available when no QoS-aware app requests it.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKLM", path: "SOFTWARE\\Policies\\Microsoft\\Windows\\Psched", name: "NonBestEffortLimit", value: RegVal::Dword(0) },
            ],
        },
        Tweak {
            id: "tcp_autotuning_normal",
            name: "Set TCP Auto-Tuning to Normal",
            category: "Network",
            description: "Forces TCP receive window auto-tuning to 'normal' level for consistent throughput.",
            rationale: "'Normal' is already the Windows default. This only restores it if another tool disabled or restricted auto-tuning, which can cap download throughput.",
            impact: "No change on a default install; restores download throughput if auto-tuning was restricted. No effect on game latency.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::Cmd {
                    apply:  "netsh int tcp set global autotuninglevel=normal",
                    revert: "netsh int tcp set global autotuninglevel=normal",
                    show: Some(ShowCapture::TcpAutotuning),
                },
            ],
        },
        Tweak {
            id: "net_rss_on",
            name: "Enable Receive Side Scaling (RSS)",
            category: "Network",
            description: "Ensures RSS is enabled so network interrupts are spread across CPU cores instead of hammering core 0.",
            rationale: "RSS is enabled by default on Windows. This global switch only re-enables it if something turned it off; it matters for multi-gigabit throughput, not for game ping.",
            impact: "No change on a default install; restores multi-core receive processing if it was disabled. No measurable gaming gain.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::Cmd {
                    apply:  "netsh int tcp set global rss=enabled",
                    revert: "netsh int tcp set global rss=disabled",
                    show: Some(ShowCapture::RssEnabled),
                },
            ],
        },
    ]
}

// ---------- registry helpers ----------

#[cfg(windows)]
fn hive(root: &str) -> RegKey {
    RegKey::predef(if root == "HKLM" {
        HKEY_LOCAL_MACHINE
    } else {
        HKEY_CURRENT_USER
    })
}

#[cfg(windows)]
fn reg_key_exists(root: &str, path: &str) -> Result<bool, String> {
    match hive(root).open_subkey_with_flags(path, KEY_READ) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("open {root}\\{path}: {e}")),
    }
}

#[cfg(windows)]
pub(crate) fn reg_read(root: &str, path: &str, name: &str) -> Result<Option<RegVal>, String> {
    use winreg::enums::RegType;
    let key = match hive(root).open_subkey_with_flags(path, KEY_READ) {
        Ok(key) => key,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("open {root}\\{path}: {e}")),
    };
    match key.get_raw_value(name) {
        Ok(rv) => match rv.vtype {
            RegType::REG_DWORD => {
                if rv.bytes.len() < 4 {
                    return Err(format!("read {root}\\{path}\\{name}: malformed DWORD"));
                }
                Ok(Some(RegVal::Dword(u32::from_le_bytes(
                    rv.bytes[..4].try_into().unwrap(),
                ))))
            }
            RegType::REG_SZ => Ok(Some(RegVal::Str(rv.to_string()))),
            other => Ok(Some(RegVal::Raw {
                ty: format!("{other:?}"),
                hex: hex_encode(&rv.bytes),
            })),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read {root}\\{path}\\{name}: {e}")),
    }
}

#[cfg(any(windows, test))]
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(any(windows, test))]
fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

#[cfg(windows)]
fn reg_write(root: &str, path: &str, name: &str, val: &RegVal) -> Result<(), String> {
    let (key, _) = hive(root)
        .create_subkey(path)
        .map_err(|e| format!("open {root}\\{path}: {e}"))?;
    match val {
        RegVal::Dword(d) => key.set_value(name, d),
        RegVal::Str(s) => key.set_value(name, s),
        RegVal::Raw { ty, hex } => {
            use winreg::enums::RegType::*;
            let bytes = hex_decode(hex)
                .ok_or_else(|| format!("set {name}: corrupt raw capture (bad hex)"))?;
            let vtype = match ty.as_str() {
                "REG_DWORD" => REG_DWORD,
                "REG_SZ" => REG_SZ,
                "REG_EXPAND_SZ" => REG_EXPAND_SZ,
                "REG_MULTI_SZ" => REG_MULTI_SZ,
                "REG_BINARY" => REG_BINARY,
                "REG_QWORD" => REG_QWORD,
                "REG_NONE" => REG_NONE,
                other => return Err(format!("set {name}: unknown captured type {other}")),
            };
            key.set_raw_value(name, &winreg::RegValue { bytes, vtype })
        }
    }
    .map_err(|e| format!("set {name}: {e}"))
}

#[cfg(windows)]
fn reg_delete(root: &str, path: &str, name: &str) -> Result<(), String> {
    let key = match hive(root).open_subkey_with_flags(path, KEY_READ | KEY_SET_VALUE) {
        Ok(key) => key,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("open {root}/{path}: {error}")),
    };
    key.delete_value(name).or_else(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Ok(()) // already absent: idempotent undo
        } else {
            Err(format!("delete {name}: {e}"))
        }
    })
}

// Sentinel returned when a service isn't installed on this machine. Kept out of
// band from real StartType values ("Automatic"/"Manual"/"Disabled"/…).
const SERVICE_ABSENT: &str = "__ABSENT__";

/// Internal startup-type token for "Automatic (Delayed Start)". Windows
/// PowerShell 5.1's Set-Service rejects it, so it is applied via sc.exe.
const DELAYED_AUTO: &str = "AutomaticDelayedStart";

fn service_get_start(name: &str) -> Result<String, String> {
    if !crate::ps::is_safe_ident(name) {
        return Err(format!(
            "refusing to query service with unsafe name '{name}'"
        ));
    }
    // SilentlyContinue + explicit sentinel: a missing service must not blow up
    // status detection or the apply's "capture previous state" step.
    // PowerShell 5.1 reports delayed-auto services as plain "Automatic"; the
    // DelayedAutostart registry flag distinguishes them so undo is exact.
    crate::ps::run(&format!(
        "$s = Get-Service -Name '{name}' -ErrorAction SilentlyContinue; \
         if (-not $s) {{ '{SERVICE_ABSENT}' }} else {{ \
           $t = [string]$s.StartType; \
           if ($t -eq 'Automatic') {{ \
             $d = (Get-ItemProperty -LiteralPath ('HKLM:\\SYSTEM\\CurrentControlSet\\Services\\' + $s.ServiceName) -Name DelayedAutostart -ErrorAction SilentlyContinue).DelayedAutostart; \
             if ($d -eq 1) {{ $t = '{DELAYED_AUTO}' }} \
           }}; $t }}"
    ))
    .map(|s| s.trim().to_string())
}

fn service_set_start(name: &str, mode: &str) -> Result<(), String> {
    // Reverting to a previously-absent state is a no-op.
    if mode == SERVICE_ABSENT {
        return Ok(());
    }
    if mode == DELAYED_AUTO {
        if !crate::ps::is_safe_ident(name) {
            return Err(format!(
                "refusing to touch service with unsafe name '{name}'"
            ));
        }
        if service_get_start(name)? == SERVICE_ABSENT {
            return Ok(());
        }
        // Separate argv entries: sc.exe requires `start=` and the value as
        // two tokens.
        return crate::ps::exec("sc.exe", &["config", name, "start=", "delayed-auto"]).map(|_| ());
    }
    // `mode` is interpolated into the PowerShell command; only documented
    // startup types may ever reach it (blocks injection via journal data).
    if !matches!(mode, "Automatic" | "Manual" | "Disabled") {
        return Err(format!("refusing to set invalid startup type '{mode}'"));
    }
    if !crate::ps::is_safe_ident(name) {
        return Err(format!(
            "refusing to touch service with unsafe name '{name}'"
        ));
    }
    // If the service doesn't exist (e.g. the VS diagnostics hub collector on a
    // non-developer machine), there's nothing to disable, treat as success
    // instead of surfacing a scary "service not found" PowerShell error.
    // Failures are reduced to one line: ACCESS_DENIED (Win32 error 5, raised
    // for services whose own ACL refuses even administrators) or the bare
    // exception message, instead of PowerShell's multi-line error record.
    crate::ps::run(&format!(
        "$s = Get-Service -Name '{name}' -ErrorAction SilentlyContinue; if (-not $s) {{ 'ABSENT' }} else {{ \
         try {{ Set-Service -Name '{name}' -StartupType {mode} -ErrorAction Stop; 'OK' }} catch {{ \
         $x = $_.Exception; while ($x) {{ if ($x -is [System.ComponentModel.Win32Exception] -and $x.NativeErrorCode -eq 5) {{ \
         [Console]::Error.WriteLine('ACCESS_DENIED'); exit 5 }}; $x = $x.InnerException }}; \
         [Console]::Error.WriteLine($_.Exception.Message); exit 1 }} }}"
    ))
    .map(|_| ())
    .map_err(|e| service_error_message(name, &e))
}

/// Human-readable form of a service_set_start failure.
fn service_error_message(name: &str, err: &str) -> String {
    if err.trim() == "ACCESS_DENIED" {
        format!(
            "Windows denied access to service '{name}': it is protected by its own permissions, \
             so even administrators cannot change it. Nothing was changed."
        )
    } else {
        err.trim().to_string()
    }
}

/// Journals written before this fix stored some reverts as
/// `powershell -Command "<script>"`. Passed through verbatim, PowerShell
/// evaluates that as an expandable string and merely prints it (exit 0), so
/// the undo silently did nothing. Strip one pair of wrapping quotes when the
/// script contains no other double quote (so `"a" + "b"` is left alone).
fn unwrap_quoted_script(script: &str) -> &str {
    let t = script.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') && !t[1..t.len() - 1].contains('"') {
        &t[1..t.len() - 1]
    } else {
        t
    }
}

fn run_cmdline(line: &str) -> Result<(), String> {
    // PowerShell wrapper lines (`powershell … -Command <expr>` from ps_expr
    // and the TCP tweak one-liners) pass the expression as ONE -Command
    // argument via ps::run, so `$`, quotes and multi-space segments stay
    // literal instead of being whitespace-split and re-joined by powershell.
    if line
        .split_whitespace()
        .next()
        .is_some_and(|t| t.eq_ignore_ascii_case("powershell"))
    {
        if let Some(pos) = line.find(" -Command ") {
            let script = unwrap_quoted_script(&line[pos + " -Command ".len()..]);
            return crate::ps::run(script).map(|_| ());
        }
    }
    let mut parts = line.split_whitespace();
    let exe = parts.next().ok_or("empty command")?;
    let args: Vec<&str> = parts.collect();
    crate::ps::exec(exe, &args).map(|_| ())
}

#[cfg(windows)]
fn vals_eq(a: &RegVal, b: &RegVal) -> bool {
    matches!((a, b), (RegVal::Dword(x), RegVal::Dword(y)) if x == y)
        || matches!((a, b), (RegVal::Str(x), RegVal::Str(y)) if x == y)
        || matches!((a, b), (RegVal::Raw { hex: hx, .. }, RegVal::Raw { hex: hy, .. }) if hx == hy)
}

// ---------- public API ----------

/// Serializes whole apply/revert transactions. The journal lock only protects
/// file updates, not the capture→mutate interval: without this, an undo
/// running between an apply's capture and its mutations marks the entry
/// reverted while the mutations still land afterwards.
static ENGINE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
pub(crate) fn engine_guard() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    ENGINE_LOCK.lock().map_err(|e| format!("engine lock: {e}"))
}

// Known Windows default startup types for every service we touch in the
// catalog. Used by the force-revert path when no journal entry is available.
// Returns None for unmapped services: force-revert must refuse rather than
// silently guess a wrong startup type.
fn service_default(name: &str) -> Option<&'static str> {
    match name {
        "DiagTrack" | "SysMain" | "TrkWks" => Some("Automatic"),
        "MapsBroker" | "WSearch" => Some(DELAYED_AUTO),
        // Tweaks set these to Disabled. Manual is the factory default for
        // most editions; RemoteRegistry ships Disabled on Home SKUs (and
        // Manual on Pro/Enterprise), so Disabled is the safe revert for it.
        "XblAuthManager"
        | "XblGameSave"
        | "XboxNetApiSvc"
        | "lfsvc"
        | "Fax"
        | "wisvc"
        | "diagnosticshub.standardcollector.service"
        | "WMPNetworkSvc" => Some("Manual"),
        "RemoteRegistry" => Some("Disabled"),
        _ => None,
    }
}

/// Every catalog id (the optimization registry must cover exactly these).
#[cfg(test)]
pub(crate) fn catalog_ids() -> Vec<&'static str> {
    catalog().iter().map(|t| t.id).collect()
}

/// Live status of one tweak plus where it came from ("live" | "journal" |
/// "none"), used to verify an apply against the real system.
pub(crate) fn status_of(id: &str) -> Option<(&'static str, &'static str)> {
    let t = catalog().into_iter().find(|t| t.id == id)?;
    let live = detect_status(&t);
    if live != "unknown" {
        return Some((live, "live"));
    }
    let completed = safety::load_journal()
        .unwrap_or_default()
        .iter()
        .any(|e| e.tweak_id == id && !e.reverted && e.completed);
    Some(if completed {
        ("applied", "journal")
    } else {
        ("unknown", "none")
    })
}

/// Catalog + live status, ready for the UI.
pub fn list_with_status() -> Value {
    // Display-only path: a broken journal degrades to "no undo entries"
    // (force-revert still works); mutating paths fail closed instead.
    let journal = safety::load_journal().unwrap_or_default();
    let out: Vec<Value> = catalog()
        .iter()
        .map(|t| {
            // Only completed entries prove "applied": an interrupted apply
            // (crash between journal append and mutations) must not show up
            // as applied/undoable.
            let undoable = journal
                .iter()
                .any(|e| e.tweak_id == t.id && !e.reverted && (e.completed || e.attempted > 0));
            let completed = journal
                .iter()
                .any(|e| e.tweak_id == t.id && !e.reverted && e.completed);
            // Cmd-only tweaks (powercfg/fsutil/netsh one-shots) have no live
            // state detect_status() can read back, so it always returns
            // "unknown", fall back to the journal: a successful, unreverted
            // apply means it IS applied, even though we can't re-verify it.
            let live = detect_status(t);
            let status = match live {
                "unknown" if completed => "applied",
                s => s,
            };
            // Where the status comes from: a live read of Windows, or only
            // the journal (no readable state; the UI must say so).
            let status_source = if live == "unknown" && completed {
                "journal"
            } else if live == "unknown" {
                "none"
            } else {
                "live"
            };
            // canUndo: a journal entry restores the saved previous values;
            // without one, an active tweak is reset to the Windows default
            // (see force_revert).
            let can_undo = undoable || matches!(live, "applied" | "partial");
            json!({
                "id": t.id, "name": t.name, "category": t.category,
                "description": t.description, "rationale": t.rationale,
                "impact": t.impact, "risk": t.risk,
                "requiresAdmin": t.requires_admin, "reversible": t.reversible,
                "status": status, "statusSource": status_source,
                "retired": crate::optregistry::is_retired(t.id),
                "undoable": undoable, "canUndo": can_undo,
            })
        })
        .collect();
    json!(out)
}

#[cfg_attr(not(windows), allow(unused_variables))]
fn detect_status(t: &Tweak) -> &'static str {
    #[cfg(windows)]
    {
        if let Some(status) = effective_status(t.id) {
            return status;
        }
        let mut checkable = 0;
        let mut matching = 0;
        let mut absent = 0;
        for a in &t.actions {
            match a {
                Action::RegSet {
                    root,
                    path,
                    name,
                    value,
                } => {
                    checkable += 1;
                    // An unreadable value counts as "not matching" here: the
                    // detection stays honest instead of failing the listing.
                    if let Some(cur) = reg_read(root, path, name).ok().flatten() {
                        if vals_eq(&cur, value) {
                            matching += 1;
                        }
                    }
                }
                Action::Service { name, target } => match service_get_start(name) {
                    // A service that isn't installed is not "applied": the
                    // tweak simply doesn't apply to this machine.
                    Ok(cur) if cur == SERVICE_ABSENT => absent += 1,
                    Ok(cur) => {
                        checkable += 1;
                        if cur.eq_ignore_ascii_case(target) {
                            matching += 1;
                        }
                    }
                    Err(_) => checkable += 1,
                },
                // powercfg setting tweaks: read the AC index back from the
                // active scheme. Other Cmd tweaks have no readable state.
                Action::Cmd {
                    apply,
                    show: Some(ShowCapture::PowerSettingIndex { subgroup, setting }),
                    ..
                } => {
                    if let Some(want) = power_cmd_target(apply) {
                        if let Some((ac, _dc)) = crate::ps::exec(
                            "powercfg.exe",
                            &["/query", "scheme_current", subgroup, setting],
                        )
                        .ok()
                        .and_then(|out| parse_power_indexes(&out))
                        {
                            checkable += 1;
                            if ac == want {
                                matching += 1;
                            }
                        }
                    }
                }
                Action::Cmd { .. } => {}
            }
        }
        // One-shot command tweaks whose effect is still readable: report the
        // live state instead of trusting the journal.
        if checkable == 0 && absent == 0 {
            if let Some(applied) = cmd_live_state(t.id) {
                return if applied { "applied" } else { "not_applied" };
            }
        }
        return classify_status(checkable, matching, absent);
    }
    #[allow(unreachable_code)]
    "unknown"
}

/// Tweaks whose effective state is not simply "all values equal the target",
/// because Windows treats a missing value as a default.
#[cfg(windows)]
fn effective_status(id: &str) -> Option<&'static str> {
    match id {
        // Game Mode is on unless AutoGameModeEnabled is explicitly 0; a fresh
        // install has no value at all.
        "game_mode_on" => Some(game_mode_status(
            reg_read("HKCU", r"Software\Microsoft\GameBar", "AutoGameModeEnabled")
                .ok()
                .flatten(),
        )),
        // The global FSE values only take effect while
        // HonorUserFSEBehaviorMode is 1; FSEBehaviorMode=2 alone is common on
        // default installs and inert, so it must not count as "active".
        "fse_optimizations_off" => Some(fse_status(
            reg_read("HKCU", r"System\GameConfigStore", "GameDVR_FSEBehaviorMode")
                .ok()
                .flatten(),
            reg_read(
                "HKCU",
                r"System\GameConfigStore",
                "GameDVR_HonorUserFSEBehaviorMode",
            )
            .ok()
            .flatten(),
        )),
        _ => None,
    }
}

#[cfg(any(windows, test))]
fn fse_status(mode: Option<RegVal>, honor: Option<RegVal>) -> &'static str {
    match (mode, honor) {
        (Some(RegVal::Dword(2)), Some(RegVal::Dword(1))) => "applied",
        (Some(RegVal::Raw { .. } | RegVal::Str(_)), _)
        | (_, Some(RegVal::Raw { .. } | RegVal::Str(_))) => "unknown",
        _ => "not_applied",
    }
}

#[cfg(any(windows, test))]
fn game_mode_status(value: Option<RegVal>) -> &'static str {
    match value {
        None => "applied",
        Some(RegVal::Dword(0)) => "not_applied",
        Some(RegVal::Dword(_)) => "applied",
        Some(_) => "unknown",
    }
}

/// Live state of command tweaks that have a readable effect. None = not
/// readable (status then falls back to the journal and is marked as such).
#[cfg(windows)]
fn cmd_live_state(id: &str) -> Option<bool> {
    const HIGH_PERFORMANCE: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";
    match id {
        "power_high_performance" => {
            let out = crate::ps::exec("powercfg.exe", &["/getactivescheme"]).ok()?;
            let guid = parse_active_scheme_guid(&out)?;
            Some(guid.eq_ignore_ascii_case(HIGH_PERFORMANCE))
        }
        "hibernate_off" => match reg_read(
            "HKLM",
            "SYSTEM\\CurrentControlSet\\Control\\Power",
            "HibernateEnabled",
        )
        .ok()
        .flatten()
        {
            Some(RegVal::Dword(v)) => Some(v == 0),
            Some(_) => None,
            // Without the value Windows decides from its default; the
            // hibernation file exists exactly while hibernation is enabled.
            None => {
                let drive = crate::ps::windows_dir()
                    .get(..2)
                    .unwrap_or("C:")
                    .to_string();
                let file = std::path::Path::new(&format!("{drive}\\")).join("hiberfil.sys");
                Some(std::fs::symlink_metadata(file).is_err())
            }
        },
        // Both cmdlets report English enum names on every display language.
        "tcp_autotuning_normal" => crate::ps::run(
            "(Get-NetTCPSetting -SettingName Internet -ErrorAction Stop).AutoTuningLevelLocal",
        )
        .ok()
        .and_then(|s| live_flag(&s, "Normal")),
        "net_rss_on" => {
            crate::ps::run("(Get-NetOffloadGlobalSetting -ErrorAction Stop).ReceiveSideScaling")
                .ok()
                .and_then(|s| live_flag(&s, "Enabled"))
        }
        _ => None,
    }
}

/// The command that restores the hibernation state found before applying.
/// Without `HibernateEnabled` Windows decides from its default; the
/// hibernation file exists exactly while hibernation is enabled (the same
/// rule the live status uses), so its presence decides.
#[cfg_attr(not(windows), allow(dead_code))]
fn hibernate_revert(reg: Option<&RegVal>, hiberfil_exists: bool) -> Option<String> {
    match reg {
        Some(RegVal::Dword(1)) => Some("powercfg /h on".into()),
        Some(RegVal::Dword(0)) => Some("powercfg /h off".into()),
        Some(_) => None,
        None if hiberfil_exists => Some("powercfg /h on".into()),
        None => Some("powercfg /h off".into()),
    }
}

/// Some(applied) for a single non-empty cmdlet output line, None otherwise.
#[cfg(any(windows, test))]
fn live_flag(out: &str, applied_value: &str) -> Option<bool> {
    let v = out.trim();
    (!v.is_empty() && !v.contains('\n')).then(|| v.eq_ignore_ascii_case(applied_value))
}

/// Status from live-check counts. Absent services are excluded from the
/// checkable set; a tweak whose only targets are absent is "unsupported"
/// (the UI then offers neither Apply nor force-Undo).
#[cfg(any(windows, test))]
fn classify_status(checkable: usize, matching: usize, absent: usize) -> &'static str {
    if checkable == 0 {
        return if absent > 0 { "unsupported" } else { "unknown" };
    }
    if matching == checkable {
        "applied"
    } else if matching > 0 {
        "partial"
    } else {
        "not_applied"
    }
}

/// Target AC index of a `powercfg /setacvalueindex … <value>` apply line.
#[cfg(any(windows, test))]
fn power_cmd_target(apply: &str) -> Option<u32> {
    let mut toks = apply.split_whitespace();
    if !toks.next()?.eq_ignore_ascii_case("powercfg")
        || !toks.next()?.eq_ignore_ascii_case("/setacvalueindex")
    {
        return None;
    }
    toks.last()?.parse().ok()
}

/// Apply a tweak: backup → write-ahead journal → mutate. Returns the journal
/// entry id so the UI can offer instant undo.
pub fn apply(tweak_id: &str) -> Result<Value, String> {
    let _engine = engine_guard()?;
    let t = catalog()
        .into_iter()
        .find(|t| t.id == tweak_id)
        .ok_or_else(|| format!("unknown tweak '{tweak_id}'"))?;
    if crate::optregistry::is_retired(t.id) {
        return Err(format!(
            "'{}' was retired because it has no measurable benefit. Only undo is still available.",
            t.name
        ));
    }
    if t.requires_admin && !crate::ps::is_admin() {
        return Err("This tweak needs administrator rights. Restart the app as admin.".into());
    }

    // 0. A second apply while an earlier one is still open would capture the
    //    tweak's OWN values as "previous" and make undo restore the tweak
    //    instead of the original state. Keep the oldest capture.
    //    If the recorded change is no longer in effect (reset by Windows, a
    //    driver reinstall or the user), reporting success would be false and
    //    re-capturing would lose the original: the earlier entry must be
    //    undone first.
    //    Only a fully active earlier apply counts as success; a partial one
    //    (some values reset since) would report success for settings that
    //    are not in effect.
    if let Some(open) = open_entry_id(&safety::load_journal()?, t.id) {
        match detect_status(&t) {
            "applied" => return Ok(json!({ "entryId": open, "status": "already_applied" })),
            "partial" => {
                return Err(format!(
                    "'{}' was applied earlier but is only partly in effect now. Undo the \
                     earlier change first (it restores the original values), then apply again.",
                    t.name
                ))
            }
            _ => {}
        }
        return Err(format!(
            "'{}' was applied earlier but is no longer in effect. Undo the earlier change \
             first (it restores the original values), then apply again.",
            t.name
        ));
    }

    // 1. Registry backups for every key we will touch. A missing key needs no
    //    backup (the apply creates it); that is decided via the registry API,
    //    never by parsing reg.exe's localized stderr. Any other failure (full
    //    disk, denied reg.exe) must not silently skip the backup stage.
    let mut backups = Vec::new();
    for a in &t.actions {
        if let Action::RegSet { root, path, .. } = a {
            #[cfg(windows)]
            if !reg_key_exists(root, path)? {
                continue;
            }
            backups.push(
                safety::backup_registry_key(root, path)
                    .map_err(|e| format!("registry backup failed: {e}"))?,
            );
        }
    }

    // 2. Capture previous state.
    let mut items = Vec::new();
    for a in &t.actions {
        match a {
            #[cfg(windows)]
            Action::RegSet {
                root,
                path,
                name,
                value,
            } => items.push(ChangeItem::Registry {
                root: root.to_string(),
                path: path.to_string(),
                name: name.to_string(),
                prev: reg_read(root, path, name)?,
                new: value.clone(),
            }),
            #[cfg(not(windows))]
            Action::RegSet { .. } => return Err("Windows only".into()),
            Action::Service { name, target } => items.push(ChangeItem::ServiceStartup {
                service: name.to_string(),
                prev: service_get_start(name)?,
                new: target.to_string(),
            }),
            Action::Cmd {
                apply,
                revert,
                show,
            } => {
                let revert_actual = match show {
                    #[cfg(windows)]
                    Some(cap) => capture_revert(cap).ok_or_else(|| {
                        // A wrong revert is worse than no apply: without the
                        // pre-apply state the undo would restore a guess.
                        format!(
                            "could not capture the previous state for an exact revert; \
                             '{}' was NOT applied and nothing was changed",
                            t.name
                        )
                    })?,
                    #[cfg(not(windows))]
                    Some(_) => revert.to_string(),
                    None => revert.to_string(),
                };
                items.push(ChangeItem::Command {
                    applied: apply.to_string(),
                    revert: revert_actual,
                });
            }
        }
    }

    // 3. Write-ahead journal entry (append returns the ACTUAL id: collisions
    //    with a same-second apply get a suffix so undo tokens stay unique).
    //    completed=false until every action succeeded; an interrupted apply
    //    must never count as applied.
    let entry_id = format!(
        "{}-{}",
        t.id,
        chrono::Local::now().format("%Y%m%d%H%M%S%6f")
    );
    let entry_id = safety::append_entry(JournalEntry {
        id: entry_id,
        tweak_id: t.id.to_string(),
        tweak_name: t.name.to_string(),
        origin: None,
        time: chrono::Local::now().to_rfc3339(),
        items: items.clone(),
        reverted: false,
        backup_files: backups,
        completed: false,
        attempted: 0,
        reverted_items: 0,
        untrusted: false,
    })?;

    // 4. Apply. On failure, roll back everything up to and including the
    //    failing action (a killed multi-interface command can already have
    //    written some keys). The journal entry is only marked reverted when
    //    the rollback fully succeeded; otherwise it stays open so the UI can
    //    offer a retry.
    for (idx, item) in items.iter().enumerate() {
        safety::with_journal(|j| {
            let en = j
                .iter_mut()
                .find(|en| en.id == entry_id)
                .ok_or("journal entry missing")?;
            en.attempted = idx + 1;
            Ok(())
        })?;
        if let Err(e) = apply_item(item) {
            let blocked = windows_blocked_hint(item, &e);
            return match rollback_entry_items(&entry_id, &items[..=idx]) {
                Ok(()) if blocked.is_some() => Err(blocked.unwrap_or_default()),
                Ok(()) => Err(format!("apply failed ({e}); all changes were rolled back")),
                Err(re) => Err(format!(
                    "apply failed ({e}); rollback incomplete ({re}); restore token: {entry_id}"
                )),
            };
        }
    }

    safety::with_journal(|j| {
        let en = j
            .iter_mut()
            .find(|en| en.id == entry_id)
            .ok_or("journal entry missing")?;
        en.completed = true;
        Ok(())
    })?;

    Ok(json!({ "entryId": entry_id, "status": "applied" }))
}

/// Id of the newest unreverted entry for `tweak_id` that actually changed
/// something (same "undoable" rule the UI uses). An entry interrupted before
/// its first mutation (attempted == 0, not completed) does not block.
pub(crate) fn open_entry_id(journal: &[JournalEntry], tweak_id: &str) -> Option<String> {
    journal
        .iter()
        .rev()
        .find(|e| e.tweak_id == tweak_id && !e.reverted && (e.completed || e.attempted > 0))
        .map(|e| e.id.clone())
}

/// Values Windows' UserChoice Protection Driver (UCPD) blocks for every
/// program that is not Microsoft-signed, admin or not (seen on 24H2/25H2):
/// the write fails with "access denied" although the key ACL allows it.
/// Group Policy can still set them. Returns the user-facing message.
fn windows_blocked_hint(item: &ChangeItem, err: &str) -> Option<String> {
    let ChangeItem::Registry { path, name, .. } = item else {
        return None;
    };
    if !err.contains("os error 5") {
        return None;
    }
    match (path.to_ascii_lowercase().as_str(), name.as_str()) {
        (r"software\policies\microsoft\dsh", "AllowNewsAndInterests") => Some(
            "Windows blocks this setting for apps (UCPD protection), so nothing was changed. Set it with Group Policy instead: gpedit.msc > Computer Configuration > Administrative Templates > Windows Components > Widgets > Allow Widgets = Disabled.".into(),
        ),
        (r"software\microsoft\windows\currentversion\explorer\advanced", "TaskbarDa") => Some(
            "Windows blocks this setting for apps (UCPD protection), so nothing was changed. Turn it off in Settings > Personalization > Taskbar > Widgets.".into(),
        ),
        _ => None,
    }
}

pub(crate) fn apply_item(item: &ChangeItem) -> Result<(), String> {
    match item {
        #[cfg(windows)]
        ChangeItem::Registry {
            root,
            path,
            name,
            new,
            ..
        } => reg_write(root, path, name, new),
        #[cfg(not(windows))]
        ChangeItem::Registry { .. } => Err("Windows only".into()),
        ChangeItem::ServiceStartup { service, new, .. } => service_set_start(service, new),
        ChangeItem::Command { applied, .. } => run_cmdline(applied),
    }
}

pub(crate) fn revert_item(item: &ChangeItem) -> Result<(), String> {
    match item {
        #[cfg(windows)]
        ChangeItem::Registry {
            root,
            path,
            name,
            prev,
            ..
        } => match prev {
            Some(v) => reg_write(root, path, name, v),
            // reg_delete itself treats NotFound as success; other failures
            // propagate so an undo never lies about having restored state.
            None => reg_delete(root, path, name),
        },
        #[cfg(not(windows))]
        ChangeItem::Registry { .. } => Err("Windows only".into()),
        ChangeItem::ServiceStartup { service, prev, .. } => service_set_start(service, prev),
        // bcdedit fails on `/deletevalue` of an absent element: an element
        // already in its recorded previous state needs no command.
        ChangeItem::Command { revert, .. } if crate::bootopt::revert_is_noop(revert) => Ok(()),
        ChangeItem::Command { revert, .. } => run_cmdline(revert),
    }
}

/// Record each successful reverse step so a later Undo resumes after a partial rollback.
pub(crate) fn rollback_entry_items(entry_id: &str, items: &[ChangeItem]) -> Result<(), String> {
    for item in items.iter().rev() {
        revert_item(item)?;
        safety::with_journal(|journal| {
            let entry = journal
                .iter_mut()
                .find(|entry| entry.id == entry_id)
                .ok_or("journal entry missing")?;
            entry.reverted_items += 1;
            Ok(())
        })?;
    }
    safety::with_journal(|journal| {
        let entry = journal
            .iter_mut()
            .find(|entry| entry.id == entry_id)
            .ok_or("journal entry missing")?;
        entry.reverted = true;
        Ok(())
    })
}

/// Undo the most recent non-reverted journal entry for a tweak.
/// If no journal entry exists but the tweak is currently active (detected via
/// live registry / service state), falls back to a synthesized force-revert:
///   • RegSet  → imports the newest .reg backup, then verifies the value changed
///   • Service → resets to the known Windows default startup type
///   • Cmd     → only uncaptured helper commands; captured ones need the journal
/// Tweaks with no live signal ("unknown") or a captured Cmd return an error:
/// without a journal entry the previous value is unknown.
pub fn revert(tweak_id: &str) -> Result<Value, String> {
    let _engine = engine_guard()?;
    // ── 1. Journal path (precise: uses saved previous values + reg backups) ──
    // Find, execute and mark atomically: a concurrent revert of the same
    // entry sees it as already-reverted and does nothing.
    let outcome = safety::with_journal(|journal| {
        let entry = match journal
            .iter_mut()
            .rev()
            .find(|e| e.tweak_id == tweak_id && !e.reverted)
        {
            Some(e) => e,
            None => return Ok(None),
        };
        // Resume semantics: reverted_items counts already-restored entries
        // from the END of the list, so a retry continues exactly where the
        // previous attempt stopped instead of re-running finished steps.
        let attempted = entry.attempted.min(entry.items.len());
        let already = entry.reverted_items.min(attempted);
        let stop = attempted - already;
        if refuse_untrusted(entry, stop).is_err() {
            // Not run; the catalog's Windows defaults are used instead below.
            return Ok(Some(Err(entry.id.clone())));
        }
        for offset in (0..stop).rev() {
            match revert_item(&entry.items[offset]) {
                Ok(()) => entry.reverted_items += 1,
                Err(e) => return Err(format!("partial revert: {e} — retry to resume")),
            }
        }
        entry.reverted = true;
        Ok(Some(Ok(entry.id.clone())))
    })?;
    match outcome {
        Some(Ok(id)) => return Ok(json!({ "entryId": id, "status": "reverted" })),
        Some(Err(untrusted_id)) => return revert_untrusted(&untrusted_id, tweak_id),
        None => {}
    }

    // ── 2. No usable journal entry: restore the Windows default ──
    force_revert(tweak_id)
}

/// What a journal-less undo writes for one catalog registry value: the
/// documented Windows default. `Keep` leaves a value alone because another
/// value already switches the setting off (FSE, see `fse_status`).
#[derive(Debug, Clone, PartialEq)]
enum WinDefault {
    Set(RegVal),
    Delete,
    Keep,
}

/// Windows defaults of every registry value the catalog writes. A test
/// keeps this table complete, so a journal-less undo never has to guess.
fn windows_default(path: &str, name: &str) -> Option<WinDefault> {
    use WinDefault::*;
    let d = |v: u32| Set(RegVal::Dword(v));
    let sz = |v: &str| Set(RegVal::Str(v.into()));
    // Policy values: "not configured" (absent) is the Windows default.
    if path
        .to_ascii_lowercase()
        .starts_with("software\\policies\\")
    {
        return Some(Delete);
    }
    Some(match name {
        "GameDVR_Enabled" => d(1),
        "AppCaptureEnabled" => Delete,
        "SystemResponsiveness" => d(20),
        "NetworkThrottlingIndex" => d(10),
        "StartupDelayInMSec" => Delete,
        "VisualFXSetting" => d(0),
        // "Let Windows choose" mask of a default profile.
        "UserPreferencesMask" => Set(RegVal::Raw {
            ty: "REG_BINARY".into(),
            hex: "9e1e078012000000".into(),
        }),
        "Enabled" if path.ends_with("AdvertisingInfo") => d(1),
        "MenuShowDelay" => sz("400"),
        // Memory integrity on: the default of current clean installs on
        // supported hardware.
        "Enabled" if path.ends_with("HypervisorEnforcedCodeIntegrity") => d(1),
        "SubscribedContent-338389Enabled"
        | "SoftLandingEnabled"
        | "RotatingLockScreenOverlayEnabled"
        | "SubscribedContent-338387Enabled" => d(1),
        "GlobalUserDisabled" => Delete,
        "MouseSpeed" => sz("1"),
        "MouseThreshold1" => sz("6"),
        "MouseThreshold2" => sz("10"),
        "AllowAutoGameMode" | "AutoGameModeEnabled" => Delete,
        "GameDVR_FSEBehaviorMode" | "GameDVR_EFSEFeatureFlags" => Keep,
        "GameDVR_HonorUserFSEBehaviorMode" => d(0),
        "HwSchMode" => Delete,
        "Win32PrioritySeparation" => d(2),
        "PowerThrottlingOff" => Delete,
        "Priority" if path.ends_with("Tasks\\Games") => d(2),
        "Scheduling Category" => sz("Medium"),
        "OverlayTestMode" => Delete,
        "ShowStartupPanel" | "UseNexusForGameBarEnabled" => Delete,
        "EnableTransparency" => d(1),
        "MinAnimate" => sz("1"),
        "TaskbarDa" => d(1),
        "Disabled" if path.ends_with("Windows Error Reporting") => Delete,
        "HiberbootEnabled" => d(1),
        "DisablePagingExecutive" => d(0),
        "NtfsDisableLastAccessUpdate" => d(0x8000_0002),
        "NtfsDisable8dot3NameCreation" => d(2),
        "IsDynamicSearchBoxEnabled" => d(1),
        "ShellFeedsTaskbarViewMode" => Delete,
        "DisableAutoplay" => d(0),
        "TCPTimedWaitDelay" | "IRPStackSize" => Delete,
        _ => return None,
    })
}

/// Undo without a journal entry (applied by another tool or an older
/// version, or a journal entry this app cannot verify). Every value goes
/// back to the newest registry backup when that really holds a different
/// value, otherwise to the documented Windows default; the key is exported
/// first. Captured commands run their catalog revert, which restores the
/// Windows default (Balanced plan, hibernation on, …), not an earlier
/// custom value; the result says so.
fn force_revert(tweak_id: &str) -> Result<Value, String> {
    let t = catalog()
        .into_iter()
        .find(|t| t.id == tweak_id)
        .ok_or_else(|| format!("unknown tweak '{tweak_id}'"))?;

    match detect_status(&t) {
        "not_applied" => return Err("tweak is not currently active, nothing to undo".into()),
        "unsupported" => {
            return Err("this tweak does not apply to this system, nothing to undo".into())
        }
        _ => {}
    }
    if t.requires_admin && !crate::ps::is_admin() {
        return Err(
            "Undoing this tweak needs administrator rights. Restart the app as admin.".into(),
        );
    }

    let mut errs = Vec::new();
    let mut used_default = false;
    for a in t.actions.iter().rev() {
        let res: Result<(), String> = match a {
            #[cfg(windows)]
            Action::RegSet {
                root,
                path,
                name,
                value,
            } => (|| {
                if !matches!(reg_read(root, path, name)?, Some(ref cur) if vals_eq(cur, value)) {
                    return Ok(()); // this value is not (or no longer) the tweak's
                }
                if let Some(backup) = safety::latest_backup_for(root, path) {
                    let _ = crate::ps::exec("reg.exe", &["import", &backup]);
                    if !matches!(reg_read(root, path, name)?, Some(ref cur) if vals_eq(cur, value))
                    {
                        return Ok(()); // the backup held the earlier value
                    }
                }
                let default = windows_default(path, name).ok_or_else(|| {
                    format!("no known Windows default for {root}\\{path}\\{name}")
                })?;
                if default == WinDefault::Keep {
                    return Ok(());
                }
                safety::backup_registry_key(root, path)
                    .map_err(|e| format!("registry backup failed, {name} unchanged: {e}"))?;
                used_default = true;
                match default {
                    WinDefault::Set(v) => reg_write(root, path, name, &v),
                    _ => reg_delete(root, path, name),
                }
            })(),
            #[cfg(not(windows))]
            Action::RegSet { .. } => Ok(()),
            Action::Service { name, .. } => match service_default(name) {
                Some(default) => {
                    used_default = true;
                    service_set_start(name, default)
                }
                None => Err(format!("no known Windows default for service '{name}'")),
            },
            Action::Cmd { revert, show, .. } => {
                used_default |= show.is_some();
                run_cmdline(revert)
            }
        };
        if let Err(e) = res {
            errs.push(e);
        }
    }

    if !errs.is_empty() {
        return Err(format!("undo incomplete: {}", errs.join("; ")));
    }
    match detect_status(&t) {
        "applied" | "partial" => Err(
            "the values were reset, but Windows still reports the setting as active \
             (a policy or another tool may set it again)"
                .into(),
        ),
        _ => Ok(
            json!({ "status": "force_reverted", "journaled": false, "windowsDefault": used_default }),
        ),
    }
}

/// Undo one specific journal entry by its id, the "restore token" handed
/// back by any apply-style call (e.g. Quick Boost). Unlike `revert(tweak_id)`,
/// which always targets the *latest* entry for a tweak id, this targets an
/// exact entry, so callers that can have multiple concurrent instances active
/// (Quick Boost on two different games at once) each get an independent,
/// unambiguous undo handle.
pub fn revert_entry(entry_id: &str) -> Result<Value, String> {
    let _engine = engine_guard()?;
    let outcome = safety::with_journal(|journal| {
        let entry = journal
            .iter_mut()
            .find(|e| e.id == entry_id)
            .ok_or("unknown restore token")?;
        if entry.reverted {
            return Err("already reverted".into());
        }
        // Only undo what apply actually attempted: an interrupted entry has
        // items that were never dispatched, and running their revert commands
        // would modify state the apply never touched. Same resume semantics
        // as revert(): reverted_items counts restored entries from the END.
        let attempted = entry.attempted.min(entry.items.len());
        let already = entry.reverted_items.min(attempted);
        let stop = attempted - already;
        if let Err(refusal) = refuse_untrusted(entry, stop) {
            return Ok(Err((entry.tweak_id.clone(), refusal)));
        }
        for offset in (0..stop).rev() {
            match revert_item(&entry.items[offset]) {
                Ok(()) => entry.reverted_items += 1,
                Err(e) => return Err(format!("partial revert: {e} — retry to resume")),
            }
        }
        entry.reverted = true;
        Ok(Ok(entry.id.clone()))
    })?;
    match outcome {
        Ok(id) => Ok(json!({ "entryId": id, "status": "reverted" })),
        // An unverifiable entry of a catalog or GPU tweak is undone with the
        // catalog's Windows defaults instead of its recorded steps.
        Err((tweak_id, refusal)) => {
            let known = catalog().iter().any(|t| t.id == tweak_id) || tweak_id.starts_with("gpu:");
            if known {
                revert_untrusted(entry_id, &tweak_id)
            } else {
                Err(refusal)
            }
        }
    }
}

/// Marks a journal entry as done without running it.
pub(crate) fn close_entry(entry_id: &str) -> Result<(), String> {
    safety::with_journal(|journal| {
        if let Some(e) = journal.iter_mut().find(|e| e.id == entry_id) {
            e.reverted = true;
        }
        Ok(())
    })
}

/// Undo for an entry whose recorded steps cannot be verified: the catalog
/// defaults are applied (they do not come from the untrusted file), then the
/// entry is closed. "Not active" also closes it: there is nothing left to do.
fn revert_untrusted(entry_id: &str, tweak_id: &str) -> Result<Value, String> {
    let res = match tweak_id.strip_prefix("gpu:") {
        Some(gpu_id) => crate::gputweaks::reset_to_defaults(gpu_id)
            .map(|m| json!({ "status": "force_reverted", "message": m })),
        None => force_revert(tweak_id),
    };
    match res {
        Ok(v) => {
            close_entry(entry_id)?;
            Ok(v)
        }
        Err(e) if e.contains("not currently active") => {
            close_entry(entry_id)?;
            Ok(json!({ "status": "already_default" }))
        }
        Err(e) => Err(e),
    }
}

pub fn history() -> Value {
    safety::load_journal()
        .map(|j| serde_json::to_value(j).unwrap_or(Value::Null))
        .unwrap_or_else(|e| json!({ "error": e }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn ucpd_blocked_values_get_a_clear_message() {
        let item = |path: &str, name: &str| ChangeItem::Registry {
            root: "HKLM".into(),
            path: path.into(),
            name: name.into(),
            prev: None,
            new: RegVal::Dword(0),
        };
        let denied = "set AllowNewsAndInterests: Zugriff verweigert (os error 5)";
        let dsh = item(
            "SOFTWARE\\Policies\\Microsoft\\Dsh",
            "AllowNewsAndInterests",
        );
        assert!(super::windows_blocked_hint(&dsh, denied)
            .unwrap()
            .contains("gpedit.msc"));
        assert!(super::windows_blocked_hint(&dsh, "set: other error").is_none());
        let other = item(
            "SOFTWARE\\Policies\\Microsoft\\Windows\\DataCollection",
            "AllowTelemetry",
        );
        assert!(super::windows_blocked_hint(&other, denied).is_none());
    }

    #[test]
    fn hibernation_revert_is_captured_without_the_registry_value() {
        use super::hibernate_revert;
        assert_eq!(
            hibernate_revert(Some(&RegVal::Dword(1)), false).as_deref(),
            Some("powercfg /h on")
        );
        assert_eq!(
            hibernate_revert(Some(&RegVal::Dword(0)), true).as_deref(),
            Some("powercfg /h off")
        );
        // Value absent (Windows default): the hibernation file decides.
        assert_eq!(
            hibernate_revert(None, true).as_deref(),
            Some("powercfg /h on")
        );
        assert_eq!(
            hibernate_revert(None, false).as_deref(),
            Some("powercfg /h off")
        );
        // An unexpected type is not guessed.
        assert_eq!(hibernate_revert(Some(&RegVal::Str("1".into())), true), None);
    }

    #[test]
    fn every_catalog_value_has_a_windows_default() {
        let mut missing = Vec::new();
        for t in catalog() {
            for a in &t.actions {
                if let Action::RegSet {
                    path, name, value, ..
                } = a
                {
                    match windows_default(path, name) {
                        None => missing.push(format!("{}: {path}\\{name} = {value:?}", t.id)),
                        // A default equal to the tweak value would make undo a no-op.
                        Some(d) => {
                            assert_ne!(d, WinDefault::Set(value.clone()), "{}: {name}", t.id)
                        }
                    }
                }
            }
        }
        assert!(
            missing.is_empty(),
            "no Windows default for:\n{}",
            missing.join("\n")
        );
        assert_eq!(
            windows_default("SOFTWARE\\Policies\\Microsoft\\Edge", "StartupBoostEnabled"),
            Some(WinDefault::Delete)
        );
        assert_eq!(windows_default("x", "unknown"), None);
    }

    use super::*;

    #[test]
    fn situational_tweaks_are_marked_high_risk() {
        for id in [
            "core_parking_off",
            "power_throttling_off",
            "usb_suspend_off",
            "sysmain_off",
            "wsearch_manual",
            "mpo_off",
        ] {
            let t = catalog().into_iter().find(|t| t.id == id).unwrap();
            assert_eq!(t.risk, "High", "{id}");
        }
    }

    #[test]
    fn delayed_auto_services_default_to_delayed_start() {
        assert_eq!(service_default("MapsBroker"), Some(DELAYED_AUTO));
        assert_eq!(service_default("WSearch"), Some(DELAYED_AUTO));
        assert_eq!(service_default("SysMain"), Some("Automatic"));
        assert_eq!(service_default("NotInCatalog"), None);
    }

    #[test]
    fn status_classification_treats_absent_services_as_unsupported() {
        assert_eq!(classify_status(0, 0, 1), "unsupported");
        assert_eq!(classify_status(0, 0, 0), "unknown");
        assert_eq!(classify_status(2, 2, 1), "applied");
        assert_eq!(classify_status(2, 1, 0), "partial");
        assert_eq!(classify_status(1, 0, 2), "not_applied");
    }

    #[test]
    fn power_cmd_target_reads_the_ac_index() {
        assert_eq!(
            power_cmd_target("powercfg /setacvalueindex scheme_current 2a737441-1930-4402-8d77-b2bebba308a3 48e6b7a6-50f5-4782-a5d4-53bb8f07e226 0"),
            Some(0)
        );
        assert_eq!(
            power_cmd_target("powercfg /setacvalueindex scheme_current a b 100"),
            Some(100)
        );
        assert_eq!(power_cmd_target("powercfg /setactive scheme_current"), None);
        assert_eq!(power_cmd_target("powercfg /h off"), None);
    }

    #[test]
    fn power_tweaks_have_readable_targets() {
        for id in ["usb_suspend_off", "core_parking_off"] {
            let t = catalog().into_iter().find(|t| t.id == id).unwrap();
            assert!(t.actions.iter().any(|a| matches!(
                a,
                Action::Cmd { apply, show: Some(ShowCapture::PowerSettingIndex { .. }), .. }
                    if power_cmd_target(apply).is_some()
            )));
        }
    }

    fn entry(
        id: &str,
        tweak: &str,
        reverted: bool,
        completed: bool,
        attempted: usize,
    ) -> JournalEntry {
        JournalEntry {
            id: id.into(),
            tweak_id: tweak.into(),
            tweak_name: tweak.into(),
            origin: None,
            time: String::new(),
            items: vec![],
            reverted,
            backup_files: vec![],
            completed,
            attempted,
            reverted_items: 0,
            untrusted: false,
        }
    }

    #[test]
    fn open_entry_blocks_reapply_only_when_something_was_changed() {
        let j = vec![
            entry("a1", "a", true, true, 1),
            entry("b1", "b", false, false, 0),
            entry("c1", "c", false, false, 1),
            entry("a2", "a", false, true, 1),
        ];
        assert_eq!(open_entry_id(&j, "a").as_deref(), Some("a2"));
        // Interrupted before the first mutation: nothing to protect.
        assert_eq!(open_entry_id(&j, "b"), None);
        // Partially applied: its capture is the real original.
        assert_eq!(open_entry_id(&j, "c").as_deref(), Some("c1"));
        assert_eq!(open_entry_id(&j, "zzz"), None);
    }

    #[test]
    fn power_scheme_guid_is_extracted_locale_independent() {
        // English
        let out = "Power Scheme GUID: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c  (High performance)";
        assert_eq!(
            parse_power_scheme(out),
            Some("powercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c".into())
        );
        // German locale, header differs, GUID is the only reliable token
        let de = "Energieschema-GUID: 381b4222-f694-41f0-9685-ff5bb260df2e (Ausbalanciert)";
        assert_eq!(
            parse_power_scheme(de),
            Some("powercfg /setactive 381b4222-f694-41f0-9685-ff5bb260df2e".into())
        );
    }

    #[test]
    fn power_scheme_guid_rejects_malformed_output() {
        assert_eq!(parse_power_scheme("No scheme here"), None);
        assert_eq!(parse_power_scheme(""), None);
        assert_eq!(
            parse_power_scheme("GUID: 8c5e7fda-zzbf-4a96-9a85-a6e23a8c635c"),
            None
        );
    }

    #[test]
    fn tcp_autotuning_values_map_to_netsh_tokens() {
        for (out, expect) in [
            ("Normal", "normal"),
            ("Disabled", "disabled"),
            ("HighlyRestricted", "highlyrestricted"),
            ("Restricted", "restricted"),
            ("Experimental", "experimental"),
        ] {
            assert_eq!(
                parse_tcp_autotuning(out),
                Some(format!("netsh int tcp set global autotuninglevel={expect}"))
            );
        }
        assert_eq!(parse_tcp_autotuning("SomethingElse"), None);
        assert_eq!(parse_tcp_autotuning(""), None);
    }

    #[test]
    fn fse_state_needs_honor_user_flag() {
        let d = |v| Some(RegVal::Dword(v));
        assert_eq!(fse_status(d(2), d(1)), "applied");
        // FSEBehaviorMode=2 without HonorUser=1 is the inert default.
        assert_eq!(fse_status(d(2), d(0)), "not_applied");
        assert_eq!(fse_status(d(2), None), "not_applied");
        assert_eq!(fse_status(d(0), d(1)), "not_applied");
        assert_eq!(fse_status(None, None), "not_applied");
    }

    #[test]
    fn game_mode_missing_value_means_windows_default_on() {
        assert_eq!(game_mode_status(None), "applied");
        assert_eq!(game_mode_status(Some(RegVal::Dword(0))), "not_applied");
        assert_eq!(game_mode_status(Some(RegVal::Dword(1))), "applied");
    }

    #[test]
    fn live_flag_needs_a_single_value() {
        assert_eq!(live_flag("Normal\r\n", "Normal"), Some(true));
        assert_eq!(live_flag("Disabled", "Normal"), Some(false));
        assert_eq!(live_flag("", "Normal"), None);
        assert_eq!(live_flag("a\nb", "Normal"), None);
    }

    #[test]
    fn quoted_legacy_scripts_are_unwrapped_once() {
        assert_eq!(unwrap_quoted_script("\"exit 3\""), "exit 3");
        assert_eq!(unwrap_quoted_script("exit 3"), "exit 3");
        // Inner quotes mean the outer pair is not a plain wrapper.
        assert_eq!(unwrap_quoted_script("\"a\" + \"b\""), "\"a\" + \"b\"");
    }

    #[test]
    fn nagle_revert_is_unquoted_and_rejects_non_numeric_captures() {
        let guid = "{12345678-1234-1234-1234-123456789abc}";
        let cmd = build_nagle_revert(&format!("{guid}|A|0")).unwrap();
        assert!(cmd.starts_with("powershell -Command "));
        assert!(
            !cmd.contains("-Command \""),
            "script must not be a string literal: {cmd}"
        );
        assert!(cmd.contains("-Value 0 -Type DWord"));
        assert_eq!(build_nagle_revert(&format!("{guid}|A|0;calc")), None);
    }

    /// Regression for the silent no-op undo: a quoted `-Command` script used
    /// to be echoed by PowerShell with exit 0. If it really executes, the
    /// non-zero exit surfaces as an error.
    #[cfg(windows)]
    #[test]
    fn run_cmdline_executes_quoted_and_unquoted_scripts() {
        assert!(run_cmdline("powershell -Command \"exit 3\"").is_err());
        assert!(run_cmdline("powershell -Command exit 3").is_err());
        assert!(run_cmdline("powershell -Command exit 0").is_ok());
    }

    /// A refused Set-Service surfaces as one readable line, not PowerShell's
    /// error record. Only meaningful unelevated, where Set-Service is always
    /// denied; the target is the service's current mode, so nothing changes
    /// even if the call were allowed.
    #[cfg(windows)]
    #[test]
    fn denied_service_change_reports_access_denied_cleanly() {
        if crate::ps::is_admin() {
            return;
        }
        let mode = service_get_start("Spooler").unwrap();
        if !matches!(mode.as_str(), "Automatic" | "Manual" | "Disabled") {
            return;
        }
        let err = service_set_start("Spooler", &mode).unwrap_err();
        assert!(
            err.starts_with("Windows denied access to service 'Spooler'"),
            "{err}"
        );
        assert!(!err.contains("CategoryInfo"), "{err}");
    }
}
