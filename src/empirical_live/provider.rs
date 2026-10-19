//! Provider-neutral LLM interface, secret redaction, and guarded execution pipeline.
//!
//! # Architectural Invariants
//! 1. **Zero Credential Leakage:** Sensitive secrets (API keys, authorization tokens)
//!    are wrapped in [`RedactedSecret`], which replaces all plaintext bytes with `"[REDACTED]"`
//!    in both `Debug` and `Display` formatting.
//! 2. **Typed Error Hierarchy:** All failure modes (transport, authentication, rate limit,
//!    malformation, budget exhaustion, overrun safety halt) are explicitly distinguished
//!    via [`ProviderError`].
//! 3. **Single Guarded Dispatch Boundary:** Autonomous agents and benchmark harnesses
//!    cannot invoke [`LlmProvider::execute_turn`] directly; all dispatches must route
//!    through [`GuardedProvider::execute_guarded`], guaranteeing atomic pre-flight budget
//!    reservation before any network or mock execution occurs.

use crate::empirical_live::budget::{
    AmbiguousFailureReason, BudgetError, BudgetGuard, ReconciliationReceipt,
};
use crate::empirical_live::schema::{
    LlmRequest, LlmResponse, SchemaValidationError, TokenTelemetryError,
};

/// Wrapper around sensitive credentials that redacts contents from logs, debug, and display.
#[derive(Clone, PartialEq, Eq)]
pub struct RedactedSecret(String);

impl RedactedSecret {
    /// Construct a new redacted secret container.
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    /// Expose the underlying plaintext secret strictly for authorized internal transport headers.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for RedactedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl std::fmt::Display for RedactedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED]")
    }
}

/// Typed error categories representing all provider and pipeline failure modes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    /// Pre-dispatch request validation failure.
    RequestValidation(SchemaValidationError),
    /// Pre-flight budget exhausted before dispatch.
    BudgetExhausted(String),
    /// Benchmark session halted due to a prior budget overrun.
    BudgetSafetyHalt(String),
    /// Provider authentication failure (401/403).
    Authentication { message: String },
    /// Provider rate limit reached (429).
    RateLimit {
        retry_after_ms: Option<u64>,
        message: String,
    },
    /// Ambiguous network failure (client timeout, socket reset, 504 gateway timeout).
    /// Funds remain held in uncertain spend unless authoritatively reconciled.
    AmbiguousFailure(AmbiguousFailureReason),
    /// Unambiguous network transport failure before request transmission (e.g. local DNS failure).
    Transport { message: String },
    /// Malformed response body from provider (JSON parsing failure, missing fields).
    MalformedResponse(String),
    /// Upstream provider error (500 internal server error, 503 service unavailable).
    ProviderError {
        status_code: Option<u16>,
        message: String,
    },
    /// Token telemetry invariant violation reported in response.
    InvalidTelemetry(TokenTelemetryError),
    /// Other internal pipeline error.
    Internal(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestValidation(e) => write!(f, "Request validation error: {e}"),
            Self::BudgetExhausted(msg) => write!(f, "Budget exhausted: {msg}"),
            Self::BudgetSafetyHalt(msg) => write!(f, "Session safety halt: {msg}"),
            Self::Authentication { message } => write!(f, "Authentication error: {message}"),
            Self::RateLimit {
                retry_after_ms,
                message,
            } => match retry_after_ms {
                Some(ms) => write!(f, "Rate limit exceeded (retry after {ms}ms): {message}"),
                None => write!(f, "Rate limit exceeded: {message}"),
            },
            Self::AmbiguousFailure(reason) => write!(f, "Ambiguous network failure: {reason:?}"),
            Self::Transport { message } => write!(f, "Transport error: {message}"),
            Self::MalformedResponse(message) => write!(f, "Malformed provider response: {message}"),
            Self::ProviderError {
                status_code,
                message,
            } => match status_code {
                Some(code) => write!(f, "Provider error (HTTP {code}): {message}"),
                None => write!(f, "Provider error: {message}"),
            },
            Self::InvalidTelemetry(e) => write!(f, "Invalid token telemetry: {e}"),
            Self::Internal(msg) => write!(f, "Internal provider pipeline error: {msg}"),
        }
    }
}

impl std::error::Error for ProviderError {}

impl From<SchemaValidationError> for ProviderError {
    fn from(err: SchemaValidationError) -> Self {
        Self::RequestValidation(err)
    }
}

impl From<TokenTelemetryError> for ProviderError {
    fn from(err: TokenTelemetryError) -> Self {
        Self::InvalidTelemetry(err)
    }
}

impl From<BudgetError> for ProviderError {
    fn from(err: BudgetError) -> Self {
        match err {
            BudgetError::BudgetExhausted {
                requested,
                available,
                hard_cap,
            } => Self::BudgetExhausted(format!(
                "Requested {requested}, available {available} of {hard_cap} hard cap"
            )),
            BudgetError::SessionHaltedOnOverrun(reason) => Self::BudgetSafetyHalt(reason),
            BudgetError::RequestValidationFailed(e) => Self::RequestValidation(e),
            BudgetError::InvalidTokenTelemetry(msg) => Self::MalformedResponse(msg),
            other => Self::Internal(other.to_string()),
        }
    }
}

/// Abstract provider-neutral interface to an LLM service.
pub trait LlmProvider: Send + Sync {
    /// Human-readable provider identifier (e.g. "mock", "anthropic").
    fn provider_name(&self) -> &'static str;

    /// Model identifier handled by this provider instance.
    fn model_id(&self) -> &str;

    /// Execute a single completion turn (mock execution or live provider request).
    fn execute_turn(&self, request: &LlmRequest) -> Result<LlmResponse, ProviderError>;
}

/// Single guarded dispatch boundary wrapping an [`LlmProvider`] and a [`BudgetGuard`].
///
/// Ensures all calls validate pre-flight constraints and hold worst-case reservations
/// before invoking the underlying provider.
pub struct GuardedProvider<P: LlmProvider> {
    provider: P,
    guard: BudgetGuard,
}

impl<P: LlmProvider> GuardedProvider<P> {
    /// Construct a new guarded provider wrapper.
    pub fn new(provider: P, guard: BudgetGuard) -> Self {
        Self { provider, guard }
    }

    /// Reference to the underlying LLM provider.
    pub fn provider(&self) -> &P {
        &self.provider
    }

    /// Reference to the attached BudgetGuard.
    pub fn guard(&self) -> &BudgetGuard {
        &self.guard
    }

    /// Execute a completion turn through the strict guarded pipeline:
    ///
    /// 1. Validate request schema and tool integrity.
    /// 2. Derive worst-case ceiling and atomically reserve against `BudgetGuard`.
    ///    **If reservation fails, the provider is NEVER invoked.**
    /// 3. Dispatch to `provider.execute_turn(request)`.
    /// 4. On success: Validate response telemetry and reconcile turn against the
    ///    ticket's immutable pricing snapshot, releasing unused surplus.
    /// 5. On ambiguous failure: Transfer reservation to `uncertain_spend` ledger.
    /// 6. On unambiguous pre-dispatch error: Cancel reservation and restore available budget.
    pub fn execute_guarded(
        &self,
        request: &LlmRequest,
        estimated_prompt_tokens: u64,
    ) -> Result<(LlmResponse, ReconciliationReceipt), ProviderError> {
        // Step 1: Pre-flight request validation
        request.validate().map_err(ProviderError::from)?;

        if request.model != self.provider.model_id() {
            return Err(ProviderError::Internal(format!(
                "Request model '{}' does not match provider model '{}'",
                request.model,
                self.provider.model_id()
            )));
        }

        // Step 2: Atomic pre-flight budget reservation
        // Fails closed if budget exhausted or session halted on prior overrun.
        // If this returns Err, self.provider.execute_turn is NEVER called!
        let ticket = self
            .guard
            .reserve_for_request(request, estimated_prompt_tokens)
            .map_err(ProviderError::from)?;

        // Step 3: Provider dispatch
        let result = self.provider.execute_turn(request);

        // Step 4: Outcome reconciliation & lifecycle state management
        match result {
            Ok(response) => {
                // Validate response telemetry invariants
                if let Err(telemetry_err) = response.usage.validate() {
                    let reason = AmbiguousFailureReason::InvalidTelemetry {
                        details: format!("{telemetry_err}"),
                    };
                    let _ = self.guard.reconcile_ambiguous(&ticket, reason);
                    return Err(ProviderError::InvalidTelemetry(telemetry_err));
                }

                // Verify completeness of primary telemetry fields
                if !response.usage.is_complete() {
                    let reason = AmbiguousFailureReason::InvalidTelemetry {
                        details:
                            "Response token telemetry missing prompt_tokens or completion_tokens"
                                .to_string(),
                    };
                    let _ = self.guard.reconcile_ambiguous(&ticket, reason);
                    return Err(ProviderError::MalformedResponse(
                        "Response token telemetry missing prompt_tokens or completion_tokens"
                            .to_string(),
                    ));
                }

                // Reconcile turn using the immutable pricing snapshot bound to the ticket
                match self.guard.reconcile_turn(&ticket, &response.usage) {
                    Ok(receipt) => Ok((response, receipt)),
                    Err(budget_err) => {
                        match budget_err {
                            BudgetError::CostExceedsReservationAndAvailableBudget {
                                reserved,
                                actual_cost,
                                available,
                            } => {
                                // Overrun condition: ledger is ALREADY halted inside reconcile_success!
                                // The reservation remains in Active; do not attempt duplicate transition.
                                Err(ProviderError::BudgetSafetyHalt(format!(
                                    "Budget overrun: actual cost {actual_cost} exceeded reserved {reserved} and available headroom {available}"
                                )))
                            }
                            BudgetError::InvalidTokenTelemetry(msg) => {
                                // Incomplete or invalid telemetry during calculation: hold in uncertain_spend
                                let reason = AmbiguousFailureReason::InvalidTelemetry {
                                    details: msg.clone(),
                                };
                                let _ = self.guard.reconcile_ambiguous(&ticket, reason);
                                Err(ProviderError::MalformedResponse(msg))
                            }
                            other => {
                                // Other calculation failure: hold in uncertain_spend
                                let reason = AmbiguousFailureReason::UncertainSocketWrite {
                                    details: format!("Reconciliation calculation error: {other}"),
                                };
                                let _ = self.guard.reconcile_ambiguous(&ticket, reason);
                                Err(ProviderError::Internal(other.to_string()))
                            }
                        }
                    }
                }
            }
            Err(err) => {
                match &err {
                    ProviderError::AmbiguousFailure(reason) => {
                        // Ambiguous network failure: funds are held in uncertain_spend, NEVER released
                        let _ = self.guard.reconcile_ambiguous(&ticket, reason.clone());
                    }
                    ProviderError::MalformedResponse(msg) => {
                        // Response payload corrupt: bytes were transmitted, hold in uncertain_spend
                        let reason = AmbiguousFailureReason::InvalidTelemetry {
                            details: format!("Malformed response: {msg}"),
                        };
                        let _ = self.guard.reconcile_ambiguous(&ticket, reason);
                    }
                    ProviderError::InvalidTelemetry(e) => {
                        // Telemetry error: hold in uncertain_spend
                        let reason = AmbiguousFailureReason::InvalidTelemetry {
                            details: format!("{e}"),
                        };
                        let _ = self.guard.reconcile_ambiguous(&ticket, reason);
                    }
                    _ => {
                        // Unambiguous pre-dispatch or gateway rejection (e.g. 401 auth): release reservation
                        let _ = self
                            .guard
                            .reconcile_preflight_cancel(&ticket, err.to_string());
                    }
                }
                Err(err)
            }
        }
    }
}
