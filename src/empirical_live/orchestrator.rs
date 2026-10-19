//! Multi-turn autonomous agent interaction loop and scenario orchestrator for Milestone B3.
//!
//! # Safety & Execution Invariants
//! 1. **Guarded Provider Dispatch Only:** Every turn dispatches strictly through
//!    [`GuardedProvider`], enforcing atomic pre-flight budget reservation, token telemetry
//!    validation, and reconciliation against immutable pricing schedules.
//! 2. **Fail-Closed Budget Halts:** Budget exhaustion or overrun safety halts immediately
//!    terminate the session with [`HaltReason::BudgetExhausted`] or [`HaltReason::BudgetSafetyHalt`],
//!    preventing any subsequent model dispatch.
//! 3. **Ambiguous Spend Protection:** If a provider encounters an ambiguous failure (timeout,
//!    dropped connection, 504), funds remain locked in `uncertain_spend` and the session
//!    halts immediately. **Ambiguous outcomes are NEVER retried as though they failed.**
//! 4. **Infrastructure vs. Agent Retry Distinction:** Transient infrastructure errors (e.g. 500/503)
//!    are limited to a strict retry ceiling (default 1). Agent task failures (syntax errors,
//!    test failures, failed tool executions) are NEVER retried.
//! 5. **Tool Result Correlation:** Every tool call emitted by the assistant is executed via
//!    the [`ToolDispatcher`] and correlated back by `tool_use_id` into a tool response message.
//! 6. **Clean Cancellation:** External cancellation halts the session cleanly without orphaned
//!    reservations or ledger imbalance.
//! 7. **Offline Invariant:** In Milestone B3, execution operates strictly offline with
//!    mock providers. OS-level process encapsulation and network firewalls are part of Milestone B5.

use crate::empirical_live::budget::PromptTokenEstimator;
use crate::empirical_live::dispatcher::{TaskFinishDeclaration, ToolDispatcher};
use crate::empirical_live::provider::{GuardedProvider, LlmProvider, ProviderError};
use crate::empirical_live::schema::{ChatMessage, ContentBlock, LlmRequest, Role, StopReason};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Configuration options for multi-turn orchestrator execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// Maximum allowed conversation turns before halting.
    pub max_turns: usize,
    /// Maximum token generation ceiling per turn.
    pub max_tokens_per_turn: u32,
    /// Target model identifier.
    pub model: String,
    /// Optional system prompt defining agent persona and behavioral instructions.
    pub system_prompt: Option<String>,
    /// Maximum consecutive tool errors tolerated before halting the session.
    pub max_consecutive_tool_errors: usize,
    /// Maximum number of infrastructure retries permitted per session (strictly <= 1).
    pub max_infrastructure_retries: usize,
}

impl OrchestratorConfig {
    /// Construct default configuration for benchmark sessions.
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            max_turns: 10,
            max_tokens_per_turn: 4096,
            model: model.into(),
            system_prompt: None,
            max_consecutive_tool_errors: 3,
            max_infrastructure_retries: 1,
        }
    }

    /// Set a custom system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Set a custom max turns ceiling.
    pub fn with_max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns;
        self
    }

    /// Set a custom max tokens per turn.
    pub fn with_max_tokens_per_turn(mut self, max_tokens: u32) -> Self {
        self.max_tokens_per_turn = max_tokens;
        self
    }
}

/// Reason why an agent session concluded prematurely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HaltReason {
    /// Pre-flight budget was insufficient for worst-case cost reservation.
    BudgetExhausted(String),
    /// Session was halted because a prior turn incurred a budget overrun.
    BudgetSafetyHalt(String),
    /// Ambiguous provider failure (timeout, network drop); funds held in uncertain spend.
    AmbiguousProviderFailure(String),
    /// Conversation exceeded the configured max turns ceiling.
    MaxTurnsExceeded { limit: usize },
    /// Turn conclusion reached max completion tokens without emitting finish.
    MaxTokensReachedInTurn,
    /// Agent produced repeated consecutive tool execution errors.
    ConsecutiveToolErrorsExceeded { count: usize },
    /// Infrastructure failure exhausted the retry ceiling (max 1 retry).
    InfrastructureFailure(String),
    /// External cancellation request was received.
    Cancelled,
    /// Fatal provider or schema error.
    ProviderError(String),
}

impl std::fmt::Display for HaltReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BudgetExhausted(msg) => write!(f, "Budget exhausted: {msg}"),
            Self::BudgetSafetyHalt(msg) => write!(f, "Session safety halt: {msg}"),
            Self::AmbiguousProviderFailure(msg) => write!(f, "Ambiguous provider failure: {msg}"),
            Self::MaxTurnsExceeded { limit } => write!(f, "Max turns exceeded ({limit})"),
            Self::MaxTokensReachedInTurn => write!(f, "Max tokens reached in generation turn"),
            Self::ConsecutiveToolErrorsExceeded { count } => {
                write!(f, "Consecutive tool errors exceeded limit ({count})")
            }
            Self::InfrastructureFailure(msg) => write!(f, "Infrastructure failure: {msg}"),
            Self::Cancelled => write!(f, "Session cancelled by external request"),
            Self::ProviderError(msg) => write!(f, "Provider error: {msg}"),
        }
    }
}

/// High-level session outcome classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionOutcome {
    /// Agent completed the session normally (either via `finish` tool or natural end turn).
    Completed {
        finish_declaration: Option<TaskFinishDeclaration>,
        turns_executed: usize,
    },
    /// Session was halted prematurely.
    Halted {
        reason: HaltReason,
        turns_executed: usize,
    },
}

/// Complete telemetry and audit trail of an executed multi-turn session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTelemetry {
    pub session_id: String,
    pub scenario_id: String,
    pub model: String,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub total_cached_tokens: u64,
    pub total_cost_micro_dollars: u64,
    pub turns_executed: usize,
    pub tool_calls_executed: usize,
    pub duration_ms: u64,
    pub outcome: SessionOutcome,
    pub conversation_history: Vec<ChatMessage>,
}

impl SessionTelemetry {
    /// Return true if the session completed its full interaction lifecycle normally.
    pub fn is_valid(&self) -> bool {
        matches!(self.outcome, SessionOutcome::Completed { .. })
    }

    /// Return true if the session halted prematurely before completion.
    pub fn is_incomplete(&self) -> bool {
        matches!(self.outcome, SessionOutcome::Halted { .. })
    }

    /// Return true if the session completed and declared task success.
    pub fn is_task_success(&self) -> bool {
        match &self.outcome {
            SessionOutcome::Completed {
                finish_declaration: Some(decl),
                ..
            } => decl.success,
            _ => false,
        }
    }
}

/// Multi-turn autonomous agent orchestrator for Milestone B3.
pub struct MultiTurnOrchestrator {
    config: OrchestratorConfig,
}

impl MultiTurnOrchestrator {
    /// Construct a new orchestrator with the specified configuration.
    pub fn new(config: OrchestratorConfig) -> Self {
        Self { config }
    }

    /// Execute a full multi-turn session.
    pub fn run_session<P: LlmProvider, D: ToolDispatcher>(
        &self,
        session_id: &str,
        scenario_id: &str,
        initial_user_prompt: &str,
        provider: &GuardedProvider<P>,
        dispatcher: &D,
        cancel_flag: Option<&AtomicBool>,
    ) -> SessionTelemetry {
        let start_time = Instant::now();
        let mut messages: Vec<ChatMessage> = vec![ChatMessage::user(initial_user_prompt)];

        let mut total_prompt_tokens = 0u64;
        let mut total_completion_tokens = 0u64;
        let mut total_cached_tokens = 0u64;
        let mut total_cost_micros = 0u64;
        let mut turns_executed = 0usize;
        let mut tool_calls_executed = 0usize;
        let mut consecutive_tool_errors = 0usize;
        let mut infra_retry_count = 0usize;
        let mut outcome = None;

        while turns_executed < self.config.max_turns {
            // Check cancellation request
            if cancel_flag.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
                outcome = Some(SessionOutcome::Halted {
                    reason: HaltReason::Cancelled,
                    turns_executed,
                });
                break;
            }

            let request = LlmRequest {
                model: self.config.model.clone(),
                system_prompt: self.config.system_prompt.clone(),
                messages: messages.clone(),
                tools: dispatcher.tool_definitions(),
                max_tokens: self.config.max_tokens_per_turn,
                temperature: Some(0.0),
                stop_sequences: Vec::new(),
            };

            // Calculate conservative upper bound for prompt token reservation
            let estimated_prompt =
                match PromptTokenEstimator::conservative_upper_bound_tokens(&request) {
                    Ok(tokens) => tokens,
                    Err(err) => {
                        outcome = Some(SessionOutcome::Halted {
                            reason: HaltReason::ProviderError(format!(
                                "Failed to bound prompt tokens: {err}"
                            )),
                            turns_executed,
                        });
                        break;
                    }
                };

            // Dispatch guarded turn
            let dispatch_result = provider.execute_guarded(&request, estimated_prompt);

            match dispatch_result {
                Err(err) => {
                    match err {
                        ProviderError::BudgetExhausted(msg) => {
                            outcome = Some(SessionOutcome::Halted {
                                reason: HaltReason::BudgetExhausted(msg),
                                turns_executed,
                            });
                            break;
                        }
                        ProviderError::BudgetSafetyHalt(msg) => {
                            outcome = Some(SessionOutcome::Halted {
                                reason: HaltReason::BudgetSafetyHalt(msg),
                                turns_executed,
                            });
                            break;
                        }
                        ProviderError::AmbiguousFailure(reason) => {
                            // Invariant: Do NOT retry ambiguous failures as though they definitely failed!
                            outcome = Some(SessionOutcome::Halted {
                                reason: HaltReason::AmbiguousProviderFailure(format!("{reason:?}")),
                                turns_executed,
                            });
                            break;
                        }
                        ProviderError::RateLimit { message, .. }
                        | ProviderError::Transport { message }
                        | ProviderError::ProviderError { message, .. } => {
                            // Transient infrastructure failures: check retry ceiling (max 1 retry)
                            if infra_retry_count < self.config.max_infrastructure_retries {
                                infra_retry_count += 1;
                                continue;
                            } else {
                                outcome = Some(SessionOutcome::Halted {
                                    reason: HaltReason::InfrastructureFailure(message),
                                    turns_executed,
                                });
                                break;
                            }
                        }
                        other => {
                            outcome = Some(SessionOutcome::Halted {
                                reason: HaltReason::ProviderError(other.to_string()),
                                turns_executed,
                            });
                            break;
                        }
                    }
                }
                Ok((response, receipt)) => {
                    turns_executed += 1;

                    // Accumulate token usage telemetry
                    if let Some(p) = response.usage.prompt_tokens {
                        total_prompt_tokens = total_prompt_tokens.saturating_add(p);
                    }
                    if let Some(c) = response.usage.completion_tokens {
                        total_completion_tokens = total_completion_tokens.saturating_add(c);
                    }
                    if let Some(cached) = response.usage.cached_input_tokens {
                        total_cached_tokens = total_cached_tokens.saturating_add(cached);
                    }
                    total_cost_micros = total_cost_micros.saturating_add(receipt.actual_cost.0);

                    // Append assistant response to conversation history
                    messages.push(ChatMessage {
                        role: Role::Assistant,
                        content: response.content.clone(),
                        name: None,
                    });

                    let tool_calls = response.tool_calls();
                    if tool_calls.is_empty() {
                        // Model concluded without requesting tools
                        match response.stop_reason {
                            StopReason::MaxTokens => {
                                outcome = Some(SessionOutcome::Halted {
                                    reason: HaltReason::MaxTokensReachedInTurn,
                                    turns_executed,
                                });
                            }
                            _ => {
                                outcome = Some(SessionOutcome::Completed {
                                    finish_declaration: dispatcher.finish_declaration(),
                                    turns_executed,
                                });
                            }
                        }
                        break;
                    }

                    // Execute requested tool calls
                    let mut tool_results = Vec::new();
                    let mut any_tool_error = false;

                    for (id, name, input) in tool_calls {
                        tool_calls_executed += 1;
                        let result_block = dispatcher.dispatch(name, id, input);
                        if let ContentBlock::ToolResult { is_error: true, .. } = &result_block {
                            any_tool_error = true;
                        }
                        tool_results.push(result_block);
                    }

                    // Track consecutive tool error ceiling
                    if any_tool_error {
                        consecutive_tool_errors += 1;
                        if consecutive_tool_errors >= self.config.max_consecutive_tool_errors {
                            outcome = Some(SessionOutcome::Halted {
                                reason: HaltReason::ConsecutiveToolErrorsExceeded {
                                    count: consecutive_tool_errors,
                                },
                                turns_executed,
                            });
                            break;
                        }
                    } else {
                        consecutive_tool_errors = 0;
                    }

                    // Check if the agent declared task finish
                    if let Some(decl) = dispatcher.finish_declaration() {
                        outcome = Some(SessionOutcome::Completed {
                            finish_declaration: Some(decl),
                            turns_executed,
                        });
                        break;
                    }

                    // Append correlated tool results to conversation history
                    messages.push(ChatMessage {
                        role: Role::Tool,
                        content: tool_results,
                        name: None,
                    });
                }
            }
        }

        if outcome.is_none() {
            outcome = Some(SessionOutcome::Halted {
                reason: HaltReason::MaxTurnsExceeded {
                    limit: self.config.max_turns,
                },
                turns_executed,
            });
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        SessionTelemetry {
            session_id: session_id.to_string(),
            scenario_id: scenario_id.to_string(),
            model: self.config.model.clone(),
            total_prompt_tokens,
            total_completion_tokens,
            total_cached_tokens,
            total_cost_micro_dollars: total_cost_micros,
            turns_executed,
            tool_calls_executed,
            duration_ms,
            outcome: outcome.expect("outcome must be set"),
            conversation_history: messages,
        }
    }
}
