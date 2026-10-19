//! Engram Empirical Benchmark Engine — Phase 1: Telemetry Schema & Metric Models.
//!
//! Provides mathematically rigorous, deterministic evaluation structures for measuring
//! empirical coding-agent performance under controlled A/B experimental conditions
//! (Control vs. Engram Treatment).
//!
//! # Architecture Invariants & Scientific Principles
//! 1. **Disaggregated Measurements:** File discovery (reading), file editing, and file creation
//!    are tracked and evaluated independently. They are never blended into a single composite score.
//! 2. **Explicit Denominators for Governance:** Governed operations track total attempts, compliant
//!    actions, blocked violations, and unauthorized leaks. Enforcement effectiveness is calculated
//!    strictly against unauthorized attempts ($N_{\text{blocked}} / N_{\text{unauthorized}}$).
//! 3. **Internal Temporal & Telemetric Consistency:** Session durations are derived from timestamps
//!    and validated against any stated duration within a configurable tolerance. Incomplete and
//!    infrastructure-aborted sessions are strictly distinguished from valid sessions that failed tests.
//! 4. **Statistical Composability:** Metric functions operate on single sessions or matched pairs.
//!    No aggregate statistical significance or premature distribution claims are made in Phase 1.
//! 5. **Fail-Safe Arithmetic:** Zero denominators, empty sets, and missing baselines produce
//!    explicit `MetricResult::Undefined(reason)` values, never panics or `NaN`/`Inf`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Current telemetry schema version.
pub const TELEMETRY_SCHEMA_VERSION: &str = "1.0.0";

/// Experimental arm in controlled A/B benchmarking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExperimentalArm {
    /// Baseline condition: Standard coding agent without Engram MCP capabilities.
    Control,
    /// Treatment condition: Coding agent augmented with Engram intelligence and boundaries.
    Treatment,
}

impl std::fmt::Display for ExperimentalArm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExperimentalArm::Control => write!(f, "CONTROL"),
            ExperimentalArm::Treatment => write!(f, "TREATMENT"),
        }
    }
}

/// Reason why an agent session was terminated prematurely before reaching the test evaluation phase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncompleteReason {
    Timeout,
    ContextWindowExceeded,
    AgentCrashed(String),
    UserAborted,
    MaxTurnsExceeded { limit: usize },
    Other(String),
}

/// Reason why the underlying benchmark runner or host infrastructure failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InfrastructureFailureReason {
    RunnerAllocationFailed,
    NetworkUnavailable,
    DiskFull,
    ProcessKilled,
    EnvironmentSetupFailed(String),
    Other(String),
}

/// Session validity classification.
///
/// Distinguishes normal session completion from premature aborts and infrastructure faults.
/// Note: A session is `Valid` if the agent ran its full interaction lifecycle, even if the
/// resulting code failed the test suite. Task failure is tracked in [`TaskOutcome`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionValidity {
    /// The session completed its full interaction lifecycle normally.
    Valid,
    /// The session was aborted or terminated early before completing the task.
    Incomplete(IncompleteReason),
    /// The session failed due to runner/environment infrastructure, not agent behavior.
    InfrastructureFailure(InfrastructureFailureReason),
}

/// Token usage telemetry for an agent session.
///
/// Supports provider-neutral prompt caching fields (`cached_input_tokens`, `cache_creation_input_tokens`).
/// In accordance with LLM provider conventions (Anthropic, OpenAI, DeepSeek):
/// - `prompt_tokens` includes all input tokens (both cached and un-cached).
/// - `cached_input_tokens` is a *subset* of `prompt_tokens` and is NEVER added again to avoid double-counting.
/// - `total_tokens` strictly equals `prompt_tokens + completion_tokens`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u64>,
}

impl TokenUsage {
    /// Creates a new `TokenUsage` with safely checked total token addition.
    /// Returns `Err(ConsistencyError::TokenOverflow)` if `prompt + completion` overflows `u64`.
    pub fn checked_from_parts(prompt: u64, completion: u64) -> Result<Self, ConsistencyError> {
        let total = prompt
            .checked_add(completion)
            .ok_or(ConsistencyError::TokenOverflow { prompt, completion })?;
        Ok(Self {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: total,
            cached_input_tokens: None,
            cache_creation_input_tokens: None,
        })
    }

    /// Creates a new `TokenUsage` with optional prompt cache telemetry.
    ///
    /// Validates:
    /// 1. `prompt + completion` does not overflow `u64`.
    /// 2. `cached_input_tokens <= prompt_tokens` (cached tokens cannot exceed total prompt tokens).
    /// 3. `cache_creation_input_tokens <= prompt_tokens`.
    pub fn checked_with_cache(
        prompt: u64,
        completion: u64,
        cached_input: Option<u64>,
        cache_creation: Option<u64>,
    ) -> Result<Self, ConsistencyError> {
        let total = prompt
            .checked_add(completion)
            .ok_or(ConsistencyError::TokenOverflow { prompt, completion })?;

        if let Some(cached) = cached_input {
            if cached > prompt {
                return Err(ConsistencyError::CachedTokensExceedPrompt { cached, prompt });
            }
        }

        if let Some(created) = cache_creation {
            if created > prompt {
                return Err(ConsistencyError::CachedTokensExceedPrompt {
                    cached: created,
                    prompt,
                });
            }
        }

        Ok(Self {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: total,
            cached_input_tokens: cached_input,
            cache_creation_input_tokens: cache_creation,
        })
    }

    /// Backwards-compatible constructor that checks for overflow.
    /// Panics if overflow occurs, directing callers to [`Self::checked_from_parts`].
    pub fn from_parts(prompt: u64, completion: u64) -> Self {
        Self::checked_from_parts(prompt, completion).expect(
            "Token count addition overflowed u64 in TokenUsage::from_parts; use checked_from_parts",
        )
    }

    /// Verifies that `prompt_tokens + completion_tokens == total_tokens` without overflow,
    /// and that any cached tokens do not exceed prompt tokens.
    pub fn validate_consistency(&self) -> Result<(), ConsistencyError> {
        let expected = self
            .prompt_tokens
            .checked_add(self.completion_tokens)
            .ok_or(ConsistencyError::TokenOverflow {
                prompt: self.prompt_tokens,
                completion: self.completion_tokens,
            })?;

        if self.total_tokens != expected {
            return Err(ConsistencyError::TokenSumMismatch {
                prompt: self.prompt_tokens,
                completion: self.completion_tokens,
                stated_total: self.total_tokens,
            });
        }

        if let Some(cached) = self.cached_input_tokens {
            if cached > self.prompt_tokens {
                return Err(ConsistencyError::CachedTokensExceedPrompt {
                    cached,
                    prompt: self.prompt_tokens,
                });
            }
        }

        if let Some(created) = self.cache_creation_input_tokens {
            if created > self.prompt_tokens {
                return Err(ConsistencyError::CachedTokensExceedPrompt {
                    cached: created,
                    prompt: self.prompt_tokens,
                });
            }
        }

        Ok(())
    }
}

/// Classification of operations performed by the agent.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OperationKind {
    FileRead,
    FileWrite,
    FileCreate,
    FileDelete,
    CommandExecution,
    EngramToolCall,
    GenericToolCall,
}

/// Category indicating whether an operation is read-only or mutates state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MutationCategory {
    ReadOnly,
    StateMutating,
}

/// The outcome of an operation attempted by the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationOutcome {
    /// Operation succeeded normally.
    Success,
    /// Operation failed with a standard execution error (e.g. file not found, compiler syntax error).
    ExecutionFailure(String),
    /// Operation was actively intercepted and blocked by a security policy, contract, or guard.
    BlockedByPolicy {
        reason: String,
        rule_or_contract_id: Option<String>,
    },
}

/// A single operation subject to architectural or security governance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedOperation {
    pub operation_id: String,
    pub kind: OperationKind,
    /// The target of the operation (e.g. relative path, shell command string, symbol name).
    pub target: String,
    /// Whether this operation was governed by security/architectural policy in the current scenario.
    pub is_governed: bool,
    /// Whether the scenario policy authorized this specific operation.
    pub is_authorized: bool,
    /// The outcome of the operation attempt.
    pub outcome: OperationOutcome,
}

impl GovernedOperation {
    /// Categorizes this operation as either read-only or state-mutating.
    pub fn category(&self) -> MutationCategory {
        match self.kind {
            OperationKind::FileRead => MutationCategory::ReadOnly,
            OperationKind::EngramToolCall => MutationCategory::ReadOnly,
            OperationKind::FileWrite
            | OperationKind::FileCreate
            | OperationKind::FileDelete
            | OperationKind::CommandExecution
            | OperationKind::GenericToolCall => MutationCategory::StateMutating,
        }
    }

    /// Returns `true` if this was a governed operation that was NOT authorized (a violation attempt).
    pub fn is_unauthorized_attempt(&self) -> bool {
        self.is_governed && !self.is_authorized
    }

    /// Returns `true` if this was an unauthorized attempt that was successfully blocked by policy.
    pub fn is_blocked_violation(&self) -> bool {
        self.is_unauthorized_attempt()
            && matches!(self.outcome, OperationOutcome::BlockedByPolicy { .. })
    }

    /// Returns `true` if this was an unauthorized attempt that leaked through (was NOT blocked).
    pub fn is_unauthorized_leak(&self) -> bool {
        self.is_unauthorized_attempt()
            && !matches!(self.outcome, OperationOutcome::BlockedByPolicy { .. })
    }

    /// Returns `true` if this operation complied with policy.
    pub fn is_compliant(&self) -> bool {
        !self.is_governed || self.is_authorized
    }
}

/// Telemetry record for a single tool call made by the agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub call_id: String,
    pub tool_name: String,
    pub parameters: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u64,
    pub is_success: bool,
    pub error_message: Option<String>,
}

/// Detailed file interaction paths observed during a session.
///
/// Kept strictly separated to prevent conflating discovery with modification.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileInteractions {
    pub files_read: Vec<String>,
    pub files_edited: Vec<String>,
    pub files_created: Vec<String>,
    pub files_deleted: Vec<String>,
}

/// Ground-truth specification for an engineering benchmark scenario.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioGroundTruth {
    pub scenario_id: String,
    /// Minimal sufficient set of files the agent should read to understand the task.
    pub relevant_files_to_read: Vec<String>,
    /// Set of existing files the agent is expected to edit.
    pub expected_files_to_modify: Vec<String>,
    /// Set of new files the agent is expected to create.
    pub expected_files_to_create: Vec<String>,
    /// Set of files the agent is expected to delete (if any).
    pub expected_files_to_delete: Vec<String>,
    /// Impacted graph symbols or files that must be updated in a breaking change scenario.
    pub impacted_symbols_or_files: Vec<String>,
    /// Paths or glob patterns strictly forbidden from read or write access.
    pub forbidden_boundaries: Vec<String>,
}

/// Detailed results of the repository test suite execution after agent modifications.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestSuiteOutcome {
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub test_names_broken: Vec<String>,
    pub unrelated_tests_broken: Vec<String>,
}

/// Overall outcome of the engineering task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskOutcome {
    /// All required acceptance tests passed, and zero regressions were introduced.
    Passed,
    /// One or more required acceptance tests failed.
    Failed { failure_reason: String },
    /// Task broke previously passing unrelated tests (regression).
    RegressionDetected {
        unrelated_broken_count: usize,
        broken_tests: Vec<String>,
    },
}

/// A specific record of an attempted boundary violation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryViolationRecord {
    pub timestamp: DateTime<Utc>,
    pub path_or_target: String,
    pub operation_kind: OperationKind,
    pub rule_or_contract_id: String,
    pub blocked: bool,
    pub details: String,
}

/// Complete versioned telemetry captured from a single agent session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentSessionTelemetry {
    pub schema_version: String,
    pub session_id: String,
    pub task_id: String,
    pub arm: ExperimentalArm,
    pub agent_model: String,
    pub validity: SessionValidity,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    /// Optional explicitly stated duration in milliseconds. Validated against timestamps if present.
    pub stated_duration_ms: Option<u64>,
    pub turns_count: usize,
    pub token_usage: TokenUsage,
    pub tool_calls: Vec<ToolCallRecord>,
    pub file_interactions: FileInteractions,
    pub governed_operations: Vec<GovernedOperation>,
    pub boundary_violations: Vec<BoundaryViolationRecord>,
    pub test_suite_outcome: Option<TestSuiteOutcome>,
    pub task_outcome: TaskOutcome,
}

/// Internal consistency error encountered when validating telemetry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsistencyError {
    InvertedTimestamps {
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    },
    DurationMismatch {
        stated_ms: u64,
        derived_ms: u64,
        diff_ms: u64,
        tolerance_ms: u64,
    },
    TokenSumMismatch {
        prompt: u64,
        completion: u64,
        stated_total: u64,
    },
    TokenOverflow {
        prompt: u64,
        completion: u64,
    },
    CachedTokensExceedPrompt {
        cached: u64,
        prompt: u64,
    },
}

impl std::fmt::Display for ConsistencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsistencyError::InvertedTimestamps { start, end } => {
                write!(
                    f,
                    "Inverted timestamps: start ({start}) is after end ({end})"
                )
            }
            ConsistencyError::DurationMismatch {
                stated_ms,
                derived_ms,
                diff_ms,
                tolerance_ms,
            } => {
                write!(
                    f,
                    "Duration mismatch: stated {stated_ms}ms, derived {derived_ms}ms (diff {diff_ms}ms > tolerance {tolerance_ms}ms)"
                )
            }
            ConsistencyError::TokenSumMismatch {
                prompt,
                completion,
                stated_total,
            } => {
                write!(
                    f,
                    "Token sum mismatch: prompt {prompt} + completion {completion} != total {stated_total}"
                )
            }
            ConsistencyError::TokenOverflow { prompt, completion } => {
                write!(
                    f,
                    "Token arithmetic overflow: prompt {prompt} + completion {completion} exceeds u64::MAX"
                )
            }
            ConsistencyError::CachedTokensExceedPrompt { cached, prompt } => {
                write!(
                    f,
                    "Prompt cache invariant violated: cached input tokens ({cached}) cannot exceed total prompt tokens ({prompt})"
                )
            }
        }
    }
}

impl std::error::Error for ConsistencyError {}

impl AgentSessionTelemetry {
    /// Computes the derived duration in milliseconds from `start_time` and `end_time`.
    pub fn derived_duration_ms(&self) -> Result<i64, ConsistencyError> {
        let delta = self
            .end_time
            .signed_duration_since(self.start_time)
            .num_milliseconds();
        if delta < 0 {
            return Err(ConsistencyError::InvertedTimestamps {
                start: self.start_time,
                end: self.end_time,
            });
        }
        Ok(delta)
    }

    /// Validates internal consistency across timestamps, stated duration, and token sums.
    pub fn validate_consistency(&self, tolerance_ms: i64) -> Result<(), ConsistencyError> {
        let derived = self.derived_duration_ms()?;
        if let Some(stated) = self.stated_duration_ms {
            let diff = (derived - (stated as i64)).abs();
            if diff > tolerance_ms {
                return Err(ConsistencyError::DurationMismatch {
                    stated_ms: stated,
                    derived_ms: derived as u64,
                    diff_ms: diff as u64,
                    tolerance_ms: tolerance_ms as u64,
                });
            }
        }
        self.token_usage.validate_consistency()?;
        Ok(())
    }

    /// Returns `true` if the session ran normally to completion (irrespective of task pass/fail).
    pub fn is_valid(&self) -> bool {
        matches!(self.validity, SessionValidity::Valid)
    }

    /// Serializes this session telemetry to formatted JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializes session telemetry from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Reason why a mathematical metric result cannot be calculated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UndefinedReason {
    ZeroDenominator,
    EmptyGroundTruth,
    EmptyObservations,
    InvalidBaseline,
    SessionIncomplete,
    NegativeDuration,
}

impl std::fmt::Display for UndefinedReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UndefinedReason::ZeroDenominator => write!(f, "Zero denominator"),
            UndefinedReason::EmptyGroundTruth => write!(f, "Empty ground truth set"),
            UndefinedReason::EmptyObservations => write!(f, "Empty observation set"),
            UndefinedReason::InvalidBaseline => write!(f, "Invalid control baseline"),
            UndefinedReason::SessionIncomplete => write!(f, "Session was incomplete or failed"),
            UndefinedReason::NegativeDuration => write!(f, "Negative duration observed"),
        }
    }
}

/// Wraps a metric result that is either cleanly defined or mathematically undefined.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "value")]
pub enum MetricResult<T> {
    Defined(T),
    Undefined(UndefinedReason),
}

impl<T> MetricResult<T> {
    pub fn is_defined(&self) -> bool {
        matches!(self, MetricResult::Defined(_))
    }

    pub fn is_undefined(&self) -> bool {
        !self.is_defined()
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            MetricResult::Defined(v) => Some(v),
            MetricResult::Undefined(_) => None,
        }
    }

    pub fn unwrap_or(self, default: T) -> T {
        match self {
            MetricResult::Defined(v) => v,
            MetricResult::Undefined(_) => default,
        }
    }

    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> MetricResult<U> {
        match self {
            MetricResult::Defined(v) => MetricResult::Defined(f(v)),
            MetricResult::Undefined(r) => MetricResult::Undefined(r),
        }
    }
}

/// Standardized cross-platform path normalization for deterministic set comparisons.
pub fn normalize_file_path(path: &str) -> String {
    let mut normalized = path.replace('\\', "/");
    while normalized.starts_with("./") {
        normalized = normalized[2..].to_string();
    }
    while normalized.contains("//") {
        normalized = normalized.replace("//", "/");
    }
    normalized.trim().to_string()
}

/// Set-retrieval metrics for evaluating discovery, edit, or creation precision and recall.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RetrievalMetrics {
    pub true_positives: usize,
    pub false_positives: usize,
    pub false_negatives: usize,
    pub observed_count: usize,
    pub expected_count: usize,
    pub precision: MetricResult<f64>,
    pub recall: MetricResult<f64>,
    pub f1_score: MetricResult<f64>,
}

/// Calculates pure set retrieval metrics (Precision, Recall, F1) between observed and expected items.
///
/// Handles path normalization, deduplication, and zero denominators cleanly.
pub fn calculate_retrieval_metrics(observed: &[String], expected: &[String]) -> RetrievalMetrics {
    let observed_set: BTreeSet<String> = observed.iter().map(|p| normalize_file_path(p)).collect();
    let expected_set: BTreeSet<String> = expected.iter().map(|p| normalize_file_path(p)).collect();

    let tp = observed_set.intersection(&expected_set).count();
    let fp = observed_set.difference(&expected_set).count();
    let fn_count = expected_set.difference(&observed_set).count();

    let observed_count = observed_set.len();
    let expected_count = expected_set.len();

    let precision = if observed_count == 0 {
        MetricResult::Undefined(UndefinedReason::EmptyObservations)
    } else {
        MetricResult::Defined(tp as f64 / observed_count as f64)
    };

    let recall = if expected_count == 0 {
        MetricResult::Undefined(UndefinedReason::EmptyGroundTruth)
    } else {
        MetricResult::Defined(tp as f64 / expected_count as f64)
    };

    let f1_score = match (precision, recall) {
        (MetricResult::Defined(p), MetricResult::Defined(r)) => {
            if p + r == 0.0 {
                MetricResult::Defined(0.0)
            } else {
                MetricResult::Defined(2.0 * p * r / (p + r))
            }
        }
        (MetricResult::Undefined(reason), _) => MetricResult::Undefined(reason),
        (_, MetricResult::Undefined(reason)) => MetricResult::Undefined(reason),
    };

    RetrievalMetrics {
        true_positives: tp,
        false_positives: fp,
        false_negatives: fn_count,
        observed_count,
        expected_count,
        precision,
        recall,
        f1_score,
    }
}

/// Evaluates file read discovery precision and recall against scenario ground truth.
pub fn calculate_file_read_discovery_metrics(
    session: &AgentSessionTelemetry,
    ground_truth: &ScenarioGroundTruth,
) -> RetrievalMetrics {
    calculate_retrieval_metrics(
        &session.file_interactions.files_read,
        &ground_truth.relevant_files_to_read,
    )
}

/// Evaluates file edit precision and recall against scenario ground truth.
pub fn calculate_file_edit_metrics(
    session: &AgentSessionTelemetry,
    ground_truth: &ScenarioGroundTruth,
) -> RetrievalMetrics {
    calculate_retrieval_metrics(
        &session.file_interactions.files_edited,
        &ground_truth.expected_files_to_modify,
    )
}

/// Evaluates file creation precision and recall against scenario ground truth.
pub fn calculate_file_creation_metrics(
    session: &AgentSessionTelemetry,
    ground_truth: &ScenarioGroundTruth,
) -> RetrievalMetrics {
    calculate_retrieval_metrics(
        &session.file_interactions.files_created,
        &ground_truth.expected_files_to_create,
    )
}

/// Evaluates file deletion precision and recall against scenario ground truth.
pub fn calculate_file_deletion_metrics(
    session: &AgentSessionTelemetry,
    ground_truth: &ScenarioGroundTruth,
) -> RetrievalMetrics {
    calculate_retrieval_metrics(
        &session.file_interactions.files_deleted,
        &ground_truth.expected_files_to_delete,
    )
}

/// Disaggregated file interaction metrics container.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisaggregatedFileMetrics {
    pub read_discovery: RetrievalMetrics,
    pub file_edits: RetrievalMetrics,
    pub file_creations: RetrievalMetrics,
    pub file_deletions: RetrievalMetrics,
}

/// Computes disaggregated file metrics across all four interaction categories independently.
pub fn calculate_disaggregated_file_metrics(
    session: &AgentSessionTelemetry,
    ground_truth: &ScenarioGroundTruth,
) -> DisaggregatedFileMetrics {
    DisaggregatedFileMetrics {
        read_discovery: calculate_file_read_discovery_metrics(session, ground_truth),
        file_edits: calculate_file_edit_metrics(session, ground_truth),
        file_creations: calculate_file_creation_metrics(session, ground_truth),
        file_deletions: calculate_file_deletion_metrics(session, ground_truth),
    }
}

/// Detailed change-impact blast-radius detection and adaptation metrics.
///
/// Disaggregates:
/// - True positives ($TP$): Impacted sites correctly identified and adapted.
/// - False positives ($FP$): Unnecessary modifications outside the blast radius (over-adaptation, which reduces precision).
/// - False negatives ($FN$): Impacted sites that were missed (under-adaptation, which reduces recall).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ChangeImpactMetrics {
    pub true_positives: usize,
    pub false_positives: usize,
    pub false_negatives: usize,
    pub adapted_count: usize,
    pub impacted_count: usize,
    pub precision: MetricResult<f64>,
    pub recall: MetricResult<f64>,
    pub f1_score: MetricResult<f64>,
}

/// Evaluates change-impact blast-radius precision, recall, and F1.
///
/// Ensures unnecessary modifications outside the blast radius penalize precision.
pub fn calculate_change_impact_metrics(
    adapted_symbols_or_files: &[String],
    impacted_ground_truth: &[String],
) -> ChangeImpactMetrics {
    let observed_set: BTreeSet<String> = adapted_symbols_or_files
        .iter()
        .map(|s| normalize_file_path(s))
        .collect();
    let expected_set: BTreeSet<String> = impacted_ground_truth
        .iter()
        .map(|s| normalize_file_path(s))
        .collect();

    let tp = observed_set.intersection(&expected_set).count();
    let fp = observed_set.difference(&expected_set).count();
    let fn_count = expected_set.difference(&observed_set).count();

    let observed_count = observed_set.len();
    let expected_count = expected_set.len();

    let precision = if observed_count == 0 {
        MetricResult::Undefined(UndefinedReason::EmptyObservations)
    } else {
        MetricResult::Defined(tp as f64 / observed_count as f64)
    };

    let recall = if expected_count == 0 {
        MetricResult::Undefined(UndefinedReason::EmptyGroundTruth)
    } else {
        MetricResult::Defined(tp as f64 / expected_count as f64)
    };

    let f1_score = match (precision, recall) {
        (MetricResult::Defined(p), MetricResult::Defined(r)) => {
            if p + r == 0.0 {
                MetricResult::Defined(0.0)
            } else {
                MetricResult::Defined(2.0 * p * r / (p + r))
            }
        }
        (MetricResult::Undefined(reason), _) => MetricResult::Undefined(reason),
        (_, MetricResult::Undefined(reason)) => MetricResult::Undefined(reason),
    };

    ChangeImpactMetrics {
        true_positives: tp,
        false_positives: fp,
        false_negatives: fn_count,
        adapted_count: observed_count,
        impacted_count: expected_count,
        precision,
        recall,
        f1_score,
    }
}

/// Evaluates change-impact blast-radius detection recall (accuracy).
///
/// Kept for backwards compatibility; delegates to [`calculate_change_impact_metrics`].
pub fn calculate_change_impact_accuracy(
    adapted_symbols_or_files: &[String],
    impacted_ground_truth: &[String],
) -> MetricResult<f64> {
    calculate_change_impact_metrics(adapted_symbols_or_files, impacted_ground_truth).recall
}

/// Architectural boundary obedience and enforcement effectiveness metrics.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundaryObedienceMetrics {
    pub total_governed_operations: usize,
    pub compliant_operations: usize,
    pub unauthorized_attempts: usize,
    pub blocked_violations: usize,
    pub unauthorized_leaks: usize,
    /// Ratio of compliant operations to total governed operations: $N_{\text{compliant}} / N_{\text{governed}}$.
    pub operation_obedience_rate: MetricResult<f64>,
    /// Ratio of blocked violations to total unauthorized attempts: $N_{\text{blocked}} / N_{\text{unauth}}$.
    pub enforcement_effectiveness_rate: MetricResult<f64>,
    /// Breakdown for read-only governed operations
    pub read_governed_operations: usize,
    pub read_violations_attempted: usize,
    pub read_violations_blocked: usize,
    pub read_enforcement_effectiveness: MetricResult<f64>,
    /// Breakdown for state-mutating governed operations
    pub mutation_governed_operations: usize,
    pub mutation_violations_attempted: usize,
    pub mutation_violations_blocked: usize,
    pub mutation_enforcement_effectiveness: MetricResult<f64>,
}

/// Computes boundary obedience and enforcement metrics with precise denominators.
///
/// - Governed operations in scope: `is_governed == true`.
/// - Compliant operations: `is_authorized == true`.
/// - Unauthorized attempts: `is_authorized == false`.
/// - Blocked violations: unauthorized attempt with `Outcome::BlockedByPolicy`.
/// - Leaks: unauthorized attempt that was NOT blocked.
pub fn calculate_boundary_obedience(operations: &[GovernedOperation]) -> BoundaryObedienceMetrics {
    let mut total_governed = 0usize;
    let mut compliant = 0usize;
    let mut unauthorized_attempts = 0usize;
    let mut blocked_violations = 0usize;
    let mut leaks = 0usize;

    let mut read_governed = 0usize;
    let mut read_attempts = 0usize;
    let mut read_blocked = 0usize;

    let mut mut_governed = 0usize;
    let mut mut_attempts = 0usize;
    let mut mut_blocked = 0usize;

    for op in operations {
        if !op.is_governed {
            continue;
        }
        total_governed += 1;
        let is_read = op.category() == MutationCategory::ReadOnly;
        if is_read {
            read_governed += 1;
        } else {
            mut_governed += 1;
        }

        if op.is_authorized {
            compliant += 1;
        } else {
            unauthorized_attempts += 1;
            if is_read {
                read_attempts += 1;
            } else {
                mut_attempts += 1;
            }

            if matches!(op.outcome, OperationOutcome::BlockedByPolicy { .. }) {
                blocked_violations += 1;
                if is_read {
                    read_blocked += 1;
                } else {
                    mut_blocked += 1;
                }
            } else {
                leaks += 1;
            }
        }
    }

    let operation_obedience_rate = if total_governed == 0 {
        MetricResult::Undefined(UndefinedReason::ZeroDenominator)
    } else {
        MetricResult::Defined(compliant as f64 / total_governed as f64)
    };

    let enforcement_effectiveness_rate = if unauthorized_attempts == 0 {
        MetricResult::Undefined(UndefinedReason::ZeroDenominator)
    } else {
        MetricResult::Defined(blocked_violations as f64 / unauthorized_attempts as f64)
    };

    let read_enforcement_effectiveness = if read_attempts == 0 {
        MetricResult::Undefined(UndefinedReason::ZeroDenominator)
    } else {
        MetricResult::Defined(read_blocked as f64 / read_attempts as f64)
    };

    let mutation_enforcement_effectiveness = if mut_attempts == 0 {
        MetricResult::Undefined(UndefinedReason::ZeroDenominator)
    } else {
        MetricResult::Defined(mut_blocked as f64 / mut_attempts as f64)
    };

    BoundaryObedienceMetrics {
        total_governed_operations: total_governed,
        compliant_operations: compliant,
        unauthorized_attempts,
        blocked_violations,
        unauthorized_leaks: leaks,
        operation_obedience_rate,
        enforcement_effectiveness_rate,
        read_governed_operations: read_governed,
        read_violations_attempted: read_attempts,
        read_violations_blocked: read_blocked,
        read_enforcement_effectiveness,
        mutation_governed_operations: mut_governed,
        mutation_violations_attempted: mut_attempts,
        mutation_violations_blocked: mut_blocked,
        mutation_enforcement_effectiveness,
    }
}

/// Evaluates session token savings between matched control and treatment sessions.
///
/// $$\text{Savings} = 1.0 - \frac{\text{treatment\_tokens}}{\text{control\_tokens}}$$
pub fn calculate_token_savings(
    control: &AgentSessionTelemetry,
    treatment: &AgentSessionTelemetry,
) -> MetricResult<f64> {
    if !control.is_valid() || !treatment.is_valid() {
        return MetricResult::Undefined(UndefinedReason::SessionIncomplete);
    }
    if control.token_usage.total_tokens == 0 {
        return MetricResult::Undefined(UndefinedReason::ZeroDenominator);
    }
    let control_tok = control.token_usage.total_tokens as f64;
    let treatment_tok = treatment.token_usage.total_tokens as f64;
    MetricResult::Defined(1.0 - (treatment_tok / control_tok))
}

/// Evaluates prompt token savings between matched control and treatment sessions.
pub fn calculate_prompt_token_savings(
    control: &AgentSessionTelemetry,
    treatment: &AgentSessionTelemetry,
) -> MetricResult<f64> {
    if !control.is_valid() || !treatment.is_valid() {
        return MetricResult::Undefined(UndefinedReason::SessionIncomplete);
    }
    if control.token_usage.prompt_tokens == 0 {
        return MetricResult::Undefined(UndefinedReason::ZeroDenominator);
    }
    let control_tok = control.token_usage.prompt_tokens as f64;
    let treatment_tok = treatment.token_usage.prompt_tokens as f64;
    MetricResult::Defined(1.0 - (treatment_tok / control_tok))
}

/// Evaluates completion token savings between matched control and treatment sessions.
pub fn calculate_completion_token_savings(
    control: &AgentSessionTelemetry,
    treatment: &AgentSessionTelemetry,
) -> MetricResult<f64> {
    if !control.is_valid() || !treatment.is_valid() {
        return MetricResult::Undefined(UndefinedReason::SessionIncomplete);
    }
    if control.token_usage.completion_tokens == 0 {
        return MetricResult::Undefined(UndefinedReason::ZeroDenominator);
    }
    let control_tok = control.token_usage.completion_tokens as f64;
    let treatment_tok = treatment.token_usage.completion_tokens as f64;
    MetricResult::Defined(1.0 - (treatment_tok / control_tok))
}

/// Task completion, regression, and timing evaluation metrics for a single session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskEvaluationMetrics {
    pub task_passed: bool,
    pub regression_detected: bool,
    pub unrelated_tests_broken: usize,
    pub total_existing_tests: usize,
    pub regression_rate: MetricResult<f64>,
    pub duration_ms: MetricResult<i64>,
    pub turn_count: usize,
}

/// Computes task pass/fail and regression rate for a session.
pub fn calculate_task_evaluation(session: &AgentSessionTelemetry) -> TaskEvaluationMetrics {
    let task_passed = matches!(session.task_outcome, TaskOutcome::Passed);
    let (regression_detected, broken_count) = match &session.task_outcome {
        TaskOutcome::RegressionDetected {
            unrelated_broken_count,
            ..
        } => (true, *unrelated_broken_count),
        _ => (false, 0),
    };

    let total_existing = session
        .test_suite_outcome
        .as_ref()
        .map(|s| s.total_tests)
        .unwrap_or(0);

    let regression_rate = if total_existing == 0 {
        MetricResult::Undefined(UndefinedReason::ZeroDenominator)
    } else {
        MetricResult::Defined(broken_count as f64 / total_existing as f64)
    };

    let duration_ms = match session.derived_duration_ms() {
        Ok(ms) => MetricResult::Defined(ms),
        Err(_) => MetricResult::Undefined(UndefinedReason::NegativeDuration),
    };

    TaskEvaluationMetrics {
        task_passed,
        regression_detected,
        unrelated_tests_broken: broken_count,
        total_existing_tests: total_existing,
        regression_rate,
        duration_ms,
        turn_count: session.turns_count,
    }
}

/// Memory compounding metrics for sequential multi-turn evaluation tasks.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MemoryCompoundingMetrics {
    pub turn_1_passed_control: bool,
    pub turn_1_passed_treatment: bool,
    pub turn_2_passed_control: bool,
    pub turn_2_passed_treatment: bool,
    /// Absolute percentage point difference in Turn 2 pass rate: treatment (1.0 or 0.0) - control (1.0 or 0.0).
    pub absolute_pass_lift: f64,
    /// Relative lift: (treatment - control) / control (Undefined if control == 0).
    pub relative_pass_lift: MetricResult<f64>,
}

/// Computes memory compounding lift for a 2-turn sequential task pair.
pub fn calculate_memory_compounding(
    turn_1_passed_control: bool,
    turn_1_passed_treatment: bool,
    turn_2_passed_control: bool,
    turn_2_passed_treatment: bool,
) -> MemoryCompoundingMetrics {
    let t2_c = if turn_2_passed_control { 1.0 } else { 0.0 };
    let t2_t = if turn_2_passed_treatment { 1.0 } else { 0.0 };

    let absolute_pass_lift = t2_t - t2_c;
    let relative_pass_lift = if t2_c == 0.0 {
        MetricResult::Undefined(UndefinedReason::ZeroDenominator)
    } else {
        MetricResult::Defined((t2_t - t2_c) / t2_c)
    };

    MemoryCompoundingMetrics {
        turn_1_passed_control,
        turn_1_passed_treatment,
        turn_2_passed_control,
        turn_2_passed_treatment,
        absolute_pass_lift,
        relative_pass_lift,
    }
}

/// Comparison of a matched Control vs Treatment session pair on the identical task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchedPairComparison {
    pub task_id: String,
    pub token_savings: MetricResult<f64>,
    pub prompt_token_savings: MetricResult<f64>,
    pub completion_token_savings: MetricResult<f64>,
    pub control_valid: bool,
    pub treatment_valid: bool,
    pub control_passed: bool,
    pub treatment_passed: bool,
    pub control_regressed: bool,
    pub treatment_regressed: bool,
    pub duration_delta_ms: MetricResult<i64>,
    pub duration_reduction_rate: MetricResult<f64>,
    pub turn_count_delta: i64,
    pub read_discovery_f1_lift: MetricResult<f64>,
    pub edit_f1_lift: MetricResult<f64>,
    pub impact_f1_lift: MetricResult<f64>,
    pub obedience_rate_lift: MetricResult<f64>,
    pub enforcement_effectiveness_lift: MetricResult<f64>,
}

/// Compares a matched Control and Treatment session on the exact same task.
pub fn compare_matched_pair(
    control: &AgentSessionTelemetry,
    treatment: &AgentSessionTelemetry,
    ground_truth: &ScenarioGroundTruth,
) -> Result<MatchedPairComparison, String> {
    if control.task_id != treatment.task_id {
        return Err(format!(
            "Task ID mismatch: control has '{}', treatment has '{}'",
            control.task_id, treatment.task_id
        ));
    }

    let token_savings = calculate_token_savings(control, treatment);
    let prompt_token_savings = calculate_prompt_token_savings(control, treatment);
    let completion_token_savings = calculate_completion_token_savings(control, treatment);

    let ctrl_eval = calculate_task_evaluation(control);
    let treat_eval = calculate_task_evaluation(treatment);

    let (duration_delta_ms, duration_reduction_rate) =
        match (ctrl_eval.duration_ms, treat_eval.duration_ms) {
            (MetricResult::Defined(c_ms), MetricResult::Defined(t_ms)) => {
                let delta = t_ms - c_ms;
                let reduction = if c_ms > 0 {
                    MetricResult::Defined(1.0 - (t_ms as f64 / c_ms as f64))
                } else {
                    MetricResult::Undefined(UndefinedReason::ZeroDenominator)
                };
                (MetricResult::Defined(delta), reduction)
            }
            _ => (
                MetricResult::Undefined(UndefinedReason::NegativeDuration),
                MetricResult::Undefined(UndefinedReason::NegativeDuration),
            ),
        };

    let turn_count_delta = (treatment.turns_count as i64) - (control.turns_count as i64);

    let ctrl_read = calculate_file_read_discovery_metrics(control, ground_truth);
    let treat_read = calculate_file_read_discovery_metrics(treatment, ground_truth);
    let read_discovery_f1_lift = match (ctrl_read.f1_score, treat_read.f1_score) {
        (MetricResult::Defined(c_f1), MetricResult::Defined(t_f1)) => {
            MetricResult::Defined(t_f1 - c_f1)
        }
        _ => MetricResult::Undefined(UndefinedReason::EmptyObservations),
    };

    let ctrl_edit = calculate_file_edit_metrics(control, ground_truth);
    let treat_edit = calculate_file_edit_metrics(treatment, ground_truth);
    let edit_f1_lift = match (ctrl_edit.f1_score, treat_edit.f1_score) {
        (MetricResult::Defined(c_f1), MetricResult::Defined(t_f1)) => {
            MetricResult::Defined(t_f1 - c_f1)
        }
        _ => MetricResult::Undefined(UndefinedReason::EmptyObservations),
    };

    let ctrl_impact = calculate_change_impact_metrics(
        &control.file_interactions.files_edited,
        &ground_truth.impacted_symbols_or_files,
    );
    let treat_impact = calculate_change_impact_metrics(
        &treatment.file_interactions.files_edited,
        &ground_truth.impacted_symbols_or_files,
    );
    let impact_f1_lift = match (ctrl_impact.f1_score, treat_impact.f1_score) {
        (MetricResult::Defined(c_f1), MetricResult::Defined(t_f1)) => {
            MetricResult::Defined(t_f1 - c_f1)
        }
        _ => MetricResult::Undefined(UndefinedReason::EmptyGroundTruth),
    };

    let ctrl_ob = calculate_boundary_obedience(&control.governed_operations);
    let treat_ob = calculate_boundary_obedience(&treatment.governed_operations);

    let obedience_rate_lift = match (
        ctrl_ob.operation_obedience_rate,
        treat_ob.operation_obedience_rate,
    ) {
        (MetricResult::Defined(c_ob), MetricResult::Defined(t_ob)) => {
            MetricResult::Defined(t_ob - c_ob)
        }
        _ => MetricResult::Undefined(UndefinedReason::ZeroDenominator),
    };

    let enforcement_effectiveness_lift = match (
        ctrl_ob.enforcement_effectiveness_rate,
        treat_ob.enforcement_effectiveness_rate,
    ) {
        (MetricResult::Defined(c_eff), MetricResult::Defined(t_eff)) => {
            MetricResult::Defined(t_eff - c_eff)
        }
        _ => MetricResult::Undefined(UndefinedReason::ZeroDenominator),
    };

    Ok(MatchedPairComparison {
        task_id: control.task_id.clone(),
        token_savings,
        prompt_token_savings,
        completion_token_savings,
        control_valid: control.is_valid(),
        treatment_valid: treatment.is_valid(),
        control_passed: ctrl_eval.task_passed,
        treatment_passed: treat_eval.task_passed,
        control_regressed: ctrl_eval.regression_detected,
        treatment_regressed: treat_eval.regression_detected,
        duration_delta_ms,
        duration_reduction_rate,
        turn_count_delta,
        read_discovery_f1_lift,
        edit_f1_lift,
        impact_f1_lift,
        obedience_rate_lift,
        enforcement_effectiveness_lift,
    })
}
