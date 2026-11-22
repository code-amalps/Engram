# Engram — Verification Polling and Token Amplification Engineering Report

**Document Identifier:** `ENGRAM_STAGE1_VERIFICATION_POLLING_FIX_REPORT`  
**Date:** 2026-10-29  
**Status:** `COMPLETED (OFFLINE ENGINEERING ONLY)`  
**Scope:** Offline harness repair and regression verification (Zero live model/API quota consumed)  
**Parent Investigation:** [`benchmarks/STAGE1_BLOCK08_LIVE_RESULTS_REPORT.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/STAGE1_BLOCK08_LIVE_RESULTS_REPORT.md)

---

## 1. Investigation & Root Cause Forensic Analysis

In Stage 1 Block 08, Engram v1.5.4 (`v154_arm3`) executed in Order 1. By Turn 9 (177,588 tokens), the model had successfully generated a valid 33-line patch implementing `RememberMeService` in `core.module.ts`. However, between Turn 12 and Turn 16, the agent entered an interactive polling loop that consumed **108,723 tokens in 28 seconds**, breaching the 350,000-token operational ceiling at 365,176 tokens.

### 1.1 How a One-Second Tool Wait Creates a Background Session
In the Codex orchestration environment, `tools.exec_command` exposes a parameter:
```ts
yield_time_ms?: number;
// Maximum time to wait before returning a session ID for a still-running command.
// Effective range on Windows is 10000-30000 ms.
```
At Turn 12, the model invoked:
```javascript
const r = await tools.exec_command({
  cmd: "git diff ...; npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit",
  yield_time_ms: 1000,
  max_output_tokens: 12000
});
```
The model explicitly requested `yield_time_ms: 1000` (1 second). Because TypeScript AST compilation in a large Angular application requires 5–15 seconds, the process could not finish in 1,000 ms. Rather than killing the command, the PTY runtime detached the process into an active background session and returned `session_id: 79526`.

### 1.2 Why the Agent Repeatedly Calls `tools.write_stdin`
The tool schema specifies:
```ts
declare const tools: { write_stdin(args: {
  chars?: string;        // Bytes to write to stdin. Defaults to empty, which polls without writing.
  session_id: number;    // Identifier of the running unified exec session.
  yield_time_ms?: number;
}): Promise<...>; };
```
Because the background compiler had not yet returned exit code or diagnostics, the agent attempted to wait for its completion. Across Turns 13, 14, 15, and 16, the agent repeatedly issued:
```javascript
const r = await tools.write_stdin({ session_id: 79526, chars: "", yield_time_ms: 1000 });
```
Each call waited only 1 second, received empty output (`output: ""`), and yielded back to the model.

### 1.3 Why Each Poll Re-Ingests the Full Model Context
Language model API interactions are stateless. On every model turn, the client harness re-transmits the entire transcript:
- System prompt and tool declarations (~16,000 tokens)
- Turn-0 injected Change Contract and AST excerpts (~1,200 tokens)
- Exploration history (6 PowerShell `Get-Content` and `rg` outputs)
- Implementation patch and reasoning blocks

By Turn 12, the context window was **26,283 tokens**. Every subsequent polling iteration re-ingested this entire window:
- **Turn 13:** 26,922 input tokens $\rightarrow$ Cumulative: 283,430
- **Turn 14:** 27,035 input tokens $\rightarrow$ Cumulative: 310,519
- **Turn 15:** 27,146 input tokens $\rightarrow$ Cumulative: 337,744
- **Turn 16:** 27,283 input tokens $\rightarrow$ Cumulative: **365,176** (Ceiling breached)

In 4 polling turns, the model consumed **108,723 tokens** merely idling on the host.

### 1.4 Division of Responsibilities: Model vs. Harness
- **Model Agent Responsibility:** Scoped synthesis of code modifications conforming to the Change Contract and authorized target boundaries. The model should **never** act as an operating system scheduler or process-polling loop.
- **Harness Responsibility:** Host-side execution and evaluation of compilers, linters, and test gates. The host runtime must wait for long-running processes to complete outside the model's token turn, capturing output incrementally and returning only the final, definitive result.

---

## 2. Architecture & Implementation of the Host-Side Verification Workflow

To eliminate verification polling and token amplification, a dedicated host-side verification module was designed and integrated into the benchmark harness.

### 2.1 Component Architecture: `TrackedSubprocessRunner`
Located in [`benchmarks/matched-pair-harness/src/verification.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/verification.py):

1. **Tracked Subprocess Lifecycle:**  
   Spawns commands as tracked subprocesses (`subprocess.Popen`) with dedicated pipes for `stdout` and `stderr`.
2. **Incremental Output Capture:**  
   Background daemon threads read lines incrementally from `stdout` and `stderr` into thread-safe buffers, guaranteeing that partial output is captured and preserved even if the process times out, crashes, or is cancelled.
3. **Host-Side Bounded Adaptive Polling:**  
   Instead of yielding back to the model or busy-looping at 100% CPU, the host loop polls `proc.poll()` with bounded exponential backoff:
   $$\Delta t_{\text{poll}} = \min(t_{\max}, \Delta t_{\text{poll}} \times \text{factor}) \quad (50\,\text{ms} \le \Delta t \le 250\,\text{ms})$$
   This wait executes entirely on the host at **0 model tokens**.
4. **Guaranteed Process-Tree Cleanup on Windows:**  
   If execution exceeds `timeout_seconds` or receives a cancellation signal, the runner invokes `terminate_pid_tree(proc.pid)` (`taskkill /F /T /PID`), cleanly killing the parent shell and all spawned child processes (`node.exe`, `tsc`, `npm`).
5. **Deterministic Status Mapping:**  
   Returns a structured `VerificationResult` containing `exit_code`, `stdout`, `stderr`, `combined_output`, `elapsed_seconds`, `timed_out`, `cancelled`, and `status` (`PASS`, `FAIL`, `TIMEOUT`, `CANCELLED`).

### 2.2 Integration with Evaluator Quality Gates
[`benchmarks/matched-pair-harness/src/evaluator.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/evaluator.py) was updated to replace its legacy, unbuffered `subprocess.run(shell=True)` call inside `run_automated_gates`:
```python
runner = TrackedSubprocessRunner(default_timeout_seconds=300)
self.last_gate_details: Dict[str, Any] = {}

for gate_name, gate_info in gate_map.items():
    ...
    res = runner.run_command(cmd, cwd=sandbox_path, timeout_seconds=timeout)
    self.last_gate_details[gate_name] = res.to_dict()

    if res.timed_out:
        results[gate_name] = "BLOCKED"
    elif res.exit_code == 0:
        results[gate_name] = "PASS"
    else:
        results[gate_name] = "FAIL"
```
Benefits:
- Gates (`L0_typecheck`, `L2_unit_tests`, `L3_production_build`) no longer leak orphaned child processes on timeout.
- Full compiler diagnostic outputs are preserved in `last_gate_details`.
- Timeouts cleanly map to `BLOCKED` with verified process-tree termination.

---

## 3. Offline Regression Verification Suite

A new dedicated test suite [`benchmarks/matched-pair-harness/tests/test_verification_workflow.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/tests/test_verification_workflow.py) was implemented, testing all 8 required properties using local synthetic scripts and mocks without live API calls:

| Test Case | Description | Result |
| :--- | :--- | :---: |
| `test_simulated_compiler_completes_without_repeated_model_polling` | Simulated compiler running 2.0s completes on host in a single call | **PASS** |
| `test_incremental_stdout_stderr_capture` | Captures interleaved stdout and stderr flushes incrementally | **PASS** |
| `test_correct_exit_code_propagation` | Verifies exit codes 0 (PASS), 1 (FAIL), and 42 (Custom FAIL) | **PASS** |
| `test_long_running_process_reaches_timeout` | Process exceeding timeout is killed; returns status `TIMEOUT` | **PASS** |
| `test_cancellation_and_child_process_termination` | External `cancel_event` cleanly halts execution and kills tree | **PASS** |
| `test_partial_output_preservation_after_failure` | Preserves compiler diagnostics even when process exits non-zero | **PASS** |
| `test_no_busy_loop_or_excessive_polling` | Verifies poll count is small (4–25 polls over 1.2s) with adaptive sleep | **PASS** |
| `test_evaluator_integration_runs_tracked_gates` | Verifies Evaluator executes gates via `TrackedSubprocessRunner` | **PASS** |

### Complete Offline Regression Suite Results:
- `test_verification_workflow.py`: **8 / 8 PASS** (9.76s)
- `test_task_b_evaluator.py`: **14 / 14 PASS** (0.57s)
- `test_resilience_repair.py`: **7 / 7 PASS** (2.93s)
- `test_task_b_readiness_fixes.py`: **12 / 12 PASS** (6.25s)
- `test_triad.py`: **8 / 8 PASS** (7.41s)
- **Total:** **49 / 49 PASS** (0 failures, 0 skipped, 0 errors).

---

## 4. Preservation of Benchmark Protocol & Safety Invariants

1. **Zero Quota Consumption:** All engineering, testing, and validation tasks were executed completely offline. No model API requests were made.
2. **Historical Integrity:** Block 06 and Block 08 recorded telemetry, patch files, logs, and report artifacts were left 100% untouched.
3. **Frozen Binary & Protocol Integrity:**
   - Frozen Engram v1.5.4 SHA-256: `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` (Unchanged)
   - Frozen Engram v1.5.1 SHA-256: `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` (Unchanged)
   - Preregistration Seal: `db3ab2956a4da40aad8f1f376aa72a50bebc96f3e1ecc3120f405f0a294ea8bb` (Unchanged)
   - Operational Thresholds: 350,000-token ceiling for Block 08, 600-second timeout, and mandatory stop-on-failure remain intact.
4. **Git Safety:** Zero commits, pushes, tags, stashes, or resets were performed.

---

## 5. Summary of Files Changed

| File | Status | Description |
| :--- | :---: | :--- |
| `benchmarks/matched-pair-harness/src/verification.py` | **New** | Host-side tracked subprocess runner with adaptive polling, incremental capture, and process-tree termination. |
| `benchmarks/matched-pair-harness/src/evaluator.py` | **Modified** | Integrated `TrackedSubprocessRunner` into `run_automated_gates`. |
| `benchmarks/matched-pair-harness/tests/test_verification_workflow.py` | **New** | Comprehensive 8-test unit suite covering compiler simulation, timeouts, and cancellation. |
| `benchmarks/STAGE1_VERIFICATION_POLLING_FIX_REPORT.md` | **New** | Formal documentation of the investigation, architecture, and verification results. |
