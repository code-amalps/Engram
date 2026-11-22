# Engram Phase 5.8: Test Evidence Reconciliation Report

**Date:** 2026-11-22  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Auditor:** Adversarial Forensic Audit Team  
**Audit Classification:** `TEST_EVIDENCE_RECONCILED_WITH_REGRESSION_DEFECT`

---

## 1. Executive Summary

Previous reports for Phase 5.5 and Phase 5.7 asserted that:
1. "87/87 Rust tests passed across unit and optimization suites."
2. "1/1 Python tetrad harness suite passed."
3. Candidate 5 was fully verified without regressions.

This forensic audit conducted a complete, independent, offline execution and reconciliation of the test claims against the actual Engram codebase.

### Key Forensic Findings:
1. **Scope Clarification on "87/87 Tests":**  
   The reported 87 tests were not the complete test suite of the repository. They represented a **targeted sub-suite** consisting of:
   - `cargo test --lib` (24 passed)
   - `cargo test --bin engram` (35 passed)
   - `cargo test --test ast_test` (13 passed)
   - `cargo test --test benchmark_harness_test` (10 passed)
   - `cargo test --test token_efficiency_test` (5 passed)  
   Total: **87 passed, 0 failed**. When re-executed offline during Phase 5.8, these exact 87 tests pass 100%.

2. **Full Repository Test Suite Reality:**  
   The Engram repository actually contains **50 integration test files** in `tests/`, along with doc tests, library tests, and binary tests, comprising over 300 test cases.
   When the full offline suite (`cargo test --offline`) was executed across all targets:
   - 49 test suites passed completely.
   - **1 test suite failed:** `tests/query_router_test.rs`.
   - Specifically, test case `test_15_missing_or_invalidated_entities` failed with an unhandled assertion panic.

3. **Python Tetrad Harness Test:**  
   `benchmarks/matched-pair-harness/tests/test_tetrad_harness.py` executed successfully offline via `pytest` (finished in 257.44s, 1 passed).

---

## 2. Test Scope Reconciliation Matrix

| Test Suite / Target | Previous Claim (Phases 5.5/5.7) | Phase 5.8 Independent Execution | Result | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `src/lib.rs` (unit) | 24 passed | 24 passed | **VERIFIED** | Unit tests for parser, AST, semantic index |
| `src/main.rs` (unit) | 35 passed | 35 passed | **VERIFIED** | CLI dispatch, MCP handlers, doctor |
| `tests/ast_test.rs` | 13 passed | 13 passed | **VERIFIED** | AST parsing across Rust, TS, Python, Go |
| `tests/benchmark_harness_test.rs` | 10 passed | 10 passed | **VERIFIED** | Harness contract validation |
| `tests/token_efficiency_test.rs` | 5 passed | 5 passed | **VERIFIED** | Phase 5.5 synthetic reduction checks |
| **Subtotal (Targeted)** | **87 passed** | **87 passed** | **MATCH** | Reconciles historical claim |
| `tests/query_router_test.rs` | Not reported | **14 passed, 1 failed** | **REGRESSION** | `test_15_missing_or_invalidated_entities` |
| Other 48 integration test files | Not reported | All passed | **VERIFIED** | Cross-engine integration suites |
| `test_tetrad_harness.py` | 1 passed | 1 passed (257.44s) | **VERIFIED** | End-to-end mock tetrad execution |

---

## 3. Forensic Analysis of Regression: `query_router_test.rs`

### 3.1 Failure Diagnostic
```
---- test_15_missing_or_invalidated_entities stdout ----
thread 'test_15_missing_or_invalidated_entities' panicked at tests\query_router_test.rs:555:5:
assertion failed: !res2.files.iter().any(|f| f.path.contains("temp_service.rs"))
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

failures:
    test_15_missing_or_invalidated_entities

test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.36s
```

### 3.2 Root Cause Mechanism: Optimization G Cache Invalidation Defect
In Phase 5.5, Optimization G introduced an in-memory LRU cache (`COMPILED_CONTEXT_CACHE`) in `src/compiler.rs`:
```rust
let cache_key = format!(
    "{}:{}:{}:{}:{}:{}",
    self.workspace_root.display(),
    query,
    format!("{:?}", mode),
    token_budget,
    target_path.as_deref().unwrap_or(""),
    head_commit
);
```

When `test_15_missing_or_invalidated_entities` executes:
1. It queries the context for `temp_service.rs`. The compiled context is computed and cached in `COMPILED_CONTEXT_CACHE`.
2. The test physically deletes `temp_service.rs` and calls the index updater.
3. The test queries the context a second time (`res2`). Because `workspace_root`, `query`, `mode`, `token_budget`, and `head_commit` (which is `"unknown"` in non-git test temp directories) remain identical, Optimization G serves the result directly from memory!
4. `res2` still contains `temp_service.rs`, violating the invariant that deleted or invalidated entities must be evicted.
5. The assertion `assert!(!res2.files.iter().any(|f| f.path.contains("temp_service.rs")))` fails.

### 3.3 Impact Classification
- **Classification:** `DEFECT-5.8-01`
- **Severity:** High (Functional correctness regression in context compiler).
- **Remediation:** Incorporate database SQLite index mtime or a sequence epoch counter into `COMPILED_CONTEXT_CACHE` key, or register a cache eviction hook upon database invalidation.

---

## 4. Audit Conclusion

The historical claim that "87/87 tests passed" was factually true for the specific 5 targets executed during Phase 5.5. However, it created a false impression of full codebase stability. A comprehensive test run reveals an active regression introduced by Optimization G. Candidate 5 cannot be certified as regression-free until `DEFECT-5.8-01` is remediated.
