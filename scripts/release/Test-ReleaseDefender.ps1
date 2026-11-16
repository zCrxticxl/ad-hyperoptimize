param(
    [string]$ReleaseDirectory = 'src-tauri/target/release',
    [string]$ReportDirectory = 'release-evidence'
)
. "$PSScriptRoot/Release.Common.ps1"
$null = New-Item -ItemType Directory -Path $ReportDirectory -Force
$reportRoot = (Resolve-Path $ReportDirectory).Path
$report = [ordered]@{
    passed = $false
    commit = $env:GITHUB_SHA
    startedAtUtc = [datetime]::UtcNow.ToString('o')
    completedAtUtc = $null
    engine = $null
    files = @()
    scans = @()
    error = $null
}
try {
    $paths = @(Get-ReleaseFiles $ReleaseDirectory)
    $report.files = @(Get-ArtifactHashes $paths)
    Update-MpSignature -ErrorAction Stop
    $status = Get-MpComputerStatus -ErrorAction Stop
    $preferences = Get-MpPreference -ErrorAction Stop
    Assert-DefenderReady $status $preferences
    $report.engine = [ordered]@{
        product = $status.AMProductVersion
        engine = $status.AMEngineVersion
        signatures = $status.AntivirusSignatureVersion
        signaturesUpdatedAtUtc = ([datetime]$status.AntivirusSignatureLastUpdated).ToUniversalTime().ToString('o')
    }
    $platforms = @(Get-ChildItem "$env:ProgramData/Microsoft/Windows Defender/Platform/*/MpCmdRun.exe" -ErrorAction Stop |
        Sort-Object { $_.VersionInfo.FileVersionRaw } -Descending)
    if (!$platforms.Count) { throw 'An up-to-date Defender platform with MpCmdRun.exe is required.' }
    $scanner = $platforms[0].FullName
    foreach ($file in $paths) {
        $name = [IO.Path]::GetFileName($file)
        $stdout = Join-Path $reportRoot "$name.defender.stdout.txt"
        $stderr = Join-Path $reportRoot "$name.defender.stderr.txt"
        # Custom scan, ignore file exclusions, scan archives and do not remediate.
        # Exit 0 from a remediating scan could mean malware was removed, not "clean".
        $process = Start-Process -FilePath $scanner -ArgumentList @(
            '-Scan', '-ScanType', '3', '-File', "`"$file`"", '-DisableRemediation'
        ) -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru -NoNewWindow
        # Retain the process handle so Windows PowerShell can read ExitCode
        # after a fast process exit as well as after WaitForExit.
        $null = $process.Handle
        $scan = [ordered]@{ name = $name; exitCode = $null; timedOut = $false }
        $report.scans += $scan
        if (!$process.WaitForExit(600000)) {
            $scan.timedOut = $true
            $process.Kill()
            $process.WaitForExit()
            throw "Defender scan exceeded 10 minutes: $name."
        }
        # Ensure redirected streams finish flushing before evidence is uploaded.
        $process.WaitForExit()
        $scan.exitCode = $process.ExitCode
        if ($scan.exitCode -ne 0) {
            throw "Defender detected a threat or could not finish scanning $name (exit $($scan.exitCode)). See scan logs."
        }
    }
    Assert-ArtifactHashes $report.files $paths
    Assert-DefenderReady (Get-MpComputerStatus -ErrorAction Stop) (Get-MpPreference -ErrorAction Stop)
    $report.passed = $true
} catch {
    $report.error = $_.Exception.Message
    throw
} finally {
    $report.completedAtUtc = [datetime]::UtcNow.ToString('o')
    Write-ReleaseJson (Join-Path $reportRoot 'defender.json') $report
}
