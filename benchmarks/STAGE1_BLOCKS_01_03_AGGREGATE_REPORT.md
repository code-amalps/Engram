# Engram — Stage 1 Core Triad: Blocks 01–03 Aggregate Benchmark Report

**Document ID:** `ENGRAM-BENCHMARK-STAGE1-BLOCKS-01-03-AGGREGATE`  
**Date:** 2026-10-28  
**Task:** `task_a_typo` (Capitalization fix in `about.component.html`)  
**Target Repository:** `angular-ngrx-material-starter` (Commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`)  
**Model:** `gpt-5.6-luna` (Reasoning Effort: `medium`) via OpenAI Codex CLI `0.164.0-alpha.11.1`  
**Auth / Quota Mode:** ChatGPT Subscription Session Token (Direct Cash Billed: **$0.00 USD**)  
**Status:** Audit & Aggregate Analysis Complete (Blocks 01–03)  
**Recommendation:** `PROCEED_TO_BLOCKS_04_05`

---

## 1. Executive Summary

This report delivers the comprehensive empirical audit and aggregate analysis of the first three blocks (9 total executions) of the Stage 1 Core Triad Benchmark for Engram.

Across Blocks 01, 02, and 03:
- **Acceptance & Correctness Parity:** All 9 runs achieved **100% acceptance ($C = 1.00$)**. Every run passed TypeScript typechecking (`L0_typecheck`), production builds (`L3_production_build`), and functional requirements (`REQ-1`, `REQ-2`) with zero diff sprawl (modifying exclusively the single intended line in `about.component.html`) and zero dependency mutations.
- **Bit-for-Bit Telemetry Reconciliation:** 100% of telemetry metrics (`total_tokens`, `prompt_tokens`, `cached_tokens`, `uncached_tokens`, `output_tokens`, `reasoning_tokens`) reconcile bit-for-bit with raw provider events in `events.jsonl`.
- **Empirical Efficiency Outcome:**
  - In **Block 01**, Baseline located the typo in 4 turns; Engram v1.5.4 finished in 4 turns with slight prompt schema overhead (**-2.06% token savings / regression**).
  - In **Block 02**, Baseline wandered across 10 turns searching templates; Engram v1.5.4 bounded exploration to 7 turns, yielding **+23.98% token savings (+36,697 tokens saved)**.
  - In **Block 03**, Baseline resolved the typo in 4 turns; Engram v1.5.4 experienced an indentation mismatch on attempt 1, triggering an edit-and-verify retry loop across 10 turns (**-123.99% token regression**).
- **Aggregate Descriptive Metrics (v1.5.4 vs. Baseline across Blocks 01–03):**
  - **Median Token Savings:** **-2.06% (-1,339 tokens)**
  - **Range of Token Savings:** **[-123.99%, +23.98%] ([-86,437, +36,697] tokens)**
  - **Pooled Token Savings:** **-17.76% (-51,079 tokens on 287,613 baseline tokens)**
  - **Arithmetic Mean of Percentages:** **-34.03%**
- **Core Scientific Finding:** On Task A (a trivial 1-line typo fix where the target file is named in the prompt), the agent never called Engram MCP tools (`mcp_tool_calls_count = 0` across all 9 runs). Total token consumption is overwhelmingly dominated (87.05%) by cached input tokens rolling out full conversation history in Codex CLI. Differences are primarily driven by whether the agent's file edit succeeds cleanly on attempt 1 (4 turns, ~65k tokens) or introduces an indentation/formatting mismatch requiring re-inspection (7–10 turns, ~116k–156k tokens).
- **Decision:** **`PROCEED_TO_BLOCKS_04_05`** to complete the preregistered 5-block Latin square sequence for Task A without violating sample-size stopping rules.

---

## 2. Experimental Verification & Provenance

### 2.1 Environmental & Binary Verification
| Verification Item | Specification / Hash | Observed Value | Verification Status |
| :--- | :--- | :--- | :--- |
| **Repository Branch** | `feature/async-precommit-guard` | `feature/async-precommit-guard` | **PASS** |
| **HEAD Commit** | `c70845b4d5913457f1b176fb0370b80dd792711f` | `c70845b4d5913457f1b176fb0370b80dd792711f` | **PASS** |
| **Engram v1.5.1 SHA-256** | `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` | Verified in Blocks 01, 02, 03 | **PASS** |
| **Engram v1.5.4 SHA-256** | `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` | Verified in Blocks 01, 02, 03 | **PASS** |
| **Preregistration Seal** | `db3ab2956a4da40aad8f1f376aa72a50bebc96f3e1ecc3120f405f0a294ea8bb` | Bit-for-bit unchanged | **PASS** |
| **Golden Tree Digest** | `f63d2e4c069e14aa3cc4765fa327c6b7e6603d19b3a6945a9dd34eec8f074476` | Verified pre- and post-run | **PASS** |
| **Sandbox Isolation** | Independent temp directory per execution | Verified clean deletion | **PASS** |

### 2.2 Telemetry Event Reconciliation
Every execution's raw `events.jsonl` was parsed to verify event stream sums against `run_telemetry.json` and `triad_summary.json`:

$$\text{Total Tokens} = \text{input\_tokens} + \text{output\_tokens}$$
$$\text{Prompt Uncached} = \text{input\_tokens} - \text{cached\_input\_tokens}$$

| Block ID | Arm | Events Total | Telemetry Total | Cached Input | Uncached Input | Output | Reasoning | Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `block_01` | `baseline_arm1` | 64,845 | 64,845 | 41,600 | 22,738 | 507 | 72 | **RECONCILED** |
| `block_01` | `v151_arm2` | 76,674 | 76,674 | 70,400 | 5,289 | 985 | 92 | **RECONCILED** |
| `block_01` | `v154_arm3` | 66,184 | 66,184 | 59,264 | 6,491 | 429 | 47 | **RECONCILED** |
| `block_02` | `baseline_arm1` | 153,054 | 153,054 | 141,440 | 10,470 | 1,144 | 179 | **RECONCILED** |
| `block_02` | `v151_arm2` | 104,566 | 104,566 | 98,432 | 5,275 | 859 | 101 | **RECONCILED** |
| `block_02` | `v154_arm3` | 116,357 | 116,357 | 108,160 | 7,379 | 818 | 142 | **RECONCILED** |
| `block_03` | `baseline_arm1` | 69,714 | 69,714 | 50,176 | 19,063 | 475 | 48 | **RECONCILED** |
| `block_03` | `v151_arm2` | 125,635 | 125,635 | 99,200 | 25,290 | 1,145 | 195 | **RECONCILED** |
| `block_03` | `v154_arm3` | 156,151 | 156,151 | 143,616 | 11,420 | 1,115 | 131 | **RECONCILED** |

---

## 3. Detailed Results Across Blocks 01–03

### 3.1 Raw Telemetry Matrix
| Block | Arm | Order | Total Tokens | Cached Prompt | Uncached Prompt | Completion | Reasoning | Turns | Elapsed | Score ($C$) |
| :--- | :--- | :---: | :--- | :--- | :--- | :--- | :--- | :---: | :--- | :---: |
| **Block 01** | `baseline_arm1` | 1 | 64,845 | 41,600 | 22,738 | 507 | 72 | 4 | 91.46s | 1.00 |
| **Block 01** | `v151_arm2` | 2 | 76,674 | 70,400 | 5,289 | 985 | 92 | 5 | 76.97s | 1.00 |
| **Block 01** | `v154_arm3` | 3 | 66,184 | 59,264 | 6,491 | 429 | 47 | 4 | 60.92s | 1.00 |
| **Block 02** | `baseline_arm1` | 3 | 153,054 | 141,440 | 10,470 | 1,144 | 179 | 10 | 155.15s | 1.00 |
| **Block 02** | `v151_arm2` | 1 | 104,566 | 98,432 | 5,275 | 859 | 101 | 7 | 101.05s | 1.00 |
| **Block 02** | `v154_arm3` | 2 | 116,357 | 108,160 | 7,379 | 818 | 142 | 7 | 117.03s | 1.00 |
| **Block 03** | `baseline_arm1` | 2 | 69,714 | 50,176 | 19,063 | 475 | 48 | 4 | 106.48s | 1.00 |
| **Block 03** | `v151_arm2` | 3 | 125,635 | 99,200 | 25,290 | 1,145 | 195 | 8 | 82.63s | 1.00 |
| **Block 03** | `v154_arm3` | 1 | 156,151 | 143,616 | 11,420 | 1,115 | 131 | 10 | 149.81s | 1.00 |

### 3.2 Pairwise Comparisons by Block

#### A. Engram v1.5.4 vs Native Baseline
$$\Delta = \text{Tokens}_{\text{baseline}} - \text{Tokens}_{\text{v154}}, \quad \text{Savings \%} = \frac{\Delta}{\text{Tokens}_{\text{baseline}}} \times 100$$

| Block | Baseline Tokens | v1.5.4 Tokens | Delta (Tokens) | Savings % | Cached Delta | Uncached Delta | Turns Delta | Wall-clock Delta | Verdict |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Block 01** | 64,845 | 66,184 | -1,339 | -2.06% | -17,664 | +16,247 | 0 | +30.54s | Regression |
| **Block 02** | 153,054 | 116,357 | +36,697 | **+23.98%** | +33,280 | +3,091 | +3 | +38.12s | **Gain** |
| **Block 03** | 69,714 | 156,151 | -86,437 | -123.99% | -93,440 | +7,643 | -6 | -43.33s | Regression |

#### B. Engram v1.5.1 vs Native Baseline
| Block | Baseline Tokens | v1.5.1 Tokens | Delta (Tokens) | Savings % | Cached Delta | Uncached Delta | Turns Delta | Wall-clock Delta | Verdict |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Block 01** | 64,845 | 76,674 | -11,829 | -18.24% | -28,800 | +17,449 | -1 | +14.49s | Regression |
| **Block 02** | 153,054 | 104,566 | +48,488 | **+31.68%** | +43,008 | +5,195 | +3 | +54.10s | **Gain** |
| **Block 03** | 69,714 | 125,635 | -55,921 | -80.21% | -49,024 | -6,227 | -4 | +23.85s | Regression |

#### C. Engram v1.5.4 vs Engram v1.5.1
| Block | v1.5.1 Tokens | v1.5.4 Tokens | Delta (Tokens) | Savings % | Cached Delta | Uncached Delta | Turns Delta | Wall-clock Delta | Verdict |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Block 01** | 76,674 | 66,184 | +10,490 | **+13.68%** | +11,136 | -1,202 | +1 | +16.05s | **Gain** |
| **Block 02** | 104,566 | 116,357 | -11,791 | -11.28% | -9,728 | -2,104 | 0 | -15.98s | Regression |
| **Block 03** | 125,635 | 156,151 | -30,516 | -24.29% | -44,416 | +13,870 | -2 | -67.18s | Regression |

---

## 4. Aggregate Descriptive Statistics

### 4.1 Summary of Central Tendency & Dispersion
Because token distributions in multi-turn agent runs are heavily skewed by turn count, both unweighted and weighted statistics are presented:

| Comparison | Median Delta | Range of Deltas | Median Savings % | Range of Savings % | Arithmetic Mean % | Pooled Delta | Pooled Savings % |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **v1.5.4 vs Baseline** | **-1,339** | [-86,437, +36,697] | **-2.06%** | [-123.99%, +23.98%] | -34.03% | **-51,079** | **-17.76%** |
| **v1.5.1 vs Baseline** | **-11,829** | [-55,921, +48,488] | **-18.24%** | [-80.21%, +31.68%] | -22.26% | **-19,262** | **-6.70%** |
| **v1.5.4 vs v1.5.1** | **-11,791** | [-30,516, +10,490] | **-11.28%** | [-24.29%, +13.68%] | -7.29% | **-31,817** | **-10.37%** |

### 4.2 Mathematical Note: Pooled Percentage vs. Arithmetic Mean of Percentages
- **Pooled Percentage:** Weights each block by its absolute token volume:
  $$\text{Pooled \%} = \frac{\sum \text{Delta}}{\sum \text{Baseline}} = \frac{-51,079}{287,613} = -17.76\%$$
  This reflects the actual balance of tokens consumed across the 3 blocks combined.
- **Arithmetic Mean of Percentages:** Treats each block equally regardless of denominator size:
  $$\text{Mean \%} = \frac{-2.06\% + 23.98\% - 123.99\%}{3} = -34.03\%$$
  In Block 03, the baseline finished quickly (69,714 tokens), creating a small denominator. When v1.5.4 took 10 turns (156,151 tokens), the resulting ratio $(-86,437 / 69,714) = -123.99\%$ heavily distorted the unweighted average.
- **Median Values:** The median savings percentage (**-2.06%**) and median delta (**-1,339 tokens**) provide the most robust indicator of central tendency across these 3 runs.

---

## 5. Analysis of Repeatability, Mechanisms, & Limitations

### 5.1 Consistency Across Blocks
- **Does v1.5.4 beat baseline consistently?** **No.** v1.5.4 demonstrated clear token savings in Block 02 (+23.98%), but experienced a slight regression in Block 01 (-2.06%) and a substantial regression in Block 03 (-123.99%).
- **Does v1.5.4 beat v1.5.1 consistently?** **No.** v1.5.4 outperformed v1.5.1 in Block 01 (+13.68%), but was less efficient in Block 02 (-11.28%) and Block 03 (-24.29%).

### 5.2 Dominance of Cached Input Tokens
Across all 9 executions, **812,288 of the 933,180 total tokens (87.05%)** were cached input tokens.
Because Codex CLI maintains conversation history by rolling out prior turns into subsequent requests, prompt token size scales quadratically with turn count:
- A 4-turn run incurs ~64,000–69,000 total tokens.
- A 7-turn run incurs ~104,000–116,000 total tokens.
- A 10-turn run incurs ~153,000–156,000 total tokens.

### 5.3 Behavioral Driver: Edit Retries vs. Architectural Indexing
A detailed event inspection reveals the root cause of the token variance on Task A:
1. **MCP Tool Usage was Zero:** In all 9 runs, `mcp_tool_calls_count` was exactly 0. The agent did not query Engram's index tools because the task prompt explicitly identified the target file (`about.component.html`) and the exact string (`Angular Cli Tips`).
2. **File Editing Stochasticity:**
   - In **Block 01**, all three arms applied the patch cleanly on turn 1 or 2, finishing in 4–5 turns.
   - In **Block 02**, the baseline agent applied an initial edit that introduced an unintended header diff. The agent noticed the diff, performed additional shell commands, and re-edited, stretching the run to 10 turns (153k tokens). Engram's arms finished in 7 turns (104k–116k tokens).
   - In **Block 03**, the reverse occurred: Engram v1.5.4 applied an initial edit that exposed an indentation mismatch. The agent stated: *"The first edit exposed an indentation-only difference in the patch, so I’m correcting it to touch only the single text line."* This triggered extra diff checks, re-reads, and a secondary patch, extending v1.5.4 to 10 turns (156k tokens), while the baseline finished cleanly in 4 turns (69k tokens).

### 5.4 Order Effects & Task A Limitations
- **Execution Order:**
  - Block 01: `[baseline_arm1, v151_arm2, v154_arm3]`
  - Block 02: `[v151_arm2, v154_arm3, baseline_arm1]`
  - Block 03: `[v154_arm3, baseline_arm1, v151_arm2]`
  Execution order did not exhibit a systematic monotonic bias; rather, turn count was dictated by edit-retry stochasticity.
- **Scope Limitations:** Task A is a minimal-complexity, single-character typo fix. Repeating this identical trivial task 3 times provides valuable information about baseline tool schema overhead and agent editing variance, but **cannot and does not support any claims of generalizable real-world token savings or statistical significance**.

---

## 6. Quota Accounting & Cost Analysis

### 6.1 Measured Consumption (Blocks 01–03, 9 Executions)
| Category | Baseline Arm 1 | v1.5.1 Arm 2 | v1.5.4 Arm 3 | Total Measured Consumption |
| :--- | :--- | :--- | :--- | :--- |
| **Cached Input Tokens** | 233,216 | 268,032 | 311,040 | **812,288** (87.05%) |
| **Uncached Input Tokens** | 52,271 | 35,854 | 25,290 | **113,415** (12.15%) |
| **Completion Output Tokens** | 2,126 | 2,989 | 2,362 | **7,477** (0.80%) |
| *— of which Reasoning Tokens* | *299* | *388* | *320* | *1,007* (0.11%) |
| **Total Model Tokens** | **287,613** | **306,875** | **338,692** | **933,180** (100.00%) |

### 6.2 Financial Accounting vs. Commercial List-Price Equivalent
- **Actual Cash Billed:** **$0.00 USD** (authenticated via ChatGPT Subscription session token).
- **Subscription Quota Impact:** Consumes standard rate-limited conversational turns within the active ChatGPT billing period. (Not zero opportunity cost; subject to provider rate limits).
- **Estimated Commercial List-Price Equivalent:**
  - Under standard published commercial frontier pricing ($2.50 / 1M uncached input, $1.25 / 1M cached input, $10.00 / 1M completion output):
    - Cached Input: $812,288 \times \$1.25 / 1\text{M} = \$1.015$
    - Uncached Input: $113,415 \times \$2.50 / 1\text{M} = \$0.284$
    - Completion Output: $7,477 \times \$10.00 / 1\text{M} = \$0.075$
    - **Total Commercial Equivalent:** **~$1.37 USD** across all 9 completed runs.

### 6.3 Quota Exposure Estimate for Blocks 04–05
- **Remaining Task A Executions:** 2 blocks $\times$ 3 arms = 6 executions.
- **Observed Mean Consumption:** ~103,687 tokens per execution (~311,060 tokens per block).
- **Projected Exposure for Blocks 04–05:** **~620,000 – 650,000 total tokens** (~$0.90 USD commercial list-price equivalent, $0.00 USD cash billed).

---

## 7. Decision & Recommendation

### **Recommendation: `PROCEED_TO_BLOCKS_04_05`**

### Concrete Justification:
1. **Zero Harness Defects:** The harness demonstrated 100% telemetry fidelity, complete sandbox isolation, zero golden-repository contamination, and perfect automated verification. There are no defects in telemetry, scheduling, isolation, or verification to fix (`FIX_HARNESS_BEFORE_CONTINUING` is inapplicable).
2. **Adherence to Preregistration Sample Size:** The Stage 1 protocol preregistered a 5-block counterbalanced design (30 total executions across Task A and Task B). Stopping Task A at $N=3$ would violate preregistered sample-size stopping rules and discard the final two rotations of the Latin square (`[Order 4: baseline, v154, v151]` and `[Order 5: v151, baseline, v154]`).
3. **Scientific Purpose of Task A:** Task A was deliberately designed as a minimal-complexity negative-control task to observe prompt schema overhead and agent behavior when architectural indexing is not needed. The finding that MCP tool calls are 0 and that variance is governed by edit retries is a genuine empirical finding that should be fully characterized across the complete 5-block sample.
4. **Controlled Quota Exposure:** Quota consumption is bounded (~620k tokens for the final 6 runs) and incurs $0.00 USD cash charges.

---

## 8. Stop Condition & Next Milestone

- Live model calls for Blocks 04–05 remain **UNAUTHORIZED** until explicit user approval is granted.
- Following authorization, the remaining 6 executions for Task A (Blocks 04 and 05) will be executed sequentially using the verified harness.
