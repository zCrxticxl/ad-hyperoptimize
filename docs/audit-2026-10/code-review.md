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
