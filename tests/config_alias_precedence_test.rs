//! Integration tests for `engram.yml` configuration alias support and precedence.
//!
//! Validates:
//! 1. 10-step complete lifecycle with `engram.yml`:
//!    - Clean index of source files.
//!    - Exclusion via `engram.yml`.
//!    - Cache hash invalidation and entity eviction from persistent SQLite cache.
//!    - Restoration via include.
//!    - Deletion and return to default behavior.
//! 2. Exact precedence when both `engram.yaml` and `engram.yml` exist (`engram.yaml` wins).
//! 3. Precedence between root `engram.yml` and `.engram/config.yaml` (root `engram.yml` wins).
//! 4. Precedence between `.engram/config.yaml` and `.engram/config.yml` (`.yaml` wins).

use engram::analyzer::analyze;
use engram::discovery::DiscoveryEngine;
use engram::incremental::incremental_analyze;
use engram::storage::{cache_dir, config_hash};
use rusqlite::Connection;
use std::fs;
use std::path::Path;
use std::process::Command;

#[allow(dead_code)]
struct DbState {
    pub analysis_key: Option<String>,
    files: Vec<String>,
    active_entities: Vec<(String, String)>,
    invalidated_entities: Vec<(String, String, Option<String>)>,
}

fn query_db(root: &Path) -> Option<DbState> {
    let dir = cache_dir(root).ok()?;
    let db_path = dir.join("derived_symbols.db");
    if !db_path.exists() {
        return None;
    }
    let conn = Connection::open(&db_path).ok()?;
    let analysis_key: Option<String> = conn
        .query_row(
            "SELECT analysis_key FROM revisions ORDER BY rowid DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok();

    let mut stmt = conn.prepare("SELECT path FROM files ORDER BY path").ok()?;
    let files: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .ok()?
        .filter_map(Result::ok)
        .collect();

    let mut stmt_ent = conn
        .prepare("SELECT name, path FROM entities WHERE invalidated_at_revision IS NULL")
        .ok()?;
    let active_entities: Vec<(String, String)> = stmt_ent
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .ok()?
        .filter_map(Result::ok)
        .collect();

    let mut stmt_inv = conn
        .prepare("SELECT name, path, invalidated_at_revision FROM entities WHERE invalidated_at_revision IS NOT NULL")
        .ok()?;
    let invalidated_entities: Vec<(String, String, Option<String>)> = stmt_inv
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .ok()?
        .filter_map(Result::ok)
        .collect();

    Some(DbState {
        analysis_key,
        files,
        active_entities,
        invalidated_entities,
    })
}

#[test]
fn test_engram_yml_10_step_lifecycle() {
    let ws = std::env::temp_dir().join(format!("engram_alias_ws_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Alias Tester"])
        .current_dir(&ws)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "alias@engram.local"])
        .current_dir(&ws)
        .output();

    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let source_file = src_dir.join("service.rs");
    fs::write(&source_file, "pub fn execute_service() -> bool { true }\n").unwrap();

    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&ws)
        .output();
    let _ = Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(&ws)
        .output();

    // Step 1: Initial index
    let summary1 = analyze(&ws).expect("initial analyze must succeed");
    assert_eq!(summary1.indexed_files, 1);
    let hash1 = config_hash(&ws);
    let db1 = query_db(&ws).expect("db must exist");
    assert_eq!(
        db1.active_entities.len(),
        1,
        "Initial entity count must be 1"
    );
    assert_eq!(db1.files.len(), 1);

    // Step 2 & 3: Add engram.yml excluding src/
    let yml_path = ws.join("engram.yml");
    fs::write(
        &yml_path,
        r#"
ignore:
  use_gitignore: true
  use_engramignore: true
  exclude:
    - "src/"
"#,
    )
    .unwrap();

    // Step 4 & 5: Incremental analysis detects config hash change and evicts source
    let hash2 = config_hash(&ws);
    assert_ne!(
        hash1, hash2,
        "config_hash must change after adding engram.yml"
    );

    let summary2 = incremental_analyze(&ws).expect("incremental analysis must succeed");
    assert_eq!(summary2.tier1_deleted_files.len(), 1);

    // Step 6: Verify source eviction from SQLite
    let db2 = query_db(&ws).expect("db must exist");
    assert_eq!(
        db2.active_entities.len(),
        0,
        "Entity must be evicted or marked invalidated in SQLite"
    );
    assert_eq!(db2.files.len(), 0, "Files table must be empty");

    // Step 7: Change engram.yml to include the source
    fs::write(
        &yml_path,
        r#"
ignore:
  use_gitignore: true
  use_engramignore: true
  exclude:
    - "other/"
  include:
    - "src/service.rs"
"#,
    )
    .unwrap();

    let hash3 = config_hash(&ws);
    assert_ne!(
        hash2, hash3,
        "config_hash must change when engram.yml is modified"
    );

    // Step 8: DiscoveryEngine and indexing restore the source
    let _ = incremental_analyze(&ws).expect("incremental analyze must succeed");
    let db3 = query_db(&ws).expect("db must exist");
    assert_eq!(
        db3.active_entities.len(),
        1,
        "src/service.rs should be restored by engram.yml include"
    );
    assert_eq!(db3.files.len(), 1);

    // Step 9: Delete engram.yml
    fs::remove_file(&yml_path).unwrap();
    let hash4 = config_hash(&ws);
    assert_ne!(
        hash3, hash4,
        "config_hash must change after deleting engram.yml"
    );

    // Step 10: Verify expected default behavior restored
    let _ = incremental_analyze(&ws).expect("incremental analyze must succeed");
    let db4 = query_db(&ws).expect("db must exist");
    assert_eq!(
        db4.active_entities.len(),
        1,
        "Default discovery must retain unexcluded source file"
    );
    assert_eq!(db4.files.len(), 1);

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_config_precedence_yaml_vs_yml() {
    let ws = std::env::temp_dir().join(format!("engram_prec_ws_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();

    let file_a = ws.join("file_a.rs");
    let file_b = ws.join("file_b.rs");
    fs::write(&file_a, "pub fn a() {}\n").unwrap();
    fs::write(&file_b, "pub fn b() {}\n").unwrap();

    // Case 1: Both engram.yaml and engram.yml exist
    // engram.yaml excludes file_a
    fs::write(
        ws.join("engram.yaml"),
        r#"
ignore:
  exclude:
    - "file_a.rs"
"#,
    )
    .unwrap();

    // engram.yml excludes file_b
    fs::write(
        ws.join("engram.yml"),
        r#"
ignore:
  exclude:
    - "file_b.rs"
"#,
    )
    .unwrap();

    let engine = DiscoveryEngine::for_root(&ws);

    // engram.yaml MUST have precedence: file_a excluded, file_b included
    assert!(
        engine.is_excluded(&file_a, false),
        "engram.yaml must take precedence and exclude file_a"
    );
    assert!(
        !engine.is_excluded(&file_b, false),
        "engram.yml must be ignored when engram.yaml exists (file_b must NOT be excluded)"
    );

    // Case 2: engram.yml vs .engram/config.yaml
    // Remove engram.yaml so root engram.yml is active
    fs::remove_file(ws.join("engram.yaml")).unwrap();

    // .engram/config.yaml excludes file_a
    let dot_engram = ws.join(".engram");
    fs::create_dir_all(&dot_engram).unwrap();
    fs::write(
        dot_engram.join("config.yaml"),
        r#"
ignore:
  exclude:
    - "file_a.rs"
"#,
    )
    .unwrap();

    let engine2 = DiscoveryEngine::for_root(&ws);
    // Root engram.yml has precedence over .engram/config.yaml:
    // engram.yml excludes file_b, so file_b is excluded and file_a is not
    assert!(
        engine2.is_excluded(&file_b, false),
        "Root engram.yml must take precedence over .engram/config.yaml (file_b excluded)"
    );
    assert!(
        !engine2.is_excluded(&file_a, false),
        ".engram/config.yaml must be ignored when root engram.yml exists (file_a not excluded)"
    );

    // Case 3: .engram/config.yaml vs .engram/config.yml
    fs::remove_file(ws.join("engram.yml")).unwrap();
    fs::write(
        dot_engram.join("config.yml"),
        r#"
ignore:
  exclude:
    - "file_b.rs"
"#,
    )
    .unwrap();

    let engine3 = DiscoveryEngine::for_root(&ws);
    // .engram/config.yaml has precedence over .engram/config.yml:
    // config.yaml excludes file_a, so file_a is excluded and file_b is not
    assert!(
        engine3.is_excluded(&file_a, false),
        ".engram/config.yaml must take precedence over .engram/config.yml (file_a excluded)"
    );
    assert!(
        !engine3.is_excluded(&file_b, false),
        ".engram/config.yml must be ignored when .engram/config.yaml exists (file_b not excluded)"
    );

    let _ = fs::remove_dir_all(&ws);
}
