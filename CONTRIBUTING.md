# Contributing

Thanks for helping improve AD HyperOptimize.

## Before opening a pull request

1. Create an issue first for new tweaks or behavioral changes.
2. Keep every tweak explicit about its effect, risk, prerequisites, verification, and rollback.
3. Never add a destructive action, remote download, or elevated command without an allowlisted target and a safe failure path.
4. Run `npm ci` and `npm run build`. Keep Rust changes formatted with `cargo fmt` before submitting them.

## Pull requests

Describe the affected Windows versions, manual verification performed, rollback behavior, and any administrator privileges required. Do not include secrets, private logs, or user-identifying machine information.
