//! Inventory of every Tauri command by the privileges it needs, phase 1 of
//! docs/audit-2026-10/elevation-design.md. Today the whole app runs elevated;
//! the broker split moves `ReadAdmin` and `Privileged` commands into the
//! elevated helper and keeps `Read` and `User` in the unelevated UI.
//!
//! Test-only for now: the tests keep this list in sync with
//! `generate_handler!` in lib.rs, so no command can be added without being
//! classified. The broker (phase 2) will use it at runtime.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Reads state a standard user can read.
    Read,
    /// Reads state that needs administrator rights (broker, read-only).
    ReadAdmin,
    /// Changes only the user's own state or app data; runs unelevated.
    User,
    /// Changes machine-wide state (HKLM, services, boot, drivers, system
    /// files) or mixes user and machine scope: broker only.
    Privileged,
}

use Class::*;

/// (command, class, why). Notes marked "verify" still need a check as a
/// standard user on Windows before phase 3 relies on them.
pub static COMMANDS: &[(&str, Class, &str)] = &[
    // ── system scan / dashboard ──
    ("cmd_is_admin", Read, "token query"),
    (
        "cmd_restart_as_admin",
        User,
        "relaunches the app elevated (UAC)",
    ),
    ("cmd_full_scan", Read, "WMI inventory"),
    (
        "cmd_boot_analysis",
        ReadAdmin,
        "Diagnostics-Performance log; may enable the log",
    ),
    ("cmd_event_logs", Read, "System log summary"),
    (
        "cmd_component_health",
        ReadAdmin,
        "storage reliability counters",
    ),
    ("cmd_dns_benchmark", Read, "DNS queries"),
    ("cmd_network_diag", Read, "adapter and route info"),
    ("cmd_start_monitor", Read, "performance counters"),
    ("cmd_stop_monitor", Read, "stops the app's own sampler"),
    ("cmd_analyze", ReadAdmin, "includes the security scan"),
    (
        "cmd_generate_report",
        ReadAdmin,
        "includes the security scan; writes a report to app data",
    ),
    ("cmd_clear_cache", User, "app cache files"),
    ("cmd_open_path", User, "explorer.exe, allowlisted targets"),
    (
        "cmd_open_windows_settings",
        User,
        "allowlisted ms-settings pages",
    ),
    // ── tweak engine / health check ──
    ("cmd_broker_status", Read, "app state"),
    (
        "cmd_broker_unlock",
        User,
        "starts the admin helper; Windows asks via UAC",
    ),
    (
        "cmd_list_tweaks",
        Read,
        "registry, services, journal (readable by decision)",
    ),
    (
        "cmd_apply_tweak",
        Privileged,
        "mixed HKCU/HKLM/services/powercfg; journal. Routed to the broker when connected (phase 3)",
    ),
    (
        "cmd_revert_tweak",
        Privileged,
        "journal undo / force-revert. Routed to the broker when connected (phase 3)",
    ),
    ("cmd_history", Read, "journal (readable by decision)"),
    (
        "cmd_revert_entry",
        Privileged,
        "journal undo. Routed to the broker when connected (phase 3)",
    ),
    (
        "cmd_health_scan",
        ReadAdmin,
        "Defender, BCD and powercfg reads; verify which parts need admin",
    ),
    (
        "cmd_reset_retired",
        Privileged,
        "engine reverts. Routed to the broker when connected (phase 3)",
    ),
    (
        "cmd_optimize_apply",
        Privileged,
        "engine applies, restore point. Routed to the broker when connected (phase 3)",
    ),
    ("cmd_profile_list", Read, "catalog"),
    (
        "cmd_profile_apply",
        Privileged,
        "engine applies. Routed to the broker when connected (phase 3)",
    ),
    (
        "cmd_profile_revert",
        Privileged,
        "engine reverts. Routed to the broker when connected (phase 3)",
    ),
    // ── restore points ──
    (
        "cmd_create_restore_point",
        Privileged,
        "Checkpoint-Computer",
    ),
    (
        "cmd_list_restore_points",
        ReadAdmin,
        "Get-ComputerRestorePoint",
    ),
    (
        "cmd_delete_restore_point",
        Privileged,
        "SRRemoveRestorePoint",
    ),
    ("cmd_launch_rstrui", User, "rstrui asks for UAC itself"),
    // ── cleanup / disk ──
    ("cmd_scan_cleanup", Read, "sizes of temp/cache folders"),
    (
        "cmd_run_cleanup",
        Privileged,
        "system temp, Windows Update cache",
    ),
    ("cmd_disk_drives", Read, "volumes"),
    ("cmd_disk_largest", Read, "file sizes"),
    ("cmd_disk_duplicates", Read, "file hashes"),
    ("cmd_disk_temp_age", Read, "file ages"),
    (
        "cmd_disk_delete",
        User,
        "user-chosen files with the user's own rights (safer than elevated)",
    ),
    (
        "cmd_disk_move",
        User,
        "user-chosen files with the user's own rights",
    ),
    ("cmd_organizer_folders", Read, "known folders"),
    ("cmd_organizer_preview", Read, "folder listing"),
    (
        "cmd_organizer_apply",
        User,
        "moves files in a user folder (logged)",
    ),
    ("cmd_organizer_runs", Read, "organizer history"),
    ("cmd_organizer_undo", User, "moves logged files back"),
    // ── security ──
    (
        "cmd_security_scan",
        ReadAdmin,
        "Defender, firewall, drivers, autoruns",
    ),
    ("cmd_defender_quick_scan", Privileged, "Start-MpScan"),
    (
        "cmd_defender_set_realtime",
        Privileged,
        "Defender preference",
    ),
    ("cmd_defender_set_cloud", Privileged, "Defender preference"),
    ("cmd_hosts_list_all", Read, "hosts file is world-readable"),
    (
        "cmd_hosts_disable_entries",
        Privileged,
        "writes the hosts file",
    ),
    (
        "cmd_hosts_enable_entries",
        Privileged,
        "writes the hosts file",
    ),
    ("cmd_security_disable_driver", Privileged, "pnputil"),
    ("cmd_security_enable_driver", Privileged, "pnputil"),
    (
        "cmd_security_remove_driver",
        Privileged,
        "pnputil /delete-driver",
    ),
    ("cmd_disable_scheduled_task", Privileged, "system task"),
    ("cmd_enable_scheduled_task", Privileged, "system task"),
    // ── benchmark / FPS / latency ──
    (
        "cmd_run_benchmark",
        User,
        "synthetic test, history in app data",
    ),
    ("cmd_bench_history", Read, "app data"),
    ("cmd_fps_import", User, "parses a CSV, stores in app data"),
    ("cmd_fps_runs", Read, "app data"),
    ("cmd_fps_delete", User, "app data"),
    ("cmd_fps_compare", Read, "app data"),
    ("cmd_latency_counters", Read, "performance counters"),
    (
        "cmd_throttle_snapshot",
        Read,
        "performance counters, nvidia-smi",
    ),
    ("cmd_session_start", User, "starts the app's own sampler"),
    (
        "cmd_session_stop",
        User,
        "stops the sampler; writes app data",
    ),
    ("cmd_session_status", Read, "the app's own sampler"),
    ("cmd_session_list", Read, "app data"),
    (
        "cmd_session_analyze",
        Read,
        "app data and a user-chosen CSV",
    ),
    ("cmd_session_delete", User, "app data"),
    ("cmd_display_check", Read, "display configuration API"),
    (
        "cmd_audio_check",
        Read,
        "HKLM MMDevices (readable by users)",
    ),
    (
        "cmd_game_storage_check",
        Read,
        "launcher files, disk info; fsutil query (verify as standard user)",
    ),
    ("cmd_pregame_scan", Read, "process CPU/disk sampling"),
    (
        "cmd_pregame_pause",
        Privileged,
        "cancels a Defender scan, stops WSearch, closes OneDrive",
    ),
    (
        "cmd_pregame_resume",
        Privileged,
        "starts WSearch, starts OneDrive as the user",
    ),
    (
        "cmd_stall_probe",
        Read,
        "timer measurement in the app process",
    ),
    (
        "cmd_wpr_start",
        Privileged,
        "Windows Performance Recorder needs admin",
    ),
    ("cmd_wpr_stop", Privileged, "Windows Performance Recorder"),
    ("cmd_wpr_cancel", Privileged, "Windows Performance Recorder"),
    // ── startup / processes / tasks / services ──
    ("cmd_startup_list", Read, "Run keys and folders"),
    (
        "cmd_startup_toggle",
        Privileged,
        "HKLM or HKCU entries (mixed scope)",
    ),
    ("cmd_proc_list", Read, "process list"),
    (
        "cmd_proc_kill",
        Privileged,
        "any process, incl. other users' (mixed)",
    ),
    (
        "cmd_proc_priority",
        Privileged,
        "High/Realtime and other users' processes",
    ),
    ("cmd_proc_affinity", Privileged, "other users' processes"),
    (
        "cmd_perm_priority_list",
        Read,
        "HKLM Image File Execution Options",
    ),
    (
        "cmd_perm_priority_set",
        Privileged,
        "HKLM Image File Execution Options",
    ),
    (
        "cmd_perm_priority_remove",
        Privileged,
        "HKLM Image File Execution Options",
    ),
    ("cmd_schedtasks_list", Read, "task list"),
    ("cmd_schedtask_toggle", Privileged, "system tasks"),
    ("cmd_services_list", Read, "service list"),
    (
        "cmd_service_set_startup",
        Privileged,
        "service configuration",
    ),
    ("cmd_service_control", Privileged, "start/stop services"),
    ("cmd_health_run", Privileged, "SFC / DISM"),
    // ── updates / drivers / software ──
    ("cmd_scan_app_updates", Read, "winget upgrade list"),
    (
        "cmd_update_apps",
        Privileged,
        "winget upgrade (machine scope)",
    ),
    (
        "cmd_scan_driver_updates",
        Read,
        "Windows Update search; verify as standard user",
    ),
    (
        "cmd_install_driver_updates",
        Privileged,
        "Windows Update install",
    ),
    ("cmd_gpu_vendor", Read, "WMI"),
    ("cmd_drivers_list", Read, "PnP signed drivers"),
    ("cmd_drivers_open_devmgr", User, "opens Device Manager"),
    ("cmd_drivers_open_windows_update", User, "opens Settings"),
    ("cmd_drivers_check_winget", Read, "winget show"),
    (
        "cmd_drivers_install_winget",
        Privileged,
        "winget install (machine scope)",
    ),
    (
        "cmd_drivers_open_vendor_url",
        User,
        "browser, allowlisted URL",
    ),
    (
        "cmd_drivers_scan_windows_update",
        Privileged,
        "pnputil driver update",
    ),
    ("cmd_sw_catalog", Read, "static catalog"),
    ("cmd_sw_check_installed", Read, "winget list"),
    (
        "cmd_sw_install",
        Privileged,
        "winget install (machine scope)",
    ),
    // ── privacy / debloater / context menu / registry ──
    ("cmd_privacy_scan", Read, "registry and services"),
    (
        "cmd_privacy_apply",
        Privileged,
        "HKLM policies and services (mixed). Routed to the broker when connected (phase 3)",
    ),
    (
        "cmd_privacy_revert",
        Privileged,
        "journal undo. Routed to the broker when connected (phase 3)",
    ),
    (
        "cmd_debloater_uwp_list",
        ReadAdmin,
        "Get-AppxPackage -AllUsers. Routed to the broker when connected (phase 3)",
    ),
    (
        "cmd_debloater_remove_uwp",
        Privileged,
        "Remove-AppxPackage -AllUsers. Routed to the broker when connected (phase 3)",
    ),
    (
        "cmd_debloater_remove_provisioned",
        Privileged,
        "Remove-AppxProvisionedPackage. Routed to the broker when connected (phase 3)",
    ),
    ("cmd_debloater_tweaks_list", Read, "registry and services"),
    (
        "cmd_debloater_tweak_apply",
        Privileged,
        "HKLM policies and services (mixed). Routed to the broker when connected (phase 3)",
    ),
    (
        "cmd_debloater_tweak_revert",
        Privileged,
        "captured state in the admin-only folder. Routed to the broker when connected (phase 3)",
    ),
    ("cmd_ctxmenu_list", Read, "HKCR/HKLM shell extensions"),
    ("cmd_ctxmenu_toggle", Privileged, "HKLM/HKCR"),
    ("cmd_ctxmenu_disable_all", Privileged, "HKLM/HKCR"),
    ("cmd_ctxmenu_enable_all", Privileged, "HKLM/HKCR"),
    ("cmd_regclean_scan", Read, "registry reads"),
    ("cmd_regclean_clean", Privileged, "HKLM deletes with backup"),
    ("cmd_regclean_list_backups", Read, "backup folder listing"),
    ("cmd_regclean_restore", Privileged, "reg import"),
    // ── uninstaller ──
    ("cmd_uninstaller_list", Read, "Uninstall keys"),
    (
        "cmd_uninstall_app",
        Privileged,
        "registered HKLM uninstallers only",
    ),
    ("cmd_scan_leftovers", Read, "file and registry reads"),
    (
        "cmd_clean_leftovers",
        Privileged,
        "Program Files / HKLM leftovers",
    ),
    // ── boot ──
    (
        "cmd_boot_scan",
        ReadAdmin,
        "bcdedit /enum and boot event log",
    ),
    ("cmd_boot_tweak_apply", Privileged, "bcdedit"),
    ("cmd_boot_tweak_revert", Privileged, "bcdedit"),
    // ── power ──
    ("cmd_powerplan_list", Read, "powercfg /list"),
    (
        "cmd_powerplan_set",
        Privileged,
        "powercfg /setactive; verify whether a standard user may",
    ),
    (
        "cmd_powerplan_unlock_ultimate",
        Privileged,
        "powercfg -duplicatescheme",
    ),
    ("cmd_powerplan_delete", Privileged, "powercfg /delete"),
    (
        "cmd_powerplan_create",
        Privileged,
        "powercfg -duplicatescheme",
    ),
    // ── perf tweaks ──
    ("cmd_timer_get", Read, "NtQueryTimerResolution"),
    (
        "cmd_timer_set",
        Privileged,
        "checks admin today; verify what it needs",
    ),
    (
        "cmd_timer_reset",
        Privileged,
        "counterpart of cmd_timer_set",
    ),
    ("cmd_msi_list", Read, "HKLM device keys"),
    ("cmd_msi_set", Privileged, "HKLM device keys"),
    ("cmd_net_adapters", Read, "adapter properties"),
    (
        "cmd_net_tweak",
        Privileged,
        "Set-NetAdapterAdvancedProperty",
    ),
    (
        "cmd_net_reset_all",
        Privileged,
        "Reset-NetAdapterAdvancedProperty",
    ),
    ("cmd_ram_info", Read, "memory counters"),
    ("cmd_pagefile_info", Read, "WMI"),
    (
        "cmd_pagefile_set_auto",
        Privileged,
        "Win32_ComputerSystem / page file settings",
    ),
    ("cmd_pagefile_set_custom", Privileged, "page file settings"),
    ("cmd_pagefile_disable", Privileged, "page file settings"),
    ("cmd_perf_open_entries", Read, "journal"),
    (
        "cmd_perf_undo",
        Privileged,
        "journal undo of MSI, NIC and page-file changes",
    ),
    // ── hardware ──
    (
        "cmd_hw_full",
        Read,
        "WMI sensors and SMART summary; verify SMART as standard user",
    ),
    ("cmd_hw_profile", Read, "WMI"),
    // ── GPU ──
    ("cmd_gpu_scan", Read, "driver registry key"),
    (
        "cmd_gpu_tweak_apply",
        Privileged,
        "HKLM driver key, services",
    ),
    ("cmd_gpu_tweak_revert", Privileged, "journal undo"),
    ("cmd_nv_status", Read, "GPU vendor, journal"),
    ("cmd_nv_open_panel", User, "starts the NVIDIA Control Panel"),
    // ── game booster / game profiles ──
    ("cmd_gameboost_background_procs", Read, "process list"),
    ("cmd_gameboost_running_games", Read, "process list"),
    (
        "cmd_gameboost_kill_background",
        Privileged,
        "validated kill list, other users' processes",
    ),
    (
        "cmd_gameboost_start",
        Privileged,
        "priorities and power plan",
    ),
    (
        "cmd_gameboost_stop",
        Privileged,
        "reverts cmd_gameboost_start",
    ),
    ("cmd_gameboost_get_status", Read, "app state"),
    ("cmd_gameboost_quick_boost", Privileged, "journaled changes"),
    (
        "cmd_gameboost_quick_boost_revert",
        Privileged,
        "journal undo",
    ),
    ("cmd_game_list", Read, "installed games"),
    ("cmd_game_switcher_status", Read, "app state"),
    (
        "cmd_game_switcher_configure",
        Privileged,
        "switches power plans and priorities in the background",
    ),
    (
        "cmd_game_apply_preset",
        Privileged,
        "power plan and priorities",
    ),
    ("cmd_game_revert", Privileged, "power plan"),
];

#[cfg(test)]
mod tests {
    use super::{Class, COMMANDS};
    use std::collections::HashSet;

    fn registered() -> Vec<String> {
        let lib = include_str!("lib.rs");
        let start = lib.find("generate_handler![").expect("generate_handler!");
        let block = &lib[start..];
        let block = &block[..block.find("])").expect("end of generate_handler!")];
        block
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|w| w.starts_with("cmd_"))
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn every_registered_command_is_classified_exactly_once() {
        let mut seen = HashSet::new();
        for (name, _, why) in COMMANDS {
            assert!(seen.insert(*name), "{name} classified twice");
            assert!(!why.is_empty(), "{name} has no reason");
        }
        let reg = registered();
        for name in &reg {
            assert!(
                seen.contains(name.as_str()),
                "{name} is registered but not classified"
            );
        }
        let reg: HashSet<&str> = reg.iter().map(String::as_str).collect();
        for (name, _, _) in COMMANDS {
            assert!(
                reg.contains(name),
                "{name} is classified but not registered"
            );
        }
    }

    #[test]
    fn every_defined_command_is_registered() {
        let lib = include_str!("lib.rs");
        let reg: HashSet<String> = registered().into_iter().collect();
        for part in lib.split("tauri::command").skip(1) {
            let Some(pos) = part.find("fn cmd_") else {
                continue;
            };
            let name: String = part[pos + 3..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            assert!(reg.contains(&name), "{name} is defined but not registered");
        }
    }

    /// A command whose name says it changes something may never be
    /// classified as a read.
    #[test]
    fn mutating_commands_are_not_reads() {
        const VERBS: &[&str] = &[
            "apply",
            "revert",
            "set",
            "delete",
            "kill",
            "remove",
            "install",
            "clean",
            "toggle",
            "disable",
            "enable",
            "create",
            "unlock",
            "restore",
            "move",
            "update",
            "reset",
            "start",
            "stop",
            "configure",
            "import",
            "run",
            "control",
            "boost",
            "priority",
            "affinity",
        ];
        // Reads whose names contain a verb but change nothing.
        const READS: &[&str] = &[
            "cmd_start_monitor",
            "cmd_stop_monitor",
            "cmd_hosts_list_all",
            "cmd_scan_app_updates",
            "cmd_scan_driver_updates",
            "cmd_list_restore_points",
            "cmd_perm_priority_list",
            "cmd_gameboost_background_procs",
            "cmd_gameboost_running_games",
            "cmd_gameboost_get_status",
            "cmd_game_switcher_status",
            "cmd_scan_cleanup",
            "cmd_regclean_list_backups",
            "cmd_debloater_tweaks_list",
            "cmd_list_tweaks",
            "cmd_privacy_scan",
        ];
        for (name, class, _) in COMMANDS {
            let words: Vec<&str> = name.trim_start_matches("cmd_").split('_').collect();
            let mutating = words.iter().any(|w| VERBS.contains(w));
            if mutating && !READS.contains(name) {
                assert!(
                    matches!(class, Class::User | Class::Privileged),
                    "{name} looks mutating but is classified {class:?}"
                );
            }
        }
    }
}
