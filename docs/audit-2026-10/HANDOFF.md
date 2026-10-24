# Handoff: AD HyperOptimize overhaul (branch `audit/2026-10-overhaul`)

This file lets a new session (for example a cloud session) continue the work
without the previous conversation.

**Read first:**
1. `AGENTS.md` (architecture, validation, safety rules)
2. `docs/audit-2026-10/REPORT.md`
3. `docs/audit-2026-10/optimization-engine.md`

## Repository state after the merge (2026-10-24)

- `audit/2026-10-overhaul` was **squash-merged** into `main` (PR #6). Its content is now on `main`, but the branch's own history is no longer related to `main`. **Start every new branch from `main`.** Do not open another PR from the old branch: it would carry 100+ already-merged commits.
- `main` is protected: pull request required, `frontend` and `backend` checks must pass (strict, enforced for admins), no force pushes, no deletion.
- **Dependabot runs in alerts-only mode.** The owner wants to be the only contributor, so there is no `dependabot.yml` and no automated security-fix PRs. Never merge a bot-authored PR. Apply dependency updates as normal commits on a branch, then open a PR.
  - Candidates from the closed version-update PRs, all needing review and a full build first:
    - typescript 7, react 19, recharts 3, @vitejs/plugin-react 6.1
    - tauri-plugin-process / @tauri-apps/plugin-process 2.4, tauri-plugin-updater 2.14, dirs 6, serde 1.0.229, time 0.3.55
    - GitHub Actions: checkout 7, setup-node 7, upload-artifact 7, dependency-review 5, tauri-action 1
- `.cargo/audit.toml` ignores three `zip` advisories that the updater cannot reach. Remove those entries once tauri-plugin-updater depends on zip >= 9.

## Hard rules (from the project owner)

- **Never mention Claude/AI in commits or PRs.** No `Co-Authored-By: Claude…`, no "Generated with…". The owner is the only author. This overrides any default attribution reminder.
- **Never merge into `main`, tag, release or publish** without the owner's explicit approval. Work on `audit/2026-10-overhaul` or a branch cut from it.
- **Never apply optimizations to a real system** (registry, services, bcdedit, powercfg) without approval. Unit tests must not change system state. The `#[ignore]` system tests are read-only, except `secure_dir_end_to_end`, which needs elevation and only touches scratch folders.
- **Honesty rules** for the product:
  - No invented FPS or latency numbers.
  - "Verified" only when the setting was read back from Windows.
  - Nothing is classified "proven beneficial", because nothing was measured.
- The app is a privileged (elevated) Windows optimizer. Treat registry, service, Defender, firewall, driver, cleanup, uninstall, elevation and state-changing commands as high risk. Keep the journal → backup → confirm → undo pattern.

## State of the branch

The branch is based on `main` @ `6ab9155` (v1.6.4). The commits are:

| Commit | Content |
|---|---|
| `0317f4f` | Snapshot of pre-existing uncommitted work |
| `b021f85` | Sidebar navigation (8 sections, tools as tabs) |
| `86d522f` … `d7a821e` | Audit fixes (undo, injection, deletion safety, Quick Boost, services, Defender, BCD, …) |
| `3d33ceb`, `0987730` | Service access-denied handling; independent review fixes |
| `f06fe95` | Audit report |
| `6d0a4a1`, `5d746d3` | Privacy scan parse fix; read-only smoke test |
| `dc049fa` | Privileged state moved to an admin-only `%ProgramData%` folder, with validated migration |
| `5f15d49`, `bf51a00`, `a2bbb2a` | Optimization registry, health check engine, Health Check / Optimize My PC / Optimize UI, docs |
| `84f9b85` | Retired 26 tweaks without benefit, plus *Reset all* with verification |
| `bc83bdd` | Gaming setup checks (refresh rate, XMP/EXPO, windowed games/VRR, ReBAR); FSE detection and reset-verdict fixes |
| `8309a36` | Performance Tweaks: placebo standby-cache flush removed |
| `8e33bb3` | Privacy Center: `prv_ceip_off` and `prv_maps_update_off` retired (shown only while active, journaled reset) |
| `6d8244f` | Debloater: obsolete and duplicate tweaks withdrawn (single source), `gamedvr_off` FSE twin in *Reset all*, reset verdict from the read-back |
| `f1b2290` | Hardware profile: no risk verdicts for retired tweaks |

The last full gate on Windows 11 25H2 passed (up to `a82a380`):
- `npm run build`
- `cargo fmt --check`
- `cargo check --locked`
- `cargo test --locked`: 135 passed, 4 ignored
- `cargo clippy --locked --all-targets -D warnings`

**Full gate on Windows 11 25H2 at `ffa1321` (owner, 2026-10-24): passed.** `npm ci`, `npm run build`, `cargo fmt --check`, `cargo check --locked`, `cargo test --locked` (159 passed, 4 ignored), `cargo clippy --locked --all-targets -D warnings`, and the read-only system tests (`real_health_scan`, `read_only_scans…`). In the app: System Check, Optimize (Ultimate plan = configured, Game Mode undo → re-apply), retired reset (Cortana) confirmed. Still open in the app: Privacy Center reset of CEIP/Maps (`ffa1321`), Optimize My PC, FPS import with real captures. `npm ci` reports 4 vulnerabilities (1 moderate, 3 high), see item 4.

Earlier note on the cloud session, kept for context. The second cleanup pass (`8309a36` … `f1b2290`) was made in a cloud (Linux) session:
- `npm ci && npm run build`: passed.
- `cargo fmt --check`: passed.
- `cargo check` / `cargo test`: the crate does not compile on Linux as committed (already true at `a82a380`): `privacy.rs`, `perftweaks.rs` and `services.rs` call `tweaks::reg_read`, which is `#[cfg(windows)]`. With a local, uncommitted non-Windows stub for `reg_read`, `cargo check` passed and `cargo test` passed 140, 1 ignored (the platform-independent tests only).
- `cargo clippy --all-targets -D warnings`: not meaningful on Linux (dozens of pre-existing dead-code errors from Windows-only callers). Needs the Windows run.
- GitHub Actions `Quality` did not run: it triggers only on pull requests and pushes to `main`, and there is no PR for this branch. Run the gate on Windows, or open a PR (owner's decision).

## Key files

| Area | File |
|---|---|
| Tweak catalog, apply/revert/journal, live state detection | `src-tauri/src/tweaks.rs` |
| Evidence registry (classes, rules, retired list) | `src-tauri/src/optregistry.rs` |
| Health check engine (detection, diagnosis, plan, apply+verify, reset, gaming checks) | `src-tauri/src/optimizer.rs` |
| Protected state folder and migration | `src-tauri/src/statestore.rs`, `src-tauri/src/safety.rs` (`state_dir`) |
| PowerShell helpers (quoting, timeouts, winget path) | `src-tauri/src/ps.rs` |
| Tauri commands | `src-tauri/src/lib.rs`; frontend wrappers in `src/api.ts` |
| Navigation shell | `src/App.tsx` |
| New pages | `src/pages/SystemCheck.tsx`, `src/pages/OptimizeMyPc.tsx`, `src/pages/Optimize.tsx` |
| Shared scan hook | `src/hooks/useHealthScan.ts` |
| Health check types | `src/lib/health.ts` |
| Cards | `src/components/OptCard.tsx`, `src/components/RetiredCard.tsx` |
| UI strings (de + en; other locales fall back to en) | `src/i18n.tsx` |

## Environment notes for a cloud (Linux) session

- Since `e7adecf` the repo has no `src-tauri/icons/icon.png`. Tauri needs it for **Linux** builds only (Windows uses `icon.ico`). In a Linux session restore it locally without committing it: `git show e7adecf^:src-tauri/icons/icon.png > src-tauri/icons/icon.png`.
- `cargo build --target x86_64-pc-windows-gnu` fails to link the `cdylib` (MinGW export limit). To build the exe for Wine, temporarily set `crate-type = ["rlib"]` in `src-tauri/Cargo.toml` and restore it afterwards (never commit that).

- **Frontend:** `npm ci && npm run build` works anywhere.
- **Rust:** the backend targets Windows. Most system code is behind `#[cfg(windows)]`. On Linux:
  - `cargo check`/`cargo test` need the Tauri Linux build dependencies (webkit2gtk-4.1, libgtk-3, libsoup-3, librsvg2, etc.).
  - Only the platform-independent tests run there.
  - Prefer pushing and letting the GitHub Actions `Quality` workflow (`windows-latest`) run the full gate.
- **Windows-only checks** cannot run in the cloud: the opt-in real-system tests (`real_health_scan`, `read_only_scans…`, `secure_dir_end_to_end`) and the app itself. Mark such work as "needs Windows verification" rather than claiming it passed.

## Done

- **In-game FPS measurement** (`src-tauri/src/frametime.rs`, Benchmark page): imports PresentMon / CapFrameX / FrameView CSV, avg FPS, 1 % / 0.1 % lows, before/after comparison with ≥ 3 runs per state and a noise threshold. Parser and statistics are unit-tested; the page was checked in the browser with mocked data only. No real capture has been imported yet.

- **Second cleanup pass on the other catalogs** (`8309a36` … `f1b2290`; details and tests in `optimization-engine.md`, section "Second cleanup pass"):
  - `perftweaks.rs`: `ram_flush_standby` removed (one-shot, nothing to reset).
  - `privacy.rs`: `prv_ceip_off`, `prv_maps_update_off` retired.
  - `debloater.rs`: `cortana_off`, `taskbar_widgets_off`, `gamedvr_off` and the seven Privacy Center duplicates withdrawn; `location_off` kept and cross-referenced.
  - `optimizer.rs`: `DEBLOATER_DUPLICATES` + `fse_optimizations_off` ↔ `gamedvr_off`; reset verdict from the read-back.
  - `hwprofile.rs`: verdicts for retired ids removed.

## Open work, in suggested order

1. **Windows verification of the second cleanup pass:** full gate (above), then the "Not tested" list of that section in `optimization-engine.md`. Remaining overlap that was left on purpose:
   - Debloater `bing_search` vs Privacy Center `prv_bing_search_off` vs Optimize `start_websearch_off`: different value sets, not merged.
   - Debloater `lock_screen_ads` vs Optimize `lockscreen_ads_off`, Debloater `xbox_gamebar` vs Optimize `game_dvr_off`/`gamebar_popups_off`: overlapping values, not merged.
   - Privacy Center vs Optimize privacy tweaks (`telemetry_minimal`, `advertising_id_off`, `content_suggestions_off`, `activity_history_off`, `wer_off`, `delivery_opt_off`): still two catalogs. Merging needs a decision on which page owns privacy.
   - Linux build: `tweaks::reg_read` is Windows-only but used unconditionally (see above).
2. **Translations:** German engine and catalog texts are done (`src/lib/engineText.ts`, `src/locales/deTweaks.ts`). Still open: es/fr/pt/pl/ru/tr/sv for the new UI strings (`hc*`, `om*`, `opt*`, `ret*`, `gs*`, `wd*`, `perfRamFlushRemoved`, `navSystemCheck`, …; `perfRamDesc` was removed from the other locales because it still described the flush) and the engine texts (an `engineText` table per language) in `src/locales/*.ts`.
3. **Small UI items:**
   - Monitor names show as "Generic PnP Monitor"; `WmiMonitorID` gives real names.
   - The Optimize page `h1` duplicates the section title.
4. **CI and supply chain:**
   - Pin GitHub Actions to commit SHAs.
   - Bump `time` (RUSTSEC-2026-0009) and check the new `cargo audit` job.
5. **Structural security (HIGH):** the whole WebView runs elevated, so all 163 commands are reachable from the renderer. The design is written: `docs/audit-2026-10/elevation-design.md` (recommended: on-demand elevated helper `--broker` over an ACL-protected named pipe, typed allowlist, five migration phases). Owner decisions are recorded in section 8 of that document (option B, UAC once per session, journal readable without admin, drop unused commands). Phases 1 (inventory, unused commands removed) and 2 (broker skeleton behind the `broker` cargo feature) are done; phase 3 has started: tweak Apply/Undo go through the broker when it is connected, the header has "Unlock admin actions", and `--broker-selftest` checks the launch end to end. See section 9 of that document, including how to test on Windows. Next: route the remaining privileged commands module by module. The cloud session can now cross-check Windows builds: `rustup target add x86_64-pc-windows-gnu`, `apt-get install gcc-mingw-w64-x86-64 wine64`, then `cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D warnings` and `cargo test --target x86_64-pc-windows-gnu --lib --no-run` + `wine <test exe>`.
6. **Windows validation** (owner or Windows VM): run the matrix from `REPORT.md` section G and the "Not tested" list in `optimization-engine.md`.
   - Apply/undo via Optimize My PC.
   - Reset all.
   - Windows 10 22H2 vs 11 24H2/25H2.
   - English vs German.
   - Admin vs non-admin.

## How to verify a change

The full gate below is the same as CI. On Windows, the opt-in system tests can be added on top.

```
npm ci
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
# Windows only, read-only:
cargo test --manifest-path src-tauri/Cargo.toml --locked -- --ignored read_only_scans real_health_scan --nocapture
```
