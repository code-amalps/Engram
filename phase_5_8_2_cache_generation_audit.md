# Engram Phase 5.8.2: Cache Generation and Transactional Integrity Audit

**Audit Phase:** 5.8.2  
**Audit Target:** `src/storage.rs`, `src/analyzer.rs`, and SQLite metadata transaction semantics.  
**Execution Mode:** STRICTLY OFFLINE, STATIC & DYNAMIC FORENSIC AUDIT  

---

## 1. Executive Summary

Phase 5.8.2 conducted an in-depth audit of the database generation counter mechanism introduced in Phase 5.8.1 to resolve `DEFECT-5.8-01`.

The audit evaluated whether:
1. Every path that mutates indexed entities, files, or relationships updates the `index_generation`.
2. Generation updates are atomic with their corresponding database mutations.
3. Transaction rollback prevents generation counter desynchronization.
4. Older pre-existing databases are initialized safely.

**Verdict:** `TRANSACTIONAL_GENERATION_VERIFIED_CORRECT`

---

## 2. Detailed Mutation Path Inspection

| Mutation Path | Entry Point | Transaction Boundary | Generation Update Mechanism | Atomicity & Invalidation |
| :--- | :--- | :--- | :--- | :--- |
| **Initial DB Creation** | `storage::init_db(conn)` | Table Creation DDL | `INSERT OR IGNORE INTO metadata(key, value) VALUES('index_generation', '1')` | Safe default generation `1` initialized. |
| **Fresh Analysis Pass** | `analyzer::analyze(root)` | Explicit `conn.transaction()?` | `INSERT INTO metadata ... ON CONFLICT(key) DO UPDATE SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT)` | Executed inside `tx` immediately before `tx.commit()?`. |
| **Incremental File Add** | `analyzer::analyze(root)` | Same `tx` | Same monotonic increment in `tx` | Atomic with file insertion. Cache purged via `clear_workspace_cache`. |
| **Incremental File Edit** | `analyzer::analyze(root)` | Same `tx` | Same monotonic increment in `tx` | Atomic with entity updates. Cache purged via `clear_workspace_cache`. |
| **File Deletion / Unlinking** | `analyzer::analyze(root)` | Same `tx` | Same monotonic increment in `tx` | Atomic with entity invalidations. Cache purged via `clear_workspace_cache`. |
| **Relationship Extraction** | `analyzer::analyze(root)` | Same `tx` | Same monotonic increment in `tx` | Atomic with graph relationship updates. |
| **Rollback / Error Path** | Any error inside `tx` | SQLite automatic rollback | Transaction rolls back `metadata` change | Generation remains unmutated; no stale state committed. |

---

## 3. Atomic Boundary Verification

Code inspection of `src/analyzer.rs` lines 2206-2218:

```rust
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('head_revision', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [&rev],
    )?;
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('index_generation', '1')
         ON CONFLICT(key) DO UPDATE SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT)",
        [],
    )?;
    tx.commit()?;
    crate::compiler::clear_workspace_cache(&root);
```

### Key Safety Invariants Established:
1. **Intra-Transaction Increment:** `index_generation` is strictly executed within the active SQLite transaction `tx`. It cannot commit unless all preceding file, entity, version, and relationship DML operations succeed.
2. **Post-Commit Invalidation:** `clear_workspace_cache(&root)` is called *after* `tx.commit()?`, ensuring memory is freed only after durable persistence is guaranteed.
3. **No Phantom Increments:** If analysis encounters an I/O or parser error prior to commit, the transaction aborts and `index_generation` is restored to its previous value by SQLite engine rollback.

---

## 4. Older Database Backward Compatibility

In pre-existing databases created before Phase 5.8.1:
1. `storage::init_db` executes `INSERT OR IGNORE INTO metadata(key, value) VALUES('index_generation', '1')`.
2. If `index_generation` is missing, `compute_index_fingerprint` in `src/compiler.rs` employs `COALESCE(value, '1')`, defaulting safely to `1`.
3. The next analysis run updates `1` to `2` smoothly without schema errors or null pointer exceptions.
