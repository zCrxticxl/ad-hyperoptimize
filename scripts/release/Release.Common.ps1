Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-SignTool {
    $tools = @(Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction Stop |
        Sort-Object { [version]$_.Directory.Parent.Name } -Descending)
    if (!$tools.Count) { throw 'Windows SDK x64 signtool.exe is required.' }
    return $tools[0].FullName
}

function Invoke-ReleaseTool {
    param([string]$Tool, [string[]]$Arguments)
    # Native failures must not turn into successful PowerShell steps.
    & $Tool @Arguments | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "$Tool failed with exit code $LASTEXITCODE." }
}

function Assert-SigningIdentity {
    param([string]$Thumbprint, [string]$TimestampUrl)
    if ($Thumbprint -notmatch '^[0-9A-Fa-f]{40}$') {
        throw 'WINDOWS_SIGNING_THUMBPRINT must identify the approved code-signing certificate.'
    }
    $uri = $null
    if (![uri]::TryCreate($TimestampUrl, [UriKind]::Absolute, [ref]$uri) -or
        $uri.Scheme -ne 'https' -or $uri.UserInfo -or $TimestampUrl -match '[\r\n]') {
        throw 'WINDOWS_TIMESTAMP_URL must be an HTTPS RFC 3161 timestamp endpoint without credentials.'
    }
}

function Assert-AuthenticodeResult {
    param($Signature, [string]$Thumbprint, [string]$Path)
    if ([string]$Signature.Status -ne 'Valid') {
        throw "Invalid Authenticode signature: $Path ($($Signature.Status))."
    }
    if (!$Signature.SignerCertificate -or $Signature.SignerCertificate.Thumbprint -ne $Thumbprint) {
        throw "Unexpected publisher certificate: $Path."
    }
    if (!$Signature.TimeStamperCertificate) { throw "Missing trusted timestamp: $Path." }
    # A catalog association must not substitute for the embedded release signature.
    if ([string]$Signature.SignatureType -ne 'Authenticode') {
        throw "An embedded Authenticode signature is required: $Path."
    }
}

function Assert-CodeSigningCertificate {
    param($Certificate, [datetime]$Now = (Get-Date))
    if ($Certificate.NotAfter -le $Now -or $Certificate.NotBefore -gt $Now -or
        '1.3.6.1.5.5.7.3.3' -notin @($Certificate.EnhancedKeyUsageList | ForEach-Object { $_.ObjectId })) {
        throw 'A currently valid certificate with Code Signing EKU is required.'
    }
}

function Assert-UpdaterSigningIdentity {
    param([string]$Signature, [string]$PublicKey)
    $keyLines = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($PublicKey)).Trim() -split '\r?\n'
    $sigLines = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Signature)).Trim() -split '\r?\n'
    $key = [Convert]::FromBase64String($keyLines[1])
    $sig = [Convert]::FromBase64String($sigLines[1])
    if ($key.Length -ne 42 -or $sig.Length -ne 74 -or
        [Text.Encoding]::ASCII.GetString($key[0..1]) -cne 'Ed' -or
        [Text.Encoding]::ASCII.GetString($sig[0..1]) -cnotin @('Ed', 'ED') -or
        [Convert]::ToBase64String($key[2..9]) -cne [Convert]::ToBase64String($sig[2..9])) {
        throw 'Updater signature key ID does not match the configured public key.'
    }
}

function Test-ReleaseSignature {
    param([string]$Path, [string]$Thumbprint, [string]$SignTool)
    $signature = Get-AuthenticodeSignature -LiteralPath $Path -ErrorAction Stop
    Assert-AuthenticodeResult $signature $Thumbprint $Path
    # /all checks every signature; /tw makes a missing timestamp a warning.
    # We reject warnings as well as errors, and require a timestamp above.
    Invoke-ReleaseTool $SignTool @('verify', '/pa', '/all', '/tw', '/v', $Path)
    return [ordered]@{
        name = [IO.Path]::GetFileName($Path)
        sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
        signerThumbprint = $signature.SignerCertificate.Thumbprint
        timestampSignerThumbprint = $signature.TimeStamperCertificate.Thumbprint
    }
}

function Get-ReleaseFiles {
    param([string]$ReleaseDirectory)
    $root = (Resolve-Path -LiteralPath $ReleaseDirectory -ErrorAction Stop).Path
    $app = Get-Item -LiteralPath (Join-Path $root 'ad-hyperoptimize.exe') -ErrorAction Stop
    $nsis = @(Get-ChildItem (Join-Path $root 'bundle/nsis/*-setup.exe') -File -ErrorAction Stop)
    $msi = @(Get-ChildItem (Join-Path $root 'bundle/msi/*.msi') -File -ErrorAction Stop)
    if ($nsis.Count -ne 1 -or $msi.Count -ne 1) {
        throw 'Expected exactly one NSIS installer and one MSI. Use a clean release output directory.'
    }
    return @($app.FullName, $nsis[0].FullName, $msi[0].FullName)
}

function Get-ArtifactHashes {
    param([string[]]$Paths)
    return @(foreach ($path in $Paths) {
        [ordered]@{
            name = [IO.Path]::GetFileName($path)
            sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256 -ErrorAction Stop).Hash.ToLowerInvariant()
        }
    })
}

function Assert-ArtifactHashes {
    param($Expected, [string[]]$Paths)
    $current = @(Get-ArtifactHashes $Paths)
    if (@($Expected).Count -ne $current.Count) { throw 'Release artifact set changed after validation.' }
    foreach ($item in $current) {
        $match = @($Expected | Where-Object { $_.name -ceq $item.name -and $_.sha256 -ceq $item.sha256 })
        if ($match.Count -ne 1) { throw "Release artifact changed after validation: $($item.name)." }
    }
}

function Assert-DefenderReady {
    param($Status, $Preferences, [datetime]$Now = (Get-Date))
    if (!$Status.AMServiceEnabled -or !$Status.AntivirusEnabled -or
        !$Status.RealTimeProtectionEnabled -or $Status.AMRunningMode -ne 'Normal') {
        throw 'Defender must be active in Normal mode with antivirus and real-time protection enabled.'
    }
    $age = $Now.ToUniversalTime() - ([datetime]$Status.AntivirusSignatureLastUpdated).ToUniversalTime()
    if ($age.TotalHours -gt 24 -or $age.TotalMinutes -lt -5 -or !$Status.AntivirusSignatureVersion) {
        throw 'Defender security intelligence must be current (at most 24 hours old).'
    }
    foreach ($property in @('ExclusionPath', 'ExclusionExtension', 'ExclusionProcess', 'ThreatIDDefaultAction_Ids')) {
        if (@($Preferences.$property | Where-Object { $null -ne $_ -and "$_" -ne '' }).Count) {
            throw "Release scanning requires a clean runner without $property overrides."
        }
    }
    if ($Preferences.DisableArchiveScanning -or $Preferences.DisableRealtimeMonitoring) {
        throw 'Archive scanning and real-time protection must not be disabled.'
    }
}

function Write-ReleaseJson {
    param([string]$Path, $Value)
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding $false))
}
