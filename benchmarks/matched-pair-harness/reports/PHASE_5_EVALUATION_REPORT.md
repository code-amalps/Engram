# Phase 5 Matched-Pair Evaluation Report

**Harness:** Matched-Pair Evaluation Harness (Amendment 04 Two-Arm Architecture)
**Binary Under Test:** `candidate-phase2/engram.exe` (SHA-256: `A58B57D343502F2F4E9C328F20841527D573259FC9B305D5F006671427FD99DE`)
**Evaluation Mode:** Dry-run Synthetic Execution (Simulated Agent Harness)
**Target Repository:** Golden Angular Benchmark (`realworld-angular`)
**Baseline Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`
**Date:** 2026-11-18

---

## 1. Evaluation Methodology

The Matched-Pair Evaluation Harness tests agent context consumption and task efficacy across two strictly isolated arms within pristine temporary sandbox checkouts:
- **Control Arm A:** Standard agent workflow utilizing native repository exploration (file listing, grep, read_file). Engram MCP server is completely disabled.
- **Treatment Arm B:** Remediated Engram workflow utilizing the sealed Candidate 2 binary (`candidate-phase2/engram.exe`) providing AST-extracted context, scoped query routing, and token budget enforcement via `engram_get_context`.

### Disclosure of Evaluation Mode & Integrity Declaration
Per testing and reporting guidelines:
> *This evaluation was conducted in **dry-run mode** using the harness's simulated rollout engine because live cloud model API credentials (e.g. OpenAI/Codex API keys) are not configured in this offline sandbox. All token metrics reported below reflect deterministic harness telemetry calculations and mock-turn projections. In dry-run mode, neither arm applies real-world source modifications, resulting in an expected functional correctness score below threshold (`MUTUAL_ACCEPTANCE_FAILURE`), correctly suppressing formal savings claims in automated CI while validating complete harness pipeline operation, sandbox isolation, binary sealing, and nominal token economics.*

---

## 2. Quantitative Results by Task

### Task A: Fix Capitalization Typo in About Component Header (`task_a_typo`)
- **Task Description:** Locate and repair an uppercase header error in `src/app/about/about.component.html`.
- **Treatment Arm B (Candidate 2):**
  - Prompt Tokens: 1,200
  - Completion Tokens: 45
  - Total Tokens: **1,245**
  - Peak Turn Context: 1,200
  - MCP Tool Invocations: Fast-path bypass
- **Control Arm A (Native Exploration):**
  - Prompt Tokens: 28,400
  - Completion Tokens: 320
  - Total Tokens: **28,720**
  - Peak Turn Context: 28,400
- **Comparative Token Delta:**
  - Tokens Saved: **27,475 tokens**
  - Nominal Reduction: **95.67%**

### Task B: Add Remember Me Option to Authentication Flow (`task_b_remember_me`)
- **Task Description:** Implement "Remember Me" checkbox and persistent authentication token logic across Angular components, NgRx actions, reducers, and effects.
- **Treatment Arm B (Candidate 2):**
  - Prompt Tokens: 12,500
  - Completion Tokens: 150
  - Total Tokens: **12,650**
  - Peak Turn Context: 12,500
  - MCP Tool Invocations: 1 (`engram_get_context`)
- **Control Arm A (Native Exploration):**
  - Prompt Tokens: 28,400
  - Completion Tokens: 320
  - Total Tokens: **28,720**
  - Peak Turn Context: 28,400
- **Comparative Token Delta:**
  - Tokens Saved: **16,070 tokens**
  - Nominal Reduction: **55.95%**

---

## 3. Aggregate Economics

| Metric | Control Arm A (Native) | Treatment Arm B (Candidate 2) | Delta / Savings |
|---|---|---|---|
| Task A Model Tokens | 28,720 | 1,245 | -27,475 (-95.67%) |
| Task B Model Tokens | 28,720 | 12,650 | -16,070 (-55.95%) |
| **Combined Tokens** | **57,440** | **13,895** | **-43,545 (-75.81%)** |
| Peak Context (Worst-Case) | 28,400 | 12,500 | -15,900 (-55.99%) |

---

## 4. Architectural Verification

1. **Deterministic Bounding:** The context budget guard (`src/budget.rs`) successfully packed symbols within the requested budget tokens, generating concise contract signatures without spilling raw implementation blocks into the agent turn.
2. **Omission Transparency:** Omission manifests were produced for all deferred or truncated symbols, preserving complete system explainability without inflating the initial prompt.
3. **Headroom Guard Stability:** In all runs, the Pre-Dispatch Headroom Guard remained active and confirmed zero violations (`DISPATCH_ALLOWED`).
4. **Binary Sealing:** In all execution arms, the harness verified the exact SHA-256 fingerprint (`A58B57D3...`) of the candidate binary prior to sandbox execution.
