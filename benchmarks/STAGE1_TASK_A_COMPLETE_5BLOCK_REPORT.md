# Engram — Stage 1 Core Triad: Task A Complete Five-Block Benchmark Report

**Document ID:** `ENGRAM-BENCHMARK-STAGE1-TASK-A-COMPLETE-5BLOCK`  
**Date:** 2026-10-28  
**Task:** `task_a_typo` (Single-line capitalization fix in `about.component.html`)  
**Target Repository:** `angular-ngrx-material-starter` (Commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`)  
**Model:** `gpt-5.6-luna` (Reasoning Effort: `medium`) via OpenAI Codex CLI `0.164.0-alpha.11.1`  
**Auth / Quota Mode:** ChatGPT Subscription Session Token (Direct Cash Billed: **$0.00 USD**)  
**Status:** Task A Execution Complete (Blocks 01–05, 15 Total Executions)  
**Next Milestone:** Authorization Gate for Stage 1 Task B (Remember-Me Feature)  

---

## 1. Executive Summary

This report delivers the authoritative empirical analysis of the complete five-block, 15-execution Stage 1 Core Triad benchmark on **Task A** (`task_a_typo`). 

### Core Empirical Findings:
1. **100% Acceptance & Verification Parity:** Across all 15 executions (5 blocks $\times$ 3 arms), all runs achieved a **perfect correctness score ($C = 1.00$)**. Every run passed TypeScript typechecking (`L0_typecheck`), production builds (`L3_production_build`), and functional requirements (`REQ-1`, `REQ-2`) with zero diff sprawl (modifying strictly 1 line in `about.component.html`) and zero package dependency mutations.
2. **Zero Engram MCP Tool Calls on Task A:** In all 15 executions across Arms 1, 2, and 3, **`mcp_tool_calls_count` was exactly 0**. Because the task prompt explicitly provides the exact file path and the target string, the agent resolved the fix using native PowerShell inspection and file updates without querying Engram's semantic index. Task A therefore serves strictly as a **minimal-task tool-schema overhead and agent edit-variance baseline**.
3. **Dominance of Cached Input Tokens (88.61%):** Total token consumption across all 15 runs was **1,950,516 tokens**, of which **1,728,384 tokens (88.61%)** were cached input tokens. Because Codex CLI appends the entire conversation history into each subsequent request, token cost scales quadratically with conversational turns:
   - 4-turn runs: ~64,000–70,000 tokens
   - 7-turn runs: ~104,000–116,000 tokens
   - 10-turn runs: ~153,000–156,000 tokens
   - 13–15 turn runs: ~202,000–235,000 tokens
4. **Overall Efficiency Outcome on Task A:**
   - In 1 block (Block 02), Engram v1.5.4 achieved **+23.98% token savings (+36,697 tokens saved)** because baseline wandered across 10 turns while v1.5.4 completed in 7 turns.
   - In 4 blocks (Blocks 01, 03, 04, 05), baseline or legacy arms completed with fewer edit retries, resulting in net regressions for v1.5.4: Block 01 (-2.06%), Block 03 (-123.99%), Block 04 (-53.14%), Block 05 (-56.61%).
   - **Pooled Token Totals:** Baseline consumed 546,545 tokens; v1.5.4 consumed 738,886 tokens (pooled delta: **-192,341 tokens, -35.19%**).
   - **Median Token Savings:** **-53.14% (-59,722 tokens)**.
5. **Billing & Quota Impact:** Actual cash billed was **$0.00 USD** under ChatGPT subscription session authentication (estimated commercial list-price equivalent: **~$2.83 USD** across all 15 runs).

---

## 2. Comprehensive 15-Run Telemetry Matrix

Every execution's raw `events.jsonl` stream was reconciled bit-for-bit against `run_telemetry.json` and `triad_summary.json`:

$$\text{Total Tokens} = \text{input\_tokens} + \text{output\_tokens}$$
$$\text{Prompt Uncached} = \text{input\_tokens} - \text{cached\_input\_tokens}$$

| Block | Arm | Ord | Total Tokens | Cached Input | Uncached Input | Output | Reas | Turns | MCP Calls | Time (s) | Score ($C$) |
| :---: | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **B01** | `baseline_arm1` | 1 | 64,845 | 41,600 | 22,738 | 507 | 72 | 4 | 0 | 91.46s | 1.00 |
| **B01** | `v151_arm2` | 2 | 76,674 | 70,400 | 5,289 | 985 | 92 | 5 | 0 | 76.97s | 1.00 |
| **B01** | `v154_arm3` | 3 | 66,184 | 59,264 | 6,491 | 429 | 47 | 4 | 0 | 60.92s | 1.00 |
| **B02** | `v151_arm2` | 1 | 104,566 | 98,432 | 5,275 | 859 | 101 | 7 | 0 | 101.05s | 1.00 |
| **B02** | `v154_arm3` | 2 | 116,357 | 108,160 | 7,379 | 818 | 142 | 7 | 0 | 117.03s | 1.00 |
| **B02** | `baseline_arm1` | 3 | 153,054 | 141,440 | 10,470 | 1,144 | 179 | 10 | 0 | 155.15s | 1.00 |
| **B03** | `v154_arm3` | 1 | 156,151 | 143,616 | 11,420 | 1,115 | 131 | 10 | 0 | 149.81s | 1.00 |
| **B03** | `baseline_arm1` | 2 | 69,714 | 50,176 | 19,063 | 475 | 48 | 4 | 0 | 106.48s | 1.00 |
| **B03** | `v151_arm2` | 3 | 125,635 | 99,200 | 25,290 | 1,145 | 195 | 8 | 0 | 82.63s | 1.00 |
| **B04** | `baseline_arm1` | 1 | 153,441 | 141,568 | 10,763 | 1,110 | 173 | 10 | 0 | 206.06s | 1.00 |
| **B04** | `v154_arm3` | 2 | 234,981 | 213,120 | 20,204 | 1,657 | 302 | 15 | 0 | 201.78s | 1.00 |
| **B04** | `v151_arm2` | 3 | 155,991 | 143,360 | 11,545 | 1,086 | 131 | 10 | 0 | 178.98s | 1.00 |
| **B05** | `v151_arm2` | 1 | 202,219 | 177,408 | 23,249 | 1,562 | 239 | 13 | 0 | 271.51s | 1.00 |
| **B05** | `baseline_arm1` | 2 | 105,491 | 96,128 | 8,698 | 665 | 80 | 7 | 0 | 143.26s | 1.00 |
| **B05** | `v154_arm3` | 3 | 165,213 | 144,512 | 19,370 | 1,331 | 127 | 9 | 0 | 199.66s | 1.00 |

---

## 3. Block-by-Block Pairwise Analyses

### 3.1 Engram v1.5.4 vs. Native Baseline
$$\Delta = \text{Baseline} - \text{v1.5.4}, \quad \text{Savings \%} = \frac{\Delta}{\text{Baseline}} \times 100$$

| Block | Baseline Tokens (Turns) | v1.5.4 Tokens (Turns) | Delta (Tokens) | Savings % | Cached Delta | Turns Delta | Wall-clock Delta | Verdict |
| :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **B01** | 64,845 (4) | 66,184 (4) | -1,339 | -2.06% | -17,664 | 0 | +30.54s | Slight Schema Overhead |
| **B02** | 153,054 (10) | 116,357 (7) | +36,697 | **+23.98%** | +33,280 | +3 | +38.12s | **Efficiency Gain** |
| **B03** | 69,714 (4) | 156,151 (10) | -86,437 | -123.99% | -93,440 | -6 | -43.33s | Edit Retry Regression |
| **B04** | 153,441 (10) | 234,981 (15) | -81,540 | -53.14% | -71,552 | -5 | +4.28s | Extended Retry Loop |
| **B05** | 105,491 (7) | 165,213 (9) | -59,722 | -56.61% | -48,384 | -2 | -56.40s | Edit Retry Regression |

### 3.2 Engram v1.5.1 vs. Native Baseline
| Block | Baseline Tokens (Turns) | v1.5.1 Tokens (Turns) | Delta (Tokens) | Savings % | Cached Delta | Turns Delta | Wall-clock Delta | Verdict |
| :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **B01** | 64,845 (4) | 76,674 (5) | -11,829 | -18.24% | -28,800 | -1 | +14.49s | Schema Overhead |
| **B02** | 153,054 (10) | 104,566 (7) | +48,488 | **+31.68%** | +43,008 | +3 | +54.10s | **Efficiency Gain** |
| **B03** | 69,714 (4) | 125,635 (8) | -55,921 | -80.21% | -49,024 | -4 | +23.85s | Edit Retry Regression |
| **B04** | 153,441 (10) | 155,991 (10) | -2,550 | -1.66% | -1,792 | 0 | +27.08s | Rough Parity |
| **B05** | 105,491 (7) | 202,219 (13) | -96,728 | -91.69% | -81,280 | -6 | -128.25s | Extended Retry Loop |

### 3.3 Engram v1.5.4 vs. Engram v1.5.1
| Block | v1.5.1 Tokens (Turns) | v1.5.4 Tokens (Turns) | Delta (Tokens) | Savings % | Cached Delta | Turns Delta | Wall-clock Delta | Verdict |
| :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **B01** | 76,674 (5) | 66,184 (4) | +10,490 | **+13.68%** | +11,136 | +1 | +16.05s | **v1.5.4 Gain** |
| **B02** | 104,566 (7) | 116,357 (7) | -11,791 | -11.28% | -9,728 | 0 | -15.98s | Turn-0 Overhead |
| **B03** | 125,635 (8) | 156,151 (10) | -30,516 | -24.29% | -44,416 | -2 | -67.18s | Extra Turn Overhead |
| **B04** | 155,991 (10) | 234,981 (15) | -78,990 | -50.64% | -69,760 | -5 | -22.80s | Extended Retry Loop |
| **B05** | 202,219 (13) | 165,213 (9) | +37,006 | **+18.30%** | +32,896 | +4 | +71.85s | **v1.5.4 Gain** |

---

## 4. Aggregate Descriptive Statistics (Complete 5-Block Sample)

| Metric | v1.5.4 vs. Baseline | v1.5.1 vs. Baseline | v1.5.4 vs. v1.5.1 |
| :--- | :---: | :---: | :---: |
| **Pooled Baseline Tokens** | 546,545 | 546,545 | 665,085 |
| **Pooled Treatment Tokens** | 738,886 | 665,085 | 738,886 |
| **Pooled Absolute Delta** | **-192,341 tokens** | **-118,540 tokens** | **-73,801 tokens** |
| **Pooled Savings Percentage** | **-35.19%** | **-21.69%** | **-11.10%** |
| **Median Savings Percentage** | **-53.14%** | **-18.24%** | **-11.28%** |
| **Range of Savings Percentages** | **[-123.99%, +23.98%]** | **[-91.69%, +31.68%]** | **[-50.64%, +18.30%]** |
| **Arithmetic Mean of Percentages** | **-42.37%** | **-32.03%** | **-10.84%** |
| **Standard Deviation of Percentages** | 57.02% | 52.57% | 28.35% |
| **Median Absolute Delta** | **-59,722 tokens** | **-11,829 tokens** | **-11,791 tokens** |
| **Range of Absolute Deltas** | **[-86,437, +36,697] tokens** | **[-96,728, +48,488] tokens** | **[-78,990, +37,006] tokens** |

### Mathematical Distinction: Pooled vs. Arithmetic Mean
- **Pooled Savings Percentage (-35.19%):** Weights each block by total token volume ($\sum \Delta / \sum \text{Baseline}$). It accurately reflects total token billing.
- **Arithmetic Mean (-42.37%):** Unweighted average of per-block percentages. Disproportionately impacted by Block 03 (-123.99%), where a small baseline denominator (69.7k tokens) magnified the percentage ratio.
- **Median (-53.14%):** Robust measure of central tendency across the 5 independent block matched pairs.

---

## 5. Token Breakdown, Turn Dynamics, & Behavioral Analysis

### 5.1 Component Breakdown Across All 15 Executions
| Category | Total Tokens | Percentage of Total | Baseline Arm 1 | v1.5.1 Arm 2 | v1.5.4 Arm 3 |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Cached Input Tokens** | 1,728,384 | **88.61%** | 470,912 | 588,800 | 668,672 |
| **Uncached Input Tokens** | 207,244 | **10.63%** | 71,732 | 70,648 | 64,864 |
| **Completion Output Tokens** | 14,888 | **0.76%** | 3,901 | 5,637 | 5,350 |
| *— Reasoning Tokens* | *2,059* | *0.11%* | *552* | *758* | *749* |
| **Total Tokens** | **1,950,516** | **100.00%** | **546,545** | **665,085** | **738,886** |
| **Total Conversational Turns** | 123 turns | — | 35 turns | 43 turns | 45 turns |
| **Mean Turns per Execution** | 8.2 turns | — | 7.0 turns | 8.6 turns | 9.0 turns |
| **Total Elapsed Wall-Clock** | 2,142.75s | — | 702.41s | 711.14s | 729.20s |

### 5.2 Why Task A Did Not Exercise Engram's Architecture
Event stream inspection confirms that the token variance observed on Task A is driven by **stochastic file-editing retries**, rather than codebase indexing or semantic context:
1. **Zero Index Queries:** Because the prompt named `about.component.html` and `'Angular Cli Tips'`, the agent never searched the codebase.
2. **The Formatting / Indentation Trap:**
   - In Codex CLI, editing a file via tool replacements frequently encounters whitespace mismatches on Windows HTML templates.
   - When the agent's first replacement matches indentation cleanly (e.g. Block 01 Baseline, Block 03 Baseline), the run terminates in **4 turns (~65k–70k tokens)**.
   - When the agent's first replacement introduces an indentation difference, the agent notices the whitespace discrepancy during `git diff --check`, inspects leading whitespace, applies a secondary edit, and re-checks git diff.
   - Because conversation history is fully rolled out on every turn, each extra turn adds ~17,000 prompt tokens. A run taking 10–15 turns consumes **150,000–235,000 tokens**.
   - Which arm falls into this retry loop on Task A is purely stochastic: Baseline stumbled in Block 02; v1.5.1 stumbled in Block 05; v1.5.4 stumbled in Blocks 03 and 04.

---

## 6. Schedule Analysis & Methodological Limitations

### 6.1 Schedule Structure: Partially Counterbalanced
As established in the read-only audit:
- Blocks 01–03 constitute a cyclic $3 \times 3$ Latin square.
- Blocks 04–05 add reversed permutations, omitting permutation 6 (`[3, 2, 1]`).
- Across the 5 blocks, ordinal positions are slightly asymmetric (Arms 1 and 2 ran 1st twice; Arm 3 ran 1st once).
- The design is **partially counterbalanced**, eliminating simple linear ordering drift while retaining a minor positional asymmetry.

### 6.2 Strict Scientific Boundaries
- **No Statistical Significance Claim:** 15 executions across 5 repetitions of a single trivial typo fix do not permit generalizable claims of token savings or inefficiency.
- **Negative Control Status:** Task A validates that when an LLM agent requires zero code exploration, registering MCP tool schemas and Turn-0 prompts adds token overhead without operational benefit.
- **Critical Need for Task B:** Evaluating whether Engram's Turn-0 index, boundary Change Contracts, and deterministic local postflight actually reduce tokens requires a task that forces architectural exploration across multiple modules (Task B: Remember-Me authentication feature).

---

## 7. Quota and Cost Accounting

- **Total Measured Tokens (15 Executions):** **1,950,516 tokens**.
- **Direct Financial Cash Cost:** **$0.00 USD** (ChatGPT Subscription token).
- **Subscription Quota Exposure:** 15 multi-turn CLI sessions executed sequentially without exceeding rate limits.
- **Commercial List-Price Equivalent:**
  - $1,728,384\text{ cached input} \times \$1.25 / 1\text{M} = \$2.16$
  - $207,244\text{ uncached input} \times \$2.50 / 1\text{M} = \$0.52$
  - $14,888\text{ output} \times \$10.00 / 1\text{M} = \$0.15$
  - **Total Commercial List-Price Value:** **~$2.83 USD**.

---

## 8. Stop Condition Enforcement

In strict compliance with authorization:
- **Task A (Blocks 01–05, 15 executions) is complete.**
- **Zero executions of Task B (Blocks 06–10) have been launched.**
- All background processes have terminated.
- Execution is stopped. Awaiting explicit instructions.
