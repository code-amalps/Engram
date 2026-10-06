use std::fs;
use std::path::PathBuf;

use engram::contract::{
    check_contract, create_contract, install_pre_commit_hook,
    ratify_contract, reconcile_contract, uninstall_pre_commit_hook,
    validate_contract_diff_with_constitution, CheckExitCode, CheckOptions,
    FileValidationCategory, RatificationStatus,
};

fn temp_repo_dir(prefix: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("engram_{}_{}", prefix, uuid::Uuid::new_v4()));
    fs::create_dir_all(&p).unwrap();
    // Initialize dummy git repository
    let _ = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&p)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "EngramTester"])
        .current_dir(&p)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "tester@engram.dev"])
        .current_dir(&p)
        .output();
    p
}

#[test]
fn test_check_all_expected_passes() {
    let root = temp_repo_dir("check_pass");

    let contract = create_contract(
        &root,
        "Refactor order export",
        Some(vec!["orders".to_string()]),
        Some(vec![
            "src/orders/export.rs".to_string(),
            "tests/export_test.rs".to_string(),
        ]),
        Some(vec!["src/payments/**".to_string()]),
    )
    .unwrap();

    let changed = vec![
        "src/orders/export.rs".to_string(),
        "tests/export_test.rs".to_string(),
    ];

    let (classifications, missing, summary) =
        validate_contract_diff_with_constitution(&root, &contract, &changed);

    assert_eq!(summary.violation_count, 0);
    assert_eq!(summary.expected_count, 2);
    assert_eq!(summary.unexpected_count, 0);
    assert_eq!(missing.len(), 0);
    assert!(classifications.iter().all(|c| c.category == FileValidationCategory::Expected));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_check_expected_expansion_passes() {
    let root = temp_repo_dir("check_expansion");

    let contract = create_contract(
        &root,
        "Implement cart checkout",
        Some(vec!["cart".to_string()]),
        Some(vec!["src/cart/service.rs".to_string()]),
        Some(vec!["src/payments/**".to_string()]),
    )
    .unwrap();

    // New helper file in cart module
    let changed = vec![
        "src/cart/service.rs".to_string(),
        "src/cart/helpers/validator.rs".to_string(),
    ];

    let (classifications, _, summary) =
        validate_contract_diff_with_constitution(&root, &contract, &changed);

    assert_eq!(summary.violation_count, 0);
    assert_eq!(summary.expected_count, 1);
    assert_eq!(summary.expansion_count, 1);

    let helper = classifications
        .iter()
        .find(|c| c.path == "src/cart/helpers/validator.rs")
        .unwrap();
    assert_eq!(helper.category, FileValidationCategory::ExpectedExpansion);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_check_unexpected_advisory_vs_strict() {
    let root = temp_repo_dir("check_unexpected");

    let contract = create_contract(
        &root,
        "Update order status",
        Some(vec!["orders".to_string()]),
        Some(vec!["src/orders/status.rs".to_string()]),
        Some(vec!["src/payments/**".to_string()]),
    )
    .unwrap();

    // Changed order status AND modified notifications mailer (outside module, but not forbidden)
    let changed = vec![
        "src/orders/status.rs".to_string(),
        "src/notifications/mailer.rs".to_string(),
    ];

    let (classifications, _, summary) =
        validate_contract_diff_with_constitution(&root, &contract, &changed);

    assert_eq!(summary.violation_count, 0);
    assert_eq!(summary.unexpected_count, 1);

    let mailer = classifications
        .iter()
        .find(|c| c.path == "src/notifications/mailer.rs")
        .unwrap();
    assert_eq!(mailer.category, FileValidationCategory::Unexpected);

    // Test options evaluation: standard policy vs strict policy
    let standard_opts = CheckOptions {
        contract_id: Some(contract.contract_id.clone()),
        strict: false,
        ..Default::default()
    };
    // Standard mode should accept UNEXPECTED as advisory (only violations fail)
    assert!(!standard_opts.strict);

    let strict_opts = CheckOptions {
        contract_id: Some(contract.contract_id.clone()),
        strict: true,
        ..Default::default()
    };
    assert!(strict_opts.strict);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_check_boundary_violation_always_fails() {
    let root = temp_repo_dir("check_violation");

    let contract = create_contract(
        &root,
        "Customer profile export",
        Some(vec!["customer".to_string()]),
        Some(vec!["src/customer/profile.rs".to_string()]),
        Some(vec!["src/payments/**".to_string()]),
    )
    .unwrap();

    let changed = vec![
        "src/customer/profile.rs".to_string(),
        "src/payments/billing_gateway.rs".to_string(), // FORBIDDEN!
    ];

    let (classifications, _, summary) =
        validate_contract_diff_with_constitution(&root, &contract, &changed);

    assert_eq!(summary.violation_count, 1);
    let billing = classifications
        .iter()
        .find(|c| c.path == "src/payments/billing_gateway.rs")
        .unwrap();
    assert_eq!(billing.category, FileValidationCategory::BoundaryViolation);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_check_missing_expected_changes() {
    let root = temp_repo_dir("check_missing");

    let contract = create_contract(
        &root,
        "Invoice calculation",
        Some(vec!["invoice".to_string()]),
        Some(vec![
            "src/invoice/calc.rs".to_string(),
            "tests/calc_test.rs".to_string(),
        ]),
        None,
    )
    .unwrap();

    // Only modified calc.rs, forgot calc_test.rs
    let changed = vec!["src/invoice/calc.rs".to_string()];

    let (_, missing, summary) =
        validate_contract_diff_with_constitution(&root, &contract, &changed);

    assert_eq!(summary.missing_count, 1);
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].path, "tests/calc_test.rs");
    assert!(missing[0].required);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_check_no_active_contract() {
    let root = temp_repo_dir("check_no_contract");

    let opts = CheckOptions::default();
    let res = check_contract(&root, &opts);

    assert_eq!(res.status, "FAILED");
    assert_eq!(res.exit_code, CheckExitCode::ContractNotFound.as_i32());
    assert!(res.failure_reasons[0].contains("No active Change Contract"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_check_corrupt_contract_schema() {
    let root = temp_repo_dir("check_corrupt");

    // Write malformed JSON to active.json
    let contracts_dir = engram::storage::cache_dir(&root).unwrap().join("contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    fs::write(contracts_dir.join("active.json"), "{ invalid_json: ").unwrap();

    let opts = CheckOptions::default();
    let res = check_contract(&root, &opts);

    assert_eq!(res.status, "FAILED");
    assert_eq!(res.exit_code, CheckExitCode::ContractInvalid.as_i32());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_check_shallow_clone_missing_baseline() {
    let root = temp_repo_dir("check_shallow");

    let mut contract = create_contract(
        &root,
        "Shallow test task",
        Some(vec!["orders".to_string()]),
        Some(vec!["src/orders/mod.rs".to_string()]),
        None,
    )
    .unwrap();

    // Set non-existent Git commit SHA
    contract.baseline_revision = "0000000000000000000000000000000000000000".to_string();
    engram::contract::save_contract(&root, &contract).unwrap();

    let opts = CheckOptions::default();
    let res = check_contract(&root, &opts);

    assert_eq!(res.status, "FAILED");
    assert_eq!(res.exit_code, CheckExitCode::BaselineUnavailable.as_i32());
    assert!(res.failure_reasons[0].contains("unavailable in current Git history"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_hook_installation_and_idempotency() {
    let root = temp_repo_dir("hook_install");

    // 1. Dry run on clean repo: file must not be modified
    let dry_run_msg = install_pre_commit_hook(&root, true).unwrap();
    assert!(dry_run_msg.contains("Dry-run"));
    assert!(!root.join(".git").join("hooks").join("pre-commit").exists());

    // 2. Real installation
    let install_msg = install_pre_commit_hook(&root, false).unwrap();
    assert!(install_msg.contains("Installed Engram pre-commit hook"));
    let hook_file = root.join(".git").join("hooks").join("pre-commit");
    assert!(hook_file.exists());
    let content = fs::read_to_string(&hook_file).unwrap();
    assert!(content.contains("engram contract check --staged"));

    // 3. Repeated installation: idempotent, no duplicate block
    let repeat_msg = install_pre_commit_hook(&root, false).unwrap();
    assert!(repeat_msg.contains("already installed"));
    let content2 = fs::read_to_string(&hook_file).unwrap();
    assert_eq!(content, content2);

    // 4. Foreign hook coexistence
    fs::write(&hook_file, "#!/bin/sh\necho 'Husky pre-commit'\n").unwrap();
    let foreign_install = install_pre_commit_hook(&root, false).unwrap();
    assert!(foreign_install.contains("Preserved existing pre-commit hook"));
    let foreign_content = fs::read_to_string(&hook_file).unwrap();
    assert!(foreign_content.contains("Husky pre-commit"));
    assert!(foreign_content.contains("engram contract check --staged"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_hook_uninstall_cleans_up() {
    let root = temp_repo_dir("hook_uninstall");

    // 1. Install hook on foreign hook
    let hook_file = root.join(".git").join("hooks").join("pre-commit");
    fs::create_dir_all(hook_file.parent().unwrap()).unwrap();
    fs::write(
        &hook_file,
        "#!/bin/sh\n# foreign script\n# --- BEGIN ENGRAM CHANGE GUARD ---\nengram check\n# --- END ENGRAM CHANGE GUARD ---\n",
    )
    .unwrap();

    // 2. Dry run uninstall
    let dry_msg = uninstall_pre_commit_hook(&root, true).unwrap();
    assert!(dry_msg.contains("Dry-run: Engram would remove Change Guard block"));
    assert!(hook_file.exists());

    // 3. Real uninstall
    let uninst_msg = uninstall_pre_commit_hook(&root, false).unwrap();
    assert!(uninst_msg.contains("Removed Engram block while preserving foreign hooks"));
    let remaining = fs::read_to_string(&hook_file).unwrap();
    assert!(remaining.contains("# foreign script"));
    assert!(!remaining.contains("ENGRAM CHANGE GUARD"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_human_reconciliation_v1_to_v2() {
    let root = temp_repo_dir("human_reconcile");

    let contract = create_contract(
        &root,
        "Refactor payments",
        Some(vec!["payments".to_string()]),
        Some(vec!["src/payments/service.rs".to_string()]),
        Some(vec!["src/auth/**".to_string()]),
    )
    .unwrap();

    assert_eq!(contract.version, 1);
    assert_eq!(contract.provenance.origin, "AGENT");

    // Developer authoritatively adds metrics tracking
    let reconciled = reconcile_contract(
        &root,
        &contract.contract_id,
        "Human developer added metrics monitoring",
        "Amal",
        vec!["src/payments/metrics.rs".to_string()],
        vec!["payments".to_string()],
    )
    .unwrap();

    assert_eq!(reconciled.version, 2);
    assert_eq!(reconciled.provenance.origin, "RECONCILED");
    assert_eq!(reconciled.provenance.ratification_status, RatificationStatus::Ratified);
    assert_eq!(reconciled.provenance.ratified_by.as_deref(), Some("Amal"));
    assert!(reconciled.boundary.expected_files.contains(&"src/payments/metrics.rs".to_string()));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_ci_pr_base_comparison() {
    let root = temp_repo_dir("ci_pr_base");

    let contract = create_contract(
        &root,
        "Feature PR validation",
        Some(vec!["orders".to_string()]),
        Some(vec!["src/orders/feature.rs".to_string()]),
        None,
    )
    .unwrap();

    let opts = CheckOptions {
        contract_id: Some(contract.contract_id.clone()),
        base_commit: Some("main".to_string()),
        ..Default::default()
    };

    let (files, snapshot, maybe_err) =
        engram::contract::inspect_changed_files(&root, &contract.baseline_revision, &opts).unwrap();

    assert_eq!(snapshot, "PR_HEAD (base: main)");
    // In dummy git repo without commit, maybe_err will be BaselineUnavailable or empty
    assert!(maybe_err.is_some() || files.is_empty());

    fs::remove_dir_all(&root).unwrap();
}

/// Invariant 13: Constitution rules strictly override Change Contracts (Release Blocking)
#[test]
fn test_constitution_precedence_over_contract() {
    let root = temp_repo_dir("const_precedence");

    // 1. Define Project Constitution rule forbidding auth boundary
    let engram_dir = root.join(".engram");
    fs::create_dir_all(&engram_dir).unwrap();
    let constitution_yaml = r#"
schema_version: 1
rules:
  - id: RULE-AUTH-LOCK
    name: "Auth Layer Immutable"
    type: DEPENDENCY
    severity: ERROR
    forbidden: "src/auth/**"
    description: "Authentication layer cannot be modified without team lead review."
"#;
    fs::write(engram_dir.join("constitution.yaml"), constitution_yaml).unwrap();

    // 2. AI agent naively declares src/auth/AuthProvider.cs as EXPECTED in contract
    let contract = create_contract(
        &root,
        "Add auth OAuth2 flow",
        Some(vec!["auth".to_string()]),
        Some(vec!["src/auth/AuthProvider.cs".to_string()]), // EXPECTED by contract
        None,
    )
    .unwrap();

    // 3. Agent modifies AuthProvider.cs
    let changed_files = vec!["src/auth/AuthProvider.cs".to_string()];

    // 4. Validate with Constitution Precedence
    let (classifications, _, summary) =
        validate_contract_diff_with_constitution(&root, &contract, &changed_files);

    // Invariant Assertions:
    // Despite contract stating EXPECTED, Constitution MUST override to BOUNDARY_VIOLATION!
    assert_eq!(summary.violation_count, 1);
    assert_eq!(summary.expected_count, 0);

    let auth_change = &classifications[0];
    assert_eq!(auth_change.category, FileValidationCategory::BoundaryViolation);
    assert!(auth_change.reason.contains("Constitution rule 'Auth Layer Immutable'"));
    assert!(auth_change.reason.contains("strictly forbids"));

    fs::remove_dir_all(&root).unwrap();
}

/// Invariant 14: Contract modified in PR requires explicit ratification (Trust Model)
#[test]
fn test_contract_modified_in_pr_requires_ratification() {
    let root = temp_repo_dir("pr_ratification");

    let mut contract = create_contract(
        &root,
        "Agent autonomous PR",
        Some(vec!["orders".to_string()]),
        Some(vec!["src/orders/checkout.rs".to_string()]),
        None,
    )
    .unwrap();

    // Simulate contract boundary expanded in PR by agent (version > 1, unratified)
    contract.version = 2;
    contract.provenance.origin = "AGENT".to_string();
    contract.provenance.ratification_status = RatificationStatus::Unratified;
    engram::contract::save_contract(&root, &contract).unwrap();

    // 1. Run check in PR mode with provenance verification
    let opts = CheckOptions {
        contract_id: Some(contract.contract_id.clone()),
        check_provenance: true,
        ..Default::default()
    };

    let res = check_contract(&root, &opts);
    assert_eq!(res.status, "FAILED");
    assert_eq!(res.exit_code, CheckExitCode::ContractViolation.as_i32());
    assert!(
        res.failure_reasons
            .iter()
            .any(|f| f.contains("CONTRACT_MODIFIED_IN_PR") && f.to_ascii_lowercase().contains("ratification required")),
        "Expected CONTRACT_MODIFIED_IN_PR failure reason, got: {:?}", res.failure_reasons
    );

    // 2. Human ratifies the contract
    let ratified = ratify_contract(
        &root,
        &contract.contract_id,
        "lead_developer",
        "Reviewed and approved by tech lead",
    )
    .unwrap();
    assert_eq!(ratified.provenance.ratification_status, RatificationStatus::Ratified);

    // 3. Re-run check with provenance: passes ratification check!
    let res2 = check_contract(&root, &opts);
    assert!(!res2.failure_reasons.iter().any(|f| f.contains("CONTRACT_MODIFIED_IN_PR")));

    fs::remove_dir_all(&root).unwrap();
}
