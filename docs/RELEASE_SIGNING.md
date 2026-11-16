# Windows release verification

The release workflow signs the application EXE before packaging. Tauri's generated
`bundle.windows.signCommand` signs and verifies packaged binaries, the NSIS
uninstaller and both NSIS/MSI installers with SHA-256 and RFC 3161 timestamps.
`Test-ReleaseSignatures.ps1` then checks the final app and installers for an embedded,
trusted Authenticode signature, the approved certificate and a timestamp.

## Configuration

GitHub Actions secrets:

- `WINDOWS_CERTIFICATE`: base64-encoded code-signing PFX containing the private key.
- `WINDOWS_CERTIFICATE_PASSWORD`: PFX password.
- `TAURI_SIGNING_PRIVATE_KEY`: existing Tauri updater key.
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: updater key password, if configured.

Repository variables:

- `WINDOWS_SIGNING_THUMBPRINT`: approved certificate's 40-character SHA-1 thumbprint.
- `WINDOWS_TIMESTAMP_URL`: provider's HTTPS RFC 3161 endpoint.
- `WINDOWS_RELEASE_RUNNER`: runner label (default `adho-release`).

Register a clean, disposable Windows x64 release runner with that label, PowerShell
7, Git, GitHub CLI, Windows SDK signing tools and the Rust MSVC build prerequisites.
Defender must satisfy the gate below. GitHub's standard Windows images
[disable Defender and exclude their build drives](https://github.com/actions/runner-images/blob/main/images/windows/scripts/build/Configure-WindowsDefender.ps1),
so `windows-latest` is not a suitable default for this job. Do not reuse a general
development machine or a worker that executes untrusted pull requests.

The current adapter requires an importable PFX. Hardware-token and cloud signing
services require a provider-specific signing adapter and credentials; purchasing
such a certificate alone does not configure this workflow. Keep the same
post-signing verification gates when adapting it. Windows Authenticode and the
Tauri updater signature are separate mechanisms.
The updater signer receives an explicit empty password when no password secret
is configured. Its output key identifier must match the configured updater public
key. This catches key configuration mistakes; it is not an independent
cryptographic verification of the generated updater signature.

## Defender gate and evidence

After signing, `Test-ReleaseDefender.ps1` updates security intelligence and requires
active Defender in Normal mode, real-time/archive scanning, definitions at most
24 hours old, and no exclusion or threat-action overrides. It does not disable
protection or remove exclusions to make a runner pass. A runner that fails these
requirements needs an appropriately configured release environment.

Each final file (app EXE, NSIS EXE, MSI) receives a custom `MpCmdRun` scan with
`-DisableRemediation`, bounded to ten minutes. Nonzero exit, timeout, unavailable
Defender, or a changed file hash blocks publication.

The workflow retains `release-verification-<run-id>` for 90 days, including:

- `authenticode.json`: commit, hashes, signer and timestamp certificate identities.
- `defender.json`: commit, hashes, engine and definition versions, scan times,
  exit codes and overall result.
- Per-file Defender stdout and stderr logs.

Publication requires both successful reports for the same commit and unchanged
file hashes; Defender evidence must be less than two hours old. Installers are
staged into a draft release. The authenticated releases list locates the draft;
verification and publication use its numeric ID, because tag lookup is documented
for published releases. Uploaded GitHub SHA-256 digests must match before the draft
becomes public. The updater URL uses the final tagged URL and verified asset name,
not a temporary draft download URL. A failed upload/verification leaves the draft
for inspection; do not overwrite an existing release to retry.

Before retrying a failed publication, inspect the leftover draft and its assets.
After confirming it is still unpublished, delete that draft in GitHub. The scripts
deliberately do not delete it automatically or overwrite its assets. Keep the
existing tag if it points at the same commit; a manual retry reuses it. If the tag
points at another commit, use a new application version and tag rather than moving
the old tag. A failed publication can therefore leave a tag without a release.

## Validation and release operation

Run `powershell -NoProfile -File scripts/release/tests/Release.Tests.ps1` locally
or the same script with `pwsh` in CI. These are inert fixtures for rejection paths,
hash binding and script parsing, not actual signature or antivirus tests.

For an authorized validation run, synchronize versions, configure the runner and
signing, and run the Release workflow on `main` with `publish=false` (the default).
It builds, signs and scans, retaining evidence without creating a tag or release.
This mode still requires approval for installer building and signing.

After inspecting the evidence, an authorized run with `publish=true` creates the
tag after the signature and Defender gates pass, then publishes the verified
draft. Tag-triggered runs publish and require an already-created matching tag.
Every run checks that its commit is reachable from `origin/main` before accessing
signing secrets.

A clean artifact scan is not a runtime behavior test or a guarantee against future
detections. Test installation and the installed app in an authorized disposable
Windows environment separately. Report false positives to Microsoft with the exact
artifact hash and detection details. Never add antivirus exclusions as a workaround.
