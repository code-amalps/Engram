use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

use engram::ci_guard::{
    install_guard_hook, parse_policy_toml, run_guard, uninstall_guard_hook, FindingSeverity,
    GuardOptions, GuardStatus, GUARD_HOOK_BODY, GUARD_HOOK_START,
};
use engram::contract::{create_contract, save_contract};

fn setup_test_git_repo(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("engram_guard_test_{}_{}", prefix, Uuid::new_v4()));
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
    run(&["config", "user.name", "Engram Guard Tester"]);
    run(&["config", "user.email", "guard-tester@engram.dev"]);

    // Initial commit
    let init_file = dir.join("README.md");
    fs::write(&init_file, "# Test Repo\n").unwrap();
    run(&["add", "README.md"]);
    run(&["commit", "-m", "Initial commit on main"]);

    dir
}

// ---------------------------------------------------------------------------
// 1. All checks passing
// ---------------------------------------------------------------------------
#[test]
fn test_01_all_checks_passing() {
    let root = setup_test_git_repo("all_pass");

    // Create contract
    let contract = create_contract(
        &root,
        "Implement tax calculation",
        Some(vec!["tax".to_string()]),
        Some(vec!["src/tax/calc.rs".to_string()]),
        Some(vec!["src/payments/**".to_string()]),
    )
    .unwrap();
    save_contract(&root, &contract).unwrap();

    // Make valid change
    let tax_file = root.join("src").join("tax").join("calc.rs");
    fs::create_dir_all(tax_file.parent().unwrap()).unwrap();
    fs::write(&tax_file, "pub fn calculate_tax() -> u32 { 10 }\n").unwrap();

    let opts = GuardOptions::default();
    let res = run_guard(&root, &opts);

    assert_eq!(res.status, GuardStatus::Pass);
    assert_eq!(res.exit_code, 0);
    assert!(!res.blocked);
    assert!(
        res.findings.is_empty()
            || res
                .findings
                .iter()
                .all(|f| f.severity == FindingSeverity::Info)
    );

    fs::remove_dir_all(&root).unwrap();
}

// ---------------------------------------------------------------------------
// 2. Warning-only findings
// ---------------------------------------------------------------------------
#[test]
fn test_02_warning_only_findings() {
    let root = setup_test_git_repo("warning_only");

    // Contract expecting calc.rs
    let contract = create_contract(
        &root,
        "Implement tax calculation",
        Some(vec!["tax".to_string()]),
        Some(vec!["src/tax/calc.rs".to_string()]),
        Some(vec!["src/payments/**".to_string()]),
    )
    .unwrap();
    save_contract(&root, &contract).unwrap();

    // Modify calc.rs and also extra.rs (unexpected, but not forbidden)
    let tax_file = root.join("src").join("tax").join("calc.rs");
    fs::create_dir_all(tax_file.parent().unwrap()).unwrap();
    fs::write(&tax_file, "// tax\n").unwrap();

    let extra_file = root.join("src").join("other").join("extra.rs");
    fs::create_dir_all(extra_file.parent().unwrap()).unwrap();
    fs::write(&extra_file, "// extra\n").unwrap();

    // Policy with strict=false and contracts severity="warn"
    let policy_toml = r#"
version = "1"
[checks.contracts]
enabled = true
required = false
severity = "warn"
strict = false
[checks.constitution]
enabled = false
severity = "block"
[checks.impact]
enabled = false
severity = "warn"
"#;
    let policy_path = root.join("custom-policy.toml");
    fs::write(&policy_path, policy_toml).unwrap();

    let opts = GuardOptions {
        policy_path: Some(policy_path),
        ..Default::default()
    };
    let res = run_guard(&root, &opts);

    assert_eq!(res.status, GuardStatus::Warn);
    assert_eq!(
        res.exit_code, 0,
        "Warning-only findings must yield exit code 0"
    );
    assert!(!res.blocked);
    assert!(res
        .findings
        .iter()
        .any(|f| f.severity == FindingSeverity::Warn));

    fs::remove_dir_all(&root).unwrap();
}

// ---------------------------------------------------------------------------
// 3. Blocking findings and nonzero exit codes
// ---------------------------------------------------------------------------
#[test]
fn test_03_blocking_findings_and_nonzero_exit_codes() {
    let root = setup_test_git_repo("blocking_findings");

    // Contract forbidding src/payments/**
    let contract = create_contract(
        &root,
        "Refactor checkout",
        Some(vec!["checkout".to_string()]),
        Some(vec!["src/checkout/flow.rs".to_string()]),
        Some(vec!["src/payments/**".to_string()]),
    )
    .unwrap();
    save_contract(&root, &contract).unwrap();

    // Breach forbidden boundary
    let forbidden_file = root.join("src").join("payments").join("vault.rs");
    fs::create_dir_all(forbidden_file.parent().unwrap()).unwrap();
    fs::write(&forbidden_file, "// breached\n").unwrap();

    let opts = GuardOptions::default();
    let res = run_guard(&root, &opts);

    assert_eq!(res.status, GuardStatus::Block);
    assert_eq!(res.exit_code, 1);
    assert!(res.blocked);
    assert!(res
        .findings
        .iter()
        .any(|f| f.severity == FindingSeverity::Block && f.rule_id == "GUARD-CONTRACT-002"));

    fs::remove_dir_all(&root).unwrap();
}

// ---------------------------------------------------------------------------
// 4. Mandatory check failures
// ---------------------------------------------------------------------------
#[test]
fn test_04_mandatory_check_failures() {
    let root = setup_test_git_repo("mandatory_failures");

    // Create policy requiring contracts
    let policy_toml = r#"
version = "1"
[checks.contracts]
enabled = true
required = true
severity = "block"
strict = true
[checks.constitution]
enabled = true
required = true
severity = "block"
[checks.impact]
enabled = false
severity = "warn"
"#;
    let policy_path = root.join("mandatory-policy.toml");
    fs::write(&policy_path, policy_toml).unwrap();

    // Create a change without an active contract
    let code_file = root.join("src").join("app.rs");
    fs::create_dir_all(code_file.parent().unwrap()).unwrap();
    fs::write(&code_file, "// code\n").unwrap();

    let opts = GuardOptions {
        policy_path: Some(policy_path),
        ..Default::default()
    };
    let res = run_guard(&root, &opts);

    assert_eq!(res.status, GuardStatus::Block);
    assert_eq!(res.exit_code, 1);
    assert!(res.blocked);
    assert!(res
        .findings
        .iter()
        .any(|f| f.rule_id == "GUARD-CONTRACT-001" && f.severity == FindingSeverity::Block));

    fs::remove_dir_all(&root).unwrap();
}

// ---------------------------------------------------------------------------
// 5. Missing configuration
// ---------------------------------------------------------------------------
#[test]
fn test_05_missing_configuration_safely_defaults() {
    let root = setup_test_git_repo("missing_config");

    // Ensure no .engram/change-guard.toml exists
    let guard_toml = root.join(".engram").join("change-guard.toml");
    if guard_toml.exists() {
        fs::remove_file(&guard_toml).unwrap();
    }

    // Default policy permits optional contracts
    let opts = GuardOptions::default();
    let res = run_guard(&root, &opts);

    // Should not error, defaults to Pass or Warn
    assert_ne!(res.status, GuardStatus::Error);
    assert_eq!(res.exit_code, 0);

    fs::remove_dir_all(&root).unwrap();
}

// ---------------------------------------------------------------------------
// 6. Malformed and unsupported policy versions
// ---------------------------------------------------------------------------
#[test]
fn test_06_malformed_and_unsupported_policy_versions() {
    let root = setup_test_git_repo("malformed_policy");

    // Case A: Syntax error in TOML
    let malformed_toml = "this is not = = = valid toml [[[";
    let bad_path = root.join("bad.toml");
    fs::write(&bad_path, malformed_toml).unwrap();

    let opts_bad = GuardOptions {
        policy_path: Some(bad_path),
        ..Default::default()
    };
    let res_bad = run_guard(&root, &opts_bad);
    assert_eq!(res_bad.status, GuardStatus::Error);
    assert_eq!(res_bad.exit_code, 2);
    assert!(res_bad.blocked);

    // Case B: Unsupported version "2"
    let unsupported_version = r#"
version = "2"
[checks.contracts]
enabled = true
severity = "block"
"#;
    let unsupp_path = root.join("unsupported.toml");
    fs::write(&unsupp_path, unsupported_version).unwrap();

    let opts_unsupp = GuardOptions {
        policy_path: Some(unsupp_path),
        ..Default::default()
    };
    let res_unsupp = run_guard(&root, &opts_unsupp);
    assert_eq!(res_unsupp.status, GuardStatus::Error);
    assert_eq!(res_unsupp.exit_code, 2);
    assert!(res_unsupp.summary.contains("Unsupported policy version"));

    fs::remove_dir_all(&root).unwrap();
}

// ---------------------------------------------------------------------------
// 7. Missing active contract
// ---------------------------------------------------------------------------
#[test]
fn test_07_missing_active_contract_distinction() {
    let root = setup_test_git_repo("missing_active_contract");

    let code_file = root.join("src").join("service.rs");
    fs::create_dir_all(code_file.parent().unwrap()).unwrap();
    fs::write(&code_file, "// service\n").unwrap();

    // Default policy has required = false -> contract check is reported as not_applicable, not failed
    let opts = GuardOptions::default();
    let res = run_guard(&root, &opts);

    assert_eq!(res.exit_code, 0);
    assert!(res
        .checks_skipped
        .iter()
        .any(|c| c.name == "contracts" && c.status == "not_applicable"));

    fs::remove_dir_all(&root).unwrap();
}

// ---------------------------------------------------------------------------
// 8. Constitution violation
// ---------------------------------------------------------------------------
#[test]
fn test_08_constitution_violation() {
    let root = setup_test_git_repo("constitution_violation");

    // Create .engram/constitution.yaml with forbidden boundary
    let engram_dir = root.join(".engram");
    fs::create_dir_all(&engram_dir).unwrap();

    let constitution_yaml = r#"
schema_version: 1
rules:
  - id: "ARCH-001"
    name: "Core Engine Integrity"
    type: PATTERN
    severity: ERROR
    forbidden: "src/core/security/**"
"#;
    fs::write(engram_dir.join("constitution.yaml"), constitution_yaml).unwrap();

    // Breach constitutional rule
    let sec_file = root
        .join("src")
        .join("core")
        .join("security")
        .join("auth.rs");
    fs::create_dir_all(sec_file.parent().unwrap()).unwrap();
    fs::write(&sec_file, "// security breach\n").unwrap();

    let opts = GuardOptions::default();
    let res = run_guard(&root, &opts);

    assert_eq!(res.status, GuardStatus::Block);
    assert_eq!(res.exit_code, 1);
    assert!(res
        .findings
        .iter()
        .any(|f| f.rule_id == "GUARD-CONST-ARCH-001" && f.severity == FindingSeverity::Block));

    fs::remove_dir_all(&root).unwrap();
}

// ---------------------------------------------------------------------------
// 9. Impact analysis findings
// ---------------------------------------------------------------------------
#[test]
fn test_09_impact_analysis_findings() {
    let root = setup_test_git_repo("impact_analysis");

    // Policy with very low blast radius limit (max_affected_symbols = 0)
    let policy_toml = r#"
version = "1"
[checks.contracts]
enabled = false
severity = "block"
[checks.constitution]
enabled = false
severity = "block"
[checks.impact]
enabled = true
max_affected_symbols = 0
severity = "warn"
"#;
    let policy_path = root.join("impact-policy.toml");
    fs::write(&policy_path, policy_toml).unwrap();

    // Create and analyze code file to populate entities
    let calc_file = root.join("src").join("calc.rs");
    fs::create_dir_all(calc_file.parent().unwrap()).unwrap();
    fs::write(&calc_file, "pub fn tax_calc() -> u32 { 42 }\n").unwrap();
    engram::analyzer::analyze(&root).unwrap();

    // Modify file so it is detected as changed
    fs::write(&calc_file, "pub fn tax_calc() -> u32 { 100 }\n").unwrap();

    let opts = GuardOptions {
        policy_path: Some(policy_path),
        ..Default::default()
    };
    let res = run_guard(&root, &opts);

    // Blast radius exceeded should produce a finding with stable rule ID GUARD-IMPACT-001
    assert!(res
        .findings
        .iter()
        .any(|f| f.rule_id == "GUARD-IMPACT-001" && f.severity == FindingSeverity::Warn));
    assert_eq!(res.status, GuardStatus::Warn);
    assert_eq!(res.exit_code, 0);

    fs::remove_dir_all(&root).unwrap();
}

// ---------------------------------------------------------------------------
// 10. Check skipped versus check failed
// ---------------------------------------------------------------------------
#[test]
fn test_10_check_skipped_versus_check_failed() {
    let root = setup_test_git_repo("skipped_vs_failed");

    // Policy with impact disabled
    let policy_toml = r#"
version = "1"
[checks.contracts]
enabled = false
severity = "block"
[checks.constitution]
enabled = true
severity = "block"
[checks.impact]
enabled = false
severity = "warn"
"#;
    let policy_path = root.join("selective-policy.toml");
    fs::write(&policy_path, policy_toml).unwrap();

    let opts = GuardOptions {
        policy_path: Some(policy_path),
        ..Default::default()
    };
    let res = run_guard(&root, &opts);

    // Impact was skipped, not failed
    assert!(!res.checks_executed.contains(&"impact".to_string()));
    assert!(res
        .checks_skipped
        .iter()
        .any(|c| c.name == "impact" && c.status == "skipped"));
    assert!(res.checks_executed.contains(&"constitution".to_string()));

    fs::remove_dir_all(&root).unwrap();
}

// ---------------------------------------------------------------------------
// 11. Deterministic JSON output
// ---------------------------------------------------------------------------
#[test]
fn test_11_deterministic_json_output() {
    let root = setup_test_git_repo("deterministic_json");

    let f = root.join("src").join("lib.rs");
    fs::create_dir_all(f.parent().unwrap()).unwrap();
    fs::write(&f, "// hello\n").unwrap();

    let opts = GuardOptions::default();
    let res1 = run_guard(&root, &opts);
    let res2 = run_guard(&root, &opts);

    let json1 = serde_json::to_string(&res1).unwrap();
    let json2 = serde_json::to_string(&res2).unwrap();

    assert_eq!(
        json1, json2,
        "Repeated guard runs must produce identical JSON"
    );

    fs::remove_dir_all(&root).unwrap();
}

// ---------------------------------------------------------------------------
// 12. Stable rule identifiers
// ---------------------------------------------------------------------------
#[test]
fn test_12_stable_rule_identifiers() {
    let root = setup_test_git_repo("stable_ids");

    let contract = create_contract(
        &root,
        "Checkout task",
        Some(vec!["checkout".to_string()]),
        Some(vec!["src/checkout/flow.rs".to_string()]),
        Some(vec!["src/payments/**".to_string()]),
    )
    .unwrap();
    save_contract(&root, &contract).unwrap();

    let forbidden_file = root.join("src").join("payments").join("gateway.rs");
    fs::create_dir_all(forbidden_file.parent().unwrap()).unwrap();
    fs::write(&forbidden_file, "// forbidden\n").unwrap();

    let res = run_guard(&root, &GuardOptions::default());

    let rule_ids: Vec<String> = res.findings.iter().map(|f| f.rule_id.clone()).collect();
    assert!(rule_ids.contains(&"GUARD-CONTRACT-002".to_string()));

    fs::remove_dir_all(&root).unwrap();
}

// ---------------------------------------------------------------------------
// 13. Path exclusions and invalid exclusion patterns
// ---------------------------------------------------------------------------
#[test]
fn test_13_path_exclusions_and_invalid_exclusion_patterns() {
    let root = setup_test_git_repo("path_exclusions");

    // Case A: Valid exclusion
    let doc_file = root.join("docs").join("guide.md");
    fs::create_dir_all(doc_file.parent().unwrap()).unwrap();
    fs::write(&doc_file, "# Documentation\n").unwrap();

    let opts = GuardOptions::default();
    let res = run_guard(&root, &opts);

    // doc_file must be excluded from evaluation
    assert!(!res.changed_files.iter().any(|f| f.contains("guide.md")));

    // Case B: Broad wildcard rejection
    let broad_wildcard_policy = r#"
version = "1"
[checks.contracts]
enabled = true
severity = "block"
[exclusions]
paths = ["*"]
"#;
    let err = parse_policy_toml(broad_wildcard_policy).unwrap_err();
    assert!(err.to_string().contains("Broad wildcard exclusion"));

    fs::remove_dir_all(&root).unwrap();
}

// ---------------------------------------------------------------------------
// 14. Read-only behavior: no source modifications or staging
// ---------------------------------------------------------------------------
#[test]
fn test_14_read_only_behavior_no_source_modifications_or_staging() {
    let root = setup_test_git_repo("read_only_check");

    let code_file = root.join("src").join("core.rs");
    fs::create_dir_all(code_file.parent().unwrap()).unwrap();
    fs::write(&code_file, "// original content\n").unwrap();

    // Verify unstaged initially
    let status_pre = Command::new("git")
        .args(["status", "--porcelain", "-uall"])
        .current_dir(&root)
        .output()
        .unwrap();
    let pre_str = String::from_utf8_lossy(&status_pre.stdout);
    assert!(pre_str.contains("src/core.rs"));

    // Run guard
    let _ = run_guard(&root, &GuardOptions::default());

    // Verify no files were staged into git index
    let staged_out = Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        staged_out.stdout.is_empty(),
        "Guard must never stage files into the git index"
    );

    // Verify source file remains untracked/unstaged and content is untouched
    let status_post = Command::new("git")
        .args(["status", "--porcelain", "-uall"])
        .current_dir(&root)
        .output()
        .unwrap();
    let post_str = String::from_utf8_lossy(&status_post.stdout);
    assert!(
        post_str.contains("?? src/core.rs"),
        "Source file must remain untracked/unstaged"
    );

    let content = fs::read_to_string(&code_file).unwrap();
    assert_eq!(content, "// original content\n");

    fs::remove_dir_all(&root).unwrap();
}

// ---------------------------------------------------------------------------
// 15. Hook installation safeguards, including existing user-owned hooks
// ---------------------------------------------------------------------------
#[test]
fn test_15_hook_installation_safeguards() {
    let root = setup_test_git_repo("hook_safeguards");
    let hook_path = root.join(".git").join("hooks").join("pre-commit");
    fs::create_dir_all(hook_path.parent().unwrap()).unwrap();

    // User has an existing custom hook
    let foreign_hook = "#!/bin/sh\n# Custom linter\nnpm run lint\n";
    fs::write(&hook_path, foreign_hook).unwrap();

    // Dry-run install
    let dry_msg = install_guard_hook(&root, true).unwrap();
    assert!(dry_msg.contains("Dry-run"));
    assert_eq!(fs::read_to_string(&hook_path).unwrap(), foreign_hook);

    // Real install
    let install_msg = install_guard_hook(&root, false).unwrap();
    assert!(install_msg.contains("Preserved existing pre-commit hook"));

    let updated_content = fs::read_to_string(&hook_path).unwrap();
    assert!(updated_content.contains("npm run lint"));
    assert!(updated_content.contains(GUARD_HOOK_START));
    assert!(updated_content.contains(GUARD_HOOK_BODY));

    // Re-running install is idempotent
    let re_install = install_guard_hook(&root, false).unwrap();
    assert!(re_install.contains("already installed"));

    fs::remove_dir_all(&root).unwrap();
}

// ---------------------------------------------------------------------------
// 16. Hook removal preserving unrelated content
// ---------------------------------------------------------------------------
#[test]
fn test_16_hook_removal_preserving_unrelated_content() {
    let root = setup_test_git_repo("hook_removal");
    let hook_path = root.join(".git").join("hooks").join("pre-commit");
    fs::create_dir_all(hook_path.parent().unwrap()).unwrap();

    let foreign_hook = "#!/bin/sh\n# Custom security scanner\ngit secrets --scan\n";
    let combined_hook = format!("{}\n\n{}\n", foreign_hook.trim(), GUARD_HOOK_BODY);
    fs::write(&hook_path, combined_hook).unwrap();

    // Dry-run removal
    let dry_msg = uninstall_guard_hook(&root, true).unwrap();
    assert!(dry_msg.contains("Dry-run"));
    assert!(hook_path.exists());

    // Real removal
    let remove_msg = uninstall_guard_hook(&root, false).unwrap();
    assert!(remove_msg.contains("Removed Engram block while preserving foreign hooks"));

    let remaining_content = fs::read_to_string(&hook_path).unwrap();
    assert!(remaining_content.contains("git secrets --scan"));
    assert!(!remaining_content.contains(GUARD_HOOK_START));

    fs::remove_dir_all(&root).unwrap();
}

// ---------------------------------------------------------------------------
// 17. CI policy behavior for warning and blocking modes
// ---------------------------------------------------------------------------
#[test]
fn test_17_ci_policy_behavior_for_warning_and_blocking_modes() {
    let root = setup_test_git_repo("ci_policy_modes");

    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(out.status.success(), "Git command failed: {:?}", args);
    };

    // Create a feature branch
    run(&["checkout", "-b", "feature/new-tax"]);

    // Contract on main
    let contract = create_contract(
        &root,
        "Feature tax",
        Some(vec!["tax".to_string()]),
        Some(vec!["src/tax.rs".to_string()]),
        Some(vec!["src/forbidden/**".to_string()]),
    )
    .unwrap();
    save_contract(&root, &contract).unwrap();

    // In feature branch, add unexpected file
    let unexpected = root.join("src").join("extra.rs");
    fs::create_dir_all(unexpected.parent().unwrap()).unwrap();
    fs::write(&unexpected, "// unexpected\n").unwrap();
    run(&["add", "src/extra.rs"]);
    run(&["commit", "-m", "Add unexpected file"]);

    // Mode A: Advisory warning (strict = false)
    let opts_advisory = GuardOptions {
        base_commit: Some("main".to_string()),
        strict: false,
        ..Default::default()
    };
    let res_advisory = run_guard(&root, &opts_advisory);
    assert_eq!(res_advisory.status, GuardStatus::Warn);
    assert_eq!(res_advisory.exit_code, 0);
    assert!(!res_advisory.blocked);

    // Mode B: Blocking (strict = true)
    let opts_strict = GuardOptions {
        base_commit: Some("main".to_string()),
        strict: true,
        ..Default::default()
    };
    let res_strict = run_guard(&root, &opts_strict);
    assert_eq!(res_strict.status, GuardStatus::Block);
    assert_eq!(res_strict.exit_code, 1);
    assert!(res_strict.blocked);

    fs::remove_dir_all(&root).unwrap();
}
