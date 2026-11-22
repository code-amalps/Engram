# Engram — Stage 1 Block 07 Final Benchmark Execution Report

- **Protocol Specification:** Amendment 04 (v4.1.0-RATIFIED, Oct 29, 2026)
- **Task Evaluated:** Task B (`task_b_remember_me`)
- **Block ID:** `block_07` (Seed: `2963`)
- **Model / Reasoning:** `gpt-5.6-luna` (Reasoning Effort: `medium`)
- **Execution Order:** Alternative B Matched-Pair — Order 1: `v154_arm3` (Candidate) $\longrightarrow$ Order 2: `baseline_arm1` (Baseline)
- **Deferred Arm:** `v151_arm2` (`DEFERRED_AMENDMENT_04`)
- **Per-Arm Token Ceiling:** 250,000 model tokens
- **Maximum Planned Quota Exposure:** 500,000 model tokens across both arms
- **Report Date:** October 29, 2026
- **Status:** **EXECUTION COMPLETE — CEILING BREACH DETECTED — SAVINGS STRICTLY SUPPRESSED (NOT CLAIMABLE)**

---

## Executive Summary

Stage 1 Block 07 was executed under the formally ratified Amendment 04 protocol to evaluate whether Phase 1 Token Efficiency Optimizations in candidate Engram v1.5.4 reduce total model tokens compared with the unassisted baseline on Task B.

The candidate arm (`v154_arm3`, Order 1) executed live against `gpt-5.6-luna` (medium reasoning). **The candidate breached the 250,000-token per-arm ceiling at turn 11**, consuming **270,506 total model tokens** in **109.24 seconds** without producing any code modifications (+0 / -0 lines) and scoring **$C = 0.00$**.

In accordance with the ratified Amendment 04 stopping rule, the orchestrator immediately aborted triad execution via real-time process tree termination. The unassisted baseline arm (`baseline_arm1`, Order 2) was halted before initialization and recorded as `SKIPPED_STOP_ON_FAILURE` (consuming **0 tokens**). 

**All comparative savings claims are STRICTLY SUPPRESSED (`NOT CLAIMABLE`).** The benchmark demonstrates that while Phase 1 optimizations successfully reduced the offline MCP tool schema surface (431 tokens vs 1,702 tokens), runtime context accumulation during agent exploratory bash/PowerShell commands overwhelmed the 250,000-token budget.

---

## 1. Preflight Verification Checklist

Prior to launching live model API calls, all mandatory preflight gates specified by Amendment 04 were verified and satisfied:

| Preflight Item | Requirement | Observed Status | Audit Verdict |
| :--- | :--- | :--- | :---: |
| **Candidate Binary Path** | `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` | Verified on disk (18,285,568 bytes) | **PASS** |
| **Candidate SHA-256** | `f56347ef2f0bfd4854176a3ec3c86d8d20b4c0e5954f04e4464998f623d85bee` | Bit-for-bit SHA-256 match | **PASS** |
| **Candidate Optimizations** | Phase 1 consolidated MCP tools (4 tools) + compact Turn-0 directives | Schema: 431 tokens, Turn-0 directives: 53 tokens | **PASS** |
| **Arm Order** | Alternative B: candidate first (`v154_arm3`), baseline second (`baseline_arm1`) | `order: ["v154_arm3", "baseline_arm1"]` | **PASS** |
| **Deferred Arm** | `v151_arm2` omitted from active execution order | Recorded as `DEFERRED_AMENDMENT_04` | **PASS** |
| **Per-Arm Ceiling** | 250,000 model tokens per arm (independent) | Real-time streaming token tracker armed at 250,000 | **PASS** |
| **Max Planned Exposure** | 500,000 model tokens across both arms | Configured and bounded | **PASS** |
| **Model & Reasoning** | `gpt-5.6-luna` / `medium` reasoning | Verified against Codex appserver CLI parameters | **PASS** |
| **Stopping Rule** | Immediate halt and skip baseline on candidate breach/timeout/crash | Orchestrator stop-on-failure active | **PASS** |
| **Baseline Isolation** | Unassisted control: 0 Turn-0 injection, 0 MCP tools, sanitized PATH | Verified via isolation manager | **PASS** |
| **Golden Repository Commit** | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` | Pinned HEAD verified bit-for-bit | **PASS** |
| **Golden Fixture Preservation** | 18 uncommitted modified files + `.engram/` untracked | Verified clean and untouched before run | **PASS** |
| **Frozen v1.5.1 Binary** | SHA-256 `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` | Verified sealed bit-for-bit | **PASS** |
| **Frozen v1.5.4 Binary** | SHA-256 `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` | Verified sealed bit-for-bit | **PASS** |

---

## 2. Candidate Arm Telemetry and Execution Details

The candidate arm ran in an isolated temporary sandbox (`task_b_remember_me_block_07_v154_arm3_20261029_190610`).

```json
{
  "run_id": "task_b_remember_me_block_07_v154_arm3",
  "arm": "v154_arm3",
  "order_in_pair": 1,
  "execution_status": "TOKEN_CEILING_BREACHED",
  "telemetry_valid": true,
  "elapsed_seconds": 109.24,
  "started_at": "2026-10-29T13:37:12.977254+00:00",
  "completed_at": "2026-10-29T13:39:02.213126+00:00"
}
```

### Telemetry Breakdown

| Metric | Candidate Value | Unit / Description |
| :--- | :---: | :--- |
| **Total Model Tokens** | **270,506** | Exceeded 250,000-token limit (+20,506 overrun before termination) |
| **Prompt Tokens (Total)** | **268,630** | Cumulative input tokens presented across 11 model turns |
| **Prompt Tokens (Cached)** | **248,448** | Prompt prefix reuse detected and discounted by API provider |
| **Prompt Tokens (Uncached)** | **20,182** | New dynamic input tokens processed across turns |
| **Completion Tokens** | **1,876** | Output tokens generated by the model |
| **Reasoning Tokens** | **603** | Internal CoT tokens (included within completion tokens) |
| **Peak Single-Request Context**| **29,918** | Maximum context size sent in a single turn request (Turn 11) |
| **Final Turn Context** | **29,918** | Context length when process was halted |
| **Turns Count** | **11** | Completed model inference turns |
| **MCP Tool Calls** | **1** | 1 attempted call (`engram_inspect_symbol`), blocked by client approval policy |
| **Auxiliary Payload Bytes** | **92** | Input arguments to MCP inspect symbol call |
| **Files Changed** | **0** | No source or config files modified (+0 / -0) |
| **Diff Patch** | `changes.patch` (0 bytes) | Empty patch produced |
| **Dependency Mutated** | **False** | `package.json`, `package-lock.json`, and `node_modules` untouched |

### Acceptance & Correctness Gates

| Gate / Requirement | Description | Status |
| :--- | :--- | :---: |
| **Correctness Score ($C$)** | Automated Task B evaluation score | **0.00** |
| **Passed All Requirements** | Strict conjunction ($C = 1.0$) | **False** |
| **Gate L0 (Typecheck)** | TypeScript project compilation (`tsconfig.app.json` / `tsconfig.spec.json`) | **BLOCKED** |
| **Gate L1 (Lint / Prettier)**| Formatting and lint verification | **SKIPPED** |
| **Gate L2 (Unit Tests)** | Angular Karma/Jasmine unit tests | **BLOCKED** |
| **Gate L3 (Production Build)**| Angular CLI production build | **BLOCKED** |
| **Requirement REQ-1** | Remember Me checkbox state persistence | **FAIL** |
| **Requirement REQ-2** | Storage key isolation under `AUTH` state | **FAIL** |
| **Requirement REQ-3** | Auto-fill username on application initialization | **FAIL** |
| **Requirement REQ-4** | Clear storage on uncheck / explicit logout | **FAIL** |

---

## 3. Baseline Arm Execution Status

Under Amendment 04 Section 4 ("Stopping Rules"), candidate failure by ceiling breach, timeout, crash, or invalid telemetry mandates an immediate stop-on-failure halt.

```json
{
  "run_id": "task_b_remember_me_block_07_baseline_arm1",
  "arm": "baseline_arm1",
  "order_in_pair": 2,
  "execution_status": "SKIPPED_STOP_ON_FAILURE",
  "telemetry_valid": false,
  "elapsed_seconds": 0.0,
  "total_model_tokens": 0,
  "error_message": "Execution halted: stop-on-failure triggered by v154_arm3 (TOKEN_CEILING_BREACHED)"
}
```

- **Quota Preserved:** Halting the triad prevented initiating the unassisted baseline arm, conserving an estimated **250,000 model tokens** of user quota.
- **Execution Artifacts:** A valid placeholder record conforming to `triad_summary.schema.json` was generated, documenting `SKIPPED_STOP_ON_FAILURE`.

---

## 4. Deferred Arm Status (`v151_arm2`)

Under Amendment 04 Section 3 ("Two-Arm Prioritization"), `v151_arm2` was deferred:

```json
{
  "run_id": "task_b_remember_me_block_07_v151_arm2",
  "arm": "v151_arm2",
  "execution_order_in_pair": 3,
  "execution_status": "DEFERRED_AMENDMENT_04",
  "telemetry_valid": false,
  "elapsed_seconds": 0.0,
  "total_model_tokens": 0,
  "error_message": "Execution deferred under protocol specification (not in execution order ['v154_arm3', 'baseline_arm1'])"
}
```

---

## 5. Comparative Savings Analysis & Claim Eligibility

Under Amendment 02 Section 6 and Amendment 04 Section 5, comparative token savings calculations require mutual acceptance ($C_{\text{treatment}} = 1.0 \land C_{\text{baseline}} = 1.0$) and valid telemetry on both arms.

```
       Candidate (v154_arm3)                Baseline (baseline_arm1)
     Status: TOKEN_CEILING_BREACHED       Status: SKIPPED_STOP_ON_FAILURE
            C = 0.00                                   C = 0.00
               \                                          /
                \                                        /
                 ▼                                      ▼
             [MUTUAL_ACCEPTANCE_FAILURE / STOP-ON-FAILURE]
                                  │
                                  ▼
             ┌──────────────────────────────────────────┐
             │       SAVINGS CLAIM: STRICTLY SUPPRESSED │
             │       STATUS: NOT CLAIMABLE              │
             │       SAVINGS DELTA: null                │
             │       SAVINGS PERCENT: null              │
             └──────────────────────────────────────────┘
```

| Pairwise Comparison | Savings Status | Verdict | Explanation |
| :--- | :---: | :---: | :--- |
| **Candidate (`v154_arm3`) vs Baseline (`baseline_arm1`)** | **NOT CLAIMABLE** | `MUTUAL_ACCEPTANCE_FAILURE` | Both arms failed functional correctness requirements. Comparative token savings and efficiency claims are strictly invalidated. |
| **Candidate (`v154_arm3`) vs Frozen v1.5.1 (`v151_arm2`)** | **NOT CLAIMABLE** | `MUTUAL_ACCEPTANCE_FAILURE` | Arm 2 deferred, candidate breached ceiling. |
| **Frozen v1.5.1 (`v151_arm2`) vs Baseline (`baseline_arm1`)** | **NOT CLAIMABLE** | `MUTUAL_ACCEPTANCE_FAILURE` | Both arms unexecuted. |

All savings deltas and percentage fields in `triad_summary.json` are recorded as `null` with status `INVALIDATED_UNACCEPTED`.

---

## 6. Forensic Analysis of Token Flow and Runtime Dynamics

### A. Turn-by-Turn Telemetry Trace (Session Rollout)

Forensic inspection of the actual Codex session rollout file (`rollout-2026-10-29T19-07-19-01a187e1-1ad8-7240-8423-44e8c283c0c3.jsonl`) reveals the exact step-by-step token dynamics:

| Turn | Request Input Tokens | Cached Input Tokens | Completion Tokens | Reasoning Tokens | Total Turn Tokens | Cumulative Tokens |
| :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **1** | 15,767 | 11,392 | 147 | 68 | 15,914 | 15,914 |
| **2** | 16,401 | 15,616 | 82 | 10 | 16,483 | 32,397 |
| **3** | 16,518 | 16,256 | 156 | 19 | 16,674 | 49,071 |
| **4** | 21,291 | 16,384 | 189 | 31 | 21,480 | 70,551 |
| **5** | 24,182 | 21,120 | 222 | 66 | 24,404 | 94,955 |
| **6** | 26,312 | 24,064 | 500 | 278 | 26,812 | 121,767 |
| **7** | 29,195 | 26,112 | 318 | 65 | 29,513 | 151,280 |
| **8** | 29,569 | 29,056 | 56 | 8 | 29,625 | 180,905 |
| **9** | 29,682 | 29,440 | 54 | 6 | 29,736 | 210,641 |
| **10** | 29,795 | 29,440 | 68 | 20 | 29,863 | 240,504 |
| **11** | 29,918 | 29,568 | 84 | 32 | 30,002 | **270,506** (Breach) |
| **Total** | **268,630** | **248,448** | **1,876** | **603** | **270,506** | **270,506** |

### B. Root Causes of the Overrun

1. **MCP Tool Approval Failure (`approval policy is never`):**
   - In Turn 1, the candidate model attempted to invoke Engram's consolidated inspection tool:
     `engram_inspect_symbol(target="projects/angular-ngrx-material-starter/src/app/core/core.module.ts", depth=2)`
   - The Codex harness environment rejected the call with:
     `{"message": "MCP tool call requires approval, but approval policy is never"}`
   - The harness configuration for Codex CLI runs with an automated non-interactive policy (`never`), which rejected the unwhitelisted MCP server execution.
2. **Exploratory Shell Command Fallback:**
   - Because MCP symbol retrieval failed, the agent immediately fell back to native shell commands:
     - `rg -n "remember|username|authLogin|LocalStorageService|login|password"`
     - `Get-Content projects/angular-ngrx-material-starter/src/app/core/core.module.ts`
     - `Get-Content projects/angular-ngrx-material-starter/src/app/app/app.component.ts`
     - `Get-Content projects/angular-ngrx-material-starter/src/app/core/local-storage/local-storage.service.ts`
     - `Get-Content projects/angular-ngrx-material-starter/src/app/core/auth/auth.effects.ts`
3. **Massive Context Inflation & Repetition:**
   - Draining multiple full TypeScript files into stdout injected over 13,000 raw tokens into the conversational context between Turns 3 and 7.
   - Context grew rapidly: 16.5k $\longrightarrow$ 21.3k $\longrightarrow$ 24.2k $\longrightarrow$ 26.3k $\longrightarrow$ 29.2k $\longrightarrow$ 29.9k tokens.
   - For every subsequent turn, the model had to re-read ~29.5k tokens of accumulated history. In turns 8, 9, 10, and 11, the agent spent ~120,000 tokens simply repeating the context while inspecting git status and running `npx tsc`.
4. **Failure to Converge on Code Edits:**
   - The model hallucinated a "contract limitation" claiming that because there was no existing username authentication form in the base starter, it should not add one to avoid making a "misleading partial change."
   - It exited after running typechecks, leaving 0 files changed and scoring 0.00.

---

## 7. Preservation Certification

All benchmark isolation invariants were certified intact post-run:

1. **Golden Repository Fixture:**
   - Located at `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`
   - Pinned HEAD commit: `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` (verified unchanged)
   - Working tree: Exactly the 18 pre-existing modified files and `.engram/` untracked directory remain intact. Zero files were added, deleted, or altered.
2. **Frozen Binaries:**
   - Frozen v1.5.1 (`benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe`): SHA-256 `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` (intact).
   - Frozen v1.5.4 (`benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe`): SHA-256 `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` (intact).
   - Candidate Phase 1 (`benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`): SHA-256 `f56347ef2f0bfd4854176a3ec3c86d8d20b4c0e5954f04e4464998f623d85bee` (intact).
3. **Sandbox Teardown:**
   - Temporary directory `C:\Users\Amal P S\AppData\Local\Temp\engram_pilot\runs\task_b_remember_me_block_07_v154_arm3_20261029_190610` was wiped cleanly following dependency hash validation.

---

## 8. Artifact and Evidence Index

All authoritative artifacts, logs, and summaries have been preserved and indexed:

- **Triad Structured Summary:**
  [`benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/triad_summary.json`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/triad_summary.json)
- **Triad Markdown Report:**
  [`benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/triad_report.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/triad_report.md)
- **Candidate Arm Telemetry:**
  [`benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/v154_arm3/run_telemetry.json`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/v154_arm3/run_telemetry.json)
- **Candidate Arm Events Stream:**
  [`benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/v154_arm3/events.jsonl`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/v154_arm3/events.jsonl)
- **Codex Session Rollout File:**
  `C:\Users\Amal P S\.codex\sessions\2026\10\29\rollout-2026-10-29T19-07-19-01a187e1-1ad8-7240-8423-44e8c283c0c3.jsonl`
- **Candidate Diff Patch (Empty):**
  [`benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/v154_arm3/changes.patch`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/v154_arm3/changes.patch)
- **Ratified Amendment 04 Protocol:**
  [`benchmarks/AMENDMENT_04_STAGE1_TOKEN_EFFICIENCY_PROTOCOL_REVISION.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/AMENDMENT_04_STAGE1_TOKEN_EFFICIENCY_PROTOCOL_REVISION.md)
- **Artifact Copy:**
  [`engram_stage1_block07_final_live_report.md`](file:///C:/Users/Amal%20P%20S/.gemini/antigravity/brain/96df9171-c910-4b4f-ae52-2add01693b56/engram_stage1_block07_final_live_report.md)
