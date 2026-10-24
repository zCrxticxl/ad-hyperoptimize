# Full code review (line by line)

Started 2026-10-24 on `audit/2026-10-overhaul` at `13fa311`, at the owner's request to review the whole code base.

**Scope:** every file, read in full:
- Rust: 26,208 lines
- TypeScript/React: 16,265 lines
- translations: 10,341 lines

**What is checked:**
- correctness
- security (injection, paths, privileges, inherited environment)
- error handling
- undo/journal integrity
- backend and frontend agreement

**Rules:**
- Clear defects are fixed in their own commits, with a test where the logic is testable.
- Borderline items are listed for a decision.
- A review lowers the risk of defects; it cannot prove the code is 100 % correct. Registry, service and PowerShell behaviour is finally confirmed only on Windows.

Severity:
- **High:** wrong result or damage in normal use, or a privilege escalation.
- **Medium:** wrong under realistic conditions, or a hardening gap with a plausible attack.
- **Low:** edge case, robustness, misleading text.
- **Info:** style, no behaviour change.

## Progress

| File | Lines | Status |
|---|---|---|
| `ps.rs` | 547 | reviewed |
| `lib.rs` | 1873 | reviewed |
| `tweaks.rs` | 2490 | reviewed |
| `statestore.rs` | 870 | reviewed |
| `safety.rs` | 500 | reviewed |
| `optimizer.rs` | 1830 | reviewed |
| `optregistry.rs` | 589 | reviewed |
| `privacy.rs` | 795 | reviewed |
| `debloater.rs` | 940 | reviewed |
| `gputweaks.rs` | 1053 | reviewed |
| `regclean.rs` | 1035 | reviewed |

## Findings

| # | File:line | Severity | Finding | Status |
|---|---|---|---|---|
| 1 | `ps.rs:135` | Medium | `Command::new("powershell.exe")` without a path. Windows searches the app dir, System32, then `PATH`, and `powershell.exe` lives in `System32\WindowsPowerShell\v1.0`, so it was found through `PATH`. A user-writable folder early in `PATH` would run a planted `powershell.exe` with admin rights. | fixed: absolute path from `GetSystemDirectory` |
| 2 | `ps.rs:configure` | Medium | **Children of the elevated process inherited the caller's environment.** Code running as the user can start the app with a tampered environment; if the user approves the genuine UAC prompt, admin children can be redirected through:<br>• `PATH`: PowerShell resolves `powercfg`, `bcdedit`, … through it<br>• `PSModulePath`: module autoload (NetAdapter, Defender, ScheduledTasks, …)<br>• `SystemRoot`: e.g. the `SYSTEM` scheduled task for the timer resolution takes its program path from `$env:SystemRoot` | fixed: every child gets `PATH`, `PSModulePath`, `SystemRoot`, `windir`, `SystemDrive` and `ComSpec` derived from `GetWindowsDirectory` |
| 3 | `lib.rs:236,1104`, `optimizer.rs:963`, `diskanalyzer.rs:820`, `tweaks.rs:252,1633` | Medium | `%SystemRoot%` / `%SystemDrive%` from the process environment built paths of programs started elevated (`explorer.exe`, `nvidia-smi.exe`) and status checks (`hiberfil.sys`). | fixed: `ps::windows_dir()` / `ps::system_dir()` from the API |
| 4 | `lib.rs:1589,1622` | Low | `ensure_admin` starts `powershell` without a path (before elevation). | fixed: absolute path |
| 5 | `lib.rs:6` | Info | `rustfmt` moved the comment of `mod commands` above `mod cleanup`. | fixed |
| 6 | `ps.rs:winget_exe` | Low | Side effect of #2: if the DesktopAppInstaller package location cannot be resolved, the fallback bare `winget.exe` is no longer found through the user's `PATH` (the user-writable App Execution Alias). Fails closed instead of running the alias elevated. | accepted, documented |
| 7 | `lib.rs` (sync `#[tauri::command(async)] fn`) | Low | `cmd_startup_toggle`, `cmd_perm_priority_set/remove` and `cmd_ctxmenu_*` do blocking work in a sync command body. Tauri runs them off the main thread, but on the async runtime rather than `spawn_blocking`. Only a responsiveness concern. | open (low) |
| 8 | `statestore.rs:sanitize_entry`, `tweaks.rs:refuse_untrusted` | High | **Migrated journal entries could write any HKLM value elevated.** The migration from the user-writable `%APPDATA%\PCOptSuite` checked only revert *commands*. Registry items were trusted when the root was HKLM/HKCU, and service items when the name was a safe identifier, with any path, name and captured value. `refuse_untrusted` also blocked only commands. Code running as the user could plant `journal.json` (for example IFEO `Debugger`, `Run`, Defender policies). Unelevated sessions keep using that folder, so this works again and again: every elevated start migrates a new file. The next Undo of that tweak then writes the planted value with admin rights. | fixed: a registry/service item is trusted only when the feature named by `tweak_id` declares exactly that target: catalog tweak, `privacy:`, `gpu:` (driver class key + declared value), `nvCtrlPanel:`, `perftweaks:timer`, `startup:<scope>:<name>` (only its own StartupApproved value). `service:<name>` entries can target any service and are never trusted from that folder. An untrusted entry runs none of its steps; the error lists them for a manual check. Tests: `planted_registry_and_service_targets_are_not_trusted`, `declared_registry_and_service_targets_stay_trusted`. **Residual:** entries migrated into `%ProgramData%` before this fix are not re-checked. |
| 9 | `tweaks.rs:34` | Info | The doc comment on `Action::Cmd.show` said a failed capture falls back to `revert`; apply actually refuses. | fixed |
| 10 | `tweaks.rs:apply` (step 0) | Low | With an open journal entry and live status `partial`, apply reports `already_applied`. The UI then shows success although part of the tweak is not in effect. | open: decide between an error ("undo first") and the current behaviour |
| 11 | `tweaks.rs:detect_status` | Low | A failed service query (`Err`) counts as "checkable, not matching". A PowerShell failure then shows `not_applied`/`partial` instead of `unknown`, and force-Undo is hidden. | open |
| 12 | `tweaks.rs` catalog, `may_help_performance` | Low | Some `rationale`/`impact` texts claim effects that were never measured: `game_dvr_off` "Measurable frametime stability gain", `transparency_off` "measurable on iGPUs", `ntfs_8dot3_off` "Faster file creation", `fse_optimizations_off` "Lower input lag", `power_high_performance` "Lower … latency". The texts appear in the global search. | open (the `may_help_performance` part is done, see #22): reword to "may"/"no measured effect" (en + `deTweaks.ts`), or measure with the FPS page |
| 13 | `tweaks.rs:revert` force path | Info | Force-revert of a RegSet imports the newest `.reg` backup of the whole key. Other values in that key that changed since the backup are reset to the backup as well. By design (fallback without a journal); the value check afterwards only covers the tweak's own value. | accepted, documented |
| 14 | `statestore.rs:trusted_bcdedit/_defender/_sched_task/_service_start` | High | **The command check accepted valid shapes regardless of the entry.** `bcdedit /set {current} <any lowercase element> <alnum>` passed, so planted `testsigning on`, `nointegritychecks on` or `hypervisorlaunchtype off` were carried over as trusted. The same applied to `Set-MpPreference -DisableRealtimeMonitoring $true` / `-MAPSReporting 0`, `Disable-ScheduledTask` on any task, and `Start-Service` for any of the four privacy services. The next Undo ran them elevated. | fixed: BCD only on the object + element of the entry's `bcd:<id>` tweak, never test signing on or `nx AlwaysOff`. Defender only reverts that turn protection back on, under its own tweak id. Scheduled task only `Enable-ScheduledTask` of the task named in `task:<path><name>`. `Start-Service` only for the service of the entry's `privacy:<id>`. Legacy entries with an older shape (e.g. `bootmenupolicy` on `{current}`) are now untrusted: manual undo, fails closed. |
| 15 | `statestore.rs:migrate_debloater` | Medium | The file content was validated, then the *path* was copied again with `fs::copy`. The legacy folder is user-writable, so a symlink swapped in between check and copy made the elevated process copy any admin-only file into `%ProgramData%\ADHyperOptimize\debloater`, which Authenticated Users can read. | fixed: the validated bytes are written |
| 16 | `statestore.rs:ensure_secure_dir` | Info | Checked: a pre-created or weak folder is moved aside; a new folder is hardened and must still be empty and not a reparse point; `%ProgramData%` comes from HKLM, not from the environment. A user holding a handle can block the rename, which only stops the app (no escalation). | ok |
| 17 | `safety.rs:latest_backup_for` | Low | The prefix match `<root>_<path>_` also hit backups of subkeys (`Key\Sub` → `Key_Sub_<ts>.reg`). Force-revert could then import the wrong key; the value check afterwards caught only the case where the tweak value stayed. An unreadable directory entry (`?` in the loop) also discarded every backup. | fixed: exact `<prefix><YYYYMMDD-HHMMSS>.reg` match, unreadable entries are skipped; test `backups_match_their_own_key_only` |
| 18 | `safety.rs:backup_registry_key` | Info | Backup names have one-second resolution. A second backup of the same key within the same second overwrites the first (`/y`). Apply calls are serialized and take longer than that in practice. | accepted |
| 19 | `optimizer.rs:windows_servicing` | Low | End-of-servicing dates:<br>• Enterprise/Education was computed as "Home/Pro + 1 year", 1–3 days off (24H2 Ent: 2027-10-13 instead of 2027-10-12; 25H2: 2028-10-12 instead of 2028-10-10; 23H2: 2026-11-11 instead of 2026-11-10; 22H2: 2025-10-08 instead of 2025-10-14).<br>• `ProfessionalEducation` got the Enterprise date ("contains Education").<br>• LTSC (`EnterpriseS`, `IoTEnterpriseS`) was judged by GA dates; it would have warned 2027 on LTSC 2024, which is supported until 2029.<br>• Windows 10 21H2 / Windows 11 21H2 (19044/22000), long unsupported, got no warning at all. | fixed: explicit table of both dates per release, LTSC excluded, edition by `EditionID` prefix; extended test |
| 20 | `optimizer.rs:ram_check` | Low | `Win32_PhysicalMemory.ConfiguredClockSpeed` is documented in MHz; some firmware reports half the transfer rate (1600 for DDR4-3200). The XMP check would then warn although XMP is on. | open: needs a real report from an affected board before changing |
| 21 | `optimizer.rs:apply` | Low | When the user asked for a restore point and Windows created none (and no recent one exists), the one-click apply still runs. The result shows the failed restore point; the journal Undo stays available. | open: decide whether a failed restore point should stop the apply |
| 22 | `tweaks.rs:may_help_performance` | Low | `performancePotential` in `list_with_status` marked tweaks as having "measurable performance potential" that `optregistry` classifies as retired or NoProvenBenefit (`mmcss_gaming`, `ntfs_8dot3_off`, `svc_maps_off`, `transparency_off`, …). No frontend reads it. | fixed: removed; the High-risk assertion kept as its own test |
| 23 | `optregistry.rs` | Info | Checked: every catalog id has exactly one entry; no NoProvenBenefit/Outdated/Harmful/Unverified entry is recommended or one-click; one-click never needs a restart. The `nagle_off` text says Windows does not read `TCPNoDelay`, while the catalog rationale says games set `TCP_NODELAY` themselves. Both lead to "no effect"; the registry claim is not verified here. | ok / note |
| 24 | `privacy.rs:apply` | Medium | When the apply script or the read-back check failed, nothing was rolled back. The script can already have changed values or stopped/disabled the service. The entry stayed open as "did not finish"; the tweak engine and the Debloater roll back in that case. | fixed: `revert_entry` restores the captured originals; the message says whether the rollback was complete (restore token otherwise) |
| 25 | `privacy.rs:apply` | Low | No check for an already open entry: a second apply captures the tweak's own values, so the first Undo restores those and only the second restores the original. The tweak engine refuses or reports `already_applied` instead. | open |
| 26 | `debloater.rs:capture_state` | **High** | **Cause of the failing Cortana reset.** `$out \| ConvertTo-Json` writes a single captured item as a bare object, not as a one-element array. That affects every tweak with exactly one value or only a service: `cortana_off`, `telemetry_off`, `advertising_id`, `location_off`, `feedback_off`, `taskbar_widgets_off`, `diagtrack_stop`, `error_reporting_off`. `restore_script` only read arrays and produced an empty script, so:<br>• Revert ran the guessed `undo` instead of the captured original (location set to `Allow`, ad ID to `1`, …). For Cortana the guess `Remove-ItemProperty … -EA SilentlyContinue` fails once the value is gone (e.g. after the Optimize reset); `powershell -Command` then exits 1, matching the report.<br>• The rollback of a failed apply reported "rolled back" without restoring anything.<br>• Migration (`state_is_valid`) discarded these files. | fixed: capture writes `ConvertTo-Json -InputObject @($out)` (always an array); existing files with a bare object are read as one item. Test `single_item_state_restores_exactly`. PowerShell behaviour still to be confirmed on Windows. |
| 27 | `gputweaks.rs:nv_set_setting/nv_get_settings`, `NvidiaControlPanel.tsx` | Medium | The "NVIDIA Control Panel" page writes exactly the values the GPU catalog itself marks as **LEGACY, no effect** (`PowerMizer*`, `Prerenderlimit`, `NVPCFLatencyPolicy`/`RTHM_MODE`, `NVTweak\Enabled`). Current drivers take these settings only from the DRS profile. The page therefore offers switches that change nothing. It also shows defaults ("adaptive", "3", "off") as the current state, which is not read from the driver. | open: recommended to block new writes as in `is_legacy_noop` (keep Undo of existing entries) and reduce the page to "Open NVIDIA Control Panel" |
| 28 | `gputweaks.rs` catalog `impact` | Low | Displayed effect claims without measurement: `nv_dynamic_pstate_off` "Smoother frametimes, eliminates clock dips", `amd_sclk_sleep_off` "fewer latency spikes", `amd_dma_power_off` "Reduces latency … noticeable". The GPU page shows `impact` verbatim. | open: same decision as #12 |
| 29 | `lib.rs:cmd_nv_set_setting`, GPU tweaks | Info | Not yet routed through the broker (phase 3 is incomplete). Unelevated, HKLM writes fail with a clear message and are rolled back. | known (broker phase 3) |
| 30 | `regclean.rs` scanners | Medium | **Valid entries could be offered as "orphaned".** Only the Uninstall scanner skipped paths on drives that are not present. Shell Extensions, SharedDLLs and App Paths judged unqualified names (`shell32.dll`) by `Path::exists` relative to the working directory, so they were always "missing". MUI and Run keys also flagged entries on an unplugged external drive. UNC paths made the elevated scan open network connections. Deletion still needs a selection and is backed up, but the list was wrong. | fixed: one rule `is_missing` for every scanner (local drive-letter path, drive present, file gone); test `only_local_qualified_paths_can_be_orphans` |
| 31 | `regclean.rs:clean/restore` | Info | Checked: deletion only acts on entries that match a fresh scan exactly; every key is exported before it is deleted; restore only imports manifests and `.reg` files under the protected `regclean` folder (canonicalized). | ok |
