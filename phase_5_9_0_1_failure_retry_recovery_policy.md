# Engram Phase 5.9.0.1: Failure, Retry & Recovery Policy

**Phase:** 5.9.0.1  
**Target:** Benchmark Execution Infrastructure (`runner.py`, `orchestrator.py`, `isolation.py`)  
**Scope:** Fail-Closed Policies for Error Modes, Process Crashes, and State Recovery  

---

## 1. Governance Invariants for Failure and Recovery

1. **No Retry May Reset the Token Budget:** Retries (if permitted) must strictly accumulate onto the current arm's consumption counter.
2. **No Failed Request May Disappear:** Every token consumed during an aborted, failed, or timed-out request remains permanently counted against the arm ceiling and global invocation budget.
3. **No Incomplete Arm May be Treated as a Success:** Any arm that terminates before passing acceptance gates is marked with its exact failure status (`FAILED_VERIFICATION`, `TOKEN_CEILING_BREACHED`, `TIMEOUT`, `CRASHED`).
4. **Crash Recovery Requires Fail-Closed Halt:** If the orchestrator crashes midway through execution and actual cumulative consumption cannot be verified from disk telemetry, the entire pentad block must be aborted. Execution may not continue using an assumed balance.

---

## 2. Failure Matrix & Policy Specifications

| Failure Mode | Retry Permitted? | Max Retries | Backoff Policy | Budget Impact | Arm Status | Subsequent Arms? | Block Impact |
|---|---|---:|---|---|---|---|---|
| **HTTP 429 Rate Limiting** | Yes (inside Codex) | 3 | Exponential (2s, 4s, 8s) | Counts against arm ceiling | If unrecovered: `HALTED_RATE_LIMIT` | Yes (`diagnostic_continue`) | Block continues |
| **HTTP 5xx Server Error** | Yes (inside Codex) | 2 | Linear (5s, 10s) | Counts against arm ceiling | If unrecovered: `HALTED_PROVIDER_ERROR` | Yes | Block continues |
| **Connection Timeout** | No | 0 | None (Child killed at 600s) | Retains tokens prior to kill | `TIMEOUT` | Yes | Block continues |
| **Provider Disconnect** | No | 0 | None | Retains parsed tokens | `CRASHED` | Yes | Block continues |
| **Malformed Provider Response**| No | 0 | None | Increments partial records | `TELEMETRY_INVALID` | Yes | Telemetry invalidated |
| **Missing Token Metadata** | No | 0 | None | Telemetry marked INVALID | `TELEMETRY_INVALID` | Yes | Telemetry invalidated |
| **Codex CLI Crash** | No | 0 | None | Retains last snapshot | `CRASHED` | Yes | Block continues |
| **Orchestrator Host Crash** | No | 0 | None | N/A (Run halted) | `ABORTED_CRASH` | No (Run halts) | Block INVALIDATED |
| **Sandbox Prep Failure** | Yes (pre-run) | 1 | Immediate retry | 0 tokens (Pre-flight) | If failed: `SANDBOX_ERROR` | No (Halts run) | Block Aborted |
| **Verification Failure** | No | 0 | None | Full arm tokens counted | `FAILED_VERIFICATION` | Yes | Block continues |
| **Arm Token Budget Exhaustion**| No | 0 | None | Ceiling tokens counted | `TOKEN_CEILING_BREACHED` | Yes | Block continues |
| **Global Budget Exhaustion** | No | 0 | None | Global tokens counted | `SKIPPED_GLOBAL_BUDGET` | No (Remaining skipped) | Block Aborted |
| **Turn Limit Exhaustion** | No | 0 | None | Full turn tokens counted| `HALTED_TURN_BUDGET` | Yes | Block continues |
| **Workspace Stalling (No diff)**| No | 0 | None | Retains parsed tokens | `HALTED_STALL` | Yes | Block continues |

---

## 3. Crash Recovery & Sandbox Rollback Procedure

When an individual arm fails or halts:
1. **Process Tree Termination:** `isolation.py:terminate_pid_tree(proc.pid)` terminates all lingering child processes (`codex.exe`, `rg.exe`, `node.exe`, `engram.exe`).
2. **Telemetry Preservation:** `runner.py` captures the final JSONL event lines and flushes `arm_result.json` to disk before sandbox deletion.
3. **Workspace Purge:** Unless `--preserve-sandboxes` is explicitly flagged, the temporary sandbox directory is wiped using `teardown_sandbox()` to prevent cross-arm state leakage.
4. **Golden Fingerprint Re-Verification:** Before launching the next arm, the orchestrator computes `compute_dependency_fingerprint()` against the source repository to verify that zero files in the golden repository were touched.
