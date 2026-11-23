//! AMstraLift Enterprise Modernization & Dependency-Upgrade Ingestion Adapter (Phase 9.0).
//!
//! Provides zero-trust ingestion, validation, and contract mapping for AMstraLift:
//! 1. Modernization Plan ingestion (YAML/JSON) and path safety validation.
//! 2. Automatic mapping of planned transformations to Engram Change Contract boundaries.
//! 3. Constitutional boundary and sensitive file enforcement.
//! 4. Ingestion and audit verification of transformation and refusal evidence records.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::Path};

use crate::storage::root_path;

// ---------------------------------------------------------------------------
// AMstraLift Declarative Plan Data Models
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlannedTransformation {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub risk: Option<String>,
    #[serde(default = "default_status_safe")]
    pub status: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub prerequisites: Vec<String>,
    #[serde(default)]
    pub affected_file_patterns: Vec<String>,
    #[serde(default)]
    pub atomic_boundary: Option<String>,
}

fn default_status_safe() -> String {
    "safe".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionPhase {
    pub phase_index: usize,
    pub name: String,
    pub phase_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub transformations: Vec<PlannedTransformation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModernizationPlan {
    pub plan_id: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub repo_path: Option<String>,
    pub ecosystem: String,
    #[serde(default)]
    pub current_version: Option<String>,
    #[serde(default)]
    pub target_version: Option<String>,
    #[serde(default)]
    pub target_major: Option<u32>,
    #[serde(default)]
    pub total_transformations: usize,
    #[serde(default)]
    pub safe_count: usize,
    #[serde(default)]
    pub dependent_count: usize,
    #[serde(default)]
    pub risky_count: usize,
    #[serde(default)]
    pub refused_count: usize,
    #[serde(default)]
    pub phases: Vec<ExecutionPhase>,
    #[serde(default)]
    pub refused_transformations: Vec<PlannedTransformation>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// AMstraLift Evidence & Refusal Data Models
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransformationRecord {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub adapter: Option<String>,
    #[serde(default)]
    pub rule_id: Option<String>,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub ast_nodes_changed: usize,
    #[serde(default)]
    pub dependencies_changed: Vec<String>,
    #[serde(default)]
    pub diff_snippet: Option<String>,
    #[serde(default)]
    pub is_deterministic: bool,
    #[serde(default)]
    pub sandbox_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RefusalRecord {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub adapter: Option<String>,
    #[serde(default)]
    pub rule_id: Option<String>,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub code_modified: bool,
    #[serde(default = "default_true")]
    pub developer_review_required: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransformationResult {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub adapter: Option<String>,
    pub rule_id: String,
    pub file_path: String,
    pub decision: String, // "APPLIED" or "REFUSED"
    #[serde(default)]
    pub transformation_record: Option<TransformationRecord>,
    #[serde(default)]
    pub refusal_record: Option<RefusalRecord>,
    #[serde(default)]
    pub ast_nodes_changed: usize,
    #[serde(default)]
    pub files_modified: usize,
    #[serde(default)]
    pub is_deterministic: bool,
}

// ---------------------------------------------------------------------------
// Validation & Ingestion Reports
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanValidationFinding {
    pub severity: String, // "BLOCK" | "WARN" | "INFO"
    pub code: String,
    pub message: String,
    pub file_pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanValidationReport {
    pub valid: bool,
    pub plan_id: String,
    pub ecosystem: String,
    pub total_phases: usize,
    pub total_transformations: usize,
    pub expected_files: Vec<String>,
    pub expected_modules: Vec<String>,
    pub findings: Vec<PlanValidationFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceVerificationReport {
    pub valid: bool,
    pub total_results: usize,
    pub applied_count: usize,
    pub refused_count: usize,
    pub files_modified: Vec<String>,
    pub files_refused_clean: Vec<String>,
    pub violations: Vec<String>,
}

// ---------------------------------------------------------------------------
// Core Ingestion & Validation APIs
// ---------------------------------------------------------------------------

/// Parses an AMstraLift modernization plan from YAML or JSON string.
pub fn parse_modernization_plan(content: &str) -> Result<ModernizationPlan> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Modernization plan content is empty"));
    }

    // Try YAML first (YAML is a superset of JSON)
    serde_yaml::from_str::<ModernizationPlan>(trimmed)
        .or_else(|_| serde_json::from_str::<ModernizationPlan>(trimmed))
        .with_context(
            || "Failed to parse AMstraLift ModernizationPlan: invalid YAML or JSON schema",
        )
}

/// Reads and parses an AMstraLift modernization plan from a file on disk.
pub fn load_modernization_plan(path: &Path) -> Result<ModernizationPlan> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read modernization plan at '{}'", path.display()))?;
    parse_modernization_plan(&content)
}

/// Validates an AMstraLift modernization plan against path safety, sensitive files,
/// and the repository's Project Constitution.
pub fn validate_modernization_plan(
    plan: &ModernizationPlan,
    repo_root: &Path,
) -> Result<PlanValidationReport> {
    let root = root_path(repo_root)?;
    let mut findings = Vec::new();
    let mut expected_files = HashSet::new();
    let mut expected_modules = HashSet::new();
    let mut total_transformations = 0;

    // Load Project Constitution if present to check for forbidden boundaries
    let constitution_rules = crate::constitution::load_constitution(&root).unwrap_or_default();

    for phase in &plan.phases {
        for t in &phase.transformations {
            total_transformations += 1;

            for pattern in &t.affected_file_patterns {
                let norm = pattern.replace('\\', "/");
                let clean = norm.trim_start_matches("./");

                // 1. Path traversal check
                if clean.contains("..") || clean.starts_with('/') || clean.contains(':') {
                    findings.push(PlanValidationFinding {
                        severity: "BLOCK".to_string(),
                        code: "AMSTRA-SEC-001".to_string(),
                        message: format!(
                            "Transformation '{}' contains unsafe path traversal pattern '{}'",
                            t.id, pattern
                        ),
                        file_pattern: Some(pattern.clone()),
                    });
                    continue;
                }

                // 2. Sensitive / credential file check
                if crate::prompt::is_security_file_path(clean) {
                    findings.push(PlanValidationFinding {
                        severity: "BLOCK".to_string(),
                        code: "AMSTRA-SEC-002".to_string(),
                        message: format!(
                            "Transformation '{}' targets security/credential file '{}'; automated upgrade forbidden",
                            t.id, pattern
                        ),
                        file_pattern: Some(pattern.clone()),
                    });
                    continue;
                }

                // 3. Sovereign policy file check
                if crate::constitution::is_policy_file(clean) {
                    findings.push(PlanValidationFinding {
                        severity: "BLOCK".to_string(),
                        code: "AMSTRA-SEC-003".to_string(),
                        message: format!(
                            "Transformation '{}' targets sovereign policy file '{}'; requires human governance PR",
                            t.id, pattern
                        ),
                        file_pattern: Some(pattern.clone()),
                    });
                    continue;
                }

                // 4. Constitution forbidden boundary check
                for rule in &constitution_rules {
                    if let Some(ref forbidden) = rule.forbidden {
                        if crate::contract::matches_boundary(clean, forbidden) {
                            findings.push(PlanValidationFinding {
                                severity: "BLOCK".to_string(),
                                code: format!("AMSTRA-CONST-{}", rule.id),
                                message: format!(
                                    "Transformation '{}' violates Constitution rule '{}' ({}) forbidding '{}'",
                                    t.id, rule.name, rule.id, forbidden
                                ),
                                file_pattern: Some(pattern.clone()),
                            });
                        }
                    }
                }

                // Collect authorized targets
                if pattern.contains('*') {
                    // Extract directory prefix as expected module
                    if let Some((dir, _)) = clean.split_once('*') {
                        let trimmed_dir = dir.trim_end_matches('/');
                        if !trimmed_dir.is_empty() {
                            expected_modules.insert(trimmed_dir.to_string());
                        }
                    }
                } else {
                    expected_files.insert(clean.to_string());
                }
            }
        }
    }

    let has_blocks = findings.iter().any(|f| f.severity == "BLOCK");

    let mut sorted_files: Vec<String> = expected_files.into_iter().collect();
    sorted_files.sort();
    let mut sorted_modules: Vec<String> = expected_modules.into_iter().collect();
    sorted_modules.sort();

    Ok(PlanValidationReport {
        valid: !has_blocks,
        plan_id: plan.plan_id.clone(),
        ecosystem: plan.ecosystem.clone(),
        total_phases: plan.phases.len(),
        total_transformations,
        expected_files: sorted_files,
        expected_modules: sorted_modules,
        findings,
    })
}

/// Generates an authorized Engram Change Contract from a validated AMstraLift modernization plan.
pub fn generate_contract_for_plan(
    repo_root: &Path,
    plan: &ModernizationPlan,
    human_rationale: &str,
) -> Result<crate::contract::ChangeContract> {
    let root = root_path(repo_root)?;
    let report = validate_modernization_plan(plan, &root)?;

    if !report.valid {
        let block_msgs: Vec<String> = report
            .findings
            .iter()
            .filter(|f| f.severity == "BLOCK")
            .map(|f| format!("[{}] {}", f.code, f.message))
            .collect();
        return Err(anyhow!(
            "Cannot create contract for rejected AMstraLift plan '{}':\n{}",
            plan.plan_id,
            block_msgs.join("\n")
        ));
    }

    let goal = format!(
        "AMstraLift Modernization (Plan {}): Upgrade {} from {} to {}",
        plan.plan_id,
        plan.ecosystem,
        plan.current_version.as_deref().unwrap_or("detected"),
        plan.target_version.as_deref().unwrap_or("target")
    );

    let mut contract = crate::contract::create_contract(
        &root,
        &goal,
        if report.expected_modules.is_empty() {
            None
        } else {
            Some(report.expected_modules.clone())
        },
        if report.expected_files.is_empty() {
            None
        } else {
            Some(report.expected_files.clone())
        },
        Some(vec!["secrets/**".to_string(), ".git/**".to_string()]),
    )?;

    // Record AMstraLift plan provenance metadata
    contract.contract_id = format!("ctr_amstralift_{}", plan.plan_id);
    contract.baseline_revision = crate::git::resolve_baseline(&root, "HEAD")
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_else(|| "HEAD".to_string());

    let _ = crate::telemetry::record_event(
        &root,
        "AMSTRALIFT_PLAN_INGESTED",
        Some(&contract.contract_id),
        "amstralift_adapter",
        serde_json::json!({
            "plan_id": plan.plan_id,
            "ecosystem": plan.ecosystem,
            "total_transformations": report.total_transformations,
            "expected_files_count": report.expected_files.len(),
            "expected_modules_count": report.expected_modules.len(),
            "rationale": human_rationale,
        }),
    );

    crate::contract::save_contract(&root, &contract)?;
    Ok(contract)
}

/// Parses AMstraLift transformation and refusal evidence results.
pub fn parse_transformation_evidence(content: &str) -> Result<Vec<TransformationResult>> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    if let Ok(vec) = serde_json::from_str::<Vec<TransformationResult>>(trimmed) {
        return Ok(vec);
    }
    if let Ok(single) = serde_json::from_str::<TransformationResult>(trimmed) {
        return Ok(vec![single]);
    }
    if let Ok(vec) = serde_yaml::from_str::<Vec<TransformationResult>>(trimmed) {
        return Ok(vec);
    }
    if let Ok(single) = serde_yaml::from_str::<TransformationResult>(trimmed) {
        return Ok(vec![single]);
    }

    serde_json::from_str::<Vec<TransformationResult>>(trimmed)
        .or_else(|_| serde_yaml::from_str::<Vec<TransformationResult>>(trimmed))
        .with_context(|| "Failed to parse AMstraLift TransformationResult evidence payload")
}

/// Verifies AMstraLift transformation evidence records:
/// Guarantees that refusals strictly preserved source code untouched (`code_modified == false`).
pub fn verify_transformation_evidence(
    results: &[TransformationResult],
) -> EvidenceVerificationReport {
    let mut applied_count = 0;
    let mut refused_count = 0;
    let mut files_modified = Vec::new();
    let mut files_refused_clean = Vec::new();
    let mut violations = Vec::new();

    for res in results {
        let clean_path = res.file_path.replace('\\', "/");

        // Verify refusal invariant
        if res.decision.eq_ignore_ascii_case("REFUSED") {
            refused_count += 1;
            if let Some(ref refusal) = res.refusal_record {
                if refusal.code_modified {
                    violations.push(format!(
                        "SAFETY BREACH: Refusal record for '{}' indicates code_modified=true (must be strictly false)",
                        clean_path
                    ));
                } else {
                    files_refused_clean.push(clean_path.clone());
                }
            } else {
                violations.push(format!(
                    "Missing refusal_record detail for refused transformation on '{}'",
                    clean_path
                ));
            }
        } else if res.decision.eq_ignore_ascii_case("APPLIED") {
            applied_count += 1;
            files_modified.push(clean_path.clone());
        }
    }

    let is_valid = violations.is_empty();

    EvidenceVerificationReport {
        valid: is_valid,
        total_results: results.len(),
        applied_count,
        refused_count,
        files_modified,
        files_refused_clean,
        violations,
    }
}
