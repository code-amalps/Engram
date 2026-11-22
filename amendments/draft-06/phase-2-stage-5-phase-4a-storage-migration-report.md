# Engram — Phase 2, Stage 5: Core Engine Remediation
## Phase 4A: SQLite Schema & Storage Migration Report

**Document ID:** `ENGRAM-PHASE2-STAGE5-PHASE4A-REPORT-01`  
**Date:** 2026-11-07  
**Module:** `src/storage.rs`, `tests/storage_migration_test.rs`  
**Governing Documents:**  
- `amendments/draft-06/phase-2-stage-4-architectural-remediation-spec.md` (Rev 2.0)  
- `amendments/draft-06/phase-2-stage-4-1-audit-report.md`  
**Candidate Binary Reference:** `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` (`F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`)  
**Status:** `PHASE 4A COMPLETE — ALL 10 TESTS PASSING (0 REGRESSIONS) — AWAITING SEPARATE OWNER AUTHORIZATION FOR PHASE 4B`

---

## 1. Executive Summary

Phase 4A of Core Engine Remediation has been successfully implemented and validated in strict accordance with the Stage 4 Rev 2.0 Architectural Specification and Stage 4.1 Audit Report.

The SQLite schema migration establishes granular member-level indexing (`entity_members`), resilience tracking (`parse_status`), and first-party/third-party boundary classification (`is_external`) while preserving 100% backward compatibility with pre-existing `entity_versions` and legacy Phase 1 databases.

### Core Achievements in Phase 4A:
1. **Schema Versioning Ratified:** Implemented versioned migration using SQLite `PRAGMA user_version = 2`.
2. **Granular Entity Members Table:** Deployed `entity_members` table supporting methods, fields, signatures, return types, visibility, async/static flags, decorators, and foreign key cascades.
3. **Overload Disambiguation:** Implemented deterministic `member_id` generation (`sha256(entity_id + ":" + name + ":" + signature)[0..16]`), allowing multiple overloaded method signatures to co-exist without collision.
4. **Boundary & Resilience Columns:** Added `parse_status TEXT NOT NULL DEFAULT 'COMPLETE'` and `is_external INTEGER NOT NULL DEFAULT 0` to `entities`, and `is_external INTEGER NOT NULL DEFAULT 0` to `relationships`.
5. **Atomic & Idempotent Migration:** Migration executes inside an immediate SQLite transaction with automatic rollback on error. Repeated execution is a validated no-op.
6. **Zero Data Loss & Backward Compatibility:** Existing tables (`files`, `entities`, `entity_versions`, `relationships`, `metadata`) retain all data without row truncation or corruption.

---

## 2. Exact Schema Changes

### 2.1 Table: `entity_members` (New)
```sql
CREATE TABLE IF NOT EXISTS entity_members (
    member_id TEXT PRIMARY KEY,
    entity_id TEXT NOT NULL,
    name TEXT NOT NULL,
    member_kind TEXT NOT NULL,
    signature TEXT NOT NULL,
    return_type TEXT,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    visibility TEXT NOT NULL DEFAULT 'public',
    is_async BOOLEAN NOT NULL DEFAULT 0,
    is_static BOOLEAN NOT NULL DEFAULT 0,
    decorators TEXT,
    FOREIGN KEY(entity_id) REFERENCES entities(entity_id) ON DELETE CASCADE
);
```

### 2.2 Table Alterations (Migrated)
- **`entities` table:**
  - Added column: `parse_status TEXT NOT NULL DEFAULT 'COMPLETE'`
  - Added column: `is_external INTEGER NOT NULL DEFAULT 0`
- **`relationships` table:**
  - Added column: `is_external INTEGER NOT NULL DEFAULT 0`

### 2.3 Indexes (New & Verified)
```sql
CREATE INDEX IF NOT EXISTS idx_members_entity ON entity_members(entity_id);
CREATE INDEX IF NOT EXISTS idx_members_entity_name ON entity_members(entity_id, name);
CREATE INDEX IF NOT EXISTS idx_members_name ON entity_members(name);
CREATE INDEX IF NOT EXISTS idx_relationships_ext ON relationships(source_id, is_external);
```

### 2.4 Rust Storage API Additions (`src/storage.rs`)
- `pub const CURRENT_SCHEMA_VERSION: u32 = 2;`
- `pub struct EntityMember` (with builder `EntityMember::new(...)` and serde support)
- `pub enum ParseStatus` (`Complete`, `Partial`, `NotAvailable`, `Error`)
- `pub fn member_id(entity_id: &str, name: &str, signature: &str) -> String`
- `pub fn get_schema_version(conn: &Connection) -> Result<u32>`
- `pub fn migrate_database(conn: &mut Connection) -> Result<()>`
- `pub fn init_db(conn: &mut Connection) -> Result<()>`
- `pub fn db_at_path(db_file: &Path) -> Result<Connection>`
- `pub fn insert_entity_members(conn: &Connection, members: &[EntityMember]) -> Result<usize>`
- `pub fn get_entity_members(conn: &Connection, entity_id: &str) -> Result<Vec<EntityMember>>`
- `pub fn get_entity_members_by_name(conn: &Connection, entity_id: &str, member_name: &str) -> Result<Vec<EntityMember>>`
- `pub fn count_entity_members(conn: &Connection, entity_id: &str) -> Result<usize>`
- `pub fn update_entity_parse_status(conn: &Connection, entity_id: &str, status: ParseStatus) -> Result<()>`
- `pub fn get_entity_parse_status(conn: &Connection, entity_id: &str) -> Result<ParseStatus>`

---

## 3. Migration Behavior & Rollback Evidence

### 3.1 Migration Lifecycle
```mermaid
flowchart TD
    OPEN["Connection::open(db_path)"] --> WAL["PRAGMA journal_mode = WAL<br/>PRAGMA foreign_keys = ON"]
    WAL --> BASE["Base Table Verification (CREATE TABLE IF NOT EXISTS)"]
    BASE --> VER_CHECK{"PRAGMA user_version < 2?"}
    VER_CHECK -->|No (>= 2)| NO_OP["No-Op Return Ok(())"]
    VER_CHECK -->|Yes (< 2)| TX["BEGIN IMMEDIATE TRANSACTION"]
    
    subgraph Atomic Migration Transaction
        TX --> C1["Check & ALTER TABLE entities ADD parse_status, is_external"]
        C1 --> C2["Check & ALTER TABLE relationships ADD is_external"]
        C2 --> C3["CREATE TABLE entity_members + Indices"]
        C3 --> COMMIT["COMMIT TRANSACTION"]
    end

    COMMIT --> PRAGMA_SET["PRAGMA user_version = 2"]
    PRAGMA_SET --> BACKFILL["Backfill entity_identity / entity_versions"]
    NO_OP --> READY["Connection Ready"]
    BACKFILL --> READY
```

### 3.2 Rollback Safety Verification
The rollback behavior was explicitly validated in test `test_05_rollback_on_migration_failure`:
- An uncommitted migration transaction encountering an error automatically aborts.
- `PRAGMA user_version` remains strictly at `0`.
- Altered columns (e.g. `parse_status`) are rolled back and do not exist in the database header.
- Pre-existing entity and relationship records remain 100% intact with zero partial state or database corruption.

---

## 4. Tests Executed and Actual Results

### 4.1 Storage Migration Integration Suite (`tests/storage_migration_test.rs`)
Command: `cargo test --test storage_migration_test`  
Result: **10 passed; 0 failed; finished in 0.06s**

| Test Identifier | Objective | Result | Verification Notes |
| :--- | :--- | :---: | :--- |
| `test_01_fresh_database_initialization` | Fresh DB initialization at v2 | **PASS** | Validated `user_version = 2`, all 12 `entity_members` columns, and 4 indexes. |
| `test_02_migration_from_existing_schema` | Upgrades un-versioned legacy v1 DB | **PASS** | Successfully transitioned `user_version` 0 $\rightarrow$ 2 and created `entity_members`. |
| `test_03_migration_against_populated_database` | Upgrades DB with existing data | **PASS** | Existing rows survived; default values (`COMPLETE`, `0`) cleanly populated. |
| `test_04_repeated_migration_execution_idempotency` | Repeated migration calls (5x) | **PASS** | Verified absolute idempotency without errors or data mutation. |
| `test_05_rollback_on_migration_failure` | Transactional rollback on failure | **PASS** | Forced error inside transaction left `user_version = 0` and uncorrupted state. |
| `test_06_existing_storage_operations_remain_functional` | Base storage utilities | **PASS** | Verified `now()`, `hash()`, and metadata key-value storage. |
| `test_07_entity_member_integrity_and_uniqueness` | Member CRUD, Overloads & FK Cascade | **PASS** | Verified `member_id` hashing, 3 overloads stored, FK violation rejection, and cascade delete. |
| `test_08_backward_compatibility_with_existing_records` | Query compatibility with Phase 1 | **PASS** | `entity_versions` archaeological queries executed with identical results. |
| `test_09_no_data_loss_comprehensive_migration` | Multi-table data preservation | **PASS** | 10 files, 10 entities, 10 versions, 10 relations checksums verified pre/post migration. |
| `test_10_semantic_metadata_incomplete_handling` | Incomplete semantic metadata | **PASS** | Handled `None` return types, missing decorators, default visibility, and parse status states. |

### 4.2 Storage Unit Tests (`src/storage.rs`)
Command: `cargo test --lib storage`  
Result: **2 passed; 0 failed; finished in 0.02s**
- `storage::tests::test_member_id_deterministic`: **PASS**
- `storage::tests::test_parse_status_conversion`: **PASS**

### 4.3 Existing Regression Test Suites
All existing suites utilizing `storage::db` were re-run to ensure zero regression:
- `tests/incremental_test.rs`: **16 passed; 0 failed** (finished in 12.58s)
- `tests/phase2_test.rs`: **6 passed; 0 failed** (finished in 0.69s)
- `tests/impact_test.rs`: **7 passed; 0 failed** (finished in 2.18s)
- `tests/decision_intelligence_test.rs`: **4 passed; 0 failed** (finished in 6.09s)

---

## 5. Incomplete / Unavailable Semantic Metadata Policy

In compliance with Objective 8, the following deterministic rules govern incomplete semantic metadata:

| Metadata Field | Storage Location | Incomplete / Unavailable Behavior | Default Value |
| :--- | :--- | :--- | :--- |
| `parse_status` | `entities` | If parser hits syntax error or incomplete tokens, set to `PARTIAL` or `ERROR`. | `'COMPLETE'` |
| `is_external` | `entities`, `relationships` | Set to 1 if node is third-party/terminal; 0 for first-party workspace code. | `0` (internal) |
| `return_type` | `entity_members` | If return type cannot be resolved from AST, stored as SQLite `NULL`. | `None` (`NULL`) |
| `decorators` | `entity_members` | If member has no decorators/annotations, stored as SQLite `NULL`. | `None` (`NULL`) |
| `visibility` | `entity_members` | If unannotated (e.g. default TS public), stored as `'public'`. | `'public'` |
| `is_async` | `entity_members` | Set to 1 only if `async` keyword is present. | `0` (false) |
| `is_static` | `entity_members` | Set to 1 only if `static` keyword is present. | `0` (false) |
| `signature` | `entity_members` | If body is unparseable, fallback signature is formatted as `format!("{name}()")`. | Non-null |

---

## 6. Safety & Environment Invariant Verification

| Safety Invariant | Target | Expected Status | Measured Status | Verification Result |
| :--- | :--- | :--- | :--- | :---: |
| **Candidate Binary Hash** | `candidate-phase1/engram.exe` | `F56347EF...` (SHA-256) | `F56347EF...` | **PASS (UNTOUCHED)** |
| **Candidate Source Code** | `src/` core Rust code | Only `src/storage.rs` modified | Only `src/storage.rs` | **PASS (CONFINED)** |
| **Harness & Benchmark Files** | `benchmarks/` | Untouched during Phase 4A | 0 benchmark files modified | **PASS (UNTOUCHED)** |
| **Live Dispatches** | Model APIs / Cloud | 0 live requests dispatched | 0 requests | **PASS (OFFLINE ONLY)** |
| **Financial Spend** | Cloud Inference Cost | \$0.00 spent | \$0.00 | **PASS (ZERO SPEND)** |

---

## 7. Changed-File List

1. **`src/storage.rs`**: Core SQLite migration implementation, `entity_members` schema, versioning pragma, and CRUD helper functions.
2. **`tests/storage_migration_test.rs`**: Comprehensive 10-test suite covering all mandatory verification requirements.
3. **`amendments/draft-06/phase-2-stage-5-phase-4a-storage-migration-report.md`**: Standalone Phase 4A completion report.

---

## 8. Unresolved Issues & Next Steps

### Unresolved Issues
**None.** Phase 4A completed with zero compilation errors, zero warnings, and zero test failures across all new and legacy test suites.

### Protocol Boundary Check
In strict accordance with user instructions:
> **Stop after Phase 4A. Do not start Phase 4B until the implementation and evidence have been reviewed and separately authorized.**

Phase 4A is complete and sealed. Awaiting explicit owner authorization before proceeding to:
- **Phase 4B: Tree-Sitter Member & Signature Extractor** (`src/analyzer.rs`)
