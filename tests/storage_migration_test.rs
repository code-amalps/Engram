use anyhow::Result;
use engram::storage::{
    self, count_entity_members, get_entity_members, get_entity_members_by_name,
    get_entity_parse_status, get_schema_version, insert_entity_members, member_id,
    migrate_database, update_entity_parse_status, EntityMember, ParseStatus,
    CURRENT_SCHEMA_VERSION,
};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

struct TempTestDir {
    path: PathBuf,
}

impl TempTestDir {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("engram_test_{}_{}", prefix, uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&path);
        Self { path }
    }
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempTestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Helper to create a legacy Phase 1 (v1) schema with PRAGMA user_version = 0,
/// before entity_members, parse_status, or is_external existed.
fn create_legacy_v1_db() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.execute_batch(
        "CREATE TABLE files(
            path TEXT PRIMARY KEY,
            content_hash TEXT NOT NULL,
            analysis_key TEXT NOT NULL DEFAULT '',
            language TEXT NOT NULL,
            excerpt TEXT NOT NULL,
            revision TEXT NOT NULL,
            analyzed_at TEXT NOT NULL
        );
        CREATE TABLE entities(
            entity_id TEXT PRIMARY KEY,
            path TEXT NOT NULL,
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            signature TEXT NOT NULL,
            ast_fingerprint TEXT NOT NULL,
            created_at_revision TEXT NOT NULL,
            invalidated_at_revision TEXT,
            status TEXT NOT NULL DEFAULT 'OBSERVED'
        );
        CREATE INDEX entities_path ON entities(path);
        CREATE TABLE entity_identity(
            entity_id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            created_at_revision TEXT NOT NULL
        );
        CREATE TABLE entity_versions(
            version_id TEXT PRIMARY KEY,
            entity_id TEXT NOT NULL,
            path TEXT NOT NULL,
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            signature TEXT NOT NULL,
            ast_fingerprint TEXT NOT NULL,
            created_at_revision TEXT NOT NULL,
            invalidated_at_revision TEXT,
            status TEXT NOT NULL DEFAULT 'OBSERVED',
            UNIQUE(entity_id, created_at_revision)
        );
        CREATE INDEX entity_versions_path ON entity_versions(path, invalidated_at_revision);
        CREATE TABLE relationships(
            source_id TEXT NOT NULL,
            target TEXT NOT NULL,
            kind TEXT NOT NULL,
            evidence TEXT NOT NULL,
            confidence REAL NOT NULL,
            path TEXT NOT NULL,
            line INTEGER NOT NULL,
            created_at_revision TEXT NOT NULL,
            invalidated_at_revision TEXT
        );
        CREATE INDEX relationships_src ON relationships(source_id);
        CREATE INDEX relationships_tgt ON relationships(target);
        CREATE INDEX relationships_path ON relationships(path);
        CREATE TABLE metadata(
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        PRAGMA user_version = 0;",
    )?;
    Ok(conn)
}

#[test]
fn test_01_fresh_database_initialization() -> Result<()> {
    let tmp = TempTestDir::new("fresh_init");
    let conn = storage::db(tmp.path())?;

    // 1. Verify schema version is 2
    let version = get_schema_version(&conn)?;
    assert_eq!(
        version, CURRENT_SCHEMA_VERSION,
        "Fresh DB must initialize at CURRENT_SCHEMA_VERSION (2)"
    );

    // 2. Verify entity_members table exists and has all required columns
    let mut stmt = conn.prepare("PRAGMA table_info(entity_members)")?;
    let cols: Vec<String> = stmt
        .query_map([], |row| row.get(1))?
        .collect::<rusqlite::Result<_>>()?;

    let expected_cols = [
        "member_id",
        "entity_id",
        "name",
        "member_kind",
        "signature",
        "return_type",
        "start_line",
        "end_line",
        "visibility",
        "is_async",
        "is_static",
        "decorators",
    ];
    for col in &expected_cols {
        assert!(
            cols.iter().any(|c| c == col),
            "Missing column in entity_members: {col}"
        );
    }

    // 3. Verify entities table has parse_status and is_external
    let mut stmt = conn.prepare("PRAGMA table_info(entities)")?;
    let entity_cols: Vec<String> = stmt
        .query_map([], |row| row.get(1))?
        .collect::<rusqlite::Result<_>>()?;
    assert!(entity_cols.iter().any(|c| c == "parse_status"));
    assert!(entity_cols.iter().any(|c| c == "is_external"));

    // 4. Verify relationships table has is_external
    let mut stmt = conn.prepare("PRAGMA table_info(relationships)")?;
    let rel_cols: Vec<String> = stmt
        .query_map([], |row| row.get(1))?
        .collect::<rusqlite::Result<_>>()?;
    assert!(rel_cols.iter().any(|c| c == "is_external"));

    // 5. Verify indexes exist
    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='index'")?;
    let indices: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    assert!(indices.iter().any(|i| i == "idx_members_entity"));
    assert!(indices.iter().any(|i| i == "idx_members_entity_name"));
    assert!(indices.iter().any(|i| i == "idx_members_name"));
    assert!(indices.iter().any(|i| i == "idx_relationships_ext"));

    Ok(())
}

#[test]
fn test_02_migration_from_existing_schema() -> Result<()> {
    let mut conn = create_legacy_v1_db()?;

    // Confirm initial state is v0
    assert_eq!(get_schema_version(&conn)?, 0);

    // Confirm entity_members table does NOT exist initially
    let table_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='entity_members'",
        [],
        |row| row.get::<_, i64>(0).map(|c| c > 0),
    )?;
    assert!(!table_exists, "entity_members must not exist in legacy v1 DB");

    // Execute migration
    migrate_database(&mut conn)?;

    // Verify migration updated version to 2
    assert_eq!(get_schema_version(&conn)?, 2);

    // Verify entity_members table now exists
    let table_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='entity_members'",
        [],
        |row| row.get::<_, i64>(0).map(|c| c > 0),
    )?;
    assert!(table_exists, "entity_members table must exist after migration");

    Ok(())
}

#[test]
fn test_03_migration_against_populated_database() -> Result<()> {
    let mut conn = create_legacy_v1_db()?;

    // Seed realistic pre-migration data
    conn.execute(
        "INSERT INTO files(path, content_hash, language, excerpt, revision, analyzed_at)
         VALUES ('src/auth/service.ts', 'hash123', 'typescript', 'class AuthService {}', 'rev1', '2026-11-01T00:00:00Z')",
        [],
    )?;
    conn.execute(
        "INSERT INTO entities(entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision, status)
         VALUES ('ent_auth', 'src/auth/service.ts', 'AuthService', 'class', 1, 50, 'export class AuthService', 'fp1', 'rev1', 'OBSERVED')",
        [],
    )?;
    conn.execute(
        "INSERT INTO entity_versions(version_id, entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision, status)
         VALUES ('ver_auth', 'ent_auth', 'src/auth/service.ts', 'AuthService', 'class', 1, 50, 'export class AuthService', 'fp1', 'rev1', 'OBSERVED')",
        [],
    )?;
    conn.execute(
        "INSERT INTO relationships(source_id, target, kind, evidence, confidence, path, line, created_at_revision)
         VALUES ('ent_auth', '@angular/core', 'IMPORTS', 'import { Injectable }', 1.0, 'src/auth/service.ts', 1, 'rev1')",
        [],
    )?;

    // Run migration
    migrate_database(&mut conn)?;

    // Verify all original data survived unchanged
    let file_count: i64 = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))?;
    let entity_count: i64 = conn.query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))?;
    let version_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM entity_versions", [], |r| r.get(0))?;
    let rel_count: i64 = conn.query_row("SELECT COUNT(*) FROM relationships", [], |r| r.get(0))?;

    assert_eq!(file_count, 1);
    assert_eq!(entity_count, 1);
    assert_eq!(version_count, 1);
    assert_eq!(rel_count, 1);

    // Verify defaults populated on pre-existing rows
    let (parse_status, is_ext): (String, i64) = conn.query_row(
        "SELECT parse_status, is_external FROM entities WHERE entity_id = 'ent_auth'",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    assert_eq!(parse_status, "COMPLETE");
    assert_eq!(is_ext, 0);

    let rel_is_ext: i64 = conn.query_row(
        "SELECT is_external FROM relationships WHERE source_id = 'ent_auth'",
        [],
        |r| r.get(0),
    )?;
    assert_eq!(rel_is_ext, 0);

    Ok(())
}

#[test]
fn test_04_repeated_migration_execution_idempotency() -> Result<()> {
    let mut conn = create_legacy_v1_db()?;

    // Seed an entity
    conn.execute(
        "INSERT INTO entities(entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision)
         VALUES ('ent_1', 'src/a.ts', 'A', 'class', 1, 10, 'class A', 'fp', 'r1')",
        [],
    )?;

    // Run migration 5 times sequentially
    for i in 1..=5 {
        migrate_database(&mut conn)?;
        assert_eq!(
            get_schema_version(&conn)?,
            2,
            "Iteration {i} must maintain schema version 2"
        );
    }

    // Verify single entity remains undisturbed
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))?;
    assert_eq!(count, 1);

    Ok(())
}

#[test]
fn test_05_rollback_on_migration_failure() -> Result<()> {
    let conn = create_legacy_v1_db()?;

    // Populate entity before failed transaction
    conn.execute(
        "INSERT INTO entities(entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision)
         VALUES ('ent_pre', 'src/pre.ts', 'Pre', 'class', 1, 10, 'class Pre', 'fp', 'r1')",
        [],
    )?;

    // Simulate a migration transaction failure
    let tx_res: Result<()> = (|| {
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "ALTER TABLE entities ADD COLUMN parse_status TEXT NOT NULL DEFAULT 'COMPLETE'",
            [],
        )?;
        // Intentionally trigger an invalid SQL error to force rollback
        tx.execute("THIS IS INVALID SQL STATEMENT THAT WILL FAIL", [])?;
        tx.commit()?;
        Ok(())
    })();

    assert!(tx_res.is_err(), "Transaction must fail and trigger rollback");

    // Verify schema version is STILL 0 (not updated)
    let version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    assert_eq!(version, 0, "Failed migration must leave user_version at 0");

    // Verify parse_status was rolled back and does NOT exist
    let mut stmt = conn.prepare("PRAGMA table_info(entities)")?;
    let cols: Vec<String> = stmt
        .query_map([], |row| row.get(1))?
        .collect::<rusqlite::Result<_>>()?;
    assert!(
        !cols.iter().any(|c| c == "parse_status"),
        "parse_status must NOT exist after rollback"
    );

    // Verify pre-existing data was NOT lost or corrupted
    let name: String = conn.query_row(
        "SELECT name FROM entities WHERE entity_id = 'ent_pre'",
        [],
        |r| r.get(0),
    )?;
    assert_eq!(name, "Pre");

    Ok(())
}

#[test]
fn test_06_existing_storage_operations_remain_functional() -> Result<()> {
    let tmp = TempTestDir::new("ops_functional");
    let conn = storage::db(tmp.path())?;

    // Test now() and hash()
    let timestamp = storage::now();
    assert!(!timestamp.is_empty());
    let h = storage::hash(b"test-bytes");
    assert_eq!(h.len(), 64);

    // Test inserting and querying metadata
    conn.execute(
        "INSERT INTO metadata(key, value) VALUES ('stage', 'phase4a')",
        [],
    )?;
    let val: String = conn.query_row(
        "SELECT value FROM metadata WHERE key = 'stage'",
        [],
        |r| r.get(0),
    )?;
    assert_eq!(val, "phase4a");

    Ok(())
}

#[test]
fn test_07_entity_member_integrity_and_uniqueness() -> Result<()> {
    let tmp = TempTestDir::new("member_integrity");
    let conn = storage::db(tmp.path())?;

    // 1. Insert parent entity
    conn.execute(
        "INSERT INTO entities(entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision)
         VALUES ('svc_storage', 'src/storage.service.ts', 'LocalStorageService', 'class', 1, 100, 'export class LocalStorageService', 'fp_store', 'rev1')",
        [],
    )?;

    // 2. Insert members
    let members = vec![
        EntityMember::new(
            "svc_storage",
            "setItem",
            "method",
            "public setItem(key: string, value: any): void",
            10,
            15,
        ),
        EntityMember::new(
            "svc_storage",
            "getItem",
            "method",
            "public getItem<T>(key: string): T | null",
            17,
            25,
        ),
        EntityMember::new(
            "svc_storage",
            "removeItem",
            "method",
            "public removeItem(key: string): void",
            27,
            32,
        ),
    ];

    let inserted = insert_entity_members(&conn, &members)?;
    assert_eq!(inserted, 3);
    assert_eq!(count_entity_members(&conn, "svc_storage")?, 3);

    // 3. Query all members
    let retrieved = get_entity_members(&conn, "svc_storage")?;
    assert_eq!(retrieved.len(), 3);
    assert_eq!(retrieved[0].name, "setItem");
    assert_eq!(retrieved[1].name, "getItem");
    assert_eq!(retrieved[2].name, "removeItem");

    // 4. Test method overloads (same name, distinct signatures -> distinct member_id)
    let overload1 = EntityMember::new(
        "svc_storage",
        "query",
        "overload_signature",
        "query(key: string): string",
        35,
        35,
    );
    let overload2 = EntityMember::new(
        "svc_storage",
        "query",
        "overload_signature",
        "query(keys: string[]): string[]",
        36,
        36,
    );
    let overload_body = EntityMember::new(
        "svc_storage",
        "query",
        "method",
        "query(keyOrKeys: any): any",
        37,
        45,
    );

    assert_ne!(overload1.member_id, overload2.member_id);
    assert_ne!(overload1.member_id, overload_body.member_id);

    insert_entity_members(&conn, &[overload1, overload2, overload_body])?;
    let queries = get_entity_members_by_name(&conn, "svc_storage", "query")?;
    assert_eq!(queries.len(), 3, "All 3 overloaded signatures must be stored");

    // 5. Test Foreign Key constraint (inserting member for non-existent entity must fail)
    let orphaned_member = EntityMember::new(
        "non_existent_entity",
        "orphanMethod",
        "method",
        "orphanMethod(): void",
        1,
        5,
    );
    let orphan_res = insert_entity_members(&conn, &[orphaned_member]);
    assert!(
        orphan_res.is_err(),
        "Inserting member with invalid foreign key entity_id must fail"
    );

    // 6. Test Foreign Key cascade delete (deleting parent entity removes members)
    conn.execute("DELETE FROM entities WHERE entity_id = 'svc_storage'", [])?;
    let remaining_members = count_entity_members(&conn, "svc_storage")?;
    assert_eq!(
        remaining_members, 0,
        "Deleting entity must cascade-delete all its members"
    );

    Ok(())
}

#[test]
fn test_08_backward_compatibility_with_existing_records() -> Result<()> {
    let mut conn = create_legacy_v1_db()?;

    // Insert records matching Phase 1 usage in engram
    conn.execute(
        "INSERT INTO entity_versions(version_id, entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision, status)
         VALUES ('ver_intercept', 'ent_intercept', 'src/http.interceptor.ts', 'HttpErrorInterceptor', 'class', 10, 45, 'export class HttpErrorInterceptor implements HttpInterceptor', 'fp_int', 'rev_legacy', 'OBSERVED')",
        [],
    )?;

    // Run migration to v2
    migrate_database(&mut conn)?;

    // Execute standard archaeology / impact query on entity_versions
    let (entity_id, path, name, kind, start_line, sig): (
        String,
        String,
        String,
        String,
        usize,
        String,
    ) = conn.query_row(
        "SELECT entity_id, path, name, kind, start_line, signature
         FROM entity_versions
         WHERE (name = ?1 OR path = ?1) AND invalidated_at_revision IS NULL",
        ["HttpErrorInterceptor"],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get::<_, i64>(4)? as usize,
                row.get(5)?,
            ))
        },
    )?;

    assert_eq!(entity_id, "ent_intercept");
    assert_eq!(path, "src/http.interceptor.ts");
    assert_eq!(name, "HttpErrorInterceptor");
    assert_eq!(kind, "class");
    assert_eq!(start_line, 10);
    assert_eq!(
        sig,
        "export class HttpErrorInterceptor implements HttpInterceptor"
    );

    Ok(())
}

#[test]
fn test_09_no_data_loss_comprehensive_migration() -> Result<()> {
    let mut conn = create_legacy_v1_db()?;

    // Populate multiple tables
    for i in 1..=10 {
        conn.execute(
            &format!(
                "INSERT INTO files(path, content_hash, language, excerpt, revision, analyzed_at)
                 VALUES ('file_{i}.ts', 'hash_{i}', 'typescript', 'excerpt_{i}', 'rev1', '2026-11-07')"
            ),
            [],
        )?;
        conn.execute(
            &format!(
                "INSERT INTO entities(entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision)
                 VALUES ('ent_{i}', 'file_{i}.ts', 'Name{i}', 'function', {i}, {i} + 10, 'sig{i}', 'fp{i}', 'rev1')"
            ),
            [],
        )?;
        conn.execute(
            &format!(
                "INSERT INTO entity_versions(version_id, entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision)
                 VALUES ('ver_{i}', 'ent_{i}', 'file_{i}.ts', 'Name{i}', 'function', {i}, {i} + 10, 'sig{i}', 'fp{i}', 'rev1')"
            ),
            [],
        )?;
        conn.execute(
            &format!(
                "INSERT INTO relationships(source_id, target, kind, evidence, confidence, path, line, created_at_revision)
                 VALUES ('ent_{i}', 'target_{i}', 'CALLS', 'call()', 0.95, 'file_{i}.ts', {i}, 'rev1')"
            ),
            [],
        )?;
    }

    // Capture counts before migration
    let files_before: i64 = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))?;
    let entities_before: i64 = conn.query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))?;
    let versions_before: i64 =
        conn.query_row("SELECT COUNT(*) FROM entity_versions", [], |r| r.get(0))?;
    let rels_before: i64 =
        conn.query_row("SELECT COUNT(*) FROM relationships", [], |r| r.get(0))?;

    assert_eq!(files_before, 10);
    assert_eq!(entities_before, 10);
    assert_eq!(versions_before, 10);
    assert_eq!(rels_before, 10);

    // Run migration
    migrate_database(&mut conn)?;

    // Verify exact counts preserved after migration
    let files_after: i64 = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))?;
    let entities_after: i64 = conn.query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))?;
    let versions_after: i64 =
        conn.query_row("SELECT COUNT(*) FROM entity_versions", [], |r| r.get(0))?;
    let rels_after: i64 = conn.query_row("SELECT COUNT(*) FROM relationships", [], |r| r.get(0))?;

    assert_eq!(files_after, 10);
    assert_eq!(entities_after, 10);
    assert_eq!(versions_after, 10);
    assert_eq!(rels_after, 10);

    // Verify specific data values preserved intact
    for i in 1..=10 {
        let sig: String = conn.query_row(
            &format!("SELECT signature FROM entities WHERE entity_id = 'ent_{i}'"),
            [],
            |r| r.get(0),
        )?;
        assert_eq!(sig, format!("sig{i}"));
    }

    Ok(())
}

#[test]
fn test_10_semantic_metadata_incomplete_handling() -> Result<()> {
    let tmp = TempTestDir::new("incomplete_meta");
    let conn = storage::db(tmp.path())?;

    conn.execute(
        "INSERT INTO entities(entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision)
         VALUES ('ent_partial', 'src/partial.ts', 'BrokenSyntax', 'class', 1, 20, 'class BrokenSyntax', 'fp', 'r1')",
        [],
    )?;

    // 1. Initial parse status must be COMPLETE by default
    assert_eq!(
        get_entity_parse_status(&conn, "ent_partial")?,
        ParseStatus::Complete
    );

    // 2. Update to PARTIAL
    update_entity_parse_status(&conn, "ent_partial", ParseStatus::Partial)?;
    assert_eq!(
        get_entity_parse_status(&conn, "ent_partial")?,
        ParseStatus::Partial
    );

    // 3. Update to ERROR
    update_entity_parse_status(&conn, "ent_partial", ParseStatus::Error)?;
    assert_eq!(
        get_entity_parse_status(&conn, "ent_partial")?,
        ParseStatus::Error
    );

    // 4. Incomplete member metadata: member with no return type, no decorators, default visibility
    let member_incomplete = EntityMember {
        member_id: member_id("ent_partial", "fallbackMember", "fallbackMember()"),
        entity_id: "ent_partial".to_string(),
        name: "fallbackMember".to_string(),
        member_kind: "method".to_string(),
        signature: "fallbackMember()".to_string(),
        return_type: None,
        start_line: 5,
        end_line: 10,
        visibility: "public".to_string(),
        is_async: false,
        is_static: false,
        decorators: None,
    };

    insert_entity_members(&conn, &[member_incomplete])?;
    let members = get_entity_members(&conn, "ent_partial")?;
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].name, "fallbackMember");
    assert_eq!(members[0].return_type, None);
    assert_eq!(members[0].decorators, None);
    assert_eq!(members[0].visibility, "public");
    assert!(!members[0].is_async);
    assert!(!members[0].is_static);

    Ok(())
}

#[test]
fn test_11_member_id_collision_detection_and_duplicate_safety() -> Result<()> {
    let tmp = TempTestDir::new("collision_safety");
    let conn = storage::db(tmp.path())?;

    // 1. Insert parent entity
    conn.execute(
        "INSERT INTO entities(entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision)
         VALUES ('ent_service', 'src/service.ts', 'TestService', 'class', 1, 50, 'class TestService', 'fp_service', 'rev1')",
        [],
    )?;

    let m1 = EntityMember {
        member_id: "m_shared_id_1234".to_string(),
        entity_id: "ent_service".to_string(),
        name: "execute".to_string(),
        member_kind: "method".to_string(),
        signature: "public execute(): boolean".to_string(),
        return_type: Some("boolean".to_string()),
        start_line: 10,
        end_line: 15,
        visibility: "public".to_string(),
        is_async: false,
        is_static: false,
        decorators: None,
    };

    // 2. Insert m1 first time
    let count1 = insert_entity_members(&conn, &[m1.clone()])?;
    assert_eq!(count1, 1);
    assert_eq!(count_entity_members(&conn, "ent_service")?, 1);

    // 3. Duplicate insertion of exact same logical member should update safely without error
    let mut m1_updated = m1.clone();
    m1_updated.end_line = 18; // slight line shift
    let count2 = insert_entity_members(&conn, &[m1_updated])?;
    assert_eq!(count2, 1);
    assert_eq!(count_entity_members(&conn, "ent_service")?, 1);
    let stored = get_entity_members(&conn, "ent_service")?;
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].end_line, 18);

    // 4. Collision scenario: A distinct logical member with different name/signature
    // that produces the same member_id must NOT overwrite m1, but fail explicitly
    let m_colliding = EntityMember {
        member_id: "m_shared_id_1234".to_string(), // identical ID collision
        entity_id: "ent_service".to_string(),
        name: "unrelatedMethod".to_string(), // different name
        member_kind: "method".to_string(),
        signature: "public unrelatedMethod(): void".to_string(), // different signature
        return_type: Some("void".to_string()),
        start_line: 20,
        end_line: 25,
        visibility: "public".to_string(),
        is_async: false,
        is_static: false,
        decorators: None,
    };

    let collision_result = insert_entity_members(&conn, &[m_colliding]);
    assert!(collision_result.is_err(), "Hash collision must trigger explicit error");
    let err_msg = collision_result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Hash collision detected"),
        "Error message must specify collision: {err_msg}"
    );

    // 5. Verify m1 was preserved and never overwritten by colliding member
    let stored_after_collision = get_entity_members(&conn, "ent_service")?;
    assert_eq!(stored_after_collision.len(), 1);
    assert_eq!(stored_after_collision[0].name, "execute");
    assert_eq!(stored_after_collision[0].signature, "public execute(): boolean");

    Ok(())
}
