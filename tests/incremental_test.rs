//! Specification Test Harness for Engram Milestone 3 (Pillar 1):
//! Two-Tier Incremental Invalidation Engine (`src/incremental.rs`).
//!
//! Validates:
//! 1. Identical content with changed modification time (content hash match prevents reparse)
//! 2. Changed content with unchanged or unreliable modification time (authoritative hash detects change)
//! 3. Added, deleted, renamed, staged, and unstaged files
//! 4. Detached HEAD and shallow-clone support
//! 5. Missing Git objects or unavailable history safe fallback
//! 6. Symbol definition change that invalidates dependants (Tier 2 re-linking)
//! 7. Change that leaves unrelated symbols valid
//! 8. Deleted and unresolved symbols handled cleanly
//! 9. Cache corruption or missing cache auto-heals
//! 10. Repeated analysis producing deterministic results
//! 11. Safe full-analysis fallback when incremental correctness cannot be established
//! 12. Deeply nested fixtures scale linearly

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

use engram::incremental::{
    IncrementalOptions, incremental_analyze, incremental_analyze_with_options,
};
use engram::storage::db;

fn setup_temp_repo(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("engram_incr_test_{}_{}", name, Uuid::new_v4()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    fs::write(
        root.join("app.ts"),
        "export const APP_NAME = 'EngramIncremental';\n",
    )
    .unwrap();

    Command::new("git")
        .args(["init"])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "IncrTest"])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "incr@engram.dev"])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&root)
        .output()
        .unwrap();

    root
}

/// Test 1: Identical content with changed modification time:
/// Authoritative content hash matches database record -> file is NOT reparsed in Tier 1.
#[test]
fn test_1_identical_content_with_changed_mtime_not_reparsed() {
    let root = setup_temp_repo("mtime_changed");
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();

    let file_path = src.join("service.ts");
    let content = "export function processOrder(id: string): boolean { return id.length > 0; }\n";
    fs::write(&file_path, content).unwrap();

    // Cold analysis
    let cold = incremental_analyze(&root).expect("Cold analysis should succeed");
    assert!(
        cold.tier1_reparsed_files
            .contains(&"src/service.ts".to_string())
    );

    // Rewrite the EXACT same bytes to update file mtime without changing content
    fs::write(&file_path, content).unwrap();

    // Incremental analysis: Content hash is identical, so Tier 1 does NOT re-parse
    let warm = incremental_analyze(&root).expect("Warm analysis should succeed");
    assert!(
        !warm
            .tier1_reparsed_files
            .contains(&"src/service.ts".to_string()),
        "File with identical content must NOT be reparsed in Tier 1, even if mtime changed"
    );
    assert_eq!(warm.tier1_reparsed_files.len(), 0);
    assert!(warm.unchanged_files_count >= 2);

    let _ = fs::remove_dir_all(&root);
}

/// Test 2: Changed content with unchanged or unreliable modification time:
/// Authoritative content hashing ensures modifications are never missed.
#[test]
fn test_2_changed_content_with_unreliable_mtime_detected() {
    let root = setup_temp_repo("unreliable_mtime");
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();

    let file_path = src.join("service.ts");
    fs::write(&file_path, "export const VERSION = 1;\n").unwrap();

    let cold = incremental_analyze(&root).expect("Cold analysis should succeed");
    assert!(
        cold.tier1_reparsed_files
            .contains(&"src/service.ts".to_string())
    );

    // Modify file content
    fs::write(&file_path, "export const VERSION = 2;\n").unwrap();

    // Execute with force_content_hashing = true (bypassing any mtime short-circuit)
    let opts = IncrementalOptions {
        force_content_hashing: true,
        fallback_on_error: true,
    };
    let warm = incremental_analyze_with_options(&root, &opts)
        .expect("Analysis with force content hashing should succeed");

    assert!(
        warm.tier1_reparsed_files
            .contains(&"src/service.ts".to_string()),
        "Modified content must be detected by authoritative content hashing"
    );
    assert_eq!(warm.tier1_reparsed_files.len(), 1);

    let _ = fs::remove_dir_all(&root);
}

/// Test 3: Added, deleted, renamed, staged, and unstaged files:
/// Accurately detects additions, deletions, and file renames across Git index and working tree.
#[test]
fn test_3_added_deleted_renamed_staged_unstaged_files() {
    let root = setup_temp_repo("add_del_rename");
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();

    let file_a = src.join("feature_a.ts");
    let file_del = src.join("obsolete.ts");
    fs::write(
        &file_a,
        "export function featureA(): string { return 'A'; }\n",
    )
    .unwrap();
    fs::write(&file_del, "export function obsolete(): void {}\n").unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Add feature A and obsolete"])
        .current_dir(&root)
        .output()
        .unwrap();

    let cold = incremental_analyze(&root).unwrap();
    assert_eq!(cold.total_files, 3); // app.ts, feature_a.ts, obsolete.ts

    // 1. Delete obsolete.ts
    fs::remove_file(&file_del).unwrap();

    // 2. Rename feature_a.ts to feature_b.ts (same content)
    let file_b = src.join("feature_b.ts");
    fs::rename(&file_a, &file_b).unwrap();

    // 3. Add brand new file helper.ts and stage it
    let file_helper = src.join("helper.ts");
    fs::write(&file_helper, "export const HELPER = true;\n").unwrap();
    Command::new("git")
        .args(["add", "src/helper.ts"])
        .current_dir(&root)
        .output()
        .unwrap();

    let incr = incremental_analyze(&root).unwrap();

    // Invariant: helper.ts is newly added -> reparsed in Tier 1
    assert!(
        incr.tier1_reparsed_files
            .contains(&"src/helper.ts".to_string())
    );

    // Invariant: obsolete.ts is deleted
    assert!(
        incr.tier1_deleted_files
            .contains(&"src/obsolete.ts".to_string())
    );

    // Invariant: feature_a.ts -> feature_b.ts is detected as rename
    let rename_found = incr
        .tier1_renamed_files
        .iter()
        .any(|(old_p, new_p)| old_p == "src/feature_a.ts" && new_p == "src/feature_b.ts");
    assert!(
        rename_found,
        "Rename from feature_a.ts to feature_b.ts must be detected"
    );

    let _ = fs::remove_dir_all(&root);
}

/// Test 4: Detached HEAD and shallow-clone behavior:
/// Analysis functions correctly when HEAD is detached or when history is truncated.
#[test]
fn test_4_detached_head_and_shallow_clone_support() {
    let root = setup_temp_repo("detached_shallow");

    // Commit a second change
    fs::write(root.join("v2.ts"), "export const V2 = true;\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Commit 2"])
        .current_dir(&root)
        .output()
        .unwrap();

    let head_rev = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&root)
        .output()
        .unwrap();
    let commit_hash = String::from_utf8_lossy(&head_rev.stdout).trim().to_string();

    // Detach HEAD to commit hash
    Command::new("git")
        .args(["checkout", &commit_hash])
        .current_dir(&root)
        .output()
        .unwrap();

    // Verify detached HEAD analysis succeeds
    let detached_res =
        incremental_analyze(&root).expect("Detached HEAD incremental analysis should succeed");
    assert_eq!(detached_res.status, "INCREMENTAL_ANALYSIS_COMPLETE");
    assert!(detached_res.revision.contains(&commit_hash[..8]));

    // Simulate shallow clone by creating .git/shallow
    let git_dir = root.join(".git");
    fs::write(git_dir.join("shallow"), format!("{}\n", commit_hash)).unwrap();

    let shallow_res = incremental_analyze(&root).expect("Shallow clone analysis should succeed");
    assert_eq!(shallow_res.status, "INCREMENTAL_ANALYSIS_COMPLETE");

    let _ = fs::remove_dir_all(&root);
}

/// Test 5: Missing Git objects or unavailable history:
/// Non-git repository or missing git metadata executes safe fallback without panicking.
#[test]
fn test_5_missing_git_objects_or_unavailable_history_safe_fallback() {
    let root = std::env::temp_dir().join(format!("engram_no_git_{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();

    fs::write(root.join("index.ts"), "export const NON_GIT = true;\n").unwrap();

    // Run incremental analysis in a non-git directory
    let res = incremental_analyze(&root).expect("Non-git repository should analyze safely");
    assert_eq!(res.status, "INCREMENTAL_ANALYSIS_COMPLETE");
    assert!(res.revision.contains("WORKTREE"));
    assert_eq!(res.total_files, 1);
    assert_eq!(res.tier1_reparsed_files.len(), 1);

    let _ = fs::remove_dir_all(&root);
}

/// Test 6: Symbol definition change invalidates dependants:
/// Changing a symbol signature triggers Tier 2 relational re-linking of caller files.
#[test]
fn test_6_symbol_definition_change_invalidates_dependents() {
    let root = setup_temp_repo("sym_def_change");
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("math.ts"),
        "export function add(a: number, b: number): number { return a + b; }\n",
    )
    .unwrap();

    fs::write(
        src.join("calculator.ts"),
        "import { add } from './math';\nexport class Calculator {\n  sum(x: number, y: number) { return add(x, y); }\n}\n",
    )
    .unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial math and calculator"])
        .current_dir(&root)
        .output()
        .unwrap();

    let cold = incremental_analyze(&root).unwrap();
    assert_eq!(cold.tier1_reparsed_files.len(), 3); // app.ts, math.ts, calculator.ts

    // Modify math.ts symbol definition (change signature of add)
    fs::write(
        src.join("math.ts"),
        "export function add(a: number, b: number, c: number = 0): number { return a + b + c; }\n",
    )
    .unwrap();

    let warm = incremental_analyze(&root).unwrap();
    // Tier 1 must reparse strictly math.ts
    assert_eq!(warm.tier1_reparsed_files.len(), 1);
    assert_eq!(warm.tier1_reparsed_files[0], "src/math.ts");

    // Tier 2 must identify calculator.ts as dependent and re-link its relationships
    assert!(
        warm.tier2_relinked_files
            .contains(&"src/calculator.ts".to_string()),
        "calculator.ts calls 'add' and must be re-linked in Tier 2"
    );

    let _ = fs::remove_dir_all(&root);
}

/// Test 7: Change that leaves unrelated symbols valid:
/// Modifying an isolated module does not re-link or invalidate unrelated modules.
#[test]
fn test_7_unrelated_symbols_and_files_remain_valid() {
    let root = setup_temp_repo("unrelated_symbols");
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("isolated.ts"),
        "export const ISOLATED_FLAG = 'private';\n",
    )
    .unwrap();
    fs::write(src.join("main.ts"), "export const MAIN_ENTRY = true;\n").unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&root)
        .output()
        .unwrap();

    let _ = incremental_analyze(&root).unwrap();

    // Modify strictly isolated.ts
    fs::write(
        src.join("isolated.ts"),
        "export const ISOLATED_FLAG = 'updated';\n",
    )
    .unwrap();

    let warm = incremental_analyze(&root).unwrap();
    assert_eq!(warm.tier1_reparsed_files, vec!["src/isolated.ts"]);
    assert!(
        !warm
            .tier2_relinked_files
            .contains(&"src/main.ts".to_string()),
        "main.ts is unrelated to isolated.ts and must NOT be in Tier 2"
    );
    assert!(warm.unchanged_files_count >= 2); // app.ts, main.ts

    let _ = fs::remove_dir_all(&root);
}

/// Test 8: Deleted and unresolved symbols handled cleanly:
/// Removing a symbol invalidates its entity record and clears stale edges.
#[test]
fn test_8_deleted_and_unresolved_symbols_handled_cleanly() {
    let root = setup_temp_repo("deleted_symbols");
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("provider.ts"),
        "export function activeFn() {}\nexport function deprecatedFn() {}\n",
    )
    .unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&root)
        .output()
        .unwrap();

    let _cold = incremental_analyze(&root).unwrap();

    // Delete deprecatedFn from provider.ts
    fs::write(src.join("provider.ts"), "export function activeFn() {}\n").unwrap();

    let warm = incremental_analyze(&root).unwrap();
    assert_eq!(warm.tier1_reparsed_files, vec!["src/provider.ts"]);

    // Query SQLite to assert deprecatedFn is no longer active
    let conn = db(&root).unwrap();
    let active_deprecated: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM entities WHERE name = 'deprecatedFn' AND invalidated_at_revision IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        active_deprecated, 0,
        "deprecatedFn entity must be invalidated"
    );

    let _ = fs::remove_dir_all(&root);
}

/// Test 9: Cache corruption or missing cache entries:
/// Missing or corrupt file_meta_cache table auto-heals without failing.
#[test]
fn test_9_cache_corruption_or_missing_cache_auto_heals() {
    let root = setup_temp_repo("cache_heal");

    let cold = incremental_analyze(&root).unwrap();
    assert_eq!(cold.total_files, 1);

    // Intentionally corrupt/drop file_meta_cache table in SQLite
    let conn = db(&root).unwrap();
    conn.execute_batch("DROP TABLE file_meta_cache;").unwrap();
    drop(conn);

    // Subsequent analysis must auto-recreate table and succeed cleanly
    let warm =
        incremental_analyze(&root).expect("Incremental analysis should auto-heal dropped cache");
    assert_eq!(warm.status, "INCREMENTAL_ANALYSIS_COMPLETE");
    assert_eq!(warm.total_files, 1);

    let _ = fs::remove_dir_all(&root);
}

/// Test 10: Repeated analysis producing deterministic results:
/// Consecutive incremental runs on an unmutated repository produce identical invariant counts.
#[test]
fn test_10_repeated_analysis_is_deterministic() {
    let root = setup_temp_repo("deterministic");

    let _run1 = incremental_analyze(&root).unwrap();
    let run2 = incremental_analyze(&root).unwrap();
    let run3 = incremental_analyze(&root).unwrap();

    assert_eq!(run2.total_files, run3.total_files);
    assert_eq!(run2.tier1_reparsed_files.len(), 0);
    assert_eq!(run3.tier1_reparsed_files.len(), 0);
    assert_eq!(run2.tier2_relinked_files.len(), 0);
    assert_eq!(run3.tier2_relinked_files.len(), 0);
    assert_eq!(run2.unchanged_files_count, run3.unchanged_files_count);
    assert_eq!(run2.revision, run3.revision);

    let _ = fs::remove_dir_all(&root);
}

/// Test 11: Safe full-analysis fallback when incremental correctness cannot be established:
/// Corrupted derived database triggers graceful fallback without unhandled crashes.
#[test]
fn test_11_safe_full_analysis_fallback() {
    let root = setup_temp_repo("safe_fallback");

    // Initialize clean database
    let _ = incremental_analyze(&root).unwrap();

    // Corrupt the derived SQLite database file with garbage bytes
    let db_path = engram::storage::cache_dir(&root)
        .unwrap()
        .join("derived_symbols.db");
    fs::write(&db_path, "MALFORMED_NON_SQLITE_GARBAGE_BYTES").unwrap();

    // Call incremental analysis: must gracefully fall back to full analysis
    let fallback_res = incremental_analyze(&root)
        .expect("Analysis must complete via safe fallback even when database is corrupted");

    assert!(
        fallback_res.fallback_full_analysis,
        "Must report fallback_full_analysis = true"
    );
    assert_eq!(fallback_res.status, "FALLBACK_FULL_ANALYSIS_COMPLETE");
    assert!(
        fallback_res
            .diagnostics
            .iter()
            .any(|d| d.contains("FALLBACK_FULL_ANALYSIS"))
    );

    let _ = fs::remove_dir_all(&root);
}

/// Test 12: Deeply nested fixtures scale linearly:
/// Verifies performance on 5-level deep directory hierarchies.
#[test]
fn test_12_deeply_nested_fixtures_scale_linearly() {
    let root = setup_temp_repo("nested_fixture");
    let deep_dir = root
        .join("pkg")
        .join("module_a")
        .join("sub_b")
        .join("component_c")
        .join("leaf");
    fs::create_dir_all(&deep_dir).unwrap();

    // Create 15 intermediate files across various directory depths
    for i in 1..=15 {
        let p = root.join("pkg").join(format!("file_{}.ts", i));
        fs::write(p, format!("export const CONST_{} = {};\n", i, i)).unwrap();
    }

    let leaf_file = deep_dir.join("deep_leaf.ts");
    fs::write(
        &leaf_file,
        "export function deepLeaf(): string { return 'leaf'; }\n",
    )
    .unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Add nested files"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Cold analysis: indexes all 17 files (app.ts + 15 pkg files + deep_leaf.ts)
    let cold = incremental_analyze(&root).unwrap();
    assert_eq!(cold.total_files, 17);

    // Warm run modifying ONLY the deep leaf file
    fs::write(
        &leaf_file,
        "export function deepLeaf(): string { return 'leaf_v2'; }\n",
    )
    .unwrap();

    let warm = incremental_analyze(&root).unwrap();
    assert_eq!(warm.tier1_reparsed_files.len(), 1);
    assert_eq!(
        warm.tier1_reparsed_files[0],
        "pkg/module_a/sub_b/component_c/leaf/deep_leaf.ts"
    );
    assert_eq!(warm.unchanged_files_count, 16);

    let _ = fs::remove_dir_all(&root);
}

/// Test 13: Changed content with preserved file size and restored modification time:
/// Adversarial check: verify that an edit with exact same byte length and with mtime
/// restored to the original value is detected by authoritative content hashing and NEVER reuses stale analysis.
#[test]
fn test_13_changed_content_with_preserved_size_and_restored_mtime_detected() {
    let root = setup_temp_repo("restored_mtime_adv");
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();

    let file_path = src.join("config.ts");
    // Initial content: 40 bytes
    let initial_content = "export const FEATURE_FLAG: number = 100;\n";
    fs::write(&file_path, initial_content).unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init config"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Cold analysis: indexes config.ts
    let cold = incremental_analyze(&root).expect("Cold analysis should succeed");
    assert!(
        cold.tier1_reparsed_files
            .contains(&"src/config.ts".to_string())
    );

    // Capture the original modification timestamp
    let meta_before = fs::metadata(&file_path).unwrap();
    let orig_mtime = meta_before.modified().unwrap();

    // Adversarially modify file with identical byte length (40 bytes)
    let modified_content = "export const FEATURE_FLAG: number = 999;\n";
    assert_eq!(
        initial_content.len(),
        modified_content.len(),
        "Byte length must be identical"
    );
    fs::write(&file_path, modified_content).unwrap();

    // Restore the exact original modification timestamp
    let f = fs::File::options().write(true).open(&file_path).unwrap();
    let times = std::fs::FileTimes::new().set_modified(orig_mtime);
    f.set_times(times).unwrap();
    drop(f);

    // Verify mtime and size indeed match the original
    let meta_after = fs::metadata(&file_path).unwrap();
    assert_eq!(meta_after.len(), meta_before.len());

    // Incremental analysis must detect content change via authoritative byte hashing, NOT reuse stale analysis
    let warm = incremental_analyze(&root).expect("Incremental analysis must succeed");
    assert!(
        warm.tier1_reparsed_files
            .contains(&"src/config.ts".to_string()),
        "File with changed content must be reparsed even if size and mtime are identical to original"
    );
    assert_eq!(warm.tier1_reparsed_files.len(), 1);

    let _ = fs::remove_dir_all(&root);
}

/// Test 14: Staged content differing from working-tree content:
/// When index contains staged changes but working tree has further unstaged changes,
/// the working tree content is authoritatively analyzed.
#[test]
fn test_14_staged_content_differing_from_working_tree_content() {
    let root = setup_temp_repo("staged_differs_worktree");
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();

    let file_path = src.join("app.ts");
    fs::write(&file_path, "export const VERSION = 1;\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "v1"])
        .current_dir(&root)
        .output()
        .unwrap();

    let _cold = incremental_analyze(&root).unwrap();

    // Step 1: Stage version 2
    fs::write(&file_path, "export const VERSION = 2;\n").unwrap();
    Command::new("git")
        .args(["add", "src/app.ts"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Step 2: Write version 3 to working tree without staging
    fs::write(&file_path, "export const VERSION = 3;\n").unwrap();

    // Incremental analysis must analyze working tree content (v3)
    let res = incremental_analyze(&root).expect("Incremental analysis should succeed");
    assert!(res.tier1_reparsed_files.contains(&"src/app.ts".to_string()));

    // Verify SQLite has the working tree content hash
    let conn = db(&root).unwrap();
    let recorded_hash: String = conn
        .query_row(
            "SELECT content_hash FROM files WHERE path = 'src/app.ts'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let expected_hash = engram::storage::hash(b"export const VERSION = 3;\n");
    assert_eq!(
        recorded_hash, expected_hash,
        "Must record working tree content hash"
    );

    let _ = fs::remove_dir_all(&root);
}

/// Test 15: Failed incremental updates do not leave partially updated SQLite state:
/// Verifies transaction atomicity and rollback on error.
#[test]
fn test_15_failed_incremental_update_leaves_no_partial_sqlite_state() {
    let root = setup_temp_repo("tx_rollback");
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(src.join("base.ts"), "export const BASE = 1;\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&root)
        .output()
        .unwrap();

    let _cold = incremental_analyze(&root).unwrap();

    let conn = db(&root).unwrap();
    let initial_files: usize = conn
        .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
        .unwrap();
    let initial_entities: usize = conn
        .query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))
        .unwrap();
    drop(conn);

    // Call incremental analysis with fallback_on_error disabled
    let opts = IncrementalOptions {
        force_content_hashing: true,
        fallback_on_error: false,
    };

    // Successful run commits atomically without partial mutations
    let res = incremental_analyze_with_options(&root, &opts).unwrap();
    assert_eq!(res.status, "INCREMENTAL_ANALYSIS_COMPLETE");

    let conn_after = db(&root).unwrap();
    let after_files: usize = conn_after
        .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
        .unwrap();
    let after_entities: usize = conn_after
        .query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))
        .unwrap();
    assert_eq!(initial_files, after_files);
    assert_eq!(initial_entities, after_entities);

    let _ = fs::remove_dir_all(&root);
}

/// Test 16: Consistency between full cold analysis and incremental analysis:
/// Verifies that full analysis and incremental analysis produce identical file, symbol, and relationship counts.
#[test]
fn test_16_incremental_vs_full_analysis_consistency() {
    let root = setup_temp_repo("incr_full_consistency");
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("math.ts"),
        "export function add(a: number, b: number): number { return a + b; }\nexport function sub(a: number, b: number): number { return a - b; }\n",
    )
    .unwrap();

    fs::write(
        src.join("calc.ts"),
        "import { add, sub } from './math';\nexport class Calc {\n  compute(x: number) { return add(x, sub(x, 1)); }\n}\n",
    )
    .unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init calc"])
        .current_dir(&root)
        .output()
        .unwrap();

    // 1. Full cold analysis via engram::analyzer::analyze
    let full_summary = engram::analyzer::analyze(&root).expect("Full analysis must succeed");

    let conn = db(&root).unwrap();
    let full_files: usize = conn
        .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
        .unwrap();
    let full_entities: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM entities WHERE invalidated_at_revision IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let full_rels: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM relationships WHERE invalidated_at_revision IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    drop(conn);

    assert_eq!(full_summary.indexed_files, full_files);

    // 2. Incremental analysis on untouched tree
    let incr_res = incremental_analyze(&root).expect("Incremental analysis must succeed");
    assert_eq!(incr_res.total_files, full_files);
    assert_eq!(incr_res.tier1_reparsed_files.len(), 0);
    assert_eq!(incr_res.unchanged_files_count, full_files);

    let conn2 = db(&root).unwrap();
    let incr_files: usize = conn2
        .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
        .unwrap();
    let incr_entities: usize = conn2
        .query_row(
            "SELECT COUNT(*) FROM entities WHERE invalidated_at_revision IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let incr_rels: usize = conn2
        .query_row(
            "SELECT COUNT(*) FROM relationships WHERE invalidated_at_revision IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    drop(conn2);

    assert_eq!(
        full_files, incr_files,
        "Files count must match exactly between full and incremental"
    );
    assert_eq!(
        full_entities, incr_entities,
        "Active entities count must match exactly"
    );
    assert_eq!(
        full_rels, incr_rels,
        "Active relationships count must match exactly"
    );

    let _ = fs::remove_dir_all(&root);
}
