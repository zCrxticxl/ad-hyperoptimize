v1.4.0: Dashboard redesign, honest by default, restorable registry cleaner

What's new
- New dashboard home. Pick a category instead of scrolling a wall of 39 tools. Tools are grouped, each with a one-line explainer, and a top search jumps straight to any of them.
- Real Beginner and Expert modes. Beginner now actually hides advanced tooling (registry, services, drivers) so you can't wander into it by accident. One click unlocks the full set.
- First-run setup. Pick your language and experience level on the first launch.

Registry cleaner is now genuinely reversible
- Before every deletion the affected key is exported to a real .reg file. If the export fails, nothing is deleted.
- New "Restore..." panel lists your past cleanups and puts any of them back with one click.

Fixes
- The "Apply" button no longer gets stuck when a tweak's value only settles after a reboot. It now shows "partially applied" with a quiet Re-apply instead.
- Applying a tweak that targets a Windows service you don't have (e.g. the VS diagnostics collector) no longer errors. A missing service is treated as already satisfied.

Honesty pass
- Corrected claims that didn't match the code. The app needs administrator rights (and says so), the Security page offers explicit opt-in controls rather than read-only audit, and the QoS "up to 20% more bandwidth" tweak now states plainly that this is a myth with no real-world gain.

Under the hood
- CI now compiles the privileged Rust backend, not just the frontend.

Free, source-available, zero telemetry. Undo is built in, and a restore point before bigger changes is still smart.
