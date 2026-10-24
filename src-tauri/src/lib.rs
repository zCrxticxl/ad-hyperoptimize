mod analysis;
mod bench;
mod bootopt;
mod cache;
// Privilege inventory of all commands (broker phase 1); test-only until the
// broker uses it at runtime.
mod cleanup;
#[cfg(test)]
mod commands;
#[cfg(windows)]
mod ctxmenu;
mod debloater;
mod diskanalyzer;
mod drivers;
mod frametime;
mod gameboost;
mod gamedb;
mod gameprofile;
mod gputweaks;
mod healthcheck;
mod hwmonitor;
mod hwprofile;
mod latency;
mod monitor;
mod optimizer;
mod optregistry;
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
mod statestore;
mod tweaks;
mod uninstaller;
mod updates;

use serde_json::Value;
use tauri::{AppHandle, Manager, State};

struct AppState {
    monitor: std::sync::Arc<monitor::MonitorState>,
    game_switcher: gameprofile::SharedState,
    boosted_pid: std::sync::Mutex<Option<u32>>,
}

// ---- diagnostics ----
#[tauri::command(async)]
async fn cmd_is_admin() -> bool {
    // PowerShell spawn is slow (~0.5-2s); must not run on the main thread
    // or every admin check freezes the UI.
    tauri::async_runtime::spawn_blocking(ps::is_admin)
        .await
        .unwrap_or(false)
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
#[tauri::command(async)]
async fn cmd_start_monitor(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let monitor = state.monitor.clone();
    tauri::async_runtime::spawn_blocking(move || monitor::start(app, &monitor))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command(async)]
async fn cmd_stop_monitor(app: AppHandle) {
    // stop() joins the monitor thread, never block the main thread on it.
    let monitor = app.state::<AppState>().monitor.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || monitor::stop(&monitor)).await;
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
// ---- system health check / verified optimization ----
#[tauri::command(async)]
async fn cmd_health_scan() -> Value {
    tauri::async_runtime::spawn_blocking(optimizer::scan)
        .await
        .unwrap_or_default()
}

/// Opens one of a fixed set of Windows Settings pages. The page name is
/// matched against an allowlist, never passed through.
#[tauri::command]
fn cmd_open_windows_settings(page: String) -> Result<(), String> {
    let uri = match page.as_str() {
        "display-advanced" => "ms-settings:display-advanced",
        "display-advancedgraphics" => "ms-settings:display-advancedgraphics",
        _ => return Err(format!("unknown settings page '{page}'")),
    };
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    std::process::Command::new(format!(r"{root}\explorer.exe"))
        .arg(uri)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not open Windows Settings: {e}"))
}

#[tauri::command(async)]
async fn cmd_reset_retired() -> Value {
    tauri::async_runtime::spawn_blocking(optimizer::reset_retired)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_optimize_apply(ids: Vec<String>, restore_point: bool) -> Value {
    tauri::async_runtime::spawn_blocking(move || optimizer::apply(ids, restore_point))
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
// Frame-time captures (PresentMon / CapFrameX / FrameView CSV) are only
// parsed; nothing is executed.
#[tauri::command(async)]
async fn cmd_fps_import(
    csv: String,
    file_name: String,
    game: String,
    state: String,
    note: String,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        frametime::import(csv, file_name, game, state, note)
    })
    .await
    .map_err(|_| "fps import task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_fps_runs() -> Value {
    tauri::async_runtime::spawn_blocking(frametime::list)
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_fps_delete(id: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || frametime::delete(id))
        .await
        .map_err(|_| "fps task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_fps_compare(game: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || frametime::compare(game))
        .await
        .map_err(|_| "fps task panicked".to_string())?
}

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
async fn cmd_profile_list() -> Value {
    tauri::async_runtime::spawn_blocking(profiles::list)
        .await
        .unwrap_or_else(|_| serde_json::json!({ "error": "profile list task panicked" }))
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
async fn cmd_startup_list() -> Value {
    tauri::async_runtime::spawn_blocking(startup::list)
        .await
        .unwrap_or_else(|_| serde_json::json!({ "error": "startup list task panicked" }))
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
async fn cmd_proc_affinity(pid: u32, mask: String) -> Result<Value, String> {
    // Decimal string: 64-bit masks exceed JS number precision (>2^53).
    let mask: u64 = mask
        .parse()
        .map_err(|_| "invalid affinity mask".to_string())?;
    tauri::async_runtime::spawn_blocking(move || procmgr::set_affinity(pid, mask))
        .await
        .map_err(|_| "process task panicked".to_string())?
}

#[tauri::command(async)]
async fn cmd_perm_priority_list() -> Value {
    tauri::async_runtime::spawn_blocking(procmgr::perm_list)
        .await
        .unwrap_or_else(|_| serde_json::json!({ "error": "perm priority list task panicked" }))
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
async fn cmd_uninstall_app(
    uninstall_string: String,
    from_hkcu: Option<bool>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        uninstaller::uninstall_app(uninstall_string, from_hkcu.unwrap_or(false))
    })
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
    // entries lack these fields and crash the localizer), bump the cache key
    // so stale pre-v2 files on disk are ignored instead of served. v3: the
    // heavy-startup finding no longer links the startup-delay tweak.
    tauri::async_runtime::spawn_blocking(move || {
        cache::get_or("analysis_v3", force, || {
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
        let analysis = cache::data_or("analysis_v3", false, || {
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

#[tauri::command(async)]
async fn cmd_game_switcher_status(app: AppHandle) -> serde_json::Value {
    let st = app.state::<AppState>().game_switcher.clone();
    tauri::async_runtime::spawn_blocking(move || gameprofile::cmd_game_switcher_status(&st))
        .await
        .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_game_switcher_configure(
    app: AppHandle,
    enabled: bool,
    default_preset: String,
) -> serde_json::Value {
    let st = app.state::<AppState>().game_switcher.clone();
    tauri::async_runtime::spawn_blocking(move || {
        gameprofile::cmd_game_switcher_configure(&st, enabled, default_preset)
    })
    .await
    .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_game_apply_preset(game_id: String, preset: String) -> serde_json::Value {
    tauri::async_runtime::spawn_blocking(move || {
        gameprofile::cmd_game_apply_preset(game_id, preset)
    })
    .await
    .unwrap_or_default()
}

#[tauri::command(async)]
async fn cmd_game_revert(app: AppHandle) -> serde_json::Value {
    // Reverting runs powercfg; keep it off the main thread.
    let st = app.state::<AppState>().game_switcher.clone();
    tauri::async_runtime::spawn_blocking(move || gameprofile::cmd_game_revert(&st))
        .await
        .unwrap_or_default()
}

// ---- misc ----
#[tauri::command(async)]
async fn cmd_clear_cache() -> Result<String, String> {
    let n = tauri::async_runtime::spawn_blocking(cache::clear)
        .await
        .map_err(|_| "cache clear task panicked".to_string())??;
    Ok(format!("Cleared {n} cached scans"))
}

#[tauri::command(async)]
fn cmd_open_path(path: String) -> Result<(), String> {
    // This process is elevated and `path` is renderer-supplied (social and
    // buy links, report/backup files, registry-derived install locations).
    // Nothing is ever executed: no PowerShell, no Start-Process, no
    // ShellExecute on an arbitrary file. Everything goes to explorer.exe as
    // one Command argument; explorer hands the request to the running
    // (unelevated) shell instance, so browsers/viewers do not inherit the
    // admin token. See classify_open_target for what is allowed.
    let reports = crate::safety::app_data_dir().join("reports");
    let arg = match classify_open_target(&path, &reports)? {
        OpenTarget::Url(u) => u,
        OpenTarget::Dir(p) | OpenTarget::ReportHtml(p) => p,
        OpenTarget::RevealFile(p) => format!("/select,{p}"),
    };
    let windir = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:\\Windows"));
    // explorer.exe's exit code is meaningless (often 1 on success), so the
    // child is detached instead of waited on.
    std::process::Command::new(windir.join("explorer.exe"))
        .arg(arg)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("explorer: {e}"))
}

/// What cmd_open_path may do with a renderer-supplied value.
#[derive(Debug, PartialEq)]
enum OpenTarget {
    /// An `https://` URL, opened in the default browser.
    Url(String),
    /// An existing local directory, opened in Explorer.
    Dir(String),
    /// A `.html` report inside the app's own reports directory, opened with
    /// its default handler (the browser). The only file ever "opened".
    ReportHtml(String),
    /// Any other existing local file: only revealed (`/select,`), never
    /// opened or executed.
    RevealFile(String),
}

/// Strip the `\\?\` prefix std::fs::canonicalize adds; a canonical path that
/// resolves to a UNC location (`\\?\UNC\…`, e.g. via a symlink) is refused.
fn strip_verbatim(p: &std::path::Path) -> Option<String> {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if rest.len() >= 2 && rest.as_bytes()[1] == b':' => Some(rest.to_string()),
        Some(_) => None,
        None => Some(s.into_owned()),
    }
}

/// Classify `path` for cmd_open_path:
/// * `https://` URLs only (no http, no other scheme), without whitespace,
///   quotes or commas (explorer splits its arguments on commas);
/// * otherwise an absolute `X:\…` local path that exists: UNC, `\\?\`,
///   device and NTFS stream (`:` after the drive) forms are rejected;
/// * existing directory → open; `.html` file inside `reports_dir` → open
///   with the default handler; any other file → reveal only.
fn classify_open_target(path: &str, reports_dir: &std::path::Path) -> Result<OpenTarget, String> {
    if path.is_empty() || path.len() > 4096 {
        return Err("invalid path".into());
    }
    if path.chars().any(char::is_control) {
        return Err("path contains control characters".into());
    }
    // Explorer parses `,` as an argument separator and `"` as quoting.
    if path.contains([',', '"']) {
        return Err("path contains unsupported characters".into());
    }
    let lower = path.to_ascii_lowercase();
    if lower.starts_with("https://") {
        if path.len() <= "https://".len() || path.chars().any(char::is_whitespace) {
            return Err("invalid URL".into());
        }
        return Ok(OpenTarget::Url(path.to_string()));
    }
    if lower.contains("://") || lower.starts_with("http:") {
        return Err("only https:// URLs can be opened".into());
    }

    let b = path.as_bytes();
    let is_abs_drive = b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b[2] == b'\\' || b[2] == b'/');
    if !is_abs_drive {
        return Err("path must be an https:// URL or an absolute local path".into());
    }
    // NTFS stream syntax / device forms: no `:` after the drive letter.
    if path[2..].contains(':') {
        return Err("invalid path".into());
    }
    let p = std::path::Path::new(path);
    let meta = std::fs::metadata(p).map_err(|_| "path does not exist".to_string())?;
    let canon = std::fs::canonicalize(p).map_err(|e| format!("cannot resolve path: {e}"))?;
    let canon_str = strip_verbatim(&canon).ok_or("network paths cannot be opened")?;

    if meta.is_dir() {
        return Ok(OpenTarget::Dir(canon_str));
    }
    if !meta.is_file() {
        return Err("not a file or directory".into());
    }
    let is_html = canon
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("html"));
    let in_reports = std::fs::canonicalize(reports_dir)
        .map(|r| canon.parent() == Some(r.as_path()))
        .unwrap_or(false);
    if is_html && in_reports {
        Ok(OpenTarget::ReportHtml(canon_str))
    } else {
        Ok(OpenTarget::RevealFile(canon_str))
    }
}

/// Runs every read-only scan the UI calls on page load against the real
/// system and fails on PowerShell parser errors (a broken generated script
/// only shows up at runtime). Read-only, but slow and system-dependent, so it
/// is opt-in: `cargo test --locked -- --ignored read_only_scans`.
#[cfg(all(test, windows))]
mod smoke {
    #[test]
    #[ignore]
    fn read_only_scans_have_no_powershell_parser_errors() {
        type Scan = fn() -> serde_json::Value;
        let scans: Vec<(&str, Scan)> = vec![
            ("bootopt", crate::bootopt::scan),
            ("cleanup", crate::cleanup::scan),
            ("debloater uwp", crate::debloater::list_uwp),
            ("debloater tweaks", crate::debloater::list_tweaks),
            ("gputweaks", crate::gputweaks::scan),
            ("health check", crate::optimizer::scan),
            ("hwprofile", crate::hwprofile::hw_profile),
            ("msi", crate::perftweaks::msi_list),
            ("powerplan", crate::powerplan::list_plans),
            ("privacy", crate::privacy::scan),
            ("processes", crate::procmgr::list),
            ("profiles", crate::profiles::list),
            ("regclean", crate::regclean::scan),
            ("schedtasks", crate::schedtasks::list),
            ("security", crate::security::scan),
            ("services", crate::services::list),
            ("startup", crate::startup::list),
            ("tweaks", crate::tweaks::list_with_status),
            ("uninstaller", crate::uninstaller::list_apps),
        ];
        let markers = [
            "ParserError",
            "ParseException",
            "MissingEndParenthesis",
            "UnexpectedToken",
            "MissingEndCurlyBrace",
            "MissingCatchOrFinally",
        ];
        let mut broken = Vec::new();
        for (name, scan) in scans {
            let out = scan().to_string();
            if let Some(m) = markers.iter().find(|m| out.contains(*m)) {
                broken.push(format!("{name}: {m}"));
            }
        }
        assert!(broken.is_empty(), "scans with parser errors: {broken:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_open_target, OpenTarget};

    #[cfg(debug_assertions)]
    #[test]
    fn debug_launch_skips_admin_relaunch() {
        assert!(!super::should_auto_elevate());
    }

    fn no_reports() -> std::path::PathBuf {
        std::env::temp_dir().join("adho-open-path-no-such-reports-dir")
    }

    #[test]
    fn open_path_accepts_https_urls() {
        for u in [
            "https://discord.gg/vFaKsVuxKP",
            "https://www.buymeacoffee.com/zCrxticxl",
            // percent-encoded query links (PcConfigurator buy buttons)
            "https://www.bestbuy.com/site/searchpage.jsp?st=NVIDIA%20GeForce%20RTX%205090&_dyncharset=UTF-8",
        ] {
            assert_eq!(
                classify_open_target(u, &no_reports()),
                Ok(OpenTarget::Url(u.to_string()))
            );
        }
    }

    #[test]
    fn open_path_rejects_unsafe_targets() {
        for bad in [
            "",
            "calc.exe",
            "not a url",
            "http://example.com/x",
            "file:///C:/Windows/System32/calc.exe",
            "ms-settings:privacy",
            "https://x.com/a\ncalc",
            "https://x.com/a b",
            "https://x.com/a\"b",
            "https://",
            r"\\server\share\folder",
            r"\\?\C:\Windows",
            r"\\.\C:\Windows",
            r"C:\Windows::$INDEX_ALLOCATION",
            r"C:\x\file.txt:stream",
            r"C:\a,b",
            r"C:\definitely\not\existing\adho-xyz",
        ] {
            assert!(
                classify_open_target(bad, &no_reports()).is_err(),
                "should reject {bad:?}"
            );
        }
        assert!(classify_open_target(&"x".repeat(4097), &no_reports()).is_err());
    }

    #[test]
    fn open_path_dirs_open_files_are_only_revealed() {
        let base = std::env::temp_dir().join("adho-open-path-test");
        let reports = base.join("reports");
        std::fs::create_dir_all(&reports).unwrap();
        let exe = base.join("evil.exe");
        let html_out = base.join("x.html");
        let html_in = reports.join("report-1.html");
        for f in [&exe, &html_out, &html_in] {
            std::fs::write(f, b"x").unwrap();
        }
        let s = |p: &std::path::Path| p.to_string_lossy().into_owned();

        if cfg!(windows) {
            assert!(matches!(
                classify_open_target(&s(&base), &reports),
                Ok(OpenTarget::Dir(_))
            ));
            // An executable is never opened, only revealed.
            assert!(matches!(
                classify_open_target(&s(&exe), &reports),
                Ok(OpenTarget::RevealFile(_))
            ));
            // HTML outside the reports dir: reveal only.
            assert!(matches!(
                classify_open_target(&s(&html_out), &reports),
                Ok(OpenTarget::RevealFile(_))
            ));
            assert!(matches!(
                classify_open_target(&s(&html_in), &reports),
                Ok(OpenTarget::ReportHtml(_))
            ));
        }
        std::fs::remove_dir_all(&base).unwrap();
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

const fn should_auto_elevate() -> bool {
    !cfg!(debug_assertions)
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
            // Quote the path as a PS literal (ASCII + typographic quotes), hide the spawning shell entirely
            let path = crate::ps::quote_single(&exe.to_string_lossy());
            // Relaunch elevated and block on the UAC outcome before this
            // unelevated instance shows any window. `Start-Process -Verb RunAs`
            // throws if the user declines UAC; the try/catch maps that to a
            // non-zero exit. On approval we exit immediately so ONLY the
            // elevated window appears (no lingering second, unelevated window);
            // on cancellation we fall through and keep running unelevated with
            // the in-app admin hint.
            let approved = std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-WindowStyle",
                    "Hidden",
                    "-Command",
                    &format!(
                        "try {{ Start-Process -FilePath {} -Verb RunAs -ErrorAction Stop; exit 0 }} \
                         catch {{ exit 1 }}",
                        path
                    ),
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            if approved {
                std::process::exit(0);
            }
        }
    }
}

pub fn run() {
    if should_auto_elevate() {
        ensure_admin();
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState {
            monitor: std::sync::Arc::new(monitor::MonitorState::default()),
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
            cmd_net_reset_all,
            cmd_ram_info,
            cmd_pagefile_info,
            cmd_pagefile_set_auto,
            cmd_pagefile_set_custom,
            cmd_pagefile_disable,
            // hw monitor
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
            cmd_gameboost_kill_background,
            cmd_gameboost_start,
            cmd_gameboost_stop,
            cmd_gameboost_get_status,
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
            cmd_fps_import,
            cmd_fps_runs,
            cmd_fps_delete,
            cmd_fps_compare,
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
            cmd_health_scan,
            cmd_optimize_apply,
            cmd_reset_retired,
            cmd_open_windows_settings,
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
