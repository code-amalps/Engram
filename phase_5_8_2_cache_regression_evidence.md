# Engram Phase 5.8.2: Cache Regression Evidence & Defect Verification

**Audit Phase:** 5.8.2  
**Target Defect:** `DEFECT-5.8-01` (Stale context cache bug in `src/compiler.rs`)  
**Target Tests:** `tests/query_router_test.rs::test_15_missing_or_invalidated_entities` and `tests/cache_invalidation_test.rs`  
**Execution Mode:** STRICTLY OFFLINE RE-VERIFICATION  

---

## 1. Defect Reproduction vs Remediation Comparison

### 1.1 Phase 5.8 Defect State (Prior to Fix)
In Phase 5.8, `cargo test --test query_router_test test_15_missing_or_invalidated_entities` failed with:
```
thread 'test_15_missing_or_invalidated_entities' panicked at tests\query_router_test.rs:555:5:
assertion failed: !res2.files.iter().any(|f| f.path.contains("temp_service.rs"))
```
The test indexed a file `temp_service.rs`, queried context (populating the LRU cache), deleted the entity, and queried again. Because the cache key was only the canonical root and `head_commit` (which was uncommitted / `"initial"`), the compiler returned the cached context containing `temp_service.rs`, violating the deletion invariant.

### 1.2 Phase 5.8.2 Audit Verification (Post-Remediation)
Inspection of `tests/query_router_test.rs` lines 545-560 confirms that line 555 has **not been modified, weakened, or removed**:

```rust
    // Second query: entity was invalidated, should NOT be returned
    let res2 = query_context(
        &temp_dir,
        "find temp service",
        8000,
        None,
        None,
        None,
    ).expect("query failed");

    assert!(
        !res2.files.iter().any(|f| f.path.contains("temp_service.rs")),
        "Deleted entity should not be returned in query results"
    );
```

### 1.3 Fresh Execution Results (Task-61745)
Command:
```powershell
cargo test --test query_router_test test_15_missing_or_invalidated_entities --offline
```
Output:
```
running 1 test
test test_15_missing_or_invalidated_entities ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.15s
```

---

## 2. Dedicated Regression Suite: `tests/cache_invalidation_test.rs`

Fresh execution in `task-61745` confirmed all 13 comprehensive edge cases pass without error:

```
running 13 tests
test test_01_cache_hit_when_nothing_changed ... ok
test test_02_cache_miss_after_entity_modified ... ok
test test_03_cache_miss_after_entity_deleted ... ok
test test_04_cache_miss_after_file_renamed ... ok
test test_05_cache_miss_after_new_file_added ... ok
test test_06_cache_invalidation_wal_mode ... ok
test test_07_cache_invalidation_db_replacement ... ok
test test_08_non_git_workspace ... ok
test test_09_workspace_isolation ... ok
test test_10_index_generation_change ... ok
test test_11_cache_eviction_at_capacity ... ok
test test_12_concurrent_reads_and_invalidation ... ok
test test_13_failed_invalidation_fallback ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 25.09s
```

---

## 3. Forensic Conclusion

`DEFECT-5.8-01` is definitively resolved. The fix is robust across entity deletions, file modifications, file renames, database replacements, non-Git workspaces, and multi-threaded queries.
