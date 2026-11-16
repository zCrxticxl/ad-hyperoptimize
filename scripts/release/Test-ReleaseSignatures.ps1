param(
    [string]$ReleaseDirectory = 'src-tauri/target/release',
    [string]$ReportDirectory = 'release-evidence'
)
. "$PSScriptRoot/Release.Common.ps1"
Assert-SigningIdentity $env:WINDOWS_SIGNING_THUMBPRINT $env:WINDOWS_TIMESTAMP_URL
$null = New-Item -ItemType Directory -Path $ReportDirectory -Force
$reportPath = Join-Path (Resolve-Path $ReportDirectory).Path 'authenticode.json'
$report = [ordered]@{ passed = $false; commit = $env:GITHUB_SHA; checkedAtUtc = [datetime]::UtcNow.ToString('o'); files = @(); error = $null }
try {
    $tool = Get-SignTool
    foreach ($file in (Get-ReleaseFiles $ReleaseDirectory)) {
        $report.files += Test-ReleaseSignature $file $env:WINDOWS_SIGNING_THUMBPRINT $tool
    }
    $report.passed = $true
} catch {
    $report.error = $_.Exception.Message
    throw
} finally {
    Write-ReleaseJson $reportPath $report
}
