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

Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "Engram Installer for Windows" -ForegroundColor Cyan
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "[Privacy & Local Governance Guarantee]" -ForegroundColor Green
Write-Host "  * 100% Local-First: Analysis and boundaries run strictly on your machine." -ForegroundColor DarkGray
Write-Host "  * Zero Exfiltration: Never uploads code, diffs, or prompts to any server." -ForegroundColor DarkGray
Write-Host "  * Zero Telemetry: No background tracking, usage beacons, or analytics." -ForegroundColor DarkGray
Write-Host "  * Factory.ai & MCP Compatible: Uses standard local stdio pipes." -ForegroundColor DarkGray
Write-Host "------------------------------------------------------" -ForegroundColor DarkGray

# 1. Pre-flight In-Use Check: Prevent modifying or replacing locked binaries
$runningEngram = Get-Process -Name "engram*" -ErrorAction SilentlyContinue
if ($runningEngram) {
    $pids = ($runningEngram | ForEach-Object { "$($_.Id)" }) -join ", "
    Write-Error "Engram is currently running (PID $pids). Upgrades cannot proceed while the executable is in use.`nPlease close active terminals, IDE sessions, or MCP server tasks using Engram, and rerun the installer.`nExisting installation preserved at $InstallDir."
    exit 1
}

# 2. Resolve Release Metadata with Resilient Fallback
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

$Tag = ""
$ZipAssetUrl = ""
$SumsAssetUrl = ""

try {
    $Release = Invoke-RestMethod -Uri $ApiUrl -Headers $Headers -UseBasicParsing
    $Tag = $Release.tag_name
    $ZipAsset = $Release.assets | Where-Object { $_.name -eq "engram-$Tag-$TargetTriple.zip" }
    $SumsAsset = $Release.assets | Where-Object { $_.name -eq "SHA256SUMS" }
    if ($ZipAsset) {
        $ZipAssetUrl = if (-not [string]::IsNullOrWhiteSpace($AuthToken)) { $ZipAsset.url } else { $ZipAsset.browser_download_url }
    }
    if ($SumsAsset) {
        $SumsAssetUrl = if (-not [string]::IsNullOrWhiteSpace($AuthToken)) { $SumsAsset.url } else { $SumsAsset.browser_download_url }
    }
} catch {
    # If GitHub REST API is rate-limited (HTTP 403) or blocked, fall back to direct web redirect resolution
    Write-Host "GitHub REST API unavailable (rate-limited or offline). Using direct release resolver..." -ForegroundColor Yellow
    if ($Version -eq "latest") {
        try {
            $webReq = [System.Net.HttpWebRequest]::Create("https://github.com/$Repo/releases/latest")
            $webReq.AllowAutoRedirect = $false
            $webReq.UserAgent = "Engram-Installer-PowerShell"
            $webResp = $webReq.GetResponse()
            $redirectLocation = $webResp.GetResponseHeader("Location")
            $webResp.Close()
            if ($redirectLocation -match "/releases/tag/(v?[0-9a-zA-Z\.\-]+)$") {
                $Tag = $Matches[1]
            }
        } catch {
            $Tag = "v1.5.2"
        }
    } else {
        $Tag = if ($Version.StartsWith("v")) { $Version } else { "v$Version" }
    }
}

if ([string]::IsNullOrWhiteSpace($Tag)) {
    $Tag = "v1.5.2"
}
$ZipName = "engram-$Tag-$TargetTriple.zip"
if ([string]::IsNullOrWhiteSpace($ZipAssetUrl)) {
    $ZipAssetUrl = "https://github.com/$Repo/releases/download/$Tag/$ZipName"
}
if ([string]::IsNullOrWhiteSpace($SumsAssetUrl)) {
    $SumsAssetUrl = "https://github.com/$Repo/releases/download/$Tag/SHA256SUMS"
}

Write-Host "Found release: $Tag" -ForegroundColor Green

# 3. Fail-Closed Checksum Verification Requirement
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
    }
    Invoke-WebRequest -Uri $ZipAssetUrl -Headers $DownloadHeaders -OutFile $TempZip -UseBasicParsing

    # Download SHA256SUMS
    Write-Host "Downloading release checksum manifest..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri $SumsAssetUrl -Headers $DownloadHeaders -OutFile $TempSums -UseBasicParsing

    # Verify Checksum Fail-Closed
    Write-Host "Verifying release checksum with SHA256SUMS..." -ForegroundColor Cyan
    $EscapedZip = [regex]::Escape($ZipName)
    $MatchingLines = @(Get-Content $TempSums | Where-Object { $_ -match "\b[0-9a-fA-F]{64}\s+(\./)?$EscapedZip\b" })

    if ($MatchingLines.Count -eq 0) {
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
    Write-Host "Checksum verified ($ActualHash)" -ForegroundColor Green

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
        Write-Host "Installed engram.exe successfully" -ForegroundColor Green
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

    # 6. Ensure PATH Environment Variable and Broadcast Update
    $UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    $Paths = $UserPath -split ";" | Where-Object { $_ -ne "" }
    if ($Paths -notcontains $InstallDir) {
        Write-Host "Adding $InstallDir to User PATH..." -ForegroundColor Yellow
        $NewUserPath = if ([string]::IsNullOrWhiteSpace($UserPath)) { $InstallDir } else { "$UserPath;$InstallDir" }
        [Environment]::SetEnvironmentVariable("PATH", $NewUserPath, "User")
        Write-Host "Added to User PATH" -ForegroundColor Green
    }
    
    $CurrentPath = $env:PATH -split ";" | Where-Object { $_ -ne "" }
    if ($CurrentPath -notcontains $InstallDir) {
        $env:PATH = "$env:PATH;$InstallDir"
    }

    # Broadcast WM_SETTINGCHANGE so Explorer, terminal sessions, and Factory.ai inherit the updated PATH
    try {
        if (-not ([System.Management.Automation.PSTypeName]'Win32Broadcast').Type) {
            $csharpDefinition = 'using System; using System.Runtime.InteropServices; public class Win32Broadcast { [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)] public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult); }'
            Add-Type -TypeDefinition $csharpDefinition
        }
        $HWND_BROADCAST = [IntPtr]0xffff
        $WM_SETTINGCHANGE = 0x001A
        $SMTO_ABORTIFHUNG = 0x0002
        $result = [UIntPtr]::Zero
        [Win32Broadcast]::SendMessageTimeout($HWND_BROADCAST, $WM_SETTINGCHANGE, [UIntPtr]::Zero, "Environment", $SMTO_ABORTIFHUNG, 3000, [ref]$result) | Out-Null
    } catch {
        # Non-fatal if broadcast fails on headless environments
    }

    # 7. Final Smoke Test
    Write-Host "------------------------------------------------------" -ForegroundColor DarkGray
    Write-Host "Verifying installed command..." -ForegroundColor Cyan
    & $DestBin --version
    Write-Host "------------------------------------------------------" -ForegroundColor DarkGray
    Write-Host "Engram $Tag installed successfully!" -ForegroundColor Green
    Write-Host "Binary Location: $DestBin" -ForegroundColor DarkGray
    Write-Host ""
    Write-Host "[For Factory.ai and Terminal Users]" -ForegroundColor White
    Write-Host "  * If Factory.ai was open, restart the app to refresh environment variables." -ForegroundColor Cyan
    Write-Host "  * To verify in terminal: engram --version" -ForegroundColor Cyan
    Write-Host "  * To connect your editor in any repository: engram setup --apply" -ForegroundColor Yellow

} finally {
    if (Test-Path $TempDir) {
        Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
