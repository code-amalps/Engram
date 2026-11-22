# Mutual Acceptance Integrity Report: N=1 Matched-Pair Pilot

**Milestone:** Engram Amendment 05 — Milestone 5C  
**Document ID:** `amendments/draft-05/milestone-5c/mutual-acceptance-integrity-report.md`  
**Evaluation Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Evaluation Engine:** `benchmarks/matched-pair-harness/src/orchestrator.py`  
**Status:** AUDITED & EMPIRICALLY VERIFIED  

---

## 1. Executive Summary

A core scientific principle of the Engram benchmark harness is that token efficiency gains cannot be claimed on broken, defective, or incomplete code. If an agent produces code that fails compilation, breaks regression tests, or violates task requirements, any reduction in token usage is scientifically invalid.

This report documents the architectural audit and offline verification of the **Mutual Acceptance Rule** in `benchmarks/matched-pair-harness/src/orchestrator.py` (`compute_comparative_analysis`).

---

## 2. Formal Specification of the Mutual Acceptance Rule

In `orchestrator.py` (`lines 500–560`):

$$\text{Savings Claim Valid} \iff (C_{\text{control}} = 1.0) \land (C_{\text{treatment}} = 1.0) \land (\text{Telemetry}_{\text{control}} = \text{VALID}) \land (\text{Telemetry}_{\text{treatment}} = \text{VALID})$$

Where:
- $C_{\text{arm}}$ is the correctness score calculated by `Evaluator.calculate_score`.
- Functional acceptance requires both $C_{\text{arm}} = 1.00$ and `passed_all_requirements = True`.
- Telemetry validity requires provider-reported token metrics with no negative, missing, or contradictory data.

---

## 3. Empirical Invariant Verifications

The mutual acceptance engine was audited against all six required edge cases via offline unit tests in `test_milestone_5c_remediation.py` (`TestMutualAcceptanceIntegrity`):

### 3.1 Invariant 1: Failed Baseline Suppresses Savings Claims
- **Scenario:** Control A achieves $C = 0.50$ (fails acceptance); Treatment B achieves $C = 1.00$ (passes).
- **Harness Action:** Comparative savings are strictly suppressed.
- **Verdict:** `CONTROL_FAILED_ACCEPTANCE`.
- **Status:** `savings_claim_eligible: False`, `savings_claim_status: "INVALIDATED_BY_ACCEPTANCE_FAILURE"`.
- **Scientific Rationale:** Without a valid baseline solution, the baseline token denominator cannot be established; efficiency claims cannot be computed.

### 3.2 Invariant 2: Failed Treatment Suppresses Savings Claims
- **Scenario:** Control A achieves $C = 1.00$ (passes); Treatment B achieves $C = 0.75$ (fails acceptance).
- **Harness Action:** Comparative savings are strictly suppressed.
- **Verdict:** `TREATMENT_FAILED_ACCEPTANCE`.
- **Status:** `savings_claim_eligible: False`, `savings_claim_status: "INVALIDATED_BY_ACCEPTANCE_FAILURE"`.
- **Scientific Rationale:** A model that generates broken code faster or with fewer tokens has not solved the software engineering task.

### 3.3 Invariant 3: Dual Failure Suppresses Savings Claims
- **Scenario:** Control A achieves $C = 0.50$; Treatment B achieves $C = 0.50$.
- **Harness Action:** Comparative savings are strictly suppressed.
- **Verdict:** `MUTUAL_ACCEPTANCE_FAILURE`.
- **Status:** `savings_claim_eligible: False`, `savings_claim_status: "INVALIDATED_BY_ACCEPTANCE_FAILURE"`.

### 3.4 Invariant 4: Telemetry Deficiency Suppresses Savings Claims
- **Scenario:** Both arms achieve $C = 1.00$, but telemetry on either arm is unparseable, missing, or zero-token.
- **Harness Action:** Comparative savings are strictly suppressed.
- **Verdict:** `CONTROL_TELEMETRY_INVALID`, `TREATMENT_TELEMETRY_INVALID`, or `MUTUAL_TELEMETRY_INVALID`.
- **Status:** `savings_claim_eligible: False`, `savings_claim_status: "INVALIDATED_BY_TELEMETRY_INVALID"`.

### 3.5 Invariant 5: Partial Runs Retain Consumed-Token Evidence
- **Scenario:** An arm breaches the token ceiling or times out midway through execution.
- **Harness Action:** In `runner.py` and `orchestrator.py`, `StreamingTokenTracker` records all tokens consumed up to the moment of termination.
- **Status:** Evidence files (`events.jsonl` and `run_telemetry.json`) record exact cumulative usage even when execution status is marked `TOKEN_CEILING_BREACHED` or `TIMEOUT`.

### 3.6 Invariant 6: Evaluator Exceptions Fail Closed
- **Scenario:** An unhandled error occurs during AST inspection or SQLite parsing.
- **Harness Action:** All evaluator static checks are wrapped in defensive `try...except` blocks that catch exceptions, log the failure, and return `False`.
- **Status:** Evaluator errors fail safe, preventing an invalid implementation from accidentally receiving passing credit.

---

## 4. Preservation of the 1.0 Acceptance Threshold

The functional acceptance threshold remains strictly at $C = 1.00$ (100% of checklist points). No requirements were weighted down, eliminated, or softened. The remediation implemented in Milestone 5C ensures that a genuinely correct implementation can achieve $C = 1.00$, while defective implementations continue to fail closed.
