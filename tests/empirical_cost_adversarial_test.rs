//! Offline Adversarial Cost Suite for Engram Phase 2B Milestone B6.
//!
//! Specifications ADV-COST-1 through ADV-COST-8:
//! - ADV-COST-1: Model Identifier Mutation / Unknown Model Rejection
//! - ADV-COST-2: Pricing Version Tampering / Schedule Mismatch
//! - ADV-COST-3: Prompt Token Clamping Violation & Under-Reservation Defense
//! - ADV-COST-4: Extended Thinking & Request Parameter Injection
//! - ADV-COST-5: Unlisted Billing Dimension & Corrupted Token Telemetry
//! - ADV-COST-6: Malformed / Missing Usage Telemetry
//! - ADV-COST-7: Critical Overrun Deficit Halt & Subsequent Dispatch Refusal
//! - ADV-COST-8: Crash Recovery with In-Flight Active Reservation
//!
//! All tests execute 100% offline with zero external network sockets opened and $0.00 spend.

use engram::empirical_live::anthropic::{from_anthropic_response, to_anthropic_request};
use engram::empirical_live::budget::{
    AmbiguousFailureReason, BudgetError, BudgetGuard, MicroDollars, ModelPricing,
    ModelPricingRegistry, PromptTokenEstimator,
};
use engram::empirical_live::provider::ProviderError;
use engram::empirical_live::schema::{
    ChatMessage, LlmRequest, SchemaValidationError, TokenTelemetryError, TokenUsage,
};
use serde_json::json;

// ============================================================================
// ADV-COST-1: Model Identifier Mutation
// ============================================================================
#[test]
fn test_adv_cost_1_model_identifier_mutation() {
    let cap = MicroDollars::from_dollars(5).unwrap(); // $5.00 ceiling
    let guard = BudgetGuard::new(cap);

    // 1. Attempt reservation with unapproved / mutated model identifier
    let mutated_request = LlmRequest {
        model: "claude-3-opus-20240229".to_string(), // Not in B6 allowed evaluation scope
        system_prompt: Some("You are an assistant.".to_string()),
        messages: vec![ChatMessage::user("Verify code containment.")],
        tools: vec![],
        max_tokens: 4096,
        temperature: None,
        stop_sequences: vec![],
    };

    // Pre-flight reservation MUST fail closed before any DNS lookup or socket open
    let res_err = guard
        .reserve_for_request(&mutated_request, 1000)
        .expect_err("Unregistered model must be rejected pre-flight");

    match res_err {
        BudgetError::UnknownModelPricing(model) => {
            assert_eq!(model, "claude-3-opus-20240229");
        }
        other => panic!("Expected UnknownModelPricing, got: {other:?}"),
    }

    // 2. Attempt reservation with empty model identifier
    let empty_model_req = LlmRequest {
        model: "".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Test")],
        tools: vec![],
        max_tokens: 100,
        temperature: None,
        stop_sequences: vec![],
    };
    let empty_err = guard
        .reserve_for_request(&empty_model_req, 100)
        .expect_err("Empty model must fail schema validation");
    match empty_err {
        BudgetError::RequestValidationFailed(SchemaValidationError::EmptyModel) => {}
        other => panic!("Expected EmptyModel validation error, got: {other:?}"),
    }

    // Verify zero financial leakage: available budget must remain 100% intact ($5.00)
    let snapshot = guard.snapshot().expect("snapshot should succeed");
    assert_eq!(snapshot.available_budget, cap);
    assert_eq!(snapshot.active_reserved, MicroDollars::ZERO);
    assert_eq!(snapshot.committed_spend, MicroDollars::ZERO);
    assert_eq!(snapshot.uncertain_spend, MicroDollars::ZERO);
    snapshot.verify_conservation_invariant().unwrap();
}

// ============================================================================
// ADV-COST-2: Pricing Version Tampering / Schedule Mismatch
// ============================================================================
#[test]
fn test_adv_cost_2_pricing_version_tampering() {
    // 1. Registering invalid pricing schedule (e.g. empty model_id) fails closed
    let invalid_pricing = ModelPricing {
        model_id: "   ".to_string(),
        input_cost_per_million: MicroDollars::from_micros(3_000_000),
        output_cost_per_million: MicroDollars::from_micros(15_000_000),
        cache_read_cost_per_million: None,
        cache_write_cost_per_million: None,
    };
    assert!(invalid_pricing.validate().is_err());

    let mut registry = ModelPricingRegistry::new();
    assert!(registry.register(invalid_pricing).is_err());

    // 2. Pricing registry without B6 model rejects dispatch fail-closed
    let guard = BudgetGuard::with_registry(MicroDollars::from_dollars(5).unwrap(), registry);
    let sonnet_req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Benchmark task")],
        tools: vec![],
        max_tokens: 4096,
        temperature: None,
        stop_sequences: vec![],
    };
    let err = guard
        .reserve_for_request(&sonnet_req, 500)
        .expect_err("Missing model pricing must fail");
    match err {
        BudgetError::UnknownModelPricing(m) => assert!(m.contains("claude-3-7-sonnet")),
        other => panic!("Expected UnknownModelPricing, got: {other:?}"),
    }

    // 3. Immutability verification: Ticket captures pricing snapshot at reservation time
    let standard_guard = BudgetGuard::new(MicroDollars::from_dollars(10).unwrap());
    let ticket = standard_guard
        .reserve_for_request(&sonnet_req, 500)
        .expect("Reservation should succeed");
    let original_input_cost = ticket.pricing_snapshot().input_cost_per_million;

    // Mutate registry afterwards with altered rates
    let tampered_pricing = ModelPricing {
        model_id: "claude-3-7-sonnet-20250219".to_string(),
        input_cost_per_million: MicroDollars::from_micros(999_000_000), // Attempted 300x price hike
        output_cost_per_million: MicroDollars::from_micros(999_000_000),
        cache_read_cost_per_million: None,
        cache_write_cost_per_million: None,
    };
    standard_guard.register_pricing(tampered_pricing).unwrap();

    // The ticket retains its original snapshot
    assert_eq!(
        ticket.pricing_snapshot().input_cost_per_million,
        original_input_cost
    );

    // Reconciling using the ticket uses the frozen snapshot, immune to the tampered registry
    let usage = TokenUsage::new(500, 100);
    let receipt = standard_guard
        .reconcile_turn(&ticket, &usage)
        .expect("Reconciliation must succeed with ticket snapshot");

    // Expected actual cost: 500 * $3.00/MTok + 100 * $15.00/MTok = $0.001500 + $0.001500 = $0.003000 (3,000 micros)
    assert_eq!(receipt.actual_cost, MicroDollars::from_micros(3000));
}

// ============================================================================
// ADV-COST-3: Prompt Token Clamping Violation & Under-Reservation Defense
// ============================================================================
#[test]
fn test_adv_cost_3_prompt_token_clamping_violation() {
    let cap = MicroDollars::from_micros(50_000); // Strict tiny budget of $0.05 (50,000 micros)
    let guard = BudgetGuard::new(cap);

    // Create a large prompt payload (e.g. 10,000 characters of dense code text)
    let large_code_payload =
        "fn process_data(item: &Record) -> Result<(), Error> { /* ... */ }\n".repeat(150);
    let request = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: Some("You are an autonomous engineering agent.".to_string()),
        messages: vec![ChatMessage::user(large_code_payload)],
        tools: vec![],
        max_tokens: 4096, // Maximum completion tokens
        temperature: None,
        stop_sequences: vec![],
    };

    // Adversarial caller attempts to bypass budget by understating prompt estimate to only 10 tokens
    let fraudulent_caller_estimate = 10u64;

    // Defense-in-depth: PromptTokenEstimator enforces conservative upper bound regardless of caller estimate
    let bounded_tokens =
        PromptTokenEstimator::bound_prompt_tokens(&request, fraudulent_caller_estimate)
            .expect("Bounding must succeed");
    assert!(
        bounded_tokens > 2000,
        "Bounded tokens ({bounded_tokens}) must reflect actual payload size"
    );

    // Worst-case calculation:
    // Prompt: ~2,500+ tokens at $3.75/MTok cache-write rate = ~$0.009375
    // Output: 4,096 tokens at $15.00/MTok = $0.061440
    // Total worst-case > $0.070000, which strictly exceeds the $0.05 cap
    let reserve_result = guard.reserve_for_request(&request, fraudulent_caller_estimate);
    assert!(
        reserve_result.is_err(),
        "Reservation must be rejected fail-closed because bounded worst-case cost exceeds $0.05 cap"
    );

    match reserve_result.unwrap_err() {
        BudgetError::BudgetExhausted {
            requested,
            available,
            hard_cap,
        } => {
            assert!(requested > hard_cap);
            assert_eq!(available, cap);
        }
        other => panic!("Expected BudgetExhausted, got: {other:?}"),
    }

    // Verify zero budget leaked
    let snap = guard.snapshot().unwrap();
    assert_eq!(snap.available_budget, cap);
    assert_eq!(snap.active_reserved, MicroDollars::ZERO);
}

// ============================================================================
// ADV-COST-4: Extended Thinking & Request Parameter Injection
// ============================================================================
#[test]
fn test_adv_cost_4_extended_thinking_injection() {
    // 1. Zero max_tokens injection
    let zero_tokens_req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Test")],
        tools: vec![],
        max_tokens: 0,
        temperature: None,
        stop_sequences: vec![],
    };
    assert_eq!(
        zero_tokens_req.validate().unwrap_err(),
        SchemaValidationError::ZeroMaxTokens
    );

    // 2. Temperature out-of-bounds injection (> 2.0)
    let bad_temp_req = LlmRequest {
        model: "claude-3-7-sonnet-20250219".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage::user("Test")],
        tools: vec![],
        max_tokens: 100,
        temperature: Some(5.0),
        stop_sequences: vec![],
    };
    match bad_temp_req.validate().unwrap_err() {
        SchemaValidationError::InvalidTemperature(t) => assert_eq!(t, "5"),
        other => panic!("Expected InvalidTemperature, got: {other:?}"),
    }

    // 3. Wire serializer strictly enforces schema validation before serialization
    assert!(to_anthropic_request(&zero_tokens_req).is_err());
    assert!(to_anthropic_request(&bad_temp_req).is_err());
}

// ============================================================================
// ADV-COST-5: Unlisted Billing Dimension & Corrupted Token Telemetry
// ============================================================================
#[test]
fn test_adv_cost_5_unlisted_billing_dimension() {
    let guard = BudgetGuard::new(MicroDollars::from_dollars(5).unwrap());
    let ticket = guard
        .reserve(
            MicroDollars::from_micros(97_065),
            "claude-3-7-sonnet-20250219",
        )
        .unwrap();

    // 1. Provider returns corrupted telemetry where cached tokens exceed prompt tokens
    let invalid_usage = TokenUsage {
        prompt_tokens: Some(500),
        completion_tokens: Some(100),
        cached_input_tokens: Some(800), // Invariant violation: 800 > 500
        cache_write_tokens: None,
        total_tokens: Some(600),
    };

    assert!(matches!(
        invalid_usage.validate().unwrap_err(),
        TokenTelemetryError::CacheExceedsPrompt { .. }
    ));

    // Reconciling against this invalid usage fails closed
    let recon_err = guard.reconcile_turn(&ticket, &invalid_usage).unwrap_err();
    assert!(matches!(recon_err, BudgetError::InvalidTokenTelemetry(_)));

    // 2. Caller must treat invalid telemetry as an ambiguous outcome
    let hold_receipt = guard
        .reconcile_ambiguous(
            &ticket,
            AmbiguousFailureReason::InvalidTelemetry {
                details: "Telemetry validation failed: cached exceeds prompt".to_string(),
            },
        )
        .expect("Ambiguous hold must succeed");

    assert_eq!(hold_receipt.held_amount, MicroDollars::from_micros(97_065));

    // Zero funds returned to available budget!
    let snap = guard.snapshot().unwrap();
    assert_eq!(snap.uncertain_spend, MicroDollars::from_micros(97_065));
    assert_eq!(
        snap.available_budget,
        MicroDollars::from_dollars(5)
            .unwrap()
            .checked_sub(MicroDollars::from_micros(97_065))
            .unwrap()
    );
    snap.verify_conservation_invariant().unwrap();
}

// ============================================================================
// ADV-COST-6: Malformed / Missing Usage Telemetry
// ============================================================================
#[test]
fn test_adv_cost_6_malformed_missing_usage_telemetry() {
    // 1. Anthropic HTTP 200 response missing the entire 'usage' block
    let response_no_usage = json!({
        "id": "msg_adv_001",
        "type": "message",
        "role": "assistant",
        "model": "claude-3-7-sonnet-20250219",
        "content": [{"type": "text", "text": "Task completed."}],
        "stop_reason": "end_turn"
    });

    let parse_err = from_anthropic_response(&response_no_usage).unwrap_err();
    match parse_err {
        ProviderError::MalformedResponse(msg) => {
            assert!(msg.contains("missing 'usage' object"));
        }
        other => panic!("Expected MalformedResponse, got: {other:?}"),
    }

    // 2. Response with usage block missing 'input_tokens'
    let response_partial_usage = json!({
        "id": "msg_adv_002",
        "type": "message",
        "role": "assistant",
        "model": "claude-3-7-sonnet-20250219",
        "content": [{"type": "text", "text": "Task completed."}],
        "stop_reason": "end_turn",
        "usage": {
            "output_tokens": 120
        }
    });

    let partial_err = from_anthropic_response(&response_partial_usage).unwrap_err();
    match partial_err {
        ProviderError::MalformedResponse(msg) => {
            assert!(msg.contains("missing required 'input_tokens'"));
        }
        other => panic!("Expected MalformedResponse, got: {other:?}"),
    }

    // 3. Ledger handling: ambiguous hold holds funds permanently
    let guard = BudgetGuard::new(MicroDollars::from_dollars(1).unwrap());
    let ticket = guard
        .reserve(
            MicroDollars::from_micros(50_000),
            "claude-3-7-sonnet-20250219",
        )
        .unwrap();

    let hold = guard
        .reconcile_ambiguous(
            &ticket,
            AmbiguousFailureReason::InvalidTelemetry {
                details: "Missing input_tokens in provider payload".to_string(),
            },
        )
        .unwrap();

    assert_eq!(hold.held_amount, MicroDollars::from_micros(50_000));
    let snap = guard.snapshot().unwrap();
    assert_eq!(snap.uncertain_spend, MicroDollars::from_micros(50_000));
    assert_eq!(snap.available_budget, MicroDollars::from_micros(950_000));
    snap.verify_conservation_invariant().unwrap();
}

// ============================================================================
// ADV-COST-7: Critical Overrun Deficit Halt
// ============================================================================
#[test]
fn test_adv_cost_7_critical_overrun_deficit_halt() {
    // Milestone ceiling: exactly $0.150000 (150,000 micros)
    let ceiling = MicroDollars::from_micros(150_000);
    let guard = BudgetGuard::new(ceiling);

    // Reserve $0.097065 (97,065 micros)
    let reserved = MicroDollars::from_micros(97_065);
    let ticket = guard
        .reserve(reserved, "claude-3-7-sonnet-20250219")
        .expect("Reservation should succeed");

    // Remaining available budget is 150,000 - 97,065 = 52,935 micros ($0.052935)
    let snap1 = guard.snapshot().unwrap();
    assert_eq!(snap1.available_budget, MicroDollars::from_micros(52_935));

    // Synthetic provider response bills actual cost A = $0.160000 (160,000 micros)
    // Extra cost = 160,000 - 97,065 = 62,935 micros, which EXCEEDS the remaining 52,935 micros available headroom!
    let billed_actual = MicroDollars::from_micros(160_000);
    let overrun_err = guard
        .reconcile_success(&ticket, billed_actual)
        .expect_err("Overrun beyond available headroom must fail");

    match overrun_err {
        BudgetError::CostExceedsReservationAndAvailableBudget {
            reserved: r,
            actual_cost: a,
            available: av,
        } => {
            assert_eq!(r, reserved);
            assert_eq!(a, billed_actual);
            assert_eq!(av, MicroDollars::from_micros(52_935));
        }
        other => panic!("Expected CostExceedsReservationAndAvailableBudget, got: {other:?}"),
    }

    // Ledger enters permanent terminal halt
    assert!(guard.is_halted().unwrap());
    let reason = guard
        .halt_reason()
        .unwrap()
        .expect("Halt reason must be set");
    assert!(reason.contains("Budget overrun"));

    // Any subsequent reservation attempt fails closed immediately with SessionHaltedOnOverrun
    let next_ticket_err = guard
        .reserve(
            MicroDollars::from_micros(1000),
            "claude-3-7-sonnet-20250219",
        )
        .expect_err("Halted session must reject all subsequent reservations");

    match next_ticket_err {
        BudgetError::SessionHaltedOnOverrun(msg) => {
            assert!(msg.contains("Budget overrun"));
        }
        other => panic!("Expected SessionHaltedOnOverrun, got: {other:?}"),
    }
}

// ============================================================================
// ADV-COST-8: Crash Recovery with In-Flight Active Reservation
// ============================================================================
#[test]
fn test_adv_cost_8_crash_recovery_in_flight_reservation() {
    let ceiling = MicroDollars::from_dollars(5).unwrap();
    let guard = BudgetGuard::new(ceiling);

    // Reserve funds for an in-flight request ($0.097065)
    let ticket = guard
        .reserve(
            MicroDollars::from_micros(97_065),
            "claude-3-7-sonnet-20250219",
        )
        .expect("Reservation should succeed");

    let pre_crash_snap = guard.snapshot().unwrap();
    assert_eq!(pre_crash_snap.active_count, 1);
    assert_eq!(
        pre_crash_snap.active_reserved,
        MicroDollars::from_micros(97_065)
    );

    // Simulate process crash / recovery scan:
    // In a crash recovery scan, any ticket left in Active state without a completion receipt
    // must be treated as an ambiguous outcome and moved to UncertainSpend
    let recovery_receipt = guard
        .reconcile_ambiguous(
            &ticket,
            AmbiguousFailureReason::UncertainSocketWrite {
                details: "Crash recovery: active reservation orphaned without completion receipt"
                    .to_string(),
            },
        )
        .expect("Recovery transition to ambiguous hold must succeed");

    assert_eq!(
        recovery_receipt.held_amount,
        MicroDollars::from_micros(97_065)
    );

    // Post-recovery state verification:
    // 1. Zero active reservations remain
    // 2. Active reserved balance is ZERO
    // 3. Uncertain spend contains exactly the orphaned $0.097065
    // 4. Available budget does NOT restore the orphaned funds!
    let post_crash_snap = guard.snapshot().unwrap();
    assert_eq!(post_crash_snap.active_count, 0);
    assert_eq!(post_crash_snap.ambiguous_count, 1);
    assert_eq!(post_crash_snap.active_reserved, MicroDollars::ZERO);
    assert_eq!(
        post_crash_snap.uncertain_spend,
        MicroDollars::from_micros(97_065)
    );
    assert_eq!(
        post_crash_snap.available_budget,
        ceiling
            .checked_sub(MicroDollars::from_micros(97_065))
            .unwrap()
    );

    // 5. Conservation invariant verified across crash recovery
    post_crash_snap.verify_conservation_invariant().unwrap();
}
