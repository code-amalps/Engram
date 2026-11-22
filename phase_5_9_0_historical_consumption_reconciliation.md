# Engram Phase 5.9.0: Historical Consumption Reconciliation & Token Budget Analysis

**Execution Phase:** 5.9.0  
**Target Repositories & Evidence:** `PHASE_5_4_TASK_A_LIVE_01_VS_LIVE_02_FORENSIC_AUDIT.md`, `PHASE_5_4_13_TASK_B_TETRAD_LIVE_02_EXECUTION_REPORT.md`  
**Execution Mode:** FORENSIC EVIDENCE RECONCILIATION  

---

## 1. Executive Summary

Phase 5.9.0 conducted an exhaustive reconciliation of historical token consumption across Task A and Task B to resolve the informal "500,000-token" budget estimate mentioned in earlier phases.

**Key Finding:** A 500,000-token global ceiling is **grossly inadequate** for a complete 10-arm evaluation across Task A and Task B. In historical Live 02 execution, **Task B Arm B alone consumed 512,449 tokens** before halting. Evaluating both tasks across five arms requires a calibrated multi-tier budget between 1.5M and 3.8M tokens depending on the chosen scenario.

---

## 2. Historical Empirical Consumption Records

### 2.1 Task A (`task_a_typo`) Live 02 Telemetry

| Arm | Identity | Status | Turns | Total Model Tokens | Prompt Total | Prompt Cached | Prompt Uncached | Completion |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Arm A** | `arm_a_native` | `SUCCESS` | 5 | 115,513 | 114,568 | 100,096 | 14,472 | 945 |
| **Arm B** | `arm_b_cand3` | `HALTED` | 7 | 240,819 | 239,745 | 195,712 | 44,033 | 1,074 |
| **Arm C** | `arm_c_cand4_always_on` | `HALTED` | 9 | 226,732 | 225,438 | 197,760 | 27,678 | 1,294 |
| **Arm D** | `arm_d_cand4_adaptive` | `SUCCESS` | 4 | **84,226** | 83,588 | 71,808 | 11,780 | 638 |
| **Arm E** | `arm_e_cand5_optimized` | N/A | N/A | Projected 70,000–85,000 (Synthetic: 2,910) | N/A | N/A | N/A | N/A |

**Key Takeaways from Task A:**
- Native Baseline completed cleanly in 5 turns and 115,513 tokens.
- Always-On arms (B and C) suffered from exploratory tool-calling loops, halting at 240k and 226k tokens when breaching the 205,000 ceiling threshold ($250,000 - 45,000$).
- Adaptive bypass (Arm D) recognized the task as `SIMPLE` $\rightarrow$ `BYPASS` in 2.01 ms, omitted MCP tools, and finished in 4 turns and 84,226 tokens (-27.09% vs Native).

### 2.2 Task B (`task_b_remember_me`) Live 02 Telemetry

| Arm | Identity | Status | Turns | Total Model Tokens | Prompt Total | Prompt Cached | Prompt Uncached | Completion |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Arm B** | `arm_b_cand3` | `HALTED` | 11 | **512,449** | 508,632 | 461,312 | 47,320 | 3,817 |
| **Arm A** | `arm_a_native` | `SKIPPED` | 0 | 0 (Hist Block 06: 253,748 at T9) | 0 | 0 | 0 | 0 |
| **Arm C** | `arm_c_cand42_always_on` | `SKIPPED` | 0 | 0 (Projected ~300,000–400,000) | 0 | 0 | 0 | 0 |
| **Arm D** | `arm_d_cand42_adaptive` | `SKIPPED` | 0 | 0 (Projected ~250,000–350,000) | 0 | 0 | 0 | 0 |
| **Arm E** | `arm_e_cand5_optimized` | `SKIPPED` | 0 | 0 (Projected ~180,000–250,000) | 0 | 0 | 0 | 0 |

**Key Takeaways from Task B:**
- Candidate 3 serialized unpruned symbol AST subgraphs (272 KB and 774 KB individual payloads), inflating single-turn prompt context to **56,611 tokens**.
- In 11 turns, cumulative usage reached 512,449 tokens.
- At turn 11, remaining budget was 37,551 tokens, which was less than the required 45,000 reserve (and less than the estimated next turn of 56,611 tokens). The runner halted Arm B with `PRE_DISPATCH_HEADROOM_EXHAUSTED`.
- In historical Block 06, Native Baseline required 9 turns and 253,748 tokens before halting at 250,000. Full unassisted native completion is projected at 350,000–450,000 tokens.

---

## 3. The 500,000-Token Ambiguity Resolved

### Why Was 500,000 Proposed?
Forensic review indicates the "350,000–500,000" figure mentioned in earlier reports reflected:
1. Either a rough estimate for **Task A alone** across five arms (where expected consumption is ~550,000 tokens), OR
2. A single arm budget for Task B (where the ceiling was set to 550,000 tokens).

### Total Expected Consumption Across Both Tasks:
- **Task A (5 Arms):**
  $115\text{k} (\text{Native}) + 240\text{k} (\text{Cand 3}) + 226\text{k} (\text{Cand 4.2}) + 84\text{k} (\text{Cand 4.2 Adaptive}) + 75\text{k} (\text{Cand 5}) \approx \mathbf{740,000\text{ tokens}}$.
- **Task B (5 Arms):**
  $350\text{k} (\text{Native}) + 512\text{k} (\text{Cand 3}) + 350\text{k} (\text{Cand 4.2}) + 300\text{k} (\text{Cand 4.2 Adaptive}) + 200\text{k} (\text{Cand 5}) \approx \mathbf{1,712,000\text{ tokens}}$.
- **Grand Total Expected Exposure:** $\approx \mathbf{2,452,000\text{ tokens}}$.

Attempting to run all 10 executions under a 500,000-token global ceiling would cause the benchmark to abort prematurely during Task A or immediately in the first arm of Task B, producing zero valid comparative data.
