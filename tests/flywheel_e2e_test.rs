//! Specification Test Harness for Engram v1.0.0 Closed-Loop Change Flywheel:
//! Verifies Pre-Flight Handshake & Idempotency (AC-1), Post-Flight 4-Tier Guard (AC-2),
//! Candidate Compounding with Zero-Poisoning Gate (AC-3), Boundary Violation Trust Rejection (AC-4b),
//! and Turn 1 -> Turn 2 Compounding Flywheel (AC-4).

use engram::flywheel::{
    postflight_check, preflight_task, PostflightOptions, PostflightVerdict, PreflightOptions,
    PreflightStatus,
};
use engram::memory::{load_durable_memories, promote_candidate};
use std::fs;
use uuid::Uuid;

fn setup_mock_project() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("engram_flywheel_test_{}", Uuid::new_v4()));
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
            "internal/orders/service_test.go",
            "package orders\n\nimport \"testing\"\n\nfunc TestExportCSV(t *testing.T) {}\n",
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

    // Git repository initialization
    let _ = std::process::Command::new("git").args(["init"]).current_dir(&root).output().unwrap();
    let _ = std::process::Command::new("git").args(["config", "user.name", "FlywheelTest"]).current_dir(&root).output().unwrap();
    let _ = std::process::Command::new("git").args(["config", "user.email", "flywheel@engram.dev"]).current_dir(&root).output().unwrap();
    let _ = std::process::Command::new("git").args(["add", "."]).current_dir(&root).output().unwrap();
    let _ = std::process::Command::new("git").args(["commit", "-m", "Initial commit"]).current_dir(&root).output().unwrap();

    root
}

#[test]
fn test_preflight_deterministic_handshake_and_idempotency() {
    let root = setup_mock_project();

    let opts = PreflightOptions {
        task: "Add CSV export to OrderService".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["internal/secrets/**".to_string()]),
    };

    // 1. Initial Pre-Flight Handshake
    let res1 = preflight_task(&root, &opts).unwrap();

    assert_eq!(res1.status, PreflightStatus::ReadyForImplementation);
    assert!(!res1.is_reused_contract);
    assert!(res1.context_files >= 1);
    assert!(res1.context_tokens > 0);
    assert!(!res1.contract_id.is_empty());
    assert!(res1.forbidden_boundaries.contains(&"internal/secrets/**".to_string()));

    // 2. Preflight Idempotency (AC-1e): Same revision and task reuses active contract
    let res2 = preflight_task(&root, &opts).unwrap();
    assert_eq!(res2.status, PreflightStatus::ReadyForImplementation);
    assert!(res2.is_reused_contract, "Preflight must be idempotent and reuse active contract");
    assert_eq!(res1.contract_id, res2.contract_id, "Contract ID must be identical on idempotent call");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_postflight_clean_mutation_and_candidate_compounding() {
    let root = setup_mock_project();

    // 1. Run Preflight
    let pre_opts = PreflightOptions {
        task: "Add CSV export streaming to OrderService".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["internal/secrets/**".to_string()]),
    };
    let pre = preflight_task(&root, &pre_opts).unwrap();

    // 2. Simulate clean agent mutations within Expected & ExpectedExpansion
    fs::write(
        root.join("internal/orders/service.go"),
        "package orders\n\ntype OrderService struct {}\n\nfunc (s *OrderService) ExportCSV() error { return nil }\n// clean update\n",
    ).unwrap();
    fs::write(
        root.join("internal/orders/export.go"),
        "package orders\n\ntype CsvExporter struct {}\n// expansion update\n",
    ).unwrap();

    // 3. Post-Flight Check
    let post_opts = PostflightOptions {
        contract_id: Some(pre.contract_id.clone()),
        strict: false,
        staged: false,
        base_commit: None,
        ci_mode: false,
    };
    let post = postflight_check(&root, &post_opts).unwrap();

    assert_eq!(post.verdict, PostflightVerdict::Pass);
    assert_eq!(post.exit_code, 0);
    assert_eq!(post.boundary_violation_count, 0);
    assert_eq!(post.total_changed_files, 2);

    // 4. Candidate Compounding (AC-3, Zero-Poisoning Gate)
    assert_eq!(post.generated_candidates.len(), 1, "Passing mutation must generate exactly 1 candidate");
    let cand = &post.generated_candidates[0];
    assert!(cand.title.contains("OrderService"));
    assert!(post.human_action_required.is_some());

    // Invariant: Candidate must NOT be in durable memory yet!
    let doc_before = load_durable_memories(&root).unwrap();
    assert!(
        !doc_before.memories.iter().any(|m| m.id == cand.candidate_id),
        "Candidate memory must NOT be in durable memory before human promotion!"
    );

    // 5. Human Promotion Gate (AC-3b)
    let promoted = promote_candidate(&root, &cand.candidate_id, "lead_architect", Some("Approved streaming export")).unwrap();
    assert!(promoted.id.starts_with("mem_"));

    let doc_after = load_durable_memories(&root).unwrap();
    assert!(
        doc_after.memories.iter().any(|m| m.id == promoted.id),
        "Promoted decision must now be in durable memory!"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_postflight_boundary_violation_and_zero_poisoning() {
    let root = setup_mock_project();

    // 1. Run Preflight with forbidden boundary on secrets
    let pre_opts = PreflightOptions {
        task: "Refactor secrets handling in orders".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["internal/secrets/**".to_string()]),
    };
    let pre = preflight_task(&root, &pre_opts).unwrap();

    // 2. Deliberate forbidden mutation by rogue agent
    fs::write(
        root.join("internal/secrets/vault.go"),
        "package secrets\n\ntype Vault struct {}\n// FORBIDDEN CHANGE\n",
    ).unwrap();

    // 3. Post-Flight Check
    let post_opts = PostflightOptions {
        contract_id: Some(pre.contract_id),
        strict: false,
        staged: false,
        base_commit: None,
        ci_mode: false,
    };
    let post = postflight_check(&root, &post_opts).unwrap();

    // Trust rejection: boundary violations are blocked with Exit 1
    assert_eq!(post.verdict, PostflightVerdict::BlockedBoundaryViolation);
    assert_eq!(post.exit_code, 1);
    assert!(post.boundary_violation_count >= 1);

    // Zero Poisoning: No candidate must ever be synthesized for boundary violations!
    assert!(post.generated_candidates.is_empty(), "Violating mutation must NEVER synthesize memory candidates");

    let doc = load_durable_memories(&root).unwrap();
    assert!(doc.memories.is_empty(), "Durable memory must remain unpolluted after boundary violation");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_postflight_unexpected_change_and_strict_mode() {
    let root = setup_mock_project();

    let pre_opts = PreflightOptions {
        task: "Update orders service".to_string(),
        token_budget: 1200,
        forbidden_boundaries: None,
    };
    let pre = preflight_task(&root, &pre_opts).unwrap();

    // Create an unexpected file in an unrelated directory
    let extra_dir = root.join("internal/unrelated");
    fs::create_dir_all(&extra_dir).unwrap();
    fs::write(extra_dir.join("extra.go"), "package unrelated\n").unwrap();

    // Normal mode: Advisory pass
    let post_normal = postflight_check(
        &root,
        &PostflightOptions {
            contract_id: Some(pre.contract_id.clone()),
            strict: false,
            staged: false,
            base_commit: None,
            ci_mode: false,
        },
    ).unwrap();
    assert_eq!(post_normal.verdict, PostflightVerdict::Pass);
    assert_eq!(post_normal.exit_code, 0);
    assert_eq!(post_normal.unexpected_count, 1);

    // Strict mode: Blocks on unexpected expansion
    let post_strict = postflight_check(
        &root,
        &PostflightOptions {
            contract_id: Some(pre.contract_id),
            strict: true,
            staged: false,
            base_commit: None,
            ci_mode: false,
        },
    ).unwrap();
    assert_eq!(post_strict.verdict, PostflightVerdict::ReviewUnexpectedExpansion);
    assert_eq!(post_strict.exit_code, 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_closed_loop_flywheel_turn1_to_turn2_compounding() {
    let root = setup_mock_project();

    // ─────────────────────────────────────────────────────────────
    // TURN 1: Task Execution -> Candidate -> Human Promotion
    // ─────────────────────────────────────────────────────────────
    let t1_task = "Stream order exports to io.Writer".to_string();
    let pre1 = preflight_task(
        &root,
        &PreflightOptions {
            task: t1_task.clone(),
            token_budget: 1200,
            forbidden_boundaries: Some(vec!["internal/secrets/**".to_string()]),
        },
    ).unwrap();

    // Simulated agent edits
    fs::write(
        root.join("internal/orders/service.go"),
        "package orders\n\nimport \"io\"\n\ntype OrderService struct {}\n\nfunc (s *OrderService) ExportCSV(w io.Writer) error { return nil }\n",
    ).unwrap();

    let post1 = postflight_check(
        &root,
        &PostflightOptions {
            contract_id: Some(pre1.contract_id),
            strict: false,
            staged: false,
            base_commit: None,
            ci_mode: false,
        },
    ).unwrap();

    assert_eq!(post1.verdict, PostflightVerdict::Pass);
    assert_eq!(post1.generated_candidates.len(), 1);

    let cand_id = &post1.generated_candidates[0].candidate_id;

    // Human promotion into durable engineering memory
    let promoted = promote_candidate(
        &root,
        cand_id,
        "lead_architect",
        Some("Ratified streaming io.Writer convention for all order exports"),
    ).unwrap();
    assert!(promoted.id.starts_with("mem_"));

    // Commit changes to establish clean git baseline for Turn 2
    let _ = std::process::Command::new("git").args(["add", "."]).current_dir(&root).output().unwrap();
    let _ = std::process::Command::new("git").args(["commit", "-m", "Turn 1 complete"]).current_dir(&root).output().unwrap();

    // ─────────────────────────────────────────────────────────────
    // TURN 2: Follow-up Task in Same Module -> Prior Rationale Retrieved!
    // ─────────────────────────────────────────────────────────────
    let t2_task = "Add JSON streaming export feature to orders".to_string();
    let pre2 = preflight_task(
        &root,
        &PreflightOptions {
            task: t2_task,
            token_budget: 1200,
            forbidden_boundaries: Some(vec!["internal/secrets/**".to_string()]),
        },
    ).unwrap();

    assert_eq!(pre2.status, PreflightStatus::ReadyForImplementation);

    // Proves AC-4 & AC-4c: Turn 1 decision automatically retrieved as governing policy upfront!
    let retrieved_turn1_decision = pre2.governing_policies.iter().any(|p| p.id == promoted.id || p.title.contains("Stream order exports"));
    assert!(
        retrieved_turn1_decision,
        "Turn 2 preflight MUST retrieve Turn 1 promoted architectural decision under governing policies!"
    );

    let _ = fs::remove_dir_all(&root);
}
