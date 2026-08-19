# v1.5.0 (unreleased) — UI/UX audit, full i18n, feature search

> Draft changelog for the next release. Cut the tag + version bump when ready.

## What's new

- **Feature search across everything.** The top search now finds not just tool names but individual tweaks, services, startup entries, scheduled tasks, GPU tweaks and privacy/debloater items — and jumps straight to the feature, highlighted. Type "HAGS" or "DiagTrack" and click through.
- **Settings page.** Language (all 9), Beginner/Expert mode, and a "clear scan cache" action — in one place.
- **Recently used.** The home screen shows your last tools for one-click return.
- **Keyboard shortcuts.** `/` focuses the search, `Esc` clears it.

## UI/UX audit (28 proposals)

- **Trust & honesty.** Auto-Optimizer now shows real per-item risk and confirms before applying; Health Check no longer shows a green "all good" while checks are still running; the hardware risk badge says "no known issue" instead of a confident "OK"; destructive actions (registry clean, debloater removal, game-booster kill, leftover clean) all require confirmation.
- **Errors never swallowed.** Action buttons surface failures; every page reports loading / done / failed honestly instead of hanging or claiming success.
- **Full i18n.** ~150 hardcoded English strings routed through the translation system — Optimize and Security are now fully translated in all 9 languages; proper singular/plural (German "1 Werkzeug", "1 Befund").
- **Accessibility.** Primary button contrast fixed to WCAG AA; visible keyboard focus on every control; reduced-motion support; real heading structure (h1/h2) and table headers; keyboard-operable cards; copyable technical text.
- **Consistency & responsive.** Single design-token palette, removed dead CSS, fixed undefined variables; tweak rows wrap and tables scroll on narrow windows.

## Fixes

- Optimize page no longer renders a blank window (a shadowed `t` variable crashed the render).
- Debloater applied status no longer flips back to "not applied" after applying (uses the captured state as fallback + re-scans).
- Live Monitor now shows data on the first open (subscribes before starting, with a connecting state).
- Vite dev watcher no longer crashes with `EBUSY` on Windows (ignores the Rust `target/` dir).
- Navigation renames for clarity: "Energieplan", "Bloatware entfernen", "Auslastung", "Temperaturen & Sensoren", "NVIDIA-Einstellungen", "Verlauf & Berichte" (36 tools, was mislabeled 39).

Free, source-available, zero telemetry. Undo is built in, and a restore point before bigger changes is still smart.
