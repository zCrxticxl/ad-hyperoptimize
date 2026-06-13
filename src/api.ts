import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

export const api = {
  isAdmin: () => invoke<boolean>("cmd_is_admin"),
  fullScan: (force = false) => invoke<any>("cmd_full_scan", { force }),
  bootAnalysis: () => invoke<any>("cmd_boot_analysis"),
  eventLogs: () => invoke<any>("cmd_event_logs"),
  componentHealth: () => invoke<any>("cmd_component_health"),
  dnsBenchmark: () => invoke<any>("cmd_dns_benchmark"),
  networkDiag: () => invoke<any>("cmd_network_diag"),
  startMonitor: () => invoke("cmd_start_monitor"),
  stopMonitor: () => invoke("cmd_stop_monitor"),
  listTweaks: () => invoke<any[]>("cmd_list_tweaks"),
  applyTweak: (id: string) => invoke<any>("cmd_apply_tweak", { id }),
  revertTweak: (id: string) => invoke<any>("cmd_revert_tweak", { id }),
  history: () => invoke<any[]>("cmd_history"),
  createRestorePoint: (description: string) =>
    invoke<string>("cmd_create_restore_point", { description }),
  listRestorePoints: () => invoke<any>("cmd_list_restore_points"),
  scanCleanup: (force = false) => invoke<any>("cmd_scan_cleanup", { force }),
  runCleanup: (ids: string[]) => invoke<any>("cmd_run_cleanup", { ids }),
  securityScan: (force = false) => invoke<any>("cmd_security_scan", { force }),
  defenderQuickScan: () => invoke<string>("cmd_defender_quick_scan"),
  // app uninstaller
  uninstallerList: () => invoke<any>("cmd_uninstaller_list"),
  uninstallApp: (uninstallString: string) => invoke<string>("cmd_uninstall_app", { uninstallString }),
  scanLeftovers: (appName: string, publisher: string, installLocation: string) =>
    invoke<any>("cmd_scan_leftovers", { appName, publisher, installLocation }),
  cleanLeftovers: (paths: string[]) => invoke<string>("cmd_clean_leftovers", { paths }),
  // context menu
  ctxmenuList: () => invoke<any>("cmd_ctxmenu_list"),
  ctxmenuToggle: (path: string, enable: boolean) => invoke<string>("cmd_ctxmenu_toggle", { path, enable }),
  ctxmenuDisableAll: () => invoke<string>("cmd_ctxmenu_disable_all"),
  ctxmenuEnableAll: () => invoke<string>("cmd_ctxmenu_enable_all"),
  // power plan
  powerplanList: () => invoke<any>("cmd_powerplan_list"),
  powerplanSet: (guid: string) => invoke<string>("cmd_powerplan_set", { guid }),
  powerplanUnlockUltimate: () => invoke<string>("cmd_powerplan_unlock_ultimate"),
  powerplanDelete: (guid: string) => invoke<string>("cmd_powerplan_delete", { guid }),
  powerplanCreate: (name: string, baseGuid: string) => invoke<string>("cmd_powerplan_create", { name, baseGuid }),
  // perf tweaks
  timerGet: () => invoke<any>("cmd_timer_get"),
  timerSet: (target100ns: number) => invoke<string>("cmd_timer_set", { target100ns }),
  timerReset: () => invoke<string>("cmd_timer_reset"),
  msiList: () => invoke<any>("cmd_msi_list"),
  msiSet: (regPath: string, enabled: boolean) => invoke<string>("cmd_msi_set", { regPath, enabled }),
  netAdapters: () => invoke<any>("cmd_net_adapters"),
  netTweak: (adapter: string, keyword: string, value: number) => invoke<string>("cmd_net_tweak", { adapter, keyword, value }),
  netTweakAllGaming: () => invoke<string>("cmd_net_tweak_all_gaming"),
  netResetAll: () => invoke<string>("cmd_net_reset_all"),
  ramInfo: () => invoke<any>("cmd_ram_info"),
  ramFlushStandby: () => invoke<string>("cmd_ram_flush_standby"),
  pagefileInfo: () => invoke<any>("cmd_pagefile_info"),
  pagefileSetAuto: () => invoke<string>("cmd_pagefile_set_auto"),
  pagefileSetCustom: (path: string, initMb: number, maxMb: number) => invoke<string>("cmd_pagefile_set_custom", { path, initMb, maxMb }),
  pagefileDisable: () => invoke<string>("cmd_pagefile_disable"),
  // hw monitor
  hwTemps: () => invoke<any>("cmd_hw_temps"),
  hwSmart: () => invoke<any>("cmd_hw_smart"),
  hwFull: () => invoke<any>("cmd_hw_full"),
  // debloater
  debloaterUwpList: () => invoke<any>("cmd_debloater_uwp_list"),
  debloaterRemoveUwp: (packageFullName: string) => invoke<string>("cmd_debloater_remove_uwp", { packageFullName }),
  debloaterRemoveProvisioned: (packageName: string) => invoke<string>("cmd_debloater_remove_provisioned", { packageName }),
  debloaterTweaksList: () => invoke<any>("cmd_debloater_tweaks_list"),
  debloaterTweakApply: (id: string) => invoke<string>("cmd_debloater_tweak_apply", { id }),
  debloaterTweakRevert: (id: string) => invoke<string>("cmd_debloater_tweak_revert", { id }),
  // drivers
  driversList: () => invoke<any>("cmd_drivers_list"),
  driversOpenDevmgr: () => invoke<string>("cmd_drivers_open_devmgr"),
  driversOpenWindowsUpdate: () => invoke<string>("cmd_drivers_open_windows_update"),
  driversCheckWinget: (packageId: string) => invoke<any>("cmd_drivers_check_winget", { packageId }),
  driversInstallWinget: (packageId: string) => invoke<string>("cmd_drivers_install_winget", { packageId }),
  driversOpenVendorUrl: (url: string) => invoke<string>("cmd_drivers_open_vendor_url", { url }),
  driversScanWindowsUpdate: () => invoke<string>("cmd_drivers_scan_windows_update"),
  // game booster
  gameboostBackgroundProcs: () => invoke<any>("cmd_gameboost_background_procs"),
  gameboostRunningGames: () => invoke<any>("cmd_gameboost_running_games"),
  gameboostBoostProcess: (pid: number) => invoke<string>("cmd_gameboost_boost_process", { pid }),
  gameboostKillBackground: (pids: number[]) => invoke<string>("cmd_gameboost_kill_background", { pids }),
  gameboostStart: (pid: number) => invoke<string>("cmd_gameboost_start", { pid }),
  gameboostStop: () => invoke<string>("cmd_gameboost_stop"),
  gameboostGpuPerf: (enable: boolean) => invoke<string>("cmd_gameboost_gpu_perf", { enable }),
  // hosts
  hostsListAll: () => invoke<any>("cmd_hosts_list_all"),
  hostsDisableEntries: (entries: string[]) => invoke<string>("cmd_hosts_disable_entries", { entries }),
  hostsEnableEntries:  (entries: string[]) => invoke<string>("cmd_hosts_enable_entries",  { entries }),
  runBenchmark: (kind: string) => invoke<any>("cmd_run_benchmark", { kind }),
  benchHistory: () => invoke<any[]>("cmd_bench_history"),
  profileList: () => invoke<any[]>("cmd_profile_list"),
  profileApply: (id: string, withBench: boolean) =>
    invoke<any>("cmd_profile_apply", { id, withBench }),
  profileRevert: (id: string) => invoke<any>("cmd_profile_revert", { id }),
  startupList: () => invoke<any>("cmd_startup_list"),
  startupToggle: (scope: string, name: string, enable: boolean) =>
    invoke<any>("cmd_startup_toggle", { scope, name, enable }),
  procList: () => invoke<any>("cmd_proc_list"),
  procKill: (pid: number) => invoke<any>("cmd_proc_kill", { pid }),
  procPriority: (pid: number, priority: string) => invoke<any>("cmd_proc_priority", { pid, priority }),
  procAffinity: (pid: number, mask: number) => invoke<any>("cmd_proc_affinity", { pid, mask }),
  permPriorityList: () => invoke<any>("cmd_perm_priority_list"),
  permPrioritySet: (exe: string, priority: string) =>
    invoke<any>("cmd_perm_priority_set", { exe, priority }),
  permPriorityRemove: (exe: string) => invoke<any>("cmd_perm_priority_remove", { exe }),
  scanAppUpdates: () => invoke<any>("cmd_scan_app_updates"),
  updateApps: (id?: string) => invoke<any>("cmd_update_apps", { id: id ?? null }),
  scanDriverUpdates: () => invoke<any>("cmd_scan_driver_updates"),
  installDriverUpdates: () => invoke<any>("cmd_install_driver_updates"),
  gpuVendor: () => invoke<any>("cmd_gpu_vendor"),
  latencyCounters: (samples: number) => invoke<any>("cmd_latency_counters", { samples }),
  stallProbe: (seconds: number) => invoke<any>("cmd_stall_probe", { seconds }),
  wprStart: () => invoke<string>("cmd_wpr_start"),
  wprStop: () => invoke<any>("cmd_wpr_stop"),
  wprCancel: () => invoke<string>("cmd_wpr_cancel"),
  gpuScan: () => invoke<any>("cmd_gpu_scan"),
  gpuTweakApply: (id: string, driverKey: string) => invoke<any>("cmd_gpu_tweak_apply", { id, driverKey }),
  gpuTweakRevert: (id: string, driverKey: string) => invoke<any>("cmd_gpu_tweak_revert", { id, driverKey }),
  regcleanScan: () => invoke<any>("cmd_regclean_scan"),
  regcleanClean: (entries: any[]) => invoke<any>("cmd_regclean_clean", { entries }),
  privacyScan: () => invoke<any>("cmd_privacy_scan"),
  privacyApply: (id: string) => invoke<any>("cmd_privacy_apply", { id }),
  privacyRevert: (id: string) => invoke<any>("cmd_privacy_revert", { id }),
  servicesList: () => invoke<any>("cmd_services_list"),
  serviceSetStartup: (name: string, startupType: string) => invoke<any>("cmd_service_set_startup", { name, startupType }),
  serviceControl: (name: string, action: string) => invoke<any>("cmd_service_control", { name, action }),
  healthRun: (kind: string) => invoke<any>("cmd_health_run", { kind }),
  bootScan: () => invoke<any>("cmd_boot_scan"),
  bootTweakApply: (id: string) => invoke<any>("cmd_boot_tweak_apply", { id }),
  bootTweakRevert: (id: string) => invoke<any>("cmd_boot_tweak_revert", { id }),
  diskDrives: () => invoke<any>("cmd_disk_drives"),
  diskLargest: (path: string, limit = 50) => invoke<any>("cmd_disk_largest", { path, limit }),
  diskDuplicates: (path: string) => invoke<any>("cmd_disk_duplicates", { path }),
  diskTempAge: () => invoke<any>("cmd_disk_temp_age"),
  diskDelete: (paths: string[]) => invoke<any>("cmd_disk_delete", { paths }),
  diskMove: (paths: string[], destDir: string) => invoke<any>("cmd_disk_move", { paths, destDir }),
  schedTasksList: () => invoke<any>("cmd_schedtasks_list"),
  schedTaskToggle: (path: string, name: string, enable: boolean) =>
    invoke<any>("cmd_schedtask_toggle", { path, name, enable }),
  analyze: (force = false) => invoke<any>("cmd_analyze", { force }),
  generateReport: () => invoke<any>("cmd_generate_report"),
  openPath: (path: string) => invoke("cmd_open_path", { path }),
  // restore points
  createRestorePoint: (description: string) => invoke<string>("cmd_create_restore_point", { description }),
  listRestorePoints: () => invoke<any>("cmd_list_restore_points"),
  deleteRestorePoint: (sequenceNumber: number) => invoke<string>("cmd_delete_restore_point", { sequenceNumber }),
  launchRstrui: () => invoke<string>("cmd_launch_rstrui"),
  // auto optimizer
  autooptScan: () => invoke<any>("cmd_autoopt_scan"),
  autooptScore: () => invoke<any>("cmd_autoopt_score"),
  autooptApply: (items: any[]) => invoke<any>("cmd_autoopt_apply", { items }),
};

export type Metrics = {
  t: string;
  cpuTotal: number;
  perCore: number[];
  freqMhz: number;
  memUsedMb: number;
  memTotalMb: number;
  netRxKbs: number;
  netTxKbs: number;
  disks: { name: string; totalGb: number; freeGb: number }[];
  topCpu: { name: string; cpu: number; memMb: number }[];
  topMem: { name: string; cpu: number; memMb: number }[];
};

export function onMetrics(cb: (m: Metrics) => void): Promise<UnlistenFn> {
  return listen<Metrics>("metrics", (e) => cb(e.payload));
}

export const fmtAge = (iso: string) => {
  const mins = Math.floor((Date.now() - new Date(iso).getTime()) / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins} min ago`;
  const h = Math.floor(mins / 60);
  if (h < 24) return `${h} h ago`;
  return `${Math.floor(h / 24)} d ago`;
};

export const fmtBytes = (b: number) => {
  if (b >= 1e9) return (b / 1e9).toFixed(2) + " GB";
  if (b >= 1e6) return (b / 1e6).toFixed(1) + " MB";
  if (b >= 1e3) return (b / 1e3).toFixed(0) + " KB";
  return b + " B";
};
