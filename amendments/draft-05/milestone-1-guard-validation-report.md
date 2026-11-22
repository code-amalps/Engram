# Engram Amendment 05 — Milestone 1: Offline Token Guard Validation Report

## Executive Summary

This report documents the completion of **Milestone 1: Offline Token Guard Validation** under Engram Amendment 05. The objective of this milestone is to validate and harden the token-guard remediation offline, establishing rigorous deterministic test coverage across token accounting, headroom calculation, event-boundary detection, process termination, stop classification, and telemetry serialization.

All testing and engineering were conducted **strictly offline** with zero model inference, zero API calls, zero network activity, and zero inference spend (\$0.00). Frozen binaries (`v1.5.1`, `v1.5.4`), the candidate binary (`candidate-phase1/engram.exe`), Amendment 04, historical reports (Blocks 06, 07, 08), and the golden repository were preserved without modification.

---

## 1. Implementations Inspected

Prior to validation, the existing harness codebase was thoroughly inspected:

1. **`StreamingTokenTracker` (`benchmarks/matched-pair-harness/src/runner.py`, lines 54–240)**:
   - Tracks streaming token usage by parsing stdout lines from Codex.
   - Distinguishes cumulative thread snapshots (`token_usage_record`, `event_msg token_count`) from incremental turn deltas (`turn.completed`).
   - Reconciles cumulative usage via `max(thread_cumulative_max, turn_deltas_sum)`.
   - Maintains `last_turn_request_tokens`, `conservative_reserve`, and computes pre-dispatch headroom.

2. **`is_pre_dispatch_boundary()` & Boundary Helpers (`benchmarks/matched-pair-harness/src/runner.py`, lines 400–475)**:
   - Analyzes JSONL events to detect potential pre-dispatch boundaries.
   - Evaluates whether events indicate a safe pause point prior to the next model inference call.
   - Explicitly models the distinction between candidate external poller triggers (`item.started`, `turn.started`, tool completion) and guaranteed pre-dispatch boundaries.

3. **`terminate_pid_tree()` (`benchmarks/matched-pair-harness/src/isolation.py`, lines 340–395)**:
   - Process tree termination mechanism for Windows and POSIX.
   - On Windows, executes `taskkill /F /T /PID <pid>` followed by process verification and fallback direct termination.

4. **External Polling Loop (`benchmarks/matched-pair-harness/src/runner.py`, lines 600–710)**:
   - Spawns Codex as a subprocess with unbuffered stdout piping.
   - Polls stdout in real-time with a 50 ms sleep interval (`poll_interval_seconds = 0.05`).
   - Invokes `token_tracker.process_line()`, evaluates `check_pre_dispatch_headroom()`, and triggers `terminate_pid_tree()` if limits are breached.

5. **`RunExecutionResult` & Telemetry Schema (`benchmarks/matched-pair-harness/src/runner.py`, lines 30–50; `orchestrator.py`, lines 450–520; `run_telemetry.schema.json`)**:
   - Encapsulates execution results, stop reasons, and the full `headroom_guard` metadata object.

---

## 2. Files Modified

Three files in the test harness were modified to implement required hardenings and schema compliance:

| File Path | Nature of Changes | Rationale & Diff Summary |
|:---|:---|:---|
| `benchmarks/matched-pair-harness/src/runner.py` | Enhanced `StreamingTokenTracker`, added sub-metrics, snapshot deduplication, boundary evaluation, schema fields to `RunExecutionResult` | Added tracking for `prompt_tokens_total`, `prompt_tokens_cached`, `prompt_tokens_uncached`, `completion_tokens`, and `reasoning_tokens`. Invariant: reasoning tokens are an informative sub-metric of completion tokens and are **never double-counted**. Added snapshot deduplication hash set. Added fail-safe fallback logic on invalid telemetry (`telemetry_valid=False`, fail-safe halt). Hardened equality boundary check (`remaining == estimated -> True`). |
| `benchmarks/matched-pair-harness/src/orchestrator.py` | Updated execution status resolution and `headroom_guard` serialization | Mapped runner stop reasons (`PRE_DISPATCH_HEADROOM_EXHAUSTED`, `TOKEN_CEILING_BREACHED`, `TELEMETRY_INVALID`, `TIMEOUT`, `PROCESS_CRASH`) to authoritative telemetry statuses. Serialized all 12 `headroom_guard` properties. |
| `benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json` | Updated `headroom_guard` property schema | Added `configured_ceiling`, `remaining_budget`, `last_turn_usage`, `guard_decision`, `timestamp`, `relevant_event_id`, and `telemetry_valid` to schema definitions to ensure strict schema validation passes. |

In addition, one new comprehensive test file was created:
- `benchmarks/matched-pair-harness/tests/test_token_guard_validation.py` (37 deterministic offline unit tests).

---

## 3. Design Decisions & Architectural Invariants

### 3.1 Sub-Metric Accounting & Reasoning Token Math
The OpenAI and Codex streaming protocol delivers tokens structured as:
$$\text{total\_tokens} = \text{input\_tokens} + \text{output\_tokens}$$
Where `output_tokens` includes `reasoning_output_tokens` (thinking tokens).
- **Rule Adopted**: `self.completion_tokens = output_tokens`, and `self.reasoning_tokens = reasoning_output_tokens`.
- **Invariant**: Reasoning tokens are treated as an informative sub-metric of output tokens and are **never added to total tokens**. Adding reasoning tokens to total tokens constitutes illegal double-counting.
- **Uncached Input Calculation**: `prompt_tokens_uncached = max(0, prompt_tokens_total - prompt_tokens_cached)`.

### 3.2 Snapshot Deduplication
In streaming stdout pipelines, identical `token_usage_record` or `event_msg token_count` lines may be emitted repeatedly or buffered across chunk reads.
- **Mechanism**: A hash set `_seen_snapshot_hashes` stores MD5 hashes of snapshot JSON payloads. Duplicate snapshots are tracked but skipped from re-updating `turns_completed` or skewing turn estimates.

### 3.3 Fail-Safe Guard Logic on Invalid Telemetry
If telemetry contains negative values, non-monotonic drops in cumulative tokens, unparseable payloads, or missing critical fields:
- `self.telemetry_valid = False`
- `self.telemetry_status = "INVALID"`
- `check_pre_dispatch_headroom()` immediately halts dispatch (`has_headroom = False`) with decision `"HALT_TELEMETRY_INVALID"`.
- If ceiling or reserve is `None` or $\le 0$, the guard fails safe and denies dispatch.

### 3.4 Boundary Condition & Equality Handling
When evaluating headroom:
$$\text{remaining\_budget} = \text{ceiling} - \text{current\_cumulative\_tokens}$$
$$\text{estimated\_next\_cost} = \max(\text{last\_turn\_request\_tokens}, \text{conservative\_reserve})$$
- **Equality Rule**: When `remaining_budget == estimated_next_cost`, dispatch is **allowed** (`has_headroom = True`, decision `"DISPATCH_ALLOWED"`).
- **Halt Rule**: When `remaining_budget < estimated_next_cost` (e.g., remaining is even 1 token lower), dispatch is **prevented** (`has_headroom = False`, decision `"HALT_PREVENTIVE"`).

### 3.5 Process Termination Sequence
Process tree killing on Windows requires terminating not only the spawned shell/python process but all descendant processes (`codex.exe`, child compilers, git commands):
- Step 1: Execute `taskkill /F /T /PID <pid>` with a 3.0-second timeout.
- Step 2: Poll `psutil` or Windows process list to confirm termination.
- Step 3: If any descendant PID remains active, execute direct `os.kill(pid, signal.SIGKILL)` or `proc.kill()`.
- Step 4: Verify that no orphaned child processes remain.

---

## 4. Test Coverage Summary

A new dedicated test suite `test_token_guard_validation.py` was authored, containing 37 tests across Sections 4.A through 4.F:

1. **Section 4.A: Token Accounting (7 tests)**:
   - Authoritative `token_usage_record` parsing (prompt cached/uncached, completion, reasoning).
   - Authoritative `event_msg` `token_count` parsing.
   - Incremental `turn.completed` delta parsing.
   - Non-double-counting invariant for reasoning tokens.
   - Monotonic reconciliation when thread snapshots exceed turn deltas.
   - Snapshot deduplication under duplicate emissions.
   - Malformed, negative, and invalid JSON fail-safe handling.

2. **Section 4.B: Headroom Calculation (10 tests)**:
   - Case 1: Initial state before any turns ($0 \to \text{estimate} = 15,000$).
   - Case 2: Turn 1 cost below reserve ($5,000 \to \text{estimate} = 15,000$).
   - Case 3: Turn 1 cost above reserve ($22,000 \to \text{estimate} = 22,000$).
   - Case 4: Near ceiling with adequate headroom.
   - Case 5: Near ceiling with inadequate headroom ($< \text{reserve}$).
   - Case 6: Remaining budget exactly equals estimated turn cost (equality boundary $\to$ allowed).
   - Case 7: Remaining budget is 1 token less than estimated turn cost ($\to$ halt).
   - Case 8: Negative remaining budget (cumulative exceeds ceiling $\to$ halt).
   - Case 9: Zero or negative reserve configuration ($\to$ fail-safe halt).
   - Case 10: Missing or invalid ceiling ($\to$ fail-safe halt).

3. **Section 4.C: Event-Boundary Detection (5 tests)**:
   - Tool start (`item.started`) candidate boundary evaluation.
   - Intermediate turn boundary without recurring `turn.started`.
   - Tool completion event candidate evaluation.
   - Assistant message generation in-progress (not a pre-dispatch boundary).
   - Documentation & verification of the unproven boundary limitation of external polling.

4. **Section 4.D: Process Termination (8 tests + Latency Benchmark)**:
   - Single process clean termination.
   - Deep process tree (parent $\to$ child $\to$ grandchild) multi-kill.
   - Already-exited process handling (no crash / ESRCH handling).
   - Unresponsive / infinite-loop process forceful termination.
   - Permission error simulation & error logging.
   - Process tree cleanup after synthetic guard trip.
   - Orphan verification (zero lingering processes).
   - Latency benchmark across 20 iterations (min, median, P90, max).

5. **Section 4.E: Stop Classification (4 tests)**:
   - `PRE_DISPATCH_HEADROOM_EXHAUSTED` classification when remaining budget is below reserve.
   - `TOKEN_CEILING_BREACHED` classification when cumulative usage breaches ceiling.
   - `TELEMETRY_INVALID` classification on malformed streaming telemetry.
   - Timeout and crash classification priority over normal completion.

6. **Section 4.F: Telemetry Integrity & Serialization (3 tests)**:
   - Complete `headroom_guard` serialization with all 12 properties.
   - Full JSON schema validation against `run_telemetry.schema.json`.
   - Dry-run schema validation compliance.

### Overall Test Suite Results:
- `test_token_guard_validation.py`: **37 / 37 passed** (100%).
- `test_block07_remediation.py`: **14 / 14 passed** (100%).
- Full harness discovery suite (`python -m unittest discover -s tests`): **114 / 114 passed** (100%).
- Zero failures, zero errors, zero regressions.

---

## 5. Known Limitations

While the token-guard implementation is robust and fully verified against synthetic streaming telemetry, offline validation established critical architectural limitations:

1. **External Polling Lag**: The external poller operates on a 50 ms polling loop. Stdout line buffering in Windows consoles and Python/Node subprocesses introduces variable latency (up to 100 ms).
2. **Process Termination Latency**: As measured across 20 trials, `taskkill /F /T` requires **197 ms to 284 ms** (median 229 ms) to recursively terminate a Windows process tree.
3. **Dispatch Race Condition**: In high-velocity agent execution, Codex may issue an outbound HTTP model request immediately upon tool execution completion ($< 20\text{ ms}$). An outbound request can already be in flight across the network before the external poller detects the turn event and issues the process kill.
4. **Conclusion**: The current architecture is a **best-effort external monitor**, NOT a deterministic zero-leakage hard gate. It is suitable as an operational safety guard with an adequate token reserve, but cannot mathematically guarantee zero overrun past a hard ceiling.
