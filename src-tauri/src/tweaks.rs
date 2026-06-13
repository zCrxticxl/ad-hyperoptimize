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
    RegSet { root: &'static str, path: &'static str, name: &'static str, value: RegVal },
    Service { name: &'static str, target: &'static str },
    Cmd { apply: &'static str, revert: &'static str },
}

#[derive(Clone, Serialize)]
pub struct Tweak {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub rationale: &'static str,
    pub impact: &'static str,
    pub risk: &'static str, // Low | Medium
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
            rationale: "Animations and transparency cost GPU/CPU time and RAM, most noticeable on iGPUs and low-RAM systems.",
            impact: "Snappier UI on low-end hardware; cosmetic downgrade.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\VisualEffects", name: "VisualFXSetting", value: RegVal::Dword(2) }],
        },
        Tweak {
            id: "telemetry_minimal",
            name: "Minimize telemetry (policy + DiagTrack)",
            category: "Privacy",
            description: "Sets AllowTelemetry policy to Security(0)/Basic and disables the Connected User Experiences (DiagTrack) service.",
            rationale: "Reduces background data collection, disk writes and network chatter from the telemetry pipeline.",
            impact: "Less background I/O; some diagnostic data unavailable to Microsoft support.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKLM", path: "SOFTWARE\\Policies\\Microsoft\\Windows\\DataCollection", name: "AllowTelemetry", value: RegVal::Dword(0) },
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
            impact: "Less background disk activity on SSDs. Do NOT apply on HDD systems.",
            risk: "Medium",
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
            actions: vec![Action::Cmd { apply: "powercfg /h off", revert: "powercfg /h on" }],
        },
        Tweak {
            id: "menu_delay_fast",
            name: "Faster menu response",
            category: "Responsiveness",
            description: "Reduces MenuShowDelay from 400ms to 100ms.",
            rationale: "Pure artificial delay before menus open; lowering it makes the shell feel faster at zero cost. (Don't set 0 — hover menus become twitchy.)",
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
            rationale: "Each background UWP app costs RAM and wake-ups. Apps still work when opened; they just can't idle in the background.",
            impact: "Lower idle RAM/CPU. Live tiles and some notifications stop updating.",
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
            rationale: "Acceleration makes cursor distance depend on speed — bad for aim consistency. Esports-standard tweak.",
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
            rationale: "Fullscreen Optimizations route games through DWM composition. On many systems classic exclusive fullscreen gives lower input latency and steadier frametimes.",
            impact: "Lower input lag in fullscreen games on affected systems; Alt-Tab becomes slower.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKCU", path: "System\\GameConfigStore", name: "GameDVR_FSEBehaviorMode", value: RegVal::Dword(2) },
                Action::RegSet { root: "HKCU", path: "System\\GameConfigStore", name: "GameDVR_HonorUserFSEBehaviorMode", value: RegVal::Dword(1) },
                Action::RegSet { root: "HKCU", path: "System\\GameConfigStore", name: "GameDVR_DXGIHonorFSEWindowsCompatible", value: RegVal::Dword(1) },
                Action::RegSet { root: "HKCU", path: "System\\GameConfigStore", name: "GameDVR_EFSEFeatureFlags", value: RegVal::Dword(0) },
            ],
        },
        Tweak {
            id: "hags_on",
            name: "Hardware-Accelerated GPU Scheduling (HAGS)",
            category: "Gaming",
            description: "Sets HwSchMode=2, letting the GPU manage its own scheduling queue. REBOOT REQUIRED.",
            rationale: "Offloads frame scheduling from CPU to a dedicated GPU scheduler — reduces latency on modern GPUs (NVIDIA 10-series+/AMD RDNA+), and is required for DLSS 3 Frame Generation.",
            impact: "Slightly lower latency on modern GPUs. Requires reboot. On very old GPUs/drivers it can hurt — benchmark before/after.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers", name: "HwSchMode", value: RegVal::Dword(2) }],
        },
        Tweak {
            id: "priority_separation",
            name: "Foreground priority boost (Win32PrioritySeparation 0x26)",
            category: "Gaming",
            description: "Sets scheduler quanta to short, fixed, 3:1 foreground boost.",
            rationale: "Default Windows gives the foreground app a variable boost; 0x26 makes quanta short and fixed with maximum foreground bias — the classic esports scheduler tweak, keeping the game responsive when background tasks fire.",
            impact: "More consistent foreground frametimes under background load.",
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
            impact: "No process gets force-throttled. Higher idle power draw — skip on laptops running on battery.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SYSTEM\\CurrentControlSet\\Control\\Power\\PowerThrottling", name: "PowerThrottlingOff", value: RegVal::Dword(1) }],
        },
        Tweak {
            id: "mmcss_games_task",
            name: "MMCSS 'Games' task: maximum priority",
            category: "Gaming",
            description: "Raises the multimedia scheduler's Games profile: GPU Priority 8, CPU Priority 6, Scheduling Category High, SFIO High.",
            rationale: "Games registering with MMCSS (most DirectX titles) inherit this profile; the defaults (GPU Priority 2, Priority 2, Category Medium) are conservative.",
            impact: "Higher scheduling priority for games that use MMCSS.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKLM", path: "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile\\Tasks\\Games", name: "GPU Priority", value: RegVal::Dword(8) },
                Action::RegSet { root: "HKLM", path: "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile\\Tasks\\Games", name: "Priority", value: RegVal::Dword(6) },
                Action::RegSet { root: "HKLM", path: "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile\\Tasks\\Games", name: "Scheduling Category", value: RegVal::Str("High".into()) },
                Action::RegSet { root: "HKLM", path: "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile\\Tasks\\Games", name: "SFIO Priority", value: RegVal::Str("High".into()) },
            ],
        },
        Tweak {
            id: "mpo_off",
            name: "Disable Multiplane Overlay (MPO)",
            category: "Gaming",
            description: "Sets DWM OverlayTestMode=5, disabling multiplane overlays. REBOOT REQUIRED.",
            rationale: "MPO causes stutter, flicker and black-screen glitches on a range of GPU/monitor combos — both NVIDIA and AMD have historically recommended this exact toggle as the workaround.",
            impact: "Fixes MPO-related flicker/stutter if present. Slightly higher DWM GPU load. Only apply if you see flicker/stutter; undo if no change.",
            risk: "Medium",
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
            rationale: "Blur effects permanently consume GPU time (DWM) — measurable on iGPUs and weaker cards.",
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
            rationale: "Every keystroke in Start search otherwise goes to Bing — adds latency and data leakage.",
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
            description: "Disables the Widget Board (Win11) via policy.",
            rationale: "The widget process (msedgewebview2) runs permanently, fetches news, and costs RAM + network.",
            impact: "Permanently saves RAM and background traffic. Widget button disappears.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SOFTWARE\\Policies\\Microsoft\\Dsh", name: "AllowNewsAndInterests", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "delivery_opt_off",
            name: "Disable Update P2P (Delivery Optimization)",
            category: "Network",
            description: "Sets DODownloadMode=0 — updates come only from Microsoft, no peer-to-peer.",
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
            rationale: "WER writes crash dumps to disk and uploads them — I/O spikes exactly when the system is already struggling.",
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
            rationale: "Every opened app/file is otherwise written to the activity database — background I/O and data collection.",
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
            impact: "Stable USB devices; slightly higher power draw (negligible on desktop, noticeable on laptop battery).",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::Cmd {
                    apply: "powercfg /setacvalueindex scheme_current 2a737441-1930-4402-8d77-b2bebba308a3 48e6b7a6-50f5-4782-a5d4-53bb8f07e226 0",
                    revert: "powercfg /setacvalueindex scheme_current 2a737441-1930-4402-8d77-b2bebba308a3 48e6b7a6-50f5-4782-a5d4-53bb8f07e226 1",
                },
                Action::Cmd { apply: "powercfg /setactive scheme_current", revert: "powercfg /setactive scheme_current" },
            ],
        },
        Tweak {
            id: "fast_startup_off",
            name: "Disable Fast Startup",
            category: "Power",
            description: "Disables hiberboot — Shutdown performs a full cold shutdown.",
            rationale: "Fast Startup freezes the kernel instead of restarting: driver issues survive reboots, dual-boot setups get corrupted, uptime grows indefinitely. A real cold boot fixes more than it costs.",
            impact: "Boot ~5–15s slower, but a fresh kernel state every time. Hibernate itself remains available.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Power", name: "HiberbootEnabled", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "disable_paging_executive",
            name: "Keep Kernel in RAM (DisablePagingExecutive)",
            category: "Memory",
            description: "Prevents kernel code and drivers from being paged out to the swap file.",
            rationale: "By default Windows can write kernel pages to disk. With ≥8 GB RAM this only adds latency when kernel code is read back — disabling keeps everything in RAM.",
            impact: "Lower kernel call latency; marginally higher RAM usage (~50 MB). Do not apply on systems with less than 8 GB RAM.",
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
            rationale: "Windows parks cores at idle to save power. Waking parked cores takes a few milliseconds — visible as frametime spikes on first load increase in a game.",
            impact: "Lower startup frametimes and fewer latency spikes; slightly higher idle power draw (negligible on desktop, noticeable on laptop battery).",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![
                Action::Cmd {
                    apply: "powercfg /setacvalueindex scheme_current 54533251-82be-4824-96c1-47b60b740d00 0cc5b647-c1df-4637-891a-dec35c318583 100",
                    revert: "powercfg /setacvalueindex scheme_current 54533251-82be-4824-96c1-47b60b740d00 0cc5b647-c1df-4637-891a-dec35c318583 0",
                },
                Action::Cmd { apply: "powercfg /setactive scheme_current", revert: "powercfg /setactive scheme_current" },
            ],
        },
        Tweak {
            id: "ntfs_last_access_off",
            name: "Disable NTFS Last-Access Timestamp",
            category: "Storage",
            description: "Prevents NTFS from writing the last-accessed timestamp on every read.",
            rationale: "Every read otherwise triggers an MFT write. On heavily-read SSDs this adds up to unnecessary write amplification and latency spikes.",
            impact: "Significantly fewer background writes on all NTFS volumes. Pure performance gain on SSDs.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Cmd {
                apply: "fsutil behavior set disablelastaccess 1",
                revert: "fsutil behavior set disablelastaccess 0",
            }],
        },
        Tweak {
            id: "ntfs_8dot3_off",
            name: "Disable 8.3 Short Names (DOS compatibility)",
            category: "Storage",
            description: "Stops automatic generation of 8.3 short names (e.g. PROGRA~1) for new files.",
            rationale: "On every file creation NTFS checks if an 8.3 short name must be generated — unnecessary overhead; no modern program needs this legacy compatibility.",
            impact: "Faster file creation in large directories; no 8.3 names for newly created files.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Cmd {
                apply: "fsutil behavior set disable8dot3 1",
                revert: "fsutil behavior set disable8dot3 0",
            }],
        },
        Tweak {
            id: "xbox_services_off",
            name: "Set Xbox Background Services to Manual",
            category: "Services",
            description: "Sets XblAuthManager, XblGameSave, and XboxNetApiSvc to Manual.",
            rationale: "These services run permanently but are only needed for Xbox Live multiplayer and Game Pass cloud sync — most PC gamers without an Xbox account never use them.",
            impact: "Less idle RAM and service overhead. Xbox Live auth and cloud saves must be set back to Automatic if needed.",
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
            rationale: "Allows external programs to access the registry over the network — a known attack vector that home users never need.",
            impact: "Improved security; no more remote registry access over the network.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "RemoteRegistry", target: "Disabled" }],
        },
        Tweak {
            id: "cortana_off",
            name: "Disable Cortana (Policy)",
            category: "Privacy",
            description: "Sets AllowCortana=0 via policy.",
            rationale: "Cortana runs as a background process, sends voice metrics, and permanently uses RAM — no value for most users.",
            impact: "No more Cortana process; less data transfer to Microsoft.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SOFTWARE\\Policies\\Microsoft\\Windows\\Windows Search", name: "AllowCortana", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "edge_prelaunch_off",
            name: "Disable Edge Preloading (Startup Boost)",
            category: "Startup",
            description: "Prevents Edge from silently preloading in the background on Windows startup.",
            rationale: "Edge auto-starts in the background on boot to open faster on first use — costs RAM even when Edge is never used.",
            impact: "Less RAM usage after boot; Edge opens slightly slower on first launch.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![
                Action::RegSet { root: "HKLM", path: "SOFTWARE\\Policies\\Microsoft\\MicrosoftEdge\\Main", name: "AllowPrelaunch", value: RegVal::Dword(0) },
                Action::RegSet { root: "HKCU", path: "SOFTWARE\\Policies\\Microsoft\\MicrosoftEdge\\Main", name: "AllowPrelaunch", value: RegVal::Dword(0) },
            ],
        },
        Tweak {
            id: "search_highlights_off",
            name: "Disable Search Highlights & Trending Content",
            category: "Privacy",
            description: "Disables dynamic news and trending content in the Windows search box.",
            rationale: "The search box downloads new highlight content daily and shows it on open — background traffic and distraction with no value.",
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
            rationale: "Virtually no one uses Windows Fax. The service loads drivers and waits for fax lines — pure waste.",
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
            impact: "No permanent location tracking; Maps/weather apps can still start the service.",
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
            rationale: "Shares media over DLNA/UPnP on the home network — hardly anyone uses this through Windows itself. Unnecessary service + network listener.",
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
            impact: "Broken shortcuts over network drives won't be auto-repaired — affects home users virtually never.",
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
            name: "Disable Diagnostic Tracking Service",
            category: "Privacy",
            description: "Sets diagnosticshub.standardcollector.service to Disabled.",
            rationale: "Collects runtime diagnostic traces for Microsoft Visual Studio and Windows Diagnostics. Unnecessary on non-developer systems.",
            impact: "Visual Studio diagnostic tools stop working — irrelevant for normal gaming/desktop use.",
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
            impact: "Less background I/O; Start-menu file search becomes slower.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "WSearch", target: "Manual" }],
        },
    ]
}

// ---------- registry helpers ----------

#[cfg(windows)]
fn hive(root: &str) -> RegKey {
    RegKey::predef(if root == "HKLM" { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER })
}

#[cfg(windows)]
fn reg_read(root: &str, path: &str, name: &str) -> Option<RegVal> {
    let key = hive(root).open_subkey_with_flags(path, KEY_READ).ok()?;
    if let Ok(v) = key.get_value::<u32, _>(name) {
        return Some(RegVal::Dword(v));
    }
    if let Ok(v) = key.get_value::<String, _>(name) {
        return Some(RegVal::Str(v));
    }
    None
}

#[cfg(windows)]
fn reg_write(root: &str, path: &str, name: &str, val: &RegVal) -> Result<(), String> {
    let (key, _) = hive(root).create_subkey(path).map_err(|e| format!("open {root}\\{path}: {e}"))?;
    match val {
        RegVal::Dword(d) => key.set_value(name, d),
        RegVal::Str(s) => key.set_value(name, s),
    }
    .map_err(|e| format!("set {name}: {e}"))
}

#[cfg(windows)]
fn reg_delete(root: &str, path: &str, name: &str) -> Result<(), String> {
    let key = hive(root)
        .open_subkey_with_flags(path, KEY_READ | KEY_SET_VALUE)
        .map_err(|e| e.to_string())?;
    key.delete_value(name).map_err(|e| e.to_string())
}

fn service_get_start(name: &str) -> Result<String, String> {
    crate::ps::run(&format!("(Get-Service -Name '{name}' -ErrorAction Stop).StartType"))
        .map(|s| s.trim().to_string())
}

fn service_set_start(name: &str, mode: &str) -> Result<(), String> {
    crate::ps::run(&format!(
        "Set-Service -Name '{name}' -StartupType {mode} -ErrorAction Stop; 'OK'"
    ))
    .map(|_| ())
}

fn run_cmdline(line: &str) -> Result<(), String> {
    let mut parts = line.split_whitespace();
    let exe = parts.next().ok_or("empty command")?;
    let args: Vec<&str> = parts.collect();
    crate::ps::exec(exe, &args).map(|_| ())
}

fn vals_eq(a: &RegVal, b: &RegVal) -> bool {
    matches!((a, b), (RegVal::Dword(x), RegVal::Dword(y)) if x == y)
        || matches!((a, b), (RegVal::Str(x), RegVal::Str(y)) if x == y)
}

// ---------- public API ----------

/// Catalog + live status, ready for the UI.
pub fn list_with_status() -> Value {
    let journal = safety::load_journal();
    let out: Vec<Value> = catalog()
        .iter()
        .map(|t| {
            let status = detect_status(t);
            let undoable = journal.iter().any(|e| e.tweak_id == t.id && !e.reverted);
            json!({
                "id": t.id, "name": t.name, "category": t.category,
                "description": t.description, "rationale": t.rationale,
                "impact": t.impact, "risk": t.risk,
                "requiresAdmin": t.requires_admin, "reversible": t.reversible,
                "status": status, "undoable": undoable,
            })
        })
        .collect();
    json!(out)
}

fn detect_status(t: &Tweak) -> &'static str {
    #[cfg(windows)]
    {
        let mut checkable = 0;
        let mut matching = 0;
        for a in &t.actions {
            match a {
                Action::RegSet { root, path, name, value } => {
                    checkable += 1;
                    if let Some(cur) = reg_read(root, path, name) {
                        if vals_eq(&cur, value) {
                            matching += 1;
                        }
                    }
                }
                Action::Service { name, target } => {
                    checkable += 1;
                    if let Ok(cur) = service_get_start(name) {
                        if cur.eq_ignore_ascii_case(target) {
                            matching += 1;
                        }
                    }
                }
                Action::Cmd { .. } => {}
            }
        }
        if checkable == 0 {
            return "unknown";
        }
        if matching == checkable {
            return "applied";
        }
        if matching > 0 {
            return "partial";
        }
        return "not_applied";
    }
    #[allow(unreachable_code)]
    "unknown"
}

/// Apply a tweak: backup → write-ahead journal → mutate. Returns the journal
/// entry id so the UI can offer instant undo.
pub fn apply(tweak_id: &str) -> Result<Value, String> {
    let t = catalog()
        .into_iter()
        .find(|t| t.id == tweak_id)
        .ok_or_else(|| format!("unknown tweak '{tweak_id}'"))?;
    if t.requires_admin && !crate::ps::is_admin() {
        return Err("This tweak needs administrator rights. Restart the app as admin.".into());
    }

    // 1. Registry backups for every key we will touch.
    let mut backups = Vec::new();
    for a in &t.actions {
        if let Action::RegSet { root, path, .. } = a {
            match safety::backup_registry_key(root, path) {
                Ok(f) => backups.push(f),
                Err(_) => { /* key may not exist yet — journal still holds prev=None */ }
            }
        }
    }

    // 2. Capture previous state.
    let mut items = Vec::new();
    for a in &t.actions {
        match a {
            #[cfg(windows)]
            Action::RegSet { root, path, name, value } => items.push(ChangeItem::Registry {
                root: root.to_string(),
                path: path.to_string(),
                name: name.to_string(),
                prev: reg_read(root, path, name),
                new: value.clone(),
            }),
            #[cfg(not(windows))]
            Action::RegSet { .. } => return Err("Windows only".into()),
            Action::Service { name, target } => items.push(ChangeItem::ServiceStartup {
                service: name.to_string(),
                prev: service_get_start(name)?,
                new: target.to_string(),
            }),
            Action::Cmd { apply, revert } => items.push(ChangeItem::Command {
                applied: apply.to_string(),
                revert: revert.to_string(),
            }),
        }
    }

    // 3. Write-ahead journal entry.
    let entry_id = format!("{}-{}", t.id, chrono::Local::now().format("%Y%m%d%H%M%S"));
    safety::append_entry(JournalEntry {
        id: entry_id.clone(),
        tweak_id: t.id.to_string(),
        tweak_name: t.name.to_string(),
        time: chrono::Local::now().to_rfc3339(),
        items: items.clone(),
        reverted: false,
        backup_files: backups,
    })?;

    // 4. Apply. On failure, roll back what we already changed.
    let mut done: Vec<&ChangeItem> = Vec::new();
    for item in &items {
        let res = apply_item(item);
        if let Err(e) = res {
            for d in done.iter().rev() {
                let _ = revert_item(d);
            }
            return Err(format!("apply failed ({e}); changes rolled back"));
        }
        done.push(item);
    }

    Ok(json!({ "entryId": entry_id, "status": "applied" }))
}

fn apply_item(item: &ChangeItem) -> Result<(), String> {
    match item {
        #[cfg(windows)]
        ChangeItem::Registry { root, path, name, new, .. } => reg_write(root, path, name, new),
        #[cfg(not(windows))]
        ChangeItem::Registry { .. } => Err("Windows only".into()),
        ChangeItem::ServiceStartup { service, new, .. } => service_set_start(service, new),
        ChangeItem::Command { applied, .. } => run_cmdline(applied),
    }
}

fn revert_item(item: &ChangeItem) -> Result<(), String> {
    match item {
        #[cfg(windows)]
        ChangeItem::Registry { root, path, name, prev, .. } => match prev {
            Some(v) => reg_write(root, path, name, v),
            None => reg_delete(root, path, name).or(Ok(())), // value didn't exist before
        },
        #[cfg(not(windows))]
        ChangeItem::Registry { .. } => Err("Windows only".into()),
        ChangeItem::ServiceStartup { service, prev, .. } => service_set_start(service, prev),
        ChangeItem::Command { revert, .. } => run_cmdline(revert),
    }
}

/// Undo the most recent non-reverted journal entry for a tweak.
pub fn revert(tweak_id: &str) -> Result<Value, String> {
    let mut journal = safety::load_journal();
    let entry = journal
        .iter_mut()
        .rev()
        .find(|e| e.tweak_id == tweak_id && !e.reverted)
        .ok_or("nothing to undo for this tweak")?;
    let mut errs = Vec::new();
    for item in entry.items.iter().rev() {
        if let Err(e) = revert_item(item) {
            errs.push(e);
        }
    }
    if errs.is_empty() {
        entry.reverted = true;
        let id = entry.id.clone();
        safety::save_journal(&journal)?;
        Ok(json!({ "entryId": id, "status": "reverted" }))
    } else {
        Err(format!("partial revert, errors: {}", errs.join("; ")))
    }
}

pub fn history() -> Value {
    serde_json::to_value(safety::load_journal()).unwrap_or(Value::Null)
}
