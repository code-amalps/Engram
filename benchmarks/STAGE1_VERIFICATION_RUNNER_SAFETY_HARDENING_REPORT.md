# Engram — Verification Runner Safety Hardening Report

**Document Identifier:** `ENGRAM_STAGE1_VERIFICATION_RUNNER_SAFETY_HARDENING_REPORT`  
**Date:** 2026-10-29  
**Status:** `COMPLETED (OFFLINE ENGINEERING & VERIFICATION ONLY)`  
**Scope:** Host-side verification runner safety hardening across monotonic timing, bounded memory, aggregate gate deadlines, robust process-tree cleanup, and expanded regression test suite  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram` (Branch: `feature/async-precommit-guard`)  
**Live Execution / Quota:** Zero model tokens, zero live API calls, zero benchmark block executions  

---

## 1. Executive Summary of Implemented Hardening Measures

Following the regression evidence reconciliation audit, a narrowly scoped safety hardening pass was implemented in [`benchmarks/matched-pair-harness/src/verification.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/verification.py), [`benchmarks/matched-pair-harness/src/evaluator.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/evaluator.py), and [`benchmarks/matched-pair-harness/tests/test_verification_workflow.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/tests/test_verification_workflow.py).

### 1.1 Monotonic Timing Replacement
- Replaced all wall-clock `time.time()` references in `verification.py` and `evaluator.py` with `time.monotonic()`.
- Process timeouts and elapsed durations are now strictly clock-independent, preventing failures or premature timeouts caused by system NTP clock steps, sleep/resume cycles, or daylight saving transitions.

### 1.2 Bounded stdout/stderr Output Capture
- Implemented `BoundedStreamBuffer` in `verification.py`:
  - Enforces a strict total memory bound (`max_output_chars`, default 50,000 characters, 25,000 characters per stream).
  - Preserves critical initial diagnostics (`head`) and trailing error diagnostics (`tail`).
  - Concurrent reader threads prune older chunks in real time via a thread-safe `collections.deque`, preventing unbounded memory consumption during runaway compilation or test loops.
  - Generates an explicit truncation banner:  
    `\n[... OUTPUT TRUNCATED: {omitted} characters omitted ...]\n`
  - Records structured boolean flags in `VerificationResult`: `output_truncated`, `stdout_truncated`, `stderr_truncated`.

### 1.3 Aggregate Verification Deadline Across Sequential Gates
- Implemented cumulative deadline enforcement in `Evaluator.run_automated_gates` and `TrackedSubprocessRunner.run_suite`:
  - Each gate in sequence receives only the remaining time from the total verification budget:  
    `effective_timeout = min(gate_timeout, remaining_budget)`.
  - When the cumulative budget is exhausted (`remaining <= 0`), subsequent enabled gates cannot start and are deterministically marked `"BLOCKED"`.
  - Disabled gates continue to be recorded as `"SKIPPED"` per existing evaluator semantics.

### 1.4 Standard Input Closure (`stdin=subprocess.DEVNULL`)
- Set `stdin=subprocess.DEVNULL` across all verification subprocess invocations in `verification.py` as well as all git diff, status, and patch subprocesses in `evaluator.py`.
- Prevents processes from hanging indefinitely on interactive prompts or missing standard input.

### 1.5 Robust Process-Tree Termination & Pipe Cleanup
- Replaced unguarded termination with `TrackedSubprocessRunner._terminate_proc_tree`:
  - Inspects `proc.poll() is not None` before force-killing to avoid benign errors if the child process exited concurrently.
  - Catches `ProcessLookupError`, `PermissionError`, and `OSError` during termination.
  - Force-closes `proc.stdout` and `proc.stderr` if reader threads remain blocked on orphaned descendant process pipes, ensuring prompt thread join without deadlocks.

### 1.6 Clarification of Harness Architecture
- Removed the unsupported claim that the evaluator silently falls back to unmonitored standard subprocesses if `verification.py` is absent. `verification.py` is explicitly established as a mandatory, first-class component of the matched-pair harness.

---

## 2. Comprehensive Offline Regression Validation Matrix

All seven offline regression test suites were executed sequentially from the project root `C:\Users\Amal P S\OneDrive\Projects\Engram` using Python 3.14's `unittest` module with verbose reporting (`-v`).

```
========================================================================================
SUITE                                  TESTS    PASSED   FAILED   ERRORS   DURATION (s)
========================================================================================
test_verification_workflow.py            12       12        0        0        11.103
test_task_b_evaluator.py                 14       14        0        0         0.227
test_resilience_repair.py                 7        7        0        0         2.704
test_harness_safety_audit.py             19       19        0        0         5.721
test_task_b_readiness_fixes.py           12       12        0        0         2.131
test_triad.py                             8        8        0        0         3.086
test_suite.py                            68       68        0        0        24.547
========================================================================================
TOTAL                                   140      140        0        0        49.519
========================================================================================
VERDICT: PASS — 100% OF OFFLINE REGRESSION SUITES SATISFIED (EXIT CODE 0)
========================================================================================
```

### 2.1 Suite Details: `test_verification_workflow.py` (12 tests)
- **Command:** `python -m unittest benchmarks/matched-pair-harness/tests/test_verification_workflow.py -v`
- **Exit Code:** `0`
- **Duration:** 11.103 seconds
- **Test Outcomes:**
  1. `test_simulated_compiler_completes_without_repeated_model_polling`: ok
  2. `test_incremental_stdout_stderr_capture`: ok
  3. `test_correct_exit_code_propagation`: ok
  4. `test_long_running_process_reaches_timeout`: ok
  5. `test_cancellation_and_child_process_termination`: ok
  6. `test_partial_output_preservation_after_failure`: ok
  7. `test_no_busy_loop_or_excessive_polling`: ok
  8. `test_evaluator_integration_runs_tracked_gates`: ok
  9. `test_clock_independent_monotonic_timeout`: ok *(New: verifies clock independence)*
  10. `test_output_cap_and_truncation_preserves_head_and_tail`: ok *(New: verifies 100k char stream capped at bound with head/tail preserved)*
  11. `test_aggregate_deadline_exhaustion_marks_subsequent_gates_blocked`: ok *(New: verifies remaining budget propagation and BLOCKED status)*
  12. `test_descendant_process_tree_termination`: ok *(New: verifies detached grandchild processes terminated by taskkill)*

### 2.2 Suite Details: `test_task_b_evaluator.py` (14 tests)
- **Command:** `python -m unittest benchmarks/matched-pair-harness/tests/test_task_b_evaluator.py -v`
- **Exit Code:** `0`
- **Duration:** 0.227 seconds
- **Outcomes:** 14 passed (REQ-1 UI layout, REQ-2 persistence restoration, REQ-3 security negative, REQ-4 boundary control).

### 2.3 Suite Details: `test_resilience_repair.py` (7 tests)
- **Command:** `python -m unittest benchmarks/matched-pair-harness/tests/test_resilience_repair.py -v`
- **Exit Code:** `0`
- **Duration:** 2.704 seconds
- **Outcomes:** 7 passed (missing telemetry recovery, JSONL truncation recovery, stop-on-failure triad halt, timeout record validation).

### 2.4 Suite Details: `test_harness_safety_audit.py` (19 tests)
- **Command:** `python -m unittest benchmarks/matched-pair-harness/tests/test_harness_safety_audit.py -v`
- **Exit Code:** `0`
- **Duration:** 5.721 seconds
- **Outcomes:** 19 passed (token ceiling boundaries, grandchild process-tree cleanup, deadlock-free reader threads, sandbox cleanup).

### 2.5 Suite Details: `test_task_b_readiness_fixes.py` (12 tests)
- **Command:** `python -m unittest benchmarks/matched-pair-harness/tests/test_task_b_readiness_fixes.py -v`
- **Exit Code:** `0`
- **Duration:** 2.131 seconds
- **Outcomes:** 12 passed (Turn-0 injection, AST contract directives, PATH sanitization with rg retention, custom/MCP event parsing).

### 2.6 Suite Details: `test_triad.py` (8 tests)
- **Command:** `python -m unittest benchmarks/matched-pair-harness/tests/test_triad.py -v`
- **Exit Code:** `0`
- **Duration:** 3.086 seconds
- **Outcomes:** 8 passed (binary hash validation, Latin square 10-block counterbalancing, full 3-arm dry run).

### 2.7 Suite Details: `test_suite.py` (68 tests)
- **Command:** `python -m unittest benchmarks/matched-pair-harness/tests/test_suite.py -v`
- **Exit Code:** `0`
- **Duration:** 24.547 seconds
- **Outcomes:** 68 passed (code review findings F-01 through F-06, dependency isolation, token provenance cases 1–9, comparative savings guards).

---

## 3. Cryptographic and Working-Tree Integrity Verification

| Artifact | File Path | Expected SHA-256 | Recomputed SHA-256 | Status |
| :--- | :--- | :--- | :--- | :---: |
| **Engram v1.5.1 Executable** | `benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe` | `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` | `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` | **MATCH** |
| **Engram v1.5.4 Executable** | `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe` | `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` | `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` | **MATCH** |
| **Preregistration Seal** | `benchmarks/cost-baseline/PREREGISTRATION.md` | `db3ab2956a4da40aad8f1f376aa72a50bebc96f3e1ecc3120f405f0a294ea8bb` | `db3ab2956a4da40aad8f1f376aa72a50bebc96f3e1ecc3120f405f0a294ea8bb` | **MATCH** |
| **Golden Commit** | `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` | Commit: `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` (68,274 files) | Structural digest `f63d2e4c069e...` intact | **MATCH** |

### Git Working Tree Status
- Only target files [`src/verification.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/verification.py), [`src/evaluator.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/evaluator.py), and [`tests/test_verification_workflow.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/tests/test_verification_workflow.py) were modified.
- No resets, stashes, reverts, commits, tags, pushes, or releases were performed.
- Pre-existing untracked report files remain preserved.

---

## 4. Final Operational State

1. **Safety Hardening:** **100% IMPLEMENTED AND VALIDATED**. All required properties (monotonic timing, bounded buffer capture, aggregate deadline enforcement, stdin closure, robust process-tree termination) are active in source code and proven by passing unit tests.
2. **Quota & Benchmark Status:** **Zero model/API quota consumed**. Blocks 06–10 remain strictly unexecuted.
3. **Execution Blocked:** Execution of live benchmarks remains stopped awaiting explicit human authorization.
