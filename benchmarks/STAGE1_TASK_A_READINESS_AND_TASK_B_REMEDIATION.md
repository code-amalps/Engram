# Engram — Stage 1 Core Triad: Task A Continuation Readiness & Task B Remediation Report

**Date:** 2026-10-28  
**Scope:** Preparation and offline validation for Stage 1 Core Triad (Blocks 02–10). Zero live model calls made.  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram` (Branch: `feature/async-precommit-guard`, Commit: `c70845b`)  
**Golden Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` (Commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`)  
**Status:** **`TASK A READY — TASK B REMEDIATED OFFLINE (AWAITING AUTHORIZATION)`**

---

## 1. Executive Summary

This report documents the preparation for Task A continuation (`block_02` through `block_05`) and the complete offline diagnostic and remediation of Task B (`block_06` through `block_10`).

### Current Status
1. **Task A (Blocks 02–05, 12 Executions):** **READY FOR EXPLICIT AUTHORIZATION.**  
   All acceptance criteria, binary hashes, schemas, and sandboxing protections remain verified. Quota exposure is conservatively estimated at ~830k tokens ($0.00 USD direct billing).
2. **Task B (Blocks 06–10, 15 Executions):** **REMEDIATED OFFLINE & VERIFIED.**  
   The three critical blockers identified in `STAGE1_SMOKE_TEST_AUDIT.md` (invalid dev-build command, broken global lint gate, and missing static evaluator check) have been remediated:
   - Restored `task_b_remember_me.json` to the authoritative **Authentication Flow** defined in `PREREGISTRATION.md` and `verify_task_b.ps1`.
   - Replaced invalid gate commands with verified TypeScript application compile (`L0`), spec compile (`L2`), and production build (`L3`) gates.
   - Implemented genuine static inspection for all Task B requirements in `evaluator.py`, eliminating the previous unconditional `return True` fallthrough.
   - Added 14 focused rejection/acceptance unit tests in `test_task_b_evaluator.py`; all 14 passed.
   - Full offline harness regression suite passed: **90/90 tests passed in 80.55s**.
3. **Execution Guard:** **Zero live model/API calls were launched.** All work was performed in offline sandboxes and test fixtures.

---

## 2. Part A — Task A Continuation Readiness (Blocks 02–05)

### 1. Invariant & Configuration Verification
- **Task Config:** `config/tasks/task_a_typo.json` verified. Target file `projects/.../about.component.html`, typo `'Angular Cli Tips'` $\rightarrow$ `'Angular CLI Tips'`.
- **Cryptographic Hashes:**
  - Engram v1.5.1: `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` (Matches `FREEZE_SEAL.md`).
  - Engram v1.5.4: `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` (Matches `AMENDMENT_02_STAGE1_CORE_TRIAD.md`).
- **Telemetry & Summary Schemas:** `run_telemetry.schema.json` and `triad_summary.schema.json` strictly enforced.
- **Cold-Start Sandboxing:** `dependency_strategy="copy"` provisions independent copies of `node_modules` in `%TEMP%\engram_pilot\runs\`. Golden repository dependency tree SHA-256 fingerprint verified pre- and post-execution.
- **Invalid-Run Exclusion:** Strict programmatic guard suppresses savings claims if any arm achieves $C < 1.00$.

### 2. Scheduler & Counterbalancing Analysis
The schedule in `orchestrator.py::TRIAD_BLOCK_SCHEDULES` specifies the following orders for Task A:
- **Block 01:** `["baseline_arm1", "v151_arm2", "v154_arm3"]` (Order 1) — *Completed*
- **Block 02:** `["v151_arm2", "v154_arm3", "baseline_arm1"]` (Order 2) — *Remaining*
- **Block 03:** `["v154_arm3", "baseline_arm1", "v151_arm2"]` (Order 3) — *Remaining*
- **Block 04:** `["baseline_arm1", "v154_arm3", "v151_arm2"]` (Order 4) — *Remaining*
- **Block 05:** `["v151_arm2", "baseline_arm1", "v154_arm3"]` (Order 5) — *Remaining*

#### Methodological Clarification on Latin Square Counterbalancing:
> [!IMPORTANT]
> This schedule is an **unbalanced fractional permutation schedule**, **NOT a balanced Latin square**.
> - In a 3-arm experiment, a balanced Latin square requires the number of rows $N$ to be a multiple of the number of treatments ($k = 3$, or $2k = 6$ for digram balance). With $N = 5$ repetitions, mathematical balance is impossible.
> - Positional Distribution across Blocks 01–05:
>   - **Position 1 (First to run):** Arm 1 appears 2 times; Arm 2 appears 2 times; **Arm 3 appears 1 time**.
>   - **Position 2 (Second to run):** Arm 1 appears 2 times; Arm 2 appears 1 time; **Arm 3 appears 2 times**.
>   - **Position 3 (Third to run):** Arm 1 appears 1 time; Arm 2 appears 2 times; **Arm 3 appears 2 times**.
> - Because Arm 3 starts first only once, it bears initial cold-start OS disk caching less frequently than Arms 1 and 2. Downstream reporting will explicitly disclose this minor structural asymmetry.

### 3. Execution Count & Conservative Quota Exposure
- **Remaining Task A Blocks:** 4 blocks (`block_02` through `block_05`).
- **Runs per Block:** 3 arms.
- **Exact Execution Count:** $4 \times 3 =$ **12 executions**.

#### Telemetry Projections & Uncertainty Labeling
Based on the observed `block_01` data:
- Arm 1: 64,845 tokens
- Arm 2: 76,674 tokens
- Arm 3: 66,184 tokens
- Average per arm: ~69,234 tokens.

| Metric Category | Observed `block_01` (3 Runs) | Projected Remaining Task A (12 Runs) |
| :--- | :---: | :---: |
| **Prompt Tokens (Total)** | 205,782 | **~820,000 tokens** |
| **Prompt Tokens (Cached)** | 171,264 (83.2%) | **~660,000 tokens** |
| **Prompt Tokens (Uncached)** | 34,518 (16.8%) | **~160,000 tokens** |
| **Completion Tokens** | 1,921 | **~7,700 tokens** |
| **Total Model Tokens** | 207,703 | **~830,000 tokens** |
| **Model Requests / Turns** | 13 turns | **~52 turns** |
| **Actual Cash Cost** | **$0.00 USD** | **$0.00 USD** (Subscription Auth) |
| **Commercial Value Equivalent** | ~$0.32 USD | **~$1.25 – $1.40 USD** |

> [!WARNING]
> **UNCERTAINTY LABELING:**  
> These projections are estimates with moderate statistical uncertainty ($\pm 15\%$). Model token usage varies per execution due to non-deterministic reasoning paths, variable prompt caching hit rates (which ranged from 64.7% to 93.0% in block 01), and the agent's turn count (which ranged from 4 to 5 turns).

---

## 3. Part B — Task B Diagnosis & Defensible Remediation

### 1. Root Cause of the Scope Discrepancy
A detailed audit of the repository history revealed how the discrepancy arose:
1. **Authoritative Specification:**
   - In `benchmarks/cost-baseline/PREREGISTRATION.md` (Section 4), `task_b.txt`, `verify_task_b.ps1`, and `AMENDMENT_02_STAGE1_CORE_TRIAD.md` (Section 3), Task B is defined as:
     > *"Add a remember-me option to the authentication flow that persists the username across reloads."*
     > Allowed Scope: `projects/.../src/app/core/auth/*`, `app/app.component.*`, `assets/i18n/en.json`.
2. **Harness Configuration Divergence:**
   - In commit `36120a7` (Oct 18), `task_b_remember_me.json` was authored targeting the **settings state** (`settings/**` and `core/settings/**`).
   - `evaluator.py` was never updated to check these settings requirements, and gates were configured with crashing commands.

### 2. Smallest Defensible Resolution
To maintain scientific validity and adhere to the rule of **not changing the frozen preregistration**, the smallest defensible resolution is to:
1. **Align `task_b_remember_me.json` to the authoritative `PREREGISTRATION.md` Authentication Flow:**
   - Prompt: `"Add a remember-me option to the authentication flow that persists the username across reloads. CRITICAL SAFETY INSTRUCTION: Do NOT run npm install or modify package.json or node_modules under any circumstances."`
   - Allowed patterns: `core/auth/**`, `app.component.*`, `assets/i18n/en.json`.
2. **Align Automated Gates to Preregistered Checks:**
   - `L0_typecheck`: Application build & typecheck gate (`npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit`). Verified exit code 0.
   - `L1_lint`: Disabled (`enabled: false`), matching `task_a_typo.json`. Resolves 141 pre-existing ESLint syntax definition errors in the golden repository.
   - `L2_unit_tests`: Spec typecheck gate (`npx tsc -p projects/angular-ngrx-material-starter/tsconfig.spec.json --noEmit`). Verified exit code 0.
   - `L3_production_build`: Production build gate (`npm run build`). Verified exit code 0.
3. **Implement Full Requirements & Static Evaluation in `evaluator.py`:**
   - `REQ-1` (UI Elements): Check `app.component.html` (uncommented) for remember-me checkbox and username input.
   - `REQ-2` (Persistence Logic): Check `app.component.ts` or `auth.effects.ts` for `localStorage` restoration logic.
   - `REQ-3` (Security Negative Rule): Scan `app.component.ts` and `auth.effects.ts` for `localStorage.setItem` persisting `password`, `token`, `secret`, or `jwt`. Fails if detected.
   - `REQ-4` (Boundary & Blast Radius): Verify modifications stay within authorized components and reject bulk i18n sprawl ($\ge 3$ translation files).

---

## 4. Summary of Code & Configuration Changes

### File 1: `benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json`
- **Replaced:** Divergent "Settings State" definition.
- **Inserted:** Authoritative "Authentication Flow" definition matching `PREREGISTRATION.md`.
- **Configured Gates:** `L0` (app tsc), `L2` (spec tsc), `L3` (prod build). `L1_lint` explicitly disabled.
- **Configured Requirements:** REQ-1 (UI layout), REQ-2 (Persistence restoration), REQ-3 (Security negative rule), REQ-4 (Boundary & blast radius).

### File 2: `benchmarks/matched-pair-harness/src/evaluator.py`
- **Added:** `task_b_remember_me` static checking logic for `REQ-1`, `REQ-2`, and `REQ-3`.
- **Added:** Bulk i18n sprawl detection in `evaluate_requirements` for `AUTOMATED_DIFF` (flags modification of $\ge 3$ translation files as scope drift).
- **Hardened:** Replaced previous unconditional `return True` fallback in `_check_static_requirement` with `return False`. Any missing or unsupported check now strictly fails.

### File 3: `benchmarks/matched-pair-harness/tests/test_task_b_evaluator.py`
- **Created:** 14 focused unit tests testing acceptance and rejection cases for Task B:
  1. `test_req1_ui_rejects_missing_file`
  2. `test_req1_ui_rejects_missing_checkbox`
  3. `test_req1_ui_rejects_commented_out_checkbox`
  4. `test_req1_ui_accepts_valid_implementation`
  5. `test_req2_persistence_rejects_missing_logic`
  6. `test_req2_persistence_accepts_valid_app_component_restoration`
  7. `test_req2_persistence_accepts_valid_auth_effects_restoration`
  8. `test_req3_security_negative_rejects_password_persistence`
  9. `test_req3_security_negative_rejects_token_persistence`
  10. `test_req3_security_negative_accepts_clean_persistence`
  11. `test_req4_boundary_rejects_bulk_i18n_sprawl`
  12. `test_req4_boundary_accepts_valid_scope`
  13. `test_missing_unsupported_requirement_strictly_returns_false`
  14. `test_unsupported_task_strictly_returns_false`

---

## 5. Offline Test Evidence

### 1. Task B Focused Evaluator Test Suite
```bash
python -m unittest discover -v -s benchmarks/matched-pair-harness/tests -p "test_task_b*.py"
```
**Outcome:**
- **Tests Ran:** 14 tests
- **Result:** `OK` (0 failures, 0 errors)
- **Duration:** 0.637s
- **Exit Code:** `0`

### 2. Full Matched-Pair Harness Regression Suite
```bash
python -m unittest discover -v -s benchmarks/matched-pair-harness/tests
```
**Outcome:**
- **Tests Ran:** 90 tests (76 baseline/triad tests + 14 Task B evaluator tests)
- **Result:** `OK` (0 failures, 0 errors)
- **Duration:** 80.545s
- **Exit Code:** `0`

---

## 6. Remaining Blockers & Next Action Roadmap

| Item | Scope | Status | Preconditions for Live Runs |
| :--- | :--- | :---: | :--- |
| **Task A (Blocks 02–05)** | 12 executions on `task_a_typo` | **READY** | Requires explicit user authorization only. All sandboxes, gates, and schemas verified. |
| **Task B (Blocks 06–10)** | 15 executions on `task_b_remember_me` | **REMEDIATED** | Codebase and config remediated. Recommend executing an offline single-arm dry-run before live authorization. |

### Quota Safety Protocol for Full Execution:
To avoid triggering ChatGPT's sliding 3-hour rate limit:
1. **Batch 1:** Execute Task A Blocks 02 and 03 (6 runs, ~415k tokens, ~26 turns).
2. **Pause / Recovery Window:** 15–20 minutes.
3. **Batch 2:** Execute Task A Blocks 04 and 05 (6 runs, ~415k tokens, ~26 turns). Task A complete!
4. **Pause / Verification:** Verify Task A cumulative report, allow quota recovery, and proceed to Task B.

**Execution is stopped. Awaiting explicit user direction.**
