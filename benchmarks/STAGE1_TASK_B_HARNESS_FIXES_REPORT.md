# Engram — Stage 1 Task B Harness Fixes Implementation & Verification Report

**Document ID:** `ENGRAM-STAGE1-TASK-B-HARNESS-FIXES-REPORT`  
**Date:** 2026-10-28  
**Scope:** Remediation of Offline Harness Blockers for Stage 1 Task B (`task_b_remember_me`)  
**Parent Specifications:** `benchmarks/cost-baseline/PREREGISTRATION.md`, `AMENDMENT_02_STAGE1_CORE_TRIAD.md`, `STAGE1_TASK_A_POST_MORTEM_AND_TASK_B_READINESS.md`  
**Final Decision:** **`READY FOR EXPLICIT AUTHORIZATION`**  

---

## 1. Executive Summary & Verdict

In accordance with the findings of `STAGE1_TASK_A_POST_MORTEM_AND_TASK_B_READINESS.md`, the four approved offline harness remediations have been implemented and verified through a dedicated regression test suite and the complete test harness.

All four fixes operate offline without making any live model or API calls, without consuming quota, without weakening preregistered acceptance criteria, and without modifying `benchmarks/cost-baseline/` or `benchmarks/profile/`.

### Summary of Fix Outcomes:

| Fix ID | Subsystem | Approved Remediation | Verification Status | Test Result |
| :---: | :--- | :--- | :---: | :---: |
| **FIX-1** | `runner.py` / `evaluator.py` | Wire `engram inject` into Arm 3 (`v154_arm3`) so Turn-0 context and contract directives are injected into workspace instructions and prepended to the initial model request. Update evaluator to ignore pre-injected infrastructure files. | **VERIFIED** | 4 focused tests pass; Turn-0 context confirmed present. |
| **FIX-2** | Task B Config / Prompt | Add balanced, non-coercive tool awareness guidance to Task B prompt (`task_b_remember_me.json`) so agents discover MCP tools for repository exploration without forcing unnecessary tool calls. | **VERIFIED** | Prompt verified non-coercive; task goals strictly identical. |
| **FIX-3** | `runner.py` (`sanitize_path`) | Ensure `rg` (`ripgrep`) is discoverable in the child sandbox environment PATH across all arms while preserving strict sanitization against Engram directory discovery. | **VERIFIED** | `rg` 15.0.0 resolves and executes; zero Engram path leaks. |
| **FIX-4** | `telemetry.py` | Update event stream parser and session rollout parser to handle both `mcp_tool_call`, `McpToolCall`, and `custom_tool_call` event shapes with deduplication and payload byte accounting. | **VERIFIED** | 5 focused tests pass; rollout, handshake, and synthetic fixtures match. |

**Comprehensive Regression Suite:** **102 / 102 tests passing** (38.26s execution time, 0 failures, 0 errors).

---

## 2. Files Changed and Engineering Rationale

### 2.1 `benchmarks/matched-pair-harness/src/runner.py`
- **Rationale:**
  1. *Turn-0 Injection (FIX-1):* In Arm 3 (`v154_arm3`), the runner now executes `engram.exe inject -C <sandbox> "<prompt>"` prior to launching Codex CLI. It extracts the generated `.github/copilot-instructions.md` containing active contract directives and baseline commit, and prepends this Turn-0 block to `effective_prompt`.
  2. *Sandbox PATH Ripgrep Hygiene (FIX-3):* In `sanitize_path()`, after stripping any directories containing `engram`, the runner checks if `rg` is discoverable via `shutil.which`. If not discoverable, it resolves `rg.exe` from known system candidate paths (e.g. Visual Studio / Copilot CLI) and appends its directory to `PATH`. This prevents agents across all arms from wasting turns on shell fallbacks.
  3. *Result Tracking:* `RunExecutionResult` now records `turn0_injected: bool` and `initial_prompt: Optional[str]`. `_write_dry_run_events` logs the initial prompt in dry-run mode.

### 2.2 `benchmarks/matched-pair-harness/src/telemetry.py`
- **Rationale (FIX-4):**
  1. *Event Stream (`parse_events`):* Updated item completion matcher from strict `type == "mcp_tool_call"` to `norm_type in ("mcp_tool_call", "mcptoolcall", "custom_tool_call")`. Extracts arguments/input payload bytes and result/output payload bytes with deduplication by ID.
  2. *Session Rollout (`parse_rollout_file`):* Added parsing for `event_msg` wrappers (which emit `item: {"type": "McpToolCall", ...}`) and `response_item` custom tool calls (e.g. `name: "engram_..."`). This resolves the defect where `real_session_rollout.jsonl` recorded 0 MCP calls despite containing `engram_doctor`.

### 2.3 `benchmarks/matched-pair-harness/src/evaluator.py`
- **Rationale (FIX-1 Defense-in-Depth):**
  In `analyze_git_impact()`, pre-injected infrastructure files (`.engram/` and `.github/copilot-instructions.md`) created during Turn-0 initialization are explicitly filtered out from `changed_files` and `untracked_files`. This ensures that legitimate Turn-0 injection does not register as unauthorized file sprawl under `REQ-4`.

### 2.4 `benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json`
- **Rationale (FIX-2):**
  Updated the task prompt to provide permissive, balanced tool awareness:
  > *"Add a remember-me option to the authentication flow that persists the username across reloads. You may use any available tools (including registered Engram MCP tools if enabled, or standard search commands) appropriate to your environment. CRITICAL SAFETY INSTRUCTION: Do NOT run npm install or modify package.json or node_modules under any circumstances."*
  This ensures agents are aware of available architectural discovery tools without forcing artificial tool invocations or modifying acceptance criteria.

### 2.5 `benchmarks/matched-pair-harness/tests/test_task_b_readiness_fixes.py`
- **Rationale:**
  Dedicated test suite containing 12 unit tests verifying each fix independently in offline mode.

---

## 3. Evidence for Each Approved Fix

### 3.1 FIX-1 — Turn-0 Context Injection
1. **Deterministic Dry-Run Verification:**
   When `CodexRunner.run` is invoked with `arm="v154_arm3"` in dry-run mode, `exec_res.turn0_injected` evaluates to `True`, and `exec_res.initial_prompt` contains:
   ```markdown
   <!-- BEGIN ENGRAM MANAGED BLOCK -->
   ## Engram Turn-0 Task Intelligence & Directives
   - **Task:** Add a remember-me option to the authentication flow
   - **Active Change Contract:** `ctr_...` (v1, Status: ReadyForImplementation)
   - **Baseline Revision:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`
   ### 🚨 Mandatory AI Agent Directives
   1. **ZERO-TURN EXECUTION:** Change Contract is ALREADY LOCKED. DO NOT CALL engram_get_context.
   2. **POSTFLIGHT AUDIT:** Call engram_postflight when complete.
   ```
2. **Arm Isolation Parity:**
   In `test_fix1_turn0_injection_omitted_for_baseline_and_v151`, Arm 1 (Baseline) and Arm 2 (v1.5.1) evaluate `turn0_injected` as `False`, and their initial prompt receives only the base prompt without injection.
3. **Frozen Binary Execution:**
   In `test_fix1_engram_binary_inject_produces_contract_directives`, the frozen `engram.exe v1.5.4` was executed against an isolated git repository. It cleanly generated `.github/copilot-instructions.md` containing the full Change Contract directives with exit code 0.
4. **Evaluator Containment:**
   In `test_fix1_evaluator_ignores_injected_turn0_files`, `analyze_git_impact` verified that pre-injected `.github/copilot-instructions.md` and `.engram/` are excluded from `changed_files`, yielding `unrelated_files_count == 0`.

### 3.2 FIX-2 — MCP Discovery Guidance
1. **Permissive Non-Coercive Language:**
   `test_fix2_task_b_prompt_discovery_guidance` verified that the prompt uses permissive wording (*"You may use any available tools (including registered Engram MCP tools if enabled, or standard search commands) appropriate to your environment"*).
2. **Absence of Coercion:**
   Verified that strings such as `"you must call engram"`, `"mandatory: call engram"`, or `"required to use engram"` are absent from the prompt.
3. **Requirement Equivalence:**
   The functional feature requirement (*"Add a remember-me option to the authentication flow that persists the username across reloads"*) and safety boundaries remain identical across all arms.

### 3.3 FIX-3 — Sandbox PATH Ripgrep Discoverability
1. **System Resolution:**
   `shutil.which("rg", path=sanitized_env["PATH"])` resolves to:
   `C:\Program Files\Microsoft Visual Studio\18\Community\Common7\IDE\CopilotCli\rg.EXE`
2. **Execution Check:**
   `rg --version` executes cleanly inside the sanitized environment:
   ```
   ripgrep 15.0.0 (rev 3a612f88b8)
   features:+pcre2
   simd(compile):+SSE2,-SSSE3,-AVX2
   simd(runtime):+SSE2,+SSSE3,+AVX2
   ```
3. **Engram Isolation:**
   Verified across 100% of PATH directory entries that the substring `"engram"` is strictly absent.

### 3.4 FIX-4 — Telemetry Parsing for Both Event Shapes
1. **`mcp_tool_call` Shape:**
   Parsed standard item events (`{"type": "item.completed", "item": {"type": "mcp_tool_call", ...}}`). Verified `mcp_tool_calls_count == 1` and payload byte extraction.
2. **`custom_tool_call` Shape:**
   Parsed custom tool call items (`{"type": "item.completed", "item": {"type": "custom_tool_call", "name": "engram_doctor", ...}}`). Verified `mcp_tool_calls_count == 1` and payload byte extraction.
3. **Real Session Rollout Reconciliation:**
   On `real_session_rollout.jsonl` (which previously recorded 0 MCP calls due to `event_msg` encapsulation), the parser now extracts:
   - `mcp_tool_calls_count: 1` (`engram_doctor`)
   - `auxiliary_tool_payload_bytes: 421`
   - `prompt_tokens_total: 47,541`
   - `completion_tokens: 83`
   - `peak_single_request_context: 16,022`
4. **Regression Parity:**
   `real_handshake.jsonl` continues to parse `mcp_tool_calls_count: 1` (431 bytes). `synthetic_multiturn.jsonl` continues to parse `mcp_tool_calls_count: 2` (386 bytes). Duplicate tool IDs are strictly deduplicated.

---

## 4. Test Execution Summary

```
----------------------------------------------------------------------
Ran 12 tests in 3.015s (test_task_b_readiness_fixes.py)
OK

Ran 14 tests in 0.287s (test_task_b_evaluator.py)
OK

Ran 102 tests in 38.255s (complete harness test suite)
OK
----------------------------------------------------------------------
```

All 102 unit tests in the harness pass cleanly with 0 failures, 0 errors, and 0 warnings.

---

## 5. Remaining Risks and Guardrails

1. **Model Non-Determinism on Multi-File Tasks:**
   - *Risk:* On Task B, models might attempt diverse file paths or make partial edits across multiple turns.
   - *Mitigation:* The Task B evaluator enforces 4 independent criteria (`REQ-1` UI, `REQ-2` logic, `REQ-3` secret exclusion, `REQ-4` boundary check). If any requirement is missed ($C < 1.0$), token savings claims are strictly suppressed.
2. **Token Quota Exposure:**
   - *Risk:* 15 executions of a multi-file task consume more quota than Task A.
   - *Mitigation:* As established in the protocol, Block 06 (1 block $\times$ 3 arms = 3 runs) should be run as an initial controlled smoke test before authorizing Blocks 07–10.
3. **Process Tree Termination:**
   - *Risk:* Headless Codex CLI or build tasks could leave background Node/PowerShell processes.
   - *Mitigation:* `isolation.py` tracks exact child PIDs and executes `taskkill /F /T /PID` on timeout or failure.

---

## 6. Final Decision & Recommendation

### **Final Decision: `READY FOR EXPLICIT AUTHORIZATION`**

- **Justification:** All four approved offline harness remediations have been implemented, verified, and regression-tested. The harness is now fully prepared to run Task B arms with genuine Turn-0 injection for Arm 3, non-coercive tool discovery for Arm 2, `rg` in the sandbox PATH for all arms, and complete telemetry capture for all tool-call shapes.
- **Execution Boundary Preserved:** **Zero live model runs have been made.** No Task B executions will take place without separate, explicit user authorization.
