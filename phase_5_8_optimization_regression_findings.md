# Phase 5.8: Optimization Regression & Defect Findings

**Document ID:** `ENGRAM-PHASE-5.8-REGRESSION-FINDINGS`  
**Date:** 2026-11-22  
**Finding ID:** `DEFECT-5.8-01`  
**Classification:** HIGH SEVERITY / CORRECTNESS REGRESSION  
**Component:** `src/compiler.rs` (`COMPILED_CONTEXT_CACHE`)  
**Trigger Test:** `tests/query_router_test.rs::test_15_missing_or_invalidated_entities`  

---

## 1. Defect Description & Failure Trace

During the full offline test suite execution across all 50 integration test suites in `tests/`, `tests/query_router_test.rs` failed with exit code 101.

### Failure Trace:
```
running 15 tests
test test_02_angular_ngrx_adapter_discovery ... ok
test test_03_exact_symbol_matching ... ok
test test_04_task_terminology_matching ... ok
...
test test_14_go_and_java_compatibility ... ok
test test_15_missing_or_invalidated_entities ... FAILED

---- test_15_missing_or_invalidated_entities stdout ----
thread 'test_15_missing_or_invalidated_entities' panicked at tests/query_router_test.rs:555:5:
assertion failed: !res2.files.iter().any(|f| f.path.contains("temp_service.rs"))
```

---

## 2. Root Cause Analysis

In `tests/query_router_test.rs:536-558`:
1. The test creates a test workspace with `src/temp_service.rs`.
2. It executes `analyze(&ws)` and `context(&ws, "TempService", 2000)`.
3. It deletes `temp_service.rs` and calls `analyze(&ws)`.
4. It calls `context(&ws, "TempService", 2000)` again, expecting `temp_service.rs` to NOT be returned.
5. However, `context(&ws, "TempService", 2000)` hits Optimization G's in-memory LRU cache (`COMPILED_CONTEXT_CACHE`).
6. The cache key formulation in `src/compiler.rs:27-38`:
   ```rust
   let cache_key = format!(
       "{}:{}:{}:{}:{}:{:?}:{:?}:{}",
       root.display(),
       task,
       budget,
       subpath.unwrap_or(""),
       language.unwrap_or(""),
       detail_level,
       target_symbols,
       mem_ver
   );
   ```
7. Notice that:
   - `root`, `task`, `budget`, `subpath`, `language`, `detail_level`, `target_symbols` are identical.
   - `mem_ver` is the modification timestamp of `.engram/memory.yaml`, which was never touched.
   - The test workspace is NOT a Git repository, so no git commit changed.
   - The cache key was identical, so the cache returned the cached context from step 2, which still contained `temp_service.rs`!

---

## 3. Why This Was Not Caught in Phase 5.5

Phase 5.5 reported: "Zero regressions across 87/87 tests."
Forensic investigation confirms that Phase 5.5 ran only the 5 targeted suites:
- `src/lib.rs` (24 unit tests)
- `src/main.rs` (35 unit tests)
- `tests/ast_extraction_test.rs` (13 tests)
- `tests/bench_test.rs` (10 tests)
- `tests/token_efficiency_test.rs` (5 tests)
Total: 24 + 35 + 13 + 10 + 5 = 87 tests.
The broader repository test suite (50 test files containing 300+ integration tests), including `tests/query_router_test.rs`, was NOT run during Phase 5.5.

---

## 4. Formal Remediation Plan (To be executed in a dedicated phase)

In accordance with Section 2.3 of the Phase 5.8 directive ("If a defect is found, document it and propose a separate remediation phase. Do not silently repair it during this forensic certification"):

### Proposed Fix:
In `src/compiler.rs`, incorporate the SQLite database mtime (`.engram/cache.db`) or index generation ID into the cache key:
```rust
let db_mtime = fs::metadata(root.join(".engram").join("cache.db"))
    .and_then(|m| m.modified())
    .ok()
    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
    .map(|d| d.as_secs())
    .unwrap_or(0);

let cache_key = format!(
    "{}:{}:{}:{}:{}:{:?}:{:?}:{}:{}",
    root.display(),
    task,
    budget,
    subpath.unwrap_or(""),
    language.unwrap_or(""),
    detail_level,
    target_symbols,
    mem_ver,
    db_mtime
);
```
Additionally, `analyze()` or index modification operations should explicitly call `clear_context_cache()`.
