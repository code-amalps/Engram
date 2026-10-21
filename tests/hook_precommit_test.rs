use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

use engram::contract::{create_contract, save_contract, ContractStatus};
use engram::hook::{
    install_hook, resolve_hooks_dir, run_hook, uninstall_hook, HOOK_END_MARKER, HOOK_START_MARKER,
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
    fs::write(src_dir.join("lib.rs"), "pub fn add(a: i32, b: i32) -> i32 { a + b }\n").unwrap();

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
    assert_eq!(code, 1, "Forbidden staged change must exit with 1 (blocked)");

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
    let space_dir = std::env::temp_dir().join(format!("engram test repo spaces {}", Uuid::new_v4()));
    fs::create_dir_all(&space_dir).unwrap();
    init_git_in_dir(&space_dir);

    // Install hook in directory with spaces
    let install_res = install_hook(&space_dir, false);
    assert!(install_res.is_ok(), "Installation in path with spaces failed");

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
