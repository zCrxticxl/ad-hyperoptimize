# Design: unelevated UI with an elevated broker

Status: **decided; phase 1 done, phases 2–5 open.** The owner answered the open questions (section 8). Implementation starts after the Windows validation of the current branch.

## 1. Current state

- `src-tauri/app.manifest` declares `requireAdministrator`, and `lib.rs::ensure_admin` relaunches with `Start-Process -Verb RunAs` (release builds only). The whole process runs elevated, including WebView2 and the React UI.
- `lib.rs` registers **163 Tauri commands**. Every one of them can be called from the renderer with the admin token.

| Module | Commands | Module | Commands |
|---|---|---|---|
| perftweaks | 14 | powerplan | 5 |
| security (Defender, firewall) | 11 | safety (journal, restore points) | 4 |
| gameboost | 9 | frametime | 4 |
| diskanalyzer | 8 | uninstaller | 4 |
| procmgr | 7 | regclean | 4 |
| drivers | 7 | ctxmenu | 4 |
| cache | 6 | autoopt, optimizer, profiles, privacy, services, bootopt, hwmonitor, softwareinstaller | 3 each |
| debloater, gputweaks | 6 each | monitor, bench, startup, schedtasks | 2 each |
| scan, tweaks, latency, updates, gameprofile | 5 each | ps, cleanup, healthcheck, hwprofile | 1 each |

### Existing mitigations, which stay in place

- **CSP:** `default-src 'self'`; IPC only through `ipc:`. There is no remote content and no `dangerouslySetInnerHTML` with backend data.
- **Inputs are validated as untrusted**, for example:
  - the uninstaller only runs uninstall strings that are registered under HKLM;
  - `cmd_open_path` only opens `https://` URLs and existing local paths, and executes nothing;
  - journal and state files live in an admin-only folder (`statestore.rs`);
  - PowerShell arguments are quoted (`ps::quote_single`).

### Why that is not enough

**Any** code running in the renderer has all 163 privileged commands available, for example through:
- a vulnerability in a frontend dependency;
- a future feature that renders external text unsafely;
- a malicious update channel.

Microsoft also advises against running WebView2 elevated: file dialogs, drag & drop and every browser process then carry the admin token.

## 2. Goal

1. The UI process (Tauri + WebView2) runs as `asInvoker`, i.e. as a normal user without elevation.
2. Privileged work happens in a small **broker** with a **fixed, typed command list** (an allowlist). The renderer never reaches the broker directly; only the UI process's Rust code does, after its own validation.
3. Anything that needs no admin rights (reading, HKCU settings, the FPS measurement, ...) never touches the broker.
4. The UAC prompt only appears when the user starts something that needs admin rights.

## 3. Options

| | A: Windows service (LocalSystem) | **B: on-demand helper, same exe with `--broker` (recommended)** |
|---|---|---|
| Starts | permanently, set up by the installer | only on the first admin action, through UAC (`runas`) |
| Runs as | SYSTEM | the signed-in user with an elevated token |
| HKCU tweaks | **wrong hive** (SYSTEM profile); needs impersonation | correct, same user profile |
| Attack surface when idle | permanently present | none (the process ends with the session) |
| Installer | must install and remove the service | no change |
| UAC | never (this is itself a risk: every local user gets admin functions) | once per session |

**Recommendation: B.** It keeps the current UAC model ("this app makes admin changes, Windows asks"), has no permanent privileged service, and HKCU stays correct.

## 4. Design of option B

### Process model

```
UI process (asInvoker)            Broker (elevated, same exe --broker <nonce>)
 ├─ WebView2 / React                ├─ only privileged modules
 ├─ Tauri commands (validation)     ├─ BrokerRequest → handler (match, no strings executed)
 └─ BrokerClient ── named pipe ────►└─ journal/state in %ProgramData% (admin-only)
```

### Startup and authentication

The UI creates a random 256-bit nonce and starts `AD-HyperOptimize.exe --broker <pipe-name>` via `ShellExecuteEx` with `runas`. The nonce goes through an inheritable anonymous handle or, if that is not available, via stdin; it never appears on the command line.

1. **The broker creates the pipe:**
   - `\\.\pipe\adho-<random>`, created with `FILE_FLAG_FIRST_PIPE_INSTANCE`;
   - DACL: only the user's SID plus `NT AUTHORITY\SYSTEM`;
   - `PIPE_REJECT_REMOTE_CLIENTS`.
2. **The broker checks the client:**
   - `GetNamedPipeClientProcessId` must equal the parent process ID;
   - the image path must equal its own;
   - the first message must carry the nonce.
3. **The UI checks the server:** `GetNamedPipeServerProcessId` must equal the process ID it started.
4. **Lifetime:** The broker ends when the pipe closes or after 15 minutes idle. The next admin action triggers UAC again.

### Protocol

- One request per line, as serde JSON of a Rust enum, e.g.:
  - `BrokerRequest::ApplyTweak { id }`
  - `RevertTweak { id }`
  - `SetService { id }`
  - `Bcd { id, apply }`
  - `PowerPlanSet { guid }`
  - …
- **Only ids, GUIDs, enums and numbers. No free strings that are executed or used as paths.**
- Where a path is unavoidable (Cleanup, Disk organizer), the broker checks it against the scan result it computed itself, just as the uninstaller does today with the registry.
- Responses: `Result<serde_json::Value, String>`; errors are passed through unchanged.
- **Versioning:** `hello { protocol: 1 }`. A broker with a different version is rejected.

### Splitting the 163 commands

Phase 1 produces a per-command table and a test that fails for any unclassified command.

| Class | Runs in | Examples |
|---|---|---|
| R: read only | UI | scans, lists, monitor, hwprofile, health scan (mostly read-only; parts need admin reads: Defender state, BCD → R-admin via broker) |
| U: user scope | UI | HKCU tweaks, privacy items under HKCU, FPS measurement, reports, cache |
| P: privileged | broker | HKLM tweaks, services, bcdedit, powercfg, Defender/firewall, drivers, system cleanup, HKLM uninstall, restore points, regclean HKLM, ctxmenu HKLM, scheduled tasks |
| X: drop | – | commands without UI use (e.g. the old `autoopt` handlers without a page) |

### State and journal

- `state_dir` (`%ProgramData%\AD HyperOptimize`, admin-only) stays admin-only. Only the broker writes there.
- The UI reads the journal and history through broker requests `Journal::List` / `History`.
- Before the broker has started, the UI shows only the read-only state ("Undo history: unlock to view").

Alternative (an open question for the owner): users get read access to the journal. It contains no secrets, but it does reveal which tweaks were applied.

### Mixed operations

- One user action can touch both HKCU and HKLM. Example: `telemetry_minimal` writes an HKLM policy and changes a service; a tweak with HKCU **and** HKLM actions also exists.
- **Rule:** a tweak runs either entirely in the UI or entirely in the broker. Mixed tweaks go to the broker. Since it runs as the same user, HKCU is still correct there.
- One journal entry per tweak, as today.

## 5. Migration plan (separate, reviewable PRs)

1. **Inventory:**
   - `commands.rs`: classification R / U / P / X for every command;
   - a test that compares the list against `generate_handler!`;
   - no change in behaviour.
2. **Broker skeleton:**
   - `--broker` mode, pipe with DACL, PID/nonce checks, `hello`/`ping`;
   - behind the feature flag `broker`; release builds unchanged;
   - unit tests for the protocol and request validation; the PID/DACL checks can only be tested on Windows.
3. **Move the privileged modules** one at a time: tweaks (HKLM part), services, bootopt, powerplan, security, drivers, cleanup, uninstaller, regclean, ctxmenu, schedtasks, restore points.
   - The UI command stays and calls `BrokerClient` internally, so the frontend does not change.
4. **Switch:**
   - `app.manifest` → `asInvoker`; remove `ensure_admin`;
   - `cmd_is_admin` becomes "broker running";
   - the UI shows an "Unlock admin actions" button instead of "Restart as administrator".
5. **Remove the P commands from the UI process:** a CI test checks that the UI binary does not call any privileged module directly, e.g. by keeping the privileged modules in a separate crate that only the broker links.

Each phase on its own branch, with the full gate (fmt, check, test, clippy) on Windows.

## 6. Tests

| Area | Automated (CI, windows-latest) | Manual (Windows VM) |
|---|---|---|
| Classification is complete | test against `generate_handler!` | – |
| Protocol, request validation (ids, GUIDs, paths against scan) | unit tests | – |
| Pipe DACL, rejection of a foreign client process | integration test that starts a second process | – |
| UAC flow, broker end, renewed UAC | – | VM, standard user and admin user |
| HKCU tweaks without broker, HKLM with broker | – | VM, apply/undo through Optimize My PC |
| Renderer cannot reach privileged functions without the broker | test: P commands are not in the UI handler | – |

## 7. Open decisions for the owner

1. **Option B (on-demand helper)** instead of a Windows service? Recommended: B.
2. **UAC:** once per session (recommended) or for every admin action?
3. **Journal:** read via the broker only (recommended), or readable by users?
4. **Dropping unused commands (class X):** may they go, e.g. the old `autoopt` handlers?
5. **Order:** first the security switch (phases 1–5), or the Windows validation of the current branch first? Recommended: Windows validation first, because phases 3–5 touch every privileged path and need a tested baseline.

## 8. Decisions (owner, 2026-10-23)

| Question | Decision |
|---|---|
| 1. Service or on-demand helper | No preference; the recommendation applies: **option B**, on-demand helper `--broker`. |
| 2. UAC | **Once per session.** |
| 3. Journal | **Readable without admin rights.** Users get read access to the journal and history in the state folder; writing stays admin-only. The UI shows the undo history without starting the broker. |
| 4. Unused commands | **Remove them when unused** (class X; first candidates: the `autoopt` handlers without a page). Each removal is checked against `src/api.ts` and the pages. |
| 5. Order | **Windows validation first**, then phases 1–5. |

## 9. Progress

### Phase 1: done (`a891773`, inventory commit after it)

- **Class X removed:** 8 commands no page calls:
  - `cmd_autoopt_scan`, `cmd_autoopt_score`, `cmd_autoopt_apply` and the `autoopt` module
  - `cmd_gameboost_boost_process`: set any PID to High without validation
  - `cmd_gameboost_gpu_perf`
  - `cmd_net_tweak_all_gaming`
  - `cmd_hw_temps`, `cmd_hw_smart`

  163 → 155 commands.
- **Inventory:** `src-tauri/src/commands.rs` lists all 155 commands:

  | Class | Count | Runs in |
  |---|---|---|
  | Read | 56 | UI |
  | User | 14 | UI |
  | ReadAdmin | 9 | broker |
  | Privileged | 76 | broker |

  Entries marked "verify" still need a check as a standard user: `cmd_health_scan`, `cmd_scan_driver_updates`, `cmd_powerplan_set`, `cmd_timer_set`, `cmd_hw_full`.
- **Tests:**
  - `every_registered_command_is_classified_exactly_once`, compared against `generate_handler!`
  - `every_defined_command_is_registered`
  - `mutating_commands_are_not_reads`
- **Behaviour change:** none apart from the removed commands, which nothing called.

### Next: phase 2 (broker skeleton)

- `--broker` mode
- named pipe with a DACL for the user's SID
- PID and nonce checks; `hello` / `ping`
- behind a `broker` cargo feature, so release builds stay unchanged

