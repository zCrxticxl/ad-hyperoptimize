# AD HyperOptimize Engineering Guide

## Architecture

- `src/` is the Vite/React frontend: `src/main.tsx` mounts `App.tsx`; `App.tsx` owns page routing; `src/api.ts` is the frontend Tauri-command surface.
- `src-tauri/src/lib.rs` declares and registers all `#[tauri::command]` handlers. A new backend capability normally needs its Rust implementation, a `lib.rs` handler/registration, and an `src/api.ts` wrapper.
- Tweak changes belong in the declarative catalog and its existing status/apply/revert flow in `src-tauri/src/tweaks.rs`; do not bypass its journal, backup, confirmation, or undo behavior.
- Tauri starts Vite with `npm run dev` at fixed port `5173`. Vite deliberately ignores `src-tauri/target/**` because Windows locks Rust DLLs and its watcher otherwise fails with `EBUSY`.

## Validation

- Install frontend dependencies with `npm ci`; `npm run build` runs `tsc && vite build` and is the frontend CI gate.
- For Rust changes, follow CI order after the frontend build: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`, `cargo check --manifest-path src-tauri/Cargo.toml --locked`, `cargo test --manifest-path src-tauri/Cargo.toml --locked`, then `cargo clippy --manifest-path src-tauri/Cargo.toml --locked -- -D warnings`.
- Run one Rust test by appending its filter: `cargo test --manifest-path src-tauri/Cargo.toml --locked <test-filter>`.

## Safety And Releases

- This is a privileged Windows optimizer. Treat changes to registry, services, Defender, firewall, drivers, cleanup, uninstall, elevation, and state-changing Tauri commands as high risk; preserve the existing explicit user action and reversible-state patterns.
- Do not launch the elevated app, apply optimizations, install toolchains, build installers, sign artifacts, tag, or publish releases without explicit approval.
- Do not discard uncommitted changes in `src-tauri/Cargo.toml`.
- Version bumps must keep `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` synchronized. Tauri bundles per-machine NSIS and MSI installers.
