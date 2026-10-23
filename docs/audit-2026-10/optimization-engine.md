# System Health Check & Verified Optimization Engine

This document covers branch `audit/2026-10-overhaul`, commits `5f15d49` (engine) and `bf51a00` (UI).

## What it does

The engine follows six steps:

1. **Detect.** Read the Windows build, edition and servicing status, the CPU (model, cores, hybrid and dual-CCD X3D traits by model name), the GPUs and their driver dates, RAM, storage types, free space on the system drive, power plans, page file and Defender state. It also reads the live state of every catalog tweak.
2. **Analyse.** Compare each tweak against the rules in `optregistry.rs`.
3. **Diagnose.** Give each tweak one verdict:
   - correctly configured
   - recommended
   - optional
   - not applicable
   - review (applied but unsuitable or harmful)
   - applied without proven benefit
   - no longer in effect (drift)
   - failed (unfinished)
   - already covered
   - state not readable
4. **Recommend.** Only items that are applicable and offerable (conditional or maintenance evidence) and not already in place. A disabled setting is never recommended just because it is disabled.
5. **Repair.** *Optimize My PC* shows a preview and applies only what the user approves. Every item is re-checked against a fresh scan before it is applied, and it goes through the journaled engine, which captures the originals.
6. **Verify.** After applying, the engine reads each setting back. Each result is one of:
   - verified
   - verified, needs restart
   - already optimized
   - skipped
   - failed
   - could not be verified

   A command whose effect Windows cannot report is never shown as verified.

No measurement of game performance is made or claimed. The UI says so after every run and links to the Benchmark tool for before/after comparisons.

## Where each capability lives now

| Capability | Location |
|---|---|
| System Health Check | Dashboard → System Health Check (new) |
| One-click optimization | Optimize → Optimize My PC (replaces Auto-Optimizer, same id `autoopt`) |
| Per-tweak control | Optimize → Optimize. Shows review / recommended / gaming / system by default; Advanced lists all other tweaks with their classification, searchable, still applicable after an explicit warning |
| SFC / DISM | Dashboard → Windows repair (renamed from Health Check) |
| Privacy one-click items of the old Auto-Optimizer | Privacy Center ("apply all"). Privacy is no longer presented as a performance optimization |
| Old Auto-Optimizer page | Removed (`src/pages/AutoOptimizer.tsx`). The backend `autoopt` commands remain registered but have no UI |

## Real-system scan (developer PC, read-only)

The scan was run with `cargo test --locked -- --ignored real_health_scan` (registry, WMI and powercfg reads only).

**System:** Windows 11 25H2 build 26200.9457, AMD Ryzen 7 5800X 8-Core Processor (8C/16T), NVIDIA GeForce RTX 4080, 32 GB RAM, Ultimate Performance power plan, system drive 7.7 % free, HDD present.

**Summary:** 7 correctly configured, 2 recommended, 2 optional, 5 to review, 2 not applicable or not present, 0 failed, 1 attention item.

**Concrete findings on this PC:**
- **Game Mode is off** (`AutoGameModeEnabled=0`). It is recommended and offered as one-click.
- **Hibernation is on** (`hiberfil.sys` is 12.8 GB) while C: has 7.8 % free. Disabling it is recommended but is not one-click.
- **Low disk space** on C: is flagged as an attention item, with a link to Cleanup.
- **Earlier tweaks to review:**
  - SysMain off is unsuitable because an HDD is present.
  - Background apps off is a Windows-10-only setting.
  - DisablePagingExecutive is outdated.
  - Xbox services forced off is potentially harmful.
  - Cortana off is outdated.

Two detection bugs were found by checking this scan against the real registry, and both are fixed:
- A missing Game Mode value was read as "off".
- Unelevated `Test-Path` cannot see `hiberfil.sys`, so it reported no file. The engine uses file metadata instead.

## Per-tweak table

Column meanings:
- *Previous status* is the classification from the first audit ([optimization-audit.md](optimization-audit.md)).
- *Evidence* is the applicability or evidence note shown in the app.
- *Verification* is the live state read on the developer PC and the resulting verdict.

| Tweak | Previous status | Evidence | Changes | New classification | Verification |
|---|---|---|---|---|---|
| `power_high_performance` | CONDITIONALLY USEFUL | Desktops only. Not offered on laptops or systems without the High performance plan (Modern Standby). | Live state from the active scheme (was journal-only); desktop + plan present only; Ultimate counts as configured | Conditionally beneficial | not_applied (read from Windows) → correctly configured |
| `game_dvr_off` | VERIFIED | Helps when background recording/capture was enabled. Turn it back on if you use Game Bar clips. | One-click when capture is enabled | Conditionally beneficial · one-click | applied (read from Windows) → correctly configured |
| `mmcss_gaming` | CONDITIONALLY USEFUL | Only affects threads registered with the multimedia scheduler; most games do not register. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `startup_delay_off` | VERIFIED | Changes how Windows looks or behaves; not a performance change. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `visual_fx_performance` | INEFFECTIVE | Changes how Windows looks or behaves; not a performance change. | Fixed: also writes UserPreferencesMask (VisualFXSetting alone only selected the dialog option) | No proven benefit | partial (read from Windows) → applied, no proven benefit |
| `telemetry_minimal` | VERIFIED | Feedback Hub diagnostics stop working. | Classified; offered only when applicable | No proven benefit | partial (read from Windows) → applied, no proven benefit |
| `advertising_id_off` | VERIFIED | Reduces data collection or ads; not a performance change. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `sysmain_off` | CONDITIONALLY USEFUL | Prefetching costs little on SSDs and still helps on hard disks; not offered when a hard disk is present. | Not applicable when an HDD is present | No proven benefit | applied (read from Windows) → review (applied, unsuitable) |
| `hibernate_off` | VERIFIED | Offered only on desktops whose system drive is low on space. Not a performance change. | Live state (registry, else hiberfil.sys presence; was journal-only); offered only on desktops with < 15 % free | System maintenance | not_applied (read from Windows) → recommended |
| `menu_delay_fast` | VERIFIED | Changes how Windows looks or behaves; not a performance change. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `content_suggestions_off` | CONDITIONALLY USEFUL | Reduces data collection or ads; not a performance change. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `background_apps_off` | OBSOLETE (Win11) | Windows 11 removed this global setting; it only works on Windows 10. | Windows 10 only | Outdated | applied (read from Windows) → review (applied, unsuitable) |
| `mouse_accel_off` | VERIFIED | Useful for mouse-aimed games. Changes how the desktop cursor feels, so it is never applied automatically. | Recommended for mouse aiming, never automatic (changes input feel) | Conditionally beneficial | applied (read from Windows) → correctly configured |
| `game_mode_on` | VERIFIED | Only relevant when Game Mode was turned off; it is on by default. | Missing value now read as Windows default (on); one-click repair only when actually off | Conditionally beneficial · one-click | not_applied (read from Windows) → recommended |
| `fse_optimizations_off` | UNVERIFIED | Per-game compatibility settings are the reliable control. | Classified; offered only when applicable | Unverified | partial (read from Windows) → applied, no proven benefit |
| `hags_on` | CONDITIONALLY USEFUL | Needs a GPU and driver that support it (WDDM 2.7+). Support cannot be confirmed before restart, so it is never applied automatically. | Dedicated GPU required; never one-click (support unconfirmable before restart) | Conditionally beneficial | applied (read from Windows) → correctly configured |
| `priority_separation` | INEFFECTIVE | 0x26 is the same scheduling as the Windows client default. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `power_throttling_off` | CONDITIONALLY USEFUL | Power throttling targets background apps; a foreground game is not throttled. Harmful on laptops and hybrid CPUs. | Desktop, non-hybrid only; never recommended | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `mmcss_games_task` | CONDITIONALLY USEFUL | Only affects threads that register with the MMCSS "Games" task. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `mpo_off` | CONDITIONALLY USEFUL | Only if you actually see those symptoms. Reportedly ignored on newer Windows builds. | Classified; offered only when applicable | Conditionally beneficial | not_applied (read from Windows) → optional |
| `gamebar_popups_off` | VERIFIED | Changes how Windows looks or behaves; not a performance change. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `transparency_off` | VERIFIED | Changes how Windows looks or behaves; not a performance change. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `min_animate_off` | VERIFIED | Changes how Windows looks or behaves; not a performance change. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `start_websearch_off` | VERIFIED | Also removes File Explorer search suggestions. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `lockscreen_ads_off` | VERIFIED | Reduces data collection or ads; not a performance change. | Classified; offered only when applicable | No proven benefit | partial (read from Windows) → applied, no proven benefit |
| `widgets_off` | NEEDS FIX → fixed text | Changes how Windows looks or behaves; not a performance change. | Classified; offered only when applicable | No proven benefit | partial (read from Windows) → applied, no proven benefit |
| `delivery_opt_off` | VERIFIED | Updates for several PCs on one network may download more data. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `wer_off` | VERIFIED | Crash reports for troubleshooting are no longer created. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `activity_history_off` | VERIFIED | Reduces data collection or ads; not a performance change. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `usb_suspend_off` | NEEDS FIX → **fixed** | Only for that problem; desktops only. Applies to the current power plan. | Desktop only; troubleshooting | Conditionally beneficial | not_applied (read from Windows) → optional |
| `fast_startup_off` | VERIFIED | Only for those cases. Already implied by disabling hibernation. | Marked redundant when hibernation is off | System maintenance | applied (read from Windows) → correctly configured |
| `disable_paging_executive` | OBSOLETE | No measurable effect with modern memory management. | Classified; offered only when applicable | Outdated | applied (read from Windows) → review (applied, unsuitable) |
| `core_parking_off` | CONDITIONALLY USEFUL / risky | Defeats the core parking AMD dual-CCD X3D and Intel hybrid CPUs rely on to keep games on the right cores; High performance plans already unpark cores. | Excluded on dual-CCD X3D and hybrid CPUs; never recommended | Potentially harmful | unknown (not readable) → not recommended |
| `ntfs_last_access_off` | INEFFECTIVE | Since Windows 10 1803 last-access updates are system-managed and already off on most volumes. | Classified; offered only when applicable | Outdated | not_applied (read from Windows) → not recommended |
| `ntfs_8dot3_off` | CONDITIONALLY USEFUL | Windows already disables short names on new non-system volumes; old installers may need them. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `xbox_services_off` | INEFFECTIVE | The services are already manual-start; forcing them off can break Game Pass and Xbox sign-in. | Reclassified harmful (Game Pass / Xbox sign-in) | Potentially harmful | partial (read from Windows) → review (applied, unsuitable) |
| `remote_registry_off` | INEFFECTIVE | Already disabled by default on Windows 10/11 client editions. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `cortana_off` | OBSOLETE | The Cortana app was retired. | Classified; offered only when applicable | Outdated | applied (read from Windows) → review (applied, unsuitable) |
| `edge_prelaunch_off` | VERIFIED | Edge will show "managed by your organization". | Classified; offered only when applicable | No proven benefit | not_applied (read from Windows) → not recommended |
| `search_highlights_off` | NEEDS FIX → fixed text | Changes how Windows looks or behaves; not a performance change. | Classified; offered only when applicable | No proven benefit | partial (read from Windows) → applied, no proven benefit |
| `auto_play_off` | VERIFIED | Stops media and USB drives from starting programs on insert (security hygiene). | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `svc_fax_off` | VERIFIED (trivial) | Fax is manual-start and idle when unused. | Classified; offered only when applicable | No proven benefit | unsupported (read from Windows) → not present |
| `svc_geolocation_off` | INEFFECTIVE | Already manual-start by default. | Classified; offered only when applicable | No proven benefit | not_applied (read from Windows) → not recommended |
| `svc_wmp_network_off` | VERIFIED (trivial) | Often absent on Windows 11. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `svc_maps_off` | VERIFIED (trivial) | Stops offline map updates. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `svc_link_tracking_off` | VERIFIED (trivial) | Breaks repair of moved-file shortcuts on network shares. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `svc_wisvc_off` | VERIFIED (trivial) | Breaks Windows Insider builds. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `svc_diag_tracking_off` | VERIFIED (relabelled) | Diagnostics Hub collector used by Visual Studio; idle otherwise. | Classified; offered only when applicable | No proven benefit | unsupported (read from Windows) → not present |
| `wsearch_manual` | CONDITIONALLY USEFUL | Slows or breaks Start menu, Outlook and Settings search. | Classified; offered only when applicable | No proven benefit | applied (read from Windows) → applied, no proven benefit |
| `nagle_off` | INEFFECTIVE | TCPNoDelay is not read by Windows; games set Nagle per socket, and most shooters (incl. Rainbow Six Siege) use UDP. | Classified; offered only when applicable | No proven benefit | unknown (not readable) → not recommended |
| `tcp_timed_wait_low` | INEFFECTIVE | Only matters for servers exhausting ports. | Classified; offered only when applicable | No proven benefit | not_applied (read from Windows) → not recommended |
| `irp_stack_high` | INEFFECTIVE | Only affects the SMB file-sharing server. | Classified; offered only when applicable | No proven benefit | not_applied (read from Windows) → not recommended |
| `qos_bandwidth_free` | INEFFECTIVE | Windows does not reserve 20 % bandwidth; this is a long-standing myth. | Classified; offered only when applicable | No proven benefit | not_applied (read from Windows) → not recommended |
| `tcp_autotuning_normal` | INEFFECTIVE | Only relevant when auto-tuning was changed from Normal. | Live state via Get-NetTCPSetting (was journal-only); one-click repair when not Normal | System maintenance · one-click | applied (read from Windows) → correctly configured |
| `net_rss_on` | INEFFECTIVE | Only relevant when RSS was turned off. | Live state via Get-NetOffloadGlobalSetting (was journal-only); one-click repair when disabled | System maintenance · one-click | applied (read from Windows) → correctly configured |

## Test coverage

| Area | Tests |
|---|---|
| Registry completeness (every catalog tweak exactly once) | `optregistry::every_catalog_tweak_has_exactly_one_entry` |
| Harmful, outdated, unverified and unproven tweaks are never recommended or one-click | `optregistry::harmful_outdated_and_unverified_are_never_recommended`, `optimizer::harmful_and_unproven_tweaks_are_never_recommended` |
| One-click never needs a restart or changes input feel | `optregistry::one_click_only_offers_offerable_tweaks` |
| Eligibility (laptop, no High performance plan, X3D, Win10-only, low disk) | `optregistry::eligibility_rules` |
| Hardware detection rules (X3D, hybrid, dedicated GPU) | `optimizer::cpu_and_gpu_model_rules` |
| Diagnosis of applied, unapplied, unsupported, unknown, drift, failure, equivalents and redundancy | `optimizer::applied_states_are_classified`, `journal_drift_and_failures`, `equivalents_and_redundancy` |
| Missing admin rights | `optimizer::one_click_requires_admin_when_needed` |
| Verification never reports unconfirmed success | `optimizer::verification_never_reports_unconfirmed_success` |
| Windows servicing and system checks (EOS, disk, page file, Defender, power saver, laptop exception) | `optimizer::servicing_and_system_checks` |
| Game Mode default, live-state parsing | `tweaks::game_mode_missing_value_means_windows_default_on`, `live_flag_needs_a_single_value` |
| Real system, read-only (opt-in) | `optimizer::real_health_scan`; the health scan is also covered by `smoke::read_only_scans_have_no_powershell_parser_errors` |

## Not tested

- **Applying and undoing on a real system.** No optimization was applied on the developer PC during this work.
  - The apply → read-back path has unit-test coverage for its decision logic only.
  - Requirement: run *Optimize My PC* on a disposable Windows 10 22H2 and Windows 11 24H2/25H2 VM (English and German). Check each result category, then undo from the history and rescan.
- **The UI with a live backend.** The pages were checked in the browser with the real scan data of this PC injected; apply and undo buttons were not exercised.
  - Requirement: start the elevated dev app, open Dashboard → System Health Check, and apply and undo one recommended item.
- **Hybrid-CPU and X3D detection** uses model names; it was checked only against test strings.
- **Performance impact.** Not measured. A credible comparison needs:
  - a fixed scene (for Rainbow Six Siege, the built-in benchmark)
  - PresentMon or CapFrameX capture
  - at least 3 runs per state
  - average FPS, 1 % lows and frametime variance
