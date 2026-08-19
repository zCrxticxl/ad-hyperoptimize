mod analysis;
mod autoopt;
mod bench;
mod bootopt;
mod cache;
mod cleanup;
#[cfg(windows)]
mod ctxmenu;
mod debloater;
mod diskanalyzer;
mod drivers;
mod gameboost;
mod gamedb;
mod gameprofile;
mod gputweaks;
mod healthcheck;
mod hwmonitor;
mod hwprofile;
mod latency;
mod monitor;
mod perftweaks;
mod powerplan;
mod privacy;
mod procmgr;
mod profiles;
mod ps;
mod regclean;
mod report;
mod safety;
mod scan;
mod schedtasks;
mod security;
mod services;
mod softwareinstaller;
mod startup;
mod tweaks;
mod uninstaller;
mod updates;

use serde_json::Value;
use tauri::{AppHandle, Manager, State};

struct AppState {
    monitor: monitor::MonitorState,
    game_switcher: gameprofile::SharedState,
    boosted_pid: std::sync::Mutex<Option<u32>>,
}

// ---- diagnostics ----
#[tauri::command]
fn cmd_is_admin() -> bool {
    ps::is_admin()
}

#[tauri::command(async)]
async fn cmd_full_scan(force: Option<bool>) -> Value {
    tauri::async_runtime::spawn_blocking(move || {
        cache::get_or("scan", force.unwrap_or(false), scan::full_scan)
    })
    .await
    .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_boot_analysis() -> Value {
    tauri::async_runtime::spawn_blocking(scan::boot_analysis)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_event_logs() -> Value {
    tauri::async_runtime::spawn_blocking(scan::event_log_summary)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_component_health() -> Value {
    tauri::async_runtime::spawn_blocking(scan::component_health)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_dns_benchmark() -> Value {
    tauri::async_runtime::spawn_blocking(scan::dns_benchmark)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_network_diag() -> Value {
    tauri::async_runtime::spawn_blocking(scan::network_diag)
        .await
        .unwrap_or_default()
}

// ---- monitoring ----
#[tauri::command]
fn cmd_start_monitor(app: AppHandle, state: State<'_, AppState>) {
    let _ = monitor::start(app, &state.monitor);
}

#[tauri::command]
fn cmd_stop_monitor(state: State<'_, AppState>) {
    monitor::stop(&state.monitor);
}

// ---- optimization engine ----
#[tauri::command(async)]
async fn cmd_list_tweaks() -> Value {
    tauri::async_runtime::spawn_blocking(tweaks::list_with_status)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_apply_tweak(id: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || tweaks::apply(&id))
        .await
        .map_err(|_| "apply task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_revert_tweak(id: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || tweaks::revert(&id))
        .await
        .map_err(|_| "revert task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_history() -> Value {
    tauri::async_runtime::spawn_blocking(tweaks::history)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_revert_entry(entry_id: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || tweaks::revert_entry(&entry_id))
        .await
        .map_err(|_| "revert task panicked".to_string())?
}

// ---- safety / restore points ----
#[tauri::command(async)]
async fn cmd_create_restore_point(description: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || safety::create_restore_point(&description))
        .await
        .map_err(|_| "restore point task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_list_restore_points() -> Value {
    tauri::async_runtime::spawn_blocking(safety::list_restore_points)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_delete_restore_point(sequence_number: u32) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || safety::delete_restore_point(sequence_number))
        .await
        .map_err(|_| "restore point task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_launch_rstrui() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(safety::launch_rstrui)
        .await
        .map_err(|_| "restore point task panicked".to_string())?
}

// ---- auto optimizer ----
#[tauri::command(async)]
async fn cmd_autoopt_scan() -> Value {
    tauri::async_runtime::spawn_blocking(autoopt::scan)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_autoopt_score() -> Value {
    tauri::async_runtime::spawn_blocking(autoopt::score)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_autoopt_apply(items: Vec<Value>) -> Value {
    tauri::async_runtime::spawn_blocking(move || autoopt::apply_selected(items))
        .await
        .unwrap_or_default()
}

// ---- cleanup ----
#[tauri::command(async)]
async fn cmd_scan_cleanup(force: Option<bool>) -> Value {
    tauri::async_runtime::spawn_blocking(move || {
        cache::get_or("cleanup", force.unwrap_or(false), cleanup::scan)
    })
    .await
    .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_run_cleanup(ids: Vec<String>) -> Value {
    tauri::async_runtime::spawn_blocking(move || cleanup::clean(ids))
        .await
        .unwrap_or_default()
}

// ---- security ----
#[tauri::command(async)]
async fn cmd_security_scan(force: Option<bool>) -> Value {
    tauri::async_runtime::spawn_blocking(move || {
        cache::get_or("security", force.unwrap_or(false), security::scan)
    })
    .await
    .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_defender_quick_scan() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(security::defender_quick_scan)
        .await
        .map_err(|_| "scan task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_hosts_list_all() -> Value {
    tauri::async_runtime::spawn_blocking(security::hosts_list_all)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_hosts_disable_entries(entries: Vec<String>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || security::hosts_disable_entries(entries))
        .await
        .map_err(|_| "hosts task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_hosts_enable_entries(entries: Vec<String>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || security::hosts_enable_entries(entries))
        .await
        .map_err(|_| "hosts task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_security_disable_driver(device_id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || security::disable_unsigned_driver(device_id))
        .await
        .map_err(|_| "driver task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_security_enable_driver(device_id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || security::enable_unsigned_driver(device_id))
        .await
        .map_err(|_| "driver task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_security_remove_driver(device_id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || security::remove_unsigned_driver(device_id))
        .await
        .map_err(|_| "driver task panicked".to_string())?
}

// ---- benchmarks ----
#[tauri::command(async)]
async fn cmd_run_benchmark(kind: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || bench::run(&kind))
        .await
        .map_err(|_| "benchmark task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_bench_history() -> Value {
    tauri::async_runtime::spawn_blocking(bench::history)
        .await
        .unwrap_or_default()
}

// ---- latency analysis ----
#[tauri::command(async)]
async fn cmd_latency_counters(samples: u32) -> Value {
    tauri::async_runtime::spawn_blocking(move || latency::counters(samples))
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_stall_probe(seconds: u32) -> Value {
    tauri::async_runtime::spawn_blocking(move || latency::stall_probe(seconds))
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_wpr_start() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(latency::wpr_start)
        .await
        .map_err(|_| "wpr task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_wpr_stop() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(latency::wpr_stop)
        .await
        .map_err(|_| "wpr task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_wpr_cancel() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(latency::wpr_cancel)
        .await
        .map_err(|_| "wpr task panicked".to_string())?
}

// ---- profiles ----
#[tauri::command(async)]
fn cmd_profile_list() -> Value {
    profiles::list()
}

#[tauri::command(async)]
async fn cmd_profile_apply(id: String, with_bench: bool) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || profiles::apply(&id, with_bench))
        .await
        .map_err(|_| "profile task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_profile_revert(id: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || profiles::revert(&id))
        .await
        .map_err(|_| "profile task panicked".to_string())?
}

// ---- startup manager ----
#[tauri::command(async)]
fn cmd_startup_list() -> Value {
    startup::list()
}

#[tauri::command(async)]
fn cmd_startup_toggle(scope: String, name: String, enable: bool) -> Result<Value, String> {
    startup::toggle(scope, name, enable)
}

// ---- process manager ----
#[tauri::command(async)]
async fn cmd_proc_list() -> Value {
    tauri::async_runtime::spawn_blocking(procmgr::list)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_proc_kill(pid: u32) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || procmgr::kill(pid))
        .await
        .map_err(|_| "process task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_proc_priority(pid: u32, priority: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || procmgr::set_priority(pid, priority))
        .await
        .map_err(|_| "process task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_proc_affinity(pid: u32, mask: u64) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || procmgr::set_affinity(pid, mask))
        .await
        .map_err(|_| "process task panicked".to_string())?
}

#[tauri::command(async)]
fn cmd_perm_priority_list() -> Value {
    procmgr::perm_list()
}

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
async fn cmd_scan_app_updates() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(updates::scan_app_updates)
        .await
        .map_err(|_| "update scan task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_update_apps(id: Option<String>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || updates::update_apps(id))
        .await
        .map_err(|_| "update task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_scan_driver_updates() -> Value {
    tauri::async_runtime::spawn_blocking(updates::scan_driver_updates)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_install_driver_updates() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(updates::install_driver_updates)
        .await
        .map_err(|_| "update task panicked".to_string())?
}

#[tauri::command(async)]
fn cmd_gpu_vendor() -> Value {
    updates::gpu_vendor_hint()
}

// ---- privacy center ----
#[tauri::command(async)]
async fn cmd_privacy_scan() -> Value {
    tauri::async_runtime::spawn_blocking(privacy::scan)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_privacy_apply(id: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || privacy::apply(id))
        .await
        .map_err(|_| "privacy task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_privacy_revert(id: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || privacy::revert(id))
        .await
        .map_err(|_| "privacy task panicked".to_string())?
}

// ---- services manager ----
#[tauri::command(async)]
async fn cmd_services_list() -> Value {
    tauri::async_runtime::spawn_blocking(services::list)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_service_set_startup(name: String, startup_type: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || services::set_startup(name, startup_type))
        .await
        .map_err(|_| "service task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_service_control(name: String, action: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || services::control(name, action))
        .await
        .map_err(|_| "service task panicked".to_string())?
}

// ---- health check ----
#[tauri::command(async)]
async fn cmd_health_run(kind: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || healthcheck::run(kind))
        .await
        .map_err(|_| "health task panicked".to_string())?
}

// ---- boot optimizer ----
#[tauri::command(async)]
async fn cmd_boot_scan() -> Value {
    tauri::async_runtime::spawn_blocking(bootopt::scan)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_boot_tweak_apply(id: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || bootopt::apply_tweak(id))
        .await
        .map_err(|_| "boot tweak task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_boot_tweak_revert(id: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || bootopt::revert_tweak(id))
        .await
        .map_err(|_| "boot tweak task panicked".to_string())?
}

// ---- disk analyzer ----
#[tauri::command(async)]
async fn cmd_disk_drives() -> Value {
    tauri::async_runtime::spawn_blocking(diskanalyzer::drives)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_disk_largest(path: String, limit: usize, app: AppHandle) -> Value {
    tauri::async_runtime::spawn_blocking(move || diskanalyzer::scan_largest(path, limit, Some(app)))
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_disk_duplicates(path: String) -> Value {
    tauri::async_runtime::spawn_blocking(move || diskanalyzer::scan_duplicates(path))
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_disk_temp_age() -> Value {
    tauri::async_runtime::spawn_blocking(diskanalyzer::scan_temp_age)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_disk_delete(paths: Vec<String>) -> Value {
    tauri::async_runtime::spawn_blocking(move || diskanalyzer::delete_items(paths))
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_disk_move(paths: Vec<String>, dest_dir: String) -> Value {
    tauri::async_runtime::spawn_blocking(move || diskanalyzer::move_items(paths, dest_dir))
        .await
        .unwrap_or_default()
}

// ---- scheduled tasks ----
#[tauri::command(async)]
async fn cmd_schedtasks_list() -> Value {
    tauri::async_runtime::spawn_blocking(schedtasks::list)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_schedtask_toggle(path: String, name: String, enable: bool) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || schedtasks::toggle(path, name, enable))
        .await
        .map_err(|_| "scheduled task panicked".to_string())?
}

// ---- debloater ----
#[tauri::command(async)]
async fn cmd_debloater_uwp_list() -> Value {
    tauri::async_runtime::spawn_blocking(debloater::list_uwp)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_debloater_remove_uwp(package_full_name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || debloater::remove_uwp(package_full_name))
        .await
        .map_err(|_| "debloater task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_debloater_remove_provisioned(package_name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || debloater::remove_uwp_provisioned(package_name))
        .await
        .map_err(|_| "debloater task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_debloater_tweaks_list() -> Value {
    tauri::async_runtime::spawn_blocking(debloater::list_tweaks)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_debloater_tweak_apply(id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || debloater::apply_tweak(id))
        .await
        .map_err(|_| "debloater task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_debloater_tweak_revert(id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || debloater::revert_tweak(id))
        .await
        .map_err(|_| "debloater task panicked".to_string())?
}

// ---- driver manager ----
#[tauri::command(async)]
async fn cmd_drivers_list() -> Value {
    tauri::async_runtime::spawn_blocking(drivers::list_drivers)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_drivers_open_devmgr() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(drivers::open_device_manager)
        .await
        .map_err(|_| "driver task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_drivers_open_windows_update() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(drivers::open_windows_update)
        .await
        .map_err(|_| "driver task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_drivers_check_winget(package_id: String) -> Value {
    tauri::async_runtime::spawn_blocking(move || drivers::check_winget_package(package_id))
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_drivers_install_winget(package_id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || drivers::install_via_winget(package_id))
        .await
        .map_err(|_| "driver task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_drivers_open_vendor_url(url: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || drivers::open_vendor_url(url))
        .await
        .map_err(|_| "driver task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_drivers_scan_windows_update() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(drivers::update_via_pnputil)
        .await
        .map_err(|_| "driver task panicked".to_string())?
}

// ---- game booster ----
#[tauri::command(async)]
async fn cmd_gameboost_background_procs() -> Value {
    tauri::async_runtime::spawn_blocking(gameboost::list_background_procs)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_gameboost_running_games() -> Value {
    tauri::async_runtime::spawn_blocking(gameboost::list_running_games)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_gameboost_boost_process(pid: u32) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || gameboost::boost_process(pid))
        .await
        .map_err(|_| "game booster task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_gameboost_kill_background(pids: Vec<u32>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || gameboost::kill_background(pids))
        .await
        .map_err(|_| "game booster task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_gameboost_start(pid: u32, state: State<'_, AppState>) -> Result<String, String> {
    let r = tauri::async_runtime::spawn_blocking(move || gameboost::boost_start(pid))
        .await
        .map_err(|_| "game booster task panicked".to_string())??;
    *state.boosted_pid.lock().unwrap_or_else(|e| e.into_inner()) = Some(pid);
    Ok(r)
}

#[tauri::command(async)]
async fn cmd_gameboost_stop(state: State<'_, AppState>) -> Result<String, String> {
    let r = tauri::async_runtime::spawn_blocking(gameboost::boost_stop)
        .await
        .map_err(|_| "game booster task panicked".to_string())??;
    *state.boosted_pid.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(r)
}

#[tauri::command]
fn cmd_gameboost_get_status(state: State<'_, AppState>) -> Option<u32> {
    *state.boosted_pid.lock().unwrap_or_else(|e| e.into_inner())
}

#[tauri::command(async)]
async fn cmd_gameboost_gpu_perf(enable: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || gameboost::set_gpu_max_perf(enable))
        .await
        .map_err(|_| "game booster task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_gameboost_quick_boost(process_name: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || gameboost::quick_boost_start(process_name))
        .await
        .map_err(|_| "game booster task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_gameboost_quick_boost_revert(restore_token: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || gameboost::quick_boost_revert(restore_token))
        .await
        .map_err(|_| "game booster task panicked".to_string())?
}

// ---- uninstaller ----
#[tauri::command(async)]
async fn cmd_uninstaller_list() -> Value {
    tauri::async_runtime::spawn_blocking(uninstaller::list_apps)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_uninstall_app(uninstall_string: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || uninstaller::uninstall_app(uninstall_string))
        .await
        .map_err(|_| "uninstall task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_scan_leftovers(
    app_name: String,
    publisher: String,
    install_location: String,
) -> Value {
    tauri::async_runtime::spawn_blocking(move || {
        uninstaller::scan_leftovers(app_name, publisher, install_location)
    })
    .await
    .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_clean_leftovers(paths: Vec<String>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || uninstaller::clean_leftovers(paths))
        .await
        .map_err(|_| "cleanup task panicked".to_string())?
}

// ---- gpu tweaks ----
#[tauri::command(async)]
async fn cmd_gpu_scan() -> Value {
    tauri::async_runtime::spawn_blocking(gputweaks::scan)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_gpu_tweak_apply(id: String, driver_key: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || gputweaks::do_tweak(id, driver_key, true))
        .await
        .map_err(|_| "gpu tweak task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_gpu_tweak_revert(id: String, driver_key: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || gputweaks::do_tweak(id, driver_key, false))
        .await
        .map_err(|_| "gpu tweak task panicked".to_string())?
}

// ---- nvidia control panel ----
#[tauri::command(async)]
async fn cmd_nv_get_settings() -> Value {
    tauri::async_runtime::spawn_blocking(gputweaks::nv_get_settings)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_nv_set_setting(setting: String, value: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || gputweaks::nv_set_setting(setting, value))
        .await
        .map_err(|_| "nvidia task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_nv_open_panel() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(gputweaks::nv_open_panel)
        .await
        .map_err(|_| "nvidia task panicked".to_string())?
}

// ---- registry clean ----
#[tauri::command(async)]
async fn cmd_regclean_scan() -> Value {
    tauri::async_runtime::spawn_blocking(regclean::scan)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_regclean_clean(entries: Vec<Value>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || regclean::clean(entries))
        .await
        .map_err(|_| "registry clean task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_regclean_list_backups() -> Value {
    tauri::async_runtime::spawn_blocking(regclean::list_backups)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_regclean_restore(backup_path: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || regclean::restore(backup_path))
        .await
        .map_err(|_| "registry clean task panicked".to_string())?
}

// ---- disk organizer ----
#[tauri::command(async)]
async fn cmd_disk_organize_preview(folder: String, recurse: bool) -> Value {
    tauri::async_runtime::spawn_blocking(move || diskanalyzer::organize_preview(folder, recurse))
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_disk_organize_apply(items: Vec<Value>) -> Value {
    tauri::async_runtime::spawn_blocking(move || diskanalyzer::organize_apply(items))
        .await
        .unwrap_or_default()
}

// ---- analysis / report ----
#[tauri::command(async)]
async fn cmd_analyze(force: bool) -> Value {
    // v2: findings now carry `code`+`params` for i18n (old cached "analysis"
    // entries lack these fields and crash the localizer) — bump the cache key
    // so stale pre-v2 files on disk are ignored instead of served.
    tauri::async_runtime::spawn_blocking(move || {
        cache::get_or("analysis_v2", force, || {
            let scan = cache::data_or("scan", force, crate::scan::full_scan);
            let security = cache::data_or("security", force, crate::security::scan);
            let cleanup = cache::data_or("cleanup", force, crate::cleanup::scan);
            analysis::analyze(&scan, &security, &cleanup)
        })
    })
    .await
    .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_generate_report() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let scan = cache::data_or("scan", false, crate::scan::full_scan);
        let security = cache::data_or("security", false, crate::security::scan);
        let cleanup = cache::data_or("cleanup", false, crate::cleanup::scan);
        let analysis = cache::data_or("analysis_v2", false, || {
            crate::analysis::analyze(&scan, &security, &cleanup)
        });
        let history = crate::tweaks::history();
        report::generate(&scan, &analysis, &security, &history)
    })
    .await
    .map_err(|_| "report task panicked".to_string())?
}

// ---- game profiles / auto-switcher ----
#[tauri::command(async)]
async fn cmd_game_list() -> serde_json::Value {
    tauri::async_runtime::spawn_blocking(gameprofile::cmd_game_list)
        .await
        .unwrap_or_default()
}

#[tauri::command]
fn cmd_game_switcher_status(state: State<'_, AppState>) -> serde_json::Value {
    gameprofile::cmd_game_switcher_status(&state.game_switcher)
}

#[tauri::command]
fn cmd_game_switcher_configure(
    state: State<'_, AppState>,
    enabled: bool,
    default_preset: String,
) -> serde_json::Value {
    gameprofile::cmd_game_switcher_configure(&state.game_switcher, enabled, default_preset)
}

#[tauri::command(async)]
async fn cmd_game_apply_preset(game_id: String, preset: String) -> serde_json::Value {
    tauri::async_runtime::spawn_blocking(move || {
        gameprofile::cmd_game_apply_preset(game_id, preset)
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
fn cmd_game_revert(state: State<'_, AppState>) -> serde_json::Value {
    gameprofile::cmd_game_revert(&state.game_switcher)
}

// ---- misc ----
#[tauri::command]
fn cmd_clear_cache() -> Result<String, String> {
    let n = cache::clear()?;
    Ok(format!("Cleared {n} cached scans"))
}

#[tauri::command(async)]
fn cmd_open_path(path: String) -> Result<(), String> {
    // Start-Process opens URLs, files and folders via their default handler
    // (browser / file association / Explorer). It is spawned with the value as
    // a single single-quoted argument through ps::run — PowerShell performs no
    // cmd-style percent/operator expansion inside single quotes, so
    // percent-encoded URLs (&, %, =, ?) pass through verbatim and the
    // renderer-supplied value cannot break out of the argument.
    validate_open_path(&path)?;
    let safe = path.replace('\'', "''");
    crate::ps::run(&format!("Start-Process '{safe}'")).map(|_| ())
}

/// `path` is renderer-supplied (including registry-derived install locations
/// and percent-encoded buy links), so reject anything that could alter the
/// single-quoted PowerShell argument: control characters (newline injection)
/// and anything that is neither an http(s) URL nor an absolute local path
/// (drive letter or UNC).
fn validate_open_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.len() > 4096 {
        return Err("invalid path".into());
    }
    if path.bytes().any(|b| b < 0x20) {
        return Err("path contains control characters".into());
    }
    let upper = path.to_ascii_uppercase();
    let is_url = upper.starts_with("HTTP://") || upper.starts_with("HTTPS://");
    let is_abs = (upper.len() >= 3
        && upper.as_bytes()[0].is_ascii_alphabetic()
        && upper.as_bytes()[1] == b':'
        && upper.as_bytes()[2] == b'\\')
        || upper.starts_with("\\\\");
    if !is_url && !is_abs {
        return Err("path must be an http(s) URL or an absolute local path".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_open_path;

    #[test]
    fn open_path_accepts_urls_and_abs_paths() {
        assert!(validate_open_path("https://discord.gg/vFaKsVuxKP").is_ok());
        assert!(validate_open_path("http://example.com/x").is_ok());
        // percent-encoded query links (PcConfigurator buy buttons)
        assert!(
            validate_open_path(
                "https://www.bestbuy.com/site/searchpage.jsp?st=NVIDIA%20GeForce%20RTX%205090&_dyncharset=UTF-8"
            )
            .is_ok()
        );
        assert!(validate_open_path(r"C:\Program Files (x86)\App & More").is_ok());
        assert!(validate_open_path(
            r"C:\Users\Me\AppData\Roaming\PCOptSuite\reports\report-1.html"
        )
        .is_ok());
        assert!(validate_open_path(r"\\server\share\folder").is_ok());
    }

    #[test]
    fn open_path_rejects_injection() {
        assert!(validate_open_path("").is_err());
        assert!(validate_open_path("calc.exe").is_err()); // relative
        assert!(validate_open_path("not a url").is_err());
        assert!(validate_open_path("https://x.com/a\ncalc").is_err()); // newline
        assert!(validate_open_path("https://x.com/a\rcalc").is_err());
        assert!(validate_open_path(&"x".repeat(4097)).is_err());
    }
}

// ---- context menu (Windows only: uses the winreg crate) ----
#[cfg(windows)]
#[tauri::command(async)]
fn cmd_ctxmenu_list() -> Value {
    ctxmenu::list_entries()
}

#[cfg(windows)]
#[tauri::command(async)]
fn cmd_ctxmenu_toggle(path: String, enable: bool) -> Result<String, String> {
    ctxmenu::toggle_entry(path, enable)
}

#[cfg(windows)]
#[tauri::command(async)]
fn cmd_ctxmenu_disable_all() -> Result<String, String> {
    ctxmenu::disable_all_bloat()
}

#[cfg(windows)]
#[tauri::command(async)]
fn cmd_ctxmenu_enable_all() -> Result<String, String> {
    ctxmenu::enable_all()
}

// ---- power plan ----
#[tauri::command(async)]
async fn cmd_powerplan_list() -> Value {
    tauri::async_runtime::spawn_blocking(powerplan::list_plans)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_powerplan_set(guid: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || powerplan::set_active(guid))
        .await
        .map_err(|_| "power plan task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_powerplan_unlock_ultimate() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(powerplan::unlock_ultimate)
        .await
        .map_err(|_| "power plan task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_powerplan_delete(guid: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || powerplan::delete_plan(guid))
        .await
        .map_err(|_| "power plan task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_powerplan_create(name: String, base_guid: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || powerplan::create_custom(name, base_guid))
        .await
        .map_err(|_| "power plan task panicked".to_string())?
}

// ---- perf tweaks ----
#[tauri::command(async)]
async fn cmd_timer_get() -> Value {
    tauri::async_runtime::spawn_blocking(perftweaks::timer_get)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_timer_set(target100ns: u32) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || perftweaks::timer_set(target100ns))
        .await
        .map_err(|_| "perf tweak task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_timer_reset() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(perftweaks::timer_reset)
        .await
        .map_err(|_| "perf tweak task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_msi_list() -> Value {
    tauri::async_runtime::spawn_blocking(perftweaks::msi_list)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_msi_set(reg_path: String, enabled: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || perftweaks::msi_set(reg_path, enabled))
        .await
        .map_err(|_| "perf tweak task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_net_adapters() -> Value {
    tauri::async_runtime::spawn_blocking(perftweaks::net_adapters)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_net_tweak(adapter: String, keyword: String, value: u32) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || perftweaks::net_tweak(adapter, keyword, value))
        .await
        .map_err(|_| "perf tweak task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_net_tweak_all_gaming() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(perftweaks::net_tweak_all_gaming)
        .await
        .map_err(|_| "perf tweak task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_net_reset_all() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(perftweaks::net_reset_all)
        .await
        .map_err(|_| "perf tweak task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_ram_info() -> Value {
    tauri::async_runtime::spawn_blocking(perftweaks::ram_info)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_ram_flush_standby() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(perftweaks::ram_flush_standby)
        .await
        .map_err(|_| "perf tweak task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_pagefile_info() -> Value {
    tauri::async_runtime::spawn_blocking(perftweaks::pagefile_info)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_pagefile_set_auto() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(perftweaks::pagefile_set_auto)
        .await
        .map_err(|_| "perf tweak task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_pagefile_set_custom(
    path: String,
    init_mb: u32,
    max_mb: u32,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        perftweaks::pagefile_set_custom(path, init_mb, max_mb)
    })
    .await
    .map_err(|_| "perf tweak task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_pagefile_disable() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(perftweaks::pagefile_disable)
        .await
        .map_err(|_| "perf tweak task panicked".to_string())?
}

// ---- hw monitor ----
#[tauri::command(async)]
async fn cmd_hw_temps() -> Value {
    tauri::async_runtime::spawn_blocking(hwmonitor::temps)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_hw_smart() -> Value {
    tauri::async_runtime::spawn_blocking(hwmonitor::smart)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_hw_full() -> Value {
    tauri::async_runtime::spawn_blocking(hwmonitor::full)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_hw_profile() -> Value {
    tauri::async_runtime::spawn_blocking(hwprofile::hw_profile)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
fn cmd_sw_catalog() -> Value {
    softwareinstaller::catalog()
}

#[tauri::command(async)]
async fn cmd_sw_check_installed() -> Value {
    tauri::async_runtime::spawn_blocking(softwareinstaller::check_installed)
        .await
        .unwrap_or_default()
}

#[tauri::command]
fn cmd_sw_install(winget_ids: Vec<String>, app: AppHandle) {
    softwareinstaller::install_apps(winget_ids, app);
}

// ═══════════════════════════════════════════════════════════════════════════
// Tauri entry point
// ═══════════════════════════════════════════════════════════════════════════

#[tauri::command(async)]
async fn cmd_disable_scheduled_task(
    task_path: String,
    task_name: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        security::disable_scheduled_task(task_path, task_name)
    })
    .await
    .map_err(|_| "scheduled task panicked".to_string())?
}
#[tauri::command(async)]
async fn cmd_enable_scheduled_task(task_path: String, task_name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        security::enable_scheduled_task(task_path, task_name)
    })
    .await
    .map_err(|_| "scheduled task panicked".to_string())?
}
#[tauri::command(async)]
async fn cmd_defender_set_realtime(enabled: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || security::defender_set_realtime(enabled))
        .await
        .map_err(|_| "defender task panicked".to_string())?
}
#[tauri::command(async)]
async fn cmd_defender_set_cloud(enabled: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || security::defender_set_cloud(enabled))
        .await
        .map_err(|_| "defender task panicked".to_string())?
}

fn ensure_admin() {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000; // no console/PowerShell flash

        let is_admin = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-WindowStyle",
                "Hidden",
                "-Command",
                "([Security.Principal.WindowsPrincipal]\
                 [Security.Principal.WindowsIdentity]::GetCurrent())\
                 .IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .eq_ignore_ascii_case("true")
            })
            .unwrap_or(false);

        if !is_admin {
            let Some(exe) = std::env::current_exe().ok() else {
                return;
            };
            // Escape single quotes in path, hide the spawning shell entirely
            let path = exe.to_string_lossy().replace('\'', "''");
            let Ok(_) = std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-WindowStyle",
                    "Hidden",
                    "-Command",
                    &format!("Start-Process -FilePath '{}' -Verb RunAs", path),
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
            else {
                return;
            };

            // Watch for the elevated sibling in the background. The moment it
            // shows up (user approved UAC) this unelevated instance exits and
            // the elevated one takes over. If the user cancels UAC no sibling
            // ever appears — the app keeps running unelevated instead of
            // silently vanishing, and the UI shows the admin hint.
            // Note: the check must exclude THIS process (its own PID), not
            // the PowerShell child's PID. Polling is bounded (5 min) so the
            // watchdog never spins forever if the user neither approves nor
            // cancels — the unelevated instance simply keeps running.
            let needle = exe.to_string_lossy().to_lowercase().replace('\'', "''");
            let own_pid = std::process::id();
            let exe_name = exe
                .file_name()
                .map(|n| n.to_string_lossy().replace('\'', "''"))
                .unwrap_or_default();
            std::thread::spawn(move || {
                for _ in 0..150 {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let found = std::process::Command::new("powershell")
                        .args([
                            "-NoProfile",
                            "-NonInteractive",
                            "-WindowStyle",
                            "Hidden",
                            "-Command",
                            &format!(
                                "$self = {own_pid}; \
                                 Get-CimInstance Win32_Process -Filter \"Name='{exe_name}'\" | \
                                 Where-Object {{ $_.ExecutablePath -and \
                                 $_.ExecutablePath.ToLower() -eq '{needle}' -and \
                                 $_.ProcessId -ne $self }} | Select-Object -First 1",
                            ),
                        ])
                        .creation_flags(CREATE_NO_WINDOW)
                        .output()
                        .map(|o| !o.stdout.is_empty())
                        .unwrap_or(false);
                    if found {
                        std::process::exit(0);
                    }
                }
            });
        }
    }
}

pub fn run() {
    ensure_admin();
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState {
            monitor: monitor::MonitorState::default(),
            game_switcher: gameprofile::new_state(),
            boosted_pid: std::sync::Mutex::new(None),
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
            cmd_revert_entry,
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
            cmd_security_disable_driver,
            cmd_security_enable_driver,
            cmd_security_remove_driver,
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
            #[cfg(windows)]
            cmd_ctxmenu_list,
            #[cfg(windows)]
            cmd_ctxmenu_toggle,
            #[cfg(windows)]
            cmd_ctxmenu_disable_all,
            #[cfg(windows)]
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
            cmd_gameboost_quick_boost,
            cmd_gameboost_quick_boost_revert,
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
            cmd_nv_get_settings,
            cmd_nv_set_setting,
            cmd_nv_open_panel,
            // registry clean
            cmd_regclean_scan,
            cmd_regclean_clean,
            cmd_regclean_list_backups,
            cmd_regclean_restore,
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
            // software installer
            cmd_sw_catalog,
            cmd_sw_check_installed,
            cmd_sw_install,
            // misc
            cmd_open_path,
            cmd_clear_cache,
        ])
        .setup(move |app| {
            let gs = app.state::<AppState>().game_switcher.clone();
            gameprofile::start(gs, app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running tauri app");
}
