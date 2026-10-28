<#
.SYNOPSIS
    Builds Engram release executable and packages the Windows Inno Setup installer.
.DESCRIPTION
    1. Compiles engram.exe in release mode via `cargo build --release`.
    2. Runs ISCC.exe (Inno Setup Compiler) to build `EngramSetup.exe` and `engram-setup.exe`
       in `dist/installer`.
#>
param(
    [string]$OutputDir = ""
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path "$PSScriptRoot\.."

Write-Host "🧠 Building Engram Release Binary..." -ForegroundColor Cyan
Set-Location $RepoRoot
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Error "Cargo build failed with code $LASTEXITCODE"
    exit 1
}

$IsccPath = "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
if (-not (Test-Path $IsccPath)) {
    $IsccCmd = Get-Command "ISCC.exe" -ErrorAction SilentlyContinue
    if ($IsccCmd) {
        $IsccPath = $IsccCmd.Source
    } else {
        $IsccPath = "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
    }
}

if (-not (Test-Path $IsccPath)) {
    Write-Error "Inno Setup compiler (ISCC.exe) was not found. Please install Inno Setup 6 (winget install JRSoftware.InnoSetup)."
    exit 1
}

Write-Host "📦 Compiling Engram Windows Installer with Inno Setup..." -ForegroundColor Cyan
$IssScript = Join-Path $RepoRoot "installer\engram.iss"
& $IsccPath $IssScript
if ($LASTEXITCODE -ne 0) {
    Write-Error "Inno Setup compiler failed with code $LASTEXITCODE"
    exit 1
}

$DistDir = Join-Path $RepoRoot "dist\installer"
$MainSetup = Join-Path $DistDir "EngramSetup.exe"
$LowerSetup = Join-Path $DistDir "engram-setup.exe"
if (Test-Path $MainSetup) {
    Copy-Item $MainSetup $LowerSetup -Force
}

Write-Host "🎉 Installer built successfully:" -ForegroundColor Green
Write-Host "   $MainSetup" -ForegroundColor White
Write-Host "   $LowerSetup" -ForegroundColor White
