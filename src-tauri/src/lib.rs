mod analysis;
mod bootopt;
mod diskanalyzer;
mod healthcheck;
mod hwmonitor;
mod perftweaks;
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

use serde_json::Value;
use tauri::{AppHandle, Manager, State};

struct AppState {
    monitor: monitor::MonitorState,
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

// ---- safety ----
#[tauri::command(async)]
fn cmd_create_restore_point(description: String) -> Result<String, String> {
    safety::create_restore_point(&description)
}

#[tauri::command(async)]
fn cmd_list_restore_points() -> Value { safety::list_restore_points() }

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

// ---- gpu tweaks ----
#[tauri::command(async)]
fn cmd_gpu_scan() -> Value { gputweaks::scan() }

#[tauri::command(async)]
fn cmd_gpu_tweak_apply(id: String, driver_key: String) -> Result<Value, String> {
    gputweaks::apply_tweak(id, driver_key)
}

#[tauri::command(async)]
fn cmd_gpu_tweak_revert(id: String, driver_key: String) -> Result<Value, String> {
    gputweaks::revert_tweak(id, driver_key)
}

// ---- registry orphan cleaner ----
#[tauri::command(async)]
fn cmd_regclean_scan() -> Value { regclean::scan() }

#[tauri::command(async)]
fn cmd_regclean_clean(entries: Vec<Value>) -> Result<Value, String> {
    regclean::clean(entries)
}

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
fn cmd_disk_largest(path: String, limit: usize) -> Value {
    diskanalyzer::scan_largest(path, limit)
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

// ---- performance tweaks ----
#[tauri::command(async)]
fn cmd_timer_get() -> Value { perftweaks::timer_get() }

#[tauri::command(async)]
fn cmd_timer_set(target_100ns: u32) -> Result<String, String> { perftweaks::timer_set(target_100ns) }

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

// ---- hardware monitor ----
#[tauri::command(async)]
fn cmd_hw_temps() -> Value { hwmonitor::temps() }

#[tauri::command(async)]
fn cmd_hw_smart() -> Value { hwmonitor::smart() }

#[tauri::command(async)]
fn cmd_hw_full() -> Value { hwmonitor::full() }

// ---- analysis & reports ----
#[tauri::command(async)]
fn cmd_analyze(force: Option<bool>) -> Value {
    let force = force.unwrap_or(false);
    if !force {
        if let Some(mut hit) = cache::load("analysis") {
            hit["fromCache"] = serde_json::json!(true);
            return hit;
        }
    }
    let s = cache::data_or("scan", force, scan::full_scan);
    let sec = cache::data_or("security", force, security::scan);
    let cl = cache::data_or("cleanup", force, cleanup::scan);
    cache::store("analysis", analysis::analyze(&s, &sec, &cl))
}

#[tauri::command(async)]
fn cmd_generate_report() -> Result<Value, String> {
    let s = cache::data_or("scan", false, scan::full_scan);
    let sec = cache::data_or("security", false, security::scan);
    let cl = cache::data_or("cleanup", false, cleanup::scan);
    let a = analysis::analyze(&s, &sec, &cl);
    let h = tweaks::history();
    report::generate(&s, &a, &sec, &h)
}

#[tauri::command]
fn cmd_open_path(path: String) -> Result<(), String> {
    // Open report/backup in the default app (explorer handles both).
    ps::exec("cmd.exe", &["/C", "start", "", &path]).map(|_| ())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(AppState { monitor: monitor::MonitorState::default() });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cmd_is_admin,
            cmd_full_scan,
            cmd_boot_analysis,
            cmd_event_logs,
            cmd_component_health,
            cmd_dns_benchmark,
            cmd_network_diag,
            cmd_start_monitor,
            cmd_stop_monitor,
            cmd_list_tweaks,
            cmd_apply_tweak,
            cmd_revert_tweak,
            cmd_history,
            cmd_create_restore_point,
            cmd_list_restore_points,
            cmd_scan_cleanup,
            cmd_run_cleanup,
            cmd_security_scan,
            cmd_defender_quick_scan,
            cmd_run_benchmark,
            cmd_bench_history,
            cmd_profile_list,
            cmd_profile_apply,
            cmd_profile_revert,
            cmd_startup_list,
            cmd_startup_toggle,
            cmd_proc_list,
            cmd_proc_kill,
            cmd_proc_priority,
            cmd_proc_affinity,
            cmd_proc_detail,
            cmd_perm_priority_list,
            cmd_perm_priority_set,
            cmd_perm_priority_remove,
            cmd_scan_app_updates,
            cmd_update_apps,
            cmd_scan_driver_updates,
            cmd_install_driver_updates,
            cmd_gpu_vendor,
            cmd_latency_counters,
            cmd_stall_probe,
            cmd_wpr_start,
            cmd_wpr_stop,
            cmd_wpr_cancel,
            cmd_gpu_scan,
            cmd_gpu_tweak_apply,
            cmd_gpu_tweak_revert,
            cmd_regclean_scan,
            cmd_regclean_clean,
            cmd_privacy_scan,
            cmd_privacy_apply,
            cmd_privacy_revert,
            cmd_services_list,
            cmd_service_set_startup,
            cmd_service_control,
            cmd_health_run,
            cmd_boot_scan,
            cmd_boot_tweak_apply,
            cmd_boot_tweak_revert,
            cmd_disk_drives,
            cmd_disk_largest,
            cmd_disk_duplicates,
            cmd_disk_temp_age,
            cmd_disk_delete,
            cmd_disk_move,
            cmd_schedtasks_list,
            cmd_schedtask_toggle,
            cmd_analyze,
            cmd_generate_report,
            cmd_open_path,
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
            cmd_hw_temps,
            cmd_hw_smart,
            cmd_hw_full,
            cmd_hosts_list_all,
            cmd_hosts_disable_entries,
            cmd_hosts_enable_entries,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
