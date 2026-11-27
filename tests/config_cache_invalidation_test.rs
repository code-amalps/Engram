use engram::analyzer::analyze;
use engram::compiler::context;
use engram::incremental::incremental_analyze;
use engram::storage::{cache_dir, config_hash};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
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

fn create_test_repo() -> PathBuf {
    let ws = std::env::temp_dir().join(format!("engram_audit_a_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Audit Tester"])
        .current_dir(&ws)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "audit@engram.local"])
        .current_dir(&ws)
        .output();

    let reports = ws.join("reports");
    let src = ws.join("src");
    fs::create_dir_all(&reports).unwrap();
    fs::create_dir_all(&src).unwrap();

    fs::write(
        reports.join("finance.rs"),
        "pub fn generate_finance_report() -> bool { true }\n",
    )
    .unwrap();
    fs::write(src.join("main.rs"), "pub fn main() {}\n").unwrap();

    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&ws)
        .output();
    let _ = Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&ws)
        .output();

    ws
}

#[test]
fn test_audit_a_engram_config_yaml_cache_invalidation() {
    let ws = create_test_repo();

    // 1. Initial indexing with .engram/config.yaml absent
    let h1 = config_hash(&ws);
    let s1 = analyze(&ws).expect("initial analyze failed");
    assert_eq!(s1.indexed_files, 2);

    let db1 = query_db(&ws).expect("db must exist after analyze");
    assert!(db1.files.contains(&"reports/finance.rs".to_string()));
    assert!(
        db1.active_entities
            .iter()
            .any(|(n, _)| n == "generate_finance_report")
    );

    // 2. Create .engram/config.yaml excluding reports/
    let cfg_path = ws.join(".engram").join("config.yaml");
    fs::create_dir_all(cfg_path.parent().unwrap()).unwrap();
    fs::write(
        &cfg_path,
        "ignore:\n  use_gitignore: true\n  use_engramignore: true\n  exclude:\n    - 'reports/'\n",
    )
    .unwrap();

    // 3. Incremental analyze without full reindex
    let h2 = config_hash(&ws);
    assert_ne!(
        h1, h2,
        "config_hash MUST change when .engram/config.yaml is created"
    );

    let s2 = incremental_analyze(&ws).expect("incremental analyze failed");
    assert_eq!(
        s2.total_files, 1,
        "Only src/main.rs should remain in total files"
    );
    assert_eq!(
        s2.tier1_deleted_files,
        vec!["reports/finance.rs".to_string()],
        "reports/finance.rs must be detected as deleted"
    );

    let db2 = query_db(&ws).expect("db must exist");
    assert_eq!(db2.files, vec!["src/main.rs".to_string()]);
    assert!(
        !db2.active_entities
            .iter()
            .any(|(n, _)| n == "generate_finance_report"),
        "generate_finance_report must not be in active entities"
    );
    assert!(
        db2.invalidated_entities
            .iter()
            .any(|(n, _, _)| n == "generate_finance_report"),
        "generate_finance_report must be recorded in invalidated entities"
    );

    // Verify context query does not find finance.rs
    let ctx2 = context(&ws, "generate_finance_report", 4000).expect("context query failed");
    assert!(
        !ctx2.files.iter().any(|f| f.path.contains("finance.rs")),
        "finance.rs must NOT be returned in context when excluded"
    );

    // 4. Modify .engram/config.yaml to include reports/ again
    fs::write(
        &cfg_path,
        "ignore:\n  use_gitignore: true\n  use_engramignore: true\n  exclude: []\n",
    )
    .unwrap();

    let h3 = config_hash(&ws);
    assert_ne!(h2, h3, "config_hash MUST change when config is modified");

    let s3 = incremental_analyze(&ws).expect("incremental analyze failed");
    assert_eq!(s3.total_files, 2);

    let db3 = query_db(&ws).expect("db must exist");
    assert!(db3.files.contains(&"reports/finance.rs".to_string()));
    assert!(
        db3.active_entities
            .iter()
            .any(|(n, _)| n == "generate_finance_report")
    );

    let ctx3 = context(&ws, "generate_finance_report", 4000).expect("context query failed");
    assert!(
        ctx3.files.iter().any(|f| f.path.contains("finance.rs")),
        "finance.rs MUST be returned in context when re-included"
    );

    // 5. Delete .engram/config.yaml -> verify default restored
    fs::remove_file(&cfg_path).unwrap();
    let h4 = config_hash(&ws);
    assert_eq!(h1, h4, "config_hash MUST return to baseline when deleted");

    let s4 = incremental_analyze(&ws).expect("incremental analyze failed");
    assert_eq!(s4.total_files, 2);

    let db4 = query_db(&ws).expect("db must exist");
    assert!(db4.files.contains(&"reports/finance.rs".to_string()));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_audit_a_engram_yaml_root_cache_invalidation() {
    let ws = create_test_repo();

    let h1 = config_hash(&ws);
    let s1 = analyze(&ws).expect("analyze failed");
    assert_eq!(s1.indexed_files, 2);

    let cfg_path = ws.join("engram.yaml");
    fs::write(
        &cfg_path,
        "ignore:\n  use_gitignore: true\n  use_engramignore: true\n  exclude:\n    - 'reports/'\n",
    )
    .unwrap();

    let h2 = config_hash(&ws);
    assert_ne!(h1, h2);

    let s2 = incremental_analyze(&ws).unwrap();
    assert_eq!(s2.total_files, 1);

    fs::remove_file(&cfg_path).unwrap();
    let h3 = config_hash(&ws);
    assert_eq!(h1, h3);

    let s3 = incremental_analyze(&ws).unwrap();
    assert_eq!(s3.total_files, 2);

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_audit_a_engramignore_cache_invalidation() {
    let ws = create_test_repo();

    let h1 = config_hash(&ws);
    let s1 = analyze(&ws).expect("analyze failed");
    assert_eq!(s1.indexed_files, 2);

    let ig_path = ws.join(".engramignore");
    fs::write(&ig_path, "reports/\n").unwrap();

    let h2 = config_hash(&ws);
    assert_ne!(h1, h2);

    let s2 = incremental_analyze(&ws).unwrap();
    assert_eq!(s2.total_files, 1);

    fs::remove_file(&ig_path).unwrap();
    let h3 = config_hash(&ws);
    assert_eq!(h1, h3);

    let s3 = incremental_analyze(&ws).unwrap();
    assert_eq!(s3.total_files, 2);

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_audit_a_gitignore_cache_invalidation() {
    let ws = create_test_repo();

    let h1 = config_hash(&ws);
    let s1 = analyze(&ws).expect("analyze failed");
    assert_eq!(s1.indexed_files, 2);

    let ig_path = ws.join(".gitignore");
    fs::write(&ig_path, "reports/\n").unwrap();

    let h2 = config_hash(&ws);
    assert_ne!(h1, h2);

    let s2 = incremental_analyze(&ws).unwrap();
    assert_eq!(s2.total_files, 1);

    fs::remove_file(&ig_path).unwrap();
    let h3 = config_hash(&ws);
    assert_eq!(h1, h3);

    let s3 = incremental_analyze(&ws).unwrap();
    assert_eq!(s3.total_files, 2);

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_audit_a_multiple_simultaneous_configs_precedence() {
    let ws = create_test_repo();

    let _rep = ws.join("reports");
    let datasets = ws.join("datasets");
    let assets = ws.join("assets");
    fs::create_dir_all(&datasets).unwrap();
    fs::create_dir_all(&assets).unwrap();
    fs::write(datasets.join("data.rs"), "pub fn data() {}\n").unwrap();
    fs::write(assets.join("res.rs"), "pub fn res() {}\n").unwrap();

    // 4 source files: reports, datasets, assets, src
    let s0 = analyze(&ws).unwrap();
    assert_eq!(s0.indexed_files, 4);

    // .gitignore excludes datasets/
    fs::write(ws.join(".gitignore"), "datasets/\n").unwrap();

    // .engramignore excludes assets/
    fs::write(ws.join(".engramignore"), "assets/\n").unwrap();

    // .engram/config.yaml excludes reports/
    let engram_cfg = ws.join(".engram").join("config.yaml");
    fs::create_dir_all(engram_cfg.parent().unwrap()).unwrap();
    fs::write(
        &engram_cfg,
        "ignore:\n  use_gitignore: true\n  use_engramignore: true\n  exclude:\n    - 'reports/'\n",
    )
    .unwrap();

    let s1 = incremental_analyze(&ws).unwrap();
    assert_eq!(s1.total_files, 1, "Only src/main.rs should remain");

    // engram.yaml sets use_gitignore: false -> datasets/ must become indexed again!
    fs::write(
        ws.join("engram.yaml"),
        "ignore:\n  use_gitignore: false\n  use_engramignore: true\n  exclude:\n    - 'reports/'\n",
    )
    .unwrap();

    let s2 = incremental_analyze(&ws).unwrap();
    assert_eq!(
        s2.total_files, 2,
        "datasets/ and src/ must be indexed when use_gitignore is disabled"
    );

    let _ = fs::remove_dir_all(&ws);
}
