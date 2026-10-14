use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

use engram::contract::{
    check_contract, create_contract, format_check_markdown_summary,
    format_check_report, save_contract, CheckExitCode, CheckOptions,
    RatificationStatus,
};

fn setup_test_git_repo(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("engram_ci_guard_{}_{}", prefix, Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();

    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "Git command failed: {:?}", args);
    };

    run(&["init", "-b", "main"]);
    run(&["config", "user.name", "Engram CI Tester"]);
    run(&["config", "user.email", "ci-tester@engram.dev"]);

    // Initial commit
    let init_file = dir.join("README.md");
    fs::write(&init_file, "# Test Repo\n").unwrap();
    run(&["add", "README.md"]);
    run(&["commit", "-m", "Initial commit on main"]);

    dir
}

#[test]
fn test_ci_guard_file_argument_standalone_contract() {
    let root = setup_test_git_repo("file_arg");

    // Create a standalone contract JSON file
    let contract = create_contract(
        &root,
        "Implement checkout tax computation",
        Some(vec!["tax".to_string()]),
        Some(vec!["src/tax/calc.rs".to_string()]),
        Some(vec!["src/payments/**".to_string()]),
    )
    .unwrap();

    let contract_json = serde_json::to_string_pretty(&contract).unwrap();
    let standalone_file = root.join("custom_contract.json");
    fs::write(&standalone_file, &contract_json).unwrap();

    // Modify the expected file in working tree
    let tax_file = root.join("src").join("tax").join("calc.rs");
    fs::create_dir_all(tax_file.parent().unwrap()).unwrap();
    fs::write(&tax_file, "// tax calculation logic\n").unwrap();

    let opts = CheckOptions {
        file: Some(standalone_file.to_string_lossy().to_string()),
        ..Default::default()
    };

    let res = check_contract(&root, &opts);
    assert_eq!(res.status, "PASSED");
    assert_eq!(res.exit_code, 0);
    assert_eq!(res.summary.expected, 1);
    assert_eq!(res.summary.boundary_violations, 0);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_ci_guard_repo_committed_contract() {
    let root = setup_test_git_repo("repo_contract");

    // Commit a contract into .engram/contracts/
    let engram_contracts_dir = root.join(".engram").join("contracts");
    fs::create_dir_all(&engram_contracts_dir).unwrap();

    let contract = create_contract(
        &root,
        "Refactor invoice generation",
        Some(vec!["invoice".to_string()]),
        Some(vec!["src/invoice/gen.rs".to_string()]),
        Some(vec!["src/auth/**".to_string()]),
    )
    .unwrap();

    // Verify it was automatically mirrored to .engram/contracts/
    let repo_contract_path = engram_contracts_dir.join(format!("{}.json", contract.contract_id));
    assert!(repo_contract_path.exists());

    // Modify expected file
    let inv_file = root.join("src").join("invoice").join("gen.rs");
    fs::create_dir_all(inv_file.parent().unwrap()).unwrap();
    fs::write(&inv_file, "// generator\n").unwrap();

    let opts = CheckOptions {
        contract_id: Some(contract.contract_id.clone()),
        ..Default::default()
    };

    let res = check_contract(&root, &opts);
    assert_eq!(res.status, "PASSED");
    assert_eq!(res.contract_id, contract.contract_id);
    assert_eq!(res.summary.expected, 1);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_ci_guard_base_commit_branch_diff() {
    let root = setup_test_git_repo("base_diff");

    // Create contract on main
    let contract = create_contract(
        &root,
        "Customer notification webhook",
        Some(vec!["webhook".to_string()]),
        Some(vec!["src/webhook/client.rs".to_string()]),
        Some(vec!["src/payments/**".to_string()]),
    )
    .unwrap();

    // Create feature branch
    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(out.status.success(), "Git command failed: {:?}", args);
    };

    run(&["checkout", "-b", "feature/webhook"]);

    // Make authorized change and commit on feature branch
    let client_file = root.join("src").join("webhook").join("client.rs");
    fs::create_dir_all(client_file.parent().unwrap()).unwrap();
    fs::write(&client_file, "// webhook client\n").unwrap();
    run(&["add", "src/webhook/client.rs"]);
    run(&["commit", "-m", "feat: implement webhook client"]);

    // Check with base_commit = main
    let opts = CheckOptions {
        contract_id: Some(contract.contract_id.clone()),
        base_commit: Some("main".to_string()),
        ..Default::default()
    };

    let res = check_contract(&root, &opts);
    assert_eq!(res.status, "PASSED");
    assert_eq!(res.exit_code, 0);
    assert_eq!(res.summary.total_changed, 1);
    assert_eq!(res.summary.expected, 1);
    assert_eq!(res.summary.boundary_violations, 0);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_ci_guard_fail_closed_boundary_violation() {
    let root = setup_test_git_repo("boundary_viol");

    let contract = create_contract(
        &root,
        "Update profile UI",
        Some(vec!["ui".to_string()]),
        Some(vec!["src/ui/profile.rs".to_string()]),
        Some(vec!["src/security/**".to_string()]), // Forbidden!
    )
    .unwrap();

    // Modifies forbidden security boundary
    let sec_file = root.join("src").join("security").join("crypto.rs");
    fs::create_dir_all(sec_file.parent().unwrap()).unwrap();
    fs::write(&sec_file, "// dangerous edit\n").unwrap();

    let opts = CheckOptions {
        contract_id: Some(contract.contract_id.clone()),
        ..Default::default()
    };

    let res = check_contract(&root, &opts);
    assert_eq!(res.status, "FAILED");
    assert_eq!(res.exit_code, CheckExitCode::ContractViolation.as_i32());
    assert_eq!(res.summary.boundary_violations, 1);
    assert!(res.failure_reasons[0].contains("CONTRACT_VIOLATION"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_ci_guard_fail_closed_constitution_violation() {
    let root = setup_test_git_repo("const_viol");

    // 1. Create constitution forbidding src/billing/**
    let engram_dir = root.join(".engram");
    fs::create_dir_all(&engram_dir).unwrap();
    let constitution_yaml = r#"
schema_version: 1
rules:
  - id: RULE-BILLING-LOCK
    name: "Billing Layer Locked"
    type: DEPENDENCY
    severity: ERROR
    forbidden: "src/billing/**"
    description: "Billing cannot be modified by agent without team sign-off."
"#;
    fs::write(engram_dir.join("constitution.yaml"), constitution_yaml).unwrap();

    // 2. Agent creates contract declaring billing/gateway.rs as EXPECTED
    let contract = create_contract(
        &root,
        "Agent modifications",
        Some(vec!["billing".to_string()]),
        Some(vec!["src/billing/gateway.rs".to_string()]),
        None,
    )
    .unwrap();

    let bill_file = root.join("src").join("billing").join("gateway.rs");
    fs::create_dir_all(bill_file.parent().unwrap()).unwrap();
    fs::write(&bill_file, "// billing modification\n").unwrap();

    let opts = CheckOptions {
        contract_id: Some(contract.contract_id.clone()),
        ..Default::default()
    };

    let res = check_contract(&root, &opts);
    // Absolute Constitution Precedence: MUST FAIL
    assert_eq!(res.status, "FAILED");
    assert_eq!(res.exit_code, CheckExitCode::ContractViolation.as_i32());
    assert_eq!(res.summary.boundary_violations, 1);
    assert!(res.changes[0].reason.contains("Constitution rule 'Billing Layer Locked'"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_ci_guard_constitution_fallback_when_no_contract() {
    let root = setup_test_git_repo("no_contract_fallback");

    // Setup constitution
    let engram_dir = root.join(".engram");
    fs::create_dir_all(&engram_dir).unwrap();
    let constitution_yaml = r#"
schema_version: 1
rules:
  - id: RULE-PROTECTED-CONFIG
    name: "Protected Config Files"
    type: PATTERN
    severity: ERROR
    forbidden: "config/secrets/**"
    description: "Secrets directory must never be altered."
"#;
    fs::write(engram_dir.join("constitution.yaml"), constitution_yaml).unwrap();

    // 1. Clean modification to ordinary file
    let app_file = root.join("src").join("app.rs");
    fs::create_dir_all(app_file.parent().unwrap()).unwrap();
    fs::write(&app_file, "// valid app code\n").unwrap();

    let opts = CheckOptions {
        allow_no_contract: true,
        ..Default::default()
    };

    let clean_res = check_contract(&root, &opts);
    assert_eq!(clean_res.status, "PASSED");
    assert_eq!(clean_res.policy, "CONSTITUTION_ONLY");
    assert_eq!(clean_res.exit_code, 0);

    // 2. Modification touches forbidden file
    let secret_file = root.join("config").join("secrets").join("api_key.txt");
    fs::create_dir_all(secret_file.parent().unwrap()).unwrap();
    fs::write(&secret_file, "LEAKED_KEY\n").unwrap();

    let viol_res = check_contract(&root, &opts);
    assert_eq!(viol_res.status, "FAILED");
    assert_eq!(viol_res.exit_code, CheckExitCode::ContractViolation.as_i32());
    assert_eq!(viol_res.summary.boundary_violations, 1);
    assert!(viol_res.failure_reasons[0].contains("CONSTITUTION_VIOLATION"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_ci_guard_fail_closed_unratified_expansion() {
    let root = setup_test_git_repo("unratified_pr");

    let mut contract = create_contract(
        &root,
        "Feature expansion",
        Some(vec!["cart".to_string()]),
        Some(vec!["src/cart/service.rs".to_string()]),
        None,
    )
    .unwrap();

    // Agent expands contract boundary in PR without human ratification
    contract.version = 2;
    contract.provenance.ratification_status = RatificationStatus::Unratified;
    save_contract(&root, &contract).unwrap();

    let cart_file = root.join("src").join("cart").join("service.rs");
    fs::create_dir_all(cart_file.parent().unwrap()).unwrap();
    fs::write(&cart_file, "// cart\n").unwrap();

    let opts = CheckOptions {
        contract_id: Some(contract.contract_id.clone()),
        check_provenance: true,
        ..Default::default()
    };

    let res = check_contract(&root, &opts);
    assert_eq!(res.status, "FAILED");
    assert_eq!(res.exit_code, CheckExitCode::ContractViolation.as_i32());
    assert!(res.failure_reasons.iter().any(|f| f.contains("CONTRACT_MODIFIED_IN_PR")));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_ci_guard_markdown_summary_and_annotations() {
    let root = setup_test_git_repo("summary_test");

    let contract = create_contract(
        &root,
        "Documentation and helper update",
        Some(vec!["docs".to_string()]),
        Some(vec!["docs/guide.md".to_string()]),
        Some(vec!["forbidden/path/**".to_string()]),
    )
    .unwrap();

    let guide_file = root.join("docs").join("guide.md");
    fs::create_dir_all(guide_file.parent().unwrap()).unwrap();
    fs::write(&guide_file, "# Guide\n").unwrap();

    let opts = CheckOptions {
        contract_id: Some(contract.contract_id.clone()),
        ..Default::default()
    };

    let res = check_contract(&root, &opts);
    let term_report = format_check_report(&res, true);
    assert!(term_report.contains("Engram Change Guard"));

    let md_summary = format_check_markdown_summary(&res);
    assert!(md_summary.contains("Engram Change Guard: PASS"));
    assert!(md_summary.contains("| Total Inspected Files | 1 |"));
    assert!(md_summary.contains("| Expected Changes | 1 |"));
    assert!(md_summary.contains("docs/guide.md"));

    fs::remove_dir_all(&root).unwrap();
}
