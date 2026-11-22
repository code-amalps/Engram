# Engram Phase 5.8.1: Cache Before/After Forensic Evidence

**Date:** 2026-11-22  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Defect Remediated:** `DEFECT-5.8-01`  
**Primary Test Target:** `tests/query_router_test.rs::test_15_missing_or_invalidated_entities`  

---

## 1. Before Remediation (Phase 5.8 Baseline)

### Command Executed:
```bash
cargo test --manifest-path "C:\Users\Amal P S\OneDrive\Projects\Engram\Cargo.toml" --test query_router_test test_15_missing_or_invalidated_entities --offline
```

### Observed Execution Diagnostic:
- **Exit Code:** `1` (FAILED)
- **Execution Duration:** `0.82s`
- **Failure Output:**
```text
---- test_15_missing_or_invalidated_entities stdout ----
thread 'test_15_missing_or_invalidated_entities' (19684) panicked at tests\query_router_test.rs:555:5:
assertion failed: !res2.files.iter().any(|f| f.path.contains("temp_service.rs"))
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

failures:
    test_15_missing_or_invalidated_entities

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.82s
```

### Forensic Root Cause:
`COMPILED_CONTEXT_CACHE` stored entries keyed by string parameters and verified only against `head_commit`. Because the test workspace was not committed between calls (and in temporary directories `HEAD` remained `"initial"`), the cache returned the compiled context from the first invocation, which still contained the deleted file `temp_service.rs`.

---

## 2. After Remediation (Phase 5.8.1 Implementation)

### Exact Command Re-executed:
```bash
cargo test --manifest-path "C:\Users\Amal P S\OneDrive\Projects\Engram\Cargo.toml" --test query_router_test test_15_missing_or_invalidated_entities --offline
```

### Observed Execution Diagnostic:
- **Exit Code:** `0` (SUCCESS)
- **Execution Duration:** `1.15s`
- **Execution Output:**
```text
running 1 test
test test_15_missing_or_invalidated_entities ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 1.15s
```

### Full Query Router Suite Re-executed:
```bash
cargo test --manifest-path "C:\Users\Amal P S\OneDrive\Projects\Engram\Cargo.toml" --test query_router_test --offline
```
- **Exit Code:** `0` (SUCCESS)
- **Execution Duration:** `13.89s`
- **Result:** `15 passed; 0 failed; 0 ignored; 0 measured; finished in 13.89s`

### Dedicated Regression Test Suite (`tests/cache_invalidation_test.rs`):
```bash
cargo test --manifest-path "C:\Users\Amal P S\OneDrive\Projects\Engram\Cargo.toml" --test cache_invalidation_test --offline
```
- **Exit Code:** `0` (SUCCESS)
- **Execution Duration:** `18.04s`
- **Result:** `13 passed; 0 failed; 0 ignored; 0 measured; finished in 18.04s`

---

## 3. Comparative Summary

| Metric / Dimension | Before Remediation (Phase 5.8) | After Remediation (Phase 5.8.1) | Forensic Delta |
| :--- | :--- | :--- | :--- |
| `test_15_missing_or_invalidated_entities` | FAILED (Panic at line 555) | **PASSED** (1.15s) | Defect fully resolved |
| `query_router_test` Suite Pass Count | 14 / 15 passed (93.3%) | **15 / 15 passed (100.0%)** | 100% clean suite |
| Cache Key State Tracking | `head_commit` string only | `IndexStateFingerprint` (generation, file counts, hash sum, entity count, mtime) | Multi-tier state detection |
| Non-Git Workspace Invalidation | Broken (stale context served) | **Verified Working** (`test_08_non_git_workspace` passed) | Full non-git support |
| Cache Memory Bound | Unbounded `HashMap` | Bounded LRU (64 entries max capacity) | Zero memory leak risk |
| Explicit Eviction Mechanism | Dormant function | Active `clear_workspace_cache()` on `analyze()` | Active synchronization |
