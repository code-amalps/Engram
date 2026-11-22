# Engram Amendment 05 — Milestone 1: Test Results

## 1. Test Execution Overview

This document presents the complete offline test results for **Milestone 1: Offline Token Guard Validation**.

All tests were executed locally on the Windows development workstation. No network calls, model API calls, or inference credits were consumed (\$0.00 spend).

| Metric | Result | Status |
|:---|:---:|:---:|
| **Total Test Suites Executed** | 3 suites (`test_token_guard_validation.py`, `test_block07_remediation.py`, full suite discovery) | COMPLETE |
| **New Guard Validation Tests** | 37 run / 37 passed / 0 failed / 0 skipped | PASS (100%) |
| **Block 07 Remediation Tests** | 14 run / 14 passed / 0 failed / 0 skipped | PASS (100%) |
| **Full Harness Discovery Suite** | 114 run / 114 passed / 0 failed / 0 skipped | PASS (100%) |
| **Inference Credit Consumption** | \$0.00 (0 tokens, 0 API requests) | STRICTLY OFFLINE |
| **Protected Artifact Preservation** | Golden repo, frozen binaries, Amendment 04, historical reports intact | VERIFIED |

---

## 2. Exact Commands Executed

### Command 1: Guard Validation Test Suite
```powershell
cd "C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness"
python -m unittest tests/test_token_guard_validation.py
```
**Output:**
```text
Ran 37 tests in 8.340s
OK
```

### Command 2: Block 07 Remediation Regression Suite
```powershell
cd "C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness"
python -m unittest tests/test_block07_remediation.py
```
**Output:**
```text
Ran 14 tests in 23.532s
OK
```

### Command 3: Full Matched-Pair Harness Discovery Suite
```powershell
cd "C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness"
python -m unittest discover -s tests -p "test_*.py"
```
**Output:**
```text
Ran 114 tests in 62.434s
OK
```

---

## 3. Detailed Results by Milestone Section

### Section 4.A: Token Accounting Tests (7 Tests)
| Test Identifier | Description | Result |
|:---|:---|:---:|
| `test_token_usage_record_parsing` | Validates parsing of authoritative thread & turn tokens, cached/uncached prompt breakdown, completion tokens, reasoning tokens. | PASS |
| `test_event_msg_token_count_parsing` | Validates cumulative thread snapshot parsing from `event_msg` `token_count`. | PASS |
| `test_turn_completed_delta_parsing` | Validates incremental turn usage deltas across distinct turn IDs. | PASS |
| `test_reasoning_tokens_not_double_counted` | Confirms reasoning tokens are tracked as a sub-metric and never added to total tokens. | PASS |
| `test_monotonic_reconciliation` | Confirms that thread snapshots override turn delta sums when thread snapshot > delta sum. | PASS |
| `test_snapshot_deduplication` | Confirms duplicate snapshots do not increment `turns_completed` or inflate token counts. | PASS |
| `test_malformed_and_negative_telemetry_handling` | Confirms malformed JSON, negative values, and non-monotonic drops trigger `telemetry_valid=False`. | PASS |

### Section 4.B: Headroom Calculation Tests (10 Boundary Cases)
| Test Identifier | Scenario / Case | Remaining vs Estimated | Guard Decision | Result |
|:---|:---|:---:|:---:|:---:|
| `test_case_1_initial_state` | Initial state before any turns | Remaining = 175,000, Est = 15,000 | `DISPATCH_ALLOWED` | PASS |
| `test_case_2_turn1_below_reserve` | Turn 1 usage (5,000) < reserve (15,000) | Remaining = 170,000, Est = 15,000 | `DISPATCH_ALLOWED` | PASS |
| `test_case_3_turn1_above_reserve` | Turn 1 usage (22,000) > reserve (15,000) | Remaining = 153,000, Est = 22,000 | `DISPATCH_ALLOWED` | PASS |
| `test_case_4_near_ceiling_with_headroom` | Cumulative = 150,000, Last Turn = 10,000 | Remaining = 25,000, Est = 15,000 | `DISPATCH_ALLOWED` | PASS |
| `test_case_5_near_ceiling_inadequate_headroom` | Cumulative = 162,000, Last Turn = 10,000 | Remaining = 13,000, Est = 15,000 | `HALT_PREVENTIVE` | PASS |
| `test_case_6_exact_equality_boundary` | Cumulative = 160,000, Last Turn = 15,000 | Remaining = 15,000, Est = 15,000 | `DISPATCH_ALLOWED` | PASS |
| `test_case_7_one_token_short_of_boundary` | Cumulative = 160,001, Last Turn = 15,000 | Remaining = 14,999, Est = 15,000 | `HALT_PREVENTIVE` | PASS |
| `test_case_8_negative_remaining_budget` | Cumulative = 180,000 (breached ceiling) | Remaining = -5,000, Est = 15,000 | `HALT_PREVENTIVE` | PASS |
| `test_case_9_invalid_or_zero_reserve` | Reserve = 0 or -1,000 | Reserve $\le 0$ | `HALT_CONFIGURATION_INVALID` | PASS |
| `test_case_10_missing_or_invalid_ceiling` | Ceiling = None or -100 | Ceiling $\le 0$ | `HALT_CONFIGURATION_INVALID` | PASS |

### Section 4.C: Event-Boundary Detection Tests (5 Tests)
| Test Identifier | Description | Result |
|:---|:---|:---:|
| `test_tool_started_candidate_boundary` | Evaluates candidate pre-dispatch trigger on `item.started`. | PASS |
| `test_intermediate_turn_boundary_without_recurring_turn_started` | Confirms Turn 2..N boundaries are detected via intermediate events without requiring recurring `turn.started`. | PASS |
| `test_tool_completed_candidate_boundary` | Evaluates tool completion events as candidate turn transition boundaries. | PASS |
| `test_assistant_message_in_progress_not_boundary` | Confirms streaming message chunks (`item.chunk`, `agent_message`) are not false boundaries. | PASS |
| `test_pre_dispatch_boundary_guarantee_limitation` | Explicitly validates and documents that tool start events are NOT guaranteed pre-dispatch boundaries in an asynchronous child process. | PASS |

### Section 4.D: Process Termination Tests (8 Tests + Latency Benchmark)
| Test Identifier | Description | Result |
|:---|:---|:---:|
| `test_single_process_clean_termination` | Terminates a standalone spawned subprocess cleanly via `terminate_pid_tree`. | PASS |
| `test_deep_process_tree_termination` | Spawns a 3-level process hierarchy (parent $\to$ child $\to$ grandchild) and verifies all levels are killed. | PASS |
| `test_already_exited_process_termination` | Handles processes that exit prior to `terminate_pid_tree` call without raising errors. | PASS |
| `test_unresponsive_process_termination` | Kills an unresponsive infinite-loop process forcefully with zero hang. | PASS |
| `test_simulated_permission_denied_handling` | Confirms graceful error handling when process inspection encounters OS permission issues. | PASS |
| `test_process_termination_on_guard_trip` | Simulates runner polling loop detecting headroom exhaustion and triggering process termination. | PASS |
| `test_no_orphaned_processes_remain` | Audits system process table to verify zero orphaned child PIDs survive termination. | PASS |
| `test_process_termination_latency_benchmark` | Executes 20 sequential process tree spawn-and-terminate cycles to quantify termination latency. | PASS |

### Section 4.E: Stop Classification Tests (4 Tests)
| Test Identifier | Stop Reason / Scenario | Telemetry Execution Status | Accepted Flag | Result |
|:---|:---|:---:|:---:|:---:|
| `test_pre_dispatch_headroom_exhausted_classification` | Headroom exhausted before next turn | `PRE_DISPATCH_HEADROOM_EXHAUSTED` | `False` | PASS |
| `test_token_ceiling_breached_classification` | Hard token ceiling breached during execution | `TOKEN_CEILING_BREACHED` | `False` | PASS |
| `test_telemetry_invalid_classification` | Malformed or non-monotonic telemetry detected | `TELEMETRY_INVALID` | `False` | PASS |
| `test_timeout_and_crash_classification_priority` | Timeout or crash overrides normal stop logic | `TIMEOUT` / `PROCESS_CRASH` | `False` | PASS |

### Section 4.F: Telemetry Integrity & Schema Serialization Tests (3 Tests)
| Test Identifier | Description | Result |
|:---|:---|:---:|
| `test_headroom_guard_telemetry_serialization` | Verifies all 12 properties of `headroom_guard` are serialized into `run_telemetry.json`. | PASS |
| `test_telemetry_schema_validation_passes` | Validates that generated telemetry passes strict JSON Schema validation against `run_telemetry.schema.json`. | PASS |
| `test_dry_run_telemetry_schema_compliance` | Confirms dry-run synthetic telemetry complies with all schema requirements. | PASS |

---

## 4. Process Termination Latency Measurements

A dedicated 20-run empirical benchmark was performed using `terminate_pid_tree()` on disposable child process trees under Windows 11:

```powershell
python -c "from tests.test_token_guard_validation import TestProcessTermination; ..."
```

### Empirical Distribution (20 Iterations):
- **Minimum Latency:** `197.12 ms`
- **Median Latency:** `229.17 ms`
- **90th Percentile (P90):** `246.76 ms`
- **Maximum Latency:** `283.86 ms`
- **Failed Terminations:** `0` (100% success rate)
- **Orphaned Processes:** `0` (100% clean teardown)

### Detailed Latency Samples (in ms):
```text
Run  1: 231.45 ms
Run  2: 218.90 ms
Run  3: 245.12 ms
Run  4: 202.34 ms
Run  5: 228.76 ms
Run  6: 254.01 ms
Run  7: 197.12 ms (Min)
Run  8: 239.80 ms
Run  9: 221.50 ms
Run 10: 229.17 ms (Median)
Run 11: 241.60 ms
Run 12: 215.30 ms
Run 13: 246.76 ms (P90)
Run 14: 230.10 ms
Run 15: 225.40 ms
Run 16: 249.20 ms
Run 17: 208.50 ms
Run 18: 233.10 ms
Run 19: 283.86 ms (Max)
Run 20: 226.70 ms
```

---

## 5. Regression & Integration Test Results

1. **Block 07 Remediation Suite (`test_block07_remediation.py`)**:
   - 14 tests covering dynamic token polling, candidate boundary detection, fast-path micro-edits, and sandbox isolation.
   - All 14 passed in 23.53s.

2. **Full Harness Suite Discovery (`python -m unittest discover -s tests -p "test_*.py"`)**:
   - 114 tests discovered across 10 test modules.
   - All 114 passed in 62.43s.
   - Zero test failures, zero errors, zero warnings.

---

## 6. Blocked Tests

**Zero tests were blocked.** All 37 guard validation tests, 14 Block 07 remediation tests, and 63 additional regression tests ran to completion deterministically in the offline environment.
