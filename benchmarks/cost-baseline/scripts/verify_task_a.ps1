param (
    [Parameter(Mandatory=$true)]
    [string]$WorkspacePath
)

$ErrorActionPreference = "Stop"

Write-Host "=========================================="
Write-Host "Verifying Task A in: $WorkspacePath"
Write-Host "=========================================="

$envFile = Join-Path $WorkspacePath "projects/angular-ngrx-material-starter/src/environments/environment.ts"
if (-not (Test-Path $envFile)) {
    Write-Error "FAILED: Environment file not found at $envFile"
    exit 1
}

$content = Get-Content -Path $envFile -Raw

# 1. Functional check: appName updated to 'Angular NgRx Material Starter'
$expectedPattern = "appName:\s*['""]Angular NgRx Material Starter['""]"
$oldPattern = "appName:\s*['""]Angular Ngrx Material Starter['""]"

if ($content -match $expectedPattern) {
    Write-Host "[PASS] environment.appName has correct casing: 'Angular NgRx Material Starter'" -ForegroundColor Green
} elseif ($content -match $oldPattern) {
    Write-Error "FAILED: environment.appName still has old casing: 'Angular Ngrx Material Starter'"
    exit 1
} else {
    Write-Error "FAILED: environment.appName does not match expected pattern"
    exit 1
}

# 2. Scope check: git status in workspace
Push-Location $WorkspacePath
try {
    $status = git status --short
    $changedFiles = ($status | Where-Object { $_ -match '\S' }).Count
    Write-Host "Changed files detected by Git: $changedFiles"
    Write-Host $status

    # Filter out acceptable engram artifacts if present
    $actualCodeChanges = @($status | Where-Object { $_ -notmatch '\.engram' -and $_ -match '\S' })
    
    if ($actualCodeChanges.Count -ne 1) {
        Write-Error "FAILED: Diff scope violation. Expected exactly 1 modified file, found $($actualCodeChanges.Count)"
        exit 1
    }

    if ($actualCodeChanges[0] -notmatch 'environment\.ts') {
        Write-Error "FAILED: Modified file is not environment.ts: $($actualCodeChanges[0])"
        exit 1
    }

    Write-Host "[PASS] Diff scope strictly confined to environment.ts" -ForegroundColor Green
} finally {
    Pop-Location
}

Write-Host "=========================================="
Write-Host "Task A Verification: SUCCESS (ALL CHECKS PASSED)" -ForegroundColor Green
Write-Host "=========================================="
exit 0
