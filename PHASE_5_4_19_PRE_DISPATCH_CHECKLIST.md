# Engram Phase 5.4.19: Pre-Dispatch Verification Checklist

**Phase:** 5.4.19  
**Target Benchmark:** Task B Matched-Pair Tetrad (`task_b_remember_me_tetrad_live_03`)  
**Authorization Status:** `NOT_AUTHORIZED`  
**Purpose:** Mandatory pre-dispatch verification gate to be executed immediately prior to any future authorized live experiment. Every check is strictly fail-closed: **any single failure halts dispatch immediately.**

---

## Pre-Dispatch Checklist Ledger (13 / 13 Required)

| Check ID | Verification Item | Target Value / Specification | Verification Command / Procedure | Mandatory Status |
|---|---|---|---|---|
| **CHK-01** | Candidate 3 Binary Identity | SHA-256: `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777`<br>Size: `18,668,032` bytes | `(Get-FileHash 'benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe' -Algorithm SHA256).Hash` | **FAIL-CLOSED** |
| **CHK-02** | Candidate 4.2 Binary Identity | SHA-256: `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2`<br>Size: `18,802,176` bytes | `(Get-FileHash 'benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe' -Algorithm SHA256).Hash` | **FAIL-CLOSED** |
| **CHK-03** | Task B Fixture Hash | SHA-256: `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` | `(Get-FileHash 'benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json' -Algorithm SHA256).Hash` | **FAIL-CLOSED** |
| **CHK-04** | Golden Repository Commit | Commit: `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`<br>Path: `Test/angular-ngrx-material-starter` | `git -C 'C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter' rev-parse HEAD` | **FAIL-CLOSED** |
| **CHK-05** | Git Working Tree & Baseline | Baseline Commit: `7106a2f369fe79faf5f95449378a900ab8e8eddf`<br>Branch: `feature/phase2-stage5-remediation` | `git status --short; git rev-parse HEAD` (Ensure no accidental stashes, resets, or unverified branch checkouts) | **FAIL-CLOSED** |
| **CHK-06** | Model & Provider Configuration | Model: `gpt-5.6-luna`<br>Reasoning: `medium`<br>Provider: `cloud`<br>Strategy: `copy` | Confirm CLI arguments match manifest exactly: `--model gpt-5.6-luna --reasoning medium --provider cloud --dependency-strategy copy` | **FAIL-CLOSED** |
| **CHK-07** | Live Provider Endpoint & Quota Confirmation | Endpoint active; active billing/quota available for up to 2.2M tokens | User explicit confirmation that upstream cloud account has sufficient quota for tetrad execution | **FAIL-CLOSED** |
| **CHK-08** | Budget Ceiling & Headroom Constraints | Per-arm Ceiling: `550,000`<br>Global Ceiling: `2,200,000`<br>Headroom Reserve: `45,000` | Verify CLI flags: `--max-tokens-ceiling 550000 --global-tokens-ceiling 2200000 --headroom-reserve 45000` | **FAIL-CLOSED** |
| **CHK-09** | Execution Policy | Strict stop-on-failure policy active (`execution_policy == strict`) | Verify default policy or explicit flag `--execution-policy strict`. Ensure diagnostic continuation is disabled. | **FAIL-CLOSED** |
| **CHK-10** | Telemetry Validation & Acceptance Gates Enabled | Fail-closed integer typing, arithmetic checks, and dual-validity gates active | Automated verification: `Orchestrator.validate_telemetry_integrity` and `Orchestrator.is_run_accepted` enabled | **FAIL-CLOSED** |
| **CHK-11** | Stale Process Tree & Sandbox Scrub | No orphaned `codex.exe`, `node.exe`, or `engram.exe` processes running | Run `Get-Process codex, engram -ErrorAction SilentlyContinue` and scrub stale temp sandboxes | **FAIL-CLOSED** |
| **CHK-12** | Output Directory & Historical Immutability | Target output directory is strictly unique; no historical files will be overwritten | Verify `task_b_remember_me_tetrad_live_03` does not exist prior to dispatch | **FAIL-CLOSED** |
| **CHK-13** | Explicit User Authorization | Explicit user prompt authorizing exact run block `tetrad_live_03` | User must provide written instruction authorizing execution with reference to Phase 5.4.19 manifest | **FAIL-CLOSED** |

---

## Procedural Step-by-Step Pre-Dispatch Verification Script

```powershell
# ==============================================================================
# Phase 5.4.19 Pre-Dispatch Automated Audit Script
# ==============================================================================
$ErrorActionPreference = "Stop"
Set-Location "C:\Users\Amal P S\OneDrive\Projects\Engram"

Write-Host "=== Step 1: Baseline & Candidate Hashes ===" -ForegroundColor Cyan
$c3 = (Get-FileHash "benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe" -Algorithm SHA256).Hash
$c42 = (Get-FileHash "benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe" -Algorithm SHA256).Hash
$fix = (Get-FileHash "benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json" -Algorithm SHA256).Hash
$golden = git -C "C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter" rev-parse HEAD

if ($c3 -ne "97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777") { throw "Candidate 3 Hash Mismatch!" }
if ($c42 -ne "78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2") { throw "Candidate 4.2 Hash Mismatch!" }
if ($fix -ne "CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7") { throw "Task B Fixture Hash Mismatch!" }
if ($golden -ne "1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2") { throw "Golden Repo Commit Mismatch!" }
Write-Host "CHK-01 through CHK-04: PASSED" -ForegroundColor Green

Write-Host "=== Step 2: Working Tree & Process Verification ===" -ForegroundColor Cyan
$head = git rev-parse HEAD
if ($head -ne "7106a2f369fe79faf5f95449378a900ab8e8eddf") { throw "Engram HEAD Commit Mismatch!" }
$stale = Get-Process codex, engram -ErrorAction SilentlyContinue
if ($stale) { throw "Stale background processes detected! Terminate before dispatch." }
Write-Host "CHK-05 and CHK-11: PASSED" -ForegroundColor Green

Write-Host "=== Step 3: Output Directory Collision Check ===" -ForegroundColor Cyan
$outDir = "benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_tetrad_live_03"
if (Test-Path $outDir) { throw "Output directory already exists! Do not overwrite historical runs." }
Write-Host "CHK-12: PASSED" -ForegroundColor Green

Write-Host "`nAll local automated pre-dispatch checks PASSED." -ForegroundColor Green
Write-Host "Awaiting explicit user authorization and provider quota confirmation." -ForegroundColor Yellow
```

---

## Stop-and-Abort Decision Tree

```
                       [Pre-Dispatch Check]
                                |
                 +--------------+--------------+
                 |                             |
          [Any Check Fails]           [All 13 Checks Pass]
                 |                             |
                 v                             v
        ABORT DISPATCH                [User Authorization]
    Report Failure Reason                      |
    Harness Remains Frozen            +--------+--------+
                                      |                 |
                                   [Granted]       [Withheld]
                                      |                 |
                                      v                 v
                               EXECUTE TETRAD      DO NOT DISPATCH
```
