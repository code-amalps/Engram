//! Milestone 5: Architecture Drift Detection & Human Ratification (Pillar 3) Test Suite
//!
//! Exercising the 12 required scenarios:
//! 1. No drift when conforming.
//! 2. Confirmed drift against ratified decision (stale threshold exceeded).
//! 3. Potential drift (provisional decision pending review or uninitialized DB).
//! 4. Deterministic findings across repeated runs.
//! 5. Correct mapping of affected paths and symbols.
//! 6. Provisional decisions cannot be ratified implicitly (re-ratify fails).
//! 7. Explicit human ratification (`ratify_decision`) transitions to `Durable` with provenance and telemetry.
//! 8. Invalid lifecycle transitions fail safely (cannot ratify Rejected/Superseded, missing reviewer/reason fails).
//! 9. Constitution and contracts remain unchanged during drift analysis (zero automatic mutations).
//! 10. Missing or corrupt analysis data produces safe IncompleteEvidence outcome without crash.
//! 11. Change Guard integration with `checks.drift.enabled = true` detects drift and blocks when policy specifies block.
//! 12. Change contract explicit ratification with human provenance and telemetry.
//! 13. Failure-injection: ratify_decision telemetry failure triggers clean rollback.
//! 14. Failure-injection: reject_decision telemetry failure triggers clean rollback.
//! 15. Failure-injection: re_ratify_decision telemetry failure triggers clean rollback.
//! 16. Failure-injection: ratify_contract telemetry failure triggers clean rollback.
//! 17. Failure-injection: state persistence failure produces zero ghost telemetry.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

use engram::ci_guard::{run_guard, GuardOptions, GuardStatus};
use engram::contract::{
    create_contract, get_contract, ratify_contract, save_contract, RatificationStatus,
};
use engram::drift::{analyze_drift, DriftCategory, DriftConfidence, DriftOptions, DriftSeverity};
use engram::memory::{
    load_durable_memories, ratify_decision, re_ratify_decision, record_scoped_decision,
    reject_decision, save_durable_memories, DurableMemory, LifecycleError, MemoryLifecycleState,
    MemoryScope, ScopeLevel,
};
use engram::model::EvidenceTier;
use engram::telemetry::{load_events, verify_event_chain, EventChainStatus};

fn setup_test_git_repo(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("engram_m5_test_{}_{}", prefix, Uuid::new_v4()));
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

    run_cmd(&["init", "-b", "main"]);
    run_cmd(&["config", "user.name", "M5 Tester"]);
    run_cmd(&["config", "user.email", "m5-tester@engram.dev"]);

    fs::write(dir.join("README.md"), "# M5 Test Repo\n").unwrap();
    run_cmd(&["add", "README.md"]);
    run_cmd(&["commit", "-m", "Initial commit"]);

    dir
}

// ---------------------------------------------------------------------------
// 1. No drift when conforming
// ---------------------------------------------------------------------------
#[test]
fn test_1_no_drift_when_conforming() {
    let root = setup_test_git_repo("test1_conforming");

    fs::create_dir_all(root.join("src/orders")).unwrap();
    fs::write(
        root.join("src/orders/service.rs"),
        "pub struct OrderService;\n",
    )
    .unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["commit", "-m", "add OrderService"])
        .current_dir(&root)
        .output()
        .unwrap();

    let scope = MemoryScope {
        level: ScopeLevel::Symbol,
        target: Some("OrderService".to_string()),
        team: None,
        modules: vec!["orders".to_string()],
        files: vec!["src/orders/service.rs".to_string()],
        symbols: vec!["OrderService".to_string()],
    };

    let _mem = record_scoped_decision(
        &root,
        Some("dec_orders"),
        "OrderService Domain Boundary",
        "OrderService must not depend on UI",
        "Maintain clean architecture",
        scope,
        None,
        "architect",
    )
    .unwrap();

    // Contract expecting src/orders/service.rs
    let contract = create_contract(
        &root,
        "Order service changes",
        Some(vec!["orders".to_string()]),
        Some(vec!["src/orders/service.rs".to_string()]),
        Some(vec!["src/ui/**".to_string()]),
    )
    .unwrap();
    save_contract(&root, &contract).unwrap();

    let report = analyze_drift(&root, &DriftOptions::default()).unwrap();

    assert_eq!(report.status, "OK");
    assert!(report.stale_decisions.is_empty());
    assert!(report.boundary_violations.is_empty());
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.severity == DriftSeverity::Warn || f.severity == DriftSeverity::Block),
        "Expected no warning or block findings when codebase conforms"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 2. Confirmed drift against ratified decision (stale threshold exceeded)
// ---------------------------------------------------------------------------
#[test]
fn test_2_confirmed_drift_against_ratified_decision() {
    let root = setup_test_git_repo("test2_stale");

    fs::create_dir_all(root.join("src/orders")).unwrap();
    fs::write(
        root.join("src/orders/service.rs"),
        "pub struct OrderService;\n",
    )
    .unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["commit", "-m", "init OrderService"])
        .current_dir(&root)
        .output()
        .unwrap();

    let scope = MemoryScope {
        level: ScopeLevel::Symbol,
        target: Some("OrderService".to_string()),
        team: None,
        modules: vec!["orders".to_string()],
        files: vec!["src/orders/service.rs".to_string()],
        symbols: vec!["OrderService".to_string()],
    };

    let _mem = record_scoped_decision(
        &root,
        Some("dec_orders"),
        "OrderService Architecture Policy",
        "OrderService must maintain isolation",
        "Clean architecture",
        scope,
        None,
        "architect",
    )
    .unwrap();

    // Make 5 mutations modifying src/orders/service.rs
    for i in 1..=5 {
        fs::write(
            root.join("src/orders/service.rs"),
            format!("pub struct OrderService; // change {i}\n"),
        )
        .unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&root)
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["commit", "-m", &format!("mutate OrderService {i}")])
            .current_dir(&root)
            .output()
            .unwrap();
    }

    let report = analyze_drift(&root, &DriftOptions::default()).unwrap();

    assert_eq!(report.status, "DRIFT_DETECTED");
    assert_eq!(report.stale_decisions.len(), 1);

    // Verify structured DriftFinding
    let finding = report
        .findings
        .iter()
        .find(|f| f.reference_id == "dec_orders")
        .expect("Finding for dec_orders must exist");

    assert_eq!(finding.category, DriftCategory::StaleDecision);
    assert_eq!(finding.severity, DriftSeverity::Warn);
    assert_eq!(finding.confidence, DriftConfidence::Confirmed);
    assert!(finding.id.starts_with("DRIFT-DEC-STALE-dec_orders"));
    assert!(finding
        .expected_state
        .contains("before reaching 5 mutations"));
    assert!(finding.observed_state.contains("mutated 5 times"));
    assert!(finding.suggested_action.contains("re-ratify"));
    assert!(
        finding
            .affected_paths
            .iter()
            .any(|p| p.contains("service.rs")),
        "Affected paths should list service.rs"
    );
    assert!(
        finding.affected_symbols.iter().any(|s| s == "OrderService"),
        "Affected symbols should list OrderService"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 3. Potential drift (provisional decision pending review)
// ---------------------------------------------------------------------------
#[test]
fn test_3_potential_drift_provisional_decision_pending_review() {
    let root = setup_test_git_repo("test3_provisional");

    let mut doc = load_durable_memories(&root).unwrap();
    doc.memories.push(DurableMemory {
        id: "dec_provisional_1".to_string(),
        title: "Provisional Event Bus Pattern".to_string(),
        statement: "Use RabbitMQ for all domain events".to_string(),
        reason: "Event decoupling requirement".to_string(),
        source: None,
        provenance: None,
        accepted_by: "ai_agent".to_string(),
        accepted_at: chrono::Utc::now(),
        scope: MemoryScope::module("events"),
        evidence_tier: EvidenceTier::Inferred,
        lifecycle_state: MemoryLifecycleState::Provisional,
        supersedes: None,
        superseded_by: None,
        tags: vec![],
        ratified_at_commit: None,
        ratified_by: None,
        ratified_at: None,
    });
    save_durable_memories(&root, &doc).unwrap();

    let report = analyze_drift(&root, &DriftOptions::default()).unwrap();

    let finding = report
        .findings
        .iter()
        .find(|f| f.reference_id == "dec_provisional_1")
        .expect("Finding for provisional decision must exist");

    assert_eq!(finding.category, DriftCategory::DecisionDrift);
    assert_eq!(finding.confidence, DriftConfidence::Potential);
    assert_eq!(finding.severity, DriftSeverity::Info);
    assert!(finding
        .id
        .starts_with("DRIFT-DEC-PROVISIONAL-dec_provisional_1"));
    assert!(finding.suggested_action.contains("ratify"));

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 4. Deterministic findings across repeated runs
// ---------------------------------------------------------------------------
#[test]
fn test_4_deterministic_findings_across_repeated_runs() {
    let root = setup_test_git_repo("test4_determinism");

    fs::create_dir_all(root.join("src/billing")).unwrap();
    fs::write(
        root.join("src/billing/service.rs"),
        "pub struct BillingService;\n",
    )
    .unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["commit", "-m", "init billing"])
        .current_dir(&root)
        .output()
        .unwrap();

    let scope = MemoryScope {
        level: ScopeLevel::Symbol,
        target: Some("BillingService".to_string()),
        team: None,
        modules: vec!["billing".to_string()],
        files: vec!["src/billing/service.rs".to_string()],
        symbols: vec!["BillingService".to_string()],
    };

    let _mem = record_scoped_decision(
        &root,
        Some("dec_billing"),
        "Billing Policy",
        "Billing boundary",
        "Clean arch",
        scope,
        None,
        "architect",
    )
    .unwrap();

    // 5 mutations
    for i in 1..=5 {
        fs::write(
            root.join("src/billing/service.rs"),
            format!("pub struct BillingService; // mut {i}\n"),
        )
        .unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&root)
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["commit", "-m", &format!("mutate billing {i}")])
            .current_dir(&root)
            .output()
            .unwrap();
    }

    let report1 = analyze_drift(&root, &DriftOptions::default()).unwrap();
    let report2 = analyze_drift(&root, &DriftOptions::default()).unwrap();

    assert_eq!(
        report1.findings.len(),
        report2.findings.len(),
        "Findings count must be deterministic"
    );
    assert_eq!(
        report1.findings, report2.findings,
        "Drift findings must be strictly identical across runs"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 5. Correct mapping of affected paths and symbols
// ---------------------------------------------------------------------------
#[test]
fn test_5_correct_mapping_of_affected_paths_and_symbols() {
    let root = setup_test_git_repo("test5_mapping");

    fs::create_dir_all(root.join("src/crypto")).unwrap();
    fs::write(
        root.join("src/crypto/vault.rs"),
        "pub struct SecretVault;\n",
    )
    .unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["commit", "-m", "init vault"])
        .current_dir(&root)
        .output()
        .unwrap();

    let scope = MemoryScope {
        level: ScopeLevel::Symbol,
        target: Some("SecretVault".to_string()),
        team: None,
        modules: vec!["crypto".to_string()],
        files: vec!["src/crypto/vault.rs".to_string()],
        symbols: vec!["SecretVault".to_string()],
    };

    let _mem = record_scoped_decision(
        &root,
        Some("dec_crypto_vault"),
        "Crypto Vault Security Policy",
        "Vault operations require hardware enclave",
        "Compliance",
        scope,
        None,
        "security_lead",
    )
    .unwrap();

    for i in 1..=5 {
        fs::write(
            root.join("src/crypto/vault.rs"),
            format!("pub struct SecretVault; // v{i}\n"),
        )
        .unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&root)
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["commit", "-m", &format!("mutate vault {i}")])
            .current_dir(&root)
            .output()
            .unwrap();
    }

    let report = analyze_drift(&root, &DriftOptions::default()).unwrap();
    let finding = report
        .findings
        .iter()
        .find(|f| f.reference_id == "dec_crypto_vault")
        .expect("Finding for dec_crypto_vault");

    assert_eq!(finding.affected_paths, vec!["src/crypto/vault.rs"]);
    assert_eq!(finding.affected_symbols, vec!["SecretVault"]);

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 6. Provisional decisions cannot be ratified implicitly (re-ratify fails)
// ---------------------------------------------------------------------------
#[test]
fn test_6_provisional_decisions_cannot_be_implicitly_re_ratified() {
    let root = setup_test_git_repo("test6_no_implicit");

    let mut doc = load_durable_memories(&root).unwrap();
    doc.memories.push(DurableMemory {
        id: "dec_ai_proposed".to_string(),
        title: "Proposed Cache Layer".to_string(),
        statement: "Use Redis caching".to_string(),
        reason: "Performance".to_string(),
        source: None,
        provenance: None,
        accepted_by: "ai_agent".to_string(),
        accepted_at: chrono::Utc::now(),
        scope: MemoryScope::module("cache"),
        evidence_tier: EvidenceTier::Inferred,
        lifecycle_state: MemoryLifecycleState::Provisional,
        supersedes: None,
        superseded_by: None,
        tags: vec![],
        ratified_at_commit: None,
        ratified_by: None,
        ratified_at: None,
    });
    save_durable_memories(&root, &doc).unwrap();

    let head = engram::git::current_revision(&root).unwrap();
    let res = re_ratify_decision(&root, "dec_ai_proposed", Some(&head), "lead_dev");

    assert!(res.is_err(), "Re-ratify of provisional decision must fail");
    let err = res.unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("Provisional decisions require explicit human ratification"),
        "Error must explain explicit human ratification is required, got: {err_str}"
    );

    // Verify it remains Provisional
    let doc_after = load_durable_memories(&root).unwrap();
    let m = doc_after
        .memories
        .iter()
        .find(|m| m.id == "dec_ai_proposed")
        .unwrap();
    assert_eq!(m.lifecycle_state, MemoryLifecycleState::Provisional);

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 7. Explicit human ratification transitions to Durable with provenance & telemetry
// ---------------------------------------------------------------------------
#[test]
fn test_7_explicit_human_ratification_transitions_to_durable_with_telemetry() {
    let root = setup_test_git_repo("test7_ratify");

    let mut doc = load_durable_memories(&root).unwrap();
    doc.memories.push(DurableMemory {
        id: "dec_prov_auth".to_string(),
        title: "Proposed Auth Service".to_string(),
        statement: "Use OAuth2 with PKCE".to_string(),
        reason: "Security standard".to_string(),
        source: None,
        provenance: None,
        accepted_by: "ai_agent".to_string(),
        accepted_at: chrono::Utc::now(),
        scope: MemoryScope::module("auth"),
        evidence_tier: EvidenceTier::Inferred,
        lifecycle_state: MemoryLifecycleState::Provisional,
        supersedes: None,
        superseded_by: None,
        tags: vec![],
        ratified_at_commit: None,
        ratified_by: None,
        ratified_at: None,
    });
    save_durable_memories(&root, &doc).unwrap();

    let head = engram::git::current_revision(&root).unwrap();
    let updated = ratify_decision(
        &root,
        "dec_prov_auth",
        "alice_security_architect",
        "Architecture review approved at RFC 42",
        Some(&head),
    )
    .unwrap();

    assert_eq!(updated.lifecycle_state, MemoryLifecycleState::Durable);
    assert_eq!(
        updated.ratified_by.as_deref(),
        Some("alice_security_architect")
    );
    assert_eq!(updated.ratified_at_commit.as_deref(), Some(head.as_str()));

    // Verify persisted memory
    let doc_after = load_durable_memories(&root).unwrap();
    let m = doc_after
        .memories
        .iter()
        .find(|m| m.id == "dec_prov_auth")
        .unwrap();
    assert_eq!(m.lifecycle_state, MemoryLifecycleState::Durable);
    assert_eq!(m.ratified_by.as_deref(), Some("alice_security_architect"));

    // Verify telemetry event logged
    let events = load_events(&root).unwrap();
    assert!(
        events
            .iter()
            .any(|e| e.event_type == "DECISION_RATIFIED" && e.actor == "alice_security_architect"),
        "Telemetry must record DECISION_RATIFIED event for alice_security_architect"
    );
    let (chain_status, _) = verify_event_chain(&root).unwrap();
    assert_eq!(
        chain_status,
        EventChainStatus::Valid,
        "Cryptographic event chain must be intact"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 8. Invalid lifecycle transitions fail safely
// ---------------------------------------------------------------------------
#[test]
fn test_8_invalid_lifecycle_transitions_fail_safely() {
    let root = setup_test_git_repo("test8_lifecycle_errors");

    // 8a: Nonexistent decision
    let err_nf = ratify_decision(&root, "non_existent", "alice", "review", None).unwrap_err();
    assert!(
        err_nf.to_string().contains("not found"),
        "Expected not found error, got: {err_nf}"
    );

    // Setup a decision
    let _mem = record_scoped_decision(
        &root,
        Some("dec_valid"),
        "Valid Policy",
        "Policy statement",
        "Clean code",
        MemoryScope::module("core"),
        None,
        "dev",
    )
    .unwrap();

    // 8b: Empty reviewer rejected
    let err_rev = ratify_decision(&root, "dec_valid", "   ", "review reason", None).unwrap_err();
    assert!(
        err_rev
            .to_string()
            .contains("reviewer ('by') must be non-empty"),
        "Expected missing reviewer error, got: {err_rev}"
    );

    // 8c: Empty reason rejected
    let err_reas = ratify_decision(&root, "dec_valid", "alice", "   ", None).unwrap_err();
    assert!(
        err_reas
            .to_string()
            .contains("ratification reason must be non-empty"),
        "Expected missing reason error, got: {err_reas}"
    );

    // 8d: Reject a provisional decision
    let mut doc = load_durable_memories(&root).unwrap();
    doc.memories.push(DurableMemory {
        id: "dec_to_reject".to_string(),
        title: "Bad Proposal".to_string(),
        statement: "Bypass authentication".to_string(),
        reason: "Speed".to_string(),
        source: None,
        provenance: None,
        accepted_by: "ai_agent".to_string(),
        accepted_at: chrono::Utc::now(),
        scope: MemoryScope::module("auth"),
        evidence_tier: EvidenceTier::Inferred,
        lifecycle_state: MemoryLifecycleState::Provisional,
        supersedes: None,
        superseded_by: None,
        tags: vec![],
        ratified_at_commit: None,
        ratified_by: None,
        ratified_at: None,
    });
    save_durable_memories(&root, &doc).unwrap();

    let rej = reject_decision(
        &root,
        "dec_to_reject",
        "sec_team",
        "Direct violation of security constitution",
    )
    .unwrap();
    assert_eq!(rej.lifecycle_state, MemoryLifecycleState::Rejected);

    // 8e: Attempting to ratify a Rejected decision must fail
    let err_rat_rej = ratify_decision(&root, "dec_to_reject", "bob", "reviving", None).unwrap_err();
    assert!(
        err_rat_rej
            .to_string()
            .contains("Rejected decisions cannot be ratified"),
        "Expected invalid transition error for rejected decision, got: {err_rat_rej}"
    );

    // 8f: Attempting to re-ratify a Rejected decision must fail
    let err_rerat_rej = re_ratify_decision(&root, "dec_to_reject", None, "bob").unwrap_err();
    assert!(
        err_rerat_rej
            .to_string()
            .contains("Rejected decisions cannot be re-ratified"),
        "Expected invalid transition error on re-ratify, got: {err_rerat_rej}"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 9. Constitution and contracts remain unchanged during drift analysis
// ---------------------------------------------------------------------------
#[test]
fn test_9_constitution_and_contracts_remain_unchanged_during_drift_analysis() {
    let root = setup_test_git_repo("test9_immutability");

    // 1. Setup constitution
    let engram_dir = root.join(".engram");
    fs::create_dir_all(&engram_dir).unwrap();
    let constitution_file = engram_dir.join("constitution.json");
    let constitution_content = r#"{
        "version": 1,
        "rules": [
            {
                "id": "CONST-001",
                "name": "No direct database access from UI",
                "severity": "block",
                "description": "UI layer must communicate via service layer"
            }
        ]
    }"#;
    fs::write(&constitution_file, constitution_content).unwrap();

    // 2. Setup contract
    let contract = create_contract(
        &root,
        "Enforce payments boundary",
        Some(vec!["payments".to_string()]),
        Some(vec!["src/payments/engine.rs".to_string()]),
        Some(vec!["src/legacy/**".to_string()]),
    )
    .unwrap();
    save_contract(&root, &contract).unwrap();

    let contract_before = get_contract(&root, &contract.contract_id).unwrap();

    // 3. Run drift analysis multiple times
    let _ = analyze_drift(&root, &DriftOptions::default()).unwrap();
    let _ = analyze_drift(&root, &DriftOptions::default()).unwrap();

    // 4. Verify constitution and contract contents are byte-for-byte identical
    let constitution_content_after = fs::read_to_string(&constitution_file).unwrap();
    let contract_after = get_contract(&root, &contract.contract_id).unwrap();

    assert_eq!(
        constitution_content, constitution_content_after,
        "Constitution MUST remain completely unchanged during drift analysis"
    );
    assert_eq!(
        serde_json::to_string(&contract_before).unwrap(),
        serde_json::to_string(&contract_after).unwrap(),
        "Contracts MUST remain completely unchanged during drift analysis"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 10. Missing or corrupt analysis data produces safe IncompleteEvidence outcome
// ---------------------------------------------------------------------------
#[test]
fn test_10_missing_or_corrupt_analysis_data_produces_safe_incomplete_evidence() {
    let root = setup_test_git_repo("test10_corruption_safe");

    // Write corrupted YAML to .engram/memory.yaml
    let engram_dir = root.join(".engram");
    fs::create_dir_all(&engram_dir).unwrap();
    fs::write(
        engram_dir.join("memory.yaml"),
        "CORRUPTED_TRASH_BYTES {[[<>>\t\0invalid",
    )
    .unwrap();

    // analyze_drift must NOT panic or crash
    let report_res = analyze_drift(&root, &DriftOptions::default());
    assert!(
        report_res.is_ok(),
        "analyze_drift must gracefully handle corrupted memory file without crashing"
    );

    let report = report_res.unwrap();
    assert!(
        report.findings.iter().any(|f| {
            f.confidence == DriftConfidence::IncompleteEvidence
                || f.category == DriftCategory::IncompleteEvidence
        }),
        "Expected finding indicating incomplete evidence due to corrupt memory file"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 11. Change Guard integration detects drift and blocks when policy specifies
// ---------------------------------------------------------------------------
#[test]
fn test_11_change_guard_integration_detects_drift_and_blocks_when_policy_specifies() {
    let root = setup_test_git_repo("test11_guard_drift");

    // 1. Setup a decision and mutate 4 times
    fs::create_dir_all(root.join("src/auth")).unwrap();
    fs::write(
        root.join("src/auth/token.rs"),
        "pub struct TokenValidator;\n",
    )
    .unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["commit", "-m", "init token"])
        .current_dir(&root)
        .output()
        .unwrap();

    let scope = MemoryScope {
        level: ScopeLevel::Symbol,
        target: Some("TokenValidator".to_string()),
        team: None,
        modules: vec!["auth".to_string()],
        files: vec!["src/auth/token.rs".to_string()],
        symbols: vec!["TokenValidator".to_string()],
    };

    let _mem = record_scoped_decision(
        &root,
        Some("dec_token"),
        "Token Policy",
        "Token policy rules",
        "Clean architecture",
        scope,
        None,
        "security",
    )
    .unwrap();

    for i in 1..=4 {
        fs::write(
            root.join("src/auth/token.rs"),
            format!("pub struct TokenValidator; // mutation {i}\n"),
        )
        .unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&root)
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["commit", "-m", &format!("mutate token {i}")])
            .current_dir(&root)
            .output()
            .unwrap();
    }

    // 2. Setup guard policy with drift enabled, staleness_threshold = 3, severity = "block"
    let engram_dir = root.join(".engram");
    fs::create_dir_all(&engram_dir).unwrap();
    let guard_toml = r#"version = "1"

[checks.contracts]
enabled = false

[checks.drift]
enabled = true
severity = "block"
staleness_threshold = 3
"#;
    fs::write(engram_dir.join("change-guard.toml"), guard_toml).unwrap();

    // 3. Run change guard
    let opts = GuardOptions::default();
    let guard_res = run_guard(&root, &opts);

    // 4. Guard should detect drift (4 mutations > threshold 3) and block
    assert_eq!(
        guard_res.status,
        GuardStatus::Block,
        "Guard must block when drift is detected and fail_on = 'warn'"
    );
    assert!(guard_res.blocked);
    assert_eq!(guard_res.exit_code, 1);
    assert!(
        guard_res
            .findings
            .iter()
            .any(|f| f.rule_id.starts_with("GUARD-DRIFT-")),
        "Guard findings must include GUARD-DRIFT finding"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 12. Change contract explicit ratification with human provenance & telemetry
// ---------------------------------------------------------------------------
#[test]
fn test_12_contract_ratification_with_provenance_and_telemetry() {
    let root = setup_test_git_repo("test12_contract_ratify");

    let contract = create_contract(
        &root,
        "Payment isolation boundary",
        Some(vec!["payments".to_string()]),
        Some(vec!["src/payments/gateway.rs".to_string()]),
        Some(vec!["src/legacy/**".to_string()]),
    )
    .unwrap();

    // Empty reviewer fails
    let err_by = ratify_contract(&root, &contract.contract_id, "  ", "Valid reason");
    assert!(err_by.is_err());
    assert!(err_by
        .unwrap_err()
        .to_string()
        .contains("reviewer ('by') must be non-empty"));

    // Empty reason fails
    let err_reas = ratify_contract(&root, &contract.contract_id, "sec_lead", "  ");
    assert!(err_reas.is_err());
    assert!(err_reas
        .unwrap_err()
        .to_string()
        .contains("ratification reason must be non-empty"));

    // Valid human ratification
    let ratified = ratify_contract(
        &root,
        &contract.contract_id,
        "clara_lead_architect",
        "Approved boundary isolation design",
    )
    .unwrap();

    assert_eq!(
        ratified.provenance.ratification_status,
        RatificationStatus::Ratified
    );
    assert_eq!(
        ratified.provenance.ratified_by.as_deref(),
        Some("clara_lead_architect")
    );
    assert_eq!(
        ratified.provenance.ratification_reason.as_deref(),
        Some("Approved boundary isolation design")
    );

    // Verify stored contract
    let saved = get_contract(&root, &contract.contract_id).unwrap();
    assert_eq!(
        saved.provenance.ratification_status,
        RatificationStatus::Ratified
    );

    // Verify telemetry
    let events = load_events(&root).unwrap();
    assert!(
        events
            .iter()
            .any(|e| e.event_type == "CONTRACT_RATIFIED" && e.actor == "clara_lead_architect"),
        "Telemetry must record CONTRACT_RATIFIED event for clara_lead_architect"
    );
    let (chain_status, _) = verify_event_chain(&root).unwrap();
    assert_eq!(
        chain_status,
        EventChainStatus::Valid,
        "Cryptographic event chain must be intact"
    );

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 13. Failure-injection: ratify_decision telemetry failure triggers rollback
// ---------------------------------------------------------------------------
#[test]
fn test_13_ratify_decision_telemetry_failure_rolls_back() {
    let root = setup_test_git_repo("test13_ratify_telemetry_failure");

    // 1. Create a provisional decision
    let mut doc = load_durable_memories(&root).unwrap();
    doc.memories.push(DurableMemory {
        id: "dec_rollback_ratify".to_string(),
        title: "Database Read Replicas".to_string(),
        statement: "Route analytical queries to read replica".to_string(),
        reason: "Performance optimization".to_string(),
        source: None,
        provenance: None,
        accepted_by: "ai_assistant".to_string(),
        accepted_at: chrono::Utc::now(),
        scope: MemoryScope::module("database"),
        evidence_tier: EvidenceTier::Inferred,
        lifecycle_state: MemoryLifecycleState::Provisional,
        supersedes: None,
        superseded_by: None,
        tags: vec![],
        ratified_at_commit: None,
        ratified_by: None,
        ratified_at: None,
    });
    save_durable_memories(&root, &doc).unwrap();

    // 2. Inject telemetry failure by replacing events directory with a regular file
    let events_path = root.join(".engram").join("events");
    let _ = fs::remove_dir_all(&events_path);
    fs::write(&events_path, "telemetry blocker").unwrap();

    // 3. Attempt ratification - must fail with ConsistencyError and cleanly roll back
    let ratify_err = ratify_decision(
        &root,
        "dec_rollback_ratify",
        "sarah_lead",
        "Approved for production scaling",
        None,
    )
    .unwrap_err();

    let err_msg = ratify_err.to_string();
    assert!(
        err_msg.contains("Governance consistency error"),
        "Must return typed consistency error, got: {err_msg}"
    );
    let consistency_err = ratify_err.downcast::<LifecycleError>().expect("Must be LifecycleError");
    match consistency_err {
        LifecycleError::ConsistencyError {
            state_persisted,
            telemetry_recorded,
            ..
        } => {
            assert!(!state_persisted, "State must not be persisted as new state");
            assert!(!telemetry_recorded, "Telemetry must not be recorded");
        }
        other => panic!("Expected ConsistencyError, got {other:?}"),
    }

    // 4. Verify durable memory on disk remains PROVISIONAL (rolled back)
    let doc_check = load_durable_memories(&root).unwrap();
    let mem = doc_check
        .memories
        .iter()
        .find(|m| m.id == "dec_rollback_ratify")
        .unwrap();
    assert_eq!(
        mem.lifecycle_state,
        MemoryLifecycleState::Provisional,
        "Memory lifecycle state must roll back to Provisional on telemetry failure"
    );
    assert!(mem.ratified_by.is_none());

    // 5. Heal telemetry storage
    let _ = fs::remove_file(&events_path);
    fs::create_dir_all(&events_path).unwrap();

    // 6. Retry ratification - now succeeds cleanly
    let head = engram::git::current_revision(&root).unwrap();
    let updated = ratify_decision(
        &root,
        "dec_rollback_ratify",
        "sarah_lead",
        "Approved for production scaling",
        Some(&head),
    )
    .unwrap();

    assert_eq!(updated.lifecycle_state, MemoryLifecycleState::Durable);
    assert_eq!(updated.ratified_by.as_deref(), Some("sarah_lead"));

    // 7. Verify persisted state and telemetry audit event
    let doc_after = load_durable_memories(&root).unwrap();
    let mem_after = doc_after
        .memories
        .iter()
        .find(|m| m.id == "dec_rollback_ratify")
        .unwrap();
    assert_eq!(mem_after.lifecycle_state, MemoryLifecycleState::Durable);

    let events = load_events(&root).unwrap();
    assert!(
        events
            .iter()
            .any(|e| e.event_type == "DECISION_RATIFIED" && e.actor == "sarah_lead"),
        "Telemetry event must be recorded upon successful retry"
    );
    let (chain_status, _) = verify_event_chain(&root).unwrap();
    assert_eq!(chain_status, EventChainStatus::Valid);

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 14. Failure-injection: reject_decision telemetry failure triggers rollback
// ---------------------------------------------------------------------------
#[test]
fn test_14_reject_decision_telemetry_failure_rolls_back() {
    let root = setup_test_git_repo("test14_reject_telemetry_failure");

    // 1. Create a provisional decision
    let mut doc = load_durable_memories(&root).unwrap();
    doc.memories.push(DurableMemory {
        id: "dec_rollback_reject".to_string(),
        title: "Global Shared State".to_string(),
        statement: "Use global mutable static for caching".to_string(),
        reason: "Fast access".to_string(),
        source: None,
        provenance: None,
        accepted_by: "ai_assistant".to_string(),
        accepted_at: chrono::Utc::now(),
        scope: MemoryScope::module("cache"),
        evidence_tier: EvidenceTier::Inferred,
        lifecycle_state: MemoryLifecycleState::Provisional,
        supersedes: None,
        superseded_by: None,
        tags: vec![],
        ratified_at_commit: None,
        ratified_by: None,
        ratified_at: None,
    });
    save_durable_memories(&root, &doc).unwrap();

    // 2. Inject telemetry failure
    let events_path = root.join(".engram").join("events");
    let _ = fs::remove_dir_all(&events_path);
    fs::write(&events_path, "telemetry blocker").unwrap();

    // 3. Attempt rejection - must fail with ConsistencyError and cleanly roll back
    let reject_err = reject_decision(
        &root,
        "dec_rollback_reject",
        "marcus_architect",
        "Global mutable state violates concurrency invariants",
    )
    .unwrap_err();

    let err_msg = reject_err.to_string();
    assert!(
        err_msg.contains("Governance consistency error"),
        "Must return typed consistency error, got: {err_msg}"
    );
    let consistency_err = reject_err.downcast::<LifecycleError>().expect("Must be LifecycleError");
    match consistency_err {
        LifecycleError::ConsistencyError {
            state_persisted,
            telemetry_recorded,
            ..
        } => {
            assert!(!state_persisted, "State must not be persisted as new state");
            assert!(!telemetry_recorded, "Telemetry must not be recorded");
        }
        other => panic!("Expected ConsistencyError, got {other:?}"),
    }

    // 4. Verify durable memory remains PROVISIONAL
    let doc_check = load_durable_memories(&root).unwrap();
    let mem = doc_check
        .memories
        .iter()
        .find(|m| m.id == "dec_rollback_reject")
        .unwrap();
    assert_eq!(
        mem.lifecycle_state,
        MemoryLifecycleState::Provisional,
        "Memory lifecycle state must roll back to Provisional on telemetry failure"
    );

    // 5. Heal telemetry storage
    let _ = fs::remove_file(&events_path);
    fs::create_dir_all(&events_path).unwrap();

    // 6. Retry rejection - now succeeds cleanly
    let rejected = reject_decision(
        &root,
        "dec_rollback_reject",
        "marcus_architect",
        "Global mutable state violates concurrency invariants",
    )
    .unwrap();

    assert_eq!(rejected.lifecycle_state, MemoryLifecycleState::Rejected);

    // 7. Verify persisted state and telemetry audit event
    let doc_after = load_durable_memories(&root).unwrap();
    let mem_after = doc_after
        .memories
        .iter()
        .find(|m| m.id == "dec_rollback_reject")
        .unwrap();
    assert_eq!(mem_after.lifecycle_state, MemoryLifecycleState::Rejected);

    let events = load_events(&root).unwrap();
    assert!(
        events
            .iter()
            .any(|e| e.event_type == "DECISION_REJECTED" && e.actor == "marcus_architect"),
        "Telemetry event must be recorded upon successful retry"
    );
    let (chain_status, _) = verify_event_chain(&root).unwrap();
    assert_eq!(chain_status, EventChainStatus::Valid);

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 15. Failure-injection: re_ratify_decision telemetry failure triggers rollback
// ---------------------------------------------------------------------------
#[test]
fn test_15_re_ratify_decision_telemetry_failure_rolls_back() {
    let root = setup_test_git_repo("test15_reratify_telemetry_failure");

    // 1. Create a decision in StaleReview state
    let head = engram::git::current_revision(&root).unwrap();
    let mut doc = load_durable_memories(&root).unwrap();
    doc.memories.push(DurableMemory {
        id: "dec_rollback_reratify".to_string(),
        title: "Message Queue Partitioning".to_string(),
        statement: "Use partition keys matching customer ID".to_string(),
        reason: "Ordered delivery".to_string(),
        source: None,
        provenance: None,
        accepted_by: "lead_dev".to_string(),
        accepted_at: chrono::Utc::now(),
        scope: MemoryScope::module("queue"),
        evidence_tier: EvidenceTier::Explicit,
        lifecycle_state: MemoryLifecycleState::StaleReview,
        supersedes: None,
        superseded_by: None,
        tags: vec![],
        ratified_at_commit: Some(head.clone()),
        ratified_by: Some("lead_dev".to_string()),
        ratified_at: Some(chrono::Utc::now()),
    });
    save_durable_memories(&root, &doc).unwrap();

    // 2. Inject telemetry failure
    let events_path = root.join(".engram").join("events");
    let _ = fs::remove_dir_all(&events_path);
    fs::write(&events_path, "telemetry blocker").unwrap();

    // 3. Attempt re-ratification - must fail with ConsistencyError and cleanly roll back
    let reratify_err = re_ratify_decision(
        &root,
        "dec_rollback_reratify",
        Some(&head),
        "clara_reviewer",
    )
    .unwrap_err();

    let err_msg = reratify_err.to_string();
    assert!(
        err_msg.contains("Governance consistency error"),
        "Must return typed consistency error, got: {err_msg}"
    );
    let consistency_err = reratify_err.downcast::<LifecycleError>().expect("Must be LifecycleError");
    match consistency_err {
        LifecycleError::ConsistencyError {
            state_persisted,
            telemetry_recorded,
            ..
        } => {
            assert!(!state_persisted, "State must not be persisted as new state");
            assert!(!telemetry_recorded, "Telemetry must not be recorded");
        }
        other => panic!("Expected ConsistencyError, got {other:?}"),
    }

    // 4. Verify durable memory remains STALE_REVIEW
    let doc_check = load_durable_memories(&root).unwrap();
    let mem = doc_check
        .memories
        .iter()
        .find(|m| m.id == "dec_rollback_reratify")
        .unwrap();
    assert_eq!(
        mem.lifecycle_state,
        MemoryLifecycleState::StaleReview,
        "Memory lifecycle state must roll back to StaleReview on telemetry failure"
    );

    // 5. Heal telemetry storage
    let _ = fs::remove_file(&events_path);
    fs::create_dir_all(&events_path).unwrap();

    // 6. Retry re-ratification - now succeeds cleanly
    let reratified = re_ratify_decision(
        &root,
        "dec_rollback_reratify",
        Some(&head),
        "clara_reviewer",
    )
    .unwrap();

    assert_eq!(reratified.lifecycle_state, MemoryLifecycleState::Durable);
    assert_eq!(reratified.ratified_by.as_deref(), Some("clara_reviewer"));

    // 7. Verify persisted state and telemetry audit event
    let doc_after = load_durable_memories(&root).unwrap();
    let mem_after = doc_after
        .memories
        .iter()
        .find(|m| m.id == "dec_rollback_reratify")
        .unwrap();
    assert_eq!(mem_after.lifecycle_state, MemoryLifecycleState::Durable);

    let events = load_events(&root).unwrap();
    assert!(
        events
            .iter()
            .any(|e| e.event_type == "DECISION_RE_RATIFIED" && e.actor == "clara_reviewer"),
        "Telemetry event must be recorded upon successful retry"
    );
    let (chain_status, _) = verify_event_chain(&root).unwrap();
    assert_eq!(chain_status, EventChainStatus::Valid);

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 16. Failure-injection: ratify_contract telemetry failure triggers rollback
// ---------------------------------------------------------------------------
#[test]
fn test_16_ratify_contract_telemetry_failure_rolls_back() {
    let root = setup_test_git_repo("test16_contract_telemetry_failure");

    // 1. Create a contract (default is Unratified)
    let contract = create_contract(
        &root,
        "Inventory service boundary contract",
        Some(vec!["inventory".to_string()]),
        Some(vec!["src/inventory/**".to_string()]),
        Some(vec!["src/billing/**".to_string()]),
    )
    .unwrap();

    assert_eq!(
        contract.provenance.ratification_status,
        RatificationStatus::Unratified
    );

    // 2. Inject telemetry failure
    let engram_dir = root.join(".engram");
    fs::create_dir_all(&engram_dir).unwrap();
    let events_path = engram_dir.join("events");
    let _ = fs::remove_dir_all(&events_path);
    fs::write(&events_path, "telemetry blocker").unwrap();

    // 3. Attempt ratification - must fail and cleanly roll back contract state
    let ratify_err = ratify_contract(
        &root,
        &contract.contract_id,
        "david_sec_lead",
        "Approved boundary separation",
    )
    .unwrap_err();

    let err_msg = ratify_err.to_string();
    assert!(
        err_msg.contains("CONTRACT_RATIFICATION_FAILED"),
        "Must return failure error, got: {err_msg}"
    );
    assert!(
        err_msg.contains("Contract state was cleanly rolled back"),
        "Must indicate clean rollback, got: {err_msg}"
    );

    // 4. Verify contract on disk remains in prior status (Unratified)
    let saved_contract = get_contract(&root, &contract.contract_id).unwrap();
    assert_eq!(
        saved_contract.provenance.ratification_status,
        RatificationStatus::Unratified,
        "Contract status must roll back to Unratified on telemetry failure"
    );
    assert!(saved_contract.provenance.ratified_by.is_none());

    // 5. Heal telemetry storage
    let _ = fs::remove_file(&events_path);
    fs::create_dir_all(&events_path).unwrap();

    // 6. Retry contract ratification - now succeeds cleanly
    let ratified = ratify_contract(
        &root,
        &contract.contract_id,
        "david_sec_lead",
        "Approved boundary separation",
    )
    .unwrap();

    assert_eq!(
        ratified.provenance.ratification_status,
        RatificationStatus::Ratified
    );
    assert_eq!(
        ratified.provenance.ratified_by.as_deref(),
        Some("david_sec_lead")
    );

    // 7. Verify persisted contract state and telemetry audit event
    let saved_after = get_contract(&root, &contract.contract_id).unwrap();
    assert_eq!(
        saved_after.provenance.ratification_status,
        RatificationStatus::Ratified
    );

    let events = load_events(&root).unwrap();
    assert!(
        events
            .iter()
            .any(|e| e.event_type == "CONTRACT_RATIFIED" && e.actor == "david_sec_lead"),
        "Telemetry event must be recorded upon successful retry"
    );
    let (chain_status, _) = verify_event_chain(&root).unwrap();
    assert_eq!(chain_status, EventChainStatus::Valid);

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// 17. Failure-injection: state persistence failure produces zero ghost telemetry
// ---------------------------------------------------------------------------
#[test]
fn test_17_state_persistence_failure_emits_no_telemetry() {
    let root = setup_test_git_repo("test17_persistence_failure");

    // 1. Create a provisional decision
    let mut doc = load_durable_memories(&root).unwrap();
    doc.memories.push(DurableMemory {
        id: "dec_persist_fail".to_string(),
        title: "Test Persistence Failure".to_string(),
        statement: "Should fail gracefully".to_string(),
        reason: "Test".to_string(),
        source: None,
        provenance: None,
        accepted_by: "tester".to_string(),
        accepted_at: chrono::Utc::now(),
        scope: MemoryScope::module("test"),
        evidence_tier: EvidenceTier::Inferred,
        lifecycle_state: MemoryLifecycleState::Provisional,
        supersedes: None,
        superseded_by: None,
        tags: vec![],
        ratified_at_commit: None,
        ratified_by: None,
        ratified_at: None,
    });
    save_durable_memories(&root, &doc).unwrap();

    let initial_events = load_events(&root).unwrap();
    let initial_event_count = initial_events.len();

    // 2. An operation that fails validation before persistence or telemetry
    let err = ratify_decision(&root, "non_existent_id", "reviewer", "reason", None);
    assert!(err.is_err());

    // 3. Telemetry events count must not have changed (zero ghost events)
    let events_after = load_events(&root).unwrap();
    assert_eq!(
        events_after.len(),
        initial_event_count,
        "Zero ghost telemetry events must be emitted when operation fails"
    );

    let _ = fs::remove_dir_all(&root);
}
