# Engram — Broader Offline Regression Validation Report

**Document Identifier:** `ENGRAM_STAGE1_VERIFICATION_OFFLINE_REGRESSION_REPORT`  
**Date:** 2026-10-29  
**Status:** `COMPLETED (OFFLINE VALIDATION ONLY)`  
**Scope:** Broader offline regression validation of the Engram matched-pair benchmark harness across all unit and integration test suites  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram` (Branch: `feature/async-precommit-guard`)  
**Quota / Live Execution:** Zero model tokens, zero live API calls, zero benchmark block executions  

---

## 1. Executive Summary & Validation Verdict

Following the implementation of the host-side verification polling architecture (`verification.py`, `evaluator.py`, and `test_verification_workflow.py`), a comprehensive offline regression validation was conducted across the entire Engram matched-pair benchmark suite.

### Key Results
- **Total Test Suites Executed:** 7
- **Total Tests Executed:** 136
- **Passed:** 136 (100%)
- **Failed:** 0
- **Errors:** 0
- **Skipped:** 0
- **Live Benchmark Blocks Executed:** 0 (Blocks 06–10 remain strictly unexecuted)
- **Model / API Quota Consumed:** 0 tokens ($0.00 cost)
- **Frozen Binary & Preregistration Integrity:** Bit-for-bit identical to sealed baselines.

```
========================================================================================
SUITE                                  TESTS    PASSED   FAILED   ERRORS   DURATION (s)
========================================================================================
test_verification_workflow.py             8        8        0        0         8.058
test_task_b_evaluator.py                 14       14        0        0         0.297
test_resilience_repair.py                 7        7        0        0         2.410
test_harness_safety_audit.py             19       19        0        0         9.834
test_task_b_readiness_fixes.py           12       12        0        0         6.274
test_triad.py                             8        8        0        0         7.329
test_suite.py                            68       68        0        0        76.784
========================================================================================
TOTAL                                   136      136        0        0       110.986
========================================================================================
VERDICT: PASS — ALL REGRESSION GATES SATISFIED
========================================================================================
```

---

## 2. Detailed Execution Matrix

Each test suite was executed offline from the project root `C:\Users\Amal P S\OneDrive\Projects\Engram` using Python 3.14's `unittest` module with verbose reporting (`-v`).

### 2.1 Suite 1: `test_verification_workflow.py`
- **Command:** `python -m unittest benchmarks/matched-pair-harness/tests/test_verification_workflow.py -v`
- **Exit Code:** `0`
- **Duration:** 8.058 seconds
- **Test Count:** 8
- **Results:**
  - `test_context_records_multiple_gates`: ok
  - `test_cancellation_token_aborts_active_process`: ok
  - `test_evaluator_uses_tracked_runner_for_all_gates`: ok
  - `test_evaluator_handles_missing_verification_runner`: ok
  - `test_runner_captures_stdout_stderr_and_exit_code`: ok
  - `test_runner_enforces_timeout_and_terminates_process`: ok
  - `test_runner_handles_nonexistent_command`: ok
  - `test_runner_truncates_oversized_output`: ok

### 2.2 Suite 2: `test_task_b_evaluator.py`
- **Command:** `python -m unittest benchmarks/matched-pair-harness/tests/test_task_b_evaluator.py -v`
- **Exit Code:** `0`
- **Duration:** 0.297 seconds
- **Test Count:** 14
- **Results:**
  - `test_disallowed_file_modifications`: ok
  - `test_exact_token_cost_zero_floor`: ok
  - `test_full_acceptance_all_requirements_met`: ok
  - `test_full_acceptance_score_normalized`: ok
  - `test_gate_results_schema_conformance`: ok
  - `test_l0_git_impact_failure`: ok
  - `test_l1_ast_validation_failure`: ok
  - `test_l2_typecheck_blocked_timeout`: ok
  - `test_l2_typecheck_failure`: ok
  - `test_l3_unit_tests_blocked_timeout`: ok
  - `test_l3_unit_tests_failure`: ok
  - `test_missing_file_in_blast_radius`: ok
  - `test_multi_arm_cost_variance_calculation`: ok
  - `test_offline_evaluator_no_network_dependency`: ok

### 2.3 Suite 3: `test_resilience_repair.py`
- **Command:** `python -m unittest benchmarks/matched-pair-harness/tests/test_resilience_repair.py -v`
- **Exit Code:** `0`
- **Duration:** 2.410 seconds
- **Test Count:** 7
- **Results:**
  - `test_runner_recovers_missing_events_file`: ok
  - `test_runner_handles_truncated_jsonl_gracefully`: ok
  - `test_orchestrator_stop_on_failure_enforcement`: ok
  - `test_sandbox_cleanup_on_unhandled_exception`: ok
  - `test_token_ceiling_hard_kill_race_prevention`: ok
  - `test_telemetry_unavailable_flag_persistence`: ok
  - `test_evaluator_timeout_gate_isolation`: ok

### 2.4 Suite 4: `test_harness_safety_audit.py`
- **Command:** `python -m unittest benchmarks/matched-pair-harness/tests/test_harness_safety_audit.py -v`
- **Exit Code:** `0`
- **Duration:** 9.834 seconds
- **Test Count:** 19
- **Results:**
  - `test_corrupted_and_truncated_jsonl_handled`: ok
  - `test_duplicate_turn_events_deduplicated`: ok
  - `test_grandchild_process_tree_termination`: ok
  - `test_missing_events_jsonl_returns_unavailable_metrics`: ok
  - `test_missing_or_delayed_tokens_active_process`: ok
  - `test_mixed_snapshots_and_turn_deltas`: ok
  - `test_native_rollout_recovery_when_stdout_empty`: ok
  - `test_process_never_exits_times_out_promptly`: ok
  - `test_reader_threads_bounded_and_no_deadlock`: ok
  - `test_sandbox_teardown_on_all_paths`: ok
  - `test_simultaneous_timeout_and_token_breach`: ok
  - `test_stop_on_failure_halts_immediately_and_suppresses_savings`: ok
  - `test_timeout_preserves_partial_stdout_and_stderr`: ok
  - `test_token_ceiling_boundary_exactly_at`: ok
  - `test_token_ceiling_boundary_just_above`: ok
  - `test_token_ceiling_boundary_just_below`: ok
  - `test_token_ceiling_single_large_jump`: ok
  - `test_token_snapshots_not_summed_repeatedly`: ok
  - `test_turn_deltas_summed_across_turns`: ok

### 2.5 Suite 5: `test_task_b_readiness_fixes.py`
- **Command:** `python -m unittest benchmarks/matched-pair-harness/tests/test_task_b_readiness_fixes.py -v`
- **Exit Code:** `0`
- **Duration:** 6.274 seconds
- **Test Count:** 12
- **Results:**
  - `test_fix1_engram_binary_inject_produces_contract_directives`: ok
  - `test_fix1_evaluator_ignores_injected_turn0_files`: ok
  - `test_fix1_turn0_injection_deterministic_offline_dry_run`: ok
  - `test_fix1_turn0_injection_omitted_for_baseline_and_v151`: ok
  - `test_fix2_task_b_prompt_discovery_guidance`: ok
  - `test_fix3_path_sanitization_removes_engram_while_retaining_rg`: ok
  - `test_fix3_sandbox_path_rg_discoverable`: ok
  - `test_fix4_parse_events_custom_tool_call_shape`: ok
  - `test_fix4_parse_events_deduplication`: ok
  - `test_fix4_parse_events_mcp_tool_call_shape`: ok
  - `test_fix4_parse_file_real_handshake_and_synthetic`: ok
  - `test_fix4_parse_rollout_file_real_session`: ok

### 2.6 Suite 6: `test_triad.py`
- **Command:** `python -m unittest benchmarks/matched-pair-harness/tests/test_triad.py -v`
- **Exit Code:** `0`
- **Duration:** 7.329 seconds
- **Test Count:** 8
- **Results:**
  - `test_binary_verification_hashes`: ok
  - `test_latin_square_10_blocks_specification`: ok
  - `test_latin_square_counterbalancing`: ok
  - `test_failure_on_baseline_suppresses_savings`: ok
  - `test_failure_on_v154_suppresses_savings`: ok
  - `test_full_acceptance_computes_exact_savings`: ok
  - `test_missing_telemetry_suppresses_savings_even_if_accepted`: ok
  - `test_offline_dry_run_triad_execution`: ok

### 2.7 Suite 7: `test_suite.py`
- **Command:** `python -m unittest benchmarks/matched-pair-harness/tests/test_suite.py -v`
- **Exit Code:** `0`
- **Duration:** 76.784 seconds
- **Test Count:** 68
- **Results Summary:**
  - Code Review Remediation (F-01 to F-06): 22 tests ok
  - Dependency Isolation and Integrity: 2 tests ok
  - Matched Pair Comparative Logic: 4 tests ok
  - Comparative Savings Guard: 4 tests ok
  - Task A Fixture and Acceptance Evaluator: 7 tests ok
  - Telemetry Accounting: 3 tests ok
  - Token Provenance and Cache Integrity (Cases 1–9): 9 tests ok
  - Runner Execution, Timeout, and Process Tree: 17 tests ok

---

## 3. Subsystem Coverage Analysis

### 3.1 Host-Side Verification Subsystem (`src/verification.py`)
- **Capabilities Verified:**
  - Synchronous process tracking with monotonic timeout tracking.
  - Reader thread output ingestion without pipe buffer deadlocks.
  - Process tree termination via Windows native `taskkill /F /T /PID`.
  - Non-blocking `CancellationToken` cooperative termination.
  - Hard output cap (50,000 characters) preventing unbounded memory growth.
- **Coverage:** Tested across synthetic short and long processes, missing executable exceptions, timeout paths, and multi-gate sequencing.

### 3.2 Evaluator Subsystem (`src/evaluator.py`)
- **Capabilities Verified:**
  - Full backward compatibility: all 4 gates (L0 AST/Git, L1 Syntax/Diff, L2 TypeScript compile, L3 Jasmine unit tests) execute cleanly through `TrackedSubprocessRunner`.
  - Result schema compatibility: `gate_results` dictionary, status enum mapping (`PASS`, `FAIL`, `BLOCKED`, `SKIPPED`), diagnostic truncation, and float score normalization ($C \in [0.0, 1.0]$) remain completely intact.
  - Fallback safety: If `verification.py` is missing or runner is unset, evaluator safely falls back to standard subprocess execution without breaking.

### 3.3 Harness Isolation and Runner Safety (`src/runner.py`, `src/isolation.py`)
- **Capabilities Verified:**
  - Ceiling boundary enforcement ($> 250,000$ or $> 350,000$).
  - Simultaneous timeout and ceiling breach race condition handling.
  - Reader thread termination within 2 seconds.
  - Sandbox teardown on normal, timeout, and unhandled exception paths.
  - Grandchild process-tree cleanup preventing dangling node/tsc processes.

---

## 4. Frozen Artifacts & Working Tree Integrity Verification

### 4.1 Frozen Binary Hashes
| Binary | Path | Expected SHA-256 | Observed SHA-256 | Status |
| :--- | :--- | :--- | :--- | :--- |
| **Engram v1.5.4** | `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe` | `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` | `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` | **MATCH** |
| **Engram v1.5.1** | `benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe` | `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` | `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` | **MATCH** |

### 4.2 Preregistration Seal Hash
| Document | Path | Expected SHA-256 | Observed SHA-256 | Status |
| :--- | :--- | :--- | :--- | :--- |
| **Frozen Preregistration** | `benchmarks/cost-baseline/PREREGISTRATION.md` | `db3ab2956a4da40aad8f1f376aa72a50bebc96f3e1ecc3120f405f0a294ea8bb` | `db3ab2956a4da40aad8f1f376aa72a50bebc96f3e1ecc3120f405f0a294ea8bb` | **MATCH** |

### 4.3 Golden Repository Verification
- **Golden Repository Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` (`HEAD`, clean).
- **Golden Dependency Fingerprint:** 68,274 files, recursive digest `f63d2e4c069e14aa3cc4765fa327c6b7e6603d19b3a6945a9dd34eec8f074476`. Verified pristine.

### 4.4 Git Working-Tree Integrity
The working tree in `Engram` contains only the expected pre-existing modifications and reports. No files were reset, stashed, overwritten, or reverted. No git commits, tags, pushes, or releases were performed.

---

## 5. Identified Limitations & Recommended Hardening Pass

The broader regression validation confirms that all existing test suites pass unconditionally. However, from the prior focused code review, the following defensive engineering refinements remain documented for consideration before executing any live model benchmark:

1. **Child Process Tree Termination on Windows:**
   - In `TrackedSubprocessRunner._kill_process_tree`, catch `NoSuchProcess` or check if `proc.poll() is not None` before invoking `taskkill /F /T /PID` to avoid benign warning logs when a child process exits concurrently with timeout expiration.
2. **Multi-byte UTF-8 Pipe Decoding:**
   - In reader threads, handle potential chunk fragmentation across UTF-8 multi-byte boundaries by using `io.TextIOWrapper` or accumulating partial bytes before decoding with `errors="replace"`.
3. **Explicit Subprocess Standard Input Closure:**
   - Set `stdin=subprocess.DEVNULL` by default when spawning gates to prevent any child process from waiting on standard input.
4. **Per-Gate vs. Block-Level Verification Timeouts:**
   - Ensure `VerificationRunContext` explicitly enforces an aggregate verification deadline across gates (e.g. max 120s total across L0–L3) so sequential slow gates cannot exceed the arm's global timeout.

---

## 6. Conclusion & Current Operational State

1. **Regression Status:** Full regression validation is **100% SUCCESSFUL** across all 136 tests.
2. **Quota Status:** **Zero model/API quota consumed**.
3. **Benchmark Execution Status:** No benchmark blocks (Blocks 06–10) were executed.
4. **Readiness:** The matched-pair harness is functionally sound and offline-validated.
5. **Next Step:** Awaiting explicit human instruction regarding whether to implement the defensive code review hardening refinements or proceed with next protocol actions.
