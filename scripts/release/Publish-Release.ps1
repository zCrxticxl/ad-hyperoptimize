param(
    [string]$ReleaseDirectory = 'src-tauri/target/release',
    [string]$ReportDirectory = 'release-evidence'
)
. "$PSScriptRoot/Release.Common.ps1"
if (!$env:GITHUB_REPOSITORY -or !$env:GITHUB_SHA -or !$env:TAG_NAME) {
    throw 'Publication requires the GitHub release environment.'
}
$paths = @(Get-ReleaseFiles $ReleaseDirectory)
$auth = Get-Content (Join-Path $ReportDirectory 'authenticode.json') -Raw | ConvertFrom-Json
$defender = Get-Content (Join-Path $ReportDirectory 'defender.json') -Raw | ConvertFrom-Json
foreach ($report in @($auth, $defender)) {
    if ($report.passed -ne $true -or $report.commit -cne $env:GITHUB_SHA) {
        throw 'Both release gates must pass for this exact commit.'
    }
    Assert-ArtifactHashes $report.files $paths
}
$scanAge = [datetime]::UtcNow - [datetime]::Parse($defender.completedAtUtc).ToUniversalTime()
if ($scanAge.TotalHours -gt 2 -or $scanAge.TotalSeconds -lt 0) {
    throw 'Defender evidence is stale. Scan the final artifacts again.'
}
$conf = Get-Content 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json
if ($env:TAG_NAME -cne "v$($conf.version)") { throw 'Release tag/version mismatch.' }

# Build a private staging set. No GitHub release or upload exists before both gates pass.
$stage = Join-Path $env:RUNNER_TEMP ('adho-publish-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $stage
$installers = @()
foreach ($path in $paths[1..2]) {
    $name = [IO.Path]::GetFileName($path).Replace(' ', '.')
    if ($name -notmatch '^[A-Za-z0-9_.-]+$') { throw "Unsupported release asset name: $name." }
    $destination = Join-Path $stage $name
    Copy-Item -LiteralPath $path -Destination $destination -ErrorAction Stop
    if ((Get-FileHash $destination).Hash -ne (Get-FileHash -LiteralPath $path).Hash) { throw 'Staged copy differs.' }
    $installers += $destination
}
$stagedHashes = @(Get-ArtifactHashes $installers)
$setup = $installers[0]
$signArgs = @('run', 'tauri', '--', 'signer', 'sign')
if (!$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) { $signArgs += '--password=' }
Invoke-ReleaseTool 'npm.cmd' ($signArgs + @($setup))
Assert-ArtifactHashes $stagedHashes $installers
$signature = (Get-Content -LiteralPath "$setup.sig" -Raw -ErrorAction Stop).Trim()
if (!$signature) { throw 'Tauri updater signature is missing.' }
Assert-UpdaterSigningIdentity $signature $conf.plugins.updater.pubkey
$manifest = [ordered]@{
    version = $conf.version
    notes = $env:RELEASE_NOTES
    pub_date = [datetime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = @{
        'windows-x86_64' = @{
            signature = $signature
            url = "https://github.com/$($env:GITHUB_REPOSITORY)/releases/download/$($env:TAG_NAME)/$([IO.Path]::GetFileName($setup))"
        }
    }
}
Write-ReleaseJson (Join-Path $stage 'latest.json') $manifest
$checksums = @(Get-ArtifactHashes $installers | ForEach-Object { "$($_.sha256)  $($_.name)" })
[IO.File]::WriteAllLines((Join-Path $stage 'SHA256SUMS.txt'), [string[]]$checksums, (New-Object Text.UTF8Encoding $false))
$assets = @($installers) + @((Join-Path $stage 'SHA256SUMS.txt'), (Join-Path $stage 'latest.json'))
$assetHashes = @(Get-ArtifactHashes $assets)
$notesFile = Join-Path $stage 'notes.md'
[IO.File]::WriteAllText($notesFile, [string]$env:RELEASE_NOTES, (New-Object Text.UTF8Encoding $false))

# Never update an existing release or overwrite its assets. Failed attempts leave
# a draft for inspection, not a partially published updater release.
Assert-ArtifactHashes $auth.files $paths
Assert-ArtifactHashes $stagedHashes $installers
Assert-ArtifactHashes $assetHashes $assets
Invoke-ReleaseTool 'gh' @('release', 'create', $env:TAG_NAME, '--verify-tag', '--draft',
    '--title', "AD HyperOptimize $($env:TAG_NAME)", '--notes-file', $notesFile)
$pagesJson = & gh api --paginate --slurp "repos/$($env:GITHUB_REPOSITORY)/releases?per_page=100"
if ($LASTEXITCODE -ne 0) { throw 'Could not locate the newly created draft.' }
$drafts = @(foreach ($page in ($pagesJson | ConvertFrom-Json)) {
    foreach ($item in $page) {
        if ($item.tag_name -ceq $env:TAG_NAME) { $item }
    }
})
if ($drafts.Count -ne 1 -or !$drafts[0].draft -or "$($drafts[0].id)" -notmatch '^[0-9]+$') {
    throw 'Expected exactly one draft for the release tag.'
}
$releaseId = $drafts[0].id
Invoke-ReleaseTool 'gh' (@('release', 'upload', $env:TAG_NAME) + $assets)
# Tag lookup only supports published releases. Inspect and publish this draft by ID.
$releaseJson = & gh api "repos/$($env:GITHUB_REPOSITORY)/releases/$releaseId"
if ($LASTEXITCODE -ne 0) { throw 'Could not inspect uploaded draft assets.' }
$release = $releaseJson | ConvertFrom-Json
if (!$release.draft -or $release.tag_name -cne $env:TAG_NAME -or
    $release.id -ne $releaseId -or @($release.assets).Count -ne $assets.Count) {
    throw 'Expected a private draft with exactly the verified assets.'
}
foreach ($item in $assetHashes) {
    $asset = @($release.assets | Where-Object { $_.name -ceq $item.name })
    if ($asset.Count -ne 1 -or $asset[0].digest -cne "sha256:$($item.sha256)") {
        throw "GitHub asset digest differs or is unavailable: $($item.name). Draft remains unpublished."
    }
}
# Draft download URLs need not be the final tagged URLs. The manifest uses the
# canonical published URL, bound above to the exact tag and verified asset name.
Invoke-ReleaseTool 'gh' @('api', '--method', 'PATCH',
    "repos/$($env:GITHUB_REPOSITORY)/releases/$releaseId", '-F', 'draft=false', '-f', 'make_latest=true')
