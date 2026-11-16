Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/../Release.Common.ps1"
$script:checks = 0

function Assert-Rejected {
    param([scriptblock]$Action)
    $rejected = $false
    try { & $Action } catch { $rejected = $true }
    if (!$rejected) { throw 'Expected the release gate to reject this input.' }
    $script:checks++
}

# Parse every release script without executing signing, scans or publication.
foreach ($file in Get-ChildItem "$PSScriptRoot/.." -Filter '*.ps1' -Recurse) {
    $tokens = $null
    $errors = $null
    $null = [Management.Automation.Language.Parser]::ParseFile($file.FullName, [ref]$tokens, [ref]$errors)
    if ($errors.Count) { throw ($errors | Out-String) }
    $script:checks++
}

$thumb = 'A' * 40
Assert-SigningIdentity $thumb 'https://timestamp.example.test'
Assert-Rejected { Assert-SigningIdentity '' 'https://timestamp.example.test' }
Assert-Rejected { Assert-SigningIdentity $thumb 'http://timestamp.example.test' }
Assert-Rejected { Assert-SigningIdentity $thumb 'https://user:password@timestamp.example.test' }
$signature = [pscustomobject]@{
    Status = 'Valid'
    SignerCertificate = [pscustomobject]@{ Thumbprint = $thumb }
    TimeStamperCertificate = [pscustomobject]@{ Thumbprint = 'B' * 40 }
    SignatureType = 'Authenticode'
}
Assert-AuthenticodeResult $signature $thumb 'fixture.exe'
foreach ($status in @('NotSigned', 'HashMismatch', 'NotTrusted', 'UnknownError')) {
    $bad = $signature.PSObject.Copy()
    $bad.Status = $status
    Assert-Rejected { Assert-AuthenticodeResult $bad $thumb 'fixture.exe' }
}
Assert-Rejected { Assert-AuthenticodeResult $signature ('C' * 40) 'fixture.exe' }
$bad = $signature.PSObject.Copy()
$bad.TimeStamperCertificate = $null
Assert-Rejected { Assert-AuthenticodeResult $bad $thumb 'fixture.exe' }
$bad = $signature.PSObject.Copy()
$bad.SignatureType = 'Catalog'
Assert-Rejected { Assert-AuthenticodeResult $bad $thumb 'fixture.exe' }

$now = Get-Date
$certificate = [pscustomobject]@{
    NotBefore = $now.AddDays(-1); NotAfter = $now.AddDays(1)
    EnhancedKeyUsageList = @([pscustomobject]@{ ObjectId = '1.3.6.1.5.5.7.3.3' })
}
Assert-CodeSigningCertificate $certificate $now
Assert-Rejected { Assert-CodeSigningCertificate $certificate $now.AddDays(2) }
Assert-Rejected { Assert-CodeSigningCertificate $certificate $now.AddDays(-2) }
$bad = $certificate.PSObject.Copy()
$bad.EnhancedKeyUsageList = @([pscustomobject]@{ ObjectId = '1.3.6.1.5.5.7.3.1' })
Assert-Rejected { Assert-CodeSigningCertificate $bad $now }
$publicKey = (Get-Content "$PSScriptRoot/../../../src-tauri/tauri.conf.json" -Raw | ConvertFrom-Json).plugins.updater.pubkey
$keyLines = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($publicKey)) -split '\r?\n'
$keyBytes = [Convert]::FromBase64String($keyLines[1])
$sigBytes = New-Object byte[] 74
$sigBytes[0] = 69
$sigBytes[1] = 68
[Array]::Copy($keyBytes, 2, $sigBytes, 2, 8)
$fixtureSignature = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(
    "untrusted comment: inert identity fixture`n$([Convert]::ToBase64String($sigBytes))`n"))
Assert-UpdaterSigningIdentity $fixtureSignature $publicKey
$sigBytes[2] = $sigBytes[2] -bxor 1
$wrongSignature = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(
    "untrusted comment: inert identity fixture`n$([Convert]::ToBase64String($sigBytes))`n"))
Assert-Rejected { Assert-UpdaterSigningIdentity $wrongSignature $publicKey }
Assert-Rejected { Assert-UpdaterSigningIdentity 'malformed' $publicKey }
$status = [pscustomobject]@{
    AMServiceEnabled = $true; AntivirusEnabled = $true
    RealTimeProtectionEnabled = $true; AMRunningMode = 'Normal'
    AntivirusSignatureLastUpdated = $now; AntivirusSignatureVersion = '1.2.3.4'
}
$preferences = [pscustomobject]@{
    ExclusionPath = @(); ExclusionExtension = @(); ExclusionProcess = @()
    ThreatIDDefaultAction_Ids = @(); DisableArchiveScanning = $false
    DisableRealtimeMonitoring = $false
}
Assert-DefenderReady $status $preferences $now
foreach ($field in @('AMServiceEnabled', 'AntivirusEnabled', 'RealTimeProtectionEnabled')) {
    $bad = $status.PSObject.Copy()
    $bad.$field = $false
    Assert-Rejected { Assert-DefenderReady $bad $preferences $now }
}
$bad = $status.PSObject.Copy()
$bad.AMRunningMode = 'Passive'
Assert-Rejected { Assert-DefenderReady $bad $preferences $now }
$bad = $status.PSObject.Copy()
$bad.AntivirusSignatureLastUpdated = $now.AddHours(-25)
Assert-Rejected { Assert-DefenderReady $bad $preferences $now }
foreach ($field in @('ExclusionPath', 'ExclusionExtension', 'ExclusionProcess', 'ThreatIDDefaultAction_Ids')) {
    $bad = $preferences.PSObject.Copy()
    $bad.$field = @('override')
    Assert-Rejected { Assert-DefenderReady $status $bad $now }
}
foreach ($field in @('DisableArchiveScanning', 'DisableRealtimeMonitoring')) {
    $bad = $preferences.PSObject.Copy()
    $bad.$field = $true
    Assert-Rejected { Assert-DefenderReady $status $bad $now }
}

$tempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { Join-Path $env:LOCALAPPDATA 'Temp/opencode' }
$fixture = Join-Path $tempRoot ('release-gate-test-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path "$fixture/bundle/nsis", "$fixture/bundle/msi" -Force
try {
    foreach ($relative in @('ad-hyperoptimize.exe', 'bundle/nsis/App-setup.exe', 'bundle/msi/App.msi')) {
        [IO.File]::WriteAllText((Join-Path $fixture $relative), 'inert test data')
    }
    $paths = @(Get-ReleaseFiles $fixture)
    if ($paths.Count -ne 3) { throw 'The app and both installers must be checked.' }
    $hashes = @(Get-ArtifactHashes $paths)
    Assert-ArtifactHashes $hashes $paths
    Assert-Rejected { Assert-ArtifactHashes $hashes $paths[0..1] }
    [IO.File]::AppendAllText($paths[0], ' changed after verification')
    Assert-Rejected { Assert-ArtifactHashes $hashes $paths }
    [IO.File]::WriteAllText("$fixture/bundle/msi/Stale.msi", 'inert')
    Assert-Rejected { Get-ReleaseFiles $fixture }
    Remove-Item -LiteralPath "$fixture/bundle/msi/Stale.msi"

    # Exercise the publication script with local command doubles. Neither npm
    # nor GitHub is executed, including for the otherwise-valid fixture.
    & {
        $saved = @{}
        foreach ($name in @('GITHUB_REPOSITORY', 'GITHUB_SHA', 'TAG_NAME', 'RUNNER_TEMP', 'RELEASE_NOTES')) {
            $saved[$name] = [Environment]::GetEnvironmentVariable($name)
        }
        try {
            $env:GITHUB_REPOSITORY = 'fixture/release'
            $env:GITHUB_SHA = 'fixture-commit'
            $env:TAG_NAME = 'v' + (Get-Content "$PSScriptRoot/../../../src-tauri/tauri.conf.json" -Raw | ConvertFrom-Json).version
            $env:RUNNER_TEMP = $fixture
            $env:RELEASE_NOTES = 'inert fixture'
            $hashes = @(Get-ArtifactHashes $paths)
            $report = @{
                passed = $true; commit = $env:GITHUB_SHA; files = $hashes
                completedAtUtc = [datetime]::UtcNow.ToString('o')
            }
            Write-ReleaseJson "$fixture/authenticode.json" $report
            $calls = @{ signer = 0; github = 0; mutate = $false; published = 0; assets = @(); scenario = '' }
            function npm.cmd {
                $calls.signer++
                $setup = $args[-1]
                [IO.File]::WriteAllText("$setup.sig", $fixtureSignature)
                if ($calls.mutate) { [IO.File]::AppendAllText($setup, 'tampered stage') }
                $global:LASTEXITCODE = 0
            }
            function gh {
                $calls.github++
                $global:LASTEXITCODE = 0
                if ($args[0] -eq 'release' -and $args[1] -eq 'create') { return }
                if ($args[0] -eq 'api' -and $args[1] -eq '--paginate') {
                    if ($calls.scenario -eq 'missing-draft') { return '[[]]' }
                    return "[[{`"id`":1,`"draft`":false,`"tag_name`":`"old`"}],[{`"id`":42,`"draft`":true,`"tag_name`":`"$env:TAG_NAME`"}]]"
                }
                if ($args[0] -eq 'release' -and $args[1] -eq 'upload') {
                    $calls.assets = @(foreach ($path in $args[3..($args.Count - 1)]) {
                        @{
                            name = [IO.Path]::GetFileName($path)
                            digest = 'sha256:' + (Get-FileHash $path).Hash.ToLowerInvariant()
                            browser_download_url = 'https://example.test/temporary-draft-url'
                        }
                    })
                    if ($calls.scenario -eq 'bad-digest') { $calls.assets[0].digest = 'sha256:wrong' }
                    return
                }
                if ($args[0] -eq 'api' -and $args[1] -eq "repos/$env:GITHUB_REPOSITORY/releases/42") {
                    return (@{ id = 42; tag_name = $env:TAG_NAME; draft = $true; assets = $calls.assets } | ConvertTo-Json -Depth 5)
                }
                if ($args[0] -eq 'api' -and $args[1] -eq '--method' -and $args[2] -eq 'PATCH' -and
                    $args[3] -eq "repos/$env:GITHUB_REPOSITORY/releases/42") {
                    $calls.published++
                    return
                }
                throw "Unexpected GitHub command: $args"
            }
            $publish = Join-Path $PSScriptRoot '../Publish-Release.ps1'
            foreach ($scenario in @('failed', 'wrong-commit', 'stale', 'changed-hash', 'valid', 'stage-mutated', 'missing-draft', 'bad-digest')) {
                $candidate = $report.Clone()
                switch ($scenario) {
                    'failed' { $candidate.passed = $false }
                    'wrong-commit' { $candidate.commit = 'other-commit' }
                    'stale' { $candidate.completedAtUtc = [datetime]::UtcNow.AddHours(-3).ToString('o') }
                    'changed-hash' { $candidate.files = @() }
                }
                Write-ReleaseJson "$fixture/defender.json" $candidate
                $calls.signer = 0
                $calls.github = 0
                $calls.published = 0
                $calls.scenario = $scenario
                $calls.mutate = $scenario -eq 'stage-mutated'
                if ($scenario -eq 'valid') {
                    & $publish -ReleaseDirectory $fixture -ReportDirectory $fixture
                    if ($calls.published -ne 1) { throw 'Verified draft was not published by ID.' }
                } else {
                    Assert-Rejected { & $publish -ReleaseDirectory $fixture -ReportDirectory $fixture }
                    if ($calls.published -ne 0) { throw "Unverified draft published: $scenario" }
                }
                $expectedSigner = if ($scenario -in @('valid', 'stage-mutated', 'missing-draft', 'bad-digest')) { 1 } else { 0 }
                $expectedGitHub = switch ($scenario) { 'valid' { 5 }; 'missing-draft' { 2 }; 'bad-digest' { 4 }; default { 0 } }
                if ($calls.signer -ne $expectedSigner -or $calls.github -ne $expectedGitHub) {
                    throw "Publication boundary failed for $scenario (signer=$($calls.signer), GitHub=$($calls.github))."
                }
            }
        } finally {
            foreach ($name in $saved.Keys) { [Environment]::SetEnvironmentVariable($name, $saved[$name]) }
        }
    }
} finally {
    Remove-Item -LiteralPath $fixture -Recurse -Force
}
Assert-Rejected { Invoke-ReleaseTool "$env:SystemRoot/System32/cmd.exe" @('/d', '/c', 'exit 7') }
Write-Host "Release gate checks passed: $script:checks rejection/parser checks and valid-input fixtures."
# The last native fixture intentionally exits 7. GitHub's pwsh wrapper reads
# LASTEXITCODE after this script, so explicitly report the successful test run.
exit 0
