. "$PSScriptRoot/Release.Common.ps1"
Assert-SigningIdentity $env:WINDOWS_SIGNING_THUMBPRINT $env:WINDOWS_TIMESTAMP_URL
if (!$env:WINDOWS_CERTIFICATE -or !$env:WINDOWS_CERTIFICATE_PASSWORD) {
    throw 'Code-signing credentials are required. Unsigned release publication is disabled.'
}

$certPath = Join-Path $env:RUNNER_TEMP 'ad-hyperoptimize-signing.pfx'
[IO.File]::WriteAllBytes($certPath, [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE))
try {
    $password = ConvertTo-SecureString $env:WINDOWS_CERTIFICATE_PASSWORD -AsPlainText -Force
    # Record imported identities immediately so cleanup also runs after validation fails.
    $certificates = @(Import-PfxCertificate -FilePath $certPath -CertStoreLocation Cert:\CurrentUser\My -Password $password)
    $thumbprints = ($certificates | ForEach-Object { $_.Thumbprint }) -join ','
    "IMPORTED_SIGNING_THUMBPRINTS=$thumbprints" | Out-File $env:GITHUB_ENV -Append -Encoding utf8
    $leaf = @($certificates | Where-Object { $_.HasPrivateKey })
    if ($leaf.Count -ne 1 -or $leaf[0].Thumbprint -ne $env:WINDOWS_SIGNING_THUMBPRINT) {
        throw 'Imported private key does not match the single approved signing identity.'
    }
    Assert-CodeSigningCertificate $leaf[0]
    # Tauri uses this hook for application binaries, the NSIS uninstaller and installers.
    # Every invocation signs and verifies before packaging can proceed.
    $config = @{
        bundle = @{
            windows = @{
                signCommand = @{
                    cmd = (Get-Command pwsh -ErrorAction Stop).Source
                    args = @('-NoProfile', '-NonInteractive', '-File',
                        (Join-Path $PSScriptRoot 'Sign-WindowsBinary.ps1'), '-Path', '%1')
                }
            }
        }
    }
    Write-ReleaseJson (Join-Path $env:RUNNER_TEMP 'adho-signing.json') $config
} finally {
    Remove-Item -LiteralPath $certPath -Force -ErrorAction Stop
}
