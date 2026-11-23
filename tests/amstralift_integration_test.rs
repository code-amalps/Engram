use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

use engram::amstralift::{
    generate_contract_for_plan, parse_modernization_plan, parse_transformation_evidence,
    validate_modernization_plan, verify_transformation_evidence,
};

fn setup_test_repo(prefix: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("engram_amstra_test_{}_{}", prefix, Uuid::new_v4()));
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
    run(&["config", "user.name", "AMstraLift Tester"]);
    run(&["config", "user.email", "amstra-tester@engram.dev"]);

    let init_file = dir.join("README.md");
    fs::write(&init_file, "# Test Repo\n").unwrap();
    run(&["add", "README.md"]);
    run(&["commit", "-m", "Initial commit"]);

    dir
}

#[test]
fn test_parse_valid_yaml_plan() {
    let yaml = r#"
plan_id: plan_angular_18_upgrade
ecosystem: angular
current_version: "16.2.0"
target_version: "18.1.0"
target_major: 18
total_transformations: 3
safe_count: 2
risky_count: 1
phases:
  - phase_index: 0
    name: "Core Dependencies"
    phase_type: "dependencies"
    description: "Upgrade framework packages"
    transformations:
      - id: "dep_angular_core"
        name: "Upgrade @angular/core to 18"
        status: "safe"
        affected_file_patterns:
          - "package.json"
  - phase_index: 1
    name: "Control Flow Migration"
    phase_type: "template_migration"
    transformations:
      - id: "tmpl_control_flow"
        name: "Convert *ngIf and *ngFor to @if and @for"
        status: "safe"
        affected_file_patterns:
          - "src/app/**/*.html"
"#;

    let plan = parse_modernization_plan(yaml).expect("Valid YAML should parse");
    assert_eq!(plan.plan_id, "plan_angular_18_upgrade");
    assert_eq!(plan.ecosystem, "angular");
    assert_eq!(plan.target_major, Some(18));
    assert_eq!(plan.phases.len(), 2);
    assert_eq!(plan.phases[0].transformations.len(), 1);
    assert_eq!(plan.phases[1].transformations[0].id, "tmpl_control_flow");
}

#[test]
fn test_parse_valid_json_plan() {
    let json = serde_json::json!({
        "plan_id": "plan_dotnet_8_upgrade",
        "ecosystem": "dotnet",
        "current_version": "6.0",
        "target_version": "8.0",
        "target_major": 8,
        "total_transformations": 1,
        "phases": [
            {
                "phase_index": 0,
                "name": "Target Framework",
                "phase_type": "configuration",
                "transformations": [
                    {
                        "id": "tfm_net8",
                        "name": "Update TargetFramework to net8.0",
                        "status": "safe",
                        "affected_file_patterns": ["src/**/*.csproj"]
                    }
                ]
            }
        ]
    })
    .to_string();

    let plan = parse_modernization_plan(&json).expect("Valid JSON should parse");
    assert_eq!(plan.plan_id, "plan_dotnet_8_upgrade");
    assert_eq!(plan.ecosystem, "dotnet");
    assert_eq!(
        plan.phases[0].transformations[0].affected_file_patterns,
        vec!["src/**/*.csproj"]
    );
}

#[test]
fn test_validate_plan_accepts_clean_targets() {
    let root = setup_test_repo("plan_clean");

    let yaml = r#"
plan_id: plan_clean_01
ecosystem: python
total_transformations: 1
phases:
  - phase_index: 0
    name: "Dependencies"
    phase_type: "dependencies"
    transformations:
      - id: "py_dep_01"
        name: "Upgrade httpx"
        status: "safe"
        affected_file_patterns:
          - "pyproject.toml"
          - "src/service.py"
"#;

    let plan = parse_modernization_plan(yaml).unwrap();
    let report = validate_modernization_plan(&plan, &root).unwrap();

    assert!(report.valid, "Clean plan should be valid");
    assert_eq!(
        report.expected_files,
        vec!["pyproject.toml", "src/service.py"]
    );
    assert!(report.findings.is_empty());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_validate_plan_rejects_path_traversal() {
    let root = setup_test_repo("plan_traversal");

    let yaml = r#"
plan_id: plan_traversal_attack
ecosystem: react
total_transformations: 1
phases:
  - phase_index: 0
    name: "Malicious"
    phase_type: "dependencies"
    transformations:
      - id: "evil_transform"
        name: "Escape workspace"
        status: "safe"
        affected_file_patterns:
          - "../../../etc/passwd"
"#;

    let plan = parse_modernization_plan(yaml).unwrap();
    let report = validate_modernization_plan(&plan, &root).unwrap();

    assert!(!report.valid, "Plan with path traversal MUST be invalid");
    assert!(report.findings.iter().any(|f| f.code == "AMSTRA-SEC-001"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_validate_plan_rejects_security_files() {
    let root = setup_test_repo("plan_security");

    let yaml = r#"
plan_id: plan_security_attack
ecosystem: python
total_transformations: 1
phases:
  - phase_index: 0
    name: "Security"
    phase_type: "configuration"
    transformations:
      - id: "touch_auth_token"
        name: "Modify token logic"
        status: "safe"
        affected_file_patterns:
          - "src/auth/token_verifier.py"
"#;

    let plan = parse_modernization_plan(yaml).unwrap();
    let report = validate_modernization_plan(&plan, &root).unwrap();

    assert!(
        !report.valid,
        "Plan touching security files MUST be invalid"
    );
    assert!(report.findings.iter().any(|f| f.code == "AMSTRA-SEC-002"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_validate_plan_rejects_sovereign_policy_files() {
    let root = setup_test_repo("plan_sovereign");

    let yaml = r#"
plan_id: plan_policy_attack
ecosystem: angular
total_transformations: 1
phases:
  - phase_index: 0
    name: "Policy"
    phase_type: "configuration"
    transformations:
      - id: "touch_constitution"
        name: "Alter governance"
        status: "safe"
        affected_file_patterns:
          - ".engram/constitution.yaml"
"#;

    let plan = parse_modernization_plan(yaml).unwrap();
    let report = validate_modernization_plan(&plan, &root).unwrap();

    assert!(
        !report.valid,
        "Plan touching sovereign policy MUST be invalid"
    );
    assert!(report.findings.iter().any(|f| f.code == "AMSTRA-SEC-003"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_generate_contract_for_valid_plan() {
    let root = setup_test_repo("contract_gen");

    let yaml = r#"
plan_id: plan_react_19
ecosystem: react
current_version: "18.2.0"
target_version: "19.0.0"
total_transformations: 2
phases:
  - phase_index: 0
    name: "Dependencies"
    phase_type: "dependencies"
    transformations:
      - id: "react_pkg"
        name: "Upgrade react and react-dom"
        status: "safe"
        affected_file_patterns:
          - "package.json"
          - "src/**/*.tsx"
"#;

    let plan = parse_modernization_plan(yaml).unwrap();
    let contract =
        generate_contract_for_plan(&root, &plan, "Human ratified React 19 upgrade").unwrap();

    assert_eq!(contract.contract_id, "ctr_amstralift_plan_react_19");
    assert!(
        contract
            .boundary
            .expected_files
            .contains(&"package.json".to_string())
    );
    assert!(
        contract
            .boundary
            .expected_modules
            .contains(&"src".to_string())
    );

    // Verify contract persisted on disk
    let loaded = engram::contract::get_contract(&root, &contract.contract_id).unwrap();
    assert_eq!(loaded.contract_id, contract.contract_id);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_generate_contract_fails_closed_on_invalid_plan() {
    let root = setup_test_repo("contract_gen_fail");

    let yaml = r#"
plan_id: plan_bad
ecosystem: react
phases:
  - phase_index: 0
    name: "Bad"
    phase_type: "dependencies"
    transformations:
      - id: "bad_traversal"
        name: "Bad traversal"
        status: "safe"
        affected_file_patterns:
          - "../evil.js"
"#;

    let plan = parse_modernization_plan(yaml).unwrap();
    let res = generate_contract_for_plan(&root, &plan, "Attempt invalid contract");
    assert!(
        res.is_err(),
        "Must fail closed and refuse to create contract for invalid plan"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_verify_evidence_guarantees_clean_refusals() {
    let json = serde_json::json!([
        {
            "id": "res_applied_01",
            "adapter": "angular",
            "rule_id": "standalone",
            "file_path": "src/app/button.component.ts",
            "decision": "APPLIED",
            "transformation_record": {
                "ast_nodes_changed": 3,
                "sandbox_verified": true
            }
        },
        {
            "id": "res_refused_01",
            "adapter": "angular",
            "rule_id": "control-flow",
            "file_path": "src/app/legacy.component.html",
            "decision": "REFUSED",
            "refusal_record": {
                "category": "unsupported_pattern",
                "reason": "Dynamic template bindings not deterministically migratable",
                "code_modified": false
            }
        }
    ])
    .to_string();

    let results = parse_transformation_evidence(&json).unwrap();
    let report = verify_transformation_evidence(&results);

    assert!(report.valid, "Evidence with clean refusal must be valid");
    assert_eq!(report.applied_count, 1);
    assert_eq!(report.refused_count, 1);
    assert_eq!(
        report.files_refused_clean,
        vec!["src/app/legacy.component.html"]
    );
    assert!(report.violations.is_empty());
}

#[test]
fn test_verify_evidence_detects_safety_breach_if_refusal_modified_code() {
    let json = serde_json::json!([
        {
            "id": "res_breach_01",
            "adapter": "python",
            "rule_id": "typing-pep604",
            "file_path": "src/models.py",
            "decision": "REFUSED",
            "refusal_record": {
                "category": "compilation_risk",
                "reason": "Incompatible union types",
                "code_modified": true // SAFETY BREACH! Refusal must NEVER modify code!
            }
        }
    ])
    .to_string();

    let results = parse_transformation_evidence(&json).unwrap();
    let report = verify_transformation_evidence(&results);

    assert!(
        !report.valid,
        "Evidence where refusal modified code MUST be invalid"
    );
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.contains("SAFETY BREACH"))
    );
}
