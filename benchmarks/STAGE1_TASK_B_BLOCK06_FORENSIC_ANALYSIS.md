# Engram — Stage 1 Task B Block 06 Forensic Analysis Report

**Investigation Date:** 2026-10-29  
**Investigated Artifacts:**
- Preserved Run Directory: [`benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/)
- Arm 1 Code Patch: [`changes.patch`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/baseline_arm1/changes.patch) (6,190 bytes)
- Arm 1 Event Stream: [`events.jsonl`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/baseline_arm1/events.jsonl) (99,179 bytes)
- Arm 1 Run Telemetry: [`run_telemetry.json`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/baseline_arm1/run_telemetry.json)
- Triad Summary: [`triad_summary.json`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/triad_summary.json)
- Task Definition: [`task_b_remember_me.json`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json)
- Authoritative Native Codex Rollout: `C:\Users\Amal P S\.codex\sessions\2026\10\29\rollout-2026-10-29T11-31-31-01a1863f-cd55-7ea0-a9fd-7c567ddfc3d5.jsonl` (363,643 bytes, 79 events)

---

## Executive Summary

On 2026-10-29, the Stage 1 Core Triad Block 06 live benchmark (`task_b_remember_me`) was executed under explicit authorization. Arm 1 (`baseline_arm1`) was terminated at **Turn 9 (109.26s)** upon reaching **253,748 tokens**, exceeding the **250,000-token per-arm ceiling**. Under the orchestrator's stop-on-failure rule, Arm 2 and Arm 3 were cleanly skipped, and comparative savings claims were strictly suppressed.

This forensic investigation reveals that:
1. **Token Overrun Mechanism:** The token accumulation was driven by **context compounding** across 9 conversational agent turns. Because the unassisted baseline agent lacked architectural indexing, it dumped multiple entire source and configuration files into terminal output. Ingesting these files caused the trajectory context to grow from 13.2k to 37.2k tokens per turn. The sum of cumulative prompt inputs across 9 turns reached 250,026 tokens.
2. **Implementation Quality:** The 6 modified files in `changes.patch` represent **highly relevant, purposeful implementation**, not aimless exploration. The model correctly built the HTML Material login menu, component state, NgRx action payload, and local storage username persistence.
3. **Evaluator 0/4 Origin:** The reported score of $C = 0.00$ (0/4 requirements passed) was caused by **harness short-circuiting**: whenever an arm terminates non-SUCCESS, automated gate and requirement evaluation is bypassed by design to prevent tainted scores. Had static checks been run, the code satisfied REQ-1, REQ-2, REQ-3, and REQ-4. However, it was halted mid-stride before adding `MatCheckboxModule` imports and running typechecks.
4. **Streaming Telemetry Performance:** The ceiling was enforced within **355 milliseconds** of Codex emitting Turn 9's `token_usage_record`. Because model consumption occurs server-side during a turn, Turn 8 was under the ceiling (216,097 tokens), and Turn 9 completed at 253,748 tokens, triggering immediate process termination.

---

## 1. Turn-by-Turn Token Accumulation Breakdown

In conversational agent harnesses, each model turn re-sends the cumulative session trajectory (system instructions, user prompt, agent messages, tool calls, and complete tool stdout). Cumulative token usage is the sum of every individual turn request.

### Telemetry Progression Across All 9 Turns

| Turn | Timestamp (UTC) | Single Call Input | Single Call Output | Reasoning Tokens | Turn Delta | Thread Cumulative Tokens | Key Activity / Tool Invocations |
| :---: | :---: | :---: | :---: | :---: | :---: | :---: | :--- |
| **1** | 06:01:37.905 | 13,259 | 198 | 24 | 13,457 | **13,457** | `Get-ChildItem -Force; rg -n "login\|username\|remember\|auth"` (200 lines returned) |
| **2** | 06:02:05.532 | 21,621 | 207 | 8 | 21,828 | **35,285** | Bulk dump: `auth.effects.ts`, `auth.models.ts`, `auth.actions.ts`, `auth.reducer.ts` |
| **3** | 06:02:11.564 | 22,934 | 150 | 16 | 23,084 | **58,369** | Broad search: `rg -n "authLogin...|password|login" ... -g '*.ts' -g '*.html'` |
| **4** | 06:02:16.268 | 25,247 | 221 | 45 | 25,468 | **83,837** | Bulk dump: `app.component.ts`, `app.component.html`, `local-storage.service.ts`, `auth.effects.spec.ts`, `core.module.ts` |
| **5** | 06:02:23.215 | 29,921 | 211 | 59 | 30,132 | **113,969** | Bulk dump: `git status`, `app.component.spec.ts`, `core.state.ts`, `en.json` (60 lines) |
| **6** | 06:02:32.238 | 31,681 | 349 | 190 | 32,030 | **145,999** | Bulk dump: `shared.module.ts`, `init-state-from-local-storage.reducer.ts`, `tsconfig.app.json`, `tsconfig.spec.json` |
| **7** | 06:02:47.402 | 33,241 | 1,626 | 371 | 34,867 | **180,866** | **Implementation Phase 1:** Applied 6-file patch to auth actions, effects, app component, scss, and i18n |
| **8** | 06:03:12.291 | 34,891 | 340 | 178 | 35,231 | **216,097** | Validation exploration: read `app.component.scss`, `rg MatFormFieldModule`, `auth.actions.spec.ts` |
| **9** | 06:03:19.147 | 37,231 | 420 | 54 | 37,651 | **253,748** | **Implementation Phase 2:** Patched `MatFormFieldModule` into `app.component.ts` & updated `auth.effects.spec.ts` |

### Mathematical Breakdown
$$\text{Total Prompt Tokens} = \sum_{t=1}^{9} \text{Input}_t = 250,026 \text{ tokens}$$
$$\text{Total Completion Tokens} = \sum_{t=1}^{9} \text{Output}_t = 3,722 \text{ tokens}$$
$$\text{Total Model Tokens} = 250,026 + 3,722 = \mathbf{253,748 \text{ tokens}}$$

**Finding:** The model did not generate excessive output (only 3,722 completion tokens across 9 turns, an average of 413 tokens/turn). Instead, **98.5% of all token consumption was prompt token re-ingestion** ($250,026 / 253,748$) caused by large file dumps accumulating in the conversational context.

---

## 2. File Exploration & Content Ingestion Patterns

Without an architectural index tool (such as Engram MCP), the baseline model had no way to inspect symbols or module boundaries without reading entire raw text files into its stdout buffer.

### Repeatedly Read or Explored Files
1. `projects/angular-ngrx-material-starter/src/app/app/app.component.ts`: Read in Turn 4, modified in Turn 7, imports patched in Turn 9.
2. `projects/angular-ngrx-material-starter/src/app/app/app.component.html`: Read in Turn 4, patched in Turn 7.
3. `projects/angular-ngrx-material-starter/src/app/app/app.component.scss`: Modified in Turn 7, re-read in Turn 8.
4. `projects/angular-ngrx-material-starter/src/app/core/auth/auth.effects.ts`: Read in Turn 2, patched in Turn 7.
5. `projects/angular-ngrx-material-starter/src/app/core/auth/auth.actions.ts`: Read in Turn 2, patched in Turn 7.
6. `projects/angular-ngrx-material-starter/src/app/core/auth/auth.effects.spec.ts`: Read in Turn 4, patched in Turn 9.

### Bulk Ingestion of Non-Target Files (Exploration Overhead)
In Turns 2, 4, 5, and 6, the agent executed compound PowerShell commands that dumped full files into stdout:
- `auth.models.ts` & `auth.reducer.ts` (Turn 2)
- `local-storage.service.ts` & `core.module.ts` (Turn 4)
- `app.component.spec.ts` & `core.state.ts` (Turn 5)
- `shared.module.ts`, `init-state-from-local-storage.reducer.ts`, `tsconfig.app.json`, and `tsconfig.spec.json` (Turn 6)
- Workspace-wide regex searches via `rg` (Turns 1, 3, and 8)

**Architectural Significance:** This directly demonstrates the architectural problem Engram was designed to solve. When an agent has no symbol map or dependency graph, it must read whole files sequentially to determine where types and actions are defined, rapidly bloating context from 13k to 37k tokens.

---

## 3. Tool Invocations & Absence of MCP Calls

### Finding: Zero MCP Calls is Correct Baseline Design
`run_telemetry.json` records:
```json
"mcp_tool_calls_count": 0,
"isolation": {
  "engram_enabled": false,
  "mcp_server_status": "DISABLED"
}
```

- **Protocol Invariant:** Arm 1 (`baseline_arm1`) is explicitly preregistered as the **unassisted native baseline**.
- **Harness Implementation:** In [`orchestrator.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L90-L115), `self.arm_binaries["baseline_arm1"] = None`. In [`runner.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L180-L210), when `engram_executable` is `None`, Codex is launched with no MCP server flags.
- **Available Tools:** The agent only had native Codex host tools:
  - `tools.exec_command`: Executed PowerShell scripts (`Get-Content`, `rg`, `git`).
  - `tools.apply_patch`: Applied git diff patches.
- **Conclusion:** `mcp_tool_calls_count = 0` is an intended characteristic of the experimental control, not a telemetry defect or tool failure.

---

## 4. Assessment of Modified Files in `changes.patch`

The git patch in `baseline_arm1/changes.patch` contains 45 insertions and 7 deletions across 6 files:

| File | Changes | Substance of Edit | Assessment |
| :--- | :---: | :--- | :--- |
| `src/app/app/app.component.html` | +15 / -1 | Replaces direct login button with `<button [matMenuTriggerFor]="loginMenu">`. Adds `<mat-menu #loginMenu>` containing a `.login-form` with `mat-form-field`, `input matInput [(ngModel)]="username"`, `mat-checkbox [(ngModel)]="rememberMe"`, and submit button. | **Highly Relevant** |
| `src/app/app/app.component.scss` | +7 / -0 | Adds `.login-form` flex column styling with padding and min-width. | **Highly Relevant** |
| `src/app/app/app.component.ts` | +7 / -2 | Adds `username = ''` and `rememberMe = false`. Restores `this.username` from `storageService.getItem('AUTH_USERNAME')` in `ngOnInit`. Dispatches `authLogin({ username, rememberMe })`. Adds `MatFormFieldModule` to imports. | **Highly Relevant** |
| `src/app/core/auth/auth.actions.ts` | +5 / -2 | Updates `authLogin` action to accept `props<{ username?: string; rememberMe?: boolean }>()`. | **Highly Relevant** |
| `src/app/core/auth/auth.effects.ts` | +8 / -3 | Defines `AUTH_USERNAME_KEY = 'AUTH_USERNAME'`. In `login$` effect, taps action payload; if `rememberMe && username`, saves to storage, else removes key. | **Highly Relevant** |
| `src/assets/i18n/en.json` | +2 / -0 | Adds translation strings `"anms.auth.username": "Username"` and `"anms.auth.remember-me": "Remember me"`. | **Highly Relevant** |

### Evaluation Verdict
The modifications represent **relevant, focused, and purposeful implementation**, not aimless or degenerative thrashing. The code directly addressed every functional element of Task B. It was not finished because the model was cut off before adding `MatCheckboxModule` to `app.component.ts` and before running compilation verification.

---

## 5. Why the Evaluator Reported 0/4 Requirements ($C = 0.00$)

In `run_telemetry.json` and `triad_report.md`, correctness is reported as:
```json
"correctness": {
  "score": 0.0,
  "passed_all_requirements": false,
  "gates": {
    "L0_typecheck": "BLOCKED",
    "L1_lint_prettier": "SKIPPED",
    "L2_unit_tests": "BLOCKED",
    "L3_production_build": "BLOCKED"
  },
  "requirements": {
    "REQ-1": "FAIL",
    "REQ-2": "FAIL",
    "REQ-3": "FAIL",
    "REQ-4": "FAIL"
  }
}
```

### Root Cause: Harness Scientific Short-Circuit
In [`orchestrator.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L860-L880):
```python
if execution_status != "SUCCESS":
    gate_results = {
        "L0_typecheck": "BLOCKED",
        "L1_lint_prettier": "SKIPPED",
        "L2_unit_tests": "BLOCKED",
        "L3_production_build": "BLOCKED"
    }
    req_results = {req["id"]: "FAIL" for req in self.task_config.get("requirements", [])}
    correctness = CorrectnessResult(
        score=0.0,
        ...
    )
```
When `execution_status == "TOKEN_CEILING_BREACHED"`, the orchestrator **intentionally suppresses running automated gates and static evaluators**. This prevents an aborted or timed-out run from falsely claiming partial correctness.

### Substantive Code Verification vs Premature Termination
When the evaluator's static evaluation functions are run directly against the patch content:
- **REQ-1 (UI Layout):** `has_checkbox` and `has_username` in `app.component.html` $\rightarrow$ **PASS**.
- **REQ-2 (Persistence Restoration):** Restores username via `getItem('AUTH_USERNAME')` $\rightarrow$ **PASS**.
- **REQ-3 (Security Negative Check):** No passwords, tokens, or JWTs stored in `localStorage` $\rightarrow$ **PASS**.
- **REQ-4 (Boundary Blast Radius):** 0 unrelated files modified; only 1 i18n file touched $\rightarrow$ **PASS**.

**Why the run would still not have achieved $C = 1.0$:**
Full acceptance requires all automated compiler gates to pass. In `app.component.html`, the model used `<mat-checkbox>`. In Turn 9, it added `MatFormFieldModule` to `AppComponent` imports, but had not yet imported `MatCheckboxModule`. Running `npx tsc` at Turn 9 would have flagged this missing import. The agent had not yet run `npx tsc` when the ceiling terminated execution.

**Conclusion:** The 0/4 result reflects **premature termination due to token ceiling breach**, which halted a substantively valid implementation immediately prior to syntax verification.

---

## 6. Token Ceiling Enforcement & Telemetry Arrival Timing

### Timeline Analysis
- **Token Ceiling Threshold:** 250,000 tokens
- **End of Turn 8 (06:03:12.291 UTC):**
  - Thread cumulative tokens: **216,097 tokens**
  - Headroom remaining: **33,903 tokens**
  - State: **UNDER CEILING (Legal to continue)**
- **Turn 9 Generation (06:03:12 – 06:03:19 UTC):**
  - Context sent to model: 37,231 tokens
  - Model generation & reasoning: 420 tokens
  - Turn 9 tokens consumed: **37,651 tokens**
- **Turn 9 Completion (06:03:19.147 UTC):**
  - Codex emitted `token_usage_record`: cumulative total = **253,748 tokens**
  - Ceiling breached: $253,748 > 250,000$ (by 3,748 tokens)
- **Harness Termination (06:03:19.502 UTC):**
  - `StreamingTokenTracker` detected the line and set `token_ceiling_breached = True`.
  - Harness invoked `terminate_pid_tree(proc.pid)` (`taskkill /F /T /PID`).
  - Total latency from event emission to process termination: **355 milliseconds**.

### Root Cause vs Streaming Characteristic
- Model inference occurs remotely on OpenAI's infrastructure. Codex cannot report token counts mid-generation; usage counts are flushed only upon completion of a turn or streaming chunk.
- Because Turn 8 was at 216k tokens and Turn 9 required a 37k prompt context, Turn 9 was guaranteed to push the cumulative count over 250k.
- The harness acted instantly (within 355ms) upon receipt of Turn 9's telemetry line.
- **Finding:** The token ceiling was exceeded by 3,748 tokens because telemetry arrives at turn boundaries, but the runner responded immediately, terminating the process tree and preventing any subsequent turns.

---

## 7. Comparative Assessment: Pre-Repair vs Post-Repair

| Metric / Behavior | Block 06 Initial Run (2026-10-28) | Block 06 Post-Repair Run (2026-10-29) | Net Change |
| :--- | :---: | :---: | :---: |
| **Elapsed Duration** | 600.00 s (Timeout) | **109.26 s** | **-490.74 s (-81.8%)** |
| **Cumulative Tokens** | 1,254,712 tokens | **253,748 tokens** | **-1,000,964 tokens (-79.8%)** |
| **Turns Executed** | 27 turns | **9 turns** | **-18 turns (-66.7%)** |
| **Termination Method** | Unguarded subprocess timeout | Real-time `terminate_pid_tree` | Clean Windows PID tree termination |
| **Telemetry State** | Crashed (`FileNotFoundError`) | Safely recovered from rollout | Schema-valid record created |
| **Subsequent Arms** | Unhandled exception crash | Skipped via `stop-on-failure` | Structured skipped records generated |
| **Quota Waste** | 1.25M tokens consumed | Stopped at ~250k ceiling | **1.0M tokens quota saved** |

---

## 8. Root Causes vs Hypotheses

| Category | Finding | Classification | Evidence |
| :--- | :--- | :---: | :--- |
| **Token Explosion** | Ingesting raw source files into conversation history swelled prompt context from 13.2k to 37.2k tokens, causing $\sum \text{Prompt} = 250,026$. | **PROVEN ROOT CAUSE** | Exact token logs across Turns 1–9 from native rollout. |
| **Zero MCP Calls** | Baseline arm design explicitly sets `engram_enabled: false` and registers zero MCP servers. | **PROVEN ROOT CAUSE** | Preregistration schedule, `orchestrator.py`, and `runner.py`. |
| **Evaluator 0/4** | Non-SUCCESS exit status (`TOKEN_CEILING_BREACHED`) triggers automatic assignment of 0/4 and BLOCKED gates in orchestrator. | **PROVEN ROOT CAUSE** | [`orchestrator.py` lines 860-880](file:///C:/Users/Amal P S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L860-L880). |
| **Code Viability** | Code in `changes.patch` satisfied REQ-1 through REQ-4 statically, but was missing `MatCheckboxModule` import and compilation verification. | **PROVEN FACT** | Code inspection of `changes.patch` and static criteria check. |
| **Ceiling Timing** | Turn 8 was at 216k; Turn 9's 37.6k turn pushed total to 253.7k; harness terminated within 355ms of line receipt. | **PROVEN FACT** | Timestamps in rollout (`06:03:19.147Z`) vs telemetry (`06:03:19.502Z`). |
| **Engram Efficiency** | Turn-0 injection and MCP tools in Arm 3 will prevent the multi-file exploration that bloated Arm 1's context. | **STRONG HYPOTHESIS** | Supported by Task A evidence where Engram reduced context by 65–85%. |

---

## 9. Minimal Corrective Actions & Next Steps

All proposed actions strictly preserve the frozen task definition, preregistration seals, frozen binaries, and historical evidence:

### Option A: Execute Latin-Square Block where Treatment Runs First (`block_08` or `block_07`) — RECOMMENDED
- **Mechanism:** In `TRIAD_BLOCK_SCHEDULES`:
  - `block_08`: Order is `["v154_arm3", "baseline_arm1", "v151_arm2"]`.
  - `block_07`: Order is `["v151_arm2", "v154_arm3", "baseline_arm1"]`.
- **Advantage:** In `block_08`, Arm 3 (`v154_arm3` with Turn-0 injection) executes **first**.
- **Scientific Value:** Testing Arm 3 first will determine whether Engram enables the model to complete `task_b_remember_me` within a small token budget (e.g. <50k tokens) by bypassing the 6-turn exploratory file-dump cycle. If Arm 3 succeeds with low tokens, its execution is completely unblocked by baseline's token ceiling behavior.
- **Constraints Preserved:** Zero code modifications, zero task changes, zero schedule edits.

### Option B: Execute Block 06 with Non-Blocking Triad Option (`--no-stop-on-failure`)
- **Mechanism:** The orchestrator CLI supports `--no-stop-on-failure`.
- **Advantage:** Allows Arm 2 (`v151_arm2`) and Arm 3 (`v154_arm3`) to execute even if Arm 1 halts on ceiling breach.
- **Scientific Value:** Collects live telemetry for all three arms on Block 06 seed (`2026102806`), providing direct comparative data.

### Option C: Formal Protocol Amendment for Task B Baseline Token Ceiling (If Comparative Savings Delta is Required)
- **Mechanism:** Draft Amendment 03 to adjust `max_tokens_ceiling` for Task B (e.g., from 250,000 to 350,000 tokens).
- **Context:** Task A was a single-line typo fix (~28k baseline tokens). Task B is a multi-file architectural change (UI, state, effects, storage, tests). An unassisted agent requires ~10–12 turns to explore and verify, which at ~30k tokens/turn requires ~350k cumulative tokens.
- **Note:** This requires explicit user approval and a formal amendment before execution.

---

*Forensic investigation complete. Artifact preserved. Awaiting explicit authorization for next steps.*
