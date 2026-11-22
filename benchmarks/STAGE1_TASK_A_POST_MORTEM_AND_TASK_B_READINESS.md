# Engram — Post-Task-A Engineering Review and Task B Readiness Report

**Document ID:** `ENGRAM-AUDIT-STAGE1-POST-TASK-A-TASK-B-READINESS`  
**Date:** 2026-10-28  
**Scope:** Task A Final Diagnostics & Task B Preparation Gate  
**Target Repository:** `angular-ngrx-material-starter` (Commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`)  
**Parent Specifications:** `benchmarks/cost-baseline/PREREGISTRATION.md`, `AMENDMENT_02_STAGE1_CORE_TRIAD.md`  
**Overall Decision:** **`READY AFTER FIXES`**  

---

## 1. Executive Summary & Verdict

Following the completion of all 15 executions (5 blocks $\times$ 3 arms) of Stage 1 Task A (`task_a_typo`), this report provides the comprehensive post-mortem engineering analysis of Task A and an audit of readiness for Task B (`task_b_remember_me`).

### Key Audit Conclusions:
1. **Task A Empirical Result:** Across 15 executions, all arms achieved $C = 1.00$ acceptance. However, Engram v1.5.4 consumed 738,886 tokens versus Baseline's 546,545 tokens (pooled delta: **-192,341 tokens, -35.19%**; median delta: **-59,722 tokens, -53.14%**).
2. **Root Cause Diagnosis:**
   - **Zero MCP Tool Invocations:** In all 15 runs, `mcp_tool_calls_count` was **0**. Because Task A explicitly specified the file path and replacement string in the prompt, Codex CLI had no reason to query an architectural index.
   - **MCP Schema & Prompt Tax:** Registering Engram MCP tools added prompt token overhead to every turn without delivering any operational offset.
   - **Missing Turn-0 Execution in Harness:** `runner.py` registered the MCP server, but **never executed `engram inject` or injected Turn-0 architectural context** into the agent's prompt or workspace instruction files. Arm 3 was running without its primary architectural optimization.
   - **Stochastic Edit/Retry Multiplier:** In Codex CLI, 88.61% of all tokens are cached prompt tokens rolling out prior conversation history. Whenever an agent stumbled on HTML indentation during a file edit, it entered a 2–3 turn diff-inspection and re-edit loop, inflating turn count from 4 turns (~65k tokens) to 10–15 turns (156k–235k tokens).
3. **Task B Readiness Status:** **`READY AFTER FIXES`**.
   - Task B's specification, evaluator logic, static security checks, and build gates have been completely remediated and verified by 14 unit tests.
   - However, **four operational harness fixes** (activating Turn-0 context injection, guiding MCP tool discovery, providing `rg` in PATH, and validating MCP telemetry capture) are required before live model calls can be authorized.

---

## 2. Part 1 — Task A Comprehensive Engineering Diagnostics

### 2.1 Independent Verification of Aggregate Numbers
All reported figures reconcile 100% bit-for-bit with the raw JSONL session logs in `.codex/sessions` and `reports/runs/task_a_typo_block_01..05`:

- **Total Tokens Consumed (All 15 Runs):** **1,950,516 tokens**
  - **Cached Input Tokens:** **1,728,384 tokens** (88.61% of total)
  - **Uncached Input Tokens:** **207,244 tokens** (10.63% of total)
  - **Completion Output Tokens:** **14,888 tokens** (0.76% of total)
  - *— of which Reasoning Tokens:* *2,059 tokens* (0.11% of total)
- **Per-Arm Breakdown:**
  - **Arm 1 (Baseline):** 546,545 tokens | 35 turns (mean 7.0) | 702.41s wall-clock
  - **Arm 2 (v1.5.1):** 665,085 tokens | 43 turns (mean 8.6) | 711.14s wall-clock
  - **Arm 3 (v1.5.4):** 738,886 tokens | 45 turns (mean 9.0) | 729.20s wall-clock
- **Pairwise Comparison (v1.5.4 vs. Baseline):**
  - Block 01: -1,339 tokens (-2.06%)
  - Block 02: **+36,697 tokens (+23.98% savings)**
  - Block 03: -86,437 tokens (-123.99%)
  - Block 04: -81,540 tokens (-53.14%)
  - Block 05: -59,722 tokens (-56.61%)
  - **Pooled Delta:** **-192,341 tokens (-35.19%)**
  - **Median Delta:** **-59,722 tokens (-53.14%)**

### 2.2 Why v1.5.4 Incurred Overhead
The overhead stems from four distinct, compounding factors:

1. **The MCP Registration Tax:**
   Even when an agent never invokes an MCP tool, registering an MCP server in Codex CLI requires Codex to inject the tool definitions (`name`, `description`, JSON schema for parameters) into the model's system prompt on every turn. In v1.5.1 and v1.5.4, this added ~1,500–2,000 prompt tokens per turn. Over 7–10 turns, this represents 10,000–20,000 baseline overhead tokens.
2. **Missing Turn-0 Execution in the Benchmark Harness:**
   The differentiation roadmap specified that v1.5.4 eliminates conversational exploration by providing Turn-0 static context injection. However, code inspection of `runner.py` and `orchestrator.py` reveals that the harness only passed `-c mcp_servers.engram.command=...`. It never called `engram inject` or passed Turn-0 context to Codex. Thus, v1.5.4 bore the prompt schema cost of MCP tools without receiving the Turn-0 context benefit.
3. **Repeated Conversation Rollout in Codex CLI:**
   Codex CLI operates as an agentic loop where each model request receives the entire prior conversational history. Because the initial context is ~16,000 tokens, every additional turn adds ~16,500 prompt tokens (mostly cached).
4. **The Indentation / Formatting Retry Trap:**
   When editing an Angular HTML template via tool replacement:
   - If the agent's replacement matches existing file indentation on turn 1, the run terminates in **4 turns (~65k tokens)**.
   - If the replacement introduces an indentation discrepancy, `git diff --check` catches it, causing the agent to inspect whitespace, apply a secondary patch, and re-run diff checks. This stretches the run to **7–15 turns (116k–235k tokens)**.
   - On Task A, which arm stumbled on indentation was purely stochastic: Baseline in B02 (10 turns); v1.5.1 in B05 (13 turns); v1.5.4 in B03 (10 turns) and B04 (15 turns).

### 2.3 Recommended Improvements for Trivial Tasks
To prevent efficiency regressions on minimal tasks:
1. **Adaptive Zero-Cost Small-Task Bypass:**
   When a user prompt targets a single file or a known trivial typo, the system should dynamically bypass MCP server registration entirely, avoiding the tool schema prompt tax.
2. **Pre-commit Indentation Normalization:**
   File editing tools should automatically normalize replacement indentation to match target file line prefix whitespace, preventing multi-turn edit-retry loops.
3. **Environment Hygiene (`ripgrep` in PATH):**
   In Blocks 03 and 04, agents wasted initial turns running `rg` which failed because `ripgrep` was not on Windows PowerShell PATH. Ensuring standard utilities are accessible prevents unnecessary fallback turns.

---

## 3. Part 2 — Task B Architectural Readiness Audit

### 3.1 Alignment with Frozen Preregistration
In `benchmarks/cost-baseline/PREREGISTRATION.md` Section 4, Task B is defined as:
- **Prompt:** `"Add a remember-me option to the authentication flow that persists the username across reloads."`
- **Scope:** `projects/angular-ngrx-material-starter/src/app/core/auth/*`, `app/app.component.*`, `assets/i18n/en.json`.
- **Security Rule:** Zero persistence of passwords, tokens, secrets, or JWTs to `localStorage`.
- **Allowed Build Gates:** `tsconfig.app.json` and `tsconfig.spec.json` (noEmit). Browser runtime is documented as unverified due to Angular 9 Karma deprecation.

The configuration in `benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json` strictly adheres to this specification:
- Allowed file patterns: `core/auth/**`, `app.component.*`, `assets/i18n/en.json`.
- Forbidden file patterns: `package.json`, `package-lock.json`, `node_modules/**`, `features/examples/**`.

### 3.2 Evaluator Logic & Unit Test Verification
Task B's evaluator (`evaluator.py`) implements four rigorous automated criteria:
- **REQ-1 (UI Layout):** Verifies `app.component.html` contains remember-me checkbox and username input elements.
- **REQ-2 (Auth Flow):** Verifies `app.component.ts` or `auth.effects.ts` implements username restoration from `localStorage.getItem`.
- **REQ-3 (Security Negative Check):** Fails immediately if `localStorage.setItem` is called with sensitive keywords (`password`, `token`, `jwt`, `secret`).
- **REQ-4 (Boundary & Blast Radius):** Fails if any unrelated file is touched or if bulk i18n sprawl occurs ($\ge 3$ translation files).

**Test Suite Verification:** All 14 independent unit tests in [`test_task_b_evaluator.py`](file:///c:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/tests/test_task_b_evaluator.py) pass cleanly in 0.24s, verifying that broken implementations fail closed and valid implementations pass.

### 3.3 Build and Type-Check Gates
- **`L0_typecheck`:** `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit` (**ACTIVE**; passes on golden repo with exit code 0).
- **`L2_unit_tests`:** `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.spec.json --noEmit` (**ACTIVE**; passes on golden repo with exit code 0).
- **`L3_production_build`:** `npm run build` (**ACTIVE**; generates production bundle).
- **`L1_lint`:** `npm run lint` (**DISABLED**).
  *Investigation:* Running `npm run lint` on the pristine, unedited golden repository fails with **141 ESLint errors** (`Definition for rule '@typescript-eslint/quotes' was not found`) due to legacy package version skew in this frozen fixture. Disabling `L1_lint` was not done to mask agent errors, but to avoid failing every arm on an upstream fixture defect.

### 3.4 MCP Tool Usage & Telemetry Capture in Task B
Unlike Task A (where the file path was named in the prompt), Task B requires the agent to discover where authentication logic resides in an unfamiliar 68,000-file repository.
- **Arm 1 (Baseline):** Must search manually using shell commands across multiple turns.
- **Arm 2 (v1.5.1):** Has interactive MCP tools (`engram_get_context`, `engram_preflight`, `engram_postflight`).
- **Arm 3 (v1.5.4):** Has `engram_prepare_task` and Turn-0 architectural context.

**Harness Blocker Identified:**
In `runner.py`, when Codex CLI is launched, Codex defaults to native PowerShell `exec`. To ensure that Arm 2 and Arm 3 demonstrate Engram tool usage rather than defaulting exclusively to shell greps:
1. The harness must support injecting Turn-0 context into Arm 3 via `engram.exe inject`.
2. The agent prompt or system environment must make Engram MCP tools discoverable and viable alternatives to raw shell commands.
3. Telemetry parsing in `telemetry.py` must be validated to capture both MCP tool calls and custom tool calls from Codex's JSON stream.

---

## 4. Part 3 — Design Review & Required Fixes

### 4.1 Primary Evaluation Metric
The primary outcome for Task B remains:
$$\text{Quality-Adjusted Cost } (QAC) = \frac{\text{Total Model Tokens}}{C}$$
where $C \in [0.0, 1.0]$. If any requirement or gate fails ($C < 1.0$), token savings claims are invalidated. Wall-clock duration, cached tokens, and uncached tokens are reported separately.

### 4.2 Required Fixes Prior to Task B Authorization

| Fix ID | Component | Required Action | Verification Check |
| :---: | :--- | :--- | :--- |
| **FIX-1** | `runner.py` / `orchestrator.py` | Implement Turn-0 context injection for Arm 3 using `engram inject` or prompt prepending. | Verify Turn-0 context appears in Arm 3 initial request. |
| **FIX-2** | Prompt / Environment | Add tool awareness guidance so Codex knows Engram MCP tools are available for code discovery. | Verify agent considers MCP tools in dry-run or synthetic trace. |
| **FIX-3** | Sandbox PATH | Add `ripgrep` (`rg`) to sandbox PATH to prevent Turn-1 shell failures across all arms. | `Get-Command rg` exits with code 0 in sandbox. |
| **FIX-4** | `telemetry.py` | Validate event parser against all Codex CLI tool call JSON shapes (`mcp_tool_call`, `custom_tool_call`). | Unit tests pass on multi-tool JSON streams. |

---

## 5. Gate Decision

### **Decision: `READY AFTER FIXES`**

- **Justification:** Task B's task configuration, evaluator, requirements, and test suites are sound and aligned with the preregistered protocol. However, executing live runs before implementing FIX-1 through FIX-4 would repeat Task A's failure mode (0 MCP calls, missing Turn-0 injection in Arm 3, and shell command PATH errors).
- **Execution Boundary:** **Zero live model runs have been launched.** No Task B execution will take place until the required harness fixes are implemented, reviewed, and explicitly authorized.
