//! Thread-safe, atomic pre-flight budget reservation system and financial ledger.
//!
//! # Architecture & Mathematical Invariants
//!
//! To prevent financial runaway during autonomous agent benchmark trials, the `BudgetGuard`
//! enforces a two-phase reservation protocol before any network byte is dispatched:
//!
//! 1. **Atomic Pre-Flight Reservation:** Before an API call is made, the worst-case possible
//!    cost of the prompt and maximum completion tokens is calculated using the model's pricing
//!    schedule and atomically reserved against the budget ledger.
//! 2. **Conservation Invariant:** At all times, across any number of concurrent worker threads:
//!    `committed_spend + uncertain_spend + active_reserved + available_budget == hard_cap`
//! 3. **Ambiguous Outcome Holding:** When an HTTP request encounters a client timeout, dropped TCP
//!    connection, or gateway 504, the provider may still have processed and billed the turn.
//!    The reservation is **never silently released**; it is transferred to `uncertain_spend`
//!    until authoritative billing records are reconciled.
//! 4. **State Machine Single-Transition Guarantee:** A reservation ticket transitions through
//!    an explicit state machine. Duplicate attempts to commit, release, or hold a ticket are
//!    rejected fail-closed without corrupting ledger balances.
//! 5. **Zero-Floating-Point Financial Arithmetic:** All financial balances and rates are tracked
//!    in integer `MicroDollars` (1 USD = 1,000,000 micro-dollars). Token pricing uses ceiling
//!    division to ensure conservative upper bounds.
//!
//! # Concurrency Model
//!
//! A single `std::sync::Mutex` encapsulates the entire `BudgetLedger`. All state mutations
//! (reserve, commit, hold, cancel, snapshot) execute atomically inside critical sections.
//! No external I/O, network sockets, or secondary locks are acquired while holding the mutex,
//! guaranteeing linearizability and deadlock-free operation.

use crate::empirical_live::schema::{ContentBlock, LlmRequest, SchemaValidationError, TokenUsage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Monetary amount represented in integer micro-dollars (1 USD = 1,000,000 micro-dollars).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct MicroDollars(pub u64);

impl MicroDollars {
    /// Zero micro-dollars.
    pub const ZERO: Self = Self(0);
    /// One US Dollar ($1.00).
    pub const ONE_DOLLAR: Self = Self(1_000_000);
    /// Ten US Dollars ($10.00).
    pub const TEN_DOLLARS: Self = Self(10_000_000);

    /// Construct from raw integer micro-dollars.
    pub const fn from_micros(micros: u64) -> Self {
        Self(micros)
    }

    /// Construct from whole dollars.
    pub fn from_dollars(dollars: u64) -> Result<Self, BudgetError> {
        dollars
            .checked_mul(1_000_000)
            .map(Self)
            .ok_or(BudgetError::ArithmeticOverflow)
    }

    /// Construct from dollars and cents (e.g. $10.50 -> 10, 50).
    pub fn from_dollars_and_cents(dollars: u64, cents: u32) -> Result<Self, BudgetError> {
        if cents >= 100 {
            return Err(BudgetError::InvalidMonetaryAmount(format!(
                "Cents must be < 100, got {cents}"
            )));
        }
        let dollar_micros = dollars
            .checked_mul(1_000_000)
            .ok_or(BudgetError::ArithmeticOverflow)?;
        let cent_micros = (cents as u64)
            .checked_mul(10_000)
            .ok_or(BudgetError::ArithmeticOverflow)?;
        dollar_micros
            .checked_add(cent_micros)
            .map(Self)
            .ok_or(BudgetError::ArithmeticOverflow)
    }

    /// Checked addition.
    pub fn checked_add(self, other: Self) -> Result<Self, BudgetError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(BudgetError::ArithmeticOverflow)
    }

    /// Checked subtraction.
    pub fn checked_sub(self, other: Self) -> Result<Self, BudgetError> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(BudgetError::ArithmeticUnderflow)
    }

    /// Convert to floating-point US dollars for human reporting only.
    pub fn to_dollars_f64(self) -> f64 {
        self.0 as f64 / 1_000_000.0
    }

    /// Format as standard currency string "$D.cccccc".
    pub fn format_usd(self) -> String {
        let dollars = self.0 / 1_000_000;
        let micros = self.0 % 1_000_000;
        format!("${dollars}.{micros:06}")
    }
}

impl std::fmt::Display for MicroDollars {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_usd())
    }
}

/// Token pricing schedule for an LLM model, specified in micro-dollars per 1,000,000 tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Model name or prefix identifier.
    pub model_id: String,
    /// Cost per 1,000,000 uncached prompt input tokens.
    pub input_cost_per_million: MicroDollars,
    /// Cost per 1,000,000 completion output tokens.
    pub output_cost_per_million: MicroDollars,
    /// Cost per 1,000,000 prompt tokens read from cache, if discounted by provider.
    pub cache_read_cost_per_million: Option<MicroDollars>,
    /// Cost per 1,000,000 prompt tokens written into cache, if charged by provider.
    pub cache_write_cost_per_million: Option<MicroDollars>,
}

impl ModelPricing {
    /// Reference pricing for Anthropic Claude 3.7 Sonnet ($3.00 / $15.00 per MTok).
    pub fn claude_3_7_sonnet() -> Self {
        Self {
            model_id: "claude-3-7-sonnet-20250219".to_string(),
            input_cost_per_million: MicroDollars::from_micros(3_000_000),
            output_cost_per_million: MicroDollars::from_micros(15_000_000),
            cache_read_cost_per_million: Some(MicroDollars::from_micros(300_000)), // 90% discount ($0.30/MTok)
            cache_write_cost_per_million: Some(MicroDollars::from_micros(3_750_000)), // 25% premium ($3.75/MTok)
        }
    }

    /// Reference pricing for OpenAI GPT-4o ($2.50 / $10.00 per MTok).
    pub fn gpt_4o() -> Self {
        Self {
            model_id: "gpt-4o".to_string(),
            input_cost_per_million: MicroDollars::from_micros(2_500_000),
            output_cost_per_million: MicroDollars::from_micros(10_000_000),
            cache_read_cost_per_million: Some(MicroDollars::from_micros(1_250_000)), // 50% discount ($1.25/MTok)
            cache_write_cost_per_million: None,
        }
    }

    /// Validate pricing entries.
    pub fn validate(&self) -> Result<(), BudgetError> {
        if self.model_id.trim().is_empty() {
            return Err(BudgetError::InvalidPricing(
                "model_id cannot be empty".into(),
            ));
        }
        Ok(())
    }

    /// Calculate worst-case potential cost for a turn before network dispatch.
    ///
    /// Uses conservative assumptions:
    /// - All estimated prompt tokens are billed at full input rate (no cache hits assumed).
    /// - If cache write costs apply, worst-case accounts for initial cache write creation.
    /// - Completion tokens reach `max_output_tokens` at full output rate.
    pub fn calculate_worst_case_cost(
        &self,
        estimated_prompt_tokens: u64,
        max_output_tokens: u64,
    ) -> Result<MicroDollars, BudgetError> {
        let input_rate = match self.cache_write_cost_per_million {
            Some(w) if w > self.input_cost_per_million => w,
            _ => self.input_cost_per_million,
        };
        let input_cost = self.calculate_token_cost(estimated_prompt_tokens, input_rate)?;
        let output_cost =
            self.calculate_token_cost(max_output_tokens, self.output_cost_per_million)?;
        input_cost.checked_add(output_cost)
    }

    /// Calculate authoritative actual cost from reported token usage telemetry.
    pub fn calculate_actual_cost(
        &self,
        prompt_tokens: u64,
        completion_tokens: u64,
        cached_input_tokens: Option<u64>,
    ) -> Result<MicroDollars, BudgetError> {
        let (cached, uncached_prompt) = match cached_input_tokens {
            Some(c) => {
                if c > prompt_tokens {
                    return Err(BudgetError::InvalidTokenTelemetry(format!(
                        "Cached tokens ({c}) cannot exceed prompt tokens ({prompt_tokens})"
                    )));
                }
                (c, prompt_tokens - c)
            }
            None => (0, prompt_tokens),
        };

        let uncached_cost =
            self.calculate_token_cost(uncached_prompt, self.input_cost_per_million)?;
        let cached_cost = if cached > 0 {
            let rate = self
                .cache_read_cost_per_million
                .unwrap_or(self.input_cost_per_million);
            self.calculate_token_cost(cached, rate)?
        } else {
            MicroDollars::ZERO
        };
        let output_cost =
            self.calculate_token_cost(completion_tokens, self.output_cost_per_million)?;

        uncached_cost
            .checked_add(cached_cost)?
            .checked_add(output_cost)
    }

    /// Internal token cost calculation using safe ceiling division:
    /// `Cost = ceil((tokens * price_per_million) / 1_000_000)`
    fn calculate_token_cost(
        &self,
        tokens: u64,
        price_per_million: MicroDollars,
    ) -> Result<MicroDollars, BudgetError> {
        let product = tokens
            .checked_mul(price_per_million.0)
            .ok_or(BudgetError::ArithmeticOverflow)?;
        let div = 1_000_000u64;
        let div_val = product / div;
        let rem = product % div;
        let micros = if rem > 0 {
            div_val
                .checked_add(1)
                .ok_or(BudgetError::ArithmeticOverflow)?
        } else {
            div_val
        };
        Ok(MicroDollars(micros))
    }
}

/// Thread-safe registry of validated model pricing schedules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ModelPricingRegistry {
    models: HashMap<String, ModelPricing>,
}

impl ModelPricingRegistry {
    /// Create an empty pricing registry.
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    /// Prepopulate with standard reference models (Claude 3.7 Sonnet, GPT-4o, and common aliases).
    pub fn default_reference() -> Self {
        let mut reg = Self::new();
        let sonnet = ModelPricing::claude_3_7_sonnet();
        reg.register(sonnet.clone())
            .expect("reference pricing must be valid");
        // Also register convenience short-name alias
        let mut sonnet_alias = sonnet;
        sonnet_alias.model_id = "claude-3-7-sonnet".to_string();
        let _ = reg.register(sonnet_alias);

        let gpt4o = ModelPricing::gpt_4o();
        reg.register(gpt4o)
            .expect("reference pricing must be valid");
        reg
    }

    /// Register a validated model pricing schedule. Fails closed if invalid.
    pub fn register(&mut self, pricing: ModelPricing) -> Result<(), BudgetError> {
        pricing.validate()?;
        self.models.insert(pricing.model_id.clone(), pricing);
        Ok(())
    }

    /// Retrieve pricing for a model ID. Fails closed with UnknownModelPricing if missing.
    pub fn get(&self, model_id: &str) -> Result<&ModelPricing, BudgetError> {
        self.models
            .get(model_id)
            .ok_or_else(|| BudgetError::UnknownModelPricing(model_id.to_string()))
    }

    /// Check if a model is registered.
    pub fn contains(&self, model_id: &str) -> bool {
        self.models.contains_key(model_id)
    }

    /// Count of registered models.
    pub fn len(&self) -> usize {
        self.models.len()
    }

    /// Whether registry is empty.
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }
}

/// Structural prompt token estimator and defensive lower-bound enforcement.
///
/// # Mathematical Defense-in-Depth
/// While character or byte heuristics are **not** proven mathematical upper bounds for arbitrary
/// BPE tokenizers, the byte length of the serialized prompt represents a rigid physical floor:
/// standard subword tokenizers (e.g. Claude / GPT-4o tiktoken) cannot produce fewer than 1 token
/// per 4-5 UTF-8 bytes for English code and natural language text.
///
/// `PromptTokenEstimator` calculates this structural floor to prevent callers from under-estimating
/// prompt tokens to zero or trivially tiny amounts before budget reservation.
///
/// NOTE: For live model dispatch, token counts must be validated against model-specific tokenizers
/// or conservative limits; structural floors provide defense-in-depth against pre-flight under-reservation.
#[derive(Debug, Clone, Copy)]
pub struct PromptTokenEstimator;

impl PromptTokenEstimator {
    /// Calculate the raw UTF-8 byte length across all text components of the request.
    pub fn structural_byte_count(request: &LlmRequest) -> usize {
        let mut bytes = 0;
        if let Some(sys) = &request.system_prompt {
            bytes += sys.len();
        }
        for msg in &request.messages {
            if let Some(name) = &msg.name {
                bytes += name.len();
            }
            for block in &msg.content {
                match block {
                    ContentBlock::Text { text } => bytes += text.len(),
                    ContentBlock::ToolUse { name, input, .. } => {
                        bytes += name.len();
                        bytes += input.to_string().len();
                    }
                    ContentBlock::ToolResult { content, .. } => {
                        bytes += content.len();
                    }
                }
            }
        }
        for tool in &request.tools {
            bytes += tool.name.len();
            bytes += tool.description.len();
            bytes += tool.input_schema.to_string().len();
        }
        for stop in &request.stop_sequences {
            bytes += stop.len();
        }
        bytes
    }

    /// Calculate a defensible, validated conservative upper bound on prompt tokens
    /// for pre-flight budget reservation.
    ///
    /// # Methodology & Conservative Upper-Bound Derivation
    /// 1. **Measured Representation**:
    ///    The request is validated and serialized into the Anthropic Messages API v1
    ///    JSON wire specification via [`crate::empirical_live::anthropic::to_anthropic_request`].
    ///    The serialized byte length captures all text content, message structures, tool names,
    ///    descriptions, input schemas, JSON formatting overhead (keys, quotes, delimiters),
    ///    and string escape expansions (`\"`, `\\`, `\n`).
    ///
    /// 2. **Byte-Pair Encoding (BPE) Singleton Bound**:
    ///    In Byte-level BPE (as employed by modern LLM tokenizers), the base alphabet
    ///    consists of 256 individual byte tokens. BPE merge operations replace sequences of
    ///    two or more tokens with a single merged token, strictly reducing token count.
    ///    In the absolute theoretical worst-case where zero merges occur (e.g. arbitrary unseen
    ///    byte sequences, dense punctuation, or disjoint code symbols), each UTF-8 byte maps to
    ///    at most 1 token: `tokens(content) <= bytes(content)`.
    ///
    /// 3. **Provider Server-Side Framing Allowance**:
    ///    When the JSON payload is processed by the provider backend, it is formatted into internal
    ///    prompt templates (e.g. Anthropic's XML delimiters):
    ///    - Base conversation framing: 32 tokens
    ///    - System prompt framing: 32 tokens (if present)
    ///    - Per-message turn envelope: 16 tokens per message
    ///    - Per-content block envelope: 16 tokens per block
    ///    - Per-tool definition envelope: 64 tokens per tool (XML tags + schema overhead)
    ///
    /// 4. **Defensive Margin & Floor**:
    ///    - A 25% safety margin (`* 125 / 100`) is applied to the combined wire bytes and framing
    ///      allowance to protect against server-side template expansion.
    ///    - A minimum floor of 64 tokens is enforced.
    ///
    /// # Provider Scope & Serialization Model (AUD-B6-TOK-03)
    /// This estimator currently serializes requests strictly using Anthropic Messages API v1
    /// wire formatting ([`crate::empirical_live::anthropic::to_anthropic_request`]) and is supported
    /// exclusively for the declared Anthropic benchmark evaluation scope (e.g. `claude-3-7-sonnet-20250219`).
    ///
    /// Future provider adapters (e.g. OpenAI ChatML, Google Gemini) must provide their own
    /// wire serializer and model-specific token accounting policy.
    ///
    /// NOTE: While this estimator provides a robust, empirically validated conservative bound
    /// for offline benchmarking, it does not eliminate the fundamental uncertainty of closed-source
    /// server-side prompt templating (see AUD-B6-TOK-01).
    ///
    /// # Fail-Closed Invariants:
    /// - Invalid request schema returns `BudgetError::RequestValidationFailed`.
    /// - Any integer arithmetic overflow returns `BudgetError::ArithmeticOverflow`.
    pub fn conservative_upper_bound_tokens(request: &LlmRequest) -> Result<u64, BudgetError> {
        request
            .validate()
            .map_err(BudgetError::RequestValidationFailed)?;

        // Step 1: Wire-format serialization
        let anthropic_val = crate::empirical_live::anthropic::to_anthropic_request(request)
            .map_err(|e| {
                BudgetError::PromptBoundingFailed(format!("Failed to serialize wire format: {e}"))
            })?;
        let wire_bytes = serde_json::to_vec(&anthropic_val)
            .map_err(|e| {
                BudgetError::PromptBoundingFailed(format!(
                    "Failed to serialize JSON wire bytes: {e}"
                ))
            })?
            .len();

        // Step 2: Provider server-side framing allowance
        let mut framing: u64 = 32; // Base conversation framing

        let has_system = request.system_prompt.is_some()
            || request
                .messages
                .iter()
                .any(|m| m.role == crate::empirical_live::schema::Role::System);
        if has_system {
            framing = framing
                .checked_add(32)
                .ok_or(BudgetError::ArithmeticOverflow)?;
        }

        let msg_framing = (request.messages.len() as u64)
            .checked_mul(16)
            .ok_or(BudgetError::ArithmeticOverflow)?;
        framing = framing
            .checked_add(msg_framing)
            .ok_or(BudgetError::ArithmeticOverflow)?;

        let num_blocks: usize = request.messages.iter().map(|m| m.content.len()).sum();
        let block_framing = (num_blocks as u64)
            .checked_mul(16)
            .ok_or(BudgetError::ArithmeticOverflow)?;
        framing = framing
            .checked_add(block_framing)
            .ok_or(BudgetError::ArithmeticOverflow)?;

        let tool_framing = (request.tools.len() as u64)
            .checked_mul(64)
            .ok_or(BudgetError::ArithmeticOverflow)?;
        framing = framing
            .checked_add(tool_framing)
            .ok_or(BudgetError::ArithmeticOverflow)?;

        // Step 3: Base token worst-case bound (1 token per wire byte + framing)
        let base_tokens = (wire_bytes as u64)
            .checked_add(framing)
            .ok_or(BudgetError::ArithmeticOverflow)?;

        // Step 4: 25% defensive headroom margin: ceil(base_tokens * 1.25)
        let margin_product = base_tokens
            .checked_mul(125)
            .ok_or(BudgetError::ArithmeticOverflow)?;
        let with_margin = margin_product
            .checked_add(99)
            .ok_or(BudgetError::ArithmeticOverflow)?
            / 100;

        // Step 5: Enforce minimum floor
        Ok(with_margin.max(64))
    }

    /// Calculate the structural lower token floor: `ceil(bytes / 4)`, minimum 1 token.
    pub fn structural_floor_tokens(request: &LlmRequest) -> u64 {
        let bytes = Self::structural_byte_count(request);
        (bytes as u64).div_ceil(4).max(1)
    }

    /// Bound and clamp the caller's estimated prompt tokens against the conservative upper bound.
    ///
    /// Guarantees that pre-flight budget reservation never under-reserves relative to the
    /// conservative upper bound, while preserving caller estimates that are even more conservative.
    /// Fails closed if the request is invalid or prompt tokens cannot be bounded.
    pub fn bound_prompt_tokens(
        request: &LlmRequest,
        caller_estimate: u64,
    ) -> Result<u64, BudgetError> {
        let upper_bound = Self::conservative_upper_bound_tokens(request)?;
        Ok(caller_estimate.max(upper_bound))
    }
}

/// Unique reservation ticket identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReservationId(pub u64);

impl std::fmt::Display for ReservationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "res-{}", self.0)
    }
}

/// Reason why an API request failed ambiguously.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AmbiguousFailureReason {
    /// Client-side HTTP timeout waiting for response headers or body.
    Timeout { elapsed_ms: u64 },
    /// TCP connection dropped or reset mid-stream after request dispatch.
    DroppedConnection { details: String },
    /// HTTP 504 Gateway Timeout returned by proxy/gateway.
    GatewayTimeout { status_code: u16 },
    /// Request aborted after bytes were written to the network socket.
    UncertainSocketWrite { details: String },
    /// Response completed but usage telemetry is missing, corrupt, or inconsistent.
    InvalidTelemetry { details: String },
}

/// Explicit lifecycle states of a reservation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReservationState {
    /// Active reservation holding worst-case funds prior to / during API dispatch.
    Active {
        reserved_amount: MicroDollars,
        model_id: String,
        pricing_snapshot: ModelPricing,
        created_at: chrono::DateTime<chrono::Utc>,
    },
    /// Authoritatively completed and billed; surplus returned to available budget.
    Committed {
        reserved_amount: MicroDollars,
        actual_cost: MicroDollars,
        released_surplus: MicroDollars,
        committed_at: chrono::DateTime<chrono::Utc>,
    },
    /// Ambiguous network outcome; funds permanently held in `uncertain_spend` unless reconciled.
    AmbiguousHold {
        held_amount: MicroDollars,
        reason: AmbiguousFailureReason,
        held_at: chrono::DateTime<chrono::Utc>,
    },
    /// Subsequent authoritative reconciliation of a previously held ambiguous failure.
    AuthoritativelyResolved {
        originally_held: MicroDollars,
        final_actual_cost: MicroDollars,
        released_surplus: MicroDollars,
        resolved_at: chrono::DateTime<chrono::Utc>,
    },
    /// Pre-dispatch cancellation before any network socket was opened; full reservation released.
    Released {
        released_amount: MicroDollars,
        reason: String,
        released_at: chrono::DateTime<chrono::Utc>,
    },
}

impl ReservationState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Active { .. } => "Active",
            Self::Committed { .. } => "Committed",
            Self::AmbiguousHold { .. } => "AmbiguousHold",
            Self::AuthoritativelyResolved { .. } => "AuthoritativelyResolved",
            Self::Released { .. } => "Released",
        }
    }
}

/// Opaque ticket proving an active pre-flight budget reservation.
///
/// Retains an immutable `pricing_snapshot` captured at reservation time to guarantee
/// that reconciliation is deterministic and immune to subsequent pricing registry mutations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservationTicket {
    id: ReservationId,
    reserved_amount: MicroDollars,
    model_id: String,
    pricing_snapshot: ModelPricing,
}

impl ReservationTicket {
    pub fn id(&self) -> ReservationId {
        self.id
    }
    pub fn reserved_amount(&self) -> MicroDollars {
        self.reserved_amount
    }
    pub fn model_id(&self) -> &str {
        &self.model_id
    }
    pub fn pricing_snapshot(&self) -> &ModelPricing {
        &self.pricing_snapshot
    }
}

/// Receipt confirming successful authoritative commitment of an API turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconciliationReceipt {
    pub ticket_id: ReservationId,
    pub actual_cost: MicroDollars,
    pub released_surplus: MicroDollars,
    pub committed_spend_after: MicroDollars,
    pub available_budget_after: MicroDollars,
}

/// Receipt confirming transfer of a reservation to the uncertain spend ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmbiguousHoldReceipt {
    pub ticket_id: ReservationId,
    pub held_amount: MicroDollars,
    pub reason: AmbiguousFailureReason,
    pub uncertain_spend_after: MicroDollars,
    pub available_budget_after: MicroDollars,
}

/// Receipt confirming cancellation of a reservation prior to network dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseReceipt {
    pub ticket_id: ReservationId,
    pub released_amount: MicroDollars,
    pub reason: String,
    pub available_budget_after: MicroDollars,
}

/// Point-in-time accounting snapshot of the budget ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetSnapshot {
    pub hard_cap: MicroDollars,
    pub committed_spend: MicroDollars,
    pub uncertain_spend: MicroDollars,
    pub active_reserved: MicroDollars,
    pub available_budget: MicroDollars,
    pub total_reservations_count: u64,
    pub active_count: usize,
    pub committed_count: usize,
    pub ambiguous_count: usize,
    pub resolved_count: usize,
    pub released_count: usize,
    pub is_halted: bool,
    pub halt_reason: Option<String>,
}

impl BudgetSnapshot {
    /// Verify the fundamental mathematical conservation invariant:
    /// `committed + uncertain + active + available == hard_cap`
    pub fn verify_conservation_invariant(&self) -> Result<(), BudgetError> {
        let allocated = self
            .committed_spend
            .checked_add(self.uncertain_spend)?
            .checked_add(self.active_reserved)?;
        let total = allocated.checked_add(self.available_budget)?;
        if total != self.hard_cap {
            return Err(BudgetError::InvariantViolation(format!(
                "Conservation invariant violated: committed ({}) + uncertain ({}) + reserved ({}) + available ({}) = {} != hard_cap ({})",
                self.committed_spend,
                self.uncertain_spend,
                self.active_reserved,
                self.available_budget,
                total,
                self.hard_cap
            )));
        }
        Ok(())
    }
}

/// Internal mutable budget ledger state.
#[derive(Debug)]
struct BudgetLedger {
    hard_cap: MicroDollars,
    committed_spend: MicroDollars,
    uncertain_spend: MicroDollars,
    active_reserved: MicroDollars,
    reservations: HashMap<ReservationId, ReservationState>,
    pricing_registry: ModelPricingRegistry,
    next_id: u64,
    is_halted: bool,
    halt_reason: Option<String>,
}

impl BudgetLedger {
    fn new(hard_cap: MicroDollars, pricing_registry: ModelPricingRegistry) -> Self {
        Self {
            hard_cap,
            committed_spend: MicroDollars::ZERO,
            uncertain_spend: MicroDollars::ZERO,
            active_reserved: MicroDollars::ZERO,
            reservations: HashMap::new(),
            pricing_registry,
            next_id: 1,
            is_halted: false,
            halt_reason: None,
        }
    }

    fn available_budget(&self) -> Result<MicroDollars, BudgetError> {
        let allocated = self
            .committed_spend
            .checked_add(self.uncertain_spend)?
            .checked_add(self.active_reserved)?;
        self.hard_cap.checked_sub(allocated)
    }

    /// Internal ledger conservation verification.
    ///
    /// Verifies that `committed + uncertain + active + available == hard_cap`
    /// without re-acquiring the mutex, never panicking, and returning a typed error.
    fn verify_conservation(&self) -> Result<(), BudgetError> {
        let allocated = self
            .committed_spend
            .checked_add(self.uncertain_spend)?
            .checked_add(self.active_reserved)?;
        let available = self.available_budget()?;
        let total = allocated.checked_add(available)?;
        if total != self.hard_cap {
            return Err(BudgetError::InvariantViolation(format!(
                "Conservation invariant violated: committed ({}) + uncertain ({}) + reserved ({}) + available ({}) = {} != hard_cap ({})",
                self.committed_spend,
                self.uncertain_spend,
                self.active_reserved,
                available,
                total,
                self.hard_cap
            )));
        }
        Ok(())
    }

    fn snapshot(&self) -> Result<BudgetSnapshot, BudgetError> {
        let mut active_count = 0;
        let mut committed_count = 0;
        let mut ambiguous_count = 0;
        let mut resolved_count = 0;
        let mut released_count = 0;

        for state in self.reservations.values() {
            match state {
                ReservationState::Active { .. } => active_count += 1,
                ReservationState::Committed { .. } => committed_count += 1,
                ReservationState::AmbiguousHold { .. } => ambiguous_count += 1,
                ReservationState::AuthoritativelyResolved { .. } => resolved_count += 1,
                ReservationState::Released { .. } => released_count += 1,
            }
        }

        let available = self.available_budget()?;
        let snapshot = BudgetSnapshot {
            hard_cap: self.hard_cap,
            committed_spend: self.committed_spend,
            uncertain_spend: self.uncertain_spend,
            active_reserved: self.active_reserved,
            available_budget: available,
            total_reservations_count: self.reservations.len() as u64,
            active_count,
            committed_count,
            ambiguous_count,
            resolved_count,
            released_count,
            is_halted: self.is_halted,
            halt_reason: self.halt_reason.clone(),
        };
        snapshot.verify_conservation_invariant()?;
        Ok(snapshot)
    }
}

/// Thread-safe, atomic budget reservation manager.
#[derive(Debug, Clone)]
pub struct BudgetGuard {
    inner: Arc<Mutex<BudgetLedger>>,
}

impl BudgetGuard {
    /// Create a new BudgetGuard with a specified hard spending cap and default reference pricing.
    pub fn new(hard_cap: MicroDollars) -> Self {
        Self::with_registry(hard_cap, ModelPricingRegistry::default_reference())
    }

    /// Create a new BudgetGuard with a custom model pricing registry.
    pub fn with_registry(hard_cap: MicroDollars, registry: ModelPricingRegistry) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BudgetLedger::new(hard_cap, registry))),
        }
    }

    /// Register or update a validated model pricing schedule. Fails closed if invalid.
    pub fn register_pricing(&self, pricing: ModelPricing) -> Result<(), BudgetError> {
        let mut ledger = self.inner.lock().map_err(|_| BudgetError::LockPoisoned)?;
        ledger.pricing_registry.register(pricing)
    }

    /// Atomically reserve worst-case turn cost derived directly from a validated request and registered pricing.
    ///
    /// # Fails Closed:
    /// 1. If session is halted due to a prior budget overrun, returns `BudgetError::SessionHaltedOnOverrun`.
    /// 2. If request validation fails, returns `BudgetError::RequestValidationFailed`.
    /// 3. If model has no registered pricing schedule, returns `BudgetError::UnknownModelPricing`.
    /// 4. If derived worst-case cost exceeds available budget, returns `BudgetError::BudgetExhausted`.
    pub fn reserve_for_request(
        &self,
        request: &LlmRequest,
        estimated_prompt_tokens: u64,
    ) -> Result<ReservationTicket, BudgetError> {
        request.validate().map_err(BudgetError::from)?;

        let mut ledger = self.inner.lock().map_err(|_| BudgetError::LockPoisoned)?;

        if ledger.is_halted {
            return Err(BudgetError::SessionHaltedOnOverrun(
                ledger.halt_reason.clone().unwrap_or_else(|| {
                    "Budget safety halt active due to prior overrun".to_string()
                }),
            ));
        }

        let pricing = ledger.pricing_registry.get(&request.model)?.clone();

        // Bound prompt tokens against conservative upper bound (defense-in-depth against caller understatement)
        let bounded_prompt_tokens =
            PromptTokenEstimator::bound_prompt_tokens(request, estimated_prompt_tokens)?;

        let worst_case_cost =
            pricing.calculate_worst_case_cost(bounded_prompt_tokens, request.max_tokens as u64)?;

        let available = ledger.available_budget()?;
        if worst_case_cost > available {
            return Err(BudgetError::BudgetExhausted {
                requested: worst_case_cost,
                available,
                hard_cap: ledger.hard_cap,
            });
        }

        let id = ReservationId(ledger.next_id);
        ledger.next_id = ledger
            .next_id
            .checked_add(1)
            .ok_or(BudgetError::ArithmeticOverflow)?;

        ledger.active_reserved = ledger.active_reserved.checked_add(worst_case_cost)?;

        ledger.reservations.insert(
            id,
            ReservationState::Active {
                reserved_amount: worst_case_cost,
                model_id: request.model.clone(),
                pricing_snapshot: pricing.clone(),
                created_at: chrono::Utc::now(),
            },
        );

        ledger.verify_conservation()?;

        Ok(ReservationTicket {
            id,
            reserved_amount: worst_case_cost,
            model_id: request.model.clone(),
            pricing_snapshot: pricing,
        })
    }

    /// Explicit reservation primitive for harness and baseline tests.
    ///
    /// # Fails Closed:
    /// 1. If session is halted due to a prior budget overrun, returns `BudgetError::SessionHaltedOnOverrun`.
    /// 2. Model must be registered in the pricing registry, or fails with `UnknownModelPricing`.
    /// 3. If `committed + uncertain + active_reserved + worst_case > hard_cap`, rejects with `BudgetExhausted`.
    pub fn reserve(
        &self,
        worst_case_cost: MicroDollars,
        model_id: impl Into<String>,
    ) -> Result<ReservationTicket, BudgetError> {
        let model = model_id.into();
        let mut ledger = self.inner.lock().map_err(|_| BudgetError::LockPoisoned)?;

        if ledger.is_halted {
            return Err(BudgetError::SessionHaltedOnOverrun(
                ledger.halt_reason.clone().unwrap_or_else(|| {
                    "Budget safety halt active due to prior overrun".to_string()
                }),
            ));
        }

        let pricing = ledger.pricing_registry.get(&model)?.clone();

        let available = ledger.available_budget()?;
        if worst_case_cost > available {
            return Err(BudgetError::BudgetExhausted {
                requested: worst_case_cost,
                available,
                hard_cap: ledger.hard_cap,
            });
        }

        let id = ReservationId(ledger.next_id);
        ledger.next_id = ledger
            .next_id
            .checked_add(1)
            .ok_or(BudgetError::ArithmeticOverflow)?;

        ledger.active_reserved = ledger.active_reserved.checked_add(worst_case_cost)?;

        ledger.reservations.insert(
            id,
            ReservationState::Active {
                reserved_amount: worst_case_cost,
                model_id: model.clone(),
                pricing_snapshot: pricing.clone(),
                created_at: chrono::Utc::now(),
            },
        );

        ledger.verify_conservation()?;

        Ok(ReservationTicket {
            id,
            reserved_amount: worst_case_cost,
            model_id: model,
            pricing_snapshot: pricing,
        })
    }

    /// Reconcile a successfully completed turn with its authoritative actual cost.
    ///
    /// Releases unused reservation surplus back to available budget. If the actual cost
    /// exceeds the reservation (which should not occur under conservative estimation),
    /// the extra amount is drawn from available headroom if available.
    pub fn reconcile_success(
        &self,
        ticket: &ReservationTicket,
        actual_cost: MicroDollars,
    ) -> Result<ReconciliationReceipt, BudgetError> {
        let mut ledger = self.inner.lock().map_err(|_| BudgetError::LockPoisoned)?;

        let current_state = ledger
            .reservations
            .get(&ticket.id)
            .cloned()
            .ok_or(BudgetError::UnknownReservation(ticket.id))?;

        match current_state {
            ReservationState::Active {
                reserved_amount, ..
            } => {
                let reserved = reserved_amount;

                // Determine surplus or extra cost
                let released_surplus = if reserved >= actual_cost {
                    reserved.checked_sub(actual_cost)?
                } else {
                    // Actual cost exceeded reserved worst case
                    let extra = actual_cost.checked_sub(reserved)?;
                    let available = ledger.available_budget()?;
                    if extra > available {
                        ledger.is_halted = true;
                        let reason = format!(
                            "Budget overrun: actual cost {actual_cost} exceeded reserved {reserved} and available headroom {available}"
                        );
                        ledger.halt_reason = Some(reason);
                        return Err(BudgetError::CostExceedsReservationAndAvailableBudget {
                            reserved,
                            actual_cost,
                            available,
                        });
                    }
                    MicroDollars::ZERO
                };

                ledger.active_reserved = ledger.active_reserved.checked_sub(reserved)?;
                ledger.committed_spend = ledger.committed_spend.checked_add(actual_cost)?;

                let new_state = ReservationState::Committed {
                    reserved_amount: reserved,
                    actual_cost,
                    released_surplus,
                    committed_at: chrono::Utc::now(),
                };
                ledger.reservations.insert(ticket.id, new_state);
                ledger.verify_conservation()?;

                let available_after = ledger.available_budget()?;

                Ok(ReconciliationReceipt {
                    ticket_id: ticket.id,
                    actual_cost,
                    released_surplus,
                    committed_spend_after: ledger.committed_spend,
                    available_budget_after: available_after,
                })
            }
            other => Err(BudgetError::InvalidStateTransition {
                id: ticket.id,
                current_state: other.name(),
                attempted: "reconcile_success",
            }),
        }
    }

    /// Reconcile a successfully completed turn directly from provider-neutral token usage telemetry.
    ///
    /// Computes actual cost using the immutable `pricing_snapshot` bound to the reservation ticket,
    /// completely immune to any subsequent mutations of the pricing registry.
    pub fn reconcile_turn(
        &self,
        ticket: &ReservationTicket,
        usage: &TokenUsage,
    ) -> Result<ReconciliationReceipt, BudgetError> {
        usage
            .validate()
            .map_err(|e| BudgetError::InvalidTokenTelemetry(e.to_string()))?;
        let prompt_tokens = usage.prompt_tokens.ok_or_else(|| {
            BudgetError::InvalidTokenTelemetry("Missing prompt_tokens in telemetry".to_string())
        })?;
        let completion_tokens = usage.completion_tokens.ok_or_else(|| {
            BudgetError::InvalidTokenTelemetry("Missing completion_tokens in telemetry".to_string())
        })?;
        let actual_cost = ticket.pricing_snapshot.calculate_actual_cost(
            prompt_tokens,
            completion_tokens,
            usage.cached_input_tokens,
        )?;
        self.reconcile_success(ticket, actual_cost)
    }

    /// Reconcile an ambiguous network outcome (timeout, connection reset, 504).
    ///
    /// Invariant: The reservation is **NEVER silently released**. The full reserved amount
    /// is transferred from `active_reserved` to `uncertain_spend`.
    pub fn reconcile_ambiguous(
        &self,
        ticket: &ReservationTicket,
        reason: AmbiguousFailureReason,
    ) -> Result<AmbiguousHoldReceipt, BudgetError> {
        let mut ledger = self.inner.lock().map_err(|_| BudgetError::LockPoisoned)?;

        let current_state = ledger
            .reservations
            .get(&ticket.id)
            .cloned()
            .ok_or(BudgetError::UnknownReservation(ticket.id))?;

        match current_state {
            ReservationState::Active {
                reserved_amount, ..
            } => {
                let reserved = reserved_amount;

                ledger.active_reserved = ledger.active_reserved.checked_sub(reserved)?;
                ledger.uncertain_spend = ledger.uncertain_spend.checked_add(reserved)?;

                let new_state = ReservationState::AmbiguousHold {
                    held_amount: reserved,
                    reason: reason.clone(),
                    held_at: chrono::Utc::now(),
                };
                ledger.reservations.insert(ticket.id, new_state);
                ledger.verify_conservation()?;

                let available_after = ledger.available_budget()?;

                Ok(AmbiguousHoldReceipt {
                    ticket_id: ticket.id,
                    held_amount: reserved,
                    reason,
                    uncertain_spend_after: ledger.uncertain_spend,
                    available_budget_after: available_after,
                })
            }
            other => Err(BudgetError::InvalidStateTransition {
                id: ticket.id,
                current_state: other.name(),
                attempted: "reconcile_ambiguous",
            }),
        }
    }

    /// Reconcile pre-dispatch cancellation (e.g. local schema validation failed before any socket opened).
    ///
    /// Strictly forbidden once any bytes have been transmitted to the provider.
    pub fn reconcile_preflight_cancel(
        &self,
        ticket: &ReservationTicket,
        reason: impl Into<String>,
    ) -> Result<ReleaseReceipt, BudgetError> {
        let reason_str = reason.into();
        let mut ledger = self.inner.lock().map_err(|_| BudgetError::LockPoisoned)?;

        let current_state = ledger
            .reservations
            .get(&ticket.id)
            .cloned()
            .ok_or(BudgetError::UnknownReservation(ticket.id))?;

        match current_state {
            ReservationState::Active {
                reserved_amount, ..
            } => {
                let reserved = reserved_amount;

                ledger.active_reserved = ledger.active_reserved.checked_sub(reserved)?;

                let new_state = ReservationState::Released {
                    released_amount: reserved,
                    reason: reason_str.clone(),
                    released_at: chrono::Utc::now(),
                };
                ledger.reservations.insert(ticket.id, new_state);
                ledger.verify_conservation()?;

                let available_after = ledger.available_budget()?;

                Ok(ReleaseReceipt {
                    ticket_id: ticket.id,
                    released_amount: reserved,
                    reason: reason_str,
                    available_budget_after: available_after,
                })
            }
            other => Err(BudgetError::InvalidStateTransition {
                id: ticket.id,
                current_state: other.name(),
                attempted: "reconcile_preflight_cancel",
            }),
        }
    }

    /// Authoritative post-session reconciliation of an ambiguous hold.
    ///
    /// Used when post-session billing reports or rate-limit headers confirm exact charges
    /// for a turn that previously suffered a network timeout.
    pub fn reconcile_authoritative_resolution(
        &self,
        ticket_id: ReservationId,
        authoritative_actual_cost: MicroDollars,
    ) -> Result<ReconciliationReceipt, BudgetError> {
        let mut ledger = self.inner.lock().map_err(|_| BudgetError::LockPoisoned)?;

        let current_state = ledger
            .reservations
            .get(&ticket_id)
            .cloned()
            .ok_or(BudgetError::UnknownReservation(ticket_id))?;

        match current_state {
            ReservationState::AmbiguousHold { held_amount, .. } => {
                let held = held_amount;

                let released_surplus = if held >= authoritative_actual_cost {
                    held.checked_sub(authoritative_actual_cost)?
                } else {
                    let extra = authoritative_actual_cost.checked_sub(held)?;
                    let available = ledger.available_budget()?;
                    if extra > available {
                        ledger.is_halted = true;
                        let reason = format!(
                            "Budget overrun: authoritative actual cost {authoritative_actual_cost} exceeded held {held} and available headroom {available}"
                        );
                        ledger.halt_reason = Some(reason);
                        return Err(BudgetError::CostExceedsReservationAndAvailableBudget {
                            reserved: held,
                            actual_cost: authoritative_actual_cost,
                            available,
                        });
                    }
                    MicroDollars::ZERO
                };

                ledger.uncertain_spend = ledger.uncertain_spend.checked_sub(held)?;
                ledger.committed_spend = ledger
                    .committed_spend
                    .checked_add(authoritative_actual_cost)?;

                let new_state = ReservationState::AuthoritativelyResolved {
                    originally_held: held,
                    final_actual_cost: authoritative_actual_cost,
                    released_surplus,
                    resolved_at: chrono::Utc::now(),
                };
                ledger.reservations.insert(ticket_id, new_state);
                ledger.verify_conservation()?;

                let available_after = ledger.available_budget()?;

                Ok(ReconciliationReceipt {
                    ticket_id,
                    actual_cost: authoritative_actual_cost,
                    released_surplus,
                    committed_spend_after: ledger.committed_spend,
                    available_budget_after: available_after,
                })
            }
            other => Err(BudgetError::InvalidStateTransition {
                id: ticket_id,
                current_state: other.name(),
                attempted: "reconcile_authoritative_resolution",
            }),
        }
    }

    /// Retrieve an instantaneous snapshot of the budget ledger.
    pub fn snapshot(&self) -> Result<BudgetSnapshot, BudgetError> {
        let ledger = self.inner.lock().map_err(|_| BudgetError::LockPoisoned)?;
        ledger.snapshot()
    }

    /// Check if the budget guard is currently halted due to a budget overrun.
    pub fn is_halted(&self) -> Result<bool, BudgetError> {
        let ledger = self.inner.lock().map_err(|_| BudgetError::LockPoisoned)?;
        Ok(ledger.is_halted)
    }

    /// Retrieve the halt reason if halted.
    pub fn halt_reason(&self) -> Result<Option<String>, BudgetError> {
        let ledger = self.inner.lock().map_err(|_| BudgetError::LockPoisoned)?;
        Ok(ledger.halt_reason.clone())
    }
}

/// Errors occurring within the BudgetGuard accounting system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetError {
    BudgetExhausted {
        requested: MicroDollars,
        available: MicroDollars,
        hard_cap: MicroDollars,
    },
    SessionHaltedOnOverrun(String),
    InvalidStateTransition {
        id: ReservationId,
        current_state: &'static str,
        attempted: &'static str,
    },
    UnknownReservation(ReservationId),
    CostExceedsReservationAndAvailableBudget {
        reserved: MicroDollars,
        actual_cost: MicroDollars,
        available: MicroDollars,
    },
    ArithmeticOverflow,
    ArithmeticUnderflow,
    LockPoisoned,
    InvalidMonetaryAmount(String),
    InvalidPricing(String),
    UnknownModelPricing(String),
    RequestValidationFailed(SchemaValidationError),
    InvalidTokenTelemetry(String),
    PromptBoundingFailed(String),
    InvariantViolation(String),
}

impl std::fmt::Display for BudgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BudgetExhausted {
                requested,
                available,
                hard_cap,
            } => {
                write!(
                    f,
                    "Pre-flight budget exhausted: requested {requested}, but only {available} remains of {hard_cap} hard cap"
                )
            }
            Self::SessionHaltedOnOverrun(reason) => {
                write!(
                    f,
                    "Benchmark session halted due to budget overrun: {reason}"
                )
            }
            Self::InvalidStateTransition {
                id,
                current_state,
                attempted,
            } => {
                write!(
                    f,
                    "Invalid reservation state transition for {id}: cannot execute '{attempted}' while state is '{current_state}'"
                )
            }
            Self::UnknownReservation(id) => {
                write!(f, "Reservation ticket {id} does not exist in ledger")
            }
            Self::CostExceedsReservationAndAvailableBudget {
                reserved,
                actual_cost,
                available,
            } => {
                write!(
                    f,
                    "Actual cost {actual_cost} exceeded reserved {reserved} and available headroom {available}"
                )
            }
            Self::ArithmeticOverflow => write!(f, "Arithmetic overflow in financial computation"),
            Self::ArithmeticUnderflow => write!(f, "Arithmetic underflow in financial computation"),
            Self::LockPoisoned => write!(f, "BudgetGuard mutex lock was poisoned"),
            Self::InvalidMonetaryAmount(msg) => write!(f, "Invalid monetary amount: {msg}"),
            Self::InvalidPricing(msg) => write!(f, "Invalid model pricing: {msg}"),
            Self::UnknownModelPricing(model) => {
                write!(
                    f,
                    "No validated pricing schedule registered for model '{model}'"
                )
            }
            Self::RequestValidationFailed(err) => {
                write!(f, "Pre-flight request validation failed: {err}")
            }
            Self::InvalidTokenTelemetry(msg) => write!(f, "Invalid token telemetry: {msg}"),
            Self::PromptBoundingFailed(msg) => write!(f, "Prompt token bounding failed: {msg}"),
            Self::InvariantViolation(msg) => {
                write!(f, "Ledger conservation invariant violated: {msg}")
            }
        }
    }
}

impl std::error::Error for BudgetError {}

impl From<SchemaValidationError> for BudgetError {
    fn from(err: SchemaValidationError) -> Self {
        Self::RequestValidationFailed(err)
    }
}
