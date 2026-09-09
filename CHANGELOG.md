# Changelog

All notable changes to AD HyperOptimize are documented here. The in-app
changelog is generated from this file, and each GitHub release uses the matching
section below as its release notes.

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
