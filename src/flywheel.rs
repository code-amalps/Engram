//! Engram v1.0 Closed-Loop Change Flywheel Engine:
//! Orchestrated Pre-Flight and Post-Flight protocol uniting Context, Impact,
//! Contracts, Guard, and Candidate Compounding under a strict Human Authority Boundary.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::analyzer::ensure_fresh;
use crate::compiler::context;
use crate::constitution::evaluate_rules;
use crate::contract::{
    create_contract, get_active_contract, get_contract, ChangeContract, FileClassification,
};
use crate::impact::compute_impact_cone;
use crate::memory::{query_scoped_memories, record_task_candidate, MemoryCandidate};
use crate::model::{RuleSeverity, RuleViolation};
use crate::storage::{db, git_changed, revision, root_path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightOptions {
    pub task: String,
    pub token_budget: usize,
    pub forbidden_boundaries: Option<Vec<String>>,
    #[serde(default)]
    pub planned_files: Option<Vec<String>>,
}

impl PreflightOptions {
    pub fn new(task: impl Into<String>, token_budget: usize) -> Self {
        Self {
            task: task.into(),
            token_budget,
            forbidden_boundaries: None,
            planned_files: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PreflightStatus {
    ReadyForImplementation,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoverningPolicySummary {
    pub id: String,
    pub title: String,
    pub scope: String,
    pub statement: String,
    pub rationale: String,
}

pub const PROTOCOL_VERSION: &str = "1.1";

fn default_protocol_version() -> String {
    PROTOCOL_VERSION.to_string()
}

fn default_contract_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightResult {
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,
    pub task_id: String,
    pub contract_id: String,
    #[serde(default = "default_contract_version")]
    pub contract_version: u32,
    pub task_goal: String,
    pub baseline_commit: String,
    pub context_files: usize,
    pub context_tokens: usize,
    pub ambiguity_gaps: Vec<String>,
    pub predicted_impact_count: usize,
    pub predicted_impact_paths: Vec<String>,
    pub governing_policies: Vec<GoverningPolicySummary>,
    pub expected_files: Vec<String>,
    pub expected_modules: Vec<String>,
    pub forbidden_boundaries: Vec<String>,
    pub is_reused_contract: bool,
    #[serde(default)]
    pub reused_reason: Option<String>,
    pub status: PreflightStatus,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostflightOptions {
    pub contract_id: Option<String>,
    pub strict: bool,
    pub staged: bool,
    pub base_commit: Option<String>,
    pub ci_mode: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PostflightVerdict {
    Pass,
    BlockedBoundaryViolation,
    ReviewUnexpectedExpansion,
    BlockedConstitutionRule,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostflightResult {
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,
    pub contract_id: String,
    pub task_goal: String,
    pub baseline_commit: String,
    pub verdict: PostflightVerdict,
    pub exit_code: i32,
    pub total_changed_files: usize,
    pub expected_count: usize,
    pub expansion_count: usize,
    pub unexpected_count: usize,
    pub boundary_violation_count: usize,
    pub changed_files: Vec<String>,
    pub classifications: Vec<FileClassification>,
    pub constitution_passed: bool,
    pub constitution_violations: Vec<RuleViolation>,
    pub generated_candidates: Vec<MemoryCandidate>,
    pub human_action_required: Option<String>,
    pub summary: String,
}

const GENERIC_UNSCOPED_NAMES: &[&str] = &[
    "constructor", "new", "init", "__init__", "toString", "clone", "equals", "hashCode",
    "render", "build", "create", "destroy",
];

fn extract_symbols_for_paths(root: &Path, paths: &[String]) -> Vec<String> {
    let mut symbols = Vec::new();
    if let Ok(conn) = db(root) {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT name FROM entity_versions WHERE path = ?1 AND invalidated_at_revision IS NULL",
        ) {
            for p in paths {
                if let Ok(mut rows) = stmt.query([p]) {
                    while let Ok(Some(row)) = rows.next() {
                        if let Ok(name) = row.get::<_, String>(0) {
                            if !GENERIC_UNSCOPED_NAMES.contains(&name.as_str())
                                && !symbols.contains(&name)
                            {
                                symbols.push(name);
                            }
                        }
                    }
                }
            }
        }
    }
    symbols
}

fn get_governing_policies_for_paths(
    root: &Path,
    task: &str,
    paths: &[String],
) -> Vec<GoverningPolicySummary> {
    let touched_symbols = extract_symbols_for_paths(root, paths);
    let governing_mems =
        query_scoped_memories(root, task, paths, &touched_symbols, false).unwrap_or_default();

    governing_mems
        .into_iter()
        .map(|m| GoverningPolicySummary {
            id: m.id,
            title: m.title,
            scope: format!("{:?}", m.scope.level),
            statement: m.statement,
            rationale: m.reason,
        })
        .collect()
}

pub fn parse_task_and_planned_files(
    task: &str,
    explicit_planned: Option<Vec<String>>,
) -> (String, Vec<String>) {
    let mut planned = explicit_planned.unwrap_or_default();
    let lower = task.to_lowercase();
    let markers = [
        "planned files:",
        "planned files -",
        "planned files",
        "planned:",
        "files planned:",
    ];
    let mut core_task = task.to_string();

    for marker in &markers {
        if let Some(idx) = lower.find(marker) {
            core_task = task[..idx]
                .trim()
                .trim_end_matches(['.', ';', '-', ':', ','])
                .trim()
                .to_string();
            let planned_str = &task[idx + marker.len()..];
            for part in planned_str.split([',', ';', '\n', '\r']) {
                let candidate = part
                    .trim()
                    .trim_start_matches("- ")
                    .trim_start_matches("* ")
                    .trim();
                if !candidate.is_empty()
                    && (candidate.contains('/')
                        || candidate.contains('\\')
                        || candidate.contains('.'))
                {
                    let norm = crate::contract::normalize_rel_path(candidate);
                    if !planned.contains(&norm) {
                        planned.push(norm);
                    }
                }
            }
            break;
        }
    }

    if core_task.is_empty() {
        core_task = task.trim().to_string();
    }

    (core_task, planned)
}

pub fn compute_preflight_fingerprint(
    root: &Path,
    core_task: &str,
    baseline_commit: &str,
    expected_files: &[String],
    forbidden: &[String],
) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(core_task.trim().to_lowercase().as_bytes());
    hasher.update(b"|");
    hasher.update(baseline_commit.as_bytes());
    hasher.update(b"|");

    let mut sorted_forbidden = forbidden.to_vec();
    sorted_forbidden.sort();
    for f in &sorted_forbidden {
        hasher.update(f.as_bytes());
        hasher.update(b",");
    }
    hasher.update(b"|");

    let mut sorted_files = expected_files.to_vec();
    sorted_files.sort();
    for file_rel in &sorted_files {
        hasher.update(file_rel.as_bytes());
        hasher.update(b":");
        let full_path = root.join(file_rel);
        if let Ok(content) = std::fs::read(&full_path) {
            let mut file_hasher = Sha256::new();
            file_hasher.update(&content);
            let file_hash = file_hasher.finalize();
            hasher.update(&file_hash);
        } else {
            hasher.update(b"NON_EXISTENT");
        }
        hasher.update(b";");
    }

    if let Ok(rules) = crate::constitution::load_constitution(root) {
        for r in &rules {
            hasher.update(r.id.as_bytes());
            hasher.update(r.enabled.to_string().as_bytes());
            if let Some(ref f) = r.forbidden {
                hasher.update(f.as_bytes());
            }
        }
    }

    format!("{:x}", hasher.finalize())
}

/// Execute deterministic Pre-Flight Handshake (`engram preflight`).
/// Grounds the task, predicts impact, retrieves governing decisions, and locks the contract.
/// Idempotency & Incremental Delta: Reuses active contract if unchanged, delta-validates expansions.
pub fn preflight_task(root: &Path, opts: &PreflightOptions) -> Result<PreflightResult> {
    let root = root_path(root)?;
    ensure_fresh(&root)?;

    let base_commit = crate::contract::get_baseline_revision(&root);
    let (core_task, planned_files) =
        parse_task_and_planned_files(&opts.task, opts.planned_files.clone());
    let normalized_task = core_task.trim();

    // 1. Idempotency Check: Check if an active contract already matches this task and revision
    if let Ok(Some(mut active)) = get_active_contract(&root) {
        let active_task_core = parse_task_and_planned_files(&active.task_goal, None).0;
        let is_task_match = active_task_core
            .trim()
            .eq_ignore_ascii_case(normalized_task)
            || active
                .task_goal
                .trim()
                .eq_ignore_ascii_case(normalized_task);

        if is_task_match
            && (active.baseline_revision == base_commit
                || active.baseline_revision == "WORKTREE"
                || base_commit == "WORKTREE")
        {
            if active.status == crate::contract::ContractStatus::BlockedAwaitingHuman {
                return Ok(PreflightResult {
                    protocol_version: PROTOCOL_VERSION.to_string(),
                    task_id: active.contract_id.clone(),
                    contract_id: active.contract_id.clone(),
                    contract_version: active.version,
                    task_goal: active.task_goal,
                    baseline_commit: base_commit,
                    context_files: 0,
                    context_tokens: 0,
                    ambiguity_gaps: vec!["Contract is BLOCKED_AWAITING_HUMAN. Human reconciliation required before proceeding.".to_string()],
                    predicted_impact_count: 0,
                    predicted_impact_paths: Vec::new(),
                    governing_policies: Vec::new(),
                    expected_files: active.boundary.expected_files,
                    expected_modules: active.boundary.expected_modules,
                    forbidden_boundaries: active.boundary.forbidden_boundaries,
                    is_reused_contract: true,
                    reused_reason: Some("Contract is blocked awaiting human reconciliation".to_string()),
                    status: PreflightStatus::Blocked,
                    message: "Contract is BLOCKED_AWAITING_HUMAN. Human reconciliation required before implementation.".into(),
                });
            }

            let current_fingerprint = compute_preflight_fingerprint(
                &root,
                normalized_task,
                &base_commit,
                &active.boundary.expected_files,
                &active.boundary.forbidden_boundaries,
            );

            let normalized_active_expected: Vec<String> = active
                .boundary
                .expected_files
                .iter()
                .map(|f| crate::contract::normalize_rel_path(f))
                .collect();
            let new_files: Vec<String> = planned_files
                .iter()
                .filter(|p| {
                    !normalized_active_expected.contains(&crate::contract::normalize_rel_path(p))
                })
                .cloned()
                .collect();

            // Case A: Exact Idempotent Reuse
            if new_files.is_empty() {
                let is_fingerprint_match =
                    active.input_fingerprint.as_deref() == Some(&current_fingerprint);
                let reason = if is_fingerprint_match {
                    "Unchanged input fingerprint (idempotent preflight reuse)".to_string()
                } else {
                    "Unchanged planned files (active contract reused)".to_string()
                };

                let governing_policies = get_governing_policies_for_paths(
                    &root,
                    normalized_task,
                    &active.boundary.expected_files,
                );

                let res = PreflightResult {
                    protocol_version: PROTOCOL_VERSION.to_string(),
                    task_id: active.contract_id.clone(),
                    contract_id: active.contract_id.clone(),
                    contract_version: active.version,
                    task_goal: active.task_goal.clone(),
                    baseline_commit: base_commit,
                    context_files: active.boundary.expected_files.len(),
                    context_tokens: 0,
                    ambiguity_gaps: Vec::new(),
                    predicted_impact_count: active.boundary.expected_files.len(),
                    predicted_impact_paths: active.boundary.expected_files.clone(),
                    governing_policies,
                    expected_files: active.boundary.expected_files.clone(),
                    expected_modules: active.boundary.expected_modules.clone(),
                    forbidden_boundaries: active.boundary.forbidden_boundaries.clone(),
                    is_reused_contract: true,
                    reused_reason: Some(reason.clone()),
                    status: PreflightStatus::ReadyForImplementation,
                    message: format!("Deterministic active contract reused: {}", reason),
                };

                let _ = crate::telemetry::record_event(
                    &root,
                    "PREFLIGHT_HANDSHAKE",
                    Some(&res.contract_id),
                    "agent_preflight",
                    serde_json::json!({
                        "task_goal": &res.task_goal,
                        "context_files": res.context_files,
                        "is_reused": true,
                        "reused_reason": &res.reused_reason,
                        "status": format!("{:?}", res.status),
                    }),
                );

                return Ok(res);
            }

            // Case B: Incremental Plan Delta Validation
            let mut delta_violations = Vec::new();
            for nf in &new_files {
                let norm_nf = crate::contract::normalize_rel_path(nf);
                if active
                    .boundary
                    .forbidden_boundaries
                    .iter()
                    .any(|fb| crate::contract::matches_boundary(&norm_nf, fb))
                {
                    delta_violations
                        .push(format!("Planned file '{}' matches forbidden boundary", nf));
                }
                if let Ok(rules) = crate::constitution::load_constitution(&root) {
                    for r in &rules {
                        if let Some(ref fb) = r.forbidden {
                            if crate::contract::matches_boundary(&norm_nf, fb) {
                                delta_violations.push(format!(
                                    "Planned file '{}' violates constitution rule '{}' ({})",
                                    nf, r.name, r.id
                                ));
                            }
                        }
                    }
                }
            }

            if !delta_violations.is_empty() {
                return Ok(PreflightResult {
                    protocol_version: PROTOCOL_VERSION.to_string(),
                    task_id: active.contract_id.clone(),
                    contract_id: active.contract_id.clone(),
                    contract_version: active.version,
                    task_goal: active.task_goal.clone(),
                    baseline_commit: base_commit,
                    context_files: active.boundary.expected_files.len(),
                    context_tokens: 0,
                    ambiguity_gaps: delta_violations.clone(),
                    predicted_impact_count: active.boundary.expected_files.len(),
                    predicted_impact_paths: active.boundary.expected_files.clone(),
                    governing_policies: Vec::new(),
                    expected_files: active.boundary.expected_files.clone(),
                    expected_modules: active.boundary.expected_modules.clone(),
                    forbidden_boundaries: active.boundary.forbidden_boundaries.clone(),
                    is_reused_contract: true,
                    reused_reason: Some(
                        "Incremental delta rejected due to boundary violations".to_string(),
                    ),
                    status: PreflightStatus::Blocked,
                    message: format!(
                        "Incremental plan update BLOCKED: {}",
                        delta_violations.join("; ")
                    ),
                });
            }

            let from_ver = active.version;
            active.version += 1;
            for nf in &new_files {
                let norm = crate::contract::normalize_rel_path(nf);
                if !active.boundary.expected_files.contains(&norm) {
                    active.boundary.expected_files.push(norm.clone());
                }
                let mod_name = crate::contract::extract_module(&norm);
                if !active.boundary.expected_modules.contains(&mod_name)
                    && mod_name != "root"
                    && mod_name != "src"
                    && mod_name != "projects"
                    && mod_name != "packages"
                {
                    active.boundary.expected_modules.push(mod_name);
                }
            }

            active
                .reconciliation_history
                .push(crate::contract::ReconciliationRecord {
                    from_version: from_ver,
                    to_version: active.version,
                    reason: format!(
                        "Incremental plan expansion: added {} file(s)",
                        new_files.len()
                    ),
                    actor: "agent_preflight".to_string(),
                    approved_additions: new_files.clone(),
                    timestamp: chrono::Utc::now(),
                });

            let new_fingerprint = compute_preflight_fingerprint(
                &root,
                normalized_task,
                &base_commit,
                &active.boundary.expected_files,
                &active.boundary.forbidden_boundaries,
            );
            active.input_fingerprint = Some(new_fingerprint);
            active.updated_at = chrono::Utc::now();
            crate::contract::save_contract(&root, &active)?;

            let governing_policies = get_governing_policies_for_paths(
                &root,
                normalized_task,
                &active.boundary.expected_files,
            );

            let res = PreflightResult {
                protocol_version: PROTOCOL_VERSION.to_string(),
                task_id: active.contract_id.clone(),
                contract_id: active.contract_id.clone(),
                contract_version: active.version,
                task_goal: active.task_goal.clone(),
                baseline_commit: base_commit,
                context_files: active.boundary.expected_files.len(),
                context_tokens: 0,
                ambiguity_gaps: Vec::new(),
                predicted_impact_count: active.boundary.expected_files.len(),
                predicted_impact_paths: active.boundary.expected_files.clone(),
                governing_policies,
                expected_files: active.boundary.expected_files.clone(),
                expected_modules: active.boundary.expected_modules.clone(),
                forbidden_boundaries: active.boundary.forbidden_boundaries.clone(),
                is_reused_contract: true,
                reused_reason: Some(format!(
                    "Incremental plan expansion approved (version {} -> {})",
                    from_ver, active.version
                )),
                status: PreflightStatus::ReadyForImplementation,
                message: format!(
                    "Plan updated with {} new file(s) (version {}). Ready for implementation.",
                    new_files.len(),
                    active.version
                ),
            };

            let _ = crate::telemetry::record_event(
                &root,
                "PREFLIGHT_HANDSHAKE",
                Some(&res.contract_id),
                "agent_preflight",
                serde_json::json!({
                    "task_goal": &res.task_goal,
                    "context_files": res.context_files,
                    "is_reused": true,
                    "reused_reason": &res.reused_reason,
                    "status": format!("{:?}", res.status),
                }),
            );

            return Ok(res);
        }
    }

    // 2. Context Compilation
    let ctx = context(&root, normalized_task, opts.token_budget)?;
    let context_files = ctx.files.len();
    let context_tokens = ctx.estimated_tokens;
    let ambiguity_gaps = ctx.gaps.clone();

    let mut touched_paths: Vec<String> = ctx.files.iter().map(|f| f.path.clone()).collect();
    for pf in &planned_files {
        let norm = crate::contract::normalize_rel_path(pf);
        if !touched_paths.contains(&norm) {
            touched_paths.push(norm);
        }
    }
    let touched_symbols = extract_symbols_for_paths(&root, &touched_paths);

    // 3. Multi-Hop Impact Blast Radius Prediction
    let conn = db(&root)?;
    let mut predicted_impact_paths = Vec::new();
    for p in &touched_paths {
        if let Ok(cone) = compute_impact_cone(&conn, &root, p, 2) {
            for aff in cone.all_affected_paths() {
                if !predicted_impact_paths.contains(&aff) {
                    predicted_impact_paths.push(aff);
                }
            }
        }
    }
    for sym in touched_symbols.iter().take(3) {
        if let Ok(cone) = compute_impact_cone(&conn, &root, sym, 2) {
            for p in cone.all_affected_paths() {
                if !predicted_impact_paths.contains(&p) {
                    predicted_impact_paths.push(p);
                }
            }
        }
    }
    for p in &touched_paths {
        if !predicted_impact_paths.contains(p) {
            predicted_impact_paths.push(p.clone());
        }
    }

    // 4. Governing Architectural Decisions Discovery
    let governing_policies =
        get_governing_policies_for_paths(&root, normalized_task, &touched_paths);

    // 5. Elastic Change Contract Generation & Lock
    let mut expected_modules: Vec<String> = Vec::new();
    for p in &touched_paths {
        let m = crate::contract::extract_module(p);
        if !expected_modules.contains(&m)
            && m != "root"
            && m != "src"
            && m != "projects"
            && m != "packages"
        {
            expected_modules.push(m);
        }
    }

    let forbidden = opts.forbidden_boundaries.clone();
    let mut contract = create_contract(
        &root,
        normalized_task,
        Some(expected_modules.clone()),
        Some(touched_paths.clone()),
        forbidden,
    )?;

    let fingerprint = compute_preflight_fingerprint(
        &root,
        normalized_task,
        &base_commit,
        &contract.boundary.expected_files,
        &contract.boundary.forbidden_boundaries,
    );
    contract.input_fingerprint = Some(fingerprint);
    crate::contract::save_contract(&root, &contract)?;

    let res = PreflightResult {
        protocol_version: PROTOCOL_VERSION.to_string(),
        task_id: contract.contract_id.clone(),
        contract_id: contract.contract_id.clone(),
        contract_version: contract.version,
        task_goal: contract.task_goal,
        baseline_commit: base_commit,
        context_files,
        context_tokens,
        ambiguity_gaps,
        predicted_impact_count: predicted_impact_paths.len(),
        predicted_impact_paths,
        governing_policies,
        expected_files: contract.boundary.expected_files,
        expected_modules: contract.boundary.expected_modules,
        forbidden_boundaries: contract.boundary.forbidden_boundaries,
        is_reused_contract: false,
        reused_reason: None,
        status: PreflightStatus::ReadyForImplementation,
        message: "Ready for implementation. Change Contract locked.".into(),
    };

    let _ = crate::telemetry::record_event(
        &root,
        "PREFLIGHT_HANDSHAKE",
        Some(&res.contract_id),
        "agent_preflight",
        serde_json::json!({
            "task_goal": &res.task_goal,
            "context_files": res.context_files,
            "is_reused": false,
            "status": format!("{:?}", res.status),
        }),
    );

    Ok(res)
}

/// Execute deterministic Post-Flight Guard Check (`engram postflight`).
/// Evaluates working tree diff against Contract and Constitution.
/// Enforces:
///   BOUNDARY_VIOLATION  -> BLOCKED (Exit 1)
///   UNEXPECTED          -> REVIEW / RATIFICATION (Advisory or Exit 1 if --strict)
///   EXPECTED_EXPANSION  -> PASS
///   EXPECTED            -> PASS
/// Preserves zero-poisoning rule: clean mutations synthesize reviewable MemoryCandidate.
pub fn postflight_check(root: &Path, opts: &PostflightOptions) -> Result<PostflightResult> {
    let root = root_path(root)?;
    ensure_fresh(&root)?;

    // 1. Resolve Active Change Contract
    let contract: ChangeContract = if let Some(ref id) = opts.contract_id {
        get_contract(&root, id)?
    } else {
        get_active_contract(&root)?
            .ok_or_else(|| anyhow!("No active Change Contract found. Run 'engram preflight \"<task>\"' before making modifications."))?
    };

    if contract.status == crate::contract::ContractStatus::BlockedAwaitingHuman {
        return Err(anyhow!(
            "CONTRACT_BLOCKED_AWAITING_HUMAN: Change Contract '{}' is BLOCKED_AWAITING_HUMAN. Automated postflight cannot proceed or auto-close until human intervention (reconcile or ratify).",
            contract.contract_id
        ));
    }

    // 2. Discover Changed Files
    let changed_files = if let Some(ref base) = opts.base_commit {
        let out = std::process::Command::new("git")
            .args(["diff", "--name-only", base, "HEAD"])
            .current_dir(&root)
            .output()?;
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    } else if opts.staged {
        let out = std::process::Command::new("git")
            .args(["diff", "--name-only", "--cached"])
            .current_dir(&root)
            .output()?;
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    } else {
        git_changed(&root)
    };

    // 3. Elastic Contract Evaluation
    let (classifications, _missing_changes, summary_stats) =
        crate::contract::validate_contract_diff_with_constitution_and_base(
            &root,
            &contract,
            &changed_files,
            opts.base_commit.as_deref(),
        );
    let expected_count = summary_stats.expected_count;
    let expansion_count = summary_stats.expansion_count;
    let unexpected_count = summary_stats.unexpected_count;
    let boundary_violation_count = summary_stats.violation_count;

    // 4. Constitution Rule Verification
    let conn = db(&root)?;
    let rule_violations = evaluate_rules(&root, &changed_files, &conn)?;
    let blocking_rule_violations: Vec<&RuleViolation> = rule_violations
        .iter()
        .filter(|v| !v.overridden && v.severity == RuleSeverity::Error)
        .collect();
    let constitution_passed = blocking_rule_violations.is_empty();

    // 5. 4-Tier Verdict Determination
    let (verdict, exit_code, summary) = if boundary_violation_count > 0 {
        (
            PostflightVerdict::BlockedBoundaryViolation,
            1,
            format!(
                "BLOCKED: {} boundary violation(s) detected. Forbidden files modified.",
                boundary_violation_count
            ),
        )
    } else if !constitution_passed {
        (
            PostflightVerdict::BlockedConstitutionRule,
            1,
            format!(
                "BLOCKED: {} architectural constitution rule violation(s).",
                blocking_rule_violations.len()
            ),
        )
    } else if unexpected_count > 0 && opts.strict {
        (
            PostflightVerdict::ReviewUnexpectedExpansion,
            1,
            format!(
                "BLOCKED: {} unexpected change(s) outside contract boundary under --strict mode.",
                unexpected_count
            ),
        )
    } else if unexpected_count > 0 {
        (
            PostflightVerdict::Pass,
            0,
            format!(
                "ADVISORY: Clean pass with {} unexpected file(s) requiring team review.",
                unexpected_count
            ),
        )
    } else {
        (
            PostflightVerdict::Pass,
            0,
            "SUCCESS: Mutation verified against contract boundaries and constitution.".into(),
        )
    };

    // 6. Candidate Compounding Engine
    // Invariant: ONLY clean passing mutations synthesize candidates; violating mutations never produce candidates.
    let mut generated_candidates = Vec::new();
    let mut human_action = None;

    if verdict == PostflightVerdict::Pass {
        let _ = crate::recovery::resolve_session(&root, &contract.contract_id);
    }

    if verdict == PostflightVerdict::Pass && !changed_files.is_empty() {
        let task_title = if contract.task_goal.len() > 40 {
            format!("{}...", &contract.task_goal[..37])
        } else {
            contract.task_goal.clone()
        };

        let candidate_title = format!("{} Implementation Policy", task_title);
        let candidate_statement = format!(
            "Modifications for '{}' conform to modules: {:?}",
            contract.task_goal, contract.boundary.expected_modules
        );
        let candidate_reason = format!(
            "Verified against Change Contract {} with zero boundary violations",
            contract.contract_id
        );

        if let Ok(cand) = record_task_candidate(
            &root,
            &contract.task_goal,
            &candidate_title,
            &candidate_statement,
            &candidate_reason,
            contract.boundary.expected_modules.clone(),
            changed_files.clone(),
            "engram_postflight",
        ) {
            let cand_id = cand.candidate_id.clone();
            generated_candidates.push(cand);
            human_action = Some(format!(
                "Promote candidate via: engram memory promote {} --by <name> --reason \"<rationale>\"",
                cand_id
            ));
        }
    }

    let res = PostflightResult {
        protocol_version: PROTOCOL_VERSION.to_string(),
        contract_id: contract.contract_id.clone(),
        task_goal: contract.task_goal,
        baseline_commit: revision(&root),
        verdict,
        exit_code,
        total_changed_files: changed_files.len(),
        expected_count,
        expansion_count,
        unexpected_count,
        boundary_violation_count,
        changed_files,
        classifications,
        constitution_passed,
        constitution_violations: rule_violations,
        generated_candidates,
        human_action_required: human_action,
        summary,
    };

    let _ = crate::telemetry::record_event(
        &root,
        "POSTFLIGHT_CHECK",
        Some(&res.contract_id),
        "agent_postflight",
        serde_json::json!({
            "verdict": format!("{:?}", res.verdict),
            "exit_code": res.exit_code,
            "total_changed_files": res.total_changed_files,
            "boundary_violations": res.boundary_violation_count,
            "candidates_count": res.generated_candidates.len(),
        }),
    );

    Ok(res)
}

/// Format human-readable Pre-Flight report for terminal output.
pub fn format_preflight_report(res: &PreflightResult) -> String {
    let mut out = String::new();
    out.push_str("🛫 Engram Pre-Flight Handshake\n");
    out.push_str("──────────────────────────────────────────────────────────────────────────\n");
    out.push_str(&format!("TASK: {}\n\n", res.task_goal));

    out.push_str("CONTEXT:\n");
    out.push_str(&format!(
        "  • Packed Files:  {} files / {} tokens\n",
        res.context_files, res.context_tokens
    ));
    if res.ambiguity_gaps.is_empty() {
        out.push_str("  • Ambiguity:     0 gaps detected ✓\n");
    } else {
        out.push_str(&format!(
            "  • Ambiguity:     {} gap(s) detected:\n",
            res.ambiguity_gaps.len()
        ));
        for g in &res.ambiguity_gaps {
            out.push_str(&format!("      ⚠️ {}\n", g));
        }
    }
    out.push_str("\n");

    out.push_str("IMPACT BLAST RADIUS (Predicted):\n");
    out.push_str(&format!(
        "  • Predicted Affected Targets: {}\n",
        res.predicted_impact_count
    ));
    for p in res.predicted_impact_paths.iter().take(4) {
        out.push_str(&format!("      • {}\n", p));
    }
    if res.predicted_impact_paths.len() > 4 {
        out.push_str(&format!(
            "      ... and {} more\n",
            res.predicted_impact_paths.len() - 4
        ));
    }
    out.push_str("\n");

    if !res.governing_policies.is_empty() {
        out.push_str("GOVERNING ARCHITECTURAL DECISIONS:\n");
        for p in &res.governing_policies {
            out.push_str(&format!(
                "  • [{}] {} (Scope: {})\n",
                p.id, p.title, p.scope
            ));
            out.push_str(&format!("      Statement: {}\n", p.statement));
            out.push_str(&format!("      Rationale: {}\n", p.rationale));
        }
        out.push_str("  ✓ Prior rationale retrieved directly (Reduced rediscovery)\n\n");
    } else {
        out.push_str("GOVERNING ARCHITECTURAL DECISIONS:\n");
        out.push_str("  • No prior governing policy constraining this area.\n\n");
    }

    out.push_str("CHANGE CONTRACT (Locked):\n");
    out.push_str(&format!(
        "  • Contract ID:  {} ({})\n",
        res.contract_id,
        if res.is_reused_contract {
            "Reused"
        } else {
            "New"
        }
    ));
    out.push_str(&format!(
        "  • Expected:     {}\n",
        res.expected_files.join(", ")
    ));
    out.push_str(&format!(
        "  • Allowed Exp:  {}\n",
        res.expected_modules.join(", ")
    ));
    if !res.forbidden_boundaries.is_empty() {
        out.push_str(&format!(
            "  • Forbidden:    {}\n",
            res.forbidden_boundaries.join(", ")
        ));
    }
    out.push_str(&format!("  • Baseline:     {}\n\n", res.baseline_commit));

    out.push_str(&format!("STATUS: {:?} - {}\n", res.status, res.message));
    out
}

/// Format human-readable Post-Flight report for terminal output or CI summary.
pub fn format_postflight_report(res: &PostflightResult, ci_mode: bool) -> String {
    let mut out = String::new();
    if ci_mode {
        out.push_str("### 🛬 Engram Post-Flight Change Verification\n\n");
        out.push_str(&format!(
            "**Contract ID:** `{}` | **Baseline:** `{}`\n\n",
            res.contract_id, res.baseline_commit
        ));
        out.push_str("| Metric | Value |\n|---|---|\n");
        out.push_str(&format!(
            "| Total Changes | {} |\n",
            res.total_changed_files
        ));
        out.push_str(&format!("| Expected | {} |\n", res.expected_count));
        out.push_str(&format!(
            "| Expected Expansion | {} |\n",
            res.expansion_count
        ));
        out.push_str(&format!("| Unexpected | {} |\n", res.unexpected_count));
        out.push_str(&format!(
            "| Boundary Violations | {} |\n\n",
            res.boundary_violation_count
        ));
        out.push_str(&format!(
            "**Verdict:** `{:?}` (Exit Code: {})\n\n",
            res.verdict, res.exit_code
        ));
        out.push_str(&format!("**Summary:** {}\n", res.summary));
        return out;
    }

    out.push_str("🛬 Engram Post-Flight Change Verification\n");
    out.push_str("──────────────────────────────────────────────────────────────────────────\n");
    out.push_str(&format!(
        "Contract: {} | Baseline: {}\n\n",
        res.contract_id, res.baseline_commit
    ));

    out.push_str("MUTATION SUMMARY:\n");
    out.push_str(&format!(
        "  • Total Changed Files:  {}\n",
        res.total_changed_files
    ));
    out.push_str(&format!(
        "  • Expected:             {}\n",
        res.expected_count
    ));
    out.push_str(&format!(
        "  • Expected Expansion:   {}\n",
        res.expansion_count
    ));
    out.push_str(&format!(
        "  • Unexpected:           {}\n",
        res.unexpected_count
    ));
    out.push_str(&format!(
        "  • Boundary Violations:  {}\n\n",
        res.boundary_violation_count
    ));

    out.push_str("POLICY & CONTRACT CHECKS:\n");
    if res.boundary_violation_count == 0 {
        out.push_str("  ✓ Elastic Contract:  PASS (Within authorized boundaries)\n");
    } else {
        out.push_str("  ❌ Elastic Contract:  FAIL (Boundary violations detected)\n");
    }
    if res.constitution_passed {
        out.push_str("  ✓ Constitution:      PASS (No blocking rule errors)\n");
    } else {
        out.push_str("  ❌ Constitution:      FAIL (Rule violations detected)\n");
    }
    out.push_str("\n");

    if !res.generated_candidates.is_empty() {
        out.push_str("CANDIDATE COMPOUNDING (Zero-Poisoning Gate):\n");
        for c in &res.generated_candidates {
            out.push_str(&format!("  • Synthesized Candidate: {}\n", c.candidate_id));
            out.push_str(&format!("      Title:     {}\n", c.title));
            out.push_str(&format!("      Statement: {}\n", c.statement));
            out.push_str("      Status:    PENDING HUMAN RATIFICATION (Not durable)\n");
        }
        if let Some(ref act) = res.human_action_required {
            out.push_str(&format!("  Action Required: {}\n", act));
        }
        out.push_str("\n");
    }

    out.push_str(&format!(
        "VERDICT: {:?} (Exit Code: {})\n",
        res.verdict, res.exit_code
    ));
    out.push_str(&format!("Summary: {}\n", res.summary));
    out
}

// ---------------------------------------------------------------------------
// Iteration 2B: Local Deterministic Postflight Prototype
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocalScopeStatus {
    Compliant,
    UnexpectedExpansion,
    BoundaryViolation,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocalEvidenceStatus {
    VerifiedByTests,
    BuildPassed,
    MissingValidationEvidence,
    ValidationFailed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocalStalenessStatus {
    Fresh,
    StaleBaselineDiverged,
    MissingBaselineCommit,
    GitUnavailable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocalPostflightVerdict {
    /// Fully compliant: Scope is within bounds AND required test/build evidence was verified.
    Pass,
    /// Scope is within bounds, but no test/build evidence was executed or provided.
    /// Explicitly does NOT claim functional correctness.
    PassScopeOnlyUnverified,
    /// Blocked due to boundary violation (forbidden files modified) or unauthorized expansion.
    BlockedScopeViolation,
    /// Blocked because repository baseline has diverged since contract creation.
    BlockedStaleContract,
    /// Blocked due to constitution architectural rule violation.
    BlockedConstitutionRule,
    /// Blocked because test or build validation explicitly failed.
    BlockedValidationFailed,
    /// Git repository or diff information is missing, corrupt, or inaccessible.
    IndeterminateGitUnavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalPostflightOptions {
    pub contract_id: Option<String>,
    pub staged: bool,
    pub strict: bool,
    pub test_evidence: Option<bool>,
    pub build_evidence: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalPostflightReport {
    pub contract_id: String,
    pub baseline_commit: String,
    pub current_head: String,
    pub verdict: LocalPostflightVerdict,
    pub scope_status: LocalScopeStatus,
    pub evidence_status: LocalEvidenceStatus,
    pub staleness_status: LocalStalenessStatus,
    pub changed_files: Vec<String>,
    pub expected_files: Vec<String>,
    pub unexpected_files: Vec<String>,
    pub boundary_violations: Vec<String>,
    pub constitution_violations: Vec<String>,
    pub summary: String,
    pub exit_code: i32,
}

/// Executes deterministic, local postflight validation directly against Git and the Change Contract.
/// Does NOT invoke any LLM or external paid services.
/// Distinguishes scope compliance from functional correctness and missing validation evidence.
pub fn validate_local_postflight(
    root: &Path,
    opts: &LocalPostflightOptions,
) -> Result<LocalPostflightReport> {
    let root = root_path(root)?;

    // 1. Resolve Contract
    let contract: ChangeContract = if let Some(ref id) = opts.contract_id {
        get_contract(&root, id)?
    } else {
        get_active_contract(&root)?
            .ok_or_else(|| anyhow!("No active Change Contract found. Run 'engram preflight \"<task>\"' or 'engram prepare' before validation."))?
    };

    // 2. Git Availability & Current HEAD
    let git_head_out = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&root)
        .output();

    let (current_head, git_available) = match git_head_out {
        Ok(out) if out.status.success() => {
            let h = String::from_utf8_lossy(&out.stdout).trim().to_string();
            (h, true)
        }
        _ => ("UNKNOWN".to_string(), false),
    };

    if !git_available {
        return Ok(LocalPostflightReport {
            contract_id: contract.contract_id.clone(),
            baseline_commit: contract.provenance.baseline_commit.clone(),
            current_head,
            verdict: LocalPostflightVerdict::IndeterminateGitUnavailable,
            scope_status: LocalScopeStatus::Compliant,
            evidence_status: LocalEvidenceStatus::MissingValidationEvidence,
            staleness_status: LocalStalenessStatus::GitUnavailable,
            changed_files: vec![],
            expected_files: contract.boundary.expected_files.clone(),
            unexpected_files: vec![],
            boundary_violations: vec![],
            constitution_violations: vec![],
            summary: "INDETERMINATE: Git repository is unavailable or inaccessible. Cannot establish diff.".to_string(),
            exit_code: 2,
        });
    }

    // 3. Staleness Detection: Check if repository baseline has diverged
    let baseline = &contract.provenance.baseline_commit;
    let staleness_status = if baseline.is_empty() || baseline == "HEAD" || baseline == "WORKTREE" {
        LocalStalenessStatus::Fresh
    } else {
        let exists = std::process::Command::new("git")
            .args(["cat-file", "-e", &format!("{}^{{commit}}", baseline)])
            .current_dir(&root)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if !exists {
            LocalStalenessStatus::MissingBaselineCommit
        } else if baseline != &current_head {
            // Check if any commits between baseline and current_head modified governed files
            let diff_between = std::process::Command::new("git")
                .args(["diff", "--name-only", baseline, &current_head])
                .current_dir(&root)
                .output();

            let has_intervening_changes = match diff_between {
                Ok(out) if out.status.success() => {
                    let files: Vec<String> = String::from_utf8_lossy(&out.stdout)
                        .lines()
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty())
                        .collect();
                    files
                        .iter()
                        .any(|f| contract.boundary.expected_files.contains(f))
                }
                _ => false,
            };

            if has_intervening_changes {
                LocalStalenessStatus::StaleBaselineDiverged
            } else {
                LocalStalenessStatus::Fresh
            }
        } else {
            LocalStalenessStatus::Fresh
        }
    };

    // 4. Discover Changed Files
    let changed_files = if opts.staged {
        let out = std::process::Command::new("git")
            .args(["diff", "--name-only", "--cached"])
            .current_dir(&root)
            .output()?;
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    } else {
        git_changed(&root)
    };

    // 5. Elastic Contract Diff Evaluation
    let (classifications, _missing_changes, summary_stats) =
        crate::contract::validate_contract_diff_with_constitution_and_base(
            &root,
            &contract,
            &changed_files,
            None,
        );

    let mut unexpected_files = Vec::new();
    let mut boundary_violations = Vec::new();

    for c in &classifications {
        match c.category {
            crate::contract::FileValidationCategory::Unexpected => {
                unexpected_files.push(c.path.clone());
            }
            crate::contract::FileValidationCategory::BoundaryViolation => {
                boundary_violations.push(c.path.clone());
            }
            _ => {}
        }
    }

    let scope_status = if summary_stats.violation_count > 0 {
        LocalScopeStatus::BoundaryViolation
    } else if summary_stats.unexpected_count > 0 {
        LocalScopeStatus::UnexpectedExpansion
    } else {
        LocalScopeStatus::Compliant
    };

    // 6. Constitution Rule Verification
    let conn = db(&root)?;
    let rule_violations = evaluate_rules(&root, &changed_files, &conn)?;
    let blocking_violations: Vec<String> = rule_violations
        .iter()
        .filter(|v| !v.overridden && v.severity == RuleSeverity::Error)
        .map(|v| format!("{}: {}", v.rule_id, v.message))
        .collect();

    // 7. Evidence Evaluation
    let evidence_status = match opts.test_evidence {
        Some(false) => LocalEvidenceStatus::ValidationFailed,
        Some(true) => LocalEvidenceStatus::VerifiedByTests,
        None => {
            if opts.build_evidence == Some(true) {
                LocalEvidenceStatus::BuildPassed
            } else {
                LocalEvidenceStatus::MissingValidationEvidence
            }
        }
    };

    // 8. Verdict Determination
    let (verdict, exit_code, summary) = if staleness_status
        == LocalStalenessStatus::StaleBaselineDiverged
    {
        (
            LocalPostflightVerdict::BlockedStaleContract,
            1,
            "BLOCKED: Contract baseline diverged from HEAD before changes were validated."
                .to_string(),
        )
    } else if scope_status == LocalScopeStatus::BoundaryViolation {
        (
            LocalPostflightVerdict::BlockedScopeViolation,
            1,
            format!(
                "BLOCKED: Boundary violation. Forbidden files modified: {:?}",
                boundary_violations
            ),
        )
    } else if scope_status == LocalScopeStatus::UnexpectedExpansion && opts.strict {
        (
            LocalPostflightVerdict::BlockedScopeViolation,
            1,
            format!(
                "BLOCKED: Unexpected changes outside contract boundary under --strict: {:?}",
                unexpected_files
            ),
        )
    } else if !blocking_violations.is_empty() {
        (
            LocalPostflightVerdict::BlockedConstitutionRule,
            1,
            format!(
                "BLOCKED: Architectural constitution rule violations: {:?}",
                blocking_violations
            ),
        )
    } else if evidence_status == LocalEvidenceStatus::ValidationFailed {
        (
            LocalPostflightVerdict::BlockedValidationFailed,
            1,
            "BLOCKED: Validation tests explicitly failed.".to_string(),
        )
    } else if evidence_status == LocalEvidenceStatus::VerifiedByTests
        || evidence_status == LocalEvidenceStatus::BuildPassed
    {
        (
            LocalPostflightVerdict::Pass,
            0,
            "SUCCESS: Mutation verified against contract boundaries, constitution, and validation evidence.".to_string(),
        )
    } else {
        (
            LocalPostflightVerdict::PassScopeOnlyUnverified,
            0,
            "PASS (SCOPE ONLY): Changed files are strictly within contract boundaries, but no test or build validation evidence was provided. Functional correctness is unverified.".to_string(),
        )
    };

    Ok(LocalPostflightReport {
        contract_id: contract.contract_id,
        baseline_commit: contract.provenance.baseline_commit,
        current_head,
        verdict,
        scope_status,
        evidence_status,
        staleness_status,
        changed_files,
        expected_files: contract.boundary.expected_files,
        unexpected_files,
        boundary_violations,
        constitution_violations: blocking_violations,
        summary,
        exit_code,
    })
}

/// Formats a human-readable Local Postflight report for terminal display.
pub fn format_local_postflight_report(report: &LocalPostflightReport) -> String {
    let mut out = String::new();
    out.push_str("🔍 Engram Local Deterministic Postflight Report\n");
    out.push_str("──────────────────────────────────────────────────────────────────────────\n");
    out.push_str(&format!("CONTRACT:   {}\n", report.contract_id));
    out.push_str(&format!(
        "BASELINE:   {} (HEAD: {})\n",
        report.baseline_commit, report.current_head
    ));
    out.push_str(&format!(
        "VERDICT:    {:?} (Exit Code: {})\n",
        report.verdict, report.exit_code
    ));
    out.push_str(&format!("SCOPE:      {:?}\n", report.scope_status));
    out.push_str(&format!("EVIDENCE:   {:?}\n", report.evidence_status));
    out.push_str(&format!("STALENESS:  {:?}\n", report.staleness_status));
    out.push_str(&format!("SUMMARY:    {}\n\n", report.summary));

    out.push_str(&format!(
        "CHANGED FILES ({}):\n",
        report.changed_files.len()
    ));
    for f in &report.changed_files {
        out.push_str(&format!("  • {}\n", f));
    }
    if !report.unexpected_files.is_empty() {
        out.push_str(&format!(
            "\nUNEXPECTED EXPANSIONS ({}):\n",
            report.unexpected_files.len()
        ));
        for f in &report.unexpected_files {
            out.push_str(&format!("  ⚠ {}\n", f));
        }
    }
    if !report.boundary_violations.is_empty() {
        out.push_str(&format!(
            "\nBOUNDARY VIOLATIONS ({}):\n",
            report.boundary_violations.len()
        ));
        for f in &report.boundary_violations {
            out.push_str(&format!("  ⛔ {}\n", f));
        }
    }
    out
}
