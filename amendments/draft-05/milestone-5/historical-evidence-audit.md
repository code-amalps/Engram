# Milestone 5A: Historical Benchmark Evidence Audit
## Reconciliation of Historical Task A and Task B Experimental Data

**Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/async-precommit-guard`  
**HEAD Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`  
**Document ID:** `ENGRAM-A05-M5A-HISTORICAL-AUDIT`  
**Classification:** Pre-Experiment Methodological and Empirical Audit  

---

### 1. Executive Summary

This audit evaluates all historical benchmark runs recorded under Engram Stage 1 (Blocks 01 through 07) across **Task A** (`task_a_typo`) and **Task B** (`task_b_remember_me`). 

The objective is to establish an uncompromised empirical baseline for designing the Milestone 5 controlled token efficiency experiment. This audit enforces strict methodological hygiene:
- Historical runs that failed functionally or terminated prematurely due to token ceilings **must not be treated as evidence of token savings**.
- Runs where Engram MCP tools were not actually executed (Task A) or where tool execution was rejected by client security approval policies (Task B Block 07) **must be segregated from primary context-efficiency evaluations**.
- Experimental blocks conducted under differing binary versions, token ceilings, or harness configurations **must not be pooled into composite savings claims**.

---

### 2. Forensic Audit of Historical Benchmark Runs

#### 2.1 Task A Empirical Record (Blocks 01–05, 15 Executions)
Task A evaluated a minimal localized change: correcting a single-character typo in `about.component.html` within the Angular starter repository. Five blocks of 3-arm triads were executed (Arm 1: unassisted baseline, Arm 2: Engram v1.5.1, Arm 3: Engram v1.5.4) using OpenAI Codex CLI (`0.164.0-alpha.11.1`, model `gpt-5.6-luna`).

| Block | Arm | Total Tokens | Cached Input | Uncached Input | Output | Turns | MCP Calls | Score ($C$) | Outcome |
| :---: | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :--- |
| **B01** | `baseline_arm1` | 64,845 | 41,600 | 22,738 | 507 | 4 | 0 | 1.00 | Clean completion |
| **B01** | `v151_arm2` | 76,674 | 70,400 | 5,289 | 985 | 5 | 0 | 1.00 | Tool schema overhead |
| **B01** | `v154_arm3` | 66,184 | 59,264 | 6,491 | 429 | 4 | 0 | 1.00 | Clean completion |
| **B02** | `v151_arm2` | 104,566 | 98,432 | 5,275 | 859 | 7 | 0 | 1.00 | Clean completion |
| **B02** | `v154_arm3` | 116,357 | 108,160 | 7,379 | 818 | 7 | 0 | 1.00 | Clean completion |
| **B02** | `baseline_arm1` | 153,054 | 141,440 | 10,470 | 1,144 | 10 | 0 | 1.00 | Extended retry loop |
| **B03** | `v154_arm3` | 156,151 | 143,616 | 11,420 | 1,115 | 10 | 0 | 1.00 | Edit retry loop |
| **B03** | `baseline_arm1` | 69,714 | 50,176 | 19,063 | 475 | 4 | 0 | 1.00 | Clean completion |
| **B03** | `v151_arm2` | 125,635 | 99,200 | 25,290 | 1,145 | 8 | 0 | 1.00 | Edit retry loop |
| **B04** | `baseline_arm1` | 153,441 | 141,568 | 10,763 | 1,110 | 10 | 0 | 1.00 | Extended retry loop |
| **B04** | `v154_arm3` | 234,981 | 213,120 | 20,204 | 1,657 | 15 | 0 | 1.00 | Edit retry loop |
| **B04** | `v151_arm2` | 155,991 | 143,360 | 11,545 | 1,086 | 10 | 0 | 1.00 | Clean completion |
| **B05** | `v151_arm2` | 202,219 | 177,408 | 23,249 | 1,562 | 13 | 0 | 1.00 | Extended retry loop |
| **B05** | `baseline_arm1` | 105,491 | 96,128 | 8,698 | 665 | 7 | 0 | 1.00 | Clean completion |
| **B05** | `v154_arm3` | 165,213 | 144,512 | 19,370 | 1,331 | 9 | 0 | 1.00 | Edit retry loop |

##### Critical Task A Findings:
1. **Zero Active MCP Tool Execution:** Across all 15 executions, `mcp_tool_calls_count` was exactly 0. Because the prompt stated the file path (`about.component.html`) and the exact string to change, the model bypassed Engram MCP entirely, inspecting and modifying the file via native PowerShell commands.
2. **Dominance of Context History Compounding:** 88.61% of all tokens consumed across the 15 runs were **cached input tokens**. In Codex CLI, each subsequent turn re-transmits the complete conversation history. Total token consumption scaled quadratically with turn count:
   - 4-turn runs: ~64k–70k tokens
   - 7-turn runs: ~104k–116k tokens
   - 10-turn runs: ~153k–156k tokens
   - 13–15 turn runs: ~202k–235k tokens
3. **Turn Variance Outweighed Tool Effects:** Net token variance was driven almost entirely by whether the agent completed the edit on the first attempt or entered a git/grep retry loop. 
4. **Methodological Conclusion for Task A:** Task A measures **static tool schema definition overhead** and **agent edit retry variance**. It does not evaluate context retrieval or architectural navigation.

---

#### 2.2 Task B Empirical Record (Blocks 06 and 07)
Task B evaluated a multi-file architectural feature: implementing a "Remember Me" login username persistence feature across Angular components, NgRx actions/effects/reducers, and local storage.

##### Block 06: Baseline Arm 1 Failure
- **Arm:** `baseline_arm1` (Native Codex, no Engram assistance)
- **Token Ceiling:** 250,000 tokens
- **Termination Point:** Turn 9 (109.26s) at **253,748 tokens** (Ceiling breached)
- **Functional Score:** $C = 0.00$ (Harness short-circuit on non-SUCCESS termination)
- **Root Cause:** Without an architectural symbol index, the baseline agent performed workspace-wide regex searches (`rg`) and dumped 6 entire source files into terminal stdout. The turn context exploded from 13.2k to 37.2k tokens. By Turn 9, cumulative prompt re-ingestion reached 250,026 tokens.
- **Harness Action:** Process terminated. Arm 2 and Arm 3 were cancelled by the stop-on-failure rule.

##### Block 07: Candidate Arm 3 Failure
- **Arm:** `v154_arm3` (Candidate Phase 1 Binary)
- **Token Ceiling:** 250,000 tokens
- **Termination Point:** Turn 11 (176.40s) at **270,506 tokens** (Ceiling breached by 20,506 tokens)
- **Functional Score:** $C = 0.00$
- **Root Causes:**
  1. *MCP Tool Rejection:* Codex executed non-interactively with `approval_policy: never`. Because no per-tool auto-approval overrides were supplied, Turn 1's `engram_inspect_symbol` failed immediately with `"MCP tool call requires approval, but approval policy is never"`.
  2. *Contract-Induced Refusal:* Turn-0 instruction injection locked the Change Contract to `core.module.ts` only. When the agent saw that Task B required editing `app.component.ts` and `auth/`, it followed the contract directive (`"Confine edits strictly to authorized expected files"`) and refused to proceed.
  3. *Fallback Context Inflation:* Blocked from using MCP tools, the agent dumped 4 entire TypeScript files via PowerShell `Get-Content`, compounding turn context to ~29.9k tokens.
  4. *In-Flight Overrun Mechanism:* Turn 10 finished at 240,504 tokens (9,496 tokens below the 250k ceiling). Turn 11 initiated with 29,918 input tokens. Because OpenAI API usage is reported only on turn completion, Turn 11 completed at 270,506 tokens before the external process killer could intervene.

---

### 3. Analysis of Ceiling Policies & Turn-Level Token Accounting

#### 3.1 The 175,000 Token Ceiling Retrospective Simulation
Milestone 2 proposed a tighter ceiling: **175,000 tokens per arm**, including a **15,000 token reserve**, triggering a stop at **160,000 nominal tokens**.

A retrospective simulation of this proposed policy against historical evidence yields:
1. **Task A Impact:**
   - Block 04 `v154_arm3` (234,981 tokens, Turn 15) would have breached the 160k threshold at Turn 11 (~165k tokens) and been killed.
   - Block 05 `v151_arm2` (202,219 tokens, Turn 13) would have breached at Turn 11 (~168k tokens) and been killed.
   - 2 out of 15 Task A runs would have failed due to ceiling truncation rather than completing successfully.
2. **Task B Impact:**
   - Block 06 Baseline: Reached 145,999 tokens at Turn 6. Turn 7 consumed 34,867 tokens, jumping to 180,866 tokens. Under a 175k ceiling, Turn 7 would have overshot the 160k trigger and terminated at 180.8k tokens.
   - Block 07 Candidate: Reached 151,280 tokens at Turn 7. Turn 8 jumped to 180,793 tokens, overshooting the 160k trigger.

#### 3.2 Fundamental Latency and Overrun Invariants
The historical data conclusively proves that **an external polling token guard cannot guarantee a zero-overrun hard ceiling**:
- In conversational coding harnesses, token consumption occurs server-side within the model provider's API.
- The harness receives usage records only when a turn completes and emits a telemetry event.
- If a turn begins with 150,000 cumulative tokens and the model processes 30,000 input/output tokens in that single turn, the turn completes at 180,000 tokens before any local polling loop or OS job object can terminate the process.
- **Therefore, the gap between the nominal stop threshold and the hard ceiling must be strictly greater than the maximum conceivable single-turn token consumption.**

---

### 4. Classification of Tool Execution Status

Historical analysis reveals three distinct states of Engram MCP tool execution that must be disambiguated in all future telemetry:

```
[Tool Attempted]  -->  [Tool Dispatched]  -->  [Tool Executed Successfully]
(Model emitted JSON-RPC) (Harness passed to MCP) (MCP server returned payload)
```

1. **Attempted but Blocked by Client Policy:** Model issued `engram_inspect_symbol`; Codex rejected it locally with `"MCP tool call requires approval"` (Task B Block 07).
2. **Dispatched and Failed:** Harness forwarded call to binary, but binary returned an application error or crash.
3. **Successfully Executed:** Tool returned valid AST context or symbol metadata which was consumed by the model.
4. **Never Attempted:** Model solved task using native shell commands without issuing any MCP calls (Task A Blocks 01–05).

---

### 5. Historical Data Suitability Matrix

| Dataset / Run | Suitable for Descriptive Analysis? | Suitable for Protocol Design? | Suitable for Statistical Inference? | Suitable for Future Baseline? | Exclusion / Inclusion Rationale |
| :--- | :---: | :---: | :---: | :---: | :--- |
| **Task A (Blocks 01–05)** | **YES** | **YES** | **NO** | **CONDITIONAL** | Zero MCP calls were made. Measures schema overhead and turn variance only. Cannot support context efficiency claims. |
| **Task B (Block 06 Baseline)** | **YES** | **YES** | **NO** | **NO** | Terminated on ceiling breach at Turn 9. Functional acceptance $C = 0.00$. Incomplete run cannot serve as matched pair. |
| **Task B (Block 07 Candidate)** | **YES** | **YES** | **NO** | **NO** | MCP blocked by client approval policy; contract locked target inappropriately; breached ceiling at Turn 11. Ineligible for savings calculation. |
| **Historical Pooled Totals** | **NO** | **NO** | **NO** | **NO** | **STRICTLY PROHIBITED.** Pooling Task A with Task B, or pooling failed ceiling runs with completed runs, produces mathematically invalid metrics. |

---

### 6. Evidence Reconciliation Conclusion

1. **Zero Documented Context Savings in Historical Benchmarks:** To date, there is **zero valid empirical evidence** demonstrating that Engram reduced total token consumption in historical live benchmarks. Task A did not use Engram context, and Task B failed in both baseline and treatment arms.
2. **Root Causes Are Operational, Not Inherent:** The historical failures were driven by client approval policies (`approval_policy: never`), restrictive Turn-0 contract boundaries, and unconstrained PowerShell file dumping, rather than fundamental flaws in Engram's index.
3. **Milestone 5 Requirement:** The new experiment must evaluate an unencumbered workflow where context is either pre-injected cleanly or MCP tools execute without client approval deadlocks, under tasks that genuinely require architectural discovery.
