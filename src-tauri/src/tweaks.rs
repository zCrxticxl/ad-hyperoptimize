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
            name: "Game Bar Popups & Startpanel aus",
            category: "Gaming",
            description: "Deaktiviert Game-Bar-Startpanel und Nexus-Overlay-Hinweise.",
            rationale: "Die Game-Bar-Popups ('Drücke Win+G') unterbrechen Spiele und kosten beim Spielstart kurz Fokus/Frames.",
            impact: "Keine Game-Bar-Einblendungen mehr; Win+G funktioniert weiterhin manuell.",
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
            name: "Transparenz-Effekte aus",
            category: "Responsiveness",
            description: "Deaktiviert Acryl/Blur-Transparenz in Taskleiste, Startmenü und Fenstern.",
            rationale: "Blur-Effekte kosten dauerhaft GPU-Zeit (DWM) — auf iGPUs und schwächeren Karten messbar.",
            impact: "Weniger DWM-GPU-Last; UI wirkt nüchterner.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKCU", path: "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize", name: "EnableTransparency", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "min_animate_off",
            name: "Fenster-Minimier-Animation aus",
            category: "Responsiveness",
            description: "Deaktiviert die Minimieren/Maximieren-Animation.",
            rationale: "Reine Warte-Animation (~200ms pro Vorgang). Ohne sie fühlt sich das Fenster-Handling sofortig an.",
            impact: "Snappigeres Fenster-Verhalten.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKCU", path: "Control Panel\\Desktop\\WindowMetrics", name: "MinAnimate", value: RegVal::Str("0".into()) }],
        },
        Tweak {
            id: "start_websearch_off",
            name: "Web-Suche im Startmenü aus",
            category: "Privacy",
            description: "Startmenü-Suche zeigt nur noch lokale Ergebnisse (keine Bing-Vorschläge).",
            rationale: "Jeder Tastendruck in der Startsuche geht sonst an Bing — Latenz in der Suche plus Datenabfluss.",
            impact: "Schnellere, rein lokale Startmenü-Suche.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKCU", path: "Software\\Policies\\Microsoft\\Windows\\Explorer", name: "DisableSearchBoxSuggestions", value: RegVal::Dword(1) }],
        },
        Tweak {
            id: "lockscreen_ads_off",
            name: "Sperrbildschirm-Spotlight/Werbung aus",
            category: "Privacy",
            description: "Deaktiviert Windows-Spotlight-Overlays und 'Fun Facts' auf dem Sperrbildschirm.",
            rationale: "Spotlight lädt Inhalte im Hintergrund nach und blendet gesponserte Hinweise ein.",
            impact: "Statischer Sperrbildschirm, kein Hintergrund-Fetching.",
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
            name: "Widgets / News & Interests aus (Policy)",
            category: "Background",
            description: "Deaktiviert das Widget-Board (Win11) per Richtlinie.",
            rationale: "Der Widget-Prozess (msedgewebview2) läuft permanent mit, lädt News nach und kostet RAM + Netzwerk.",
            impact: "Spart dauerhaft RAM und Hintergrund-Traffic. Widget-Button verschwindet.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SOFTWARE\\Policies\\Microsoft\\Dsh", name: "AllowNewsAndInterests", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "delivery_opt_off",
            name: "Update-P2P (Übermittlungsoptimierung) aus",
            category: "Network",
            description: "Setzt DODownloadMode=0 — Updates kommen nur noch direkt von Microsoft, kein Peer-to-Peer.",
            rationale: "Windows lädt sonst Updates an fremde PCs hoch (Upload-Bandbreite!) und hält dafür einen Cache auf der Platte.",
            impact: "Kein Upload-Verbrauch durch Windows Update; minimal langsamere Update-Downloads möglich.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SOFTWARE\\Policies\\Microsoft\\Windows\\DeliveryOptimization", name: "DODownloadMode", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "wer_off",
            name: "Windows Error Reporting aus",
            category: "Background",
            description: "Deaktiviert das automatische Sammeln/Hochladen von Fehlerberichten.",
            rationale: "WER schreibt bei jedem App-Crash Dumps auf die Platte und lädt sie hoch — I/O-Spitzen genau dann, wenn das System eh stolpert.",
            impact: "Keine WER-Dumps/-Uploads mehr. Nachteil: weniger Crash-Diagnose-Daten, falls du selbst debuggen willst.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SOFTWARE\\Microsoft\\Windows\\Windows Error Reporting", name: "Disabled", value: RegVal::Dword(1) }],
        },
        Tweak {
            id: "activity_history_off",
            name: "Aktivitätsverlauf (Timeline) aus",
            category: "Privacy",
            description: "Stoppt das Protokollieren von App-/Dokument-Aktivitäten (PublishUserActivities=0).",
            rationale: "Jede geöffnete App/Datei wird sonst in die Activity-Datenbank geschrieben — Hintergrund-I/O plus Datensammlung.",
            impact: "Weniger Hintergrund-Schreibzugriffe; Win+Tab-Timeline bleibt leer.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SOFTWARE\\Policies\\Microsoft\\Windows\\System", name: "PublishUserActivities", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "usb_suspend_off",
            name: "USB Selective Suspend aus (aktiver Plan)",
            category: "Gaming",
            description: "Verhindert, dass Windows USB-Geräte im Betrieb schlafen legt.",
            rationale: "Selective Suspend verursacht Aussetzer bei USB-Audio, Mikro-Ruckler bei Mäusen/Headsets und Reconnects bei Controllern.",
            impact: "Stabile USB-Geräte; minimal höherer Verbrauch (am Desktop egal, am Laptop-Akku spürbar).",
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
            name: "Schnellstart (Fast Startup) aus",
            category: "Power",
            description: "Deaktiviert Hiberboot — 'Herunterfahren' fährt wirklich komplett herunter.",
            rationale: "Fast Startup friert den Kernel ein statt neu zu starten: Treiber-Probleme überleben Reboots, Dual-Boot-Setups korrumpieren, Uptime wächst endlos. Ein echter Kaltstart behebt mehr, als er kostet.",
            impact: "Boot ~5–15s langsamer, dafür jedes Mal frischer Kernel-Zustand. Hibernate selbst bleibt verfügbar.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Power", name: "HiberbootEnabled", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "disable_paging_executive",
            name: "Kernel im RAM halten (DisablePagingExecutive)",
            category: "Memory",
            description: "Verhindert das Auslagern von Kernel-Code und Treibern auf den Auslagerungsspeicher.",
            rationale: "Standardmäßig darf Windows Kernel-Seiten auf die Festplatte schreiben. Bei ≥8 GB RAM entsteht nur Latenz wenn Kernel-Code zurückgelesen wird — deaktivieren hält alles im RAM.",
            impact: "Geringere Kernel-Aufruf-Latenz; minimal höherer RAM-Verbrauch (~50 MB). Nicht auf Systemen unter 8 GB RAM anwenden.",
            risk: "Medium",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Memory Management", name: "DisablePagingExecutive", value: RegVal::Dword(1) }],
        },
        Tweak {
            id: "core_parking_off",
            name: "CPU Core Parking deaktivieren",
            category: "Gaming",
            description: "Hält alle CPU-Kerne dauerhaft aktiv (Core Parking Min Cores = 100%). REBOOT EMPFOHLEN.",
            rationale: "Windows parkt Kerne im Leerlauf um Strom zu sparen. Das Aufwecken parkierter Kerne dauert einige Millisekunden — sichtbar als Frametime-Spike beim ersten Lastanstieg in einem Spiel.",
            impact: "Niedrigere Anlauf-Frametimes und weniger Latenz-Spitzen; etwas höherer Idle-Stromverbrauch (am Desktop egal, am Laptop-Akku spürbar).",
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
            name: "NTFS Last-Access-Timestamp deaktivieren",
            category: "Storage",
            description: "Verhindert, dass NTFS bei jedem Lesezugriff den 'Zuletzt aufgerufen'-Zeitstempel schreibt.",
            rationale: "Jeder Lesezugriff löst sonst auch einen MFT-Schreibzugriff aus. Auf viel gelesenen SSDs summiert sich das zu unnötiger Write-Amplification und Latenzmessspitzen.",
            impact: "Deutlich weniger Hintergrundschreibzugriffe auf alle NTFS-Volumes. Reiner Performance-Gewinn auf SSDs.",
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
            name: "8.3-Kurznamen (DOS-Kompatibilität) deaktivieren",
            category: "Storage",
            description: "Stoppt die automatische Generierung von 8.3-Kurznamen (z.B. PROGRA~1) für neue Dateien.",
            rationale: "Bei jedem Dateianlegen prüft NTFS ob ein 8.3-Kurzname generiert werden muss — unnötiger Overhead; kein modernes Programm braucht diese Legacy-Kompatibilität.",
            impact: "Schnelleres Erstellen von Dateien in vollen Verzeichnissen; keine 8.3-Namen für neu angelegte Dateien.",
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
            name: "Xbox-Hintergrunddienste auf Manuell",
            category: "Services",
            description: "Setzt XblAuthManager, XblGameSave und XboxNetApiSvc auf Manual.",
            rationale: "Diese Dienste laufen dauerhaft, werden aber nur für Xbox-Live-Multiplayer und Game-Pass-Cloud-Sync benötigt — die meisten PC-Spieler ohne Xbox-Konto nutzen sie nie.",
            impact: "Weniger Idle-RAM und Service-Overhead. Xbox-Live-Auth und Cloud-Saves müssen bei Bedarf wieder auf Automatic gesetzt werden.",
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
            name: "Remote Registry deaktivieren",
            category: "Services",
            description: "Setzt den RemoteRegistry-Dienst auf Disabled.",
            rationale: "Erlaubt externen Programmen Registry-Zugriff über das Netzwerk — ein bekannter Angriffsvektor, den normale Heimanwender nie benötigen.",
            impact: "Verbesserte Sicherheit; kein Remote-Registrierungszugriff mehr über das Netzwerk möglich.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "RemoteRegistry", target: "Disabled" }],
        },
        Tweak {
            id: "cortana_off",
            name: "Cortana deaktivieren (Policy)",
            category: "Privacy",
            description: "Setzt AllowCortana=0 per Richtlinie.",
            rationale: "Cortana läuft als Hintergrundprozess, sendet Sprachmetriken und beansprucht dauerhaft RAM — für die meisten Nutzer ohne Mehrwert.",
            impact: "Kein Cortana-Prozess mehr; weniger Datenübertragung an Microsoft.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKLM", path: "SOFTWARE\\Policies\\Microsoft\\Windows\\Windows Search", name: "AllowCortana", value: RegVal::Dword(0) }],
        },
        Tweak {
            id: "edge_prelaunch_off",
            name: "Edge Vorladen (Startup Boost) deaktivieren",
            category: "Startup",
            description: "Verhindert, dass Edge sich beim Windows-Start unsichtbar im Hintergrund vorlädt.",
            rationale: "Edge startet sich beim Booten automatisch im Hintergrund um sich beim ersten Aufruf schneller zu öffnen — kostet RAM auch wenn Edge nie benutzt wird.",
            impact: "Weniger RAM-Verbrauch nach dem Boot; Edge öffnet sich beim ersten Start minimal langsamer.",
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
            name: "Suchfeld-Highlights & Trending-Inhalte aus",
            category: "Privacy",
            description: "Deaktiviert dynamische News- und Trending-Inhalte im Windows-Suchfeld.",
            rationale: "Das Suchfeld lädt täglich neue Highlight-Inhalte aus dem Netz und zeigt sie beim Öffnen an — Hintergrundtraffic und Ablenkung ohne Nutzen.",
            impact: "Kein Netzwerk-Fetching durch das Suchfeld; ruhigeres, schnelleres Suchfeld.",
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
            name: "AutoPlay deaktivieren",
            category: "Privacy",
            description: "Verhindert, dass Windows USB-Laufwerke oder Medien beim Einstecken automatisch ausführt.",
            rationale: "AutoPlay ist ein klassischer Angriffsvektor für autorun-Malware. Kein modernes System braucht automatisches Starten unbekannter Geräte.",
            impact: "Sicherheitsverbesserung; USB-Geräte und Disc-Medien öffnen sich nicht mehr automatisch.",
            risk: "Low",
            requires_admin: false,
            reversible: true,
            actions: vec![Action::RegSet { root: "HKCU", path: "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\AutoplayHandlers", name: "DisableAutoplay", value: RegVal::Dword(1) }],
        },
        Tweak {
            id: "svc_fax_off",
            name: "Fax-Dienst deaktivieren",
            category: "Services",
            description: "Setzt den Fax-Dienst auf Disabled.",
            rationale: "Praktisch niemand nutzt Windows-Fax. Der Dienst lädt Treiber und wartet auf Faxleitungen — reine Verschwendung.",
            impact: "Einer weniger startender Dienst. Fax-Funktionalität entfernt (wiederherstellbar).",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "Fax", target: "Disabled" }],
        },
        Tweak {
            id: "svc_geolocation_off",
            name: "Standortdienst (lfsvc) auf Manuell",
            category: "Privacy",
            description: "Setzt den Geolocation-Dienst auf Manual.",
            rationale: "lfsvc sendet kontinuierlich Standortanfragen wenn aktiv. Apps die GPS/Standort brauchen starten ihn bei Bedarf selbst.",
            impact: "Kein dauerhafter Standort-Tracking; Maps/Wetter-Apps können den Dienst weiterhin starten.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "lfsvc", target: "Manual" }],
        },
        Tweak {
            id: "svc_wmp_network_off",
            name: "Windows Media Player Netzwerkfreigabe deaktivieren",
            category: "Services",
            description: "Setzt WMPNetworkSvc auf Disabled.",
            rationale: "Teilt Medien über DLNA/UPnP im Heimnetzwerk — kaum jemand nutzt das über Windows selbst. Unnötiger Dienst + Netzwerk-Listener.",
            impact: "Kein DLNA-Streaming mehr über Windows Media Player.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "WMPNetworkSvc", target: "Disabled" }],
        },
        Tweak {
            id: "svc_maps_off",
            name: "Offline-Karten-Manager deaktivieren",
            category: "Services",
            description: "Setzt MapsBroker auf Disabled.",
            rationale: "Verwaltet Windows-Offline-Karten-Downloads. Wer kein Windows-Maps nutzt, braucht diesen Dienst nie.",
            impact: "Keine automatischen Karten-Updates; Windows Maps Offline-Funktion nicht nutzbar.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "MapsBroker", target: "Disabled" }],
        },
        Tweak {
            id: "svc_link_tracking_off",
            name: "Distributed Link Tracking Client deaktivieren",
            category: "Services",
            description: "Setzt TrkWks auf Disabled.",
            rationale: "Verfolgt NTFS-Verknüpfungen über Netzlaufwerke (Legacy-Enterprise-Feature). Auf Heimsystemen läuft er völlig umsonst.",
            impact: "Broken Shortcuts über Netzlaufwerke werden nicht automatisch repariert — betrifft Heimnutzer in der Regel gar nicht.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "TrkWks", target: "Disabled" }],
        },
        Tweak {
            id: "svc_wisvc_off",
            name: "Windows Insider Service deaktivieren",
            category: "Services",
            description: "Setzt wisvc auf Disabled.",
            rationale: "Wird ausschließlich für das Windows Insider Programm benötigt. Für Nicht-Insider reiner Overhead.",
            impact: "Keine Auswirkung für Nicht-Insider; Insider-Programm kann nicht mehr aktiviert werden.",
            risk: "Low",
            requires_admin: true,
            reversible: true,
            actions: vec![Action::Service { name: "wisvc", target: "Disabled" }],
        },
        Tweak {
            id: "svc_diag_tracking_off",
            name: "Diagnose-Tracking Service deaktivieren",
            category: "Privacy",
            description: "Setzt diagnosticshub.standardcollector.service auf Disabled.",
            rationale: "Sammelt Laufzeit-Diagnose-Traces für Microsoft Visual Studio und Windows Diagnostics. Auf Nicht-Entwickler-Systemen unnötig.",
            impact: "Visual Studio Diagnosetools funktionieren nicht mehr — für normale Gaming/Desktop-Nutzung irrelevant.",
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
