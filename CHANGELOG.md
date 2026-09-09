# Changelog

All notable changes to AD HyperOptimize are documented here. The in-app
changelog is generated from this file, and each GitHub release uses the matching
section below as its release notes.

## v1.6.4 — Services overview, uninstaller privilege hardening

### Services Manager
- Services are now categorized by origin like scheduled tasks: anything whose executable lives outside the Windows directory is tagged and filterable as third-party (own tab, summary card, and badge), so installed-software services are immediately distinguishable from inbox ones.
- The one-card-per-service layout is replaced by a dense table — roughly three times as many services per screen, with description and bloat reason as hover tooltips and start-type/start/stop/restart controls inline.

### Security
- Closed an admin-escalation path in the App Uninstaller: the per-user (HKCU) uninstall list could be forged by user-level software, and executing such an entry through the cmd.exe fallback would have run it elevated. The fallback is now refused for per-user apps with a clear manual-uninstall message; quoted-exe and MsiExec uninstallers (the overwhelming majority) keep working.

### Fixes
- Host-target builds compile again (a Windows-only visibility attribute leaked into a cross-platform helper).

## v1.6.3 — Security & reliability hardening (external audit response)

An independent audit against v1.5.0 reported ~218 findings. All Critical and
High findings are closed in this release; the actionable Medium tier is done
as well.

### Security
- Every PowerShell child now runs with forced UTF-8 output encoding: localized Windows (CP1252, CP936, …) no longer corrupts service names, file paths and process names into replacement characters.
- MSI-mode toggle rejects wildcard device paths — a single `*` could have flipped MSI on every device, including the boot storage controller.
- Disk Analyzer, Uninstaller and file organization refuse symlink/junction paths and skip reparse points while scanning, so a link can never pull protected system folders into a delete or move.
- The debloater's restore script escapes every field of its state file and whitelists startup types.
- Hosts-file edits require administrator rights and serialize on a lock (no more lost-update races).
- Winget package ids are quote- and regex-escaped; scheduled-task and service identifiers reject wildcard characters.
- Force-revert no longer deletes registry values it cannot prove it created: it restores from the newest backup or refuses with the exact key path.
- Quick Boost validates the captured process priority and HAGS value before writing them back.
- The updater minisign fingerprint (75A816CD47EDE457) is now published in the README for out-of-band verification.

### Undo & journal engine
- Registry captures are type-faithful: exotic value types are restored in their original representation instead of being degraded or deleted.
- Interrupted applies no longer count as applied, and a partial undo resumes exactly where it stopped instead of re-running finished steps.
- Nagle undo is built from a per-adapter snapshot taken before the change: pre-existing custom values are restored verbatim, later-added adapters are never touched.
- Restore tokens are collision-proof; a failed backup now aborts instead of skipping silently; the power-scheme undo works and no longer targets the wrong plan.
- Privacy tweaks and the timer-resolution task are journaled and undoable.
- Registry Cleaner runs reg.exe with a hard timeout and keeps its backups in the same data folder as the journal (old backups are still found).

### Fixes
- No more flashing console window during game detection (every 3 seconds on some systems).
- Disk health verdicts are locale-independent (exit codes instead of English DISM phrases); Wi-Fi detection no longer reports every Ethernet system.
- Affinity masks work beyond 32 cores; the PC Configurator no longer re-scans on every visit and no longer recommends DDR5 for high-clocked DDR4 kits.
- Context-menu wildcards, benchmark temp files, boot-log read access, RAM counters on localized Windows, protected-process list: all corrected.
- Pending backend errors no longer leave pages spinning forever (Profiles, RegClean, Optimize, Software Installer, Debloater).
- First-run language now follows the system locale; dates follow the selected language.

## v1.6.2 — Instant update check, glowing self-update button

### Updates page
- Opening the Updates page now checks for an app update automatically: the install button appears right away, without an extra "check for updates" click.
- The app self-update actions (download/install and restart) pulse with an accent glow so the one button that updates the app itself stands out. With reduced-motion enabled a static glow is shown instead.

## v1.6.1 — Single-window admin start, scheduled task sorting

### Fixes
- Fixed the app starting twice when the UAC prompt was confirmed. The unelevated instance now decides before any window appears: "Yes" launches only the admin window, "No" only the normal one.

### Scheduled Tasks
- Tasks can now be sorted by name, creation date, and last run (click the column headers; entries without a date sort last).
- Tasks installed outside \Microsoft\ are marked and filterable as third-party, shown with their author and executable so they can be reviewed and disabled deliberately.

## v1.6.0 — Exact undo, complete localization, in-app changelog

### What's new
- The changelog now lives inside the app: a dedicated page (scroll down for older versions), a link in Settings, and a compact "What's new" bar on the home screen.
- Complete localization for all seven additional languages (Spanish, French, Portuguese, Polish, Russian, Turkish, Swedish). Any still-missing string now falls back to English instead of German.

### Reliability & honesty
- Exact undo: the RSS, USB selective-suspend and CPU core-parking tweaks now capture their real previous value instead of restoring a hardcoded guess.
- A multi-step tweak that fails now reports honestly whether the automatic rollback fully succeeded, instead of always claiming the changes were rolled back.
- Power-plan health detection uses locale-independent GUIDs, so non-English/German Windows no longer produces a false "suboptimal power plan" finding.

### Performance & UX
- Admin checks and the monitor / profile / startup / process listings no longer run on the UI thread, so the interface stays responsive.
- Pages now surface backend errors instead of hanging on a spinner, disable their buttons while an action is running (no accidental double-apply), and a Disk Analyzer scan race plus Live Monitor NaN readouts were fixed.

### Under the hood
- Sturdier registry cleaner, debloater status check and Game Boost state handling, safe environment-variable expansion, and iterative directory walks that cannot overflow the stack.

## v1.5.0 — UI/UX audit, full i18n, feature search

### What's new
- Feature search now finds individual tweaks, services, startup entries, scheduled tasks, GPU tweaks, and privacy/debloater items, then jumps straight to the matching feature.
- A Settings page centralizes language, Beginner/Expert mode, and clearing the scan cache.
- The home screen shows recently used tools for one-click return.
- Keyboard shortcuts: / focuses search and Esc clears it.

### Quality and accessibility
- Auto-Optimizer shows per-item risk and confirms before applying; destructive actions require confirmation.
- Actions report loading, success, and failures instead of silently hanging or claiming success.
- Translation coverage, keyboard focus, reduced motion, semantic headings, table headers, and narrow-window layouts were improved.

### Fixes
- Fixed a blank Optimize page caused by a shadowed translation variable.
- Fixed Debloater status refreshing after apply and Live Monitor data on first open.
- Fixed the Windows Vite watcher crash caused by the Rust target directory.

## v1.4.0 — Dashboard redesign, honest by default, restorable registry cleaner

### What's new
- New category-based dashboard home, Beginner/Expert modes, and first-run language and experience setup.
- Registry Cleaner exports the affected key before deletion and provides a Restore panel for prior cleanups.

### Fixes and honesty
- Tweaks that settle after reboot report partially applied and can be re-applied.
- Missing Windows services are treated as already satisfied instead of failing an apply operation.
- Administrator requirements, Security controls, and QoS bandwidth claims were clarified to match actual behavior.

### Under the hood
- CI now compiles the privileged Rust backend, not only the frontend.
