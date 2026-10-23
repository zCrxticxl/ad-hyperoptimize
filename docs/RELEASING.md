# Releasing AD HyperOptimize

Fewer, tested releases build more trust than several releases a day. Each release goes through the steps below in order. `main` is protected: changes arrive only through pull requests with green `frontend` and `backend` checks.

## 1. Collect

- Merge feature work into `main` via pull requests. Every change that users notice gets a line in `CHANGELOG.md` under `## Unreleased`.
- Batch fixes. A typo or a single small fix does not need its own release; let it ride with the next one.

## 2. Verify before tagging

Run the full gate locally (same as CI):

```
npm ci
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
```

Run the read-only system tests on a real Windows machine:

```
cargo test --manifest-path src-tauri/Cargo.toml --locked -- --ignored read_only_scans real_health_scan --nocapture
```

Then do a manual pass on a **disposable Windows VM**, never the release machine:
- Windows 10 22H2 and Windows 11 (current release).
- English and German.
- Run as admin and as a standard user.

On each VM:
- Open System Health Check, then *Optimize My PC*: apply, read the result categories, undo from the history, rescan.
- *Reset all* for retired tweaks.
- Open each sidebar section once; no page may show an error banner on a healthy system.

Write down what was tested, and on which build, in the PR or release notes.

## 3. Version and changelog

- Bump the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`. All three must match.
- Rename `## Unreleased` to `## vX.Y.Z — Short title`. The in-app changelog and the GitHub release notes are both generated from that section.

## 4. Tag and publish

- Tag `vX.Y.Z` on `main` and push the tag. The `Release` workflow builds the NSIS/MSI installers, signs them when the signing secrets are configured, and publishes `SHA256SUMS.txt` and the updater `latest.json`.
- Check the release page:
  - installers present
  - `SHA256SUMS.txt` matches
  - the updater signature fingerprint in the README is unchanged

## 5. If something is wrong

Do not publish a quick follow-up release on the same day. Mark the release as a pre-release or delete it, fix it on a branch, and go through steps 2–4 again.
