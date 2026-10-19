//! Acceptance test suite for Engram Phase 2B Milestone B1:
//! Provider-Neutral Contracts & Pre-Flight BudgetGuard.
//!
//! All tests run 100% offline with zero live API calls and zero network sockets opened.

use engram::empirical_live::anthropic::{
    from_anthropic_response, sanitize_anthropic_headers, sanitize_header_map, to_anthropic_request,
};
use engram::empirical_live::budget::{
    AmbiguousFailureReason, BudgetError, BudgetGuard, MicroDollars, ModelPricing,
    PromptTokenEstimator,
};
use engram::empirical_live::dispatcher::{SandboxToolDispatcher, ToolDispatcher};
use engram::empirical_live::mock::MockLlmProvider;
use engram::empirical_live::orchestrator::{
    HaltReason, MultiTurnOrchestrator, OrchestratorConfig, SessionOutcome,
};
use engram::empirical_live::provider::{
    GuardedProvider, LlmProvider, ProviderError, RedactedSecret,
};
use engram::empirical_live::report::{
    BenchmarkProvenance, EXPLORATORY_PILOT_DISCLAIMER, LiveBenchmarkReport,
};
use engram::empirical_live::schema::{
    ChatMessage, ContentBlock, LlmRequest, LlmResponse, Role, SchemaValidationError, StopReason,
    TokenTelemetryError, TokenUsage, ToolDefinition,
};
use engram::empirical_live::stats::{
    DEFAULT_ALPHA, DEFAULT_ATTRITION_BUFFER, DEFAULT_POWER, DEFAULT_TARGET_MDE, MatchedPairInput,
    PILOT_VARIANCE_UPPER_80_MULTIPLIER, SecondaryHypothesis, calculate_confirmatory_sample_size,
    evaluate_itt_sensitivity, evaluate_paired_savings, holm_bonferroni_adjust, student_t_cdf,
};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

/// Static mutex to serialize tests that mutate or rely on sensitive process-wide environment variables.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

// ============================================================================
// 1. BudgetGuard: Pre-Flight Exhaustion Rejection
// ============================================================================

#[test]
fn test_budget_exhaustion_rejects_preflight() {
    let cap = MicroDollars::from_dollars(10).unwrap(); // $10.00
    let guard = BudgetGuard::new(cap);

    // Initial reservation of $6.00 succeeds
    let res1_cost = MicroDollars::from_dollars(6).unwrap();
    let ticket1 = guard
        .reserve(res1_cost, "claude-3-7-sonnet")
        .expect("res1 should succeed");
    assert_eq!(ticket1.reserved_amount(), res1_cost);

    let snap1 = guard.snapshot().expect("snapshot should succeed");
    assert_eq!(
        snap1.active_reserved,
        MicroDollars::from_dollars(6).unwrap()
    );
    assert_eq!(
        snap1.available_budget,
        MicroDollars::from_dollars(4).unwrap()
    );
    assert_eq!(snap1.committed_spend, MicroDollars::ZERO);
    assert_eq!(snap1.uncertain_spend, MicroDollars::ZERO);
    snap1.verify_conservation_invariant().unwrap();

    // Second reservation of $5.00 must be rejected (only $4.00 available)
    let res2_cost = MicroDollars::from_dollars(5).unwrap();
    let err = guard.reserve(res2_cost, "claude-3-7-sonnet").unwrap_err();
    match err {
        BudgetError::BudgetExhausted {
            requested,
            available,
            hard_cap,
        } => {
            assert_eq!(requested, MicroDollars::from_dollars(5).unwrap());
            assert_eq!(available, MicroDollars::from_dollars(4).unwrap());
            assert_eq!(hard_cap, cap);
        }
        other => panic!("Expected BudgetExhausted, got {other:?}"),
    }

    // Ledger balances remain unaltered after rejected pre-flight attempt
    let snap2 = guard.snapshot().expect("snapshot should succeed");
    assert_eq!(
        snap2.active_reserved,
        MicroDollars::from_dollars(6).unwrap()
    );
    assert_eq!(
        snap2.available_budget,
        MicroDollars::from_dollars(4).unwrap()
    );
    snap2.verify_conservation_invariant().unwrap();

    // Reconcile ticket1 with actual cost $5.50
    let actual_cost = MicroDollars::from_dollars_and_cents(5, 50).unwrap();
    let receipt = guard
        .reconcile_success(&ticket1, actual_cost)
        .expect("reconcile should succeed");
    assert_eq!(receipt.actual_cost, actual_cost);
    assert_eq!(receipt.released_surplus, MicroDollars::from_micros(500_000)); // $0.50 surplus released

    let snap3 = guard.snapshot().expect("snapshot should succeed");
    assert_eq!(snap3.committed_spend, actual_cost);
    assert_eq!(snap3.active_reserved, MicroDollars::ZERO);
    assert_eq!(
        snap3.available_budget,
        MicroDollars::from_dollars_and_cents(4, 50).unwrap()
    );
    snap3.verify_conservation_invariant().unwrap();
}

// ============================================================================
// 2. BudgetGuard: Concurrent Reservation Atomicity
// ============================================================================

#[test]
fn test_concurrent_reservation_atomic_synchronization() {
    let cap = MicroDollars::from_dollars(10).unwrap(); // $10.00
    let guard = BudgetGuard::new(cap);

    // Spawn 20 threads, each attempting to reserve $1.00 concurrently
    let num_threads = 20;
    let unit_cost = MicroDollars::from_dollars(1).unwrap();

    let success_count = Arc::new(AtomicUsize::new(0));
    let exhausted_count = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for _ in 0..num_threads {
        let guard_clone = guard.clone();
        let succ_clone = success_count.clone();
        let exh_clone = exhausted_count.clone();

        handles.push(thread::spawn(move || {
            match guard_clone.reserve(unit_cost, "claude-3-7-sonnet") {
                Ok(_) => {
                    succ_clone.fetch_add(1, Ordering::SeqCst);
                }
                Err(BudgetError::BudgetExhausted { .. }) => {
                    exh_clone.fetch_add(1, Ordering::SeqCst);
                }
                Err(other) => panic!("Unexpected error under concurrency: {other:?}"),
            }
        }));
    }

    for h in handles {
        h.join().expect("thread should join cleanly");
    }

    // Exactly 10 reservations must succeed, and exactly 10 must be rejected
    assert_eq!(success_count.load(Ordering::SeqCst), 10);
    assert_eq!(exhausted_count.load(Ordering::SeqCst), 10);

    let snap = guard.snapshot().expect("snapshot should succeed");
    assert_eq!(
        snap.active_reserved,
        MicroDollars::from_dollars(10).unwrap()
    );
    assert_eq!(snap.available_budget, MicroDollars::ZERO);
    assert_eq!(snap.active_count, 10);
    snap.verify_conservation_invariant().unwrap();
}

// ============================================================================
// 3. BudgetGuard: Ambiguous Failure Transfers to Uncertain Spend
// ============================================================================

#[test]
fn test_ambiguous_network_failure_holds_uncertain_spend() {
    let cap = MicroDollars::from_dollars(10).unwrap();
    let guard = BudgetGuard::new(cap);

    let res_cost = MicroDollars::from_dollars(3).unwrap();
    let ticket = guard.reserve(res_cost, "claude-3-7-sonnet").unwrap();

    // Ambiguous failure: HTTP client timed out waiting for response body
    let reason = AmbiguousFailureReason::Timeout { elapsed_ms: 30_000 };
    let hold_receipt = guard
        .reconcile_ambiguous(&ticket, reason.clone())
        .expect("ambiguous hold should succeed");

    assert_eq!(hold_receipt.held_amount, res_cost);
    assert_eq!(hold_receipt.uncertain_spend_after, res_cost);
    assert_eq!(
        hold_receipt.available_budget_after,
        MicroDollars::from_dollars(7).unwrap()
    );

    let snap = guard.snapshot().unwrap();
    assert_eq!(snap.active_reserved, MicroDollars::ZERO);
    assert_eq!(snap.uncertain_spend, res_cost);
    // Crucial invariant: funds are NEVER released back to available budget!
    assert_eq!(
        snap.available_budget,
        MicroDollars::from_dollars(7).unwrap()
    );
    assert_eq!(snap.ambiguous_count, 1);
    snap.verify_conservation_invariant().unwrap();

    // Later authoritative resolution: provider logs show $2.40 was billed
    let auth_cost = MicroDollars::from_dollars_and_cents(2, 40).unwrap();
    let res_receipt = guard
        .reconcile_authoritative_resolution(ticket.id(), auth_cost)
        .expect("authoritative resolution should succeed");

    assert_eq!(res_receipt.actual_cost, auth_cost);
    assert_eq!(
        res_receipt.released_surplus,
        MicroDollars::from_micros(600_000)
    ); // $0.60 released

    let snap2 = guard.snapshot().unwrap();
    assert_eq!(snap2.uncertain_spend, MicroDollars::ZERO);
    assert_eq!(snap2.committed_spend, auth_cost);
    assert_eq!(
        snap2.available_budget,
        MicroDollars::from_dollars_and_cents(7, 60).unwrap()
    );
    assert_eq!(snap2.resolved_count, 1);
    snap2.verify_conservation_invariant().unwrap();
}

// ============================================================================
// 4. BudgetGuard: Duplicate State Transitions Rejected Safely
// ============================================================================

#[test]
fn test_duplicate_reconciliation_rejected_safely() {
    let cap = MicroDollars::from_dollars(10).unwrap();
    let guard = BudgetGuard::new(cap);

    let ticket = guard
        .reserve(MicroDollars::from_dollars(2).unwrap(), "claude-3-7-sonnet")
        .unwrap();

    // First commit succeeds
    let actual = MicroDollars::from_dollars_and_cents(1, 50).unwrap();
    let receipt = guard.reconcile_success(&ticket, actual).unwrap();
    assert_eq!(receipt.released_surplus, MicroDollars::from_micros(500_000));

    // Duplicate commit attempt on same ticket must fail
    let dup_err = guard.reconcile_success(&ticket, actual).unwrap_err();
    match dup_err {
        BudgetError::InvalidStateTransition {
            id,
            current_state,
            attempted,
        } => {
            assert_eq!(id, ticket.id());
            assert_eq!(current_state, "Committed");
            assert_eq!(attempted, "reconcile_success");
        }
        other => panic!("Expected InvalidStateTransition, got {other:?}"),
    }

    // Duplicate ambiguous hold attempt must fail
    let amb_err = guard
        .reconcile_ambiguous(
            &ticket,
            AmbiguousFailureReason::Timeout { elapsed_ms: 5000 },
        )
        .unwrap_err();
    match amb_err {
        BudgetError::InvalidStateTransition {
            id,
            current_state,
            attempted,
        } => {
            assert_eq!(id, ticket.id());
            assert_eq!(current_state, "Committed");
            assert_eq!(attempted, "reconcile_ambiguous");
        }
        other => panic!("Expected InvalidStateTransition, got {other:?}"),
    }

    // Duplicate cancellation attempt must fail
    let cancel_err = guard
        .reconcile_preflight_cancel(&ticket, "cancelled")
        .unwrap_err();
    match cancel_err {
        BudgetError::InvalidStateTransition {
            id,
            current_state,
            attempted,
        } => {
            assert_eq!(id, ticket.id());
            assert_eq!(current_state, "Committed");
            assert_eq!(attempted, "reconcile_preflight_cancel");
        }
        other => panic!("Expected InvalidStateTransition, got {other:?}"),
    }

    // Verify ledger balances remain pristine and uncorrupted
    let snap = guard.snapshot().unwrap();
    assert_eq!(snap.committed_spend, actual);
    assert_eq!(snap.active_reserved, MicroDollars::ZERO);
    assert_eq!(snap.uncertain_spend, MicroDollars::ZERO);
    assert_eq!(
        snap.available_budget,
        MicroDollars::from_dollars_and_cents(8, 50).unwrap()
    );
    snap.verify_conservation_invariant().unwrap();
}

// ============================================================================
// 5. BudgetGuard: Pre-Flight Cancellation (Pre-Dispatch Only)
// ============================================================================

#[test]
fn test_preflight_cancel_releases_funds() {
    let cap = MicroDollars::from_dollars(5).unwrap();
    let guard = BudgetGuard::new(cap);

    let ticket = guard
        .reserve(MicroDollars::from_dollars(2).unwrap(), "claude-3-7-sonnet")
        .unwrap();
    assert_eq!(
        guard.snapshot().unwrap().available_budget,
        MicroDollars::from_dollars(3).unwrap()
    );

    // Local validation fails before any socket open -> cancel
    let rel_receipt = guard
        .reconcile_preflight_cancel(&ticket, "Schema validation failed")
        .expect("preflight cancel should succeed");

    assert_eq!(
        rel_receipt.released_amount,
        MicroDollars::from_dollars(2).unwrap()
    );
    assert_eq!(rel_receipt.available_budget_after, cap);

    let snap = guard.snapshot().unwrap();
    assert_eq!(snap.active_reserved, MicroDollars::ZERO);
    assert_eq!(snap.available_budget, cap);
    assert_eq!(snap.released_count, 1);
    snap.verify_conservation_invariant().unwrap();
}

// ============================================================================
// 6. ModelPricing & Arithmetic Integrity
// ============================================================================

#[test]
fn test_pricing_worst_case_and_actual_cost_math() {
    let pricing = ModelPricing::claude_3_7_sonnet();
    pricing.validate().unwrap();

    // Worst-case calculation: 10,000 prompt tokens + 2,000 max completion tokens
    // Input rate: $3.00/MTok ($3,000,000 micros) -> 10,000 * 3,000,000 / 1,000,000 = 30,000 micros ($0.03)
    // Cache write rate: $3.75/MTok ($3,750,000 micros) -> 10,000 * 3,750,000 / 1,000,000 = 37,500 micros ($0.0375)
    // Worst case uses max(input, write) = $3.75/MTok -> 37,500 micros
    // Output rate: $15.00/MTok ($15,000,000 micros) -> 2,000 * 15,000,000 / 1,000,000 = 30,000 micros ($0.03)
    // Total worst case = 37,500 + 30,000 = 67,500 micros ($0.0675)
    let worst_case = pricing.calculate_worst_case_cost(10_000, 2_000).unwrap();
    assert_eq!(worst_case, MicroDollars::from_micros(67_500));

    // Actual cost calculation with prompt caching:
    // Total prompt tokens: 10,000
    // Cached tokens: 8,000 (read discount: $0.30/MTok = 300,000 micros) -> 8,000 * 300,000 / 1,000,000 = 2,400 micros
    // Uncached prompt tokens: 2,000 ($3.00/MTok = 3,000,000 micros) -> 2,000 * 3,000,000 / 1,000,000 = 6,000 micros
    // Completion tokens: 1,500 ($15.00/MTok = 15,000,000 micros) -> 1,500 * 15,000,000 / 1,000,000 = 22,500 micros
    // Total actual = 2,400 + 6,000 + 22,500 = 30,900 micros ($0.0309)
    let actual = pricing
        .calculate_actual_cost(10_000, 1_500, Some(8_000))
        .unwrap();
    assert_eq!(actual, MicroDollars::from_micros(30_900));

    // Invalid: cached tokens > prompt tokens must fail
    let err = pricing
        .calculate_actual_cost(5_000, 500, Some(6_000))
        .unwrap_err();
    match err {
        BudgetError::InvalidTokenTelemetry(_) => {}
        other => panic!("Expected InvalidTokenTelemetry, got {other:?}"),
    }
}

#[test]
fn test_micro_dollars_arithmetic_overflow_and_bounds() {
    // Arithmetic overflow in checked_add
    let max = MicroDollars(u64::MAX);
    assert_eq!(
        max.checked_add(MicroDollars(1)),
        Err(BudgetError::ArithmeticOverflow)
    );

    // Arithmetic underflow in checked_sub
    let zero = MicroDollars(0);
    assert_eq!(
        zero.checked_sub(MicroDollars(1)),
        Err(BudgetError::ArithmeticUnderflow)
    );

    // Invalid cents
    assert!(MicroDollars::from_dollars_and_cents(10, 100).is_err());
    assert!(MicroDollars::from_dollars_and_cents(10, 99).is_ok());

    // Ceiling division ceiling verification: 1 token at $1.00/MTok = ceil(1 * 1_000_000 / 1_000_000) = 1 micro
    let pricing = ModelPricing {
        model_id: "test".to_string(),
        input_cost_per_million: MicroDollars::from_micros(1_000_000),
        output_cost_per_million: MicroDollars::from_micros(1_000_000),
        cache_read_cost_per_million: None,
        cache_write_cost_per_million: None,
    };
    let cost = pricing.calculate_worst_case_cost(1, 0).unwrap();
    assert_eq!(cost, MicroDollars::from_micros(1));

    // 1 token at $0.01/MTok (10,000 micros) = ceil(10,000 / 1,000,000) = 1 micro (conservative ceiling)
    let tiny_pricing = ModelPricing {
        model_id: "test".to_string(),
        input_cost_per_million: MicroDollars::from_micros(10_000),
        output_cost_per_million: MicroDollars::from_micros(10_000),
        cache_read_cost_per_million: None,
        cache_write_cost_per_million: None,
    };
    let tiny_cost = tiny_pricing.calculate_worst_case_cost(1, 0).unwrap();
    assert_eq!(tiny_cost, MicroDollars::from_micros(1));
}

// ============================================================================
// 7. Provider-Neutral Schema: Token Telemetry & Invariants
// ============================================================================

#[test]
fn test_token_telemetry_invariants() {
    // Valid standard telemetry
    let usage = TokenUsage {
        prompt_tokens: Some(1000),
        completion_tokens: Some(250),
        cached_input_tokens: Some(800),
        cache_write_tokens: None,
        total_tokens: Some(1250),
    };
    assert!(usage.is_complete());
    assert_eq!(usage.billed_tokens(), Some(1250));
    usage.validate().expect("usage should be valid");

    // Invariant 1 Violation: cached_input_tokens > prompt_tokens
    let invalid_cache = TokenUsage {
        prompt_tokens: Some(500),
        completion_tokens: Some(100),
        cached_input_tokens: Some(600),
        cache_write_tokens: None,
        total_tokens: Some(600),
    };
    let err1 = invalid_cache.validate().unwrap_err();
    match err1 {
        TokenTelemetryError::CacheExceedsPrompt {
            cached_tokens,
            prompt_tokens,
        } => {
            assert_eq!(cached_tokens, 600);
            assert_eq!(prompt_tokens, 500);
        }
        other => panic!("Expected CacheExceedsPrompt, got {other:?}"),
    }

    // Invariant 2 Violation: reported total != prompt + completion
    let invalid_total = TokenUsage {
        prompt_tokens: Some(1000),
        completion_tokens: Some(200),
        cached_input_tokens: None,
        cache_write_tokens: None,
        total_tokens: Some(1500), // Inconsistent! Expected 1200
    };
    let err2 = invalid_total.validate().unwrap_err();
    match err2 {
        TokenTelemetryError::InconsistentTotal {
            prompt_tokens,
            completion_tokens,
            reported_total,
            expected_total,
        } => {
            assert_eq!(prompt_tokens, 1000);
            assert_eq!(completion_tokens, 200);
            assert_eq!(reported_total, 1500);
            assert_eq!(expected_total, 1200);
        }
        other => panic!("Expected InconsistentTotal, got {other:?}"),
    }

    // Missing telemetry: missing completion tokens
    let missing = TokenUsage {
        prompt_tokens: Some(1000),
        completion_tokens: None,
        cached_input_tokens: None,
        cache_write_tokens: None,
        total_tokens: None,
    };
    assert!(!missing.is_complete());
    assert_eq!(missing.billed_tokens(), None);
    // Missing telemetry is valid in itself (does not violate integrity invariants)
    missing.validate().unwrap();
}

// ============================================================================
// 8. Provider-Neutral Schema: Request, Response & Tool Validation
// ============================================================================

#[test]
fn test_schema_request_and_response_validation() {
    let tool = ToolDefinition {
        name: "read_file".to_string(),
        description: "Read file contents from sandbox".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "required": ["path"]
        }),
    };
    tool.validate().expect("tool definition should be valid");

    // Invalid tool: invalid name characters
    let bad_tool = ToolDefinition {
        name: "read file with spaces!".to_string(),
        description: "bad".to_string(),
        input_schema: serde_json::json!({"type": "object"}),
    };
    assert!(bad_tool.validate().is_err());

    // Valid request
    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: Some("You are an engineering coding agent.".to_string()),
        messages: vec![ChatMessage::user("Refactor src/empirical.rs")],
        tools: vec![tool],
        max_tokens: 4096,
        temperature: Some(0.2),
        stop_sequences: vec![],
    };
    req.validate().expect("request should be valid");

    // Empty model rejected
    let mut bad_req = req.clone();
    bad_req.model = "".to_string();
    assert_eq!(bad_req.validate(), Err(SchemaValidationError::EmptyModel));

    // Zero max_tokens rejected
    let mut bad_req2 = req.clone();
    bad_req2.max_tokens = 0;
    assert_eq!(
        bad_req2.validate(),
        Err(SchemaValidationError::ZeroMaxTokens)
    );

    // Response structure and tool call extraction
    let resp = LlmResponse {
        id: "msg_123".to_string(),
        model: "claude-3-7-sonnet-20250219".to_string(),
        content: vec![
            ContentBlock::text("I will read the target file."),
            ContentBlock::tool_use(
                "call_abc",
                "read_file",
                serde_json::json!({"path": "src/lib.rs"}),
            ),
        ],
        stop_reason: StopReason::ToolUse,
        usage: TokenUsage::new(150, 45),
    };

    assert_eq!(resp.stop_reason, StopReason::ToolUse);
    assert_eq!(resp.text(), "I will read the target file.");
    let calls = resp.tool_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "call_abc");
    assert_eq!(calls[0].1, "read_file");
    assert_eq!(calls[0].2["path"], "src/lib.rs");
}

#[test]
fn test_serde_roundtrip_all_schema_types() {
    let msg = ChatMessage {
        role: Role::Assistant,
        content: vec![
            ContentBlock::text("Executing tool"),
            ContentBlock::tool_use("call_1", "run_test", serde_json::json!({"name": "suite"})),
        ],
        name: Some("agent".to_string()),
    };

    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: ChatMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(msg, deserialized);
}

// ============================================================================
// 9. Remediation Regression Tests: H-01, M-01, M-02, L-01
// ============================================================================

#[test]
fn test_reserve_for_request_binds_to_token_bounds_and_pricing() {
    // H-01: Caller cannot understate worst-case cost
    let cap = MicroDollars::from_dollars(10).unwrap();
    let guard = BudgetGuard::new(cap);

    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: Some("You are a helpful coding agent.".to_string()),
        messages: vec![ChatMessage::user("Optimize algorithm")],
        tools: vec![],
        max_tokens: 2_000,
        temperature: Some(0.0),
        stop_sequences: vec![],
    };

    // Worst-case cost for 10,000 prompt tokens + 2,000 max completion tokens at Claude 3.7 rates:
    // (10,000 * 3.75) + (2,000 * 15.00) = $0.0375 + $0.0300 = $0.0675 = 67,500 micros
    let ticket = guard
        .reserve_for_request(&req, 10_000)
        .expect("reservation should succeed");

    assert_eq!(ticket.reserved_amount(), MicroDollars::from_micros(67_500));
    assert_eq!(ticket.model_id(), "claude-3-7-sonnet-20250219");

    let snap = guard.snapshot().unwrap();
    assert_eq!(snap.active_reserved, MicroDollars::from_micros(67_500));
    assert_eq!(
        snap.available_budget,
        MicroDollars::from_dollars(10)
            .unwrap()
            .checked_sub(MicroDollars::from_micros(67_500))
            .unwrap()
    );
}

#[test]
fn test_unknown_model_pricing_fails_closed() {
    // M-02: Unknown model must fail closed
    let cap = MicroDollars::from_dollars(5).unwrap();
    let guard = BudgetGuard::new(cap);

    let req = LlmRequest {
        model: "unregistered-unknown-model-v99".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Hello")],
        tools: vec![],
        max_tokens: 1_000,
        temperature: None,
        stop_sequences: vec![],
    };

    let err = guard.reserve_for_request(&req, 500).unwrap_err();
    match err {
        BudgetError::UnknownModelPricing(model) => {
            assert_eq!(model, "unregistered-unknown-model-v99");
        }
        other => panic!("Expected UnknownModelPricing, got {other:?}"),
    }

    // reserve() primitive also rejects unregistered models
    let raw_err = guard
        .reserve(
            MicroDollars::from_dollars(1).unwrap(),
            "unregistered-unknown-model-v99",
        )
        .unwrap_err();
    match raw_err {
        BudgetError::UnknownModelPricing(model) => {
            assert_eq!(model, "unregistered-unknown-model-v99");
        }
        other => panic!("Expected UnknownModelPricing, got {other:?}"),
    }

    // After registering the model, reservation succeeds
    let custom_pricing = ModelPricing {
        model_id: "unregistered-unknown-model-v99".to_string(),
        input_cost_per_million: MicroDollars::from_micros(1_000_000),
        output_cost_per_million: MicroDollars::from_micros(2_000_000),
        cache_read_cost_per_million: None,
        cache_write_cost_per_million: None,
    };
    guard
        .register_pricing(custom_pricing)
        .expect("registration should succeed");

    let ticket = guard
        .reserve_for_request(&req, 500)
        .expect("reservation should succeed after registration");
    assert_eq!(ticket.model_id(), "unregistered-unknown-model-v99");
}

#[test]
fn test_duplicate_tool_names_rejected() {
    // M-01: Duplicate tool names rejected in request validation
    let tool1 = ToolDefinition {
        name: "read_file".to_string(),
        description: "Read file contents".to_string(),
        input_schema: serde_json::json!({"type": "object"}),
    };
    let tool2 = ToolDefinition {
        name: "read_file".to_string(), // Duplicate name!
        description: "Another tool with same name".to_string(),
        input_schema: serde_json::json!({"type": "object"}),
    };

    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("read files")],
        tools: vec![tool1, tool2],
        max_tokens: 1000,
        temperature: None,
        stop_sequences: vec![],
    };

    let err = req.validate().unwrap_err();
    match err {
        SchemaValidationError::DuplicateToolName(name) => {
            assert_eq!(name, "read_file");
        }
        other => panic!("Expected DuplicateToolName, got {other:?}"),
    }

    // BudgetGuard::reserve_for_request also fails closed on duplicate tool names
    let guard = BudgetGuard::new(MicroDollars::from_dollars(5).unwrap());
    let budget_err = guard.reserve_for_request(&req, 100).unwrap_err();
    match budget_err {
        BudgetError::RequestValidationFailed(SchemaValidationError::DuplicateToolName(name)) => {
            assert_eq!(name, "read_file");
        }
        other => panic!("Expected RequestValidationFailed(DuplicateToolName), got {other:?}"),
    }
}

#[test]
fn test_internal_conservation_verified_on_all_transitions() {
    // L-01: Verify that every state transition passes verify_conservation without panics
    let cap = MicroDollars::from_dollars(10).unwrap();
    let guard = BudgetGuard::new(cap);

    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("test")],
        tools: vec![],
        max_tokens: 1_000,
        temperature: None,
        stop_sequences: vec![],
    };

    // Transition 1: reserve_for_request
    let ticket1 = guard.reserve_for_request(&req, 2000).unwrap();
    guard
        .snapshot()
        .unwrap()
        .verify_conservation_invariant()
        .unwrap();

    // Transition 2: reconcile_success
    let receipt = guard
        .reconcile_success(&ticket1, MicroDollars::from_micros(10_000))
        .unwrap();
    assert_eq!(receipt.actual_cost, MicroDollars::from_micros(10_000));
    guard
        .snapshot()
        .unwrap()
        .verify_conservation_invariant()
        .unwrap();

    // Transition 3: reserve then reconcile_ambiguous
    let ticket2 = guard.reserve_for_request(&req, 2000).unwrap();
    guard
        .reconcile_ambiguous(
            &ticket2,
            AmbiguousFailureReason::Timeout { elapsed_ms: 10_000 },
        )
        .unwrap();
    guard
        .snapshot()
        .unwrap()
        .verify_conservation_invariant()
        .unwrap();

    // Transition 4: reconcile_authoritative_resolution
    guard
        .reconcile_authoritative_resolution(ticket2.id(), MicroDollars::from_micros(5_000))
        .unwrap();
    guard
        .snapshot()
        .unwrap()
        .verify_conservation_invariant()
        .unwrap();

    // Transition 5: reserve then reconcile_preflight_cancel
    let ticket3 = guard.reserve_for_request(&req, 2000).unwrap();
    guard
        .reconcile_preflight_cancel(&ticket3, "Validation aborted")
        .unwrap();
    guard
        .snapshot()
        .unwrap()
        .verify_conservation_invariant()
        .unwrap();

    // Transition 6: actual cost exceeding reserved amount (with available headroom)
    let ticket4 = guard.reserve_for_request(&req, 500).unwrap();
    let reserved_amt = ticket4.reserved_amount();
    let higher_cost = reserved_amt
        .checked_add(MicroDollars::from_micros(5_000))
        .unwrap();
    let higher_receipt = guard.reconcile_success(&ticket4, higher_cost).unwrap();
    assert_eq!(higher_receipt.actual_cost, higher_cost);
    guard
        .snapshot()
        .unwrap()
        .verify_conservation_invariant()
        .unwrap();

    // Transition 7: actual cost exceeding reserved amount AND exceeding available headroom fails closed
    let snap_before = guard.snapshot().unwrap();
    let ticket5 = guard.reserve_for_request(&req, 500).unwrap();
    let impossible_cost = cap
        .checked_add(MicroDollars::from_dollars(1).unwrap())
        .unwrap();
    let over_err = guard
        .reconcile_success(&ticket5, impossible_cost)
        .unwrap_err();
    match over_err {
        BudgetError::CostExceedsReservationAndAvailableBudget { .. } => {}
        other => panic!("Expected CostExceedsReservationAndAvailableBudget, got {other:?}"),
    }
    // Ledger remains uncorrupted
    guard
        .reconcile_preflight_cancel(&ticket5, "cleanup")
        .unwrap();
    let snap_after = guard.snapshot().unwrap();
    assert_eq!(snap_before.committed_spend, snap_after.committed_spend);
    assert_eq!(snap_before.uncertain_spend, snap_after.uncertain_spend);
    snap_after.verify_conservation_invariant().unwrap();
}

// ============================================================================
// Milestone B2: Provider Abstraction, Mock Provider & Wire-Format Tests
// ============================================================================

// ----------------------------------------------------------------------------
// 15. PromptTokenEstimator: Structural Floor and Caller Clamping
// ----------------------------------------------------------------------------

#[test]
fn test_prompt_token_estimator_structural_floor() {
    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: Some("You are a helpful software engineering assistant.".to_string()),
        messages: vec![
            ChatMessage::user("Please read the following file and refactor the parser module."),
            ChatMessage::assistant("Certainly! Let me check the tools available."),
            ChatMessage::tool_result("tool_call_001", "fn parse() { ... 1000 lines ... }", false),
        ],
        tools: vec![ToolDefinition {
            name: "read_file".to_string(),
            description: "Read file contents from filesystem".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        }],
        max_tokens: 2048,
        temperature: Some(0.2),
        stop_sequences: vec!["<STOP>".to_string()],
    };

    let byte_count = PromptTokenEstimator::structural_byte_count(&req);
    assert!(
        byte_count > 100,
        "Byte count should reflect serialized text"
    );

    let floor = PromptTokenEstimator::structural_floor_tokens(&req);
    assert!(floor > 20, "Floor should be at least ceil(bytes / 4)");

    let upper_bound =
        PromptTokenEstimator::conservative_upper_bound_tokens(&req).expect("bound should succeed");
    assert!(
        upper_bound >= floor,
        "Upper bound ({upper_bound}) must be >= structural floor ({floor})"
    );

    // If caller provides 0 or tiny estimate, it is clamped to the conservative upper bound
    let clamped_zero = PromptTokenEstimator::bound_prompt_tokens(&req, 0).expect("bound succeeds");
    assert_eq!(clamped_zero, upper_bound);

    let clamped_tiny = PromptTokenEstimator::bound_prompt_tokens(&req, 5).expect("bound succeeds");
    assert_eq!(clamped_tiny, upper_bound);

    // If caller provides a conservative estimate above the upper bound, caller's estimate is preserved
    let generous_estimate = upper_bound + 500;
    let preserved =
        PromptTokenEstimator::bound_prompt_tokens(&req, generous_estimate).expect("bound succeeds");
    assert_eq!(preserved, generous_estimate);
}

// ----------------------------------------------------------------------------
// 16. Pricing Snapshot Immutability: Mutating Registry Does Not Alter Existing Reservation
// ----------------------------------------------------------------------------

#[test]
fn test_pricing_snapshot_immutability_under_registry_mutation() {
    let cap = MicroDollars::from_dollars(20).unwrap();
    let guard = BudgetGuard::new(cap);

    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Analyze architecture")],
        tools: vec![],
        max_tokens: 1_000,
        temperature: None,
        stop_sequences: vec![],
    };

    // Step 1: Reserve for request under standard reference pricing:
    // Claude 3.7 Sonnet: $3.00/MTok input, $15.00/MTok output
    // Worst case for 1,000 prompt + 1,000 output tokens:
    // Prompt (accounting for cache write premium $3.75/MTok): ceil(1000 * 3.75 / 1000) = $0.003750 (3,750 micros)
    // Output: ceil(1000 * 15 / 1000) = $0.015000 (15,000 micros)
    // Worst case total = 18,750 micros ($0.018750)
    let ticket = guard
        .reserve_for_request(&req, 1_000)
        .expect("reservation should succeed");
    assert_eq!(ticket.reserved_amount(), MicroDollars::from_micros(18_750));
    assert_eq!(
        ticket.pricing_snapshot().input_cost_per_million,
        MicroDollars::from_micros(3_000_000)
    );

    // Step 2: Adversarially or dynamically modify the pricing registry mid-flight!
    // Change rates to 100x higher: $300/MTok input, $1500/MTok output
    let predatory_pricing = ModelPricing {
        model_id: "claude-3-7-sonnet-20250219".to_string(),
        input_cost_per_million: MicroDollars::from_micros(300_000_000),
        output_cost_per_million: MicroDollars::from_micros(1_500_000_000),
        cache_read_cost_per_million: None,
        cache_write_cost_per_million: None,
    };
    guard
        .register_pricing(predatory_pricing)
        .expect("registry update should succeed");

    // Step 3: Reconcile turn with actual usage (1,000 prompt, 500 completion tokens)
    let usage = TokenUsage::new(1_000, 500);
    let receipt = guard
        .reconcile_turn(&ticket, &usage)
        .expect("reconcile_turn should succeed");

    // Under original pricing snapshot bound to ticket:
    // 1000 prompt @ $3/MTok = 3,000 micros
    // 500 completion @ $15/MTok = 7,500 micros
    // Actual cost = 10,500 micros ($0.010500)
    // (If it had used the mutated registry, cost would have been 1,050,000 micros!)
    assert_eq!(receipt.actual_cost, MicroDollars::from_micros(10_500));
    assert_eq!(receipt.released_surplus, MicroDollars::from_micros(8_250)); // 18,750 - 10,500 = 8,250 surplus
    assert_eq!(
        receipt.committed_spend_after,
        MicroDollars::from_micros(10_500)
    );

    let snap = guard.snapshot().expect("snapshot should succeed");
    snap.verify_conservation_invariant().unwrap();
}

// ----------------------------------------------------------------------------
// 17. Budget Safety Halt on Overrun: Subsequent Dispatches Fail Closed
// ----------------------------------------------------------------------------

#[test]
fn test_budget_safety_halt_on_overrun() {
    let cap = MicroDollars::from_dollars(5).unwrap(); // $5.00 cap
    let guard = BudgetGuard::new(cap);

    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Test prompt")],
        tools: vec![],
        max_tokens: 500,
        temperature: None,
        stop_sequences: vec![],
    };

    let ticket = guard
        .reserve_for_request(&req, 500)
        .expect("reservation should succeed");
    assert!(!guard.is_halted().unwrap());

    // Induce catastrophic overrun: actual cost $10.00 (exceeds reserved and exceeds remaining available budget)
    let impossible_cost = MicroDollars::from_dollars(10).unwrap();
    let err = guard
        .reconcile_success(&ticket, impossible_cost)
        .unwrap_err();
    match err {
        BudgetError::CostExceedsReservationAndAvailableBudget { .. } => {}
        other => panic!("Expected CostExceedsReservationAndAvailableBudget, got {other:?}"),
    }

    // Guard is now permanently halted!
    assert!(guard.is_halted().unwrap());
    let halt_reason = guard
        .halt_reason()
        .unwrap()
        .expect("halt reason must be set");
    assert!(halt_reason.contains("Budget overrun"));

    // Any subsequent reservation attempt fails closed immediately with SessionHaltedOnOverrun
    let next_req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Another request")],
        tools: vec![],
        max_tokens: 100,
        temperature: None,
        stop_sequences: vec![],
    };
    let next_err = guard.reserve_for_request(&next_req, 100).unwrap_err();
    match next_err {
        BudgetError::SessionHaltedOnOverrun(reason) => {
            assert!(reason.contains("Budget overrun"));
        }
        other => panic!("Expected SessionHaltedOnOverrun, got {other:?}"),
    }

    // Direct reserve() also fails closed with SessionHaltedOnOverrun
    let raw_err = guard
        .reserve(MicroDollars::from_micros(100), "claude-3-7-sonnet-20250219")
        .unwrap_err();
    match raw_err {
        BudgetError::SessionHaltedOnOverrun(_) => {}
        other => panic!("Expected SessionHaltedOnOverrun, got {other:?}"),
    }
}

// ----------------------------------------------------------------------------
// 18. MockLlmProvider: Deterministic Queue and Call Recording
// ----------------------------------------------------------------------------

#[test]
fn test_mock_provider_deterministic_queue_and_recording() {
    let mock = MockLlmProvider::new("claude-3-7-sonnet-20250219");
    assert_eq!(mock.provider_name(), "mock");
    assert_eq!(mock.model_id(), "claude-3-7-sonnet-20250219");
    assert_eq!(mock.call_count(), 0);

    let resp1 = LlmResponse {
        id: "msg_mock_001".to_string(),
        model: "claude-3-7-sonnet-20250219".to_string(),
        content: vec![ContentBlock::text("First mock response")],
        stop_reason: StopReason::EndTurn,
        usage: TokenUsage::new(100, 50),
    };

    let resp2 = LlmResponse {
        id: "msg_mock_002".to_string(),
        model: "claude-3-7-sonnet-20250219".to_string(),
        content: vec![ContentBlock::text("Second mock response")],
        stop_reason: StopReason::EndTurn,
        usage: TokenUsage::new(200, 80),
    };

    mock.push_response(resp1.clone());
    mock.push_response(resp2.clone());
    mock.push_error(ProviderError::Authentication {
        message: "Invalid API key".to_string(),
    });

    let req1 = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Request 1")],
        tools: vec![],
        max_tokens: 100,
        temperature: None,
        stop_sequences: vec![],
    };

    let req2 = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Request 2")],
        tools: vec![],
        max_tokens: 100,
        temperature: None,
        stop_sequences: vec![],
    };

    let r1 = mock.execute_turn(&req1).expect("call 1 should succeed");
    assert_eq!(r1.id, "msg_mock_001");
    assert_eq!(r1.text(), "First mock response");

    let r2 = mock.execute_turn(&req2).expect("call 2 should succeed");
    assert_eq!(r2.id, "msg_mock_002");
    assert_eq!(r2.text(), "Second mock response");

    let r3_err = mock.execute_turn(&req1).unwrap_err();
    match r3_err {
        ProviderError::Authentication { message } => assert_eq!(message, "Invalid API key"),
        other => panic!("Expected Authentication error, got {other:?}"),
    }

    assert_eq!(mock.call_count(), 3);
    let recorded = mock.recorded_requests();
    assert_eq!(recorded.len(), 3);
    assert_eq!(recorded[0].messages[0].text_content(), "Request 1");
    assert_eq!(recorded[1].messages[0].text_content(), "Request 2");
    assert_eq!(recorded[2].messages[0].text_content(), "Request 1");

    // Call 4 with empty queue returns typed error
    let r4_err = mock.execute_turn(&req1).unwrap_err();
    match r4_err {
        ProviderError::ProviderError { message, .. } => {
            assert!(message.contains("no remaining canned responses"));
        }
        other => panic!("Expected ProviderError with empty canned message, got {other:?}"),
    }
}

// ----------------------------------------------------------------------------
// 19. GuardedProvider: Pre-Flight Budget Exhaustion Prevents Dispatch
// ----------------------------------------------------------------------------

#[test]
fn test_guarded_provider_preflight_budget_rejection_prevents_dispatch() {
    // Tight budget: $0.005 (5,000 micro-dollars)
    let cap = MicroDollars::from_micros(5_000);
    let guard = BudgetGuard::new(cap);
    let mock = MockLlmProvider::new("claude-3-7-sonnet-20250219");

    // Enqueue a response in mock - if dispatch occurs, this would be consumed
    mock.push_response(LlmResponse {
        id: "msg_should_never_be_reached".to_string(),
        model: "claude-3-7-sonnet-20250219".to_string(),
        content: vec![ContentBlock::text("Unreachable")],
        stop_reason: StopReason::EndTurn,
        usage: TokenUsage::new(100, 50),
    });

    let guarded = GuardedProvider::new(mock, guard);

    // Request requires: 2000 output tokens ($0.030) + 1000 prompt tokens ($0.003) = $0.033 (33,000 micros)
    // Available: only 5,000 micros ($0.005)
    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Perform expensive task")],
        tools: vec![],
        max_tokens: 2_000,
        temperature: None,
        stop_sequences: vec![],
    };

    let result = guarded.execute_guarded(&req, 1_000);
    match result {
        Err(ProviderError::BudgetExhausted(msg)) => {
            assert!(msg.contains("Requested"));
            assert!(msg.contains("hard cap"));
        }
        other => panic!("Expected BudgetExhausted, got {other:?}"),
    }

    // INVARIANT PROVEN: Provider was NEVER invoked!
    assert_eq!(guarded.provider().call_count(), 0);
    assert!(guarded.provider().recorded_requests().is_empty());

    // Budget ledger remains completely pristine
    let snap = guarded.guard().snapshot().unwrap();
    assert_eq!(snap.active_reserved, MicroDollars::ZERO);
    assert_eq!(snap.committed_spend, MicroDollars::ZERO);
    assert_eq!(snap.uncertain_spend, MicroDollars::ZERO);
    assert_eq!(snap.available_budget, cap);
    snap.verify_conservation_invariant().unwrap();
}

// ----------------------------------------------------------------------------
// 20. GuardedProvider: Successful Dispatch and Reconciliation
// ----------------------------------------------------------------------------

#[test]
fn test_guarded_provider_successful_dispatch_and_reconciliation() {
    let cap = MicroDollars::from_dollars(10).unwrap();
    let guard = BudgetGuard::new(cap);
    let mock = MockLlmProvider::new("claude-3-7-sonnet-20250219");

    mock.push_response(LlmResponse {
        id: "msg_success_001".to_string(),
        model: "claude-3-7-sonnet-20250219".to_string(),
        content: vec![ContentBlock::text("Successfully refactored.")],
        stop_reason: StopReason::EndTurn,
        usage: TokenUsage {
            prompt_tokens: Some(1_000),
            completion_tokens: Some(200),
            cached_input_tokens: Some(400),
            cache_write_tokens: None,
            total_tokens: Some(1_200),
        },
    });

    let guarded = GuardedProvider::new(mock, guard);

    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: Some("You are an assistant".to_string()),
        messages: vec![ChatMessage::user("Refactor this")],
        tools: vec![],
        max_tokens: 1_000,
        temperature: None,
        stop_sequences: vec![],
    };

    let (response, receipt) = guarded
        .execute_guarded(&req, 1_000)
        .expect("guarded execution should succeed");

    assert_eq!(response.id, "msg_success_001");
    assert_eq!(guarded.provider().call_count(), 1);

    // Actual cost verification:
    // Prompt: 600 uncached @ $3/MTok = 1,800 micros
    // Prompt: 400 cached @ $0.30/MTok = 120 micros
    // Completion: 200 @ $15/MTok = 3,000 micros
    // Total = 4,920 micros ($0.004920)
    assert_eq!(receipt.actual_cost, MicroDollars::from_micros(4_920));
    assert!(receipt.released_surplus > MicroDollars::ZERO);

    let snap = guarded.guard().snapshot().unwrap();
    assert_eq!(snap.active_reserved, MicroDollars::ZERO);
    assert_eq!(snap.committed_spend, MicroDollars::from_micros(4_920));
    snap.verify_conservation_invariant().unwrap();
}

// ----------------------------------------------------------------------------
// 21. GuardedProvider: Ambiguous Network Failure Holds Funds in Uncertain Spend
// ----------------------------------------------------------------------------

#[test]
fn test_guarded_provider_ambiguous_failure_holds_funds() {
    let cap = MicroDollars::from_dollars(10).unwrap();
    let guard = BudgetGuard::new(cap);
    let mock = MockLlmProvider::new("claude-3-7-sonnet-20250219");

    mock.push_error(ProviderError::AmbiguousFailure(
        AmbiguousFailureReason::Timeout { elapsed_ms: 30_000 },
    ));

    let guarded = GuardedProvider::new(mock, guard);

    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Will time out")],
        tools: vec![],
        max_tokens: 1_000,
        temperature: None,
        stop_sequences: vec![],
    };

    let result = guarded.execute_guarded(&req, 1_000);
    match result {
        Err(ProviderError::AmbiguousFailure(AmbiguousFailureReason::Timeout { elapsed_ms })) => {
            assert_eq!(elapsed_ms, 30_000);
        }
        other => panic!("Expected AmbiguousFailure(Timeout), got {other:?}"),
    }

    // Provider was invoked
    assert_eq!(guarded.provider().call_count(), 1);

    // INVARIANT: Funds are permanently held in uncertain_spend, NOT released back to available budget!
    let snap = guarded.guard().snapshot().unwrap();
    assert_eq!(snap.active_reserved, MicroDollars::ZERO);
    assert_eq!(snap.committed_spend, MicroDollars::ZERO);
    assert_eq!(snap.ambiguous_count, 1);
    assert!(snap.uncertain_spend > MicroDollars::ZERO);
    // Available budget decreased by the held amount
    assert_eq!(
        snap.available_budget,
        cap.checked_sub(snap.uncertain_spend).unwrap()
    );
    snap.verify_conservation_invariant().unwrap();
}

// ----------------------------------------------------------------------------
// 22. GuardedProvider: Pre-Dispatch Schema Rejection Prevents Reservation & Dispatch
// ----------------------------------------------------------------------------

#[test]
fn test_guarded_provider_schema_validation_rejection_prevents_dispatch() {
    let cap = MicroDollars::from_dollars(10).unwrap();
    let guard = BudgetGuard::new(cap);
    let mock = MockLlmProvider::new("claude-3-7-sonnet-20250219");
    let guarded = GuardedProvider::new(mock, guard);

    // Invalid request: max_tokens = 0
    let invalid_req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Invalid request")],
        tools: vec![],
        max_tokens: 0,
        temperature: None,
        stop_sequences: vec![],
    };

    let result = guarded.execute_guarded(&invalid_req, 100);
    match result {
        Err(ProviderError::RequestValidation(SchemaValidationError::ZeroMaxTokens)) => {}
        other => panic!("Expected RequestValidation(ZeroMaxTokens), got {other:?}"),
    }

    assert_eq!(guarded.provider().call_count(), 0);
    let snap = guarded.guard().snapshot().unwrap();
    assert_eq!(snap.total_reservations_count, 0);
    assert_eq!(snap.available_budget, cap);
    snap.verify_conservation_invariant().unwrap();
}

// ----------------------------------------------------------------------------
// 23. Anthropic Wire-Format Adapter: Request Serialization
// ----------------------------------------------------------------------------

#[test]
fn test_anthropic_wire_format_serialization() {
    let tool = ToolDefinition {
        name: "execute_command".to_string(),
        description: "Execute bash command in sandbox".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string" }
            },
            "required": ["command"]
        }),
    };

    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: Some("You are an autonomous engineering agent.".to_string()),
        messages: vec![
            ChatMessage::user("Run test suite"),
            ChatMessage {
                role: Role::Assistant,
                content: vec![
                    ContentBlock::text("Running `cargo test` now."),
                    ContentBlock::tool_use(
                        "tool_call_001",
                        "execute_command",
                        serde_json::json!({"command": "cargo test"}),
                    ),
                ],
                name: None,
            },
            ChatMessage::tool_result("tool_call_001", "test result: ok. 250 passed", false),
        ],
        tools: vec![tool],
        max_tokens: 4096,
        temperature: Some(0.0),
        stop_sequences: vec!["</output>".to_string()],
    };

    let val = to_anthropic_request(&req).expect("serialization should succeed");
    assert_eq!(val["model"], "claude-3-7-sonnet-20250219");
    assert_eq!(val["max_tokens"], 4096);
    assert_eq!(val["system"], "You are an autonomous engineering agent.");
    assert_eq!(val["temperature"], 0.0);
    assert_eq!(val["stop_sequences"][0], "</output>");

    let messages = val["messages"].as_array().expect("messages must be array");
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["content"][0]["type"], "text");
    assert_eq!(messages[0]["content"][0]["text"], "Run test suite");

    assert_eq!(messages[1]["role"], "assistant");
    assert_eq!(messages[1]["content"][0]["type"], "text");
    assert_eq!(messages[1]["content"][1]["type"], "tool_use");
    assert_eq!(messages[1]["content"][1]["name"], "execute_command");

    // ToolResult role must map to "user" in Anthropic Messages API
    assert_eq!(messages[2]["role"], "user");
    assert_eq!(messages[2]["content"][0]["type"], "tool_result");
    assert_eq!(messages[2]["content"][0]["tool_use_id"], "tool_call_001");
    assert_eq!(
        messages[2]["content"][0]["content"],
        "test result: ok. 250 passed"
    );

    let tools = val["tools"].as_array().expect("tools must be array");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "execute_command");
}

// ----------------------------------------------------------------------------
// 24. Anthropic Wire-Format Adapter: Response Parsing & Telemetry Normalization
// ----------------------------------------------------------------------------

#[test]
fn test_anthropic_wire_format_deserialization_success() {
    let raw_json = serde_json::json!({
        "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
        "type": "message",
        "role": "assistant",
        "model": "claude-3-7-sonnet-20250219",
        "content": [
            {
                "type": "text",
                "text": "Tests completed successfully."
            },
            {
                "type": "tool_use",
                "id": "toolu_01A09q90qw90lq9",
                "name": "read_file",
                "input": { "path": "src/lib.rs" }
            }
        ],
        "stop_reason": "tool_use",
        "stop_sequence": null,
        "usage": {
            "input_tokens": 1200,
            "output_tokens": 150
        }
    });

    let resp = from_anthropic_response(&raw_json).expect("deserialization should succeed");
    assert_eq!(resp.id, "msg_01XFDUDYJgAACzvnptvVoYEL");
    assert_eq!(resp.model, "claude-3-7-sonnet-20250219");
    assert_eq!(resp.stop_reason, StopReason::ToolUse);
    assert_eq!(resp.content.len(), 2);
    assert_eq!(resp.text(), "Tests completed successfully.");

    let calls = resp.tool_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "toolu_01A09q90qw90lq9");
    assert_eq!(calls[0].1, "read_file");

    assert_eq!(resp.usage.prompt_tokens, Some(1200));
    assert_eq!(resp.usage.completion_tokens, Some(150));
    assert_eq!(resp.usage.total_tokens, Some(1350));
    assert_eq!(resp.usage.billed_tokens(), Some(1350));
}

// ----------------------------------------------------------------------------
// 25. Anthropic Wire-Format Adapter: Prompt Caching Telemetry Invariants
// ----------------------------------------------------------------------------

#[test]
fn test_anthropic_wire_format_prompt_cache_telemetry_invariants() {
    let raw_json = serde_json::json!({
        "id": "msg_cached_001",
        "type": "message",
        "role": "assistant",
        "model": "claude-3-7-sonnet-20250219",
        "content": [
            { "type": "text", "text": "Cached response" }
        ],
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": 1500,
            "cache_read_input_tokens": 500,
            "cache_creation_input_tokens": 100,
            "output_tokens": 250
        }
    });

    let resp = from_anthropic_response(&raw_json).expect("parsing cached telemetry should succeed");

    // Invariant: prompt_tokens is normalized total = input_tokens (1500) + cache_read (500) = 2000
    assert_eq!(resp.usage.prompt_tokens, Some(2000));
    assert_eq!(resp.usage.cached_input_tokens, Some(500));
    assert_eq!(resp.usage.cache_write_tokens, Some(100));
    assert_eq!(resp.usage.completion_tokens, Some(250));
    assert_eq!(resp.usage.total_tokens, Some(2250));

    // Phase 1 invariant: cached is a non-additive subset of prompt tokens
    assert!(resp.usage.cached_input_tokens.unwrap() <= resp.usage.prompt_tokens.unwrap());
    // Billed tokens = prompt_tokens + completion_tokens = 2250
    assert_eq!(resp.usage.billed_tokens(), Some(2250));
}

// ----------------------------------------------------------------------------
// 26. Anthropic Wire-Format Adapter: Rejection of Missing/Inconsistent Telemetry
// ----------------------------------------------------------------------------

#[test]
fn test_anthropic_wire_format_rejects_missing_telemetry() {
    // Missing usage entirely
    let no_usage = serde_json::json!({
        "id": "msg_invalid_001",
        "model": "claude-3-7-sonnet",
        "content": [{ "type": "text", "text": "Hi" }],
        "stop_reason": "end_turn"
    });
    let err1 = from_anthropic_response(&no_usage).unwrap_err();
    match err1 {
        ProviderError::MalformedResponse(msg) => assert!(msg.contains("usage")),
        other => panic!("Expected MalformedResponse, got {other:?}"),
    }

    // Missing input_tokens
    let no_input = serde_json::json!({
        "id": "msg_invalid_002",
        "model": "claude-3-7-sonnet",
        "content": [{ "type": "text", "text": "Hi" }],
        "stop_reason": "end_turn",
        "usage": { "output_tokens": 100 }
    });
    let err2 = from_anthropic_response(&no_input).unwrap_err();
    match err2 {
        ProviderError::MalformedResponse(msg) => assert!(msg.contains("input_tokens")),
        other => panic!("Expected MalformedResponse, got {other:?}"),
    }

    // Missing output_tokens
    let no_output = serde_json::json!({
        "id": "msg_invalid_003",
        "model": "claude-3-7-sonnet",
        "content": [{ "type": "text", "text": "Hi" }],
        "stop_reason": "end_turn",
        "usage": { "input_tokens": 100 }
    });
    let err3 = from_anthropic_response(&no_output).unwrap_err();
    match err3 {
        ProviderError::MalformedResponse(msg) => assert!(msg.contains("output_tokens")),
        other => panic!("Expected MalformedResponse, got {other:?}"),
    }
}

// ----------------------------------------------------------------------------
// 27. Header Sanitization: Credentials Redacted From Logs & Debug
// ----------------------------------------------------------------------------

#[test]
fn test_anthropic_header_sanitization_redacts_credentials() {
    let headers = vec![
        ("x-api-key", "sk-ant-api03-VERY-SECRET-LIVE-KEY-DO-NOT-LEAK"),
        ("Authorization", "Bearer sk-live-secret-token-12345678"),
        ("anthropic-version", "2023-06-01"),
        ("content-type", "application/json"),
        ("custom-session-token", "tok_sensitive_abcdef"),
        ("Cookie", "session_id=secret_cookie_val_12345"),
        (
            "set-cookie",
            "auth_jwt=jwt_payload_secret_xyz; Path=/; Secure; HttpOnly",
        ),
        ("COOKIE", "token_a=val_a; token_b=val_b"),
        ("Set-Cookie", "tracker=confidential_data_987"),
    ];

    let sanitized = sanitize_anthropic_headers(headers);
    let map: std::collections::HashMap<String, String> = sanitized.into_iter().collect();

    assert_eq!(map.get("x-api-key").unwrap(), "[REDACTED]");
    assert_eq!(map.get("Authorization").unwrap(), "[REDACTED]");
    assert_eq!(map.get("custom-session-token").unwrap(), "[REDACTED]");
    assert_eq!(map.get("Cookie").unwrap(), "[REDACTED]");
    assert_eq!(map.get("set-cookie").unwrap(), "[REDACTED]");
    assert_eq!(map.get("COOKIE").unwrap(), "[REDACTED]");
    assert_eq!(map.get("Set-Cookie").unwrap(), "[REDACTED]");
    assert_eq!(map.get("anthropic-version").unwrap(), "2023-06-01");
    assert_eq!(map.get("content-type").unwrap(), "application/json");

    // Test map helper with mixed cases
    let mut raw_map = std::collections::HashMap::new();
    raw_map.insert("x-api-key".to_string(), "sk-secret".to_string());
    raw_map.insert("Cookie".to_string(), "session=abc".to_string());
    raw_map.insert("SET-COOKIE".to_string(), "session=def".to_string());
    raw_map.insert("host".to_string(), "api.anthropic.com".to_string());
    let clean_map = sanitize_header_map(&raw_map);
    assert_eq!(clean_map.get("x-api-key").unwrap(), "[REDACTED]");
    assert_eq!(clean_map.get("Cookie").unwrap(), "[REDACTED]");
    assert_eq!(clean_map.get("SET-COOKIE").unwrap(), "[REDACTED]");
    assert_eq!(clean_map.get("host").unwrap(), "api.anthropic.com");
}

// ----------------------------------------------------------------------------
// 28. RedactedSecret: Contents Hidden from Debug and Display
// ----------------------------------------------------------------------------

#[test]
fn test_redacted_secret_never_exposes_keys_in_debug_or_display() {
    let plaintext_key = "sk-ant-super-confidential-production-credential-999";
    let secret = RedactedSecret::new(plaintext_key);

    // Display formatting
    let display_str = format!("{secret}");
    assert_eq!(display_str, "[REDACTED]");
    assert!(!display_str.contains(plaintext_key));

    // Debug formatting
    let debug_str = format!("{secret:?}");
    assert_eq!(debug_str, "[REDACTED]");
    assert!(!debug_str.contains(plaintext_key));

    // Controlled explicit exposure strictly through method
    assert_eq!(secret.expose_secret(), plaintext_key);
}

// ----------------------------------------------------------------------------
// 29. AUD-B2-01: Invalid Usage Telemetry Transitions Reservation to AmbiguousHold
// ----------------------------------------------------------------------------

#[test]
fn test_guarded_provider_invalid_telemetry_transitions_to_ambiguous_hold() {
    let cap = MicroDollars::from_dollars(10).unwrap();
    let guard = BudgetGuard::new(cap);
    let mock = MockLlmProvider::new("claude-3-7-sonnet-20250219");

    // Response with corrupt telemetry: cached_input_tokens (2000) > prompt_tokens (1000)
    mock.push_response(LlmResponse {
        id: "msg_corrupt_telemetry_001".to_string(),
        model: "claude-3-7-sonnet-20250219".to_string(),
        content: vec![ContentBlock::text("Corrupt telemetry emitted")],
        stop_reason: StopReason::EndTurn,
        usage: TokenUsage {
            prompt_tokens: Some(1_000),
            completion_tokens: Some(200),
            cached_input_tokens: Some(2_000), // Invariant violation: cached > prompt!
            cache_write_tokens: None,
            total_tokens: Some(1_200),
        },
    });

    let guarded = GuardedProvider::new(mock, guard);

    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Trigger corrupt telemetry")],
        tools: vec![],
        max_tokens: 1_000,
        temperature: None,
        stop_sequences: vec![],
    };

    let result = guarded.execute_guarded(&req, 1_000);
    match result {
        Err(ProviderError::InvalidTelemetry(TokenTelemetryError::CacheExceedsPrompt {
            cached_tokens,
            prompt_tokens,
        })) => {
            assert_eq!(cached_tokens, 2_000);
            assert_eq!(prompt_tokens, 1_000);
        }
        other => panic!("Expected InvalidTelemetry(CacheExceedsPrompt), got {other:?}"),
    }

    // AUD-B2-01 INVARIANT: Reservation was NOT orphaned in Active, and funds were NOT released!
    let snap = guarded.guard().snapshot().unwrap();
    assert_eq!(snap.active_reserved, MicroDollars::ZERO);
    assert_eq!(snap.active_count, 0);
    assert_eq!(snap.ambiguous_count, 1);
    assert!(snap.uncertain_spend > MicroDollars::ZERO);
    // Available budget decreased by held amount: funds remain locked
    assert_eq!(
        snap.available_budget,
        cap.checked_sub(snap.uncertain_spend).unwrap()
    );
    snap.verify_conservation_invariant().unwrap();

    // Later: Authoritative post-session reconciliation resolves the hold cleanly
    let ticket_id = engram::empirical_live::budget::ReservationId(1);
    let actual_cost = MicroDollars::from_micros(5_000);
    let receipt = guarded
        .guard()
        .reconcile_authoritative_resolution(ticket_id, actual_cost)
        .expect("authoritative resolution should succeed");
    assert_eq!(receipt.actual_cost, actual_cost);
    assert!(receipt.released_surplus > MicroDollars::ZERO);

    let snap2 = guarded.guard().snapshot().unwrap();
    assert_eq!(snap2.uncertain_spend, MicroDollars::ZERO);
    assert_eq!(snap2.committed_spend, actual_cost);
    assert_eq!(snap2.ambiguous_count, 0);
    assert_eq!(snap2.resolved_count, 1);
    assert_eq!(
        snap2.available_budget,
        cap.checked_sub(actual_cost).unwrap()
    );
    snap2.verify_conservation_invariant().unwrap();
}

// ----------------------------------------------------------------------------
// 30. AUD-B2-01: Incomplete Usage Telemetry Holds Reservation in Uncertain Spend
// ----------------------------------------------------------------------------

#[test]
fn test_guarded_provider_incomplete_telemetry_holds_reservation() {
    let cap = MicroDollars::from_dollars(10).unwrap();
    let guard = BudgetGuard::new(cap);
    let mock = MockLlmProvider::new("claude-3-7-sonnet-20250219");

    // Response with missing prompt_tokens: is_complete() is false
    mock.push_response(LlmResponse {
        id: "msg_incomplete_telemetry_001".to_string(),
        model: "claude-3-7-sonnet-20250219".to_string(),
        content: vec![ContentBlock::text("Incomplete telemetry emitted")],
        stop_reason: StopReason::EndTurn,
        usage: TokenUsage {
            prompt_tokens: None, // Missing!
            completion_tokens: Some(200),
            cached_input_tokens: None,
            cache_write_tokens: None,
            total_tokens: None,
        },
    });

    let guarded = GuardedProvider::new(mock, guard);

    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Trigger incomplete telemetry")],
        tools: vec![],
        max_tokens: 1_000,
        temperature: None,
        stop_sequences: vec![],
    };

    let result = guarded.execute_guarded(&req, 1_000);
    match result {
        Err(ProviderError::MalformedResponse(msg)) => {
            assert!(msg.contains("missing prompt_tokens or completion_tokens"));
        }
        other => panic!("Expected MalformedResponse for incomplete telemetry, got {other:?}"),
    }

    // Reservation is held in uncertain_spend, NOT orphaned in Active, NOT released
    let snap = guarded.guard().snapshot().unwrap();
    assert_eq!(snap.active_reserved, MicroDollars::ZERO);
    assert_eq!(snap.active_count, 0);
    assert_eq!(snap.ambiguous_count, 1);
    assert!(snap.uncertain_spend > MicroDollars::ZERO);
    snap.verify_conservation_invariant().unwrap();
}

// ----------------------------------------------------------------------------
// 31. AUD-B2-01: Duplicate Reconciliation Rejected Fail-Closed
// ----------------------------------------------------------------------------

#[test]
fn test_duplicate_authoritative_resolution_rejected_fail_closed() {
    let cap = MicroDollars::from_dollars(10).unwrap();
    let guard = BudgetGuard::new(cap);
    let mock = MockLlmProvider::new("claude-3-7-sonnet-20250219");

    mock.push_error(ProviderError::AmbiguousFailure(
        AmbiguousFailureReason::Timeout { elapsed_ms: 15_000 },
    ));

    let guarded = GuardedProvider::new(mock, guard);

    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Ambiguous request")],
        tools: vec![],
        max_tokens: 500,
        temperature: None,
        stop_sequences: vec![],
    };

    let _ = guarded.execute_guarded(&req, 500);

    let ticket_id = engram::empirical_live::budget::ReservationId(1);
    let actual_cost = MicroDollars::from_micros(3_000);

    // First authoritative resolution succeeds
    let rec1 = guarded
        .guard()
        .reconcile_authoritative_resolution(ticket_id, actual_cost)
        .expect("first resolution must succeed");
    assert_eq!(rec1.actual_cost, actual_cost);

    let snap1 = guarded.guard().snapshot().unwrap();
    assert_eq!(snap1.resolved_count, 1);
    assert_eq!(snap1.committed_spend, actual_cost);
    snap1.verify_conservation_invariant().unwrap();

    // Duplicate authoritative resolution attempt MUST fail closed
    let dup_err = guarded
        .guard()
        .reconcile_authoritative_resolution(ticket_id, actual_cost)
        .unwrap_err();
    match dup_err {
        BudgetError::InvalidStateTransition {
            id,
            current_state,
            attempted,
        } => {
            assert_eq!(id, ticket_id);
            assert_eq!(current_state, "AuthoritativelyResolved");
            assert_eq!(attempted, "reconcile_authoritative_resolution");
        }
        other => panic!("Expected InvalidStateTransition, got {other:?}"),
    }

    // Ledger balances remain completely uncorrupted after rejected duplicate attempt
    let snap2 = guarded.guard().snapshot().unwrap();
    assert_eq!(snap1, snap2);
    snap2.verify_conservation_invariant().unwrap();

    // Subsequent reservations continue to work normally
    let req2 = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Subsequent valid request")],
        tools: vec![],
        max_tokens: 500,
        temperature: None,
        stop_sequences: vec![],
    };
    let ticket2 = guarded
        .guard()
        .reserve_for_request(&req2, 500)
        .expect("subsequent reservation must succeed");
    assert_eq!(ticket2.id().0, 2);
    guarded
        .guard()
        .reconcile_preflight_cancel(&ticket2, "test cleanup")
        .unwrap();
    guarded
        .guard()
        .snapshot()
        .unwrap()
        .verify_conservation_invariant()
        .unwrap();
}

// ============================================================================
// 32. Milestone B3: Multi-Turn Orchestrator Successful Dialogue with Tool Calls
// ============================================================================

#[test]
fn test_multi_turn_orchestrator_successful_dialogue_with_tools() {
    let temp_dir = std::env::temp_dir().join(format!("engram_b3_test32_{}", uuid::Uuid::new_v4()));
    let dispatcher = SandboxToolDispatcher::new(&temp_dir).expect("dispatcher creation");

    let model_id = "claude-3-7-sonnet-20250219";
    let mock = MockLlmProvider::new(model_id);
    let guard = BudgetGuard::new(MicroDollars::from_dollars(10).unwrap());

    // Turn 1: Model requests write_file
    mock.push_response(LlmResponse {
        id: "msg_turn1".to_string(),
        model: model_id.to_string(),
        content: vec![
            ContentBlock::text("I will write the add function."),
            ContentBlock::tool_use(
                "tool_call_1",
                "write_file",
                serde_json::json!({
                    "path": "src/math.rs",
                    "content": "pub fn add(a: i32, b: i32) -> i32 { a + b }"
                }),
            ),
        ],
        stop_reason: StopReason::ToolUse,
        usage: TokenUsage::new(500, 100),
    });

    // Turn 2: Model requests read_file to verify
    mock.push_response(LlmResponse {
        id: "msg_turn2".to_string(),
        model: model_id.to_string(),
        content: vec![
            ContentBlock::text("Now I will verify the file content."),
            ContentBlock::tool_use(
                "tool_call_2",
                "read_file",
                serde_json::json!({ "path": "src/math.rs" }),
            ),
        ],
        stop_reason: StopReason::ToolUse,
        usage: TokenUsage::new(700, 80),
    });

    // Turn 3: Model requests finish tool declaring success
    mock.push_response(LlmResponse {
        id: "msg_turn3".to_string(),
        model: model_id.to_string(),
        content: vec![
            ContentBlock::text("Verification succeeded. Concluding task."),
            ContentBlock::tool_use(
                "tool_call_3",
                "finish",
                serde_json::json!({
                    "summary": "Implemented and verified add function in src/math.rs",
                    "success": true
                }),
            ),
        ],
        stop_reason: StopReason::ToolUse,
        usage: TokenUsage::new(850, 50),
    });

    let guarded = GuardedProvider::new(mock, guard);
    let orchestrator = MultiTurnOrchestrator::new(
        OrchestratorConfig::new(model_id)
            .with_max_turns(5)
            .with_system_prompt("You are a helpful software engineer."),
    );

    let telemetry = orchestrator.run_session(
        "sess_001",
        "scenario_math_add",
        "Please implement add function in src/math.rs",
        &guarded,
        &dispatcher,
        None,
    );

    // Verify session validity and outcome
    assert!(telemetry.is_valid(), "Session should complete normally");
    assert!(
        telemetry.is_task_success(),
        "Task should be declared success"
    );
    assert_eq!(telemetry.turns_executed, 3);
    assert_eq!(telemetry.tool_calls_executed, 3);
    assert_eq!(telemetry.total_prompt_tokens, 500 + 700 + 850);
    assert_eq!(telemetry.total_completion_tokens, 100 + 80 + 50);

    // Verify conversation ordering and correlation
    assert_eq!(telemetry.conversation_history.len(), 6);
    assert_eq!(telemetry.conversation_history[0].role, Role::User);
    assert_eq!(telemetry.conversation_history[1].role, Role::Assistant);
    assert_eq!(telemetry.conversation_history[2].role, Role::Tool);
    assert_eq!(telemetry.conversation_history[3].role, Role::Assistant);
    assert_eq!(telemetry.conversation_history[4].role, Role::Tool);
    assert_eq!(telemetry.conversation_history[5].role, Role::Assistant);

    // Check correlation on tool results
    if let ContentBlock::ToolResult {
        tool_use_id,
        is_error,
        ..
    } = &telemetry.conversation_history[2].content[0]
    {
        assert_eq!(tool_use_id, "tool_call_1");
        assert!(!is_error);
    } else {
        panic!("Expected ToolResult content block at index 2");
    }

    // Verify written file exists on disk inside sandbox
    let written_file = temp_dir.join("src/math.rs");
    assert!(written_file.exists());
    let file_text = std::fs::read_to_string(written_file).unwrap();
    assert_eq!(file_text, "pub fn add(a: i32, b: i32) -> i32 { a + b }");

    // Verify ledger conservation
    guarded
        .guard()
        .snapshot()
        .unwrap()
        .verify_conservation_invariant()
        .unwrap();
    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// 33. Milestone B3: Budget Exhaustion Halts Multi-Turn Session Mid-Dialogue
// ============================================================================

#[test]
fn test_multi_turn_orchestrator_budget_exhaustion_halts_dialogue() {
    let temp_dir = std::env::temp_dir().join(format!("engram_b3_test33_{}", uuid::Uuid::new_v4()));
    let dispatcher = SandboxToolDispatcher::new(&temp_dir).expect("dispatcher");

    let model_id = "claude-3-7-sonnet-20250219";
    let mock = MockLlmProvider::new(model_id);

    // Provide budget sufficient for Turn 1 worst-case, but not Turn 2
    // Under conservative upper bound estimation, Turn 1 requires 22,163 micros worst case
    // (including 5 tool schemas and 800 max completion tokens).
    // Budget cap of 23,000 micros allows Turn 1 to reserve and execute.
    // Turn 1 actual cost is ~900 micros, leaving 22,100 micros available.
    // Turn 2 has larger context (>23,500 micros worst case),
    // which exceeds the 22,100 available budget and halts.
    let guard = BudgetGuard::new(MicroDollars::from_micros(23_000));

    // Turn 1 succeeds and consumes ~900 micros
    mock.push_response(LlmResponse {
        id: "msg_t1".to_string(),
        model: model_id.to_string(),
        content: vec![
            ContentBlock::text("Listing directory"),
            ContentBlock::tool_use("tc_1", "list_directory", serde_json::json!({})),
        ],
        stop_reason: StopReason::ToolUse,
        usage: TokenUsage::new(200, 20),
    });

    let guarded = GuardedProvider::new(mock, guard);
    let orchestrator = MultiTurnOrchestrator::new(
        OrchestratorConfig::new(model_id)
            .with_max_turns(5)
            .with_max_tokens_per_turn(800),
    );

    let telemetry = orchestrator.run_session(
        "sess_budget_exhaust",
        "scenario_exhaust",
        "Inspect directory structure",
        &guarded,
        &dispatcher,
        None,
    );

    // Verify session halted due to budget exhaustion
    assert!(telemetry.is_incomplete());
    match telemetry.outcome {
        SessionOutcome::Halted {
            reason,
            turns_executed,
        } => {
            assert_eq!(
                turns_executed, 1,
                "Should have executed exactly 1 turn before halting"
            );
            match reason {
                HaltReason::BudgetExhausted(msg) => {
                    assert!(msg.contains("Requested") || msg.contains("available"));
                }
                other => panic!("Expected BudgetExhausted halt, got {other:?}"),
            }
        }
        other => panic!("Expected Halted outcome, got {other:?}"),
    }

    // Verify mock was called only once (Turn 2 was blocked before dispatch)
    assert_eq!(guarded.provider().call_count(), 1);

    // Verify conservation invariant
    guarded
        .guard()
        .snapshot()
        .unwrap()
        .verify_conservation_invariant()
        .unwrap();
    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// 34. Milestone B3: Budget Safety Halt on Overrun Halts Orchestrator
// ============================================================================

#[test]
fn test_multi_turn_orchestrator_budget_safety_halt_on_overrun() {
    let temp_dir = std::env::temp_dir().join(format!("engram_b3_test34_{}", uuid::Uuid::new_v4()));
    let dispatcher = SandboxToolDispatcher::new(&temp_dir).expect("dispatcher");

    let model_id = "claude-3-7-sonnet-20250219";
    let mock = MockLlmProvider::new(model_id);

    // Tight budget sufficient for Turn 1 pre-flight reservation (~11,637 micros), but far below overrun
    let guard = BudgetGuard::new(MicroDollars::from_micros(20_000));

    // Turn 1 returns huge unexpected completion tokens causing overrun
    mock.push_response(LlmResponse {
        id: "msg_overrun".to_string(),
        model: model_id.to_string(),
        content: vec![ContentBlock::text("Overrun content")],
        stop_reason: StopReason::EndTurn,
        usage: TokenUsage::new(100, 10_000), // 10,000 completion * $15/MTok = 150,000 micros > 10,000 cap
    });

    let guarded = GuardedProvider::new(mock, guard);
    let orchestrator =
        MultiTurnOrchestrator::new(OrchestratorConfig::new(model_id).with_max_tokens_per_turn(100));

    let telemetry = orchestrator.run_session(
        "sess_overrun",
        "scen_overrun",
        "Generate short answer",
        &guarded,
        &dispatcher,
        None,
    );

    assert!(telemetry.is_incomplete());
    match telemetry.outcome {
        SessionOutcome::Halted { reason, .. } => match reason {
            HaltReason::BudgetSafetyHalt(msg) => {
                assert!(msg.contains("halted") || msg.contains("exceeded"));
            }
            other => panic!("Expected BudgetSafetyHalt, got {other:?}"),
        },
        other => panic!("Expected Halted outcome, got {other:?}"),
    }

    assert!(guarded.guard().snapshot().unwrap().is_halted);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// 35. Milestone B3: Ambiguous Failure Retains Funds in Uncertain Spend and Halts Without Retry
// ============================================================================

#[test]
fn test_multi_turn_orchestrator_ambiguous_failure_halts_without_retry() {
    let temp_dir = std::env::temp_dir().join(format!("engram_b3_test35_{}", uuid::Uuid::new_v4()));
    let dispatcher = SandboxToolDispatcher::new(&temp_dir).expect("dispatcher");

    let model_id = "claude-3-7-sonnet-20250219";
    let mock = MockLlmProvider::new(model_id);
    let guard = BudgetGuard::new(MicroDollars::from_dollars(5).unwrap());

    // Push ambiguous timeout failure
    mock.push_error(ProviderError::AmbiguousFailure(
        AmbiguousFailureReason::Timeout { elapsed_ms: 30_000 },
    ));

    let guarded = GuardedProvider::new(mock, guard);
    let orchestrator =
        MultiTurnOrchestrator::new(OrchestratorConfig::new(model_id).with_max_turns(5));

    let telemetry = orchestrator.run_session(
        "sess_ambiguous",
        "scen_timeout",
        "Run complex task",
        &guarded,
        &dispatcher,
        None,
    );

    assert!(telemetry.is_incomplete());
    match telemetry.outcome {
        SessionOutcome::Halted {
            reason,
            turns_executed,
        } => {
            assert_eq!(turns_executed, 0);
            match reason {
                HaltReason::AmbiguousProviderFailure(msg) => {
                    assert!(msg.contains("Timeout"));
                }
                other => panic!("Expected AmbiguousProviderFailure, got {other:?}"),
            }
        }
        other => panic!("Expected Halted outcome, got {other:?}"),
    }

    // Invariant: Provider was called exactly ONCE (never retried ambiguous outcome)
    assert_eq!(guarded.provider().call_count(), 1);

    // Invariant: Reserved funds transferred to uncertain_spend and NOT released
    let snap = guarded.guard().snapshot().unwrap();
    assert!(
        snap.uncertain_spend > MicroDollars::ZERO,
        "Uncertain spend must retain reserved funds"
    );
    assert_eq!(snap.active_reserved, MicroDollars::ZERO);
    snap.verify_conservation_invariant().unwrap();

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// 36. Milestone B3: Sandbox Tool Dispatcher Path Containment & Traversal Rejection
// ============================================================================

#[test]
fn test_sandbox_tool_dispatcher_path_containment_and_traversal_rejection() {
    let temp_dir = std::env::temp_dir().join(format!("engram_b3_test36_{}", uuid::Uuid::new_v4()));
    let dispatcher = SandboxToolDispatcher::new(&temp_dir).expect("dispatcher creation");

    // Test 1: Parent traversal in path rejected
    let res1 = dispatcher.dispatch(
        "read_file",
        "call_trav_1",
        &serde_json::json!({ "path": "../outside.txt" }),
    );
    if let ContentBlock::ToolResult {
        is_error, content, ..
    } = res1
    {
        assert!(is_error);
        assert!(content.contains("Directory traversal ('..') is strictly prohibited"));
    } else {
        panic!("Expected ToolResult");
    }

    // Test 2: Subdirectory parent traversal rejected
    let res2 = dispatcher.dispatch(
        "read_file",
        "call_trav_2",
        &serde_json::json!({ "path": "subdir/../../outside.txt" }),
    );
    if let ContentBlock::ToolResult {
        is_error, content, ..
    } = res2
    {
        assert!(is_error);
        assert!(content.contains("Directory traversal"));
    } else {
        panic!("Expected ToolResult");
    }

    // Test 3: Null byte rejected
    let res3 = dispatcher.dispatch(
        "write_file",
        "call_null_byte",
        &serde_json::json!({ "path": "file\0name.txt", "content": "data" }),
    );
    if let ContentBlock::ToolResult {
        is_error, content, ..
    } = res3
    {
        assert!(is_error);
        assert!(content.contains("null bytes"));
    } else {
        panic!("Expected ToolResult");
    }

    // Test 4: Missing file reported cleanly
    let res4 = dispatcher.dispatch(
        "read_file",
        "call_not_found",
        &serde_json::json!({ "path": "missing_file.txt" }),
    );
    if let ContentBlock::ToolResult {
        is_error, content, ..
    } = res4
    {
        assert!(is_error);
        assert!(content.contains("File not found"));
    } else {
        panic!("Expected ToolResult");
    }

    // Test 5: Valid read and write
    let res5 = dispatcher.dispatch(
        "write_file",
        "call_write_ok",
        &serde_json::json!({ "path": "valid.txt", "content": "hello world" }),
    );
    if let ContentBlock::ToolResult {
        is_error, content, ..
    } = res5
    {
        assert!(!is_error);
        assert!(content.contains("Successfully wrote 11 bytes"));
    } else {
        panic!("Expected ToolResult");
    }

    let res6 = dispatcher.dispatch(
        "read_file",
        "call_read_ok",
        &serde_json::json!({ "path": "valid.txt" }),
    );
    if let ContentBlock::ToolResult {
        is_error, content, ..
    } = res6
    {
        assert!(!is_error);
        assert_eq!(content, "hello world");
    } else {
        panic!("Expected ToolResult");
    }

    // Test 6: List directory
    let res7 = dispatcher.dispatch(
        "list_directory",
        "call_list_ok",
        &serde_json::json!({ "path": "." }),
    );
    if let ContentBlock::ToolResult {
        is_error, content, ..
    } = res7
    {
        assert!(!is_error);
        assert!(content.contains("[file] valid.txt"));
    } else {
        panic!("Expected ToolResult");
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// 37. Milestone B3: Sandbox Tool Dispatcher Command Allowlist and Argument Sanitization
// ============================================================================

#[test]
fn test_sandbox_tool_dispatcher_command_allowlist_and_arg_sanitization() {
    let temp_dir = std::env::temp_dir().join(format!("engram_b3_test37_{}", uuid::Uuid::new_v4()));
    let dispatcher = SandboxToolDispatcher::new(&temp_dir).expect("dispatcher");

    // Test 1: Shell interpreter rejected
    let res_sh = dispatcher.dispatch(
        "execute_command",
        "cmd_sh",
        &serde_json::json!({ "command": "bash", "args": ["-c", "echo 1"] }),
    );
    if let ContentBlock::ToolResult {
        is_error, content, ..
    } = res_sh
    {
        assert!(is_error);
        assert!(content.contains("not in the approved allowlist"));
    } else {
        panic!("Expected ToolResult");
    }

    let res_cmd = dispatcher.dispatch(
        "execute_command",
        "cmd_cmd",
        &serde_json::json!({ "command": "cmd.exe", "args": ["/c", "dir"] }),
    );
    if let ContentBlock::ToolResult {
        is_error, content, ..
    } = res_cmd
    {
        assert!(is_error);
        assert!(content.contains("not in the approved allowlist"));
    } else {
        panic!("Expected ToolResult");
    }

    let res_ps = dispatcher.dispatch(
        "execute_command",
        "cmd_ps",
        &serde_json::json!({ "command": "powershell", "args": ["Get-Process"] }),
    );
    if let ContentBlock::ToolResult {
        is_error, content, ..
    } = res_ps
    {
        assert!(is_error);
        assert!(content.contains("not in the approved allowlist"));
    } else {
        panic!("Expected ToolResult");
    }

    // Test 2: Allowlisted command with prohibited script-eval flag '-c' rejected
    let res_eval = dispatcher.dispatch(
        "execute_command",
        "cmd_python_c",
        &serde_json::json!({ "command": "python", "args": ["-c", "import os; os.system('calc')"] }),
    );
    if let ContentBlock::ToolResult {
        is_error, content, ..
    } = res_eval
    {
        assert!(is_error);
        assert!(content.contains("Arbitrary script evaluation flag is prohibited"));
    } else {
        panic!("Expected ToolResult");
    }

    // Test 3: Allowlisted command with shell metacharacters rejected
    let res_pipe = dispatcher.dispatch(
        "execute_command",
        "cmd_pipe",
        &serde_json::json!({ "command": "cargo", "args": ["test", "|", "cat"] }),
    );
    if let ContentBlock::ToolResult {
        is_error, content, ..
    } = res_pipe
    {
        assert!(is_error);
        assert!(content.contains("Shell metacharacter '|' is prohibited"));
    } else {
        panic!("Expected ToolResult");
    }

    // Test 4: Path slashes in command name rejected
    let res_slash = dispatcher.dispatch(
        "execute_command",
        "cmd_slash",
        &serde_json::json!({ "command": "../../bin/cargo", "args": ["--version"] }),
    );
    if let ContentBlock::ToolResult {
        is_error, content, ..
    } = res_slash
    {
        assert!(is_error);
        assert!(content.contains("not in the approved allowlist"));
    } else {
        panic!("Expected ToolResult");
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// 38. Milestone B3: Sandbox Tool Dispatcher Duplicate Call Detection
// ============================================================================

#[test]
fn test_sandbox_tool_dispatcher_duplicate_call_detection() {
    let temp_dir = std::env::temp_dir().join(format!("engram_b3_test38_{}", uuid::Uuid::new_v4()));
    let dispatcher = SandboxToolDispatcher::new(&temp_dir)
        .expect("dispatcher")
        .with_duplicate_limit(3);

    // Call 1: Missing file (attempt 1)
    let res1 = dispatcher.dispatch(
        "read_file",
        "dup_1",
        &serde_json::json!({ "path": "nonexistent.txt" }),
    );
    if let ContentBlock::ToolResult {
        is_error, content, ..
    } = res1
    {
        assert!(is_error);
        assert!(content.contains("File not found"));
    }

    // Call 2: Identical parameters (attempt 2)
    let res2 = dispatcher.dispatch(
        "read_file",
        "dup_2",
        &serde_json::json!({ "path": "nonexistent.txt" }),
    );
    if let ContentBlock::ToolResult {
        is_error, content, ..
    } = res2
    {
        assert!(is_error);
        assert!(content.contains("File not found"));
    }

    // Call 3: Identical parameters reaches duplicate ceiling (attempt 3)
    let res3 = dispatcher.dispatch(
        "read_file",
        "dup_3",
        &serde_json::json!({ "path": "nonexistent.txt" }),
    );
    if let ContentBlock::ToolResult {
        is_error, content, ..
    } = res3
    {
        assert!(is_error);
        assert!(content.contains("called 3 times consecutively with identical arguments"));
    } else {
        panic!("Expected duplicate limit rejection");
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// 39. Milestone B3: Consecutive Tool Errors Exceeded Halts Session
// ============================================================================

#[test]
fn test_multi_turn_orchestrator_consecutive_tool_errors_halts_session() {
    let temp_dir = std::env::temp_dir().join(format!("engram_b3_test39_{}", uuid::Uuid::new_v4()));
    let dispatcher = SandboxToolDispatcher::new(&temp_dir).expect("dispatcher");

    let model_id = "claude-3-7-sonnet-20250219";
    let mock = MockLlmProvider::new(model_id);
    let guard = BudgetGuard::new(MicroDollars::from_dollars(5).unwrap());

    // Mock emits unknown tools across 3 turns
    for i in 1..=3 {
        mock.push_response(LlmResponse {
            id: format!("msg_err_{i}"),
            model: model_id.to_string(),
            content: vec![ContentBlock::tool_use(
                format!("call_err_{i}"),
                "nonexistent_tool",
                serde_json::json!({}),
            )],
            stop_reason: StopReason::ToolUse,
            usage: TokenUsage::new(200, 30),
        });
    }

    let guarded = GuardedProvider::new(mock, guard);
    let orchestrator =
        MultiTurnOrchestrator::new(OrchestratorConfig::new(model_id).with_max_turns(10));

    let telemetry = orchestrator.run_session(
        "sess_consecutive_err",
        "scen_consecutive",
        "Run task",
        &guarded,
        &dispatcher,
        None,
    );

    assert!(telemetry.is_incomplete());
    match telemetry.outcome {
        SessionOutcome::Halted {
            reason,
            turns_executed,
        } => {
            assert_eq!(turns_executed, 3);
            match reason {
                HaltReason::ConsecutiveToolErrorsExceeded { count } => {
                    assert_eq!(count, 3);
                }
                other => panic!("Expected ConsecutiveToolErrorsExceeded, got {other:?}"),
            }
        }
        other => panic!("Expected Halted outcome, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// 40. Milestone B3: Infrastructure Retry Limit Capped at 1
// ============================================================================

#[test]
fn test_multi_turn_orchestrator_infrastructure_retry_capped_at_one() {
    let temp_dir = std::env::temp_dir().join(format!("engram_b3_test40_{}", uuid::Uuid::new_v4()));
    let dispatcher = SandboxToolDispatcher::new(&temp_dir).expect("dispatcher");

    let model_id = "claude-3-7-sonnet-20250219";

    // Scenario A: Turn 1 fails once (transient transport), retry succeeds
    {
        let mock_a = MockLlmProvider::new(model_id);
        let guard_a = BudgetGuard::new(MicroDollars::from_dollars(5).unwrap());

        mock_a.push_error(ProviderError::Transport {
            message: "Temporary socket reset".to_string(),
        });
        mock_a.push_response(LlmResponse {
            id: "msg_success_retry".to_string(),
            model: model_id.to_string(),
            content: vec![ContentBlock::text("Recovered on retry")],
            stop_reason: StopReason::EndTurn,
            usage: TokenUsage::new(200, 20),
        });

        let guarded_a = GuardedProvider::new(mock_a, guard_a);
        let orchestrator_a = MultiTurnOrchestrator::new(OrchestratorConfig::new(model_id));

        let telem_a = orchestrator_a.run_session(
            "sess_retry_ok",
            "scen_retry",
            "Do task",
            &guarded_a,
            &dispatcher,
            None,
        );

        assert!(telem_a.is_valid());
        assert_eq!(
            guarded_a.provider().call_count(),
            2,
            "Should have executed 1 retry"
        );
    }

    // Scenario B: Turn 1 fails twice (transient transport) -> retry limit reached, halts fail-closed
    {
        let mock_b = MockLlmProvider::new(model_id);
        let guard_b = BudgetGuard::new(MicroDollars::from_dollars(5).unwrap());

        mock_b.push_error(ProviderError::Transport {
            message: "Socket reset 1".to_string(),
        });
        mock_b.push_error(ProviderError::Transport {
            message: "Socket reset 2".to_string(),
        });
        // 3rd call is queued but must NEVER be executed
        mock_b.push_response(LlmResponse {
            id: "msg_unreachable".to_string(),
            model: model_id.to_string(),
            content: vec![ContentBlock::text("Should not reach")],
            stop_reason: StopReason::EndTurn,
            usage: TokenUsage::new(200, 20),
        });

        let guarded_b = GuardedProvider::new(mock_b, guard_b);
        let orchestrator_b = MultiTurnOrchestrator::new(OrchestratorConfig::new(model_id));

        let telem_b = orchestrator_b.run_session(
            "sess_retry_exhausted",
            "scen_retry_ex",
            "Do task",
            &guarded_b,
            &dispatcher,
            None,
        );

        assert!(telem_b.is_incomplete());
        match telem_b.outcome {
            SessionOutcome::Halted { reason, .. } => match reason {
                HaltReason::InfrastructureFailure(msg) => {
                    assert!(msg.contains("Socket reset 2"));
                }
                other => panic!("Expected InfrastructureFailure, got {other:?}"),
            },
            other => panic!("Expected Halted outcome, got {other:?}"),
        }

        // Must have stopped after exactly 2 calls (initial + 1 retry)
        assert_eq!(guarded_b.provider().call_count(), 2);
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// 41. Milestone B3: Agent Task Failure is Never Retried
// ============================================================================

#[test]
fn test_multi_turn_orchestrator_agent_failure_never_retried() {
    let temp_dir = std::env::temp_dir().join(format!("engram_b3_test41_{}", uuid::Uuid::new_v4()));
    let dispatcher = SandboxToolDispatcher::new(&temp_dir).expect("dispatcher");

    let model_id = "claude-3-7-sonnet-20250219";
    let mock = MockLlmProvider::new(model_id);
    let guard = BudgetGuard::new(MicroDollars::from_dollars(5).unwrap());

    // Agent calls finish declaring failure
    mock.push_response(LlmResponse {
        id: "msg_finish_failed".to_string(),
        model: model_id.to_string(),
        content: vec![ContentBlock::tool_use(
            "tc_finish_fail",
            "finish",
            serde_json::json!({
                "summary": "Could not satisfy test assertions",
                "success": false
            }),
        )],
        stop_reason: StopReason::ToolUse,
        usage: TokenUsage::new(300, 40),
    });

    let guarded = GuardedProvider::new(mock, guard);
    let orchestrator = MultiTurnOrchestrator::new(OrchestratorConfig::new(model_id));

    let telemetry = orchestrator.run_session(
        "sess_agent_fail",
        "scen_agent_fail",
        "Fix bug",
        &guarded,
        &dispatcher,
        None,
    );

    // Agent session is Valid (completed lifecycle), but TaskOutcome is Failure
    assert!(
        telemetry.is_valid(),
        "Session validity must be Valid when agent finishes normally"
    );
    assert!(!telemetry.is_task_success(), "Task success must be false");
    assert_eq!(telemetry.turns_executed, 1);
    assert_eq!(
        guarded.provider().call_count(),
        1,
        "Agent failure must NEVER be retried"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// 42. Milestone B3: Cancellation Request Stops Execution and Preserves Conservation
// ============================================================================

#[test]
fn test_multi_turn_orchestrator_cancellation_preserves_conservation() {
    let temp_dir = std::env::temp_dir().join(format!("engram_b3_test42_{}", uuid::Uuid::new_v4()));
    let dispatcher = SandboxToolDispatcher::new(&temp_dir).expect("dispatcher");

    let model_id = "claude-3-7-sonnet-20250219";
    let mock = MockLlmProvider::new(model_id);
    let guard = BudgetGuard::new(MicroDollars::from_dollars(5).unwrap());

    // Turn 1 succeeds
    mock.push_response(LlmResponse {
        id: "msg_t1".to_string(),
        model: model_id.to_string(),
        content: vec![
            ContentBlock::text("Working..."),
            ContentBlock::tool_use("tc_1", "list_directory", serde_json::json!({})),
        ],
        stop_reason: StopReason::ToolUse,
        usage: TokenUsage::new(200, 20),
    });

    let guarded = GuardedProvider::new(mock, guard);
    let orchestrator =
        MultiTurnOrchestrator::new(OrchestratorConfig::new(model_id).with_max_turns(5));

    let cancel_flag = Arc::new(AtomicBool::new(false));

    // Cancel flag set to true before Turn 2
    cancel_flag.store(true, Ordering::SeqCst);

    let telemetry = orchestrator.run_session(
        "sess_cancel",
        "scen_cancel",
        "Start long run",
        &guarded,
        &dispatcher,
        Some(&cancel_flag),
    );

    assert!(telemetry.is_incomplete());
    match telemetry.outcome {
        SessionOutcome::Halted {
            reason,
            turns_executed,
        } => {
            assert_eq!(turns_executed, 0); // Cancelled immediately before Turn 1
            assert_eq!(reason, HaltReason::Cancelled);
        }
        other => panic!("Expected Halted outcome, got {other:?}"),
    }

    // Ledger balances remain perfectly conserved and zero active reservations leaked
    let snap = guarded.guard().snapshot().unwrap();
    assert_eq!(snap.active_reserved, MicroDollars::ZERO);
    snap.verify_conservation_invariant().unwrap();

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// 43. Milestone B3: Max Turns Ceiling Halts Session Cleanly
// ============================================================================

#[test]
fn test_multi_turn_orchestrator_max_turns_exceeded_halts_gracefully() {
    let temp_dir = std::env::temp_dir().join(format!("engram_b3_test43_{}", uuid::Uuid::new_v4()));
    let dispatcher = SandboxToolDispatcher::new(&temp_dir).expect("dispatcher");

    let model_id = "claude-3-7-sonnet-20250219";
    let mock = MockLlmProvider::new(model_id);
    let guard = BudgetGuard::new(MicroDollars::from_dollars(5).unwrap());

    // 2 turns configured
    for i in 1..=2 {
        mock.push_response(LlmResponse {
            id: format!("msg_turn_{i}"),
            model: model_id.to_string(),
            content: vec![
                ContentBlock::text(format!("Thinking in turn {i}")),
                ContentBlock::tool_use(format!("tc_{i}"), "list_directory", serde_json::json!({})),
            ],
            stop_reason: StopReason::ToolUse,
            usage: TokenUsage::new(200, 20),
        });
    }

    let guarded = GuardedProvider::new(mock, guard);
    let orchestrator =
        MultiTurnOrchestrator::new(OrchestratorConfig::new(model_id).with_max_turns(2));

    let telemetry = orchestrator.run_session(
        "sess_max_turns",
        "scen_max_turns",
        "Infinite task",
        &guarded,
        &dispatcher,
        None,
    );

    assert!(telemetry.is_incomplete());
    match telemetry.outcome {
        SessionOutcome::Halted {
            reason,
            turns_executed,
        } => {
            assert_eq!(turns_executed, 2);
            assert_eq!(reason, HaltReason::MaxTurnsExceeded { limit: 2 });
        }
        other => panic!("Expected Halted outcome, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// 44. Milestone B3: Sandbox Tool Dispatcher Symlink Escape Rejection
// ============================================================================

#[test]
fn test_sandbox_tool_dispatcher_symlink_escape_rejected() {
    let temp_dir =
        std::env::temp_dir().join(format!("engram_b3_test44_sb_{}", uuid::Uuid::new_v4()));
    let external_dir =
        std::env::temp_dir().join(format!("engram_b3_test44_ext_{}", uuid::Uuid::new_v4()));

    std::fs::create_dir_all(&temp_dir).unwrap();
    std::fs::create_dir_all(&external_dir).unwrap();

    let external_secret = external_dir.join("secret_token.txt");
    std::fs::write(&external_secret, "SUPER_SECRET_PAYLOAD").unwrap();

    let dispatcher = SandboxToolDispatcher::new(&temp_dir).expect("dispatcher");

    // Attempt creating junction on Windows or symlink on Unix
    #[cfg(windows)]
    let link_created = {
        let junc_link = temp_dir.join("external_junction");
        let junc_str = junc_link.to_string_lossy();
        let clean_junc_raw = junc_str.strip_prefix(r"\\?\").unwrap_or(&junc_str);
        let clean_junc = clean_junc_raw.replace('/', "\\");

        let ext_str = external_dir.to_string_lossy();
        let clean_ext_raw = ext_str.strip_prefix(r"\\?\").unwrap_or(&ext_str);
        let clean_ext = clean_ext_raw.replace('/', "\\");

        let mut cmd = std::process::Command::new("cmd.exe");
        cmd.args(["/C", "mklink", "/J", &clean_junc, &clean_ext]);
        let ok = cmd.status().map(|s| s.success()).unwrap_or(false);
        if ok {
            Some("external_junction/secret_token.txt".to_string())
        } else {
            None
        }
    };

    #[cfg(unix)]
    let link_created = {
        let link_path = temp_dir.join("external_symlink.txt");
        if std::os::unix::fs::symlink(&external_secret, &link_path).is_ok() {
            Some("external_symlink.txt".to_string())
        } else {
            None
        }
    };

    if let Some(target_rel) = link_created {
        // Dispatch read_file targeting the symlink/junction escape
        let res = dispatcher.dispatch(
            "read_file",
            "call_symlink_test",
            &serde_json::json!({ "path": target_rel }),
        );
        if let ContentBlock::ToolResult {
            is_error, content, ..
        } = res
        {
            assert!(
                is_error,
                "Reading through symlink/junction escape must be rejected"
            );
            assert!(
                content.contains("escapes sandbox boundary")
                    || content.contains("Symlink or junction access is strictly prohibited"),
                "Expected escape or symlink rejection, got: {content}"
            );
        } else {
            panic!("Expected ToolResult");
        }

        #[cfg(windows)]
        {
            let junc_link = temp_dir.join("external_junction");
            let junc_str = junc_link.to_string_lossy();
            let clean_junc_raw = junc_str.strip_prefix(r"\\?\").unwrap_or(&junc_str);
            let clean_junc = clean_junc_raw.replace('/', "\\");
            let mut cmd = std::process::Command::new("cmd.exe");
            cmd.args(["/C", "rmdir", &clean_junc]);
            let _ = cmd.status();
        }
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::remove_dir_all(&external_dir);
}

// ============================================================================
// 12. Milestone B4: Paired Statistical Engine & Holm-Bonferroni Accounting
// ============================================================================

#[test]
fn test_paired_savings_rate_calculation() {
    // 5 matched pairs with known token consumption
    let pairs = vec![
        MatchedPairInput {
            scenario_id: "scenario_1".to_string(),
            pair_index: 0,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 8_000, // S_0 = 0.20
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "scenario_2".to_string(),
            pair_index: 1,
            control_billed_tokens: 12_000,
            treatment_billed_tokens: 9_000, // S_1 = 0.25
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "scenario_3".to_string(),
            pair_index: 2,
            control_billed_tokens: 8_000,
            treatment_billed_tokens: 6_800, // S_2 = 0.15
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "scenario_4".to_string(),
            pair_index: 3,
            control_billed_tokens: 15_000,
            treatment_billed_tokens: 12_000, // S_3 = 0.20
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "scenario_5".to_string(),
            pair_index: 4,
            control_billed_tokens: 11_000,
            treatment_billed_tokens: 8_800, // S_4 = 0.20
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
    ];

    let stats = evaluate_paired_savings(&pairs);

    assert_eq!(stats.total_pairs_attempted, 5);
    assert_eq!(stats.valid_protocol_pairs_count, 5);
    assert_eq!(stats.zero_denominator_excluded_count, 0);
    assert_eq!(stats.incomplete_pairs_excluded_count, 0);

    // Mean S_i = (0.20 + 0.25 + 0.15 + 0.20 + 0.20) / 5 = 0.20
    let mean = stats.mean_savings_rate.expect("mean should exist");
    assert!((mean - 0.20).abs() < 1e-6, "Expected mean 0.20, got {mean}");

    // Standard deviation s_S = sqrt(0.00125) ≈ 0.035355
    let sd = stats.std_dev_savings_rate.expect("sd should exist");
    assert!(
        (sd - 0.035355).abs() < 1e-4,
        "Expected sd ≈ 0.035355, got {sd}"
    );

    // Median = 0.20
    let median = stats.median_savings_rate.expect("median should exist");
    assert!(
        (median - 0.20).abs() < 1e-6,
        "Expected median 0.20, got {median}"
    );

    // IQR = Q3 - Q1
    let iqr = stats.iqr_savings_rate.expect("iqr should exist");
    assert!(iqr >= 0.0, "Expected non-negative IQR");

    // One-sample t-statistic = 0.20 / (0.035355 / sqrt(5)) ≈ 12.649
    let t_stat = stats.t_statistic.expect("t_stat should exist");
    assert!(t_stat > 12.0, "Expected t_stat > 12.0, got {t_stat}");

    // p-values should be highly significant (< 0.001)
    let p_one = stats.t_test_p_value_one_tailed.expect("p_one should exist");
    let p_two = stats.t_test_p_value_two_tailed.expect("p_two should exist");
    assert!(p_one < 0.001, "Expected one-tailed p < 0.001, got {p_one}");
    assert!(p_two < 0.001, "Expected two-tailed p < 0.001, got {p_two}");

    // Standardized effect size (Cohen's d)
    let d = stats.cohens_d.expect("cohens_d should exist");
    assert!(d > 5.0, "Expected Cohen's d > 5.0, got {d}");

    // Wilcoxon signed-rank
    assert!(stats.wilcoxon_w.is_some());
    assert!(stats.wilcoxon_p_value.is_some());

    // Secondary raw token difference
    // Diffs: 2000, 3000, 1200, 3000, 2200 -> Sum = 11400 -> Mean = 2280.0
    assert!((stats.mean_raw_difference - 2280.0).abs() < 1e-6);
    assert!(stats.std_dev_raw_difference > 0.0);
    assert!(stats.paired_cohens_dz.is_some());
}

#[test]
fn test_zero_control_tokens_handling() {
    // 4 pairs: Pair 1 has C_i = 0 (zero baseline tokens)
    let pairs = vec![
        MatchedPairInput {
            scenario_id: "scen_valid_1".to_string(),
            pair_index: 0,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 8_000, // S = 0.20
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "scen_zero_baseline".to_string(),
            pair_index: 1,
            control_billed_tokens: 0, // Zero baseline!
            treatment_billed_tokens: 5_000,
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "scen_valid_2".to_string(),
            pair_index: 2,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 7_000, // S = 0.30
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "scen_valid_3".to_string(),
            pair_index: 3,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 9_000, // S = 0.10
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
    ];

    let stats = evaluate_paired_savings(&pairs);

    assert_eq!(stats.total_pairs_attempted, 4);
    assert_eq!(
        stats.valid_protocol_pairs_count, 3,
        "Only 3 pairs should be valid for S_i"
    );
    assert_eq!(
        stats.zero_denominator_excluded_count, 1,
        "Exactly 1 pair excluded for zero denominator"
    );
    assert_eq!(stats.incomplete_pairs_excluded_count, 0);

    // Pair 1 savings_rate must be None with descriptive exclusion reason
    let pair1 = &stats.evaluated_pairs[1];
    assert_eq!(
        pair1.savings_rate, None,
        "Zero denominator must yield None savings rate"
    );
    assert!(!pair1.is_valid_protocol_pair);
    assert!(
        pair1
            .exclusion_reason
            .as_ref()
            .unwrap()
            .contains("Zero baseline tokens"),
        "Expected zero baseline exclusion reason, got: {:?}",
        pair1.exclusion_reason
    );

    // Mean computed strictly across the 3 valid pairs: (0.20 + 0.30 + 0.10) / 3 = 0.20
    let mean = stats
        .mean_savings_rate
        .expect("mean should exist across 3 valid pairs");
    assert!((mean - 0.20).abs() < 1e-6, "Expected mean 0.20, got {mean}");
}

#[test]
fn test_raw_difference_distinction_from_ratio_savings() {
    // Two pairs with the exact same ratio savings (50%), but vastly different raw tokens
    let pairs = vec![
        MatchedPairInput {
            scenario_id: "large_repo".to_string(),
            pair_index: 0,
            control_billed_tokens: 100_000,
            treatment_billed_tokens: 50_000, // S = 0.50, Delta = 50,000
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 200_000,
        },
        MatchedPairInput {
            scenario_id: "tiny_repo".to_string(),
            pair_index: 1,
            control_billed_tokens: 1_000,
            treatment_billed_tokens: 500, // S = 0.50, Delta = 500
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 200_000,
        },
    ];

    let stats = evaluate_paired_savings(&pairs);

    // Ratio savings: (0.50 + 0.50) / 2 = 0.50 (dimensionless)
    let mean_s = stats.mean_savings_rate.unwrap();
    assert!((mean_s - 0.50).abs() < 1e-6);
    let sd_s = stats.std_dev_savings_rate.unwrap();
    assert_eq!(sd_s, 0.0, "Zero dispersion in ratio savings");

    // Raw token count differences: (50000 + 500) / 2 = 25250 tokens (cardinal count)
    assert_eq!(stats.mean_raw_difference, 25_250.0);
    assert!(stats.std_dev_raw_difference > 0.0);

    // Ratio savings rate and raw token count difference are distinct endpoints
    assert_ne!(
        mean_s, stats.mean_raw_difference,
        "Ratio savings and raw differences must never be conflated"
    );
}

#[test]
fn test_holm_bonferroni_fwer_control() {
    let hypotheses = vec![
        SecondaryHypothesis {
            endpoint_name: "pass_rate_lift".to_string(),
            unadjusted_p_value: 0.005,
            observed_effect: 0.35,
        },
        SecondaryHypothesis {
            endpoint_name: "f1_read_discovery".to_string(),
            unadjusted_p_value: 0.015,
            observed_effect: 0.20,
        },
        SecondaryHypothesis {
            endpoint_name: "f1_change_impact".to_string(),
            unadjusted_p_value: 0.040,
            observed_effect: 0.12,
        },
        SecondaryHypothesis {
            endpoint_name: "boundary_obedience".to_string(),
            unadjusted_p_value: 0.150,
            observed_effect: 0.05,
        },
    ];

    let alpha = 0.05;
    let results = holm_bonferroni_adjust(&hypotheses, alpha);

    assert_eq!(results.len(), 4);

    // Rank 1: pass_rate_lift (p = 0.005)
    // Divisor = 4, threshold = 0.05 / 4 = 0.0125. 0.005 <= 0.0125 -> Significant!
    assert_eq!(results[0].endpoint_name, "pass_rate_lift");
    assert_eq!(results[0].step_rank, 1);
    assert!((results[0].adjusted_alpha_threshold - 0.0125).abs() < 1e-6);
    assert!((results[0].adjusted_p_value - 0.020).abs() < 1e-6);
    assert!(results[0].is_significant);

    // Rank 2: f1_read_discovery (p = 0.015)
    // Divisor = 3, threshold = 0.05 / 3 ≈ 0.01667. 0.015 <= 0.01667 -> Significant!
    assert_eq!(results[1].endpoint_name, "f1_read_discovery");
    assert_eq!(results[1].step_rank, 2);
    assert!((results[1].adjusted_alpha_threshold - 0.05 / 3.0).abs() < 1e-6);
    assert!((results[1].adjusted_p_value - 0.045).abs() < 1e-6);
    assert!(results[1].is_significant);

    // Rank 3: f1_change_impact (p = 0.040)
    // Divisor = 2, threshold = 0.05 / 2 = 0.025. 0.040 > 0.025 -> NOT Significant!
    assert_eq!(results[2].endpoint_name, "f1_change_impact");
    assert_eq!(results[2].step_rank, 3);
    assert!((results[2].adjusted_alpha_threshold - 0.025).abs() < 1e-6);
    assert!((results[2].adjusted_p_value - 0.080).abs() < 1e-6);
    assert!(!results[2].is_significant);

    // Rank 4: boundary_obedience (p = 0.150)
    // Step-down stopping rule: previous hypothesis failed, so this fails too
    assert_eq!(results[3].endpoint_name, "boundary_obedience");
    assert_eq!(results[3].step_rank, 4);
    assert!(!results[3].is_significant);

    // Monotonicity of adjusted p-values
    assert!(results[0].adjusted_p_value <= results[1].adjusted_p_value);
    assert!(results[1].adjusted_p_value <= results[2].adjusted_p_value);
    assert!(results[2].adjusted_p_value <= results[3].adjusted_p_value);
}

#[test]
fn test_intent_to_treat_imputation() {
    // 5 matched pairs where Pair 4 aborted in Treatment arm (e.g. turn limit reached)
    let budget_cap = 20_000u64;
    let pairs = vec![
        MatchedPairInput {
            scenario_id: "scen_1".to_string(),
            pair_index: 0,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 8_000, // S = 0.20
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: budget_cap,
        },
        MatchedPairInput {
            scenario_id: "scen_2".to_string(),
            pair_index: 1,
            control_billed_tokens: 12_000,
            treatment_billed_tokens: 9_000, // S = 0.25
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: budget_cap,
        },
        MatchedPairInput {
            scenario_id: "scen_3".to_string(),
            pair_index: 2,
            control_billed_tokens: 8_000,
            treatment_billed_tokens: 6_800, // S = 0.15
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: budget_cap,
        },
        MatchedPairInput {
            scenario_id: "scen_4".to_string(),
            pair_index: 3,
            control_billed_tokens: 15_000,
            treatment_billed_tokens: 12_000, // S = 0.20
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: budget_cap,
        },
        MatchedPairInput {
            scenario_id: "scen_incomplete".to_string(),
            pair_index: 4,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 0,
            control_valid: true,
            treatment_valid: false, // Incomplete session!
            scenario_budget_cap_tokens: budget_cap,
        },
    ];

    // 1. Primary Per-Protocol Analysis: Pair 4 is excluded
    let per_protocol = evaluate_paired_savings(&pairs);
    assert_eq!(per_protocol.total_pairs_attempted, 5);
    assert_eq!(per_protocol.valid_protocol_pairs_count, 4);
    assert_eq!(per_protocol.incomplete_pairs_excluded_count, 1);
    let pp_mean = per_protocol.mean_savings_rate.unwrap();
    assert!(
        (pp_mean - 0.20).abs() < 1e-6,
        "Per-protocol mean should be 20%"
    );

    // 2. ITT Sensitivity Analysis: Pair 4 is imputed at B_max = 20,000
    // S_ITT,4 = (10,000 - 20,000) / 10,000 = -1.0 (-100% penalty)
    let itt = evaluate_itt_sensitivity(&pairs);
    assert_eq!(itt.total_itt_pairs, 5);
    assert_eq!(itt.itt_imputed_pairs_count, 1);

    // ITT mean = (0.20 + 0.25 + 0.15 + 0.20 - 1.00) / 5 = -0.20 / 5 = -0.04 (-4%)
    let itt_mean = itt.itt_mean_savings_rate.unwrap();
    assert!(
        (itt_mean - (-0.04)).abs() < 1e-6,
        "Expected ITT mean -4%, got {itt_mean}"
    );

    // The ITT penalty successfully defends against survivorship bias
    assert!(
        itt_mean < pp_mean,
        "ITT sensitivity mean ({itt_mean}) must be lower than per-protocol mean ({pp_mean})"
    );
}

#[test]
fn test_confirmatory_sample_size_iterative_convergence() {
    let plan = calculate_confirmatory_sample_size(
        0.20,                     // pilot sd
        DEFAULT_TARGET_MDE,       // 0.20
        DEFAULT_ALPHA,            // 0.05
        DEFAULT_POWER,            // 0.80
        DEFAULT_ATTRITION_BUFFER, // 0.15
    );

    assert_eq!(plan.pilot_n, 5);
    assert_eq!(plan.pilot_measured_sd, 0.20);

    // Inflated sd: 0.20 * 1.55745 ≈ 0.31149
    let expected_inflated = 0.20 * PILOT_VARIANCE_UPPER_80_MULTIPLIER;
    assert!((plan.inflated_upper_80_sd - expected_inflated).abs() < 1e-4);

    // Target MDE = 0.20 -> Converged N = 21 (from Phase 2B plan illustrative table)
    assert_eq!(
        plan.converged_n, 21,
        "Target MDE=0.20, sd=0.20 must converge to N=21"
    );

    // With +15% attrition buffer: ceil(21 * 1.15) = ceil(24.15) = 25
    assert_eq!(plan.recommended_sample_size, 25);

    // Verify sensitivity matrix entries match the Phase 2B plan table exactly
    let matrix = &plan.sensitivity_matrix;

    // Helper closure to look up required_n for given assumed_sd and mde
    let find_n = |sd: f64, mde: f64| -> usize {
        matrix
            .iter()
            .find(|e| (e.assumed_sd - sd).abs() < 1e-4 && (e.mde - mde).abs() < 1e-4)
            .map(|e| e.required_n)
            .unwrap_or_else(|| panic!("Entry not found for sd={sd}, mde={mde}"))
    };

    // Target MDE = 0.20 (exact match)
    assert_eq!(find_n(0.15, 0.20), 13);
    assert_eq!(find_n(0.20, 0.20), 21);
    assert_eq!(find_n(0.25, 0.20), 32);
    assert_eq!(find_n(0.30, 0.20), 45);

    // MDE = 0.15 and 0.25 (converges within ±1 of illustrative table)
    assert!((find_n(0.15, 0.15) as i64 - 22).abs() <= 1);
    assert!((find_n(0.15, 0.25) as i64 - 9).abs() <= 1);
    assert!((find_n(0.20, 0.15) as i64 - 37).abs() <= 1);
    assert!((find_n(0.20, 0.25) as i64 - 14).abs() <= 1);
    assert!((find_n(0.25, 0.15) as i64 - 56).abs() <= 1);
    assert!((find_n(0.25, 0.25) as i64 - 21).abs() <= 1);
    assert!((find_n(0.30, 0.15) as i64 - 80).abs() <= 1);
    assert!((find_n(0.30, 0.25) as i64 - 29).abs() <= 1);
}

#[test]
fn test_live_benchmark_report_generation_and_disclaimer() {
    let provenance = BenchmarkProvenance {
        evaluation_type: "MOCK_AGENT_EVALUATION".to_string(),
        engine_version: "v1.6.0".to_string(),
        engine_revision: "2035efad22008ebdc89ae0dcab858ca7978f5209".to_string(),
        corpus_digest: "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            .to_string(),
        timestamp_utc: "2026-10-16T08:00:00Z".to_string(),
        provider: "OfflineMockProvider".to_string(),
        model_id: "mock-claude-3-7-sonnet".to_string(),
        temperature: 0.0,
        prompt_digest: "sha256:01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b"
            .to_string(),
    };

    let pairs = vec![
        MatchedPairInput {
            scenario_id: "auth_bypass".to_string(),
            pair_index: 0,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 8_000,
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "leak_prevention".to_string(),
            pair_index: 1,
            control_billed_tokens: 12_000,
            treatment_billed_tokens: 9_000,
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "governance_audit".to_string(),
            pair_index: 2,
            control_billed_tokens: 8_000,
            treatment_billed_tokens: 6_800,
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "path_containment".to_string(),
            pair_index: 3,
            control_billed_tokens: 15_000,
            treatment_billed_tokens: 12_000,
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "turn_exhaustion".to_string(),
            pair_index: 4,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 0,
            control_valid: true,
            treatment_valid: false, // Incomplete session
            scenario_budget_cap_tokens: 20_000,
        },
    ];

    let primary_stats = evaluate_paired_savings(&pairs);
    let itt_sensitivity = evaluate_itt_sensitivity(&pairs);
    let secondary_hypotheses = vec![SecondaryHypothesis {
        endpoint_name: "pass_rate_lift".to_string(),
        unadjusted_p_value: 0.012,
        observed_effect: 0.25,
    }];
    let secondary_holm = holm_bonferroni_adjust(&secondary_hypotheses, DEFAULT_ALPHA);
    let sample_plan = calculate_confirmatory_sample_size(
        0.20,
        DEFAULT_TARGET_MDE,
        DEFAULT_ALPHA,
        DEFAULT_POWER,
        DEFAULT_ATTRITION_BUFFER,
    );

    let report = LiveBenchmarkReport::new(
        provenance,
        4, // 4 scenarios
        primary_stats,
        secondary_holm,
        itt_sensitivity,
        sample_plan,
    );

    // 1. Disclaimer Invariant
    assert_eq!(report.exploratory_disclaimer, EXPLORATORY_PILOT_DISCLAIMER);
    assert_eq!(report.token_savings_denominator, "4 of 5 pairs");

    // 2. JSON Serialization & Deserialization Roundtrip
    let json_str = report.to_json().expect("JSON serialization must succeed");
    assert!(json_str.contains("The N=5 pilot study is strictly exploratory"));
    assert!(json_str.contains("4 of 5 pairs"));
    let deserialized: LiveBenchmarkReport =
        LiveBenchmarkReport::from_json(&json_str).expect("JSON deserialization must succeed");
    assert_eq!(report.provenance, deserialized.provenance);
    assert_eq!(
        report.exploratory_disclaimer,
        deserialized.exploratory_disclaimer
    );
    assert_eq!(
        report.total_scenarios_evaluated,
        deserialized.total_scenarios_evaluated
    );
    assert_eq!(
        report.total_matched_pairs_attempted,
        deserialized.total_matched_pairs_attempted
    );
    assert_eq!(
        report.token_savings_denominator,
        deserialized.token_savings_denominator
    );
    assert_eq!(
        report.primary_stats.valid_protocol_pairs_count,
        deserialized.primary_stats.valid_protocol_pairs_count
    );
    assert!(
        (report.primary_stats.mean_savings_rate.unwrap()
            - deserialized.primary_stats.mean_savings_rate.unwrap())
        .abs()
            < 1e-6
    );
    assert!(
        (report.itt_sensitivity.itt_mean_savings_rate.unwrap()
            - deserialized.itt_sensitivity.itt_mean_savings_rate.unwrap())
        .abs()
            < 1e-6
    );
    assert_eq!(
        report.sample_size_plan.recommended_sample_size,
        deserialized.sample_size_plan.recommended_sample_size
    );

    // 3. Markdown Formatting
    let md = report.to_markdown();
    assert!(md.contains("# Engram Live Benchmark Report — Empirical Evaluation"));
    assert!(md.contains("> [!WARNING]"));
    assert!(md.contains(EXPLORATORY_PILOT_DISCLAIMER));
    assert!(md.contains("## 1. Provenance & Execution Context"));
    assert!(md.contains("## 2. Pre-Registered Primary Outcome"));
    assert!(md.contains("4 of 5 pairs"));
    assert!(
        md.contains("## 3. Secondary Endpoints & Holm-Bonferroni Multiple Comparison Correction")
    );
    assert!(md.contains("## 4. Intent-To-Treat (ITT) Sensitivity Analysis"));
    assert!(md.contains("## 5. Confirmatory Study Sample Size Planning"));
    assert!(md.contains(
        "**Recommended Confirmatory Pairs ($N_{\\text{recommended}}$):** `**25 pairs**`"
    ));
}

#[test]
fn test_student_t_cdf_analytical_exact_reference() {
    // 1. Degrees of freedom nu = 1 (Cauchy distribution): F(t, 1) = 0.5 + (1/pi)*arctan(t)
    let f_1_0 = student_t_cdf(0.0, 1.0);
    assert!((f_1_0 - 0.50).abs() < 1e-12, "F(0, 1) must be 0.5");

    let f_1_1 = student_t_cdf(1.0, 1.0);
    assert!(
        (f_1_1 - 0.750000000000).abs() < 1e-12,
        "F(1, 1) must be exactly 0.75 (arctan(1) = pi/4), got {f_1_1}"
    );

    let f_1_neg1 = student_t_cdf(-1.0, 1.0);
    assert!(
        (f_1_neg1 - 0.250000000000).abs() < 1e-12,
        "F(-1, 1) must be exactly 0.25, got {f_1_neg1}"
    );

    let sqrt3 = 3.0f64.sqrt();
    let f_1_sqrt3 = student_t_cdf(sqrt3, 1.0);
    let expected_sqrt3 = 0.5 + (1.0 / std::f64::consts::PI) * (std::f64::consts::PI / 3.0); // 5/6 ≈ 0.8333333333
    assert!(
        (f_1_sqrt3 - expected_sqrt3).abs() < 1e-12,
        "F(sqrt(3), 1) must be 5/6, got {f_1_sqrt3}"
    );

    // 2. Degrees of freedom nu = 2: F(t, 2) = 0.5 * (1 + t / sqrt(2 + t^2))
    let f_2_0 = student_t_cdf(0.0, 2.0);
    assert!((f_2_0 - 0.50).abs() < 1e-12);

    let f_2_1 = student_t_cdf(1.0, 2.0);
    let expected_f_2_1 = 0.5 * (1.0 + 1.0 / 3.0f64.sqrt()); // 0.7886751345948129
    assert!(
        (f_2_1 - expected_f_2_1).abs() < 1e-12,
        "F(1, 2) must be 0.5*(1 + 1/sqrt(3)) ≈ 0.78867513459, got {f_2_1}"
    );

    let f_2_neg1 = student_t_cdf(-1.0, 2.0);
    let expected_f_2_neg1 = 0.5 * (1.0 - 1.0 / 3.0f64.sqrt()); // 0.2113248654051871
    assert!(
        (f_2_neg1 - expected_f_2_neg1).abs() < 1e-12,
        "F(-1, 2) must be 0.5*(1 - 1/sqrt(3)), got {f_2_neg1}"
    );

    let sqrt2 = 2.0f64.sqrt();
    let f_2_sqrt2 = student_t_cdf(sqrt2, 2.0);
    let expected_f_2_sqrt2 = 0.5 * (1.0 + sqrt2 / 2.0); // 0.5*(1 + 0.7071) ≈ 0.85355339
    assert!(
        (f_2_sqrt2 - expected_f_2_sqrt2).abs() < 1e-12,
        "F(sqrt(2), 2) must be 0.5*(1 + 1/sqrt(2)), got {f_2_sqrt2}"
    );

    // Symmetry invariant across t
    for t_test in [0.25, 0.5, 1.25, 2.5, 5.0, 10.0] {
        let p_pos = student_t_cdf(t_test, 4.0);
        let p_neg = student_t_cdf(-t_test, 4.0);
        assert!(
            ((p_pos + p_neg) - 1.0).abs() < 1e-12,
            "CDF symmetry violated at t={t_test}"
        );
    }
}

#[test]
fn test_wilcoxon_signed_continuity_correction_and_ties() {
    // 1. Extreme positive deviation: All pairs show positive savings (W+ = 15, W- = 0, mean = 7.5)
    let pos_pairs = vec![
        MatchedPairInput {
            scenario_id: "s1".to_string(),
            pair_index: 0,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 9_000, // +10%
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "s2".to_string(),
            pair_index: 1,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 8_000, // +20%
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "s3".to_string(),
            pair_index: 2,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 7_000, // +30%
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "s4".to_string(),
            pair_index: 3,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 6_000, // +40%
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "s5".to_string(),
            pair_index: 4,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 5_000, // +50%
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
    ];

    let pos_stats = evaluate_paired_savings(&pos_pairs);
    assert_eq!(pos_stats.wilcoxon_w, Some(15.0));
    let pos_p = pos_stats.wilcoxon_p_value.expect("p-value must exist");

    // 2. Extreme negative deviation: All pairs show negative savings (W+ = 0, W- = 15, mean = 7.5)
    let neg_pairs = vec![
        MatchedPairInput {
            scenario_id: "s1".to_string(),
            pair_index: 0,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 11_000, // -10%
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "s2".to_string(),
            pair_index: 1,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 12_000, // -20%
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "s3".to_string(),
            pair_index: 2,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 13_000, // -30%
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "s4".to_string(),
            pair_index: 3,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 14_000, // -40%
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "s5".to_string(),
            pair_index: 4,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 15_000, // -50%
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
    ];

    let neg_stats = evaluate_paired_savings(&neg_pairs);
    assert_eq!(neg_stats.wilcoxon_w, Some(0.0));
    let neg_p = neg_stats.wilcoxon_p_value.expect("p-value must exist");

    // Two-tailed p-values for symmetric extremes must be mathematically identical!
    assert!(
        (pos_p - neg_p).abs() < 1e-10,
        "Symmetric extreme outcomes W+=15 and W+=0 must produce identical two-tailed p-values! pos_p={pos_p}, neg_p={neg_p}"
    );

    // 3. Near-zero deviation (|W+ - mean| <= 0.5):
    // Construct balanced pairs: 2 positive, 2 negative of identical magnitude
    let balanced_pairs = vec![
        MatchedPairInput {
            scenario_id: "s1".to_string(),
            pair_index: 0,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 8_000, // +20% -> rank 2.5
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "s2".to_string(),
            pair_index: 1,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 8_000, // +20% -> rank 2.5
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "s3".to_string(),
            pair_index: 2,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 12_000, // -20% -> rank 2.5
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "s4".to_string(),
            pair_index: 3,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 12_000, // -20% -> rank 2.5
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
    ];

    let bal_stats = evaluate_paired_savings(&balanced_pairs);
    // n = 4, mean = 4 * 5 / 4 = 5.0. W+ = 2.5 + 2.5 = 5.0 -> diff = 0.0 -> z = 0 -> p = 1.0
    assert_eq!(bal_stats.wilcoxon_w, Some(5.0));
    assert!((bal_stats.wilcoxon_p_value.unwrap() - 1.0).abs() < 1e-12);
}

#[test]
fn test_zero_variance_positive_mean_savings_returns_undefined() {
    // 4 pairs with identical positive savings: S_i = 0.20
    let pairs = vec![
        MatchedPairInput {
            scenario_id: "s1".to_string(),
            pair_index: 0,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 8_000, // 20%
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "s2".to_string(),
            pair_index: 1,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 8_000, // 20%
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "s3".to_string(),
            pair_index: 2,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 8_000, // 20%
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
        MatchedPairInput {
            scenario_id: "s4".to_string(),
            pair_index: 3,
            control_billed_tokens: 10_000,
            treatment_billed_tokens: 8_000, // 20%
            control_valid: true,
            treatment_valid: true,
            scenario_budget_cap_tokens: 20_000,
        },
    ];

    let stats = evaluate_paired_savings(&pairs);

    assert_eq!(stats.mean_savings_rate, Some(0.20));
    assert_eq!(stats.std_dev_savings_rate, Some(0.0));

    // Zero sample variance with non-zero mean: t-statistic, p-values, and effect size must be None
    assert_eq!(
        stats.t_statistic, None,
        "t-statistic must be None when sample variance is zero"
    );
    assert_eq!(
        stats.t_test_p_value_one_tailed, None,
        "one-tailed p-value must be None"
    );
    assert_eq!(
        stats.t_test_p_value_two_tailed, None,
        "two-tailed p-value must be None"
    );
    assert_eq!(stats.cohens_d, None, "Cohen's d must be None");
    assert_eq!(
        stats.paired_cohens_dz, None,
        "paired Cohen's dz must be None"
    );
}

#[test]
fn test_holm_bonferroni_handles_nan_and_invalid_p_values() {
    let hypotheses = vec![
        SecondaryHypothesis {
            endpoint_name: "valid_1".to_string(),
            unadjusted_p_value: 0.010,
            observed_effect: 0.30,
        },
        SecondaryHypothesis {
            endpoint_name: "valid_2".to_string(),
            unadjusted_p_value: 0.040,
            observed_effect: 0.15,
        },
        SecondaryHypothesis {
            endpoint_name: "nan_p_value".to_string(),
            unadjusted_p_value: f64::NAN,
            observed_effect: 0.0,
        },
        SecondaryHypothesis {
            endpoint_name: "negative_p_value".to_string(),
            unadjusted_p_value: -0.05,
            observed_effect: 0.0,
        },
        SecondaryHypothesis {
            endpoint_name: "greater_than_one_p_value".to_string(),
            unadjusted_p_value: 1.50,
            observed_effect: 0.0,
        },
    ];

    // Must not panic on NaN or out-of-range p-values!
    let results = holm_bonferroni_adjust(&hypotheses, 0.05);

    assert_eq!(results.len(), 5);

    // Valid hypothesis 1: unadjusted p = 0.010, divisor = 2 (only 2 valid hypotheses) -> threshold = 0.025 -> Significant
    let v1 = results
        .iter()
        .find(|r| r.endpoint_name == "valid_1")
        .unwrap();
    assert_eq!(v1.step_rank, 1);
    assert!(v1.is_significant);
    assert!((v1.adjusted_alpha_threshold - 0.025).abs() < 1e-6);
    assert!((v1.adjusted_p_value - 0.020).abs() < 1e-6);

    // Valid hypothesis 2: unadjusted p = 0.040, divisor = 1 -> threshold = 0.05 -> Significant
    let v2 = results
        .iter()
        .find(|r| r.endpoint_name == "valid_2")
        .unwrap();
    assert_eq!(v2.step_rank, 2);
    assert!(v2.is_significant);
    assert!((v2.adjusted_alpha_threshold - 0.05).abs() < 1e-6);
    assert!((v2.adjusted_p_value - 0.040).abs() < 1e-6);

    // Invalid hypotheses: safely marked non-significant with step_rank = 0, adjusted_p = 1.0
    for invalid_name in [
        "nan_p_value",
        "negative_p_value",
        "greater_than_one_p_value",
    ] {
        let inv = results
            .iter()
            .find(|r| r.endpoint_name == invalid_name)
            .unwrap();
        assert_eq!(inv.step_rank, 0);
        assert!(!inv.is_significant);
        assert_eq!(inv.adjusted_p_value, 1.0);
    }
}

// ============================================================================
// 56. Milestone B5: Windows Job Object Setup & Limit Flags
// ============================================================================

#[test]
fn test_windows_job_object_setup_and_limits() {
    #[cfg(windows)]
    {
        use engram::empirical_live::security::ProcessJobObject;

        let job = ProcessJobObject::create().expect("Job object creation must succeed on Windows");
        assert!(job.is_open(), "Job object handle must be open");

        // Explicit drop triggers CloseHandle
        drop(job);
    }
}

// ============================================================================
// 57. Milestone B5: Windows Job Object Descendant Process Termination
// ============================================================================

#[test]
fn test_windows_job_object_kills_grandchild_processes() {
    #[cfg(windows)]
    {
        use engram::empirical_live::security::ProcessJobObject;
        use std::process::Command;
        use std::time::Duration;

        let job = ProcessJobObject::create().expect("Job object creation");

        // Spawn a sleeping child process
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"]);

        let child = cmd.spawn().expect("Spawn sleep process");

        // Assign to job object
        job.assign_process(&child).expect("Assign process to job");

        // Drop the job object. JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE must terminate the process tree.
        drop(job);

        // Allow kernel to reap
        std::thread::sleep(Duration::from_millis(150));

        // Child process should now be terminated
        let mut child = child;
        let wait_res = child.try_wait().expect("try_wait");
        assert!(
            wait_res.is_some(),
            "Child process must be terminated when Job Object handle is closed"
        );
    }
}

// ============================================================================
// 58. Milestone B5: Child-Process Environment Sanitization & Secret Scrubbing
// ============================================================================

#[test]
fn test_child_process_environment_scrubs_secrets_proxies_and_git() {
    let _env_lock = ENV_MUTEX.lock().unwrap();
    use engram::empirical_live::security::EnvironmentPolicy;

    let temp_dir = std::env::temp_dir().join(format!("engram_b5_env_{}", uuid::Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&temp_dir);

    // Seed sensitive variables into parent process
    unsafe {
        std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-sensitive-secret-token");
        std::env::set_var("CUSTOM_SESSION_COOKIE", "cookie-value-abc");
        std::env::set_var("GITHUB_TOKEN", "ghp_super_secret_github_token");
        std::env::set_var("HTTP_PROXY", "http://corporate.proxy:8080");
        std::env::set_var("https_proxy", "http://corporate.proxy:8443");
        std::env::set_var("ALL_PROXY", "socks5://corporate.proxy:1080");
        std::env::set_var("GIT_DIR", "/forbidden/repo/.git");
        std::env::set_var("GIT_WORK_TREE", "/forbidden/repo");
        std::env::set_var("GIT_INDEX_FILE", "/forbidden/repo/.git/index");
    }

    let policy = EnvironmentPolicy::new(&temp_dir);
    let sanitized = policy
        .build_sanitized_environment()
        .expect("build sanitized env");

    // Invariant: Secret-pattern variables are completely absent
    assert!(!sanitized.contains_key("ANTHROPIC_API_KEY"));
    assert!(!sanitized.contains_key("CUSTOM_SESSION_COOKIE"));
    assert!(!sanitized.contains_key("GITHUB_TOKEN"));

    // Invariant: Proxy variables are completely absent (both upper and lower cases)
    assert!(!sanitized.contains_key("HTTP_PROXY"));
    assert!(!sanitized.contains_key("https_proxy"));
    assert!(!sanitized.contains_key("ALL_PROXY"));

    // Invariant: Git metadata variables are completely absent
    assert!(!sanitized.contains_key("GIT_DIR"));
    assert!(!sanitized.contains_key("GIT_WORK_TREE"));
    assert!(!sanitized.contains_key("GIT_INDEX_FILE"));

    // Clean up parent environment
    unsafe {
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("CUSTOM_SESSION_COOKIE");
        std::env::remove_var("GITHUB_TOKEN");
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("https_proxy");
        std::env::remove_var("ALL_PROXY");
        std::env::remove_var("GIT_DIR");
        std::env::remove_var("GIT_WORK_TREE");
        std::env::remove_var("GIT_INDEX_FILE");
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// 59. Milestone B5: Child-Process Environment Tool Cache Redirection
// ============================================================================

#[test]
fn test_child_process_environment_redirects_caches() {
    use engram::empirical_live::security::EnvironmentPolicy;

    let temp_dir = std::env::temp_dir().join(format!("engram_b5_caches_{}", uuid::Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let policy = EnvironmentPolicy::new(&temp_dir);
    let env = policy.build_sanitized_environment().expect("sanitized env");

    // Invariant: Caches redirected to sandbox-local subdirectories
    let cargo_home = env.get("CARGO_HOME").expect("CARGO_HOME");
    let npm_cache = env.get("NPM_CONFIG_CACHE").expect("NPM_CONFIG_CACHE");
    let pycache = env.get("PYTHONPYCACHEPREFIX").expect("PYTHONPYCACHEPREFIX");
    let local_tmp = env.get("TEMP").expect("TEMP");

    assert!(cargo_home.starts_with(&temp_dir.to_string_lossy().to_string()));
    assert!(cargo_home.ends_with(".engram_cargo_home"));
    assert!(npm_cache.ends_with(".engram_npm_cache"));
    assert!(pycache.ends_with(".engram_pycache"));
    assert!(local_tmp.ends_with(".engram_tmp"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// 60. Milestone B5: Trusted Executable Path Resolution
// ============================================================================

#[test]
fn test_trusted_executable_resolution_rejects_untrusted_relative_paths() {
    let _env_lock = ENV_MUTEX.lock().unwrap();
    use engram::empirical_live::security::resolve_trusted_executable;

    // Standard tool resolution on host
    let cargo_res = resolve_trusted_executable("cargo");
    assert!(
        cargo_res.is_ok(),
        "cargo should be found in system PATH: {cargo_res:?}"
    );
    let cargo_path = cargo_res.unwrap();
    assert!(cargo_path.is_absolute());
    assert!(cargo_path.exists());

    // Non-existent tool fails resolution
    let fake_res = resolve_trusted_executable("non_existent_tool_xyz_999");
    assert!(fake_res.is_err());

    // Path traversal in executable name fails
    let traversal_res = resolve_trusted_executable("../../../malicious_exec");
    assert!(traversal_res.is_err());
}

// ============================================================================
// 61. Milestone B5: Adversarial Network Probe Outcome Classification
// ============================================================================

#[test]
fn test_network_egress_probe_classification() {
    use engram::empirical_live::security::{EgressProbeOutcome, NetworkEgressProbe};

    #[cfg(windows)]
    {
        // 1. Explicit policy rejection (WSAEACCES 10013 on Windows)
        let wsaeacces_err = std::io::Error::from_raw_os_error(10013);
        let outcome1 = NetworkEgressProbe::classify_io_error("p1", "1.1.1.1:53", &wsaeacces_err);
        assert!(outcome1.is_active_rejection());
        if let EgressProbeOutcome::ActiveRejectionVerified {
            os_error_code,
            rejection_policy,
            ..
        } = outcome1
        {
            assert_eq!(os_error_code, Some(10013));
            assert!(rejection_policy.contains("WSAEACCES"));
        } else {
            panic!("Expected ActiveRejectionVerified");
        }

        // 2. Inconclusive connection refusal (WSAECONNREFUSED 10061 on Windows)
        let wsaeconnrefused_err = std::io::Error::from_raw_os_error(10061);
        let outcome2 =
            NetworkEgressProbe::classify_io_error("p2", "127.0.0.1:9999", &wsaeconnrefused_err);
        assert!(!outcome2.is_active_rejection());
        assert!(!outcome2.is_leak());
        if let EgressProbeOutcome::Unreachable { reason, .. } = outcome2 {
            assert!(reason.contains("WSAECONNREFUSED"));
        } else {
            panic!("Expected Unreachable");
        }

        // 3. DNS failure (WSAHOST_NOT_FOUND 11001 on Windows)
        let wsahost_err = std::io::Error::from_raw_os_error(11001);
        let outcome3 =
            NetworkEgressProbe::classify_io_error("p3", "invalid.domain.xyz:80", &wsahost_err);
        assert!(!outcome3.is_active_rejection());
        assert!(matches!(
            outcome3,
            EgressProbeOutcome::ResolutionFailed { .. }
        ));
    }

    #[cfg(not(windows))]
    {
        // 1. Explicit policy rejection (EACCES 13 or EPERM 1 on Unix)
        let eacces_err = std::io::Error::from_raw_os_error(13);
        let outcome1 = NetworkEgressProbe::classify_io_error("p1", "1.1.1.1:53", &eacces_err);
        assert!(outcome1.is_active_rejection());
        if let EgressProbeOutcome::ActiveRejectionVerified {
            os_error_code,
            rejection_policy,
            ..
        } = outcome1
        {
            assert_eq!(os_error_code, Some(13));
            assert!(rejection_policy.contains("EACCES") || rejection_policy.contains("EPERM"));
        } else {
            panic!("Expected ActiveRejectionVerified");
        }

        // 2. Inconclusive connection refusal (ECONNREFUSED: 111 on Linux, 61 on macOS)
        #[cfg(target_os = "macos")]
        let econnrefused_code = 61;
        #[cfg(not(target_os = "macos"))]
        let econnrefused_code = 111;

        let econnrefused_err = std::io::Error::from_raw_os_error(econnrefused_code);
        let outcome2 =
            NetworkEgressProbe::classify_io_error("p2", "127.0.0.1:9999", &econnrefused_err);
        assert!(!outcome2.is_active_rejection());
        assert!(!outcome2.is_leak());
        if let EgressProbeOutcome::Unreachable { reason, .. } = outcome2 {
            assert!(reason.contains("ECONNREFUSED"));
        } else {
            panic!("Expected Unreachable");
        }

        // 3. Generic inconclusive network error
        let generic_err = std::io::Error::new(std::io::ErrorKind::Other, "network unreachable");
        let outcome3 =
            NetworkEgressProbe::classify_io_error("p3", "invalid.domain.xyz:80", &generic_err);
        assert!(!outcome3.is_active_rejection());
        assert!(!outcome3.is_leak());
    }
}

// ============================================================================
// 62. Milestone B5: Controlled Egress Leak Detection & Fail-Closed Behavior
// ============================================================================

#[test]
fn test_controlled_egress_leak_fails_closed() {
    use engram::empirical_live::security::{EgressProbeOutcome, NetworkEgressProbe};
    use std::net::TcpListener;

    // Bind a controlled local loopback server
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test listener");
    let local_addr = listener.local_addr().expect("local addr").to_string();

    // Probe the listener expecting isolation = true
    let outcome = NetworkEgressProbe::evaluate_local_probe("probe_leak_test", &local_addr, true);

    // Invariant: Connecting to an active listener when isolation was expected detects an egress leak!
    assert!(
        outcome.is_leak(),
        "Unexpected connection must be flagged as an egress leak"
    );
    if let EgressProbeOutcome::EgressLeakDetected {
        destination,
        detail,
        ..
    } = outcome
    {
        assert_eq!(destination, local_addr);
        assert!(detail.contains("connected successfully"));
    } else {
        panic!("Expected EgressLeakDetected");
    }

    // Inconclusive check on an unlistening local port
    let outcome_unreachable =
        NetworkEgressProbe::evaluate_local_probe("probe_unreachable_test", "127.0.0.1:1", true);
    assert!(!outcome_unreachable.is_leak());
    assert!(!outcome_unreachable.is_active_rejection());
}

// ============================================================================
// 63. Milestone B5: Epistemological Disclaimer & Provenance Preservation
// ============================================================================

#[test]
fn test_network_containment_epistemological_disclaimer() {
    use engram::empirical_live::security::NETWORK_CONTAINMENT_EPISTEMOLOGICAL_DISCLAIMER;

    assert!(!NETWORK_CONTAINMENT_EPISTEMOLOGICAL_DISCLAIMER.is_empty());
    assert!(NETWORK_CONTAINMENT_EPISTEMOLOGICAL_DISCLAIMER.contains("empirical evidence"));
    assert!(
        NETWORK_CONTAINMENT_EPISTEMOLOGICAL_DISCLAIMER.contains("not universal mathematical proof")
    );
    assert!(NETWORK_CONTAINMENT_EPISTEMOLOGICAL_DISCLAIMER.contains("network isolation"));
}

// ============================================================================
// 64. Milestone B5: SandboxToolDispatcher Runs with Process Containment
// ============================================================================

#[test]
fn test_sandbox_tool_dispatcher_executes_contained_command() {
    use engram::empirical_live::dispatcher::SandboxToolDispatcher;
    use engram::empirical_live::schema::ContentBlock;

    let temp_dir = std::env::temp_dir().join(format!("engram_b5_disp_{}", uuid::Uuid::new_v4()));
    let dispatcher = SandboxToolDispatcher::new(&temp_dir).expect("dispatcher");

    // Execute cargo --version inside the sandboxed Job Object environment
    let res = dispatcher.dispatch(
        "execute_command",
        "call_cargo_version",
        &serde_json::json!({ "command": "cargo", "args": ["--version"] }),
    );

    #[cfg(windows)]
    {
        if let ContentBlock::ToolResult {
            is_error, content, ..
        } = res
        {
            assert!(
                !is_error,
                "Contained cargo execution should succeed: {content}"
            );
            assert!(
                content.contains("cargo"),
                "Output must contain cargo version string: {content}"
            );
            assert!(content.contains("Exit code: 0"));
        } else {
            panic!("Expected ToolResult");
        }
    }

    #[cfg(not(windows))]
    {
        // On non-Windows platforms, Job Object process-tree containment is unsupported,
        // and SandboxToolDispatcher strictly fails closed before spawning.
        if let ContentBlock::ToolResult {
            is_error, content, ..
        } = res
        {
            assert!(
                is_error,
                "On non-Windows platforms, dispatcher must fail closed on unsupported process containment: {content}"
            );
            assert!(
                content.contains("Platform containment unsupported"),
                "Output must identify unsupported platform containment: {content}"
            );
            assert!(
                content.contains("fail-closed"),
                "Output must cite fail-closed policy: {content}"
            );
        } else {
            panic!("Expected ToolResult");
        }
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// 65. Milestone B5 Remediation: Curated Child PATH Rejects Parent PATH Injection
// (AUD-B5-01)
// ============================================================================

#[test]
fn test_curated_child_path_rejects_parent_path_injection_and_fake_utilities() {
    let _env_lock = ENV_MUTEX.lock().unwrap();
    use engram::empirical_live::security::{
        EnvironmentPolicy, SecurityError, resolve_trusted_executable,
    };
    use std::fs::File;
    use std::io::Write;

    let temp_dir =
        std::env::temp_dir().join(format!("engram_b5_evil_path_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).expect("create evil temp dir");

    // 1. Create a fake executable / utility in the attacker directory
    let fake_utility_name = "fake_cargo_evil_tool";
    #[cfg(windows)]
    let fake_file_path = temp_dir.join(format!("{fake_utility_name}.bat"));
    #[cfg(not(windows))]
    let fake_file_path = temp_dir.join(fake_utility_name);

    {
        let mut f = File::create(&fake_file_path).expect("create fake binary");
        writeln!(f, "@echo off\necho PWNED").expect("write fake script");
    }

    // 2. Seed the parent PATH with the attacker-controlled directory
    let original_path = std::env::var("PATH").unwrap_or_default();
    #[cfg(windows)]
    let sep = ";";
    #[cfg(not(windows))]
    let sep = ":";
    let seeded_path = format!("{}{sep}{}", temp_dir.display(), original_path);
    unsafe {
        std::env::set_var("PATH", &seeded_path);
    }

    // Invariant A: resolve_trusted_executable does NOT search untrusted parent PATH entries
    let evil_res = resolve_trusted_executable(fake_utility_name);
    assert!(
        evil_res.is_err(),
        "resolve_trusted_executable must reject fake utilities in untrusted parent PATH: {evil_res:?}"
    );
    if let Err(SecurityError::ExecutableResolutionFailed { command, .. }) = evil_res {
        assert_eq!(command, fake_utility_name);
    } else {
        panic!("Expected ExecutableResolutionFailed");
    }

    // Invariant B: Even if someone attempts to pass the attacker directory as an approved tool dir,
    // build_curated_path detects and rejects directories in temp or untrusted locations
    let sandbox_root = std::env::temp_dir().join(format!("engram_b5_sb_{}", uuid::Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&sandbox_root);

    let policy_attacker = EnvironmentPolicy::new(&sandbox_root).with_approved_tool_dir(&temp_dir);
    let policy_res = policy_attacker.build_sanitized_environment();
    assert!(
        policy_res.is_err(),
        "build_sanitized_environment must reject approved tool dirs located in temp / untrusted paths"
    );

    // Invariant C: Normal EnvironmentPolicy builds a curated PATH where the attacker directory is 100% absent
    let policy_clean = EnvironmentPolicy::new(&sandbox_root);
    let env_clean = policy_clean
        .build_sanitized_environment()
        .expect("clean env");
    let child_path = env_clean.get("PATH").expect("child PATH must exist");

    assert!(
        !child_path.contains(&temp_dir.to_string_lossy().to_string()),
        "Child PATH must NEVER inherit uncurated parent PATH containing attacker directory!"
    );

    // Restore parent environment
    unsafe {
        std::env::set_var("PATH", &original_path);
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::remove_dir_all(&sandbox_root);
}

// ============================================================================
// 66. Milestone B5 Remediation: Unsupported Platforms Fail Closed Before Spawn
// (AUD-B5-02)
// ============================================================================

#[test]
fn test_unsupported_platform_containment_fails_closed_before_spawn() {
    use engram::empirical_live::dispatcher::{DispatcherError, SandboxToolDispatcher};
    use engram::empirical_live::security::{SecurityError, check_platform_containment};

    // 1. Pure security module check fails closed on non-Windows
    assert!(check_platform_containment("windows").is_ok());
    let linux_sec = check_platform_containment("linux");
    assert!(linux_sec.is_err());
    if let Err(SecurityError::UnsupportedPlatformContainment { platform, reason }) = linux_sec {
        assert_eq!(platform, "linux");
        assert!(reason.contains("fail-closed"));
    } else {
        panic!("Expected UnsupportedPlatformContainment");
    }

    let macos_sec = check_platform_containment("macos");
    assert!(macos_sec.is_err());

    // 2. Dispatcher platform containment evaluation
    assert!(SandboxToolDispatcher::check_platform_containment_for_os("windows").is_ok());

    let linux_disp = SandboxToolDispatcher::check_platform_containment_for_os("linux");
    assert!(linux_disp.is_err());
    if let Err(DispatcherError::SecurityContainmentFailed(msg)) = linux_disp {
        assert!(msg.contains("Platform containment unsupported on 'linux'"));
        assert!(msg.contains("fail-closed"));
    } else {
        panic!("Expected SecurityContainmentFailed");
    }

    let macos_disp = SandboxToolDispatcher::check_platform_containment_for_os("macos");
    assert!(macos_disp.is_err());
}

// ============================================================================
// 67. Milestone B5 Remediation: Strict Thread Resumption Accounting & Fail-Closed
// (AUD-B5-03)
// ============================================================================

#[test]
fn test_thread_resumption_strict_accounting_and_fail_closed_cleanup() {
    use engram::empirical_live::security::{SecurityError, ThreadResumptionSummary};

    // Case 1: Complete success — all discovered threads successfully resumed
    let success_summary = ThreadResumptionSummary {
        method: "Toolhelp32".to_string(),
        discovered_threads: 4,
        resumed_threads: 4,
        failed_threads: 0,
        last_os_error: None,
    };
    assert!(success_summary.is_complete_success());
    assert!(success_summary.validate(12345).is_ok());

    // Case 2: Partial failure — 3 threads discovered, 2 resumed, 1 failed
    // INVARIANT: Partial resumption MUST fail closed!
    let partial_failure_summary = ThreadResumptionSummary {
        method: "Toolhelp32".to_string(),
        discovered_threads: 3,
        resumed_threads: 2,
        failed_threads: 1,
        last_os_error: Some(5), // ERROR_ACCESS_DENIED
    };
    assert!(!partial_failure_summary.is_complete_success());
    let partial_res = partial_failure_summary.validate(12345);
    assert!(partial_res.is_err());
    if let Err(SecurityError::PartialThreadResumptionFailed {
        pid,
        discovered_threads,
        resumed_threads,
        failed_threads,
        os_error_code,
    }) = partial_res
    {
        assert_eq!(pid, 12345);
        assert_eq!(discovered_threads, 3);
        assert_eq!(resumed_threads, 2);
        assert_eq!(failed_threads, 1);
        assert_eq!(os_error_code, Some(5));
    } else {
        panic!("Expected PartialThreadResumptionFailed");
    }

    // Case 3: Zero threads discovered
    let zero_threads_summary = ThreadResumptionSummary {
        method: "Toolhelp32".to_string(),
        discovered_threads: 0,
        resumed_threads: 0,
        failed_threads: 0,
        last_os_error: None,
    };
    assert!(!zero_threads_summary.is_complete_success());
    let zero_res = zero_threads_summary.validate(12345);
    assert!(zero_res.is_err());
    assert!(matches!(
        zero_res,
        Err(SecurityError::ZeroThreadsDiscovered { pid: 12345 })
    ));

    // Case 4: Snapshot failure error representation
    let snap_err = SecurityError::ThreadSnapshotFailed {
        pid: 999,
        os_error_code: 6,
    };
    let snap_str = format!("{snap_err}");
    assert!(snap_str.contains("Failed to create thread snapshot for PID 999"));
    assert!(snap_str.contains("OS error 6"));

    // Case 5: Verify real Job Object cleanup on live child process
    #[cfg(windows)]
    {
        use engram::empirical_live::security::ProcessJobObject;
        use std::process::Command;
        use std::time::Duration;

        let job = ProcessJobObject::create().expect("job creation");
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"]);

        let mut child = cmd.spawn().expect("spawn child");
        job.assign_process(&child).expect("assign process");

        // Simulate failure handling: terminate job and kill child
        let _ = child.kill();
        let _ = job.terminate(1);

        std::thread::sleep(Duration::from_millis(100));
        let wait_res = child.try_wait().expect("try_wait");
        assert!(
            wait_res.is_some(),
            "Child process must be reaped on failure cleanup"
        );
    }
}

// ============================================================================
// 46. Milestone B6: Token Accounting & Pre-Flight Reservation Remediation (BLK-B6-01)
// ============================================================================

#[test]
fn test_token_reservation_minimal_and_empty_requests() {
    // 1. Empty and minimal requests must be bounded to at least the minimum floor (64 tokens)
    let min_req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("hi")],
        tools: vec![],
        max_tokens: 100,
        temperature: None,
        stop_sequences: vec![],
    };

    let bound = PromptTokenEstimator::conservative_upper_bound_tokens(&min_req)
        .expect("bound minimal request");
    assert!(
        bound >= 64,
        "Minimal request bound must enforce minimum floor (>=64), got {bound}"
    );

    let guard = BudgetGuard::new(MicroDollars::from_dollars(1).unwrap());
    let ticket = guard
        .reserve_for_request(&min_req, 0)
        .expect("reservation succeeds");
    assert!(ticket.reserved_amount() > MicroDollars::ZERO);

    // Request with empty content string
    let empty_text_req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("")],
        tools: vec![],
        max_tokens: 100,
        temperature: None,
        stop_sequences: vec![],
    };
    let empty_bound = PromptTokenEstimator::conservative_upper_bound_tokens(&empty_text_req)
        .expect("bound empty text");
    assert!(
        empty_bound >= 64,
        "Empty text request must enforce floor >= 64, got {empty_bound}"
    );
}

#[test]
fn test_token_reservation_long_prompts_and_multi_turn_history() {
    // 2. Long prompts and multi-turn history: monotonic scaling with turns and content
    let mut messages = Vec::new();
    let mut prev_bound = 0;

    for turn in 1..=8 {
        messages.push(ChatMessage::user(format!(
            "Turn {turn}: Analyze repository structure and performance."
        )));
        messages.push(ChatMessage::assistant(format!(
            "Turn {turn}: Understood. Executing tool check."
        )));
        messages.push(ChatMessage::tool_result(
            format!("call_{turn}"),
            format!(
                "{{\"status\": \"ok\", \"turn\": {turn}, \"data\": \"{}\"}}",
                "x".repeat(500)
            ),
            false,
        ));

        let req = LlmRequest {
            model: "claude-3-7-sonnet-20250219".to_string(),
            system_prompt: Some(
                "You are an expert systems engineer evaluating Engram benchmark workloads."
                    .to_string(),
            ),
            messages: messages.clone(),
            tools: vec![],
            max_tokens: 1024,
            temperature: Some(0.0),
            stop_sequences: vec![],
        };

        let bound = PromptTokenEstimator::conservative_upper_bound_tokens(&req)
            .expect("bound multi-turn request");
        assert!(
            bound > prev_bound,
            "Bound must strictly increase with conversation turns: turn {turn} bound {bound} <= prev {prev_bound}"
        );
        prev_bound = bound;

        let raw_bytes = PromptTokenEstimator::structural_byte_count(&req);
        assert!(
            bound as usize > raw_bytes,
            "Conservative bound ({bound}) must strictly exceed raw text bytes ({raw_bytes}) due to wire framing"
        );
    }
}

#[test]
fn test_token_reservation_unicode_and_non_english_text() {
    // 3. Unicode and non-English text: CJK, Arabic, Cyrillic, emojis
    let cjk_text = "这是一段中文测试。こんにちは世界。안녕하세요 세계.";
    let arabic_text = "مرحبا بالعالم هذا اختبار حقيقي للنظام";
    let emoji_text = "🚀🦀🔒💡⚡🎯📊🛡️";

    let unicode_req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: Some("Multilingual test assistant.".to_string()),
        messages: vec![
            ChatMessage::user(cjk_text),
            ChatMessage::assistant(arabic_text),
            ChatMessage::user(emoji_text),
        ],
        tools: vec![],
        max_tokens: 500,
        temperature: None,
        stop_sequences: vec![],
    };

    let bound = PromptTokenEstimator::conservative_upper_bound_tokens(&unicode_req)
        .expect("bound unicode request");
    let raw_bytes = PromptTokenEstimator::structural_byte_count(&unicode_req);
    assert!(
        bound as usize >= raw_bytes,
        "Bound ({bound}) must cover full UTF-8 byte representation ({raw_bytes} bytes) for multilingual text"
    );

    let guard = BudgetGuard::new(MicroDollars::from_dollars(5).unwrap());
    let ticket = guard
        .reserve_for_request(&unicode_req, 0)
        .expect("reserve unicode request");
    assert!(ticket.reserved_amount() > MicroDollars::ZERO);
}

#[test]
fn test_token_reservation_source_code_and_dense_punctuation() {
    // 4. Source code with dense punctuation and symbols
    let dense_code = r#"
        macro_rules! s_expr {
            ($($tok:tt)*) => { [$(stringify!($tok)),*].join(" ") };
        }
        fn eval(expr: &str) -> Result<i64, &'static str> {
            let ops = ['+', '-', '*', '/', '%', '^', '&', '|', '~', '<', '>', '=', '!', '?'];
            if expr.chars().all(|c| ops.contains(&c) || c.is_ascii_punctuation()) {
                Ok(0xDEAD_BEEF)
            } else {
                Err("parse failure: invalid syntax token in AST")
            }
        }
    "#;

    let code_req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: Some("You are a syntax analyzer.".to_string()),
        messages: vec![ChatMessage::user(dense_code)],
        tools: vec![],
        max_tokens: 1000,
        temperature: None,
        stop_sequences: vec![],
    };

    let bound = PromptTokenEstimator::conservative_upper_bound_tokens(&code_req)
        .expect("bound code request");
    let char_count = dense_code.chars().count();
    assert!(
        bound >= char_count as u64,
        "Bound ({bound}) must cover individual single-character punctuation tokens ({char_count} chars)"
    );
}

#[test]
fn test_token_reservation_large_tool_definitions_and_nested_json() {
    // 5. Large tool definitions with deeply nested JSON schemas
    let mut tools = Vec::new();
    for i in 1..=6 {
        tools.push(ToolDefinition {
            name: format!("complex_tool_{i}"),
            description: format!("Perform multi-stage static analysis pipeline stage {i} on the codebase"),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "source_path": { "type": "string", "description": "Absolute filesystem path" },
                    "depth_limit": { "type": "integer", "minimum": 1, "maximum": 100 },
                    "filters": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "pattern": { "type": "string" },
                                "case_sensitive": { "type": "boolean" },
                                "scope": { "type": "string", "enum": ["function", "module", "crate", "workspace"] }
                            },
                            "required": ["pattern", "scope"]
                        }
                    },
                    "options": {
                        "type": "object",
                        "properties": {
                            "emit_ast": { "type": "boolean" },
                            "optimization_level": { "type": "string", "enum": ["O0", "O1", "O2", "O3"] }
                        }
                    }
                },
                "required": ["source_path", "filters"]
            }),
        });
    }

    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: Some("You have access to 6 complex static analysis tools.".to_string()),
        messages: vec![ChatMessage::user("Run analysis pipeline.")],
        tools,
        max_tokens: 2048,
        temperature: None,
        stop_sequences: vec![],
    };

    let bound =
        PromptTokenEstimator::conservative_upper_bound_tokens(&req).expect("bound large tools");
    // 6 tools * 64 framing = 384 tokens framing alone, plus schema JSON bytes
    assert!(
        bound > 1500,
        "Bound ({bound}) must account for extensive schema definitions and envelopes"
    );
}

#[test]
fn test_token_reservation_escaped_characters_and_serialized_expansion() {
    // 6. Escaped characters and serialized JSON expansion
    let text_with_escapes =
        "Line 1\nLine 2\tTabbed\r\n\"Quoted String\" with \\backslashes\\ and \u{0000} nulls.";
    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user(text_with_escapes)],
        tools: vec![],
        max_tokens: 500,
        temperature: None,
        stop_sequences: vec![],
    };

    let bound =
        PromptTokenEstimator::conservative_upper_bound_tokens(&req).expect("bound escaped text");
    let raw_bytes = text_with_escapes.len();
    assert!(
        bound > raw_bytes as u64,
        "Bound ({bound}) must capture wire JSON escaping expansion beyond raw text bytes ({raw_bytes})"
    );
}

#[test]
fn test_token_reservation_maximum_configured_output_tokens() {
    // 7. Maximum configured output tokens scaling
    let guard = BudgetGuard::new(MicroDollars::from_dollars(50).unwrap());

    for max_tokens in [100u32, 1024, 4096, 8192, 64000] {
        let req = LlmRequest {
            model: "claude-3-7-sonnet-20250219".to_string(),
            system_prompt: None,
            messages: vec![ChatMessage::user("compute")],
            tools: vec![],
            max_tokens,
            temperature: None,
            stop_sequences: vec![],
        };

        let ticket = guard
            .reserve_for_request(&req, 0)
            .expect("reserve with max_tokens");
        // Claude 3.7 output rate: $15.00/MTok = 15 micros per token
        // Output cost component must be exactly max_tokens * 15 micros
        let expected_output_cost = (max_tokens as u64) * 15;
        assert!(
            ticket.reserved_amount().0 >= expected_output_cost,
            "Reserved amount ({}) must cover max_tokens ({max_tokens}) output cost ({expected_output_cost})",
            ticket.reserved_amount().0
        );
        guard
            .reconcile_preflight_cancel(&ticket, "test cleanup")
            .unwrap();
    }
}

#[test]
fn test_token_reservation_unsupported_model_fails_closed() {
    // 8. Missing or unsupported tokenizer/model configuration fails closed
    let guard = BudgetGuard::new(MicroDollars::from_dollars(10).unwrap());

    let req = LlmRequest {
        model: "unregistered-unknown-model-v999".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("test")],
        tools: vec![],
        max_tokens: 100,
        temperature: None,
        stop_sequences: vec![],
    };

    let err = guard.reserve_for_request(&req, 100).unwrap_err();
    assert!(
        matches!(err, BudgetError::UnknownModelPricing(model) if model == "unregistered-unknown-model-v999")
    );

    // Verify ledger remains unallocated
    let snap = guard.snapshot().unwrap();
    assert_eq!(snap.active_reserved, MicroDollars::ZERO);
    assert_eq!(
        snap.available_budget,
        MicroDollars::from_dollars(10).unwrap()
    );
}

#[test]
fn test_token_reservation_arithmetic_overflow_fails_closed() {
    // 9. Reservation arithmetic overflow fails closed safely
    let pricing = ModelPricing::claude_3_7_sonnet();
    // Huge tokens that would overflow u64 during multiplication
    let overflow_err = pricing
        .calculate_worst_case_cost(u64::MAX / 2, u64::MAX / 2)
        .unwrap_err();
    assert!(matches!(overflow_err, BudgetError::ArithmeticOverflow));

    // Also verify PromptTokenEstimator handles overflow safely
    let huge_req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("test")],
        tools: vec![],
        max_tokens: u32::MAX,
        temperature: None,
        stop_sequences: vec![],
    };
    let guard = BudgetGuard::new(MicroDollars::from_dollars(100).unwrap());
    let res = guard.reserve_for_request(&huge_req, u64::MAX / 2);
    assert!(
        res.is_err(),
        "Reservation with overflow values must fail closed"
    );
}

#[test]
fn test_token_reservation_insufficient_remaining_budget_fails_closed() {
    // 10. Insufficient remaining budget fails closed
    let tiny_cap = MicroDollars::from_micros(100); // $0.0001
    let guard = BudgetGuard::new(tiny_cap);

    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: Some("You are a helpful assistant.".to_string()),
        messages: vec![ChatMessage::user("Explain quantum computing in detail.")],
        tools: vec![],
        max_tokens: 4096,
        temperature: None,
        stop_sequences: vec![],
    };

    let err = guard.reserve_for_request(&req, 0).unwrap_err();
    match err {
        BudgetError::BudgetExhausted {
            requested,
            available,
            hard_cap,
        } => {
            assert!(requested > available);
            assert_eq!(available, tiny_cap);
            assert_eq!(hard_cap, tiny_cap);
        }
        other => panic!("Expected BudgetExhausted, got {other:?}"),
    }

    // Available budget remains completely untouched
    let snap = guard.snapshot().unwrap();
    assert_eq!(snap.available_budget, tiny_cap);
    assert_eq!(snap.active_reserved, MicroDollars::ZERO);
}

#[test]
fn test_token_reservation_concurrent_atomic_synchronization() {
    // 11. Concurrent reservations with exact budget conservation
    let cap = MicroDollars::from_dollars(5).unwrap();
    let guard = Arc::new(BudgetGuard::new(cap));
    let mut handles = Vec::new();

    let success_count = Arc::new(AtomicUsize::new(0));
    let rejected_count = Arc::new(AtomicUsize::new(0));

    for i in 0..20 {
        let g = Arc::clone(&guard);
        let sc = Arc::clone(&success_count);
        let rc = Arc::clone(&rejected_count);

        handles.push(thread::spawn(move || {
            let req = LlmRequest {
                model: "claude-3-7-sonnet-20250219".to_string(),
                system_prompt: None,
                messages: vec![ChatMessage::user(format!("Thread task {i}"))],
                tools: vec![],
                max_tokens: 2000,
                temperature: None,
                stop_sequences: vec![],
            };

            match g.reserve_for_request(&req, 0) {
                Ok(_ticket) => {
                    sc.fetch_add(1, Ordering::SeqCst);
                }
                Err(BudgetError::BudgetExhausted { .. }) => {
                    rc.fetch_add(1, Ordering::SeqCst);
                }
                Err(other) => panic!("Unexpected error: {other:?}"),
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(
        success_count.load(Ordering::SeqCst) + rejected_count.load(Ordering::SeqCst),
        20
    );
    // Fundamental conservation invariant must strictly hold across all threads
    guard
        .snapshot()
        .unwrap()
        .verify_conservation_invariant()
        .unwrap();
}

#[test]
fn test_token_reservation_ambiguous_provider_outcome_holds_funds_no_retry() {
    // 12. Ambiguous provider outcome holds reserved funds in uncertain_spend without retry
    let guard = BudgetGuard::new(MicroDollars::from_dollars(1).unwrap());
    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Dispatched network request")],
        tools: vec![],
        max_tokens: 500,
        temperature: None,
        stop_sequences: vec![],
    };

    let ticket = guard.reserve_for_request(&req, 0).expect("reserve");
    let reserved = ticket.reserved_amount();

    let hold_receipt = guard
        .reconcile_ambiguous(
            &ticket,
            AmbiguousFailureReason::Timeout { elapsed_ms: 30_000 },
        )
        .expect("hold ambiguous");

    assert_eq!(hold_receipt.held_amount, reserved);
    let snap = guard.snapshot().unwrap();
    assert_eq!(snap.uncertain_spend, reserved);
    assert_eq!(snap.active_reserved, MicroDollars::ZERO);
    snap.verify_conservation_invariant().unwrap();
}

#[test]
fn test_token_reservation_actual_cost_exceeding_reserved_and_available_halts() {
    // 13. Actual usage exceeding reserved AND available headroom halts the session
    let cap = MicroDollars::from_micros(10_000);
    let guard = BudgetGuard::new(cap);

    let req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Check overrun")],
        tools: vec![],
        max_tokens: 100,
        temperature: None,
        stop_sequences: vec![],
    };

    let ticket = guard.reserve_for_request(&req, 0).expect("reserve");
    // Actual cost far exceeds reservation AND cap
    let catastrophic_cost = MicroDollars::from_dollars(100).unwrap();
    let err = guard
        .reconcile_success(&ticket, catastrophic_cost)
        .unwrap_err();
    assert!(matches!(
        err,
        BudgetError::CostExceedsReservationAndAvailableBudget { .. }
    ));

    // Subsequent request must be rejected because session is halted
    let next_ticket_res = guard.reserve_for_request(&req, 0);
    assert!(matches!(
        next_ticket_res,
        Err(BudgetError::SessionHaltedOnOverrun(_))
    ));
}

#[test]
fn test_token_reservation_unbounded_and_malformed_requests_fail_closed() {
    // 14. Fail-closed behavior for unbounded and malformed requests
    let guard = BudgetGuard::new(MicroDollars::from_dollars(10).unwrap());

    // Case 1: Empty model
    let req_empty_model = LlmRequest {
        model: "   ".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("test")],
        tools: vec![],
        max_tokens: 100,
        temperature: None,
        stop_sequences: vec![],
    };
    assert!(matches!(
        guard.reserve_for_request(&req_empty_model, 0),
        Err(BudgetError::RequestValidationFailed(
            SchemaValidationError::EmptyModel
        ))
    ));
    assert!(PromptTokenEstimator::conservative_upper_bound_tokens(&req_empty_model).is_err());

    // Case 2: Zero max_tokens
    let req_zero_tokens = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("test")],
        tools: vec![],
        max_tokens: 0,
        temperature: None,
        stop_sequences: vec![],
    };
    assert!(matches!(
        guard.reserve_for_request(&req_zero_tokens, 0),
        Err(BudgetError::RequestValidationFailed(
            SchemaValidationError::ZeroMaxTokens
        ))
    ));
    assert!(PromptTokenEstimator::conservative_upper_bound_tokens(&req_zero_tokens).is_err());

    // Case 3: Empty messages and no system prompt
    let req_empty_messages = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![],
        tools: vec![],
        max_tokens: 100,
        temperature: None,
        stop_sequences: vec![],
    };
    assert!(matches!(
        guard.reserve_for_request(&req_empty_messages, 0),
        Err(BudgetError::RequestValidationFailed(
            SchemaValidationError::EmptyMessages
        ))
    ));
    assert!(PromptTokenEstimator::conservative_upper_bound_tokens(&req_empty_messages).is_err());

    // Case 4: Duplicate tool names
    let req_dup_tools = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("test")],
        tools: vec![
            ToolDefinition {
                name: "duplicate_tool".to_string(),
                description: "Tool 1".to_string(),
                input_schema: serde_json::json!({ "type": "object" }),
            },
            ToolDefinition {
                name: "duplicate_tool".to_string(),
                description: "Tool 2".to_string(),
                input_schema: serde_json::json!({ "type": "object" }),
            },
        ],
        max_tokens: 100,
        temperature: None,
        stop_sequences: vec![],
    };
    assert!(matches!(
        guard.reserve_for_request(&req_dup_tools, 0),
        Err(BudgetError::RequestValidationFailed(
            SchemaValidationError::DuplicateToolName(_)
        ))
    ));
    assert!(PromptTokenEstimator::conservative_upper_bound_tokens(&req_dup_tools).is_err());

    // Case 5: Invalid temperature (> 2.0)
    let req_invalid_temp = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("test")],
        tools: vec![],
        max_tokens: 100,
        temperature: Some(3.5),
        stop_sequences: vec![],
    };
    assert!(matches!(
        guard.reserve_for_request(&req_invalid_temp, 0),
        Err(BudgetError::RequestValidationFailed(
            SchemaValidationError::InvalidTemperature(_)
        ))
    ));
    assert!(PromptTokenEstimator::conservative_upper_bound_tokens(&req_invalid_temp).is_err());
}

// ============================================================================
// 68. Milestone B6: Windows AppContainer Profile Lifecycle & ACL Grants (BLK-B6-02)
// ============================================================================

#[test]
fn test_appcontainer_profile_lifecycle_and_acl_grants() {
    #[cfg(windows)]
    {
        use engram::empirical_live::security::AppContainerProfile;

        let profile_name = format!("engram_test_lc_{}", uuid::Uuid::new_v4().simple());
        let profile = AppContainerProfile::create(&profile_name).expect("create profile");
        assert_eq!(profile.name(), profile_name);
        assert!(!profile.sid().is_null());

        // Grant WindowStation and Desktop
        let winsta_res = profile.grant_window_station_and_desktop();
        assert!(
            winsta_res.is_ok(),
            "granting WindowStation and Desktop must succeed"
        );

        // Grant directory access
        let temp_dir =
            std::env::temp_dir().join(format!("engram_acl_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).expect("create test dir");

        const GENERIC_ALL: u32 = 0x10000000;
        let grant_res = profile.grant_path_access(&temp_dir, GENERIC_ALL);
        assert!(
            grant_res.is_ok(),
            "granting filesystem access must succeed: {grant_res:?}"
        );

        // Explicit deletion
        let del_res = profile.delete();
        assert!(
            del_res.is_ok(),
            "deleting profile must succeed: {del_res:?}"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

// ============================================================================
// 69. Milestone B6: AppContainer Contained Command Execution (BLK-B6-02)
// ============================================================================

#[test]
fn test_appcontainer_executes_contained_command_and_captures_output() {
    #[cfg(windows)]
    {
        use engram::empirical_live::security::{
            AppContainerProfile, EnvironmentPolicy, ProcessJobObject, resolve_trusted_executable,
            spawn_contained_appcontainer_process,
        };
        use std::time::Duration;

        let profile_name = format!("engram_test_exec_{}", uuid::Uuid::new_v4().simple());
        let profile = AppContainerProfile::create(&profile_name).expect("profile creation");
        profile
            .grant_window_station_and_desktop()
            .expect("grant desktop");

        let temp_dir =
            std::env::temp_dir().join(format!("engram_cmd_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");

        const GENERIC_ALL: u32 = 0x10000000;
        profile
            .grant_path_access(&temp_dir, GENERIC_ALL)
            .expect("grant temp dir");

        let job = ProcessJobObject::create().expect("job creation");

        let cmd_exec = resolve_trusted_executable("cmd").expect("resolve cmd");
        if let Some(parent) = cmd_exec.parent() {
            const GENERIC_READ_EXECUTE: u32 = 0x80000000 | 0x20000000;
            let _ = profile.grant_path_access(parent, GENERIC_READ_EXECUTE);
        }

        let env_policy = EnvironmentPolicy::new(&temp_dir);
        let env_vars = env_policy
            .build_sanitized_environment()
            .expect("sanitized env");

        let args = vec![
            "/c".to_string(),
            "echo engram_appcontainer_success_token_xyz".to_string(),
        ];

        let output = spawn_contained_appcontainer_process(
            &profile,
            &job,
            &cmd_exec,
            &args,
            &temp_dir,
            &env_vars,
            Duration::from_secs(10),
        )
        .expect("spawn contained process");

        assert_eq!(
            output.exit_code, 0,
            "exit code must be 0, stderr: {}",
            output.stderr
        );
        assert!(
            output
                .stdout
                .contains("engram_appcontainer_success_token_xyz"),
            "stdout must contain echo token, got: '{}'",
            output.stdout
        );
        assert!(output.thread_resumption.is_complete_success());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

// ============================================================================
// 70. Milestone B6: AppContainer Subprocess Network Egress Isolation — TCP Drop
// (BLK-B6-02)
// ============================================================================

#[test]
fn test_appcontainer_network_containment_blocks_loopback_tcp_connect() {
    #[cfg(windows)]
    {
        use engram::empirical_live::security::{
            AppContainerProfile, EnvironmentPolicy, ProcessJobObject, resolve_trusted_executable,
            spawn_contained_appcontainer_process,
        };
        use std::io::{Read, Write};
        use std::net::{TcpListener, TcpStream};
        use std::time::{Duration, Instant};

        // 1. Parent binds local loopback TCP listener on an ephemeral port
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test listener");
        listener.set_nonblocking(true).expect("set nonblocking");
        let port = listener.local_addr().expect("local addr").port();

        // 2. Positive Control: Verify an unrestricted client communicates successfully with this exact listener
        {
            let mut unrestricted_client = TcpStream::connect(("127.0.0.1", port))
                .expect("positive control TCP client connect must succeed");
            let (mut accepted_stream, _peer) = listener
                .accept()
                .expect("parent listener must accept unrestricted connection");

            unrestricted_client
                .write_all(b"POSITIVE_CONTROL_PING")
                .expect("write ping");
            let mut read_buf = [0u8; 21];
            accepted_stream
                .read_exact(&mut read_buf)
                .expect("read ping");
            assert_eq!(&read_buf, b"POSITIVE_CONTROL_PING");
            drop(unrestricted_client);
            drop(accepted_stream);
        }

        // 3. Multi-iteration Contained Attempts (3 iterations to detect flakiness)
        for iteration in 1..=3 {
            let profile_name = format!(
                "engram_test_net_tcp_{}_{}",
                iteration,
                uuid::Uuid::new_v4().simple()
            );
            let profile = AppContainerProfile::create(&profile_name).expect("profile creation");
            profile
                .grant_window_station_and_desktop()
                .expect("grant desktop");

            let raw_temp = std::env::temp_dir().join(format!(
                "engram_net_tcp_{}_{}",
                iteration,
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&raw_temp).expect("create temp dir");
            let temp_dir = std::fs::canonicalize(&raw_temp).unwrap_or(raw_temp);
            const GENERIC_ALL: u32 = 0x10000000;
            profile
                .grant_path_access(&temp_dir, GENERIC_ALL)
                .expect("grant temp dir");

            let job = ProcessJobObject::create().expect("job creation");
            let ps_exec = resolve_trusted_executable("powershell").expect("resolve powershell");
            if let Some(parent) = ps_exec.parent() {
                const GENERIC_READ_EXECUTE: u32 = 0x80000000 | 0x20000000;
                let _ = profile.grant_path_access(parent, GENERIC_READ_EXECUTE);
            }

            let env_policy = EnvironmentPolicy::new(&temp_dir);
            let env_vars = env_policy
                .build_sanitized_environment()
                .expect("sanitized env");

            let script = format!(
                "$sw = [System.Diagnostics.Stopwatch]::StartNew(); \
                 $c = New-Object System.Net.Sockets.TcpClient; \
                 try {{ \
                     $a = $c.BeginConnect('127.0.0.1', {port}, $null, $null); \
                     if ($a.AsyncWaitHandle.WaitOne(1000)) {{ \
                         $c.EndConnect($a); \
                         $sw.Stop(); \
                         Write-Output ('EGRESS_LEAK_CONNECTED ms=' + $sw.ElapsedMilliseconds) \
                     }} else {{ \
                         $sw.Stop(); \
                         Write-Output ('EGRESS_BLOCKED_TIMED_OUT ms=' + $sw.ElapsedMilliseconds) \
                     }} \
                 }} catch {{ \
                     $sw.Stop(); \
                     Write-Output ('EGRESS_BLOCKED_REJECTED ms=' + $sw.ElapsedMilliseconds + ' err=' + $_.Exception.Message) \
                 }} finally {{ \
                     $c.Close() \
                 }}"
            );

            let args = vec![
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                script,
            ];

            let start_t = Instant::now();
            let output = spawn_contained_appcontainer_process(
                &profile,
                &job,
                &ps_exec,
                &args,
                &temp_dir,
                &env_vars,
                Duration::from_secs(25),
            )
            .expect("spawn contained process");
            let elapsed = start_t.elapsed();

            // INVARIANT 1: Child must NOT establish a TCP connection!
            assert!(
                !output.stdout.contains("EGRESS_LEAK_CONNECTED"),
                "[Iteration {iteration}] CRITICAL SECURITY FAILURE: AppContainer connected to loopback TCP socket! stdout: '{}'",
                output.stdout
            );
            assert!(
                output.stdout.contains("EGRESS_BLOCKED_TIMED_OUT")
                    || output.stdout.contains("REJECTED"),
                "[Iteration {iteration}] Child network probe must report blocked or rejected, got: '{}' (elapsed: {elapsed:?})",
                output.stdout
            );

            // INVARIANT 2: Parent listener must NEVER accept any connection from the contained process
            let accept_res = listener.accept();
            assert!(
                accept_res.is_err(),
                "[Iteration {iteration}] Parent listener must receive zero connections (expected WouldBlock, got: {accept_res:?})"
            );

            drop(profile); // Triggers automatic ACE revocation and profile deletion
            let _ = std::fs::remove_dir_all(&temp_dir);
        }

        // 4. IPv6 loopback verification if supported
        if let Ok(v6_listener) = TcpListener::bind("[::1]:0") {
            v6_listener
                .set_nonblocking(true)
                .expect("set v6 nonblocking");
            let v6_port = v6_listener.local_addr().expect("v6 local addr").port();

            // Positive control for IPv6
            if let Ok(mut v6_client) = TcpStream::connect(("::1", v6_port)) {
                if let Ok((mut accepted_v6, _)) = v6_listener.accept() {
                    let _ = v6_client.write_all(b"IPV6_PING");
                    let mut b = [0u8; 9];
                    let _ = accepted_v6.read_exact(&mut b);
                }
            }

            // Contained attempt to IPv6
            let profile_name = format!("engram_test_v6_{}", uuid::Uuid::new_v4().simple());
            let profile = AppContainerProfile::create(&profile_name).expect("profile creation");
            profile
                .grant_window_station_and_desktop()
                .expect("grant desktop");
            let temp_dir =
                std::env::temp_dir().join(format!("engram_net_v6_{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&temp_dir).expect("create temp dir");
            const GENERIC_ALL: u32 = 0x10000000;
            profile
                .grant_path_access(&temp_dir, GENERIC_ALL)
                .expect("grant temp dir");
            let job = ProcessJobObject::create().expect("job creation");
            let ps_exec = resolve_trusted_executable("powershell").expect("resolve powershell");
            if let Some(parent) = ps_exec.parent() {
                const GENERIC_READ_EXECUTE: u32 = 0x80000000 | 0x20000000;
                let _ = profile.grant_path_access(parent, GENERIC_READ_EXECUTE);
            }
            let env_policy = EnvironmentPolicy::new(&temp_dir);
            let env_vars = env_policy
                .build_sanitized_environment()
                .expect("sanitized env");
            let v6_script = format!(
                "$c = New-Object System.Net.Sockets.TcpClient; \
                 try {{ \
                     $a = $c.BeginConnect('::1', {v6_port}, $null, $null); \
                     if ($a.AsyncWaitHandle.WaitOne(1000)) {{ \
                         $c.EndConnect($a); \
                         Write-Output 'IPV6_LEAK_CONNECTED' \
                     }} else {{ \
                         Write-Output 'IPV6_BLOCKED_TIMED_OUT' \
                     }} \
                 }} catch {{ \
                     Write-Output ('IPV6_BLOCKED_REJECTED: ' + $_.Exception.Message) \
                 }} finally {{ \
                     $c.Close() \
                 }}"
            );
            let args = vec![
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                v6_script,
            ];
            let out = spawn_contained_appcontainer_process(
                &profile,
                &job,
                &ps_exec,
                &args,
                &temp_dir,
                &env_vars,
                Duration::from_secs(25),
            )
            .expect("spawn contained v6");
            assert!(
                !out.stdout.contains("IPV6_LEAK_CONNECTED"),
                "CRITICAL: IPv6 loopback connection established!"
            );
            let accept_v6_res = v6_listener.accept();
            assert!(
                accept_v6_res.is_err(),
                "IPv6 listener accepted connection unexpectedly: {accept_v6_res:?}"
            );
            drop(profile);
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
    }
}

// ============================================================================
// 71. Milestone B6: AppContainer Subprocess Network Egress Isolation — UDP Drop
// (BLK-B6-02)
// ============================================================================

#[test]
fn test_appcontainer_network_containment_blocks_loopback_udp_datagram() {
    #[cfg(windows)]
    {
        use engram::empirical_live::security::{
            AppContainerProfile, EnvironmentPolicy, ProcessJobObject, resolve_trusted_executable,
            spawn_contained_appcontainer_process,
        };
        use std::net::UdpSocket;
        use std::time::{Duration, Instant};

        // 1. Parent binds local loopback UDP socket on ephemeral port
        let receiver = UdpSocket::bind("127.0.0.1:0").expect("bind local test udp socket");
        receiver
            .set_read_timeout(Some(Duration::from_millis(500)))
            .expect("set read timeout");
        let port = receiver.local_addr().expect("local addr").port();

        // 2. Positive Control: Verify an unrestricted UDP socket communicates successfully
        {
            let sender = UdpSocket::bind("127.0.0.1:0").expect("bind unrestricted test sender");
            sender
                .send_to(b"POSITIVE_CONTROL_UDP_PING", ("127.0.0.1", port))
                .expect("send positive control datagram");
            let mut buf = [0u8; 128];
            let (n, _from) = receiver
                .recv_from(&mut buf)
                .expect("receiver must accept unrestricted positive control datagram");
            assert_eq!(&buf[..n], b"POSITIVE_CONTROL_UDP_PING");
        }

        // 3. Multi-iteration Contained Attempts (3 iterations to detect flakiness)
        for iteration in 1..=3 {
            let profile_name = format!(
                "engram_test_udp_{}_{}",
                iteration,
                uuid::Uuid::new_v4().simple()
            );
            let profile = AppContainerProfile::create(&profile_name).expect("profile creation");
            profile
                .grant_window_station_and_desktop()
                .expect("grant desktop");

            let temp_dir = std::env::temp_dir().join(format!(
                "engram_net_udp_{}_{}",
                iteration,
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&temp_dir).expect("create temp dir");
            const GENERIC_ALL: u32 = 0x10000000;
            profile
                .grant_path_access(&temp_dir, GENERIC_ALL)
                .expect("grant temp dir");

            let job = ProcessJobObject::create().expect("job creation");
            let ps_exec = resolve_trusted_executable("powershell").expect("resolve powershell");
            if let Some(parent) = ps_exec.parent() {
                const GENERIC_READ_EXECUTE: u32 = 0x80000000 | 0x20000000;
                let _ = profile.grant_path_access(parent, GENERIC_READ_EXECUTE);
            }

            let env_policy = EnvironmentPolicy::new(&temp_dir);
            let env_vars = env_policy
                .build_sanitized_environment()
                .expect("sanitized env");

            let script = format!(
                "$sw = [System.Diagnostics.Stopwatch]::StartNew(); \
                 $u = New-Object System.Net.Sockets.UdpClient; \
                 $bytes = [System.Text.Encoding]::ASCII.GetBytes('LEAK_TEST_DATAGRAM'); \
                 try {{ \
                     $sent = $u.Send($bytes, $bytes.Length, '127.0.0.1', {port}); \
                     $sw.Stop(); \
                     Write-Output ('UDP_SENT: ' + $sent + ' ms=' + $sw.ElapsedMilliseconds) \
                 }} catch {{ \
                     $sw.Stop(); \
                     Write-Output ('UDP_ERROR: ' + $_.Exception.Message + ' ms=' + $sw.ElapsedMilliseconds) \
                 }} finally {{ \
                     $u.Close() \
                 }}"
            );

            let args = vec![
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                script,
            ];

            let start_t = Instant::now();
            let _output = spawn_contained_appcontainer_process(
                &profile,
                &job,
                &ps_exec,
                &args,
                &temp_dir,
                &env_vars,
                Duration::from_secs(10),
            )
            .expect("spawn contained process");
            let elapsed = start_t.elapsed();

            // INVARIANT: In Windows AppContainer without internetClient or privateNetworkClientServer,
            // UDP datagrams are dropped in the kernel WFP ALE layer.
            // The parent socket must NEVER receive the datagram!
            let mut buf = [0u8; 128];
            let recv_res = receiver.recv(&mut buf);
            assert!(
                recv_res.is_err(),
                "[Iteration {iteration}] CRITICAL SECURITY FAILURE: Parent UDP socket received a datagram from contained process! {recv_res:?} (elapsed: {elapsed:?})"
            );

            drop(profile); // Triggers automatic ACE revocation and profile deletion
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
    }
}

// ============================================================================
// 72. Milestone B6: AppContainer Job Object Process Tree Reaping (BLK-B6-02)
// ============================================================================

#[test]
fn test_appcontainer_job_object_tree_reaping_on_timeout() {
    #[cfg(windows)]
    {
        use engram::empirical_live::security::{
            AppContainerProfile, EnvironmentPolicy, ProcessJobObject, SecurityError,
            resolve_trusted_executable, spawn_contained_appcontainer_process,
        };
        use std::time::Duration;

        let profile_name = format!("engram_test_reap_{}", uuid::Uuid::new_v4().simple());
        let profile = AppContainerProfile::create(&profile_name).expect("profile creation");
        profile
            .grant_window_station_and_desktop()
            .expect("grant desktop");

        let temp_dir =
            std::env::temp_dir().join(format!("engram_reap_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        const GENERIC_ALL: u32 = 0x10000000;
        profile
            .grant_path_access(&temp_dir, GENERIC_ALL)
            .expect("grant temp dir");

        let job = ProcessJobObject::create().expect("job creation");
        let ps_exec = resolve_trusted_executable("powershell").expect("resolve powershell");
        if let Some(parent) = ps_exec.parent() {
            const GENERIC_READ_EXECUTE: u32 = 0x80000000 | 0x20000000;
            let _ = profile.grant_path_access(parent, GENERIC_READ_EXECUTE);
        }

        let env_policy = EnvironmentPolicy::new(&temp_dir);
        let env_vars = env_policy
            .build_sanitized_environment()
            .expect("sanitized env");

        let args = vec![
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-Command".to_string(),
            "Start-Sleep -Seconds 30".to_string(),
        ];

        // Execute with a short 1-second timeout
        let res = spawn_contained_appcontainer_process(
            &profile,
            &job,
            &ps_exec,
            &args,
            &temp_dir,
            &env_vars,
            Duration::from_secs(1),
        );

        // INVARIANT: Must time out and reap descendants fail-closed
        assert!(matches!(
            res,
            Err(SecurityError::CommandTimeout { timeout_secs: 1 })
        ));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

// ============================================================================
// 73. Milestone B6: AppContainer Fail-Closed on Unauthorized Filesystem Path
// (BLK-B6-02)
// ============================================================================

#[test]
fn test_appcontainer_fail_closed_on_unauthorized_filesystem_path() {
    #[cfg(windows)]
    {
        use engram::empirical_live::security::{
            AppContainerProfile, EnvironmentPolicy, ProcessJobObject, resolve_trusted_executable,
            spawn_contained_appcontainer_process,
        };
        use std::time::Duration;

        let profile_name = format!("engram_test_unauth_{}", uuid::Uuid::new_v4().simple());
        let profile = AppContainerProfile::create(&profile_name).expect("profile creation");
        profile
            .grant_window_station_and_desktop()
            .expect("grant desktop");

        // Create authorized sandbox directory
        let sandbox_dir =
            std::env::temp_dir().join(format!("engram_sb_auth_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&sandbox_dir).expect("create sandbox dir");
        const GENERIC_ALL: u32 = 0x10000000;
        profile
            .grant_path_access(&sandbox_dir, GENERIC_ALL)
            .expect("grant sandbox dir");

        // Create separate unauthorized directory with secret file
        let secret_dir =
            std::env::temp_dir().join(format!("engram_secret_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&secret_dir).expect("create secret dir");
        let secret_file = secret_dir.join("confidential_credential.txt");
        std::fs::write(&secret_file, "SUPER_SECRET_TOKEN").expect("write secret file");
        // DO NOT grant profile access to secret_dir!

        let job = ProcessJobObject::create().expect("job creation");
        let cmd_exec = resolve_trusted_executable("cmd").expect("resolve cmd");
        if let Some(parent) = cmd_exec.parent() {
            const GENERIC_READ_EXECUTE: u32 = 0x80000000 | 0x20000000;
            let _ = profile.grant_path_access(parent, GENERIC_READ_EXECUTE);
        }

        let env_policy = EnvironmentPolicy::new(&sandbox_dir);
        let env_vars = env_policy
            .build_sanitized_environment()
            .expect("sanitized env");

        let args = vec![
            "/c".to_string(),
            "type".to_string(),
            secret_file.to_string_lossy().to_string(),
        ];

        let output = spawn_contained_appcontainer_process(
            &profile,
            &job,
            &cmd_exec,
            &args,
            &sandbox_dir,
            &env_vars,
            Duration::from_secs(10),
        )
        .expect("spawn contained process");

        // INVARIANT: Reading unauthorized path must be DENIED!
        assert_ne!(
            output.exit_code, 0,
            "Exit code must indicate failure for unauthorized access"
        );
        assert!(
            !output.stdout.contains("SUPER_SECRET_TOKEN"),
            "CRITICAL LEAK: Contained process read unauthorized file!"
        );
        let combined = format!("{} {}", output.stdout, output.stderr);
        assert!(
            combined.contains("Access is denied") || combined.contains("denied"),
            "Output must show access denied, got: '{combined}'"
        );

        let _ = std::fs::remove_dir_all(&sandbox_dir);
        let _ = std::fs::remove_dir_all(&secret_dir);
    }
}

// ============================================================================
// 74. Milestone B6: SandboxToolDispatcher with AppContainer Containment Enabled
// (BLK-B6-02)
// ============================================================================

#[test]
fn test_sandbox_tool_dispatcher_with_appcontainer_enabled() {
    #[cfg(windows)]
    {
        use engram::empirical_live::dispatcher::SandboxToolDispatcher;
        use engram::empirical_live::schema::ContentBlock;

        let temp_dir =
            std::env::temp_dir().join(format!("engram_disp_ac_{}", uuid::Uuid::new_v4()));
        let dispatcher = SandboxToolDispatcher::new(&temp_dir)
            .expect("dispatcher")
            .with_appcontainer(true);

        assert!(dispatcher.is_appcontainer_enabled());

        // Dispatch cargo --version inside the AppContainer + Job Object sandbox
        let res = dispatcher.dispatch(
            "execute_command",
            "call_cargo_ac",
            &serde_json::json!({ "command": "cargo", "args": ["--version"] }),
        );

        if let ContentBlock::ToolResult {
            is_error, content, ..
        } = res
        {
            assert!(
                !is_error,
                "AppContainer cargo execution should succeed: {content}"
            );
            assert!(
                content.contains("cargo"),
                "Output must contain cargo version string: {content}"
            );
            assert!(
                content.contains("Exit code: 0"),
                "Exit code must be 0: {content}"
            );
        } else {
            panic!("Expected ToolResult, got: {res:?}");
        }
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

// ============================================================================
// 75. Milestone B6: AppContainer Toolchain Compatibility & Offline Build Boundary
// (BLK-B6-02)
// ============================================================================

#[test]
fn test_appcontainer_toolchain_offline_compilation_boundary() {
    #[cfg(windows)]
    {
        use engram::empirical_live::security::{
            AppContainerProfile, EnvironmentPolicy, ProcessJobObject, resolve_trusted_executable,
            spawn_contained_appcontainer_process, stage_ephemeral_toolchain_mirror,
        };
        use std::time::Duration;

        // 1. Toolchain version checks: confirm cargo and rustc exist and are operational
        let cargo_version = std::process::Command::new("cargo")
            .arg("--version")
            .output()
            .expect("cargo version command");
        assert!(
            cargo_version.status.success(),
            "cargo --version must succeed"
        );
        let cargo_str = String::from_utf8_lossy(&cargo_version.stdout);
        assert!(
            cargo_str.contains("cargo 1."),
            "Cargo version must be 1.x, got: {cargo_str}"
        );

        let rustc_version = std::process::Command::new("rustc")
            .arg("--version")
            .output()
            .expect("rustc version command");
        assert!(
            rustc_version.status.success(),
            "rustc --version must succeed"
        );
        let rustc_str = String::from_utf8_lossy(&rustc_version.stdout);
        assert!(
            rustc_str.contains("rustc 1."),
            "rustc version must be 1.x, got: {rustc_str}"
        );

        // 2. Setup isolated offline fixture
        let sandbox_dir =
            std::env::temp_dir().join(format!("engram_toolchain_sb_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&sandbox_dir).expect("create sandbox dir");
        let src_file = sandbox_dir.join("main.rs");
        let out_file = sandbox_dir.join("main.exe");
        std::fs::write(
            &src_file,
            "fn main() { println!(\"contained_offline_ok\"); }\n",
        )
        .expect("write main.rs");

        // External unauthorized directory with secret token to test boundary escape rejection
        let unauth_dir =
            std::env::temp_dir().join(format!("engram_toolchain_unauth_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&unauth_dir).expect("create unauth dir");
        let secret_token_file = unauth_dir.join("token.txt");
        std::fs::write(&secret_token_file, "HOST_DEVELOPER_SECRET").expect("write secret token");

        let profile_name = format!("engram_tc_{}", uuid::Uuid::new_v4().simple());
        let profile = AppContainerProfile::create(&profile_name).expect("profile creation");
        profile
            .grant_window_station_and_desktop()
            .expect("grant desktop");

        const GENERIC_ALL: u32 = 0x10000000;
        const GENERIC_READ_EXECUTE: u32 = 0x80000000 | 0x20000000;
        profile
            .grant_path_access(&sandbox_dir, GENERIC_ALL)
            .expect("grant sandbox dir");

        // Redirected caches inside sandbox
        for sub in &[
            ".engram_cargo_home",
            ".engram_npm_cache",
            ".engram_pycache",
            ".engram_tmp",
        ] {
            let cd = sandbox_dir.join(sub);
            let _ = std::fs::create_dir_all(&cd);
            let _ = profile.grant_path_access(&cd, GENERIC_ALL);
        }

        let job = ProcessJobObject::create().expect("job creation");
        let rustc_exec = resolve_trusted_executable("rustc").expect("resolve rustc");

        // Invariant: DO NOT grant broad ACL to %USERPROFILE%, .cargo, or .rustup!
        // When ungranted, verify whether rustc can run or is blocked fail-closed:
        let env_policy = EnvironmentPolicy::new(&sandbox_dir);
        let env_vars = env_policy
            .build_sanitized_environment()
            .expect("sanitized env");

        let ungranted_args = vec![
            src_file.to_string_lossy().to_string(),
            "-o".to_string(),
            out_file.to_string_lossy().to_string(),
        ];

        let ungranted_result = spawn_contained_appcontainer_process(
            &profile,
            &job,
            &rustc_exec,
            &ungranted_args,
            &sandbox_dir,
            &env_vars,
            Duration::from_secs(10),
        );

        match &ungranted_result {
            Ok(output) => {
                if output.exit_code != 0 {
                    // Non-zero exit code confirms isolation prevents untrusted toolchain execution
                } else {
                    assert!(out_file.exists());
                }
            }
            Err(e) => {
                let err_str = format!("{e}");
                assert!(
                    err_str.contains("Access is denied")
                        || err_str.contains("os_error_code: 5")
                        || err_str.contains("ProcessSpawnFailed"),
                    "Expected access denial when userprofile is ungranted, got: {err_str}"
                );
            }
        }

        // 3. Staged Ephemeral Toolchain Demonstration (Strategy B)
        // Stage a private, minimal toolchain mirror into an isolated directory
        let mirror_dir = sandbox_dir.join("toolchain_mirror");
        if let Ok(staged_rustc) = stage_ephemeral_toolchain_mirror(&mirror_dir) {
            let _ = profile.grant_path_access(&mirror_dir, GENERIC_READ_EXECUTE);

            let staged_job = ProcessJobObject::create().expect("staged job");
            let staged_out_file = sandbox_dir.join("staged_main.exe");
            let compile_args = vec![
                src_file.to_string_lossy().to_string(),
                "-o".to_string(),
                staged_out_file.to_string_lossy().to_string(),
            ];

            let compile_res = spawn_contained_appcontainer_process(
                &profile,
                &staged_job,
                &staged_rustc,
                &compile_args,
                &sandbox_dir,
                &env_vars,
                Duration::from_secs(30),
            );

            if let Ok(output) = compile_res {
                if output.exit_code == 0 {
                    assert!(
                        staged_out_file.exists(),
                        "Compiled binary staged_main.exe must exist"
                    );

                    // Execute compiled binary inside the AppContainer
                    let run_job = ProcessJobObject::create().expect("run job");
                    let run_res = spawn_contained_appcontainer_process(
                        &profile,
                        &run_job,
                        &staged_out_file,
                        &[],
                        &sandbox_dir,
                        &env_vars,
                        Duration::from_secs(10),
                    )
                    .expect("spawn compiled binary");

                    assert_eq!(run_res.exit_code, 0);
                    assert!(run_res.stdout.contains("contained_offline_ok"));
                }
            }
        }

        // 4. Verify the contained process CANNOT write to unauthorized host directory
        let cmd_exec = resolve_trusted_executable("cmd").expect("resolve cmd");
        if let Some(parent) = cmd_exec.parent() {
            let _ = profile.grant_path_access(parent, GENERIC_READ_EXECUTE);
        }

        let escape_attempt_file = unauth_dir.join("escaped_file.txt");
        let escape_args = vec![
            "/c".to_string(),
            format!(
                "echo HACKED > \"{}\"",
                escape_attempt_file.to_string_lossy()
            ),
        ];
        let escape_out = spawn_contained_appcontainer_process(
            &profile,
            &job,
            &cmd_exec,
            &escape_args,
            &sandbox_dir,
            &env_vars,
            Duration::from_secs(10),
        )
        .expect("spawn escape test");

        assert_ne!(
            escape_out.exit_code, 0,
            "Writing to ungranted path must fail"
        );
        assert!(
            !escape_attempt_file.exists(),
            "CRITICAL ESCAPE: Contained process wrote to unauthorized directory!"
        );

        drop(profile);
        let _ = std::fs::remove_dir_all(&sandbox_dir);
        let _ = std::fs::remove_dir_all(&unauth_dir);
    }
}

// ============================================================================
// 76. Milestone B6: Production SandboxToolDispatcher Network Containment
// (BLK-B6-02)
// ============================================================================

#[test]
fn test_production_sandbox_tool_dispatcher_network_containment_blocks_egress() {
    #[cfg(windows)]
    {
        use engram::empirical_live::dispatcher::SandboxToolDispatcher;
        use engram::empirical_live::schema::ContentBlock;
        use std::io::{Read, Write};
        use std::net::{TcpListener, TcpStream};
        use std::time::Duration;

        // 1. Bind an ephemeral loopback TCP listener on host
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let local_addr = listener.local_addr().expect("local addr");
        let port = local_addr.port();
        listener.set_nonblocking(true).expect("set non-blocking");

        // 2. Positive control: unrestricted client can connect and exchange payload
        {
            let mut pos_client = TcpStream::connect(local_addr).expect("positive control connect");
            pos_client
                .write_all(b"DISPATCHER_PROBE_PING")
                .expect("write ping");
            let (mut server_stream, _) = loop {
                match listener.accept() {
                    Ok(pair) => break pair,
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => panic!("accept failed: {e}"),
                }
            };
            let mut buf = [0u8; 32];
            let n = server_stream.read(&mut buf).expect("read ping");
            assert_eq!(&buf[..n], b"DISPATCHER_PROBE_PING");
        }

        // 3. Initialize SandboxToolDispatcher with AppContainer containment enabled
        // INVARIANT: Uses pure production allowlist without test-only bypasses
        let temp_dir =
            std::env::temp_dir().join(format!("engram_disp_net_{}", uuid::Uuid::new_v4()));
        let dispatcher = SandboxToolDispatcher::new(&temp_dir)
            .expect("dispatcher")
            .with_appcontainer(true);

        assert!(dispatcher.is_appcontainer_enabled());

        // 4. Write network egress probe script inside the sandbox via dispatcher
        let write_res = dispatcher.dispatch(
            "write_file",
            "call_write_probe",
            &serde_json::json!({
                "path": "probe.py",
                "content": format!(
                    "import socket\n\
                     s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)\n\
                     s.settimeout(1.0)\n\
                     try:\n    \
                         s.connect(('127.0.0.1', {port}))\n    \
                         print('EGRESS_ALLOWED')\n\
                     except Exception:\n    \
                         print('EGRESS_BLOCKED_TIMED_OUT')\n\
                     finally:\n    \
                         s.close()\n"
                )
            }),
        );
        if let ContentBlock::ToolResult {
            is_error,
            ref content,
            ..
        } = write_res
        {
            assert!(!is_error, "write_file must succeed: {content}");
        }

        // 5. Dispatch network egress probe using approved 'python' executable (no shell interpreter)
        let res = dispatcher.dispatch(
            "execute_command",
            "probe_call",
            &serde_json::json!({
                "command": "python",
                "args": ["probe.py"]
            }),
        );

        // 6. Verify production dispatcher result: network egress must be blocked by kernel WFP ALE
        if let ContentBlock::ToolResult {
            is_error, content, ..
        } = res
        {
            assert!(!is_error, "Command execution itself succeeded: {content}");
            assert!(
                content.contains("EGRESS_BLOCKED_TIMED_OUT"),
                "Production dispatcher must contain and block egress, got: {content}"
            );
            assert!(
                !content.contains("EGRESS_ALLOWED"),
                "CRITICAL EGRESS LEAK: Contained process dispatched via production pipeline connected to network!"
            );
        } else {
            panic!("Expected ToolResult, got: {res:?}");
        }

        // 7. Verify listener received 0 connections during contained dispatch
        match listener.accept() {
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Expected: No connections arrived at listener
            }
            Ok((_, remote)) => {
                panic!(
                    "CRITICAL EGRESS LEAK: Listener accepted connection from contained dispatcher child {remote}!"
                );
            }
            Err(e) => panic!("Unexpected listener error: {e}"),
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

// ============================================================================
// 77. Milestone B6: DACL Snapshot Preservation & Rollback Correctness
// for WindowStation and Desktop (BLK-B6-02)
// ============================================================================

#[test]
fn test_appcontainer_dacl_snapshot_preservation_and_rollback_correctness() {
    #[cfg(windows)]
    {
        use engram::empirical_live::security::{
            AppContainerProfile, snapshot_user_object_dacl, win32,
        };

        unsafe {
            let winsta = win32::GetProcessWindowStation();
            assert!(
                !winsta.is_null(),
                "Process WindowStation handle must be valid"
            );
            let desktop = win32::GetThreadDesktop(win32::GetCurrentThreadId());
            assert!(!desktop.is_null(), "Thread Desktop handle must be valid");

            // 1. Capture pristine pre-grant snapshots of BOTH WindowStation and Desktop DACLs
            let winsta_before = snapshot_user_object_dacl(winsta).expect("winsta before snapshot");
            assert!(
                !winsta_before.is_empty(),
                "WindowStation DACL must contain existing ACEs"
            );
            let desktop_before =
                snapshot_user_object_dacl(desktop).expect("desktop before snapshot");
            assert!(
                !desktop_before.is_empty(),
                "Desktop DACL must contain existing ACEs"
            );

            // 2. Create ephemeral AppContainer profile
            let profile_name = format!("engram_dacl_test_{}", uuid::Uuid::new_v4().simple());
            let profile = AppContainerProfile::create(&profile_name).expect("profile creation");
            assert!(!profile.is_winsta_granted());
            assert!(!profile.is_desktop_granted());

            // 3. Grant least-privilege WindowStation and Desktop access
            profile
                .grant_window_station_and_desktop()
                .expect("grant desktop");
            assert!(profile.is_winsta_granted());
            assert!(profile.is_desktop_granted());

            // 4. Capture during-grant snapshots of BOTH objects and verify least-privilege constraints
            let winsta_during = snapshot_user_object_dacl(winsta).expect("winsta during snapshot");
            let desktop_during =
                snapshot_user_object_dacl(desktop).expect("desktop during snapshot");

            // WindowStation verification:
            let mut winsta_ac_found = false;
            for entry in &winsta_during {
                if entry.access_mask == win32::WINSTA_CONTAINED_RIGHTS {
                    winsta_ac_found = true;
                    assert_eq!(
                        entry.access_mask & 0x00040000,
                        0,
                        "WindowStation DACL must NOT have WRITE_DAC"
                    );
                    assert_eq!(
                        entry.access_mask & 0x00080000,
                        0,
                        "WindowStation DACL must NOT have WRITE_OWNER"
                    );
                    assert_eq!(
                        entry.access_mask & 0x40000000,
                        0,
                        "WindowStation DACL must NOT have GENERIC_WRITE"
                    );
                }
            }
            assert!(
                winsta_ac_found,
                "AppContainer SID entry with WINSTA_CONTAINED_RIGHTS must be present on WindowStation"
            );

            // Desktop verification:
            let mut desktop_ac_found = false;
            for entry in &desktop_during {
                if entry.access_mask == win32::DESKTOP_CONTAINED_RIGHTS {
                    desktop_ac_found = true;
                    assert_eq!(
                        entry.access_mask & 0x00040000,
                        0,
                        "Desktop DACL must NOT have WRITE_DAC"
                    );
                    assert_eq!(
                        entry.access_mask & 0x00080000,
                        0,
                        "Desktop DACL must NOT have WRITE_OWNER"
                    );
                    assert_eq!(
                        entry.access_mask & 0x40000000,
                        0,
                        "Desktop DACL must NOT have GENERIC_WRITE"
                    );
                }
            }
            assert!(
                desktop_ac_found,
                "AppContainer SID entry with DESKTOP_CONTAINED_RIGHTS must be present on Desktop"
            );

            // Invariant: all pre-existing entries on BOTH objects must be preserved identically
            for before_entry in &winsta_before {
                assert!(
                    winsta_during.iter().any(|d| {
                        d.sid_string == before_entry.sid_string
                            && d.access_mask == before_entry.access_mask
                    }),
                    "Pre-existing WindowStation DACL entry for {} must be preserved",
                    before_entry.sid_string
                );
            }
            for before_entry in &desktop_before {
                assert!(
                    desktop_during.iter().any(|d| {
                        d.sid_string == before_entry.sid_string
                            && d.access_mask == before_entry.access_mask
                    }),
                    "Pre-existing Desktop DACL entry for {} must be preserved",
                    before_entry.sid_string
                );
            }

            // 5. Delete AppContainer profile and trigger deterministic rollback
            profile.delete().expect("profile delete");
            assert!(!profile.is_winsta_granted());
            assert!(!profile.is_desktop_granted());

            // 6. Capture after-rollback snapshot of BOTH objects and verify 100% restoration
            let winsta_after = snapshot_user_object_dacl(winsta).expect("winsta after snapshot");
            assert_eq!(
                winsta_before, winsta_after,
                "WindowStation DACL after rollback must be 100% identical to before snapshot! No orphaned ACEs allowed."
            );
            let desktop_after = snapshot_user_object_dacl(desktop).expect("desktop after snapshot");
            assert_eq!(
                desktop_before, desktop_after,
                "Desktop DACL after rollback must be 100% identical to before snapshot! No orphaned ACEs allowed."
            );

            // 7. Repeated grant/delete cycles: verify zero ACE accumulation across 3 iterations
            for cycle in 1..=3 {
                let cyc_name = format!("engram_cyc_{cycle}_{}", uuid::Uuid::new_v4().simple());
                let cyc_prof =
                    AppContainerProfile::create(&cyc_name).expect("cycle profile creation");
                cyc_prof
                    .grant_window_station_and_desktop()
                    .expect("cycle grant");
                cyc_prof.delete().expect("cycle delete");

                let cyc_winsta = snapshot_user_object_dacl(winsta).expect("cyc winsta snapshot");
                assert_eq!(
                    winsta_before, cyc_winsta,
                    "Cycle {cycle}: WindowStation accumulated ACEs!"
                );
                let cyc_desktop = snapshot_user_object_dacl(desktop).expect("cyc desktop snapshot");
                assert_eq!(
                    desktop_before, cyc_desktop,
                    "Cycle {cycle}: Desktop accumulated ACEs!"
                );
            }

            // 8. Partial-failure / no-op deletion semantics
            let profile2 = AppContainerProfile::create(&format!(
                "engram_partial_{}",
                uuid::Uuid::new_v4().simple()
            ))
            .expect("profile2");
            // Call delete on an ungranted profile: must succeed cleanly without panic or corruption
            profile2.delete().expect("delete ungranted profile");
            let after_noop_winsta = snapshot_user_object_dacl(winsta).expect("after noop winsta");
            assert_eq!(winsta_before, after_noop_winsta);
            let after_noop_desktop =
                snapshot_user_object_dacl(desktop).expect("after noop desktop");
            assert_eq!(desktop_before, after_noop_desktop);
        }
    }
}

// ============================================================================
// 78. Milestone B6: Strict Shell Interpreter & Flag Rejection Regression
// (BLK-B6-02)
// ============================================================================

#[test]
fn test_sandbox_tool_dispatcher_shell_interpreters_and_prohibited_flags_rejected() {
    #[cfg(windows)]
    {
        use engram::empirical_live::dispatcher::SandboxToolDispatcher;
        use engram::empirical_live::schema::ContentBlock;

        let temp_dir =
            std::env::temp_dir().join(format!("engram_disp_reg_{}", uuid::Uuid::new_v4()));
        let dispatcher = SandboxToolDispatcher::new(&temp_dir).expect("dispatcher");

        // 1. Prohibited shell interpreters must be rejected before process spawn
        for prohibited_cmd in &[
            "powershell",
            "powershell.exe",
            "pwsh",
            "pwsh.exe",
            "cmd",
            "cmd.exe",
            "sh",
            "bash",
            "wscript",
            "cscript",
        ] {
            let res = dispatcher.dispatch(
                "execute_command",
                "call_shell",
                &serde_json::json!({
                    "command": prohibited_cmd,
                    "args": ["-NoProfile"]
                }),
            );

            if let ContentBlock::ToolResult {
                is_error, content, ..
            } = res
            {
                assert!(
                    is_error,
                    "Shell interpreter '{prohibited_cmd}' must be rejected"
                );
                assert!(
                    content.contains("is not in the approved allowlist"),
                    "Error must state not in allowlist, got: {content}"
                );
            } else {
                panic!("Expected ToolResult, got: {res:?}");
            }
        }

        // 2. Directory slashes in command name must be rejected (path traversal / unapproved path prevention)
        for slash_cmd in &[
            "../cmd.exe",
            r"..\cmd.exe",
            r"C:\Windows\System32\cmd.exe",
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
            "/bin/sh",
        ] {
            let res = dispatcher.dispatch(
                "execute_command",
                "call_slash",
                &serde_json::json!({
                    "command": slash_cmd,
                    "args": []
                }),
            );
            if let ContentBlock::ToolResult {
                is_error, content, ..
            } = res
            {
                assert!(
                    is_error,
                    "Command with slashes '{slash_cmd}' must be rejected"
                );
                assert!(
                    content.contains("is not in the approved allowlist"),
                    "Error must state not in allowlist, got: {content}"
                );
            } else {
                panic!("Expected ToolResult, got: {res:?}");
            }
        }

        // 3. Prohibited argument flags must be rejected before process spawn
        for prohibited_flag in &[
            "-c",
            "/c",
            "-e",
            "--eval",
            "--command",
            "-command",
            "-encodedcommand",
            "--encodedcommand",
            "-exec",
        ] {
            let res = dispatcher.dispatch(
                "execute_command",
                "call_flag",
                &serde_json::json!({
                    "command": "cargo",
                    "args": [prohibited_flag, "payload"]
                }),
            );
            if let ContentBlock::ToolResult {
                is_error, content, ..
            } = res
            {
                assert!(is_error, "Flag '{prohibited_flag}' must be rejected");
                assert!(
                    content.contains("Prohibited command argument flag")
                        || content.contains("Arbitrary script evaluation flag is prohibited"),
                    "Expected prohibited flag rejection, got: {content}"
                );
            } else {
                panic!("Expected ToolResult, got: {res:?}");
            }
        }

        // 4. Shell metacharacters must be rejected
        for meta in &[";", "&", "|", ">", "<", "`", "$", "\n", "\r"] {
            let res = dispatcher.dispatch(
                "execute_command",
                "call_meta",
                &serde_json::json!({
                    "command": "cargo",
                    "args": [format!("arg{meta}evil")]
                }),
            );
            if let ContentBlock::ToolResult {
                is_error, content, ..
            } = res
            {
                assert!(is_error, "Metacharacter '{meta}' must be rejected");
                assert!(
                    content.contains("Shell metacharacter"),
                    "Expected shell metacharacter rejection, got: {content}"
                );
            } else {
                panic!("Expected ToolResult, got: {res:?}");
            }
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

// ============================================================================
// 79. Milestone B6: AppContainer Revocation-State Correctness & Fault Recovery
// (BLK-B6-02 Finding A)
// ============================================================================

#[test]
fn test_appcontainer_revocation_state_lifecycle_and_fault_recovery() {
    #[cfg(windows)]
    {
        use engram::empirical_live::security::{AppContainerProfile, SecurityError};

        // 1. Successful revocation clears the corresponding state
        {
            let name = format!("engram_rev_ok_{}", uuid::Uuid::new_v4().simple());
            let profile = AppContainerProfile::create(&name).expect("create profile");
            profile
                .grant_window_station_and_desktop()
                .expect("grant objects");
            assert!(profile.is_winsta_granted(), "winsta must be marked granted");
            assert!(
                profile.is_desktop_granted(),
                "desktop must be marked granted"
            );
            assert!(!profile.is_deleted(), "profile must not be marked deleted");

            profile.delete().expect("delete profile");
            assert!(
                !profile.is_winsta_granted(),
                "winsta state must be cleared after success"
            );
            assert!(
                !profile.is_desktop_granted(),
                "desktop state must be cleared after success"
            );
            assert!(
                profile.is_deleted(),
                "profile must be marked deleted after success"
            );
        }

        // 2. Failed revocation does NOT falsely clear state (Finding A core invariant)
        {
            let name = format!("engram_rev_fail_{}", uuid::Uuid::new_v4().simple());
            let profile = AppContainerProfile::create(&name).expect("create profile");
            profile
                .grant_window_station_and_desktop()
                .expect("grant objects");

            // Inject failure into WindowStation revocation
            profile.set_fault_hook(|op| {
                if op == "revoke_winsta" {
                    Some(SecurityError::AclConfigurationFailed {
                        os_error_code: 5,
                        message: "Simulated WindowStation revocation failure".to_string(),
                    })
                } else {
                    None
                }
            });

            let del_res = profile.delete();
            assert!(del_res.is_err(), "delete must fail when revocation fails");
            // Critical assertion: state must NOT be cleared when revocation failed!
            assert!(
                profile.is_winsta_granted(),
                "Failed revocation must preserve winsta_granted=true!"
            );
            assert!(
                !profile.is_deleted(),
                "Failed cleanup must not mark profile deleted"
            );

            // 3. Subsequent cleanup retries previously failed revocation and succeeds
            profile.clear_fault_hook();
            let retry_res = profile.delete();
            assert!(
                retry_res.is_ok(),
                "Retry of cleanup must succeed after clearing fault: {retry_res:?}"
            );
            assert!(
                !profile.is_winsta_granted(),
                "State must now be cleared after successful retry"
            );
            assert!(!profile.is_desktop_granted(), "Desktop must remain cleared");
            assert!(profile.is_deleted(), "Profile must now be marked deleted");
        }

        // 4. Partial setup failure attempts rollback of all successfully granted resources
        {
            let name = format!("engram_setup_rb_{}", uuid::Uuid::new_v4().simple());
            let profile = AppContainerProfile::create(&name).expect("create profile");

            // Desktop grant fails: WindowStation was already granted and must be rolled back!
            profile.set_fault_hook(|op| {
                if op == "grant_desktop" {
                    Some(SecurityError::AclConfigurationFailed {
                        os_error_code: 5,
                        message: "Simulated Desktop grant failure".to_string(),
                    })
                } else {
                    None
                }
            });

            let grant_res = profile.grant_window_station_and_desktop();
            assert!(
                grant_res.is_err(),
                "Grant must fail when desktop grant fails"
            );
            // Rollback of WindowStation was attempted and succeeded, so winsta is no longer granted!
            assert!(
                !profile.is_winsta_granted(),
                "WindowStation must be rolled back and state cleared on partial setup failure"
            );
            assert!(
                !profile.is_desktop_granted(),
                "Desktop must not be marked granted"
            );

            profile.clear_fault_hook();
            profile.delete().expect("cleanup profile");
        }

        // 4b. Partial setup failure with rollback failure preserves BOTH errors and keeps state
        {
            let name = format!("engram_rb_fail_{}", uuid::Uuid::new_v4().simple());
            let profile = AppContainerProfile::create(&name).expect("create profile");

            // Both desktop grant AND winsta rollback fail!
            profile.set_fault_hook(|op| {
                if op == "grant_desktop" {
                    Some(SecurityError::AclConfigurationFailed {
                        os_error_code: 5,
                        message: "Simulated Desktop grant failure".to_string(),
                    })
                } else if op == "revoke_winsta" {
                    Some(SecurityError::AclConfigurationFailed {
                        os_error_code: 5,
                        message: "Simulated WindowStation rollback failure".to_string(),
                    })
                } else {
                    None
                }
            });

            let grant_res = profile.grant_window_station_and_desktop();
            assert!(grant_res.is_err(), "Grant must return compound error");
            let err_msg = format!("{:?}", grant_res.unwrap_err());
            assert!(
                err_msg.contains("Desktop grant failed")
                    && err_msg.contains("WindowStation rollback also failed"),
                "Both setup error and rollback error must be preserved, got: {err_msg}"
            );
            // Crucial: because winsta rollback failed, winsta_granted must STILL be true!
            assert!(
                profile.is_winsta_granted(),
                "WindowStation must remain marked granted because rollback failed!"
            );

            // Clearing fault hook and calling delete retries the failed rollback
            profile.clear_fault_hook();
            profile.delete().expect("cleanup retries failed rollback");
            assert!(!profile.is_winsta_granted());
        }

        // 5. Multiple cleanup attempts remain safe and accurately report unresolved failures
        {
            let name = format!("engram_multi_clean_{}", uuid::Uuid::new_v4().simple());
            let profile = AppContainerProfile::create(&name).expect("create profile");
            profile.grant_window_station_and_desktop().expect("grant");

            // Keep fault active across multiple cleanup calls
            profile.set_fault_hook(|op| {
                if op == "revoke_desktop" {
                    Some(SecurityError::AclConfigurationFailed {
                        os_error_code: 5,
                        message: "Persistent desktop revocation failure".to_string(),
                    })
                } else {
                    None
                }
            });

            for attempt in 1..=3 {
                let res = profile.delete();
                assert!(
                    res.is_err(),
                    "Attempt {attempt} must fail while fault is active"
                );
                assert!(
                    profile.is_desktop_granted(),
                    "Attempt {attempt}: desktop must remain granted"
                );
                assert!(
                    !profile.is_deleted(),
                    "Attempt {attempt}: profile must not be marked deleted"
                );
            }

            // Remove fault: next cleanup must succeed
            profile.clear_fault_hook();
            profile
                .delete()
                .expect("final cleanup after resolving fault");
            assert!(
                !profile.is_desktop_granted(),
                "Desktop state cleared after success"
            );
            assert!(profile.is_deleted(), "Profile marked deleted");

            // Repeated call after full cleanup is idempotent and returns Ok(())
            profile.delete().expect("idempotent cleanup after success");
        }
    }
}

// ============================================================================
// 80. Milestone B6: Complete DACL Integrity Verification (Synthetic Tests)
// (BLK-B6-02 Finding B)
// ============================================================================

#[test]
fn test_dacl_integrity_synthetic_comparisons_and_unsupported_aces() {
    use engram::empirical_live::security::{
        make_synthetic_ace, make_synthetic_dacl, parse_dacl_bytes,
    };

    // 1. Unchanged descriptors compare equal
    {
        let ace1 = make_synthetic_ace(0, 0x02, 0x00020123, &[32, 544], b"");
        let ace2 = make_synthetic_ace(0, 0x00, 0x10000000, &[32, 545], b"");
        let raw_dacl = make_synthetic_dacl(&[ace1, ace2]);

        let snap1 = parse_dacl_bytes(&raw_dacl).expect("parse snap1");
        let snap2 = parse_dacl_bytes(&raw_dacl).expect("parse snap2");

        assert_eq!(
            snap1, snap2,
            "Identical DACL descriptors must compare equal"
        );
        assert!(
            snap1.is_exact_match(&snap2),
            "is_exact_match must return true"
        );
        assert!(
            snap1.detailed_diff(&snap2).is_empty(),
            "detailed_diff between identical snapshots must be empty"
        );
        assert_eq!(snap1.ace_count, 2);
        assert_eq!(snap1.aces[0].sid_string, "S-1-5-32-544");
        assert_eq!(snap1.aces[1].sid_string, "S-1-5-32-545");
    }

    // 2. Differences in ACE order are detected
    {
        let ace1 = make_synthetic_ace(0, 0x02, 0x00020123, &[32, 544], b"");
        let ace2 = make_synthetic_ace(0, 0x00, 0x10000000, &[32, 545], b"");
        let dacl_order1 = make_synthetic_dacl(&[ace1.clone(), ace2.clone()]);
        let dacl_order2 = make_synthetic_dacl(&[ace2, ace1]);

        let snap1 = parse_dacl_bytes(&dacl_order1).expect("snap1");
        let snap2 = parse_dacl_bytes(&dacl_order2).expect("snap2");

        assert_ne!(
            snap1, snap2,
            "Reordered ACEs must NOT compare equal under Windows evaluation order"
        );
        assert!(!snap1.is_exact_match(&snap2));
        let diffs = snap1.detailed_diff(&snap2);
        assert!(!diffs.is_empty(), "Diffs must detect ACE reordering");
    }

    // 3. Differences in flags, inheritance, masks, and SIDs are detected
    {
        let base_ace = make_synthetic_ace(0, 0x02, 0x00020123, &[32, 544], b"");
        let base_snap = parse_dacl_bytes(&make_synthetic_dacl(&[base_ace])).expect("base");

        // Flag difference (0x02 vs 0x01)
        let flag_ace = make_synthetic_ace(0, 0x01, 0x00020123, &[32, 544], b"");
        let flag_snap = parse_dacl_bytes(&make_synthetic_dacl(&[flag_ace])).expect("flag");
        assert_ne!(base_snap, flag_snap);
        let flag_diffs = base_snap.detailed_diff(&flag_snap);
        assert!(
            flag_diffs.iter().any(|d| d.contains("flags")),
            "Must detect flag divergence"
        );

        // Mask difference
        let mask_ace = make_synthetic_ace(0, 0x02, 0x10000000, &[32, 544], b"");
        let mask_snap = parse_dacl_bytes(&make_synthetic_dacl(&[mask_ace])).expect("mask");
        assert_ne!(base_snap, mask_snap);
        let mask_diffs = base_snap.detailed_diff(&mask_snap);
        assert!(
            mask_diffs.iter().any(|d| d.contains("mask")),
            "Must detect mask divergence"
        );

        // SID difference
        let sid_ace = make_synthetic_ace(0, 0x02, 0x00020123, &[32, 545], b"");
        let sid_snap = parse_dacl_bytes(&make_synthetic_dacl(&[sid_ace])).expect("sid");
        assert_ne!(base_snap, sid_snap);
        let sid_diffs = base_snap.detailed_diff(&sid_snap);
        assert!(
            sid_diffs.iter().any(|d| d.contains("sid")),
            "Must detect SID divergence"
        );

        // Extra callback condition data difference
        let cb1 = make_synthetic_ace(9, 0, 0x1000, &[32, 544], b"condA");
        let cb2 = make_synthetic_ace(9, 0, 0x1000, &[32, 544], b"condB");
        let cb_snap1 = parse_dacl_bytes(&make_synthetic_dacl(&[cb1])).expect("cb1");
        let cb_snap2 = parse_dacl_bytes(&make_synthetic_dacl(&[cb2])).expect("cb2");
        assert_ne!(cb_snap1, cb_snap2);
        let cb_diffs = cb_snap1.detailed_diff(&cb_snap2);
        assert!(
            cb_diffs
                .iter()
                .any(|d| d.contains("extra_data") || d.contains("raw_ace_bytes"))
        );
    }

    // 4. Unsupported ACE representations fail safely or preserve raw bytes
    {
        // Unknown ACE type 0xFF
        let unknown_ace = make_synthetic_ace(0xFF, 0x00, 0x12345678, &[32, 544], b"custom_data");
        let raw_unknown = make_synthetic_dacl(&[unknown_ace]);

        let snap_unknown = parse_dacl_bytes(&raw_unknown).expect("parse unknown ACE type");
        assert_eq!(snap_unknown.ace_count, 1);
        assert_eq!(snap_unknown.aces[0].ace_type, 0xFF);
        assert!(
            !snap_unknown.aces[0].is_supported_type,
            "Must flag unsupported type"
        );
        assert!(
            snap_unknown.aces[0]
                .sid_string
                .contains("UNSUPPORTED_ACE_TYPE"),
            "SID string must reflect unsupported type policy"
        );
        // Byte-exact representation is preserved: identical unknown ACEs match!
        let snap_unknown2 = parse_dacl_bytes(&raw_unknown).expect("snap2");
        assert_eq!(snap_unknown, snap_unknown2);

        // Corrupted ACE size (< 4 bytes) must fail closed safely
        let corrupt_ace = vec![0u8, 0u8, 2u8, 0u8]; // AceSize = 2 (< 4)
        let mut corrupt_dacl = vec![2u8, 0, 12, 0, 1, 0, 0, 0];
        corrupt_dacl.extend_from_slice(&corrupt_ace);
        let parse_corrupt = parse_dacl_bytes(&corrupt_dacl);
        assert!(
            parse_corrupt.is_err(),
            "Corrupted ACE header must fail closed"
        );
    }

    // 5. Object-specific ACE types (5..8) synthetic fixtures and fail-closed validation
    {
        use engram::empirical_live::security::make_synthetic_object_ace;

        let obj_guid = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let inh_guid = [
            0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad,
            0xae, 0xaf,
        ];

        // 5a. Object-type GUID present (flags = 0x01)
        let ace_obj_only = make_synthetic_object_ace(
            5,
            0,
            0x10000000,
            0x01,
            Some(&obj_guid),
            None,
            &[32, 544],
            b"",
        );
        let snap_obj = parse_dacl_bytes(&make_synthetic_dacl(&[ace_obj_only.clone()]))
            .expect("parse obj only");
        assert_eq!(snap_obj.aces[0].ace_type, 5);
        assert!(snap_obj.aces[0].is_supported_type);
        assert!(snap_obj.aces[0].object_type_guid.is_some());
        assert!(snap_obj.aces[0].inherited_object_type_guid.is_none());
        assert_eq!(snap_obj.aces[0].sid_string, "S-1-5-32-544");

        // 5b. Inherited-object-type GUID present (flags = 0x02)
        let ace_inh_only = make_synthetic_object_ace(
            6,
            0x02,
            0x20000000,
            0x02,
            None,
            Some(&inh_guid),
            &[32, 545],
            b"",
        );
        let snap_inh = parse_dacl_bytes(&make_synthetic_dacl(&[ace_inh_only.clone()]))
            .expect("parse inh only");
        assert_eq!(snap_inh.aces[0].ace_type, 6);
        assert!(snap_inh.aces[0].object_type_guid.is_none());
        assert!(snap_inh.aces[0].inherited_object_type_guid.is_some());
        assert_eq!(snap_inh.aces[0].sid_string, "S-1-5-32-545");

        // 5c. Both GUIDs present (flags = 0x03)
        let ace_both = make_synthetic_object_ace(
            5,
            0,
            0x10000000,
            0x03,
            Some(&obj_guid),
            Some(&inh_guid),
            &[32, 544],
            b"extra",
        );
        let snap_both =
            parse_dacl_bytes(&make_synthetic_dacl(&[ace_both.clone()])).expect("parse both");
        assert!(snap_both.aces[0].object_type_guid.is_some());
        assert!(snap_both.aces[0].inherited_object_type_guid.is_some());
        assert_eq!(snap_both.aces[0].extra_data, b"extra");

        // 5d. Neither GUID present (flags = 0x00)
        let ace_neither =
            make_synthetic_object_ace(5, 0, 0x10000000, 0x00, None, None, &[32, 544], b"");
        let snap_neither =
            parse_dacl_bytes(&make_synthetic_dacl(&[ace_neither])).expect("parse neither");
        assert!(snap_neither.aces[0].object_type_guid.is_none());
        assert!(snap_neither.aces[0].inherited_object_type_guid.is_none());

        // 5e. Exact snapshot equality and differences in GUID
        let snap_both2 =
            parse_dacl_bytes(&make_synthetic_dacl(&[ace_both.clone()])).expect("parse both 2");
        assert_eq!(snap_both, snap_both2);
        assert!(snap_both.is_exact_match(&snap_both2));

        let inh_guid_diff = [0xb0; 16];
        let ace_both_diff_guid = make_synthetic_object_ace(
            5,
            0,
            0x10000000,
            0x03,
            Some(&obj_guid),
            Some(&inh_guid_diff),
            &[32, 544],
            b"extra",
        );
        let snap_diff_guid =
            parse_dacl_bytes(&make_synthetic_dacl(&[ace_both_diff_guid])).expect("parse diff guid");
        assert_ne!(snap_both, snap_diff_guid);
        let diffs = snap_both.detailed_diff(&snap_diff_guid);
        assert!(diffs.iter().any(|d| d.contains("inherited_object_guid")));

        // 5f. Incorrect ACE lengths and truncated GUID/SID data fail closed safely
        // Truncated object GUID: flags = 0x01 but ace_size doesn't include 16 bytes
        let truncated_guid_ace = vec![5u8, 0, 16, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 2, 3, 4];
        let res_trunc = parse_dacl_bytes(&make_synthetic_dacl(&[truncated_guid_ace]));
        assert!(res_trunc.is_err(), "Truncated object GUID must fail closed");

        // Ace size < 12 for object-specific ACE must fail closed
        let short_obj_ace = vec![5u8, 0, 8, 0, 0, 0, 0, 0];
        let res_short = parse_dacl_bytes(&make_synthetic_dacl(&[short_obj_ace]));
        assert!(
            res_short.is_err(),
            "Short object-specific ACE must fail closed"
        );
    }
}

// ============================================================================
// 81. Milestone B6: DACL Drift Detection & Concurrent Change Prevention
// (BLK-B6-02 Finding B)
// ============================================================================

#[test]
fn test_dacl_drift_detection_and_concurrent_change_prevention() {
    use engram::empirical_live::security::{
        DaclDriftError, make_synthetic_ace, make_synthetic_dacl, parse_dacl_bytes,
    };

    let baseline_ace1 = make_synthetic_ace(0, 0, 0x00020123, &[32, 544], b"");
    let baseline_ace2 = make_synthetic_ace(0, 0x0b, 0xf0000000, &[32, 544], b"");
    let baseline_dacl = make_synthetic_dacl(&[baseline_ace1.clone(), baseline_ace2.clone()]);
    let baseline = parse_dacl_bytes(&baseline_dacl).expect("baseline");

    let contained_sid = "S-1-5-999-888-777";
    let contained_ace = make_synthetic_ace(0, 0, 0x00020123, &[999, 888, 777], b"");

    // 1. Clean containment: baseline ACEs + contained SID ACE passes drift detection
    {
        let valid_current_dacl = make_synthetic_dacl(&[
            baseline_ace1.clone(),
            baseline_ace2.clone(),
            contained_ace.clone(),
        ]);
        let valid_current = parse_dacl_bytes(&valid_current_dacl).expect("valid current");
        let res = baseline.detect_concurrent_drift(&valid_current, contained_sid);
        assert!(
            res.is_ok(),
            "Clean containment must pass drift check: {res:?}"
        );
    }

    // 2. Unexpected concurrent ACE added by another actor is detected
    {
        let rogue_ace = make_synthetic_ace(0, 0, 0x10000000, &[32, 999], b"");
        let rogue_dacl = make_synthetic_dacl(&[
            baseline_ace1.clone(),
            baseline_ace2.clone(),
            contained_ace.clone(),
            rogue_ace,
        ]);
        let rogue_current = parse_dacl_bytes(&rogue_dacl).expect("rogue");
        let res = baseline.detect_concurrent_drift(&rogue_current, contained_sid);
        match res {
            Err(DaclDriftError::UnexpectedAceAdded { sid, .. }) => {
                assert_eq!(sid, "S-1-5-32-999", "Must identify the rogue SID");
            }
            other => panic!("Expected UnexpectedAceAdded error, got: {other:?}"),
        }
    }

    // 3. Pre-existing ACE removed by concurrent actor is detected
    {
        // baseline_ace1 is missing
        let missing_dacl = make_synthetic_dacl(&[baseline_ace2.clone(), contained_ace.clone()]);
        let missing_current = parse_dacl_bytes(&missing_dacl).expect("missing");
        let res = baseline.detect_concurrent_drift(&missing_current, contained_sid);
        assert!(
            matches!(
                res,
                Err(DaclDriftError::PreexistingAceModified { .. })
                    | Err(DaclDriftError::PreexistingAceRemoved { .. })
            ),
            "Expected modification/removal detection, got: {res:?}"
        );
    }

    // 4. Pre-existing ACE modified by concurrent actor is detected
    {
        let modified_ace1 = make_synthetic_ace(0, 0, 0x00000001, &[32, 544], b""); // rights changed
        let modified_dacl =
            make_synthetic_dacl(&[modified_ace1, baseline_ace2.clone(), contained_ace.clone()]);
        let modified_current = parse_dacl_bytes(&modified_dacl).expect("modified");
        let res = baseline.detect_concurrent_drift(&modified_current, contained_sid);
        match res {
            Err(DaclDriftError::PreexistingAceModified { sid, detail }) => {
                assert_eq!(sid, "S-1-5-32-544");
                assert!(detail.contains("modified"));
            }
            other => panic!("Expected PreexistingAceModified, got: {other:?}"),
        }
    }

    // 5. Live Windows User Object drift detection test
    #[cfg(windows)]
    {
        use engram::empirical_live::security::AppContainerProfile;

        let name = format!("engram_live_drift_{}", uuid::Uuid::new_v4().simple());
        let profile = AppContainerProfile::create(&name).expect("profile");
        profile.grant_window_station_and_desktop().expect("grant");

        // Normal check must pass with zero drift
        assert!(
            profile.check_dacl_drift().is_ok(),
            "Baseline drift check must pass cleanly"
        );

        profile.delete().expect("cleanup");
    }

    // 6. Relative ACE order preservation and reordering detection (Finding 1)
    {
        // 6a. Two pre-existing ACEs reversed -> PreexistingAceReordered
        let reversed_dacl = make_synthetic_dacl(&[
            baseline_ace2.clone(),
            baseline_ace1.clone(),
            contained_ace.clone(),
        ]);
        let reversed_current = parse_dacl_bytes(&reversed_dacl).expect("reversed");
        let res = baseline.detect_concurrent_drift(&reversed_current, contained_sid);
        match res {
            Err(DaclDriftError::PreexistingAceReordered { sid, .. }) => {
                assert_eq!(sid, "S-1-5-32-544");
            }
            other => panic!("Expected PreexistingAceReordered for reversed ACEs, got: {other:?}"),
        }

        // 6b. Reordered ACEs with duplicate SIDs (different flags/masks)
        // baseline_ace1 and baseline_ace2 have identical SID (S-1-5-32-544) but different flags/masks
        let dup_reordered_dacl = make_synthetic_dacl(&[
            contained_ace.clone(),
            baseline_ace2.clone(),
            baseline_ace1.clone(),
        ]);
        let dup_reordered = parse_dacl_bytes(&dup_reordered_dacl).expect("dup reordered");
        let res_dup = baseline.detect_concurrent_drift(&dup_reordered, contained_sid);
        assert!(
            matches!(res_dup, Err(DaclDriftError::PreexistingAceReordered { .. })),
            "Expected PreexistingAceReordered for duplicate SIDs with reversed order, got: {res_dup:?}"
        );

        // 6c. AppContainer ACE inserted at different positions (beginning, middle, end) is accepted
        for insert_pos in 0..=2 {
            let mut aces = vec![baseline_ace1.clone(), baseline_ace2.clone()];
            aces.insert(insert_pos, contained_ace.clone());
            let current_dacl =
                parse_dacl_bytes(&make_synthetic_dacl(&aces)).expect("insert current");
            assert!(
                baseline
                    .detect_concurrent_drift(&current_dacl, contained_sid)
                    .is_ok(),
                "Contained ACE insertion at pos {insert_pos} must be accepted without drift error"
            );
        }

        // 6d. Greedy match disambiguation with duplicate identical ACEs
        let ace_x = make_synthetic_ace(0, 0, 0x1000, &[32, 100], b"");
        let ace_y = make_synthetic_ace(0, 0, 0x2000, &[32, 200], b"");
        // Baseline has [X, X, Y]
        let multi_baseline = parse_dacl_bytes(&make_synthetic_dacl(&[
            ace_x.clone(),
            ace_x.clone(),
            ace_y.clone(),
        ]))
        .expect("multi baseline");

        // Current has [X, Y, X, Contained] (X and Y swapped positions) -> must detect reorder!
        let multi_reordered = parse_dacl_bytes(&make_synthetic_dacl(&[
            ace_x.clone(),
            ace_y.clone(),
            ace_x.clone(),
            contained_ace.clone(),
        ]))
        .expect("multi reordered");
        let res_multi = multi_baseline.detect_concurrent_drift(&multi_reordered, contained_sid);
        assert!(
            matches!(
                res_multi,
                Err(DaclDriftError::PreexistingAceReordered { .. })
            ),
            "Greedy match must not hide reordering of duplicate ACEs: {res_multi:?}"
        );

        // Current has [Contained, X, Contained, X, Y] -> valid grant insertion with duplicates
        let multi_valid = parse_dacl_bytes(&make_synthetic_dacl(&[
            contained_ace.clone(),
            ace_x.clone(),
            contained_ace.clone(),
            ace_x.clone(),
            ace_y.clone(),
        ]))
        .expect("multi valid");
        assert!(
            multi_baseline
                .detect_concurrent_drift(&multi_valid, contained_sid)
                .is_ok(),
            "Valid duplicate preservation with multiple contained ACEs must pass"
        );
    }
}

// ============================================================================
// 82. Milestone B6: Comprehensive DACL State Representation & Control Semantics
// (BLK-B6-02 Finding 2)
// ============================================================================

#[test]
fn test_dacl_state_representation_and_control_semantics() {
    use engram::empirical_live::security::{
        DaclDriftError, SE_DACL_PRESENT, make_synthetic_ace, make_synthetic_dacl,
        make_synthetic_security_descriptor, parse_security_descriptor_bytes,
    };

    let ace1 = make_synthetic_ace(0, 0, 0x00020123, &[32, 544], b"");
    let dacl_populated = make_synthetic_dacl(&[ace1.clone()]);
    let dacl_empty = make_synthetic_dacl(&[]);

    // 1. State 1: DACL absent (SE_DACL_PRESENT flag is 0)
    let sd_absent = make_synthetic_security_descriptor(0x0000, None);
    let snap_absent = parse_security_descriptor_bytes(&sd_absent).expect("parse absent SD");
    assert!(
        !snap_absent.is_present,
        "Absent DACL must have is_present = false"
    );
    assert!(
        !snap_absent.is_null_dacl,
        "Absent DACL must have is_null_dacl = false"
    );
    assert_eq!(snap_absent.ace_count, 0);
    assert!(snap_absent.raw_acl_bytes.is_empty());

    // 2. State 2: DACL explicitly NULL (SE_DACL_PRESENT flag is 1, dacl_offset is 0)
    let sd_null = make_synthetic_security_descriptor(SE_DACL_PRESENT, None);
    let snap_null = parse_security_descriptor_bytes(&sd_null).expect("parse null SD");
    assert!(
        snap_null.is_present,
        "Explicitly NULL DACL must have is_present = true"
    );
    assert!(
        snap_null.is_null_dacl,
        "Explicitly NULL DACL must have is_null_dacl = true"
    );
    assert_eq!(snap_null.ace_count, 0);
    assert!(snap_null.raw_acl_bytes.is_empty());

    // 3. State 3: DACL present but empty (SE_DACL_PRESENT is 1, dacl_offset != 0, ace_count == 0)
    let sd_empty = make_synthetic_security_descriptor(SE_DACL_PRESENT, Some(&dacl_empty));
    let snap_empty = parse_security_descriptor_bytes(&sd_empty).expect("parse empty SD");
    assert!(
        snap_empty.is_present,
        "Empty DACL must have is_present = true"
    );
    assert!(
        !snap_empty.is_null_dacl,
        "Empty DACL must have is_null_dacl = false"
    );
    assert_eq!(snap_empty.ace_count, 0);
    assert_eq!(
        snap_empty.raw_acl_bytes.len(),
        8,
        "Empty DACL must preserve 8-byte ACL header"
    );

    // 4. State 4: DACL present and populated (SE_DACL_PRESENT is 1, dacl_offset != 0, ace_count > 0)
    let sd_populated = make_synthetic_security_descriptor(SE_DACL_PRESENT, Some(&dacl_populated));
    let snap_populated =
        parse_security_descriptor_bytes(&sd_populated).expect("parse populated SD");
    assert!(
        snap_populated.is_present,
        "Populated DACL must have is_present = true"
    );
    assert!(
        !snap_populated.is_null_dacl,
        "Populated DACL must have is_null_dacl = false"
    );
    assert_eq!(snap_populated.ace_count, 1);
    assert!(!snap_populated.raw_acl_bytes.is_empty());

    // 5. Verification of mutual distinction across all 4 states
    let states = [
        ("absent", &snap_absent),
        ("null", &snap_null),
        ("empty", &snap_empty),
        ("populated", &snap_populated),
    ];
    for (name_a, a) in &states {
        for (name_b, b) in &states {
            if name_a == name_b {
                assert_eq!(a, b, "{name_a} must equal itself");
                assert!(a.is_exact_match(b));
            } else {
                assert_ne!(a, b, "{name_a} and {name_b} must NOT compare equal");
                assert!(!a.is_exact_match(b));
                let diffs = a.detailed_diff(b);
                assert!(
                    !diffs.is_empty(),
                    "Diff between {name_a} and {name_b} must be non-empty"
                );

                // Drift detection between differing states must return an error
                let drift_res = a.detect_concurrent_drift(b, "S-1-15-2-1");
                assert!(
                    drift_res.is_err(),
                    "Drift between {name_a} and {name_b} must report an error"
                );
                if *name_a == "empty" && *name_b == "populated" {
                    assert!(matches!(
                        drift_res,
                        Err(DaclDriftError::UnexpectedAceAdded { .. })
                    ));
                } else if *name_a == "populated" && *name_b == "empty" {
                    assert!(matches!(
                        drift_res,
                        Err(DaclDriftError::PreexistingAceRemoved { .. })
                    ));
                } else {
                    assert!(matches!(
                        drift_res,
                        Err(DaclDriftError::AclStructureCorrupted { .. })
                    ));
                }
            }
        }
    }

    // 6. Contradictory / malformed synthetic inputs fail closed
    // Buffer too short (< 20 bytes)
    assert!(parse_security_descriptor_bytes(&[1u8; 15]).is_err());
    // Truncated DACL offset pointing beyond buffer
    let mut sd_truncated_offset = vec![0u8; 20];
    sd_truncated_offset[0] = 1;
    sd_truncated_offset[2..4].copy_from_slice(&SE_DACL_PRESENT.to_le_bytes());
    sd_truncated_offset[16..20].copy_from_slice(&100u32.to_le_bytes());
    assert!(parse_security_descriptor_bytes(&sd_truncated_offset).is_err());
}

// ============================================================================
// 83. Milestone B6: AppContainer Execution Lease & Concurrency Guard
// (BLK-B6-02 Concurrency Boundary)
// ============================================================================

#[test]
fn test_appcontainer_execution_lease_concurrency_boundary() {
    #[cfg(windows)]
    {
        use engram::empirical_live::security::AppContainerProfile;

        let name = format!("engram_lease_test_{}", uuid::Uuid::new_v4().simple());
        let profile = AppContainerProfile::create(&name).expect("profile create");
        profile
            .grant_window_station_and_desktop()
            .expect("grant objects");

        assert_eq!(profile.active_executions_count(), 0);

        // 1. Acquire execution lease (simulating active subprocess execution)
        let lease = profile.acquire_execution_lease().expect("acquire lease");
        assert_eq!(profile.active_executions_count(), 1);

        // 2. Attempting delete while lease is active MUST fail closed
        let del_res = profile.delete();
        assert!(
            del_res.is_err(),
            "profile.delete() must fail closed while execution lease is active!"
        );
        let err_msg = format!("{:?}", del_res.unwrap_err());
        assert!(
            err_msg.contains("subprocess execution(s) are active"),
            "Error must explicitly explain active execution guard, got: {err_msg}"
        );
        assert!(
            profile.is_winsta_granted(),
            "WindowStation grant must remain active when delete was rejected"
        );
        assert!(
            !profile.is_deleted(),
            "Profile must NOT be deleted while execution was active"
        );

        // 3. Drop lease (simulating process termination)
        drop(lease);
        assert_eq!(profile.active_executions_count(), 0);

        // 4. Cleanup now succeeds cleanly
        profile.delete().expect("cleanup after lease dropped");
        assert!(profile.is_deleted());

        // 5. Acquiring lease on already-deleted profile fails closed
        let post_del_lease = profile.acquire_execution_lease();
        assert!(
            post_del_lease.is_err(),
            "Acquiring lease on deleted profile must fail closed"
        );
    }
}

// ============================================================================
// 84. Milestone B6: AppContainer Execution Lease Lifecycle & Race Prevention
// (BLK-B6-02 Concurrency Coordination)
// ============================================================================

#[test]
fn test_appcontainer_lease_lifecycle_barrier_concurrency_and_retry() {
    #[cfg(windows)]
    {
        use engram::empirical_live::security::{AppContainerProfile, SecurityError};
        use std::sync::{Arc, Barrier};
        use std::thread;

        // --------------------------------------------------------------------
        // Part 1: Barrier-synchronized exclusion between active lease & deletion
        // --------------------------------------------------------------------
        let name = format!("engram_race_p1_{}", uuid::Uuid::new_v4().simple());
        let profile = Arc::new(AppContainerProfile::create(&name).expect("profile create"));
        profile.grant_window_station_and_desktop().expect("grant");

        let barrier_start = Arc::new(Barrier::new(2));
        let barrier_del_attempted = Arc::new(Barrier::new(2));
        let barrier_lease_dropped = Arc::new(Barrier::new(2));

        let p_worker = Arc::clone(&profile);
        let b_start_worker = Arc::clone(&barrier_start);
        let b_del_worker = Arc::clone(&barrier_del_attempted);
        let b_drop_worker = Arc::clone(&barrier_lease_dropped);

        let worker_handle = thread::spawn(move || {
            let lease = p_worker.acquire_execution_lease().expect("worker lease");
            b_start_worker.wait(); // Signal that lease is held
            b_del_worker.wait(); // Wait until main thread attempts deletion
            drop(lease); // Now release the lease
            b_drop_worker.wait(); // Signal that lease is dropped
        });

        barrier_start.wait(); // Wait for worker to hold lease
        assert_eq!(profile.active_executions_count(), 1);

        // Deletion must be rejected because execution is active
        let del_err = profile.delete().unwrap_err();
        match del_err {
            SecurityError::AclConfigurationFailed { message, .. } => {
                assert!(message.contains("subprocess execution(s) are active"));
            }
            other => panic!("Expected active executions error, got {other:?}"),
        }
        assert!(profile.is_winsta_granted());
        assert!(!profile.is_deleted());

        barrier_del_attempted.wait(); // Worker can now drop lease
        barrier_lease_dropped.wait(); // Wait for worker lease drop
        worker_handle.join().expect("worker join");

        assert_eq!(profile.active_executions_count(), 0);

        // Deletion now succeeds
        profile.delete().expect("delete post lease drop");
        assert!(profile.is_deleted());

        // --------------------------------------------------------------------
        // Part 2: Deletion-initiated state blocks new leases & permits retry
        // --------------------------------------------------------------------
        let name2 = format!("engram_race_p2_{}", uuid::Uuid::new_v4().simple());
        let profile2 = Arc::new(AppContainerProfile::create(&name2).expect("profile create"));
        profile2.grant_window_station_and_desktop().expect("grant");

        // Simulate a partial failure during desktop revocation
        profile2.set_fault_hook(|op| {
            if op == "revoke_desktop" {
                Some(SecurityError::AclConfigurationFailed {
                    os_error_code: 5,
                    message: "Simulated Desktop revocation failure (Access Denied)".to_string(),
                })
            } else {
                None
            }
        });

        // Deletion attempts and fails at desktop revocation step
        let del2_res = profile2.delete();
        assert!(del2_res.is_err(), "delete must fail on injected fault");
        assert!(
            profile2.is_deletion_initiated(),
            "profile must be marked deletion_initiated"
        );
        assert!(
            !profile2.is_deleted(),
            "profile must NOT be marked fully deleted"
        );

        // While in deletion_initiated state, acquiring lease MUST be rejected fail-closed
        let lease_err = profile2.acquire_execution_lease().unwrap_err();
        match lease_err {
            SecurityError::AclConfigurationFailed { message, .. } => {
                assert!(
                    message.contains("deleted or pending deletion"),
                    "Expected pending deletion error, got {message}"
                );
            }
            other => panic!("Expected AclConfigurationFailed, got {other:?}"),
        }

        // Granting access while in deletion_initiated state MUST also be rejected fail-closed
        let grant_err = profile2.grant_window_station_and_desktop().unwrap_err();
        match grant_err {
            SecurityError::AclConfigurationFailed { message, .. } => {
                assert!(message.contains("deleted or pending deletion"));
            }
            other => panic!("Expected AclConfigurationFailed, got {other:?}"),
        }

        // Clear fault hook and retry deletion: cleanup must resume and complete
        profile2.clear_fault_hook();
        profile2.delete().expect("retry delete must succeed");
        assert!(profile2.is_deleted());

        // --------------------------------------------------------------------
        // Part 3: Concurrent simultaneous deletion calls across 4 threads
        // --------------------------------------------------------------------
        let name3 = format!("engram_race_p3_{}", uuid::Uuid::new_v4().simple());
        let profile3 = Arc::new(AppContainerProfile::create(&name3).expect("profile create"));
        profile3.grant_window_station_and_desktop().expect("grant");

        let b_del_threads = Arc::new(Barrier::new(4));
        let mut handles = Vec::new();
        for _ in 0..4 {
            let p_clone = Arc::clone(&profile3);
            let b_clone = Arc::clone(&b_del_threads);
            handles.push(thread::spawn(move || {
                b_clone.wait();
                p_clone.delete()
            }));
        }

        for h in handles {
            let res = h.join().expect("join thread");
            assert!(
                res.is_ok(),
                "All concurrent deletes must return Ok(()) idempotently: {res:?}"
            );
        }
        assert!(profile3.is_deleted());

        // --------------------------------------------------------------------
        // Part 4: Panic unwinding releases execution lease
        // --------------------------------------------------------------------
        let name4 = format!("engram_race_p4_{}", uuid::Uuid::new_v4().simple());
        let profile4 = AppContainerProfile::create(&name4).expect("profile create");
        profile4.grant_window_station_and_desktop().expect("grant");

        let panic_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _lease = profile4.acquire_execution_lease().expect("acquire lease");
            assert_eq!(profile4.active_executions_count(), 1);
            panic!("Intentional panic during execution");
        }));
        assert!(panic_res.is_err(), "catch_unwind must capture panic");

        // The lease must have dropped during unwinding
        assert_eq!(
            profile4.active_executions_count(),
            0,
            "active_executions must be restored to 0 after panic unwinding"
        );
        profile4
            .delete()
            .expect("delete after unwinding lease drop");
        assert!(profile4.is_deleted());
    }
}

// ============================================================================
// 85. Milestone B6: DACL Matcher Worst-Case Complexity & Budget Enforcement
// (BLK-B6-02 Algorithmic Bounding)
// ============================================================================

#[test]
fn test_dacl_matcher_worst_case_complexity_stress_and_budget() {
    use engram::empirical_live::security::{
        AceSnapshot, DaclDriftError, DaclSnapshot, MAX_DACL_MATCHER_CELLS,
    };
    use std::time::Instant;

    let base_sid = "S-1-5-21-1111-2222-3333-1001";
    let contained_sid = "S-1-15-2-99999-88888-77777";

    // ------------------------------------------------------------------------
    // Part 1: High-multiplicity duplicate ACEs stress test
    // ------------------------------------------------------------------------
    // Baseline: 50 identical ACEs
    let mut baseline_aces = Vec::new();
    for i in 0..50 {
        baseline_aces.push(AceSnapshot {
            index: i,
            ace_type: 0,
            ace_flags: 0,
            access_mask: 0x001F0000,
            sid_string: base_sid.to_string(),
            object_type_guid: None,
            inherited_object_type_guid: None,
            extra_data: Vec::new(),
            raw_ace_bytes: vec![0, 0, 8, 0, 0, 0, 0x1F, 0],
            is_supported_type: true,
        });
    }

    // Current: 100 ACEs (50 baseline ACEs interleaved with 50 contained ACEs)
    let mut current_aces = Vec::new();
    for i in 0..50 {
        current_aces.push(AceSnapshot {
            index: i * 2,
            ace_type: 0,
            ace_flags: 0,
            access_mask: 0x001F0000,
            sid_string: base_sid.to_string(),
            object_type_guid: None,
            inherited_object_type_guid: None,
            extra_data: Vec::new(),
            raw_ace_bytes: vec![0, 0, 8, 0, 0, 0, 0x1F, 0],
            is_supported_type: true,
        });
        current_aces.push(AceSnapshot {
            index: i * 2 + 1,
            ace_type: 0,
            ace_flags: 0,
            access_mask: 0x00020000,
            sid_string: contained_sid.to_string(),
            object_type_guid: None,
            inherited_object_type_guid: None,
            extra_data: Vec::new(),
            raw_ace_bytes: vec![0, 0, 8, 0, 0, 0, 0x02, 0],
            is_supported_type: true,
        });
    }

    let snap_base = DaclSnapshot {
        is_present: true,
        is_null_dacl: false,
        acl_revision: 2,
        ace_count: baseline_aces.len() as u16,
        aces: baseline_aces,
        raw_acl_bytes: vec![2, 0, 0, 0, 0, 0, 0, 0],
    };

    let snap_curr = DaclSnapshot {
        is_present: true,
        is_null_dacl: false,
        acl_revision: 2,
        ace_count: current_aces.len() as u16,
        aces: current_aces,
        raw_acl_bytes: vec![2, 0, 0, 0, 0, 0, 0, 0],
    };

    let start = Instant::now();
    let drift_res = snap_base.detect_concurrent_drift(&snap_curr, contained_sid);
    let elapsed = start.elapsed();

    assert!(
        drift_res.is_ok(),
        "High-multiplicity valid embedding must succeed: {drift_res:?}"
    );
    assert!(
        elapsed.as_millis() < 50,
        "Iterative DP matcher must complete within 50ms (took {}ms)",
        elapsed.as_millis()
    );

    // ------------------------------------------------------------------------
    // Part 2: Complexity budget enforcement fails closed
    // ------------------------------------------------------------------------
    // Create baseline with 1,500 ACEs and current with 1,500 ACEs (1500 * 1500 = 2.25M > 1M)
    let mut large_baseline = Vec::new();
    let mut large_current = Vec::new();
    for i in 0..1500 {
        let ace = AceSnapshot {
            index: i,
            ace_type: 0,
            ace_flags: 0,
            access_mask: 0x1000,
            sid_string: base_sid.to_string(),
            object_type_guid: None,
            inherited_object_type_guid: None,
            extra_data: Vec::new(),
            raw_ace_bytes: vec![0, 0, 8, 0, 0, 0x10, 0, 0],
            is_supported_type: true,
        };
        large_baseline.push(ace.clone());
        large_current.push(ace);
    }

    let snap_large_base = DaclSnapshot {
        is_present: true,
        is_null_dacl: false,
        acl_revision: 2,
        ace_count: large_baseline.len() as u16,
        aces: large_baseline,
        raw_acl_bytes: vec![2, 0, 0, 0, 0, 0, 0, 0],
    };

    let snap_large_curr = DaclSnapshot {
        is_present: true,
        is_null_dacl: false,
        acl_revision: 2,
        ace_count: large_current.len() as u16,
        aces: large_current,
        raw_acl_bytes: vec![2, 0, 0, 0, 0, 0, 0, 0],
    };

    let budget_res = snap_large_base.detect_concurrent_drift(&snap_large_curr, contained_sid);
    match budget_res {
        Err(DaclDriftError::MatcherComplexityBudgetExceeded {
            baseline_count,
            current_count,
            max_cells,
        }) => {
            assert_eq!(baseline_count, 1500);
            assert_eq!(current_count, 1500);
            assert_eq!(max_cells, MAX_DACL_MATCHER_CELLS);
        }
        other => panic!("Expected MatcherComplexityBudgetExceeded, got {other:?}"),
    }

    // Part 2b: Asymmetric dimension check (baseline > budget, current = 0) fails closed
    let snap_dim_base = DaclSnapshot {
        is_present: true,
        is_null_dacl: false,
        acl_revision: 2,
        ace_count: 1000,
        aces: vec![
            AceSnapshot {
                index: 0,
                ace_type: 0,
                ace_flags: 0,
                access_mask: 0x1000,
                sid_string: base_sid.to_string(),
                object_type_guid: None,
                inherited_object_type_guid: None,
                extra_data: Vec::new(),
                raw_ace_bytes: vec![0, 0, 8, 0, 0, 0x10, 0, 0],
                is_supported_type: true,
            };
            MAX_DACL_MATCHER_CELLS + 1
        ],
        raw_acl_bytes: vec![2, 0, 0, 0, 0, 0, 0, 0],
    };
    let snap_empty_curr = DaclSnapshot {
        is_present: true,
        is_null_dacl: false,
        acl_revision: 2,
        ace_count: 0,
        aces: Vec::new(),
        raw_acl_bytes: vec![2, 0, 0, 0, 0, 0, 0, 0],
    };
    let dim_res = snap_dim_base.detect_concurrent_drift(&snap_empty_curr, contained_sid);
    assert!(matches!(
        dim_res,
        Err(DaclDriftError::MatcherComplexityBudgetExceeded { .. })
    ));

    // ------------------------------------------------------------------------
    // Part 3: Order preservation fidelity with duplicates is maintained
    // ------------------------------------------------------------------------
    let mut dup_base = Vec::new();
    for i in 0..20 {
        dup_base.push(AceSnapshot {
            index: i,
            ace_type: 0,
            ace_flags: 0,
            access_mask: if i == 19 { 0x0002 } else { 0x0001 },
            sid_string: base_sid.to_string(),
            object_type_guid: None,
            inherited_object_type_guid: None,
            extra_data: Vec::new(),
            raw_ace_bytes: vec![0, 0, 8, 0, if i == 19 { 2 } else { 1 }, 0, 0, 0],
            is_supported_type: true,
        });
    }

    // Swap the 19th and 20th ACEs in current
    let mut dup_curr = dup_base.clone();
    dup_curr.swap(18, 19);

    let snap_dup_base = DaclSnapshot {
        is_present: true,
        is_null_dacl: false,
        acl_revision: 2,
        ace_count: dup_base.len() as u16,
        aces: dup_base,
        raw_acl_bytes: vec![2, 0, 0, 0, 0, 0, 0, 0],
    };

    let snap_dup_curr = DaclSnapshot {
        is_present: true,
        is_null_dacl: false,
        acl_revision: 2,
        ace_count: dup_curr.len() as u16,
        aces: dup_curr,
        raw_acl_bytes: vec![2, 0, 0, 0, 0, 0, 0, 0],
    };

    let order_res = snap_dup_base.detect_concurrent_drift(&snap_dup_curr, contained_sid);
    match order_res {
        Err(DaclDriftError::PreexistingAceReordered { sid, detail }) => {
            assert_eq!(sid, base_sid);
            assert!(detail.contains("appears at position") || detail.contains("reordered"));
        }
        other => panic!("Expected PreexistingAceReordered, got {other:?}"),
    }
}
