# Objective Comparison: Amendment 05 Variant A vs. Variant B

**Document Identifier:** `ENGRAM_DRAFT05_VARIANT_COMPARISON`  
**Governing Context:** Engram Amendment 05 Pass 3 Offline Feasibility Investigation  
**Status:** OBJECTIVE ASSESSMENT — NO RECOMMENDATION OR SELECTION STATED

---

## 1. Overview & Evaluation Principles

This document presents an objective, side-by-side technical comparison between:
- **Variant A (Bounded Task Protocol):** Replaces Task B with a smaller, localized engineering task designed to achieve functional completion within budget.
- **Variant B (Exploratory Task B Protocol):** Retains Task B, formalizing the execution as a purely descriptive diagnostic study of exploration and MCP tool dynamics.

In compliance with project instructions, this comparison is grounded strictly in historical empirical observations (Blocks 06–08). Unsupported claims are marked `UNKNOWN`, and **no recommendation or variant preference is stated**.

---

## 2. Multi-Dimensional Comparison Matrix

| Dimension | Variant A: Bounded Task Protocol | Variant B: Exploratory Task B Protocol | Empirical Basis & Supporting Evidence |
| :--- | :--- | :--- | :--- |
| **Target Task** | Bounded Task ($\le 3$ files, $\le 2$ hops) | `task_b_remember_me` (Unmodified) | Historical Task B requires wide exploration across Angular sibling dirs. |
| **Per-Arm Ceiling** | `[OWNER TO SET]` (e.g. 175,000) | `[OWNER TO SET]` (e.g. 175,000) | Governed by Owner Decision 1. |
| **Headroom Reserve** | `[OWNER TO SET]` (Floor 15k–30k) | `[OWNER TO SET]` (Floor 15k–30k) | Informed by Task 1 $P_{90} = 34,867$ tokens. |
| **Expected Total Cost** | **Estimated 120k – 180k tokens** (if task finishes in 4–5 turns) | **Estimated 145k – 300k tokens** (each arm halts near trigger) | B06/B07 consumed ~145k–151k by Turn 6/7. |
| **Budget Overrun Risk** | **Low:** Small turn increments near completion ($\Delta \approx 15\text{k}$) | **Moderate to High:** Late-stage turns exceed 30k tokens | B06 Turn 9: $\Delta = 37,651$; B07 Turn 11: $\Delta = 30,002$. |
| **Information Value** | Measures end-to-end token savings ($S$) and code correctness ($C=1.0$) | Measures tool usage, context inflation, and exploration patterns | Variant A evaluates end-to-end outcome; Variant B evaluates process. |
| **Study Task B Specifically** | **None (0%):** Does not test Task B | **High (100%):** Directly measures Task B initial exploration | Task B is completely omitted in Variant A. |
| **Completion / Censoring Risk**| **Low to Moderate:** `UNKNOWN` pending task selection | **Near Certain (100% Censored):** Historical completion is 0/3 | All 3 historical Task B runs exceeded 175,000 tokens. |
| **Limits on Conclusions** | Findings cannot extrapolate to complex architectural refactoring | Cannot claim token savings, efficiency superiority, or acceptance | Neither variant solves both problems simultaneously. |

---

## 3. Detailed Dimension-by-Dimension Analysis

### 3.1 Expected Cost and Budget Risk
- **Variant A:** By scoping the task to $\le 3$ files and $\le 6$ turns, early turns cost ~13k–16k tokens and total trajectory can conclude around 60k–90k tokens per arm. Risk of sudden context spikes is low.
- **Variant B:** Because Task B requires exploring Angular authentication and storage architecture, cumulative tokens will invariably reach ~145k–151k tokens by Turn 6–8. If Stop-on-Failure halts after Arm 1, exposure is bounded to ~150k tokens; if both arms run, exposure reaches ~300k tokens. In addition, because late turns in Task B exceed 30,000 tokens, any race condition in stdout polling carries a high probability of an in-flight overrun into the 180,000s (`OVERRUN_CENSORED`).

### 3.2 Information Value
- **Variant A:** Delivers an unambiguous empirical test of whether Engram's context retrieval produces a net positive savings percentage ($S > 0$) while preserving functional correctness ($C = 1.0$) under an automated test suite.
- **Variant B:** Delivers granular diagnostic data on how the model navigates Angular state management, whether Turn-0 context injection dampens initial exploration commands, and how frequently the agent adopts MCP symbol inspection tools (`engram_inspect_symbol`).

### 3.3 Ability to Study Task B Specifically
- **Variant A:** Zero ability to study Task B. Replaces Task B entirely.
- **Variant B:** Maximizes continuity with historical Blocks 06, 07, and 08, allowing direct comparison of initial turn deltas and context expansion across all four blocks.

### 3.4 Completion and Censoring Risk
- **Variant A:** Risk of premature guard halting is low, provided the candidate task is properly calibrated. (Exact completion rate on a new task is `UNKNOWN` until specified).
- **Variant B:** Empirical completion probability under a 175,000 ceiling is **0% (0/3)**. Halted / right-censored observation is an operational certainty.

### 3.5 Limits on Conclusions
- **Variant A:** Does not prove that Engram can solve or save tokens on diffuse, multi-file architectural refactoring tasks.
- **Variant B:** Cannot support any public or comparative marketing claim of token efficiency or cost savings, as no arm will achieve functional acceptance.

---

## 4. Objective Summary Matrix for Owner Decision

| Decision Factor | Variant A | Variant B |
| :--- | :---: | :---: |
| **Can claim token savings if successful?** | **YES** (Requires $C=1.0$) | **NO** (Claims strictly out of scope) |
| **Tests real-world Task B repository?** | NO (Uses smaller bounded task) | **YES** (Uses `task_b_remember_me`) |
| **Expected execution outcome** | Task completion ($C=1.0$) | Right-censored halt (`OVERRUN_CENSORED`) |
| **Harness changes required** | Register new task config JSON | None (Uses existing configuration) |
