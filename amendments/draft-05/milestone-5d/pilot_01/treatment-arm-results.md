# Treatment Arm Results: Candidate Phase 1 (`treatment_B`)

**Run ID:** `task_commit_cache_invalidation_pilot_01_treatment_B`  
**Arm Identity:** `treatment_B` (Engram Assisted)  
**Binary Path:** `target/release/engram.exe`  
**Binary SHA-256:** `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` (Candidate Phase 1)  
**Model:** `gpt-5.6-luna` (Reasoning Effort: `medium`)  
**Execution Timestamp:** 2026-10-30T10:42:13.540Z to 2026-10-30T10:43:24.657Z  
**Wall-Clock Duration:** 71.12 seconds  
**Execution Status:** `PRE_DISPATCH_HEADROOM_EXHAUSTED` (Preventive Guard Halt)  

---

## 1. Provider-Reported Telemetry

All token values are derived directly from the authoritative provider `SESSION_ROLLOUT` record parsed by `TelemetryParser`:

| Telemetry Metric | Measured Value | Accounting Notes |
| :--- | :--- | :--- |
| **Total Model Tokens** | **117,135** | `prompt_tokens_total + completion_tokens` |
| **Prompt Tokens (Total)** | **115,992** | Sum of input tokens across all 4 requests |
| **Prompt Tokens (Cached)** | **85,888** | 74.05% cache hit rate (informative sub-metric) |
| **Prompt Tokens (Uncached)** | **30,104** | Directly billed input tokens |
| **Completion Tokens** | **1,143** | Model generation tokens |
| **Reasoning Tokens** | **500** | Model chain-of-thought (subset of completion; not double-counted) |
| **Peak Turn Context** | **41,011 tokens** | Maximum input context in a single request |
| **Final Turn Context** | **41,011 tokens** | Context size at turn 4 |
| **Turns Completed** | **4** | 4 interaction turns |
| **Model Requests** | **4** | 4 distinct model invocations |
| **Telemetry Validity** | **VALID (`True`)** | All required fields present, non-negative, and monotonic |

> [!NOTE]
> In accordance with experimental preregistration, cached input tokens (85,888) and uncached input tokens (30,104) sum exactly to total prompt tokens (115,992). Reasoning tokens (500) are contained within completion tokens (1,143) and are not double-counted.

---

## 2. Headroom Guard Intervention

At Turn 4, the streaming token tracker recorded:
- Cumulative token usage: `117,135` tokens.
- Nominal token ceiling: `175,000` tokens.
- Remaining budget: `175,000 - 117,135 = 57,865` tokens.
- Last turn request context: `115,992` tokens.
- Conservative reserve parameter: `45,000` tokens.
- Estimated next turn cost: `max(115,992, 45,000) = 115,992` tokens.

```
Remaining Budget (57,865) < Estimated Turn Cost (115,992)
Guard Action: PREVENTIVE_HALT
Result Status: PRE_DISPATCH_HEADROOM_EXHAUSTED
```

The process tree for `codex.exe` was immediately terminated via `terminate_pid_tree()` at timestamp `2026-10-30T10:43:24.651Z`.

### Guard Forensic Evaluation
1. **Ceiling Protection:** The pre-dispatch guard prevented an estimated 115,992-token dispatch that would have propelled total usage to ~233,127 tokens (a 33.2% overrun of the 175,000 ceiling).
2. **Deterministic Enforcement:** Process termination occurred cleanly without child orphan processes or hung threads.
3. **Limitation Disclosed:** As established in Milestone 5B-Review, host-side process termination guarantees that local token accumulation halts, but does not provide cryptographic proof of remote API cancellation if an in-flight request was already on the wire. Here, termination occurred at the pre-dispatch boundary *before* Turn 5 request dispatch.

---

## 3. Tool Utilization and Agent Behavior Analysis

| Tool Category | Registered Tools | Attempted Calls | Successful Executions | Notes |
| :--- | :--- | :---: | :---: | :--- |
| **Engram MCP** | `engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight` | 0 | 0 | Unused by model |
| **Host Shell** | PowerShell (`powershell.exe`) | 3 | 3 | Used for broad AST text dumps |

### Forensic Analysis of Agent Strategy
Detailed inspection of `events.jsonl` demonstrates that the agent did not leverage the configured Engram MCP server:
1. **Turn 1 (Prompt Intake):** Agent acknowledged task prompt: *"I’ll inspect the existing cache/index lifecycle and CLI wiring, then make only the four authorized file changes..."*
2. **Turn 2 (Broad Grep):** Executed `rg -n "ensure_fresh|index\.db|memories\.json|clean|meta|HEAD|git" src tests -S`. Returned extensive match listings across the entire repository.
3. **Turn 3 (Bulk File Dumps):** Executed a PowerShell script reading the first 220 lines of `src/analyzer.rs`, `src/compiler.rs`, `src/main.rs`, `src/storage.rs`, `src/incremental.rs`, and `Cargo.toml`.
4. **Turn 4 (Slice Dumps):** Executed PowerShell reading lines 560..900 of `src/analyzer.rs` and lines 180..330 of `src/storage.rs`.

**Root Cause of Token Explosion:**  
The agent adopted an unassisted raw file dumping pattern through host PowerShell subcommands rather than invoking targeted Engram retrieval tools (`engram_get_context` or `engram_inspect_symbol`). Consequently, 41,011 tokens of raw source code were pulled into context across 4 turns, exhausting token headroom before any code edits could be authored.

---

## 4. Functional Acceptance Results

Because the agent was halted during its investigative phase before authoring modifications:
- **Files Modified:** 0 (`changes.patch` is empty, 0 bytes).
- **L0 Typecheck:** `BLOCKED`
- **L1 Lint:** `SKIPPED`
- **L2 Unit Tests:** `BLOCKED`
- **L3 Build:** `BLOCKED`
- **Requirements (REQ-1..4):** `FAIL`
- **Correctness Score ($C$):** `0.00`
- **Functional Acceptance:** `FAIL`
