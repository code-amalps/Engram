# Engram Phase 5.9.0.6: Budget Conservation Test Report

**Phase:** 5.9.0.6  
**Test Suite:** `benchmarks/matched-pair-harness/tests/test_budget_coordinator.py` and `scripts/run_two_task_pentad.ps1`  
**Execution Mode:** STRICTLY OFFLINE TEST & DRY-RUN EXECUTION  
**Date:** 2026-11-22  

---

## 1. Test Suite Summary

The unit test suite `test_budget_coordinator.py` and the dry-run execution of `run_two_task_pentad.ps1` were conducted to prove aggregate budget conservation, overshoot cancellation, Candidate 5 headroom guarantees, and 14-point summary validation gate enforcement.

| Metric | Value |
| :--- | :--- |
| **Unit Tests Executed** | 6 |
| **Unit Tests Passed** | 6 (100%) |
| **Unit Test Duration** | 0.18 seconds |
| **Two-Task Coordinator Dry-Run** | Completed successfully (Exit code: 0) |
| **Total Coordinator Runtime** | 35.8 seconds |
| **Aggregate Budget Conservation Status** | **VERIFIED COMPLIANT** |

---

## 2. Unit Test Evidence Breakdown

### Test 1: `test_01_nominal_consumption_scenario`
- **Condition:** $T_A = 950,000$ tokens ($< 1,000,000$).
- **Observation:** $C_B = \min(1,500,000, 2,500,000 - 950,000 = 1,550,000) = 1,500,000$ tokens.
- **Total Exposure:** $T_{\text{total}} = 950,000 + 1,500,000 = 2,450,000 \le 2,500,000$.
- **Result:** **PASSED**

### Test 2: `test_02_task_a_overshoot_cancellation_proof`
- **Condition:** Evaluated grid across $O_A \in \{1, 5000, 20000, 31384\}$ and $O_B \in \{0, 10000, 20000, 31384\}$.
- **Observation:** In every single case, $T_{\text{total}} = 2,500,000 + O_B$. The Task A overshoot $O_A$ cancelled out completely with zero residual.
- **Maximum Observed:** $2,531,384$ tokens (at $O_B = 31,384$).
- **Result:** **PASSED**

### Test 3: `test_03_candidate_5_task_a_headroom_guarantee`
- **Condition:** Slot 3 dispatch in Task A after Arms 1 and 2 consume 11,000 tokens each.
- **Observation:** Available headroom: $1,000,000 - 22,000 = 978,000$ tokens. Candidate 5 requires 200,000 tokens. Margin: 4.89x.
- **Result:** **PASSED**

### Test 4: `test_04_candidate_5_task_b_headroom_guarantee`
- **Condition:** Slot 3 dispatch in Task B after Task A consumed 1,020,000 tokens and Arms 1 and 2 consume 300,000 tokens each.
- **Observation:** Dynamic ceiling $C_B = 1,480,000$ tokens. Prior usage: $600,000$ tokens. Available headroom: $1,480,000 - 600,000 = 880,000$ tokens. Candidate 5 requires 300,000 tokens. Margin: 2.93x.
- **Result:** **PASSED**

### Test 5: `test_05_insufficient_headroom_fail_closed`
- **Condition:** Task A consumes $2,400,000$ tokens (extreme abnormal scenario).
- **Observation:** Dynamic ceiling computed as $100,000$ tokens, which is $< 150,000$ tokens threshold. Coordinator fails closed and refuses to dispatch Task B.
- **Result:** **PASSED**

### Test 6: `test_06_maximum_theoretical_exposure_reconciliation`
- **Condition:** Mathematical comparison between corrected $E_{\max} = 2,500,000 + O_B = 2,531,384$ and the flawed Phase 5.9.0.4 claim ($2,562,768$).
- **Observation:** Proved that $2,562,768 - 2,531,384 = 31,384$, which exactly equals the double-counted $O_A$ term.
- **Result:** **PASSED**

---

## 3. End-to-End Coordinator Dry-Run Execution Evidence

The full coordinator script was executed via PowerShell:
```powershell
powershell -ExecutionPolicy Bypass -File benchmarks/matched-pair-harness/scripts/run_two_task_pentad.ps1 -DryRun
```

### Execution Log Highlights:
1. **Task A Execution:**
   - Command: `python orchestrator.py --mode pentad --task task_a_typo.json --global-tokens-ceiling 1000000 --dry-run`
   - Generated `pentad_summary.json` with 28,720 tokens consumed.
2. **14-Point Validation Gate Evaluation:**
   - All 14 checks evaluated:
     `[PASS] Check 1: Summary file exists and parsed valid JSON`
     `[PASS] Check 2: Task ID verified (task_a_typo)`
     `[PASS] Check 3: Schema version verified (2.1.0)`
     `[PASS] Check 4: All 5 pentad arms present in execution runs`
     `[PASS] Check 5: Cumulative tokens calculated (28720 tokens)`
     `[PASS] Check 6: Per-arm token ceilings respected across all arms`
     `[PASS] Check 7: Output token accounting present and valid`
     `[PASS] Check 8: Input token accounting present and valid`
     `[PASS] Check 9: Pre-dispatch request size gate passed on all arms`
     `[PASS] Check 10: Task A total tokens (28720) <= upper bound (1031384)`
     `[PASS] Check 11: Dynamic Task B ceiling valid (1500000 tokens >= 150,000 minimum)`
     `[PASS] Check 12: Acceptance matrix and requirement results recorded`
     `[PASS] Check 13: Process exit codes and execution statuses valid`
     `[PASS] Check 14: Clean execution without unhandled exceptions`
   - Verdict: **ALL 14 CHECKS PASSED**.
3. **Dynamic Task B Ceiling Calculation:**
   - Formula: $C_B = \min(1,500,000, 2,500,000 - 28,720) = 1,500,000$ tokens.
   - Per-arm ceiling: $1,500,000 / 5 = 300,000$ tokens.
4. **Task B Execution:**
   - Command: `python orchestrator.py --mode pentad --task task_b_remember_me.json --global-tokens-ceiling 1500000 --dry-run`
   - Generated `pentad_summary.json` with 28,720 tokens consumed.
5. **Post-Execution Reconciliation:**
   - $T_A = 28,720$ tokens.
   - $T_B = 28,720$ tokens.
   - $T_{\text{total}} = 57,440$ tokens.
   - Global Ceiling: $2,500,000$ tokens.
   - Max Exposure Ceiling: $2,531,384$ tokens.
   - Verification: $57,440 \le 2,531,384$ (**PASSED**).
   - Consolidated summary file written to `runs/two_task_pentad/two_task_pentad_summary.json`.
   - Process Exit Code: `0`.
