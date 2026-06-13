Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot

function Has($cmd) { $null -ne (Get-Command $cmd -ErrorAction SilentlyContinue) }

Write-Host ""
Write-Host "=== AD HyperOptimize Build ===" -ForegroundColor Cyan

# 1. Node.js
if (-not (Has "node")) {
    Write-Host "[1/4] Installing Node.js..." -ForegroundColor Yellow
    winget install --id OpenJS.NodeJS.LTS --accept-source-agreements --accept-package-agreements -e
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
    if (-not (Has "node")) {
        Write-Host "Node.js install failed. Get it from https://nodejs.org" -ForegroundColor Red
        pause; exit 1
    }
} else {
    Write-Host "[1/4] Node.js $(node --version) OK" -ForegroundColor Green
}

# 2. Rust
if (-not (Has "cargo")) {
    Write-Host "[2/4] Installing Rust..." -ForegroundColor Yellow
    winget install --id Rustlang.Rustup --accept-source-agreements --accept-package-agreements -e
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User") + ";" + "$env:USERPROFILE\.cargo\bin"
    if (-not (Has "cargo")) {
        Write-Host "Rust install failed. Get it from https://rustup.rs" -ForegroundColor Red
        pause; exit 1
    }
    rustup target add x86_64-pc-windows-msvc 2>$null
} else {
    Write-Host "[2/4] Rust $(rustc --version) OK" -ForegroundColor Green
}

# 3. npm install
Write-Host "[3/4] npm install..." -ForegroundColor Yellow
Set-Location $Root
npm install --prefer-offline
if ($LASTEXITCODE -ne 0) { Write-Host "npm install failed" -ForegroundColor Red; pause; exit 1 }
Write-Host "      npm OK" -ForegroundColor Green

# 4. Build
Write-Host "[4/4] Building installer (first build takes ~5 min)..." -ForegroundColor Yellow
npm run tauri build
if ($LASTEXITCODE -ne 0) { Write-Host "Build failed" -ForegroundColor Red; pause; exit 1 }

# Done
$OutDir = Join-Path $Root "src-tauri\target\release\bundle"
Write-Host ""
Write-Host "BUILD COMPLETE" -ForegroundColor Green
Write-Host "Output: $OutDir" -ForegroundColor Cyan
Write-Host ""
if (Test-Path $OutDir) { Invoke-Item $OutDir }
pause
