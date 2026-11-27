use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::thread;

use engram::analyzer::analyze;
use engram::compiler::{
    clear_context_cache, clear_workspace_cache, compute_index_fingerprint, context,
};
use engram::storage::db;

fn setup_test_workspace(name: &str, is_git: bool) -> PathBuf {
    let ws = std::env::temp_dir().join(format!(
        "engram_cache_test_{}_{}",
        name,
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&ws).unwrap();

    if is_git {
        let _ = Command::new("git").args(["init"]).current_dir(&ws).output();
        let _ = Command::new("git")
            .args(["config", "user.name", "Engram Tester"])
            .current_dir(&ws)
            .output();
        let _ = Command::new("git")
            .args(["config", "user.email", "tester@engram.local"])
            .current_dir(&ws)
            .output();
    }

    ws
}

#[test]
fn test_01_cache_hit_when_nothing_changed() {
    let ws = setup_test_workspace("hit", true);
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("auth_service.rs"), "pub struct AuthService;\n").unwrap();
    analyze(&ws).unwrap();

    let res1 = context(&ws, "AuthService", 2000).unwrap();
    assert!(
        res1.files
            .iter()
            .any(|f| f.path.contains("auth_service.rs"))
    );

    let res2 = context(&ws, "AuthService", 2000).unwrap();
    assert!(
        res2.files
            .iter()
            .any(|f| f.path.contains("auth_service.rs"))
    );
    assert_eq!(res1.files.len(), res2.files.len());

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_02_cache_miss_after_entity_modified() {
    let ws = setup_test_workspace("mod", true);
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    let file = src.join("user_service.rs");
    fs::write(&file, "pub struct UserService;\n").unwrap();
    analyze(&ws).unwrap();

    let res1 = context(&ws, "UserService", 2000).unwrap();
    assert!(
        res1.files
            .iter()
            .any(|f| f.path.contains("user_service.rs"))
    );

    // Modify file to add new symbol
    fs::write(&file, "pub struct UserService;\npub struct UserProfile;\n").unwrap();
    analyze(&ws).unwrap();

    let res2 = context(&ws, "UserProfile", 2000).unwrap();
    assert!(
        res2.files
            .iter()
            .any(|f| f.path.contains("user_service.rs"))
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_03_cache_miss_after_entity_deleted() {
    let ws = setup_test_workspace("del", true);
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    let file = src.join("temp_worker.rs");
    fs::write(&file, "pub struct TempWorker;\n").unwrap();
    analyze(&ws).unwrap();

    let res1 = context(&ws, "TempWorker", 2000).unwrap();
    assert!(res1.files.iter().any(|f| f.path.contains("temp_worker.rs")));

    // Delete file
    fs::remove_file(&file).unwrap();
    analyze(&ws).unwrap();

    let res2 = context(&ws, "TempWorker", 2000).unwrap();
    assert!(!res2.files.iter().any(|f| f.path.contains("temp_worker.rs")));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_04_cache_miss_after_file_renamed() {
    let ws = setup_test_workspace("rename", true);
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    let old_file = src.join("old_module.rs");
    let new_file = src.join("new_module.rs");
    fs::write(&old_file, "pub struct ModuleEntity;\n").unwrap();
    analyze(&ws).unwrap();

    let res1 = context(&ws, "ModuleEntity", 2000).unwrap();
    assert!(res1.files.iter().any(|f| f.path.contains("old_module.rs")));

    fs::rename(&old_file, &new_file).unwrap();
    analyze(&ws).unwrap();

    let res2 = context(&ws, "ModuleEntity", 2000).unwrap();
    assert!(!res2.files.iter().any(|f| f.path.contains("old_module.rs")));
    assert!(res2.files.iter().any(|f| f.path.contains("new_module.rs")));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_05_cache_miss_after_new_file_added() {
    let ws = setup_test_workspace("add", true);
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("base.rs"), "pub struct BaseService;\n").unwrap();
    analyze(&ws).unwrap();

    let res1 = context(&ws, "ExtensionService", 2000).unwrap();
    assert!(!res1.files.iter().any(|f| f.path.contains("ext.rs")));

    // Add new file
    fs::write(src.join("ext.rs"), "pub struct ExtensionService;\n").unwrap();
    analyze(&ws).unwrap();

    let res2 = context(&ws, "ExtensionService", 2000).unwrap();
    assert!(res2.files.iter().any(|f| f.path.contains("ext.rs")));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_06_cache_invalidation_wal_mode() {
    let ws = setup_test_workspace("wal", true);
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("db_client.rs"), "pub struct DbClient;\n").unwrap();
    analyze(&ws).unwrap();

    let res1 = context(&ws, "DbClient", 2000).unwrap();
    assert!(res1.files.iter().any(|f| f.path.contains("db_client.rs")));

    // Open connection, modify metadata directly in WAL mode
    {
        let conn = db(&ws).unwrap();
        conn.execute(
            "INSERT INTO metadata(key, value) VALUES('index_generation', '999') ON CONFLICT(key) DO UPDATE SET value='999'",
            [],
        ).unwrap();
    }

    // Now context should detect index_generation mismatch and compile freshly
    let res2 = context(&ws, "DbClient", 2000).unwrap();
    assert!(res2.files.iter().any(|f| f.path.contains("db_client.rs")));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_07_cache_invalidation_db_replacement() {
    let ws = setup_test_workspace("replace", true);
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("component.rs"), "pub struct ComponentA;\n").unwrap();
    analyze(&ws).unwrap();

    let _res1 = context(&ws, "ComponentA", 2000).unwrap();

    // Rebuild database from scratch
    let conn = db(&ws).unwrap();
    conn.execute_batch("DELETE FROM files; DELETE FROM entities; DELETE FROM entity_versions;")
        .unwrap();
    conn.execute(
        "INSERT INTO metadata(key, value) VALUES('index_generation', '500') ON CONFLICT(key) DO UPDATE SET value='500'",
        [],
    ).unwrap();
    drop(conn);

    let res2 = context(&ws, "ComponentA", 2000).unwrap();
    // After DB wipe, ensure_fresh re-indexes component.rs and finds ComponentA
    assert!(res2.files.iter().any(|f| f.path.contains("component.rs")));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_08_non_git_workspace() {
    // Crucial test: workspace without git (HEAD is unknown/initial)
    let ws = setup_test_workspace("nongit", false);
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    let file = src.join("standalone.rs");
    fs::write(&file, "pub struct Standalone;\n").unwrap();
    analyze(&ws).unwrap();

    let res1 = context(&ws, "Standalone", 2000).unwrap();
    assert!(res1.files.iter().any(|f| f.path.contains("standalone.rs")));

    // Delete file in non-git workspace
    fs::remove_file(&file).unwrap();
    analyze(&ws).unwrap();

    let res2 = context(&ws, "Standalone", 2000).unwrap();
    assert!(!res2.files.iter().any(|f| f.path.contains("standalone.rs")));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_09_workspace_isolation() {
    let ws1 = setup_test_workspace("iso1", true);
    let ws2 = setup_test_workspace("iso2", true);

    let src1 = ws1.join("src");
    let src2 = ws2.join("src");
    fs::create_dir_all(&src1).unwrap();
    fs::create_dir_all(&src2).unwrap();

    fs::write(src1.join("calc_ws1.rs"), "pub struct Calculator;\n").unwrap();
    fs::write(src2.join("calc_ws2.rs"), "pub struct Calculator;\n").unwrap();

    analyze(&ws1).unwrap();
    analyze(&ws2).unwrap();

    let res1 = context(&ws1, "Calculator", 2000).unwrap();
    let res2 = context(&ws2, "Calculator", 2000).unwrap();

    assert!(res1.files.iter().any(|f| f.path.contains("calc_ws1.rs")));
    assert!(!res1.files.iter().any(|f| f.path.contains("calc_ws2.rs")));

    assert!(res2.files.iter().any(|f| f.path.contains("calc_ws2.rs")));
    assert!(!res2.files.iter().any(|f| f.path.contains("calc_ws1.rs")));

    let _ = fs::remove_dir_all(&ws1);
    let _ = fs::remove_dir_all(&ws2);
}

#[test]
fn test_10_index_generation_change() {
    let ws = setup_test_workspace("gen", true);
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("gen_service.rs"), "pub struct GenService;\n").unwrap();
    analyze(&ws).unwrap();

    let _res1 = context(&ws, "GenService", 2000).unwrap();

    let conn = db(&ws).unwrap();
    let fp1 = compute_index_fingerprint(&ws, &conn).unwrap();

    conn.execute(
        "UPDATE metadata SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT) WHERE key='index_generation'",
        [],
    ).unwrap();

    let fp2 = compute_index_fingerprint(&ws, &conn).unwrap();
    assert_ne!(fp1.index_generation, fp2.index_generation);
    assert_ne!(fp1, fp2);

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_11_cache_eviction_at_capacity() {
    clear_context_cache();
    let ws = setup_test_workspace("capacity", true);
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("multi.rs"), "pub struct MultiItem;\n").unwrap();
    analyze(&ws).unwrap();

    // Query 70 distinct queries to exceed MAX_CACHE_ENTRIES (64)
    for i in 0..70 {
        let q = format!("QueryItem_{}", i);
        let _ = context(&ws, &q, 2000).unwrap();
    }

    // Cache should remain functional without unbounded growth or panic
    let res = context(&ws, "MultiItem", 2000).unwrap();
    assert!(res.files.iter().any(|f| f.path.contains("multi.rs")));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_12_concurrent_reads_and_invalidation() {
    let ws = Arc::new(setup_test_workspace("concurrent", true));
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("concurrent.rs"), "pub struct ConcurrentTarget;\n").unwrap();
    analyze(&ws).unwrap();

    let mut handles = Vec::new();

    for _ in 0..4 {
        let ws_clone = Arc::clone(&ws);
        handles.push(thread::spawn(move || {
            for _ in 0..10 {
                let res = context(&ws_clone, "ConcurrentTarget", 2000);
                assert!(res.is_ok());
            }
        }));
    }

    let ws_clone = Arc::clone(&ws);
    handles.push(thread::spawn(move || {
        for _ in 0..5 {
            clear_workspace_cache(&ws_clone);
            thread::sleep(std::time::Duration::from_millis(10));
        }
    }));

    for h in handles {
        h.join().unwrap();
    }

    let _ = fs::remove_dir_all(&*ws);
}

#[test]
fn test_13_failed_invalidation_fallback() {
    let ws = setup_test_workspace("fallback", true);
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("fallback.rs"), "pub struct FallbackItem;\n").unwrap();
    analyze(&ws).unwrap();

    // Verify context compiles successfully even when cache is explicitly cleared
    clear_context_cache();
    let res = context(&ws, "FallbackItem", 2000).unwrap();
    assert!(res.files.iter().any(|f| f.path.contains("fallback.rs")));

    let _ = fs::remove_dir_all(&ws);
}
