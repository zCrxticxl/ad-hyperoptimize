# Windows Optimization Audit (2026-10)

Scope: every optimization the app can apply, as implemented on branch
`audit/2026-10-overhaul`. Target platforms: Windows 10 22H2, Windows 11 23H2–25H2.

**How this was verified.** Each entry was traced from its UI control to the
exact registry value, service, or command, with the code cited. The
classification combines that code review with documented Windows behavior.

**What was not verified.** No tweak was applied to a live system during this
audit (AGENTS.md forbids it without explicit approval). "VERIFIED" therefore
means the implementation writes the intended, currently valid setting, captures
the original, and restores it. It is **not** a measured performance gain. No
FPS numbers were produced or are claimed.

Classes: VERIFIED · CONDITIONALLY USEFUL · NEEDS FIX · OBSOLETE · INEFFECTIVE ·
UNSAFE · UNVERIFIED.

**Rollback column:**

- **CAP**: the original value is captured in the journal; undo restores it, or
  deletes the value if it did not exist before.
- **CAP-cmd**: an exact revert command is built from the captured state.
- **HC**: a hardcoded default.
- **NONE**: no undo.

## Optimize → Tweaks catalog (`src-tauri/src/tweaks.rs`)

| id | What it changes | Rollback | Admin / reboot | Class | Notes |
|---|---|---|---|---|---|
| power_high_performance | `powercfg /setactive` High performance | CAP-cmd | no / no | CONDITIONALLY USEFUL | Missing on Modern Standby systems; costs battery on laptops |
| game_dvr_off | HKCU GameConfigStore `GameDVR_Enabled`=0, GameDVR `AppCaptureEnabled`=0 | CAP | no / no | VERIFIED | Often already 0 on Windows 11 |
| mmcss_gaming | `SystemProfile` SystemResponsiveness=10, NetworkThrottlingIndex=0xFFFFFFFF | CAP | yes / reboot | CONDITIONALLY USEFUL | Only affects MMCSS-registered threads |
| startup_delay_off | `Serialize\StartupDelayInMSec`=0 | CAP | no / logon | VERIFIED | Faster startup-app launch, not a game gain |
| visual_fx_performance | `VisualFXSetting`=2 | CAP | no / – | INEFFECTIVE | Only sets the radio button; the real UserPreferencesMask is untouched |
| telemetry_minimal | `AllowTelemetry`=1 + DiagTrack Disabled | CAP | yes / reboot | VERIFIED | Privacy, not performance |
| advertising_id_off | AdvertisingInfo `Enabled`=0 | CAP | no / no | VERIFIED | Privacy |
| sysmain_off | SysMain Disabled | CAP | yes / reboot | CONDITIONALLY USEFUL | No SSD/HDD check; minor cost on HDDs |
| hibernate_off | `powercfg /h off` | CAP-cmd | yes / no | VERIFIED | Also disables Fast Startup; frees hiberfil.sys |
| menu_delay_fast | `MenuShowDelay`="100" | CAP | no / logoff | VERIFIED | Cosmetic |
| content_suggestions_off | CDM 338389 / SoftLanding = 0 | CAP | no / no | CONDITIONALLY USEFUL | Tips only; Start recommendations untouched |
| background_apps_off | `GlobalUserDisabled`=1 | CAP | no / no | OBSOLETE (Win11) | Windows 11 removed the global toggle |
| mouse_accel_off | Mouse `MouseSpeed`/`Threshold1`/`Threshold2`="0" | CAP | no / logoff | VERIFIED | Real input-consistency change for FPS players |
| game_mode_on | GameBar `AutoGameModeEnabled`=1 | CAP | no / no | VERIFIED | Already the Windows default |
| fse_optimizations_off | GameConfigStore FSEBehaviorMode=2 etc. | CAP | no / logoff | UNVERIFIED | Largely ignored for DX12/flip-model on Windows 11 |
| hags_on | `GraphicsDrivers\HwSchMode`=2 | CAP | yes / reboot | CONDITIONALLY USEFUL | Needs WDDM 2.7+ GPU; Windows 11 24H2 often enables it already |
| priority_separation | `Win32PrioritySeparation`=0x26 | CAP | yes / no | INEFFECTIVE | 0x26 = short, variable, 3:1 = the client default. The old description ("fixed") was wrong |
| power_throttling_off | `PowerThrottlingOff`=1 | CAP | yes / reboot | CONDITIONALLY USEFUL | Hurts laptop battery and hybrid E-core scheduling |
| mmcss_games_task | `Tasks\Games` Priority=6, Scheduling Category=High | CAP | yes / reboot | CONDITIONALLY USEFUL | Only threads that register with MMCSS "Games" |
| mpo_off | Dwm `OverlayTestMode`=5 | CAP | yes / reboot | CONDITIONALLY USEFUL | Fixes MPO flicker only; ignored on newer builds |
| gamebar_popups_off | GameBar ShowStartupPanel / UseNexusForGameBarEnabled | CAP | no / no | VERIFIED | The second value is the controller Xbox button |
| transparency_off | `EnableTransparency`=0 | CAP | no / – | VERIFIED | Cosmetic |
| min_animate_off | `MinAnimate`="0" | CAP | no / logoff | VERIFIED | Cosmetic |
| start_websearch_off | Policy `DisableSearchBoxSuggestions`=1 | CAP | no / Explorer restart | VERIFIED | Privacy |
| lockscreen_ads_off | CDM RotatingLockScreenOverlay / 338387 | CAP | no / no | VERIFIED | |
| widgets_off | Dsh `AllowNewsAndInterests`=0 + `TaskbarDa`=0 | CAP | yes / Explorer | NEEDS FIX → fixed text | UCPD blocks `TaskbarDa` writes; no effect on Windows 10 |
| delivery_opt_off | DO `DODownloadMode`=0 | CAP | yes / no | VERIFIED | Turns off LAN peering too |
| wer_off | WER `Disabled`=1 | CAP | yes / no | VERIFIED | Loses crash diagnostics |
| activity_history_off | `PublishUserActivities`=0 | CAP | yes / no | VERIFIED | Privacy |
| usb_suspend_off | powercfg USB selective suspend AC=0 | CAP-cmd | yes / no | NEEDS FIX → **fixed** | Undo was a silent no-op (quoting bug); status now read from the active scheme |
| fast_startup_off | `HiberbootEnabled`=0 | CAP | yes / next shutdown | VERIFIED | |
| disable_paging_executive | `DisablePagingExecutive`=1 | CAP | yes / reboot | OBSOLETE | No measurable benefit on modern systems |
| core_parking_off | powercfg CPMINCORES=100 | CAP-cmd | yes / no | CONDITIONALLY USEFUL / risky | Undo was a no-op (**fixed**). Defeats AMD X3D CCD parking and Intel hybrid parking |
| ntfs_last_access_off | `NtfsDisableLastAccessUpdate`=1 | CAP | yes / reboot | INEFFECTIVE | Already system-managed off since 1803 |
| ntfs_8dot3_off | `NtfsDisable8dot3NameCreation`=1 | CAP | yes / reboot | CONDITIONALLY USEFUL | Tiny gain; legacy installers may break |
| xbox_services_off | Xbl* services Manual | CAP | yes / no | INEFFECTIVE | Already Manual (trigger-start) by default |
| remote_registry_off | RemoteRegistry Disabled | CAP | yes / no | INEFFECTIVE | Already disabled on client editions |
| cortana_off | `AllowCortana`=0 | CAP | yes / no | OBSOLETE | Cortana app retired |
| edge_prelaunch_off | Edge `StartupBoostEnabled`=0 | CAP | yes / no | VERIFIED | Edge shows "managed by your organization" |
| search_highlights_off | `IsDynamicSearchBoxEnabled`=0 (+ Win10 Feeds value) | CAP | no / Explorer | NEEDS FIX → fixed text | |
| auto_play_off | `DisableAutoplay`=1 | CAP | no / no | VERIFIED | Security hygiene |
| svc_fax_off / svc_wmp_network_off / svc_maps_off / svc_link_tracking_off / svc_wisvc_off | service Disabled | CAP | yes / no | VERIFIED (trivial) | Missing service no longer shown as "applied" |
| svc_geolocation_off | lfsvc Manual | CAP | yes / no | INEFFECTIVE | Already Manual |
| svc_diag_tracking_off | diagnosticshub.standardcollector Disabled | CAP | yes / no | VERIFIED (relabelled) | This is the VS/ETW collector, not DiagTrack |
| wsearch_manual | WSearch Manual | CAP | yes / no | CONDITIONALLY USEFUL | Slows Start, Outlook and Settings search |
| nagle_off | TcpAckFrequency / TCPNoDelay per interface | CAP-cmd | yes / reboot | INEFFECTIVE | TCPNoDelay is not read by tcpip.sys; R6 Siege and most shooters use UDP. Undo fixed |
| tcp_timed_wait_low | `TCPTimedWaitDelay`=30 | CAP | yes / reboot | INEFFECTIVE | Server port-exhaustion setting |
| irp_stack_high | LanmanServer `IRPStackSize`=32 | CAP | yes / reboot | INEFFECTIVE | SMB server only; the gaming claim is a myth |
| qos_bandwidth_free | Psched `NonBestEffortLimit`=0 | CAP | yes / reboot | INEFFECTIVE | Long-standing myth |
| tcp_autotuning_normal | netsh autotuninglevel=normal | CAP-cmd | yes / no | INEFFECTIVE | Already the default |
| net_rss_on | netsh rss=enabled | CAP-cmd | yes / no | INEFFECTIVE | Already enabled by default; blind force-revert disabled RSS (**fixed**) |

## Perf tweaks (`perftweaks.rs`)

| id | Change | Rollback | Class | Notes |
|---|---|---|---|---|
| timer_set | `GlobalTimerResolutionRequests`=1 + SYSTEM logon task | CAP + reset | UNSAFE → **fixed** / CONDITIONALLY USEFUL (Win11 only) | The task ran a script from a user-writable ProgramData folder as SYSTEM (local privilege escalation); the reset never deleted it because of a literal TAB byte in the path |
| msi_set | `MSISupported` per device | NONE | NEEDS FIX → **fixed** | Accepted any device, including the boot storage controller |
| net_tweak | `Set-NetAdapterAdvancedProperty` (6 keywords) | NONE | CONDITIONALLY USEFUL | |
| net_reset_all | `Reset-NetAdapterAdvancedProperty` on all NICs | NONE | UNSAFE | Wipes VLAN, jumbo frames, WoL. Now reports real per-adapter failures |
| ram_flush_standby | `NtSetSystemInformation(80,4)` | n/a | INEFFECTIVE | Placebo even when it works |
| pagefile_set_auto | WMI AutomaticManagedPagefile=true | NONE | VERIFIED | |
| pagefile_set_custom | WMI `Win32_PageFileSetting` | NONE | CONDITIONALLY USEFUL | |
| pagefile_disable | delete pagefile settings | NONE | UNSAFE | Out-of-memory crashes, no crash dumps; always requires confirmation |

## GPU tweaks (`gputweaks.rs`)

| id | Class | Notes |
|---|---|---|
| nv_power_max_perf, nv_threaded_opt, nv_prerender_limit_1, nv_low_latency_ultra | INEFFECTIVE | Modern drivers read these from the DRS profile, not these registry values. Hidden from the active list; undo kept for users who applied them |
| nv_dynamic_pstate_off | CONDITIONALLY USEFUL / risky | Pins P0: heat, power, laptop battery |
| nv_fts_telemetry_off | UNVERIFIED | Old GeForce Experience-era flags |
| nv_telemetry_svc_off | relabelled | NvContainerLocalSystem is the NVIDIA App/GFE container; disabling it breaks overlay and instant replay |
| amd_ulps_off | CONDITIONALLY USEFUL | Multi-GPU only; reboot required (flag fixed) |
| amd_sclk_sleep_off, amd_dma_power_off | OBSOLETE / UNVERIFIED | Pre-RDNA keys |
| amd_compute_preemption_off | UNSAFE | Risks TDRs and hangs |
| amd_rx_tex_cache, dx_shader_cache_unlimited | INEFFECTIVE | Were already blocked; not real settings |

## Power plans (`powerplan.rs`), boot (`bootopt.rs`), gaming (`gameboost.rs`, `gameprofile.rs`)

| Action | Class | Notes |
|---|---|---|
| set active plan | VERIFIED | |
| unlock Ultimate | CONDITIONALLY USEFUL | Desktop only. Duplicated on every click in non-EN/DE locales (**fixed**) |
| BCD timeout 0 | NEEDS FIX → **fixed** | Previous value is now captured |
| BCD bootlog off / quietboot | INEFFECTIVE | Removed from the active list |
| BCD nx OptOut | UNSAFE | Removed from the active list (client default is OptIn) |
| BCD testsigning off | CONDITIONALLY USEFUL | |
| Quick Boost: kill overlays | UNSAFE → **fixed** | Substring matching killed msedgewebview2 (this app's own window) and Ubisoft Connect (required by Rainbow Six Siege) |
| Quick Boost: affinity "first half of CPUs" | UNSAFE → removed | Halved real cores on SMT CPUs |
| Quick Boost: HAGS toggle | INEFFECTIVE → removed | Needs a reboot and persisted |
| Quick Boost: High priority | CONDITIONALLY USEFUL | BattlEye/EAC/Vanguard deny it; now skipped instead of rolling back the whole boost |
| set_gpu_max_perf | UNSAFE → disabled | Wrote HwSchMode=1, which turns HAGS **off** |
| per-game "ultimate" preset | INEFFECTIVE → **fixed** | Used a template GUID that is never installed |

## Privacy, debloat, services, security

| Area | Finding | Status |
|---|---|---|
| Privacy telemetry status | A missing value was reported as "applied" | fixed |
| Debloat list | XboxIdentityProvider breaks Game Pass sign-in; ZuneMusic is the Windows 11 Media Player | no longer recommended |
| Services | No backend deny-list: wuauserv, BFE, Audiosrv and others could be disabled | deny-list added |
| Defender real-time/cloud off | One click, no confirmation, no undo, wrong cloud status | confirmation + captured revert |
| Scheduled tasks | Wildcard and smart-quote names could inject into PowerShell | validated |

## Anti-cheat and Rainbow Six Siege

None of the catalog entries change VBS/HVCI, Spectre/Meltdown mitigations,
test-signing or driver signature enforcement. So nothing the app applies by
default puts a BattlEye-protected title at risk.

Two behaviors were directly harmful for R6 Siege and are fixed:

- Quick Boost killed Ubisoft Connect (`upc`).
- Process-priority changes denied by BattlEye rolled back the whole boost.

Process priority for anti-cheat titles is now a skipped step.

## Recommended one-click preset

The Auto-Optimizer preset applies only `game_dvr_off` and `game_mode_on`. Both
are VERIFIED and low risk, so the preset is unchanged. Nothing in this audit
justifies adding more items to a one-click preset without measured evidence on
the target machine.
