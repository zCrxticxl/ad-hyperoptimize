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
| 12 | `tweaks.rs` catalog, `may_help_performance` | Low | Some `rationale`/`impact` texts claim effects that were never measured: `game_dvr_off` "Measurable frametime stability gain", `transparency_off` "measurable on iGPUs", `ntfs_8dot3_off` "Faster file creation", `fse_optimizations_off` "Lower input lag", `power_high_performance` "Lower … latency". `svc_maps_off` and `ntfs_8dot3_off` are in the beginner "performance potential" list. The texts appear in the global search. | open: reword to "may"/"no measured effect" (en + `deTweaks.ts`), or measure with the FPS page |
| 13 | `tweaks.rs:revert` force path | Info | Force-revert of a RegSet imports the newest `.reg` backup of the whole key. Other values in that key that changed since the backup are reset to the backup as well. By design (fallback without a journal); the value check afterwards only covers the tweak's own value. | accepted, documented |
| 14 | `statestore.rs:trusted_bcdedit/_defender/_sched_task/_service_start` | High | **The command check accepted valid shapes regardless of the entry.** `bcdedit /set {current} <any lowercase element> <alnum>` passed, so planted `testsigning on`, `nointegritychecks on` or `hypervisorlaunchtype off` were carried over as trusted. The same applied to `Set-MpPreference -DisableRealtimeMonitoring $true` / `-MAPSReporting 0`, `Disable-ScheduledTask` on any task, and `Start-Service` for any of the four privacy services. The next Undo ran them elevated. | fixed: BCD only on the object + element of the entry's `bcd:<id>` tweak, never test signing on or `nx AlwaysOff`. Defender only reverts that turn protection back on, under its own tweak id. Scheduled task only `Enable-ScheduledTask` of the task named in `task:<path><name>`. `Start-Service` only for the service of the entry's `privacy:<id>`. Legacy entries with an older shape (e.g. `bootmenupolicy` on `{current}`) are now untrusted: manual undo, fails closed. |
| 15 | `statestore.rs:migrate_debloater` | Medium | The file content was validated, then the *path* was copied again with `fs::copy`. The legacy folder is user-writable, so a symlink swapped in between check and copy made the elevated process copy any admin-only file into `%ProgramData%\ADHyperOptimize\debloater`, which Authenticated Users can read. | fixed: the validated bytes are written |
| 16 | `statestore.rs:ensure_secure_dir` | Info | Checked: a pre-created or weak folder is moved aside; a new folder is hardened and must still be empty and not a reparse point; `%ProgramData%` comes from HKLM, not from the environment. A user holding a handle can block the rename, which only stops the app (no escalation). | ok |
| 17 | `safety.rs:latest_backup_for` | Low | The prefix match `<root>_<path>_` also hit backups of subkeys (`Key\Sub` → `Key_Sub_<ts>.reg`). Force-revert could then import the wrong key; the value check afterwards caught only the case where the tweak value stayed. An unreadable directory entry (`?` in the loop) also discarded every backup. | fixed: exact `<prefix><YYYYMMDD-HHMMSS>.reg` match, unreadable entries are skipped; test `backups_match_their_own_key_only` |
| 18 | `safety.rs:backup_registry_key` | Info | Backup names have one-second resolution. A second backup of the same key within the same second overwrites the first (`/y`). Apply calls are serialized and take longer than that in practice. | accepted |
