<#
.SYNOPSIS
    Engram 1-Click Installer for Windows (PowerShell)
.DESCRIPTION
    Downloads, verifies SHA256 checksum, and installs the latest official standalone
    Engram executable into the user profile, adding it to PATH.
.PARAMETER Version
    Specific release version tag to install (e.g. "0.3.0"). Defaults to "latest".
.PARAMETER InstallDir
    Target directory for the engram binary. Defaults to "$env:LOCALAPPDATA\Programs\Engram\bin".
#>
[CmdletBinding()]
param(
    [string]$Version = "latest",
    [string]$InstallDir = ""
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$Repo = "code-amalps/Engram"
$Arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { "unsupported" }

if ($Arch -ne "x86_64") {
    Write-Error "Engram Windows binary currently requires a 64-bit OS (x86_64)."
    exit 1
}

$TargetTriple = "x86_64-pc-windows-msvc"

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    if ($env:ENGRAM_INSTALL_DIR) {
        $InstallDir = $env:ENGRAM_INSTALL_DIR
    } else {
        $InstallDir = [System.IO.Path]::Combine($env:LOCALAPPDATA, "Programs", "Engram", "bin")
    }
}

Write-Host "🧠 Engram Installer for Windows" -ForegroundColor Cyan
Write-Host "──────────────────────────────────────────────────────" -ForegroundColor DarkGray

# 1. Resolve Release Metadata
$ApiUrl = if ($Version -eq "latest") {
    "https://api.github.com/repos/$Repo/releases/latest"
} else {
    $Tag = if ($Version.StartsWith("v")) { $Version } else { "v$Version" }
    "https://api.github.com/repos/$Repo/releases/tags/$Tag"
}

Write-Host "Resolving release ($Version)..." -ForegroundColor Yellow
$Headers = @{ "User-Agent" = "Engram-Installer-PowerShell" }

try {
    $Release = Invoke-RestMethod -Uri $ApiUrl -Headers $Headers -UseBasicParsing
} catch {
    Write-Error "Failed to fetch release metadata from $ApiUrl. Verify internet connection or release tag."
    exit 1
}

$Tag = $Release.tag_name
$CleanVersion = $Tag.TrimStart('v')
$ZipName = "engram-$Tag-$TargetTriple.zip"

Write-Host "Found release: $Tag" -ForegroundColor Green

# Locate Download URLs
$ZipAsset = $Release.assets | Where-Object { $_.name -eq $ZipName }
$SumsAsset = $Release.assets | Where-Object { $_.name -eq "SHA256SUMS" }

if (-not $ZipAsset) {
    Write-Error "Asset $ZipName not found in release $Tag."
    exit 1
}

$TempDir = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), "engram_install_" + [System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $TempDir -Force | Out-Null

try {
    $TempZip = [System.IO.Path]::Combine($TempDir, $ZipName)
    $TempSums = [System.IO.Path]::Combine($TempDir, "SHA256SUMS")

    Write-Host "Downloading $ZipName..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri $ZipAsset.browser_download_url -OutFile $TempZip -UseBasicParsing

    if ($SumsAsset) {
        Write-Host "Verifying release checksum with SHA256SUMS..." -ForegroundColor Cyan
        Invoke-WebRequest -Uri $SumsAsset.browser_download_url -OutFile $TempSums -UseBasicParsing

        $ExpectedLine = Get-Content $TempSums | Where-Object { $_ -match $ZipName }
        if ($ExpectedLine) {
            $ExpectedHash = ($ExpectedLine -split "\s+")[0].Trim().ToLower()
            $ActualHash = (Get-FileHash -Path $TempZip -Algorithm SHA256).Hash.ToLower()

            if ($ExpectedHash -ne $ActualHash) {
                Write-Error "Checksum verification failed!`nExpected: $ExpectedHash`nActual:   $ActualHash"
                exit 1
            }
            Write-Host "Checksum verified ✓ ($ActualHash)" -ForegroundColor Green
        } else {
            Write-Warning "Could not find $ZipName in SHA256SUMS. Proceeding with caution."
        }
    } else {
        Write-Warning "SHA256SUMS asset not present in release. Skipping checksum validation."
    }

    # 2. Extract and Install
    Write-Host "Installing to: $InstallDir" -ForegroundColor Cyan
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    Expand-Archive -Path $TempZip -DestinationPath $TempDir -Force
    $ExtractedBin = [System.IO.Path]::Combine($TempDir, "engram.exe")
    if (-not (Test-Path $ExtractedBin)) {
        # Search recursively if nested
        $Found = Get-ChildItem -Path $TempDir -Filter "engram.exe" -Recurse | Select-Object -First 1
        if ($Found) { $ExtractedBin = $Found.FullName }
    }

    if (-not (Test-Path $ExtractedBin)) {
        Write-Error "engram.exe was not found inside the downloaded archive."
        exit 1
    }

    $DestBin = [System.IO.Path]::Combine($InstallDir, "engram.exe")
    Copy-Item -Path $ExtractedBin -Destination $DestBin -Force
    Write-Host "Installed engram.exe successfully ✓" -ForegroundColor Green

    # 3. Ensure PATH Environment Variable
    $UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    $Paths = $UserPath -split ";" | Where-Object { $_ -ne "" }
    if ($Paths -notcontains $InstallDir) {
        Write-Host "Adding $InstallDir to User PATH..." -ForegroundColor Yellow
        $NewUserPath = "$UserPath;$InstallDir"
        [Environment]::SetEnvironmentVariable("PATH", $NewUserPath, "User")
        Write-Host "Added to User PATH ✓" -ForegroundColor Green
    }
    
    # Also add to current process environment so verification works immediately
    $CurrentPath = $env:PATH -split ";" | Where-Object { $_ -ne "" }
    if ($CurrentPath -notcontains $InstallDir) {
        $env:PATH = "$env:PATH;$InstallDir"
    }

    # 4. Smoke Test
    Write-Host "──────────────────────────────────────────────────────" -ForegroundColor DarkGray
    Write-Host "Verifying installation..." -ForegroundColor Cyan
    & $DestBin --version
    Write-Host "──────────────────────────────────────────────────────" -ForegroundColor DarkGray
    Write-Host "🎉 Engram $Tag installed successfully!" -ForegroundColor Green
    Write-Host "To get started in any repository, run:" -ForegroundColor White
    Write-Host "  engram init" -ForegroundColor Yellow
    Write-Host "  engram setup cursor  # or claude, windsurf" -ForegroundColor Yellow
    Write-Host "  engram bench" -ForegroundColor Yellow

} finally {
    if (Test-Path $TempDir) {
        Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
