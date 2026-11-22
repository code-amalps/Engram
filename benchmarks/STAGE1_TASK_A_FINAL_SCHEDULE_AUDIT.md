# Engram — Task A Final Schedule Audit and Blocks 04–05 Authorization Gate Report

**Document ID:** `ENGRAM-AUDIT-STAGE1-TASK-A-SCHEDULE-GATE`  
**Date:** 2026-10-28  
**Task:** `task_a_typo` (Capitalization fix in `about.component.html`)  
**Target Repository:** `angular-ngrx-material-starter` (Commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`)  
**Model:** `gpt-5.6-luna` (Reasoning Effort: `medium`) via OpenAI Codex CLI `0.164.0-alpha.11.1`  
**Auth Mode:** ChatGPT Subscription Session Token (Direct Cash Billed: **$0.00 USD**)  
**Status:** Read-Only Audit Complete; Awaiting Blocks 04–05 Authorization  

---

## 1. Experimental Schedule Audit & Mathematical Verdict

### 1.1 Schedule Specification Review
The schedule defined in `benchmarks/matched-pair-harness/src/orchestrator.py` (lines 47–52) and described in Amendment 02 Section 4 is:

```python
TRIAD_BLOCK_SCHEDULES = {
    "block_01": {"task": "task_a_typo", "order": ["baseline_arm1", "v151_arm2", "v154_arm3"], "seed": 2026102801},
    "block_02": {"task": "task_a_typo", "order": ["v151_arm2", "v154_arm3", "baseline_arm1"], "seed": 2026102802},
    "block_03": {"task": "task_a_typo", "order": ["v154_arm3", "baseline_arm1", "v151_arm2"], "seed": 2026102803},
    "block_04": {"task": "task_a_typo", "order": ["baseline_arm1", "v154_arm3", "v151_arm2"], "seed": 2026102804},
    "block_05": {"task": "task_a_typo", "order": ["v151_arm2", "baseline_arm1", "v154_arm3"], "seed": 2026102805},
}
```

### 1.2 Schedule Verdict: Partially Counterbalanced (Not a Balanced Latin Square)
Amendment 02 describes this 5-block schedule as a *"Balanced Latin-Square Order"*. 

**Mathematical Audit Verdict:** The 5-block schedule is **partially counterbalanced**, not a Balanced Latin Square.

#### Mathematical Proof:
1. **Divisibility Constraint:** A Balanced Latin Square requires the number of blocks $N$ to be an integer multiple of the number of treatments $k$ ($N = m \cdot k$). Here, $k = 3$ arms. Because $5 \not\equiv 0 \pmod 3$, a 5-block design cannot form a balanced Latin square.
2. **Positional Frequency Asymmetry:** In a balanced Latin square, every arm must appear in each ordinal position ($1^{\text{st}}, 2^{\text{nd}}, 3^{\text{rd}}$) an equal number of times. Across the 5 blocks:
   - **Arm 1 (Baseline):** Position 1 = **2** (B01, B04), Position 2 = **1** (B03), Position 3 = **2** (B02, B05).
   - **Arm 2 (v1.5.1):** Position 1 = **2** (B02, B05), Position 2 = **1** (B01), Position 3 = **2** (B03, B04).
   - **Arm 3 (v1.5.4):** Position 1 = **1** (B03), Position 2 = **3** (B02, B04), Position 3 = **1** (B01, B05).
   *(Position frequencies are asymmetric: Arm 3 only runs first once, while Arms 1 and 2 run first twice).*
3. **Sub-Structure:**
   - **Blocks 01–03** constitute a genuine, complete $3 \times 3$ standard cyclic Latin square:
     - B01: `[1, 2, 3]`
     - B02: `[2, 3, 1]`
     - B03: `[3, 1, 2]`
     *(Each arm appears exactly once in positions 1, 2, and 3).*
   - **Blocks 04–05** add two reversed permutations (`[1, 3, 2]` and `[2, 1, 3]`), while omitting the 6th possible permutation (`[3, 2, 1]`).
   - A fully balanced, complete counterbalanced design (Williams Design) across 3 arms would require **6 blocks (18 executions)**.
4. **Correction Guideline:** In accordance with protocol safeguards, frozen preregistration artifacts (`PREREGISTRATION.md` and `FREEZE_SEAL.md`) remain untouched. This correction is documented authoritatively in this audit report.

---

## 2. Comprehensive Verification of Nine Completed Runs

All 9 executions (Blocks 01, 02, and 03 across all 3 arms) were inspected against raw disk artifacts:

| Block | Arm | Binary SHA-256 Verified | Gate: L0 (Typecheck) | Gate: L3 (Build) | REQ-1 & REQ-2 | Patch Focus | Golden Integrity |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **B01** | `baseline_arm1` | `NONE` (Clean) | **PASS** | **PASS** | **PASS** | Exactly 1 line in `about.component.html` | Intact |
| **B01** | `v151_arm2` | `474a1a7...` (Match) | **PASS** | **PASS** | **PASS** | Exactly 1 line in `about.component.html` | Intact |
| **B01** | `v154_arm3` | `cf807ea...` (Match) | **PASS** | **PASS** | **PASS** | Exactly 1 line in `about.component.html` | Intact |
| **B02** | `baseline_arm1` | `NONE` (Clean) | **PASS** | **PASS** | **PASS** | Exactly 1 line in `about.component.html` | Intact |
| **B02** | `v151_arm2` | `474a1a7...` (Match) | **PASS** | **PASS** | **PASS** | Exactly 1 line in `about.component.html` | Intact |
| **B02** | `v154_arm3` | `cf807ea...` (Match) | **PASS** | **PASS** | **PASS** | Exactly 1 line in `about.component.html` | Intact |
| **B03** | `baseline_arm1` | `NONE` (Clean) | **PASS** | **PASS** | **PASS** | Exactly 1 line in `about.component.html` | Intact |
| **B03** | `v151_arm2` | `474a1a7...` (Match) | **PASS** | **PASS** | **PASS** | Exactly 1 line in `about.component.html` | Intact |
| **B03** | `v154_arm3` | `cf807ea...` (Match) | **PASS** | **PASS** | **PASS** | Exactly 1 line in `about.component.html` | Intact |

- **Score Parity:** All 9 runs scored $C = 1.00$.
- **Diff Sprawl:** Zero unintended files modified; zero package dependencies altered; zero test changes.
- **Telemetry Reconciliation:** In every run, the raw `events.jsonl` stream matched `run_telemetry.json` bit-for-bit:
  $$\text{Total Tokens} = \text{input\_tokens} + \text{output\_tokens}$$
  $$\text{Uncached Input} = \text{input\_tokens} - \text{cached\_input\_tokens}$$

---

## 3. Investigation of Block 03's Edit/Verification Retry Loop

In Block 03, Engram v1.5.4 consumed **156,151 tokens across 10 turns**, compared to Baseline's **69,714 tokens across 4 turns** (-123.99% token regression). 

An event-by-event inspection of `events.jsonl` explains the exact mechanism:

1. **Missing Utility Error on Turn 1:**
   - Both `v154_arm3` (Event 5) and `v151_arm2` (Event 5) began by executing:
     `rg -n -C 2 "Angular Cli Tips|Angular CLI Tips" ...`
   - In this Windows PowerShell environment, `rg` (`ripgrep`) was not in the execution PATH, producing:
     `rg : The term 'rg' is not recognized as the name of a cmdlet...`
   - The agents had to fall back to PowerShell `Get-Content` / `Select-String`, burning an initial discovery round-trip.
2. **Initial Edit Indentation Discrepancy on Turn 3:**
   - At Event 9, `v154_arm3` applied its first file update to `about.component.html`.
   - At Event 11, it ran `git diff --check; git diff ...`.
   - The diff revealed that the replacement line had an indentation difference relative to the surrounding HTML block.
3. **Self-Correction & Re-Verification Loop (Turns 4–6):**
   - At Event 13, the agent explicitly reasoned:
     *"The first edit exposed an indentation-only difference in the patch, so I’m correcting that to ensure the final diff is strictly the requested text change."*
   - At Event 14, it executed a secondary replacement to adjust leading spaces.
   - At Event 16, it re-ran `git diff --check; git diff ...` to verify the patch was 100% clean.
4. **Why Baseline Avoided the Loop in Block 03:**
   - In Block 03, `baseline_arm1` did not attempt `rg`; it ran `Get-Content` directly, applied the replacement with exact whitespace on attempt 1, verified the patch in 1 turn, and exited immediately in 4 turns.
   - *(Note: In Block 02, the exact reverse occurred—the baseline made a whitespace error on attempt 1 and took 10 turns, while Engram finished in 7 turns).*
5. **Conclusion & Validity:**
   - The run was 100% valid, produced an identical single-line patch, passed all gates, and adhered strictly to protocol.
   - Multi-turn quadratic token accumulation in Codex CLI (where each additional conversational turn adds ~17k cached prompt tokens) explains why a 10-turn run costs ~156k tokens while a 4-turn run costs ~69k tokens.

---

## 4. Confirmation of Engram MCP Tool Invocations

Across all 9 completed runs:
- `mcp_tool_calls_count`: **0 (Zero)** in all runs across Arms 1, 2, and 3.

### Explanation:
- The prompt given to the agent for Task A is:
  `Fix typo in projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html: change "Angular Cli Tips" to "Angular CLI Tips".`
- Because the prompt explicitly provides the exact target file path and target strings, Codex CLI has no need to query Engram's semantic index, call `engram_get_context`, or call `engram_prepare_task`.
- In Arm 3 (v1.5.4), Turn-0 context injection was delivered via system context, but no interactive MCP tool was invoked.
- **Scientific Implication:** Task A operates purely as a **minimal-task overhead and agent variance baseline**. Token differences on Task A reflect MCP prompt-schema overhead and stochastic file-editing retry loops, rather than retrieval efficiency. Testing Engram's core retrieval and boundary-containment capabilities requires **Task B** (the architectural multi-file feature).

---

## 5. Recalculated Aggregate Telemetry & Statistics

### 5.1 Per-Block Pairwise Results
$$\Delta = \text{Baseline} - \text{v1.5.4}, \quad \text{Savings \%} = \frac{\Delta}{\text{Baseline}} \times 100$$

| Block | Baseline Tokens (Turns) | v1.5.4 Tokens (Turns) | Absolute Delta | Savings % | Verdict |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Block 01** | 64,845 (4 turns) | 66,184 (4 turns) | -1,339 tokens | -2.06% | Slight Schema Overhead |
| **Block 02** | 153,054 (10 turns) | 116,357 (7 turns) | +36,697 tokens | **+23.98%** | **Efficiency Gain** |
| **Block 03** | 69,714 (4 turns) | 156,151 (10 turns) | -86,437 tokens | -123.99% | Edit Retry Regression |

### 5.2 Descriptive Statistics Across Blocks 01–03
- **Median Absolute Delta:** **-1,339 tokens**
- **Range of Absolute Deltas:** **[-86,437, +36,697] tokens**
- **Median Savings Percentage:** **-2.06%**
- **Range of Savings Percentages:** **[-123.99%, +23.98%]**
- **Arithmetic Mean of Percentages:** **-34.03%**
- **Pooled Baseline Tokens (Sum):** 287,613 tokens
- **Pooled v1.5.4 Tokens (Sum):** 338,692 tokens
- **Pooled Absolute Delta:** **-51,079 tokens**
- **Pooled Savings Percentage:** **-17.76%**

---

## 6. Quota Exposure Estimate for Blocks 04–05

- **Remaining Executions:** 2 blocks $\times$ 3 arms = **6 executions**.
- **Historical Observed Range:** 64,845 tokens (min) to 156,151 tokens (max).
- **Observed Mean Rate:** 103,687 tokens per execution (~311,060 tokens per block).
- **Projected Exposure for Blocks 04–05:**
  - Expected (Mean): **~622,000 total tokens**
  - Conservative Upper Bound: **~900,000 total tokens**
  - Best-Case Lower Bound: **~402,000 total tokens**
- **Financial Billed Cost:** **$0.00 USD** (ChatGPT Subscription token).
- **Commercial List-Price Equivalent:** **~$0.91 USD** (at $1.47 / 1M token blended rate).

---

## 7. Stop Condition

In strict accordance with experimental instructions:
- **Zero live model calls were made during this audit.**
- **Blocks 04 and 05 have NOT been launched.**
- No code or frozen baseline artifacts were modified.
- Execution is stopped here to await explicit user confirmation before proceeding with Blocks 04–05.
