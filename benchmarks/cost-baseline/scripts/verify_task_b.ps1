param (
    [Parameter(Mandatory=$true)]
    [string]$WorkspacePath
)

$ErrorActionPreference = "Stop"

Write-Host "=========================================="
Write-Host "Verifying Task B (Remember-Me) in: $WorkspacePath"
Write-Host "=========================================="

$failures = @()
$warnings = @()

# 1. Inspect Changed Files Scope
Push-Location $WorkspacePath
try {
    $diffFiles = git diff --name-only HEAD
    $statusUntracked = git status --short | Where-Object { $_ -match '^\?\?' } | ForEach-Object { $_.Substring(3).Trim() }
} finally {
    Pop-Location
}

$allChanged = @(@($diffFiles) + @($statusUntracked) | Where-Object { $_ -notmatch '^\.engram' })

Write-Host "Total files modified/added (excluding .engram): $($allChanged.Count)"
foreach ($f in $allChanged) {
    Write-Host "  - $f"
}

# Scope Verification:
$allowedPatterns = @(
    "projects/angular-ngrx-material-starter/src/app/core/auth/.*",
    "projects/angular-ngrx-material-starter/src/app/app/app\.component\..*",
    "projects/angular-ngrx-material-starter/src/assets/i18n/en\.json"
)

$forbiddenFiles = @()
$i18nFiles = @()

foreach ($f in $allChanged) {
    if ($f -match "projects/angular-ngrx-material-starter/src/assets/i18n/(.+)\.json") {
        $i18nFiles += $f
    }

    $isAllowed = $false
    foreach ($pat in $allowedPatterns) {
        if ($f -match $pat) {
            $isAllowed = $true
            break
        }
    }
    if (-not $isAllowed) {
        $forbiddenFiles += $f
    }
}

if ($i18nFiles.Count -ge 3) {
    $failures += "Scope Drift: Bulk i18n modification detected ($($i18nFiles.Count) locale files modified). Only en.json or keys are allowed."
} else {
    Write-Host "[PASS] No bulk i18n sprawl detected ($($i18nFiles.Count) translation files touched)" -ForegroundColor Green
}

if ($forbiddenFiles.Count -gt 0) {
    $failures += "Scope Drift: Files modified outside allowed auth and header components: $($forbiddenFiles -join ', ')"
} else {
    Write-Host "[PASS] All changed files are within the authorized auth/app component boundary" -ForegroundColor Green
}

# 2. Functional & Architecture Checks
$authActionsFile = Join-Path $WorkspacePath "projects/angular-ngrx-material-starter/src/app/core/auth/auth.actions.ts"
$authEffectsFile = Join-Path $WorkspacePath "projects/angular-ngrx-material-starter/src/app/core/auth/auth.effects.ts"
$appComponentHtml = Join-Path $WorkspacePath "projects/angular-ngrx-material-starter/src/app/app/app.component.html"
$appComponentTs = Join-Path $WorkspacePath "projects/angular-ngrx-material-starter/src/app/app/app.component.ts"

if (-not (Test-Path $appComponentHtml)) {
    $failures += "Missing app.component.html"
} else {
    $htmlContent = Get-Content $appComponentHtml -Raw
    if ($htmlContent -match "rememberMe" -or $htmlContent -match "remember" -or $htmlContent -match "mat-checkbox") {
        Write-Host "[PASS] UI contains remember-me checkbox in login layout" -ForegroundColor Green
    } else {
        $failures += "UI Missing: No remember-me checkbox found in app.component.html"
    }

    if ($htmlContent -match "username" -or $htmlContent -match "user-input" -or $htmlContent -match "matInput") {
        Write-Host "[PASS] UI contains username input field in login layout" -ForegroundColor Green
    } else {
        $failures += "UI Missing: No username input field found in app.component.html"
    }
}

if (Test-Path $appComponentTs) {
    $tsContent = Get-Content $appComponentTs -Raw
    if ($tsContent -match "localStorage" -or $tsContent -match "getItem" -or $tsContent -match "AUTH_USERNAME") {
        Write-Host "[PASS] AppComponent implements username persistence restoration logic" -ForegroundColor Green
    } else {
        $warnings += "AppComponent does not directly restore username; checking AuthEffects"
    }
}

# 3. Security Check: No password or token persistence in localStorage
foreach ($file in @($appComponentTs, $authEffectsFile)) {
    if (Test-Path $file) {
        $code = Get-Content $file -Raw
        if ($code -match "localStorage.*setItem.*(?:password|token|secret|jwt)") {
            $failures += "SECURITY VIOLATION: Password, token, or secret is being persisted to storage in $file"
        }
    }
}
Write-Host "[PASS] Security check: No authentication passwords or tokens persisted" -ForegroundColor Green

# 4. Summary
Write-Host "=========================================="
if ($failures.Count -gt 0) {
    Write-Host "Task B Verification: FAILED ($($failures.Count) failures)" -ForegroundColor Red
    foreach ($err in $failures) {
        Write-Host "  - $err" -ForegroundColor Red
    }
    exit 1
} else {
    Write-Host "Task B Verification: SUCCESS (ALL CRITERIA PASSED)" -ForegroundColor Green
    if ($warnings.Count -gt 0) {
        foreach ($w in $warnings) {
            Write-Host "  [NOTE] $w" -ForegroundColor Yellow
        }
    }
    exit 0
}
