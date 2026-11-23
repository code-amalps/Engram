use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

use engram::contract::{ContractStatus, create_contract, save_contract};
use engram::hook::{
    HOOK_END_MARKER, HOOK_START_MARKER, install_hook, resolve_hooks_dir, run_hook, uninstall_hook,
};

fn setup_test_git_repo(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("engram_precommit_{}_{}", prefix, Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    init_git_in_dir(&dir);
    dir
}

fn init_git_in_dir(dir: &Path) {
    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "Git command failed: {:?}", args);
    };

    run(&["init", "-b", "main"]);
    run(&["config", "user.name", "Engram Hook Tester"]);
    run(&["config", "user.email", "hook-tester@engram.dev"]);

    // Ignore .engram directory
    let gitignore = dir.join(".gitignore");
    fs::write(&gitignore, ".engram/\n").unwrap();
    run(&["add", ".gitignore"]);

    // Initial commit
    let init_file = dir.join("README.md");
    fs::write(&init_file, "# Hook Test Repo\n").unwrap();
    run(&["add", "README.md"]);
    run(&["commit", "-m", "Initial commit on main"]);
}

// ---------------------------------------------------------------------------
// 1. Compliant staged changes pass
// ---------------------------------------------------------------------------
#[test]
fn test_01_compliant_staged_changes_pass() {
    let root = setup_test_git_repo("compliant_pass");

    // Create contract expecting src/lib.rs
    let contract = create_contract(
        &root,
        "Implement compliant feature",
        Some(vec!["src".to_string()]),
        Some(vec!["src/lib.rs".to_string()]),
        Some(vec!["secrets/**".to_string()]),
    )
    .unwrap();
    save_contract(&root, &contract).unwrap();

    // Stage compliant change
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        src_dir.join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .unwrap();

    let add_out = Command::new("git")
        .args(["add", "src/lib.rs"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(add_out.status.success());

    // Validate staged changes: should pass (exit code 0)
    let code = run_hook(&root, false, false, None, None, false).unwrap();
    assert_eq!(code, 0, "Compliant staged change must exit with 0");

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 2. Out-of-contract staged changes are blocked
// ---------------------------------------------------------------------------
#[test]
fn test_02_out_of_contract_staged_changes_blocked() {
    let root = setup_test_git_repo("out_of_contract_blocked");

    // Create contract expecting src/lib.rs and forbidding secrets/**
    let contract = create_contract(
        &root,
        "Add core math utility",
        Some(vec!["src".to_string()]),
        Some(vec!["src/lib.rs".to_string()]),
        Some(vec!["secrets/**".to_string()]),
    )
    .unwrap();
    save_contract(&root, &contract).unwrap();

    // Stage forbidden change
    let secret_dir = root.join("secrets");
    fs::create_dir_all(&secret_dir).unwrap();
    fs::write(secret_dir.join("key.pem"), "SECRET_KEY_DATA\n").unwrap();

    let add_out = Command::new("git")
        .args(["add", "secrets/key.pem"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(add_out.status.success());

    // Validate staged changes: should be blocked (exit code 1)
    let code = run_hook(&root, false, false, None, None, false).unwrap();
    assert_eq!(
        code, 1,
        "Forbidden staged change must exit with 1 (blocked)"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 3. Unstaged unrelated changes do not affect the result
// ---------------------------------------------------------------------------
#[test]
fn test_03_unstaged_unrelated_changes_do_not_affect_result() {
    let root = setup_test_git_repo("unstaged_isolation");

    // Contract allows src/lib.rs, forbids secrets/**
    let contract = create_contract(
        &root,
        "Scoped task",
        Some(vec!["src".to_string()]),
        Some(vec!["src/lib.rs".to_string()]),
        Some(vec!["secrets/**".to_string()]),
    )
    .unwrap();
    save_contract(&root, &contract).unwrap();

    // Stage compliant change
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("lib.rs"), "pub fn ok() {}\n").unwrap();
    Command::new("git")
        .args(["add", "src/lib.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Modify forbidden file in working tree but leave UNSTAGED
    let secret_dir = root.join("secrets");
    fs::create_dir_all(&secret_dir).unwrap();
    fs::write(secret_dir.join("token.env"), "TOKEN=xyz\n").unwrap();

    // Run hook: unstaged forbidden file must be ignored, returning 0
    let code = run_hook(&root, false, false, None, None, false).unwrap();
    assert_eq!(
        code, 0,
        "Unstaged forbidden file must not block clean staged changes"
    );

    // Now stage the forbidden file
    Command::new("git")
        .args(["add", "secrets/token.env"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Run hook again: now it must be blocked (1)
    let code_after = run_hook(&root, false, false, None, None, false).unwrap();
    assert_eq!(
        code_after, 1,
        "Once staged, forbidden file must block the commit"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 4. Missing active contract follows documented policy (permissive by default,
//    blocks when enforce_contract is true)
// ---------------------------------------------------------------------------
#[test]
fn test_04_missing_active_contract_permissive_by_default_and_enforceable() {
    let root = setup_test_git_repo("no_contract_policy");

    // Stage an arbitrary file without creating any contract
    let app_file = root.join("app.rs");
    fs::write(&app_file, "fn main() {}\n").unwrap();
    Command::new("git")
        .args(["add", "app.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Default pre-commit: permissive mode allows ordinary developer commits (exit code 0)
    let code_default = run_hook(&root, false, false, None, None, false).unwrap();
    assert_eq!(
        code_default, 0,
        "Default pre-commit must permit ordinary Git workflows without contract"
    );

    // Explicit opt-in enforcement (--enforce-contract): blocks if no active contract exists (exit code 1)
    let code_enforced = run_hook(&root, false, true, None, None, false).unwrap();
    assert_eq!(
        code_enforced, 1,
        "Pre-commit with enforce_contract must block when no active contract exists"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 5. Invalid, stale, or malformed contract state fails safely
// ---------------------------------------------------------------------------
#[test]
fn test_05_malformed_corrupt_contract_fails_safely() {
    let root = setup_test_git_repo("malformed_contract");

    // Create a malformed active contract file (.engram/contracts/active.json with invalid JSON)
    let contracts_dir = root.join(".engram").join("contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    fs::write(contracts_dir.join("active.json"), "{ invalid json: 1234").unwrap();

    // Stage a file
    let file = root.join("src.rs");
    fs::write(&file, "// test\n").unwrap();
    Command::new("git")
        .args(["add", "src.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Must fail safely: block with exit code 1, does NOT silently pass
    let code = run_hook(&root, false, false, None, None, false).unwrap();
    assert_eq!(
        code, 1,
        "Malformed active contract must fail safely and block commit"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 6. Contract blocked awaiting human status fails safely
// ---------------------------------------------------------------------------
#[test]
fn test_06_blocked_awaiting_human_status_fails_safely() {
    let root = setup_test_git_repo("blocked_status");

    let mut contract = create_contract(
        &root,
        "Needs human review",
        Some(vec!["src".to_string()]),
        Some(vec!["src/lib.rs".to_string()]),
        None,
    )
    .unwrap();
    contract.status = ContractStatus::BlockedAwaitingHuman;
    save_contract(&root, &contract).unwrap();

    // Stage a file
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("lib.rs"), "pub fn test() {}\n").unwrap();
    Command::new("git")
        .args(["add", "src/lib.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Must block because contract requires human reconciliation
    let code = run_hook(&root, false, false, None, None, false).unwrap();
    assert_eq!(
        code, 1,
        "Contract in BLOCKED_AWAITING_HUMAN state must block staged commit"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 7. Contract baseline or repository mismatch handled correctly
// ---------------------------------------------------------------------------
#[test]
fn test_07_contract_baseline_mismatch_fails_safely() {
    let root = setup_test_git_repo("baseline_mismatch");

    let mut contract = create_contract(
        &root,
        "Foreign repository task",
        Some(vec!["src".to_string()]),
        Some(vec!["src/lib.rs".to_string()]),
        None,
    )
    .unwrap();
    // Non-existent commit SHA
    contract.baseline_revision = "deadbeef00000000000000000000000000000000".to_string();
    save_contract(&root, &contract).unwrap();

    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("lib.rs"), "pub fn test() {}\n").unwrap();
    Command::new("git")
        .args(["add", "src/lib.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Must block because baseline commit is unreachable in current repository
    let code = run_hook(&root, false, false, None, None, false).unwrap();
    assert_eq!(
        code, 1,
        "Contract with unreachable baseline commit must block staged commit"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 8. No user files are modified by validation
// ---------------------------------------------------------------------------
#[test]
fn test_08_no_user_files_modified_by_validation() {
    let root = setup_test_git_repo("no_file_mods");

    let contract = create_contract(
        &root,
        "Safety check",
        Some(vec!["src".to_string()]),
        Some(vec!["src/lib.rs".to_string()]),
        None,
    )
    .unwrap();
    save_contract(&root, &contract).unwrap();

    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let staged_content = "// Staged content\n";
    fs::write(src_dir.join("lib.rs"), staged_content).unwrap();
    Command::new("git")
        .args(["add", "src/lib.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    let unstaged_content = "// Unstaged content\n";
    let unstaged_file = root.join("scratch.rs");
    fs::write(&unstaged_file, unstaged_content).unwrap();

    // Get git status before validation
    let status_before = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&root)
        .output()
        .unwrap();

    let _ = run_hook(&root, false, false, None, None, false).unwrap();

    // Get git status after validation
    let status_after = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&root)
        .output()
        .unwrap();

    assert_eq!(
        status_before.stdout, status_after.stdout,
        "Git index and working tree must remain identical after validation"
    );
    assert_eq!(
        fs::read_to_string(src_dir.join("lib.rs")).unwrap(),
        staged_content
    );
    assert_eq!(
        fs::read_to_string(&unstaged_file).unwrap(),
        unstaged_content
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 9. Hook installation does not overwrite existing foreign hooks
// ---------------------------------------------------------------------------
#[test]
fn test_09_hook_installation_preserves_foreign_hooks() {
    let root = setup_test_git_repo("foreign_hook_preservation");
    let hooks_dir = root.join(".git").join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    let hook_file = hooks_dir.join("pre-commit");

    let foreign_script = "#!/bin/sh\n# Custom CI check\nnpm test\n";
    fs::write(&hook_file, foreign_script).unwrap();

    let install_msg = install_hook(&root, false).unwrap();
    assert!(install_msg.contains("Preserved existing pre-commit hook"));

    let content_after = fs::read_to_string(&hook_file).unwrap();
    assert!(content_after.contains("npm test"));
    assert!(content_after.contains(HOOK_START_MARKER));
    assert!(content_after.contains(HOOK_END_MARKER));

    let uninstall_msg = uninstall_hook(&root, false).unwrap();
    assert!(uninstall_msg.contains("preserving foreign hook content"));

    let final_content = fs::read_to_string(&hook_file).unwrap();
    assert!(final_content.contains("npm test"));
    assert!(!final_content.contains(HOOK_START_MARKER));

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 10. Repeated installation and removal are safe and idempotent
// ---------------------------------------------------------------------------
#[test]
fn test_10_repeated_installation_and_removal_are_safe_and_idempotent() {
    let root = setup_test_git_repo("idempotent_lifecycle");

    // Install 1
    let msg1 = install_hook(&root, false).unwrap();
    assert!(msg1.contains("Installed Engram pre-commit hook successfully"));

    // Install 2 (idempotent)
    let msg2 = install_hook(&root, false).unwrap();
    assert!(msg2.contains("already installed"));

    let hook_file = root.join(".git").join("hooks").join("pre-commit");
    let content = fs::read_to_string(&hook_file).unwrap();
    assert_eq!(
        content.matches(HOOK_START_MARKER).count(),
        1,
        "Marker must not be duplicated on repeated install"
    );

    // Uninstall 1
    let un1 = uninstall_hook(&root, false).unwrap();
    assert!(un1.contains("Removed empty pre-commit hook file cleanly"));
    assert!(!hook_file.exists());

    // Uninstall 2 (idempotent)
    let un2 = uninstall_hook(&root, false).unwrap();
    assert!(un2.contains("No pre-commit hook exists"));

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 11. Git paths containing spaces work
// ---------------------------------------------------------------------------
#[test]
fn test_11_git_paths_with_spaces_work() {
    let space_dir =
        std::env::temp_dir().join(format!("engram test repo spaces {}", Uuid::new_v4()));
    fs::create_dir_all(&space_dir).unwrap();
    init_git_in_dir(&space_dir);

    // Install hook in directory with spaces
    let install_res = install_hook(&space_dir, false);
    assert!(
        install_res.is_ok(),
        "Installation in path with spaces failed"
    );

    // Create contract and stage file
    let contract = create_contract(
        &space_dir,
        "Spaces test",
        Some(vec!["src".to_string()]),
        Some(vec!["src/app.rs".to_string()]),
        None,
    )
    .unwrap();
    save_contract(&space_dir, &contract).unwrap();

    let src_dir = space_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("app.rs"), "fn main() {}\n").unwrap();
    Command::new("git")
        .args(["add", "src/app.rs"])
        .current_dir(&space_dir)
        .output()
        .unwrap();

    let code = run_hook(&space_dir, false, false, None, None, false).unwrap();
    assert_eq!(code, 0, "Hook execution in path with spaces must pass");

    let _ = fs::remove_dir_all(&space_dir);
}

// ---------------------------------------------------------------------------
// 12. Custom core.hooksPath is respected
// ---------------------------------------------------------------------------
#[test]
fn test_12_custom_core_hooks_path_respected() {
    let root = setup_test_git_repo("custom_hooks_path");

    // Configure custom hooksPath
    let custom_dir_name = "custom_githooks";
    let set_cfg = Command::new("git")
        .args(["config", "core.hooksPath", custom_dir_name])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(set_cfg.status.success());

    // Resolve hooks dir
    let resolved = resolve_hooks_dir(&root).unwrap();
    assert!(
        resolved.ends_with(custom_dir_name) || resolved.to_string_lossy().contains(custom_dir_name),
        "Resolved hooks dir must point to core.hooksPath: {}",
        resolved.display()
    );

    // Install hook into custom hooksPath
    let msg = install_hook(&root, false).unwrap();
    assert!(msg.contains("Installed Engram pre-commit hook"));

    let custom_hook_file = root.join(custom_dir_name).join("pre-commit");
    assert!(
        custom_hook_file.exists(),
        "Hook file must be installed in custom hooksPath directory"
    );

    // Uninstall hook from custom hooksPath
    let un_msg = uninstall_hook(&root, false).unwrap();
    assert!(un_msg.contains("Removed empty pre-commit hook"));
    assert!(!custom_hook_file.exists());

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 13. Trivial documentation changes pass under enforce_contract (Zero-cost bypass)
// ---------------------------------------------------------------------------
#[test]
fn test_13_trivial_documentation_bypass_under_enforce_contract() {
    let root = setup_test_git_repo("doc_bypass");

    // Stage a pure documentation file without creating any contract
    let doc_file = root.join("docs").join("guide.md");
    fs::create_dir_all(doc_file.parent().unwrap()).unwrap();
    fs::write(&doc_file, "# Documentation Guide\n\nContent here.\n").unwrap();

    Command::new("git")
        .args(["add", "docs/guide.md"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Even with enforce_contract: true, pure doc edits qualify for zero-cost bypass
    let code = run_hook(&root, false, true, None, None, false).unwrap();
    assert_eq!(
        code, 0,
        "Documentation-only edits must pass with zero model tokens under enforce_contract"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 14. Trivial task bypass marker authorizes single leaf code edit and cleans up
// ---------------------------------------------------------------------------
#[test]
fn test_14_trivial_task_bypass_marker_authorizes_edit_and_cleans_up() {
    let root = setup_test_git_repo("marker_bypass");

    // Stage a single leaf file
    let leaf_file = root.join("leaf.rs");
    fs::write(&leaf_file, "// Trivial comment edit\n").unwrap();
    Command::new("git")
        .args(["add", "leaf.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Without a bypass marker, code edit with enforce_contract: true blocks
    let code_blocked = run_hook(&root, false, true, None, None, false).unwrap();
    assert_eq!(
        code_blocked, 1,
        "Code edit without bypass marker or contract must block under enforce_contract"
    );

    // Save bypass marker (as engram_engage / prompt does for Trivial tasks)
    engram::storage::save_bypass_trivial(
        &root,
        "fix comment in leaf.rs",
        "Trivial comment edit in single file",
        &["leaf.rs".to_string()],
    )
    .unwrap();

    // Now pre-commit hook must authorize the bypass and clean up the marker
    let code_passed = run_hook(&root, false, true, None, None, false).unwrap();
    assert_eq!(
        code_passed, 0,
        "Trivial task with active bypass marker must exit 0 under zero-cost bypass"
    );

    // Verify marker was cleaned up
    let marker = engram::storage::load_bypass_trivial(&root).unwrap();
    assert!(
        marker.is_none(),
        "Bypass marker must be removed after successful commit verification"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 15. Non-trivial code modification without contract is strictly blocked (Zero false bypasses)
// ---------------------------------------------------------------------------
#[test]
fn test_15_non_trivial_code_modification_without_contract_is_blocked() {
    let root = setup_test_git_repo("zero_false_bypass");

    // Stage two code files without any contract
    let f1 = root.join("mod_a.rs");
    let f2 = root.join("mod_b.rs");
    fs::write(&f1, "pub fn a() {}\n").unwrap();
    fs::write(&f2, "pub fn b() {}\n").unwrap();
    Command::new("git")
        .args(["add", "mod_a.rs", "mod_b.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Under enforce_contract: true, multiple files must never be false-bypassed
    let code = run_hook(&root, false, true, None, None, false).unwrap();
    assert_eq!(
        code, 1,
        "Multi-file code edit must NEVER qualify for trivial bypass (0.0% false bypass guarantee)"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 16. Local postflight verification resolves active recovery session (0 model tokens)
// ---------------------------------------------------------------------------
#[test]
fn test_16_local_postflight_resolves_recovery_session() {
    let root = setup_test_git_repo("local_postflight_recovery");

    let contract_id = "ctr_postflight_test_01";
    let target_file = "src/lib.rs";

    // Create target file and initial commit
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let file = src_dir.join("lib.rs");
    fs::write(&file, "// initial\n").unwrap();
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

    // Create active contract
    let mut contract = create_contract(
        &root,
        "Implement lib features",
        Some(vec!["src".to_string()]),
        Some(vec![target_file.to_string()]),
        Some(vec!["secrets/**".to_string()]),
    )
    .unwrap();
    contract.contract_id = contract_id.to_string();
    contract.status = ContractStatus::Active;
    contract.baseline_revision = "WORKTREE".to_string();
    save_contract(&root, &contract).unwrap();

    // Create an in-progress recovery session
    let mut session = engram::recovery::create_recovery_session(contract_id);
    session.status = engram::recovery::RecoveryStatus::InProgress;
    engram::recovery::save_session(&root, &session).unwrap();
    assert_eq!(session.status, engram::recovery::RecoveryStatus::InProgress);

    // Modify and stage the target file
    fs::write(&file, "// modified\n").unwrap();
    Command::new("git")
        .args(["add", "src/lib.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Run hook: pre-commit must verify contract and resolve the recovery session
    let code = run_hook(&root, false, false, None, None, false).unwrap();
    assert_eq!(
        code, 0,
        "Compliant contract commit must succeed in local postflight"
    );

    // Verify recovery session is now Resolved
    let resolved_session = engram::recovery::load_session(&root, contract_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        resolved_session.status,
        engram::recovery::RecoveryStatus::Resolved,
        "Local postflight in pre-commit hook must resolve active recovery session"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 17. Hook outputs real contract status (Active) and verification passed
// ---------------------------------------------------------------------------
#[test]
fn test_17_hook_outputs_real_contract_status() {
    let root = setup_test_git_repo("status_accuracy");
    let contract_id = "ctr_status_test_01";
    let target_file = "src/lib.rs";

    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let file = src_dir.join("lib.rs");
    fs::write(&file, "// initial\n").unwrap();
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

    let mut contract = create_contract(
        &root,
        "Test status accuracy",
        Some(vec!["src".to_string()]),
        Some(vec![target_file.to_string()]),
        None,
    )
    .unwrap();
    contract.contract_id = contract_id.to_string();
    contract.status = ContractStatus::Active;
    contract.baseline_revision = "WORKTREE".to_string();
    save_contract(&root, &contract).unwrap();

    // Stage change
    fs::write(&file, "// updated\n").unwrap();
    Command::new("git")
        .args(["add", "src/lib.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    let code = run_hook(&root, false, false, None, None, false).unwrap();
    assert_eq!(code, 0);

    // Verify contract status remains Active in storage (not corrupted or mutated to an invented state)
    let loaded = engram::contract::get_contract(&root, contract_id).unwrap();
    assert_eq!(loaded.status, ContractStatus::Active);

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 18. Hook fails closed (exit code 2) on corrupt recovery session
// ---------------------------------------------------------------------------
#[test]
fn test_18_hook_fails_closed_on_corrupt_recovery_session() {
    let root = setup_test_git_repo("corrupt_session_fail_closed");
    let contract_id = "ctr_corrupt_session_01";
    let target_file = "src/lib.rs";

    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let file = src_dir.join("lib.rs");
    fs::write(&file, "// initial\n").unwrap();
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

    let mut contract = create_contract(
        &root,
        "Test corrupt recovery fail closed",
        Some(vec!["src".to_string()]),
        Some(vec![target_file.to_string()]),
        None,
    )
    .unwrap();
    contract.contract_id = contract_id.to_string();
    contract.status = ContractStatus::Active;
    contract.baseline_revision = "WORKTREE".to_string();
    save_contract(&root, &contract).unwrap();

    // Create a corrupted session file in recovery cache dir
    let session_dir = engram::recovery::recovery_cache_dir(&root);
    fs::create_dir_all(&session_dir).unwrap();
    let session_file = engram::recovery::recovery_session_path(&root, contract_id);
    fs::write(&session_file, "{corrupt json!").unwrap();

    // Stage change
    fs::write(&file, "// updated\n").unwrap();
    Command::new("git")
        .args(["add", "src/lib.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Hook must fail closed with exit code 2
    let code = run_hook(&root, false, false, None, None, false).unwrap();
    assert_eq!(
        code, 2,
        "Corrupted recovery session MUST cause fail-closed exit 2"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 19. Trivial bypass rejects expired record
// ---------------------------------------------------------------------------
#[test]
fn test_19_bypass_rejects_expired_record() {
    let root = setup_test_git_repo("expired_bypass");
    let leaf_file = root.join("leaf.rs");
    fs::write(&leaf_file, "// edit\n").unwrap();
    Command::new("git")
        .args(["add", "leaf.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    let mut record = engram::storage::save_bypass_trivial(
        &root,
        "fix comment in leaf.rs",
        "Trivial comment edit",
        &["leaf.rs".to_string()],
    )
    .unwrap();

    // Manually expire the record in .engram/bypass_trivial.json
    record.expires_at = chrono::Utc::now().timestamp() - 100;
    fs::write(
        root.join(".engram").join("bypass_trivial.json"),
        serde_json::to_string_pretty(&record).unwrap(),
    )
    .unwrap();

    let code = run_hook(&root, false, true, None, None, false).unwrap();
    assert_eq!(code, 1, "Expired bypass record must be rejected");

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 20. Trivial bypass rejects target path mismatch
// ---------------------------------------------------------------------------
#[test]
fn test_20_bypass_rejects_target_path_mismatch() {
    let root = setup_test_git_repo("target_mismatch");

    // Save bypass authorized only for leaf_a.rs
    engram::storage::save_bypass_trivial(
        &root,
        "fix comment in leaf_a.rs",
        "Trivial comment edit",
        &["leaf_a.rs".to_string()],
    )
    .unwrap();

    // But stage leaf_b.rs instead
    let leaf_b = root.join("leaf_b.rs");
    fs::write(&leaf_b, "// edit b\n").unwrap();
    Command::new("git")
        .args(["add", "leaf_b.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    let code = run_hook(&root, false, true, None, None, false).unwrap();
    assert_eq!(code, 1, "Bypass target path mismatch must be rejected");

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 21. Trivial bypass rejects baseline commit mismatch
// ---------------------------------------------------------------------------
#[test]
fn test_21_bypass_rejects_baseline_commit_mismatch() {
    let root = setup_test_git_repo("baseline_mismatch");

    // Save bypass on initial commit
    let mut record = engram::storage::save_bypass_trivial(
        &root,
        "fix comment in leaf.rs",
        "Trivial comment edit",
        &["leaf.rs".to_string()],
    )
    .unwrap();

    // Alter recorded baseline_commit to a stale hash
    record.baseline_commit = "0000000000000000000000000000000000000000".to_string();
    fs::write(
        root.join(".engram").join("bypass_trivial.json"),
        serde_json::to_string_pretty(&record).unwrap(),
    )
    .unwrap();

    let leaf_file = root.join("leaf.rs");
    fs::write(&leaf_file, "// edit\n").unwrap();
    Command::new("git")
        .args(["add", "leaf.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    let code = run_hook(&root, false, true, None, None, false).unwrap();
    assert_eq!(
        code, 1,
        "Stale baseline commit in bypass record must be rejected"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 22. Trivial bypass cannot authorize sensitive or protected files
// ---------------------------------------------------------------------------
#[test]
fn test_22_bypass_rejects_sensitive_files() {
    let root = setup_test_git_repo("sensitive_bypass");

    // Attempting to save bypass for sensitive files must return Err
    assert!(
        engram::storage::save_bypass_trivial(
            &root,
            "edit auth",
            "reason",
            &["src/auth/token.rs".to_string()]
        )
        .is_err()
    );
    assert!(
        engram::storage::save_bypass_trivial(
            &root,
            "edit test",
            "reason",
            &["tests/test_foo.rs".to_string()]
        )
        .is_err()
    );
    assert!(
        engram::storage::save_bypass_trivial(
            &root,
            "edit manifest",
            "reason",
            &["Cargo.toml".to_string()]
        )
        .is_err()
    );
    assert!(
        engram::storage::save_bypass_trivial(
            &root,
            "edit policy",
            "reason",
            &[".engram/constitution.yaml".to_string()]
        )
        .is_err()
    );
    assert!(
        engram::storage::save_bypass_trivial(
            &root,
            "multi file",
            "reason",
            &["a.rs".to_string(), "b.rs".to_string()]
        )
        .is_err()
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 23. Atomic consumption prevents repeated bypass reuse
// ---------------------------------------------------------------------------
#[test]
fn test_23_bypass_atomic_consumption_prevents_reuse() {
    let root = setup_test_git_repo("bypass_reuse");

    // Stage leaf file
    let leaf_file = root.join("leaf.rs");
    fs::write(&leaf_file, "// first edit\n").unwrap();
    Command::new("git")
        .args(["add", "leaf.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Authorize bypass
    engram::storage::save_bypass_trivial(
        &root,
        "fix comment in leaf.rs",
        "Trivial comment edit",
        &["leaf.rs".to_string()],
    )
    .unwrap();

    // First commit verification succeeds and atomically consumes record
    let code1 = run_hook(&root, false, true, None, None, false).unwrap();
    assert_eq!(code1, 0);

    // Commit the changes in git so working tree is clean
    Command::new("git")
        .args(["commit", "-m", "first leaf edit"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Stage a second edit to the same file without creating a new bypass
    fs::write(&leaf_file, "// second edit\n").unwrap();
    Command::new("git")
        .args(["add", "leaf.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Second hook execution must block because bypass was already consumed
    let code2 = run_hook(&root, false, true, None, None, false).unwrap();
    assert_eq!(
        code2, 1,
        "Consumed bypass marker CANNOT be reused for a second commit"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 24. Malformed constitution YAML fails closed (GUARD-CONST-000)
// ---------------------------------------------------------------------------
#[test]
fn test_24_malformed_constitution_fails_closed() {
    let root = setup_test_git_repo("malformed_constitution");

    let engram_dir = root.join(".engram");
    fs::create_dir_all(&engram_dir).unwrap();
    fs::write(
        engram_dir.join("constitution.yaml"),
        "rules:\n  - id: 1\n    name: [unclosed invalid yaml",
    )
    .unwrap();

    let leaf_file = root.join("leaf.rs");
    fs::write(&leaf_file, "// test\n").unwrap();
    Command::new("git")
        .args(["add", "leaf.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    let code = run_hook(&root, false, false, None, None, false).unwrap();
    assert_eq!(code, 1, "Malformed constitution YAML MUST fail closed");

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 25. Modifying sovereign policy file in PR mode is blocked (GUARD-CONST-001)
// ---------------------------------------------------------------------------
#[test]
fn test_25_sovereign_policy_mutation_blocked_in_pr() {
    let root = setup_test_git_repo("policy_pr_block");

    // Commit a sovereign workflow file on main
    let wf_dir = root.join(".github").join("workflows");
    fs::create_dir_all(&wf_dir).unwrap();
    let wf_file = wf_dir.join("ci.yml");
    fs::write(&wf_file, "name: CI\n").unwrap();
    Command::new("git")
        .args(["add", ".github/workflows/ci.yml"])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add CI workflow"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Now modify the sovereign workflow file in PR
    fs::write(&wf_file, "name: CI Modified\n").unwrap();
    Command::new("git")
        .args(["add", ".github/workflows/ci.yml"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Run guard directly with a base_commit specified (PR mode)
    let opts = engram::ci_guard::GuardOptions {
        base_commit: Some("HEAD~1".to_string()),
        staged: true,
        strict: false,
        enforce_contract: false,
        ..Default::default()
    };
    let res = engram::ci_guard::run_guard(&root, &opts);
    assert!(
        res.blocked,
        "Modifying sovereign policy in PR mode must be blocked"
    );
    assert!(
        res.findings.iter().any(|f| f.rule_id == "GUARD-CONST-001"),
        "Must emit GUARD-CONST-001 finding"
    );

    let _ = fs::remove_dir_all(&root);
}
