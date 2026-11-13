import { invoke as tauriInvoke, InvokeArgs } from "@tauri-apps/api/core";
import { trackOperation } from "./lib/pendingOperations";

// Passive reads and polling must never hold navigation open.
const actionCommands = new Set(`apply_tweak revert_tweak revert_entry run_cleanup defender_quick_scan uninstall_app scan_leftovers clean_leftovers ctxmenu_toggle ctxmenu_disable_all ctxmenu_enable_all powerplan_set powerplan_unlock_ultimate powerplan_delete powerplan_create timer_set timer_reset msi_set net_tweak net_reset_all pagefile_set_auto pagefile_set_custom pagefile_disable perf_undo debloater_remove_uwp debloater_remove_provisioned debloater_tweak_apply debloater_tweak_revert drivers_install_winget drivers_scan_windows_update gameboost_kill_background gameboost_start gameboost_stop gameboost_stop_all gameboost_quick_boost gameboost_quick_boost_revert disable_scheduled_task enable_scheduled_task defender_set_realtime defender_set_cloud hosts_disable_entries hosts_enable_entries security_disable_driver security_enable_driver security_remove_driver run_benchmark fps_import fps_delete profile_apply profile_revert startup_toggle proc_kill proc_priority proc_affinity perm_priority_set perm_priority_remove update_apps install_driver_updates latency_counters session_start session_stop session_analyze session_delete pregame_pause pregame_resume power_fix leftover_reset irq_set rebar_set stall_probe wpr_start wpr_stop wpr_cancel gpu_tweak_apply gpu_tweak_revert regclean_scan regclean_clean regclean_restore privacy_apply privacy_revert service_set_startup service_control health_run boot_tweak_apply boot_tweak_revert disk_largest disk_duplicates disk_delete disk_move organizer_preview organizer_apply organizer_undo schedtask_toggle generate_report clear_cache game_switcher_configure game_apply_preset game_revert create_restore_point delete_restore_point optimize_apply reset_retired`.split(" ").map(name => `cmd_${name}`));

function invoke<T>(command: string, args?: InvokeArgs): Promise<T> {
  const run = () => tauriInvoke<T>(command, args);
  return actionCommands.has(command) ? trackOperation(run) : run();
}
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
  brokerStatus: () => invoke<{ available: boolean; connected: boolean }>("cmd_broker_status"),
  brokerUnlock: () => invoke<any>("cmd_broker_unlock"),
  restartAsAdmin: () => invoke<void>("cmd_restart_as_admin"),
  applyTweak: (id: string) => invoke<any>("cmd_apply_tweak", { id }),
  revertTweak: (id: string) => invoke<any>("cmd_revert_tweak", { id }),
  history: () => invoke<any[]>("cmd_history"),
  revertEntry: (entryId: string) => invoke<any>("cmd_revert_entry", { entryId }),
  scanCleanup: (force = false) => invoke<any>("cmd_scan_cleanup", { force }),
  runCleanup: (ids: string[]) => invoke<any>("cmd_run_cleanup", { ids }),
  securityScan: (force = false) => invoke<any>("cmd_security_scan", { force }),
  defenderQuickScan: () => invoke<string>("cmd_defender_quick_scan"),
  // app uninstaller
  uninstallerList: () => invoke<any>("cmd_uninstaller_list"),
  uninstallApp: (uninstallString: string, fromHkcu: boolean) => invoke<string>("cmd_uninstall_app", { uninstallString, fromHkcu }),
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
  netResetAll: () => invoke<string>("cmd_net_reset_all"),
  ramInfo: () => invoke<any>("cmd_ram_info"),
  pagefileInfo: () => invoke<any>("cmd_pagefile_info"),
  pagefileSetAuto: () => invoke<string>("cmd_pagefile_set_auto"),
  pagefileSetCustom: (path: string, initMb: number, maxMb: number) => invoke<string>("cmd_pagefile_set_custom", { path, initMb, maxMb }),
  pagefileDisable: () => invoke<string>("cmd_pagefile_disable"),
  perfOpenEntries: () => invoke<{ entries: { id: string; name: string; time: string }[] }>("cmd_perf_open_entries"),
  perfUndo: (entryId: string) => invoke<any>("cmd_perf_undo", { entryId }),
  // hw monitor
  hwFull: () => invoke<any>("cmd_hw_full"),
  hwProfile: () => invoke<any>("cmd_hw_profile"),
  swCatalog: () => invoke<any>("cmd_sw_catalog"),
  swCheckInstalled: () => invoke<any>("cmd_sw_check_installed"),
  swInstall: (wingetIds: string[], onProgress?: (progress: { wingetId: string; status: string; message: string }) => void) => trackOperation(async () => {
    const unlisten = onProgress ? await listen<{ wingetId: string; status: string; message: string }>("sw-install-progress", event => onProgress(event.payload)) : undefined;
    try { await invoke<void>("cmd_sw_install", { wingetIds }); }
    finally { unlisten?.(); }
  }),
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
  gameboostKillBackground: (pids: number[]) => invoke<string>("cmd_gameboost_kill_background", { pids }),
  gameboostStart: (pid: number) => invoke<string>("cmd_gameboost_start", { pid }),
  gameboostStop: () => invoke<string>("cmd_gameboost_stop"),
  gameboostStopAll: () => invoke<string>("cmd_gameboost_stop_all"),
  gameboostGetStatus: () => invoke<{ pid: number | null; active: boolean; quick?: { id: string; tweak_name: string }[]; error?: string }>("cmd_gameboost_get_status"),
  quickBoostStart: (processName: string) => invoke<any>("cmd_gameboost_quick_boost", { processName }),
  quickBoostRevert: (restoreToken: string) => invoke<any>("cmd_gameboost_quick_boost_revert", { restoreToken }),
  // hosts
  disableScheduledTask: (taskPath: string, taskName: string) => invoke<string>("cmd_disable_scheduled_task", { taskPath, taskName }),
  enableScheduledTask:  (taskPath: string, taskName: string) => invoke<string>("cmd_enable_scheduled_task",  { taskPath, taskName }),
  defenderSetRealtime:  (enabled: boolean) => invoke<string>("cmd_defender_set_realtime", { enabled }),
  defenderSetCloud:     (enabled: boolean) => invoke<string>("cmd_defender_set_cloud",    { enabled }),
  hostsListAll: () => invoke<any>("cmd_hosts_list_all"),
  hostsDisableEntries: (entries: string[]) => invoke<string>("cmd_hosts_disable_entries", { entries }),
  hostsEnableEntries:  (entries: string[]) => invoke<string>("cmd_hosts_enable_entries",  { entries }),
  securityDisableDriver: (deviceId: string) => invoke<string>("cmd_security_disable_driver", { deviceId }),
  securityEnableDriver:  (deviceId: string) => invoke<string>("cmd_security_enable_driver",  { deviceId }),
  securityRemoveDriver:  (deviceId: string) => invoke<string>("cmd_security_remove_driver",  { deviceId }),
  runBenchmark: (kind: string) => invoke<any>("cmd_run_benchmark", { kind }),
  benchHistory: () => invoke<any[]>("cmd_bench_history"),
  fpsImport: (csv: string, fileName: string, game: string, state: "before" | "after", note: string) =>
    invoke<any>("cmd_fps_import", { csv, fileName, game, state, note }),
  fpsRuns: () => invoke<any>("cmd_fps_runs"),
  fpsDelete: (id: string) => invoke<any>("cmd_fps_delete", { id }),
  fpsCompare: (game: string) => invoke<any>("cmd_fps_compare", { game }),
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
  procAffinity: (pid: number, mask: string) => invoke<any>("cmd_proc_affinity", { pid, mask }),
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
  throttleSnapshot: () => invoke<any>("cmd_throttle_snapshot"),
  sessionStart: () => invoke<any>("cmd_session_start"),
  sessionStop: () => invoke<any>("cmd_session_stop"),
  sessionStatus: () => invoke<any>("cmd_session_status"),
  sessionList: () => invoke<any>("cmd_session_list"),
  sessionAnalyze: (id: string, csv?: string, captureEndMs?: number) =>
    invoke<any>("cmd_session_analyze", { id, csv: csv ?? null, captureEndMs: captureEndMs ?? null }),
  sessionDelete: (id: string) => invoke<any>("cmd_session_delete", { id }),
  displayCheck: () => invoke<any>("cmd_display_check"),
  audioCheck: () => invoke<any>("cmd_audio_check"),
  gameStorageCheck: () => invoke<any>("cmd_game_storage_check"),
  pregameScan: () => invoke<any>("cmd_pregame_scan"),
  pregamePause: (activities: string[]) => invoke<any>("cmd_pregame_pause", { activities }),
  pregameResume: () => invoke<any>("cmd_pregame_resume"),
  hwCheck: () => invoke<any>("cmd_hwcheck"),
  gpuLinkLive: () => invoke<any>("cmd_gpu_link_live"),
  powerFix: (alias: string) => invoke<any>("cmd_power_fix", { alias }),
  leftoversScan: () => invoke<any>("cmd_leftovers_scan"),
  leftoverReset: (id: string) => invoke<any>("cmd_leftover_reset", { id }),
  irqList: () => invoke<any>("cmd_irq_list"),
  irqSet: (regPath: string, core: number | null) => invoke<any>("cmd_irq_set", { regPath, core }),
  rebarStatus: (extra?: string) => invoke<any>("cmd_rebar_status", { extra: extra ?? null }),
  rebarSet: (exe: string, enable: boolean) => invoke<any>("cmd_rebar_set", { exe, enable }),
  stallProbe: (seconds: number) => invoke<any>("cmd_stall_probe", { seconds }),
  wprStart: () => invoke<string>("cmd_wpr_start"),
  wprStop: () => invoke<any>("cmd_wpr_stop"),
  wprCancel: () => invoke<string>("cmd_wpr_cancel"),
  gpuScan: () => invoke<any>("cmd_gpu_scan"),
  gpuTweakApply: (id: string, driverKey: string) => invoke<any>("cmd_gpu_tweak_apply", { id, driverKey }),
  gpuTweakRevert: (id: string, driverKey: string) => invoke<any>("cmd_gpu_tweak_revert", { id, driverKey }),
  // nvidia control panel
  nvStatus: () => invoke<any>("cmd_nv_status"),
  nvOpenPanel: () => invoke<string>("cmd_nv_open_panel"),
  regcleanScan: () => invoke<any>("cmd_regclean_scan"),
  regcleanClean: (entries: any[]) => invoke<any>("cmd_regclean_clean", { entries }),
  regcleanListBackups: () => invoke<any>("cmd_regclean_list_backups"),
  regcleanRestore: (backupPath: string) => invoke<any>("cmd_regclean_restore", { backupPath }),
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
  organizerFolders: () => invoke<any>("cmd_organizer_folders"),
  organizerPreview: (folder: string, byMonth: boolean) => invoke<any>("cmd_organizer_preview", { folder, byMonth }),
  organizerApply: (folder: string, byMonth: boolean, srcs: string[]) => invoke<any>("cmd_organizer_apply", { folder, byMonth, srcs }),
  organizerRuns: () => invoke<any>("cmd_organizer_runs"),
  organizerUndo: (runId: string) => invoke<any>("cmd_organizer_undo", { runId }),
  schedTasksList: () => invoke<any>("cmd_schedtasks_list"),
  schedTaskToggle: (path: string, name: string, enable: boolean) =>
    invoke<any>("cmd_schedtask_toggle", { path, name, enable }),
  analyze: (force = false) => invoke<any>("cmd_analyze", { force }),
  generateReport: () => invoke<any>("cmd_generate_report"),
  openPath: (path: string) => invoke("cmd_open_path", { path }),
  clearCache: () => invoke<string>("cmd_clear_cache"),
  // game profiles / auto-switcher
  gameList: () => invoke<any[]>("cmd_game_list"),
  gameSwitcherStatus: () => invoke<any>("cmd_game_switcher_status"),
  gameSwitcherConfigure: (enabled: boolean, defaultPreset: string) =>
    invoke<any>("cmd_game_switcher_configure", { enabled, defaultPreset }),
  gameApplyPreset: (gameId: string, preset: string) =>
    invoke<any>("cmd_game_apply_preset", { gameId, preset }),
  gameRevert: () => invoke<any>("cmd_game_revert"),
  // restore points
  createRestorePoint: (description: string) => invoke<string>("cmd_create_restore_point", { description }),
  listRestorePoints: () => invoke<any>("cmd_list_restore_points"),
  deleteRestorePoint: (sequenceNumber: number) => invoke<string>("cmd_delete_restore_point", { sequenceNumber }),
  launchRstrui: () => invoke<string>("cmd_launch_rstrui"),
  // auto optimizer
  healthScan: () => invoke<any>("cmd_health_scan"),
  optimizeApply: (ids: string[], restorePoint: boolean) => invoke<any>("cmd_optimize_apply", { ids, restorePoint }),
  resetRetired: () => invoke<any>("cmd_reset_retired"),
  openWindowsSettings: (page: string) => invoke<void>("cmd_open_windows_settings", { page }),
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

/** "5 minutes ago" in the UI language (Intl, so every locale is covered). */
export const fmtAge = (iso: string, lang = "en") => {
  const t = iso ? new Date(iso).getTime() : NaN;
  if (isNaN(t)) return "–";
  const rtf = new Intl.RelativeTimeFormat(lang, { numeric: "auto" });
  const mins = Math.floor((Date.now() - t) / 60000);
  if (mins < 60) return rtf.format(-mins, "minute");
  const h = Math.floor(mins / 60);
  if (h < 24) return rtf.format(-h, "hour");
  return rtf.format(-Math.floor(h / 24), "day");
};

export const fmtBytes = (b: number) => {
  // 1024-based: matches Windows Explorer, which is what users compare against.
  if (b >= 1024 ** 4) return (b / 1024 ** 4).toFixed(2) + " TB";
  if (b >= 1024 ** 3) return (b / 1024 ** 3).toFixed(2) + " GB";
  if (b >= 1024 ** 2) return (b / 1024 ** 2).toFixed(1) + " MB";
  if (b >= 1024) return (b / 1024).toFixed(0) + " KB";
  return b + " B";
};
