//! Regression and empirical tests for Fix A, B, C, D:
//! Context Efficiency, Incremental Preflight, Minimal Blast Radius Enforcement, and Copilot Integration.

use engram::contract::{
    ChangeContract, ContractBoundary, ContractProvenance, ContractStatus, FileValidationCategory,
    extract_module, validate_contract_diff_with_constitution,
};
use engram::flywheel::{
    PreflightOptions, PreflightStatus, parse_task_and_planned_files, preflight_task,
};
use engram::instructions::{self, DEFAULT_INSTRUCTION_BODY, ManagedBlockAction};
use engram::mcp::mcp_call;
use engram::prompt;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn setup_test_workspace(name: &str) -> PathBuf {
    let tmp = std::env::temp_dir().join(format!("engram_eff_test_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let files = vec![
        ("package.json", r#"{"name": "test-project", "version": "1.0.0", "dependencies": {"@angular/core": "^17.0.0"}}"#),
        ("src/app/core/auth/auth.effects.ts", "export class AuthEffects { login$ = true; }"),
        ("src/app/core/auth/auth.models.ts", "export interface User { username: string; }"),
        ("src/app/core/auth/auth.reducer.ts", "export function authReducer() { return {}; }"),
        ("src/app/features/settings/settings.component.ts", "export class SettingsComponent {}"),
        ("src/assets/i18n/en.json", r#"{"auth.login": "Log In"}"#),
        ("src/assets/i18n/de.json", r#"{"auth.login": "Anmelden"}"#),
        ("src/assets/i18n/fr.json", r#"{"auth.login": "Connexion"}"#),
        ("src/assets/i18n/es.json", r#"{"auth.login": "Iniciar Sesión"}"#),
        ("src/assets/i18n/it.json", r#"{"auth.login": "Accedi"}"#),
    ];

    for (rel, content) in files {
        let p = tmp.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    Command::new("git")
        .args(["init"])
        .current_dir(&tmp)
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.name", "Test Engineer"])
        .current_dir(&tmp)
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@engram.dev"])
        .current_dir(&tmp)
        .output()
        .unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(&tmp)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&tmp)
        .output()
        .unwrap();

    engram::analyzer::analyze(&tmp).unwrap();
    tmp
}

#[test]
fn test_fix_a_compact_vs_detailed_get_context() {
    let root = setup_test_workspace("fix_a_context");

    // 1. Default Compact Call (details: false / omitted)
    let compact_args = json!({
        "task": "Add a remember-me option to authentication flow",
        "token_budget": 1200
    });
    let compact_val = mcp_call(&root, "engram_get_context", &compact_args).unwrap();
    let compact_str = serde_json::to_string(&compact_val).unwrap();

    // 2. Detailed Call (details: true)
    let detailed_args = json!({
        "task": "Add a remember-me option to authentication flow",
        "token_budget": 1200,
        "details": true
    });
    let detailed_val = mcp_call(&root, "engram_get_context", &detailed_args).unwrap();
    let detailed_str = serde_json::to_string(&detailed_val).unwrap();

    // Compact output must contain structured essentials
    let text = compact_val["content"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["status"], "READY");
    assert!(parsed["target_files"].is_array());
    assert!(parsed["next_action"].is_string());

    // Character size reduction: compact must be significantly smaller than detailed
    let compact_len = compact_str.len();
    let detailed_len = detailed_str.len();
    assert!(
        compact_len < detailed_len,
        "Compact payload ({compact_len} chars) must be smaller than detailed ({detailed_len} chars)"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_fix_a_compact_vs_detailed_preflight_and_postflight() {
    let root = setup_test_workspace("fix_a_pre_post");

    // 1. Compact Preflight
    let pre_compact_args = json!({
        "task": "Add remember me option. Planned files: src/app/core/auth/auth.effects.ts",
        "token_budget": 1200
    });
    let pre_compact = mcp_call(&root, "engram_preflight", &pre_compact_args).unwrap();
    let pre_compact_text = pre_compact["content"][0]["text"].as_str().unwrap();
    let pre_parsed: serde_json::Value = serde_json::from_str(pre_compact_text).unwrap();
    assert_eq!(pre_parsed["status"], "READY_FOR_IMPLEMENTATION");
    assert!(pre_parsed["contract_id"].is_string());
    assert!(pre_parsed["expected_files"].is_array());
    assert!(pre_parsed["next_action"].is_string());
    let contract_id = pre_parsed["contract_id"].as_str().unwrap().to_string();

    // 2. Detailed Preflight
    let pre_detailed_args = json!({
        "task": "Add remember me option. Planned files: src/app/core/auth/auth.effects.ts",
        "token_budget": 1200,
        "details": true
    });
    let pre_detailed = mcp_call(&root, "engram_preflight", &pre_detailed_args).unwrap();
    let pre_detailed_text = pre_detailed["content"][0]["text"].as_str().unwrap();
    assert!(pre_compact_text.len() < pre_detailed_text.len());

    // 3. Compact Postflight
    let post_compact_args = json!({
        "contract_id": contract_id,
        "details": false
    });
    let post_compact = mcp_call(&root, "engram_postflight", &post_compact_args).unwrap();
    let post_compact_text = post_compact["content"][0]["text"].as_str().unwrap();
    let post_parsed: serde_json::Value = serde_json::from_str(post_compact_text).unwrap();
    assert_eq!(post_parsed["verdict"], "PASS");
    assert_eq!(post_parsed["status"], "COMPLIANT");
    assert!(post_parsed["violations"].is_array());
    assert!(post_parsed["next_action"].is_string());

    // 4. Detailed Postflight
    let post_detailed_args = json!({
        "contract_id": contract_id,
        "details": true
    });
    let post_detailed = mcp_call(&root, "engram_postflight", &post_detailed_args).unwrap();
    let post_detailed_text = post_detailed["content"][0]["text"].as_str().unwrap();
    assert!(post_compact_text.len() < post_detailed_text.len());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_fix_b_idempotent_preflight_and_incremental_delta() {
    let root = setup_test_workspace("fix_b_idempotency");

    // Turn 1: Cold Preflight
    let opts1 = PreflightOptions {
        task: "Add remember me option to auth".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["src/app/features/**".to_string()]),
        planned_files: Some(vec!["src/app/core/auth/auth.effects.ts".to_string()]),
    };
    let res1 = preflight_task(&root, &opts1).unwrap();
    assert_eq!(res1.status, PreflightStatus::ReadyForImplementation);
    assert!(!res1.is_reused_contract);
    assert_eq!(res1.contract_version, 1);
    let original_id = res1.contract_id.clone();

    // Turn 2: Exact Idempotent Call (same task and files)
    let res2 = preflight_task(&root, &opts1).unwrap();
    assert_eq!(res2.status, PreflightStatus::ReadyForImplementation);
    assert!(res2.is_reused_contract);
    assert_eq!(res2.contract_id, original_id);
    assert_eq!(res2.contract_version, 1);
    assert!(res2.reused_reason.as_deref().unwrap_or("").contains("fingerprint"));

    // Turn 3: Incremental Delta (planned_files expanded)
    let opts3 = PreflightOptions {
        task: "Add remember me option to auth".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["src/app/features/**".to_string()]),
        planned_files: Some(vec![
            "src/app/core/auth/auth.effects.ts".to_string(),
            "src/app/core/auth/auth.tokens.ts".to_string(),
        ]),
    };
    let res3 = preflight_task(&root, &opts3).unwrap();
    assert_eq!(res3.status, PreflightStatus::ReadyForImplementation);
    assert!(res3.is_reused_contract);
    assert_eq!(res3.contract_id, original_id);
    assert_eq!(res3.contract_version, 2, "Contract version must increment on approved delta expansion");
    assert!(res3.expected_files.contains(&"src/app/core/auth/auth.tokens.ts".to_string()));

    // Turn 4: Delta adding forbidden file -> BLOCKED
    let opts4 = PreflightOptions {
        task: "Add remember me option to auth".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["src/app/features/**".to_string()]),
        planned_files: Some(vec![
            "src/app/core/auth/auth.effects.ts".to_string(),
            "src/app/features/settings/settings.component.ts".to_string(),
        ]),
    };
    let res4 = preflight_task(&root, &opts4).unwrap();
    assert_eq!(res4.status, PreflightStatus::Blocked);
    assert!(res4.message.contains("forbidden"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_fix_b_task_string_planned_files_parsing() {
    let task = "Add remember-me option to auth flow. Planned files: src/app/core/auth/auth.effects.ts, src/app/core/auth/auth.models.ts";
    let (core, planned) = parse_task_and_planned_files(task, None);
    assert_eq!(core, "Add remember-me option to auth flow");
    assert_eq!(planned.len(), 2);
    assert_eq!(planned[0], "src/app/core/auth/auth.effects.ts");
    assert_eq!(planned[1], "src/app/core/auth/auth.models.ts");
}

#[test]
fn test_fix_c_module_extraction_monorepos_and_assets() {
    // Monorepo Angular paths
    assert_eq!(
        extract_module("projects/angular-starter/src/app/core/auth/auth.effects.ts"),
        "core"
    );
    assert_eq!(
        extract_module("projects/angular-starter/src/app/features/settings/settings.component.ts"),
        "features"
    );
    assert_eq!(
        extract_module("projects/angular-starter/src/assets/i18n/de.json"),
        "assets"
    );

    // Standard Go / Rust / Python layouts
    assert_eq!(extract_module("src/orders/export/csv.rs"), "orders");
    assert_eq!(extract_module("internal/auth/handler.go"), "auth");
    assert_eq!(extract_module("lib/services/billing.py"), "services");
}

#[test]
fn test_fix_c_scope_drift_detection_bulk_localization() {
    let dummy_root = PathBuf::from(".");

    // Contract for a task that did NOT authorize localization
    let contract = ChangeContract {
        contract_id: "ctr_test".to_string(),
        task_id: "task_test".to_string(),
        task_goal: "Add remember-me option to authentication flow".to_string(),
        version: 1,
        baseline_revision: "HEAD".to_string(),
        status: ContractStatus::Active,
        boundary: ContractBoundary {
            expected_modules: vec!["core".to_string()],
            expected_files: vec!["src/app/core/auth/auth.effects.ts".to_string()],
            targets: Vec::new(),
            expected_symbols: Vec::new(),
            expected_tests: Vec::new(),
            forbidden_boundaries: Vec::new(),
            constraints: Vec::new(),
        },
        provenance: ContractProvenance::default(),
        input_fingerprint: None,
        reconciliation_history: Vec::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    // Agent attempts to touch 5 localization files
    let changed_files = vec![
        "src/app/core/auth/auth.effects.ts".to_string(),
        "src/assets/i18n/en.json".to_string(),
        "src/assets/i18n/de.json".to_string(),
        "src/assets/i18n/fr.json".to_string(),
        "src/assets/i18n/es.json".to_string(),
        "src/assets/i18n/it.json".to_string(),
    ];

    let (classifications, _, summary) =
        validate_contract_diff_with_constitution(&dummy_root, &contract, &changed_files);

    // 1 expected file + 5 unexpected scope drift files
    assert_eq!(summary.expected_count, 1);
    assert_eq!(summary.unexpected_count, 5);

    for c in &classifications {
        if c.path.contains("i18n") {
            assert_eq!(c.category, FileValidationCategory::Unexpected);
            assert!(c.reason.contains("SCOPE_DRIFT_WARNING"), "Reason must contain SCOPE_DRIFT_WARNING: {}", c.reason);
        }
    }
}

#[test]
fn test_fix_c_scope_drift_detection_unrequested_dialog() {
    let dummy_root = PathBuf::from(".");

    let contract = ChangeContract {
        contract_id: "ctr_test".to_string(),
        task_id: "task_test".to_string(),
        task_goal: "Add remember-me option to authentication flow".to_string(),
        version: 1,
        baseline_revision: "HEAD".to_string(),
        status: ContractStatus::Active,
        boundary: ContractBoundary {
            expected_modules: vec!["core".to_string()],
            expected_files: vec!["src/app/core/auth/auth.effects.ts".to_string()],
            targets: Vec::new(),
            expected_symbols: Vec::new(),
            expected_tests: Vec::new(),
            forbidden_boundaries: Vec::new(),
            constraints: Vec::new(),
        },
        provenance: ContractProvenance::default(),
        input_fingerprint: None,
        reconciliation_history: Vec::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let changed = vec![
        "src/app/core/auth/auth.effects.ts".to_string(),
        "src/app/remember-me-dialog.component.ts".to_string(),
    ];

    let (classifications, _, summary) =
        validate_contract_diff_with_constitution(&dummy_root, &contract, &changed);

    assert_eq!(summary.expected_count, 1);
    assert_eq!(summary.unexpected_count, 1);

    let dialog_cls = classifications.iter().find(|c| c.path.contains("dialog")).unwrap();
    assert_eq!(dialog_cls.category, FileValidationCategory::Unexpected);
    assert!(dialog_cls.reason.contains("SCOPE_DRIFT_WARNING"));
}

#[test]
fn test_fix_d_instruction_block_sync_and_idempotency() {
    let tmp = std::env::temp_dir().join(format!("engram_inst_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let copilot_file = tmp.join("copilot-instructions.md");
    let rep1 = instructions::sync_managed_block(&copilot_file, DEFAULT_INSTRUCTION_BODY, false).unwrap();
    assert_eq!(rep1.action, ManagedBlockAction::Created);

    let content = fs::read_to_string(&copilot_file).unwrap();
    assert!(content.contains("Focused Context"));
    assert!(content.contains("One-Shot Preflight"));
    assert!(content.contains("Minimal Blast Radius"));

    // Syncing again must be unchanged (idempotent)
    let rep2 = instructions::sync_managed_block(&copilot_file, DEFAULT_INSTRUCTION_BODY, false).unwrap();
    assert_eq!(rep2.action, ManagedBlockAction::Unchanged);

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_iteration_1_task_aware_context_preparation() {
    let root = setup_test_workspace("iter1_prep");

    // Initialize engram index in root
    let _ = engram::analyzer::analyze(&root);

    let task = "Add remember-me option to authentication flow. Planned files: src/app/core/auth/auth.effects.ts, src/app/core/auth/auth.models.ts";
    let res = prompt::prepare_task_context(
        &root,
        task,
        1200,
        prompt::PromptFormat::Markdown,
        false,
        true,
    )
    .unwrap();

    assert!(res.contract_id.starts_with("ctr_"));
    assert_eq!(res.contract_version, 1);
    assert_eq!(res.status, "ReadyForImplementation");
    assert!(res.expected_files.contains(&"src/app/core/auth/auth.effects.ts".to_string()));
    assert!(res.content.contains("# Task"));
    assert!(res.content.contains("Engram Active Contract & Directives"));
    assert!(res.content.contains(&res.contract_id));
    assert!(res.content.contains("Do NOT call `engram_get_context` or `engram_preflight`"));

    // Verify .engram/active_task.md was written
    let active_task_file = root.join(".engram/active_task.md");
    assert!(active_task_file.exists());
    let active_content = fs::read_to_string(&active_task_file).unwrap();
    assert!(active_content.contains(&res.contract_id));

    // Verify active contract was persisted in cache and is reused on second call
    let res2 = prompt::prepare_task_context(
        &root,
        task,
        1200,
        prompt::PromptFormat::Markdown,
        false,
        false,
    )
    .unwrap();
    assert_eq!(res2.contract_id, res.contract_id, "Fingerprinted preflight must reuse contract");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_iteration_2a_deterministic_task_classifier() {
    use prompt::{classify_task_complexity, TaskComplexity};

    // 1. Trivial tasks -> TaskComplexity::Trivial
    let trivial_a = "In projects/angular-ngrx-material-starter/src/environments/environment.ts, correct the branding typo in appName from 'Angular Ngrx Material Starter' to 'Angular NgRx Material Starter' so that 'NgRx' matches the official casing. Do not change any other file or configuration.";
    let sig_a = classify_task_complexity(trivial_a);
    assert_eq!(sig_a.classification, TaskComplexity::Trivial, "Task A must be classified as Trivial: {}", sig_a.reason);
    assert!(sig_a.has_trivial_action_keywords);
    assert_eq!(sig_a.file_count, 1);

    let trivial_typo = "Fix typo in README.md: change 'teh' to 'the'";
    let sig_typo = classify_task_complexity(trivial_typo);
    assert_eq!(sig_typo.classification, TaskComplexity::Trivial, "Typo fix in README must be Trivial: {}", sig_typo.reason);

    let trivial_rename = "Rename variable in src/config.ts to match casing convention";
    let sig_rename = classify_task_complexity(trivial_rename);
    assert_eq!(sig_rename.classification, TaskComplexity::Trivial, "Casing fix must be Trivial: {}", sig_rename.reason);

    // 2. Complex multi-file tasks -> TaskComplexity::Standard
    let task_b = "Add remember-me option to authentication flow. Planned files: src/app/core/auth/auth.effects.ts, src/app/core/auth/auth.models.ts";
    let sig_b = classify_task_complexity(task_b);
    assert_eq!(sig_b.classification, TaskComplexity::Standard, "Task B must be Standard: {}", sig_b.reason);

    let multi_explicit = "Fix typo. Planned files: src/app/a.ts, src/app/b.ts";
    let sig_mult_exp = classify_task_complexity(multi_explicit);
    assert_eq!(sig_mult_exp.classification, TaskComplexity::Standard, "Multiple explicit files must be Standard: {}", sig_mult_exp.reason);
    assert_eq!(sig_mult_exp.file_count, 2);

    let multi_inline = "Fix typo in src/header.ts and src/footer.ts";
    let sig_mult_inl = classify_task_complexity(multi_inline);
    assert_eq!(sig_mult_inl.classification, TaskComplexity::Standard, "Multiple inline files must be Standard: {}", sig_mult_inl.reason);
    assert_eq!(sig_mult_inl.file_count, 2);

    // 3. Broad symbol renames and changes affecting all usages or all files -> Standard
    let broad_usages = "In src/index.ts, rename symbol and update all usages across all files";
    let sig_broad = classify_task_complexity(broad_usages);
    assert_eq!(sig_broad.classification, TaskComplexity::Standard, "Broad change with single explicit file must be Standard: {}", sig_broad.reason);
    assert!(sig_broad.has_multi_file_keywords);

    let broad_all = "Update styles across all files in src/components";
    let sig_all = classify_task_complexity(broad_all);
    assert_eq!(sig_all.classification, TaskComplexity::Standard, "Multi-file keyword must be Standard: {}", sig_all.reason);
    assert!(sig_all.has_multi_file_keywords);

    // 4. Security & Authentication variations -> Standard
    let sec_auth = "Add remember-me option to authentication flow";
    assert_eq!(classify_task_complexity(sec_auth).classification, TaskComplexity::Standard);

    let sec_path = "Fix typo in src/auth/keys.ts";
    assert_eq!(classify_task_complexity(sec_path).classification, TaskComplexity::Standard, "Security file path must force Standard");

    let sec_token = "Update authorization token in src/api.ts";
    assert_eq!(classify_task_complexity(sec_token).classification, TaskComplexity::Standard, "Auth token keyword must force Standard");

    let sec_pass = "Fix password reset session expiration in src/session.ts";
    assert_eq!(classify_task_complexity(sec_pass).classification, TaskComplexity::Standard, "Password/session must force Standard");

    let sec_cred = "Update credentials in src/config.ts";
    assert_eq!(classify_task_complexity(sec_cred).classification, TaskComplexity::Standard, "Credentials keyword must force Standard");

    let sec_xss = "Sanitize user input against XSS in src/form.ts";
    assert_eq!(classify_task_complexity(sec_xss).classification, TaskComplexity::Standard, "Sanitize/XSS must force Standard");

    let sec_negated = "Do not change auth, only fix typo in app.component.html";
    assert_eq!(classify_task_complexity(sec_negated).classification, TaskComplexity::Standard, "Negated auth keyword must safely fallback to Standard");

    // 5. Version changes affecting manifests, lockfiles, or dependencies -> Standard
    let manifest_pkg = "Update version in package.json";
    assert_eq!(classify_task_complexity(manifest_pkg).classification, TaskComplexity::Standard, "package.json modification must force Standard");

    let manifest_cargo = "Bump version in Cargo.toml and update dependencies";
    assert_eq!(classify_task_complexity(manifest_cargo).classification, TaskComplexity::Standard, "Cargo.toml and dependencies must force Standard");

    // 6. Database, schema, migration, middleware, interceptor, state-management -> Standard
    let arch_db = "Fix typo in schema migration in src/db/001.sql";
    assert_eq!(classify_task_complexity(arch_db).classification, TaskComplexity::Standard, "Database/schema/migration must force Standard");

    let arch_interceptor = "Update interceptor logging in src/interceptor.ts";
    assert_eq!(classify_task_complexity(arch_interceptor).classification, TaskComplexity::Standard, "Interceptor keyword must force Standard");

    let arch_state = "Refactor state management in src/store.ts";
    assert_eq!(classify_task_complexity(arch_state).classification, TaskComplexity::Standard, "State management must force Standard");

    // 7. Test creation, modification, or removal -> Standard
    let test_add = "Add unit test for login controller in tests/login.spec.ts";
    assert_eq!(classify_task_complexity(test_add).classification, TaskComplexity::Standard, "Test creation must force Standard");

    let test_del = "Delete obsolete test in tests/old_test.ts";
    assert_eq!(classify_task_complexity(test_del).classification, TaskComplexity::Standard, "Test deletion must force Standard");

    let test_typo = "Fix typo in tests/app.spec.ts";
    assert_eq!(classify_task_complexity(test_typo).classification, TaskComplexity::Standard, "Test file modification must force Standard");

    // 8. Scope boundaries: 0 target files, unknown tasks, malformed input -> Standard
    let zero_files = "Fix typo";
    let sig_zero = classify_task_complexity(zero_files);
    assert_eq!(sig_zero.classification, TaskComplexity::Standard, "Zero files must safely fallback to Standard: {}", sig_zero.reason);
    assert_eq!(sig_zero.file_count, 0);

    let unknown_task = "foobar bazqux 12345";
    let sig_unk = classify_task_complexity(unknown_task);
    assert_eq!(sig_unk.classification, TaskComplexity::Standard, "Unrecognized task must safely fallback to Standard: {}", sig_unk.reason);

    let malformed_planned = "Fix typo. Planned files:   ,  ; ;  ";
    let sig_mal = classify_task_complexity(malformed_planned);
    assert_eq!(sig_mal.classification, TaskComplexity::Standard, "Malformed planned-files must safely fallback to Standard: {}", sig_mal.reason);

    // 9. Conflicting signals: trivial action combined with high-risk keyword -> Standard
    let conflict = "Fix typo in environment.ts and refactor authentication";
    assert_eq!(classify_task_complexity(conflict).classification, TaskComplexity::Standard, "Conflicting trivial and high-risk signals must force Standard");
}

#[test]
fn test_iteration_2a_adaptive_bypass_execution() {
    let root = setup_test_workspace("iter2a_bypass");
    let _ = engram::analyzer::analyze(&root);

    // 1. Trivial Task (Task A typo fix) -> Bypassed
    let task_a = "In projects/angular-ngrx-material-starter/src/environments/environment.ts, correct the branding typo in appName from 'Angular Ngrx Material Starter' to 'Angular NgRx Material Starter'";
    let res_a = prompt::prepare_task_context_adaptive(
        &root,
        task_a,
        1200,
        prompt::PromptFormat::Markdown,
        false,
        false,
        false, // force_no_bypass = false
    )
    .unwrap();

    assert!(res_a.bypass_active, "Task A must be bypassed");
    assert_eq!(res_a.status, "Bypassed");
    assert_eq!(res_a.contract_id, "");
    assert_eq!(res_a.contract_version, 0);
    assert_eq!(res_a.estimated_tokens, 0);
    assert!(res_a.content.contains("Engram Bypass Notice"));
    assert!(res_a.content.contains("classified as **trivial**"));

    // 2. Forced Non-Bypass on Trivial Task (--no-bypass) -> Full Workflow
    let res_a_forced = prompt::prepare_task_context_adaptive(
        &root,
        task_a,
        1200,
        prompt::PromptFormat::Markdown,
        false,
        false,
        true, // force_no_bypass = true
    )
    .unwrap();

    assert!(!res_a_forced.bypass_active, "Forced no-bypass must run full preparation");
    assert!(res_a_forced.contract_id.starts_with("ctr_"));
    assert_eq!(res_a_forced.status, "ReadyForImplementation");
    assert!(res_a_forced.content.contains("Engram Active Contract & Directives"));

    // 3. Complex Task (Task B remember-me feature) -> Standard (Not Bypassed)
    let task_b = "Add remember-me option to authentication flow. Planned files: src/app/core/auth/auth.effects.ts, src/app/core/auth/auth.models.ts";
    let res_b = prompt::prepare_task_context_adaptive(
        &root,
        task_b,
        1200,
        prompt::PromptFormat::Markdown,
        false,
        false,
        false,
    )
    .unwrap();

    assert!(!res_b.bypass_active, "Task B must NOT be bypassed");
    assert!(res_b.contract_id.starts_with("ctr_"));
    assert_eq!(res_b.status, "ReadyForImplementation");
    assert!(res_b.expected_files.contains(&"src/app/core/auth/auth.effects.ts".to_string()));
    assert!(res_b.content.contains("Engram Active Contract & Directives"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_iteration_2b_local_postflight_compliant_and_evidence() {
    use engram::flywheel::{
        validate_local_postflight, LocalEvidenceStatus, LocalPostflightOptions,
        LocalPostflightVerdict, LocalScopeStatus, PreflightOptions, preflight_task,
    };

    let root = setup_test_workspace("iter2b_compliant");
    let _ = engram::analyzer::analyze(&root);

    let opts = PreflightOptions {
        task: "Update authentication effects".to_string(),
        token_budget: 1200,
        forbidden_boundaries: None,
        planned_files: Some(vec!["src/app/core/auth/auth.effects.ts".to_string()]),
    };
    let pre = preflight_task(&root, &opts).unwrap();

    // Modify authorized file within scope
    let target = root.join("src/app/core/auth/auth.effects.ts");
    fs::write(&target, "export class AuthEffects { login$ = true; rememberMe$ = true; }").unwrap();

    // 1. With test validation evidence -> Pass (fully verified)
    let rep_verified = validate_local_postflight(
        &root,
        &LocalPostflightOptions {
            contract_id: Some(pre.contract_id.clone()),
            staged: false,
            strict: false,
            test_evidence: Some(true),
            build_evidence: None,
        },
    )
    .unwrap();

    assert_eq!(rep_verified.verdict, LocalPostflightVerdict::Pass);
    assert_eq!(rep_verified.scope_status, LocalScopeStatus::Compliant);
    assert_eq!(rep_verified.evidence_status, LocalEvidenceStatus::VerifiedByTests);
    assert_eq!(rep_verified.exit_code, 0);

    // 2. Without test validation evidence -> PassScopeOnlyUnverified (honest distinction)
    let rep_unverified = validate_local_postflight(
        &root,
        &LocalPostflightOptions {
            contract_id: Some(pre.contract_id.clone()),
            staged: false,
            strict: false,
            test_evidence: None,
            build_evidence: None,
        },
    )
    .unwrap();

    assert_eq!(rep_unverified.verdict, LocalPostflightVerdict::PassScopeOnlyUnverified);
    assert_eq!(rep_unverified.scope_status, LocalScopeStatus::Compliant);
    assert_eq!(rep_unverified.evidence_status, LocalEvidenceStatus::MissingValidationEvidence);
    assert!(rep_unverified.summary.contains("PASS (SCOPE ONLY)"));
    assert_eq!(rep_unverified.exit_code, 0);

    // 3. With explicit test failure -> BlockedValidationFailed
    let rep_failed = validate_local_postflight(
        &root,
        &LocalPostflightOptions {
            contract_id: Some(pre.contract_id.clone()),
            staged: false,
            strict: false,
            test_evidence: Some(false),
            build_evidence: None,
        },
    )
    .unwrap();

    assert_eq!(rep_failed.verdict, LocalPostflightVerdict::BlockedValidationFailed);
    assert_eq!(rep_failed.evidence_status, LocalEvidenceStatus::ValidationFailed);
    assert_eq!(rep_failed.exit_code, 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_iteration_2b_local_postflight_boundary_violation() {
    use engram::flywheel::{
        validate_local_postflight, LocalPostflightOptions, LocalPostflightVerdict,
        LocalScopeStatus, PreflightOptions, preflight_task,
    };

    let root = setup_test_workspace("iter2b_boundary");
    let _ = engram::analyzer::analyze(&root);

    let opts = PreflightOptions {
        task: "Update authentication effects".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["src/assets/i18n/de.json".to_string()]),
        planned_files: Some(vec!["src/app/core/auth/auth.effects.ts".to_string()]),
    };
    let pre = preflight_task(&root, &opts).unwrap();

    // Modify a forbidden boundary file
    let forbidden = root.join("src/assets/i18n/de.json");
    fs::write(&forbidden, r#"{"auth.login": "Verboten"}"#).unwrap();

    let rep = validate_local_postflight(
        &root,
        &LocalPostflightOptions {
            contract_id: Some(pre.contract_id),
            staged: false,
            strict: false,
            test_evidence: Some(true),
            build_evidence: None,
        },
    )
    .unwrap();

    assert_eq!(rep.verdict, LocalPostflightVerdict::BlockedScopeViolation);
    assert_eq!(rep.scope_status, LocalScopeStatus::BoundaryViolation);
    assert!(rep.boundary_violations.contains(&"src/assets/i18n/de.json".to_string()));
    assert_eq!(rep.exit_code, 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_iteration_2b_local_postflight_stale_contract() {
    use engram::flywheel::{
        validate_local_postflight, LocalPostflightOptions, LocalPostflightVerdict,
        LocalStalenessStatus, PreflightOptions, preflight_task,
    };

    let root = setup_test_workspace("iter2b_stale");
    let _ = engram::analyzer::analyze(&root);

    let opts = PreflightOptions {
        task: "Update authentication effects".to_string(),
        token_budget: 1200,
        forbidden_boundaries: None,
        planned_files: Some(vec!["src/app/core/auth/auth.effects.ts".to_string()]),
    };
    let pre = preflight_task(&root, &opts).unwrap();

    // Commit an intervening mutation to an expected file, advancing HEAD past the contract baseline
    let target = root.join("src/app/core/auth/auth.effects.ts");
    fs::write(&target, "// intervening commit by another actor").unwrap();
    Command::new("git").args(["add", "."]).current_dir(&root).output().unwrap();
    Command::new("git").args(["commit", "-m", "Intervening commit"]).current_dir(&root).output().unwrap();

    // Now validate against the older contract baseline
    let rep = validate_local_postflight(
        &root,
        &LocalPostflightOptions {
            contract_id: Some(pre.contract_id),
            staged: false,
            strict: false,
            test_evidence: Some(true),
            build_evidence: None,
        },
    )
    .unwrap();

    assert_eq!(rep.staleness_status, LocalStalenessStatus::StaleBaselineDiverged);
    assert_eq!(rep.verdict, LocalPostflightVerdict::BlockedStaleContract);
    assert_eq!(rep.exit_code, 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_iteration_2b_local_postflight_git_unavailable() {
    use engram::contract::{ChangeContract, ContractBoundary, ContractProvenance, ContractStatus, RatificationStatus};
    use engram::flywheel::{
        validate_local_postflight, LocalPostflightOptions, LocalPostflightVerdict,
        LocalStalenessStatus,
    };

    let tmp = std::env::temp_dir().join(format!("engram_iter2b_nogit_{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    // Create contract in non-git directory
    let contract_id = "ctr_test_nogit".to_string();
    let contract = ChangeContract {
        contract_id: contract_id.clone(),
        task_id: "task_test_nogit".to_string(),
        task_goal: "Test non-git validation".to_string(),
        version: 1,
        baseline_revision: "abc1234".to_string(),
        status: ContractStatus::Active,
        boundary: ContractBoundary {
            expected_modules: vec![],
            expected_files: vec!["file.txt".to_string()],
            targets: vec![],
            expected_symbols: vec![],
            expected_tests: vec![],
            forbidden_boundaries: vec![],
            constraints: vec![],
        },
        provenance: ContractProvenance {
            origin: "test".to_string(),
            baseline_commit: "abc1234".to_string(),
            created_at: chrono::Utc::now(),
            ratification_status: RatificationStatus::Unratified,
            ratified_by: None,
            ratification_reason: None,
        },
        input_fingerprint: None,
        reconciliation_history: vec![],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    engram::contract::save_contract(&tmp, &contract).unwrap();

    let rep = validate_local_postflight(
        &tmp,
        &LocalPostflightOptions {
            contract_id: Some(contract_id),
            staged: false,
            strict: false,
            test_evidence: None,
            build_evidence: None,
        },
    )
    .unwrap();

    assert_eq!(rep.verdict, LocalPostflightVerdict::IndeterminateGitUnavailable);
    assert_eq!(rep.staleness_status, LocalStalenessStatus::GitUnavailable);
    assert_eq!(rep.exit_code, 2);

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_iteration_2b_local_postflight_ambiguous_results() {
    use engram::flywheel::{
        validate_local_postflight, LocalPostflightOptions, LocalPostflightVerdict,
        LocalScopeStatus, PreflightOptions, preflight_task,
    };

    let root = setup_test_workspace("iter2b_ambig");
    let _ = engram::analyzer::analyze(&root);

    let opts = PreflightOptions {
        task: "Update authentication effects".to_string(),
        token_budget: 1200,
        forbidden_boundaries: None,
        planned_files: Some(vec!["src/app/core/auth/auth.effects.ts".to_string()]),
    };
    let pre = preflight_task(&root, &opts).unwrap();

    // Modify an unexpected file outside planned files (settings component)
    let unexpected = root.join("src/app/features/settings/settings.component.ts");
    fs::write(&unexpected, "// unexpected modification").unwrap();

    // 1. In non-strict mode: flagged as UnexpectedExpansion, advisory pass scope
    let rep_advisory = validate_local_postflight(
        &root,
        &LocalPostflightOptions {
            contract_id: Some(pre.contract_id.clone()),
            staged: false,
            strict: false,
            test_evidence: None,
            build_evidence: None,
        },
    )
    .unwrap();

    assert_eq!(rep_advisory.scope_status, LocalScopeStatus::UnexpectedExpansion);
    assert!(rep_advisory.unexpected_files.contains(&"src/app/features/settings/settings.component.ts".to_string()));
    assert_eq!(rep_advisory.verdict, LocalPostflightVerdict::PassScopeOnlyUnverified);

    // 2. In strict mode: BlockedScopeViolation
    let rep_strict = validate_local_postflight(
        &root,
        &LocalPostflightOptions {
            contract_id: Some(pre.contract_id),
            staged: false,
            strict: true,
            test_evidence: None,
            build_evidence: None,
        },
    )
    .unwrap();

    assert_eq!(rep_strict.verdict, LocalPostflightVerdict::BlockedScopeViolation);
    assert_eq!(rep_strict.exit_code, 1);

    let _ = fs::remove_dir_all(&root);
}
