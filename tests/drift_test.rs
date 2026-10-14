//! Drift Subsystem & Scoped Staleness Test Suite (Phase 4):
//! - Test 1: Scoped staleness ignores unrelated commits (e.g. commits to docs/ do not trigger staleness for OrderService).
//! - Test 2: Governed code evolution flags STALE_REVIEW after threshold commits.
//! - Test 3: Explicit decision re-ratification resets governed mutation count and clears STALE_REVIEW.
//! - Test 4: Boundary erosion and repeated contract failures trend calculation.
//! - Test 5: Drift CLI and MCP output formatting (both human-readable and JSON protocol 1.1).
//! - Test 6: Shallow Git fallback: gracefully outputs SHALLOW_HISTORY_TRUNCATED without crashing.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

use engram::contract::create_contract;
use engram::drift::{
    analyze_drift, calculate_trend, format_drift_report, DriftOptions, TrendIndicator,
};
use engram::git::SHALLOW_HISTORY_TRUNCATED;
use engram::mcp::mcp_call;
use engram::memory::{
    load_durable_memories, re_ratify_decision, record_scoped_decision, MemoryLifecycleState,
    MemoryScope, ScopeLevel,
};

fn setup_test_repo(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("engram_drift_{prefix}_{}", Uuid::new_v4()));
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

    run_cmd(&["init"]);
    run_cmd(&["config", "user.name", "Drift Test"]);
    run_cmd(&["config", "user.email", "drift@engram.dev"]);

    fs::write(dir.join("README.md"), "# Drift Test Repo\n").unwrap();
    run_cmd(&["add", "README.md"]);
    run_cmd(&["commit", "-m", "Initial commit"]);

    dir
}

#[test]
fn test_1_scoped_staleness_ignores_unrelated_commits() {
    let root = setup_test_repo("test1");

    // 1. Record decision governing OrderService (symbol: OrderService, file: src/orders/service.rs)
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

    let mem = record_scoped_decision(
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

    let initial_ratified_commit = mem.ratified_commit().unwrap().to_string();

    // 2. Commit unrelated files (10 commits to docs/)
    fs::create_dir_all(root.join("docs")).unwrap();
    for i in 1..=10 {
        let doc_file = root.join("docs").join(format!("doc_{i}.md"));
        fs::write(&doc_file, format!("Documentation page {i}\n")).unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&root)
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["commit", "-m", &format!("docs commit {i}")])
            .current_dir(&root)
            .output()
            .unwrap();
    }

    // 3. Run drift analysis
    let report = analyze_drift(&root, &DriftOptions::default()).unwrap();

    // 4. Verification:
    // Global commits analyzed >= 10
    assert!(report.global_commits_analyzed >= 10);

    // Decision drift evidence for dec_orders shows governed_mutations == 0
    let dd = report
        .decision_drift
        .iter()
        .find(|d| d.decision_id == "dec_orders")
        .expect("Decision drift evidence found");
    assert_eq!(dd.governed_mutations, 0);
    assert_eq!(dd.status, "ACTIVE");
    assert_eq!(
        dd.ratified_at_commit.as_deref(),
        Some(initial_ratified_commit.as_str())
    );

    // Decision is NOT in stale_decisions
    assert!(!report
        .stale_decisions
        .iter()
        .any(|s| s.decision_id == "dec_orders"));

    // Durable memory in YAML remains Durable
    let doc = load_durable_memories(&root).unwrap();
    let current_mem = doc.memories.iter().find(|m| m.id == "dec_orders").unwrap();
    assert_eq!(current_mem.lifecycle_state, MemoryLifecycleState::Durable);
}

#[test]
fn test_2_governed_code_evolution_flags_stale_review_after_threshold() {
    let root = setup_test_repo("test2");

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
        .args(["commit", "-m", "add orders"])
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
        "OrderService Policy",
        "OrderService rules",
        "Clean architecture",
        scope,
        None,
        "architect",
    )
    .unwrap();

    // Make 5 commits modifying the governed file src/orders/service.rs
    for i in 1..=5 {
        fs::write(
            root.join("src/orders/service.rs"),
            format!("pub struct OrderService; // mutation {i}\n"),
        )
        .unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&root)
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["commit", "-m", &format!("update OrderService {i}")])
            .current_dir(&root)
            .output()
            .unwrap();
    }

    // Run drift analysis with default threshold (5)
    let report = analyze_drift(&root, &DriftOptions::default()).unwrap();

    // Governed mutations should be 5
    let dd = report
        .decision_drift
        .iter()
        .find(|d| d.decision_id == "dec_orders")
        .expect("Decision drift record");
    assert_eq!(dd.governed_mutations, 5);
    assert_eq!(dd.status, "STALE_REVIEW");

    // Decision IS in stale_decisions
    let stale = report
        .stale_decisions
        .iter()
        .find(|s| s.decision_id == "dec_orders")
        .expect("Stale decision found");
    assert_eq!(stale.governed_mutations, 5);
    assert_eq!(stale.threshold, 5);
    assert_eq!(stale.status, "STALE_REVIEW");
    assert!(stale.recommended_action.contains("re-ratify"));

    // State in durable memory transitioned to StaleReview
    let doc = load_durable_memories(&root).unwrap();
    let current_mem = doc.memories.iter().find(|m| m.id == "dec_orders").unwrap();
    assert_eq!(
        current_mem.lifecycle_state,
        MemoryLifecycleState::StaleReview
    );

    // Deterministic evidence includes STALE_DECISIONS
    assert!(report.deterministic_evidence.iter().any(|e| {
        e.evidence_type == "STALE_DECISIONS" && e.target.contains("dec_orders")
    }));
}

#[test]
fn test_3_decision_re_ratification_resets_mutations_and_clears_stale_review() {
    let root = setup_test_repo("test3");

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
        .args(["commit", "-m", "init orders"])
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
        "OrderService Policy",
        "OrderService rules",
        "Clean architecture",
        scope,
        None,
        "architect",
    )
    .unwrap();

    // 5 mutations
    for i in 1..=5 {
        fs::write(
            root.join("src/orders/service.rs"),
            format!("pub struct OrderService; // v{i}\n"),
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

    // Verify it is flagged STALE_REVIEW
    let report1 = analyze_drift(&root, &DriftOptions::default()).unwrap();
    assert!(report1
        .stale_decisions
        .iter()
        .any(|s| s.decision_id == "dec_orders"));

    // Explicit re-ratification
    let head = engram::git::current_revision(&root).unwrap();
    let updated =
        re_ratify_decision(&root, "dec_orders", Some(&head), "lead_engineer").unwrap();
    assert_eq!(updated.lifecycle_state, MemoryLifecycleState::Durable);
    assert_eq!(updated.ratified_at_commit.as_deref(), Some(head.as_str()));
    assert_eq!(updated.ratified_by.as_deref(), Some("lead_engineer"));

    // Run drift analysis again: governed mutations reset to 0, STALE_REVIEW cleared!
    let report2 = analyze_drift(&root, &DriftOptions::default()).unwrap();
    let dd2 = report2
        .decision_drift
        .iter()
        .find(|d| d.decision_id == "dec_orders")
        .unwrap();
    assert_eq!(dd2.governed_mutations, 0);
    assert_eq!(dd2.status, "ACTIVE");
    assert!(!report2
        .stale_decisions
        .iter()
        .any(|s| s.decision_id == "dec_orders"));

    // Memory in YAML is Durable
    let doc = load_durable_memories(&root).unwrap();
    let m = doc.memories.iter().find(|m| m.id == "dec_orders").unwrap();
    assert_eq!(m.lifecycle_state, MemoryLifecycleState::Durable);
}

#[test]
fn test_4_boundary_erosion_and_repeated_contract_failures_trend_calculation() {
    let root = setup_test_repo("test4");

    // Unit test trend helper first
    assert_eq!(calculate_trend(3, 1), TrendIndicator::Improving);
    assert_eq!(calculate_trend(1, 3), TrendIndicator::Deteriorating);
    assert_eq!(calculate_trend(2, 2), TrendIndicator::Stable);
    assert_eq!(calculate_trend(0, 0), TrendIndicator::Stable);

    // Setup contract: expected_modules: ["domain"], forbidden_boundaries: ["infrastructure"]
    fs::create_dir_all(root.join("src/domain")).unwrap();
    fs::create_dir_all(root.join("src/infrastructure")).unwrap();

    let _contract = create_contract(
        &root,
        "Enforce Domain Boundary",
        Some(vec!["domain".to_string()]),
        Some(vec!["src/domain/model.rs".to_string()]),
        Some(vec!["infrastructure".to_string()]),
    )
    .unwrap();

    // Create 4 chronological commits:
    // Commits 1, 2: violation in older window (modifies domain and infrastructure)
    // Commit 3: violation in recent window
    // Commit 4: clean commit in recent window
    // Older window violations: 2, Recent window violations: 1 -> Trend: Improving
    fs::write(root.join("src/domain/model.rs"), "pub struct ModelV1;\n").unwrap();
    fs::write(root.join("src/infrastructure/db.rs"), "pub struct DbV1;\n").unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["commit", "-m", "violation 1"])
        .current_dir(&root)
        .output()
        .unwrap();

    fs::write(root.join("src/domain/model.rs"), "pub struct ModelV2;\n").unwrap();
    fs::write(root.join("src/infrastructure/db.rs"), "pub struct DbV2;\n").unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["commit", "-m", "violation 2"])
        .current_dir(&root)
        .output()
        .unwrap();

    fs::write(root.join("src/domain/model.rs"), "pub struct ModelV3;\n").unwrap();
    fs::write(root.join("src/infrastructure/db.rs"), "pub struct DbV3;\n").unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["commit", "-m", "violation 3"])
        .current_dir(&root)
        .output()
        .unwrap();

    fs::write(root.join("src/domain/model.rs"), "pub struct ModelV4;\n").unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["commit", "-m", "clean domain commit"])
        .current_dir(&root)
        .output()
        .unwrap();

    let report = analyze_drift(&root, &DriftOptions::default()).unwrap();

    // Verify boundary erosion evidence
    let be = report
        .boundary_violations
        .iter()
        .find(|b| b.source_module == "domain" && b.target_boundary == "infrastructure")
        .expect("Boundary erosion found");
    assert_eq!(be.violation_count, 3);
    assert_eq!(be.trend, TrendIndicator::Improving);

    // Verify repeated contract failures evidence
    let rf = report
        .repeated_contract_failures
        .iter()
        .find(|r| r.module == "domain")
        .expect("Contract failure found");
    assert_eq!(rf.failure_count, 3);
    assert_eq!(rf.trend, TrendIndicator::Improving);

    // Verify deterministic evidence contains both
    assert!(report.deterministic_evidence.iter().any(|e| {
        e.evidence_type == "BOUNDARY_VIOLATIONS" && e.trend == TrendIndicator::Improving
    }));
    assert!(report.deterministic_evidence.iter().any(|e| {
        e.evidence_type == "REPEATED_CONTRACT_FAILURES" && e.trend == TrendIndicator::Improving
    }));
}

#[test]
fn test_5_drift_cli_and_mcp_output_formatting() {
    let root = setup_test_repo("test5");

    let report = analyze_drift(&root, &DriftOptions::default()).unwrap();

    // 1. JSON protocol version 1.1 verification
    let json_val = serde_json::to_value(&report).unwrap();
    assert_eq!(json_val["protocol_version"], "1.1");
    assert!(json_val.get("status").is_some());
    assert!(json_val.get("boundary_violations").is_some());
    assert!(json_val.get("decision_drift").is_some());
    assert!(json_val.get("stale_decisions").is_some());
    assert!(json_val.get("repeated_contract_failures").is_some());
    assert!(json_val.get("deterministic_evidence").is_some());

    // 2. Human-readable format verification
    let formatted = format_drift_report(&report);
    assert!(formatted.contains("Engram Architectural Drift & Boundary Analysis"));
    assert!(formatted.contains("1.1"));
    assert!(formatted.contains("Commits Analyzed"));

    // 3. MCP tool call verification
    let mcp_res = mcp_call(&root, "engram_drift", &serde_json::json!({})).unwrap();
    let text = mcp_res["content"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["protocol_version"], "1.1");
    assert_eq!(parsed["status"], "OK");
}

#[test]
fn test_6_shallow_git_fallback_outputs_truncated_gracefully() {
    let root = setup_test_repo("test6");

    // Simulate shallow repository via .git/shallow marker
    fs::write(
        root.join(".git").join("shallow"),
        "0123456789abcdef0123456789abcdef01234567\n",
    )
    .unwrap();
    assert!(engram::git::is_shallow_repository(&root));

    // Run drift analysis on shallow repo
    let report = analyze_drift(&root, &DriftOptions::default()).unwrap();

    // Graceful response with SHALLOW_HISTORY_TRUNCATED
    assert_eq!(report.status, SHALLOW_HISTORY_TRUNCATED);
    assert_eq!(report.repository_status, SHALLOW_HISTORY_TRUNCATED);
    assert!(report
        .warnings
        .iter()
        .any(|w| w.contains(SHALLOW_HISTORY_TRUNCATED)));

    // Formatted report includes SHALLOW_HISTORY_TRUNCATED
    let formatted = format_drift_report(&report);
    assert!(formatted.contains(SHALLOW_HISTORY_TRUNCATED));

    // MCP call on shallow repo executes cleanly without crashing
    let mcp_res = mcp_call(&root, "engram_drift", &serde_json::json!({})).unwrap();
    let text = mcp_res["content"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["status"], SHALLOW_HISTORY_TRUNCATED);
    assert_eq!(parsed["protocol_version"], "1.1");
}

#[test]
fn test_7_docs_and_non_code_files_mentioning_symbol_do_not_trigger_staleness() {
    let root = setup_test_repo("test7");

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
        .args(["commit", "-m", "init orders"])
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
        "OrderService Policy",
        "OrderService rules",
        "Maintain clean domain boundary",
        scope,
        None,
        "architect",
    )
    .unwrap();

    // Make 6 commits modifying documentation that mentions OrderService
    fs::create_dir_all(root.join("docs")).unwrap();
    for i in 1..=6 {
        fs::write(
            root.join("docs/OrderService.md"),
            format!("# OrderService Guide\nOrderService is great! Revision {i}\n"),
        )
        .unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&root)
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["commit", "-m", &format!("docs edit {i}")])
            .current_dir(&root)
            .output()
            .unwrap();
    }

    let report = analyze_drift(&root, &DriftOptions::default()).unwrap();

    // Governed mutations MUST remain 0 because docs are not code symbol definitions
    let dd = report
        .decision_drift
        .iter()
        .find(|d| d.decision_id == "dec_orders")
        .expect("Decision drift record");
    assert_eq!(dd.governed_mutations, 0);
    assert_eq!(dd.status, "ACTIVE");
    assert!(!report
        .stale_decisions
        .iter()
        .any(|s| s.decision_id == "dec_orders"));

    // State remains Durable
    let doc = load_durable_memories(&root).unwrap();
    let m = doc.memories.iter().find(|m| m.id == "dec_orders").unwrap();
    assert_eq!(m.lifecycle_state, MemoryLifecycleState::Durable);
}

#[test]
fn test_8_re_ratification_with_worktree_resets_mutations_and_clears_stale_review() {
    let root = setup_test_repo("test8");

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
        .args(["commit", "-m", "init orders"])
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
        "OrderService Policy",
        "OrderService rules",
        "Clean architecture",
        scope,
        None,
        "architect",
    )
    .unwrap();

    // Make 5 mutations to trigger STALE_REVIEW
    for i in 1..=5 {
        fs::write(
            root.join("src/orders/service.rs"),
            format!("pub struct OrderService; // mut {i}\n"),
        )
        .unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&root)
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["commit", "-m", &format!("mutate {i}")])
            .current_dir(&root)
            .output()
            .unwrap();
    }

    let report1 = analyze_drift(&root, &DriftOptions::default()).unwrap();
    assert!(report1
        .stale_decisions
        .iter()
        .any(|s| s.decision_id == "dec_orders"));

    // Re-ratify explicitly with WORKTREE
    let updated =
        re_ratify_decision(&root, "dec_orders", Some("WORKTREE"), "architect").unwrap();
    assert_eq!(updated.lifecycle_state, MemoryLifecycleState::Durable);
    assert_eq!(updated.ratified_at_commit.as_deref(), Some("WORKTREE"));

    // Drift analysis must reset governed mutations to 0
    let report2 = analyze_drift(&root, &DriftOptions::default()).unwrap();
    let dd2 = report2
        .decision_drift
        .iter()
        .find(|d| d.decision_id == "dec_orders")
        .unwrap();
    assert_eq!(dd2.governed_mutations, 0);
    assert_eq!(dd2.status, "ACTIVE");
    assert!(!report2
        .stale_decisions
        .iter()
        .any(|s| s.decision_id == "dec_orders"));
}

#[test]
fn test_9_drift_since_worktree_returns_zero_commits() {
    let root = setup_test_repo("test9");

    // Make a commit
    fs::write(root.join("app.rs"), "pub fn run() {}\n").unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["commit", "-m", "add app.rs"])
        .current_dir(&root)
        .output()
        .unwrap();

    let opts = DriftOptions {
        module: None,
        decision: None,
        since: Some("WORKTREE".to_string()),
        staleness_threshold: 5,
    };

    let report = analyze_drift(&root, &opts).unwrap();
    assert_eq!(report.global_commits_analyzed, 0);
    assert_eq!(report.status, "OK");
}

#[test]
fn test_10_flat_single_file_module_matching() {
    let root = setup_test_repo("test10");

    // Flat single file module: src/contract.rs (belongs to module 'contract')
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/contract.rs"), "pub struct Contract;\n").unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["commit", "-m", "init contract"])
        .current_dir(&root)
        .output()
        .unwrap();

    let scope = MemoryScope {
        level: ScopeLevel::Module,
        target: Some("contract".to_string()),
        team: None,
        modules: vec!["contract".to_string()],
        files: vec![],
        symbols: vec![],
    };

    let _mem = record_scoped_decision(
        &root,
        Some("dec_contract"),
        "Contract Engine Policy",
        "Contracts must be elastic",
        "Architecture",
        scope,
        None,
        "architect",
    )
    .unwrap();

    // 5 mutations to flat file src/contract.rs
    for i in 1..=5 {
        fs::write(
            root.join("src/contract.rs"),
            format!("pub struct Contract; // v{i}\n"),
        )
        .unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&root)
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["commit", "-m", &format!("contract edit {i}")])
            .current_dir(&root)
            .output()
            .unwrap();
    }

    let report = analyze_drift(&root, &DriftOptions::default()).unwrap();
    let dd = report
        .decision_drift
        .iter()
        .find(|d| d.decision_id == "dec_contract")
        .expect("Found dec_contract");
    assert_eq!(dd.governed_mutations, 5);
    assert_eq!(dd.status, "STALE_REVIEW");
    assert!(report
        .stale_decisions
        .iter()
        .any(|s| s.decision_id == "dec_contract"));
}

#[test]
fn test_11_mcp_threshold_alias_and_module_filter() {
    let root = setup_test_repo("test11");

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
        .args(["commit", "-m", "init orders"])
        .current_dir(&root)
        .output()
        .unwrap();

    let scope = MemoryScope {
        level: ScopeLevel::Module,
        target: Some("orders".to_string()),
        team: None,
        modules: vec!["orders".to_string()],
        files: vec!["src/orders/service.rs".to_string()],
        symbols: vec![],
    };

    let _mem = record_scoped_decision(
        &root,
        Some("dec_orders"),
        "Orders Policy",
        "Orders policy statement",
        "Architecture",
        scope,
        None,
        "architect",
    )
    .unwrap();

    // 3 mutations
    for i in 1..=3 {
        fs::write(
            root.join("src/orders/service.rs"),
            format!("pub struct OrderService; // v{i}\n"),
        )
        .unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&root)
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["commit", "-m", &format!("edit {i}")])
            .current_dir(&root)
            .output()
            .unwrap();
    }

    // Call MCP with threshold alias (threshold: 3 instead of staleness_threshold: 3)
    let mcp_res = mcp_call(
        &root,
        "engram_drift",
        &serde_json::json!({
            "module": "orders",
            "threshold": 3
        }),
    )
    .unwrap();

    let text = mcp_res["content"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

    assert_eq!(parsed["status"], "DRIFT_DETECTED");
    assert_eq!(parsed["stale_decisions"].as_array().unwrap().len(), 1);
    assert_eq!(
        parsed["stale_decisions"][0]["decision_id"],
        "dec_orders"
    );
    assert_eq!(parsed["stale_decisions"][0]["threshold"], 3);
}
