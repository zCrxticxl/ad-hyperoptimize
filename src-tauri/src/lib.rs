mod analysis;
mod autoopt;
mod gamedb;
mod gameprofile;
mod bootopt;
mod ctxmenu;
mod debloater;
mod drivers;
mod gameboost;
mod diskanalyzer;
mod healthcheck;
mod hwmonitor;
mod perftweaks;
mod powerplan;
mod uninstaller;
mod privacy;
mod services;
mod gputweaks;
mod regclean;
mod bench;
mod cache;
mod cleanup;
mod latency;
mod monitor;
mod ps;
mod procmgr;
mod profiles;
mod report;
mod schedtasks;
mod startup;
mod safety;
mod scan;
mod security;
mod tweaks;
mod updates;
mod hwprofile;

use serde_json::Value;
use tauri::{AppHandle, Manager, State};

struct AppState {
    monitor:      monitor::MonitorState,
    game_switcher: gameprofile::SharedState,
    boosted_pid:  std::sync::Mutex<Option<u32>>,
}

// ---- diagnostics ----
#[tauri::command]
fn cmd_is_admin() -> bool { ps::is_admin() }

#[tauri::command(async)]
fn cmd_full_scan(force: Option<bool>) -> Value {
    cache::get_or("scan", force.unwrap_or(false), scan::full_scan)
}

#[tauri::command(async)]
fn cmd_boot_analysis() -> Value { scan::boot_analysis() }

#[tauri::command(async)]
fn cmd_event_logs() -> Value { scan::event_log_summary() }

#[tauri::command(async)]
fn cmd_component_health() -> Value { scan::component_health() }

#[tauri::command(async)]
fn cmd_dns_benchmark() -> Value { scan::dns_benchmark() }

#[tauri::command(async)]
fn cmd_network_diag() -> Value { scan::network_diag() }

// ---- monitoring ----
#[tauri::command]
fn cmd_start_monitor(app: AppHandle, state: State<AppState>) {
    monitor::start(app, state.monitor.running.clone());
}

#[tauri::command]
fn cmd_stop_monitor(state: State<AppState>) {
    monitor::stop(&state.monitor.running);
}

// ---- optimization engine ----
#[tauri::command(async)]
fn cmd_list_tweaks() -> Value { tweaks::list_with_status() }

#[tauri::command(async)]
fn cmd_apply_tweak(id: String) -> Result<Value, String> { tweaks::apply(&id) }

#[tauri::command(async)]
fn cmd_revert_tweak(id: String) -> Result<Value, String> { tweaks::revert(&id) }

#[tauri::command(async)]
fn cmd_history() -> Value { tweaks::history() }

// ---- safety / restore points ----
#[tauri::command(async)]
fn cmd_create_restore_point(description: String) -> Result<String, String> {
    safety::create_restore_point(&description)
}

#[tauri::command(async)]
fn cmd_list_restore_points() -> Value { safety::list_restore_points() }

#[tauri::command(async)]
fn cmd_delete_restore_point(sequence_number: u32) -> Result<String, String> {
    safety::delete_restore_point(sequence_number)
}

#[tauri::command(async)]
fn cmd_launch_rstrui() -> Result<String, String> { safety::launch_rstrui() }

// ---- auto optimizer ----
#[tauri::command(async)]
fn cmd_autoopt_scan() -> Value { autoopt::scan() }

#[tauri::command(async)]
fn cmd_autoopt_score() -> Value { autoopt::score() }

#[tauri::command(async)]
fn cmd_autoopt_apply(items: Vec<Value>) -> Value { autoopt::apply_selected(items) }

// ---- cleanup ----
#[tauri::command(async)]
fn cmd_scan_cleanup(force: Option<bool>) -> Value {
    cache::get_or("cleanup", force.unwrap_or(false), cleanup::scan)
}

#[tauri::command(async)]
fn cmd_run_cleanup(ids: Vec<String>) -> Value { cleanup::clean(ids) }

// ---- security ----
#[tauri::command(async)]
fn cmd_security_scan(force: Option<bool>) -> Value {
    cache::get_or("security", force.unwrap_or(false), security::scan)
}

#[tauri::command(async)]
fn cmd_defender_quick_scan() -> Result<String, String> { security::defender_quick_scan() }

#[tauri::command(async)]
fn cmd_hosts_list_all() -> Value { security::hosts_list_all() }

#[tauri::command(async)]
fn cmd_hosts_disable_entries(entries: Vec<String>) -> Result<String, String> {
    security::hosts_disable_entries(entries)
}

#[tauri::command(async)]
fn cmd_hosts_enable_entries(entries: Vec<String>) -> Result<String, String> {
    security::hosts_enable_entries(entries)
}

// ---- benchmarks ----
#[tauri::command(async)]
fn cmd_run_benchmark(kind: String) -> Result<Value, String> { bench::run(&kind) }

#[tauri::command(async)]
fn cmd_bench_history() -> Value { bench::history() }

// ---- latency analysis ----
#[tauri::command(async)]
fn cmd_latency_counters(samples: u32) -> Value { latency::counters(samples) }

#[tauri::command(async)]
fn cmd_stall_probe(seconds: u32) -> Value { latency::stall_probe(seconds) }

#[tauri::command(async)]
fn cmd_wpr_start() -> Result<String, String> { latency::wpr_start() }

#[tauri::command(async)]
fn cmd_wpr_stop() -> Result<Value, String> { latency::wpr_stop() }

#[tauri::command(async)]
fn cmd_wpr_cancel() -> Result<String, String> { latency::wpr_cancel() }

// ---- profiles ----
#[tauri::command(async)]
fn cmd_profile_list() -> Value { profiles::list() }

#[tauri::command(async)]
fn cmd_profile_apply(id: String, with_bench: bool) -> Result<Value, String> {
    profiles::apply(&id, with_bench)
}

#[tauri::command(async)]
fn cmd_profile_revert(id: String) -> Result<Value, String> { profiles::revert(&id) }

// ---- startup manager ----
#[tauri::command(async)]
fn cmd_startup_list() -> Value { startup::list() }

#[tauri::command(async)]
fn cmd_startup_toggle(scope: String, name: String, enable: bool) -> Result<Value, String> {
    startup::toggle(scope, name, enable)
}

// ---- process manager ----
#[tauri::command(async)]
fn cmd_proc_list() -> Value { procmgr::list() }

#[tauri::command(async)]
fn cmd_proc_kill(pid: u32) -> Result<Value, String> { procmgr::kill(pid) }

#[tauri::command(async)]
fn cmd_proc_priority(pid: u32, priority: String) -> Result<Value, String> {
    procmgr::set_priority(pid, priority)
}

#[tauri::command(async)]
fn cmd_proc_affinity(pid: u32, mask: u64) -> Result<Value, String> {
    procmgr::set_affinity(pid, mask)
}

#[tauri::command(async)]
#[allow(dead_code)]
fn cmd_proc_detail(pid: u32) -> Value { procmgr::get_detail(pid) }

#[tauri::command(async)]
fn cmd_perm_priority_list() -> Value { procmgr::perm_list() }

#[tauri::command(async)]
fn cmd_perm_priority_set(exe: String, priority: String) -> Result<Value, String> {
    procmgr::perm_set(exe, priority)
}

#[tauri::command(async)]
fn cmd_perm_priority_remove(exe: String) -> Result<Value, String> {
    procmgr::perm_remove(exe)
}

// ---- updates ----
#[tauri::command(async)]
fn cmd_scan_app_updates() -> Result<Value, String> { updates::scan_app_updates() }

#[tauri::command(async)]
fn cmd_update_apps(id: Option<String>) -> Result<Value, String> { updates::update_apps(id) }

#[tauri::command(async)]
fn cmd_scan_driver_updates() -> Value { updates::scan_driver_updates() }

#[tauri::command(async)]
fn cmd_install_driver_updates() -> Result<Value, String> { updates::install_driver_updates() }

#[tauri::command(async)]
fn cmd_gpu_vendor() -> Value { updates::gpu_vendor_hint() }

// ---- privacy center ----
#[tauri::command(async)]
fn cmd_privacy_scan() -> Value { privacy::scan() }

#[tauri::command(async)]
fn cmd_privacy_apply(id: String) -> Result<Value, String> { privacy::apply(id) }

#[tauri::command(async)]
fn cmd_privacy_revert(id: String) -> Result<Value, String> { privacy::revert(id) }

// ---- services manager ----
#[tauri::command(async)]
fn cmd_services_list() -> Value { services::list() }

#[tauri::command(async)]
fn cmd_service_set_startup(name: String, startup_type: String) -> Result<Value, String> {
    services::set_startup(name, startup_type)
}

#[tauri::command(async)]
fn cmd_service_control(name: String, action: String) -> Result<Value, String> {
    services::control(name, action)
}

// ---- health check ----
#[tauri::command(async)]
fn cmd_health_run(kind: String) -> Result<Value, String> { healthcheck::run(kind) }

// ---- boot optimizer ----
#[tauri::command(async)]
fn cmd_boot_scan() -> Value { bootopt::scan() }

#[tauri::command(async)]
fn cmd_boot_tweak_apply(id: String) -> Result<Value, String> { bootopt::apply_tweak(id) }

#[tauri::command(async)]
fn cmd_boot_tweak_revert(id: String) -> Result<Value, String> { bootopt::revert_tweak(id) }

// ---- disk analyzer ----
#[tauri::command(async)]
fn cmd_disk_drives() -> Value { diskanalyzer::drives() }

#[tauri::command(async)]
fn cmd_disk_largest(path: String, limit: usize, app: AppHandle) -> Value {
    diskanalyzer::scan_largest(path, limit, Some(app))
}

#[tauri::command(async)]
fn cmd_disk_duplicates(path: String) -> Value {
    diskanalyzer::scan_duplicates(path)
}

#[tauri::command(async)]
fn cmd_disk_temp_age() -> Value { diskanalyzer::scan_temp_age() }

#[tauri::command(async)]
fn cmd_disk_delete(paths: Vec<String>) -> Value { diskanalyzer::delete_items(paths) }

#[tauri::command(async)]
fn cmd_disk_move(paths: Vec<String>, dest_dir: String) -> Value {
    diskanalyzer::move_items(paths, dest_dir)
}

// ---- scheduled tasks ----
#[tauri::command(async)]
fn cmd_schedtasks_list() -> Value { schedtasks::list() }

#[tauri::command(async)]
fn cmd_schedtask_toggle(path: String, name: String, enable: bool) -> Result<Value, String> {
    schedtasks::toggle(path, name, enable)
}

// ---- debloater ----
#[tauri::command(async)]
fn cmd_debloater_uwp_list() -> Value { debloater::list_uwp() }

#[tauri::command(async)]
fn cmd_debloater_remove_uwp(package_full_name: String) -> Result<String, String> {
    debloater::remove_uwp(package_full_name)
}

#[tauri::command(async)]
fn cmd_debloater_remove_provisioned(package_name: String) -> Result<String, String> {
    debloater::remove_uwp_provisioned(package_name)
}

#[tauri::command(async)]
fn cmd_debloater_tweaks_list() -> Value { debloater::list_tweaks() }

#[tauri::command(async)]
fn cmd_debloater_tweak_apply(id: String) -> Result<String, String> { debloater::apply_tweak(id) }

#[tauri::command(async)]
fn cmd_debloater_tweak_revert(id: String) -> Result<String, String> { debloater::revert_tweak(id) }

// ---- driver manager ----
#[tauri::command(async)]
fn cmd_drivers_list() -> Value { drivers::list_drivers() }

#[tauri::command(async)]
fn cmd_drivers_open_devmgr() -> Result<String, String> { drivers::open_device_manager() }

#[tauri::command(async)]
fn cmd_drivers_open_windows_update() -> Result<String, String> { drivers::open_windows_update() }

#[tauri::command(async)]
fn cmd_drivers_check_winget(package_id: String) -> Value {
    drivers::check_winget_package(package_id)
}

#[tauri::command(async)]
fn cmd_drivers_install_winget(package_id: String) -> Result<String, String> {
    drivers::install_via_winget(package_id)
}

#[tauri::command(async)]
fn cmd_drivers_open_vendor_url(url: String) -> Result<String, String> {
    drivers::open_vendor_url(url)
}

#[tauri::command(async)]
fn cmd_drivers_scan_windows_update() -> Result<String, String> {
    drivers::update_via_pnputil(String::new())
}

// ---- game booster ----
#[tauri::command(async)]
fn cmd_gameboost_background_procs() -> Value { gameboost::list_background_procs() }

#[tauri::command(async)]
fn cmd_gameboost_running_games() -> Value { gameboost::list_running_games() }

#[tauri::command(async)]
fn cmd_gameboost_boost_process(pid: u32) -> Result<String, String> {
    gameboost::boost_process(pid)
}

#[tauri::command(async)]
fn cmd_gameboost_kill_background(pids: Vec<u32>) -> Result<String, String> {
    gameboost::kill_background(pids)
}

#[tauri::command(async)]
fn cmd_gameboost_start(pid: u32, state: State<AppState>) -> Result<String, String> {
    let r = gameboost::boost_start(pid)?;
    *state.boosted_pid.lock().unwrap() = Some(pid);
    Ok(r)
}

#[tauri::command(async)]
fn cmd_gameboost_stop(state: State<AppState>) -> Result<String, String> {
    let r = gameboost::boost_stop()?;
    *state.boosted_pid.lock().unwrap() = None;
    Ok(r)
}

#[tauri::command]
fn cmd_gameboost_get_status(state: State<AppState>) -> Option<u32> {
    *state.boosted_pid.lock().unwrap()
}

#[tauri::command(async)]
fn cmd_gameboost_gpu_perf(enable: bool) -> Result<String, String> {
    gameboost::set_gpu_max_perf(enable)
}


// ---- uninstaller ----
#[tauri::command(async)]
fn cmd_uninstaller_list() -> Value { uninstaller::list_apps() }

#[tauri::command(async)]
fn cmd_uninstall_app(uninstall_string: String) -> Result<String, String> {
    uninstaller::uninstall_app(uninstall_string)
}

#[tauri::command(async)]
fn cmd_scan_leftovers(app_name: String, publisher: String, install_location: String) -> Value {
    uninstaller::scan_leftovers(app_name, publisher, install_location)
}

#[tauri::command(async)]
fn cmd_clean_leftovers(paths: Vec<String>) -> Result<String, String> {
    uninstaller::clean_leftovers(paths)
}

// ---- gpu tweaks ----
#[tauri::command(async)]
fn cmd_gpu_scan() -> Value { gputweaks::scan() }

#[tauri::command(async)]
fn cmd_gpu_tweak_apply(id: String, driver_key: String) -> Result<String, String> {
    gputweaks::do_tweak(id, driver_key, true)
}

#[tauri::command(async)]
fn cmd_gpu_tweak_revert(id: String, driver_key: String) -> Result<String, String> {
    gputweaks::do_tweak(id, driver_key, false)
}

// ---- registry clean ----
#[tauri::command(async)]
fn cmd_regclean_scan() -> Value { regclean::scan() }

#[tauri::command(async)]
fn cmd_regclean_clean(entries: Vec<Value>) -> Result<Value, String> { regclean::clean(entries) }

// ---- disk organizer ----
#[tauri::command(async)]
fn cmd_disk_organize_preview(folder: String, recurse: bool) -> Value {
    diskanalyzer::organize_preview(folder, recurse)
}

#[tauri::command(async)]
fn cmd_disk_organize_apply(items: Vec<Value>) -> Value {
    diskanalyzer::organize_apply(items)
}

// ---- analysis / report ----
#[tauri::command(async)]
fn cmd_analyze(force: bool) -> Value {
    cache::get_or("analysis", force, || {
        let scan     = cache::data_or("scan",     force, crate::scan::full_scan);
        let security = cache::data_or("security", force, crate::security::scan);
        let cleanup  = cache::data_or("cleanup",  force, crate::cleanup::scan);
        analysis::analyze(&scan, &security, &cleanup)
    })
}

#[tauri::command(async)]
fn cmd_generate_report() -> Result<Value, String> {
    let scan     = cache::data_or("scan",      false, crate::scan::full_scan);
    let security = cache::data_or("security",  false, crate::security::scan);
    let cleanup  = cache::data_or("cleanup",   false, crate::cleanup::scan);
    let analysis = cache::data_or("analysis",  false, || {
        crate::analysis::analyze(&scan, &security, &cleanup)
    });
    let history  = crate::tweaks::history();
    report::generate(&scan, &analysis, &security, &history)
}

// ---- game profiles / auto-switcher ----
#[tauri::command(async)]
fn cmd_game_list() -> serde_json::Value { gameprofile::cmd_game_list() }

#[tauri::command(async)]
fn cmd_game_switcher_status(state: State<AppState>) -> serde_json::Value {
    gameprofile::cmd_game_switcher_status(&state.game_switcher)
}

#[tauri::command(async)]
fn cmd_game_switcher_configure(
    state: State<AppState>, enabled: bool, default_preset: String,
) -> serde_json::Value {
    gameprofile::cmd_game_switcher_configure(&state.game_switcher, enabled, default_preset)
}

#[tauri::command(async)]
fn cmd_game_apply_preset(game_id: String, preset: String) -> serde_json::Value {
    gameprofile::cmd_game_apply_preset(game_id, preset)
}

#[tauri::command(async)]
fn cmd_game_revert(state: State<AppState>) -> serde_json::Value {
    gameprofile::cmd_game_revert(&state.game_switcher)
}

// ---- misc ----
#[tauri::command(async)]
fn cmd_open_path(path: String) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

// ---- context menu ----
#[tauri::command(async)]
fn cmd_ctxmenu_list() -> Value { ctxmenu::list_entries() }

#[tauri::command(async)]
fn cmd_ctxmenu_toggle(path: String, enable: bool) -> Result<String, String> {
    ctxmenu::toggle_entry(path, enable)
}

#[tauri::command(async)]
fn cmd_ctxmenu_disable_all() -> Result<String, String> { ctxmenu::disable_all_bloat() }

#[tauri::command(async)]
fn cmd_ctxmenu_enable_all() -> Result<String, String> { ctxmenu::enable_all() }

// ---- power plan ----
#[tauri::command(async)]
fn cmd_powerplan_list() -> Value { powerplan::list_plans() }

#[tauri::command(async)]
fn cmd_powerplan_set(guid: String) -> Result<String, String> { powerplan::set_active(guid) }

#[tauri::command(async)]
fn cmd_powerplan_unlock_ultimate() -> Result<String, String> { powerplan::unlock_ultimate() }

#[tauri::command(async)]
fn cmd_powerplan_delete(guid: String) -> Result<String, String> { powerplan::delete_plan(guid) }

#[tauri::command(async)]
fn cmd_powerplan_create(name: String, base_guid: String) -> Result<String, String> {
    powerplan::create_custom(name, base_guid)
}

// ---- perf tweaks ----
#[tauri::command(async)]
fn cmd_timer_get() -> Value { perftweaks::timer_get() }

#[tauri::command(async)]
fn cmd_timer_set(target100ns: u32) -> Result<String, String> { perftweaks::timer_set(target100ns) }

#[tauri::command(async)]
fn cmd_timer_reset() -> Result<String, String> { perftweaks::timer_reset() }

#[tauri::command(async)]
fn cmd_msi_list() -> Value { perftweaks::msi_list() }

#[tauri::command(async)]
fn cmd_msi_set(reg_path: String, enabled: bool) -> Result<String, String> {
    perftweaks::msi_set(reg_path, enabled)
}

#[tauri::command(async)]
fn cmd_net_adapters() -> Value { perftweaks::net_adapters() }

#[tauri::command(async)]
fn cmd_net_tweak(adapter: String, keyword: String, value: u32) -> Result<String, String> {
    perftweaks::net_tweak(adapter, keyword, value)
}

#[tauri::command(async)]
fn cmd_net_tweak_all_gaming() -> Result<String, String> { perftweaks::net_tweak_all_gaming() }

#[tauri::command(async)]
fn cmd_net_reset_all() -> Result<String, String> { perftweaks::net_reset_all() }

#[tauri::command(async)]
fn cmd_ram_info() -> Value { perftweaks::ram_info() }

#[tauri::command(async)]
fn cmd_ram_flush_standby() -> Result<String, String> { perftweaks::ram_flush_standby() }

#[tauri::command(async)]
fn cmd_pagefile_info() -> Value { perftweaks::pagefile_info() }

#[tauri::command(async)]
fn cmd_pagefile_set_auto() -> Result<String, String> { perftweaks::pagefile_set_auto() }

#[tauri::command(async)]
fn cmd_pagefile_set_custom(path: String, init_mb: u32, max_mb: u32) -> Result<String, String> {
    perftweaks::pagefile_set_custom(path, init_mb, max_mb)
}

#[tauri::command(async)]
fn cmd_pagefile_disable() -> Result<String, String> { perftweaks::pagefile_disable() }

// ---- hw monitor ----
#[tauri::command(async)]
fn cmd_hw_temps() -> Value { hwmonitor::temps() }

#[tauri::command(async)]
fn cmd_hw_smart() -> Value { hwmonitor::smart() }

#[tauri::command(async)]
fn cmd_hw_full() -> Value { hwmonitor::full() }

#[tauri::command(async)]
fn cmd_hw_profile() -> Value { hwprofile::hw_profile() }

// ═══════════════════════════════════════════════════════════════════════════
// Tauri entry point
// ═══════════════════════════════════════════════════════════════════════════


#[tauri::command(async)]
fn cmd_disable_scheduled_task(task_path: String, task_name: String) -> Result<String, String> {
    security::disable_scheduled_task(task_path, task_name)
}
#[tauri::command(async)]
fn cmd_enable_scheduled_task(task_path: String, task_name: String) -> Result<String, String> {
    security::enable_scheduled_task(task_path, task_name)
}
#[tauri::command(async)]
fn cmd_defender_set_realtime(enabled: bool) -> Result<String, String> {
    security::defender_set_realtime(enabled)
}
#[tauri::command(async)]
fn cmd_defender_set_cloud(enabled: bool) -> Result<String, String> {
    security::defender_set_cloud(enabled)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Re-launch the current process with UAC elevation if not already admin.
/// Returns immediately if already elevated or not on Windows.
fn ensure_admin() {
    #[cfg(windows)]
    {
        let is_admin = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command",
                "([Security.Principal.WindowsPrincipal]\
                 [Security.Principal.WindowsIdentity]::GetCurrent())\
                 .IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        if !is_admin {
            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new("powershell")
                    .args([
                        "-NoProfile", "-NonInteractive", "-Command",
                        &format!("Start-Process '{}' -Verb RunAs", exe.display()),
                    ])
                    .spawn();
                std::process::exit(0);
            }
        }
    }
}

pub fn run() {
    ensure_admin();
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState {
            monitor:       monitor::MonitorState::default(),
            game_switcher: gameprofile::new_state(),
            boosted_pid:   std::sync::Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            // system scan
            cmd_is_admin,
            cmd_full_scan,
            cmd_boot_analysis,
            cmd_event_logs,
            cmd_component_health,
            cmd_dns_benchmark,
            cmd_network_diag,
            // monitor
            cmd_start_monitor,
            cmd_stop_monitor,
            // tweaks
            cmd_list_tweaks,
            cmd_apply_tweak,
            cmd_revert_tweak,
            cmd_history,
            // safety
            cmd_create_restore_point,
            cmd_list_restore_points,
            cmd_delete_restore_point,
            cmd_launch_rstrui,
            // cleanup
            cmd_scan_cleanup,
            cmd_run_cleanup,
            // security
            cmd_security_scan,
            cmd_defender_quick_scan,
            cmd_hosts_list_all,
            cmd_hosts_disable_entries,
            cmd_hosts_enable_entries,
            cmd_disable_scheduled_task,
            cmd_enable_scheduled_task,
            cmd_defender_set_realtime,
            cmd_defender_set_cloud,
            // uninstaller
            cmd_uninstaller_list,
            cmd_uninstall_app,
            cmd_scan_leftovers,
            cmd_clean_leftovers,
            // context menu
            cmd_ctxmenu_list,
            cmd_ctxmenu_toggle,
            cmd_ctxmenu_disable_all,
            cmd_ctxmenu_enable_all,
            // power plan
            cmd_powerplan_list,
            cmd_powerplan_set,
            cmd_powerplan_unlock_ultimate,
            cmd_powerplan_delete,
            cmd_powerplan_create,
            // perf tweaks
            cmd_timer_get,
            cmd_timer_set,
            cmd_timer_reset,
            cmd_msi_list,
            cmd_msi_set,
            cmd_net_adapters,
            cmd_net_tweak,
            cmd_net_tweak_all_gaming,
            cmd_net_reset_all,
            cmd_ram_info,
            cmd_ram_flush_standby,
            cmd_pagefile_info,
            cmd_pagefile_set_auto,
            cmd_pagefile_set_custom,
            cmd_pagefile_disable,
            // hw monitor
            cmd_hw_temps,
            cmd_hw_smart,
            cmd_hw_full,
            cmd_hw_profile,
            // debloater
            cmd_debloater_uwp_list,
            cmd_debloater_remove_uwp,
            cmd_debloater_remove_provisioned,
            cmd_debloater_tweaks_list,
            cmd_debloater_tweak_apply,
            cmd_debloater_tweak_revert,
            // drivers
            cmd_drivers_list,
            cmd_drivers_open_devmgr,
            cmd_drivers_open_windows_update,
            cmd_drivers_check_winget,
            cmd_drivers_install_winget,
            cmd_drivers_open_vendor_url,
            cmd_drivers_scan_windows_update,
            // game booster
            cmd_gameboost_background_procs,
            cmd_gameboost_running_games,
            cmd_gameboost_boost_process,
            cmd_gameboost_kill_background,
            cmd_gameboost_start,
            cmd_gameboost_stop,
            cmd_gameboost_get_status,
            cmd_gameboost_gpu_perf,
            // privacy
            cmd_privacy_scan,
            cmd_privacy_apply,
            cmd_privacy_revert,
            // services
            cmd_services_list,
            cmd_service_set_startup,
            cmd_service_control,
            // health
            cmd_health_run,
            // process manager
            cmd_proc_list,
            cmd_proc_kill,
            cmd_proc_priority,
            cmd_proc_affinity,
            cmd_perm_priority_list,
            cmd_perm_priority_set,
            cmd_perm_priority_remove,
            // updates
            cmd_scan_app_updates,
            cmd_update_apps,
            cmd_scan_driver_updates,
            cmd_install_driver_updates,
            cmd_gpu_vendor,
            // latency / WPR
            cmd_latency_counters,
            cmd_stall_probe,
            cmd_wpr_start,
            cmd_wpr_stop,
            cmd_wpr_cancel,
            // GPU tweaks
            cmd_gpu_scan,
            cmd_gpu_tweak_apply,
            cmd_gpu_tweak_revert,
            // registry clean
            cmd_regclean_scan,
            cmd_regclean_clean,
            // boot
            cmd_boot_scan,
            cmd_boot_tweak_apply,
            cmd_boot_tweak_revert,
            // disk analyzer
            cmd_disk_drives,
            cmd_disk_largest,
            cmd_disk_duplicates,
            cmd_disk_temp_age,
            cmd_disk_delete,
            cmd_disk_move,
            cmd_disk_organize_preview,
            cmd_disk_organize_apply,
            // sched tasks
            cmd_schedtasks_list,
            cmd_schedtask_toggle,
            // benchmarks / profiles
            cmd_run_benchmark,
            cmd_bench_history,
            cmd_profile_list,
            cmd_profile_apply,
            cmd_profile_revert,
            // startup
            cmd_startup_list,
            cmd_startup_toggle,
            // analysis / report
            cmd_analyze,
            cmd_generate_report,
            // auto optimizer
            cmd_autoopt_scan,
            cmd_autoopt_score,
            cmd_autoopt_apply,
            // game profiles / auto-switcher
            cmd_game_list,
            cmd_game_switcher_status,
            cmd_game_switcher_configure,
            cmd_game_apply_preset,
            cmd_game_revert,
            // restore points
            cmd_create_restore_point,
            cmd_list_restore_points,
            cmd_delete_restore_point,
            cmd_launch_rstrui,
            // misc
            cmd_open_path,
        ])
        .setup(move |app| {
            let gs = app.state::<AppState>().game_switcher.clone();
                     gameprofile::start(gs, app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running tauri app");
}
