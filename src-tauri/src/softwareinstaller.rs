//! Software Installer — curated winget catalog with streaming install progress.

use serde_json::{json, Value};
use tauri::AppHandle;
use tauri::Emitter;

// ── catalog ──────────────────────────────────────────────────────────────────

pub fn catalog() -> Value {
    json!([
        // ── Gaming ────────────────────────────────────────────────────────────
        { "id": "steam",             "name": "Steam",                      "category": "gaming",    "desc": "PC gaming platform — library, multiplayer, workshop",          "wingetId": "Valve.Steam",                                    "icon": "🎮", "recommended": true  },
        { "id": "discord",           "name": "Discord",                    "category": "gaming",    "desc": "Voice, video & text chat for gaming communities",             "wingetId": "Discord.Discord",                                "icon": "💬", "recommended": true  },
        { "id": "geforce-exp",       "name": "NVIDIA GeForce Experience",  "category": "gaming",    "desc": "NVIDIA driver updater, Game Ready drivers & overlay",         "wingetId": "Nvidia.GeForceExperience",                       "icon": "🟢", "recommended": false },
        { "id": "amd-adrenalin",     "name": "AMD Software: Adrenalin",    "category": "gaming",    "desc": "AMD GPU driver suite, Radeon overlay & performance tuning",   "wingetId": "AdvancedMicroDevices.AMDSoftware.Adrenalin",      "icon": "🔴", "recommended": false },
        { "id": "intel-arc",         "name": "Intel Arc Control",          "category": "gaming",    "desc": "Intel GPU driver & performance overlay",                      "wingetId": "Intel.ArcControl",                               "icon": "🔵", "recommended": false },
        { "id": "ea-app",            "name": "EA App",                     "category": "gaming",    "desc": "Electronic Arts game launcher (replaces Origin)",             "wingetId": "ElectronicArts.EADesktop",                       "icon": "🎯", "recommended": false },
        { "id": "battlenet",         "name": "Battle.net",                 "category": "gaming",    "desc": "Blizzard launcher — WoW, Overwatch 2, Diablo IV",            "wingetId": "Blizzard.BattleNet",                             "icon": "⚔",  "recommended": false },
        { "id": "epic",              "name": "Epic Games Launcher",        "category": "gaming",    "desc": "Epic Games store — free games every week",                   "wingetId": "EpicGames.EpicGamesLauncher",                    "icon": "🚀", "recommended": false },
        { "id": "gog",               "name": "GOG Galaxy",                 "category": "gaming",    "desc": "DRM-free game platform, unified library",                    "wingetId": "GOG.Galaxy",                                     "icon": "🌌", "recommended": false },
        { "id": "xbox",              "name": "Xbox App",                   "category": "gaming",    "desc": "Game Pass, cloud gaming & Xbox social features",              "wingetId": "Microsoft.GamingApp",                            "icon": "🕹", "recommended": false },
        { "id": "ubisoft",           "name": "Ubisoft Connect",            "category": "gaming",    "desc": "Ubisoft game launcher",                                       "wingetId": "Ubisoft.Connect",                                "icon": "🔶", "recommended": false },

        // ── Communication ─────────────────────────────────────────────────────
        { "id": "teamspeak",         "name": "TeamSpeak",                  "category": "comms",     "desc": "Low-latency voice chat, preferred in competitive gaming",     "wingetId": "TeamSpeakSystems.TeamSpeakClient",                "icon": "🎙", "recommended": false },
        { "id": "mumble",            "name": "Mumble",                     "category": "comms",     "desc": "Open-source low-latency voice chat",                          "wingetId": "Mumble.Mumble",                                  "icon": "🔊", "recommended": false },

        // ── Browsers ──────────────────────────────────────────────────────────
        { "id": "chrome",            "name": "Google Chrome",              "category": "browsers",  "desc": "Fast, widely compatible browser",                            "wingetId": "Google.Chrome",                                  "icon": "🌐", "recommended": false },
        { "id": "firefox",           "name": "Mozilla Firefox",            "category": "browsers",  "desc": "Privacy-focused open-source browser",                        "wingetId": "Mozilla.Firefox",                                "icon": "🦊", "recommended": false },
        { "id": "brave",             "name": "Brave",                      "category": "browsers",  "desc": "Built-in ad & tracker blocker, Chromium-based",              "wingetId": "Brave.Brave",                                    "icon": "🦁", "recommended": false },

        // ── Utilities ─────────────────────────────────────────────────────────
        { "id": "7zip",              "name": "7-Zip",                      "category": "utilities", "desc": "Free file archiver — zip, rar, 7z and more",                 "wingetId": "7zip.7zip",                                      "icon": "📦", "recommended": true  },
        { "id": "vlc",               "name": "VLC Media Player",           "category": "utilities", "desc": "Plays anything — every video & audio format",                "wingetId": "VideoLAN.VLC",                                   "icon": "📺", "recommended": true  },
        { "id": "obs",               "name": "OBS Studio",                 "category": "utilities", "desc": "Free streaming & recording — Twitch, YouTube",               "wingetId": "OBSProject.OBSStudio",                           "icon": "🎥", "recommended": false },
        { "id": "sharex",            "name": "ShareX",                     "category": "utilities", "desc": "Screenshot & screen recorder with annotation tools",         "wingetId": "ShareX.ShareX",                                  "icon": "📸", "recommended": false },
        { "id": "notepadpp",         "name": "Notepad++",                  "category": "utilities", "desc": "Advanced text/code editor",                                  "wingetId": "Notepad++.Notepad++",                            "icon": "📝", "recommended": false },
        { "id": "everything",        "name": "Everything",                 "category": "utilities", "desc": "Instant file search — finds files in milliseconds",          "wingetId": "voidtools.Everything",                           "icon": "🔍", "recommended": true  },
        { "id": "winrar",            "name": "WinRAR",                     "category": "utilities", "desc": "RAR & ZIP archiver",                                         "wingetId": "RARLab.WinRAR",                                  "icon": "🗜", "recommended": false },

        // ── Monitoring & Tools ────────────────────────────────────────────────
        { "id": "msi-afterburner",   "name": "MSI Afterburner",            "category": "tools",     "desc": "GPU overclocking, fan curves & in-game overlay",             "wingetId": "Guru3D.MSIAfterburner",                          "icon": "🔥", "recommended": true  },
        { "id": "hwinfo",            "name": "HWiNFO64",                   "category": "tools",     "desc": "Deep hardware monitoring — sensors, voltages, clocks",        "wingetId": "REALiX.HWiNFO",                                  "icon": "📊", "recommended": false },
        { "id": "cpuz",              "name": "CPU-Z",                      "category": "tools",     "desc": "CPU, RAM & motherboard info",                                "wingetId": "CPUID.CPU-Z",                                    "icon": "💻", "recommended": false },
        { "id": "gpuz",              "name": "GPU-Z",                      "category": "tools",     "desc": "Detailed GPU specifications & sensor monitoring",             "wingetId": "TechPowerUp.GPU-Z",                              "icon": "🖥", "recommended": false },
        { "id": "crystaldisk",       "name": "CrystalDiskMark",            "category": "tools",     "desc": "SSD & HDD speed benchmark",                                  "wingetId": "CrystalDewWorld.CrystalDiskMark",                "icon": "💾", "recommended": false },
        { "id": "furmark",           "name": "FurMark",                    "category": "tools",     "desc": "GPU stress test & burn-in tool",                             "wingetId": "Geeks3D.FurMark",                                "icon": "🌡", "recommended": false },
        { "id": "speccy",            "name": "Speccy",                     "category": "tools",     "desc": "System information & hardware overview",                     "wingetId": "Piriform.Speccy",                                "icon": "🔬", "recommended": false }
    ])
}

// ── check installed ───────────────────────────────────────────────────────────

pub fn check_installed() -> Value {
    // Returns map of wingetId -> bool
    let script = r#"
$result = @{}
try {
    $installed = winget list --accept-source-agreements 2>$null
    $catalog = @(
        'Valve.Steam','Discord.Discord','Nvidia.GeForceExperience',
        'AdvancedMicroDevices.AMDSoftware.Adrenalin','Intel.ArcControl',
        'ElectronicArts.EADesktop','Blizzard.BattleNet','EpicGames.EpicGamesLauncher',
        'GOG.Galaxy','Microsoft.GamingApp','Ubisoft.Connect',
        'TeamSpeakSystems.TeamSpeakClient','Mumble.Mumble',
        'Google.Chrome','Mozilla.Firefox','Brave.Brave',
        '7zip.7zip','VideoLAN.VLC','OBSProject.OBSStudio','ShareX.ShareX',
        'Notepad++.Notepad++','voidtools.Everything','RARLab.WinRAR',
        'Guru3D.MSIAfterburner','REALiX.HWiNFO','CPUID.CPU-Z',
        'TechPowerUp.GPU-Z','CrystalDewWorld.CrystalDiskMark',
        'Geeks3D.FurMark','Piriform.Speccy'
    )
    foreach ($id in $catalog) {
        $result[$id] = ($installed | Select-String ([regex]::Escape($id))) -ne $null
    }
} catch {}
$result | ConvertTo-Json -Compress
"#;
    crate::ps::run_json(script).unwrap_or_else(|_| json!({}))
}

// ── install ───────────────────────────────────────────────────────────────────

pub fn install_apps(winget_ids: Vec<String>, app: AppHandle) {
    std::thread::spawn(move || {
        for id in &winget_ids {
            let _ = app.emit("sw-install-progress", json!({
                "wingetId": id,
                "status":   "installing",
                "message":  format!("Installing {}…", id),
            }));

            let script = format!(
                r#"$o = winget install --id "{}" --exact --silent --accept-package-agreements --accept-source-agreements 2>&1 | Out-String; "EXIT:$($LASTEXITCODE)"; $o"#,
                id
            );

            match crate::ps::run(&script) {
                Ok(out) => {
                    // Use exit code as primary indicator — language-agnostic
                    // -1978335212 = already installed, -1978335189 = no applicable upgrade
                    let first = out.lines().next().unwrap_or("");
                    let exit_ok = first == "EXIT:0"
                        || first == "EXIT:-1978335212"
                        || first == "EXIT:-1978335189";
                    let success = exit_ok
                        || out.contains("Successfully installed")
                        || out.contains("already installed")
                        || out.contains("No applicable upgrade found");
                    let msg = out.lines()
                        .filter(|l| !l.starts_with("EXIT:") && !l.trim().is_empty())
                        .last()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    let _ = app.emit("sw-install-progress", json!({
                        "wingetId": id,
                        "status":   if success { "done" } else { "error" },
                        "message":  msg,
                    }));
                }
                Err(e) => {
                    let _ = app.emit("sw-install-progress", json!({
                        "wingetId": id,
                        "status":   "error",
                        "message":  e,
                    }));
                }
            }
        }

        let _ = app.emit("sw-install-progress", json!({
            "wingetId": "__done__",
            "status":   "all_done",
            "message":  format!("Finished installing {} app(s)", winget_ids.len()),
        }));
    });
}
