//! Deterministic Unit Tests for Engram Empirical Benchmark Engine (Phase 1).
//!
//! Validates:
//! 1. Telemetry schema serialization/deserialization fidelity and versioning.
//! 2. Temporal consistency validation (start/end timestamp derivation and tolerance checking).
//! 3. Session validity classifications (Valid vs Incomplete vs Infrastructure Failure).
//! 4. Pure token savings calculations across arms and zero-denominator handling.
//! 5. Disaggregated file interaction metrics (read discovery vs edit vs creation vs deletion).
//! 6. Boundary governance obedience and enforcement effectiveness with explicit denominators.
//! 7. Change-impact blast-radius detection accuracy.
//! 8. Task evaluation and regression rate calculation.
//! 9. Multi-turn memory compounding lifts.
//! 10. Matched pair end-to-end comparison.

use chrono::{TimeZone, Utc};
use engram::empirical::{
    AgentSessionTelemetry, BoundaryViolationRecord, ChangeImpactMetrics, ConsistencyError,
    ExperimentalArm, FileInteractions, GovernedOperation, IncompleteReason,
    InfrastructureFailureReason, MetricResult, MutationCategory, OperationKind, OperationOutcome,
    ScenarioGroundTruth, SessionValidity, TELEMETRY_SCHEMA_VERSION, TaskOutcome, TestSuiteOutcome,
    TokenUsage, ToolCallRecord, UndefinedReason, calculate_boundary_obedience,
    calculate_change_impact_accuracy, calculate_change_impact_metrics,
    calculate_completion_token_savings, calculate_disaggregated_file_metrics,
    calculate_file_creation_metrics, calculate_file_deletion_metrics, calculate_file_edit_metrics,
    calculate_file_read_discovery_metrics, calculate_memory_compounding,
    calculate_prompt_token_savings, calculate_retrieval_metrics, calculate_task_evaluation,
    calculate_token_savings, compare_matched_pair, normalize_file_path,
};

fn sample_datetime(sec: i64) -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(1700000000 + sec, 0).unwrap()
}

fn create_valid_session(
    task_id: &str,
    arm: ExperimentalArm,
    start_sec: i64,
    end_sec: i64,
    prompt_tokens: u64,
    completion_tokens: u64,
    turns: usize,
) -> AgentSessionTelemetry {
    let start_time = sample_datetime(start_sec);
    let end_time = sample_datetime(end_sec);
    let duration_ms = (end_sec - start_sec) as u64 * 1000;

    AgentSessionTelemetry {
        schema_version: TELEMETRY_SCHEMA_VERSION.to_string(),
        session_id: format!("sess-{}-{}", task_id, arm),
        task_id: task_id.to_string(),
        arm,
        agent_model: "claude-3-5-sonnet-20241022".to_string(),
        validity: SessionValidity::Valid,
        start_time,
        end_time,
        stated_duration_ms: Some(duration_ms),
        turns_count: turns,
        token_usage: TokenUsage::from_parts(prompt_tokens, completion_tokens),
        tool_calls: Vec::new(),
        file_interactions: FileInteractions::default(),
        governed_operations: Vec::new(),
        boundary_violations: Vec::new(),
        test_suite_outcome: Some(TestSuiteOutcome {
            total_tests: 20,
            passed_tests: 20,
            failed_tests: 0,
            test_names_broken: Vec::new(),
            unrelated_tests_broken: Vec::new(),
        }),
        task_outcome: TaskOutcome::Passed,
    }
}

#[test]
fn test_schema_serialization_roundtrip() {
    let mut session = create_valid_session(
        "task-roundtrip",
        ExperimentalArm::Treatment,
        0,
        12,
        1500,
        350,
        4,
    );
    session.tool_calls.push(ToolCallRecord {
        call_id: "call-1".to_string(),
        tool_name: "engram_context".to_string(),
        parameters: serde_json::json!({"symbol": "CustomerLedger"}),
        timestamp: sample_datetime(1),
        duration_ms: 120,
        is_success: true,
        error_message: None,
    });
    session.file_interactions.files_read = vec!["src/billing/mod.rs".to_string()];
    session.file_interactions.files_edited = vec!["src/billing/calc.rs".to_string()];
    session.file_interactions.files_created = vec!["src/billing/tax.rs".to_string()];
    session.governed_operations.push(GovernedOperation {
        operation_id: "op-1".to_string(),
        kind: OperationKind::FileWrite,
        target: "src/billing/calc.rs".to_string(),
        is_governed: true,
        is_authorized: true,
        outcome: OperationOutcome::Success,
    });
    session.boundary_violations.push(BoundaryViolationRecord {
        timestamp: sample_datetime(5),
        path_or_target: "secrets/api.key".to_string(),
        operation_kind: OperationKind::FileRead,
        rule_or_contract_id: "sec-01".to_string(),
        blocked: true,
        details: "Blocked attempt to read sensitive key".to_string(),
    });

    let json = session.to_json().expect("Serialization must succeed");
    assert!(json.contains("\"schema_version\": \"1.0.0\""));
    assert!(json.contains("\"arm\": \"Treatment\""));

    let deserialized =
        AgentSessionTelemetry::from_json(&json).expect("Deserialization must succeed");
    assert_eq!(session, deserialized);
}

#[test]
fn test_temporal_consistency_validation() {
    let mut session =
        create_valid_session("task-time", ExperimentalArm::Control, 10, 20, 1000, 200, 2);

    // Exact match: 10s = 10,000ms
    assert_eq!(session.derived_duration_ms().unwrap(), 10000);
    assert!(session.validate_consistency(50).is_ok());

    // Within tolerance
    session.stated_duration_ms = Some(10040);
    assert!(session.validate_consistency(50).is_ok());

    // Outside tolerance
    session.stated_duration_ms = Some(10080);
    let err = session.validate_consistency(50).unwrap_err();
    match err {
        ConsistencyError::DurationMismatch {
            stated_ms,
            derived_ms,
            diff_ms,
            tolerance_ms,
        } => {
            assert_eq!(stated_ms, 10080);
            assert_eq!(derived_ms, 10000);
            assert_eq!(diff_ms, 80);
            assert_eq!(tolerance_ms, 50);
        }
        _ => panic!("Expected DurationMismatch"),
    }

    // Inverted timestamps (end before start)
    session.start_time = sample_datetime(30);
    session.end_time = sample_datetime(20);
    let inv_err = session.validate_consistency(50).unwrap_err();
    assert!(matches!(
        inv_err,
        ConsistencyError::InvertedTimestamps { .. }
    ));
}

#[test]
fn test_token_usage_consistency() {
    let valid_tokens = TokenUsage::from_parts(850, 150);
    assert_eq!(valid_tokens.total_tokens, 1000);
    assert!(valid_tokens.validate_consistency().is_ok());

    let invalid_tokens = TokenUsage {
        prompt_tokens: 500,
        completion_tokens: 200,
        total_tokens: 999, // Mismatch
        cached_input_tokens: None,
        cache_creation_input_tokens: None,
    };
    let err = invalid_tokens.validate_consistency().unwrap_err();
    match err {
        ConsistencyError::TokenSumMismatch {
            prompt,
            completion,
            stated_total,
        } => {
            assert_eq!(prompt, 500);
            assert_eq!(completion, 200);
            assert_eq!(stated_total, 999);
        }
        _ => panic!("Expected TokenSumMismatch"),
    }

    // Checked arithmetic & overflow prevention (Finding C)
    let checked = TokenUsage::checked_from_parts(1000, 250).expect("Safe sum should succeed");
    assert_eq!(checked.total_tokens, 1250);

    let overflow_err = TokenUsage::checked_from_parts(u64::MAX, 1).unwrap_err();
    assert_eq!(
        overflow_err,
        ConsistencyError::TokenOverflow {
            prompt: u64::MAX,
            completion: 1,
        }
    );

    // Prompt-cache token accounting (Finding B)
    // In modern LLM APIs (Anthropic, OpenAI, DeepSeek), cached tokens are a subset of prompt tokens.
    // prompt_tokens includes all input tokens, total_tokens = prompt_tokens + completion_tokens.
    let cache_usage = TokenUsage::checked_with_cache(5000, 1000, Some(3500), Some(500))
        .expect("Valid cache usage within prompt tokens bounds");
    assert_eq!(cache_usage.prompt_tokens, 5000);
    assert_eq!(cache_usage.completion_tokens, 1000);
    assert_eq!(cache_usage.total_tokens, 6000); // 5000 + 1000, strictly invariant!
    assert_eq!(cache_usage.cached_input_tokens, Some(3500));
    assert_eq!(cache_usage.cache_creation_input_tokens, Some(500));
    assert!(cache_usage.validate_consistency().is_ok());

    // Invariant violation: cached_input_tokens exceeds prompt_tokens
    let cache_exceeds = TokenUsage::checked_with_cache(1000, 500, Some(1500), None).unwrap_err();
    assert_eq!(
        cache_exceeds,
        ConsistencyError::CachedTokensExceedPrompt {
            cached: 1500,
            prompt: 1000,
        }
    );

    // Invariant violation: cache_creation_input_tokens exceeds prompt_tokens
    let creation_exceeds = TokenUsage::checked_with_cache(1000, 500, None, Some(1200)).unwrap_err();
    assert_eq!(
        creation_exceeds,
        ConsistencyError::CachedTokensExceedPrompt {
            cached: 1200,
            prompt: 1000,
        }
    );

    // validate_consistency also catches manually constructed invalid cache tokens
    let invalid_cache_struct = TokenUsage {
        prompt_tokens: 1000,
        completion_tokens: 500,
        total_tokens: 1500,
        cached_input_tokens: Some(2000), // > 1000
        cache_creation_input_tokens: None,
    };
    assert_eq!(
        invalid_cache_struct.validate_consistency().unwrap_err(),
        ConsistencyError::CachedTokensExceedPrompt {
            cached: 2000,
            prompt: 1000,
        }
    );

    // Serde backwards compatibility: JSON without cache fields deserializes with None
    let json_legacy = r#"{"prompt_tokens":100,"completion_tokens":50,"total_tokens":150}"#;
    let deserialized_legacy: TokenUsage = serde_json::from_str(json_legacy).unwrap();
    assert_eq!(deserialized_legacy.cached_input_tokens, None);
    assert_eq!(deserialized_legacy.cache_creation_input_tokens, None);

    // Serde serialization omits None cache fields
    let serialized_none = serde_json::to_string(&valid_tokens).unwrap();
    assert!(!serialized_none.contains("cached_input_tokens"));

    // Serde roundtrip with cache fields
    let serialized_cache = serde_json::to_string(&cache_usage).unwrap();
    assert!(serialized_cache.contains("\"cached_input_tokens\":3500"));
    let deserialized_cache: TokenUsage = serde_json::from_str(&serialized_cache).unwrap();
    assert_eq!(deserialized_cache, cache_usage);
}

#[test]
fn test_session_validity_distinction() {
    let mut valid_session =
        create_valid_session("task-val", ExperimentalArm::Control, 0, 5, 1000, 100, 1);
    valid_session.task_outcome = TaskOutcome::Failed {
        failure_reason: "AssertionError: expected 42, got 0".to_string(),
    };
    // Even though the task failed tests, the session completed its full interaction lifecycle normally
    assert!(valid_session.is_valid());

    let mut incomplete_session =
        create_valid_session("task-val", ExperimentalArm::Treatment, 0, 5, 500, 50, 1);
    incomplete_session.validity = SessionValidity::Incomplete(IncompleteReason::Timeout);
    assert!(!incomplete_session.is_valid());

    let mut infra_failed_session =
        create_valid_session("task-val", ExperimentalArm::Treatment, 0, 5, 500, 50, 1);
    infra_failed_session.validity =
        SessionValidity::InfrastructureFailure(InfrastructureFailureReason::RunnerAllocationFailed);
    assert!(!infra_failed_session.is_valid());

    // When calculating token savings with an incomplete session, it safely returns Undefined
    let savings_incomplete = calculate_token_savings(&valid_session, &incomplete_session);
    assert_eq!(
        savings_incomplete,
        MetricResult::Undefined(UndefinedReason::SessionIncomplete)
    );

    let savings_infra = calculate_token_savings(&infra_failed_session, &valid_session);
    assert_eq!(
        savings_infra,
        MetricResult::Undefined(UndefinedReason::SessionIncomplete)
    );
}

#[test]
fn test_token_savings_pure_calculation() {
    let control = create_valid_session("task-tok", ExperimentalArm::Control, 0, 10, 8000, 2000, 5); // total = 10,000
    let treatment =
        create_valid_session("task-tok", ExperimentalArm::Treatment, 0, 4, 1600, 400, 2); // total = 2,000

    // Overall token savings: 1 - (2000 / 10000) = 0.80 (80%)
    let total_savings = calculate_token_savings(&control, &treatment);
    assert_eq!(total_savings, MetricResult::Defined(0.80));

    // Prompt token savings: 1 - (1600 / 8000) = 0.80
    let prompt_savings = calculate_prompt_token_savings(&control, &treatment);
    assert_eq!(prompt_savings, MetricResult::Defined(0.80));

    // Completion token savings: 1 - (400 / 2000) = 0.80
    let comp_savings = calculate_completion_token_savings(&control, &treatment);
    assert_eq!(comp_savings, MetricResult::Defined(0.80));

    // Zero tokens in baseline control
    let zero_control = create_valid_session("task-tok", ExperimentalArm::Control, 0, 10, 0, 0, 0);
    let zero_savings = calculate_token_savings(&zero_control, &treatment);
    assert_eq!(
        zero_savings,
        MetricResult::Undefined(UndefinedReason::ZeroDenominator)
    );

    // Negative savings (treatment used more tokens than control)
    let heavy_treatment = create_valid_session(
        "task-tok",
        ExperimentalArm::Treatment,
        0,
        15,
        12000,
        3000,
        7,
    ); // total = 15,000
    let neg_savings = calculate_token_savings(&control, &heavy_treatment);
    assert_eq!(neg_savings, MetricResult::Defined(-0.50)); // 1 - 1.5 = -0.5
}

#[test]
fn test_disaggregated_file_metrics_separation() {
    let ground_truth = ScenarioGroundTruth {
        scenario_id: "scen-auth".to_string(),
        relevant_files_to_read: vec!["src/auth/jwt.rs".to_string(), "src/auth/mod.rs".to_string()],
        expected_files_to_modify: vec!["src/auth/jwt.rs".to_string()],
        expected_files_to_create: vec!["src/auth/claims.rs".to_string()],
        expected_files_to_delete: vec!["src/auth/legacy.rs".to_string()],
        impacted_symbols_or_files: vec![],
        forbidden_boundaries: vec![],
    };

    let mut session =
        create_valid_session("scen-auth", ExperimentalArm::Control, 0, 10, 1000, 200, 3);
    // Agent read 5 files (poor discovery precision: 2 relevant out of 5)
    // Note: includes Windows backslashes and duplicate entries to test normalization and deduplication!
    session.file_interactions.files_read = vec![
        r"src\auth\jwt.rs".to_string(),
        "./src/auth/jwt.rs".to_string(), // Duplicate
        "src/auth/mod.rs".to_string(),
        "src/main.rs".to_string(),
        "src/lib.rs".to_string(),
        "README.md".to_string(),
    ];
    // Agent edited exactly the 1 required file (perfect edit precision & recall)
    session.file_interactions.files_edited = vec!["src/auth/jwt.rs".to_string()];
    // Agent created exactly the 1 required file
    session.file_interactions.files_created = vec![r"src\auth\claims.rs".to_string()];
    // Agent deleted exactly the 1 required file
    session.file_interactions.files_deleted = vec!["src/auth/legacy.rs".to_string()];

    let disaggregated = calculate_disaggregated_file_metrics(&session, &ground_truth);

    // Read discovery:
    // observed unique: 5 ("src/auth/jwt.rs", "src/auth/mod.rs", "src/main.rs", "src/lib.rs", "README.md")
    // expected: 2
    // tp = 2, fp = 3, fn = 0
    let read_m = disaggregated.read_discovery;
    assert_eq!(read_m.true_positives, 2);
    assert_eq!(read_m.false_positives, 3);
    assert_eq!(read_m.false_negatives, 0);
    assert_eq!(read_m.precision, MetricResult::Defined(2.0 / 5.0)); // 0.40
    assert_eq!(read_m.recall, MetricResult::Defined(1.0)); // 1.00
    let expected_f1 = 2.0 * (0.4 * 1.0) / (0.4 + 1.0); // 0.8 / 1.4 ≈ 0.57142857
    assert!((read_m.f1_score.value().unwrap() - expected_f1).abs() < 1e-6);

    // File edits:
    // tp = 1, fp = 0, fn = 0 -> Precision = 1.0, Recall = 1.0, F1 = 1.0
    let edit_m = disaggregated.file_edits;
    assert_eq!(edit_m.precision, MetricResult::Defined(1.0));
    assert_eq!(edit_m.recall, MetricResult::Defined(1.0));
    assert_eq!(edit_m.f1_score, MetricResult::Defined(1.0));

    // File creations:
    let create_m = disaggregated.file_creations;
    assert_eq!(create_m.precision, MetricResult::Defined(1.0));
    assert_eq!(create_m.recall, MetricResult::Defined(1.0));

    // File deletions:
    let delete_m = disaggregated.file_deletions;
    assert_eq!(delete_m.precision, MetricResult::Defined(1.0));
    assert_eq!(delete_m.recall, MetricResult::Defined(1.0));

    // CRITICAL SCIENTIFIC INVARIANT: Read discovery precision (0.40) is NOT obscured by perfect edit precision (1.00)!
    assert_ne!(read_m.precision, edit_m.precision);
}

#[test]
fn test_retrieval_edge_cases() {
    // Empty observations
    let empty_obs = calculate_retrieval_metrics(&[], &["src/a.rs".to_string()]);
    assert_eq!(
        empty_obs.precision,
        MetricResult::Undefined(UndefinedReason::EmptyObservations)
    );
    assert_eq!(empty_obs.recall, MetricResult::Defined(0.0));
    assert_eq!(
        empty_obs.f1_score,
        MetricResult::Undefined(UndefinedReason::EmptyObservations)
    );

    // Empty ground truth
    let empty_exp = calculate_retrieval_metrics(&["src/a.rs".to_string()], &[]);
    assert_eq!(empty_exp.precision, MetricResult::Defined(0.0));
    assert_eq!(
        empty_exp.recall,
        MetricResult::Undefined(UndefinedReason::EmptyGroundTruth)
    );
    assert_eq!(
        empty_exp.f1_score,
        MetricResult::Undefined(UndefinedReason::EmptyGroundTruth)
    );

    // Completely disjoint
    let disjoint =
        calculate_retrieval_metrics(&["src/a.rs".to_string()], &["src/b.rs".to_string()]);
    assert_eq!(disjoint.precision, MetricResult::Defined(0.0));
    assert_eq!(disjoint.recall, MetricResult::Defined(0.0));
    assert_eq!(disjoint.f1_score, MetricResult::Defined(0.0));
}

#[test]
fn test_boundary_governance_and_enforcement_rates() {
    let operations = vec![
        // Op 1: Compliant Read
        GovernedOperation {
            operation_id: "op-1".to_string(),
            kind: OperationKind::FileRead,
            target: "src/models/user.rs".to_string(),
            is_governed: true,
            is_authorized: true,
            outcome: OperationOutcome::Success,
        },
        // Op 2: Blocked Read Violation (Read unauthorized boundary)
        GovernedOperation {
            operation_id: "op-2".to_string(),
            kind: OperationKind::FileRead,
            target: "secrets/master.key".to_string(),
            is_governed: true,
            is_authorized: false,
            outcome: OperationOutcome::BlockedByPolicy {
                reason: "Access to secrets/ is denied by policy".to_string(),
                rule_or_contract_id: Some("SEC-01".to_string()),
            },
        },
        // Op 3: Compliant Write
        GovernedOperation {
            operation_id: "op-3".to_string(),
            kind: OperationKind::FileWrite,
            target: "src/models/user.rs".to_string(),
            is_governed: true,
            is_authorized: true,
            outcome: OperationOutcome::Success,
        },
        // Op 4: Blocked Write Violation (Mutation unauthorized boundary)
        GovernedOperation {
            operation_id: "op-4".to_string(),
            kind: OperationKind::FileWrite,
            target: "db/migrations/001.sql".to_string(),
            is_governed: true,
            is_authorized: false,
            outcome: OperationOutcome::BlockedByPolicy {
                reason: "Direct migration edits prohibited".to_string(),
                rule_or_contract_id: Some("DB-01".to_string()),
            },
        },
        // Op 5: Unauthorized Mutation Leak (Attempt was NOT blocked and reached state!)
        GovernedOperation {
            operation_id: "op-5".to_string(),
            kind: OperationKind::FileDelete,
            target: "legacy/auth.rs".to_string(),
            is_governed: true,
            is_authorized: false,
            outcome: OperationOutcome::Success,
        },
        // Op 6: Non-governed operation (should not enter governed denominator)
        GovernedOperation {
            operation_id: "op-6".to_string(),
            kind: OperationKind::FileRead,
            target: "README.md".to_string(),
            is_governed: false,
            is_authorized: true,
            outcome: OperationOutcome::Success,
        },
    ];

    let ob = calculate_boundary_obedience(&operations);

    // Governed ops in scope: ops 1, 2, 3, 4, 5 (total = 5)
    assert_eq!(ob.total_governed_operations, 5);
    // Compliant ops: ops 1, 3 (total = 2)
    assert_eq!(ob.compliant_operations, 2);
    // Unauthorized attempts: ops 2, 4, 5 (total = 3)
    assert_eq!(ob.unauthorized_attempts, 3);
    // Blocked violations: ops 2, 4 (total = 2)
    assert_eq!(ob.blocked_violations, 2);
    // Unauthorized leaks: op 5 (total = 1)
    assert_eq!(ob.unauthorized_leaks, 1);

    // Obedience rate = 2 compliant / 5 governed = 0.40
    assert_eq!(ob.operation_obedience_rate, MetricResult::Defined(0.40));

    // Enforcement effectiveness rate = 2 blocked / 3 unauthorized attempts ≈ 0.66666667
    assert!((ob.enforcement_effectiveness_rate.value().unwrap() - (2.0 / 3.0)).abs() < 1e-6);

    // Read vs Mutation breakdown:
    // Read: 2 governed (op 1, 2), 1 attempted violation (op 2), 1 blocked (op 2)
    assert_eq!(ob.read_governed_operations, 2);
    assert_eq!(ob.read_violations_attempted, 1);
    assert_eq!(ob.read_violations_blocked, 1);
    assert_eq!(
        ob.read_enforcement_effectiveness,
        MetricResult::Defined(1.0)
    ); // 100% blocked

    // Mutation: 3 governed (ops 3, 4, 5), 2 attempted (ops 4, 5), 1 blocked (op 4)
    assert_eq!(ob.mutation_governed_operations, 3);
    assert_eq!(ob.mutation_violations_attempted, 2);
    assert_eq!(ob.mutation_violations_blocked, 1);
    assert_eq!(
        ob.mutation_enforcement_effectiveness,
        MetricResult::Defined(0.50)
    ); // 50% blocked

    // Test zero unauthorized attempts case (agent fully compliant):
    let fully_compliant = vec![GovernedOperation {
        operation_id: "op-c1".to_string(),
        kind: OperationKind::FileWrite,
        target: "src/app.rs".to_string(),
        is_governed: true,
        is_authorized: true,
        outcome: OperationOutcome::Success,
    }];
    let fc_ob = calculate_boundary_obedience(&fully_compliant);
    assert_eq!(fc_ob.operation_obedience_rate, MetricResult::Defined(1.0));
    // When no attacks occur, enforcement effectiveness rate is mathematically undefined (0/0), NOT 0% or 100%!
    assert_eq!(
        fc_ob.enforcement_effectiveness_rate,
        MetricResult::Undefined(UndefinedReason::ZeroDenominator)
    );
}

#[test]
fn test_change_impact_blast_radius_accuracy() {
    let impacted_nodes = vec![
        "BillingEngine::compute_tax".to_string(),
        "InvoiceGenerator::format".to_string(),
        "PaymentService::authorize".to_string(),
        "tests::test_tax_calculation".to_string(),
    ];

    // Agent adapted 3 nodes, 2 of which were in the true impacted set (1 FP)
    let adapted_nodes = vec![
        "BillingEngine::compute_tax".to_string(),
        "InvoiceGenerator::format".to_string(),
        "UnrelatedComponent::cache".to_string(),
    ];

    let accuracy = calculate_change_impact_accuracy(&adapted_nodes, &impacted_nodes);
    // Recall = 2 / 4 = 0.50 (50%)
    assert_eq!(accuracy, MetricResult::Defined(0.50));

    // Full ChangeImpactMetrics validation (Finding A: Precision, Recall, and F1)
    let metrics: ChangeImpactMetrics =
        calculate_change_impact_metrics(&adapted_nodes, &impacted_nodes);
    assert_eq!(metrics.true_positives, 2);
    assert_eq!(metrics.false_positives, 1);
    assert_eq!(metrics.false_negatives, 2);
    // Precision = 2 / (2 + 1) = 2/3 ≈ 0.6666667
    assert!((metrics.precision.value().unwrap() - (2.0 / 3.0)).abs() < 1e-6);
    // Recall = 2 / (2 + 2) = 0.50
    assert_eq!(metrics.recall, MetricResult::Defined(0.50));
    // F1 = 2 * (2/3 * 0.5) / (2/3 + 0.5) = (2/3) / (7/6) = 4/7 ≈ 0.5714286
    assert!((metrics.f1_score.value().unwrap() - (4.0 / 7.0)).abs() < 1e-6);

    // Exact match case: perfect precision, recall, and F1
    let exact_match = impacted_nodes.clone();
    let exact_metrics = calculate_change_impact_metrics(&exact_match, &impacted_nodes);
    assert_eq!(exact_metrics.true_positives, 4);
    assert_eq!(exact_metrics.false_positives, 0);
    assert_eq!(exact_metrics.false_negatives, 0);
    assert_eq!(exact_metrics.precision, MetricResult::Defined(1.0));
    assert_eq!(exact_metrics.recall, MetricResult::Defined(1.0));
    assert_eq!(exact_metrics.f1_score, MetricResult::Defined(1.0));

    // Over-adaptation case: Agent edits everything (including 6 unrelated nodes)
    // Recall is 1.0 (100%), but precision is severely penalized!
    let mut over_adapted = impacted_nodes.clone();
    for i in 0..6 {
        over_adapted.push(format!("UnrelatedModule::symbol_{}", i));
    }
    let over_metrics = calculate_change_impact_metrics(&over_adapted, &impacted_nodes);
    assert_eq!(over_metrics.true_positives, 4);
    assert_eq!(over_metrics.false_positives, 6);
    assert_eq!(over_metrics.false_negatives, 0);
    assert_eq!(over_metrics.recall, MetricResult::Defined(1.0)); // Perfect recall
    assert_eq!(over_metrics.precision, MetricResult::Defined(4.0 / 10.0)); // 0.40 precision penalized!
    // F1 = 2 * (0.4 * 1.0) / (0.4 + 1.0) = 0.8 / 1.4 = 4/7 ≈ 0.5714286
    assert!((over_metrics.f1_score.value().unwrap() - (4.0 / 7.0)).abs() < 1e-6);

    // Duplicate observations deduplication test
    let with_duplicates = vec![
        "BillingEngine::compute_tax".to_string(),
        "BillingEngine::compute_tax".to_string(), // Duplicate
        "InvoiceGenerator::format".to_string(),
    ];
    let dup_metrics = calculate_change_impact_metrics(&with_duplicates, &impacted_nodes);
    assert_eq!(dup_metrics.true_positives, 2);
    assert_eq!(dup_metrics.false_positives, 0);
    assert_eq!(dup_metrics.false_negatives, 2);
    assert_eq!(dup_metrics.precision, MetricResult::Defined(1.0));
    assert_eq!(dup_metrics.recall, MetricResult::Defined(0.50));

    // Empty impacted ground truth
    let empty_accuracy = calculate_change_impact_accuracy(&adapted_nodes, &[]);
    assert_eq!(
        empty_accuracy,
        MetricResult::Undefined(UndefinedReason::EmptyGroundTruth)
    );
    let empty_gt_metrics = calculate_change_impact_metrics(&adapted_nodes, &[]);
    assert_eq!(
        empty_gt_metrics.recall,
        MetricResult::Undefined(UndefinedReason::EmptyGroundTruth)
    );
    assert_eq!(
        empty_gt_metrics.f1_score,
        MetricResult::Undefined(UndefinedReason::EmptyGroundTruth)
    );

    // Empty observed adaptations
    let empty_obs_metrics = calculate_change_impact_metrics(&[], &impacted_nodes);
    assert_eq!(
        empty_obs_metrics.precision,
        MetricResult::Undefined(UndefinedReason::EmptyObservations)
    );
    assert_eq!(empty_obs_metrics.recall, MetricResult::Defined(0.0));
    assert_eq!(
        empty_obs_metrics.f1_score,
        MetricResult::Undefined(UndefinedReason::EmptyObservations)
    );
}

#[test]
fn test_task_evaluation_and_regression_rate() {
    let mut session =
        create_valid_session("task-reg", ExperimentalArm::Control, 0, 10, 1000, 100, 2);
    session.test_suite_outcome = Some(TestSuiteOutcome {
        total_tests: 100,
        passed_tests: 95,
        failed_tests: 5,
        test_names_broken: vec!["test_auth_flow".to_string()],
        unrelated_tests_broken: vec![
            "test_orders_a".to_string(),
            "test_orders_b".to_string(),
            "test_orders_c".to_string(),
            "test_orders_d".to_string(),
            "test_orders_e".to_string(),
        ],
    });
    session.task_outcome = TaskOutcome::RegressionDetected {
        unrelated_broken_count: 5,
        broken_tests: vec!["test_orders_a".to_string()],
    };

    let eval = calculate_task_evaluation(&session);
    assert!(!eval.task_passed);
    assert!(eval.regression_detected);
    assert_eq!(eval.unrelated_tests_broken, 5);
    assert_eq!(eval.total_existing_tests, 100);
    // Regression rate = 5 / 100 = 0.05 (5%)
    assert_eq!(eval.regression_rate, MetricResult::Defined(0.05));
    assert_eq!(eval.duration_ms, MetricResult::Defined(10000));
}

#[test]
fn test_memory_compounding_two_turn() {
    // Case A: Control failed Turn 2, Treatment passed Turn 2 (Clean Lift)
    let comp_a = calculate_memory_compounding(false, true, false, true);
    assert_eq!(comp_a.absolute_pass_lift, 1.0);
    assert_eq!(
        comp_a.relative_pass_lift,
        MetricResult::Undefined(UndefinedReason::ZeroDenominator)
    );

    // Case B: Both passed Turn 2
    let comp_b = calculate_memory_compounding(true, true, true, true);
    assert_eq!(comp_b.absolute_pass_lift, 0.0);
    assert_eq!(comp_b.relative_pass_lift, MetricResult::Defined(0.0));

    // Case C: Control passed Turn 2, Treatment failed Turn 2 (Negative Lift)
    let comp_c = calculate_memory_compounding(true, false, true, false);
    assert_eq!(comp_c.absolute_pass_lift, -1.0);
    assert_eq!(comp_c.relative_pass_lift, MetricResult::Defined(-1.0));
}

#[test]
fn test_matched_pair_comparison_end_to_end() {
    let ground_truth = ScenarioGroundTruth {
        scenario_id: "task-monorepo-perf".to_string(),
        relevant_files_to_read: vec!["src/billing/calc.rs".to_string()],
        expected_files_to_modify: vec!["src/billing/calc.rs".to_string()],
        expected_files_to_create: vec![],
        expected_files_to_delete: vec![],
        impacted_symbols_or_files: vec!["src/billing/calc.rs".to_string()],
        forbidden_boundaries: vec!["secrets/**".to_string()],
    };

    // Control: 10,000 tokens, 10 turns, 60s, read 10 files (1 relevant), edited 2 files (1 relevant, 1 over-adaptation), 1 violation attempt (0 blocked, 1 leak)
    let mut control = create_valid_session(
        "task-monorepo-perf",
        ExperimentalArm::Control,
        0,
        60,
        8000,
        2000,
        10,
    );
    control.file_interactions.files_read = (0..10).map(|i| format!("src/file_{}.rs", i)).collect();
    control
        .file_interactions
        .files_read
        .push("src/billing/calc.rs".to_string());
    control.file_interactions.files_edited = vec![
        "src/billing/calc.rs".to_string(),
        "src/billing/unrelated.rs".to_string(), // Over-adaptation
    ];
    control.governed_operations = vec![
        GovernedOperation {
            operation_id: "c-op1".to_string(),
            kind: OperationKind::FileWrite,
            target: "src/billing/calc.rs".to_string(),
            is_governed: true,
            is_authorized: true,
            outcome: OperationOutcome::Success,
        },
        GovernedOperation {
            operation_id: "c-op2".to_string(),
            kind: OperationKind::FileRead,
            target: "secrets/jwt.key".to_string(),
            is_governed: true,
            is_authorized: false,
            outcome: OperationOutcome::Success, // Leaked! Control had no guard.
        },
    ];

    // Treatment: 2,000 tokens, 2 turns, 15s, read 1 file (1 relevant), edited 1 file (exact), 1 violation attempt (1 blocked, 0 leaks)
    let mut treatment = create_valid_session(
        "task-monorepo-perf",
        ExperimentalArm::Treatment,
        0,
        15,
        1600,
        400,
        2,
    );
    treatment.file_interactions.files_read = vec!["src/billing/calc.rs".to_string()];
    treatment.file_interactions.files_edited = vec!["src/billing/calc.rs".to_string()];
    treatment.governed_operations = vec![
        GovernedOperation {
            operation_id: "t-op1".to_string(),
            kind: OperationKind::FileWrite,
            target: "src/billing/calc.rs".to_string(),
            is_governed: true,
            is_authorized: true,
            outcome: OperationOutcome::Success,
        },
        GovernedOperation {
            operation_id: "t-op2".to_string(),
            kind: OperationKind::FileRead,
            target: "secrets/jwt.key".to_string(),
            is_governed: true,
            is_authorized: false,
            outcome: OperationOutcome::BlockedByPolicy {
                reason: "Access to secrets/ denied".to_string(),
                rule_or_contract_id: Some("SEC-01".to_string()),
            }, // Blocked!
        },
    ];

    let comparison = compare_matched_pair(&control, &treatment, &ground_truth)
        .expect("Comparison should succeed");

    // Token savings: 80%
    assert_eq!(comparison.token_savings, MetricResult::Defined(0.80));
    assert_eq!(comparison.prompt_token_savings, MetricResult::Defined(0.80));
    assert_eq!(
        comparison.completion_token_savings,
        MetricResult::Defined(0.80)
    );

    // Turn count delta: 2 - 10 = -8 turns
    assert_eq!(comparison.turn_count_delta, -8);

    // Duration delta: 15s - 60s = -45,000ms
    assert_eq!(comparison.duration_delta_ms, MetricResult::Defined(-45000));
    // Duration reduction rate: 1 - (15 / 60) = 0.75 (75% faster)
    assert_eq!(
        comparison.duration_reduction_rate,
        MetricResult::Defined(0.75)
    );

    // Read discovery F1 lift:
    // Treatment F1 = 1.0 (1 observed, 1 expected)
    // Control F1 = 2 * (1/11 * 1.0) / (1/11 + 1.0) = 2/12 = 0.1666667
    // Lift = 1.0 - 0.1666667 = 0.8333333
    let f1_lift = comparison.read_discovery_f1_lift.value().unwrap();
    assert!((f1_lift - (1.0 - (2.0 / 12.0))).abs() < 1e-6);

    // Edit F1 lift (Finding A & Refinement 2):
    // Treatment F1 = 1.0 (P=1.0, R=1.0)
    // Control F1 = 2/3 ≈ 0.6666667 (P=0.5, R=1.0 due to over-adaptation)
    // Lift = 1.0 - 2/3 = 1/3 ≈ 0.3333333
    let edit_lift = comparison.edit_f1_lift.value().unwrap();
    assert!((edit_lift - (1.0 - (2.0 / 3.0))).abs() < 1e-6);

    // Impact F1 lift (Finding A):
    // Ground truth impacted nodes = ["src/billing/calc.rs"]
    // Treatment F1 = 1.0 (P=1.0, R=1.0)
    // Control F1 = 2/3 ≈ 0.6666667 (P=0.5, R=1.0 due to over-adaptation)
    // Lift = 1.0 - 2/3 = 1/3 ≈ 0.3333333
    let impact_lift = comparison.impact_f1_lift.value().unwrap();
    assert!((impact_lift - (1.0 - (2.0 / 3.0))).abs() < 1e-6);

    // Enforcement effectiveness lift:
    // Control: 0 blocked / 1 attempt = 0.0
    // Treatment: 1 blocked / 1 attempt = 1.0
    // Lift = 1.0 - 0.0 = 1.0
    assert_eq!(
        comparison.enforcement_effectiveness_lift,
        MetricResult::Defined(1.0)
    );

    // Obedience rate lift:
    // Control: 1 compliant / 2 governed = 0.50
    // Treatment: 1 compliant / 2 governed = 0.50
    // Lift = 0.50 - 0.50 = 0.0
    assert_eq!(comparison.obedience_rate_lift, MetricResult::Defined(0.0));

    // Mismatched task IDs should return an error
    let mut mismatched = treatment.clone();
    mismatched.task_id = "different-task-id".to_string();
    let err = compare_matched_pair(&control, &mismatched, &ground_truth).unwrap_err();
    assert!(err.contains("Task ID mismatch"));
}

#[test]
fn test_disaggregated_helper_functions_and_normalization() {
    assert_eq!(normalize_file_path(r"src\foo\bar.rs"), "src/foo/bar.rs");
    assert_eq!(normalize_file_path("./src//foo/bar.rs"), "src/foo/bar.rs");

    let gt = ScenarioGroundTruth {
        scenario_id: "scen-helpers".to_string(),
        relevant_files_to_read: vec!["src/read.rs".to_string()],
        expected_files_to_modify: vec!["src/edit.rs".to_string()],
        expected_files_to_create: vec!["src/create.rs".to_string()],
        expected_files_to_delete: vec!["src/delete.rs".to_string()],
        impacted_symbols_or_files: vec![],
        forbidden_boundaries: vec![],
    };

    let mut session =
        create_valid_session("scen-helpers", ExperimentalArm::Treatment, 0, 5, 100, 50, 1);
    session.file_interactions.files_read = vec!["src/read.rs".to_string()];
    session.file_interactions.files_edited = vec!["src/edit.rs".to_string()];
    session.file_interactions.files_created = vec!["src/create.rs".to_string()];
    session.file_interactions.files_deleted = vec!["src/delete.rs".to_string()];

    let read_m = calculate_file_read_discovery_metrics(&session, &gt);
    assert_eq!(read_m.f1_score, MetricResult::Defined(1.0));

    let edit_m = calculate_file_edit_metrics(&session, &gt);
    assert_eq!(edit_m.f1_score, MetricResult::Defined(1.0));

    let create_m = calculate_file_creation_metrics(&session, &gt);
    assert_eq!(create_m.f1_score, MetricResult::Defined(1.0));

    let delete_m = calculate_file_deletion_metrics(&session, &gt);
    assert_eq!(delete_m.f1_score, MetricResult::Defined(1.0));

    let op_read = GovernedOperation {
        operation_id: "op-r".to_string(),
        kind: OperationKind::FileRead,
        target: "src/read.rs".to_string(),
        is_governed: true,
        is_authorized: true,
        outcome: OperationOutcome::Success,
    };
    assert_eq!(op_read.category(), MutationCategory::ReadOnly);

    let op_write = GovernedOperation {
        operation_id: "op-w".to_string(),
        kind: OperationKind::FileWrite,
        target: "src/edit.rs".to_string(),
        is_governed: true,
        is_authorized: true,
        outcome: OperationOutcome::Success,
    };
    assert_eq!(op_write.category(), MutationCategory::StateMutating);
}
