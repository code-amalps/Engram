<#
.SYNOPSIS
    Engram 1-Click Installer for Windows (PowerShell)
.DESCRIPTION
    Downloads, cryptographically verifies SHA256 checksum, and safely installs or upgrades
    the standalone Engram executable into the user profile, adding it to PATH.
    Guarantees fail-closed integrity verification and zero in-use executable corruption.
.PARAMETER Version
    Specific release version tag to install (e.g. "1.2.0"). Defaults to "latest".
.PARAMETER InstallDir
    Target directory for the engram binary. Defaults to "$env:LOCALAPPDATA\Programs\Engram\bin".
.PARAMETER Token
    Optional GitHub personal access token for private repository installations.
    May also be supplied securely via the $env:GITHUB_TOKEN environment variable.
#>
[CmdletBinding()]
param(
    [string]$Version = "latest",
    [string]$InstallDir = "",
    [string]$Token = ""
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$Repo = "code-amalps/Engram"
$Arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { "unsupported" }

if ($Arch -ne "x86_64") {
    Write-Error "Engram Windows binary currently requires a 64-bit OS (x86_64). Windows ARM64 is not currently supported."
    exit 1
}

$TargetTriple = "x86_64-pc-windows-msvc"

# Resolve target directory
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    if ($env:ENGRAM_INSTALL_DIR) {
        $InstallDir = $env:ENGRAM_INSTALL_DIR
    } else {
        $InstallDir = [System.IO.Path]::Combine($env:LOCALAPPDATA, "Programs", "Engram", "bin")
    }
}

# Resolve GitHub token safely (never echoed or logged)
$AuthToken = if (-not [string]::IsNullOrWhiteSpace($Token)) {
    $Token
} elseif (-not [string]::IsNullOrWhiteSpace($env:GITHUB_TOKEN)) {
    $env:GITHUB_TOKEN
} else {
    ""
}

Write-Host "🧠 Engram Installer for Windows" -ForegroundColor Cyan
Write-Host "──────────────────────────────────────────────────────" -ForegroundColor DarkGray

# 1. Pre-flight In-Use Check: Prevent modifying or replacing locked binaries
$runningEngram = Get-Process -Name "engram*" -ErrorAction SilentlyContinue
if ($runningEngram) {
    $pids = ($runningEngram | ForEach-Object { "$($_.Id)" }) -join ", "
    Write-Error "Engram is currently running (PID $pids). Upgrades cannot proceed while the executable is in use.`nPlease close active terminals, IDE sessions, or MCP server tasks using Engram, and rerun the installer.`nExisting installation preserved at $InstallDir."
    exit 1
}

# 2. Resolve Release Metadata
$ApiUrl = if ($Version -eq "latest") {
    "https://api.github.com/repos/$Repo/releases/latest"
} else {
    $Tag = if ($Version.StartsWith("v")) { $Version } else { "v$Version" }
    "https://api.github.com/repos/$Repo/releases/tags/$Tag"
}

Write-Host "Resolving release ($Version)..." -ForegroundColor Yellow

$Headers = @{ 
    "User-Agent" = "Engram-Installer-PowerShell"
    "Accept"     = "application/vnd.github.v3+json"
}

if (-not [string]::IsNullOrWhiteSpace($AuthToken)) {
    $Headers["Authorization"] = "Bearer $AuthToken"
}

try {
    $Release = Invoke-RestMethod -Uri $ApiUrl -Headers $Headers -UseBasicParsing
} catch {
    $statusCode = $_.Exception.Response.StatusCode.value__
    if ($statusCode -eq 401 -or $statusCode -eq 403) {
        Write-Error "GitHub API authentication failed (HTTP $statusCode). If using a GitHub token, ensure it is active and has 'repo' access."
    } elseif ($statusCode -eq 404) {
        Write-Error "Release not found at $ApiUrl (HTTP 404). If '$Repo' is a private repository, provide a valid GitHub token via the Token parameter or `$env:GITHUB_TOKEN."
    } else {
        Write-Error "Failed to fetch release metadata from GitHub API: $($_.Exception.Message)"
    }
    exit 1
}

$Tag = $Release.tag_name
$ZipName = "engram-$Tag-$TargetTriple.zip"

Write-Host "Found release: $Tag" -ForegroundColor Green

# Locate Download URLs
$ZipAsset = $Release.assets | Where-Object { $_.name -eq $ZipName }
$SumsAsset = $Release.assets | Where-Object { $_.name -eq "SHA256SUMS" }

if (-not $ZipAsset) {
    Write-Error "Asset '$ZipName' was not found in release $Tag."
    exit 1
}

# 3. Fail-Closed Checksum Verification Requirement
if (-not $SumsAsset) {
    Write-Error "Integrity verification failed: 'SHA256SUMS' manifest was not published with release $Tag.`nRefusing to install unverified binary."
    exit 1
}

$TempDir = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), "engram_install_" + [System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $TempDir -Force | Out-Null

try {
    $TempZip = [System.IO.Path]::Combine($TempDir, $ZipName)
    $TempSums = [System.IO.Path]::Combine($TempDir, "SHA256SUMS")

    # Download archive
    Write-Host "Downloading $ZipName..." -ForegroundColor Cyan
    $DownloadHeaders = @{ "User-Agent" = "Engram-Installer-PowerShell" }
    if (-not [string]::IsNullOrWhiteSpace($AuthToken)) {
        $DownloadHeaders["Authorization"] = "Bearer $AuthToken"
        $DownloadHeaders["Accept"] = "application/octet-stream"
        Invoke-WebRequest -Uri $ZipAsset.url -Headers $DownloadHeaders -OutFile $TempZip -UseBasicParsing
    } else {
        Invoke-WebRequest -Uri $ZipAsset.browser_download_url -Headers $DownloadHeaders -OutFile $TempZip -UseBasicParsing
    }

    # Download SHA256SUMS
    Write-Host "Downloading release checksum manifest..." -ForegroundColor Cyan
    if (-not [string]::IsNullOrWhiteSpace($AuthToken)) {
        Invoke-WebRequest -Uri $SumsAsset.url -Headers $DownloadHeaders -OutFile $TempSums -UseBasicParsing
    } else {
        Invoke-WebRequest -Uri $SumsAsset.browser_download_url -Headers $DownloadHeaders -OutFile $TempSums -UseBasicParsing
    }

    # Verify Checksum Fail-Closed
    Write-Host "Verifying release checksum with SHA256SUMS..." -ForegroundColor Cyan
    $EscapedZip = [regex]::Escape($ZipName)
    $MatchingLines = Get-Content $TempSums | Where-Object { $_ -match "\b[0-9a-fA-F]{64}\s+(\./)?$EscapedZip\b" }

    if (-not $MatchingLines -or $MatchingLines.Count -eq 0) {
        Write-Error "Integrity verification failed: Could not find checksum entry for '$ZipName' in SHA256SUMS.`nRefusing to install unverified artifact."
        exit 1
    }
    if ($MatchingLines.Count -gt 1) {
        Write-Error "Integrity verification failed: Duplicate entries for '$ZipName' found in SHA256SUMS.`nManifest may be corrupt. Refusing to install."
        exit 1
    }

    $ExpectedHash = ($MatchingLines[0] -split "\s+")[0].Trim().ToLower()
    $ActualHash = (Get-FileHash -Path $TempZip -Algorithm SHA256).Hash.ToLower()

    if ($ExpectedHash -ne $ActualHash) {
        Write-Error "Integrity verification failed: SHA-256 checksum mismatch for $ZipName!`nExpected: $ExpectedHash`nActual:   $ActualHash`nRefusing to install corrupted or tampered binary."
        exit 1
    }
    Write-Host "Checksum verified ✓ ($ActualHash)" -ForegroundColor Green

    # 4. Staged Extraction & Pre-Installation Verification
    $StagingDir = [System.IO.Path]::Combine($TempDir, "staged")
    New-Item -ItemType Directory -Path $StagingDir -Force | Out-Null
    Expand-Archive -Path $TempZip -DestinationPath $StagingDir -Force

    $ExtractedBin = [System.IO.Path]::Combine($StagingDir, "engram.exe")
    if (-not (Test-Path $ExtractedBin)) {
        $Found = Get-ChildItem -Path $StagingDir -Filter "engram.exe" -Recurse | Select-Object -First 1
        if ($Found) { $ExtractedBin = $Found.FullName }
    }

    if (-not (Test-Path $ExtractedBin)) {
        Write-Error "Extraction failed: 'engram.exe' was not found inside the downloaded archive."
        exit 1
    }

    # Execute smoke-test on the staged binary BEFORE replacing installed binary
    Write-Host "Verifying staged binary integrity..." -ForegroundColor Cyan
    $StagedVersionOutput = & $ExtractedBin --version 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Staged binary verification failed: engram.exe returned non-zero exit code ($LASTEXITCODE).`nOutput: $StagedVersionOutput`nAborting installation. Existing installation preserved."
        exit 1
    }

    # 5. Safe Installation & Replacement
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    $DestBin = [System.IO.Path]::Combine($InstallDir, "engram.exe")
    $BackupBin = [System.IO.Path]::Combine($InstallDir, "engram.exe.bak")

    # If an existing binary exists, back it up
    $HadExisting = Test-Path $DestBin
    if ($HadExisting) {
        try {
            Copy-Item -Path $DestBin -Destination $BackupBin -Force -ErrorAction Stop
        } catch {
            Write-Warning "Could not create backup of existing binary. Proceeding with caution."
        }
    }

    try {
        Copy-Item -Path $ExtractedBin -Destination $DestBin -Force -ErrorAction Stop
        Write-Host "Installed engram.exe successfully ✓" -ForegroundColor Green
        if (Test-Path $BackupBin) {
            Remove-Item -Path $BackupBin -Force -ErrorAction SilentlyContinue
        }
    } catch {
        # Rollback if replacement failed
        if ($HadExisting -and (Test-Path $BackupBin)) {
            Copy-Item -Path $BackupBin -Destination $DestBin -Force -ErrorAction SilentlyContinue
            Write-Error "Installation failed: $($_.Exception.Message)`nRolled back to existing working binary."
        } else {
            Write-Error "Installation failed: $($_.Exception.Message)"
        }
        exit 1
    }

    # 6. Ensure PATH Environment Variable
    $UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    $Paths = $UserPath -split ";" | Where-Object { $_ -ne "" }
    if ($Paths -notcontains $InstallDir) {
        Write-Host "Adding $InstallDir to User PATH..." -ForegroundColor Yellow
        $NewUserPath = "$UserPath;$InstallDir"
        [Environment]::SetEnvironmentVariable("PATH", $NewUserPath, "User")
        Write-Host "Added to User PATH ✓" -ForegroundColor Green
    }
    
    $CurrentPath = $env:PATH -split ";" | Where-Object { $_ -ne "" }
    if ($CurrentPath -notcontains $InstallDir) {
        $env:PATH = "$env:PATH;$InstallDir"
    }

    # 7. Final Smoke Test
    Write-Host "──────────────────────────────────────────────────────" -ForegroundColor DarkGray
    Write-Host "Verifying installed command..." -ForegroundColor Cyan
    & $DestBin --version
    Write-Host "──────────────────────────────────────────────────────" -ForegroundColor DarkGray
    Write-Host "🎉 Engram $Tag installed successfully!" -ForegroundColor Green
    Write-Host "To connect your editor in any repository, run:" -ForegroundColor White
    Write-Host "  engram setup --apply" -ForegroundColor Yellow

} finally {
    if (Test-Path $TempDir) {
        Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
