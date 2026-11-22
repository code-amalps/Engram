# Engram — Stage 1 Core Triad Post-Smoke-Test Audit & Full Benchmark Readiness Gate

**Audit Date:** 2026-10-28  
**Audit Scope:** Read-only inspection and verification of `block_01` smoke-test evidence, execution harness, scheduling parameters, binary hashes, cost telemetry, and Task B readiness.  
**Auditor:** Automated Benchmark Integrity Auditor  
**Working Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram` (Branch: `feature/async-precommit-guard`, Commit: `c70845b`)  
**Target Golden Repository:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` (Commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`)  
**Readiness Verdict:** **`READY AFTER FIXES`** (Task A is validated; Task B contains three critical blockers that must be resolved before executing blocks 06–10).

---

## 1. Executive Summary & Audit Decision

A comprehensive, read-only audit of the completed Stage 1 Core Triad smoke test (`block_01` on Task A) and the surrounding benchmark infrastructure was conducted. 

### Final Readiness Verdict: **`READY AFTER FIXES`**

- **Task A Blocks (02–05):** **READY FOR EXPLICIT AUTHORIZATION.**  
  The smoke test for Task A validated 100% cold-start sandboxing, deterministic local postflight, automated gate execution (`L0` and `L3`), session rollout telemetry extraction, and strict $C = 1.00$ correctness across all three arms.
- **Task B Blocks (06–10):** **NOT READY (BLOCKERS IDENTIFIED).**  
  An offline preflight probe of `task_b_remember_me.json` revealed three blocking defects:
  1. **Invalid Gate Command:** `L0_typecheck` is configured as `npx ng build --configuration=development --no-watch`, which fails with exit code 1 (`Configuration 'development' ... is not set in the workspace`).
  2. **Pre-Existing Broken Lint Gate:** `L1_lint` is enabled as `npm run lint`, which fails in the golden repository with exit code 1 due to 141 pre-existing `@typescript-eslint/quotes` definition errors. Because mandatory gates require `PASS`, this would cause 100% of Task B executions to fail ($C = 0.00$, with $100\times$ penalty cost).
  3. **Missing Static Evaluator Branch:** `evaluator.py::_check_static_requirement` lacks a branch for `task_b_remember_me`, causing REQ-1 and REQ-2 to return `PASS` unconditionally.
- **Action Required Before Full Benchmark:** Resolve the three Task B configuration defects before initiating blocks 06–10. Task A blocks 02–05 may be executed once explicitly authorized.

---

## 2. Independent Recalculation & Telemetry Reconciliation

Every figure reported in `triad_summary.json`, `triad_report.md`, and `STAGE1_SMOKE_TEST_REPORT.md` was independently audited against the raw `turn.completed` rollout events in each arm's `events.jsonl`.

### Raw Telemetry Inspection

From `task_a_typo_block_01/<arm>/events.jsonl`:
- **Arm 1 (`baseline_arm1`):**  
  `{"input_tokens": 64338, "cached_input_tokens": 41600, "cache_write_input_tokens": 0, "output_tokens": 507, "reasoning_output_tokens": 72}`
- **Arm 2 (`v151_arm2`):**  
  `{"input_tokens": 75689, "cached_input_tokens": 70400, "cache_write_input_tokens": 0, "output_tokens": 985, "reasoning_output_tokens": 92}`
- **Arm 3 (`v154_arm3`):**  
  `{"input_tokens": 65755, "cached_input_tokens": 59264, "cache_write_input_tokens": 0, "output_tokens": 429, "reasoning_output_tokens": 47}`

### Reconciled Metrics Table

| Metric | Raw Event Formula | Arm 1 (Baseline) | Arm 2 (v1.5.1) | Arm 3 (v1.5.4) | Reconciliation Status |
| :--- | :--- | :---: | :---: | :---: | :---: |
| **Total Prompt Tokens** | `input_tokens` | 64,338 | 75,689 | 65,755 | **EXACT MATCH** |
| **Cached Prompt Tokens** | `cached_input_tokens` | 41,600 | 70,400 | 59,264 | **EXACT MATCH** |
| **Uncached Prompt Tokens** | `input - cached` | 22,738 | 5,289 | 6,491 | **EXACT MATCH** |
| **Completion Tokens** | `output_tokens` | 507 | 985 | 429 | **EXACT MATCH** |
| **Reasoning Tokens** | `reasoning_output_tokens` | 72 | 92 | 47 | **EXACT MATCH** |
| **Total Model Tokens** | `input + output` | 64,845 | 76,674 | 66,184 | **EXACT MATCH** |
| **Peak Context Window** | Max single request | 17,121 | 15,892 | 17,358 | **EXACT MATCH** |
| **Agent Elapsed Time** | Timestamps delta | 91.46s | 76.97s | 60.92s | **EXACT MATCH** |
| **Correctness ($C$)** | Automated evaluation | 1.00 | 1.00 | 1.00 | **EXACT MATCH** |
| **Quality-Adjusted Cost** | `total_tokens / C` | 64,845.0 | 76,674.0 | 66,184.0 | **EXACT MATCH** |

### Independent Recalculation of Pairwise Deltas & Percentages

Harness Formula: $\text{delta} = \text{baseline} - \text{treatment}$, $\text{savings\_pct} = \frac{\text{baseline} - \text{treatment}}{\text{baseline}} \times 100\%$

1. **Arm 3 (v1.5.4) vs Arm 1 (Baseline) — Primary Outcome:**
   - Total Tokens: $64,845 - 66,184 = -1,339$ tokens ($\mathbf{-2.0649\% \rightarrow -2.06\%}$). Exactly matches reported figure.
   - Prompt Tokens: $64,338 - 65,755 = -1,417$ tokens ($\mathbf{-2.2024\% \rightarrow -2.20\%}$). Exactly matches.
   - Completion Tokens: $507 - 429 = +78$ tokens ($\mathbf{+15.3846\% \rightarrow +15.38\%}$). Exactly matches.
   - Wall-Clock Time: $91.46\text{s} - 60.92\text{s} = +30.54\text{s}$ faster ($\mathbf{+33.39\%}$ duration reduction). Exactly matches.

2. **Arm 3 (v1.5.4) vs Arm 2 (v1.5.1) — Architectural Evolution:**
   - Total Tokens: $76,674 - 66,184 = +10,490$ tokens ($\mathbf{+13.6813\% \rightarrow +13.68\%}$). Exactly matches.
   - Prompt Tokens: $75,689 - 65,755 = +9,934$ tokens ($\mathbf{+13.1247\% \rightarrow +13.12\%}$). Exactly matches.
   - Completion Tokens: $985 - 429 = +556$ tokens ($\mathbf{+56.4467\% \rightarrow +56.45\%}$). Exactly matches.
   - Wall-Clock Time: $76.97\text{s} - 60.92\text{s} = +16.05\text{s}$ faster ($\mathbf{+20.85\%}$ duration reduction). Exactly matches.

3. **Arm 2 (v1.5.1) vs Arm 1 (Baseline) — Historical Benchmark:**
   - Total Tokens: $64,845 - 76,674 = -11,829$ tokens ($\mathbf{-18.2419\% \rightarrow -18.24\%}$). Exactly matches.
   - Completion Tokens: $507 - 985 = -478$ tokens ($\mathbf{-94.279\% \rightarrow -94.28\%}$). Exactly matches.

**Conclusion:** Zero mathematical discrepancies, rounding errors, or transcription mismatches exist between the raw events and the published reports.

---

## 3. Financial & Quota Accounting Audit

To maintain strict accounting integrity, actual expenditure must be distinguished from hypothetical valuations:

1. **Actual Cash Billed:** **$0.00 USD**
   - The execution provider (`codex-cli 0.164.0-alpha.11.1`) is authenticated via OpenAI ChatGPT subscription session tokens (`stored auth mode: chatgpt`).
   - No OpenAI API developer platform credits were debited, and no corporate credit card charges were incurred.
2. **Actual Quota Consumed:**
   - **Total Tokens:** 207,703 tokens across 3 agent executions.
   - **Total Model Requests / Turns:** 13 turns (Arm 1: 4 turns, Arm 2: 5 turns, Arm 3: 4 turns).
   - This usage was deducted entirely from the user's sliding 3-hour ChatGPT message allowance.
3. **Hypothetical Commercial List-Price Equivalent:**
   - Based on standard OpenAI platform list prices ($2.50/M uncached input, $1.25/M cached input, $10.00/M output):
     - Uncached Input ($34,518$ tokens): $\$0.086$
     - Cached Input ($171,264$ tokens): $\$0.214$
     - Completion ($1,921$ tokens): $\$0.019$
     - **Estimated Commercial Equivalent:** **~$0.32 USD**.
   - *Audit Note:* This figure is strictly a benchmark reference metric and must never be recorded as actual cash expenditure.

---

## 4. Binary Integrity & Cryptographic Hash Verification

Cryptographic SHA-256 hashes of the executables in `benchmarks/matched-pair-harness/bin/` were re-measured and cross-checked against the frozen documents:

| Binary | Verified On-Disk Path | Measured SHA-256 Hash | Documented Reference Hash | Alignment |
| :--- | :--- | :--- | :--- | :---: |
| **Engram v1.5.1** | `benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe` | `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` | `FREEZE_SEAL.md` Line 16: `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` | **EXACT MATCH** |
| **Engram v1.5.4** | `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe` | `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` | `AMENDMENT_02_STAGE1_CORE_TRIAD.md` Line 63: `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` | **EXACT MATCH** |

*Audit Observation on Prior Prompt Hash String:*  
In earlier correspondence, a truncated string missing the character `5` (`...aa1a978...` instead of `...aa51a978...`) was referenced. The actual binary on disk and the entry in `FREEZE_SEAL.md` both contain the authoritative hash `...aa51a978...`. The binary has not changed.

---

## 5. Schedule & Counterbalancing Verification

The schedule defined in `orchestrator.py::TRIAD_BLOCK_SCHEDULES` and Section 4 of `AMENDMENT_02_STAGE1_CORE_TRIAD.md` was audited:

- **Total Planned Blocks:** 10 blocks (5 on `task_a_typo`, 5 on `task_b_remember_me`).
- **Executions per Block:** 3 arms.
- **Total Planned Executions:** 30 executions ($10 \times 3$).
- **Completed Executions:** 3 (`block_01`).
- **Remaining Executions:** 27 (`block_02` through `block_10`).

### Counterbalancing Analysis & Inherent Limitations

The 5 blocks for each task are assigned the following orders:
- Block 01 / 06: `[Arm 1, Arm 2, Arm 3]` (Order 1)
- Block 02 / 07: `[Arm 2, Arm 3, Arm 1]` (Order 2)
- Block 03 / 08: `[Arm 3, Arm 1, Arm 2]` (Order 3)
- Block 04 / 09: `[Arm 1, Arm 3, Arm 2]` (Order 4)
- Block 05 / 10: `[Arm 2, Arm 1, Arm 3]` (Order 5)

**Audit Findings on Schedule Limitations:**
1. **Incomplete Latin Square:** A 3-arm design has $3! = 6$ unique permutations. Because each task has exactly $N = 5$ repetitions, one permutation (`[Arm 3, Arm 2, Arm 1]`) is omitted from the schedule.
2. **Positional Imbalance:**
   - **Position 1 (First Run):** Arm 1 appears 2 times; Arm 2 appears 2 times; **Arm 3 appears only 1 time**.
   - **Position 2 (Second Run):** Arm 1 appears 2 times; Arm 2 appears 1 time; **Arm 3 appears 2 times**.
   - **Position 3 (Third Run):** Arm 1 appears 1 time; Arm 2 appears 2 times; **Arm 3 appears 2 times**.
3. **Methodological Impact:** In 5-repetition blocks, Arm 3 bears cold-start cache/disk latency less often than Arms 1 and 2 in the first position. While full write isolation and dependency re-copying mitigate disk cache effects, downstream statistical reports must document this minor positional asymmetry.

---

## 6. Audit of Task B Readiness & Blocker Identification

A rigorous offline dry-run and configuration inspection was performed on `task_b_remember_me.json` and `evaluator.py`. Three severe defects were identified:

### [BLOCKER 1] Invalid L0 Typecheck Command in `task_b_remember_me.json`
- **Configured Command:** `"npx ng build --configuration=development --no-watch"`
- **Offline Probe Outcome:** Executed inside golden repository; failed immediately with **exit code 1**:
  ```
  An unhandled exception occurred: Configuration 'development' for target 'build'
  in project 'angular-ngrx-material-starter' is not set in the workspace.
  ```
- **Root Cause:** The project's `angular.json` does not declare a `development` build configuration.
- **Impact:** Every single run of Task B will fail `L0_typecheck`, causing mandatory gate failure and forcing $C = 0.00$.

### [BLOCKER 2] Broken L1 Lint Gate Enabled in `task_b_remember_me.json`
- **Configured Command:** `"npm run lint"` (enabled: `true`)
- **Offline Probe Outcome:** Executed inside golden repository; failed immediately with **exit code 1**:
  ```
  ✖ 141 problems (141 errors, 0 warnings)
  Definition for rule '@typescript-eslint/quotes' was not found
  ```
- **Root Cause:** The golden repository has 141 pre-existing ESLint configuration errors. In `task_a_typo.json`, `L1_lint` was disabled (`enabled: false`), but in `task_b_remember_me.json`, it was left enabled.
- **Impact:** In `evaluator.py::calculate_score`, if any enabled mandatory gate fails, `mandatory_gates_passed = False`, which zeroes the correctness score ($C = 0.00$) and multiplies effective cost by $100\times$. 100% of Task B executions would be discarded.

### [BLOCKER 3] Missing Domain Evaluator Check for Task B in `evaluator.py`
- **Inspection of `evaluator.py::_check_static_requirement`:**
  The method contains explicit branches for `task_a_typo` and `task_c_theme_toggle`, but **no branch for `task_b_remember_me`**.
  ```python
  if self.task_id == "task_a_typo": ...
  if self.task_id == "task_c_theme_toggle": ...
  return True  # Fallthrough for any other task!
  ```
- **Impact:** REQ-1 (settings model property) and REQ-2 (reducer initialization) will return `PASS` unconditionally, even if the agent produces an empty or broken patch.

### [SIGNIFICANT CONCERN 4] Scope Discrepancy Between Baseline Preregistration and Task Config
- **In `cost-baseline/PREREGISTRATION.md` and `cost-baseline/scripts/verify_task_b.ps1`:**  
  Task B is defined as adding a remember-me option to the **authentication flow** (touching `core/auth/` and `app.component`).
- **In `matched-pair-harness/config/tasks/task_b_remember_me.json`:**  
  Task B is configured to add remember-me to the **settings container** (touching `settings/**` and `core/settings/**`).
- **Impact:** The harness configuration does not match the frozen preregistration's Task B definition.

---

## 7. Audit of Harness Safety Invariants

The harness codebase was inspected for potential operational hazards:

1. **Automatic Retries:** **VERIFIED ABSENT.**  
   The execution loop in `orchestrator.py` runs each arm exactly once without retries. Failures halt execution immediately.
2. **Accidental Extra Runs:** **VERIFIED ABSENT.**  
   Run IDs and paths are strictly mapped 1:1 to block arm assignments.
3. **Zero-Token & Missing Telemetry Detection:** **VERIFIED ACTIVE.**  
   `validate_telemetry_integrity` explicitly rejects missing keys, malformed integers, and zero-token executions (`ZERO_TOKEN_ANOMALY`).
4. **Invalid-Run Exclusion:** **VERIFIED ACTIVE.**  
   Under preregistered rules, if any arm in a triad achieves $C < 1.00$, token savings claims are suppressed (`null`) and marked `SAVINGS_INVALIDATED_DUE_TO_QUALITY_FAILURE`.
5. **Cold-Start Sandboxing:** **VERIFIED ACTIVE.**  
   `dependency_strategy="copy"` creates an isolated workspace copy; golden repository fingerprints are verified pre- and post-execution.

---

## 8. Statistical Analysis Plan Audit

Moving from a 2-arm matched-pair design to a 3-arm Core Triad introduces methodological requirements that must be handled in the final analysis:

1. **Design Structure:** The experiment is a **Randomized Complete Block Design (RCBD)** with repeated measures across 3 conditions (`baseline_arm1`, `v151_arm2`, `v154_arm3`).
2. **Family-Wise Error Rate Control:** Testing three pairwise hypotheses (Arm 3 vs Baseline, Arm 3 vs Arm 2, Arm 2 vs Baseline) requires **Holm-Bonferroni correction** ($\alpha_1 = 0.0167, \alpha_2 = 0.025, \alpha_3 = 0.05$) to prevent alpha inflation.
3. **Small-Sample Limitations ($N = 5$ Blocks per Task):**
   - With $N = 5$, parametric normality tests (Shapiro-Wilk) lack power.
   - The analysis plan must report **non-parametric Wilcoxon signed-rank tests** or exact permutation tests alongside paired t-tests.
   - $N = 5$ has statistical power ($1 - \beta \ge 0.80$) only to detect large effect sizes ($d_z > 1.2$). It is sufficiently powered to validate a 70–85% token reduction hypothesis, but cannot resolve subtle 2–5% variations with statistical significance.

---

## 9. Conservative Remaining Quota & Execution Estimates

### Remaining Execution Scope
- **Task A (Blocks 02–05):** 4 blocks $\times$ 3 arms = **12 executions**.
- **Task B (Blocks 06–10):** 5 blocks $\times$ 3 arms = **15 executions**.
- **Total Remaining Executions:** **27 executions**.

### Estimated Token & Resource Consumption

| Category | Task A (12 Runs) | Task B (15 Runs) | Total Remaining (27 Runs) |
| :--- | :---: | :---: | :---: |
| **Estimated Tokens / Run** | ~65k – 75k | ~130k – 180k | — |
| **Total Prompt Tokens** | ~800,000 | ~2,200,000 | **~3,000,000 tokens** |
| **Total Completion Tokens** | ~6,000 | ~15,000 | **~21,000 tokens** |
| **Total Model Tokens** | **~810,000 tokens** | **~2,250,000 tokens** | **~3,060,000 tokens (~3.1M)** |
| **Estimated Agent Time** | ~15 minutes | ~35 minutes | **~50 minutes** |
| **Sandbox & Gate Overhead** | ~15 minutes | ~25 minutes | **~40 minutes** |
| **Total Wall-Clock Time** | ~30 minutes | ~60 minutes | **~90 minutes** |

### Quota & Billing Risk Assessment
- **Direct Financial Risk:** **$0.00 USD**. All runs execute via ChatGPT subscription authentication.
- **Subscription Rate-Limit Risk:** **MEDIUM-HIGH.**  
  Executing 27 runs requires ~130–180 agent requests. Running all 27 runs in a continuous loop will likely trigger ChatGPT's 3-hour sliding message limit.
- **Recommended Execution Protocol:**  
  Execute in batches of 2 blocks (6 runs) with a 15–20 minute pause between batches, or execute Task A (Blocks 02–05) first, allow quota recovery, and then execute Task B.

---

## 10. Integrity Confirmation of Assets

- **Frozen Baseline Integrity:**  
  `PREREGISTRATION.md` SHA-256 (`DB3AB2956A4DA40AAD8F1F376AA72A50BEBC96F3E1ECC3120F405F0A294EA8BB`) matches `FREEZE_SEAL.md` bit-for-bit. Zero files in `benchmarks/cost-baseline/` have been modified or staged.
- **Smoke-Test Evidence Integrity:**  
  All telemetry files, event logs, diff patches, and reports in `benchmarks/matched-pair-harness/reports/runs/task_a_typo_block_01/` remain intact and unchanged.

---

## 11. Final Audit Recommendations & Staged Roadmap

| Phase | Scope | Status | Preconditions for Execution |
| :--- | :--- | :---: | :--- |
| **Phase 1** | Task A Blocks 02–05 (12 runs) | **READY** | Requires explicit user authorization only. All gates, configs, and sandboxes validated. |
| **Phase 2** | Task B Remediation | **REQUIRED** | 1. Replace broken dev build command with `npx tsc -p ...`<br>2. Disable broken `L1_lint` or scope eslint to target files<br>3. Implement Task B static checks in `evaluator.py`<br>4. Align auth vs settings scope discrepancy |
| **Phase 3** | Task B Blocks 06–10 (15 runs) | **BLOCKED** | Authorized only after Phase 2 dry-run passes 100% offline. |

**Audit Complete. Execution is stopped. Awaiting explicit user direction.**
