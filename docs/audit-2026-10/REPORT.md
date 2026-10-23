# AD HyperOptimize: Audit, Repair and UX Overhaul (2026-10)

- **Branch:** `audit/2026-10-overhaul`, based on `main` @ `6ab9155` (v1.6.4)
- **Not merged, not released.** Nothing in this branch was applied to a live system.
- **Companion document:** [optimization-audit.md](optimization-audit.md) has the per-optimization classification table.

---

## A. Executive summary

### What was wrong

The app had broad functional depth, but several of its safety guarantees did not
hold in practice.

**Undo and status**
- Three "exact undo" commands never ran, yet were marked as reverted.
- Undo without a journal entry ran hardcoded "defaults", for example
  disabling RSS or forcing the Balanced power plan.
- Applying a tweak twice overwrote the captured original value.
- Every registry tweak whose key did not exist yet failed on non-English Windows.

**Security (the app runs fully elevated)**
- PowerShell string escaping could be bypassed with Unicode smart quotes.
  This was verified by actually running the breakout.
- A wildcard in a user-controlled app name could recursively delete
  `HKLM\Software\Microsoft`.
- The timer-resolution SYSTEM task executed a script from a user-writable
  folder: a local privilege escalation.
- `open path` would launch any executable with admin rights.
- Elevated temp cleanup was vulnerable to a junction race.

**Gaming**
- Quick Boost matched processes by substring. That killed the app's own
  WebView2 window and Ubisoft Connect, which Rainbow Six Siege needs.
- Quick Boost also halved the usable CPU cores.
- A "GPU max performance" switch actually turned HAGS **off**.

**Honesty**
- Roughly 15 catalog tweaks are Windows defaults, myths, or no longer read by
  Windows. Several were promoted as performance gains.
- Two share-of-catalog numbers were presented as "/100 scores".
- One unsourced "+5–15% FPS" claim was shown to users.

**UX**
- 37 tools sat behind a three-level launcher (home → category → tool).
- The undo hub was hidden in Beginner mode, while the "revertible" pill linked
  to it.
- A "revertible" claim appeared on tools whose actions are irreversible.
- One failed backend call could blank the whole app.

### What was improved

- Every issue listed above is fixed. Each fix has unit tests where the logic is
  testable without changing the system.
- The navigation is now a persistent sidebar of 8 task-oriented sections, each
  with its tools as tabs. All 37 tools and all 156 backend commands are kept.
- The catalog text is now truthful.
- No-gain tweaks were demoted rather than deleted.
- Retired or deprecated actions keep their undo path.
- Rust test count went from 35 to 110 (plus 3 opt-in system tests).

### What remains

The main remaining risks are structural:

- The whole WebView runs elevated.
- The undo journal lives in user-writable `%APPDATA%`.
- Nothing has been validated on real Windows installs.

See section I.

---

## B. Feature inventory

There are 37 tools; their location in the new navigation is listed in E. The
per-optimization verification table is in
[optimization-audit.md](optimization-audit.md).

**How to read "Works?":**
- "code-verified" means the UI → `api.ts` → command → Windows operation chain
  was traced and is consistent.
- "fixed" means it was broken and has been repaired.
- No tool was exercised against a live system. Runtime verification is pending
  for all of them (section G).

| Tool | Backend module | Admin | Works? | Changes this pass |
|---|---|---|---|---|
| Dashboard / analysis | analysis.rs | no | code-verified | – |
| Health check (SFC/DISM) | healthcheck.rs | yes | fixed | SFC verdict was always "unknown" |
| Auto-Optimizer | autoopt.rs | partly | fixed | crashed the UI when the scan failed; score relabelled |
| Optimize (tweak catalog) | tweaks.rs | per tweak | fixed | undo, locale, double-apply, status, catalog text |
| Game Booster | gameboost.rs | partly | fixed | kill list, affinity, HAGS, state overwrite |
| Power plans | powerplan.rs | no | fixed | Ultimate duplicated on non-EN/DE systems |
| GPU tweaks | gputweaks.rs | yes | fixed | legacy no-op keys hidden, flags corrected |
| Perf tweaks (timer/MSI/NIC/pagefile/RAM) | perftweaks.rs | yes | fixed | LPE, MSI device guard, honest results |
| Latency | latency.rs | partly | code-verified | description corrected |
| NVIDIA control panel | gputweaks.rs | yes | code-verified | unknown value no longer guessed |
| Boot optimizer | bootopt.rs | yes | fixed | captured BCD values, retired unsafe items |
| Profiles | profiles.rs | yes | fixed | failures reported, hardware-gated items |
| Game profiles | gameprofile.rs, gamedb.rs | no | fixed | Ultimate resolution, real previous plan |
| Cleanup | cleanup.rs | partly | fixed | handle-based safe delete |
| Uninstaller | uninstaller.rs | yes | fixed | leftover wildcard deletion, backups |
| Debloater | debloater.rs | yes | fixed | write-ahead undo, partial removal, risky defaults |
| Disk analyzer | diskanalyzer.rs | partly | fixed | Organize crash, path protection, duplicate hashing |
| Registry cleaner | regclean.rs | yes | fixed | renderer-trusted deletes, false positives |
| Context menu | ctxmenu.rs | yes | code-verified | – |
| Startup | startup.rs | partly | code-verified | – |
| Services | services.rs | yes | fixed | deny-list, delayed start, admin check |
| Scheduled tasks | schedtasks.rs | yes | fixed | injection validation |
| Processes | procmgr.rs | partly | fixed | IFEO key no longer wiped |
| Drivers | drivers.rs | yes | fixed | winget pinning |
| Software installer | softwareinstaller.rs | yes | fixed | exit-code success, no concurrent runs |
| Privacy center | privacy.rs | partly | fixed | telemetry status, journal values, admin gating |
| Security | security.rs | yes | fixed | Defender confirm/undo/status, Quick Scan wiring, driver Enable |
| Hardware / Monitor / Sensors | hwprofile.rs, monitor.rs, hwmonitor.rs | no | fixed | monitor start/stop race, error states |
| Benchmark | bench.rs | no | fixed | overlapping runs prevented |
| PC configurator | lib/pcAdvisor.ts | no | fixed | estimates labelled |
| Restore points | safety.rs | yes | code-verified | – |
| Reports / undo history | safety.rs, report.rs | partly | fixed | Beginner access, HTML escaping, single undo |
| Updates (app + winget + drivers) | updates.rs | partly | fixed | error handling, `--source winget` |
| Settings, Changelog | – | no | code-verified | – |

---

## C. Bugs found and fixed

Every row below was fixed on the branch; the Commit column names the commit
with the fix.

### Gate result for all rows

- **Gate:** the full CI gate passed on the final tree:
  - `npm run build`
  - `cargo fmt --check`
  - `cargo check`
  - `cargo test` (110 passed, 3 opt-in ignored)
  - `cargo clippy -D warnings`
- **Windows runtime:** not run, which is why the table has no "Windows test"
  column. See G.

| # | Sev | Feature | Root cause | Commit |
|---|---|---|---|---|
| 1 | Critical | Undo of USB suspend, core parking and Nagle | `powershell -Command "<script>"` was evaluated as a string literal and echoed with exit 0 | 86d522f |
| 2 | Critical | Quick Boost | Substring kill list (`*msedge*` matched msedgewebview2, `*upc*` matched Ubisoft Connect) | d7a821e |
| 3 | High | All PowerShell interpolation | Unicode quotes U+2018–U+201E were not escaped or rejected | 057f49b |
| 4 | High | Uninstaller leftovers | `-Path` wildcard expansion plus a user-controlled DisplayName led to recursive HKLM delete without backup | 057f49b |
| 5 | High | Timer resolution | SYSTEM task ran `%ProgramData%\ADHyperOptimize\timerres.ps1`, a path a standard user can pre-create | 92ce487 |
| 6 | High | Temp cleanup | Gap between the reparse-point check and the path-based delete (junction race) | 057f49b |
| 7 | High | Tweaks on non-English Windows | Localized `reg.exe` stderr was parsed to detect a missing key | 92ce487 |
| 8 | High | Undo without journal | Hardcoded catalog reverts were run blind | 92ce487 |
| 9 | High | Force-revert | `.reg` import merges values; success was never verified | 92ce487 |
| 10 | High | `kill_background` | Any PID from the renderer was killed elevated | d7a821e |
| 11 | High | `set_gpu_max_perf` | Wrote HwSchMode=1, which disables HAGS | d7a821e |
| 12 | High | Defender toggles | No confirmation, no undo, cloud status read from the wrong cmdlet | d7a821e, 455bfcb |
| 13 | High | Boot optimizer | No captured originals; numproc always Ok; localized Yes/No | d7a821e |
| 14 | High | AutoOptimizer | `data.total` on null after a failed scan blanked the whole app | b021f85 |
| 15 | High | Disk Organize | Result object rendered as a React child, crashing after files were moved | 455bfcb |
| 16 | High | `cmd_open_path` | `Start-Process` on any local or UNC path | 057f49b |
| 17 | Medium | Double apply | Second apply re-captured the tweaked value as the "original" | 92ce487 |
| 18 | Medium | Nagle revert | `{GUID}` keys failed `is_guid`, so the revert was never built | 86d522f |
| 19 | Medium | Timer reset | Literal TAB byte in the cleanup path | 92ce487 |
| 20 | Medium | MSI mode | Any `Enum\` device accepted, including the boot storage controller | 92ce487 |
| 21 | Medium | Services | No deny-list; `AutomaticDelayedStart` invalid in PowerShell 5.1 | d7a821e |
| 22 | Medium | Process manager | `delete_subkey` removed the whole IFEO key, including mitigations | d7a821e |
| 23 | Medium | Privacy | `$null -le 1` reported telemetry as applied | d7a821e |
| 24 | Medium | Debloater | State written after apply, so a partial apply lost its undo; partial removal reported as success | d7a821e |
| 25 | Medium | Registry cleaner | Trusted renderer entries; env/exe parsing false positives | 057f49b |
| 26 | Medium | Disk analyzer | `is_protected` bypass via 8.3 names and stream syntax | 057f49b |
| 27 | Medium | Game profiles | Template GUID never installed; previous plan lost | d7a821e |
| 28 | Medium | Legacy game boost | State overwrite; `$null.ToString()` threw | d7a821e |
| 29 | Medium | Monitor | start/stop race left two emit loops | 057f49b |
| 30 | Medium | Security page | Cached state shown after changes; Quick Scan did not scan; startup items sent to the task API; no driver Enable | 455bfcb |
| 31 | Medium | Power plans | Ultimate duplicated on non-EN/DE locales; rename failure ignored | 92ce487 |
| 32 | Medium | Status pill | "Revertible" shown on irreversible tools | 455bfcb |
| 33 | Medium | Beginner mode | Undo hub unreachable | b021f85 |
| 34 | Low | SFC | Verdict parsed from filtered CBS.log, so it was always unknown | d7a821e |
| 35 | Low | Report HTML | Quotes and summary not escaped | 057f49b |
| 36 | Low | Missing services | Shown as "applied" | 92ce487 |
| 37 | Low | Debloater | 24 i18n keys missing, so raw key names were shown | 455bfcb |
| 38 | Low | Many pages | No catch → endless spinners; double submits | 455bfcb |
| 48 | High | Privacy center (found in live use) | A statement-list check inside `[bool]( )` made the whole status scan unparsable | 6d0a4a1 |
| 49 | High | Undo journal / backups / state files | Stored in user-writable `%APPDATA%` but executed or imported elevated on undo (planted revert = silent UAC bypass) | dc049fa |
| 39 | Medium | Services (found in live use) | A service whose own ACL refuses admins (ChatGPT/Codex sandbox) showed a raw PowerShell error and "rollback incomplete", and left an open undo entry although nothing changed | 3d33ceb |
| 40 | High | Security (found in review) | Cloud Protection level (0/1/2) read as a bool, so it always showed OFF | 0987730 |
| 41 | High | GPU tweaks (found in review) | After a driver reinstall, Apply reported success without writing and Undo was hidden | 0987730 |
| 42 | Medium | Tweak engine (found in review) | Same no-op-as-success via Auto-Optimizer and Profiles | 0987730 |
| 43 | Medium | Debloater (found in review) | Partial UWP removal skipped the provisioned-package removal | 0987730 |
| 44 | Medium | Services (found in review) | Delayed start not shown in the list; delayed → Automatic failed verification | 0987730 |
| 45 | Medium | Game profiles (found in review) | A stale previous-plan record could override a plan the user picked later | 0987730 |
| 46 | Low | Disk analyzer (found in review) | Legitimate `~1` names were refused | 0987730 |
| 47 | Low | Navigation (found in review) | Deep link into the already-open tool did nothing | 0987730 |

---

## D. Windows optimization audit

See [optimization-audit.md](optimization-audit.md). It covers about 90
optimizations, each with:
- what it changes
- rollback type
- admin and reboot needs
- classification and evidence

### Summary by class

**INEFFECTIVE or OBSOLETE (about 20)**
- Examples: Nagle/TCPNoDelay, IRPStackSize, QoS NonBestEffortLimit,
  Win32PrioritySeparation=0x26 (equals the Windows default), legacy NVIDIA
  registry values, Cortana.
- Demoted from the "may help performance" set, with truthful text.
- Kept available, with undo.

**UNSAFE**
- Retired from the active lists, with undo kept:
  - BCD nx OptOut
  - Quick Boost affinity halving
  - the HAGS-off switch
- Kept behind explicit confirmation:
  - pagefile disable
  - network reset
  - AMD compute preemption off

**Security weakening**
- The default flows contain no VBS/HVCI, mitigation, test-signing or DSE changes.
- Defender off requires confirmation, is hidden in Beginner mode, and is journaled.

**One-click preset**
- The preset is unchanged: Game DVR off, Game Mode on. Both are VERIFIED and low risk.

---

## E. UI/UX changes

### Before

The app opened on a launcher with 8 featured cards. Six category cards each led
to a tool grid, and only then to the tool. That meant three levels of
navigation, with breadcrumbs as the only orientation. Overlapping tools sat in
unrelated categories.

### After

A persistent left sidebar holds 8 sections. Each section shows its tools as tabs:

| Section | Tools (tabs) |
|---|---|
| Dashboard | Overview (health score, findings, quick access, recents), Health check |
| Optimize | Auto-Optimize (recommended), Tweaks, Game Booster, Power plan, GPU tweaks, Advanced perf tweaks, Latency*, NVIDIA*, Boot* |
| Profiles | Profiles*, Game profiles* |
| Clean up | Cleanup, Uninstaller, Debloater, Disk analyzer*, Registry cleaner*, Context menu* |
| System | Startup, Services, Scheduled tasks, Processes*, Drivers*, Software installer* |
| Privacy & Security | Privacy center, Security |
| Monitor & Benchmark | Hardware, Monitor, Sensors*, Benchmark, PC configurator* |
| Recovery & Settings | Restore points, Reports & undo history, Updates, Settings, Changelog |

\* Expert mode only.

### What stays the same

- Global search, including feature-level deep links (e.g. "HAGS"), is unchanged.
- Beginner mode hides expert tabs and shows how many are hidden.

### Supporting changes

- Pills: "Changes are revertible" appears only on revertible tools. A warning
  pill pointing to restore points appears on irreversible ones.
- Crashes: a crash in one tool shows an inline error with Retry; the shell stays
  usable.
- Confirmations are now required for:
  - Defender off
  - bcdedit changes
  - bulk privacy/debloat apply
  - disabling a service
  - disk organize
  - MSI mode and pagefile disable
  - undo from history
- Narrow windows: the sidebar collapses to icons below 900 px.

---

## F. Code quality

### Single ownership

- HAGS is owned only by the `hags_on` catalog tweak.
- Scheduled-task toggles from Security route through `schedtasks::toggle`,
  which carries validation and the journal.
- Delayed-start services use one `sc.exe` path, shared by tweaks and Services.

### Shared helpers

| Helper | Purpose |
|---|---|
| `ps::quote_single` / `ps::escape_single` | PowerShell quoting |
| `ps::winget_exe` | trusted winget path |
| `regclean::RegBackup` / `delete_key_tree` | backup-then-delete, shared with the uninstaller |
| `ToolErrorBoundary` | contains a crashing tool |
| `TKey` | i18n key type; missing keys now fail `tsc` |

### Result model

Results now distinguish more outcomes:

- tweak status: `unsupported`, `already_applied`
- Quick Boost and profiles: `skipped` / `hwSkipped`
- debloat: partial removal
- profiles: `restorePointFailed`
- force-revert: verified result

No unified `{attempted, succeeded, failed, skipped, unsupported, unverified}`
result type was introduced across all modules; see I.

### Not done

There was no wholesale rewrite: modules keep their structure. The large
duplicate catalogs (privacy vs debloater vs tweaks) still exist; see I.

---

## G. Test results

All commands below were actually executed on the final tree (Windows 11 Pro
26200, Node 24.19, cargo 1.98.1).

| Command | Result |
|---|---|
| `npm run build` | **Passed** (only warning: one chunk > 500 kB) |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | **Passed** |
| `cargo check --manifest-path src-tauri/Cargo.toml --locked` | **Passed** |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked` | **Passed: 110 tests, 0 failed** (baseline before this work: 35) |
| `cargo test --locked -- --ignored read_only_scans` (opt-in, real system, read-only) | **Passed**: 19 read-only scans, no PowerShell parser errors |
| `cargo test --locked -- --ignored secure_dir_end_to_end` (opt-in, **run elevated** after the user approved UAC) | **Passed**: protected folder created, hardened, reused; planted weak folder moved aside; scratch folders removed |
| Migration dry run against a copy of the developer's real journal | 52 entries: 49 trusted, 3 untrusted (legacy privacy commands that were never runnable) |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --locked -- -D warnings` | **Passed** |
| `npx tsc --noEmit -p .` | **Passed** |

### Real side-effect tests (Windows, run in the test suite)

These touch nothing outside a temp directory:

- `run_cmdline` executes quoted and unquoted `-Command` scripts and returns an
  error on a non-zero exit. This is the regression test for bug 1.
- Cleanup refuses to delete through a real junction created in temp.

### Manual UI checks

Performed in the browser pane against the Vite dev server, without the Tauri
backend:

- The sidebar and tabs render.
- Switching to Expert shows the expert tabs.
- A failing backend call shows an inline error instead of a blank screen.

### Not executed (requires a Windows test machine or VM, with approval)

- Applying, verifying and reverting any tweak on a live system: registry,
  services, powercfg, bcdedit, Defender, debloat, uninstall, cleanup.
- Repeated apply/revert (idempotency), locale variants (German/other UI
  language), Windows 10 22H2 vs 11 24H2/25H2.
- Elevation and relaunch flow, installer (NSIS/MSI) build, updater.
- Performance benchmarks.

**No FPS or latency measurements were taken.** None are claimed.

A credible gaming validation needs:
1. A fixed scene or replay; for R6 Siege, the in-game benchmark.
2. CapFrameX or PresentMon capture.
3. At least 3 runs per state.
4. Reporting avg FPS, 1% lows and frametime variance, before and after each
   change, with background load controlled.

---

## H. Files modified

Branch commits after the baseline snapshot (`0317f4f`):

| Commit | Files |
|---|---|
| `b021f85` feat(ui): sidebar shell | `src/App.tsx`, `src/components/ui.tsx`, `src/pages/AutoOptimizer.tsx`, `src/i18n.tsx`, `src/styles.css` |
| `86d522f` fix(tweaks): undo quoting | `src-tauri/src/tweaks.rs` |
| `455bfcb` fix(ui): frontend audit | `src/App.tsx`, `src/i18n.tsx`, `src/styles.css`, `src/hooks/useFeatureFocus.ts`, `src/locales/*.ts` (7), 26 files in `src/pages/` |
| `057f49b` fix(security) | `src-tauri/src/{ps,uninstaller,diskanalyzer,regclean,report,cleanup,drivers,updates,softwareinstaller,monitor,lib}.rs`, `.github/workflows/{release,security}.yml` |
| `92ce487` fix(tweaks) catalog | `src-tauri/src/{tweaks,perftweaks,gputweaks,powerplan,latency}.rs` |
| `d7a821e` fix(gaming, services, security) | `src-tauri/src/{gameboost,gameprofile,gamedb,security,services,bootopt,privacy,procmgr,debloater,healthcheck,profiles,schedtasks,safety,ps}.rs` |

New files:
- `docs/audit-2026-10/REPORT.md`
- `docs/audit-2026-10/optimization-audit.md`
- `.gitignore` gains an entry for `.omc/`

Later commits:
- `6d0a4a1` fix(privacy): status scan parse error (`privacy.rs`)
- `5d746d3` test: read-only scan smoke test (`lib.rs`)
- `dc049fa` feat(security): admin-only state folder (new `statestore.rs`; `safety`, `tweaks`, `perftweaks`, `debloater`, `gameboost`, `gameprofile`, `powerplan`, `regclean`, `security`, `Reports.tsx`, `i18n.tsx`)
- `3d33ceb` fix(services): access-denied handling (`services.rs`, `tweaks.rs`)
- `0987730` fix: independent review findings (`security`, `gputweaks`,
  `tweaks`, `debloater`, `services`, `gameprofile`, `diskanalyzer`, `App.tsx`,
  `AutoOptimizer.tsx`, `GpuTweaks.tsx`, `Security.tsx`, `Debloater.tsx`,
  `i18n.tsx`)

### Independent review

After implementation, a separate reviewer audited the whole diff
(`0317f4f..HEAD`). It found 9 issues: 2 high, 4 medium, 3 low. All are fixed
in `0987730` (bugs 40–47).

Three open questions need manual testing:
- Does winget work when started from its package path under an elevated token?
- Does `explorer /select,` handle paths with spaces correctly?
- How often does `Remove-AppxPackage -AllUsers` partially fail?

No files were removed.

---

## I. Remaining issues

| Sev | Issue | Next step |
|---|---|---|
| High | The whole WebView2 renderer runs elevated; all 156 commands are reachable from it | Split into an unelevated UI and a small elevated broker with an allow-listed command surface |
| High | No runtime validation on real Windows installs | Run the manual matrix in G on disposable Windows 10/11 VMs (EN and DE) |
| Medium | Three overlapping catalogs (privacy.rs, debloater.rs, tweaks.rs) write the same settings with separate undo stores | Make tweaks.rs the single source; have the other modules reference catalog ids |
| Medium | `app.manifest` (`requireAdministrator`) is not embedded; elevation happens by self-relaunch | Embed it or delete it, and document the elevation model |
| Medium | GitHub Actions pinned to tags, not commit SHAs, in a job holding the signing key | Pin to SHAs; Dependabot is already configured to update them |
| Medium | `time = "=0.3.41"` is affected by RUSTSEC-2026-0009; other transitive advisories (quick-xml, crossbeam-epoch) | Bump and run the new `cargo audit` CI job |
| Medium | Security driver "Enable" only knows devices disabled from this page (stored locally) | Backend should report device enabled state |
| Medium | `nv_set_setting` (NVIDIA control panel page) writes registry values modern drivers ignore | Replace with NVAPI/DRS or retire with undo only |
| Low | Some English strings remain in GpuTweaks and BootOptimizer logs; locale overlays for corrected tweaks fall back to English | Translate |
| Low | No unified operation-result type across modules | Introduce `OpResult {status: attempted\|succeeded\|failed\|skipped\|unsupported\|unverified}` when consolidating catalogs |
| Low | `update_apps` now uses `--source winget` only, so msstore-sourced updates are excluded | Offer msstore explicitly if needed |
| Medium | winget launched by absolute package path may fail to load its runtime (0xC0000135) under some elevated tokens; the fallback only covers resolution failure | Test on a clean machine; fall back to the alias on that exit code |

---

## J. Readiness assessment

| Area | Assessment |
|---|---|
| Functional correctness | Considerably improved at the code level. All confirmed bugs are fixed and unit-tested where possible. **Not runtime-verified**, so treat it as a release candidate that needs VM testing. |
| Windows compatibility | Locale handling is fixed for backups, BCD, power plans and SFC. Windows 10 vs 11 behavior is documented per tweak but not executed. |
| Security | The worst exploitable paths are closed: smart-quote injection, wildcard HKLM delete, timer LPE, junction race, elevated launch. Privileged state now lives in an admin-only folder, and migrated legacy commands are validated. The elevated renderer remains HIGH. |
| Stability | Crash paths in the UI are contained; failures surface instead of reporting false success. |
| Maintainability | Better (shared helpers, typed i18n keys, 100 tests). Catalog duplication remains the largest debt. |
| User experience | Navigation is flatter and consistent, with honest labels and confirmations where they matter. Capability is unchanged. |
| Release readiness | **Not ready to release without the VM validation pass in G.** It is ready for review and for that test pass. Do not merge to `main` or publish before both. |

---

## K. Privileged state hardening (follow-up)

**Problem.** Undo executed or imported content from `%APPDATA%\PCOptSuite`:
- journal revert commands
- `.reg` backups
- debloater and game-booster state

Any unelevated process of the same user can write that folder, so a planted
revert command became a silent UAC bypass on the next Undo.

**What changed in `dc049fa`.**

*Location.* Elevated sessions use `%ProgramData%\ADHyperOptimize`. The path is
read from HKLM rather than from the environment, because an elevated process
inherits the user's overridable environment.

*Permissions.* The folder is owned by Administrators with a protected DACL:
- SYSTEM and Administrators: full control
- Authenticated Users: read

*Existing folders.* A folder that is not already strict is moved aside and
recreated, because its content may be planted. This applies to the folder the
old timer feature left behind. A freshly created folder must still be empty
after hardening.

*Failure.* If the protected folder can't be established, privileged operations
fail. The app never falls back to a writable location.

*Migration.* Data from the legacy folder is migrated once and validated:
- Registry and service steps are re-validated.
- Revert commands are kept only if they match a shape the app generates.
- The timer reset command is re-derived from code.
- Debloater state is accepted only for targets the catalog declares for that
  tweak.
- Anything else marks the entry *untrusted*. Its commands are never run
  automatically; the Undo error shows them for manual review, and the history
  shows an "unverified" badge.

*Restore scope.* The registry cleaner restores only backups from the protected
folder.

*Unelevated sessions* keep using `%APPDATA%`. Nothing recorded there can run
with more rights than the session that recorded it.

**Side effect for existing installs.** The old timer feature registered a
SYSTEM task pointing at `%ProgramData%\ADHyperOptimize	imerres.ps1`. Once the
weak folder is moved aside, that task finds no script. Re-apply the timer
tweak to register the new, file-less task.

---

## L. System Health Check & verified optimization engine (follow-up)

The details are in [optimization-engine.md](optimization-engine.md). That document has the per-tweak table for all 55 catalog tweaks (previous status, evidence, changes, new classification, verification on the developer PC) and the test coverage.

### What was added
- **Evidence registry** for every tweak, in `optregistry.rs`.
- **Real system detection** and a diagnosis for each tweak, in `optimizer.rs`.
- **System Health Check page** under Dashboard.
- **Optimize My PC:** preview, approval, apply, and a read-back verification of every change.
- **New Optimize view.** By default it shows only what fits this PC; Advanced keeps everything else, classified.

### Detection fixes found on the real system
- A missing Game Mode value was read as "off".
- Hibernation state was invisible to unelevated `Test-Path`.
- Power plan, hibernation, TCP auto-tuning and RSS now report their live state instead of only what the history recorded.
- The visual-effects tweak only changed a dialog option; it now also writes the setting that applies the effects.

### Tests
- 126 automated tests pass.
- The opt-in read-only system scan passes, including the health check.
- The CI gate is green.

### Not done
- No optimization was applied to a live system.
- The apply and undo buttons need a test run on a Windows VM.
- Performance impact was not measured.
