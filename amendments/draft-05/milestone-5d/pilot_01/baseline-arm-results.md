# Baseline Arm Results: Unassisted Control Arm A (`control_A`)

**Run ID:** `task_commit_cache_invalidation_pilot_01_control_A`  
**Arm Identity:** `control_A` (Unassisted Baseline)  
**Execution Order in Pair:** 2  
**Model:** `gpt-5.6-luna` (Reasoning Effort: `medium`)  
**Execution Status:** `SKIPPED_STOP_ON_FAILURE`  
**Halt Reason:** Mandatory Stop-on-Failure triggered by Treatment Arm B (`PRE_DISPATCH_HEADROOM_EXHAUSTED`)  

---

## 1. Execution Status and Protocol Adherence

Under the preregistered Amendment 04 two-arm stop policy, Treatment Candidate Phase 1 executes first. Baseline Control executes second **only if permitted by the stop policy**:

> *"If the treatment breaches the ceiling, times out, crashes or produces invalid telemetry, skip the baseline. Never launch another arm after a mandatory stop condition. Do not retry a failed arm."*

Because `treatment_B` experienced a preventive headroom halt (`PRE_DISPATCH_HEADROOM_EXHAUSTED`) and failed functional acceptance before authoring changes, the orchestrator immediately invoked the stop-on-failure rule.

The baseline arm was **NOT LAUNCHED**:
- Zero model requests were dispatched.
- Zero Codex agent processes were spawned.
- Zero tokens were consumed.
- Zero network or API costs were incurred.

---

## 2. Telemetry and Resource Record

| Metric | Recorded Value | Status |
| :--- | :--- | :---: |
| **Total Model Tokens** | **0** | `UNAVAILABLE` |
| **Prompt Tokens (Total)** | **0** | `UNAVAILABLE` |
| **Prompt Tokens (Cached)** | **0** | `UNAVAILABLE` |
| **Prompt Tokens (Uncached)** | **0** | `UNAVAILABLE` |
| **Completion Tokens** | **0** | `UNAVAILABLE` |
| **Reasoning Tokens** | **0** | `UNAVAILABLE` |
| **Peak Context** | **None** | `UNAVAILABLE` |
| **Turns Completed** | **0** | `UNAVAILABLE` |
| **Wall-Clock Duration** | **0.00 seconds** | `UNAVAILABLE` |
| **Telemetry Validity** | **`False`** | Telemetry unavailable due to skip |
| **Error Message** | `Execution halted: stop-on-failure triggered by treatment_B (PRE_DISPATCH_HEADROOM_EXHAUSTED)` | Documented |

---

## 3. Functional Acceptance Evaluation

Because the baseline arm did not run:
- **L0 Typecheck:** `SKIPPED`
- **L1 Lint:** `SKIPPED`
- **L2 Unit Tests:** `SKIPPED`
- **L3 Build:** `SKIPPED`
- **Requirements (REQ-1..4):** `NOT_EVALUATED`
- **Files Modified:** 0
- **Correctness Score ($C$):** `0.00`
- **Functional Acceptance:** `FAIL`

---

## 4. Policy Compliance Verification

The skipping of `control_A` confirms full compliance with experimental safety rules:
1. **Financial Preservation:** By aborting upon treatment failure, the harness prevented wasting an additional 175,000 token budget on an unassisted run whose comparison would be scientifically invalid.
2. **Deterministic Halt:** No background tasks, orphan threads, or unmonitored processes remained running.
3. **No Retries:** The harness adhered strictly to the zero-retry mandate.
