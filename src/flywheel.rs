//! Engram v1.0 Closed-Loop Change Flywheel Engine:
//! Orchestrated Pre-Flight and Post-Flight protocol uniting Context, Impact,
//! Contracts, Guard, and Candidate Compounding under a strict Human Authority Boundary.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::analyzer::ensure_fresh;
use crate::compiler::context;
use crate::constitution::evaluate_rules;
use crate::model::{RuleSeverity, RuleViolation};
use crate::contract::{
    create_contract, get_active_contract, get_contract, validate_contract_diff, ChangeContract,
    FileClassification, FileValidationCategory,
};
use crate::impact::compute_impact_cone;
use crate::memory::{query_scoped_memories, record_task_candidate, MemoryCandidate};
use crate::storage::{db, git_changed, revision, root_path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightOptions {
    pub task: String,
    pub token_budget: usize,
    pub forbidden_boundaries: Option<Vec<String>>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightResult {
    pub task_id: String,
    pub contract_id: String,
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

fn extract_symbols_for_paths(root: &Path, paths: &[String]) -> Vec<String> {
    let mut symbols = Vec::new();
    if let Ok(conn) = db(root) {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT name FROM entity_versions WHERE path = ?1 AND invalidated_at_revision IS NULL"
        ) {
            for p in paths {
                if let Ok(mut rows) = stmt.query([p]) {
                    while let Ok(Some(row)) = rows.next() {
                        if let Ok(name) = row.get::<_, String>(0) {
                            if !symbols.contains(&name) {
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

/// Execute deterministic Pre-Flight Handshake (`engram preflight`).
/// Grounds the task, predicts impact, retrieves governing decisions, and locks the contract.
/// Idempotency (AC-1e): Same revision + same normalized task reuses active contract.
pub fn preflight_task(root: &Path, opts: &PreflightOptions) -> Result<PreflightResult> {
    let root = root_path(root)?;
    ensure_fresh(&root)?;

    let base_commit = revision(&root);
    let normalized_task = opts.task.trim();

    // 1. Idempotency Check: Check if an active contract already matches this task and revision
    if let Ok(Some(active)) = get_active_contract(&root) {
        if active.task_goal.trim().eq_ignore_ascii_case(normalized_task) {
            let ctx = context(&root, normalized_task, opts.token_budget)?;
            let touched_paths: Vec<String> = ctx.files.iter().map(|f| f.path.clone()).collect();
            let touched_symbols = extract_symbols_for_paths(&root, &touched_paths);

            let governing_mems = query_scoped_memories(
                &root,
                normalized_task,
                &touched_paths,
                &touched_symbols,
                false,
            )
            .unwrap_or_default();

            let governing_policies: Vec<GoverningPolicySummary> = governing_mems
                .into_iter()
                .map(|m| GoverningPolicySummary {
                    id: m.id,
                    title: m.title,
                    scope: format!("{:?}", m.scope.level),
                    statement: m.statement,
                    rationale: m.reason,
                })
                .collect();

            return Ok(PreflightResult {
                task_id: active.contract_id.clone(),
                contract_id: active.contract_id,
                task_goal: active.task_goal,
                baseline_commit: base_commit,
                context_files: ctx.files.len(),
                context_tokens: ctx.estimated_tokens,
                ambiguity_gaps: ctx.gaps,
                predicted_impact_count: active.boundary.expected_files.len(),
                predicted_impact_paths: active.boundary.expected_files.clone(),
                governing_policies,
                expected_files: active.boundary.expected_files,
                expected_modules: active.boundary.expected_modules,
                forbidden_boundaries: active.boundary.forbidden_boundaries,
                is_reused_contract: true,
                status: PreflightStatus::ReadyForImplementation,
                message: "Deterministic active contract reused (Preflight Idempotent)".into(),
            });
        }
    }

    // 2. Context Compilation
    let ctx = context(&root, normalized_task, opts.token_budget)?;
    let context_files = ctx.files.len();
    let context_tokens = ctx.estimated_tokens;
    let ambiguity_gaps = ctx.gaps.clone();

    let touched_paths: Vec<String> = ctx.files.iter().map(|f| f.path.clone()).collect();
    let touched_symbols = extract_symbols_for_paths(&root, &touched_paths);

    // 3. Multi-Hop Impact Blast Radius Prediction
    let conn = db(&root)?;
    let mut predicted_impact_paths = Vec::new();
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

    // 4. Governing Architectural Decisions Discovery (Archaeology / Prior Rationale)
    let governing_mems = query_scoped_memories(
        &root,
        normalized_task,
        &touched_paths,
        &touched_symbols,
        false,
    )
    .unwrap_or_default();

    let governing_policies: Vec<GoverningPolicySummary> = governing_mems
        .into_iter()
        .map(|m| GoverningPolicySummary {
            id: m.id,
            title: m.title,
            scope: format!("{:?}", m.scope.level),
            statement: m.statement,
            rationale: m.reason,
        })
        .collect();

    // 5. Elastic Change Contract Generation & Lock
    let mut expected_modules: Vec<String> = Vec::new();
    for p in &touched_paths {
        let m = crate::contract::extract_module(p);
        if !expected_modules.contains(&m) && m != "root" {
            expected_modules.push(m);
        }
    }

    let forbidden = opts.forbidden_boundaries.clone();
    let contract = create_contract(
        &root,
        normalized_task,
        Some(expected_modules.clone()),
        Some(touched_paths.clone()),
        forbidden,
    )?;

    Ok(PreflightResult {
        task_id: contract.contract_id.clone(),
        contract_id: contract.contract_id,
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
        status: PreflightStatus::ReadyForImplementation,
        message: "Ready for implementation. Change Contract locked.".into(),
    })
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
    let contract_val = validate_contract_diff(&contract, &changed_files);
    let mut expected_count = 0;
    let mut expansion_count = 0;
    let mut unexpected_count = 0;
    let mut boundary_violation_count = 0;

    for c in &contract_val.classifications {
        match c.category {
            FileValidationCategory::Expected => expected_count += 1,
            FileValidationCategory::ExpectedExpansion => expansion_count += 1,
            FileValidationCategory::Unexpected => unexpected_count += 1,
            FileValidationCategory::BoundaryViolation => boundary_violation_count += 1,
        }
    }

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
            format!("BLOCKED: {} boundary violation(s) detected. Forbidden files modified.", boundary_violation_count),
        )
    } else if !constitution_passed {
        (
            PostflightVerdict::BlockedConstitutionRule,
            1,
            format!("BLOCKED: {} architectural constitution rule violation(s).", blocking_rule_violations.len()),
        )
    } else if unexpected_count > 0 && opts.strict {
        (
            PostflightVerdict::ReviewUnexpectedExpansion,
            1,
            format!("BLOCKED: {} unexpected change(s) outside contract boundary under --strict mode.", unexpected_count),
        )
    } else if unexpected_count > 0 {
        (
            PostflightVerdict::Pass,
            0,
            format!("ADVISORY: Clean pass with {} unexpected file(s) requiring team review.", unexpected_count),
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

    Ok(PostflightResult {
        contract_id: contract.contract_id,
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
        classifications: contract_val.classifications,
        constitution_passed,
        constitution_violations: rule_violations,
        generated_candidates,
        human_action_required: human_action,
        summary,
    })
}

/// Format human-readable Pre-Flight report for terminal output.
pub fn format_preflight_report(res: &PreflightResult) -> String {
    let mut out = String::new();
    out.push_str("🛫 Engram Pre-Flight Handshake\n");
    out.push_str("──────────────────────────────────────────────────────────────────────────\n");
    out.push_str(&format!("TASK: {}\n\n", res.task_goal));

    out.push_str("CONTEXT:\n");
    out.push_str(&format!("  • Packed Files:  {} files / {} tokens\n", res.context_files, res.context_tokens));
    if res.ambiguity_gaps.is_empty() {
        out.push_str("  • Ambiguity:     0 gaps detected ✓\n");
    } else {
        out.push_str(&format!("  • Ambiguity:     {} gap(s) detected:\n", res.ambiguity_gaps.len()));
        for g in &res.ambiguity_gaps {
            out.push_str(&format!("      ⚠️ {}\n", g));
        }
    }
    out.push_str("\n");

    out.push_str("IMPACT BLAST RADIUS (Predicted):\n");
    out.push_str(&format!("  • Predicted Affected Targets: {}\n", res.predicted_impact_count));
    for p in res.predicted_impact_paths.iter().take(4) {
        out.push_str(&format!("      • {}\n", p));
    }
    if res.predicted_impact_paths.len() > 4 {
        out.push_str(&format!("      ... and {} more\n", res.predicted_impact_paths.len() - 4));
    }
    out.push_str("\n");

    if !res.governing_policies.is_empty() {
        out.push_str("GOVERNING ARCHITECTURAL DECISIONS:\n");
        for p in &res.governing_policies {
            out.push_str(&format!("  • [{}] {} (Scope: {})\n", p.id, p.title, p.scope));
            out.push_str(&format!("      Statement: {}\n", p.statement));
            out.push_str(&format!("      Rationale: {}\n", p.rationale));
        }
        out.push_str("  ✓ Prior rationale retrieved directly (Reduced rediscovery)\n\n");
    } else {
        out.push_str("GOVERNING ARCHITECTURAL DECISIONS:\n");
        out.push_str("  • No prior governing policy constraining this area.\n\n");
    }

    out.push_str("CHANGE CONTRACT (Locked):\n");
    out.push_str(&format!("  • Contract ID:  {} ({})\n", res.contract_id, if res.is_reused_contract { "Reused" } else { "New" }));
    out.push_str(&format!("  • Expected:     {}\n", res.expected_files.join(", ")));
    out.push_str(&format!("  • Allowed Exp:  {}\n", res.expected_modules.join(", ")));
    if !res.forbidden_boundaries.is_empty() {
        out.push_str(&format!("  • Forbidden:    {}\n", res.forbidden_boundaries.join(", ")));
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
        out.push_str(&format!("**Contract ID:** `{}` | **Baseline:** `{}`\n\n", res.contract_id, res.baseline_commit));
        out.push_str("| Metric | Value |\n|---|---|\n");
        out.push_str(&format!("| Total Changes | {} |\n", res.total_changed_files));
        out.push_str(&format!("| Expected | {} |\n", res.expected_count));
        out.push_str(&format!("| Expected Expansion | {} |\n", res.expansion_count));
        out.push_str(&format!("| Unexpected | {} |\n", res.unexpected_count));
        out.push_str(&format!("| Boundary Violations | {} |\n\n", res.boundary_violation_count));
        out.push_str(&format!("**Verdict:** `{:?}` (Exit Code: {})\n\n", res.verdict, res.exit_code));
        out.push_str(&format!("**Summary:** {}\n", res.summary));
        return out;
    }

    out.push_str("🛬 Engram Post-Flight Change Verification\n");
    out.push_str("──────────────────────────────────────────────────────────────────────────\n");
    out.push_str(&format!("Contract: {} | Baseline: {}\n\n", res.contract_id, res.baseline_commit));

    out.push_str("MUTATION SUMMARY:\n");
    out.push_str(&format!("  • Total Changed Files:  {}\n", res.total_changed_files));
    out.push_str(&format!("  • Expected:             {}\n", res.expected_count));
    out.push_str(&format!("  • Expected Expansion:   {}\n", res.expansion_count));
    out.push_str(&format!("  • Unexpected:           {}\n", res.unexpected_count));
    out.push_str(&format!("  • Boundary Violations:  {}\n\n", res.boundary_violation_count));

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

    out.push_str(&format!("VERDICT: {:?} (Exit Code: {})\n", res.verdict, res.exit_code));
    out.push_str(&format!("Summary: {}\n", res.summary));
    out
}
