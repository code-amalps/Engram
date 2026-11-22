# Offline Regression Test Results: Milestone 5C

**Milestone:** Engram Amendment 05 — Milestone 5C  
**Document ID:** `amendments/draft-05/milestone-5c/offline-regression-test-results.md`  
**Evaluation Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Total Tests Executed:** 77  
**Total Passed:** 77  
**Total Failed:** 0  
**Status:** 100% PASS RATE ACROSS ALL ACTIVE SUITES  

---

## 1. Executive Summary

This report documents the comprehensive offline test execution verifying the evaluator remediation, binary selection integrity, task acceptance checks, and mutual acceptance rules.

Four independent test suites were executed sequentially within the local environment:
1. `test_milestone_5c_remediation.py`: 25 tests (remediation, task acceptance, binary validation, mutual acceptance).
2. `test_milestone_5b_remediation.py`: 20 tests (Cargo manifests, streaming token tracker, headroom reserve).
3. `test_task_b_evaluator.py`: 23 tests (Task B evaluator regression checks).
4. `test_amendment_04_two_arm.py`: 9 tests (two-arm protocol, stop-on-failure, comparative math).

All 77 tests passed cleanly with zero live inference, zero network requests, and zero model endpoint calls.

---

## 2. Test Execution Breakdown

### 2.1 Suite 1: `test_milestone_5c_remediation.py` (25 Tests)
**Path:** `benchmarks/matched-pair-harness/tests/test_milestone_5c_remediation.py`  
**Execution Time:** 0.612s  
**Result:** 25/25 PASSED  

| Test Class | Test Name | Invariant Verified | Outcome |
|---|---|---|---|
| `TestBinarySelectionIntegrity` | `test_correct_candidate_hash_passes` | Candidate hash `F56347EF...` validates candidate binary | **PASS** |
| `TestBinarySelectionIntegrity` | `test_incorrect_expected_hash_fails_closed` | Corrupted hash raises `IsolationError` | **PASS** |
| `TestBinarySelectionIntegrity` | `test_missing_executable_fails_closed` | Non-existent binary raises `IsolationError` | **PASS** |
| `TestBinarySelectionIntegrity` | `test_wrong_binary_path_fails_closed` | Wrong executable rejected by hash validation | **PASS** |
| `TestBinarySelectionIntegrity` | `test_accidental_frozen_v154_selection_fails_closed` | Frozen v1.5.4 (`CF807EAC...`) fails closed against candidate hash | **PASS** |
| `TestBinarySelectionIntegrity` | `test_orchestrator_default_binary_mapping` | `DEFAULT_ENGRAM_BIN` resolves to absolute `target/release/engram.exe` | **PASS** |
| `TestCommitCacheEvaluatorRequirements` | `test_req1_sqlite_meta_table_positive` | Physical SQLite `index.db` with meta table and `git_commit_head` passes REQ-1 | **PASS** |
| `TestCommitCacheEvaluatorRequirements` | `test_req1_source_pattern_positive` | Source code with meta table creation and `git_commit_head` passes REQ-1 | **PASS** |
| `TestCommitCacheEvaluatorRequirements` | `test_req1_missing_meta_negative` | Missing meta table returns `False` | **PASS** |
| `TestCommitCacheEvaluatorRequirements` | `test_req1_malformed_meta_negative` | SQLite meta table lacking `git_commit_head` returns `False` | **PASS** |
| `TestCommitCacheEvaluatorRequirements` | `test_req2_drift_detection_positive` | `ensure_fresh` inspecting commit divergence passes REQ-2 | **PASS** |
| `TestCommitCacheEvaluatorRequirements` | `test_req2_missing_drift_negative` | `ensure_fresh` without commit comparison returns `False` | **PASS** |
| `TestCommitCacheEvaluatorRequirements` | `test_req3_clean_subcommand_source_positive` | `main.rs` defining `Clean { cache: bool }` unlinking cache passes REQ-3 | **PASS** |
| `TestCommitCacheEvaluatorRequirements` | `test_req3_clean_missing_cache_flag_negative` | `Clean` subcommand lacking `--cache` flag returns `False` | **PASS** |
| `TestCommitCacheEvaluatorRequirements` | `test_req3_clean_deletes_memories_negative` | Subcommand that deletes `memories.json` fails safety check | **PASS** |
| `TestCommitCacheEvaluatorRequirements` | `test_req4_boundary_adherence_positive` | Changes confined to allowed files pass REQ-4 (`AUTOMATED_DIFF`) | **PASS** |
| `TestCommitCacheEvaluatorRequirements` | `test_req4_unauthorized_files_negative` | Changes touching `src/mcp.rs` fail REQ-4 | **PASS** |
| `TestCommitCacheEvaluatorRequirements` | `test_req4_dependency_mutation_negative` | `Cargo.toml` modification forces $C = 0.0$ and fails all requirements | **PASS** |
| `TestCommitCacheEvaluatorRequirements` | `test_full_acceptance_valid_fixture_achieves_c10` | Valid implementation satisfying all 4 requirements achieves $C = 1.0$ | **PASS** |
| `TestCommitCacheEvaluatorRequirements` | `test_invalid_fixture_fails_acceptance` | Defective implementation yields $C = 0.75$, `passed_all_requirements=False` | **PASS** |
| `TestMutualAcceptanceIntegrity` | `test_failed_baseline_suppresses_savings` | Baseline failure ($C < 1.0$) returns `CONTROL_FAILED_ACCEPTANCE` | **PASS** |
| `TestMutualAcceptanceIntegrity` | `test_failed_treatment_suppresses_savings` | Treatment failure ($C < 1.0$) returns `TREATMENT_FAILED_ACCEPTANCE` | **PASS** |
| `TestMutualAcceptanceIntegrity` | `test_mutual_acceptance_failure_suppresses_savings` | Dual failure returns `MUTUAL_ACCEPTANCE_FAILURE` | **PASS** |
| `TestMutualAcceptanceIntegrity` | `test_missing_telemetry_suppresses_savings` | Invalid telemetry invalidates savings claim even if $C = 1.0$ | **PASS** |
| `TestMutualAcceptanceIntegrity` | `test_full_acceptance_computes_exact_savings` | Dual $C = 1.0$ with valid telemetry computes exact 40.0% savings | **PASS** |

---

### 2.2 Suite 2: `test_milestone_5b_remediation.py` (20 Tests)
**Path:** `benchmarks/matched-pair-harness/tests/test_milestone_5b_remediation.py`  
**Execution Time:** 4.744s  
**Result:** 20/20 PASSED  
- Verified Cargo manifest generation (`compute_cargo_manifest`).
- Verified Cargo dependency integrity checking under lockfile drift.
- Verified sandbox cleanliness and worktree teardown safety for Cargo tasks.
- Verified token headroom reserve (45,000 tokens), stop threshold ($130,000$), and token deduplication.

---

### 2.3 Suite 3: `test_task_b_evaluator.py` (23 Tests)
**Path:** `benchmarks/matched-pair-harness/tests/test_task_b_evaluator.py`  
**Execution Time:** 0.401s  
**Result:** 23/23 PASSED  
- Verified that Task B static checks remain fully operational and unaffected by Milestone 5C changes.
- Tested HTML comments, UI layouts, local storage security, and reducer state transitions.

---

### 2.4 Suite 4: `test_amendment_04_two_arm.py` (9 Tests)
**Path:** `benchmarks/matched-pair-harness/tests/test_amendment_04_two_arm.py`  
**Execution Time:** 9.826s  
**Result:** 9/9 PASSED  
- Verified Alternative B two-arm selection (`v154_arm3` vs `baseline_arm1`).
- Verified stop-on-failure behavior across matched pairs.
- Verified token ceiling propagation and baseline isolation.

---

## 3. Test Status Classification

| Category | Count | Status | Notes |
|---|---|---|---|
| **Tests Executed** | 77 | **PASSED** | 100% pass rate across 4 independent test files |
| **Tests Skipped** | 0 | — | No tests were skipped |
| **Tests Blocked** | 0 | — | All environment prerequisites met locally |
