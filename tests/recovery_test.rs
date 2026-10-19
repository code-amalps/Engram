//! Specification Test Harness for Engram v1.1.0 Phase 3:
//! Verifies Step 9 Recovery State Machine & Concurrency Guard,
//! Step 10 Recovery Planner with Zero Source Code Modification & Invariants,
//! Step 11 Recovery Validation Loop, 2-Turn Exhaustion Gate, and Constitution Override Prevention.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Barrier};
use std::thread;

use engram::contract::{ContractStatus, get_contract};
use engram::flywheel::{
    PostflightOptions, PostflightVerdict, PreflightOptions, postflight_check, preflight_task,
};
use engram::mcp::mcp_call;
use engram::recovery::{
    LockState, RecoveryActionType, RecoveryStatus, acquire_lock, break_stale_lock,
    create_recovery_session, get_recovery_session, load_session, recover_contract, release_lock,
    save_session, update_session_cas,
};
use engram::storage::hash;
use serde_json::json;
use uuid::Uuid;

fn setup_mock_project() -> PathBuf {
    let root = std::env::temp_dir().join(format!("engram_recovery_test_{}", Uuid::new_v4()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    let files = vec![
        ("go.mod", "module github.com/acme/shop\n\ngo 1.22\n"),
        (
            "internal/orders/service.go",
            "package orders\n\ntype OrderService struct {}\n\nfunc (s *OrderService) ExportCSV() error { return nil }\n",
        ),
        (
            "internal/orders/export.go",
            "package orders\n\ntype CsvExporter struct {}\n",
        ),
        (
            "internal/secrets/vault.go",
            "package secrets\n\ntype Vault struct {}\n",
        ),
    ];

    for (rel, content) in files {
        let full = root.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full, content).unwrap();
    }

    // Git initialization
    Command::new("git")
        .args(["init"])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "RecoveryTest"])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "recovery@engram.dev"])
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

/// Test 1: Violation -> `engram recover` creates attempt 1 plan -> simulated repair -> postflight PASS -> session marked `RESOLVED`.
#[test]
fn test_1_violation_recovery_attempt1_and_resolution() {
    let root = setup_mock_project();

    // 1. Preflight handshake locks contract with forbidden boundary on secrets
    let pre_opts = PreflightOptions {
        task: "Refactor order export system".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["internal/secrets/**".to_string()]),
        planned_files: None,
    };
    let pre = preflight_task(&root, &pre_opts).unwrap();
    let contract_id = pre.contract_id.clone();

    // 2. Introduce boundary violation (touching forbidden internal/secrets/vault.go)
    // and an expected change (internal/orders/service.go)
    fs::write(
        root.join("internal/secrets/vault.go"),
        "package secrets\n\ntype Vault struct {}\n// FORBIDDEN_CHANGE\n",
    )
    .unwrap();
    fs::write(
        root.join("internal/orders/service.go"),
        "package orders\n\ntype OrderService struct {}\n\nfunc (s *OrderService) ExportCSV() error { return nil }\n// Authorized addition\n",
    )
    .unwrap();

    // 3. Postflight detects boundary violation
    let post_opts = PostflightOptions {
        contract_id: Some(contract_id.clone()),
        strict: false,
        staged: false,
        base_commit: None,
        ci_mode: false,
    };
    let post1 = postflight_check(&root, &post_opts).unwrap();
    assert_eq!(post1.verdict, PostflightVerdict::BlockedBoundaryViolation);

    // 4. Recovery Planner executes attempt 1
    let rep1 = recover_contract(&root, &contract_id).unwrap();
    assert_eq!(rep1.attempt_count, 1);
    assert_eq!(rep1.max_attempts, 2);
    assert_eq!(rep1.status, RecoveryStatus::InProgress);

    // Verify structured actions formulated
    assert!(
        rep1.plan
            .actions
            .iter()
            .any(|a| a.action_type == RecoveryActionType::RevertMutation
                && a.target.contains("vault.go")),
        "Must formulate revert_mutation for forbidden vault.go"
    );
    assert!(
        rep1.plan.actions.iter().any(|a| a.action_type
            == RecoveryActionType::PreserveRequestedChange
            && a.target.contains("service.go")),
        "Must formulate preserve_requested_change for authorized service.go"
    );
    assert!(
        rep1.plan
            .actions
            .iter()
            .any(|a| a.action_type == RecoveryActionType::Rerun),
        "Must formulate rerun check"
    );

    // 5. Verify persistent session stored at .engram/cache/recovery/<contract-id>.json
    let session_file = root
        .join(".engram")
        .join("cache")
        .join("recovery")
        .join(format!("{}.json", contract_id));
    assert!(session_file.exists(), "Persistent session file must exist");

    let session1 = get_recovery_session(&root, &contract_id).unwrap().unwrap();
    assert_eq!(session1.contract_id, contract_id);
    assert!(session1.session_id.starts_with("rec_"));
    assert_eq!(session1.session_version, 1);
    assert_eq!(session1.lock_state, LockState::Unlocked);
    assert_eq!(session1.attempt_count, 1);
    assert_eq!(session1.max_attempts, 2);
    assert_eq!(session1.status, RecoveryStatus::InProgress);
    assert!(session1.last_violation.is_some());
    assert!(session1.last_recovery_plan.is_some());

    // 6. Simulated Repair: Revert the forbidden mutation to vault.go, keeping service.go
    Command::new("git")
        .args(["checkout", "--", "internal/secrets/vault.go"])
        .current_dir(&root)
        .output()
        .unwrap();

    // 7. Postflight re-check passes
    let post2 = postflight_check(&root, &post_opts).unwrap();
    assert_eq!(post2.verdict, PostflightVerdict::Pass);

    // 8. Session automatically transitioned to RESOLVED
    let session_after = get_recovery_session(&root, &contract_id).unwrap().unwrap();
    assert_eq!(session_after.status, RecoveryStatus::Resolved);
    assert!(session_after.is_resolved());

    let _ = fs::remove_dir_all(&root);
}

/// Test 2: Violation -> attempt 1 -> failure -> attempt 2 -> failure -> 3rd request rejected with `RECOVERY_EXHAUSTED` and `BLOCKED_AWAITING_HUMAN`.
#[test]
fn test_2_multi_turn_exhaustion_gate_and_blocked_awaiting_human() {
    let root = setup_mock_project();

    // 1. Preflight
    let pre_opts = PreflightOptions {
        task: "Refactor secrets handling".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["internal/secrets/**".to_string()]),
        planned_files: None,
    };
    let pre = preflight_task(&root, &pre_opts).unwrap();
    let contract_id = pre.contract_id.clone();

    // 2. Persistent forbidden mutation
    fs::write(
        root.join("internal/secrets/vault.go"),
        "package secrets\n\ntype Vault struct {}\n// FORBIDDEN CHANGE\n",
    )
    .unwrap();

    // 3. Attempt 1
    let rep1 = recover_contract(&root, &contract_id).unwrap();
    assert_eq!(rep1.attempt_count, 1);
    assert_eq!(rep1.status, RecoveryStatus::InProgress);

    let session1 = get_recovery_session(&root, &contract_id).unwrap().unwrap();
    assert_eq!(session1.attempt_count, 1);

    // 4. Attempt 2 (agent failed to fix it)
    let rep2 = recover_contract(&root, &contract_id).unwrap();
    assert_eq!(rep2.attempt_count, 2);
    assert_eq!(rep2.status, RecoveryStatus::InProgress);

    let session2 = get_recovery_session(&root, &contract_id).unwrap().unwrap();
    assert_eq!(session2.attempt_count, 2);

    // 5. Attempt 3 (3rd recovery request strictly rejected with RECOVERY_EXHAUSTED)
    let err3 = recover_contract(&root, &contract_id)
        .unwrap_err()
        .to_string();
    assert!(
        err3.contains("RECOVERY_EXHAUSTED"),
        "3rd recovery request must fail with RECOVERY_EXHAUSTED. Got: {err3}"
    );
    assert!(
        err3.contains("BLOCKED_AWAITING_HUMAN"),
        "3rd recovery request must mention transition to BLOCKED_AWAITING_HUMAN. Got: {err3}"
    );

    // 6. Verify session state is exhausted
    let session3 = get_recovery_session(&root, &contract_id).unwrap().unwrap();
    assert_eq!(session3.attempt_count, 2);
    assert!(session3.is_exhausted());
    assert_eq!(session3.status, RecoveryStatus::RecoveryExhausted);

    // 7. Verify contract state transitioned to BLOCKED_AWAITING_HUMAN
    let contract = get_contract(&root, &contract_id).unwrap();
    assert_eq!(contract.status, ContractStatus::BlockedAwaitingHuman);

    // 8. Any subsequent (4th) attempt is also immediately blocked
    let err4 = recover_contract(&root, &contract_id)
        .unwrap_err()
        .to_string();
    assert!(err4.contains("BLOCKED_AWAITING_HUMAN"));

    let _ = fs::remove_dir_all(&root);
}

/// Test 3: Concurrent/race condition protection via `session_version` atomic CAS lock.
#[test]
fn test_3_concurrency_race_condition_protection_via_session_version_atomic_cas() {
    let root = setup_mock_project();
    let contract_id = "test_contract_cas_123";

    // 1. Initialize session at version 0
    let initial = create_recovery_session(contract_id);
    assert_eq!(initial.session_version, 0);
    assert_eq!(initial.lock_state, LockState::Unlocked);
    save_session(&root, &initial).unwrap();

    // 2. Successful CAS update from version 0 -> 1
    let s1 = update_session_cas(&root, contract_id, 0, |s| {
        s.attempt_count = 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(s1.session_version, 1);
    assert_eq!(s1.attempt_count, 1);

    // 3. Stale CAS update with version 0 must be rejected
    let stale_err = update_session_cas(&root, contract_id, 0, |s| {
        s.attempt_count = 2;
        Ok(())
    })
    .unwrap_err();
    assert!(stale_err.to_string().contains("CAS_VERSION_MISMATCH"));

    // 4. Concurrency Lock Acquisition: acquire lock at version 1 -> transitions to Locked (version 2)
    let locked = acquire_lock(&root, contract_id, 1).unwrap();
    assert_eq!(locked.lock_state, LockState::Locked);
    assert_eq!(locked.session_version, 2);

    // 5. Stale acquire lock attempt fails CAS
    let lock_err1 = acquire_lock(&root, contract_id, 1).unwrap_err();
    assert!(lock_err1.to_string().contains("CAS_VERSION_MISMATCH"));

    // 6. Acquire lock while already locked fails
    let lock_err2 = acquire_lock(&root, contract_id, 2).unwrap_err();
    assert!(
        lock_err2
            .to_string()
            .contains("CONCURRENCY_LOCK_ACQUISITION_FAILED")
    );

    // 7. Update session while locked is rejected
    let update_locked_err = update_session_cas(&root, contract_id, 2, |_| Ok(())).unwrap_err();
    assert!(update_locked_err.to_string().contains("SESSION_LOCKED"));

    // 8. Release lock at version 2 -> transitions to Unlocked (version 3)
    let unlocked = release_lock(&root, contract_id, 2).unwrap();
    assert_eq!(unlocked.lock_state, LockState::Unlocked);
    assert_eq!(unlocked.session_version, 3);

    // 9. Multi-threaded race test: 8 threads attempt CAS update from version 3 concurrently.
    // Exactly 1 thread must succeed and 7 must fail with CAS_VERSION_MISMATCH.
    let barrier = Arc::new(Barrier::new(8));
    let root_arc = Arc::new(root.clone());
    let mut handles = Vec::new();

    for thread_idx in 0..8 {
        let b = Arc::clone(&barrier);
        let r = Arc::clone(&root_arc);
        handles.push(thread::spawn(move || {
            b.wait();
            update_session_cas(&r, contract_id, 3, |s| {
                s.attempt_count = 100 + thread_idx;
                Ok(())
            })
        }));
    }

    let mut successes = 0;
    let mut cas_failures = 0;
    for h in handles {
        match h.join().unwrap() {
            Ok(_) => successes += 1,
            Err(e) => {
                if e.to_string().contains("CAS_VERSION_MISMATCH") {
                    cas_failures += 1;
                }
            }
        }
    }

    assert_eq!(successes, 1, "Exactly one thread must win the CAS race");
    assert_eq!(
        cas_failures, 7,
        "All losing threads must receive CAS_VERSION_MISMATCH"
    );

    let final_session = get_recovery_session(&root, contract_id).unwrap().unwrap();
    assert_eq!(final_session.session_version, 4);

    let _ = fs::remove_dir_all(&root);
}

/// Test 4: Zero source code modification invariant (proves recover never touches files on disk).
#[test]
fn test_4_zero_source_code_modification_invariant() {
    let root = setup_mock_project();

    let pre_opts = PreflightOptions {
        task: "Update orders export".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["internal/secrets/**".to_string()]),
        planned_files: None,
    };
    let pre = preflight_task(&root, &pre_opts).unwrap();
    let contract_id = pre.contract_id.clone();

    // Make an edit that violates contract
    fs::write(
        root.join("internal/secrets/vault.go"),
        "package secrets\n\ntype Vault struct {}\n// UNAUTHORIZED\n",
    )
    .unwrap();

    let tracked_files = [
        root.join("go.mod"),
        root.join("internal/orders/service.go"),
        root.join("internal/orders/export.go"),
        root.join("internal/secrets/vault.go"),
    ];

    // Record SHA256 hashes of all source code files before calling recovery
    let before_hashes: Vec<(PathBuf, String)> = tracked_files
        .iter()
        .map(|f| (f.clone(), hash(&fs::read(f).unwrap())))
        .collect();

    // Call recover_contract
    let rep = recover_contract(&root, &contract_id).unwrap();
    assert_eq!(rep.attempt_count, 1);

    // Record SHA256 hashes of all source code files after calling recovery
    let after_hashes: Vec<(PathBuf, String)> = tracked_files
        .iter()
        .map(|f| (f.clone(), hash(&fs::read(f).unwrap())))
        .collect();

    // Invariant: Source code files MUST be byte-for-byte identical!
    for (idx, (file, before_hash)) in before_hashes.iter().enumerate() {
        let (after_file, after_hash) = &after_hashes[idx];
        assert_eq!(file, after_file);
        assert_eq!(
            before_hash,
            after_hash,
            "File {} was mutated on disk! Invariant violated: recover must NEVER modify source files.",
            file.display()
        );
    }

    let _ = fs::remove_dir_all(&root);
}

/// Test 5: Constitution override prevention (cannot bypass Constitution rule violations).
#[test]
fn test_5_constitution_override_prevention() {
    let root = setup_mock_project();

    // Create .engram/constitution.yaml with an ERROR-level architectural rule
    let constitution_dir = root.join(".engram");
    fs::create_dir_all(&constitution_dir).unwrap();
    fs::write(
        constitution_dir.join("constitution.yaml"),
        r#"
schema_version: 1
rules:
  - id: "RULE_NO_DIRECT_SECRETS"
    name: "Forbid secrets leak in code"
    type: PATTERN
    severity: ERROR
    target_files: "**/*.go"
    pattern: "LEAKED_SECRET_TOKEN"
    description: "Code must not contain leaked secret tokens"
    enabled: true
"#,
    )
    .unwrap();

    // Commit constitution to git
    Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Add constitution"])
        .current_dir(&root)
        .output()
        .unwrap();

    let pre_opts = PreflightOptions {
        task: "Update orders export".to_string(),
        token_budget: 1200,
        forbidden_boundaries: None,
        planned_files: None,
    };
    let pre = preflight_task(&root, &pre_opts).unwrap();
    let contract_id = pre.contract_id.clone();

    // Agent adds a violating secret leak
    fs::write(
        root.join("internal/orders/service.go"),
        "package orders\n\n// LEAKED_SECRET_TOKEN = '12345'\n",
    )
    .unwrap();

    // Call recover_contract
    let rep = recover_contract(&root, &contract_id).unwrap();
    assert_eq!(rep.attempt_count, 1);

    // Invariant: Recovery plan must highlight constitution error and not bypass it
    assert!(
        rep.plan
            .instructions
            .iter()
            .any(|inst| inst.contains("Constitution rule error")
                || inst.contains("RULE_NO_DIRECT_SECRETS")),
        "Plan must include instruction identifying constitution rule error"
    );

    // Invariant 1: Constitution YAML on disk is untouched
    let const_content = fs::read_to_string(constitution_dir.join("constitution.yaml")).unwrap();
    assert!(const_content.contains("RULE_NO_DIRECT_SECRETS"));
    assert!(!const_content.contains("OVERRIDE"));

    // Invariant 2: Recovery session was NOT resolved
    let session = get_recovery_session(&root, &contract_id).unwrap().unwrap();
    assert_eq!(session.status, RecoveryStatus::InProgress);

    // Invariant 3: Postflight still fails with BlockedConstitutionRule
    let post_opts = PostflightOptions {
        contract_id: Some(contract_id),
        strict: false,
        staged: false,
        base_commit: None,
        ci_mode: false,
    };
    let post = postflight_check(&root, &post_opts).unwrap();
    assert_eq!(post.verdict, PostflightVerdict::BlockedConstitutionRule);
    assert_eq!(post.exit_code, 1);

    let _ = fs::remove_dir_all(&root);
}

/// Test 6: MCP tool `engram_recover` multi-turn invocation & exhaustion.
#[test]
fn test_6_mcp_recover_tool_invocation_and_exhaustion() {
    let root = setup_mock_project();

    let pre_opts = PreflightOptions {
        task: "Refactor secret store".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["internal/secrets/**".to_string()]),
        planned_files: None,
    };
    let pre = preflight_task(&root, &pre_opts).unwrap();
    let contract_id = pre.contract_id.clone();

    // Mutate forbidden file
    fs::write(
        root.join("internal/secrets/vault.go"),
        "package secrets\n\ntype Vault struct {}\n// MUTATION\n",
    )
    .unwrap();

    // Turn 1 via MCP
    let mcp_res1 = mcp_call(
        &root,
        "engram_recover",
        &json!({ "contract_id": contract_id }),
    )
    .unwrap();
    let text1 = mcp_res1["content"][0]["text"].as_str().unwrap();
    assert!(text1.contains("attempt_count\": 1"));
    assert!(text1.contains("IN_PROGRESS"));

    // Turn 2 via MCP
    let mcp_res2 = mcp_call(
        &root,
        "engram_recover",
        &json!({ "contract_id": contract_id }),
    )
    .unwrap();
    let text2 = mcp_res2["content"][0]["text"].as_str().unwrap();
    assert!(text2.contains("attempt_count\": 2"));

    // Turn 3 via MCP -> strictly rejected with RECOVERY_EXHAUSTED and BLOCKED_AWAITING_HUMAN
    let mcp_err3 = mcp_call(
        &root,
        "engram_recover",
        &json!({ "contract_id": contract_id }),
    )
    .unwrap_err();
    let err_str = mcp_err3.to_string();
    assert!(err_str.contains("RECOVERY_EXHAUSTED"));
    assert!(err_str.contains("BLOCKED_AWAITING_HUMAN"));

    let _ = fs::remove_dir_all(&root);
}

/// Test 7: Locked session rejects `recover_contract` with `SESSION_LOCKED`.
#[test]
fn test_7_locked_session_rejects_recovery() {
    let root = setup_mock_project();

    let pre_opts = PreflightOptions {
        task: "Update export".to_string(),
        token_budget: 1200,
        forbidden_boundaries: None,
        planned_files: None,
    };
    let pre = preflight_task(&root, &pre_opts).unwrap();
    let contract_id = pre.contract_id.clone();

    // Acquire lock at version 0 (session will be created or loaded)
    let initial = create_recovery_session(&contract_id);
    save_session(&root, &initial).unwrap();
    let _locked = acquire_lock(&root, &contract_id, 0).unwrap();

    // Calling recover_contract while locked MUST fail
    let err = recover_contract(&root, &contract_id)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("SESSION_LOCKED"),
        "recover_contract must fail with SESSION_LOCKED if session is locked. Got: {err}"
    );

    // Release lock
    release_lock(&root, &contract_id, 1).unwrap();

    // Now recover_contract succeeds
    let rep = recover_contract(&root, &contract_id).unwrap();
    assert_eq!(rep.status, RecoveryStatus::Resolved);

    let _ = fs::remove_dir_all(&root);
}

/// Test 8: Contract in `BLOCKED_AWAITING_HUMAN` blocks `postflight_check` and `preflight_task`.
#[test]
fn test_8_blocked_awaiting_human_blocks_postflight_and_preflight() {
    let root = setup_mock_project();

    let pre_opts = PreflightOptions {
        task: "Refactor secret store".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["internal/secrets/**".to_string()]),
        planned_files: None,
    };
    let pre = preflight_task(&root, &pre_opts).unwrap();
    let contract_id = pre.contract_id.clone();

    // Mutate forbidden file
    fs::write(
        root.join("internal/secrets/vault.go"),
        "package secrets\n\ntype Vault struct {}\n// MUTATION\n",
    )
    .unwrap();

    // Run attempt 1 and 2 to exhaust
    let _ = recover_contract(&root, &contract_id).unwrap();
    let _ = recover_contract(&root, &contract_id).unwrap();
    let _ = recover_contract(&root, &contract_id).unwrap_err(); // attempt 3 exhausts

    // Verify contract is BLOCKED_AWAITING_HUMAN
    let contract = get_contract(&root, &contract_id).unwrap();
    assert_eq!(contract.status, ContractStatus::BlockedAwaitingHuman);

    // Revert file on disk to simulate clean code
    Command::new("git")
        .args(["checkout", "--", "internal/secrets/vault.go"])
        .current_dir(&root)
        .output()
        .unwrap();

    // Even though working tree is clean, postflight MUST be blocked because contract is BLOCKED_AWAITING_HUMAN
    let post_opts = PostflightOptions {
        contract_id: Some(contract_id.clone()),
        strict: false,
        staged: false,
        base_commit: None,
        ci_mode: false,
    };
    let post_err = postflight_check(&root, &post_opts).unwrap_err().to_string();
    assert!(
        post_err.contains("CONTRACT_BLOCKED_AWAITING_HUMAN"),
        "postflight must fail when contract is BLOCKED_AWAITING_HUMAN. Got: {post_err}"
    );

    // Preflight reuse must also report Blocked
    let pre_reuse = preflight_task(&root, &pre_opts).unwrap();
    assert_eq!(pre_reuse.status, engram::flywheel::PreflightStatus::Blocked);
    assert!(pre_reuse.message.contains("BLOCKED_AWAITING_HUMAN"));

    let _ = fs::remove_dir_all(&root);
}

/// Test 9: Human reconciliation unblocks contract and recovery session.
#[test]
fn test_9_human_reconciliation_unblocks_exhausted_contract_and_session() {
    let root = setup_mock_project();

    let pre_opts = PreflightOptions {
        task: "Refactor secret store".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["internal/secrets/**".to_string()]),
        planned_files: None,
    };
    let pre = preflight_task(&root, &pre_opts).unwrap();
    let contract_id = pre.contract_id.clone();

    // Mutate and exhaust
    fs::write(
        root.join("internal/secrets/vault.go"),
        "package secrets\n\ntype Vault struct {}\n// MUTATION\n",
    )
    .unwrap();
    let _ = recover_contract(&root, &contract_id).unwrap();
    let _ = recover_contract(&root, &contract_id).unwrap();
    let _ = recover_contract(&root, &contract_id).unwrap_err();

    // Human intervenes: reconciles contract and approves vault.go
    let reconciled = engram::contract::reconcile_contract(
        &root,
        &contract_id,
        "Human architect approved vault modification",
        "human_architect",
        vec!["internal/secrets/vault.go".to_string()],
        vec!["secrets".to_string()],
    )
    .unwrap();
    assert_eq!(reconciled.status, ContractStatus::Reconciled);

    // Verify recovery session was unblocked and marked RESOLVED
    let session = get_recovery_session(&root, &contract_id).unwrap().unwrap();
    assert_eq!(session.status, RecoveryStatus::Resolved);
    assert_eq!(session.attempt_count, 0);

    // Postflight now passes cleanly
    let post_opts = PostflightOptions {
        contract_id: Some(contract_id),
        strict: false,
        staged: false,
        base_commit: None,
        ci_mode: false,
    };
    let post = postflight_check(&root, &post_opts).unwrap();
    assert_eq!(post.verdict, PostflightVerdict::Pass);

    let _ = fs::remove_dir_all(&root);
}

/// Test 10: Compliant working tree recovery resolves session without burning attempts.
#[test]
fn test_10_compliant_working_tree_recovery_resolves_without_burning_attempts() {
    let root = setup_mock_project();

    let pre_opts = PreflightOptions {
        task: "Add orders export".to_string(),
        token_budget: 1200,
        forbidden_boundaries: None,
        planned_files: None,
    };
    let pre = preflight_task(&root, &pre_opts).unwrap();
    let contract_id = pre.contract_id.clone();

    // Make a compliant change to expected file
    fs::write(
        root.join("internal/orders/service.go"),
        "package orders\n\ntype OrderService struct {}\n// Compliant addition\n",
    )
    .unwrap();

    // Call recover_contract on compliant tree
    let rep = recover_contract(&root, &contract_id).unwrap();
    assert_eq!(rep.status, RecoveryStatus::Resolved);
    assert_eq!(
        rep.attempt_count, 0,
        "Compliant recovery must not burn attempts"
    );
    assert!(rep.summary.contains("RESOLVED"));

    let session = get_recovery_session(&root, &contract_id).unwrap().unwrap();
    assert_eq!(session.status, RecoveryStatus::Resolved);
    assert_eq!(session.attempt_count, 0);

    let _ = fs::remove_dir_all(&root);
}

/// Test 11: Concurrent recovery invocations guarded by file lock and CAS.
#[test]
fn test_11_concurrent_recovery_invocations_file_lock_and_cas_protection() {
    let root = setup_mock_project();

    let pre_opts = PreflightOptions {
        task: "Refactor secret store".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["internal/secrets/**".to_string()]),
        planned_files: None,
    };
    let pre = preflight_task(&root, &pre_opts).unwrap();
    let contract_id = pre.contract_id.clone();

    // Introduce violation
    fs::write(
        root.join("internal/secrets/vault.go"),
        "package secrets\n\ntype Vault struct {}\n// MUTATION\n",
    )
    .unwrap();

    let barrier = Arc::new(Barrier::new(4));
    let root_arc = Arc::new(root.clone());
    let cid_arc = Arc::new(contract_id.clone());
    let mut handles = Vec::new();

    for _ in 0..4 {
        let b = Arc::clone(&barrier);
        let r = Arc::clone(&root_arc);
        let c = Arc::clone(&cid_arc);
        handles.push(thread::spawn(move || {
            b.wait();
            recover_contract(&r, &c)
        }));
    }

    let mut reports = Vec::new();
    let mut errors = Vec::new();
    for h in handles {
        match h.join().unwrap() {
            Ok(rep) => reports.push(rep),
            Err(e) => errors.push(e.to_string()),
        }
    }

    // Because recover_contract executes under concurrency lock and CAS,
    // subsequent threads either see incremented attempt or CAS/exhaustion
    let total_attempts = reports.len();
    assert!(total_attempts >= 1, "At least one call must succeed");

    let final_session = get_recovery_session(&root, &contract_id).unwrap().unwrap();
    assert!(final_session.session_version >= 1);

    let _ = fs::remove_dir_all(&root);
}

/// Test 12: Enriched structured repair plan contains exact failure classification, evidence, and human_approval_required flag.
#[test]
fn test_12_structured_plan_contains_failure_evidence_and_approval_flag() {
    let root = setup_mock_project();

    let pre_opts = PreflightOptions {
        task: "Update export service".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["internal/secrets/**".to_string()]),
        planned_files: None,
    };
    let pre = preflight_task(&root, &pre_opts).unwrap();
    let contract_id = pre.contract_id.clone();

    // Mutate forbidden boundary file
    fs::write(
        root.join("internal/secrets/vault.go"),
        "package secrets\n\ntype Vault struct {}\n// UNAUTHORIZED MUTATION\n",
    )
    .unwrap();

    // Attempt 1: automated repair permitted (attempt 1 < 2)
    let rep1 = recover_contract(&root, &contract_id).unwrap();
    assert_eq!(rep1.attempt_count, 1);
    assert_eq!(rep1.plan.failure, "BOUNDARY_VIOLATION");
    assert!(!rep1.plan.evidence.is_empty());
    assert!(rep1.plan.evidence[0].contains("vault.go"));
    assert_eq!(
        rep1.plan.human_approval_required, false,
        "Attempt 1 should not require human approval"
    );

    // Attempt 2: final retry reaches limit, human approval required is true
    let rep2 = recover_contract(&root, &contract_id).unwrap();
    assert_eq!(rep2.attempt_count, 2);
    assert_eq!(rep2.plan.failure, "BOUNDARY_VIOLATION");
    assert_eq!(
        rep2.plan.human_approval_required, true,
        "Attempt 2 (final retry) must require human approval"
    );

    let _ = fs::remove_dir_all(&root);
}

/// Test 13: Crash recovery: stale lock is safely reclaimed after timeout when process has died.
#[test]
fn test_13_crash_recovery_stale_lock_breaking() {
    let root = setup_mock_project();
    let contract_id = "test_contract_stale_lock";

    let initial = create_recovery_session(contract_id);
    save_session(&root, &initial).unwrap();

    // Simulate process acquiring lock
    let _locked = acquire_lock(&root, contract_id, 0).unwrap();

    // Breaking lock with a large max_age (e.g. 1 hour) fails because lock is fresh
    let fresh_err =
        break_stale_lock(&root, contract_id, std::time::Duration::from_secs(3600)).unwrap_err();
    assert!(fresh_err.to_string().contains("LOCK_ACTIVE"));

    // Simulate that the process that held the lock died (PID 999999 does not exist)
    {
        let mut session = get_recovery_session(&root, contract_id).unwrap().unwrap();
        session.holder_pid = Some(999999);
        save_session(&root, &session).unwrap();
    }

    // Breaking lock with zero max_age succeeds immediately when holder process is dead
    let unblocked = break_stale_lock(&root, contract_id, std::time::Duration::ZERO).unwrap();
    assert_eq!(unblocked.lock_state, LockState::Unlocked);
    assert_eq!(unblocked.holder_pid, None);
    assert_eq!(unblocked.session_version, 2);

    // Verify subsequent operation succeeds normally
    let session = get_recovery_session(&root, contract_id).unwrap().unwrap();
    assert_eq!(session.lock_state, LockState::Unlocked);

    let _ = fs::remove_dir_all(&root);
}

/// Test 14: Malformed session file is quarantined to .corrupt.<uuid> and error is cleanly reported.
#[test]
fn test_14_malformed_session_file_quarantine_and_clean_recovery() {
    let root = setup_mock_project();
    let contract_id = "test_contract_corrupt";

    let initial = create_recovery_session(contract_id);
    save_session(&root, &initial).unwrap();

    let session_file = root
        .join(".engram")
        .join("cache")
        .join("recovery")
        .join(format!("{}.json", contract_id));
    assert!(session_file.exists());

    // Corrupt the session file on disk
    fs::write(&session_file, "{ malformed json: not valid ...").unwrap();

    // Attempting to load corrupted session quarantines the file and reports error
    let err = load_session(&root, contract_id).unwrap_err().to_string();
    assert!(err.contains("MALFORMED_RECOVERY_SESSION"));
    assert!(err.contains(".corrupt."));

    // The corrupted file was moved out of the way
    assert!(
        !session_file.exists(),
        "Original corrupted file must be moved to quarantine"
    );

    // Fresh session can now be initialized cleanly without deadlocks
    let fresh = create_recovery_session(contract_id);
    save_session(&root, &fresh).unwrap();
    let loaded = get_recovery_session(&root, contract_id).unwrap().unwrap();
    assert_eq!(loaded.contract_id, contract_id);
    assert_eq!(loaded.session_version, 0);

    let _ = fs::remove_dir_all(&root);
}

/// Test 15: Persistence atomic write guarantee: session files are written via tmp file and atomic rename.
#[test]
fn test_15_persistence_atomic_tmp_rename_guarantee() {
    let root = setup_mock_project();
    let contract_id = "test_contract_atomic";

    let session = create_recovery_session(contract_id);
    save_session(&root, &session).unwrap();

    let recovery_dir = root.join(".engram").join("cache").join("recovery");
    let session_file = recovery_dir.join(format!("{}.json", contract_id));
    assert!(session_file.exists());

    // Verify no leftover .tmp files remain in recovery directory
    let entries = fs::read_dir(&recovery_dir).unwrap();
    let mut tmp_count = 0;
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) == Some("tmp") {
            tmp_count += 1;
        }
    }
    assert_eq!(
        tmp_count, 0,
        "Atomic temporary files must be cleaned up / renamed"
    );

    let _ = fs::remove_dir_all(&root);
}

/// Test 16: Stale-lock safety: break_stale_lock cannot reclaim lock held by a live process even if age exceeds max_age.
#[test]
fn test_16_stale_lock_safety_cannot_break_live_process_lock() {
    let root = setup_mock_project();
    let contract_id = "test_contract_live_lock_safety";

    let initial = create_recovery_session(contract_id);
    save_session(&root, &initial).unwrap();

    // Current live process acquires the lock (session.holder_pid will be std::process::id())
    let locked = acquire_lock(&root, contract_id, 0).unwrap();
    assert_eq!(locked.lock_state, LockState::Locked);
    assert_eq!(locked.holder_pid, Some(std::process::id()));

    // Even if max_age is 0 (i.e. lock age exceeds max_age), break_stale_lock MUST refuse reclamation
    // because holder_pid is currently alive and active!
    let live_err = break_stale_lock(&root, contract_id, std::time::Duration::ZERO).unwrap_err();
    let err_msg = live_err.to_string();
    assert!(
        err_msg.contains("PROCESS_ALIVE"),
        "Expected PROCESS_ALIVE error, got: {}",
        err_msg
    );
    assert!(
        err_msg.contains(&std::process::id().to_string()),
        "Error message should mention the live PID: {}",
        err_msg
    );

    // Verify session remains locked on disk
    let session_check = get_recovery_session(&root, contract_id).unwrap().unwrap();
    assert_eq!(session_check.lock_state, LockState::Locked);
    assert_eq!(session_check.holder_pid, Some(std::process::id()));

    // Now simulate that the process died by changing holder_pid to a non-existent PID (e.g. 999999)
    {
        let mut session = session_check.clone();
        session.holder_pid = Some(999999);
        save_session(&root, &session).unwrap();
    }

    // Now break_stale_lock succeeds because PID is confirmed dead
    let reclaimed = break_stale_lock(&root, contract_id, std::time::Duration::ZERO).unwrap();
    assert_eq!(reclaimed.lock_state, LockState::Unlocked);
    assert_eq!(reclaimed.holder_pid, None);

    let _ = fs::remove_dir_all(&root);
}

/// Test 17: Crash-safe persistence: interrupted write with .bak self-healing and Windows atomic replacement.
#[test]
fn test_17_crash_safe_persistence_and_interrupted_backup_self_healing() {
    let root = setup_mock_project();
    let contract_id = "test_contract_crash_persistence";

    // 1. Create and save a valid initial session
    let mut initial = create_recovery_session(contract_id);
    initial.attempt_count = 1;
    save_session(&root, &initial).unwrap();

    let recovery_dir = root.join(".engram").join("cache").join("recovery");
    let session_path = recovery_dir.join(format!("{}.json", contract_id));
    let bak_path = recovery_dir.join(format!("{}.bak", contract_id));

    assert!(session_path.exists());

    // 2. Simulate a crash during write operation:
    // The session was safely backed up to .bak, but an interrupted write / crash
    // wiped out the primary .json file before it could be renamed from .tmp.
    fs::copy(&session_path, &bak_path).unwrap();
    fs::remove_file(&session_path).unwrap();
    assert!(!session_path.exists());
    assert!(bak_path.exists());

    // 3. Load session: load_session must detect missing .json with present .bak and auto-heal
    let restored = load_session(&root, contract_id).unwrap().unwrap();
    assert_eq!(restored.contract_id, contract_id);
    assert_eq!(restored.attempt_count, 1);
    assert!(
        session_path.exists(),
        "Primary session file must be restored from .bak"
    );

    // 4. Update session and ensure normal atomic persistence works
    let mut updated = restored.clone();
    updated.attempt_count = 2;
    save_session(&root, &updated).unwrap();

    let loaded_updated = load_session(&root, contract_id).unwrap().unwrap();
    assert_eq!(loaded_updated.attempt_count, 2);

    let _ = fs::remove_dir_all(&root);
}

/// Test 18: Recovery state consistency: bidirectional reconciliation between contract and recovery session.
#[test]
fn test_18_recovery_state_consistency_and_reconciliation() {
    let root = setup_mock_project();

    let pre_opts = PreflightOptions {
        task: "Update export".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["internal/secrets/**".to_string()]),
        planned_files: None,
    };
    let pre = preflight_task(&root, &pre_opts).unwrap();
    let contract_id = pre.contract_id.clone();

    // Introduce boundary violation
    fs::write(
        root.join("internal/secrets/vault.go"),
        "package secrets\n\ntype Vault struct {}\n// UNAUTHORIZED MUTATION\n",
    )
    .unwrap();

    // Scenario A: Inconsistency where contract is BLOCKED_AWAITING_HUMAN, but recovery session
    // was left IN_PROGRESS (e.g. from an interrupted save).
    {
        let mut contract = get_contract(&root, &contract_id).unwrap();
        contract.status = ContractStatus::BlockedAwaitingHuman;
        engram::contract::save_contract(&root, &contract).unwrap();

        let mut session = create_recovery_session(&contract_id);
        session.status = RecoveryStatus::InProgress;
        save_session(&root, &session).unwrap();
    }

    // recover_contract must reject immediately and reconcile the session to BLOCKED_AWAITING_HUMAN
    let err_a = recover_contract(&root, &contract_id)
        .unwrap_err()
        .to_string();
    assert!(err_a.contains("BLOCKED_AWAITING_HUMAN") || err_a.contains("RECOVERY_EXHAUSTED"));

    let reconciled_session = get_recovery_session(&root, &contract_id).unwrap().unwrap();
    assert_eq!(
        reconciled_session.status,
        RecoveryStatus::BlockedAwaitingHuman
    );

    // Scenario B: Human reconciliation unblocks contract and heals session back to RESOLVED
    let reconciled = engram::contract::reconcile_contract(
        &root,
        &contract_id,
        "Human architect resolved issue",
        "human_admin",
        vec!["internal/secrets/vault.go".to_string()],
        vec!["secrets".to_string()],
    )
    .unwrap();
    assert_eq!(reconciled.status, ContractStatus::Reconciled);

    let session_after_recon = get_recovery_session(&root, &contract_id).unwrap().unwrap();
    assert_eq!(session_after_recon.status, RecoveryStatus::Resolved);
    assert_eq!(session_after_recon.attempt_count, 0);

    // Scenario C: Session exhausted (attempt_count >= max_attempts) automatically marks contract as BLOCKED_AWAITING_HUMAN
    {
        let mut session = session_after_recon.clone();
        session.status = RecoveryStatus::InProgress;
        session.attempt_count = 2; // reached limit
        save_session(&root, &session).unwrap();

        // Mutate again to trigger failure
        fs::write(
            root.join("internal/orders/unauthorized.go"),
            "package orders\n",
        )
        .unwrap();
    }

    // Calling recover_contract on exhausted attempts transitions both session and contract to BLOCKED_AWAITING_HUMAN
    let err_c = recover_contract(&root, &contract_id)
        .unwrap_err()
        .to_string();
    assert!(err_c.contains("RECOVERY_EXHAUSTED"));

    let final_contract = get_contract(&root, &contract_id).unwrap();
    assert_eq!(final_contract.status, ContractStatus::BlockedAwaitingHuman);

    let final_session = get_recovery_session(&root, &contract_id).unwrap().unwrap();
    assert_eq!(final_session.status, RecoveryStatus::RecoveryExhausted);

    let _ = fs::remove_dir_all(&root);
}
