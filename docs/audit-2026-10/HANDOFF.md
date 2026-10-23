# Handoff: AD HyperOptimize overhaul (branch `audit/2026-10-overhaul`)

This file lets a new session (for example a cloud session) continue the work
without the previous conversation.

**Read first:**
1. `AGENTS.md` (architecture, validation, safety rules)
2. `docs/audit-2026-10/REPORT.md`
3. `docs/audit-2026-10/optimization-engine.md`

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

The last full gate on Windows 11 25H2 passed:
- `npm run build`
- `cargo fmt --check`
- `cargo check --locked`
- `cargo test --locked`: 135 passed, 4 ignored
- `cargo clippy --locked --all-targets -D warnings`

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

- **Frontend:** `npm ci && npm run build` works anywhere.
- **Rust:** the backend targets Windows. Most system code is behind `#[cfg(windows)]`. On Linux:
  - `cargo check`/`cargo test` need the Tauri Linux build dependencies (webkit2gtk-4.1, libgtk-3, libsoup-3, librsvg2, etc.).
  - Only the platform-independent tests run there.
  - Prefer pushing and letting the GitHub Actions `Quality` workflow (`windows-latest`) run the full gate.
- **Windows-only checks** cannot run in the cloud: the opt-in real-system tests (`real_health_scan`, `read_only_scans…`, `secure_dir_end_to_end`) and the app itself. Mark such work as "needs Windows verification" rather than claiming it passed.

## Open work, in suggested order

1. **Second cleanup pass on the other catalogs**, using the same retire + reset pattern, or merge them into `tweaks.rs` as the single source:
   - `perftweaks.rs`: `ram_flush_standby` (placebo).
   - `privacy.rs`: `prv_ceip_off` and `prv_maps_update_off` (obsolete).
   - `debloater.rs` tweaks:
     - `cortana_off` and `taskbar_widgets_off` (Windows 11: obsolete or wrong mechanism)
     - `gamedvr_off`, which mixes an FSE change into a DVR toggle
     - its duplicates of telemetry / advertising ID / suggestions / location
   - `hwprofile.rs` still emits risk verdicts for retired tweak ids; remove them.
2. **Translations** of the new strings (`hc*`, `om*`, `opt*`, `ret*`, `gs*`, `navSystemCheck`, …) for es/fr/pt/pl/ru/tr/sv in `src/locales/*.ts`.
3. **Small UI items:**
   - `fmtAge` is English-only ("just now").
   - Monitor names show as "Generic PnP Monitor"; `WmiMonitorID` gives real names.
   - The Optimize page `h1` duplicates the section title.
4. **CI and supply chain:**
   - Pin GitHub Actions to commit SHAs.
   - Bump `time` (RUSTSEC-2026-0009) and check the new `cargo audit` job.
5. **Structural security (HIGH):** the whole WebView runs elevated, so all ~160 commands are reachable from the renderer. Design an unelevated UI plus a small elevated broker with an allow-listed command surface. Write the design document before implementing.
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
