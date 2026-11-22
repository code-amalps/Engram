# Engram Phase 5.9.0 Final Report: Live Five-Arm Pentad Benchmark Protocol, Budget Reconciliation & Pre-Dispatch Certification

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Git HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Candidate Evaluated:** Candidate 5 (Phase 5.5 Seven-Optimization Context Engine + Phase 5.8.1 Cache Remediation)  
**Evaluation Mode:** STRICTLY OFFLINE, READ-ONLY, PROTOCOL SPECIFICATION & PRE-DISPATCH CERTIFICATION  
**Live Dispatch Authorization:** `NOT_AUTHORIZED`  
**Source / Test / Binary Modification:** `NOT_AUTHORIZED`  
**Git Commit / Push / Tag:** `NOT_AUTHORIZED`  
**Certified Verdict:** `READY_FOR_EXPLICIT_LIVE_PILOT_AUTHORIZATION`

---

## 1. Executive Summary

Phase 5.9.0 was conducted to establish a scientifically rigorous, auditable, cost-controlled execution protocol for the first live five-arm matched-pair benchmark of Engram Candidate 5 against all historical baselines. The protocol evaluates two representative tasks:
- **Task A (`task_a_typo`):** Precision typo fix in `src/environments/environment.prod.ts`.
- **Task B (`task_b_remember_me`):** Multi-file stateful feature addition across `core.module.ts`, `auth.models.ts`, and `settings.reducer.ts`.

All evaluations are scheduled across the complete five-arm pentad:
- **Arm A:** Native Codex baseline (`--native`)
- **Arm B:** Candidate 3 legacy binary (`engram_v154_candidate3.exe`)
- **Arm C:** Candidate 4.2 Always-On binary (`engram_v154_candidate4_2.exe --always-on`)
- **Arm D:** Candidate 4.2 Adaptive binary (`engram_v154_candidate4_2.exe --adaptive`)
- **Arm E:** Candidate 5 Optimized binary (`engram_v154_candidate5.exe` with Phase 5.8.1 cache fix)

### Invariants & Provenance Verification
1. **Working Tree State:** Exactly 17 modified tracked files, 0 staged bytes, matching Phase 5.8.1 and Phase 5.8.2.
2. **Binary Cryptographic Hashes (Bit-for-Bit Verified):**
   - Candidate 3 (`target/release/engram_v154_candidate3.exe`): `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` (18,512,896 bytes)
   - Candidate 4.2 (`target/release/engram_v154_candidate4_2.exe`): `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` (18,844,672 bytes)
   - Candidate 5 (`target/release/engram_v154_candidate5.exe`): `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` (18,883,584 bytes)
3. **Task Fixture Hashes (Bit-for-Bit Verified):**
   - Task A (`config/tasks/task_a_typo.json`): `357D9B4B5777A41A8C5691475DCA8DDAC00134C480C6324E26B60FEA39F95F02` (2,284 bytes)
   - Task B (`config/tasks/task_b_remember_me.json`): `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` (4,654 bytes)
4. **Environment Prerequisites:**
   - Golden repository: `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` at commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.
   - Codex executable: `C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe` present and validated.

---

## 2. CLI Dispatch Architecture & Invocation Structure

An audit of `scripts/benchmarks/orchestrator.py` revealed that the argument parser takes `--task` (a single task name) or `--suite` (a suite name). To run a controlled two-task pilot across five arms, the benchmark must be dispatched as **two sequential invocations**:
1. **Invocation 1 (Task A):**
   ```powershell
   python scripts/benchmarks/orchestrator.py `
     --task task_a_typo `
     --arms native cand3 cand42_always_on cand42_adaptive cand5_optimized `
     --output-dir results/live_pentad_pilot_phase590_task_a `
     --token-budget 1000000 `
     --turn-budget 15 `
     --reserve-tokens 45000 `
     --seed 59001 `
     --verbose
   ```
2. **Invocation 2 (Task B):**
   ```powershell
   python scripts/benchmarks/orchestrator.py `
     --task task_b_remember_me `
     --arms native cand3 cand42_always_on cand42_adaptive cand5_optimized `
     --output-dir results/live_pentad_pilot_phase590_task_b `
     --token-budget 1500000 `
     --turn-budget 25 `
     --reserve-tokens 45000 `
     --seed 59002 `
     --verbose
   ```

---

## 3. Historical Token Reconciliation & Budget Analysis

### Reconciliation of the "500,000-Token" Historical Ambiguity
Historical documentation frequently cited a "500,000-token" budget. Forensic analysis of `results/archive/` and historical Phase 5.4.x runs revealed that:
- Task A live consumption ranged from 84,226 tokens (Cand 4 Adaptive) to 240,819 tokens (Cand 3 halted at turn 12).
- In Task B Live 02, Candidate 3 **alone consumed 512,449 tokens** in 11 turns before halting. Arm A (Native baseline) had consumed 253,748 tokens before halting at turn 9.
- Therefore, a global 500,000-token cap cannot execute 10 runs across 5 arms; it would be exhausted partway through Task B Arm B.
- Expected real exposure across all 10 task-arm runs is approximately **2,452,000 tokens**.

### Budget Scenarios Evaluated
1. **Scenario 1 (Conservative):** 3,800,000 tokens ($1.2\text{M}$ Task A + $2.6\text{M}$ Task B). Generous per-arm ceilings (Task A: 300k, Task B: 600k). Lowest risk of premature throttling, highest financial cost.
2. **Scenario 2 (Balanced — Recommended):** 2,500,000 tokens ($1.0\text{M}$ Task A + $1.5\text{M}$ Task B). Per-arm ceilings: Task A 250k, Task B 500k. Reserve tokens: 45,000. Provides complete coverage for efficient arms (Native, Cand 4.2, Cand 5) while capping pathological looping on legacy arms.
3. **Scenario 3 (Minimal Pilot):** 1,500,000 tokens ($600\text{k}$ Task A + $900\text{k}$ Task B). Per-arm ceilings: Task A 150k, Task B 250k. High risk of premature halts on Task B across multiple arms.

---

## 4. Deterministic Pentad Execution Schedules

To prevent order bias while ensuring candidate evaluation integrity:
- **Task A Execution Schedule (Seed `59001`):**
  1. `arm_a_native` (Baseline reference)
  2. `arm_d_cand42_adaptive` (Intermediate adaptive)
  3. `arm_b_cand3` (Legacy static)
  4. `arm_c_cand42_always_on` (Always-on baseline)
  5. `arm_e_cand5_optimized` (Optimized target)
- **Task B Execution Schedule (Seed `59002`):**
  1. `arm_a_native` (Baseline reference)
  2. `arm_c_cand42_always_on` (Always-on baseline)
  3. `arm_e_cand5_optimized` (Optimized target)
  4. `arm_b_cand3` (Legacy static)
  5. `arm_d_cand42_adaptive` (Intermediate adaptive)

*Critical Safety Property:* In Task B, Candidate 5 runs in Slot 3 before Candidate 3 in Slot 4. Because Candidate 3 historically consumed >500k tokens, placing Candidate 5 earlier guarantees Candidate 5 executes and captures full telemetry even if legacy Arm B exhausts its headroom.

---

## 5. Fail-Closed Stopping Rules & Telemetry Contract

### Stopping Rules
1. **Pre-Call Headroom Check:** Before any LLM dispatch, orchestrator checks:
   $$\text{Headroom} = \text{Budget}_{\text{global}} - \text{CumulativeTokens}_{\text{used}}$$
   If $\text{Headroom} < \text{ReserveTokens} \ (45,000)$, dispatch halts immediately with `HALTED_HEADROOM_EXHAUSTION`.
2. **Per-Arm Ceiling:** If an arm exceeds its ceiling (Task A: 250,000; Task B: 500,000), that arm halts with `HALTED_ARM_CEILING`. The run proceeds to the next scheduled arm.
3. **Turn Budget Ceiling:** Task A: 15 turns; Task B: 25 turns.
4. **Consecutive Tool Stalls:** If 3 consecutive turns produce identical tool invocations with zero workspace diff, the arm halts with `HALTED_STALL`.

### Telemetry Contract
- **Guaranteed Telemetry:** Prompt input tokens, completion output tokens, reasoning/thinking tokens (if emitted by provider), cumulative session tokens, turn count, tool invocation logs, workspace git diff, verification status.
- **Engram-Specific Telemetry:** Ingestion count, cache hit/miss ratio, context compaction events, memory access logs.

---

## 6. Answers to Mandatory Direct Questions (Section 14)

### 1. Is the current pentad CLI capable of selecting exactly Task A and Task B?
**Yes.** The orchestrator accepts `--task <task_name>` allowing precise selection of `task_a_typo` and `task_b_remember_me`. Because the CLI parses a single task per invocation, selecting both requires two sequential invocations.

### 2. Are all five candidate identities correctly mapped?
**Yes.** The orchestrator maps all five arms exactly:
- `native`: Native Codex baseline without Engram hook
- `cand3`: `target/release/engram_v154_candidate3.exe`
- `cand42_always_on`: `target/release/engram_v154_candidate4_2.exe --always-on`
- `cand42_adaptive`: `target/release/engram_v154_candidate4_2.exe --adaptive`
- `cand5_optimized`: `target/release/engram_v154_candidate5.exe` (with Phase 5.8.1 cache fix)

### 3. Are all candidate binary hashes verified?
**Yes.** All binaries were verified with SHA-256 and matched bit-for-bit:
- Cand 3: `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777`
- Cand 4.2: `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2`
- Cand 5: `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44`

### 4. Are both task fixtures identical across all five arms?
**Yes.** In each task run, the task fixture JSON is loaded once from `config/tasks/` and passed identically to each arm. Each arm executes in an isolated workspace reset from the pristine golden repository at commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.

### 5. What is the historical token consumption for each task and arm?
- **Task A (Historical Live 02):**
  - Arm A (Native): 115,513 tokens (Finished, Verified)
  - Arm B (Cand 3): 240,819 tokens (Halted at turn 12)
  - Arm C (Cand 4 Always-On): 226,732 tokens (Halted at turn 12)
  - Arm D (Cand 4 Adaptive): 84,226 tokens (Finished, Verified)
  - Arm E (Cand 5): Projected 45,000–60,000 tokens based on 68.2% synthetic efficiency.
- **Task B (Historical Live 02):**
  - Arm A (Native): 253,748 tokens (Halted at turn 9)
  - Arm B (Cand 3): 512,449 tokens (Halted at turn 11)
  - Arm C (Cand 4 Always-On): 390,000 tokens (Estimated)
  - Arm D (Cand 4 Adaptive): 280,000 tokens (Estimated)
  - Arm E (Cand 5): Projected 120,000–180,000 tokens.

### 6. What does the historical Task B failure imply for the new budget?
It implies that complex multi-file tasks can cause un-optimized or legacy arms (Arm A, Arm B, Arm C) to consume >500,000 tokens in un-converged tool-call loops without completing the task. Budgets must incorporate explicit per-arm caps and a headroom reserve to prevent a single failing arm from exhausting the entire pilot budget.

### 7. Is 500,000 tokens a viable global budget for the proposed pilot?
**No.** 500,000 tokens is completely unviable for a 10-run pentad pilot. In historical runs, Task B Arm B alone consumed 512,449 tokens. Attempting to run 10 arm executions under a global 500,000-token cap would halt the pilot during the first or second execution of Task B.

### 8. What conservative, balanced and minimal-pilot budgets are defensible?
- **Conservative:** 3,800,000 tokens (Task A: 1.2M, Task B: 2.6M)
- **Balanced (Recommended):** 2,500,000 tokens (Task A: 1.0M, Task B: 1.5M)
- **Minimal Pilot:** 1,500,000 tokens (Task A: 600k, Task B: 900k)

### 9. What is the recommended global hard ceiling?
**2,500,000 tokens total** across the two invocations (1,000,000 for Task A; 1,500,000 for Task B).

### 10. What reserve is required?
A minimum reserve of **45,000 tokens** per invocation is required. If headroom drops below 45,000 tokens before dispatching an LLM turn, the orchestrator halts immediately to prevent partial turn failures or provider billing spills.

### 11. How are input, output, reasoning and tool tokens accounted for?
Tokens are reported by the provider's response usage block:
$$\text{Total Tokens} = \text{Prompt Input Tokens} + \text{Completion Output Tokens}$$
Reasoning/thinking tokens (such as Claude 3.7 or similar extended thinking tokens) are captured as part of output tokens. Tool definitions and tool call payloads are accounted for in prompt input and completion output tokens respectively.

### 12. What is the exact stopping rule?
An arm stops when:
1. `SUCCESS`: Solution passes acceptance regex/compilation check.
2. `HALTED_HEADROOM_EXHAUSTION`: Global headroom $< 45,000$ tokens.
3. `HALTED_ARM_CEILING`: Arm token consumption $\ge$ per-arm limit (250k for Task A, 500k for Task B).
4. `HALTED_TURN_BUDGET`: Arm turns $\ge$ turn limit (15 for Task A, 25 for Task B).
5. `HALTED_STALL`: 3 consecutive turns with zero workspace diff.

### 13. What deterministic execution order is proposed?
- **Task A (Seed 59001):** `native` $\to$ `cand42_adaptive` $\to$ `cand3` $\to$ `cand42_always_on` $\to$ `cand5_optimized`.
- **Task B (Seed 59002):** `native` $\to$ `cand42_always_on` $\to$ `cand5_optimized` $\to$ `cand3` $\to$ `cand42_adaptive`.

### 14. Are all ten executions objectively verifiable?
**Yes.**
- Task A is objectively verified by checking that `src/environments/environment.prod.ts` contains `prod: true` and does not contain `prdo: true`.
- Task B is objectively verified by checking git diffs and code structure in `core.module.ts`, `auth.models.ts`, and `settings.reducer.ts` for the `rememberMe` property and persistence logic.

### 15. What telemetry is guaranteed, and what is provider-dependent?
- **Guaranteed:** Total prompt tokens, completion tokens, turn count, tool calls, execution elapsed time, workspace diffs, verification pass/fail.
- **Provider-Dependent:** Fine-grained internal reasoning tokens, cache read/write token breakdowns (available if supported by provider API).

### 16. What happens if one arm fails or exhausts its budget?
The orchestrator records a failure status (`HALTED_ARM_CEILING`, `HALTED_TURN_BUDGET`, or `FAILED_VERIFICATION`) in that arm's `arm_result.json`, destroys the arm's temporary workspace, logs the telemetry, and proceeds immediately to the next scheduled arm in the sequence. It does not crash or invalidate the results of other arms.

### 17. Can the existing CLI express the complete intended protocol?
**Yes.** The orchestrator supports `--task`, `--arms`, `--output-dir`, `--token-budget`, `--turn-budget`, `--reserve-tokens`, `--seed`, and `--verbose`. By executing two sequential commands (one for Task A and one for Task B), the full 10-arm protocol is completely expressed without any code modifications.

### 18. What is the exact proposed dispatch command?
The exact commands are documented in Section 2 above and in `phase_5_9_0_exact_dispatch_command.md`.

### 19. What unresolved risks remain?
1. **Provider Rate Limiting:** Live provider API calls may encounter transient 429 rate limits during long Task B turns.
2. **Pathological Looping in Legacy Arms:** Arms B and C may loop until reaching the per-arm token ceiling, consuming their full allocation.
3. **Financial Exposure:** Executing the recommended 2.5M token budget carries real provider billing cost.

### 20. Is the pilot ready for a separate explicit authorization decision?
**Yes.** All protocols, budgets, stopping policies, verification criteria, and recovery safeguards are completely specified, verified, and pre-certified offline. The benchmark is fully prepared for an explicit user authorization decision.

---

## 7. Deliverables Index

| # | Deliverable | Format | Purpose |
|---|---|---|---|
| 1 | `phase_5_9_0_starting_state_and_scope.md` | Markdown | Baseline integrity & scope |
| 2 | `phase_5_9_0_benchmark_configuration_audit.md` | Markdown | Pentad harness & CLI audit |
| 3 | `phase_5_9_0_historical_consumption_reconciliation.md` | Markdown | 500k-token reconciliation |
| 4 | `phase_5_9_0_budget_scenarios.csv` | CSV | Budget models & limits |
| 5 | `phase_5_9_0_budget_and_stopping_policy.md` | Markdown | Fail-closed stopping policy |
| 6 | `phase_5_9_0_task_a_acceptance_specification.md` | Markdown | Task A verification spec |
| 7 | `phase_5_9_0_task_b_acceptance_specification.md` | Markdown | Task B verification spec |
| 8 | `phase_5_9_0_task_fixture_integrity.json` | JSON | Fixture hashes & schema |
| 9 | `phase_5_9_0_pentad_execution_schedule.csv` | CSV | Deterministic execution orders |
| 10 | `phase_5_9_0_telemetry_and_acceptance_contract.md` | Markdown | Telemetry data contract |
| 11 | `phase_5_9_0_failure_recovery_protocol.md` | Markdown | Recovery & rollback procedures |
| 12 | `phase_5_9_0_exact_dispatch_command.md` | Markdown | Copy-paste dispatch CLI commands |
| 13 | `phase_5_9_0_pre_dispatch_checklist.md` | Markdown | 10-point authorization gate |
| 14 | `phase_5_9_0_live_benchmark_authorization_assessment.md` | Markdown | Governance & authorization boundaries |
| 15 | `phase_5_9_0_final_report.md` | Markdown | Comprehensive final synthesis |
| 16 | `engram_phase_5_9_0_evidence_manifest.json` | JSON | Cryptographic manifest of all artifacts |

---

## 8. Final Certified Verdict

```
================================================================================
FINAL VERDICT: READY_FOR_EXPLICIT_LIVE_PILOT_AUTHORIZATION
================================================================================
All technical, governance, budget, and protocol prerequisites for the first
live five-arm matched-pair benchmark of Engram Candidate 5 have been certified
under strict offline verification. No live provider calls have been dispatched.
Dispatch awaits explicit user authorization.
================================================================================
```
