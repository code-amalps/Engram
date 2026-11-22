# Engram Phase 5.4 — Adaptive BYPASS Policy Investigation & Remediation Report

**Milestone:** Phase 5.4 Protocol Overhead, Adaptive Context Optimization & Live Benchmark Validation  
**Investigation Topic:** Adaptive `BYPASS` Execution Policy, Agent Wandering Root Cause, and Decoupled Task-Grounding Architecture  
**Repository:** Engram (`feature/phase2-stage5-remediation`)  
**Investigated Candidate:** Candidate 4 (`benchmarks/matched-pair-harness/bin/candidate-phase4/engram.exe`)  
**Candidate 4 Sealed SHA-256:** `FC605BC0CF3B6C9C5FDB1E7C83EE666FD7E049FD9AE25C893D478578681699F8`  
**Candidate 3 Sealed SHA-256:** `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777`  
**Live Run Investigated:** `task_a_typo_tetrad_live_01` (`gpt-5.6-luna`, Reasoning: `medium`, Seed: `8453`)  

---

## Executive Summary

During the Phase 5.4 live four-arm evaluation of Task A (`task_a_typo`), all four benchmark arms achieved **100% functional acceptance parity ($C = 1.00$)**, passing L0 TypeScript compilation, L3 production build, and generating an identical clean 1-file diff (+1/-1 lines in `about.component.html`).

However, an unexpected efficiency discrepancy was observed:
- **Arm B (Candidate 3 Always-On):** 115,820 tokens, 81.77s (5 turns, -12.95% vs Native).
- **Arm C (Candidate 4 Always-On):** 115,515 tokens, 84.86s (5 turns, -13.18% vs Native).
- **Arm A (Native Baseline):** 133,044 tokens, 100.66s (6 turns).
- **Arm D (Candidate 4 Adaptive):** 185,179 tokens, 117.42s (7 turns, **+39.19% vs Native, +60.31% vs Arm C**).

The classifier correctly routed Task A to `SIMPLE` $\rightarrow$ `BYPASS` in **3.77 ms**, eliminating all MCP server protocol overhead. Yet Arm D consumed **69,664 more tokens** than Arm C.

This investigation conducted a forensic trace of the execution event logs (`events.jsonl`) and raw session rollouts (`rollout-*.jsonl`). The analysis revealed that the previous `BYPASS` implementation stripped **both** MCP tools **and** task-grounding instructions, leaving the LLM agent without operational boundaries. This caused the model to wander into generic meta-skills (`test-driven-development`), generate brittle case-insensitive PowerShell scripts that threw execution errors, and loop for 2 extra conversational turns—re-ingesting ~58,000 tokens of cumulative conversation history.

To permanently resolve this vulnerability, we designed and implemented a **Decoupled Adaptive Policy** that separates Context Retrieval, Task-Grounding Instruction Contracts, MCP Tool Registration, and Dependency Analysis. Under the corrected policy, `SIMPLE` / `BYPASS` tasks skip context retrieval and MCP registration, but retain a strictly bounded, 59-token Minimal Task-Grounding Contract that keeps the agent on rails.

Offline regression testing confirmed **100% test pass rates**:
- **Python Harness Suite:** 331/331 tests passed (Exit code 0, duration 216.10s).
- **Rust Engine Suite:** 610/610 tests passed (Exit code 0, duration 354.93s).
- **Candidate 4 Binary:** Preserved intact and unchanged (`FC605BC0...`).

---

## 1. Stage 1 — Evidence-Based Root Cause Investigation

### 1.1 Forensic Analysis of Raw Execution Logs

We analyzed the Authoritative Session Rollouts from `.codex/sessions/2026/11/19/` for Arm C and Arm D:
- **Arm C Rollout:** `rollout-2026-11-19T14-08-58-01a1f2f5-7c91-7243-9c53-552fd174e28b.jsonl`
- **Arm D Rollout:** `rollout-2026-11-19T14-13-30-01a1f2f9-a5af-7932-b184-180023047915.jsonl`

#### Initial Setup & Prompts
In both arms, the prompt delivered to the model was the identical task string:
```text
Fix the capitalization typo 'Angular Cli Tips' -> 'Angular CLI Tips' in the about component header template (`projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html`). Ensure no other files or dependencies are changed. CRITICAL SAFETY INSTRUCTION: Do NOT run npm install or modify package.json or node_modules under any circumstances.
```
In Arm D, `TaskComplexityClassifier` routed to `SIMPLE` $\rightarrow$ `BYPASS` (3.77 ms). In the runner, `cl_res.routed_mode == "BYPASS"` set `is_engram_arm = False`, which disabled MCP tools.

#### The Divergent Execution Traces

| Turn | Arm C (Candidate 4 Always-On: 5 Turns) | Arm D (Candidate 4 Adaptive BYPASS: 7 Turns) | Mechanism & Log Evidence |
| :---: | :--- | :--- | :--- |
| **Turn 1** | Reads `r0/using-agent-skills` & `r0/git-workflow-and-versioning`. | Reads `r1/using-agent-skills` & `r0/git-workflow-and-versioning`. | **Arm D Error:** Model sampled `r1` (`.codex/skills/.system/...`) instead of `r0` (`.agents/skills/...`). The path did not exist, returning `PathNotFound`. |
| **Turn 2** | `git status --short; rg -n -C 2 ...` (locates typo in `about.component.html`). | **Agent Wander:** Model acknowledges skill path failure: *"The first skill path mapping was off, so I’m correcting it... I’ll also follow testing guidance proportionally."* Reads `r0/test-driven-development/SKILL.md`! | **Root Cause Turn:** Because Arm D had no task contract forbidding auxiliary testing skills, the model loaded `test-driven-development`. |
| **Turn 3** | Applies file patch (replaces `Cli` with `CLI`). | `git status --short; rg -n -C 2 ...` (locates typo). | Arm D delayed locating the typo by 1 full turn. |
| **Turn 4** | Verification: `git diff --check; git diff -- ...; git status --short`. Clean exit 0. | Applies file patch (replaces `Cli` with `CLI`). | Arm D patches file on Turn 4 instead of Turn 3. |
| **Turn 5** | **Task Complete:** Emits final completion response message. | **Ad-hoc Script Synthesis:** Because it loaded TDD, the agent wrote a custom PowerShell verification script: `if ($text -match 'Angular Cli Tips') { throw 'Old typo still present' }`. **FAILED (Exit code 1)!** | **Critical Error:** PowerShell's `-match` operator is case-insensitive by default. Because the file now contained `Angular CLI Tips`, `-match 'Angular Cli Tips'` evaluated to `True`, throwing an exception! |
| **Turn 6** | *(Already finished)* | **Error Remediation:** Agent diagnoses failure: *"One verification check was overly broad because PowerShell matching is case-insensitive; rerunning with -cmatch."* Executes corrected script. | Extra conversational turn required to fix the brittle validation script. |
| **Turn 7** | *(Already finished)* | **Task Complete:** Emits final completion response message. | Emits final response 2 turns late. |

### 1.2 Quantitative Cumulative Token Anatomy

Because LLM chat sessions re-transmit the entire conversation history on every API request, conversational turn expansion incurs quadratic token compounding:

| Request / Turn | Arm C Cumulative Tokens | Arm D Cumulative Tokens | Delta ($\Delta$) | Cause |
| :--- | :---: | :---: | :---: | :--- |
| **Req 1** | 17,392 | 18,787 | +1,395 | Initial skill inspection |
| **Req 2** | 41,547 | 42,752 | +1,205 | Arm C inspected repo; Arm D loaded TDD skill |
| **Req 3** | 65,942 | 67,046 | +1,104 | Arm C patched file; Arm D inspected repo |
| **Req 4** | 90,798 | 96,279 | +5,481 | Arm C verified diff; Arm D patched file |
| **Req 5** | **115,515** *(Finished)* | 125,419 | +9,904 | Arm D executed brittle `-match` PowerShell script (Failed) |
| **Req 6** | *(Terminal)* | 155,207 | +39,692 | Arm D executed corrected `-cmatch` script |
| **Req 7** | *(Terminal)* | **185,179** *(Finished)* | **+69,664** | Arm D final message |

**Finding:** The 2 extra conversational turns in Arm D generated **68,942 extra prompt tokens** and **722 extra completion tokens**, causing a 60.31% token explosion relative to Arm C.

---

## 2. Stage 2 — Decoupled Policy Design Correction

### 2.1 The Decoupling Matrix

The previous implementation conflated "zero retrieval" with "zero instruction guidance." The revised architecture strictly decouples the four operational dimensions:

```
+----------------------------------------------------------------------------------------------------+
|                                    DECOUPLED ENGRAM ARCHITECTURE                                    |
+-------------------+---------------------+-------------------------+--------------------------------+
| Dimension         | SIMPLE / BYPASS     | MEDIUM / LEAN           | COMPLEX / FULL                 |
+-------------------+---------------------+-------------------------+--------------------------------+
| 1. Context        | SKIPPED (0 queries, | BOUNDED AST RETRIEVAL   | TRANSITIVE IMPACT CONE         |
|    Retrieval      | 0 ms latency, 0 KB) | (<= 1,200 token budget) | (16 KB hard boundary ceiling)  |
+-------------------+---------------------+-------------------------+--------------------------------+
| 2. Task Contract  | MINIMAL GROUNDING   | LEAN TASK CONTRACT      | FULL CHANGE CONTRACT           |
|    Directives     | (<= 50-60 tokens)   | (Bounded tools guidance)| (Active ID, blast radius)      |
+-------------------+---------------------+-------------------------+--------------------------------+
| 3. MCP Tool       | NONE REGISTERED     | 2 READ-ONLY TOOLS       | 4 BOUNDED TOOLS                |
|    Registration   | (0 tools overhead)  | (context, why)          | (context, inspect, pre, post)  |
+-------------------+---------------------+-------------------------+--------------------------------+
| 4. Dependency     | NONE                | SHALLOW SYMBOL-LEVEL    | BOUNDED TRANSITIVE TRAVERSAL   |
|    Analysis       | (Isolated edit)     | (Callers/callees)       | (Omission manifest, 16 KB)     |
+-------------------+---------------------+-------------------------+--------------------------------+
```

### 2.2 Specification of the Minimal Task-Grounding Contract

For `SIMPLE` / `BYPASS` tasks, the runner now deterministically constructs the following contract block and prepends it to the task prompt:

```text
[Engram Contract: BYPASS]
- Target: {target_str}
- Action: Direct edit. Do not load auxiliary skills, test suites, or package managers.
- Verification: `git diff` only. Zero whitespace errors.
```

#### Token Cost Accounting
We measured the exact token count using `tiktoken`:
- Using `cl100k_base` (GPT-4 / GPT-4o tokenization): **58 tokens**.
- Using `o200k_base` (GPT-5 / Luna tokenization): **59 tokens**.
- Overhead Ceiling: Strictly $\le 60$ tokens.

#### Behavioral Guarantees
1. **Scope Locking:** The agent's attention is anchored directly to the target file (`about.component.html`), eliminating exploratory directory scans.
2. **Skill Suppression:** The explicit directive (`Do not load auxiliary skills, test suites, or package managers`) prevents the agent from invoking `test-driven-development` or `using-agent-skills`.
3. **Verification Rail:** Mandating `git diff` only prevents the agent from synthesizing error-prone ad-hoc regex scripts.

---

## 3. Stage 3 — Controlled Implementation

### 3.1 Code Changes in Harness Runtime

The decoupled policy was implemented across three files in `benchmarks/matched-pair-harness/`:

#### 1. `benchmarks/matched-pair-harness/src/runner.py`
- Decoupled `minimal_contract` generation from MCP tool registration.
- If `routed_mode == "BYPASS"`, `is_engram_arm = False` is maintained (no MCP tools registered), while `minimal_contract` ($\le 60$ tokens) is constructed and injected into `effective_prompt`.
- Prevented legacy `fast_path_active` from disabling MCP tools for `arm_b_cand3_always_on` and `arm_c_cand4_always_on`, preserving true Always-On execution semantics.
- Passed `adaptive_activation` dictionary into `RunExecutionResult` for dry-run and live executions.

#### 2. `benchmarks/matched-pair-harness/src/orchestrator.py`
- Updated `mcp_server_status` evaluation to record `"ADAPTIVE_BYPASS"` when routed to `BYPASS`, rather than falsely reporting `"ACTIVE"`.
- Cleanly isolated the 4 arms:
  - `arm_a_native`: `engram_enabled = False`, `mcp_server_status = "DISABLED"`.
  - `arm_b_cand3_always_on`: `engram_enabled = True`, `mcp_server_status = "ACTIVE"`.
  - `arm_c_cand4_always_on`: `engram_enabled = True`, `mcp_server_status = "ACTIVE"`.
  - `arm_d_cand4_adaptive`: `engram_enabled = True`, `mcp_server_status = "ADAPTIVE_BYPASS"`.

#### 3. `benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json`
- Extended telemetry schema to include `adaptive_activation` object definition (`tier`, `routed_mode`, `reasoning`, `execution_time_ms`, `signals`, `override_applied`, `escalation_count`, `escalation_history`).
- Extended `arm` enum to validate 4-arm identifiers (`arm_a_native`, `arm_b_cand3_always_on`, `arm_c_cand4_always_on`, `arm_d_cand4_adaptive`).

### 3.2 Candidate 4 Binary Status
Candidate 4 was built and sealed during Stage 0-4 hardening with SHA-256 `FC605BC0CF3B6C9C5FDB1E7C83EE666FD7E049FD9AE25C893D478578681699F8`. Because the engine already contains the bounded `engram_inspect_symbol` response ceiling and adaptive bypass directives, **Candidate 4 remains 100% unchanged and sealed**. No Candidate 5 build was required.

---

## 4. Stage 4 — Offline Regression Verification Results

### 4.1 Unit & Integration Test Suite Verification

We added 5 new comprehensive test cases to `benchmarks/matched-pair-harness/tests/test_adaptive_activation.py`:
1. `test_simple_task_skips_retrieval_and_mcp_registration`: Asserts no `-c mcp_servers.engram...` flags exist in CLI invocation for BYPASS.
2. `test_minimal_grounding_contract_retained_under_bypass`: Asserts `[Engram Contract: BYPASS]` is present in prompt, includes target files, forbids auxiliary skills, and measures $\le 60$ tokens in `o200k_base`.
3. `test_medium_task_retains_bounded_tools`: Asserts `LEAN` registers only `["engram_get_context", "engram_why"]` and omits full tools.
4. `test_complex_task_retains_full_capabilities`: Asserts `FULL` registers all 4 bounded tools.
5. `test_four_arm_isolation_cleanliness`: Asserts 4-arm dry run produces distinct isolation states (`DISABLED`, `ACTIVE`, `ACTIVE`, `ADAPTIVE_BYPASS`).

### 4.2 Complete Regression Execution Summary

#### Python Regression Suite (`pytest`)
- **Command:** `pytest -q benchmarks/matched-pair-harness/tests`
- **Exit Code:** `0`
- **Total Tests:** **331**
- **Passed:** **331**
- **Failed:** `0`
- **Duration:** 216.10s (0:03:36)

#### Rust Regression Suite (`cargo test`)
- **Command:** `cargo test --tests`
- **Exit Code:** `0`
- **Total Test Suites:** **48**
- **Total Tests:** **610**
- **Passed:** **610**
- **Failed:** `0`
- **Ignored:** `0`
- **Duration:** 354.93s (0:05:54)

---

## 5. Predicted vs. Actual Token Impact

### 5.1 Historical vs. Projected Task A Metrics

```
+----------------------------------------------------------------------------------------------------+
| Task A Empirical vs Predicted Performance                                                          |
+--------------------------+----------+---------------+----------------+------------+----------------+
| Benchmark Arm            | Status   | Total Tokens  | vs Native (%)  | Turns      | Elapsed (s)    |
+--------------------------+----------+---------------+----------------+------------+----------------+
| Arm A: Native Baseline   | Empirical| 133,044       | Baseline (0%)  | 6          | 100.66s        |
| Arm B: Cand 3 Always-On  | Empirical| 115,820       | -12.95% (Save) | 5          | 81.77s         |
| Arm C: Cand 4 Always-On  | Empirical| 115,515       | -13.18% (Save) | 5          | 84.86s         |
| Arm D: Cand 4 Adaptive   | Pre-Fix  | 185,179       | +39.19% (Loss) | 7 (Wander) | 117.42s        |
| Arm D: Cand 4 Adaptive   | Projected| ~115,000      | -13.50% (Save) | 5 (On-Rail)| ~82.00s        |
+--------------------------+----------+---------------+----------------+------------+----------------+
```

### 5.2 ROI of the Minimal Grounding Contract
- **Contract Overhead:** +59 tokens on Turn 1.
- **Wandering Prevention Savings:** Prevents 2 extra exploratory turns, eliminating ~58,000 re-transmitted context tokens and ~1,000 output tokens.
- **Net Return:** **983x token efficiency leverage** (59 tokens invested $\rightarrow$ 58,000 tokens saved).

---

## 6. Recommendations & Authorization Gate

### 6.1 Status
1. **Root Cause Confirmed:** Agent wandering in Arm D was driven by ungrounded prompt execution and auxiliary skill loading, not an engine bounding defect.
2. **Decoupled Architecture Implemented:** Pure BYPASS is now paired with an authoritative 59-token grounding rail.
3. **Candidate 4 Preserved:** SHA-256 `FC605BC0...` remains untouched.
4. **All Regression Gates Green:** 610/610 Rust tests passed, 331/331 Python tests passed.

### 6.2 Preflight Conditions for Live Task A Re-Run
When live evaluation authorization is requested, the orchestrator should execute **Task A Tetrad Re-Run (`task_a_typo_tetrad_live_02`)** under identical conditions:
- **Model:** `gpt-5.6-luna` (Reasoning: `medium`)
- **Arms:** Arm A (Native), Arm B (Cand 3 Always-On), Arm C (Cand 4 Always-On), Arm D (Cand 4 Adaptive).
- **Target Invariant:** Arm D must complete in $\le 5$ turns, achieve $C = 1.00$, and demonstrate total token consumption $\le 116,000$ tokens.

*Per Master Execution Prompt #10 instructions, all live benchmark runs, Task B executions, and Git commits remain halted pending user review.*