# Token Usage Comparison: Pilot 01 Analysis

**Experiment ID:** `pilot_01`  
**Task ID:** `task_commit_cache_invalidation`  
**Model:** `gpt-5.6-luna` (Reasoning Effort: `medium`)  
**Date:** 2026-10-30  
**Status:** COMPARATIVE SAVINGS STRICTLY SUPPRESSED & INVALIDATED  

---

## 1. Primary Comparative Telemetry Table

All figures are drawn directly from `matched_pair_summary.json` validated against `matched_pair_summary.schema.json`:

| Telemetry Metric | Treatment Arm B (Candidate Phase 1) | Baseline Arm A (Unassisted Control) | Delta (Baseline - Treatment) | Token Savings (%) | Validated Status |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Total Model Tokens** | **117,135** | **0 (Skipped)** | `null` | `null` | `INVALIDATED_UNACCEPTED` |
| **Prompt Tokens (Total)** | 115,992 | 0 (Skipped) | `null` | `null` | `INVALIDATED_UNACCEPTED` |
| **Prompt Tokens (Cached)** | 85,888 | 0 (Skipped) | `null` | `null` | `INVALIDATED_UNACCEPTED` |
| **Prompt Tokens (Uncached)** | 30,104 | 0 (Skipped) | `null` | `null` | `INVALIDATED_UNACCEPTED` |
| **Completion Tokens** | 1,143 | 0 (Skipped) | `null` | `null` | `INVALIDATED_UNACCEPTED` |
| **Reasoning Tokens** | 500 | 0 (Skipped) | `null` | `null` | `INVALIDATED_UNACCEPTED` |
| **Peak Turn Context** | 41,011 | None | `null` | `null` | `INVALIDATED_UNACCEPTED` |
| **Turns Completed** | 4 | 0 | -4 | `null` | Disclosed |
| **Wall-Clock Duration** | 71.12 s | 0.00 s | -71.12 s | `null` | Disclosed |
| **Correctness Score ($C$)** | 0.00 | 0.00 | 0.00 | `null` | `MUTUAL_FAILURE` |
| **Functional Acceptance** | FAIL | FAIL | Parity | `null` | `NON_COMPLIANT` |
| **Quality-Adjusted Cost** | 1,171,350.0 | 0.0 | `null` | `null` | `INVALIDATED_UNACCEPTED` |

---

## 2. Mathematical Integrity and Accounting Rules

In strict compliance with Amendment 05 preregistered accounting rules:
1. **Total Model Tokens Formula:**  
   $$\text{Total Tokens} = \text{Provider Prompt Tokens} + \text{Provider Completion Tokens}$$  
   $$117,135 = 115,992 + 1,143$$
2. **Sub-Metric Containment (No Double-Counting):**
   - Cached input tokens ($85,888$) and uncached input tokens ($30,104$) are non-overlapping subsets of total prompt tokens ($85,888 + 30,104 = 115,992$). Neither is added separately to total tokens.
   - Reasoning tokens ($500$) are an informative sub-metric of completion tokens ($1,143$) and are not added separately to total tokens.
3. **Savings Calculation Formula:**  
   $$\text{Token Difference} = \text{Baseline Total Tokens} - \text{Treatment Total Tokens}$$  
   $$\text{Savings Percentage} = \left(\frac{\text{Token Difference}}{\text{Baseline Total Tokens}}\right) \times 100$$
4. **Invalidation Invariant:**  
   Under Amendment 04 Alternative B Rule 5 and Amendment 05 Section 9:
   > *"Only calculate savings if both arms have valid token telemetry and both achieve functional acceptance ($C = 1.00$). If either arm fails functional acceptance, report the token observations but suppress the successful-task savings claim."*
   Because `treatment_B` achieved $C = 0.00$ and `control_A` achieved $C = 0.00$, the token savings delta and savings percentage are strictly set to `null`.

---

## 3. Evaluation of Hypothesis H1

- **Preregistered Hypothesis H1:** Using Engram-generated engineering context reduces total AI-agent token consumption per successfully completed engineering task, compared with an otherwise identical coding-agent workflow without Engram.
- **Trial Outcome:** **UNPROVEN / INCONCLUSIVE ON TASK SUCCESS**.
- **Explanation:**  
  Treatment Arm B consumed 117,135 tokens before triggering the pre-dispatch token headroom guard without modifying any files or passing quality gates. Because the task was not completed successfully, this trial provides zero evidence supporting token reduction on a completed engineering task.
- **Observed Behavior:**  
  The agent failed to invoke Engram MCP retrieval tools (`mcp_tool_calls_count = 0`), instead issuing broad PowerShell discovery commands that saturated the context window with raw repository source text.
