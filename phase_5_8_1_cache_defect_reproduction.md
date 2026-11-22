# Engram Phase 5.8.1: Cache Defect Reproduction Report

**Date:** 2026-11-22  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Defect Identifier:** `DEFECT-5.8-01`  
**Affected Component:** In-Memory Compiler LRU Cache (`COMPILED_CONTEXT_CACHE`) in `src/compiler.rs`  
**Reproduced Test:** `tests/query_router_test.rs::test_15_missing_or_invalidated_entities`  
**Execution Mode:** Strictly Offline Reproduction  

---

## 1. Reproduction Execution & Evidence

Before applying any code modifications, the defect was reproduced in isolation using the existing working tree source:

### Command Executed:
```bash
cargo test --manifest-path "C:\Users\Amal P S\OneDrive\Projects\Engram\Cargo.toml" --test query_router_test test_15_missing_or_invalidated_entities --offline
```

### Execution Diagnostics:
- **Exit Code:** `1` (Test failure)
- **Duration:** `0.82s`
- **Filtered Out:** 14 tests
- **Failed:** 1 test (`test_15_missing_or_invalidated_entities`)

### Captured Failure Output:
```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.29s
     Running tests\query_router_test.rs (C:\Users\Amal P S\OneDrive\Projects\Engram\target\debug\deps\query_router_test-71f8fce3c4a74040.exe)

running 1 test
test test_15_missing_or_invalidated_entities ... FAILED

failures:

---- test_15_missing_or_invalidated_entities stdout ----

thread 'test_15_missing_or_invalidated_entities' (19684) panicked at tests\query_router_test.rs:555:5:
assertion failed: !res2.files.iter().any(|f| f.path.contains("temp_service.rs"))
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

failures:
    test_15_missing_or_invalidated_entities

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.82s

error: test failed, to rerun pass `--test query_router_test`
```

---

## 2. Root Cause Forensic Analysis

A complete inspection of `src/compiler.rs`, `src/analyzer.rs`, and `tests/query_router_test.rs` reveals the precise architectural breakdown:

### 2.1 The In-Memory Cache Implementation in `src/compiler.rs`
Optimization G introduced `COMPILED_CONTEXT_CACHE` to avoid repeated AST traversal and database retrieval for identical query requests:
```rust
static COMPILED_CONTEXT_CACHE: OnceLock<Mutex<HashMap<String, (String, ContextResult)>>> = OnceLock::new();
```

Inside `compile_context_internal` (`src/compiler.rs` lines 160–192):
```rust
ensure_fresh(&root)?;
let conn = db(&root)?;

let head_rev: String = conn
    .query_row(
        "SELECT COALESCE(value, 'initial') FROM metadata WHERE key='head_commit'",
        [],
        |r| r.get(0),
    )
    .unwrap_or_else(|_| "initial".into());

let mem_ver = std::fs::metadata(root.join(".engram").join("memory.yaml"))
    .and_then(|m| m.modified())
    .map(|t| format!("{:?}", t))
    .unwrap_or_else(|_| "none".into());

let cache_key = format!(
    "{}:{}:{}:{}:{}:{:?}:{:?}:{}",
    root.display(), task, budget, subpath.unwrap_or(""), language.unwrap_or(""), detail_level, target_symbols, mem_ver
);

let cache = COMPILED_CONTEXT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
if let Ok(guard) = cache.lock() {
    if let Some((rev, res)) = guard.get(&cache_key) {
        if rev == &head_rev {
            return Ok(res.clone());
        }
    }
}
```

### 2.2 Forensic Flaws Identified:

1. **Database Key Mismatch:**
   - In `src/analyzer.rs` (line 2207), `analyze()` records the current revision into metadata using the key `'head_revision'`:
     `INSERT INTO metadata(key, value) VALUES('head_revision', ?1) ...`
   - In `src/compiler.rs` (line 164), the cache checks metadata for the key `'head_commit'`. Because `'head_commit'` is never written by `analyze()`, `head_rev` always defaults to `"initial"`.

2. **Non-Git Workspace Revision Invariance:**
   - In non-git workspaces (such as test workspaces created in temporary directories), `revision(&root)` always returns `"initial"`.
   - Even if `'head_revision'` were queried, its value would remain `"initial"` both before and after file changes.

3. **Complete Absence of Database State / Generation Fingerprinting:**
   - When files are created, modified, or deleted, `analyze(&ws)` updates SQLite tables (`files`, `entities`, `entity_versions`, `relationships`).
   - However, the `cache_key` in `src/compiler.rs` contains no SQLite generation counter, transaction counter, or index modification fingerprint.

4. **Dormant `clear_context_cache()` Function:**
   - `src/compiler.rs` exports `pub fn clear_context_cache()`, but it is **never called** in any code path—neither after `analyze()`, nor after `ensure_fresh()`, nor inside test teardowns or file mutations.

### 2.3 The Failure Sequence in `test_15_missing_or_invalidated_entities`
1. Test creates `temp_service.rs` in workspace `ws`.
2. First call: `context(&ws, "TempService", 2000)` executes `compile_context_internal`.
   - Index is built, finding `temp_service.rs`.
   - Result is stored in `COMPILED_CONTEXT_CACHE` under `cache_key` paired with `head_rev = "initial"`.
3. Test deletes `temp_service.rs` and calls `analyze(&ws)`:
   - `analyze` removes the file from `files` and invalidates entities in `entities`.
4. Second call: `context(&ws, "TempService", 2000)` executes `compile_context_internal`:
   - `ensure_fresh(&root)` verifies database is synchronized with disk.
   - `head_rev` is evaluated as `"initial"`.
   - Cache lookup succeeds: `cache_key` matches and `rev ("initial") == head_rev ("initial")`.
   - **Stale context is returned immediately without consulting the database.**
5. Assertion `assert!(!res2.files.iter().any(|f| f.path.contains("temp_service.rs")))` fails.

---

## 3. Reproduction Conclusion

`DEFECT-5.8-01` is an active, deterministic regression in `src/compiler.rs`. The cache fails to detect database and filesystem invalidations in non-Git workspaces and across subsequent `analyze()` runs. A comprehensive invalidation design is required.
