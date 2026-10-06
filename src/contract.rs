//! Change Contract Engine: elastic boundary validation, versioning, baseline anchoring,
//! Constitution precedence, provenance ratification, Git hook management, and Change Guard CI checks.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

use crate::compiler::context;
use crate::storage::{cache_dir, root_path};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContractStatus {
    Active,
    Reconciled,
    Completed,
    Cancelled,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExpectedTarget {
    pub path: String,
    #[serde(default = "default_true")]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RatificationStatus {
    Unratified,
    Ratified,
    ExplicitOverride,
}

impl Default for RatificationStatus {
    fn default() -> Self {
        RatificationStatus::Unratified
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContractProvenance {
    #[serde(default = "default_agent_origin")]
    pub origin: String,
    #[serde(default = "default_worktree")]
    pub baseline_commit: String,
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub ratification_status: RatificationStatus,
    #[serde(default)]
    pub ratified_by: Option<String>,
    #[serde(default)]
    pub ratification_reason: Option<String>,
}

fn default_agent_origin() -> String {
    "AGENT".to_string()
}

fn default_worktree() -> String {
    "WORKTREE".to_string()
}

impl Default for ContractProvenance {
    fn default() -> Self {
        Self {
            origin: "AGENT".to_string(),
            baseline_commit: "WORKTREE".to_string(),
            created_at: Utc::now(),
            ratification_status: RatificationStatus::Unratified,
            ratified_by: None,
            ratification_reason: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractBoundary {
    pub expected_modules: Vec<String>,
    pub expected_files: Vec<String>,
    #[serde(default)]
    pub targets: Vec<ExpectedTarget>,
    pub expected_symbols: Vec<String>,
    pub expected_tests: Vec<String>,
    pub forbidden_boundaries: Vec<String>,
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeContract {
    pub contract_id: String,
    pub task_id: String,
    pub task_goal: String,
    pub version: u32,
    pub baseline_revision: String,
    pub status: ContractStatus,
    pub boundary: ContractBoundary,
    #[serde(default)]
    pub provenance: ContractProvenance,
    pub reconciliation_history: Vec<ReconciliationRecord>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationRecord {
    pub from_version: u32,
    pub to_version: u32,
    pub reason: String,
    pub actor: String,
    pub approved_additions: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FileValidationCategory {
    Expected,
    ExpectedExpansion,
    Unexpected,
    BoundaryViolation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileClassification {
    pub path: String,
    pub category: FileValidationCategory,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MissingChange {
    pub path: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractValidationResult {
    pub contract_id: String,
    pub version: u32,
    pub is_valid: bool,
    pub classifications: Vec<FileClassification>,
    pub missing_expected_changes: Vec<String>,
    pub summary: ContractSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractSummary {
    pub total_changed: usize,
    pub expected_count: usize,
    pub expansion_count: usize,
    pub unexpected_count: usize,
    pub violation_count: usize,
    pub missing_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum CheckExitCode {
    Pass = 0,
    ContractViolation = 1,
    ContractNotFound = 2,
    ContractInvalid = 3,
    BaselineUnavailable = 4,
    InternalError = 5,
}

impl CheckExitCode {
    pub fn as_i32(&self) -> i32 {
        *self as i32
    }
}

#[derive(Debug, Clone, Default)]
pub struct CheckOptions {
    pub contract_id: Option<String>,
    pub strict: bool,
    pub staged: bool,
    pub base_commit: Option<String>,
    pub check_provenance: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CheckSummary {
    pub total_changed: usize,
    pub expected: usize,
    pub expected_expansion: usize,
    pub unexpected: usize,
    pub boundary_violations: usize,
    pub missing_expected: usize,
    pub missing_required: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub contract_id: String,
    pub task_id: String,
    pub task_goal: String,
    pub baseline_commit: String,
    pub target_snapshot: String,
    pub status: String,
    pub exit_code: i32,
    pub policy: String,
    pub provenance: ContractProvenance,
    pub summary: CheckSummary,
    pub changes: Vec<FileClassification>,
    pub missing_changes: Vec<MissingChange>,
    pub failure_reasons: Vec<String>,
    pub advisory_warnings: Vec<String>,
}

fn contracts_dir(root: &Path) -> Result<PathBuf> {
    let dir = cache_dir(root)?.join("contracts");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

fn active_contract_path(root: &Path) -> Result<PathBuf> {
    Ok(contracts_dir(root)?.join("active.json"))
}

fn contract_file_path(root: &Path, contract_id: &str) -> Result<PathBuf> {
    Ok(contracts_dir(root)?.join(format!("{contract_id}.json")))
}

/// Retrieve the currently active Change Contract, if any.
pub fn get_active_contract(root: &Path) -> Result<Option<ChangeContract>> {
    let path = active_contract_path(root)?;
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(&path)?;
    let contract: ChangeContract = serde_json::from_str(&data)?;
    Ok(Some(contract))
}

/// Load a specific contract by ID.
pub fn get_contract(root: &Path, contract_id: &str) -> Result<ChangeContract> {
    let path = contract_file_path(root, contract_id)?;
    if !path.exists() {
        return Err(anyhow!("Contract {contract_id} not found"));
    }
    let data = fs::read_to_string(&path)?;
    let contract: ChangeContract = serde_json::from_str(&data)?;
    Ok(contract)
}

/// Save a contract to cache and update the active pointer.
pub fn save_contract(root: &Path, contract: &ChangeContract) -> Result<()> {
    let file = contract_file_path(root, &contract.contract_id)?;
    let json_data = serde_json::to_string_pretty(contract)?;
    fs::write(&file, &json_data)?;

    if contract.status == ContractStatus::Active || contract.status == ContractStatus::Reconciled {
        let active_file = active_contract_path(root)?;
        fs::write(&active_file, &json_data)?;
    }
    Ok(())
}

/// Fetch the current Git HEAD revision hash or return "WORKTREE".
pub fn get_baseline_revision(root: &Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !sha.is_empty() {
                return sha;
            }
        }
    }
    "WORKTREE".to_string()
}

/// Normalize path for matching (forward slashes, stripped prefixes).
pub fn normalize_rel_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

/// Check if a path matches a glob pattern or prefix boundary.
pub fn matches_boundary(path: &str, pattern: &str) -> bool {
    let p = normalize_rel_path(path);
    let pat = normalize_rel_path(pattern);

    if pat == "**" || pat == "*" {
        return true;
    }
    if pat.ends_with("/**") {
        let prefix = &pat[..pat.len() - 3];
        p.starts_with(prefix) || p == prefix
    } else if pat.ends_with("/*") {
        let prefix = &pat[..pat.len() - 2];
        p.starts_with(prefix)
    } else if pat.contains('*') {
        let regex_pat = format!("^{}$", regex::escape(&pat).replace(r"\*", ".*"));
        regex::Regex::new(&regex_pat).map(|r| r.is_match(&p)).unwrap_or(false)
    } else {
        p.starts_with(&pat) || p == pat
    }
}

/// Extract module from a file path (e.g. "src/orders/export/csv.rs" -> "orders" or "src/orders")
pub fn extract_module(path: &str) -> String {
    let norm = normalize_rel_path(path);
    let parts: Vec<&str> = norm.split('/').collect();
    if parts.len() >= 2 && (parts[0] == "src" || parts[0] == "lib" || parts[0] == "packages") {
        parts[1].to_string()
    } else if !parts.is_empty() {
        parts[0].to_string()
    } else {
        "root".to_string()
    }
}

/// Create a new Change Contract, auto-grounding expected files and modules if not provided.
pub fn create_contract(
    root: &Path,
    task_goal: &str,
    expected_modules: Option<Vec<String>>,
    expected_files: Option<Vec<String>>,
    forbidden_boundaries: Option<Vec<String>>,
) -> Result<ChangeContract> {
    let root = root_path(root)?;
    let baseline_revision = get_baseline_revision(&root);
    let contract_id = format!("ctr_{}", Uuid::new_v4().simple());
    let task_id = format!("task_{}", Uuid::new_v4().simple());
    let now_ts = Utc::now();

    let (final_files, final_modules, final_tests, constraints) = match (expected_files, expected_modules) {
        (Some(files), Some(modules)) => {
            let tests: Vec<String> = files
                .iter()
                .filter(|f| f.contains("test") || f.contains("spec"))
                .cloned()
                .collect();
            (files, modules, tests, Vec::new())
        }
        (maybe_files, maybe_modules) => {
            let ctx = context(&root, task_goal, 3000)?;
            let mut derived_files: Vec<String> = ctx.files.iter().map(|f| normalize_rel_path(&f.path)).collect();
            let mut derived_modules: HashSet<String> = HashSet::new();
            let mut derived_tests: Vec<String> = Vec::new();

            if let Some(explicit) = maybe_files {
                for f in explicit {
                    let norm = normalize_rel_path(&f);
                    if !derived_files.contains(&norm) {
                        derived_files.push(norm);
                    }
                }
            }

            for f in &derived_files {
                derived_modules.insert(extract_module(f));
                if f.contains("test") || f.contains("spec") {
                    derived_tests.push(f.clone());
                }
            }

            if let Some(explicit_mods) = maybe_modules {
                for m in explicit_mods {
                    derived_modules.insert(m);
                }
            }

            (
                derived_files,
                derived_modules.into_iter().collect(),
                derived_tests,
                ctx.constraints,
            )
        }
    };

    let forbidden = forbidden_boundaries.unwrap_or_default();
    let targets: Vec<ExpectedTarget> = final_files
        .iter()
        .map(|f| ExpectedTarget {
            path: f.clone(),
            required: true,
        })
        .collect();

    let provenance = ContractProvenance {
        origin: "AGENT".to_string(),
        baseline_commit: baseline_revision.clone(),
        created_at: now_ts,
        ratification_status: RatificationStatus::Unratified,
        ratified_by: None,
        ratification_reason: None,
    };

    let contract = ChangeContract {
        contract_id,
        task_id,
        task_goal: task_goal.to_string(),
        version: 1,
        baseline_revision,
        status: ContractStatus::Active,
        boundary: ContractBoundary {
            expected_modules: final_modules,
            expected_files: final_files,
            targets,
            expected_symbols: Vec::new(),
            expected_tests: final_tests,
            forbidden_boundaries: forbidden,
            constraints,
        },
        provenance,
        reconciliation_history: Vec::new(),
        created_at: now_ts,
        updated_at: now_ts,
    };

    save_contract(&root, &contract)?;
    Ok(contract)
}

/// Ratify a Change Contract, affirming human authorization of contract boundaries.
pub fn ratify_contract(
    root: &Path,
    contract_id: &str,
    ratified_by: &str,
    reason: &str,
) -> Result<ChangeContract> {
    let root = root_path(root)?;
    let mut contract = get_contract(&root, contract_id)?;
    contract.provenance.ratification_status = RatificationStatus::Ratified;
    contract.provenance.ratified_by = Some(ratified_by.to_string());
    contract.provenance.ratification_reason = Some(reason.to_string());
    contract.updated_at = Utc::now();
    save_contract(&root, &contract)?;
    Ok(contract)
}

/// Validate actual changed files against Change Contract elastic boundaries
/// with absolute precedence of the Project Constitution.
pub fn validate_contract_diff_with_constitution(
    root: &Path,
    contract: &ChangeContract,
    changed_files: &[String],
) -> (Vec<FileClassification>, Vec<MissingChange>, ContractSummary) {
    let mut classifications = Vec::new();
    let mut expected_count = 0;
    let mut expansion_count = 0;
    let mut unexpected_count = 0;
    let mut violation_count = 0;

    let constitution_rules = crate::constitution::load_constitution(root).unwrap_or_default();

    let normalized_expected: Vec<String> = contract
        .boundary
        .expected_files
        .iter()
        .map(|f| normalize_rel_path(f))
        .collect();

    for file in changed_files {
        let norm_file = normalize_rel_path(file);

        // 1. Check Project Constitution rules FIRST (Absolute Precedence)
        let mut constitution_violation = None;
        for rule in &constitution_rules {
            let matches_rule = if let Some(ref f) = rule.forbidden {
                matches_boundary(&norm_file, f)
            } else if let Some(ref t) = rule.target_files {
                rule.severity == crate::model::RuleSeverity::Error && matches_boundary(&norm_file, t)
            } else if let Some(ref s) = rule.source {
                rule.severity == crate::model::RuleSeverity::Error && matches_boundary(&norm_file, s)
            } else {
                false
            };

            if matches_rule {
                constitution_violation = Some(format!(
                    "Constitution rule '{}' ({}) strictly forbids modifying this boundary",
                    rule.name, rule.id
                ));
                break;
            }
        }

        if let Some(reason) = constitution_violation {
            violation_count += 1;
            classifications.push(FileClassification {
                path: file.clone(),
                category: FileValidationCategory::BoundaryViolation,
                reason,
            });
            continue;
        }

        // 2. Check Contract forbidden boundaries
        let is_forbidden = contract
            .boundary
            .forbidden_boundaries
            .iter()
            .any(|pat| matches_boundary(&norm_file, pat));

        if is_forbidden {
            violation_count += 1;
            classifications.push(FileClassification {
                path: file.clone(),
                category: FileValidationCategory::BoundaryViolation,
                reason: "File matches forbidden boundary constraint in Change Contract".to_string(),
            });
            continue;
        }

        // 3. Check if explicitly in expected files
        if normalized_expected.contains(&norm_file) {
            expected_count += 1;
            classifications.push(FileClassification {
                path: file.clone(),
                category: FileValidationCategory::Expected,
                reason: "File is explicitly part of contract scope".to_string(),
            });
            continue;
        }

        // 4. Check if belongs to expected module (EXPECTED_EXPANSION)
        let file_module = extract_module(&norm_file);
        let is_module_expected = contract
            .boundary
            .expected_modules
            .iter()
            .any(|m| m.eq_ignore_ascii_case(&file_module) || norm_file.contains(m));

        if is_module_expected {
            expansion_count += 1;
            classifications.push(FileClassification {
                path: file.clone(),
                category: FileValidationCategory::ExpectedExpansion,
                reason: format!("File is within expected module boundary '{file_module}'"),
            });
            continue;
        }

        // 5. Otherwise, UNEXPECTED change
        unexpected_count += 1;
        classifications.push(FileClassification {
            path: file.clone(),
            category: FileValidationCategory::Unexpected,
            reason: "File is outside expected files and modules".to_string(),
        });
    }

    // 6. Check for MISSING expected changes (separate required from optional)
    let mut missing_changes = Vec::new();
    let mut missing_count = 0;

    for exp in &contract.boundary.expected_files {
        let norm_exp = normalize_rel_path(exp);
        if !changed_files.iter().any(|c| normalize_rel_path(c) == norm_exp) {
            let is_req = contract
                .boundary
                .targets
                .iter()
                .find(|t| normalize_rel_path(&t.path) == norm_exp)
                .map(|t| t.required)
                .unwrap_or(true);

            missing_changes.push(MissingChange {
                path: exp.clone(),
                required: is_req,
            });
            missing_count += 1;
        }
    }

    for test in &contract.boundary.expected_tests {
        let norm_test = normalize_rel_path(test);
        if !changed_files.iter().any(|c| normalize_rel_path(c) == norm_test)
            && !missing_changes.iter().any(|m| m.path == *test)
        {
            missing_changes.push(MissingChange {
                path: test.clone(),
                required: true,
            });
            missing_count += 1;
        }
    }

    let summary = ContractSummary {
        total_changed: changed_files.len(),
        expected_count,
        expansion_count,
        unexpected_count,
        violation_count,
        missing_count,
    };

    (classifications, missing_changes, summary)
}

/// Backward-compatible diff validation function.
pub fn validate_contract_diff(
    contract: &ChangeContract,
    changed_files: &[String],
) -> ContractValidationResult {
    let dummy_root = PathBuf::from(".");
    let (classifications, missing_changes, summary) =
        validate_contract_diff_with_constitution(&dummy_root, contract, changed_files);

    let is_valid = summary.violation_count == 0;
    let missing_expected_changes = missing_changes.into_iter().map(|m| m.path).collect();

    ContractValidationResult {
        contract_id: contract.contract_id.clone(),
        version: contract.version,
        is_valid,
        classifications,
        missing_expected_changes,
        summary,
    }
}

/// Inspect actual changed files from Git based on snapshot policy:
/// - Staged: diff from authoritative baseline to staged index (`git diff --name-only --cached <baseline>`)
/// - PR / Base override: diff between base branch and HEAD (`git diff --name-only <base>...HEAD`)
/// - Working tree: diff between baseline and current files (`git diff --name-only <baseline>`)
pub fn inspect_changed_files(
    root: &Path,
    baseline: &str,
    options: &CheckOptions,
) -> Result<(Vec<String>, String, Option<CheckExitCode>)> {
    // Verify baseline commit if it's a specific SHA
    if baseline != "WORKTREE" && !baseline.is_empty() {
        let check_sha = Command::new("git")
            .args(["rev-parse", "--verify", "--quiet", &format!("{baseline}^{{commit}}")])
            .current_dir(root)
            .output();

        if let Ok(out) = check_sha {
            if !out.status.success() {
                return Ok((
                    Vec::new(),
                    "UNAVAILABLE".to_string(),
                    Some(CheckExitCode::BaselineUnavailable),
                ));
            }
        }
    }

    let (args, snapshot_name) = if let Some(ref base) = options.base_commit {
        (
            vec!["diff".to_string(), "--name-only".to_string(), format!("{base}...HEAD")],
            format!("PR_HEAD (base: {base})"),
        )
    } else if options.staged {
        if baseline != "WORKTREE" && !baseline.is_empty() {
            (
                vec![
                    "diff".to_string(),
                    "--name-only".to_string(),
                    "--cached".to_string(),
                    baseline.to_string(),
                ],
                format!("STAGED (baseline: {baseline})"),
            )
        } else {
            (
                vec!["diff".to_string(), "--name-only".to_string(), "--cached".to_string()],
                "STAGED".to_string(),
            )
        }
    } else if baseline != "WORKTREE" && !baseline.is_empty() {
        (
            vec!["diff".to_string(), "--name-only".to_string(), baseline.to_string()],
            format!("WORKING_TREE (baseline: {baseline})"),
        )
    } else {
        (
            vec!["status".to_string(), "--porcelain".to_string()],
            "WORKING_TREE".to_string(),
        )
    };

    let output = Command::new("git")
        .args(&args)
        .current_dir(root)
        .output();

    match output {
        Ok(out) => {
            if !out.status.success() {
                // If git diff failed because baseline commit wasn't found
                let err_str = String::from_utf8_lossy(&out.stderr);
                if err_str.contains("unknown revision") || err_str.contains("bad revision") {
                    return Ok((Vec::new(), snapshot_name, Some(CheckExitCode::BaselineUnavailable)));
                }
                return Ok((Vec::new(), snapshot_name, Some(CheckExitCode::InternalError)));
            }

            let text = String::from_utf8_lossy(&out.stdout);
            let files: Vec<String> = if args[0] == "status" {
                text.lines()
                    .filter_map(|l| {
                        let trimmed = l.trim();
                        if trimmed.len() >= 3 {
                            Some(trimmed[3..].trim().to_string())
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                text.lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(String::from)
                    .collect()
            };

            Ok((files, snapshot_name, None))
        }
        Err(_) => Ok((Vec::new(), snapshot_name, Some(CheckExitCode::InternalError))),
    }
}

/// The centerpiece Change Guard verification engine.
pub fn check_contract(root: &Path, options: &CheckOptions) -> CheckResult {
    let root = match root_path(root) {
        Ok(r) => r,
        Err(e) => {
            return make_error_result("unknown", CheckExitCode::InternalError, format!("{e}"));
        }
    };

    // 1. Locate active or requested Change Contract
    let contract = if let Some(ref id) = options.contract_id {
        match get_contract(&root, id) {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("{e}");
                if err_msg.contains("not found") {
                    return make_error_result(id, CheckExitCode::ContractNotFound, format!("Change Contract '{id}' not found."));
                }
                return make_error_result(id, CheckExitCode::ContractInvalid, format!("Corrupt contract schema: {e}"));
            }
        }
    } else {
        match get_active_contract(&root) {
            Ok(Some(c)) => c,
            Ok(None) => {
                return make_error_result("none", CheckExitCode::ContractNotFound, "No active Change Contract found in repository.".to_string());
            }
            Err(e) => {
                return make_error_result("active", CheckExitCode::ContractInvalid, format!("Failed reading active contract: {e}"));
            }
        }
    };

    // 2. Inspect changed files based on snapshot policy
    let (changed_files, snapshot_name, maybe_err) = match inspect_changed_files(&root, &contract.baseline_revision, options) {
        Ok(r) => r,
        Err(e) => {
            return make_error_result(&contract.contract_id, CheckExitCode::InternalError, format!("Git inspection failed: {e}"));
        }
    };

    if let Some(err_code) = maybe_err {
        let msg = match err_code {
            CheckExitCode::BaselineUnavailable => format!(
                "Baseline commit '{}' is unavailable in current Git history (shallow clone or missing commit).",
                contract.baseline_revision
            ),
            _ => "Internal Git execution error.".to_string(),
        };
        return make_error_result(&contract.contract_id, err_code, msg);
    }

    // 3. Classify changes with Constitution precedence
    let (classifications, missing_changes, summary_counts) =
        validate_contract_diff_with_constitution(&root, &contract, &changed_files);

    let mut failure_reasons = Vec::new();
    let mut advisory_warnings = Vec::new();

    // 4. Evaluate Contract Trust Model (Provenance Check in PR Mode)
    if options.check_provenance || options.base_commit.is_some() {
        if contract.provenance.ratification_status == RatificationStatus::Unratified
            && contract.version > 1
        {
            failure_reasons.push(
                "CONTRACT_MODIFIED_IN_PR: Contract boundary was expanded after baseline commit. Explicit ratification required ('engram contract ratify <id>').".to_string(),
            );
        }
    }

    // 5. Evaluate Classifications
    if summary_counts.violation_count > 0 {
        failure_reasons.push(format!(
            "CONTRACT_VIOLATION: Detected {} boundary violation(s) crossing forbidden boundaries.",
            summary_counts.violation_count
        ));
    }

    if summary_counts.unexpected_count > 0 {
        if options.strict {
            failure_reasons.push(format!(
                "STRICT_POLICY_VIOLATION: Detected {} unexpected file modification(s) outside expected module boundaries.",
                summary_counts.unexpected_count
            ));
        } else {
            advisory_warnings.push(format!(
                "UNEXPECTED_CHANGES: Detected {} unexpected change(s) outside module scope (Review required).",
                summary_counts.unexpected_count
            ));
        }
    }

    let required_missing = missing_changes.iter().filter(|m| m.required).count();
    if required_missing > 0 {
        if options.strict {
            failure_reasons.push(format!(
                "MISSING_REQUIRED_CHANGES: {} required target change(s) were not performed.",
                required_missing
            ));
        } else {
            advisory_warnings.push(format!(
                "MISSING_EXPECTED_CHANGES: {} expected target(s) were not modified.",
                missing_changes.len()
            ));
        }
    }

    let is_passed = failure_reasons.is_empty();
    let exit_code = if is_passed {
        CheckExitCode::Pass.as_i32()
    } else {
        CheckExitCode::ContractViolation.as_i32()
    };

    let summary = CheckSummary {
        total_changed: changed_files.len(),
        expected: summary_counts.expected_count,
        expected_expansion: summary_counts.expansion_count,
        unexpected: summary_counts.unexpected_count,
        boundary_violations: summary_counts.violation_count,
        missing_expected: missing_changes.len(),
        missing_required: required_missing,
    };

    CheckResult {
        contract_id: contract.contract_id,
        task_id: contract.task_id,
        task_goal: contract.task_goal,
        baseline_commit: contract.baseline_revision,
        target_snapshot: snapshot_name,
        status: if is_passed { "PASSED".to_string() } else { "FAILED".to_string() },
        exit_code,
        policy: if options.strict { "STRICT".to_string() } else { "STANDARD".to_string() },
        provenance: contract.provenance,
        summary,
        changes: classifications,
        missing_changes,
        failure_reasons,
        advisory_warnings,
    }
}

fn make_error_result(contract_id: &str, code: CheckExitCode, reason: String) -> CheckResult {
    CheckResult {
        contract_id: contract_id.to_string(),
        task_id: "none".to_string(),
        task_goal: "none".to_string(),
        baseline_commit: "none".to_string(),
        target_snapshot: "none".to_string(),
        status: "FAILED".to_string(),
        exit_code: code.as_i32(),
        policy: "STANDARD".to_string(),
        provenance: ContractProvenance::default(),
        summary: CheckSummary::default(),
        changes: Vec::new(),
        missing_changes: Vec::new(),
        failure_reasons: vec![reason],
        advisory_warnings: Vec::new(),
    }
}

/// Format a clean, human-readable report with optional GitHub Actions workflow annotations.
pub fn format_check_report(result: &CheckResult, is_github_ci: bool) -> String {
    let mut out = String::new();
    out.push_str("Engram Change Guard\n");
    out.push_str("──────────────────────────────────────────────────────\n");
    out.push_str(&format!("Contract:        {}\n", result.contract_id));
    out.push_str(&format!("Task:            {}\n", result.task_goal));
    out.push_str(&format!("Baseline:        {}\n", result.baseline_commit));
    out.push_str(&format!("Target Snapshot: {}\n", result.target_snapshot));
    out.push_str(&format!("Policy:          {}\n", result.policy));
    out.push_str("──────────────────────────────────────────────────────\n");

    if !result.changes.is_empty() {
        out.push_str("Changed Files:\n");
        for c in &result.changes {
            match c.category {
                FileValidationCategory::Expected => {
                    out.push_str(&format!("  ✓ {}\n      EXPECTED ({})\n", c.path, c.reason));
                }
                FileValidationCategory::ExpectedExpansion => {
                    out.push_str(&format!("  ✓ {}\n      EXPECTED_EXPANSION ({})\n", c.path, c.reason));
                }
                FileValidationCategory::Unexpected => {
                    out.push_str(&format!("  ⚠ {}\n      UNEXPECTED ({})\n", c.path, c.reason));
                    if is_github_ci {
                        out.push_str(&format!("::warning file={}::{}\n", c.path, c.reason));
                    }
                }
                FileValidationCategory::BoundaryViolation => {
                    out.push_str(&format!("  ✗ {}\n      BOUNDARY_VIOLATION: {}\n", c.path, c.reason));
                    if is_github_ci {
                        out.push_str(&format!("::error file={}::{}\n", c.path, c.reason));
                    }
                }
            }
        }
    } else {
        out.push_str("No files changed in inspected snapshot.\n");
    }

    if !result.missing_changes.is_empty() {
        out.push_str("\nMissing Expected Targets:\n");
        for m in &result.missing_changes {
            let label = if m.required { "[REQUIRED]" } else { "[OPTIONAL]" };
            out.push_str(&format!("  - {} {}\n", m.path, label));
        }
    }

    out.push_str("──────────────────────────────────────────────────────\n");
    out.push_str(&format!(
        "Summary: {} expected, {} expansion, {} unexpected, {} violations\n",
        result.summary.expected,
        result.summary.expected_expansion,
        result.summary.unexpected,
        result.summary.boundary_violations
    ));

    if !result.failure_reasons.is_empty() {
        out.push_str("\nFailures:\n");
        for f in &result.failure_reasons {
            out.push_str(&format!("  ✗ {}\n", f));
            if is_github_ci {
                out.push_str(&format!("::error::{}\n", f));
            }
        }
    }

    if !result.advisory_warnings.is_empty() {
        out.push_str("\nAdvisory Warnings:\n");
        for w in &result.advisory_warnings {
            out.push_str(&format!("  ⚠ {}\n", w));
        }
    }

    out.push_str("──────────────────────────────────────────────────────\n");
    if result.status == "PASSED" {
        out.push_str("Result: CONTRACT CHECK PASSED ✓\n");
    } else {
        out.push_str(&format!("Result: CONTRACT CHECK FAILED (Exit Code: {})\n", result.exit_code));
    }

    out
}

const HOOK_START_MARKER: &str = "# --- BEGIN ENGRAM CHANGE GUARD ---";
const HOOK_END_MARKER: &str = "# --- END ENGRAM CHANGE GUARD ---";

const HOOK_BODY: &str = r#"# --- BEGIN ENGRAM CHANGE GUARD ---
if command -v engram >/dev/null 2>&1; then
  engram contract check --staged || exit 1
fi
# --- END ENGRAM CHANGE GUARD ---"#;

/// Install the pre-commit hook into .git/hooks/pre-commit with dry-run support.
pub fn install_pre_commit_hook(root: &Path, dry_run: bool) -> Result<String> {
    let hooks_dir = root.join(".git").join("hooks");
    if !hooks_dir.exists() && !dry_run {
        fs::create_dir_all(&hooks_dir)?;
    }

    let hook_path = hooks_dir.join("pre-commit");

    if !hook_path.exists() {
        let content = format!("#!/bin/sh\n{}\n", HOOK_BODY);
        if dry_run {
            return Ok(format!(
                "Dry-run: No hook detected.\nEngram would create: {}\nContent:\n{}",
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
        return Ok(format!("Installed Engram pre-commit hook successfully at {}", hook_path.display()));
    }

    let existing = fs::read_to_string(&hook_path)?;
    if existing.contains(HOOK_START_MARKER) {
        if dry_run {
            return Ok(format!(
                "Dry-run: Existing Engram hook detected in {}. No modifications needed.",
                hook_path.display()
            ));
        }
        return Ok(format!("Engram Change Guard hook is already installed in {}", hook_path.display()));
    }

    // Existing foreign hook (e.g. Husky, Lefthook)
    let new_content = format!("{}\n\n{}\n", existing.trim_end(), HOOK_BODY);
    if dry_run {
        return Ok(format!(
            "Dry-run: Foreign hook detected in {}.\nEngram would safely append Change Guard block while preserving existing hook.\nNo files modified.",
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

/// Uninstall the pre-commit hook cleanly with dry-run support.
pub fn uninstall_pre_commit_hook(root: &Path, dry_run: bool) -> Result<String> {
    let hook_path = root.join(".git").join("hooks").join("pre-commit");
    if !hook_path.exists() {
        return Ok(format!("No pre-commit hook exists at {}", hook_path.display()));
    }

    let content = fs::read_to_string(&hook_path)?;
    if !content.contains(HOOK_START_MARKER) {
        return Ok(format!(
            "Pre-commit hook at {} does not contain Engram Change Guard. Left unchanged.",
            hook_path.display()
        ));
    }

    let mut cleaned_lines = Vec::new();
    let mut skipping = false;

    for line in content.lines() {
        if line.trim() == HOOK_START_MARKER {
            skipping = true;
            continue;
        }
        if line.trim() == HOOK_END_MARKER {
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
            return Ok(format!("Dry-run: Engram would remove {}", hook_path.display()));
        } else {
            return Ok(format!(
                "Dry-run: Engram would remove Change Guard block from {} and preserve remaining lines.",
                hook_path.display()
            ));
        }
    }

    if remaining.is_empty() || remaining == "#!/bin/sh" {
        fs::remove_file(&hook_path)?;
        Ok(format!("Removed Engram pre-commit hook from {}", hook_path.display()))
    } else {
        fs::write(&hook_path, format!("{}\n", remaining))?;
        Ok(format!(
            "Removed Engram block while preserving foreign hooks in {}",
            hook_path.display()
        ))
    }
}

/// Reconcile an existing contract when a human authoritatively intervenes or approves additions.
pub fn reconcile_contract(
    root: &Path,
    contract_id: &str,
    reason: &str,
    actor: &str,
    approved_files: Vec<String>,
    approved_modules: Vec<String>,
) -> Result<ChangeContract> {
    let root = root_path(root)?;
    let mut contract = get_contract(&root, contract_id)?;
    let from_version = contract.version;
    let to_version = from_version + 1;

    for file in &approved_files {
        let norm = normalize_rel_path(file);
        if !contract.boundary.expected_files.contains(&norm) {
            contract.boundary.expected_files.push(norm.clone());
        }
        if !contract.boundary.targets.iter().any(|t| normalize_rel_path(&t.path) == norm) {
            contract.boundary.targets.push(ExpectedTarget {
                path: norm.clone(),
                required: true,
            });
        }
        // Remove from forbidden boundaries if explicitly approved
        contract
            .boundary
            .forbidden_boundaries
            .retain(|pat| !matches_boundary(&norm, pat));
    }

    for module in approved_modules {
        if !contract.boundary.expected_modules.contains(&module) {
            contract.boundary.expected_modules.push(module);
        }
    }

    contract.version = to_version;
    contract.status = ContractStatus::Reconciled;
    contract.provenance.origin = "RECONCILED".to_string();
    contract.provenance.ratification_status = RatificationStatus::Ratified;
    contract.provenance.ratified_by = Some(actor.to_string());
    contract.provenance.ratification_reason = Some(reason.to_string());
    contract.updated_at = Utc::now();
    contract.reconciliation_history.push(ReconciliationRecord {
        from_version,
        to_version,
        reason: reason.to_string(),
        actor: actor.to_string(),
        approved_additions: approved_files,
        timestamp: Utc::now(),
    });

    save_contract(&root, &contract)?;
    Ok(contract)
}
