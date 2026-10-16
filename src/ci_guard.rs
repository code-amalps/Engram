//! CI/CD Change Guard: Policy-driven change governance across local Git and CI workflows.
//!
//! Evaluates working tree, staged index, or pull-request diffs against:
//! 1. Elastic Change Contracts (expected boundaries, forbidden boundaries, missing changes)
//! 2. Sovereign Project Constitution (architectural boundaries and forbidden patterns)
//! 3. Multi-Hop Impact Blast Radius (affected caller and dependency entity thresholds)
//!
//! Emits structured JSON findings, developer-friendly terminal output, and deterministic exit codes:
//! - 0: Pass or advisory Warn (continuation permitted)
//! - 1: Block (mandatory check failed or forbidden boundary breached)
//! - 2: Error (configuration error, malformed policy, fail-closed diagnostic)

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::constitution::{evaluate_rules, is_policy_file, load_constitution};
use crate::contract::{
    inspect_changed_files, matches_boundary, normalize_rel_path,
    validate_contract_diff_with_constitution_and_base, CheckOptions, FileValidationCategory,
};
use crate::impact::compute_impact_cone;
use crate::model::RuleSeverity;
use crate::storage::{db, root_path};

/// Overall Guard Status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuardStatus {
    Pass,
    Warn,
    Block,
    Error,
}

impl GuardStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warn => "warn",
            Self::Block => "block",
            Self::Error => "error",
        }
    }
}

/// Severity of a single finding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingSeverity {
    Info,
    Warn,
    Block,
}

impl FindingSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Block => "block",
        }
    }
}

/// A structured finding produced by Change Guard
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuardFinding {
    pub rule_id: String,
    pub severity: FindingSeverity,
    pub message: String,
    pub path: Option<String>,
    pub remediation: Option<String>,
}

/// Execution record of an individual check
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckExecutionRecord {
    pub name: String,
    pub status: String, // "executed", "skipped", "not_applicable", "failed"
    pub reason: Option<String>,
}

/// Context of the Guard execution run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardContext {
    pub repo_root: String,
    pub baseline_commit: Option<String>,
    pub target_snapshot: String,
    pub contract_id: Option<String>,
    pub policy_version: String,
}

/// Machine-readable complete Guard Result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardResult {
    pub status: GuardStatus,
    pub exit_code: i32,
    pub blocked: bool,
    pub context: GuardContext,
    pub checks_executed: Vec<String>,
    pub checks_skipped: Vec<CheckExecutionRecord>,
    pub findings: Vec<GuardFinding>,
    pub changed_files: Vec<String>,
    pub affected_symbols: Vec<String>,
    pub summary: String,
}

/// Runtime invocation options
#[derive(Debug, Clone, Default)]
pub struct GuardOptions {
    pub policy_path: Option<PathBuf>,
    pub base_commit: Option<String>,
    pub contract_file: Option<String>,
    pub staged: bool,
    pub strict: bool,
    pub diagnostic: bool,
}

// ---------------------------------------------------------------------------
// Policy Configuration (.engram/change-guard.toml)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractCheckPolicy {
    pub enabled: bool,
    pub required: bool,
    pub severity: FindingSeverity,
    pub strict: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstitutionCheckPolicy {
    pub enabled: bool,
    pub required: bool,
    pub severity: FindingSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactCheckPolicy {
    pub enabled: bool,
    pub max_affected_symbols: usize,
    pub severity: FindingSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardChecksPolicy {
    pub contracts: ContractCheckPolicy,
    pub constitution: ConstitutionCheckPolicy,
    pub impact: ImpactCheckPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuardExclusionsPolicy {
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardPolicy {
    pub version: String,
    pub checks: GuardChecksPolicy,
    pub exclusions: GuardExclusionsPolicy,
}

impl Default for GuardPolicy {
    fn default() -> Self {
        Self {
            version: "1".to_string(),
            checks: GuardChecksPolicy {
                contracts: ContractCheckPolicy {
                    enabled: true,
                    required: false,
                    severity: FindingSeverity::Block,
                    strict: false,
                },
                constitution: ConstitutionCheckPolicy {
                    enabled: true,
                    required: true,
                    severity: FindingSeverity::Block,
                },
                impact: ImpactCheckPolicy {
                    enabled: true,
                    max_affected_symbols: 50,
                    severity: FindingSeverity::Warn,
                },
            },
            exclusions: GuardExclusionsPolicy {
                paths: vec!["docs/**".to_string(), "*.md".to_string()],
            },
        }
    }
}

/// Parse and validate a TOML policy string with fail-closed security guarantees.
pub fn parse_policy_toml(content: &str) -> Result<GuardPolicy> {
    let mut policy = GuardPolicy::default();
    let mut current_section = String::new();
    let mut version_found = false;

    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let raw_line = lines[i].trim();
        i += 1;

        if raw_line.is_empty() || raw_line.starts_with('#') {
            continue;
        }

        // Section header
        if raw_line.starts_with('[') && raw_line.ends_with(']') {
            current_section = raw_line[1..raw_line.len() - 1].trim().to_string();
            continue;
        }

        // Key-value pair
        let (key, value_part) = match raw_line.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => {
                return Err(anyhow!(
                    "Syntax error in policy: expected 'key = value', found '{}'",
                    raw_line
                ));
            }
        };

        if current_section.is_empty() {
            if key == "version" {
                let v = value_part.trim_matches('"').trim_matches('\'');
                policy.version = v.to_string();
                version_found = true;
            } else {
                return Err(anyhow!("Unknown top-level property '{}' in policy", key));
            }
        } else if current_section == "checks.contracts" {
            match key {
                "enabled" => policy.checks.contracts.enabled = parse_bool(value_part)?,
                "required" | "require_contract" => {
                    policy.checks.contracts.required = parse_bool(value_part)?
                }
                "severity" => policy.checks.contracts.severity = parse_severity(value_part)?,
                "strict" => policy.checks.contracts.strict = parse_bool(value_part)?,
                _ => return Err(anyhow!("Unknown property '{}' in [checks.contracts]", key)),
            }
        } else if current_section == "checks.constitution" {
            match key {
                "enabled" => policy.checks.constitution.enabled = parse_bool(value_part)?,
                "required" => policy.checks.constitution.required = parse_bool(value_part)?,
                "severity" => policy.checks.constitution.severity = parse_severity(value_part)?,
                _ => {
                    return Err(anyhow!(
                        "Unknown property '{}' in [checks.constitution]",
                        key
                    ))
                }
            }
        } else if current_section == "checks.impact" {
            match key {
                "enabled" => policy.checks.impact.enabled = parse_bool(value_part)?,
                "max_affected_symbols" => {
                    policy.checks.impact.max_affected_symbols =
                        value_part.parse::<usize>().map_err(|_| {
                            anyhow!("Invalid integer for max_affected_symbols: '{}'", value_part)
                        })?;
                }
                "severity" => policy.checks.impact.severity = parse_severity(value_part)?,
                _ => return Err(anyhow!("Unknown property '{}' in [checks.impact]", key)),
            }
        } else if current_section == "exclusions" {
            if key == "paths" {
                let mut full_val = value_part.to_string();
                if !full_val.ends_with(']') {
                    // Multi-line array
                    while i < lines.len() {
                        let next_line = lines[i].trim();
                        i += 1;
                        full_val.push(' ');
                        full_val.push_str(next_line);
                        if next_line.contains(']') {
                            break;
                        }
                    }
                }
                policy.exclusions.paths = parse_string_array(&full_val)?;
            } else {
                return Err(anyhow!("Unknown property '{}' in [exclusions]", key));
            }
        } else {
            return Err(anyhow!("Unknown section '[{}]' in policy", current_section));
        }
    }

    if !version_found && !content.trim().is_empty() {
        return Err(anyhow!("Missing required 'version' field in policy"));
    }

    // Version validation
    if policy.version != "1" {
        return Err(anyhow!(
            "Unsupported policy version '{}'; currently only version '1' is supported",
            policy.version
        ));
    }

    // Safety validation for exclusion patterns
    for pattern in &policy.exclusions.paths {
        let p = pattern.trim();
        if p.is_empty() {
            return Err(anyhow!("Empty path exclusion pattern is not permitted"));
        }
        if p == "*" || p == "**" || p == "/" || p == "/*" || p == "/**" {
            return Err(anyhow!(
                "Broad wildcard exclusion '{}' is forbidden as it would silently disable change governance",
                p
            ));
        }
    }

    Ok(policy)
}

fn parse_bool(val: &str) -> Result<bool> {
    match val.trim().to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(anyhow!("Invalid boolean value: '{}'", val)),
    }
}

fn parse_severity(val: &str) -> Result<FindingSeverity> {
    let clean = val
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_ascii_lowercase();
    match clean.as_str() {
        "info" => Ok(FindingSeverity::Info),
        "warn" | "warning" => Ok(FindingSeverity::Warn),
        "block" | "error" => Ok(FindingSeverity::Block),
        _ => Err(anyhow!(
            "Invalid severity '{}'; expected 'info', 'warn', or 'block'",
            val
        )),
    }
}

fn parse_string_array(val: &str) -> Result<Vec<String>> {
    let trimmed = val.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Err(anyhow!(
            "Expected array bracketed by '[...]', found '{}'",
            val
        ));
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let mut items = Vec::new();
    for part in inner.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        let clean = p.trim_matches('"').trim_matches('\'');
        items.push(clean.to_string());
    }
    Ok(items)
}

/// Load policy from file or fallback to default
pub fn load_policy(root: &Path, custom_path: Option<&Path>) -> Result<(GuardPolicy, String)> {
    if let Some(path) = custom_path {
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        };
        if !abs_path.exists() {
            return Err(anyhow!(
                "Specified policy configuration file '{}' not found",
                abs_path.display()
            ));
        }
        let content = fs::read_to_string(&abs_path)?;
        let policy = parse_policy_toml(&content)?;
        return Ok((policy, abs_path.display().to_string()));
    }

    let default_repo_policy = root.join(".engram").join("change-guard.toml");
    if default_repo_policy.exists() {
        let content = fs::read_to_string(&default_repo_policy)?;
        let policy = parse_policy_toml(&content)?;
        return Ok((policy, default_repo_policy.display().to_string()));
    }

    Ok((GuardPolicy::default(), "default-in-memory".to_string()))
}

// ---------------------------------------------------------------------------
// Execution Engine
// ---------------------------------------------------------------------------

/// Main entry point: run Change Guard against repository working tree, staged index, or PR base
pub fn run_guard(root: &Path, options: &GuardOptions) -> GuardResult {
    let resolved_root = match root_path(root) {
        Ok(r) => r,
        Err(e) => {
            return make_error_result(
                &root.display().to_string(),
                None,
                "unknown",
                "0",
                format!("Failed resolving repository root path: {e}"),
            );
        }
    };

    // 1. Load Policy
    let (policy, policy_source) = match load_policy(&resolved_root, options.policy_path.as_deref())
    {
        Ok(p) => p,
        Err(e) => {
            return make_error_result(
                &resolved_root.display().to_string(),
                options.base_commit.clone(),
                "unknown",
                "0",
                format!("Configuration error loading Change Guard policy: {e}"),
            );
        }
    };

    let baseline_ref = options.base_commit.as_deref().unwrap_or("HEAD");

    // 2. Discover changed files
    let check_opts = CheckOptions {
        base_commit: options.base_commit.clone(),
        staged: options.staged,
        strict: options.strict || policy.checks.contracts.strict,
        file: options.contract_file.clone(),
        ..Default::default()
    };

    let (raw_changed_files, snapshot_name, maybe_err) =
        match inspect_changed_files(&resolved_root, baseline_ref, &check_opts) {
            Ok(res) => res,
            Err(e) => {
                return make_error_result(
                    &resolved_root.display().to_string(),
                    options.base_commit.clone(),
                    "unknown",
                    &policy.version,
                    format!("Git changed file inspection failed: {e}"),
                );
            }
        };

    if let Some(err_code) = maybe_err {
        return make_error_result(
            &resolved_root.display().to_string(),
            options.base_commit.clone(),
            &snapshot_name,
            &policy.version,
            format!("Git inspection returned error code: {:?}", err_code),
        );
    }

    // 3. Filter changed files against exclusion patterns
    let mut changed_files = Vec::new();
    for file in raw_changed_files {
        let norm = normalize_rel_path(&file);
        let clean = norm.trim_start_matches("./").trim_end_matches('/');

        // Always skip internal runtime directories
        if clean == ".git"
            || clean.starts_with(".git/")
            || clean.starts_with(".engram/cache")
            || clean.starts_with(".engram/sessions")
            || clean.starts_with(".engram/events")
            || clean.ends_with(".db")
            || clean.ends_with(".db-wal")
            || clean.ends_with(".db-shm")
        {
            continue;
        }

        let is_excluded = policy
            .exclusions
            .paths
            .iter()
            .any(|pat| matches_boundary(&norm, pat));

        if !is_excluded {
            changed_files.push(file);
        }
    }

    let mut checks_executed = Vec::new();
    let mut checks_skipped = Vec::new();
    let mut findings = Vec::new();
    let mut contract_id_used = None;

    // 4. Elastic Change Contract Check
    if !policy.checks.contracts.enabled {
        checks_skipped.push(CheckExecutionRecord {
            name: "contracts".to_string(),
            status: "skipped".to_string(),
            reason: Some("Change contract verification disabled in policy".to_string()),
        });
    } else {
        // Resolve contract: explicit file, active contract in repo
        let contract_opt = if let Some(ref file_path) = options.contract_file {
            let p = Path::new(file_path);
            let abs = if p.is_absolute() {
                p.to_path_buf()
            } else {
                resolved_root.join(p)
            };
            if !abs.exists() {
                findings.push(GuardFinding {
                    rule_id: "GUARD-CONTRACT-001".to_string(),
                    severity: FindingSeverity::Block,
                    message: format!("Specified contract file '{}' does not exist", abs.display()),
                    path: Some(abs.display().to_string()),
                    remediation: Some("Check path or provide valid contract file".to_string()),
                });
                None
            } else {
                match fs::read_to_string(&abs) {
                    Ok(data) => {
                        match serde_json::from_str::<crate::contract::ChangeContract>(&data) {
                            Ok(c) => Some(c),
                            Err(e) => {
                                findings.push(GuardFinding {
                                    rule_id: "GUARD-CONTRACT-001".to_string(),
                                    severity: FindingSeverity::Block,
                                    message: format!("Failed to parse contract file: {e}"),
                                    path: Some(abs.display().to_string()),
                                    remediation: Some(
                                        "Ensure contract file is valid JSON".to_string(),
                                    ),
                                });
                                None
                            }
                        }
                    }
                    Err(e) => {
                        findings.push(GuardFinding {
                            rule_id: "GUARD-CONTRACT-001".to_string(),
                            severity: FindingSeverity::Block,
                            message: format!("Failed to read contract file: {e}"),
                            path: Some(abs.display().to_string()),
                            remediation: None,
                        });
                        None
                    }
                }
            }
        } else {
            crate::contract::get_active_contract(&resolved_root).unwrap_or(None)
        };

        match contract_opt {
            Some(contract) => {
                contract_id_used = Some(contract.contract_id.clone());
                checks_executed.push("contracts".to_string());

                let (classifications, missing_changes, _summary) =
                    validate_contract_diff_with_constitution_and_base(
                        &resolved_root,
                        &contract,
                        &changed_files,
                        options.base_commit.as_deref(),
                    );

                for class in classifications {
                    match class.category {
                        FileValidationCategory::BoundaryViolation => {
                            findings.push(GuardFinding {
                                rule_id: "GUARD-CONTRACT-002".to_string(),
                                severity: FindingSeverity::Block,
                                message: format!(
                                    "File '{}' violates forbidden contract boundary: {}",
                                    class.path, class.reason
                                ),
                                path: Some(class.path),
                                remediation: Some(
                                    "Revert forbidden changes or update contract boundary with approval"
                                        .to_string(),
                                ),
                            });
                        }
                        FileValidationCategory::Unexpected => {
                            let is_strict = options.strict || policy.checks.contracts.strict;
                            let severity = if is_strict {
                                FindingSeverity::Block
                            } else {
                                FindingSeverity::Warn
                            };
                            findings.push(GuardFinding {
                                rule_id: "GUARD-CONTRACT-003".to_string(),
                                severity,
                                message: format!(
                                    "File '{}' is outside expected contract boundary: {}",
                                    class.path, class.reason
                                ),
                                path: Some(class.path),
                                remediation: Some(
                                    "Add file to contract expected_files or revert change"
                                        .to_string(),
                                ),
                            });
                        }
                        FileValidationCategory::ExpectedExpansion => {
                            findings.push(GuardFinding {
                                rule_id: "GUARD-CONTRACT-004".to_string(),
                                severity: FindingSeverity::Info,
                                message: format!(
                                    "File '{}' is an expected module expansion: {}",
                                    class.path, class.reason
                                ),
                                path: Some(class.path),
                                remediation: None,
                            });
                        }
                        FileValidationCategory::Expected => {}
                    }
                }

                for missing in missing_changes {
                    if missing.required {
                        findings.push(GuardFinding {
                            rule_id: "GUARD-CONTRACT-005".to_string(),
                            severity: FindingSeverity::Warn,
                            message: format!(
                                "Required expected file '{}' was not modified",
                                missing.path
                            ),
                            path: Some(missing.path),
                            remediation: Some(
                                "Implement required changes or adjust contract requirements"
                                    .to_string(),
                            ),
                        });
                    }
                }
            }
            None => {
                if policy.checks.contracts.required {
                    findings.push(GuardFinding {
                        rule_id: "GUARD-CONTRACT-001".to_string(),
                        severity: FindingSeverity::Block,
                        message: "No active Change Contract found and policy requires a contract"
                            .to_string(),
                        path: None,
                        remediation: Some(
                            "Create an active contract using 'engram preflight \"<task>\"' or 'engram contract create \"<goal>\"'"
                                .to_string(),
                        ),
                    });
                    checks_executed.push("contracts".to_string());
                } else {
                    checks_skipped.push(CheckExecutionRecord {
                        name: "contracts".to_string(),
                        status: "not_applicable".to_string(),
                        reason: Some(
                            "No active Change Contract present (optional in policy)".to_string(),
                        ),
                    });
                }
            }
        }
    }

    // 5. Sovereign Constitution Check
    if !policy.checks.constitution.enabled {
        checks_skipped.push(CheckExecutionRecord {
            name: "constitution".to_string(),
            status: "skipped".to_string(),
            reason: Some("Constitution rule evaluation disabled in policy".to_string()),
        });
    } else {
        checks_executed.push("constitution".to_string());

        // In PR mode, protect sovereign policy files from unratified mutation
        if options.base_commit.is_some() {
            for file in &changed_files {
                let norm = normalize_rel_path(file);
                if is_policy_file(&norm) {
                    findings.push(GuardFinding {
                        rule_id: "GUARD-CONST-001".to_string(),
                        severity: FindingSeverity::Block,
                        message: format!(
                            "Sovereign policy file '{}' cannot be modified in PR without human ratification",
                            file
                        ),
                        path: Some(file.clone()),
                        remediation: Some(
                            "Submit policy modifications through a separate approved governance PR"
                                .to_string(),
                        ),
                    });
                }
            }
        }

        // Evaluate loaded Project Constitution rules
        let rules = load_constitution(&resolved_root).unwrap_or_default();
        for file in &changed_files {
            let norm = normalize_rel_path(file);
            for rule in &rules {
                let violated = if let Some(ref forbidden) = rule.forbidden {
                    matches_boundary(&norm, forbidden)
                } else {
                    false
                };
                if violated {
                    findings.push(GuardFinding {
                        rule_id: format!("GUARD-CONST-{}", rule.id),
                        severity: policy.checks.constitution.severity,
                        message: format!(
                            "Constitution rule '{}' ({}) strictly forbids modifying '{}'",
                            rule.name, rule.id, file
                        ),
                        path: Some(file.clone()),
                        remediation: Some(
                            "Do not modify files within this forbidden constitutional boundary"
                                .to_string(),
                        ),
                    });
                }
            }
        }

        // Evaluate AST/dependency pattern rules if SQLite DB is available
        if let Ok(conn) = db(&resolved_root) {
            if let Ok(violations) = evaluate_rules(&resolved_root, &changed_files, &conn) {
                for v in violations {
                    let finding_id = format!("GUARD-CONST-{}", v.rule_id);
                    if findings
                        .iter()
                        .any(|f| f.rule_id == finding_id && f.path.as_deref() == Some(&v.file_path))
                    {
                        continue;
                    }
                    if v.overridden {
                        findings.push(GuardFinding {
                            rule_id: finding_id,
                            severity: FindingSeverity::Info,
                            message: format!(
                                "Constitution rule '{}' ({}) overridden: {}",
                                v.rule_name,
                                v.rule_id,
                                v.override_reason.unwrap_or_default()
                            ),
                            path: Some(v.file_path),
                            remediation: None,
                        });
                    } else {
                        let severity = match v.severity {
                            RuleSeverity::Error => policy.checks.constitution.severity,
                            RuleSeverity::Warning => FindingSeverity::Warn,
                            RuleSeverity::Info => FindingSeverity::Info,
                        };
                        findings.push(GuardFinding {
                            rule_id: finding_id,
                            severity,
                            message: format!(
                                "Constitution rule '{}' ({}) violated in '{}': {}",
                                v.rule_name, v.rule_id, v.file_path, v.message
                            ),
                            path: Some(v.file_path),
                            remediation: Some(
                                "Adjust implementation to satisfy constitutional boundary rule"
                                    .to_string(),
                            ),
                        });
                    }
                }
            }
        }
    }

    // 6. Impact Analysis & Blast Radius Check
    let mut affected_symbols = Vec::new();
    if !policy.checks.impact.enabled {
        checks_skipped.push(CheckExecutionRecord {
            name: "impact".to_string(),
            status: "skipped".to_string(),
            reason: Some("Blast radius impact analysis disabled in policy".to_string()),
        });
    } else {
        checks_executed.push("impact".to_string());
        let conn_res = db(&resolved_root);

        if let Ok(conn) = conn_res {
            let mut all_affected = HashSet::new();
            let mut analyzed_symbols = HashSet::new();

            for file in &changed_files {
                if all_affected.len() > policy.checks.impact.max_affected_symbols * 2 {
                    break;
                }
                let mut stmt = conn
                    .prepare(
                        "SELECT name, kind FROM entity_versions WHERE path=?1 AND invalidated_at_revision IS NULL LIMIT 5",
                    )
                    .ok();
                if let Some(ref mut s) = stmt {
                    if let Ok(rows) = s.query_map([file], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                    }) {
                        for row in rows.flatten() {
                            let sym_name = row.0;
                            if !analyzed_symbols.insert(sym_name.clone()) {
                                continue;
                            }
                            if let Ok(cone) =
                                compute_impact_cone(&conn, &resolved_root, &sym_name, 2)
                            {
                                for entity in cone.all_entities() {
                                    if let Some(ref name) = entity.symbol_name {
                                        all_affected.insert(format!("{}::{}", entity.path, name));
                                    } else {
                                        all_affected.insert(entity.path.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            affected_symbols = all_affected.into_iter().collect();
            affected_symbols.sort();

            let total_affected = affected_symbols.len();
            if total_affected > policy.checks.impact.max_affected_symbols {
                findings.push(GuardFinding {
                    rule_id: "GUARD-IMPACT-001".to_string(),
                    severity: policy.checks.impact.severity,
                    message: format!(
                        "Blast radius threshold exceeded: {} affected downstream entities (max allowed: {})",
                        total_affected, policy.checks.impact.max_affected_symbols
                    ),
                    path: None,
                    remediation: Some(
                        "Review downstream dependencies or increase max_affected_symbols threshold in policy"
                            .to_string(),
                    ),
                });
            }
        } else {
            checks_skipped.push(CheckExecutionRecord {
                name: "impact".to_string(),
                status: "skipped".to_string(),
                reason: Some("Database not initialized; impact cone analysis skipped".to_string()),
            });
        }
    }

    // 7. Calculate Status and Exit Code
    let has_blocking = findings
        .iter()
        .any(|f| f.severity == FindingSeverity::Block);
    let has_warnings = findings.iter().any(|f| f.severity == FindingSeverity::Warn);

    let (status, exit_code, blocked, summary) = if has_blocking {
        let block_count = findings
            .iter()
            .filter(|f| f.severity == FindingSeverity::Block)
            .count();
        (
            GuardStatus::Block,
            1,
            true,
            format!(
                "BLOCKED: {} mandatory check failure(s) detected. Merge or commit rejected.",
                block_count
            ),
        )
    } else if has_warnings {
        let warn_count = findings
            .iter()
            .filter(|f| f.severity == FindingSeverity::Warn)
            .count();
        (
            GuardStatus::Warn,
            0,
            false,
            format!(
                "ADVISORY: All required checks passed, but {} warning(s) were flagged for review.",
                warn_count
            ),
        )
    } else {
        (
            GuardStatus::Pass,
            0,
            false,
            format!(
                "PASSED: All {} check(s) passed successfully across {} changed file(s).",
                checks_executed.len(),
                changed_files.len()
            ),
        )
    };

    let context = GuardContext {
        repo_root: resolved_root.display().to_string(),
        baseline_commit: options.base_commit.clone(),
        target_snapshot: snapshot_name,
        contract_id: contract_id_used,
        policy_version: policy.version,
    };

    let res = GuardResult {
        status,
        exit_code,
        blocked,
        context,
        checks_executed,
        checks_skipped,
        findings,
        changed_files,
        affected_symbols,
        summary,
    };

    let _ = crate::telemetry::record_event(
        &resolved_root,
        "CHANGE_GUARD_RUN",
        res.context.contract_id.as_deref(),
        "change_guard",
        serde_json::json!({
            "status": res.status.as_str(),
            "exit_code": res.exit_code,
            "blocked": res.blocked,
            "findings_count": res.findings.len(),
            "changed_files_count": res.changed_files.len(),
            "policy_source": policy_source,
        }),
    );

    res
}

fn make_error_result(
    repo_root: &str,
    baseline: Option<String>,
    snapshot: &str,
    policy_ver: &str,
    error_msg: String,
) -> GuardResult {
    GuardResult {
        status: GuardStatus::Error,
        exit_code: 2,
        blocked: true,
        context: GuardContext {
            repo_root: repo_root.to_string(),
            baseline_commit: baseline,
            target_snapshot: snapshot.to_string(),
            contract_id: None,
            policy_version: policy_ver.to_string(),
        },
        checks_executed: Vec::new(),
        checks_skipped: Vec::new(),
        findings: vec![GuardFinding {
            rule_id: "GUARD-ERROR-001".to_string(),
            severity: FindingSeverity::Block,
            message: error_msg.clone(),
            path: None,
            remediation: Some("Fix policy configuration or repository state and rerun".to_string()),
        }],
        changed_files: Vec::new(),
        affected_symbols: Vec::new(),
        summary: format!("ERROR: Guard encountered an unrecoverable error: {error_msg}"),
    }
}

// ---------------------------------------------------------------------------
// Human-Readable and Markdown Reporters
// ---------------------------------------------------------------------------

pub fn format_guard_terminal(result: &GuardResult, diagnostic: bool) -> String {
    let mut out = String::new();
    out.push_str("======================================================================\n");
    out.push_str("                     ENGRAM CI/CD CHANGE GUARD                        \n");
    out.push_str("======================================================================\n\n");

    out.push_str(&format!(
        "  Status:          {}\n",
        result.status.as_str().to_ascii_uppercase()
    ));
    out.push_str(&format!("  Exit Code:       {}\n", result.exit_code));
    out.push_str(&format!(
        "  Target Snapshot: {}\n",
        result.context.target_snapshot
    ));
    if let Some(ref base) = result.context.baseline_commit {
        out.push_str(&format!("  Base Revision:   {}\n", base));
    }
    if let Some(ref cid) = result.context.contract_id {
        out.push_str(&format!("  Change Contract: {}\n", cid));
    }
    out.push_str(&format!(
        "  Changed Files:   {}\n",
        result.changed_files.len()
    ));

    out.push_str("\n--- Checks Overview ---\n");
    for check in &result.checks_executed {
        out.push_str(&format!("  ✅ [EXECUTED]       {}\n", check));
    }
    for skipped in &result.checks_skipped {
        let reason_str = skipped.reason.as_deref().unwrap_or("skipped");
        out.push_str(&format!(
            "  ⚪ [{}] {} ({})\n",
            skipped.status.to_ascii_uppercase(),
            skipped.name,
            reason_str
        ));
    }

    if !result.findings.is_empty() {
        out.push_str("\n--- Findings ---\n");
        for f in &result.findings {
            let icon = match f.severity {
                FindingSeverity::Block => "❌ BLOCK",
                FindingSeverity::Warn => "⚠️  WARN ",
                FindingSeverity::Info => "ℹ️  INFO ",
            };
            out.push_str(&format!("  {} [{}] {}\n", icon, f.rule_id, f.message));
            if let Some(ref p) = f.path {
                out.push_str(&format!("           File:        {}\n", p));
            }
            if let Some(ref rem) = f.remediation {
                out.push_str(&format!("           Remediation: {}\n", rem));
            }
        }
    } else {
        out.push_str("\n--- Findings ---\n  No compliance findings or violations detected.\n");
    }

    if diagnostic {
        out.push_str("\n--- Diagnostic Details ---\n");
        out.push_str(&format!(
            "  Repo Root:       {}\n",
            result.context.repo_root
        ));
        out.push_str(&format!(
            "  Policy Version:  {}\n",
            result.context.policy_version
        ));
        out.push_str(&format!(
            "  Downstream Cone: {} affected symbols/entities\n",
            result.affected_symbols.len()
        ));
        if !result.changed_files.is_empty() {
            out.push_str("  Evaluated Files:\n");
            for cf in &result.changed_files {
                out.push_str(&format!("    • {}\n", cf));
            }
        }
    }

    out.push_str("\n----------------------------------------------------------------------\n");
    out.push_str(&format!("VERDICT: {}\n", result.summary));
    out.push_str("======================================================================\n");
    out
}

pub fn format_guard_markdown_summary(result: &GuardResult) -> String {
    let mut out = String::new();
    let badge = match result.status {
        GuardStatus::Pass => "### ✅ Engram Change Guard — Passed",
        GuardStatus::Warn => "### ⚠️ Engram Change Guard — Advisory Warnings",
        GuardStatus::Block => "### ❌ Engram Change Guard — Blocked",
        GuardStatus::Error => "### 💥 Engram Change Guard — Execution Error",
    };

    out.push_str(&format!("{}\n\n", badge));
    out.push_str(&format!("> **Verdict:** {}\n\n", result.summary));

    out.push_str("| Metric | Value |\n");
    out.push_str("| :--- | :--- |\n");
    out.push_str(&format!(
        "| **Status** | `{}` |\n",
        result.status.as_str().to_ascii_uppercase()
    ));
    out.push_str(&format!("| **Exit Code** | `{}` |\n", result.exit_code));
    out.push_str(&format!(
        "| **Snapshot** | `{}` |\n",
        result.context.target_snapshot
    ));
    out.push_str(&format!(
        "| **Changed Files** | `{}` |\n",
        result.changed_files.len()
    ));
    if let Some(ref base) = result.context.baseline_commit {
        out.push_str(&format!("| **Base Commit** | `{}` |\n", base));
    }
    if let Some(ref cid) = result.context.contract_id {
        out.push_str(&format!("| **Active Contract** | `{}` |\n", cid));
    }
    out.push_str("\n");

    if !result.findings.is_empty() {
        out.push_str("#### Findings\n\n");
        out.push_str("| Severity | Rule ID | Message | File |\n");
        out.push_str("| :--- | :--- | :--- | :--- |\n");
        for f in &result.findings {
            let icon = match f.severity {
                FindingSeverity::Block => "❌ Block",
                FindingSeverity::Warn => "⚠️ Warn",
                FindingSeverity::Info => "ℹ️ Info",
            };
            let path_str = f.path.as_deref().unwrap_or("-");
            out.push_str(&format!(
                "| {} | `{}` | {} | `{}` |\n",
                icon, f.rule_id, f.message, path_str
            ));
        }
        out.push_str("\n");
    }

    out.push_str("<details><summary>Checks Overview</summary>\n\n");
    out.push_str("| Check | Status | Notes |\n");
    out.push_str("| :--- | :--- | :--- |\n");
    for check in &result.checks_executed {
        out.push_str(&format!(
            "| `{}` | ✅ Executed | Satisfied policy |\n",
            check
        ));
    }
    for skipped in &result.checks_skipped {
        out.push_str(&format!(
            "| `{}` | ⚪ {} | {} |\n",
            skipped.name,
            skipped.status,
            skipped.reason.as_deref().unwrap_or("-")
        ));
    }
    out.push_str("\n</details>\n");

    out
}

// ---------------------------------------------------------------------------
// Git Pre-Commit Hook Management (Opt-In & Non-Destructive)
// ---------------------------------------------------------------------------

pub const GUARD_HOOK_START: &str = "# --- BEGIN ENGRAM CHANGE GUARD ---";
pub const GUARD_HOOK_END: &str = "# --- END ENGRAM CHANGE GUARD ---";

pub const GUARD_HOOK_BODY: &str = r#"# --- BEGIN ENGRAM CHANGE GUARD ---
if command -v engram >/dev/null 2>&1; then
  engram guard --staged || exit 1
fi
# --- END ENGRAM CHANGE GUARD ---"#;

/// Safe opt-in pre-commit hook installer
pub fn install_guard_hook(root: &Path, dry_run: bool) -> Result<String> {
    let hooks_dir = root.join(".git").join("hooks");
    if !hooks_dir.exists() && !dry_run {
        fs::create_dir_all(&hooks_dir)?;
    }

    let hook_path = hooks_dir.join("pre-commit");

    if !hook_path.exists() {
        let content = format!("#!/bin/sh\n{}\n", GUARD_HOOK_BODY);
        if dry_run {
            return Ok(format!(
                "Dry-run: Hook does not exist. Engram would create: {}\nContent:\n{}",
                hook_path.display(),
                content
            ));
        }
        fs::write(&hook_path, content)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;
        }
        return Ok(format!(
            "Installed Engram Change Guard pre-commit hook at {}",
            hook_path.display()
        ));
    }

    let existing = fs::read_to_string(&hook_path)?;
    if existing.contains(GUARD_HOOK_START) {
        if dry_run {
            return Ok(format!(
                "Dry-run: Engram Change Guard block already present in {}. No modifications needed.",
                hook_path.display()
            ));
        }
        return Ok(format!(
            "Engram Change Guard hook is already installed in {}",
            hook_path.display()
        ));
    }

    // Preserve existing foreign hooks (Husky, Lefthook, user scripts)
    let new_content = format!("{}\n\n{}\n", existing.trim_end(), GUARD_HOOK_BODY);
    if dry_run {
        return Ok(format!(
            "Dry-run: Foreign hook detected in {}. Engram would safely append Change Guard block while preserving existing hook.",
            hook_path.display()
        ));
    }

    fs::write(&hook_path, new_content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;
    }

    Ok(format!(
        "Preserved existing pre-commit hook and appended Engram Change Guard block to {}",
        hook_path.display()
    ))
}

/// Safe pre-commit hook uninstaller
pub fn uninstall_guard_hook(root: &Path, dry_run: bool) -> Result<String> {
    let hook_path = root.join(".git").join("hooks").join("pre-commit");
    if !hook_path.exists() {
        return Ok(format!(
            "No pre-commit hook found at {}",
            hook_path.display()
        ));
    }

    let content = fs::read_to_string(&hook_path)?;
    if !content.contains(GUARD_HOOK_START) {
        return Ok(format!(
            "Pre-commit hook at {} does not contain Engram Change Guard block. Left untouched.",
            hook_path.display()
        ));
    }

    let mut cleaned_lines = Vec::new();
    let mut skipping = false;

    for line in content.lines() {
        if line.trim() == GUARD_HOOK_START {
            skipping = true;
            continue;
        }
        if line.trim() == GUARD_HOOK_END {
            skipping = false;
            continue;
        }
        if !skipping {
            cleaned_lines.push(line);
        }
    }

    let remaining = cleaned_lines.join("\n").trim().to_string();

    if dry_run {
        if remaining.is_empty() || remaining == "#!/bin/sh" {
            return Ok(format!(
                "Dry-run: Engram would remove entire hook file at {}",
                hook_path.display()
            ));
        } else {
            return Ok(format!(
                "Dry-run: Engram would remove Change Guard block from {} and preserve remaining lines.",
                hook_path.display()
            ));
        }
    }

    if remaining.is_empty() || remaining == "#!/bin/sh" {
        fs::remove_file(&hook_path)?;
        Ok(format!(
            "Removed Engram pre-commit hook from {}",
            hook_path.display()
        ))
    } else {
        fs::write(&hook_path, format!("{}\n", remaining))?;
        Ok(format!(
            "Removed Engram block while preserving foreign hooks in {}",
            hook_path.display()
        ))
    }
}
