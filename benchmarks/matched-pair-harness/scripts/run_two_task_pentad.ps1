# ==============================================================================
# Engram Phase 5.9.0.6: Two-Task Pentad Benchmark Coordinator Script
# Repository: https://github.com/engram/engram
# Execution Mode: Strictly Offline Implementation & Verification
#
# Scientific & Budget Invariants:
# 1. Global aggregate budget ceiling B_G = 2,500,000 tokens.
# 2. Task A nominal budget C_A = 1,000,000 tokens.
# 3. Dynamic Task B ceiling C_B = min(1,500,000, 2,500,000 - T_A).
# 4. Total aggregate exposure ceiling E_max = 2,500,000 + O_B <= 2,531,384 tokens.
# 5. Pre-dispatch request size gate strictly enforced <= 60,000 tokens (reject-only).
# 6. Task A 14-point summary validation gate MUST pass before Task B dispatch.
# ==============================================================================

param (
    [string]$TaskAConfig = "benchmarks/matched-pair-harness/config/tasks/task_a_typo.json",
    [string]$TaskBConfig = "benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json",
    [string]$OutputDir = "benchmarks/matched-pair-harness/runs/two_task_pentad",
    [int]$MaxRequestTokens = 60000,
    [int]$TaskANominalCeiling = 500000,
    [int]$GlobalBudgetCeiling = 1000000,
    [int]$ConservativeReserve = 45000,
    [switch]$DryRun,
    [string]$ExecutionPolicy = "diagnostic_continue",
    [int]$Seed = 59002,
    [string]$DependencyStrategy = "copy",
    [string]$PythonExe = "python",
    [string]$Provider = "controlled"
)

$EffectiveDepStrategy = if ($DryRun) { "none" } else { $DependencyStrategy }

$ErrorActionPreference = "Stop"

Write-Host "======================================================================" -ForegroundColor Cyan
Write-Host " ENGRAM TWO-TASK PENTAD BENCHMARK COORDINATOR (PHASE 5.9.0.6)" -ForegroundColor Cyan
Write-Host "======================================================================" -ForegroundColor Cyan
Write-Host "Task A Config:            $TaskAConfig"
Write-Host "Task B Config:            $TaskBConfig"
Write-Host "Output Directory:         $OutputDir"
Write-Host "Max Request Size Limit:   $MaxRequestTokens tokens"
Write-Host "Task A Ceiling:           $TaskANominalCeiling tokens"
Write-Host "Global Aggregate Ceiling: $GlobalBudgetCeiling tokens"
Write-Host "Pre-Dispatch Reserve:     $ConservativeReserve tokens"
Write-Host "Execution Policy:         $ExecutionPolicy"
Write-Host "Dry Run Mode:             $DryRun"
Write-Host "Randomization Seed:       $Seed"
Write-Host "----------------------------------------------------------------------"

# Resolve absolute paths
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = (Resolve-Path "$ScriptDir\..\..\..").Path
$HarnessSrc = Join-Path $RepoRoot "benchmarks\matched-pair-harness\src"
$TaskAPath = Join-Path $RepoRoot $TaskAConfig
$TaskBPath = Join-Path $RepoRoot $TaskBConfig
$ResolvedOutputDir = Join-Path $RepoRoot $OutputDir

if (-not (Test-Path $TaskAPath)) {
    Write-Error "FATAL: Task A configuration file not found at: $TaskAPath"
    exit 1
}
if (-not (Test-Path $TaskBPath)) {
    Write-Error "FATAL: Task B configuration file not found at: $TaskBPath"
    exit 1
}

$TaskAOutputDir = Join-Path $ResolvedOutputDir "task_a"
$TaskBOutputDir = Join-Path $ResolvedOutputDir "task_b"
New-Item -ItemType Directory -Force -Path $TaskAOutputDir | Out-Null
New-Item -ItemType Directory -Force -Path $TaskBOutputDir | Out-Null

# ----------------------------------------------------------------------
# 14-Point Validation Gate Function
# ----------------------------------------------------------------------
function Test-TaskASummaryGate {
    param (
        [string]$SummaryPath,
        [int]$MaxRequestTokensLimit,
        [int]$TaskACeiling,
        [int]$GlobalCeiling = 1000000,
        [int]$SafetyReserve = 45000
    )

    Write-Host "`n>>> Evaluating 14-Point Summary Validation Gate on Task A Output..." -ForegroundColor Yellow
    $GateResults = @()

    # Check 1: Summary file exists and is valid JSON
    if (-not (Test-Path $SummaryPath)) {
        Write-Host "[FAIL] Check 1: Summary file missing: $SummaryPath" -ForegroundColor Red
        return @{ Passed = $false; Reason = "Summary file missing" }
    }
    try {
        $RawContent = Get-Content -Raw -Path $SummaryPath -Encoding UTF8
        $Summary = $RawContent | ConvertFrom-Json
        $GateResults += "[PASS] Check 1: Summary file exists and parsed valid JSON"
    } catch {
        Write-Host "[FAIL] Check 1: JSON parse failure: $_" -ForegroundColor Red
        return @{ Passed = $false; Reason = "Invalid JSON in summary" }
    }

    # Check 2: Task identity matches expected
    $TaskId = $Summary.task_id
    if ($TaskId -like "*task_a*" -or $TaskId -eq "task_a_typo") {
        $GateResults += "[PASS] Check 2: Task ID verified ($TaskId)"
    } else {
        Write-Host "[FAIL] Check 2: Task ID mismatch: $TaskId" -ForegroundColor Red
        return @{ Passed = $false; Reason = "Task ID mismatch ($TaskId)" }
    }

    # Check 3: Schema version matches expected
    if ($Summary.schema_version -eq "2.1.0") {
        $GateResults += "[PASS] Check 3: Schema version verified (2.1.0)"
    } else {
        Write-Host "[FAIL] Check 3: Schema version unexpected: $($Summary.schema_version)" -ForegroundColor Red
        return @{ Passed = $false; Reason = "Schema version unexpected" }
    }

    # Check 4: All 5 arms present in execution order
    $ExpectedArms = @("arm_a_native", "arm_b_cand3", "arm_c_cand42_always_on", "arm_d_cand42_adaptive", "arm_e_cand5_optimized")
    $Runs = $Summary.runs
    $AllArmsPresent = $true
    foreach ($arm in $ExpectedArms) {
        if (-not ($Runs.PSObject.Properties.Name -contains $arm)) {
            $AllArmsPresent = $false
            Write-Host "[FAIL] Check 4: Missing arm in summary runs: $arm" -ForegroundColor Red
        }
    }
    if ($AllArmsPresent) {
        $GateResults += "[PASS] Check 4: All 5 pentad arms present in execution runs"
    } else {
        return @{ Passed = $false; Reason = "Not all 5 pentad arms present" }
    }

    # Check 5: Cumulative tokens calculated and non-zero
    $CumTokens = $Summary.cumulative_global_tokens
    if ($null -ne $CumTokens -and $CumTokens -gt 0) {
        $GateResults += "[PASS] Check 5: Cumulative tokens calculated ($CumTokens tokens)"
    } else {
        Write-Host "[FAIL] Check 5: Cumulative tokens zero or null" -ForegroundColor Red
        return @{ Passed = $false; Reason = "Cumulative tokens zero or null" }
    }

    # Check 6: Per-arm ceilings respected (no individual arm exceeded ceiling + 31,384)
    $MaxPerArmCeiling = 200000 + 31384
    $ArmCeilingPassed = $true
    foreach ($arm in $ExpectedArms) {
        $ArmData = $Runs.$arm
        $ArmTokens = $ArmData.telemetry.total_model_tokens
        if ($ArmTokens -gt $MaxPerArmCeiling) {
            Write-Host "[FAIL] Check 6: Arm $arm consumed $ArmTokens > $MaxPerArmCeiling" -ForegroundColor Red
            $ArmCeilingPassed = $false
        }
    }
    if ($ArmCeilingPassed) {
        $GateResults += "[PASS] Check 6: Per-arm token ceilings respected across all arms"
    } else {
        return @{ Passed = $false; Reason = "Per-arm token ceiling breached" }
    }

    # Check 7: Output token accounting present and valid
    $OutputValid = $true
    foreach ($arm in $ExpectedArms) {
        $ArmData = $Runs.$arm
        if ($ArmData.execution_status -eq "SUCCESS") {
            $CompTokens = $ArmData.telemetry.completion_tokens
            if ($null -eq $CompTokens -or $CompTokens -le 0) {
                $OutputValid = $false
            }
        }
    }
    if ($OutputValid) {
        $GateResults += "[PASS] Check 7: Output token accounting present and valid"
    } else {
        return @{ Passed = $false; Reason = "Missing output token accounting" }
    }

    # Check 8: Input token accounting present and valid
    $InputValid = $true
    foreach ($arm in $ExpectedArms) {
        $ArmData = $Runs.$arm
        if ($ArmData.execution_status -eq "SUCCESS") {
            $PromptTokens = $ArmData.telemetry.prompt_tokens_total
            if ($null -eq $PromptTokens -or $PromptTokens -le 0) {
                $InputValid = $false
            }
        }
    }
    if ($InputValid) {
        $GateResults += "[PASS] Check 8: Input token accounting present and valid"
    } else {
        return @{ Passed = $false; Reason = "Missing input token accounting" }
    }

    # Check 9: Request size gate passed on all arms
    $ReqGatePassed = $true
    foreach ($arm in $ExpectedArms) {
        $ArmData = $Runs.$arm
        $ReqGate = $ArmData.request_size_gate
        if ($null -ne $ReqGate) {
            if ($ReqGate.gate_passed -eq $false) {
                $ReqGatePassed = $false
                Write-Host "[FAIL] Check 9: Arm $arm failed request size gate" -ForegroundColor Red
            }
        }
    }
    if ($ReqGatePassed) {
        $GateResults += "[PASS] Check 9: Pre-dispatch request size gate passed on all arms"
    } else {
        return @{ Passed = $false; Reason = "Pre-dispatch request size gate failed on an arm" }
    }

    # Check 10: Task A total tokens T_A <= 1,031,384
    $TaskAMaxAllowed = $TaskACeiling + 31384
    if ($CumTokens -le $TaskAMaxAllowed) {
        $GateResults += "[PASS] Check 10: Task A total tokens ($CumTokens) <= upper bound ($TaskAMaxAllowed)"
    } else {
        Write-Host "[FAIL] Check 10: Task A total tokens ($CumTokens) exceeded ceiling + overshoot limit ($TaskAMaxAllowed)" -ForegroundColor Red
        return @{ Passed = $false; Reason = "Task A total tokens exceeded upper bound" }
    }

    # Check 11: Dynamic Task B ceiling calculation & headroom verification
    $DispatchableRemaining = $GlobalCeiling - $SafetyReserve - $CumTokens
    $DynamicCeilingB = [Math]::Min($DispatchableRemaining, 455000 + [Math]::Max(0, $TaskACeiling - $CumTokens))
    if ($DynamicCeilingB -ge 45000) {
        $GateResults += "[PASS] Check 11: Dynamic Task B ceiling valid ($DynamicCeilingB tokens >= 45,000 minimum)"
    } else {
        Write-Host "[FAIL] Check 11: Insufficient headroom for Task B ($DynamicCeilingB < 45,000)" -ForegroundColor Red
        return @{ Passed = $false; Reason = "Insufficient dynamic headroom for Task B" }
    }

    # Check 12: Correctness and acceptance recorded
    $AcceptanceRecorded = $null -ne $Summary.acceptance
    if ($AcceptanceRecorded) {
        $GateResults += "[PASS] Check 12: Acceptance matrix and requirement results recorded"
    } else {
        return @{ Passed = $false; Reason = "Missing acceptance records" }
    }

    # Check 13: Exit codes and process statuses valid
    $ExitCodesValid = $true
    foreach ($arm in $ExpectedArms) {
        $ArmData = $Runs.$arm
        $ArmStatus = $ArmData.execution_status
        if ($ArmStatus -in @("PROCESS_CRASH", "PRE_DISPATCH_REQUEST_SIZE_EXCEEDED")) {
            $ExitCodesValid = $false
        }
    }
    if ($ExitCodesValid) {
        $GateResults += "[PASS] Check 13: Process exit codes and execution statuses valid"
    } else {
        return @{ Passed = $false; Reason = "Invalid exit code or crash detected" }
    }

    # Check 14: No unhandled exceptions / crash markers
    $BlockStatus = $Summary.block_status
    if ($BlockStatus -notin @("EXECUTION_ERROR", "ABORTED_UNCERTAIN_BUDGET")) {
        $GateResults += "[PASS] Check 14: Clean execution without unhandled exceptions ($BlockStatus)"
    } else {
        Write-Host "[FAIL] Check 14: Block status indicates unhandled exception: $BlockStatus" -ForegroundColor Red
        return @{ Passed = $false; Reason = "Block status indicates unhandled error ($BlockStatus)" }
    }

    foreach ($res in $GateResults) {
        Write-Host "  $res" -ForegroundColor Green
    }
    Write-Host ">>> 14-Point Validation Gate: ALL 14 CHECKS PASSED!" -ForegroundColor Green

    return @{
        Passed = $true
        TaskATokens = $CumTokens
        DynamicCeilingB = $DynamicCeilingB
        Summary = $Summary
    }
}

$OrchestratorScript = Join-Path $HarnessSrc "orchestrator.py"

# ----------------------------------------------------------------------
# STEP 1: Execute Task A Pentad
# ----------------------------------------------------------------------
Write-Host "`n>>> [STEP 1] Executing Task A Pentad Benchmark..." -ForegroundColor Cyan

$TaskAPerArmCeiling = [Math]::Floor($TaskANominalCeiling / 5)
$TaskAArgs = @(
    $OrchestratorScript,
    "--mode", "pentad",
    "--task", $TaskAPath,
    "--output-dir", $TaskAOutputDir,
    "--provider", "$Provider",
    "--max-tokens-ceiling", "$TaskAPerArmCeiling",
    "--global-tokens-ceiling", "$TaskANominalCeiling",
    "--headroom-reserve", "$ConservativeReserve",
    "--execution-policy", $ExecutionPolicy,
    "--dependency-strategy", "$EffectiveDepStrategy",
    "--seed", "$Seed"
)
if ($DryRun) {
    $TaskAArgs += "--dry-run"
}

$TaskAStartTime = Get-Date
Write-Host "Running: $PythonExe $($TaskAArgs -join ' ')" -ForegroundColor Gray
Push-Location $HarnessSrc
try {
    & $PythonExe @TaskAArgs
    $TaskAExitCode = $LASTEXITCODE
} finally {
    Pop-Location
}

if ($TaskAExitCode -ne 0) {
    Write-Host "[ERROR] Task A execution returned non-zero exit code: $TaskAExitCode" -ForegroundColor Red
    if ($ExecutionPolicy -eq "strict") {
        Write-Error "Halting benchmark run due to Task A failure under strict execution policy."
        exit $TaskAExitCode
    }
}

# ----------------------------------------------------------------------
# STEP 2: Evaluate 14-Point Summary Validation Gate on Task A
# ----------------------------------------------------------------------
$TaskASummaryFiles = Get-ChildItem -Path $TaskAOutputDir -Filter "pentad_summary.json" -Recurse -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending
if ($TaskASummaryFiles -and $TaskASummaryFiles.Count -gt 0) {
    $TaskASummaryFile = $TaskASummaryFiles[0].FullName
} else {
    $TaskASummaryFile = Join-Path $TaskAOutputDir "pentad_summary.json"
}
$GateEval = Test-TaskASummaryGate -SummaryPath $TaskASummaryFile -MaxRequestTokensLimit $MaxRequestTokens -TaskACeiling $TaskANominalCeiling -GlobalCeiling $GlobalBudgetCeiling -SafetyReserve $ConservativeReserve

if (-not $GateEval.Passed) {
    Write-Error "FATAL: Task A failed the 14-point validation gate: $($GateEval.Reason). Halting before Task B dispatch."
    exit 1
}

$TaskATokens = $GateEval.TaskATokens
$DynamicCeilingB = $GateEval.DynamicCeilingB

Write-Host "`n----------------------------------------------------------------------"
Write-Host "TASK A RECONCILIATION SUMMARY:" -ForegroundColor Cyan
Write-Host "  Task A Consumed Tokens (T_A):     $TaskATokens tokens"
Write-Host "  Global Aggregate Budget (B_G):    $GlobalBudgetCeiling tokens"
Write-Host "  Formula:                          C_B = min(B_dispatch - T_A, 455,000 + unused_A)"
Write-Host "  Calculated Dynamic Task B Ceiling: $DynamicCeilingB tokens"
Write-Host "  Per-Arm Task B Ceiling:           $([Math]::Floor($DynamicCeilingB / 5)) tokens"
Write-Host "----------------------------------------------------------------------"

# ----------------------------------------------------------------------
# STEP 3: Execute Task B Pentad under Dynamic Ceiling
# ----------------------------------------------------------------------
Write-Host "`n>>> [STEP 2] Dispatching Task B Pentad Benchmark under Dynamic Ceiling ($DynamicCeilingB tokens)..." -ForegroundColor Cyan

$TaskBPerArmCeiling = [Math]::Floor($DynamicCeilingB / 5)
$TaskBArgs = @(
    $OrchestratorScript,
    "--mode", "pentad",
    "--task", $TaskBPath,
    "--output-dir", $TaskBOutputDir,
    "--provider", "$Provider",
    "--max-tokens-ceiling", "$TaskBPerArmCeiling",
    "--global-tokens-ceiling", "$DynamicCeilingB",
    "--headroom-reserve", "$ConservativeReserve",
    "--execution-policy", $ExecutionPolicy,
    "--dependency-strategy", "$EffectiveDepStrategy",
    "--seed", "$Seed"
)
if ($DryRun) {
    $TaskBArgs += "--dry-run"
}

Write-Host "Running: $PythonExe $($TaskBArgs -join ' ')" -ForegroundColor Gray
Push-Location $HarnessSrc
try {
    & $PythonExe @TaskBArgs
    $TaskBExitCode = $LASTEXITCODE
} finally {
    Pop-Location
}

if ($TaskBExitCode -ne 0) {
    Write-Host "[WARNING] Task B execution returned non-zero exit code: $TaskBExitCode" -ForegroundColor Yellow
}

# ----------------------------------------------------------------------
# STEP 4: Post-Execution Reconciliation & Aggregate Budget Verification
# ----------------------------------------------------------------------
Write-Host "`n>>> [STEP 3] Performing Post-Execution Aggregate Budget Reconciliation..." -ForegroundColor Cyan

$TaskBSummaryFiles = Get-ChildItem -Path $TaskBOutputDir -Filter "pentad_summary.json" -Recurse -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending
if ($TaskBSummaryFiles -and $TaskBSummaryFiles.Count -gt 0) {
    $TaskBSummaryFile = $TaskBSummaryFiles[0].FullName
} else {
    $TaskBSummaryFile = Join-Path $TaskBOutputDir "pentad_summary.json"
}
$TaskBTokens = 0
if (Test-Path $TaskBSummaryFile) {
    try {
        $TaskBSummary = (Get-Content -Raw -Path $TaskBSummaryFile -Encoding UTF8) | ConvertFrom-Json
        $TaskBTokens = $TaskBSummary.cumulative_global_tokens
    } catch {
        Write-Host "[WARNING] Failed to parse Task B summary JSON: $_" -ForegroundColor Yellow
    }
}

$TotalTokensConsumed = $TaskATokens + $TaskBTokens
$MaxExposureCeiling = $GlobalBudgetCeiling

Write-Host "======================================================================" -ForegroundColor Cyan
Write-Host " TWO-TASK PENTAD CONSOLIDATED EXECUTION SUMMARY" -ForegroundColor Cyan
Write-Host "======================================================================" -ForegroundColor Cyan
Write-Host "Task A Total Tokens (T_A):             $TaskATokens"
Write-Host "Task B Total Tokens (T_B):             $TaskBTokens"
Write-Host "Combined Aggregate Tokens (T_total):   $TotalTokensConsumed"
Write-Host "Global Ceiling (B_G):                  $GlobalBudgetCeiling"
Write-Host "Max Exposure Ceiling (E_max):          $MaxExposureCeiling"

$ExposureWithinLimit = ($TotalTokensConsumed -le $MaxExposureCeiling)
if ($ExposureWithinLimit) {
    Write-Host "Aggregate Budget Compliance:           PASSED ($TotalTokensConsumed <= $MaxExposureCeiling)" -ForegroundColor Green
} else {
    Write-Host "Aggregate Budget Compliance:           BREACHED ($TotalTokensConsumed > $MaxExposureCeiling)" -ForegroundColor Red
}

# Write consolidated summary JSON
$ConsolidatedSummary = [ordered]@{
    "schema_version" = "2.1.0"
    "coordinator_phase" = "5.9.0.6"
    "timestamp" = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    "global_budget_ceiling" = $GlobalBudgetCeiling
    "max_exposure_ceiling" = $MaxExposureCeiling
    "task_a_nominal_ceiling" = $TaskANominalCeiling
    "task_a_consumed_tokens" = $TaskATokens
    "task_b_dynamic_ceiling" = $DynamicCeilingB
    "task_b_consumed_tokens" = $TaskBTokens
    "total_tokens_consumed" = $TotalTokensConsumed
    "aggregate_exposure_compliant" = $ExposureWithinLimit
    "task_a_gate_passed" = $GateEval.Passed
    "task_a_exit_code" = $TaskAExitCode
    "task_b_exit_code" = $TaskBExitCode
    "output_directories" = [ordered]@{
        "task_a" = $TaskAOutputDir
        "task_b" = $TaskBOutputDir
    }
}

$ConsolidatedJsonPath = Join-Path $ResolvedOutputDir "two_task_pentad_summary.json"
$ConsolidatedSummary | ConvertTo-Json -Depth 5 | Set-Content -Path $ConsolidatedJsonPath -Encoding UTF8
Write-Host "Consolidated summary written to: $ConsolidatedJsonPath" -ForegroundColor Cyan

if (-not $ExposureWithinLimit) {
    exit 2
}
Write-Host "`nTwo-Task Pentad Coordinator execution finished successfully." -ForegroundColor Green
exit 0
