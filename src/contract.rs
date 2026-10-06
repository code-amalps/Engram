//! Change Contract Engine: elastic boundary validation, versioning, baseline anchoring, and reconciliation.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractBoundary {
    pub expected_modules: Vec<String>,
    pub expected_files: Vec<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileClassification {
    pub path: String,
    pub category: FileValidationCategory,
    pub reason: String,
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
    let output = std::process::Command::new("git")
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
            // Auto-ground using ContextCompiler
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
            expected_symbols: Vec::new(),
            expected_tests: final_tests,
            forbidden_boundaries: forbidden,
            constraints,
        },
        reconciliation_history: Vec::new(),
        created_at: now_ts,
        updated_at: now_ts,
    };

    save_contract(&root, &contract)?;
    Ok(contract)
}

/// Validate actual changed files against the Change Contract elastic boundaries.
pub fn validate_contract_diff(
    contract: &ChangeContract,
    changed_files: &[String],
) -> ContractValidationResult {
    let mut classifications = Vec::new();
    let mut expected_count = 0;
    let mut expansion_count = 0;
    let mut unexpected_count = 0;
    let mut violation_count = 0;

    let normalized_expected: Vec<String> = contract
        .boundary
        .expected_files
        .iter()
        .map(|f| normalize_rel_path(f))
        .collect();

    for file in changed_files {
        let norm_file = normalize_rel_path(file);

        // 1. Check for FORBIDDEN boundary violations first (highest severity)
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
                reason: format!("File matches forbidden boundary constraint"),
            });
            continue;
        }

        // 2. Check if explicitly in expected files
        if normalized_expected.contains(&norm_file) {
            expected_count += 1;
            classifications.push(FileClassification {
                path: file.clone(),
                category: FileValidationCategory::Expected,
                reason: "File is explicitly part of the contract scope".to_string(),
            });
            continue;
        }

        // 3. Check if belongs to an expected module (EXPECTED_EXPANSION)
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

        // 4. Otherwise, it is an UNEXPECTED change (outside expected boundaries, but not forbidden)
        unexpected_count += 1;
        classifications.push(FileClassification {
            path: file.clone(),
            category: FileValidationCategory::Unexpected,
            reason: format!("File is outside expected files and modules"),
        });
    }

    // 5. Check for MISSING expected changes
    let mut missing_expected = Vec::new();
    for exp in &contract.boundary.expected_files {
        let norm_exp = normalize_rel_path(exp);
        if !changed_files.iter().any(|c| normalize_rel_path(c) == norm_exp) {
            missing_expected.push(exp.clone());
        }
    }
    for test in &contract.boundary.expected_tests {
        let norm_test = normalize_rel_path(test);
        if !changed_files.iter().any(|c| normalize_rel_path(c) == norm_test)
            && !missing_expected.contains(test)
        {
            missing_expected.push(test.clone());
        }
    }

    let is_valid = violation_count == 0;

    ContractValidationResult {
        contract_id: contract.contract_id.clone(),
        version: contract.version,
        is_valid,
        classifications,
        missing_expected_changes: missing_expected.clone(),
        summary: ContractSummary {
            total_changed: changed_files.len(),
            expected_count,
            expansion_count,
            unexpected_count,
            violation_count,
            missing_count: missing_expected.len(),
        },
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
