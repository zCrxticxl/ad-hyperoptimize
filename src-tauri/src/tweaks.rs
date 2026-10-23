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
        /// default). Falls back to `revert` when the capture fails.
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
            let prev = reg_read(
                "HKLM",
                "SYSTEM\\CurrentControlSet\\Control\\Power",
                "HibernateEnabled",
            )
            .ok()
            .flatten()?;
            match prev {
                RegVal::Dword(1) => Some("powercfg /h on".into()),
                RegVal::Dword(0) => Some("powercfg /h off".into()),
                _ => None,
            }
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
                // One PowerShell wrapper so run_cmdline executes all three
                // steps in order and aborts on the first failure; the final
                // setactive makes the restored indexes effective immediately.
                format!(
                    "powershell -Command powercfg /setacvalueindex {guid} {subgroup} {setting} {ac}; \
                     if ($LASTEXITCODE -eq 0) {{ powercfg /setdcvalueindex {guid} {subgroup} {setting} {dc}; \
                     if ($LASTEXITCODE -eq 0) {{ powercfg /setactive scheme_current }} }}; \
                     exit $LASTEXITCODE"
                )
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
            rationale: "Balanced plan downclocks cores aggressively; High Performance keeps clocks ready, reducing latency spikes in games and interactive workloads.",
            impact: "Lower input/scheduling latency; higher idle power draw (relevant on laptops).",
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
            rationale: "Background capture continuously encodes frames, costing GPU/CPU time and occasionally introducing frametime spikes.",
            impact: "Measurable frametime stability gain on weaker GPUs.",
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
            impact: "Smoother frametimes under load; slightly less CPU reserved for background services.",
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
            actions: vec![Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\VisualEffects", name: "VisualFXSetting", value: RegVal::Dword(2) }],
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
            impact: "Lower input lag in fullscreen games on affected systems; Alt-Tab becomes slower.",
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
            rationale: "Offloads frame scheduling from CPU to a dedicated GPU scheduler, reduces latency on modern GPUs (NVIDIA 10-series+/AMD RDNA+), and is required for DLSS 3 Frame Generation.",
            impact: "Slightly lower latency on modern GPUs. Requires reboot. On very old GPUs/drivers it can hurt, benchmark before/after.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers", name: "HwSchMode", value: RegVal::Dword(2) }],
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
            rationale: "Blur effects permanently consume GPU time (DWM), measurable on iGPUs and weaker cards.",
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
            impact: "Boot ~5-15s slower, but a fresh kernel state every time. Hibernate itself remains available.",
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
            impact: "Faster file creation in large directories; no 8.3 names for newly created files.",
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
    crate::ps::run(&format!(
        "$s = Get-Service -Name '{name}' -ErrorAction SilentlyContinue; if (-not $s) {{ 'ABSENT' }} else {{ Set-Service -Name '{name}' -StartupType {mode} -ErrorAction Stop; 'OK' }}"
    ))
    .map(|_| ())
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

/// Tweaks with a plausible, measurable performance effect. The UI's beginner
/// mode shows only these. Situational/advanced tweaks (risk "High": core
/// parking, power throttling, USB suspend, SysMain, WSearch, MPO) and
/// tweaks with no measurable gain stay available in expert mode only.
fn may_help_performance(id: &str) -> bool {
    matches!(
        id,
        "power_high_performance"
            | "game_dvr_off"
            | "mmcss_gaming"
            | "startup_delay_off"
            | "game_mode_on"
            | "fse_optimizations_off"
            | "hags_on"
            | "mmcss_games_task"
            | "transparency_off"
            | "min_animate_off"
            | "widgets_off"
            | "ntfs_8dot3_off"
            | "edge_prelaunch_off"
            | "svc_maps_off"
    )
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
            // canUndo: true when we have a journal entry (precise restore with
            // saved previous values) OR when live detection confirms the tweak
            // is currently active AND a force-revert would not have to guess
            // (covers "app reinstalled / journal lost" for RegSet + Service
            // tweaks). Captured Cmd tweaks always require the journal.
            let can_undo = undoable || (live == "applied" && force_revertable(t));
            json!({
                "id": t.id, "name": t.name, "category": t.category,
                "description": t.description, "rationale": t.rationale,
                "impact": t.impact, "risk": t.risk,
                "performancePotential": may_help_performance(t.id),
                "requiresAdmin": t.requires_admin, "reversible": t.reversible,
                "status": status, "undoable": undoable, "canUndo": can_undo,
            })
        })
        .collect();
    json!(out)
}

/// False when a journal-less revert would have to run a hardcoded guess:
/// Cmd actions with a pre-apply capture only have an exact undo in the
/// journal (the catalog `revert` is a default, not the user's prior state).
fn force_revertable(t: &Tweak) -> bool {
    !t.actions
        .iter()
        .any(|a| matches!(a, Action::Cmd { show: Some(_), .. }))
}

#[cfg_attr(not(windows), allow(unused_variables))]
fn detect_status(t: &Tweak) -> &'static str {
    #[cfg(windows)]
    {
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
        return classify_status(checkable, matching, absent);
    }
    #[allow(unreachable_code)]
    "unknown"
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
    if t.requires_admin && !crate::ps::is_admin() {
        return Err("This tweak needs administrator rights. Restart the app as admin.".into());
    }

    // 0. A second apply while an earlier one is still open would capture the
    //    tweak's OWN values as "previous" and make undo restore the tweak
    //    instead of the original state. Keep the oldest capture.
    if let Some(open) = open_entry_id(&safety::load_journal()?, t.id) {
        return Ok(json!({ "entryId": open, "status": "already_applied" }));
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
            return match rollback_entry_items(&entry_id, &items[..=idx]) {
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
    if let Some(id) = safety::with_journal(|journal| {
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
        for offset in (0..stop).rev() {
            match revert_item(&entry.items[offset]) {
                Ok(()) => entry.reverted_items += 1,
                Err(e) => return Err(format!("partial revert: {e} — retry to resume")),
            }
        }
        entry.reverted = true;
        Ok(Some(entry.id.clone()))
    })? {
        return Ok(json!({ "entryId": id, "status": "reverted" }));
    }

    // ── 2. Force-revert path (no journal, synthesize from catalog + live state) ──
    let t = catalog()
        .into_iter()
        .find(|t| t.id == tweak_id)
        .ok_or_else(|| format!("unknown tweak '{tweak_id}'"))?;

    let live = detect_status(&t);
    match live {
        "not_applied" => return Err("tweak is not currently active, nothing to undo".into()),
        "unsupported" => {
            return Err("this tweak does not apply to this system, nothing to undo".into())
        }
        // No journal entry and no live signal: running the catalog's
        // hardcoded revert would impose a guessed default (e.g. Balanced
        // plan, rss=disabled), not the user's previous state.
        "unknown" => {
            return Err(
                "nothing recorded to undo: no journal entry exists for this tweak and its \
                 current state cannot be read back, so the previous value is unknown"
                    .into(),
            )
        }
        _ => {}
    }
    if !force_revertable(&t) {
        return Err(
            "nothing recorded to undo: the previous value of this setting was only stored in \
             the journal, which has no entry for it; refusing to guess a default"
                .into(),
        );
    }

    // live == "applied" | "partial" → undo each action using catalog defaults
    let mut errs = Vec::new();
    for a in t.actions.iter().rev() {
        let res: Result<(), String> = match a {
            #[cfg(windows)]
            Action::RegSet {
                root, path, name, ..
            } => {
                // Blind deletion would destroy values set by the user, Group
                // Policy or other tools. Restore from the newest .reg backup
                // when one exists; refuse when there is no backup and the
                // value is present. Only a genuinely absent value is a no-op.
                match safety::latest_backup_for(root, path) {
                    Some(backup) => crate::ps::exec("reg.exe", &["import", &backup]).map(|_| ()),
                    None => match reg_read(root, path, name) {
                        Ok(Some(_)) => Err(format!(
                            "no backup exists for {root}\\{path}\\{name}; refusing to delete \
                             a value the app cannot prove it created — restore it manually \
                             or re-apply the tweak and use journal Undo"
                        )),
                        other => other.map(|_| ()),
                    },
                }
            }
            #[cfg(not(windows))]
            Action::RegSet { .. } => Ok(()),
            Action::Service { name, .. } => match service_default(name) {
                Some(default) => service_set_start(name, default),
                None => Err(format!(
                    "no known Windows default for service '{name}'; refusing to guess"
                )),
            },
            Action::Cmd { revert, .. } => run_cmdline(revert),
        };
        if let Err(e) = res {
            errs.push(e);
        }
    }

    // A .reg import "succeeds" even when the backup was taken after the tweak
    // was applied (it then re-imports the tweak's own values). Only report
    // success when every value really differs from what the tweak writes.
    #[cfg(windows)]
    if errs.is_empty() {
        for a in &t.actions {
            if let Action::RegSet {
                root,
                path,
                name,
                value,
            } = a
            {
                match reg_read(root, path, name) {
                    Ok(Some(cur)) if vals_eq(&cur, value) => errs.push(format!(
                        "{root}\\{path}\\{name} still holds the tweak value after restoring \
                         the newest backup (the backup already contained it); the original \
                         value is unknown, so it was NOT restored"
                    )),
                    Ok(_) => {}
                    Err(e) => errs.push(format!("could not verify restore: {e}")),
                }
            }
        }
    }

    if errs.is_empty() {
        Ok(json!({ "status": "force_reverted", "journaled": false }))
    } else {
        Err(format!("force revert errors: {}", errs.join("; ")))
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
    let id = safety::with_journal(|journal| {
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
        for offset in (0..stop).rev() {
            match revert_item(&entry.items[offset]) {
                Ok(()) => entry.reverted_items += 1,
                Err(e) => return Err(format!("partial revert: {e} — retry to resume")),
            }
        }
        entry.reverted = true;
        Ok(entry.id.clone())
    })?;
    Ok(json!({ "entryId": id, "status": "reverted" }))
}

pub fn history() -> Value {
    safety::load_journal()
        .map(|j| serde_json::to_value(j).unwrap_or(Value::Null))
        .unwrap_or_else(|e| json!({ "error": e }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn performance_candidates_exclude_bonus_only_tweaks() {
        assert!(may_help_performance("mmcss_gaming"));
        assert!(may_help_performance("game_dvr_off"));
        assert!(!may_help_performance("qos_bandwidth_free"));
        assert!(!may_help_performance("advertising_id_off"));
        assert!(!may_help_performance("disable_paging_executive"));
    }

    #[test]
    fn no_gain_and_advanced_tweaks_are_not_performance_candidates() {
        for id in [
            "priority_separation",
            "nagle_off",
            "tcp_autotuning_normal",
            "net_rss_on",
            "xbox_services_off",
            "ntfs_last_access_off",
            "visual_fx_performance",
            "irp_stack_high",
            "tcp_timed_wait_low",
            "qos_bandwidth_free",
            "remote_registry_off",
            "svc_geolocation_off",
            "cortana_off",
            "background_apps_off",
            "disable_paging_executive",
            "core_parking_off",
            "power_throttling_off",
            "usb_suspend_off",
            "sysmain_off",
            "wsearch_manual",
            "mpo_off",
        ] {
            assert!(!may_help_performance(id), "{id}");
            // Every listed id must still exist: capability is kept.
            assert!(catalog().iter().any(|t| t.id == id), "{id} missing");
        }
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
            assert!(!force_revertable(&t));
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
}
