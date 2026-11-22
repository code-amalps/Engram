# Engram — Stage 1 Block 07 Offline Remediation Milestone Closure Report

**Milestone Name:** Stage 1 Block 07 Offline Remediation  
**Closure Date:** October 29, 2026  
**Status:** CLOSED (OFFLINE VERIFIED)  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/async-precommit-guard`  
**Base Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`  
**Candidate Binary Path:** `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`  
**Candidate Binary SHA-256:** `f56347ef2f0bfd4854176a3ec3c86d8d20b4c0e5954f04e4464998f623d85bee`  
**Golden Repository Baseline Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` (18 modified files verified intact)  

---

## 1. Executive Summary & Milestone Scope

This report formally closes the **Stage 1 Block 07 Offline Remediation Milestone**.

Following the live Block 07 execution breach (270,506 tokens consumed, $C = 0.00$), a thorough post-mortem root-cause investigation identified three mechanical failure modes:
1. `engram_inspect_symbol` was blocked by the client's non-interactive approval policy (`approval policy is never`).
2. The change contract scoped authorization to only 1 of the 5 required Task B files (`auth.actions.ts`), causing the candidate to fall back to shell-based whole-file inspection.
3. The pre-dispatch token headroom guard was missing/ineffective in multi-turn execution, allowing context to expand uncontrolled beyond the 250,000-token per-arm ceiling.

All remediation engineering and subsequent corrective audits were conducted **strictly offline**. Zero live inference requests, model API calls, or live benchmark executions occurred during this milestone.

---

## 2. Implemented Fixes & Modified Files

### Fix 1: Fine-Grained MCP Server Auto-Approval
- **Root Cause:** In headless non-interactive mode (`codex exec`), tool calls without explicit approval configurations fail with `approval policy is never`.
- **Implementation:** In `benchmarks/matched-pair-harness/src/runner.py` (`build_codex_command`), dynamic CLI `-c` configuration explicitly enables and approves the four allowlisted Engram tools (`engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight`) and sets `mcp_servers.engram.default_tools_approval_mode="approve"`. Full sandbox isolation (`-s workspace-write`) is preserved; no global security bypasses were applied.
- **Affected File:** `benchmarks/matched-pair-harness/src/runner.py`

### Fix 2: Change Contract Target File Scope Expansion
- **Root Cause:** The Task B specification only declared a single target file, causing `engram inject` to authorize only `auth.actions.ts`. When the agent attempted to edit the other 4 files, the contract blocked the edits, prompting repeated file dumps.
- **Implementation:** In `benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json`, all five target files are declared (`auth.actions.ts`, `auth.effects.ts`, `app.component.ts`, `app.component.html`, `en.json`). In `runner.py`, target files are passed into the prompt context for `engram inject` without in-place config mutation.
- **Affected Files:**
  - `benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json`
  - `benchmarks/matched-pair-harness/src/runner.py`

### Fix 3: Real Multi-Turn Pre-Dispatch Token Headroom Guard
- **Root Cause:** In `codex exec`, `turn.started` is emitted only once at Turn 1 launch (and 0 times in native rollout files). Multi-turn transitions occur via tool calls without recurring `turn.started` events. The original guard failed to trigger during Turns 2..11.
- **Implementation:**
  1. `StreamingTokenTracker` tracks pending tools (`has_pending_tool`) and multi-turn lifecycle boundaries via `is_pre_dispatch_boundary()`.
  2. Evaluates headroom ($\text{remaining} = \text{ceiling} - \text{effective\_tokens}$; $\text{estimated} = \max(\text{last\_request\_tokens}, \text{headroom\_reserve})$) on tool initiation (`item.started`, `custom_tool_call`), usage snapshots (`token_usage_record`), and tool completions (`item.completed`, `custom_tool_call_output`).
  3. Single-turn completions without tool calls are protected against false halts.
  4. Responsive polling interval lowered to 50ms.
  5. When triggered below ceiling: sets `pre_dispatch_guard_triggered = True`, `token_ceiling_breached = False`, `stop_reason = "PRE_DISPATCH_HEADROOM_EXHAUSTED"`, and `orchestrator.py` assigns `execution_status = "PRE_DISPATCH_HEADROOM_EXHAUSTED"`.
  6. Cumulative tokens at stop are faithfully preserved.
- **Affected Files:**
  - `benchmarks/matched-pair-harness/src/runner.py`
  - `benchmarks/matched-pair-harness/src/orchestrator.py`

### Additional Mitigation: Shell Retrieval Guidance
- **Implementation:** For the candidate arm, `SHELL_RETRIEVAL_GUIDANCE` is injected into the prompt, explicitly instructing the model to prefer Engram MCP tools, forbidding whole-file shell dumps, and mandating bounded line ranges (`Select-Object -First 50`, `rg -n -C 3`). The baseline arm remains unassisted and receives no guidance.
- **Affected File:** `benchmarks/matched-pair-harness/src/runner.py`

### Configurable Headroom Reserve & Telemetry Schema Updates
- **Implementation:** Added `--headroom-reserve <int>` (default: 15,000 tokens) to the orchestrator CLI parser, propagated through `Orchestrator` to `CodexRunner`. Updated `run_telemetry.schema.json` with an optional `headroom_guard` object (`headroom_reserve`, `pre_dispatch_guard_triggered`, `stop_reason`, `cumulative_tokens_at_stop`, `estimated_next_turn_cost`).
- **Affected Files:**
  - `benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json`
  - `benchmarks/matched-pair-harness/src/orchestrator.py`
  - `benchmarks/matched-pair-harness/src/runner.py`

---

## 3. Offline Test Evidence & Reported Counts

All test verification was executed strictly offline using synthetic mocks, schema validation, and stdio JSON-RPC interactions:

1. **Focused Remediation Test Suite (`benchmarks/matched-pair-harness/tests/test_block07_remediation.py`):**
   - Result: **14 passed out of 14 collected** (100% pass rate in 36.04s).
   - Covered:
     * `test_actual_ceiling_breach_distinct_classification`
     * `test_build_codex_command_mcp_approvals`
     * `test_candidate_binary_mcp_tools`
     * `test_codex_features_list_with_mcp_config`
     * `test_headroom_reserve_configurability_and_orchestrator_integration`
     * `test_inject_does_not_mutate_task_config_in_place`
     * `test_inject_with_task_b_target_files`
     * `test_orchestrator_execution_status_mapping_and_schema`
     * `test_runner_pre_dispatch_guard_triggers`
     * `test_runner_pre_dispatch_guard_triggers_realistic_multiturn`
     * `test_shell_retrieval_guidance_prompt_injection`
     * `test_streaming_token_tracker_headroom_guard`
     * `test_streaming_token_tracker_is_pre_dispatch_boundary_multiturn`
     * `test_task_b_config_contains_five_target_files`

2. **Safety Audit & Two-Arm Protocol Suites:**
   - Command: `pytest benchmarks/matched-pair-harness/tests/test_harness_safety_audit.py benchmarks/matched-pair-harness/tests/test_amendment_04_two_arm.py -v`
   - Result: **31 passed out of 31 collected** (100% pass rate in 19.50s).

3. **Full Matched-Pair Harness Regression Suite:**
   - Command: `pytest benchmarks/matched-pair-harness/tests/ -v`
   - Result: **191 passed out of 191 collected** (100% pass rate in 113.95s).

---

## 4. Preservation & Integrity Evidence

1. **Candidate Binary Preservation:**
   - Executable: `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`
   - Verified SHA-256: `f56347ef2f0bfd4854176a3ec3c86d8d20b4c0e5954f04e4464998f623d85bee` (Unchanged).
2. **Frozen Control Binaries:**
   - v1.5.1 SHA-256: `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` (Unchanged).
   - v1.5.4 SHA-256: `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` (Unchanged).
3. **Golden Repository Preservation (`C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`):**
   - Head commit: `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.
   - Exactly 18 dirty files preserved in modified state; untracked `.engram/` preserved.
4. **Git Operations:**
   - Working tree branch: `feature/async-precommit-guard`.
   - Zero git commits, pushes, tags, releases, stashes, cleans, or reverts occurred.

---

## 5. Known Limitations & Scientific Boundaries

1. **Best-Effort External Guard (Residual Concurrency Window):**
   The harness monitors child execution via external stdout streaming and filesystem rollout tailing on a 50ms polling loop. If Codex completes a tool locally and dispatches an HTTP request to the upstream API within $< 50\text{ms}$ before the runner's polling cycle executes, an outbound network request could theoretically be in flight at the moment `terminate_pid_tree` is invoked. While evaluating headroom upon tool initiation provides maximum external containment, external process polling cannot provide a mathematical zero-millisecond kernel/middleware block.
2. **No Token-Savings Claim Established:**
   All fixes completed in this milestone are harness-level, task-scoping, and protocol-containment fixes. Because no live benchmark has been run since the Block 07 failure, **no comparative token-savings claim, efficiency percentage, or production readiness has been established**.

---

## 6. Next Milestone Definition

### Milestone Name:
**Stage 1 — Controlled End-to-End Token-Efficiency Validation**

### Objective:
Execute an authorized live matched-pair benchmark under ratified Amendment 04 protocol rules to evaluate whether the remediated candidate achieves valid functional correctness ($C = 1.0$) and verified token savings compared to the unassisted baseline.

### Mandatory Prerequisites:
1. Stage 1 Block 07 Offline Remediation Milestone formally reviewed and closed.
2. Amendment 04 protocol and benchmark configuration remain unchanged:
   - Candidate arm executed first (`v154_arm3`); baseline arm executed second (`baseline_arm1`); v1.5.1 arm deferred.
   - Per-arm token ceiling strictly set to 250,000 tokens (maximum exposure: 500,000 tokens total across both arms).
   - Stop-on-failure rule enforced: if candidate crashes, times out, breaches ceiling, exhausts headroom, or yields invalid telemetry, the baseline is skipped.
3. Baseline arm remains completely unassisted (no Engram MCP servers, no Turn-0 injection, no shell guidance).
4. Strict comparative claim suppression: any acceptance failure ($C < 1.0$), invalid telemetry, or ceiling breach automatically invalidates comparative savings claims.
5. **Separate Explicit Authorization:** Live execution requires separate, explicit user authorization. No agent or automated harness process may initiate live execution autonomously.
