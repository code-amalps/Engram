//! Engram Consolidated Engagement Endpoint (`engram_engage`)
//!
//! Provides a single, unified entry point for AI coding agents to engage with
//! an Engram repository, binding repository baseline, compiling AST context,
//! evaluating Project Constitution rules, locking Change Contracts, and
//! providing deterministic next-action guidance.

use std::path::Path;
use std::process::Command;

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::budget::DetailLevel;
use crate::mcp::validate_workspace_path;
use crate::prompt::{TaskComplexity, classify_task_complexity};
use crate::storage::root_path;

/// Request parameters for the `engram_engage` consolidated endpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EngageRequest {
    /// The coding task goal, user prompt, or intent.
    pub task: String,
    /// Context token budget limit (default 1200).
    #[serde(default = "default_token_budget")]
    pub token_budget: usize,
    /// Explicit list of relative workspace file paths the agent intends to edit.
    #[serde(default)]
    pub planned_files: Option<Vec<String>>,
    /// Explicit forbidden boundary patterns beyond the Project Constitution.
    #[serde(default)]
    pub forbidden: Option<Vec<String>>,
    /// Targeted symbol filter: prioritize excerpting these specific functions, classes, or types.
    #[serde(default)]
    pub symbols: Option<Vec<String>>,
    /// Optional repository subdirectory scope to restrict AST traversal.
    #[serde(default)]
    pub subpath: Option<String>,
    /// Optional programming language filter.
    #[serde(default)]
    pub language: Option<String>,
    /// Level of excerpt detail: "compact", "standard", or "full".
    #[serde(default = "default_detail_level")]
    pub detail_level: String,
    /// Force full Change Contract locking and AST compilation even if the task qualifies as trivial.
    #[serde(default)]
    pub no_bypass: bool,
    /// Also inject Turn-0 context and directives into workspace agent instruction files.
    #[serde(default)]
    pub inject_instructions: bool,
    /// Maximum serialized JSON-RPC response size in bytes.
    #[serde(default = "default_max_bytes")]
    pub max_bytes: usize,
}

fn default_token_budget() -> usize {
    1200
}

fn default_detail_level() -> String {
    "standard".to_string()
}

fn default_max_bytes() -> usize {
    16384
}

/// Helper to get current git branch safely.
fn get_git_branch(root: &Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(root)
        .output();
    if let Ok(out) = output {
        if out.status.success() {
            let b = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !b.is_empty() {
                return b;
            }
        }
    }
    "HEAD".to_string()
}

/// Parse and execute an `engram_engage` tool invocation.
pub fn handle_engram_engage(root: &Path, args: &Value) -> Result<Value> {
    let req: EngageRequest = serde_json::from_value(args.clone())
        .map_err(|e| anyhow!("Invalid arguments for engram_engage: {}", e))?;
    execute_engage(root, req)
}

/// Execute the consolidated engagement workflow.
pub fn execute_engage(root: &Path, req: EngageRequest) -> Result<Value> {
    let root = root_path(root)?;

    // 1. Validate task string
    let trimmed_task = req.task.trim();
    if trimmed_task.is_empty() {
        bail!("Task description must not be empty");
    }
    if trimmed_task.len() > 4096 {
        bail!("Task description exceeds maximum length of 4096 characters");
    }

    // 2. Validate explicit planned files
    if let Some(ref planned) = req.planned_files {
        for p in planned {
            validate_workspace_path(&root, p)
                .map_err(|e| anyhow!("Planned file validation failed: {}", e))?;
        }
    }

    // 3. Validate explicit forbidden boundaries
    if let Some(ref forbidden) = req.forbidden {
        for p in forbidden {
            validate_workspace_path(&root, p)
                .map_err(|e| anyhow!("Forbidden boundary validation failed: {}", e))?;
        }
    }

    // 4. Parse task and planned files declarations from task prompt
    let (core_task, parsed_planned) =
        crate::flywheel::parse_task_and_planned_files(trimmed_task, req.planned_files.clone());

    for p in &parsed_planned {
        validate_workspace_path(&root, p)
            .map_err(|e| anyhow!("Planned file validation failed: {}", e))?;
    }

    let effective_planned = if parsed_planned.is_empty() {
        None
    } else {
        Some(parsed_planned.clone())
    };

    // 5. Gather repository metadata
    let root_str = root.display().to_string();
    let baseline_commit = crate::contract::get_baseline_revision(&root);
    let branch = get_git_branch(&root);
    let git_state_res = crate::git::detect_git_state(&root);
    let (is_clean_val, git_state_str, git_state_err_val) = match git_state_res {
        Ok(true) => (json!(true), "CLEAN", Value::Null),
        Ok(false) => (json!(false), "DIRTY", Value::Null),
        Err(ref e) => (Value::Null, "UNKNOWN", json!(e.to_string())),
    };

    // 6. Task complexity classification & adaptive bypass evaluation
    let signals = classify_task_complexity(&core_task);
    if signals.classification == TaskComplexity::Trivial && !req.no_bypass {
        return Ok(json!({
            "status": "BYPASS_TRIVIAL",
            "repository": {
                "root": root_str,
                "baseline_commit": baseline_commit,
                "branch": branch,
                "is_clean": is_clean_val,
                "git_state": git_state_str,
                "git_state_error": git_state_err_val,
            },
            "task": {
                "goal": core_task,
                "classification": "TRIVIAL",
                "classification_reason": signals.reason,
                "bypass_active": true,
            },
            "contract": {
                "contract_id": Value::Null,
                "contract_version": Value::Null,
                "is_reused": false,
                "reused_reason": Value::Null,
            },
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
                "token_budget": req.token_budget,
                "estimated_tokens": 0,
                "omission_manifest": Value::Null,
            },
            "instructions": {
                "injected_files": Value::Array(Vec::new()),
                "directives": [
                    "Task classified as trivial leaf edit. Proceed directly with edit without MCP overhead."
                ]
            },
            "blocking_violations": Value::Array(Vec::new()),
            "next_action": "Task classified as trivial leaf edit. Proceed directly to implementation without Change Contract overhead. Call engram_postflight only if compliance audit is desired."
        }));
    }

    // 7. Compile AST context
    let budget = req.token_budget.clamp(100, 32000);
    let detail_level = if req.detail_level.eq_ignore_ascii_case("compact") {
        DetailLevel::Compact
    } else if req.detail_level.eq_ignore_ascii_case("full") {
        DetailLevel::Full
    } else {
        DetailLevel::Contracted
    };

    let ctx = crate::compiler::context_scoped_optimized(
        &root,
        &core_task,
        budget,
        req.subpath.as_deref(),
        req.language.as_deref(),
        detail_level,
        req.symbols.as_deref(),
    )?;

    // 8. Preflight handshake and contract locking
    let preflight_opts = crate::flywheel::PreflightOptions {
        task: core_task.clone(),
        token_budget: budget,
        forbidden_boundaries: req.forbidden.clone(),
        planned_files: effective_planned.clone(),
    };
    let preflight_res = crate::flywheel::preflight_task(&root, &preflight_opts)?;

    // Check for boundary violations against req.forbidden and Project Constitution
    let boundary_violations = if let Some(ref planned) = effective_planned {
        crate::contract::validate_planned_boundaries(&root, planned, req.forbidden.as_deref())
    } else {
        Vec::new()
    };

    let is_blocked = preflight_res.status == crate::flywheel::PreflightStatus::Blocked
        || !boundary_violations.is_empty();

    let status_str = if is_blocked { "BLOCKED" } else { "READY" };

    let mut blocking_violations =
        if preflight_res.status == crate::flywheel::PreflightStatus::Blocked {
            preflight_res.ambiguity_gaps.clone()
        } else {
            Vec::new()
        };
    blocking_violations.extend(boundary_violations);

    if is_blocked && !preflight_res.contract_id.is_empty() {
        if let Ok(Some(mut contract)) = crate::contract::get_active_contract(&root) {
            if contract.contract_id == preflight_res.contract_id {
                contract.status = crate::contract::ContractStatus::BlockedAwaitingHuman;
                let _ = crate::contract::save_contract(&root, &contract);
            }
        }
    }

    let next_action = if is_blocked {
        "Reconcile blocking violations before modifying code or request human override."
    } else if preflight_res.is_reused_contract {
        "Valid Change Contract active. Proceed to edit expected files. Call engram_postflight when finished."
    } else {
        "Change Contract locked. Proceed directly to implement changes in authorized files. Call engram_postflight when finished."
    };

    // 9. Format structured context components
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

    let cochange_warnings: Vec<String> = ctx.cochange_alerts.iter().take(5).cloned().collect();
    let relevant_decisions: Vec<String> = ctx.relevant_memories.iter().take(5).cloned().collect();

    // Bounded omission manifest
    let bounded_omission_manifest = if let Some(ref raw_man) = ctx.omission_manifest {
        let total_omitted = raw_man.omitted_items_count;
        let mut reason_counts: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for entry in &raw_man.omission_manifest {
            *reason_counts
                .entry(entry.omission_reason.clone())
                .or_insert(0) += 1;
        }

        let mut sorted_entries = raw_man.omission_manifest.clone();
        sorted_entries.sort_by_key(|e| e.priority_tier);
        let sample_entries: Vec<_> = sorted_entries.into_iter().take(3).collect();
        let manifest_truncated = total_omitted > sample_entries.len();
        let unrepresented = total_omitted.saturating_sub(sample_entries.len());

        Some(json!({
            "budget_status": raw_man.budget_status,
            "budget_tokens_limit": raw_man.budget_tokens_limit,
            "budget_tokens_estimated": raw_man.budget_tokens_estimated,
            "included_items_count": raw_man.included_items_count,
            "omitted_items_count": total_omitted,
            "omission_reasons": reason_counts,
            "manifest_truncated": manifest_truncated,
            "unrepresented_omissions_count": unrepresented,
            "sample_omissions": sample_entries,
            "targeted_retrieval_guidance": "Use engram_inspect_symbol(target) or targeted engram_get_context(subpath=...) to inspect specific omitted components."
        }))
    } else {
        None
    };

    // 10. Optional instruction injection
    let mut injected_files = Vec::new();
    if req.inject_instructions {
        let prep_output = crate::prompt::PreparedTaskOutput {
            task: core_task.clone(),
            contract_id: preflight_res.contract_id.clone(),
            contract_version: preflight_res.contract_version,
            baseline_commit: preflight_res.baseline_commit.clone(),
            status: status_str.to_string(),
            expected_files: preflight_res.expected_files.clone(),
            forbidden_boundaries: preflight_res.forbidden_boundaries.clone(),
            content: String::new(),
            estimated_tokens: ctx.estimated_tokens,
            token_budget: ctx.token_budget,
            files_included: ctx.files.len(),
            files_omitted: ctx
                .omission_manifest
                .as_ref()
                .map(|m| m.omitted_items_count)
                .unwrap_or(0),
            completeness_status: "COMPLETE".to_string(),
            clipboard_copied: false,
            active_file_path: None,
            bypass_active: false,
            classification_signals: Some(signals.clone()),
            injected_files: Vec::new(),
        };

        if let Ok(reports) =
            crate::instructions::inject_task_instructions(&root, &prep_output, None, None, false)
        {
            injected_files = reports
                .into_iter()
                .map(|r| r.file.to_string_lossy().to_string())
                .collect();
        }
    }

    // 11. Directives synthesis
    let mut directives = Vec::new();
    if status_str == "READY" {
        directives.push("Modify only files listed in expected_files.".to_string());
        directives.push("Do not edit forbidden_boundaries.".to_string());
        directives.push(
            "Call engram_postflight upon completion to verify boundaries and test evidence."
                .to_string(),
        );
    } else if status_str == "BLOCKED" {
        directives.push(
            "STOP: Planned modifications violate repository boundaries or Project Constitution."
                .to_string(),
        );
        directives.push(
            "Do not edit forbidden files. Reconcile blocking violations before proceeding."
                .to_string(),
        );
    }

    Ok(json!({
        "status": status_str,
        "repository": {
            "root": root_str,
            "baseline_commit": baseline_commit,
            "branch": branch,
            "is_clean": is_clean_val,
            "git_state": git_state_str,
            "git_state_error": git_state_err_val,
        },
        "task": {
            "goal": core_task,
            "classification": match signals.classification {
                TaskComplexity::Trivial => "TRIVIAL",
                TaskComplexity::Standard => "STANDARD",
            },
            "classification_reason": signals.reason,
            "bypass_active": false,
        },
        "contract": {
            "contract_id": preflight_res.contract_id,
            "contract_version": preflight_res.contract_version,
            "is_reused": preflight_res.is_reused_contract,
            "reused_reason": preflight_res.reused_reason,
        },
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
            "omission_manifest": bounded_omission_manifest,
        },
        "instructions": {
            "injected_files": injected_files,
            "directives": directives,
        },
        "blocking_violations": blocking_violations,
        "next_action": next_action,
    }))
}
