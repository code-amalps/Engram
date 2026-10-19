//! Anthropic Messages API wire-format adapter and header sanitization.
//!
//! # Specification & Invariants
//! 1. **Messages API v1 Translation:** Maps between Engram provider-neutral [`LlmRequest`]/[`LlmResponse`]
//!    and Anthropic's `/v1/messages` JSON wire format.
//! 2. **Token Telemetry Integrity:**
//!    - Anthropic separates uncached `input_tokens` from `cache_read_input_tokens`.
//!    - Total `prompt_tokens` is normalized to `input_tokens + cache_read_input_tokens`.
//!    - Invariant enforced: `cached_input_tokens <= prompt_tokens` (non-additive subset rule).
//!    - Invariant enforced: Missing telemetry fields are rejected fail-closed; **never fabricated**.
//! 3. **Secret Redaction:** [`sanitize_anthropic_headers`] strips API keys (`x-api-key`, `Authorization`)
//!    from headers and error logs before any debug or telemetry capture.
//! 4. **100% Offline in B2:** Translates JSON structures entirely in-memory; zero network transmission.

use crate::empirical_live::provider::ProviderError;
use crate::empirical_live::schema::{
    ContentBlock, LlmRequest, LlmResponse, Role, StopReason, TokenUsage,
};
use serde_json::{Value, json};
use std::collections::HashMap;

/// Translate a provider-neutral [`LlmRequest`] into an Anthropic Messages API JSON payload.
///
/// Fails closed if request validation fails.
pub fn to_anthropic_request(request: &LlmRequest) -> Result<Value, ProviderError> {
    request.validate().map_err(ProviderError::from)?;

    let mut payload = serde_json::Map::new();
    payload.insert("model".to_string(), json!(request.model));
    payload.insert("max_tokens".to_string(), json!(request.max_tokens));

    // Handle system prompt: extract top-level system_prompt or system chat messages
    let mut system_text = request.system_prompt.clone().unwrap_or_default();
    let mut messages_json = Vec::new();

    for msg in &request.messages {
        match msg.role {
            Role::System => {
                let text = msg.text_content();
                if !text.is_empty() {
                    if !system_text.is_empty() {
                        system_text.push_str("\n\n");
                    }
                    system_text.push_str(&text);
                }
            }
            Role::User => {
                let blocks = serialize_content_blocks(&msg.content);
                messages_json.push(json!({
                    "role": "user",
                    "content": blocks,
                }));
            }
            Role::Assistant => {
                let blocks = serialize_content_blocks(&msg.content);
                messages_json.push(json!({
                    "role": "assistant",
                    "content": blocks,
                }));
            }
            Role::Tool => {
                // In Anthropic Messages API, tool results are represented as user role messages
                // with content blocks of type "tool_result".
                let blocks = serialize_content_blocks(&msg.content);
                messages_json.push(json!({
                    "role": "user",
                    "content": blocks,
                }));
            }
        }
    }

    if !system_text.is_empty() {
        payload.insert("system".to_string(), json!(system_text));
    }

    payload.insert("messages".to_string(), json!(messages_json));

    // Tools
    if !request.tools.is_empty() {
        let tools_json: Vec<Value> = request
            .tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                })
            })
            .collect();
        payload.insert("tools".to_string(), json!(tools_json));
    }

    // Sampling parameters
    if let Some(t) = request.temperature {
        payload.insert("temperature".to_string(), json!(t));
    }

    if !request.stop_sequences.is_empty() {
        payload.insert("stop_sequences".to_string(), json!(request.stop_sequences));
    }

    Ok(Value::Object(payload))
}

fn serialize_content_blocks(blocks: &[ContentBlock]) -> Vec<Value> {
    blocks
        .iter()
        .map(|b| match b {
            ContentBlock::Text { text } => json!({
                "type": "text",
                "text": text,
            }),
            ContentBlock::ToolUse { id, name, input } => json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input,
            }),
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                let mut map = serde_json::Map::new();
                map.insert("type".to_string(), json!("tool_result"));
                map.insert("tool_use_id".to_string(), json!(tool_use_id));
                map.insert("content".to_string(), json!(content));
                if *is_error {
                    map.insert("is_error".to_string(), json!(true));
                }
                Value::Object(map)
            }
        })
        .collect()
}

/// Translate an Anthropic Messages API response JSON payload into a provider-neutral [`LlmResponse`].
///
/// Fails closed if response contains API errors, malformed fields, or missing/inconsistent usage telemetry.
pub fn from_anthropic_response(value: &Value) -> Result<LlmResponse, ProviderError> {
    // Check for Anthropic API error envelope: {"type": "error", "error": {"type": "...", "message": "..."}}
    if let Some(err_obj) = value.get("error") {
        let err_type = err_obj
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown_error");
        let err_msg = err_obj
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Anthropic API error");

        return match err_type {
            "authentication_error" => Err(ProviderError::Authentication {
                message: err_msg.to_string(),
            }),
            "rate_limit_error" => Err(ProviderError::RateLimit {
                retry_after_ms: None,
                message: err_msg.to_string(),
            }),
            "invalid_request_error" => Err(ProviderError::MalformedResponse(err_msg.to_string())),
            _ => Err(ProviderError::ProviderError {
                status_code: None,
                message: format!("{err_type}: {err_msg}"),
            }),
        };
    }

    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            ProviderError::MalformedResponse("Anthropic response missing or empty 'id'".to_string())
        })?
        .to_string();

    let model = value
        .get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            ProviderError::MalformedResponse(
                "Anthropic response missing or empty 'model'".to_string(),
            )
        })?
        .to_string();

    let stop_reason_raw = value
        .get("stop_reason")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ProviderError::MalformedResponse("Anthropic response missing 'stop_reason'".to_string())
        })?;

    let stop_reason = match stop_reason_raw {
        "end_turn" => StopReason::EndTurn,
        "max_tokens" => StopReason::MaxTokens,
        "stop_sequence" => StopReason::StopSequence,
        "tool_use" => StopReason::ToolUse,
        other => StopReason::Unknown(other.to_string()),
    };

    // Parse content blocks
    let content_arr = value
        .get("content")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            ProviderError::MalformedResponse(
                "Anthropic response missing 'content' array".to_string(),
            )
        })?;

    let mut content = Vec::new();
    for block in content_arr {
        let block_type = block.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
            ProviderError::MalformedResponse("Content block missing required 'type'".to_string())
        })?;

        match block_type {
            "text" => {
                let text = block.get("text").and_then(|v| v.as_str()).ok_or_else(|| {
                    ProviderError::MalformedResponse("Text block missing 'text' field".to_string())
                })?;
                content.push(ContentBlock::Text {
                    text: text.to_string(),
                });
            }
            "tool_use" => {
                let tool_id = block.get("id").and_then(|v| v.as_str()).ok_or_else(|| {
                    ProviderError::MalformedResponse("ToolUse block missing 'id'".to_string())
                })?;
                let tool_name = block.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                    ProviderError::MalformedResponse("ToolUse block missing 'name'".to_string())
                })?;
                let input = block
                    .get("input")
                    .cloned()
                    .unwrap_or(Value::Object(serde_json::Map::new()));

                content.push(ContentBlock::ToolUse {
                    id: tool_id.to_string(),
                    name: tool_name.to_string(),
                    input,
                });
            }
            other => {
                return Err(ProviderError::MalformedResponse(format!(
                    "Unrecognized Anthropic content block type: '{other}'"
                )));
            }
        }
    }

    // Parse usage telemetry
    let usage_obj = value
        .get("usage")
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            ProviderError::MalformedResponse(
                "Anthropic response missing 'usage' object".to_string(),
            )
        })?;

    let raw_input_tokens = usage_obj
        .get("input_tokens")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            ProviderError::MalformedResponse("Usage missing required 'input_tokens'".to_string())
        })?;

    let completion_tokens = usage_obj
        .get("output_tokens")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            ProviderError::MalformedResponse("Usage missing required 'output_tokens'".to_string())
        })?;

    let cache_read = usage_obj
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_u64());

    let cache_write = usage_obj
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_u64());

    // Anthropic Messages API telemetry semantics:
    // `input_tokens` is the count of uncached prompt tokens.
    // Total prompt tokens = uncached input_tokens + cache_read_input_tokens.
    let total_prompt = match cache_read {
        Some(read) => raw_input_tokens
            .checked_add(read)
            .ok_or_else(|| ProviderError::MalformedResponse("Prompt token overflow".to_string()))?,
        None => raw_input_tokens,
    };

    let total_tokens = total_prompt
        .checked_add(completion_tokens)
        .ok_or_else(|| ProviderError::MalformedResponse("Total token overflow".to_string()))?;

    let usage = TokenUsage {
        prompt_tokens: Some(total_prompt),
        completion_tokens: Some(completion_tokens),
        cached_input_tokens: cache_read,
        cache_write_tokens: cache_write,
        total_tokens: Some(total_tokens),
    };

    // Validate token telemetry against Engram invariants (cache <= prompt, total == prompt + completion)
    usage.validate().map_err(ProviderError::from)?;

    Ok(LlmResponse {
        id,
        model,
        content,
        stop_reason,
        usage,
    })
}

/// Sanitize an iterator of HTTP header key-value pairs, completely redacting sensitive credentials.
///
/// Strips API keys (`x-api-key`, `Authorization`, `Anthropic-Auth-Token`) replacing values with `"[REDACTED]"`.
pub fn sanitize_anthropic_headers<'a, I>(headers: I) -> Vec<(String, String)>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    headers
        .into_iter()
        .map(|(key, value)| {
            let lower = key.to_ascii_lowercase();
            if lower == "x-api-key"
                || lower == "authorization"
                || lower == "anthropic-auth-token"
                || lower == "cookie"
                || lower == "set-cookie"
                || lower.contains("secret")
                || lower.contains("key")
                || lower.contains("token")
            {
                (key.to_string(), "[REDACTED]".to_string())
            } else {
                (key.to_string(), value.to_string())
            }
        })
        .collect()
}

/// Sanitize a HashMap of headers.
pub fn sanitize_header_map(headers: &HashMap<String, String>) -> HashMap<String, String> {
    headers
        .iter()
        .map(|(k, v)| {
            let lower = k.to_ascii_lowercase();
            if lower == "x-api-key"
                || lower == "authorization"
                || lower == "anthropic-auth-token"
                || lower == "cookie"
                || lower == "set-cookie"
                || lower.contains("secret")
                || lower.contains("key")
                || lower.contains("token")
            {
                (k.clone(), "[REDACTED]".to_string())
            } else {
                (k.clone(), v.clone())
            }
        })
        .collect()
}
