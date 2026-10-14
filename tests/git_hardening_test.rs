//! Git Hardening Test Suite (Step 5 Verification):
//! - Robust handling of staged vs unstaged diffs, file renames, deletions, copies, merge commits, detached HEAD.
//! - Shallow clone fallback: detect shallow repository (.git/shallow or git rev-parse --is-shallow-repository)
//!   and degrade gracefully with SHALLOW_HISTORY_TRUNCATED rather than crashing.
//! - Baseline resolution: ensure dependable baseline commits for contract validation.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

use engram::contract::{
    check_contract, create_contract, CheckExitCode, CheckOptions,
};
use engram::git::{
    current_branch, current_revision, extract_changed_paths, get_diff_changed_files,
    is_detached_head, is_merge_commit, is_shallow_repository, resolve_baseline,
    GitFileDiffKind, SHALLOW_HISTORY_TRUNCATED,
};

fn setup_git_repo(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("engram_git_harden_{prefix}_{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();

    let run_cmd = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "Git command failed: {:?} -> {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    };

    run_cmd(&["init"]);
    run_cmd(&["config", "user.name", "Engram Test"]);
    run_cmd(&["config", "user.email", "test@engram.dev"]);

    // Initial commit
    fs::write(dir.join("README.md"), "# Test Repo\n").unwrap();
    run_cmd(&["add", "README.md"]);
    run_cmd(&["commit", "-m", "Initial commit"]);

    dir
}

#[test]
fn test_detached_head_detection_and_revision_resolution() {
    let root = setup_git_repo("detached_head");

    // Initially on a branch (main or master)
    assert!(!is_detached_head(&root));
    assert!(current_branch(&root).unwrap().is_some());

    let initial_rev = current_revision(&root).unwrap();
    assert_ne!(initial_rev, "WORKTREE");
    assert_eq!(initial_rev.len(), 40);

    // Detach HEAD
    let out = Command::new("git")
        .args(["checkout", "--detach", "HEAD"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(out.status.success());

    // Verify detached HEAD state
    assert!(is_detached_head(&root), "Repository must be recognized as detached HEAD");
    assert_eq!(
        current_branch(&root).unwrap(),
        None,
        "Branch must be None when HEAD is detached"
    );

    // Current revision must still resolve cleanly to full SHA
    let detached_rev = current_revision(&root).unwrap();
    assert_eq!(detached_rev, initial_rev);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_staged_vs_unstaged_diff_separation() {
    let root = setup_git_repo("staged_vs_unstaged");

    // Create file 1 and stage it
    fs::write(root.join("staged_file.txt"), "staged content\n").unwrap();
    Command::new("git")
        .args(["add", "staged_file.txt"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Create file 2 and modify without staging
    fs::write(root.join("unstaged_file.txt"), "unstaged content\n").unwrap();

    // 1. Query staged changes only
    let (staged_files, _) = extract_changed_paths(&root, None, true, None)
        .unwrap()
        .expect("Staged diff must succeed");

    assert!(
        staged_files.contains(&"staged_file.txt".to_string()),
        "Staged query must contain staged_file.txt"
    );
    assert!(
        !staged_files.contains(&"unstaged_file.txt".to_string()),
        "Staged query must NOT contain unstaged_file.txt"
    );

    // 2. Query all working tree changes
    let (all_files, _) = extract_changed_paths(&root, None, false, None)
        .unwrap()
        .expect("Working tree query must succeed");

    assert!(all_files.contains(&"staged_file.txt".to_string()));
    assert!(all_files.contains(&"unstaged_file.txt".to_string()));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_file_rename_and_deletion_handling() {
    let root = setup_git_repo("rename_delete");

    // Commit two base files
    fs::write(root.join("original.txt"), "hello world\n").unwrap();
    fs::write(root.join("to_delete.txt"), "delete me\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Add base files"])
        .current_dir(&root)
        .output()
        .unwrap();

    // 1. Rename original.txt -> renamed.txt using git mv
    Command::new("git")
        .args(["mv", "original.txt", "renamed.txt"])
        .current_dir(&root)
        .output()
        .unwrap();

    // 2. Delete to_delete.txt using git rm
    Command::new("git")
        .args(["rm", "to_delete.txt"])
        .current_dir(&root)
        .output()
        .unwrap();

    let (changed_files, _) = get_diff_changed_files(&root, None, true, None)
        .unwrap()
        .expect("Diff must succeed");

    // Verify rename categorization
    let rename_entry = changed_files.iter().find(|f| f.path == "renamed.txt");
    assert!(rename_entry.is_some(), "Must track destination of rename");
    match &rename_entry.unwrap().kind {
        GitFileDiffKind::Renamed { old_path } => {
            assert_eq!(old_path, "original.txt");
        }
        other => panic!("Expected Renamed kind, got {:?}", other),
    }

    // Verify delete categorization
    let delete_entry = changed_files.iter().find(|f| f.path == "to_delete.txt");
    assert!(delete_entry.is_some(), "Must track deleted file");
    assert_eq!(delete_entry.unwrap().kind, GitFileDiffKind::Deleted);

    // Verify extract_changed_paths includes both renamed target and deleted file
    let (paths, _) = extract_changed_paths(&root, None, true, None)
        .unwrap()
        .expect("Extract paths must succeed");
    assert!(paths.contains(&"renamed.txt".to_string()));
    assert!(paths.contains(&"to_delete.txt".to_string()));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_merge_commit_detection_and_handling() {
    let root = setup_git_repo("merge_commit");

    // Create a feature branch
    Command::new("git")
        .args(["checkout", "-b", "feature/orders"])
        .current_dir(&root)
        .output()
        .unwrap();

    fs::write(root.join("orders.txt"), "order processing\n").unwrap();
    Command::new("git")
        .args(["add", "orders.txt"])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Feature commit"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Switch back to master/main
    Command::new("git")
        .args(["checkout", "-"])
        .current_dir(&root)
        .output()
        .unwrap();

    fs::write(root.join("main_update.txt"), "main branch work\n").unwrap();
    Command::new("git")
        .args(["add", "main_update.txt"])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Main update"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Merge feature branch with a merge commit
    let merge_out = Command::new("git")
        .args(["merge", "--no-ff", "feature/orders", "-m", "Merge orders feature"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(merge_out.status.success());

    let head_rev = current_revision(&root).unwrap();

    // Verify it is recognized as a merge commit
    assert!(
        is_merge_commit(&root, &head_rev),
        "HEAD must be recognized as a merge commit"
    );

    // Resolving baseline against merge commit HEAD works seamlessly
    let resolved = resolve_baseline(&root, &head_rev).unwrap();
    assert_eq!(resolved, Ok(head_rev));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_shallow_clone_detection_and_graceful_fallback() {
    let root = setup_git_repo("shallow_fallback");

    // Initially not shallow
    assert!(!is_shallow_repository(&root));

    // Simulate shallow clone by creating .git/shallow marker file
    let git_dir = root.join(".git");
    let shallow_marker = git_dir.join("shallow");
    fs::write(&shallow_marker, "0000000000000000000000000000000000000001\n").unwrap();

    // Verify shallow repository is detected
    assert!(
        is_shallow_repository(&root),
        "Repository with .git/shallow must be detected as shallow"
    );

    // Check baseline resolution on a missing commit in a shallow repository:
    // must return SHALLOW_HISTORY_TRUNCATED rather than crashing
    let missing_sha = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    let baseline_res = resolve_baseline(&root, missing_sha).unwrap();
    assert_eq!(
        baseline_res,
        Err(SHALLOW_HISTORY_TRUNCATED.to_string()),
        "Missing baseline in shallow repository must emit SHALLOW_HISTORY_TRUNCATED"
    );

    // Contract evaluation against shallow repository with missing baseline
    let mut contract = create_contract(
        &root,
        "Shallow contract task",
        Some(vec!["orders".to_string()]),
        Some(vec!["src/orders/service.rs".to_string()]),
        None,
    )
    .unwrap();

    contract.baseline_revision = missing_sha.to_string();
    engram::contract::save_contract(&root, &contract).unwrap();

    let opts = CheckOptions::default();
    let res = check_contract(&root, &opts);

    // Verify graceful degradation:
    // 1. Zero crash / zero panic
    // 2. Status is FAILED
    // 3. Exit code is BaselineUnavailable
    // 4. Failure reason explicitly declares SHALLOW_HISTORY_TRUNCATED
    assert_eq!(res.status, "FAILED");
    assert_eq!(res.exit_code, CheckExitCode::BaselineUnavailable.as_i32());
    assert!(
        res.failure_reasons.iter().any(|r| r.contains(SHALLOW_HISTORY_TRUNCATED)),
        "Failure reasons must clearly report SHALLOW_HISTORY_TRUNCATED, got: {:?}",
        res.failure_reasons
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_baseline_resolution_and_validation() {
    let root = setup_git_repo("baseline_res");

    // 1. WORKTREE baseline
    assert_eq!(
        resolve_baseline(&root, "WORKTREE").unwrap(),
        Ok("WORKTREE".to_string())
    );
    assert_eq!(
        resolve_baseline(&root, "").unwrap(),
        Ok("WORKTREE".to_string())
    );

    // 2. Real commit SHA
    let rev = current_revision(&root).unwrap();
    assert_eq!(
        resolve_baseline(&root, &rev).unwrap(),
        Ok(rev.clone())
    );

    // 3. Unknown commit SHA in non-shallow repo: returns clear error
    let unknown_sha = "abcdefabcdefabcdefabcdefabcdefabcdefabcd";
    let res = resolve_baseline(&root, unknown_sha).unwrap();
    assert!(res.is_err());
    let err_msg = res.unwrap_err();
    assert!(err_msg.contains("unavailable in current Git history"));
    assert!(!err_msg.contains(SHALLOW_HISTORY_TRUNCATED));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_untracked_nested_directory_and_files_handled_individually() {
    let root = setup_git_repo("nested_untracked");

    // Create nested subdirectories with multiple files inside
    let nested_dir = root.join("src").join("orders").join("deep");
    fs::create_dir_all(&nested_dir).unwrap();
    fs::write(nested_dir.join("file_a.ts"), "export const a = 1;\n").unwrap();
    fs::write(nested_dir.join("file_b.ts"), "export const b = 2;\n").unwrap();

    let (paths, _) = extract_changed_paths(&root, None, false, None)
        .unwrap()
        .expect("Working tree query must succeed");

    // Both files must be individually present, NOT aggregated as "src/" or "src/orders/"
    assert!(
        paths.contains(&"src/orders/deep/file_a.ts".to_string()),
        "Must contain individual file_a.ts, got: {:?}", paths
    );
    assert!(
        paths.contains(&"src/orders/deep/file_b.ts".to_string()),
        "Must contain individual file_b.ts, got: {:?}", paths
    );
    assert!(
        !paths.contains(&"src/orders/deep".to_string()) && !paths.contains(&"src".to_string()),
        "Must not contain raw directory paths, got: {:?}", paths
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_dirty_baseline_resolution() {
    let root = setup_git_repo("dirty_baseline");
    let rev = current_revision(&root).unwrap();

    // Baseline string produced by storage::revision with dirty marker
    let dirty_baseline = format!("{rev}+dirty:1234567890abcdef");
    let resolved = resolve_baseline(&root, &dirty_baseline).unwrap();

    assert_eq!(
        resolved,
        Ok(rev),
        "Dirty baseline must resolve cleanly to underlying commit SHA"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_submodule_gitdir_shallow_detection() {
    let dir = std::env::temp_dir().join(format!("engram_submodule_test_{}", Uuid::new_v4()));
    let git_modules_dir = dir.join(".git_modules_target");
    let fake_submodule_dir = dir.join("my_submodule");
    fs::create_dir_all(&git_modules_dir).unwrap();
    fs::create_dir_all(&fake_submodule_dir).unwrap();

    // In a submodule, .git is a text file containing "gitdir: <path>"
    fs::write(
        fake_submodule_dir.join(".git"),
        "gitdir: ../.git_modules_target\n",
    ).unwrap();

    // Target does not have shallow yet
    assert!(!is_shallow_repository(&fake_submodule_dir));

    // Now write shallow marker in gitdir
    fs::write(git_modules_dir.join("shallow"), "1111111111111111111111111111111111111111\n").unwrap();
    assert!(
        is_shallow_repository(&fake_submodule_dir),
        "Submodule with shallow marker in gitdir must be detected as shallow"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_rename_with_spaces_in_path() {
    let root = setup_git_repo("rename_spaces");

    fs::write(root.join("old file with spaces.txt"), "hello world\n").unwrap();
    Command::new("git").args(["add", "."]).current_dir(&root).output().unwrap();
    Command::new("git").args(["commit", "-m", "Add space file"]).current_dir(&root).output().unwrap();

    Command::new("git")
        .args(["mv", "old file with spaces.txt", "new file with spaces.txt"])
        .current_dir(&root)
        .output()
        .unwrap();

    let (paths, _) = extract_changed_paths(&root, None, true, None)
        .unwrap()
        .expect("Extract changed paths should succeed");

    assert!(paths.contains(&"new file with spaces.txt".to_string()));
    assert!(paths.contains(&"old file with spaces.txt".to_string()));

    let _ = fs::remove_dir_all(&root);
}
