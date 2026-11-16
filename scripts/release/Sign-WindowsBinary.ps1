param([Parameter(Mandatory = $true)][string]$Path)
. "$PSScriptRoot/Release.Common.ps1"

Assert-SigningIdentity $env:WINDOWS_SIGNING_THUMBPRINT $env:WINDOWS_TIMESTAMP_URL
$file = (Get-Item -LiteralPath $Path -ErrorAction Stop).FullName
$tool = Get-SignTool
Invoke-ReleaseTool $tool @(
    'sign', '/sha1', $env:WINDOWS_SIGNING_THUMBPRINT, '/s', 'My',
    '/fd', 'SHA256', '/tr', $env:WINDOWS_TIMESTAMP_URL, '/td', 'SHA256', $file
)
$null = Test-ReleaseSignature $file $env:WINDOWS_SIGNING_THUMBPRINT $tool
