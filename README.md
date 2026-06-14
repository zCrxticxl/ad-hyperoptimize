# PC Optimization & Deep Diagnostic Suite

Join my dc for help: https://discord.gg/GPcTdABdcY

Modern, safe, fully reversible Windows 10/11 optimization + diagnostics desktop app.
**Stack:** Tauri 2 (Rust backend) · React 18 + TypeScript · Recharts · WMI/PowerShell/SMART/Event Log integration.

## Architecture

```
optimize app/
├── src/                      # React + TS frontend
│   ├── App.tsx               # Shell: sidebar, beginner/expert mode, admin badge
│   ├── api.ts                # Typed invoke() wrappers + metrics event stream
│   ├── components/ui.tsx     # Card, Badge, Bar, ActionBtn, RawJson
│   └── pages/                # Dashboard, Hardware, Monitor, Optimize,
│                             # Cleanup, Security, Benchmark, Reports
└── src-tauri/
    ├── tauri.conf.json       # NSIS + MSI bundling config
    ├── capabilities/         # Tauri 2 permission scoping (least privilege)
    └── src/
        ├── ps.rs             # PowerShell/exec bridge (no console flashes)
        ├── scan.rs           # Full WMI/SMART/boot/event/DNS/network analysis
        ├── monitor.rs        # 1s real-time metrics thread → "metrics" events
        ├── tweaks.rs         # Declarative tweak catalog + apply/revert engine
        ├── safety.rs         # Restore points, .reg backups, write-ahead journal
        ├── cleanup.rs        # Whitelisted-roots cache/temp cleaner
        ├── security.rs       # Defender/firewall/unsigned drivers/autoruns/hosts
        ├── bench.rs          # CPU/memory/disk benchmarks + history
        ├── analysis.rs       # Rule-based findings engine + health score
        └── report.rs         # Dark HTML + JSON reports (print → PDF)
```

### Module responsibilities & safety model
- **Write-ahead journaling:** every tweak captures previous values and writes a journal entry (`%APPDATA%\PCOptSuite\journal.json`) *before* mutating anything. Failed applies auto-roll back.
- **Registry backups:** each touched key is exported via `reg.exe` to `%APPDATA%\PCOptSuite\backups\*.reg` first.
- **Restore points:** one click (`Checkpoint-Computer`), surfaced before Medium-risk tweaks.
- **Explicit consent:** UI requires per-tweak confirm; details panel shows what/why/impact/risk/reversibility.
- **No myths:** catalog contains only documented, measurable tweaks (MMCSS, Game DVR, power plans, telemetry policy, etc.). No "registry cleaning".
- **Cleanup whitelist:** deletion only inside hardcoded cache/temp roots; locked files skipped, never forced.
- **Security page is read-only.** Telemetry of the app itself: none.

## Prerequisites (build machine, Windows)
1. **Rust** — https://rustup.rs (MSVC toolchain: `rustup default stable-msvc`)
2. **Node.js 20+** — https://nodejs.org
3. **Visual Studio Build Tools** with "Desktop development with C++"
4. **WebView2 runtime** (preinstalled on Win 10/11; installer bootstraps it otherwise)

## Build

```powershell
cd "E:\optimize app"
npm install

# one-time: generate app icons from any 1024px PNG
npx tauri icon path\to\icon.png    # creates src-tauri/icons/* incl. icon.ico

# dev (hot reload)
npm run tauri dev

# release: produces NSIS .exe installer + .msi
npm run tauri build
```

Artifacts land in `src-tauri\target\release\bundle\nsis\*.exe` and `...\msi\*.msi`.
**Portable build:** `src-tauri\target\release\pc-opt-suite.exe` runs standalone (requires WebView2 runtime present).

## Code signing (release-ready)
`tauri.conf.json → bundle.windows` accepts:
```json
"certificateThumbprint": "<thumbprint>",
"digestAlgorithm": "sha256",
"timestampUrl": "http://timestamp.digicert.com"
```
Or sign post-build: `signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /sha1 <thumbprint> <installer.exe>`.
Unsigned builds trigger SmartScreen — expected; sign with an EV/OV cert for distribution.

## Admin elevation
The app runs as standard user; HKLM/service/powercfg tweaks detect non-admin and instruct a restart "as administrator". To force elevation for the installer build, add an NSIS `installMode: "perMachine"` or embed a manifest (`requestedExecutionLevel: requireAdministrator`) — deliberately not default, least privilege first.

## Security considerations
- All shell invocations are fixed strings or strictly interpolated identifiers (no user-supplied command text).
- PowerShell runs `-NoProfile -NonInteractive`; no remote content executed.
- Tauri capability file grants only `core` permissions; no FS/network plugin surface exposed to the webview.
- Cleanup cannot escape whitelisted roots; tweak engine can only touch cataloged keys/services.

## Extending the tweak catalog
Add one `Tweak {}` block in `src-tauri/src/tweaks.rs`. Status detection, backup, journaling, confirm UI and undo come free — no other code changes needed.

## Roadmap hooks (architecture supports)
ETW/DPC latency tracing (`tracelogging`/`xperf` integration point in `scan.rs`), crash-dump parsing, GPU vendor APIs (NVML/ADL), overlay (separate transparent Tauri window fed by the existing `metrics` event), LLM-backed summaries on top of `analysis.rs` findings JSON.
