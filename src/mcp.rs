//! MCP Server implementation: stdio JSON-RPC 2.0 protocol exposing engineering intelligence.

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
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

pub fn mcp_call(root: &Path, name: &str, args: &Value) -> Result<Value> {
    let root = root_path(root)?;
    match name {
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

            let planned_files_raw: Option<Vec<String>> = args["planned_files"].as_array().map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            });

            if let Some(ref pf) = planned_files_raw {
                for p in pf {
                    validate_workspace_path(&root, p)
                        .map_err(|e| anyhow!("Planned file validation failed: {}", e))?;
                }
            }

            // Adaptive bypass evaluation for small/trivial leaf tasks
            let signals = crate::prompt::classify_task_complexity(task);
            if signals.classification == crate::prompt::TaskComplexity::Trivial && !no_bypass {
                return Ok(json_result(json!({
                    "status": "BYPASS_TRIVIAL",
                    "task": task,
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
                })));
            }

            // 1. Context compilation
            let ctx = context(&root, task, budget)?;

            // 2. Resolve planned files: if not explicitly supplied, seed from top context files (up to 5)
            let mut effective_planned = planned_files_raw.unwrap_or_default();
            if effective_planned.is_empty() {
                for f in ctx.files.iter().take(5) {
                    if !effective_planned.contains(&f.path) {
                        effective_planned.push(f.path.clone());
                    }
                }
            }

            // 3. Preflight handshake and contract locking
            let preflight_opts = crate::flywheel::PreflightOptions {
                task: task.to_string(),
                token_budget: budget,
                forbidden_boundaries: forbidden,
                planned_files: if effective_planned.is_empty() {
                    None
                } else {
                    Some(effective_planned)
                },
            };
            let preflight_res = crate::flywheel::preflight_task(&root, &preflight_opts)?;

            // 4. Return structured response
            if details {
                Ok(json_result(json!({
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
                })))
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
                let blocking_violations = if preflight_res.status == crate::flywheel::PreflightStatus::Blocked {
                    preflight_res.ambiguity_gaps.clone()
                } else {
                    Vec::new()
                };
                let next_action = if preflight_res.status == crate::flywheel::PreflightStatus::Blocked {
                    "Reconcile blocking violations before modifying code or request human override."
                } else if preflight_res.is_reused_contract {
                    "Valid Change Contract active. Proceed to edit expected files."
                } else {
                    "Change Contract locked. Proceed to edit expected files. Re-run engram_prepare_task if new files are needed."
                };

                Ok(json_result(json!({
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
                })))
            }
        }
        "engram_get_context" => {
            let task = args["task"].as_str().unwrap_or("").trim();
            if task.is_empty() {
                bail!("Task description must not be empty");
            }
            let budget = args["token_budget"].as_u64().unwrap_or(1200) as usize;
            if budget == 0 {
                bail!("Token budget must be greater than 0");
            }
            let details = args["details"]
                .as_bool()
                .or_else(|| args["verbose"].as_bool())
                .unwrap_or(false);
            let ctx = context(&root, task, budget)?;
            if details {
                Ok(json_result(serde_json::to_value(ctx)?))
            } else {
                let target_files: Vec<Value> = ctx
                    .files
                    .iter()
                    .map(|f| {
                        json!({
                            "path": f.path,
                            "level": f.level,
                            "score": (f.score * 100.0).round() / 100.0,
                        })
                    })
                    .collect();
                let governing_rules: Vec<String> =
                    ctx.constraints.iter().take(5).cloned().collect();
                let cochange_warnings: Vec<String> =
                    ctx.cochange_alerts.iter().take(5).cloned().collect();
                let relevant_decisions: Vec<String> =
                    ctx.relevant_memories.iter().take(5).cloned().collect();
                Ok(json_result(json!({
                    "status": "READY",
                    "task": ctx.task,
                    "target_files": target_files,
                    "governing_rules": governing_rules,
                    "cochange_warnings": cochange_warnings,
                    "relevant_decisions": relevant_decisions,
                    "ambiguity_gaps": ctx.gaps,
                    "token_budget": ctx.token_budget,
                    "estimated_tokens": ctx.estimated_tokens,
                    "next_action": "Plan changes within target files and run engram_preflight with planned_files."
                })))
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

            crate::analyzer::ensure_fresh(&root)?;
            let conn = db(&root)?;
            let cone = crate::impact::compute_impact_cone(&conn, &root, target, depth)?;
            let active_contract = crate::contract::get_active_contract(&root).ok().flatten();
            let authorization = active_contract
                .as_ref()
                .map(|c| crate::contract::evaluate_impact_against_contract(c, &cone));
            let explanation = crate::impact::format_impact_explanation(&cone);
            let affected_paths = cone.all_affected_paths();
            let audit_memories = crate::memory::query_scoped_memories(
                &root,
                target,
                &affected_paths,
                &[target.to_string()],
                true,
            )
            .unwrap_or_default();
            let memory_conflicts = crate::memory::detect_memory_conflicts(&audit_memories);

            Ok(json_result(json!({
                "target": cone.target_symbol,
                "target_file": cone.target_file,
                "impact_cone": cone,
                "active_contract": active_contract,
                "authorization": authorization,
                "explanation": explanation,
                "memories": audit_memories,
                "memory_conflicts": memory_conflicts,
            })))
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
            Ok(json_result(serde_json::to_value(expl)?))
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
            Ok(json_result(impact(&root, target)?))
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
            Ok(json_result(serde_json::to_value(signals)?))
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
                Ok(json_result(serde_json::to_value(doc.memories)?))
            } else {
                let memories = query_relevant_memories(&root, query, &files)?;
                Ok(json_result(serde_json::to_value(memories)?))
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
            Ok(json_result(serde_json::to_value(memory)?))
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
            Ok(json_result(serde_json::to_value(violations)?))
        }
        "engram_get_ecosystem" => {
            let report = crate::manifest::detect_ecosystem(&root)?;
            Ok(json_result(serde_json::to_value(report)?))
        }
        "engram_doctor" => {
            let report = crate::doctor::diagnose_memory_health(&root)?;
            Ok(json_result(serde_json::to_value(report)?))
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
                Ok(json_result(serde_json::to_value(res)?))
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

                Ok(json_result(json!({
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
                })))
            }
        }
        "engram_postflight" => {
            let contract_id = args["contract_id"].as_str().map(str::to_string);
            let strict = args["strict"].as_bool().unwrap_or(false);
            let staged = args["staged"].as_bool().unwrap_or(false);
            let details = args["details"]
                .as_bool()
                .or_else(|| args["verbose"].as_bool())
                .unwrap_or(false);
            let opts = crate::flywheel::PostflightOptions {
                contract_id,
                strict,
                staged,
                base_commit: None,
                ci_mode: false,
            };
            let res = crate::flywheel::postflight_check(&root, &opts)?;
            if details {
                Ok(json_result(serde_json::to_value(res)?))
            } else {
                let verdict_str = match res.verdict {
                    crate::flywheel::PostflightVerdict::Pass => "PASS",
                    crate::flywheel::PostflightVerdict::BlockedBoundaryViolation => {
                        "BLOCKED_BOUNDARY_VIOLATION"
                    }
                    crate::flywheel::PostflightVerdict::ReviewUnexpectedExpansion => {
                        "REVIEW_UNEXPECTED_EXPANSION"
                    }
                    crate::flywheel::PostflightVerdict::BlockedConstitutionRule => {
                        "BLOCKED_CONSTITUTION_RULE"
                    }
                };
                let violations: Vec<Value> = res
                    .classifications
                    .iter()
                    .filter(|c| {
                        c.category != crate::contract::FileValidationCategory::Expected
                            && c.category
                                != crate::contract::FileValidationCategory::ExpectedExpansion
                    })
                    .map(|c| {
                        json!({
                            "path": c.path,
                            "category": format!("{:?}", c.category),
                            "reason": c.reason,
                        })
                    })
                    .collect();
                let next_action = if res.verdict == crate::flywheel::PostflightVerdict::Pass {
                    "All changes compliant with Change Contract. Ready to commit / create PR."
                } else {
                    "Revert violating/unexpected files or invoke engram_recover for structured remediation."
                };
                Ok(json_result(json!({
                    "verdict": verdict_str,
                    "contract_id": res.contract_id,
                    "status": if res.verdict == crate::flywheel::PostflightVerdict::Pass { "COMPLIANT" } else { "VIOLATION_DETECTED" },
                    "total_changed_files": res.total_changed_files,
                    "expected_count": res.expected_count,
                    "expansion_count": res.expansion_count,
                    "unexpected_count": res.unexpected_count,
                    "boundary_violation_count": res.boundary_violation_count,
                    "violations": violations,
                    "constitution_passed": res.constitution_passed,
                    "summary": res.summary,
                    "next_action": next_action,
                })))
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
            Ok(json_result(serde_json::to_value(rep)?))
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
            Ok(json_result(serde_json::to_value(rep)?))
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
            Ok(json_result(serde_json::to_value(mem)?))
        }
        _ => Err(anyhow!("Unknown tool: {name}")),
    }
}

pub fn tools_definitions() -> Value {
    json!([
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
                    "depth": { "type": "integer", "minimum": 1, "maximum": 3, "description": "Dependency traversal depth (default 2)" }
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
            "description": "Orchestrated Post-Flight Guard Check: evaluate mutation against Change Contract boundaries, enforce zero poisoning, and synthesize candidate memory.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "contract_id": { "type": "string", "description": "Active Change Contract ID to validate against" },
                    "strict": { "type": "boolean", "description": "Fail if unexpected files are modified" },
                    "staged": { "type": "boolean", "description": "Validate git staged index instead of entire working tree" },
                    "details": { "type": "boolean", "description": "Return full verbose payload with all file classification objects (default false for compact token-efficient payload)" }
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
                let tool_res = match mcp_call(&root, name, args) {
                    Ok(v) => v,
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
