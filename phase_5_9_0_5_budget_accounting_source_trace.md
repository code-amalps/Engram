# Engram Phase 5.9.0.5: Budget Accounting Source-Code Trace

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Phase:** 5.9.0.5 Workstream A  
**Execution Mode:** Strictly Offline, Read-Only Adversarial Audit  
**Target Codebase:** `benchmarks/matched-pair-harness/src/orchestrator.py`, `runner.py`, `telemetry.py`  
**Audit Status:** `FULLY_TRACED_AND_VERIFIED`

---

## 1. Purpose

This document provides a line-by-line source-code trace of every token counter, budget ceiling, and reserve check in the benchmark execution stack. It traces the lifecycle from the PowerShell dynamic coordinator down to the subprocess request submission in `runner.py`, auditing scope, initialization, enforcement, reset behavior, and failure modes.

---

## 2. Exhaustive Counter & Ceiling Inventory

| Counter / Ceiling Name | Implementation Variable / Field | Scope | Initial Value | Increment / Assignment Point | Measurement Source | Enforcement Point | Reset Behavior | Failure Behavior | Persistence |
|---|---|---|---|---|---|---|---|---|---|
| **Aggregate Ceiling** | `$aggregateCeiling` | Cross-Process Global | $2,500,000$ | Fixed constant in coordinator | Operational Protocol | Coordinator Step 3 | Never resets | Fail-closed (`-lt 95000` aborts) | Script variable |
| **Task A Ceiling** | `--global-tokens-ceiling` | Invocation 1 Global | $1,000,000$ | Set via CLI argument | Benchmark config | `orchestrator.py:2749` | Process lifetime | Fail-closed (halts pentad) | CLI parameter |
| **Task B Dynamic Ceiling** | `$taskBCeiling` / `self.global_tokens_ceiling` | Invocation 2 Global | Dynamic: $\min(1.5\text{M}, 2.5\text{M} - T_A)$ | Computed by coordinator after Task A | Summary JSON (`cumulative_global_tokens`) | `orchestrator.py:2749` | Process lifetime | Fail-closed (halts pentad) | Passed via CLI flag |
| **Per-Arm Ceiling** | `self.max_tokens_ceiling` / `effective_arm_ceiling` | Single Arm | $250\text{k}$ (Task A) / $500\text{k}$ (Task B) | `orchestrator.py:2767`: `min(max_tokens_ceiling, global_remaining)` | Static config & gross remaining | `runner.py:449` (`check_pre_dispatch_headroom`) | Re-computed per arm | Fail-closed (`DISPATCH_REFUSED`) | In-memory |
| **Protected Reserve** | `self.headroom_reserve` | Invocation & Arm | $45,000$ | Fixed default ($45,000$) or CLI `--headroom-reserve` | Protocol invariant | `orchestrator.py:2750` & `runner.py:420` | Never resets | Fail-closed (halts invocation / arm) | CLI / In-memory |
| **Invocation Cumulative Tokens** | `cumulative_global_tokens` | Invocation | $0$ | `orchestrator.py:2804`: `+= spent` | Provider streaming telemetry | `orchestrator.py:2749` (before each arm) | Resets to 0 at process start | Fail-closed on missing telemetry | Persisted in `pentad_summary.json` |
| **Arm Cumulative Tokens** | `effective_tokens` (`thread_cumulative_max` / `turn_deltas_sum`) | Single Arm | $0$ | `runner.py:242` / `319` | Provider streaming JSONL events | `runner.py:458` (before each turn) | Resets per arm subprocess | Fail-closed (`telemetry_valid = False`) | In-memory StreamingTokenTracker |
| **Last Turn Request Cost** | `last_turn_request_tokens` | Single Turn | $0$ | `runner.py:224`: parsed from `usage.input_tokens` | Provider usage event | `runner.py:411` (`estimate_next_request_tokens`) | Updated each turn | Conservative fallback to reserve ($45\text{k}$) | In-memory |
| **Retry & Error Tokens** | Accumulated in `turn_deltas_sum` | Single Arm | $0$ | `runner.py:319` | Provider usage events | Deducted from active arm ceiling | Resets per arm | Included in cumulative usage | In-memory |
| **In-Flight Tokens** | Unobserved during HTTP turn | Request Turn | N/A | Synchronous single-turn lock | Not observable pre-response | Blocked until response returns | N/A | Subprocess kill aborts summary | Ephemeral network packet |

---

## 3. Structural Trace: From Coordinator to Request Submission

### Step 1: Coordinator Computation (Cross-Process Entry Point)
In the coordinator script:
```powershell
$tokensSpentA = $taskAData.cumulative_global_tokens
$taskBCeiling = [Math]::Min(1500000, (2500000 - $tokensSpentA))
```
- If Task A consumed $1,000,000$ tokens, `$taskBCeiling = 1,500,000$.
- If Task A overshot to $1,020,000$ tokens, `$taskBCeiling = 1,480,000$.
- The ceiling is passed to Invocation 2 via `--global-tokens-ceiling $taskBCeiling`.

### Step 2: Orchestrator Pentad Loop (`orchestrator.py:2743–2805`)
- When Invocation 2 initializes, `cumulative_global_tokens` starts at **0**.
- Before dispatching Arm $k$ (e.g. Slot 3 / Candidate 5):
  ```python
  global_remaining = self.global_tokens_ceiling - cumulative_global_tokens
  if global_remaining < self.headroom_reserve: # 45,000
      block_status = "ABORTED_GLOBAL_BUDGET"
      break
  effective_arm_ceiling = min(self.max_tokens_ceiling, global_remaining)
  ```
- The arm is invoked with `arm_max_tokens_ceiling = effective_arm_ceiling`.

### Step 3: Runner Pre-Dispatch Turn Gating (`runner.py:414–463`)
Inside `StreamingTokenTracker`:
```python
remaining = ceiling - self.effective_tokens
estimated = self.estimate_next_request_tokens() # max(last_turn_request_tokens, 45_000)
has_headroom = (remaining >= estimated)
```
- Turn $n+1$ is permitted only if remaining arm budget $\ge 45,000$ (or last turn prompt cost).
- If `has_headroom == False`, `StreamingTokenTracker` returns `PREVENTIVE_HALT`, terminating the subprocess loop cleanly.

### Step 4: Post-Response Telemetry Ingestion
- When turn $n$ completes, Codex emits a JSONL line: `token_usage_record` or `turn.completed`.
- `StreamingTokenTracker.process_line()` parses `input_tokens` and `output_tokens`, updating `thread_cumulative_max` or `turn_deltas_sum`.
- When the arm process exits, `orchestrator.py:2804` increments `cumulative_global_tokens += spent`.

---

## 4. Key Accounting Invariants Established

1. **Zero Double-Counting Inside Invocation:** `cumulative_global_tokens` is updated strictly once per arm from authoritative parsed telemetry (`spent = tel_data.get("total_model_tokens")`).
2. **Synchronous Turn Serialization:** Subprocess turn execution in `runner.py` is strictly serial. Turn $n+1$ cannot be submitted while turn $n$ is in-flight.
3. **Post-Hoc Accounting Reality:** Token usage is discovered **strictly after provider generation concludes**. The client has no pre-flight token counter.
4. **Independent Invocation Counters:** Invocation 2 starts its own in-memory `cumulative_global_tokens = 0`. Cross-process budget conservation is enforced entirely by the coordinator clamping `$taskBCeiling`.
