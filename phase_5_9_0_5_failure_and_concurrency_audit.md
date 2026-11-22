# Engram Phase 5.9.0.5: Failure, Concurrency & Accounting Integrity Audit

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Phase:** 5.9.0.5 Workstream F  
**Execution Mode:** Strictly Offline, Read-Only Adversarial Audit  
**Audit Target:** Subprocess Failure Modes, Concurrency Races & Cross-Process Crash Integrity  
**Safety Classification:** `FAIL_CLOSED_WITH_EXPLICIT_RECOVERY_BOUNDARIES`

---

## 1. Purpose

This deliverable provides an adversarial forensic audit of the failure, concurrency, and crash-recovery mechanisms in the benchmark stack. It evaluates 14 concrete failure and concurrency vectors, determining whether unobserved provider tokens can compromise budget conservation, whether subsequent arms can be dispatched with corrupted accounting, and what code-level evidence supports crash safety.

---

## 2. Exhaustive Audit of the 14 Failure & Concurrency Scenarios

| Scenario # | Failure / Concurrency Event | Unobserved Provider Tokens Possible? | Subsequent Arm Dispatched? | Aggregate Undercount Possible? | System Behavior | Recovery & Duplicate Dispatch Risk |
|---|---|---|---|---|---|---|
| **S-01** | Process termination during in-flight request | **YES** (Server generates before TCP drop) | **NO** | **NO** (Pentad halts) | `orchestrator.py:2807` catches non-zero exit; sets `arm_status = EXECUTION_ERROR`. Coordinator aborts Invocation 2 | Fail-closed. Re-run requires clean workspace; no automated duplicate dispatch |
| **S-02** | Provider timeout (HTTP 504 / gateway drop) | **YES** (Partial provider work) | **NO** | **NO** | `runner.py:54` sets `timed_out = True`; `is_budget_uncertain = True`; halts loop | Subprocess killed via `terminate_pid_tree`; run marked `TIMEOUT`; no automated restart |
| **S-03** | Partial streaming response (corrupted JSONL) | **YES** (Unparsed tail events) | **NO** | **NO** | `StreamingTokenTracker:131` increments `partial_records_count`; marks `telemetry_valid = False` | Halts with `ABORTED_UNCERTAIN_BUDGET`; prevents subsequent arm dispatch |
| **S-04** | Missing final usage event | **YES** (Last turn uncounted) | **NO** | **NO** | `Orchestrator.validate_telemetry_integrity` detects missing `total_model_tokens`; sets `is_budget_uncertain = True` | Halts immediately; Invocation 2 coordinator validation Gate 10 rejects manifest |
| **S-05** | Duplicate usage events emitted | **NO** (Deduplicated) | **YES** (Safe) | **NO** | `StreamingTokenTracker:98` hashes lines into `_seen_snapshot_hashes`; deduplicates snapshots | Deduplication prevents phantom inflation; execution continues safely |
| **S-06** | Retry after uncertain completion | **YES** (Provider retries internally) | **NO** | **NO** | If retry occurs within Codex, `StreamingTokenTracker:319` sums turn deltas; if crashed, `is_budget_uncertain = True` | Halts on unverified state; zero cascade |
| **S-07** | Cancellation after request submission | **YES** (Remote server finishes) | **NO** | **NO** | `terminate_pid_tree` destroys child PID tree; summary file omitted; coordinator aborts | Fail-closed; coordinator requires complete valid summary |
| **S-08** | Orchestrator process crash (OOM / SIGKILL) | **YES** (In-flight memory lost) | **NO** | **NO** | No summary JSON written; coordinator Step 2 `Test-Path` fails (Exit 101) | Invocation 2 strictly blocked from starting |
| **S-09** | Coordinator restart / reboot | **NO** | **NO** | **NO** | Coordinator state is ephemeral; rebooting requires manual human restart from Task A | Safe; requires full human re-initialization |
| **S-10** | Stale persisted aggregate counters | **NO** | **NO** | **NO** | Coordinator checks `run_timestamp > invocation_start_time`; rejects stale files (Exit 106) | Stale files rejected; aborts run |
| **S-11** | Multiple simultaneous harness processes | Potential Race | Dependent | Potential Race | Harness relies on dedicated output directories; running concurrent instances on same directory causes file contention | **Operational Constraint:** Benchmark must execute as a single sequential coordinator script |
| **S-12** | Cross-process lock / lease failure | **NO** | **NO** | **NO** | Dynamic coordinator runs sequentially in one process tree (Task A exits before Task B begins) | No concurrent process locks required in sequential model |
| **S-13** | Duplicate arm dispatch | **NO** | **NO** | **NO** | `orchestrator.py:2747` iterates `order` list strictly once; deduplication invariant proven | Invariant loop over shuffled array |
| **S-14** | Incomplete telemetry (`telemetry_valid = False`)| **YES** | **NO** | **NO** | `orchestrator.py:2820` immediately sets `block_status = ABORTED_UNCERTAIN_BUDGET`; skips remaining arms | Fail-closed halt; skips all downstream arms |

---

## 3. Forensic Assessment of Prior Crash-Safety Claims

In prior reports (e.g. Phase 5.9.0.3), it was claimed that "11 crash scenarios were verified crash-safe."
An adversarial inspection of the test suite (`benchmarks/matched-pair-harness/tests/test_harness_safety_audit.py` and `test_pentad_harness.py`) reveals:
1. **What is Directly Evidenced by Tests:**
   - Unit tests explicitly verify that corrupted JSON lines, negative token counts, missing telemetry payloads, and `telemetry_valid = False` cause `StreamingTokenTracker` to fail closed and set `is_budget_uncertain = True`.
   - Unit tests verify that `orchestrator.py` halts on `is_budget_uncertain = True` and sets `block_status = ABORTED_UNCERTAIN_BUDGET`.
2. **What is an Operational Boundary (Not an Offline Automated Test):**
   - True OS-level hard SIGKILL / power loss during a network socket transmission cannot be simulated offline without mocking OS process termination.
   - However, the architectural fail-closed mechanism is **structurally guaranteed**: because Invocation 2 cannot launch without a valid, uncorrupted `task_a_summary.json` file carrying `global_budget_uncertain: false`, an ungraceful crash in Invocation 1 guarantees that Invocation 2 will never be dispatched.

---

## 4. Concurrency & Concurrency-Control Invariants

1. **Sequential Single-Process Dispatch:** The benchmark architecture is strictly sequential. Task A and Task B never run concurrently. Task A must exit cleanly with code 0 and emit a valid summary before the coordinator invokes Task B.
2. **Sequential Single-Thread Subprocess Turns:** Inside each arm, turns are serialized. Exactly one HTTP request is active at any time.
3. **Fail-Closed Default Rule:** If actual consumption cannot be definitively established from disk artifacts, **Invocation 2 does not launch**.
