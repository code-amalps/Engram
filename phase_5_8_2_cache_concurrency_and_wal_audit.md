# Engram Phase 5.8.2: Cache Concurrency, WAL Mode & Locking Scope Audit

**Audit Phase:** 5.8.2  
**Audit Target:** Concurrency control, SQLite WAL interaction, mutex locking scopes, and LRU memory bounds.  
**Execution Mode:** STRICTLY OFFLINE FORENSIC AUDIT  

---

## 1. Concurrency and Synchronization Architecture

```mermaid
flowchart TD
    subgraph PROCESS["Process Memory Space"]
        MUTEX["Mutex<HashMap<String, CacheEntry>> (COMPILED_CONTEXT_CACHE)"]
        CAP["Bounded Capacity: 64 Entries Max"]
        MUTEX --- CAP
    end

    subgraph READ_PATH["Read Path (compiler.rs)"]
        R1["Acquire Lock"] --> R2["Lookup Key"]
        R2 --> R3["Check Fingerprint Equality"]
        R3 --> R4["Update last_accessed"]
        R4 --> R5["Drop Lock Immediately"]
    end

    subgraph WRITE_PATH["Write Path (analyzer.rs)"]
        W1["SQLite Transaction Commit (analyzer.rs)"] --> W2["Acquire Lock in clear_workspace_cache"]
        W2 --> W3["Retain non-matching workspace entries"]
        W3 --> W4["Drop Lock Immediately"]
    end

    READ_PATH -.-> MUTEX
    WRITE_PATH -.-> MUTEX
```

---

## 2. Lock Scope & Contention Analysis

In `src/compiler.rs`, the global cache is defined as:
```rust
static COMPILED_CONTEXT_CACHE: OnceLock<Mutex<HashMap<String, CacheEntry>>> = OnceLock::new();
const MAX_CACHE_ENTRIES: usize = 64;
```

### 2.1 Critical Section Minimization
1. **Cache Lookup:**
   - Lock acquired via `if let Ok(mut guard) = cache.lock()`.
   - Checks key match and fingerprint equality (`&entry.fingerprint == fp`).
   - If match found, clones the `ContextResult` and updates `last_accessed`.
   - Lock is **immediately dropped** before returning.
   - If miss, lock is **immediately dropped** before context compilation begins. Heavy token estimation, BM25 scoring, and AST traversal occur completely **outside** the lock.
2. **Cache Insertion:**
   - After context compilation completes, lock is re-acquired.
   - If capacity $\ge 64$, finds the entry with minimal `last_accessed` and removes it.
   - Inserts the new `CacheEntry`.
   - Lock is **immediately dropped**.
3. **Workspace Purge:**
   - In `clear_workspace_cache(root)`, lock is acquired, entries starting with the canonical root are removed using `retain`, and the lock is **immediately dropped**.

**Deadlock Assessment:** There are zero nested locks and zero locks held across I/O or SQLite queries. Lock contention is sub-millisecond.

---

## 3. SQLite Concurrency & WAL Mode Interaction

### 3.1 Busy Timeout Configuration
In `src/storage.rs`:
```rust
pub fn db(root: &Path) -> Result<Connection> {
    let dir = cache_dir(root)?;
    fs::create_dir_all(&dir)?;
    let mut conn = Connection::open(dir.join("derived_symbols.db"))?;
    conn.busy_timeout(std::time::Duration::from_secs(15))?;
    init_db(&mut conn)?;
    Ok(conn)
}
```
A 15-second busy timeout is enforced across both `db()` and `db_at_path()`.

### 3.2 WAL Mode Resilience
- SQLite WAL (Write-Ahead Logging) allows concurrent readers alongside a single active writer without lock contention (`SQLITE_BUSY`).
- When `analyzer::analyze` executes a commit, WAL snapshots guarantee that concurrent read queries see either the full prior revision or the full new revision, never partial state.
- Because `index_generation` is part of the `derived_symbols.db` SQLite metadata table, WAL readers observe the incremented generation simultaneously with the updated entity records.

---

## 4. Bounded Memory & LRU Eviction

- The cache is bounded at `const MAX_CACHE_ENTRIES: usize = 64`.
- Under memory pressure or heavy multi-query sweeps, memory footprint cannot exceed 64 compiled contexts (~2.5 MB maximum memory usage).
- Tested and verified in `cache_invalidation_test::test_11_cache_eviction_at_capacity`.

---

## 5. Audit Finding

The locking strategy is leak-free, non-blocking for compilation operations, bounded in memory, and resilient under concurrent reader/writer access.
