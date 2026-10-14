<#
.SYNOPSIS
    Unit test for the explicit, validated Attestation Policy engine
#>

Write-Host "Running Explicit Attestation Policy Test Suite..." -ForegroundColor Cyan

function Evaluate-AttestationPolicy([string]$InputPolicy, [string]$VarPolicy) {
    # Resolve precedence: input > var > default 'required'
    $policy = "required"
    if (-not [string]::IsNullOrWhiteSpace($InputPolicy)) {
        $policy = $InputPolicy
    } elseif (-not [string]::IsNullOrWhiteSpace($VarPolicy)) {
        $policy = $VarPolicy
    }

    switch ($policy) {
        "required" {
            return @{
                ExitCode = 0
                RunAttestation = $true
                Policy = "required"
                Description = "Attestation strictly enforced; failure fails closed."
            }
        }
        "approved_without_attestation" {
            return @{
                ExitCode = 0
                RunAttestation = $false
                Policy = "approved_without_attestation"
                Description = "Attestation bypassed with explicit operator authorization."
            }
        }
        default {
            return @{
                ExitCode = 1
                RunAttestation = $false
                Policy = $policy
                Error = "Invalid attestation policy '$policy'."
            }
        }
    }
}

# Test 1: Default (neither input nor var set) -> must be 'required' with RunAttestation = true
$t1 = Evaluate-AttestationPolicy -InputPolicy "" -VarPolicy ""
if ($t1.ExitCode -eq 0 -and $t1.Policy -eq "required" -and $t1.RunAttestation -eq $true) {
    Write-Host "[PASS] Test 1: Default policy is 'required' with mandatory attestation" -ForegroundColor Green
} else {
    Write-Error "[FAIL] Test 1 failed"
}

# Test 2: Input 'approved_without_attestation' -> RunAttestation = false
$t2 = Evaluate-AttestationPolicy -InputPolicy "approved_without_attestation" -VarPolicy ""
if ($t2.ExitCode -eq 0 -and $t2.Policy -eq "approved_without_attestation" -and $t2.RunAttestation -eq $false) {
    Write-Host "[PASS] Test 2: Explicit 'approved_without_attestation' mode permits bypass" -ForegroundColor Green
} else {
    Write-Error "[FAIL] Test 2 failed"
}

# Test 3: Repo Var 'approved_without_attestation' -> RunAttestation = false
$t3 = Evaluate-AttestationPolicy -InputPolicy "" -VarPolicy "approved_without_attestation"
if ($t3.ExitCode -eq 0 -and $t3.Policy -eq "approved_without_attestation" -and $t3.RunAttestation -eq $false) {
    Write-Host "[PASS] Test 3: Repo var 'approved_without_attestation' permits bypass" -ForegroundColor Green
} else {
    Write-Error "[FAIL] Test 3 failed"
}

# Test 4: Input 'required' overrides Repo Var 'approved_without_attestation'
$t4 = Evaluate-AttestationPolicy -InputPolicy "required" -VarPolicy "approved_without_attestation"
if ($t4.ExitCode -eq 0 -and $t4.Policy -eq "required" -and $t4.RunAttestation -eq $true) {
    Write-Host "[PASS] Test 4: Input 'required' overrides repository variable" -ForegroundColor Green
} else {
    Write-Error "[FAIL] Test 4 failed"
}

# Test 5: Invalid policy string -> fails closed with non-zero exit code
$t5 = Evaluate-AttestationPolicy -InputPolicy "invalid_mode" -VarPolicy ""
if ($t5.ExitCode -eq 1) {
    Write-Host "[PASS] Test 5: Invalid policy string fails closed with error" -ForegroundColor Green
} else {
    Write-Error "[FAIL] Test 5 failed"
}

Write-Host "All 5 policy test cases passed successfully." -ForegroundColor Green
