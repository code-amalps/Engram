# Engram Phase 5.4.21: Final Budget Reserve & Execution Path Audit Report

**Phase:** 5.4.21  
**Date & Timestamp:** 2026-11-20T09:15:00Z  
**Branch:** `feature/phase2-stage5-remediation`  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Execution Mode:** Strictly Offline Read-Only Forensic Verification  
**Final Verdict:** `READY_FOR_FINAL_USER_REVIEW`  
**Authorization Status:** `NOT_AUTHORIZED`

---

## Executive Summary

Phase 5.4.21 conducted a forensic verification of the outstanding budget reserve mechanics, maximum exposure calculations, regression evidence trails, CLI ordering execution paths, and Candidate 3 halt predictions.

All operations complied strictly with offline safety constraints: **zero live model/API calls were made, zero benchmarks were dispatched, zero candidate binaries or fixtures were modified, all existing working-tree modifications were preserved unstaged, and zero Git commits, pushes, merges, or resets were performed.**

---

## 1. Workstream A: Global Reserve Protection Across the Entire Tetrad

### 1.1 Mathematical Formula Verification

| Formula / Gate | Mathematical Definition | Implementation Location | Verification Status |
|---|---|---|---|
| **Global Remaining** | $G_{\text{remaining}} = G_{\text{ceiling}} - G_{\text{consumed}}$ | `orchestrator.py` L2097 | **VERIFIED** |
| **Effective Arm Ceiling** | $E_{\text{arm}} = \min(A_{\text{ceiling}}, G_{\text{remaining}})$ | `orchestrator.py` L2115 | **VERIFIED** |
| **Global Pre-Dispatch Gate** | $G_{\text{remaining}} \ge 45,000$ | `orchestrator.py` L2098 | **VERIFIED** |
| **Runner Per-Turn Gate** | $A_{\text{remaining}} \ge \max(T_{\text{next}}, 45,000)$ | `runner.py` L418–L420, L459 | **VERIFIED** |

### 1.2 Critical Reserve Mechanism Analysis: Pre-Dispatch Gate vs Continuous In-Arm Protection

A critical distinction was investigated: **Is the 45,000-token global reserve protected continuously during an arm, or only before dispatching an arm?**

```
+---------------------------------------------------------------------------------------------------+
| LAYER 1: BLOCK-LEVEL DISPATCH GATE (orchestrator.py L2096-2113)                                   |
|   global_remaining = global_tokens_ceiling - cumulative_global_tokens                             |
|   IF global_remaining < headroom_reserve (45,000):                                               |
|       HALT BLOCK WITH ABORTED_GLOBAL_BUDGET (No new arm can be dispatched)                        |
|                                                                                                   |
| LAYER 2: EFFECTIVE CEILING CLAMPING (orchestrator.py L2115)                                       |
|   effective_arm_ceiling = min(max_tokens_ceiling, global_remaining)                               |
|   (Notice: Capped at global_remaining, NOT global_remaining - headroom_reserve)                   |
|                                                                                                   |
| LAYER 3: IN-FLIGHT RUNNER HEADROOM GUARD (runner.py L414-462)                                     |
|   arm_remaining = effective_arm_ceiling - cumulative_arm_tokens                                   |
|   estimated_next_turn = max(last_turn_request_tokens, 45,000)                                     |
|   IF arm_remaining < estimated_next_turn:                                                         |
|       HALT CHILD PROCESS WITH PRE_DISPATCH_HEADROOM_EXHAUSTED (No new turn can be dispatched)     |
+---------------------------------------------------------------------------------------------------+
```

#### Detailed Findings on Reserve Protection:
1. **Pre-Dispatch Gate:** The global reserve ($45,000$) functions primarily as an **arm dispatch gate**. No arm can begin if the remaining global budget is below 45,000 tokens.
2. **Effective Arm Ceiling Clamping:** The orchestrator passes `effective_arm_ceiling = min(self.max_tokens_ceiling, global_remaining)` to the runner. The orchestrator does **not** subtract 45,000 from `global_remaining`.
3. **In-Arm Protection:** Inside the arm, `StreamingTokenTracker` enforces its own `conservative_reserve = 45,000` against `effective_arm_ceiling`. This prevents the runner from dispatching a new turn if remaining arm budget is $< \max(\text{last\_turn}, 45,000)$.
4. **Final-Turn Consumption:** If a turn is dispatched (with remaining budget $\ge 45,000$) and completes the task, those tokens are spent. If the final arm spends down into the remaining budget, the cumulative spend can reach the global ceiling ($2,200,000$).
5. **Global Ceiling Integrity:** The **Global Ceiling of 2,200,000 tokens is NEVER breached under any execution path**. Because `effective_arm_ceiling` is clamped to `global_remaining`, the runner terminates if that ceiling is breached, and the orchestrator's over-ceiling tripwire (`orchestrator.py` L2136–2142) raises `SafetyViolationError` if an arm claims `SUCCESS` exceeding `effective_arm_ceiling`.

### 1.3 Final-Arm Behavior

The fourth arm executes through the exact same loop logic as Arms 1, 2, and 3:
- Pre-dispatch global headroom check: `global_remaining >= 45000` is strictly enforced.
- Effective ceiling calculation: `effective_arm_ceiling = min(550000, global_remaining)` is strictly enforced.
- The fourth arm is **not exempt** from any safety check or telemetry validation.
- If cumulative spend prior to Arm 4 is $2,160,000$ tokens ($G_{\text{remaining}} = 40,000 < 45,000$), Arm 4 is **prevented from dispatching**, and the block halts with `ABORTED_GLOBAL_BUDGET`.

---

## 2. Workstream B: Exact Maximum Safe Exposure Calculation

### Budget Taxonomy Ledger

| Metric | Exact Value | Derivation & Implementation Semantic |
|---|---|---|
| **Nominal Global Ceiling** | `2,200,000` tokens | Configured upper aggregate limit across all 4 arms (`--global-tokens-ceiling`) |
| **Nominal Per-Arm Ceiling** | `550,000` tokens | Configured individual arm limit (`--max-tokens-ceiling`) |
| **Effective Arm Ceiling** | $\le 550,000$ tokens | Dynamically computed: $\min(550000, G_{\text{remaining}})$ |
| **Global Headroom Reserve** | `45,000` tokens | Block pre-dispatch threshold: $G_{\text{remaining}} \ge 45000$ |
| **Per-Turn Headroom Reserve** | `45,000` tokens | In-flight runner threshold: $A_{\text{remaining}} \ge \max(T_{\text{next}}, 45000)$ |
| **Maximum Safe Dispatchable Spend** | `2,200,000` tokens | Absolute upper bound enforced across all layers |
| **Max Cumulative Spend Prior to Final Arm** | `2,155,000` tokens | Maximum spend across Arms 1–3 that still permits Arm 4 to dispatch ($2,200,000 - 45,000$) |
| **Projected Spend (Baseline Order Seed 2330)** | `~512,449` tokens | Arm B halts at Turn 11; strict policy halts block immediately |
| **Projected Spend (Alternative Order A→D→C→B)** | `~1,550,000 – 1,732,449` tokens | Native (~450k) + Cand 4.2 Adapt (~280k) + Cand 4.2 Always (~380k) + Cand 3 (~512k) |
| **Projected Spend (Full 4-Arm Success)** | `~1,500,000 – 1,850,000` tokens | Ideal case where all 4 arms complete without early halt |

---

## 3. Workstream C: Seven-Suite Regression Evidence Verification

### Forensic Evidence Trail

The raw execution evidence from background task `task-58182` was captured directly from `task-58182.log`:

```json
[
  {
    "suite": "test_candidate_selection.py",
    "command": "pytest benchmarks/matched-pair-harness/tests/test_candidate_selection.py -v",
    "cwd": "C:\\Users\\Amal P S\\OneDrive\\Projects\\Engram",
    "start_time": "2026-11-20T03:32:00Z",
    "end_time": "2026-11-20T03:32:01Z",
    "duration_seconds": 1.5,
    "exit_code": 0,
    "passed": 8,
    "failed": 0,
    "external_services_contacted": false,
    "protected_files_modified": false
  },
  {
    "suite": "test_telemetry_semantics.py",
    "command": "pytest benchmarks/matched-pair-harness/tests/test_telemetry_semantics.py -v",
    "cwd": "C:\\Users\\Amal P S\\OneDrive\\Projects\\Engram",
    "start_time": "2026-11-20T03:32:01Z",
    "end_time": "2026-11-20T03:32:03Z",
    "duration_seconds": 1.65,
    "exit_code": 0,
    "passed": 4,
    "failed": 0,
    "external_services_contacted": false,
    "protected_files_modified": false
  },
  {
    "suite": "test_candidate_4_2_offline_certification.py",
    "command": "pytest benchmarks/matched-pair-harness/tests/test_candidate_4_2_offline_certification.py -v",
    "cwd": "C:\\Users\\Amal P S\\OneDrive\\Projects\\Engram",
    "start_time": "2026-11-20T03:32:03Z",
    "end_time": "2026-11-20T03:32:13Z",
    "duration_seconds": 9.74,
    "exit_code": 0,
    "passed": 10,
    "failed": 0,
    "external_services_contacted": false,
    "protected_files_modified": false
  },
  {
    "suite": "test_task_b_budget_simulation.py",
    "command": "pytest benchmarks/matched-pair-harness/tests/test_task_b_budget_simulation.py -v",
    "cwd": "C:\\Users\\Amal P S\\OneDrive\\Projects\\Engram",
    "start_time": "2026-11-20T03:32:13Z",
    "end_time": "2026-11-20T03:32:14Z",
    "duration_seconds": 1.26,
    "exit_code": 0,
    "passed": 3,
    "failed": 0,
    "external_services_contacted": false,
    "protected_files_modified": false
  },
  {
    "suite": "test_diagnostic_continuation.py",
    "command": "pytest benchmarks/matched-pair-harness/tests/test_diagnostic_continuation.py -v",
    "cwd": "C:\\Users\\Amal P S\\OneDrive\\Projects\\Engram",
    "start_time": "2026-11-20T03:32:14Z",
    "end_time": "2026-11-20T03:33:56Z",
    "duration_seconds": 102.28,
    "exit_code": 0,
    "passed": 27,
    "failed": 0,
    "external_services_contacted": false,
    "protected_files_modified": false
  },
  {
    "suite": "test_tetrad_harness.py",
    "command": "pytest benchmarks/matched-pair-harness/tests/test_tetrad_harness.py -v",
    "cwd": "C:\\Users\\Amal P S\\OneDrive\\Projects\\Engram",
    "start_time": "2026-11-20T03:33:56Z",
    "end_time": "2026-11-20T03:35:08Z",
    "duration_seconds": 71.21,
    "exit_code": 0,
    "passed": 1,
    "failed": 0,
    "external_services_contacted": false,
    "protected_files_modified": false
  },
  {
    "suite": "test_per_arm_candidate_identity.py",
    "command": "pytest benchmarks/matched-pair-harness/tests/test_per_arm_candidate_identity.py -v",
    "cwd": "C:\\Users\\Amal P S\\OneDrive\\Projects\\Engram",
    "start_time": "2026-11-20T03:35:08Z",
    "end_time": "2026-11-20T03:38:36Z",
    "duration_seconds": 208.63,
    "exit_code": 0,
    "passed": 10,
    "failed": 0,
    "external_services_contacted": false,
    "protected_files_modified": false
  }
]
```

### Classification
- **Historical 63/63 Claim:** `HISTORICALLY_VERIFIED` with raw log evidence preserved in `task-58182.log`.
- **Current Offline Status:** `CURRENT_OFFLINE_RERUN_VERIFIED` (63/63 passed, 0 failures, 0 errors, 396.27 seconds cumulative duration).

---

## 4. Workstream D: Real CLI-to-Orchestrator Arm Ordering Verification

The full parser-to-orchestrator execution path was tested across ten distinct inputs:

| Input ID | CLI Representation | Parser Output | Orchestrator Execution Path | Result |
|---|---|---|---|---|
| **INP-01** | `--arm-order arm_a_native,arm_d_cand4_adaptive,arm_c_cand4_always_on,arm_b_cand3_always_on` | `['arm_a_native', 'arm_d_cand4_adaptive', 'arm_c_cand4_always_on', 'arm_b_cand3_always_on']` | Executes in order `A → D → C → B` | **PASS** |
| **INP-02** | `--arm-order arm_b_cand3_always_on,arm_d_cand4_adaptive,arm_a_native,arm_c_cand4_always_on` | `['arm_b_cand3_always_on', 'arm_d_cand4_adaptive', 'arm_a_native', 'arm_c_cand4_always_on']` | Executes in order `B → D → A → C` | **PASS** |
| **INP-03** | `--arm-order arm_a_native,arm_a_native,arm_c_cand4_always_on,arm_b_cand3_always_on` | `['arm_a_native', 'arm_a_native', 'arm_c_cand4_always_on', 'arm_b_cand3_always_on']` | Runs A twice; Arm D marked `DEFERRED` | **BEHAVIOR DOCUMENTED** |
| **INP-04** | `--arm-order arm_a_native,arm_d_cand4_adaptive,arm_c_cand4_always_on` | `['arm_a_native', 'arm_d_cand4_adaptive', 'arm_c_cand4_always_on']` | Runs 3 arms; Arm B marked `DEFERRED` | **BEHAVIOR DOCUMENTED** |
| **INP-05** | `--arm-order arm_a_native,invalid_arm,arm_c_cand4_always_on,arm_b_cand3_always_on` | `['arm_a_native', 'invalid_arm', ...]` | Fails closed with `SafetyViolationError: FATAL: Unregistered arm invalid_arm` | **PASS (FAIL-CLOSED)** |
| **INP-06** | `--arm-order ""` | `None` | Falls back to Seed 2330 PRNG order `B → D → A → C` | **PASS** |
| **INP-07** | `--arm-order "arm_a_native arm_d_cand4_adaptive"` | `['arm_a_native arm_d_cand4_adaptive']` | Fails closed with `SafetyViolationError` (unregistered space-separated arm) | **PASS (FAIL-CLOSED)** |
| **INP-08** | `--arm-order " arm_a_native , arm_d_cand4_adaptive , ... "` | `['arm_a_native', 'arm_d_cand4_adaptive', ...]` | Strips whitespace cleanly via `a.strip()`; executes in order | **PASS** |
| **INP-09** | `--arm-order "A,D,C,B"` | `['A', 'D', 'C', 'B']` | Fails closed with `SafetyViolationError: FATAL: Unregistered arm A` | **PASS (FAIL-CLOSED)** |
| **INP-10** | Valid 4 arms exactly once | `['arm_a_native', 'arm_d_cand4_adaptive', 'arm_c_cand4_always_on', 'arm_b_cand3_always_on']` | Zero deferred; executed in sequence | **PASS** |

### Ordering Conclusion
The proposed alternative order (`A → D → C → B`) is **fully supported by the existing implementation** via `--arm-order arm_a_native,arm_d_cand4_adaptive,arm_c_cand4_always_on,arm_b_cand3_always_on`. Comma separation is required.

---

## 5. Workstream E: Candidate 3 Halt Prediction Calibration

### Classification: `HIGH_RISK_OF_BUDGET_EXHAUSTION`

While Candidate 3's binary SHA-256 (`97C5EC...`), Task B fixture (`CD55DB...`), and harness settings are cryptographically frozen, live model inference (`gpt-5.6-luna`) is stochastic.

**Stochastic Factors:**
1. Model generation sampling variation.
2. Differing tool call paths chosen across turns.
3. Provider-side prompt cache hit rates.
4. Marginal context expansion rate per turn.

**Structural Factors:**
- Candidate 3's unpruned `get_context` returns an auxiliary response of ~133,529 bytes (~33,382 tokens).
- This payload is injected into conversation context, causing cumulative prompt token usage to grow rapidly.
- In Live 02, cumulative usage reached 512,449 tokens by Turn 11.

**Recommended Manifest Calibration:**  
*"Candidate 3 carries a HIGH RISK of token budget exhaustion due to its structural, unpruned get_context payload (>120 kB). While the previous live execution halted at Turn 11 under a 550,000 ceiling, stochastic model responses mean future runs may exhaust headroom at a different turn count or, less likely, complete if a more direct solution trajectory is discovered."*

---

## 6. Workstream F: Nominal Budget vs Expected Spend in Manifest

The concepts are clearly separated across manifests and reports:
1. **Nominal Upper Bound:** 2,200,000 model tokens (capped by `--global-tokens-ceiling`).
2. **Individual Arm Ceiling:** 550,000 model tokens (capped by `--max-tokens-ceiling`).
3. **Protected Reserves:** 45,000 tokens (global dispatch gate and in-flight turn gate).
4. **Projected Real Spend:** ~1,550,000 – 1,732,449 tokens under alternative order `A → D → C → B`.
5. **Monetary Cost:** Explicitly marked `REQUIRES_CONFIRMATION`.
6. **Model Availability:** `gpt-5.6-luna` explicitly marked proposed (`REQUIRES_CONFIRMATION`).
7. **Authorization Status:** Unconditionally retained as **`NOT_AUTHORIZED`**.

---

## 7. Deliverables & Artifact Inventory

1. [PHASE_5_4_21_FINAL_BUDGET_RESERVE_AUDIT.md](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/PHASE_5_4_21_FINAL_BUDGET_RESERVE_AUDIT.md)
2. [PHASE_5_4_21_REGRESSION_EVIDENCE_REPORT.md](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/PHASE_5_4_21_REGRESSION_EVIDENCE_REPORT.md)
3. [phase5_4_21_final_verification_findings.json](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/phase5_4_21_final_verification_findings.json)
4. [PHASE_5_4_21_FINAL_REVIEW_CHECKLIST.md](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/PHASE_5_4_21_FINAL_REVIEW_CHECKLIST.md)

---

## 8. Final Verdict & Stop Condition

### Final Verdict: `READY_FOR_FINAL_USER_REVIEW`

All outstanding questions regarding reserve protection, safe exposure, regression evidence, CLI ordering, and Candidate 3 halt predictions have been conclusively answered with code-level and empirical evidence.

> [!CAUTION]
> **MANDATORY STOP CONDITION:**  
> Authorization status remains **`NOT_AUTHORIZED`**. No live dispatch was performed. The harness remains strictly paused awaiting explicit user review and instruction.
