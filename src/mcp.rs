//! MCP Server implementation: stdio JSON-RPC 2.0 protocol exposing engineering intelligence.

use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
};

use crate::compiler::context;
use crate::constitution::evaluate_rules;
use crate::git_history::get_cochanges_for_file;
use crate::guard::impact;
use crate::memory::{load_durable_memories, query_relevant_memories, record_provisional_decision};
use crate::storage::{db, root_path};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Safely truncates a string slice to at most `max_bytes` without slicing inside a multi-byte UTF-8 character boundary.
/// If `max_bytes` falls inside a character, the boundary is moved backwards to the start of that character.
/// If `max_bytes == 0` or less than the first character's byte length, returns an empty string slice `""`.
pub fn safe_truncate_str(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &s[..boundary]
}

/// Validate that a user-provided file path resides strictly inside the workspace root.
/// Helper to normalize verbatim prefixes and path casing for component-aware containment.
fn normalize_for_containment(p: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let s = p.to_string_lossy();
        let stripped = if s.starts_with(r"\\?\") { &s[4..] } else { &s };
        PathBuf::from(stripped.to_lowercase().replace('/', "\\"))
    }
    #[cfg(not(windows))]
    {
        p.to_path_buf()
    }
}

/// Validate that a user-provided file path resides strictly inside the workspace root.
/// Blocks path traversal (../), external absolute paths, sibling prefix escapes,
/// symlink/junction escapes, and null bytes. Fail closed if containment cannot be established.
pub fn validate_workspace_path(root: &Path, user_path: &str) -> Result<PathBuf> {
    if user_path.contains('\0') {
        bail!("Path validation error: path contains null bytes");
    }
    let trimmed = user_path.trim();
    if trimmed.is_empty() {
        bail!("Path validation error: path cannot be empty");
    }

    let path_obj = Path::new(trimmed);

    // 1. Prohibit explicit parent traversal components ('..')
    for comp in path_obj.components() {
        if matches!(comp, std::path::Component::ParentDir) {
            bail!(
                "Path validation error: directory traversal ('..') is prohibited: {}",
                trimmed
            );
        }
    }

    let canonical_root = root_path(root)?;
    let norm_root = normalize_for_containment(&canonical_root);

    // 2. Resolve candidate path
    let candidate = if path_obj.is_absolute() {
        // If absolute, perform component-aware containment check before any filesystem access
        let norm_cand = normalize_for_containment(path_obj);
        if !norm_cand.starts_with(&norm_root) {
            bail!(
                "Path validation error: absolute path '{}' is outside workspace root '{}'",
                trimmed,
                canonical_root.display()
            );
        }
        path_obj.to_path_buf()
    } else {
        // Relative path: strictly append to canonical workspace root
        canonical_root.join(path_obj)
    };

    // 3. Component-aware containment check on actual target or nearest ancestor
    if candidate.exists() {
        let canonical_candidate = candidate
            .canonicalize()
            .map_err(|e| anyhow!("Failed to resolve path '{}': {}", candidate.display(), e))?;
        let norm_canonical = normalize_for_containment(&canonical_candidate);
        if !norm_canonical.starts_with(&norm_root) {
            bail!(
                "Path validation error: path '{}' escapes workspace root '{}'",
                trimmed,
                canonical_root.display()
            );
        }
        Ok(canonical_candidate)
    } else {
        // Target does not exist yet. Walk up existing ancestors to ensure no symlink escape occurs.
        let mut existing_ancestor = candidate.as_path();
        let mut unverified_components = Vec::new();

        while !existing_ancestor.exists() {
            if let Some(comp) = existing_ancestor.file_name() {
                unverified_components.push(comp);
            }
            match existing_ancestor.parent() {
                Some(parent) => existing_ancestor = parent,
                None => break,
            }
        }

        if !existing_ancestor.exists() {
            bail!(
                "Path validation error: unable to locate valid workspace ancestor for '{}'",
                trimmed
            );
        }

        let canonical_ancestor = existing_ancestor.canonicalize().map_err(|e| {
            anyhow!(
                "Failed to resolve ancestor path '{}': {}",
                existing_ancestor.display(),
                e
            )
        })?;
        let norm_ancestor = normalize_for_containment(&canonical_ancestor);

        if !norm_ancestor.starts_with(&norm_root) {
            bail!(
                "Path validation error: ancestor path '{}' escapes workspace root '{}'",
                trimmed,
                canonical_root.display()
            );
        }

        // Reconstruct validated candidate path
        unverified_components.reverse();
        let mut validated_candidate = canonical_ancestor;
        for comp in unverified_components {
            validated_candidate.push(comp);
        }

        let norm_validated = normalize_for_containment(&validated_candidate);
        if !norm_validated.starts_with(&norm_root) {
            bail!(
                "Path validation error: path '{}' escapes workspace root",
                trimmed
            );
        }

        Ok(validated_candidate)
    }
}

pub fn json_result(v: Value) -> Value {
    let text = if let Some(s) = v.as_str() {
        s.to_string()
    } else {
        serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())
    };
    json!({
        "content": [{
            "type": "text",
            "text": text
        }],
        "isError": false
    })
}

pub fn tool_error(msg: impl Into<String>) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": msg.into()
        }],
        "isError": true
    })
}

/// Resolve the configured response byte ceiling.
/// Precedence:
/// 1. Tool argument "max_bytes" or "byte_limit" (clamped to 1024..=65536)
/// 2. Environment variable ENGRAM_MCP_MAX_BYTES (clamped to 1024..=65536)
/// 3. Default: 16,384 bytes (16 KB)
pub fn resolve_byte_limit(args: &Value) -> usize {
    if let Some(limit) = args
        .get("max_bytes")
        .and_then(|v| v.as_u64())
        .or_else(|| args.get("byte_limit").and_then(|v| v.as_u64()))
    {
        return (limit as usize).clamp(1024, 65536);
    }
    if let Ok(env_val) = std::env::var("ENGRAM_MCP_MAX_BYTES") {
        if let Ok(limit) = env_val.trim().parse::<usize>() {
            return limit.clamp(1024, 65536);
        }
    }
    16_384
}

/// Compute the exact UTF-8 serialized byte length of the complete JSON-RPC response,
/// including the JSON-RPC envelope, tool result wrapper, and trailing newline.
pub fn compute_envelope_bytes(id: &Value, tool_res: &Value) -> usize {
    let rpc_msg = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": tool_res
    });
    serde_json::to_vec(&rpc_msg)
        .map(|v| v.len() + 1)
        .unwrap_or(usize::MAX)
}

/// Enforce universal response boundary across every MCP endpoint.
/// Progressively sheds payload content through structured levels:
/// - Level 0: Pretty-printed response within budget.
/// - Level 1: Compact JSON (eliminate unnecessary whitespace/indentation).
/// - Level 2: Condense narrative explanations and retrieval guidance.
/// - Level 3: Prune low-priority entities, warnings, and memories.
/// - Level 4: Truncate omission manifest sample entries to minimal form (counts only).
/// - Level 5: Condense code excerpts to minimal signature/stub.
/// - Level 6: Minimal valid response containing essential target identification and payload-truncated notification.
/// - Level 7: Structured JSON error response indicating context exceeds boundary.
pub fn enforce_mcp_envelope_boundary(
    tool_name: &str,
    mut payload: Value,
    byte_limit: usize,
    id: &Value,
) -> Value {
    // Level 0: Pretty-printed response within budget
    if let Ok(pretty_str) = serde_json::to_string_pretty(&payload) {
        let tool_res = json!({
            "content": [{
                "type": "text",
                "text": pretty_str
            }],
            "isError": false
        });
        if compute_envelope_bytes(id, &tool_res) <= byte_limit {
            return tool_res;
        }
    }

    // Level 1: Compact JSON (eliminate whitespace/indentation)
    if let Ok(compact_str) = serde_json::to_string(&payload) {
        let tool_res = json!({
            "content": [{
                "type": "text",
                "text": compact_str
            }],
            "isError": false
        });
        if compute_envelope_bytes(id, &tool_res) <= byte_limit {
            return tool_res;
        }
    }

    // Level 2: Condense narrative explanations and retrieval guidance
    if let Some(obj) = payload.as_object_mut() {
        if let Some(exp) = obj
            .get_mut("explanation")
            .and_then(|v| v.as_str().map(str::to_string))
        {
            if exp.len() > 150 {
                let truncated = safe_truncate_str(&exp, 150);
                obj.insert(
                    "explanation".to_string(),
                    json!(format!("{}... [Condensed]", truncated)),
                );
            }
        }
        if let Some(guidance) = obj.get_mut("targeted_retrieval_guidance") {
            *guidance = json!("Use engram_inspect_symbol for omitted items.");
        }
        if let Some(man) = obj
            .get_mut("omission_manifest")
            .and_then(|v| v.as_object_mut())
        {
            if let Some(g) = man.get_mut("targeted_retrieval_guidance") {
                *g = json!("Use engram_inspect_symbol for omitted items.");
            }
        }
        if let Some(ctx_obj) = obj.get_mut("context").and_then(|v| v.as_object_mut()) {
            if let Some(man) = ctx_obj
                .get_mut("omission_manifest")
                .and_then(|v| v.as_object_mut())
            {
                if let Some(g) = man.get_mut("targeted_retrieval_guidance") {
                    *g = json!("Use engram_inspect_symbol for omitted items.");
                }
            }
        }
        if let Some(summary) = obj
            .get_mut("summary")
            .and_then(|v| v.as_str().map(str::to_string))
        {
            if summary.len() > 100 {
                let truncated = safe_truncate_str(&summary, 100);
                obj.insert(
                    "summary".to_string(),
                    json!(format!("{}... [Condensed]", truncated)),
                );
            }
        }
        if let Some(na) = obj
            .get_mut("next_action")
            .and_then(|v| v.as_str().map(str::to_string))
        {
            if na.len() > 80 {
                obj.insert(
                    "next_action".to_string(),
                    json!("Proceed with planned changes."),
                );
            }
        }
    }
    if let Ok(compact_str) = serde_json::to_string(&payload) {
        let tool_res = json!({
            "content": [{ "type": "text", "text": compact_str }],
            "isError": false
        });
        if compute_envelope_bytes(id, &tool_res) <= byte_limit {
            return tool_res;
        }
    }

    // Level 3: Prune low-priority entities, warnings, and memories
    if let Some(obj) = payload.as_object_mut() {
        if let Some(mem) = obj.get_mut("memories") {
            *mem = json!([]);
        }
        if let Some(mc) = obj.get_mut("memory_conflicts") {
            *mc = json!([]);
        }
        if let Some(cw) = obj
            .get_mut("cochange_warnings")
            .and_then(|v| v.as_array_mut())
        {
            if cw.len() > 1 {
                cw.truncate(1);
            }
        }
        if let Some(gr) = obj
            .get_mut("governing_rules")
            .and_then(|v| v.as_array_mut())
        {
            if gr.len() > 2 {
                gr.truncate(2);
            }
        }
        if let Some(rd) = obj.get_mut("relevant_decisions") {
            *rd = json!([]);
        }
        let mut shed_from_cone = 0;
        if let Some(cone) = obj.get_mut("impact_cone").and_then(|v| v.as_object_mut()) {
            if let Some(hist) = cone
                .get_mut("historical_entities")
                .and_then(|v| v.as_array_mut())
            {
                shed_from_cone += hist.len();
                hist.clear();
            }
            if let Some(deps) = cone
                .get_mut("import_dependent_entities")
                .and_then(|v| v.as_array_mut())
            {
                if deps.len() > 1 {
                    shed_from_cone += deps.len() - 1;
                    deps.truncate(1);
                }
            }
            if let Some(deps) = cone
                .get_mut("dependency_entities")
                .and_then(|v| v.as_array_mut())
            {
                if deps.len() > 1 {
                    deps.truncate(1);
                }
            }
        }
        if shed_from_cone > 0 {
            if let Some(man) = obj
                .get_mut("omission_manifest")
                .and_then(|v| v.as_object_mut())
            {
                man.insert("budget_status".to_string(), json!("OMISSIONS_RECORDED"));
                let cur = man
                    .get("omitted_items_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                man.insert(
                    "omitted_items_count".to_string(),
                    json!(cur + shed_from_cone as u64),
                );
            }
        }
        if let Some(files) = obj.get_mut("target_files").and_then(|v| v.as_array_mut()) {
            for f in files.iter_mut() {
                if let Some(fobj) = f.as_object_mut() {
                    fobj.insert("provenance".to_string(), json!([]));
                }
            }
        }
        if let Some(ctx_obj) = obj.get_mut("context").and_then(|v| v.as_object_mut()) {
            if let Some(cw) = ctx_obj
                .get_mut("cochange_warnings")
                .and_then(|v| v.as_array_mut())
            {
                if cw.len() > 1 {
                    cw.truncate(1);
                }
            }
            if let Some(gr) = ctx_obj
                .get_mut("governing_rules")
                .and_then(|v| v.as_array_mut())
            {
                if gr.len() > 2 {
                    gr.truncate(2);
                }
            }
            if let Some(rd) = ctx_obj.get_mut("relevant_decisions") {
                *rd = json!([]);
            }
            if let Some(files) = ctx_obj
                .get_mut("target_files")
                .and_then(|v| v.as_array_mut())
            {
                for f in files.iter_mut() {
                    if let Some(fobj) = f.as_object_mut() {
                        fobj.insert("provenance".to_string(), json!([]));
                    }
                }
            }
        }
    }
    if let Ok(compact_str) = serde_json::to_string(&payload) {
        let tool_res = json!({
            "content": [{ "type": "text", "text": compact_str }],
            "isError": false
        });
        if compute_envelope_bytes(id, &tool_res) <= byte_limit {
            return tool_res;
        }
    }

    // Level 4: Truncate omission manifest sample entries to minimal form (counts only)
    if let Some(obj) = payload.as_object_mut() {
        let mut shed_from_cone_l4 = 0;
        if let Some(cone) = obj.get_mut("impact_cone").and_then(|v| v.as_object_mut()) {
            if let Some(direct) = cone
                .get_mut("direct_entities")
                .and_then(|v| v.as_array_mut())
            {
                if direct.len() > 1 {
                    direct.truncate(1);
                }
            }
            if let Some(deps) = cone
                .get_mut("import_dependent_entities")
                .and_then(|v| v.as_array_mut())
            {
                shed_from_cone_l4 += deps.len();
                deps.clear();
            }
        }
        if let Some(man) = obj
            .get_mut("omission_manifest")
            .and_then(|v| v.as_object_mut())
        {
            man.insert("sample_omissions".to_string(), json!([]));
            man.insert("omission_manifest".to_string(), json!([]));
            man.insert("omission_reasons".to_string(), json!({}));
            man.insert("manifest_truncated".to_string(), json!(true));
            if shed_from_cone_l4 > 0 {
                man.insert("budget_status".to_string(), json!("OMISSIONS_RECORDED"));
                let cur = man
                    .get("omitted_items_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                man.insert(
                    "omitted_items_count".to_string(),
                    json!(cur + shed_from_cone_l4 as u64),
                );
            }
        }
        if let Some(gaps) = obj.get_mut("ambiguity_gaps").and_then(|v| v.as_array_mut()) {
            if gaps.len() > 1 {
                gaps.truncate(1);
            }
        }
        if let Some(viols) = obj.get_mut("violations").and_then(|v| v.as_array_mut()) {
            if viols.len() > 1 {
                viols.truncate(1);
            }
        }
        if let Some(ctx_obj) = obj.get_mut("context").and_then(|v| v.as_object_mut()) {
            if let Some(man) = ctx_obj
                .get_mut("omission_manifest")
                .and_then(|v| v.as_object_mut())
            {
                man.insert("sample_omissions".to_string(), json!([]));
                man.insert("omission_manifest".to_string(), json!([]));
                man.insert("omission_reasons".to_string(), json!({}));
                man.insert("manifest_truncated".to_string(), json!(true));
            }
            if let Some(gaps) = ctx_obj
                .get_mut("ambiguity_gaps")
                .and_then(|v| v.as_array_mut())
            {
                if gaps.len() > 1 {
                    gaps.truncate(1);
                }
            }
        }
    }
    if let Ok(compact_str) = serde_json::to_string(&payload) {
        let tool_res = json!({
            "content": [{ "type": "text", "text": compact_str }],
            "isError": false
        });
        if compute_envelope_bytes(id, &tool_res) <= byte_limit {
            return tool_res;
        }
    }

    // Level 5: Condense code excerpts to minimal signature/stub
    if let Some(obj) = payload.as_object_mut() {
        if obj.contains_key("code_excerpt") {
            obj.insert("code_excerpt".to_string(), json!(null));
        }
        if let Some(files) = obj.get_mut("target_files").and_then(|v| v.as_array_mut()) {
            for f in files.iter_mut() {
                if let Some(fobj) = f.as_object_mut() {
                    fobj.insert("content".to_string(), json!(""));
                }
            }
            if files.len() > 2 {
                files.truncate(2);
            }
        }
        if let Some(ctx_obj) = obj.get_mut("context").and_then(|v| v.as_object_mut()) {
            if let Some(files) = ctx_obj
                .get_mut("target_files")
                .and_then(|v| v.as_array_mut())
            {
                for f in files.iter_mut() {
                    if let Some(fobj) = f.as_object_mut() {
                        fobj.insert("content".to_string(), json!(""));
                    }
                }
                if files.len() > 2 {
                    files.truncate(2);
                }
            }
        }
        obj.insert("explanation".to_string(), json!(""));
        obj.insert("active_contract".to_string(), json!(null));
        obj.insert("authorization".to_string(), json!(null));
    }
    if let Ok(compact_str) = serde_json::to_string(&payload) {
        let tool_res = json!({
            "content": [{ "type": "text", "text": compact_str }],
            "isError": false
        });
        if compute_envelope_bytes(id, &tool_res) <= byte_limit {
            return tool_res;
        }
    }

    // Level 6: Minimal valid response containing essential target identification and payload_truncated
    let mut min_payload = json!({
        "status": payload.get("status").cloned().unwrap_or_else(|| json!("READY")),
        "payload_truncated": true,
        "truncation_reason": "Serialized response exceeded max_bytes limit; non-essential fields shed.",
        "byte_limit": byte_limit
    });
    if let Some(t) = payload.get("target") {
        min_payload["target"] = t.clone();
    }
    if let Some(tf) = payload.get("target_file") {
        min_payload["target_file"] = tf.clone();
    }
    if let Some(t) = payload.get("task") {
        let task_str = if let Some(s) = t.as_str() {
            s
        } else if let Some(s) = t.get("goal").and_then(|g| g.as_str()) {
            s
        } else {
            ""
        };
        let condensed_task = if task_str.len() > 60 {
            safe_truncate_str(task_str, 60)
        } else {
            task_str
        };
        min_payload["task"] = json!(condensed_task);
    }
    if let Some(cid) = payload
        .get("contract_id")
        .or_else(|| payload.get("contract").and_then(|c| c.get("contract_id")))
    {
        min_payload["contract_id"] = cid.clone();
    }

    if let Ok(compact_str) = serde_json::to_string(&min_payload) {
        let tool_res = json!({
            "content": [{ "type": "text", "text": compact_str }],
            "isError": false
        });
        if compute_envelope_bytes(id, &tool_res) <= byte_limit {
            return tool_res;
        }
    }

    // Level 7: Structured error response
    let err_msg = format!(
        "Response boundary error: response cannot fit within max_bytes={} budget for {}. Query with narrower parameters.",
        byte_limit, tool_name
    );
    let err_json = json!({
        "error": "BOUNDARY_EXCEEDED",
        "message": err_msg,
        "byte_limit": byte_limit
    });
    let err_str = serde_json::to_string(&err_json)
        .unwrap_or_else(|_| r#"{"error":"BOUNDARY_EXCEEDED"}"#.to_string());
    let err_res = json!({
        "content": [{
            "type": "text",
            "text": err_str
        }],
        "isError": true
    });
    if compute_envelope_bytes(id, &err_res) <= byte_limit {
        return err_res;
    }

    // Extreme fallback
    json!({
        "content": [{
            "type": "text",
            "text": r#"{"error":"OVERSIZED"}"#
        }],
        "isError": true
    })
}

pub fn mcp_call_raw(root: &Path, name: &str, args: &Value) -> Result<Value> {
    let root = root_path(root)?;
    match name {
        "engram_engage" => crate::engage::handle_engram_engage(&root, args),
        "engram_prepare_task" | "engram_prepare" => {
            let task = args["task"].as_str().unwrap_or("").trim();
            if task.is_empty() {
                bail!("Task description must not be empty");
            }
            let budget = args["token_budget"].as_u64().unwrap_or(1200) as usize;
            if budget == 0 {
                bail!("Token budget must be greater than 0");
            }
            let no_bypass = args["no_bypass"]
                .as_bool()
                .or_else(|| args["force_no_bypass"].as_bool())
                .unwrap_or(false);
            let details = args["details"]
                .as_bool()
                .or_else(|| args["verbose"].as_bool())
                .unwrap_or(false);

            let forbidden_raw: Option<Vec<String>> = args["forbidden"].as_array().map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            });

            let forbidden = if let Some(fb) = forbidden_raw {
                for p in &fb {
                    validate_workspace_path(&root, p)
                        .map_err(|e| anyhow!("Forbidden boundary validation failed: {}", e))?;
                }
                Some(fb)
            } else {
                None
            };

            let planned_files_raw: Option<Vec<String>> =
                args["planned_files"].as_array().map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                });

            // 1. Resolve planned files: extract explicitly supplied files or parse declarations directly
            // from task prompt. Under NO circumstances seed authorized edit targets from ambient read context files.
            let (core_task, parsed_planned) =
                crate::flywheel::parse_task_and_planned_files(task, planned_files_raw);

            for p in &parsed_planned {
                validate_workspace_path(&root, p)
                    .map_err(|e| anyhow!("Planned file validation failed: {}", e))?;
            }

            let effective_planned = if parsed_planned.is_empty() {
                None
            } else {
                Some(parsed_planned)
            };

            // Adaptive bypass evaluation for small/trivial leaf tasks
            let signals = crate::prompt::classify_task_complexity(&core_task);
            if signals.classification == crate::prompt::TaskComplexity::Trivial && !no_bypass {
                let mut bypass_targets = effective_planned.clone().unwrap_or_default();
                if bypass_targets.is_empty() {
                    let (_, mut task_planned) =
                        crate::flywheel::parse_task_and_planned_files(&core_task, None);
                    if task_planned.is_empty() {
                        for inline in crate::prompt::collect_inline_file_references(&core_task) {
                            if !task_planned.contains(&inline) {
                                task_planned.push(inline);
                            }
                        }
                    }
                    bypass_targets = task_planned;
                }

                if bypass_targets.len() == 1
                    && crate::storage::save_bypass_trivial(
                        &root,
                        &core_task,
                        &signals.reason,
                        &bypass_targets,
                    )
                    .is_ok()
                {
                    return Ok(json!({
                        "status": "BYPASS_TRIVIAL",
                        "task": core_task,
                        "classification": "TRIVIAL",
                        "reason": signals.reason,
                        "contract_id": Value::Null,
                        "contract_version": Value::Null,
                        "baseline_commit": Value::Null,
                        "boundaries": {
                            "expected_files": Value::Array(Vec::new()),
                            "expected_modules": Value::Array(Vec::new()),
                            "forbidden_boundaries": Value::Array(Vec::new()),
                        },
                        "context": {
                            "target_files": Value::Array(Vec::new()),
                            "governing_rules": Value::Array(Vec::new()),
                            "cochange_warnings": Value::Array(Vec::new()),
                            "relevant_decisions": Value::Array(Vec::new()),
                            "ambiguity_gaps": Value::Array(Vec::new()),
                            "token_budget": budget,
                            "estimated_tokens": 0,
                        },
                        "blocking_violations": Value::Array(Vec::new()),
                        "is_reused_contract": false,
                        "reused_reason": Value::Null,
                        "next_action": "Task classified as trivial leaf edit. Proceed directly to implementation without Change Contract overhead. Call engram_postflight only if compliance audit is desired."
                    }));
                }
            }

            // 2. Context compilation
            let ctx = context(&root, &core_task, budget)?;

            // 3. Preflight handshake and contract locking
            let preflight_opts = crate::flywheel::PreflightOptions {
                task: core_task,
                token_budget: budget,
                forbidden_boundaries: forbidden,
                planned_files: effective_planned,
            };
            let preflight_res = crate::flywheel::preflight_task(&root, &preflight_opts)?;

            // 4. Return structured response
            if details {
                Ok(json!({
                    "status": match preflight_res.status {
                        crate::flywheel::PreflightStatus::ReadyForImplementation => "READY",
                        crate::flywheel::PreflightStatus::Blocked => "BLOCKED",
                    },
                    "contract_id": preflight_res.contract_id,
                    "contract_version": preflight_res.contract_version,
                    "baseline_commit": preflight_res.baseline_commit,
                    "task_goal": preflight_res.task_goal,
                    "preflight": serde_json::to_value(&preflight_res)?,
                    "context": serde_json::to_value(&ctx)?,
                    "is_reused_contract": preflight_res.is_reused_contract,
                    "reused_reason": preflight_res.reused_reason,
                    "next_action": if preflight_res.status == crate::flywheel::PreflightStatus::Blocked {
                        "Reconcile blocking violations before modifying code or request human override."
                    } else if preflight_res.is_reused_contract {
                        "Valid Change Contract active. Proceed to edit expected files."
                    } else {
                        "Change Contract locked. Proceed to edit expected files. Re-run engram_prepare_task if new files are needed."
                    }
                }))
            } else {
                let status_str = match preflight_res.status {
                    crate::flywheel::PreflightStatus::ReadyForImplementation => "READY",
                    crate::flywheel::PreflightStatus::Blocked => "BLOCKED",
                };
                let target_files: Vec<Value> = ctx
                    .files
                    .iter()
                    .take(8)
                    .map(|f| {
                        json!({
                            "path": f.path,
                            "level": f.level,
                            "score": (f.score * 100.0).round() / 100.0,
                        })
                    })
                    .collect();
                let governing_rules: Vec<String> = preflight_res
                    .governing_policies
                    .iter()
                    .take(5)
                    .map(|p| format!("{}: {}", p.id, p.title))
                    .collect();
                let cochange_warnings: Vec<String> =
                    ctx.cochange_alerts.iter().take(5).cloned().collect();
                let relevant_decisions: Vec<String> =
                    ctx.relevant_memories.iter().take(5).cloned().collect();
                let blocking_violations =
                    if preflight_res.status == crate::flywheel::PreflightStatus::Blocked {
                        preflight_res.ambiguity_gaps.clone()
                    } else {
                        Vec::new()
                    };
                let next_action = if preflight_res.status
                    == crate::flywheel::PreflightStatus::Blocked
                {
                    "Reconcile blocking violations before modifying code or request human override."
                } else if preflight_res.is_reused_contract {
                    "Valid Change Contract active. Proceed to edit expected files."
                } else {
                    "Change Contract locked. Proceed to edit expected files. Re-run engram_prepare_task if new files are needed."
                };

                Ok(json!({
                    "status": status_str,
                    "contract_id": preflight_res.contract_id,
                    "contract_version": preflight_res.contract_version,
                    "baseline_commit": preflight_res.baseline_commit,
                    "task_goal": preflight_res.task_goal,
                    "boundaries": {
                        "expected_files": preflight_res.expected_files,
                        "expected_modules": preflight_res.expected_modules,
                        "forbidden_boundaries": preflight_res.forbidden_boundaries,
                    },
                    "context": {
                        "target_files": target_files,
                        "governing_rules": governing_rules,
                        "cochange_warnings": cochange_warnings,
                        "relevant_decisions": relevant_decisions,
                        "ambiguity_gaps": ctx.gaps,
                        "token_budget": ctx.token_budget,
                        "estimated_tokens": ctx.estimated_tokens,
                    },
                    "blocking_violations": blocking_violations,
                    "is_reused_contract": preflight_res.is_reused_contract,
                    "reused_reason": preflight_res.reused_reason,
                    "next_action": next_action,
                }))
            }
        }
        "engram_get_context" => {
            let task = args["task"].as_str().unwrap_or("").trim();
            if task.is_empty() {
                bail!("Task description must not be empty");
            }
            let budget = args["token_budget"]
                .as_u64()
                .or_else(|| args["budget_tokens"].as_u64())
                .unwrap_or(2000) as usize;
            if budget == 0 {
                bail!("Token budget must be greater than 0");
            }
            let byte_limit = resolve_byte_limit(args);
            let detail_level_str = args["detail_level"].as_str().unwrap_or("");
            let is_compact = args["compact"]
                .as_bool()
                .unwrap_or_else(|| detail_level_str.eq_ignore_ascii_case("compact"));
            let full_blocks = args["full_blocks"]
                .as_bool()
                .or_else(|| args["include_bodies"].as_bool())
                .unwrap_or_else(|| detail_level_str.eq_ignore_ascii_case("full"));
            let details = args["details"]
                .as_bool()
                .or_else(|| args["verbose"].as_bool())
                .unwrap_or(false);

            let adaptive = args["adaptive"].as_bool().unwrap_or(false);
            let bypass_trivial = args["bypass_trivial"].as_bool().unwrap_or(adaptive);
            let no_bypass = args["no_bypass"].as_bool().unwrap_or(false);

            if bypass_trivial && !no_bypass {
                let signals = crate::prompt::classify_task_complexity(task);
                if signals.classification == crate::prompt::TaskComplexity::Trivial {
                    return Ok(json!({
                        "status": "BYPASS_TRIVIAL",
                        "task": task,
                        "classification": "TRIVIAL",
                        "reason": signals.reason,
                        "net_benefit": "NEGATIVE_RETRIEVAL_OVERHEAD_EXCEEDS_SAVINGS",
                        "target_files": [],
                        "governing_rules": [],
                        "cochange_warnings": [],
                        "relevant_decisions": [],
                        "ambiguity_gaps": [],
                        "token_budget": budget,
                        "estimated_tokens": 0,
                        "confidence": "HIGH",
                        "omission_manifest": Value::Null,
                        "next_action": "Task classified as trivial leaf edit. Context retrieval bypassed to preserve token budget. Proceed directly with edits."
                    }));
                }
            }

            let detail_level = if is_compact {
                crate::budget::DetailLevel::Compact
            } else if full_blocks || details {
                crate::budget::DetailLevel::Full
            } else {
                crate::budget::DetailLevel::Contracted
            };

            let subpath = args["subpath"].as_str();
            let language = args["language"].as_str();
            let target_symbols: Option<Vec<String>> = args["symbols"].as_array().map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            });

            let ctx = crate::compiler::context_scoped_optimized(
                &root,
                task,
                budget,
                subpath,
                language,
                detail_level,
                target_symbols.as_deref(),
            )?;

            // Build bounded omission manifest to prevent repository-wide enumeration bloat
            let bounded_omission_manifest = if let Some(ref raw_man) = ctx.omission_manifest {
                let total_omitted = raw_man.omitted_items_count;
                let mut reason_counts: std::collections::BTreeMap<String, usize> =
                    std::collections::BTreeMap::new();
                for entry in &raw_man.omission_manifest {
                    *reason_counts
                        .entry(entry.omission_reason.clone())
                        .or_insert(0) += 1;
                }

                // Sample at most 3 to 5 omitted entries, prioritized by priority_tier ascending
                let sample_limit = if byte_limit <= 4096 { 2 } else { 4 };
                let mut sorted_entries = raw_man.omission_manifest.clone();
                sorted_entries.sort_by_key(|e| e.priority_tier);
                let sample_entries: Vec<_> =
                    sorted_entries.into_iter().take(sample_limit).collect();
                let manifest_truncated = total_omitted > sample_entries.len();
                let unrepresented = total_omitted.saturating_sub(sample_entries.len());

                Some(json!({
                    "budget_status": raw_man.budget_status,
                    "budget_tokens_limit": raw_man.budget_tokens_limit,
                    "budget_tokens_estimated": raw_man.budget_tokens_estimated,
                    "reserved_metadata_tokens": raw_man.reserved_metadata_tokens,
                    "packed_tokens": raw_man.packed_tokens,
                    "remaining_tokens": raw_man.remaining_tokens,
                    "omitted_tokens": raw_man.omitted_tokens,
                    "included_items_count": raw_man.included_items_count,
                    "omitted_items_count": total_omitted,
                    "omission_reasons": reason_counts,
                    "manifest_truncated": manifest_truncated,
                    "unrepresented_omissions_count": unrepresented,
                    "sample_omissions": sample_entries,
                    "omission_manifest": sample_entries,
                    "targeted_retrieval_guidance": "Full repository omission list suppressed to preserve token budget. Use engram_inspect_symbol(target) or targeted engram_get_context(subpath=...) to inspect specific omitted components."
                }))
            } else {
                None
            };

            if details {
                let mut ctx_val = serde_json::to_value(&ctx)?;
                if let Some(ref bounded_man) = bounded_omission_manifest {
                    ctx_val["omission_manifest"] = bounded_man.clone();
                }
                Ok(ctx_val)
            } else {
                let target_files: Vec<Value> = ctx
                    .files
                    .iter()
                    .take(8)
                    .map(|f| {
                        let prov: Vec<String> = f.provenance.iter().take(3).cloned().collect();
                        json!({
                            "path": f.path,
                            "level": f.level,
                            "score": (f.score * 100.0).round() / 100.0,
                            "provenance": prov,
                            "content": f.content,
                        })
                    })
                    .collect();
                let governing_rules: Vec<String> =
                    ctx.constraints.iter().take(5).cloned().collect();
                let cochange_warnings: Vec<String> =
                    ctx.cochange_alerts.iter().take(5).cloned().collect();
                let relevant_decisions: Vec<String> =
                    ctx.relevant_memories.iter().take(5).cloned().collect();
                Ok(json!({
                    "status": "READY",
                    "task": ctx.task,
                    "target_files": target_files,
                    "governing_rules": governing_rules,
                    "cochange_warnings": cochange_warnings,
                    "relevant_decisions": relevant_decisions,
                    "ambiguity_gaps": ctx.gaps,
                    "token_budget": ctx.token_budget,
                    "estimated_tokens": ctx.estimated_tokens,
                    "confidence": ctx.confidence,
                    "omission_manifest": bounded_omission_manifest,
                    "next_action": "Plan changes within target files and run engram_preflight with planned_files."
                }))
            }
        }
        "engram_inspect_symbol" => {
            let target = args["target"]
                .as_str()
                .or_else(|| args["symbol"].as_str())
                .unwrap_or("")
                .trim();
            if target.is_empty() {
                bail!("Target symbol or file must not be empty");
            }
            if target.contains('/') || target.contains('\\') || target.contains('.') {
                validate_workspace_path(&root, target)?;
            }
            let depth = args["depth"].as_u64().unwrap_or(2).clamp(1, 3) as usize;
            let entity_limit = args["max_entities"].as_u64().unwrap_or(10).clamp(1, 50) as usize;
            let byte_limit = args["max_bytes"]
                .as_u64()
                .or_else(|| args["byte_limit"].as_u64())
                .unwrap_or(16_384)
                .clamp(1024, 65536) as usize;

            crate::analyzer::ensure_fresh(&root)?;
            let conn = db(&root)?;
            let budget_opts = crate::impact::ImpactBudgetOptions {
                max_depth: depth,
                max_entities_per_category: entity_limit,
                max_relationships_per_entity: 5,
                prioritize_workspace: true,
            };
            let (cone, mut omissions) =
                crate::impact::compute_impact_cone_with_budget(&conn, &root, target, budget_opts)?;
            let active_contract = crate::contract::get_active_contract(&root).ok().flatten();
            let authorization = active_contract
                .as_ref()
                .map(|c| crate::contract::evaluate_impact_against_contract(c, &cone));

            // Generate bounded explanation: full format bounded by the already pruned cone
            let raw_explanation = crate::impact::format_impact_explanation(&cone);
            let mut explanation = if raw_explanation.len() > 1500 {
                let truncated = safe_truncate_str(&raw_explanation, 1500);
                format!(
                    "{}\n... [Explanation condensed to fit context budget]",
                    truncated
                )
            } else {
                raw_explanation
            };

            let affected_paths = cone.all_affected_paths();
            let mut audit_memories = crate::memory::query_scoped_memories(
                &root,
                target,
                &affected_paths,
                &[target.to_string()],
                true,
            )
            .unwrap_or_default();
            if audit_memories.len() > 5 {
                audit_memories.truncate(5);
            }
            let memory_conflicts = crate::memory::detect_memory_conflicts(&audit_memories);

            let mut code_excerpt = None;
            let mut line_range = None;
            let mut completeness = "NOT_AVAILABLE";
            if let Some(ref tf) = cone.target_file {
                if let Ok(mut s) = conn.prepare("SELECT entity_id, start_line, end_line, signature FROM entity_versions WHERE (name=?1 OR path=?1) AND path=?2 AND invalidated_at_revision IS NULL LIMIT 1") {
                    if let Ok(row) = s.query_row(rusqlite::params![target, tf], |r| Ok((r.get::<_, String>(0)?, r.get::<_, usize>(1)?, r.get::<_, usize>(2)?, r.get::<_, String>(3)?))) {
                        let (ent_id, start_line, end_line, sig) = row;
                        line_range = Some(vec![start_line, end_line]);
                        if let Ok(file_content) = std::fs::read_to_string(root.join(tf)) {
                            let lines: Vec<&str> = file_content.lines().collect();
                            let members = crate::storage::get_entity_members(&conn, &ent_id).unwrap_or_default();
                            let (excerpt_str, comp_str) = crate::analyzer::generate_bounded_excerpt(
                                &lines,
                                start_line,
                                end_line,
                                &members,
                                "typescript",
                            );
                            code_excerpt = Some(format!("// Signature: {}\n{}", sig, excerpt_str));
                            completeness = comp_str;
                        }
                    }
                }
            }

            let mut res_json = json!({
                "target": cone.target_symbol,
                "target_file": cone.target_file,
                "line_range": line_range,
                "code_excerpt": code_excerpt,
                "completeness": completeness,
                "impact_cone": cone,
                "active_contract": active_contract,
                "authorization": authorization,
                "explanation": explanation,
                "memories": audit_memories,
                "memory_conflicts": memory_conflicts,
            });

            // Enforce serialized byte ceiling via structured progressive shedding
            let mut serialized = serde_json::to_string(&res_json)?;
            if serialized.len() > byte_limit {
                // 1. Condense explanation further
                if explanation.len() > 300 {
                    let truncated = safe_truncate_str(&explanation, 300);
                    explanation = format!("{}\n... [Condensed]", truncated);
                    res_json["explanation"] = json!(explanation);
                    serialized = serde_json::to_string(&res_json)?;
                }
                // 2. Shed Tier 5 historical entities
                if serialized.len() > byte_limit {
                    if let Some(cone_obj) = res_json.get_mut("impact_cone") {
                        if let Some(hist) = cone_obj.get_mut("historical_entities") {
                            if let Some(arr) = hist.as_array_mut() {
                                for item in arr.drain(..) {
                                    omissions.push(crate::budget::OmissionEntry {
                                        context_item_id: item["entity_id"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string(),
                                        symbol: None,
                                        file: item["path"].as_str().unwrap_or("").to_string(),
                                        context_type: "HISTORICAL_ENTITY".to_string(),
                                        priority_tier: 5,
                                        original_token_count: 50,
                                        included_token_count: 0,
                                        omitted_token_count: 50,
                                        omission_reason: "Byte limit exceeded; shed Tier 5 entity"
                                            .to_string(),
                                        omission_status: "CAPACITY_PRUNED".to_string(),
                                        retrievable_via_targeted_request: true,
                                    });
                                }
                            }
                        }
                    }
                    serialized = serde_json::to_string(&res_json)?;
                }
                // 3. Shed code_excerpt if oversized
                if serialized.len() > byte_limit {
                    if let Some(exc) = res_json.get_mut("code_excerpt") {
                        if let Some(s) = exc.as_str() {
                            if s.len() > 400 {
                                let truncated = safe_truncate_str(s, 250);
                                *exc = json!(format!(
                                    "{}\n// ... [Excerpt truncated to fit context budget]",
                                    truncated
                                ));
                                res_json["completeness"] = json!("CONTRACTED");
                            }
                        }
                    }
                    serialized = serde_json::to_string(&res_json)?;
                }
                // 4. Shed import_dependent_entities down to 1 if still over limit
                if serialized.len() > byte_limit {
                    if let Some(cone_obj) = res_json.get_mut("impact_cone") {
                        if let Some(deps) = cone_obj.get_mut("import_dependent_entities") {
                            if let Some(arr) = deps.as_array_mut() {
                                if arr.len() > 1 {
                                    for item in arr.drain(1..) {
                                        omissions.push(crate::budget::OmissionEntry {
                                            context_item_id: item["entity_id"].as_str().unwrap_or("").to_string(),
                                            symbol: None,
                                            file: item["path"].as_str().unwrap_or("").to_string(),
                                            context_type: "IMPORT_DEPENDENT_ENTITY".to_string(),
                                            priority_tier: 3,
                                            original_token_count: 50,
                                            included_token_count: 0,
                                            omitted_token_count: 50,
                                            omission_reason: "Byte limit exceeded; pruned excess dependent entities".to_string(),
                                            omission_status: "CAPACITY_PRUNED".to_string(),
                                            retrievable_via_targeted_request: true,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    serialized = serde_json::to_string(&res_json)?;
                }
                // 5. If STILL over limit, truncate explanation and clear code_excerpt
                if serialized.len() > byte_limit {
                    res_json["explanation"] = json!("... [Condensed to fit budget]");
                    res_json["code_excerpt"] = json!(null);
                    serialized = serde_json::to_string(&res_json)?;
                }
            }

            let total_omitted = omissions.len();
            let omitted_tokens = total_omitted * 50;
            // Cap detailed omission entries in payload to prevent manifest itself from bloating the payload
            let max_omissions = if byte_limit <= 4096 { 2 } else { 5 };
            let sample_omissions: Vec<_> = omissions.into_iter().take(max_omissions).collect();
            let manifest = crate::budget::OmissionManifest {
                budget_status: if total_omitted > 0 {
                    "OMISSIONS_RECORDED".to_string()
                } else {
                    "PACKED_WITHIN_BUDGET".to_string()
                },
                budget_tokens_limit: byte_limit / 4,
                budget_tokens_estimated: (serialized.len() + 3) / 4,
                reserved_metadata_tokens: 150,
                packed_tokens: (serialized.len() + 3) / 4,
                remaining_tokens: (byte_limit.saturating_sub(serialized.len()) + 3) / 4,
                omitted_tokens,
                serialized_payload_bytes: serialized.len(),
                included_items_count: cone.total_entities().saturating_sub(total_omitted),
                omitted_items_count: total_omitted,
                omission_manifest: sample_omissions,
            };
            res_json["omission_manifest"] = serde_json::to_value(&manifest)?;
            res_json["budget_bytes_limit"] = json!(byte_limit);

            Ok(res_json)
        }
        "engram_why" => {
            let target = args["target"]
                .as_str()
                .or_else(|| args["symbol"].as_str())
                .unwrap_or("")
                .trim();
            if target.is_empty() {
                bail!("Target symbol or file must not be empty");
            }
            if target.contains('/') || target.contains('\\') || target.contains('.') {
                validate_workspace_path(&root, target)?;
            }
            crate::analyzer::ensure_fresh(&root)?;
            let expl = crate::archaeology::explain_why(&root, target)?;
            Ok(serde_json::to_value(expl)?)
        }
        "engram_get_impact" => {
            let target = args["target"]
                .as_str()
                .or_else(|| args["symbol"].as_str())
                .unwrap_or("")
                .trim();
            if target.is_empty() {
                bail!("Target symbol or file must not be empty");
            }
            if target.contains('/') || target.contains('\\') || target.contains('.') {
                validate_workspace_path(&root, target)?;
            }
            Ok(impact(&root, target)?)
        }
        "engram_get_cochanges" => {
            let file_path = args["file_path"].as_str().unwrap_or("").trim();
            if file_path.is_empty() {
                bail!("file_path must not be empty");
            }
            validate_workspace_path(&root, file_path)?;
            let min_prob = args["min_probability"]
                .as_f64()
                .unwrap_or(0.25)
                .clamp(0.0, 1.0);
            let signals = get_cochanges_for_file(&root, file_path, min_prob)?;
            Ok(serde_json::to_value(signals)?)
        }
        "engram_get_memory" => {
            let query = args["query"].as_str().unwrap_or("");
            let files: Vec<String> = args["files"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();

            for f in &files {
                validate_workspace_path(&root, f)?;
            }

            if query.is_empty() && files.is_empty() {
                let doc = load_durable_memories(&root)?;
                Ok(serde_json::to_value(doc.memories)?)
            } else {
                let memories = query_relevant_memories(&root, query, &files)?;
                Ok(serde_json::to_value(memories)?)
            }
        }
        "engram_record_decision" => {
            let title = args["title"]
                .as_str()
                .or_else(|| args["decision"].as_str())
                .unwrap_or("")
                .trim();
            if title.is_empty() {
                bail!("Decision title must not be empty");
            }
            let statement = args["statement"].as_str().unwrap_or(title).trim();
            if statement.is_empty() {
                bail!("Decision statement must not be empty");
            }
            let reason = args["reason"].as_str().unwrap_or("").trim();
            if reason.is_empty() {
                bail!("Decision reason must not be empty");
            }
            let actor = args["accepted_by"]
                .as_str()
                .or_else(|| args["proposed_by"].as_str())
                .unwrap_or("agent");

            let modules: Vec<String> = args["modules"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let files: Vec<String> = args["files"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();

            for f in &files {
                validate_workspace_path(&root, f)?;
            }

            // Strictly assign PROVISIONAL lifecycle state: agent decisions require human CLI ratification
            let memory = record_provisional_decision(
                &root, title, statement, reason, modules, files, None, actor,
            )?;
            Ok(serde_json::to_value(memory)?)
        }
        "engram_check_constitution" => {
            let conn = db(&root)?;
            let files_arg: Option<Vec<String>> = args["files"].as_array().map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            });

            let changed_files = if let Some(files) = files_arg {
                for f in &files {
                    validate_workspace_path(&root, f)?;
                }
                files
            } else {
                crate::storage::git_changed(&root)
            };

            let violations = evaluate_rules(&root, &changed_files, &conn)?;
            Ok(serde_json::to_value(violations)?)
        }
        "engram_get_ecosystem" => {
            let report = crate::manifest::detect_ecosystem(&root)?;
            Ok(serde_json::to_value(report)?)
        }
        "engram_doctor" => {
            let report = crate::doctor::diagnose_memory_health(&root)?;
            Ok(serde_json::to_value(report)?)
        }
        "engram_preflight" => {
            let task = args["task"].as_str().unwrap_or("").trim();
            if task.is_empty() {
                bail!("Task must not be empty for engram_preflight");
            }
            let budget = args["token_budget"].as_u64().unwrap_or(1200) as usize;
            let forbidden_raw: Option<Vec<String>> = args["forbidden"].as_array().map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            });

            let forbidden = if let Some(fb) = forbidden_raw {
                for p in &fb {
                    validate_workspace_path(&root, p)
                        .map_err(|e| anyhow!("Forbidden boundary validation failed: {}", e))?;
                }
                Some(fb)
            } else {
                None
            };

            let planned_files: Option<Vec<String>> = args["planned_files"].as_array().map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            });

            if let Some(ref pf) = planned_files {
                for p in pf {
                    validate_workspace_path(&root, p)
                        .map_err(|e| anyhow!("Planned file validation failed: {}", e))?;
                }
            }

            let details = args["details"]
                .as_bool()
                .or_else(|| args["verbose"].as_bool())
                .unwrap_or(false);

            let opts = crate::flywheel::PreflightOptions {
                task: task.to_string(),
                token_budget: budget,
                forbidden_boundaries: forbidden,
                planned_files,
            };
            let res = crate::flywheel::preflight_task(&root, &opts)?;

            if details {
                Ok(serde_json::to_value(res)?)
            } else {
                let governing_rule_ids: Vec<String> = res
                    .governing_policies
                    .iter()
                    .map(|p| format!("{}: {}", p.id, p.title))
                    .collect();
                let status_str = match res.status {
                    crate::flywheel::PreflightStatus::ReadyForImplementation => {
                        "READY_FOR_IMPLEMENTATION"
                    }
                    crate::flywheel::PreflightStatus::Blocked => "BLOCKED",
                };
                let blocking_violations = if res.status == crate::flywheel::PreflightStatus::Blocked
                {
                    res.ambiguity_gaps.clone()
                } else {
                    Vec::new()
                };
                let next_action = if res.status == crate::flywheel::PreflightStatus::Blocked {
                    "Reconcile blocking violations before modifying code or request human override."
                } else if res.is_reused_contract {
                    "Valid Change Contract active. Proceed to edit expected files."
                } else {
                    "Change Contract locked. Proceed to edit expected files. Re-run preflight with planned_files if new files are needed."
                };

                Ok(json!({
                    "status": status_str,
                    "contract_id": res.contract_id,
                    "contract_version": res.contract_version,
                    "task_goal": res.task_goal,
                    "expected_files": res.expected_files,
                    "expected_modules": res.expected_modules,
                    "forbidden_boundaries": res.forbidden_boundaries,
                    "governing_rules": governing_rule_ids,
                    "blocking_violations": blocking_violations,
                    "is_reused_contract": res.is_reused_contract,
                    "reused_reason": res.reused_reason,
                    "next_action": next_action,
                }))
            }
        }
        "engram_postflight" => {
            let contract_id = args["contract_id"].as_str().map(str::to_string);
            let strict = args["strict"].as_bool().unwrap_or(false);
            let staged = args["staged"].as_bool().unwrap_or(false);
            let test_evidence = args["test_evidence"]
                .as_bool()
                .or_else(|| args["test_passed"].as_bool());
            let build_evidence = args["build_evidence"]
                .as_bool()
                .or_else(|| args["build_passed"].as_bool());
            let details = args["details"]
                .as_bool()
                .or_else(|| args["verbose"].as_bool())
                .unwrap_or(false);

            let opts = crate::flywheel::LocalPostflightOptions {
                contract_id,
                staged,
                strict,
                test_evidence,
                build_evidence,
            };
            let res = crate::flywheel::validate_local_postflight(&root, &opts)?;

            let verdict_str = match res.verdict {
                crate::flywheel::LocalPostflightVerdict::Pass => "PASS",
                crate::flywheel::LocalPostflightVerdict::PassScopeOnlyUnverified => {
                    "PASS_SCOPE_ONLY_UNVERIFIED"
                }
                crate::flywheel::LocalPostflightVerdict::BlockedScopeViolation => {
                    "BLOCKED_SCOPE_VIOLATION"
                }
                crate::flywheel::LocalPostflightVerdict::BlockedConstitutionRule => {
                    "BLOCKED_CONSTITUTION_RULE"
                }
                crate::flywheel::LocalPostflightVerdict::BlockedValidationFailed => {
                    "BLOCKED_VALIDATION_FAILED"
                }
                crate::flywheel::LocalPostflightVerdict::BlockedStaleContract => {
                    "BLOCKED_STALE_CONTRACT"
                }
                crate::flywheel::LocalPostflightVerdict::IndeterminateGitUnavailable => {
                    "INDETERMINATE_GIT_UNAVAILABLE"
                }
            };

            let mut violations: Vec<Value> = Vec::new();
            for bv in &res.boundary_violations {
                violations.push(json!({
                    "path": bv,
                    "category": "BoundaryViolation",
                    "reason": "Forbidden file modified",
                }));
            }
            for uv in &res.unexpected_files {
                violations.push(json!({
                    "path": uv,
                    "category": "Unexpected",
                    "reason": "File modified outside expected contract boundary",
                }));
            }
            for cv in &res.constitution_violations {
                violations.push(json!({
                    "rule": cv,
                    "category": "ConstitutionViolation",
                    "reason": "Architectural constitution rule violation",
                }));
            }

            let next_action = match res.verdict {
                crate::flywheel::LocalPostflightVerdict::Pass => {
                    "All changes compliant with Change Contract and verified by validation evidence. Ready to commit / create PR."
                }
                crate::flywheel::LocalPostflightVerdict::PassScopeOnlyUnverified => {
                    "All changes compliant with Change Contract boundaries (scope only). Test evidence was not supplied. Run tests before committing."
                }
                crate::flywheel::LocalPostflightVerdict::BlockedValidationFailed => {
                    "Validation tests explicitly failed. Resolve failing tests before committing code."
                }
                crate::flywheel::LocalPostflightVerdict::BlockedStaleContract => {
                    "Contract baseline diverged from Git HEAD. Run engram_prepare_task to re-ground and rebase the active contract."
                }
                crate::flywheel::LocalPostflightVerdict::BlockedScopeViolation => {
                    "Revert violating/unexpected files or invoke engram_recover for structured remediation."
                }
                crate::flywheel::LocalPostflightVerdict::BlockedConstitutionRule => {
                    "Reconcile architectural constitution rule violations before modifying code or request human override."
                }
                crate::flywheel::LocalPostflightVerdict::IndeterminateGitUnavailable => {
                    "Ensure git repository is initialized and accessible."
                }
            };

            let is_compliant = res.scope_status == crate::flywheel::LocalScopeStatus::Compliant;

            if details {
                Ok(json!({
                    "verdict": if res.verdict == crate::flywheel::LocalPostflightVerdict::PassScopeOnlyUnverified {
                        "PASS"
                    } else {
                        verdict_str
                    },
                    "local_verdict": verdict_str,
                    "contract_id": res.contract_id,
                    "baseline_commit": res.baseline_commit,
                    "current_head": res.current_head,
                    "status": if is_compliant { "COMPLIANT" } else { "VIOLATION_DETECTED" },
                    "scope_status": serde_json::to_value(res.scope_status)?,
                    "evidence_status": serde_json::to_value(res.evidence_status)?,
                    "staleness_status": serde_json::to_value(res.staleness_status)?,
                    "total_changed_files": res.changed_files.len(),
                    "changed_files": res.changed_files,
                    "expected_count": res.expected_files.len(),
                    "expected_files": res.expected_files,
                    "unexpected_count": res.unexpected_files.len(),
                    "unexpected_files": res.unexpected_files,
                    "boundary_violation_count": res.boundary_violations.len(),
                    "boundary_violations": res.boundary_violations,
                    "constitution_violations": res.constitution_violations,
                    "violations": violations,
                    "constitution_passed": res.constitution_violations.is_empty(),
                    "summary": res.summary,
                    "exit_code": res.exit_code,
                    "next_action": next_action,
                    "report": serde_json::to_value(&res)?,
                }))
            } else {
                Ok(json!({
                    "verdict": if res.verdict == crate::flywheel::LocalPostflightVerdict::PassScopeOnlyUnverified {
                        "PASS"
                    } else {
                        verdict_str
                    },
                    "contract_id": res.contract_id,
                    "status": if is_compliant { "COMPLIANT" } else { "VIOLATION_DETECTED" },
                    "scope_status": serde_json::to_value(res.scope_status)?,
                    "evidence_status": serde_json::to_value(res.evidence_status)?,
                    "staleness_status": serde_json::to_value(res.staleness_status)?,
                    "total_changed_files": res.changed_files.len(),
                    "expected_count": res.expected_files.len(),
                    "unexpected_count": res.unexpected_files.len(),
                    "boundary_violation_count": res.boundary_violations.len(),
                    "violations": violations,
                    "constitution_passed": res.constitution_violations.is_empty(),
                    "summary": res.summary,
                    "next_action": next_action,
                }))
            }
        }
        "engram_recover" => {
            let contract_id = args["contract_id"]
                .as_str()
                .or_else(|| args["id"].as_str())
                .unwrap_or("")
                .trim();
            let id = if contract_id.is_empty() {
                crate::contract::get_active_contract(&root)?
                    .map(|c| c.contract_id)
                    .ok_or_else(|| {
                        anyhow!("No contract_id specified and no active contract found")
                    })?
            } else {
                contract_id.to_string()
            };
            let rep = crate::recovery::recover_contract(&root, &id)?;
            Ok(serde_json::to_value(rep)?)
        }
        "engram_drift" => {
            let module = args["module"].as_str().map(str::to_string);
            let decision = args["decision"]
                .as_str()
                .or_else(|| args["decision_id"].as_str())
                .map(str::to_string);
            let since = args["since"]
                .as_str()
                .or_else(|| args["since_commit"].as_str())
                .map(str::to_string);
            let threshold = args["staleness_threshold"]
                .as_u64()
                .or_else(|| args["threshold"].as_u64())
                .unwrap_or(crate::drift::DEFAULT_STALENESS_THRESHOLD as u64)
                .max(1) as usize;

            let opts = crate::drift::DriftOptions {
                module,
                decision,
                since,
                staleness_threshold: threshold,
            };
            let rep = crate::drift::analyze_drift(&root, &opts)?;
            Ok(serde_json::to_value(rep)?)
        }
        "engram_ratify_decision" => {
            let decision_id = args["decision_id"]
                .as_str()
                .ok_or_else(|| anyhow!("Missing required 'decision_id'"))?;
            let by = args["by"].as_str().unwrap_or("developer");
            let reason = args["reason"]
                .as_str()
                .unwrap_or("Human ratified architectural decision");
            let commit = args["commit"].as_str();

            let mem = crate::memory::ratify_decision(&root, decision_id, by, reason, commit)?;
            Ok(serde_json::to_value(mem)?)
        }
        _ => Err(anyhow!("Unknown tool: {name}")),
    }
}

pub fn mcp_call(root: &Path, name: &str, args: &Value) -> Result<Value> {
    let byte_limit = resolve_byte_limit(args);
    let payload = mcp_call_raw(root, name, args)?;
    Ok(enforce_mcp_envelope_boundary(
        name,
        payload,
        byte_limit,
        &json!(1),
    ))
}

pub fn core_tools_definitions() -> Value {
    json!([
        {
            "name": "engram_engage",
            "description": "Consolidated single-shot engagement: bind repository baseline, compile context, evaluate constitution, lock Change Contract, and receive execution guidance.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task": { "type": "string", "description": "The coding task goal, user prompt, or intent" },
                    "token_budget": { "type": "integer", "minimum": 100, "maximum": 32000, "description": "Context token budget limit (default 1200)" },
                    "planned_files": { "type": "array", "items": { "type": "string" }, "description": "Explicit list of relative workspace file paths the agent intends to edit" },
                    "forbidden": { "type": "array", "items": { "type": "string" }, "description": "Explicit forbidden boundary patterns beyond the Project Constitution" },
                    "symbols": { "type": "array", "items": { "type": "string" }, "description": "Targeted symbol filter: prioritize excerpting these specific functions, classes, or types" },
                    "subpath": { "type": "string", "description": "Optional repository subdirectory scope to restrict AST traversal" },
                    "language": { "type": "string", "description": "Optional programming language filter" },
                    "detail_level": { "type": "string", "enum": ["compact", "standard", "full"], "description": "Level of excerpt detail: compact, standard, or full (default standard)" },
                    "no_bypass": { "type": "boolean", "description": "Force Change Contract locking and AST compilation even if task qualifies as trivial" },
                    "inject_instructions": { "type": "boolean", "description": "Also inject Turn-0 context and directives into workspace agent instruction files" },
                    "max_bytes": { "type": "integer", "minimum": 1024, "maximum": 65536, "description": "Maximum serialized response size in bytes (default 16384)" }
                },
                "required": ["task"]
            }
        },
        {
            "name": "engram_get_context",
            "description": "Compile focused context with durable memories and co-change alerts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task": { "type": "string", "description": "The coding task goal or intent" },
                    "token_budget": { "type": "integer", "description": "Context token budget limit" },
                    "details": { "type": "boolean", "description": "Return full verbose excerpts" },
                    "compact": { "type": "boolean", "description": "Progressive disclosure: return compact symbol signatures only (no excerpt bodies)" },
                    "detail_level": { "type": "string", "enum": ["compact", "contracted", "full"], "description": "Level of detail: compact (signatures), contracted (bounded), or full" },
                    "symbols": { "type": "array", "items": { "type": "string" }, "description": "Targeted symbol filter: excerpt only these specific symbols" },
                    "adaptive": { "type": "boolean", "description": "Enable adaptive small-task bypass to save tokens on self-contained tasks" },
                    "max_bytes": { "type": "integer", "minimum": 1024, "maximum": 65536, "description": "Maximum serialized response size in bytes (default 16384)" }
                },
                "required": ["task"]
            }
        },
        {
            "name": "engram_inspect_symbol",
            "description": "Query blast radius impact, contract authorization, and memory lookup.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Target symbol or file path" },
                    "depth": { "type": "integer", "description": "Dependency depth (1-3, default 2)" },
                    "max_entities": { "type": "integer", "description": "Maximum entities per category (default 10)" },
                    "max_bytes": { "type": "integer", "description": "Maximum response size in bytes (default 16384)" }
                },
                "required": ["target"]
            }
        },
        {
            "name": "engram_preflight",
            "description": "Pre-flight handshake: compile context, check blast radius, lock Change Contract.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task": { "type": "string", "description": "The coding task goal or intent" },
                    "token_budget": { "type": "integer", "description": "Context token budget limit" },
                    "forbidden": { "type": "array", "items": { "type": "string" }, "description": "Optional forbidden boundary patterns" },
                    "planned_files": { "type": "array", "items": { "type": "string" }, "description": "Planned target files for validation" },
                    "max_bytes": { "type": "integer", "minimum": 1024, "maximum": 65536, "description": "Maximum serialized response size in bytes (default 16384)" }
                },
                "required": ["task"]
            }
        },
        {
            "name": "engram_postflight",
            "description": "Post-flight guard check: evaluate mutation against Change Contract and tests.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "contract_id": { "type": "string", "description": "Active Change Contract ID" },
                    "strict": { "type": "boolean", "description": "Fail if unexpected files modified" },
                    "test_evidence": { "type": "boolean", "description": "Whether validation tests passed" },
                    "max_bytes": { "type": "integer", "minimum": 1024, "maximum": 65536, "description": "Maximum serialized response size in bytes (default 16384)" }
                }
            }
        }
    ])
}

pub fn all_tools_definitions() -> Value {
    json!([
        {
            "name": "engram_engage",
            "description": "Consolidated single-shot engagement: bind repository baseline, compile context, evaluate constitution, lock Change Contract, and receive execution guidance.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task": { "type": "string", "description": "The coding task goal, user prompt, or intent" },
                    "token_budget": { "type": "integer", "minimum": 100, "maximum": 32000, "description": "Context token budget limit (default 1200)" },
                    "planned_files": { "type": "array", "items": { "type": "string" }, "description": "Explicit list of relative workspace file paths the agent intends to edit" },
                    "forbidden": { "type": "array", "items": { "type": "string" }, "description": "Explicit forbidden boundary patterns beyond the Project Constitution" },
                    "symbols": { "type": "array", "items": { "type": "string" }, "description": "Targeted symbol filter: prioritize excerpting these specific functions, classes, or types" },
                    "subpath": { "type": "string", "description": "Optional repository subdirectory scope to restrict AST traversal" },
                    "language": { "type": "string", "description": "Optional programming language filter" },
                    "detail_level": { "type": "string", "enum": ["compact", "standard", "full"], "description": "Level of excerpt detail: compact, standard, or full (default standard)" },
                    "no_bypass": { "type": "boolean", "description": "Force Change Contract locking and AST compilation even if task qualifies as trivial" },
                    "inject_instructions": { "type": "boolean", "description": "Also inject Turn-0 context and directives into workspace agent instruction files" },
                    "max_bytes": { "type": "integer", "minimum": 1024, "maximum": 65536, "description": "Maximum serialized response size in bytes (default 16384)" }
                },
                "required": ["task"]
            }
        },
        {
            "name": "engram_prepare_task",
            "description": "Single-shot task preparation: compile focused context, discover governing rules, validate boundaries, and lock Change Contract in one atomic round-trip.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task": { "type": "string", "description": "The coding task goal or intent" },
                    "token_budget": { "type": "integer", "minimum": 1, "description": "Context token budget limit (default 1200)" },
                    "planned_files": { "type": "array", "items": { "type": "string" }, "description": "Optional list of planned target files; auto-seeded from top context files if omitted" },
                    "forbidden": { "type": "array", "items": { "type": "string" }, "description": "Optional forbidden boundary patterns" },
                    "no_bypass": { "type": "boolean", "description": "Force Change Contract creation and preflight even for trivial tasks (default false)" },
                    "details": { "type": "boolean", "description": "Return full verbose payload with complete context and preflight details (default false for compact token-efficient payload)" }
                },
                "required": ["task"]
            }
        },
        {
            "name": "engram_get_context",
            "description": "Compile focused, evidence-backed context with durable memories and co-change alerts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task": { "type": "string", "description": "The coding task goal or intent" },
                    "token_budget": { "type": "integer", "minimum": 1, "description": "Maximum token budget limit" },
                    "details": { "type": "boolean", "description": "Return full verbose payload including complete file excerpts and ecosystem dependencies (default false for compact token-efficient payload)" }
                },
                "required": ["task"]
            }
        },
        {
            "name": "engram_inspect_symbol",
            "description": "Multi-hop blast radius impact query, Change Contract authorization check, and scoped memory lookup.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Target symbol or file path to inspect" },
                    "depth": { "type": "integer", "minimum": 1, "maximum": 3, "description": "Dependency traversal depth (default 2)" },
                    "max_entities": { "type": "integer", "minimum": 1, "maximum": 50, "description": "Maximum entities per category (default 10)" },
                    "max_bytes": { "type": "integer", "minimum": 1024, "maximum": 65536, "description": "Maximum response size in bytes (default 16384)" }
                },
                "required": ["target"]
            }
        },
        {
            "name": "engram_why",
            "description": "Architectural archaeology: explain rationale, governing decisions, and commit history for a symbol or file.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Symbol name or relative file path" }
                },
                "required": ["target"]
            }
        },
        {
            "name": "engram_get_impact",
            "description": "Fast 1-hop reverse dependency and blast radius impact query for a target symbol or file.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Symbol name or relative file path" }
                },
                "required": ["target"]
            }
        },
        {
            "name": "engram_get_cochanges",
            "description": "Retrieve historical Git co-change coupling signals and statistics for a given file.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Relative file path within repository" },
                    "min_probability": { "type": "number", "minimum": 0.0, "maximum": 1.0, "description": "Minimum co-change probability threshold" }
                },
                "required": ["file_path"]
            }
        },
        {
            "name": "engram_get_memory",
            "description": "Retrieve durable architectural decisions, policies, and institutional engineering memories.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Semantic query filter" },
                    "files": { "type": "array", "items": { "type": "string" }, "description": "Scope files to match" }
                }
            }
        },
        {
            "name": "engram_record_decision",
            "description": "Propose an architectural decision into engineering memory (marked PROVISIONAL until human CLI ratification).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Short title of the architectural decision" },
                    "statement": { "type": "string", "description": "The exact decision policy or architectural invariant" },
                    "reason": { "type": "string", "description": "Underlying rationale or trade-off explanation" },
                    "modules": { "type": "array", "items": { "type": "string" }, "description": "Architectural modules governed by this decision" },
                    "files": { "type": "array", "items": { "type": "string" }, "description": "Specific files governed by this decision" },
                    "accepted_by": { "type": "string", "description": "Agent or engineer proposing this decision" }
                },
                "required": ["title", "statement", "reason"]
            }
        },
        {
            "name": "engram_check_constitution",
            "description": "Evaluate architectural constitution rules and boundary isolation policies against changed files.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "files": { "type": "array", "items": { "type": "string" }, "description": "Specific file paths to check (defaults to git diff)" }
                }
            }
        },
        {
            "name": "engram_get_ecosystem",
            "description": "Detect workspace ecosystem, primary language, manifest dependencies, build tools, and framework rules.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "engram_doctor",
            "description": "Run diagnostic health checks on memory storage, index integrity, and append-only cryptographic chains.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "engram_preflight",
            "description": "Orchestrated Pre-Flight Handshake: compile context, predict blast radius, discover governing decisions, and lock Change Contract.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task": { "type": "string", "description": "The coding task goal or intent" },
                    "token_budget": { "type": "integer", "description": "Context token budget limit (default 1200)" },
                    "forbidden": { "type": "array", "items": { "type": "string" }, "description": "Optional forbidden boundary patterns" },
                    "planned_files": { "type": "array", "items": { "type": "string" }, "description": "List of planned target files for incremental delta validation" },
                    "details": { "type": "boolean", "description": "Return full verbose payload with complete impact trees and policy rationale (default false for compact token-efficient payload)" }
                },
                "required": ["task"]
            }
        },
        {
            "name": "engram_postflight",
            "description": "Deterministic Local Post-Flight Guard Check: evaluate mutation against Change Contract boundaries, constitution rules, git diffs, and test evidence without external LLM dependencies.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "contract_id": { "type": "string", "description": "Active Change Contract ID to validate against (defaults to active contract)" },
                    "strict": { "type": "boolean", "description": "Fail if unexpected files are modified outside expected boundaries" },
                    "staged": { "type": "boolean", "description": "Validate git staged index instead of entire working tree" },
                    "test_evidence": { "type": "boolean", "description": "Whether validation tests passed (true/false, optional)" },
                    "build_evidence": { "type": "boolean", "description": "Whether compilation/build succeeded (true/false, optional)" },
                    "details": { "type": "boolean", "description": "Return full verbose payload with all classification objects (default false for compact token-efficient payload)" }
                }
            }
        },
        {
            "name": "engram_recover",
            "description": "Formulate structured, non-destructive recovery recipe for contract violations and check exhaustion gate.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "contract_id": { "type": "string", "description": "Change Contract ID to formulate recovery plan for" }
                }
            }
        },
        {
            "name": "engram_drift",
            "description": "Analyze architectural decision drift, boundary erosion, and scoped staleness across commits.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "module": { "type": "string", "description": "Filter by module name" },
                    "decision": { "type": "string", "description": "Filter by decision ID" },
                    "since": { "type": "string", "description": "Analyze commits since this commit hash" },
                    "staleness_threshold": { "type": "integer", "description": "Configured mutation threshold for STALE_REVIEW (default 5)" }
                }
            }
        },
        {
            "name": "engram_ratify_decision",
            "description": "Explicitly ratify an architectural decision with human reviewer authorization and provenance tracking.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "decision_id": { "type": "string", "description": "The decision ID to ratify" },
                    "by": { "type": "string", "description": "Reviewer identity or authorizing human" },
                    "reason": { "type": "string", "description": "Ratification rationale or justification" },
                    "commit": { "type": "string", "description": "Git commit SHA being ratified (defaults to HEAD)" }
                },
                "required": ["decision_id"]
            }
        }
    ])
}

pub fn tools_definitions() -> Value {
    let show_all = std::env::var("ENGRAM_SPECIALIST_TOOLS")
        .or_else(|_| std::env::var("ENGRAM_ALL_TOOLS"))
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if show_all {
        all_tools_definitions()
    } else {
        core_tools_definitions()
    }
}

pub fn resources_definitions() -> Value {
    json!([
        {
            "uri": "engram://memories",
            "name": "Durable Engineering Memories",
            "description": "Durable architectural decisions, rationale, and institutional memory",
            "mimeType": "text/markdown"
        },
        {
            "uri": "engram://constitution",
            "name": "Architecture Constitution & Rules",
            "description": "Architectural constraints, boundary definitions, and validation rules",
            "mimeType": "application/json"
        },
        {
            "uri": "engram://contract/active",
            "name": "Active Change Contract",
            "description": "Currently active change contract governing pre-flight boundary enforcement",
            "mimeType": "application/json"
        },
        {
            "uri": "engram://ecosystem",
            "name": "Ecosystem & Framework Guard",
            "description": "Detected project language, framework, dependencies, and ecosystem rules",
            "mimeType": "application/json"
        }
    ])
}

pub fn prompts_definitions() -> Value {
    json!([
        {
            "name": "engram_preflight_handshake",
            "description": "Orchestrated Pre-Flight Handshake: compile context, discover architectural decisions, predict blast radius, and lock Change Contract before code edits.",
            "arguments": [
                {
                    "name": "task",
                    "description": "The coding task goal or intent",
                    "required": true
                }
            ]
        },
        {
            "name": "engram_postflight_audit",
            "description": "Orchestrated Post-Flight Guard Check: evaluate changes against Change Contract boundaries, enforce zero poisoning, and generate candidate memories.",
            "arguments": [
                {
                    "name": "contract_id",
                    "description": "The active Change Contract ID to audit against",
                    "required": true
                }
            ]
        }
    ])
}

fn mcp_read_resource(root: &Path, uri: &str) -> std::result::Result<Value, Value> {
    match uri {
        "engram://memories" => {
            let doc = crate::memory::load_durable_memories(root).unwrap_or_default();
            let mut text = String::from("# Durable Engineering Memories\n\n");
            if doc.memories.is_empty() {
                text.push_str("No durable memories recorded.\n");
            } else {
                for m in &doc.memories {
                    text.push_str(&format!(
                        "## [{}] {}\n- **State:** {:?}\n- **Scope:** {:?} ({})\n- **Statement:** {}\n- **Reason:** {}\n- **Accepted By:** {} ({})\n\n",
                        m.id,
                        m.title,
                        m.lifecycle_state,
                        m.scope.level,
                        m.scope.target.as_deref().unwrap_or("workspace"),
                        m.statement,
                        m.reason,
                        m.accepted_by,
                        m.accepted_at.to_rfc3339()
                    ));
                }
            }
            Ok(json!({
                "contents": [{
                    "uri": "engram://memories",
                    "mimeType": "text/markdown",
                    "text": text
                }]
            }))
        }
        "engram://constitution" => {
            let rules = crate::constitution::load_constitution(root).unwrap_or_default();
            let text = serde_json::to_string_pretty(&rules).unwrap_or_else(|_| "[]".to_string());
            Ok(json!({
                "contents": [{
                    "uri": "engram://constitution",
                    "mimeType": "application/json",
                    "text": text
                }]
            }))
        }
        "engram://contract/active" => {
            let active = crate::contract::get_active_contract(root).ok().flatten();
            let text = match active {
                Some(contract) => {
                    serde_json::to_string_pretty(&contract).unwrap_or_else(|_| "{}".to_string())
                }
                None => serde_json::to_string_pretty(&json!({ "status": "NO_ACTIVE_CONTRACT" }))
                    .unwrap(),
            };
            Ok(json!({
                "contents": [{
                    "uri": "engram://contract/active",
                    "mimeType": "application/json",
                    "text": text
                }]
            }))
        }
        "engram://ecosystem" => {
            let report = crate::manifest::detect_ecosystem(root).unwrap_or_default();
            let text = serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string());
            Ok(json!({
                "contents": [{
                    "uri": "engram://ecosystem",
                    "mimeType": "application/json",
                    "text": text
                }]
            }))
        }
        _ => Err(json!({
            "code": -32602,
            "message": format!("Invalid params: Unknown resource URI '{uri}'")
        })),
    }
}

fn mcp_get_prompt(name: &str, args: &Value) -> std::result::Result<Value, Value> {
    match name {
        "engram_preflight_handshake" => {
            let task = args["task"].as_str().unwrap_or("<TASK_DESCRIPTION>");
            Ok(json!({
                "description": "Orchestrated Pre-Flight Handshake for Engram",
                "messages": [{
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": format!(
                            "Before modifying any files for task: \"{}\", invoke engram_preflight to inspect symbol blast radius, review governing architectural decisions, and lock a Change Contract. Obey all boundary constraints.",
                            task
                        )
                    }
                }]
            }))
        }
        "engram_postflight_audit" => {
            let contract_id = args["contract_id"]
                .as_str()
                .unwrap_or("<ACTIVE_CONTRACT_ID>");
            Ok(json!({
                "description": "Orchestrated Post-Flight Guard Check for Engram",
                "messages": [{
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": format!(
                            "Code modifications complete. Invoke engram_postflight with contract_id: \"{}\" to verify changes against Change Contract boundaries, detect constitution violations, and synthesize candidate memories.",
                            contract_id
                        )
                    }
                }]
            }))
        }
        _ => Err(json!({
            "code": -32602,
            "message": format!("Invalid params: Unknown prompt '{name}'")
        })),
    }
}

pub fn mcp(root: &Path) -> Result<()> {
    let root = root_path(root)?;
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                write_message(
                    &mut stdout,
                    &json!({
                        "jsonrpc": "2.0",
                        "id": null,
                        "error": { "code": -32700, "message": e.to_string() }
                    }),
                )?;
                continue;
            }
        };

        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let has_id = !id.is_null();

        if !req.is_object() {
            if has_id {
                write_message(
                    &mut stdout,
                    &json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32600, "message": "Invalid Request: expected JSON object" }
                    }),
                )?;
            }
            continue;
        }

        let method = match req.get("method").and_then(|m| m.as_str()) {
            Some(m) => m,
            None => {
                if has_id {
                    write_message(
                        &mut stdout,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": -32600, "message": "Invalid Request: missing or invalid 'method'" }
                        }),
                    )?;
                }
                continue;
            }
        };

        if method == "notifications/initialized" {
            continue;
        }

        let resp_result: std::result::Result<Value, Value> = match method {
            "initialize" => Ok(json!({
                "protocolVersion": req["params"]["protocolVersion"].as_str().unwrap_or("2024-11-05"),
                "capabilities": {
                    "tools": {},
                    "resources": {},
                    "prompts": {}
                },
                "serverInfo": { "name": "engram", "version": VERSION }
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({
                "tools": tools_definitions()
            })),
            "tools/call" => {
                let name = req["params"]["name"].as_str().unwrap_or("");
                let args = &req["params"]["arguments"];
                let byte_limit = resolve_byte_limit(args);
                let tool_res = match mcp_call_raw(&root, name, args) {
                    Ok(payload) => enforce_mcp_envelope_boundary(name, payload, byte_limit, &id),
                    Err(e) => tool_error(e.to_string()),
                };
                Ok(tool_res)
            }
            "resources/list" => Ok(json!({
                "resources": resources_definitions()
            })),
            "resources/read" => {
                let uri = req["params"]["uri"].as_str().unwrap_or("");
                mcp_read_resource(&root, uri)
            }
            "prompts/list" => Ok(json!({
                "prompts": prompts_definitions()
            })),
            "prompts/get" => {
                let name = req["params"]["name"].as_str().unwrap_or("");
                let args = &req["params"]["arguments"];
                mcp_get_prompt(name, args)
            }
            _ => {
                if has_id {
                    write_message(
                        &mut stdout,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": -32601, "message": format!("Method not found: {}", method) }
                        }),
                    )?;
                }
                continue;
            }
        };

        if has_id {
            let response = match resp_result {
                Ok(v) => json!({ "jsonrpc": "2.0", "id": id, "result": v }),
                Err(err_obj) => json!({ "jsonrpc": "2.0", "id": id, "error": err_obj }),
            };
            write_message(&mut stdout, &response)?;
        }
    }
    Ok(())
}

pub fn write_message(out: &mut impl Write, v: &Value) -> Result<()> {
    serde_json::to_writer(&mut *out, v)?;
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}
