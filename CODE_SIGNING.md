# Windows code signing

AD HyperOptimize already uses Tauri updater signatures. Windows Authenticode signing is a separate signature that identifies the publisher of the downloaded `.exe` and `.msi` files.

## One-time setup

1. Obtain a Windows **code-signing** certificate from a trusted provider. An SSL certificate cannot sign Windows software.
2. Export it as a password-protected `.pfx` file.
3. Add these GitHub Actions secrets in the repository settings:
   - `WINDOWS_CERTIFICATE`: Base64-encoded bytes of the `.pfx` file.
   - `WINDOWS_CERTIFICATE_PASSWORD`: the `.pfx` export password.
4. Add the repository variable `WINDOWS_TIMESTAMP_URL` with the HTTPS timestamp URL provided by the certificate issuer.

To create the secret value locally in PowerShell without uploading the certificate file itself:

```powershell
[Convert]::ToBase64String([System.IO.File]::ReadAllBytes('certificate.pfx'))
```

## Release behavior

When all three settings exist, the release workflow imports the certificate only into the temporary GitHub runner, injects its thumbprint into the build configuration, validates every generated `.exe` and `.msi`, and removes the temporary certificate file. The workflow also publishes `SHA256SUMS.txt` for every installer.

Until those settings exist, releases stay unsigned exactly as before. Never commit a `.pfx`, password, thumbprint, or timestamp credential to this repository.

## Verify a downloaded installer

```powershell
Get-AuthenticodeSignature .\AD-HyperOptimize-setup.exe
Get-FileHash .\AD-HyperOptimize-setup.exe -Algorithm SHA256
```
