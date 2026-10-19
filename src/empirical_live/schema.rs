//! Provider-neutral schemas, message models, and token telemetry for live agent evaluation.
//!
//! This module defines provider-agnostic representations of chat messages, tool
//! definitions, tool invocations, token telemetry, and completion requests/responses.
//! Concrete provider clients (e.g. Anthropic, OpenAI) translate these structures to and
//! from provider-specific JSON wire formats.

use serde::{Deserialize, Serialize};

/// Role of an entity in a multi-turn conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System prompt setting agent behavior and boundaries.
    System,
    /// Human user or benchmark orchestrator.
    User,
    /// AI coding agent under evaluation.
    Assistant,
    /// Execution environment returning tool output.
    Tool,
}

/// A structured content block within a conversation message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text content.
    Text { text: String },
    /// A tool invocation requested by the model.
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// The result of executing a previously requested tool.
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
}

impl ContentBlock {
    /// Create a plain text content block.
    pub fn text(t: impl Into<String>) -> Self {
        Self::Text { text: t.into() }
    }

    /// Create a tool use content block.
    pub fn tool_use(
        id: impl Into<String>,
        name: impl Into<String>,
        input: serde_json::Value,
    ) -> Self {
        Self::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        }
    }

    /// Create a tool result content block.
    pub fn tool_result(
        tool_use_id: impl Into<String>,
        content: impl Into<String>,
        is_error: bool,
    ) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error,
        }
    }
}

/// A single message in a multi-turn conversation.
///
/// NOTE (AUD-B6-TOK-02): While `ChatMessage` contains an optional `name` field for
/// provider-neutral metadata, this field is not accepted or transmitted by the
/// Anthropic Messages API wire format adapter (`to_anthropic_request`), which strictly
/// adheres to the `/v1/messages` schema containing only `role` and `content`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role of the message sender.
    pub role: Role,
    /// Content blocks comprising the message.
    pub content: Vec<ContentBlock>,
    /// Optional participant name or identifier (provider-neutral metadata; not emitted in Anthropic wire payloads).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    /// Construct a user message with plain text content.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlock::Text { text: text.into() }],
            name: None,
        }
    }

    /// Construct an assistant message with plain text content.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![ContentBlock::Text { text: text.into() }],
            name: None,
        }
    }

    /// Construct a system message with plain text content.
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: vec![ContentBlock::Text { text: text.into() }],
            name: None,
        }
    }

    /// Construct a tool result message.
    pub fn tool_result(
        tool_use_id: impl Into<String>,
        content: impl Into<String>,
        is_error: bool,
    ) -> Self {
        Self {
            role: Role::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                content: content.into(),
                is_error,
            }],
            name: None,
        }
    }

    /// Extract concatenated plain text from all Text blocks in the message.
    pub fn text_content(&self) -> String {
        let mut out = String::new();
        for block in &self.content {
            if let ContentBlock::Text { text } = block {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(text);
            }
        }
        out
    }
}

/// Specification of a tool available to the model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique name of the tool (e.g. `read_file`, `write_file`).
    pub name: String,
    /// Human- and model-readable description of tool purpose and parameters.
    pub description: String,
    /// JSON Schema object describing the required and optional input properties.
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    /// Validate that the tool definition satisfies basic schema requirements.
    pub fn validate(&self) -> Result<(), SchemaValidationError> {
        if self.name.trim().is_empty() {
            return Err(SchemaValidationError::EmptyToolName);
        }
        if !self
            .name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(SchemaValidationError::InvalidToolName(self.name.clone()));
        }
        if !self.input_schema.is_object() {
            return Err(SchemaValidationError::InvalidToolSchema(
                self.name.clone(),
                "input_schema must be a JSON object".to_string(),
            ));
        }
        Ok(())
    }
}

/// Token usage reported by an LLM provider for a single request/response turn.
///
/// Invariants enforced:
/// 1. `cached_input_tokens <= prompt_tokens`: Cached tokens are a strict non-additive
///    subset of prompt tokens (Phase 1 invariant). They are never double-counted.
/// 2. `total_tokens == prompt_tokens + completion_tokens` when all three are present.
/// 3. Missing telemetry fields are represented explicitly as `None`, never fabricated as 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    /// Total input tokens processed in the prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u64>,
    /// Tokens generated in the completion response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u64>,
    /// Tokens read from provider prompt cache (subset of `prompt_tokens`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_input_tokens: Option<u64>,
    /// Tokens written into provider prompt cache, if tracked by provider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u64>,
    /// Provider-reported total token count, if provided directly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
}

impl TokenUsage {
    /// Construct usage where prompt and completion are known.
    pub fn new(prompt_tokens: u64, completion_tokens: u64) -> Self {
        Self {
            prompt_tokens: Some(prompt_tokens),
            completion_tokens: Some(completion_tokens),
            cached_input_tokens: None,
            cache_write_tokens: None,
            total_tokens: Some(prompt_tokens + completion_tokens),
        }
    }

    /// Check whether primary token telemetry is completely present.
    pub fn is_complete(&self) -> bool {
        self.prompt_tokens.is_some() && self.completion_tokens.is_some()
    }

    /// Calculate total billed tokens strictly following the Phase 1 invariant:
    /// `BilledTokens = prompt_tokens + completion_tokens`.
    ///
    /// Returns `None` if either prompt or completion tokens are missing.
    pub fn billed_tokens(&self) -> Option<u64> {
        match (self.prompt_tokens, self.completion_tokens) {
            (Some(p), Some(c)) => p.checked_add(c),
            _ => None,
        }
    }

    /// Validate token usage against Engram mathematical integrity invariants.
    pub fn validate(&self) -> Result<(), TokenTelemetryError> {
        // Invariant 1: Cache read tokens cannot exceed prompt tokens.
        if let (Some(cached), Some(prompt)) = (self.cached_input_tokens, self.prompt_tokens)
            && cached > prompt
        {
            return Err(TokenTelemetryError::CacheExceedsPrompt {
                cached_tokens: cached,
                prompt_tokens: prompt,
            });
        }

        // Invariant 2: Total tokens must equal prompt + completion if all are provided.
        if let (Some(prompt), Some(completion), Some(total)) = (
            self.prompt_tokens,
            self.completion_tokens,
            self.total_tokens,
        ) {
            let expected = prompt
                .checked_add(completion)
                .ok_or(TokenTelemetryError::ArithmeticOverflow)?;
            if total != expected {
                return Err(TokenTelemetryError::InconsistentTotal {
                    prompt_tokens: prompt,
                    completion_tokens: completion,
                    reported_total: total,
                    expected_total: expected,
                });
            }
        }

        Ok(())
    }
}

/// Reason why the model terminated its generation turn.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Natural completion (stop token encountered).
    EndTurn,
    /// Generation halted because `max_tokens` was reached.
    MaxTokens,
    /// Model emitted one or more tool calls awaiting host execution.
    ToolUse,
    /// A custom stop sequence was matched.
    StopSequence,
    /// Provider-specific or unrecognized stop reason.
    Unknown(String),
}

/// Provider-neutral request to an LLM provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmRequest {
    /// Target model identifier (e.g. `claude-3-7-sonnet-20250219`).
    pub model: String,
    /// System prompt instruction string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Conversation message history.
    pub messages: Vec<ChatMessage>,
    /// Tool definitions available for model invocation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDefinition>,
    /// Maximum completion tokens permitted in the response.
    pub max_tokens: u32,
    /// Sampling temperature (typically 0.0 to 1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Stop sequences that immediately halt model generation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop_sequences: Vec<String>,
}

impl LlmRequest {
    /// Validate request parameters before dispatching to any provider.
    pub fn validate(&self) -> Result<(), SchemaValidationError> {
        if self.model.trim().is_empty() {
            return Err(SchemaValidationError::EmptyModel);
        }
        if self.max_tokens == 0 {
            return Err(SchemaValidationError::ZeroMaxTokens);
        }
        if self.messages.is_empty() && self.system_prompt.is_none() {
            return Err(SchemaValidationError::EmptyMessages);
        }
        if let Some(t) = self.temperature
            && !(0.0..=2.0).contains(&t)
        {
            return Err(SchemaValidationError::InvalidTemperature(t.to_string()));
        }
        let mut seen_tools = std::collections::HashSet::new();
        for tool in &self.tools {
            tool.validate()?;
            if !seen_tools.insert(&tool.name) {
                return Err(SchemaValidationError::DuplicateToolName(tool.name.clone()));
            }
        }
        Ok(())
    }
}

/// Provider-neutral response returned by an LLM provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmResponse {
    /// Unique provider response ID.
    pub id: String,
    /// Model name reported by provider.
    pub model: String,
    /// Generated content blocks.
    pub content: Vec<ContentBlock>,
    /// Reason why the generation concluded.
    pub stop_reason: StopReason,
    /// Authoritative token usage telemetry.
    pub usage: TokenUsage,
}

impl LlmResponse {
    /// Extract all tool use requests emitted in this response.
    pub fn tool_calls(&self) -> Vec<(&str, &str, &serde_json::Value)> {
        let mut calls = Vec::new();
        for block in &self.content {
            if let ContentBlock::ToolUse { id, name, input } = block {
                calls.push((id.as_str(), name.as_str(), input));
            }
        }
        calls
    }

    /// Extract concatenated plain text from all Text blocks.
    pub fn text(&self) -> String {
        let mut text = String::new();
        for block in &self.content {
            if let ContentBlock::Text { text: t } = block {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(t);
            }
        }
        text
    }
}

/// Errors occurring during schema validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaValidationError {
    EmptyModel,
    ZeroMaxTokens,
    EmptyMessages,
    InvalidTemperature(String),
    EmptyToolName,
    InvalidToolName(String),
    InvalidToolSchema(String, String),
    DuplicateToolName(String),
}

impl std::fmt::Display for SchemaValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyModel => write!(f, "Model identifier cannot be empty"),
            Self::ZeroMaxTokens => write!(f, "max_tokens must be greater than zero"),
            Self::EmptyMessages => write!(
                f,
                "Request must contain at least one message or system prompt"
            ),
            Self::InvalidTemperature(t) => {
                write!(f, "Temperature {t} is outside valid range [0.0, 2.0]")
            }
            Self::EmptyToolName => write!(f, "Tool name cannot be empty"),
            Self::InvalidToolName(name) => write!(
                f,
                "Invalid tool name: '{name}' (must be alphanumeric with '_' or '-')"
            ),
            Self::InvalidToolSchema(name, msg) => {
                write!(f, "Invalid schema for tool '{name}': {msg}")
            }
            Self::DuplicateToolName(name) => write!(f, "Duplicate tool name in request: '{name}'"),
        }
    }
}

impl std::error::Error for SchemaValidationError {}

/// Errors occurring during token telemetry validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenTelemetryError {
    CacheExceedsPrompt {
        cached_tokens: u64,
        prompt_tokens: u64,
    },
    InconsistentTotal {
        prompt_tokens: u64,
        completion_tokens: u64,
        reported_total: u64,
        expected_total: u64,
    },
    ArithmeticOverflow,
}

impl std::fmt::Display for TokenTelemetryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CacheExceedsPrompt {
                cached_tokens,
                prompt_tokens,
            } => {
                write!(
                    f,
                    "Cached input tokens ({cached_tokens}) exceeds prompt tokens ({prompt_tokens})"
                )
            }
            Self::InconsistentTotal {
                prompt_tokens,
                completion_tokens,
                reported_total,
                expected_total,
            } => {
                write!(
                    f,
                    "Total tokens ({reported_total}) does not equal prompt ({prompt_tokens}) + completion ({completion_tokens}) = {expected_total}"
                )
            }
            Self::ArithmeticOverflow => {
                write!(f, "Arithmetic overflow calculating token telemetry")
            }
        }
    }
}

impl std::error::Error for TokenTelemetryError {}
