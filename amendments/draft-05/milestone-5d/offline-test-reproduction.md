# Offline Test Reproduction Report: Milestone 5D

**Milestone:** Engram Amendment 05 — Milestone 5D  
**Document ID:** `amendments/draft-05/milestone-5d/offline-test-reproduction.md`  
**Execution Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Environment:** Windows (PowerShell, Python 3.14.0)  
**Total Tests Executed:** 77  
**Total Passed:** 77  
**Total Failed:** 0  
**Status:** 100% REPRODUCIBILITY CONFIRMED  

---

## 1. Executive Summary

As required by Milestone 5D, an independent execution of the offline test suite was performed to reproduce the reported Milestone 5C test results.

The test run was executed via a single unified `unittest` invocation covering all four active test suites in `benchmarks/matched-pair-harness/tests/`. All 77 unit and integration tests passed cleanly in 12.146 seconds with zero failures, zero errors, and zero skipped tests.

---

## 2. Reproduction Command & Execution Summary

### 2.1 Invocation Command
```powershell
Set-Location "C:\Users\Amal P S\OneDrive\Projects\Engram"
python -m unittest `
  benchmarks/matched-pair-harness/tests/test_milestone_5c_remediation.py `
  benchmarks/matched-pair-harness/tests/test_milestone_5b_remediation.py `
  benchmarks/matched-pair-harness/tests/test_task_b_evaluator.py `
  benchmarks/matched-pair-harness/tests/test_amendment_04_two_arm.py
```

### 2.2 Execution Metrics
- **Runtime:** 12.146 seconds
- **Exit Code:** 0
- **Suites Executed:** 4
- **Tests Evaluated:** 77
- **Tests Passed:** 77 (100%)
- **Tests Failed:** 0
- **Tests Skipped:** 0
- **Environmental Failures:** 0

---

## 3. Detailed Results by Test Suite

### 3.1 Suite 1: `test_milestone_5c_remediation.py` (25 Tests)
- **Binary Selection Integrity (6 tests):** Valid candidate hash passes; corrupted hash fails closed; missing binary fails closed; wrong binary path fails closed; frozen v1.5.4 substitution fails closed; default binary mapping verified.
- **Task Acceptance Evaluator (14 tests):** Physical SQLite metadata passes REQ-1; source AST pattern passes REQ-1; missing meta table fails REQ-1; malformed meta table fails REQ-1; `ensure_fresh` commit comparison passes REQ-2; missing drift fails REQ-2; `main.rs` `Clean { cache: bool }` passes REQ-3; missing cache flag fails REQ-3; deleting `memories.json` fails safety check; clean impact passes REQ-4; unauthorized files fail REQ-4; Cargo drift forces $C = 0.0$; valid implementation achieves $C = 1.00$; defective implementation achieves $C = 0.75$.
- **Mutual Acceptance Invariants (5 tests):** Baseline failure suppresses savings; treatment failure suppresses savings; dual failure suppresses savings; invalid telemetry suppresses savings; full acceptance computes exact 40.0% savings.

### 3.2 Suite 2: `test_milestone_5b_remediation.py` (20 Tests)
- **Cargo Isolation & Manifests (8 tests):** Manifest generation for `Cargo.toml`/`Cargo.lock`; drift detection on modified lockfiles; worktree cleanliness assertion; teardown safety guards.
- **Streaming Token Tracker & Guard (8 tests):** 45k headroom reserve initialization; pre-dispatch stop at $\ge 130,000$ tokens; boundary equality behavior; worst-case turn delta containment; streaming event deduplication; malformed telemetry fail-safe handling.
- **Orchestrator Cargo Configuration (4 tests):** Cargo repository auto-detection; custom ceiling and reserve parameterization.

### 3.3 Suite 3: `test_task_b_evaluator.py` (23 Tests)
- **Task B Evaluator Invariants (23 tests):** HTML comment stripping; UI checkbox verification; localStorage security negative checks; reducer state transitions. Zero regressions observed.

### 3.4 Suite 4: `test_amendment_04_two_arm.py` (9 Tests)
- **Protocol & Comparative Math (9 tests):** Two-arm execution ordering (`v154_arm3` vs `baseline_arm1`); stop-on-failure downstream abort; token ceiling propagation; baseline unassisted isolation; schema validation.

---

## 4. Test Reproduction Verdict

The 77/77 test result reported in Milestone 5C has been independently reproduced and verified in its entirety. Zero discrepancies or flaky test behaviors were detected.
