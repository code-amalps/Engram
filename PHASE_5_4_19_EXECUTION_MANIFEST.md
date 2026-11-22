# Engram Phase 5.4.19: Task B Live Tetrad Execution Manifest

**Manifest Version:** `1.0.0`  
**Phase:** 5.4.19  
**Execution Mode:** Strictly Offline Preparation and Read-Only Verification  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Target Live Experiment Identifier:** `task_b_remember_me_tetrad_live_03`  
**Target Block Identifier:** `tetrad_live_03`  
**Authorization Status:** `NOT_AUTHORIZED`  
**Manifest Verdict:** `CONDITIONALLY_READY` (Requires explicit user authorization and provider quota confirmation)

---

## 1. Executive Summary & Purpose

This execution manifest defines an immutable, reproducible, cost-bounded specification for conducting the next four-arm matched-pair benchmark experiment on Task B (`task_b_remember_me`).

Following the adversarial audit of Phase 5.4.18 (which established harness dispatch safety and acceptance gate integrity), this manifest locks all experimental dimensions—cryptographic identities, arm configurations, deterministic scheduling, token budget ceilings, headroom reserves, telemetry validity criteria, comparative evaluation logic, and authorization requirements—so that a future execution phase can proceed without ambiguous assumptions, runtime guesswork, or silent parameter drifts.

> [!CAUTION]
> **STRICT EXECUTION RESTRICTION:**  
> This document specifies an intended experiment. **It does NOT authorize dispatch.** Execution remains prohibited until explicit, written user authorization is granted.

---

## 2. Authoritative Baseline & Cryptographic Identities

Every binary, fixture, and repository dependency is cryptographically sealed. If any component produces a checksum mismatch at pre-dispatch time, the run must fail closed immediately.

### 2.1 Repository & Fixture Seals

| Component | Target Identity | Absolute / Relative Path | Cryptographic Status |
|---|---|---|---|
| **Git Baseline Commit** | `7106a2f369fe79faf5f95449378a900ab8e8eddf` | `c:\Users\Amal P S\OneDrive\Projects\Engram` | **CONFIRMED** |
| **Git Working Branch** | `feature/phase2-stage5-remediation` | `feature/phase2-stage5-remediation` | **CONFIRMED** |
| **Task B Fixture** | `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` | `benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json` | **SEALED (4,654 bytes)** |
| **Golden Repository** | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` | `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` | **SEALED** |
| **Phase 5.4.18 Findings** | `E8B861AF6095E9CD307A44A3F2334AA5FD7E855FF0AB596F7AA057C808E489D0` | `benchmarks/matched-pair-harness/reports/phase5_4_18_findings.json` | **VERIFIED (6,200 bytes)** |
| **Phase 5.4.18 Report** | `1FB9B5D34E987D6A6A0FB2C6386824C9A2B15D999CCEE5A58EA98922E7FC776D` | `PHASE_5_4_18_ADVERSARIAL_AUDIT_REPORT.md` | **VERIFIED (20,963 bytes)** |

### 2.2 Candidate Binary Seals

```
Candidate 3 (arm_b_cand3_always_on):
  Path:    benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe
  SHA-256: 97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777
  Size:    18,668,032 bytes
  Status:  FROZEN & SEALED

Candidate 4.2 (arm_c_cand4_always_on & arm_d_cand4_adaptive):
  Path:    benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe
  SHA-256: 78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2
  Size:    18,802,176 bytes
  Status:  FROZEN & SEALED
```

---

## 3. Four-Arm Experimental Design & Architecture

The benchmark evaluates four strictly isolated arms under identical task prompts, repository baselines, and evaluation criteria.

```
+---------------------------------------------------------------------------------------------------+
|                                      TASK B TETRAD ARCHITECTURE                                   |
+---------------------------------------------------------------------------------------------------+
|  Arm A: Native Baseline               | No Engram MCP tools. Standard search/grep baseline.       |
|  Arm B: Candidate 3 Always-On         | Original Engram binary. Unpruned get_context payload.      |
|  Arm C: Candidate 4.2 Always-On       | Remediated engine. Strict bounded output schemas.         |
|  Arm D: Candidate 4.2 Adaptive        | Remediated engine + Adaptive complexity-aware activation.  |
+---------------------------------------------------------------------------------------------------+
```

### 3.1 Arm Specifications

| Arm ID | Architectural Role | Binary Version | MCP Registration | Routing Policy | Experimental Purpose |
|---|---|---|---|---|---|
| **Arm A** | Native Baseline | `native` (None) | **Disabled** (`[]`) | None | Ground-truth baseline for token spend, wall-clock time, and correctness without Engram. |
| **Arm B** | Candidate 3 Treatment | `cand3` | **Enabled** (`engram.exe`) | `ALWAYS_ON` | Quantifies historical token overhead from unpruned auxiliary payloads (>120 kB). |
| **Arm C** | Candidate 4.2 Treatment | `cand4_2` | **Enabled** (`engram.exe`) | `ALWAYS_ON` | Isolates impact of engine response-boundary remediation and bounded context schema. |
| **Arm D** | Candidate 4.2 Adaptive | `cand4_2` | **Enabled** (`engram.exe`) | `ADAPTIVE` | Isolates incremental token savings from complexity-aware dynamic MCP routing. |

### 3.2 Controlled Invariants Across Arms
1. **Identical Task Prompt:** Exact prompt text from `task_b_remember_me.json` (4,654 bytes).
2. **Identical Base Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` of `angular-ngrx-material-starter`.
3. **Identical Model & Temperature:** `gpt-5.6-luna`, default sampling, reasoning effort `medium`.
4. **Identical Isolation Strategy:** Full cold-start directory copy (`copy` strategy) in clean temp sandboxes.
5. **Identical Verification Gates:** L0 Typecheck (`tsc`), L2 Spec Tests (`tsconfig.spec.json`), L3 Production Build (`npm run build`).
6. **Identical Evaluation Logic:** Automated static requirement checks (REQ-1 to REQ-4) scored by `evaluator.py`.

---

## 4. Experimental Comparisons & Validity Rules

To protect against misleading scientific claims, all pairwise comparisons are governed by strict mutual acceptance rules.

| Comparison | Contrast | Experimental Hypothesis | Validity Precondition |
|---|---|---|---|
| **A vs B** | Candidate 3 vs Native | Candidate 3 incurs massive prompt inflation due to unpruned payloads. | **Valid ONLY IF** Arm A and Arm B both achieve `execution_status == SUCCESS` with valid telemetry. |
| **B vs C** | Candidate 4.2 vs Candidate 3 | Candidate 4.2 response bounding eliminates turn-by-turn context explosion. | **Valid ONLY IF** Arm B and Arm C both achieve `execution_status == SUCCESS` with valid telemetry. |
| **C vs D** | Adaptive vs Always-On | Adaptive activation reduces token consumption on low-complexity turns. | **Valid ONLY IF** Arm C and Arm D both achieve `execution_status == SUCCESS` with valid telemetry. |
| **A vs D** | Candidate 4.2 Adaptive vs Native | Candidate 4.2 delivers net token efficiency while matching/exceeding native correctness. | **Valid ONLY IF** Arm A and Arm D both achieve `execution_status == SUCCESS` with valid telemetry. |

> [!IMPORTANT]
> **Negative Invalidation Invariant:**  
> If an arm fails or exhausts headroom (e.g. Arm B halting at turn 11), that failure demonstrates operational boundary limits, but **does not permit computing percentage savings against an incomplete arm**. A failed arm receives score `0.0` and invalidates comparative efficiency ratios.

---

## 5. Model, Provider & Runtime Environment

### 5.1 Configuration Ledger

| Parameter | Manifest Value | Local Verification Status | Runtime Notes |
|---|---|---|---|
| **Model Identifier** | `gpt-5.6-luna` | Locally parsed & validated | CLI flag `-m gpt-5.6-luna` |
| **Reasoning Effort** | `medium` | Locally parsed & validated | CLI flag `-c model_reasoning_effort="medium"` |
| **Provider** | `cloud` (`openai`) | Locally validated in `SUPPORTED_PROVIDERS` | Standard cloud endpoint dispatch |
| **Dependency Strategy** | `copy` | Locally validated | Full write-isolated sandbox copy |
| **Codex CLI Path** | `C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe` | **EXISTS** | Binary present on disk |
| **Codex CLI Version** | `codex-cli 0.167.0-alpha.4.1` | **VERIFIED** | CLI version reported via `--version` |
| **Python Runtime** | `Python 3.14.7` (MSC v.1944 64-bit AMD64) | **VERIFIED** | Active interpreter |
| **Host OS** | `Windows 11` (`10.0.26300-SP0`) | **VERIFIED** | Host OS environment |
| **Remote API Endpoint**| Cloud provider API | `REQUIRES_CONFIRMATION` | Offline audit cannot ping remote endpoints |
| **Account Quota** | Active token balance | `REQUIRES_CONFIRMATION` | User must confirm sufficient account billing quota |

---

## 6. Token Budget, Cost Boundaries & Mathematical Consistency

### 6.1 Budget Architecture & Reconciliation

The historical Task B Live 02 run demonstrated the critical need to distinguish between **per-arm ceilings** and **global block ceilings**:
- In Live 02, `--max-tokens-ceiling 550000` was specified.
- Arm B consumed **512,449 model tokens** before its remaining budget ($37,551$) dropped below the estimated next-turn context ($56,611$) and headroom reserve ($45,000$), triggering a safe preventive halt.
- If a global ceiling of $550,000$ were applied across the entire tetrad, Arm B alone would have consumed $93.2\%$ of the block budget, leaving only $37,551$ tokens for the remaining three arms combined. Arms D, A, and C would have been immediately aborted without dispatch under `ABORTED_GLOBAL_BUDGET`.

### 6.2 Mathematical Budget Model for Tetrad

```
Per-Arm Token Ceiling:                   550,000 tokens
Headroom Reserve:                         45,000 tokens
Pre-Dispatch Guard Threshold:            max(Estimated Next Turn Context, Headroom Reserve)
Global Tetrad Token Ceiling:           2,200,000 tokens (4 arms x 550,000 max per arm)
Effective Arm Ceiling Formula:           min(Per-Arm Ceiling, Global Remaining Budget)
```

### 6.3 Maximum Token Exposure & Stopping Point Scenarios

| Scenario | Execution Sequence | Stop Condition | Maximum Model Token Exposure |
|---|---|---|---|
| **Scenario 1: Live 02 Reproduction** | Arm B runs first | Arm B halts at Turn 11 ($37,551 < 45,000$) under strict policy | **~512,449 tokens** |
| **Scenario 2: Arm B Halts, Arm D Completes** | Alternative order (D first, then B) | Arm D succeeds (~280k), Arm B halts (~512k) | **~792,000 tokens** |
| **Scenario 3: Three Arms Complete, Arm B Halts** | Alternative order (A, D, C, then B) | A (~450k) + D (~280k) + C (~380k) + B (~512k) | **~1,622,000 tokens** |
| **Scenario 4: All Four Arms Complete** | All 4 arms succeed | Sum of 4 completed runs | **~1,500,000 – 1,850,000 tokens** |
| **Theoretical Maximum Ceiling** | All 4 arms consume full ceiling | Global ceiling cap | **2,200,000 tokens** |

### 6.4 Token Measurement Semantics
- **Total Model Tokens:** $\text{Prompt Tokens (Total)} + \text{Completion Tokens}$.
- **Prompt Tokens:** $\text{Cached Prompt Tokens} + \text{Uncached Prompt Tokens}$.
- **Completion Tokens:** $\text{Reasoning Tokens} + \text{Output Tokens}$. Reasoning tokens are strictly an informative subset of completion tokens and are **never double-counted**.
- **Auxiliary Bytes:** JSON-RPC payload bytes transferred via stdio pipes. Measured separately; **never conflated with model tokens**.
- **Monetary Cost:** `REQUIRES_CONFIRMATION`. Upstream provider billing rates per token class (cached vs uncached prompt vs reasoning) cannot be audited offline.

---

## 7. Execution Ordering & Deterministic Scheduling

### 7.1 Baseline Historical Order (Seed `2330`)
- **Order:** `arm_b_cand3_always_on` → `arm_d_cand4_adaptive` → `arm_a_native` → `arm_c_cand4_always_on`
- **Derivation:** Deterministically produced by `random.Random(2330).shuffle(...)`.
- **Isolation Guarantee:** Zero cross-arm state contamination. Every arm executes in a freshly cloned, cold-start filesystem sandbox with independent environment variables and temporary directory trees.
- **Experimental Implication:** Under strict execution policy and the frozen Candidate 3 binary, running Arm B first will test whether Candidate 3 deterministically reproduces its token exhaustion. If it does, strict policy halts before dispatching Arm D, A, or C, preserving token expenditure.

### 7.2 Proposed Alternative Order (Requires Explicit User Approval)
- **Proposed Order:** `arm_a_native` → `arm_d_cand4_adaptive` → `arm_c_cand4_always_on` → `arm_b_cand3_always_on`
- **Experimental Justification:** If the user's primary goal is to observe Candidate 4.2's live multi-turn performance and compare it against Native baseline, running Arm B last ensures that Native, Candidate 4.2 Adaptive, and Candidate 4.2 Always-On collect complete execution data before Candidate 3 reaches its projected budget limit.
- **Status:** Documented as an option; **requires explicit user authorization to deviate from seed `2330`**.

---

## 8. Failure and Continuation Policy

### 8.1 Primary Policy: Strict Stop-on-Failure
The live experiment must execute with `--execution-policy strict` (`stop_on_failure = True`).

Dispatch must halt immediately on any of the following:
1. `UNKNOWN_OR_INVALID_TELEMETRY`: Telemetry missing, unparseable, negative, zero-token anomaly, or arithmetic mismatch.
2. `PRE_DISPATCH_HEADROOM_EXHAUSTED`: Remaining arm budget $< \max(\text{estimated next turn}, \text{headroom reserve})$.
3. `ABORTED_GLOBAL_BUDGET`: Cumulative token spend reaches global ceiling.
4. `ABORTED_SAFETY_FAILURE`: Candidate binary SHA-256 mismatch, golden repository mismatch, or sandbox policy breach.
5. `SAFETY_VIOLATION`: Runner over-ceiling execution returning `SUCCESS`.
6. `CANCELLED_BY_USER`: Explicit keyboard interrupt or kill signal.

### 8.2 Execution Status Mapping

| Event / Outcome | Arm Execution Status | Tetrad Block Status | Trailing Arms Status |
|---|---|---|---|
| Complete requirement fulfillment & typecheck pass | `SUCCESS` | `RUNNING` | Dispatched in order |
| Headroom reserve exhaustion | `PRE_DISPATCH_HEADROOM_EXHAUSTED` | `HALTED_STOP_ON_FAILURE` | `SKIPPED_STOP_ON_FAILURE` |
| Malformed or arithmetically invalid telemetry | `TELEMETRY_INVALID` | `ABORTED_UNCERTAIN_BUDGET` | `SKIPPED_UNCERTAIN_BUDGET` |
| Cumulative spend exceeds global ceiling | `GLOBAL_CEILING_REACHED` | `ABORTED_GLOBAL_BUDGET` | `SKIPPED_GLOBAL_BUDGET` |
| Binary hash or sandbox integrity breach | `INTEGRITY_MISMATCH` | `ABORTED_SAFETY_FAILURE` | `SKIPPED_SAFETY_FAILURE` |
| User cancellation via SIGINT | `INTERRUPTED` | `CANCELLED_BY_USER` | `SKIPPED_USER_CANCELLED` |

---

## 9. Acceptance Gates & Verification Integrity

An arm is classified as **ACCEPTED** if and only if all ten criteria are met:

```
[ ] 1. execution_status == "SUCCESS"
[ ] 2. evaluator_score == 1.0 (100% on REQ-1 through REQ-4)
[ ] 3. passed_all_requirements is True
[ ] 4. top-level telemetry_valid is True
[ ] 5. nested telemetry.telemetry_valid is True
[ ] 6. total_model_tokens > 0 (Strict integer, positive)
[ ] 7. prompt_tokens_total + completion_tokens == total_model_tokens
[ ] 8. prompt_tokens_cached + prompt_tokens_uncached == prompt_tokens_total
[ ] 9. run_id and arm match expected isolation metadata
[ ] 10. telemetry provenance is DIRECT_API (SYNTHETIC_TEST rejected)
```

Mutual acceptance across all four arms is required before any comparative claims of superiority can be asserted.

---

## 10. Measurement & Statistical Safeguards

### 10.1 Functional & Efficiency Metrics
- **Functional:** Requirement fulfillment (`REQ-1` to `REQ-4`), build gate pass/fail (`L0`, `L2`, `L3`), patch size, modified file count.
- **Efficiency:** Cumulative model tokens, prompt cached vs uncached tokens, completion & reasoning tokens, turns count, MCP tool call count, aggregate MCP auxiliary payload bytes, peak turn context, wall-clock duration.
- **Comparative:** Absolute and percentage deltas between accepted arms relative to Native (Arm A).

### 10.2 Statistical Safeguards
1. **Single Tetrad Observation:** A single 4-arm tetrad provides an observational case study; it does **not** establish formal statistical significance.
2. **Task Specificity:** Results on Task B reflect Angular authentication patterns and cannot be generalized across all software engineering domains without multi-benchmark evaluation.
3. **Replication Strategy:** A minimum of 5 matched blocks with randomized, counterbalanced schedules is required for robust hypothesis testing.

---

## 11. Command-Line Invocation Specification

When explicit authorization is granted, the future execution phase must invoke the orchestrator with the following exact, parser-validated arguments:

### Baseline Execution Command (Seed `2330`, Historical Order `B → D → A → C`):
```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --mode tetrad `
  --task task_b_remember_me `
  --block tetrad_live_03 `
  --candidate-version cand4_2 `
  --provider cloud `
  --model gpt-5.6-luna `
  --reasoning medium `
  --dependency-strategy copy `
  --max-tokens-ceiling 550000 `
  --global-tokens-ceiling 2200000 `
  --headroom-reserve 45000 `
  --execution-policy strict
```

### Alternative Execution Command (Alternative Order `A → D → C → B`):
```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --mode tetrad `
  --task task_b_remember_me `
  --block tetrad_live_03_alt `
  --candidate-version cand4_2 `
  --provider cloud `
  --model gpt-5.6-luna `
  --reasoning medium `
  --dependency-strategy copy `
  --max-tokens-ceiling 550000 `
  --global-tokens-ceiling 2200000 `
  --headroom-reserve 45000 `
  --execution-policy strict `
  --arm-order arm_a_native arm_d_cand4_adaptive arm_c_cand4_always_on arm_b_cand3_always_on
```

---

## 12. Authorization Gate & Sign-off

```
+---------------------------------------------------------------------------------------------------+
|                                     AUTHORIZATION STATUS                                          |
+---------------------------------------------------------------------------------------------------+
|  CURRENT STATUS:                 NOT_AUTHORIZED                                                   |
|  DISPATCH PERMISSION:            DENIED                                                           |
|  ACTION REQUIRED:                Wait for explicit user instruction specifically authorizing      |
|                                  live dispatch of Task B Tetrad Live 03.                          |
+---------------------------------------------------------------------------------------------------+
```
