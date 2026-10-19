//! Acceptance Integration Tests for Engram Phase 2A: Benchmark Corpus & Multi-Arm Test Runner.
//!
//! Enforces:
//! 1. Deterministic Replay Reproducibility (byte-identical normalized JSON reports).
//! 2. Identical Initial States Across Experimental Arms (canonical tree digest equality).
//! 3. Malicious Path Traversal Protection (rejects parent directory escapes).
//! 4. Forbidden Command & Shell Argument Rejection (blocks interpreters and script-evaluation flags).
//! 5. Subprocess Watchdog Timeout & Hermetic Cleanup (RAII drop on timeout).
//! 6. Invalid Session Quarantine & Explicit Denominator Accounting.
//! 7. Corpus Provenance & Tree Digest Tamper-Detection.
//! 8. End-to-End Replay Benchmark Execution over the full Phase 2A Corpus.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use engram::empirical::{
    AgentSessionTelemetry, ExperimentalArm, IncompleteReason, MetricResult, SessionValidity,
};
use engram::empirical_runner::{
    AgentDriver, BenchmarkRunnerError, CorpusRegistry, DeterministicReplayDriver,
    EVALUATION_TYPE_REPLAY, MultiArmOrchestrator, REPORT_SCHEMA_VERSION, RunnerSecurityError,
    Sandbox, ScenarioDefinition, compute_directory_tree_digest, validate_command,
    validate_command_arguments, validate_path_containment,
};

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("empirical")
}

// ---------------------------------------------------------------------------
// Test 1: Deterministic Replay Reproducibility
// ---------------------------------------------------------------------------
#[test]
fn test_deterministic_reproducibility() {
    let root = corpus_dir();
    let registry1 = CorpusRegistry::load_from_directory(&root).expect("Load registry 1");
    let registry2 = CorpusRegistry::load_from_directory(&root).expect("Load registry 2");

    let driver1 = Box::new(DeterministicReplayDriver::new(root.clone()));
    let driver2 = Box::new(DeterministicReplayDriver::new(root.clone()));

    let orchestrator1 = MultiArmOrchestrator::new(
        registry1,
        driver1,
        "2035efad22008ebdc89ae0dcab858ca7978f5209",
    );
    let orchestrator2 = MultiArmOrchestrator::new(
        registry2,
        driver2,
        "2035efad22008ebdc89ae0dcab858ca7978f5209",
    );

    let report1 = orchestrator1.execute_run().expect("Execute run 1");
    let report2 = orchestrator2.execute_run().expect("Execute run 2");

    // Normalized copies strip volatile UUIDs and timestamps
    let det1 = report1.deterministic_copy();
    let det2 = report2.deterministic_copy();

    let json1 = serde_json::to_string_pretty(&det1).expect("Serialize 1");
    let json2 = serde_json::to_string_pretty(&det2).expect("Serialize 2");

    assert_eq!(
        json1, json2,
        "Sequential benchmark executions must produce byte-identical deterministic JSON reports"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Identical Initial States Across Arms
// ---------------------------------------------------------------------------
#[test]
fn test_identical_initial_states_across_arms() {
    let workspace_template = corpus_dir().join("scen_rust_tokens_01").join("workspace");
    assert!(workspace_template.exists(), "Workspace template must exist");

    let sandbox_ctrl =
        Sandbox::from_template("test_ctrl", &workspace_template).expect("Create control sandbox");
    let sandbox_treat = Sandbox::from_template("test_treat", &workspace_template)
        .expect("Create treatment sandbox");

    sandbox_ctrl.init_hermetic_git().expect("Git init ctrl");
    sandbox_treat.init_hermetic_git().expect("Git init treat");

    let digest_ctrl = sandbox_ctrl
        .compute_tree_digest()
        .expect("Compute ctrl digest");
    let digest_treat = sandbox_treat
        .compute_tree_digest()
        .expect("Compute treat digest");

    assert_eq!(
        digest_ctrl, digest_treat,
        "Control and Treatment arms must start from bit-for-bit identical file trees"
    );

    // Verify sensitivity: mutating a file in one arm changes the digest
    let mutate_target = sandbox_ctrl.path.join("src").join("ledger.rs");
    fs::write(&mutate_target, b"// Accidental modification").expect("Write mutation");

    let mutated_digest = sandbox_ctrl
        .compute_tree_digest()
        .expect("Compute mutated digest");
    assert_ne!(
        digest_ctrl, mutated_digest,
        "Canonical tree digest must detect accidental mutations"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Malicious Path Traversal Protection
// ---------------------------------------------------------------------------
#[test]
fn test_malicious_path_traversal_rejected() {
    let sandbox = Sandbox::new("test_traversal").expect("Create sandbox");

    // Parent directory traversal
    let bad_path_1 = Path::new("../escaped.txt");
    let res1 = validate_path_containment(&sandbox.path, bad_path_1);
    assert!(matches!(
        res1,
        Err(RunnerSecurityError::PathEscapesSandbox { .. })
    ));

    // Nested parent traversal
    let bad_path_2 = Path::new("sub/../../escaped.txt");
    let res2 = validate_path_containment(&sandbox.path, bad_path_2);
    assert!(matches!(
        res2,
        Err(RunnerSecurityError::PathEscapesSandbox { .. })
    ));

    // Contained path should pass
    let safe_path = Path::new("src/main.rs");
    let res3 = validate_path_containment(&sandbox.path, safe_path);
    assert!(res3.is_ok());
}

// ---------------------------------------------------------------------------
// Test 4: Forbidden Command & Shell Argument Rejection
// ---------------------------------------------------------------------------
#[test]
fn test_forbidden_command_execution_rejected() {
    // Arbitrary shell interpreters are strictly forbidden
    assert!(matches!(
        validate_command("cmd.exe"),
        Err(RunnerSecurityError::DisallowedCommand { .. })
    ));
    assert!(matches!(
        validate_command("powershell.exe"),
        Err(RunnerSecurityError::DisallowedCommand { .. })
    ));
    assert!(matches!(
        validate_command("/bin/sh"),
        Err(RunnerSecurityError::DisallowedCommand { .. })
    ));
    assert!(matches!(
        validate_command("bash"),
        Err(RunnerSecurityError::DisallowedCommand { .. })
    ));

    // Allowlisted compilers and runtimes pass
    assert!(validate_command("cargo").is_ok());
    assert!(validate_command("cargo.exe").is_ok());
    assert!(validate_command("python").is_ok());
    assert!(validate_command("node").is_ok());

    // Prohibit arbitrary script evaluation flags
    let bad_args_c = vec!["-c".to_string(), "print(1)".to_string()];
    assert!(matches!(
        validate_command_arguments(&bad_args_c),
        Err(RunnerSecurityError::DisallowedArgument { .. })
    ));

    let bad_args_slash_c = vec!["/c".to_string(), "dir".to_string()];
    assert!(matches!(
        validate_command_arguments(&bad_args_slash_c),
        Err(RunnerSecurityError::DisallowedArgument { .. })
    ));

    let bad_args_e = vec!["-e".to_string(), "process.exit(1)".to_string()];
    assert!(matches!(
        validate_command_arguments(&bad_args_e),
        Err(RunnerSecurityError::DisallowedArgument { .. })
    ));

    // Safe arguments pass
    let safe_args = vec![
        "test".to_string(),
        "--".to_string(),
        "--nocapture".to_string(),
    ];
    assert!(validate_command_arguments(&safe_args).is_ok());
}

// ---------------------------------------------------------------------------
// Test 5: Subprocess Watchdog Timeout & Hermetic Cleanup
// ---------------------------------------------------------------------------
#[test]
fn test_subprocess_timeout_watchdog_and_cleanup() {
    let sandbox = Sandbox::new("test_timeout").expect("Create sandbox");
    let sandbox_path = sandbox.path.clone();

    // Spawning cargo with a 1-millisecond timeout is guaranteed to trigger the watchdog
    let cmd = vec!["cargo".to_string(), "test".to_string()];
    let res = sandbox.run_verification_command(&cmd, Duration::from_millis(1));

    assert!(
        matches!(
            res,
            Err(BenchmarkRunnerError::Security(
                RunnerSecurityError::ExecutionTimeout { .. }
            ))
        ),
        "Expected ExecutionTimeout error, got: {:?}",
        res
    );

    // Verify RAII cleanup on drop
    drop(sandbox);
    assert!(
        !sandbox_path.exists(),
        "Sandbox directory must be recursively deleted on Drop"
    );
}

// ---------------------------------------------------------------------------
// Test 6: Invalid Session Quarantine & Exclusion Accounting
// ---------------------------------------------------------------------------
#[test]
fn test_invalid_session_quarantine_and_exclusion_accounting() {
    // Custom driver that emits an incomplete control session
    struct IncompleteControlDriver {
        fallback: DeterministicReplayDriver,
    }

    impl AgentDriver for IncompleteControlDriver {
        fn driver_name(&self) -> &'static str {
            "IncompleteControlDriver"
        }

        fn execute_arm(
            &self,
            scenario: &ScenarioDefinition,
            arm: ExperimentalArm,
            sandbox: &Sandbox,
        ) -> Result<AgentSessionTelemetry, BenchmarkRunnerError> {
            let mut session = self.fallback.execute_arm(scenario, arm, sandbox)?;
            if arm == ExperimentalArm::Control && scenario.scenario_id == "scen_rust_tokens_01" {
                session.validity = SessionValidity::Incomplete(IncompleteReason::Timeout);
            }
            Ok(session)
        }
    }

    let root = corpus_dir();
    let registry = CorpusRegistry::load_from_directory(&root).expect("Load registry");
    let driver = Box::new(IncompleteControlDriver {
        fallback: DeterministicReplayDriver::new(root),
    });

    let orchestrator =
        MultiArmOrchestrator::new(registry, driver, "2035efad22008ebdc89ae0dcab858ca7978f5209");
    let report = orchestrator.execute_run().expect("Execute run");

    // The incomplete control session must be quarantined in accounting
    assert_eq!(report.accounting.total_scenarios, 4);
    assert_eq!(report.accounting.attempted_pairs, 4);
    assert_eq!(report.accounting.excluded_incomplete_control, 1);
    assert_eq!(report.accounting.valid_matched_pairs, 3);
    assert_eq!(report.matched_pair_results.len(), 3);
}

// ---------------------------------------------------------------------------
// Test 7: Corpus Provenance & Tree Digest Tamper-Detection
// ---------------------------------------------------------------------------
#[test]
fn test_provenance_and_corpus_digest_verification() {
    let root = corpus_dir();
    let registry = CorpusRegistry::load_from_directory(&root).expect("Load registry");

    let original_digest = registry.compute_corpus_digest().expect("Compute digest");
    assert_eq!(
        original_digest.len(),
        64,
        "SHA-256 hex digest must be 64 chars"
    );

    // Compute tree digest on single workspace
    let rust_ws = root.join("scen_rust_tokens_01").join("workspace");
    let ws_digest_1 = compute_directory_tree_digest(&rust_ws).expect("Tree digest 1");
    let ws_digest_2 = compute_directory_tree_digest(&rust_ws).expect("Tree digest 2");
    assert_eq!(
        ws_digest_1, ws_digest_2,
        "Directory tree digest must be deterministic"
    );
}

// ---------------------------------------------------------------------------
// Test 8: End-to-End Replay Benchmark Execution over Phase 2A Corpus
// ---------------------------------------------------------------------------
#[test]
fn test_end_to_end_phase2a_corpus_execution() {
    let root = corpus_dir();
    let registry = CorpusRegistry::load_from_directory(&root).expect("Load registry");
    assert_eq!(
        registry.scenarios.len(),
        4,
        "Phase 2A corpus must contain exactly 4 scenarios"
    );

    let driver = Box::new(DeterministicReplayDriver::new(root));
    let orchestrator =
        MultiArmOrchestrator::new(registry, driver, "2035efad22008ebdc89ae0dcab858ca7978f5209");

    let report = orchestrator.execute_run().expect("Execute e2e run");

    // Report Provenance Invariants
    assert_eq!(report.schema_version, REPORT_SCHEMA_VERSION);
    assert_eq!(report.evaluation_type, EVALUATION_TYPE_REPLAY);
    assert_eq!(
        report.git_commit,
        "2035efad22008ebdc89ae0dcab858ca7978f5209"
    );

    // Accounting Invariants
    assert_eq!(report.accounting.total_scenarios, 4);
    assert_eq!(report.accounting.attempted_pairs, 4);
    assert_eq!(report.accounting.valid_matched_pairs, 4);
    assert_eq!(report.accounting.excluded_incomplete_control, 0);
    assert_eq!(report.accounting.excluded_incomplete_treatment, 0);
    assert_eq!(report.accounting.excluded_infrastructure_failures, 0);

    // Aggregate Metric Outcomes
    // Control: 3 passed, 1 failed (tax impact). Treatment: 4 passed.
    assert!((report.aggregate_metrics.control_pass_rate - 0.75).abs() < 1e-4);
    assert!((report.aggregate_metrics.treatment_pass_rate - 1.00).abs() < 1e-4);
    assert!((report.aggregate_metrics.pass_rate_lift - 0.25).abs() < 1e-4);

    // Token savings should be positive
    if let MetricResult::Defined(mean_savings) = report.aggregate_metrics.mean_token_savings {
        assert!(
            mean_savings > 0.80,
            "Mean token savings should exceed 80%, got: {:.2}%",
            mean_savings * 100.0
        );
    } else {
        panic!("Mean token savings must be Defined");
    }

    // Markdown summary formatting
    let md = report.to_markdown_summary();
    assert!(md.contains("# Engram Empirical Benchmark Run Summary"));
    assert!(md.contains("HARNESS_VALIDATION_REPLAY"));
    assert!(md.contains("Task Pass Rate"));
}
