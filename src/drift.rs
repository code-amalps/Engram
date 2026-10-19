//! Decision/Code Drift and Scoped Staleness Subsystem:
//! Detects divergence between active governing decisions and repository evolution,
//! tracks historical contract violation frequency per module across commits,
//! measures boundary erosion with trend indicators (↓ improving, → stable, ↑ deteriorating),
//! and implements scoped staleness analysis with explicit ratification.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

use crate::contract::{extract_module, list_all_contracts};
use crate::memory::{
    load_durable_memories, save_durable_memories, DurableMemory, MemoryLifecycleState,
};
use crate::storage::{db, root_path};
use crate::telemetry::load_events;

pub const PROTOCOL_VERSION: &str = "1.1";
pub const DEFAULT_STALENESS_THRESHOLD: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendIndicator {
    #[serde(rename = "↓ improving")]
    Improving,
    #[serde(rename = "→ stable")]
    Stable,
    #[serde(rename = "↑ deteriorating")]
    Deteriorating,
}

impl std::fmt::Display for TrendIndicator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrendIndicator::Improving => write!(f, "↓ improving"),
            TrendIndicator::Stable => write!(f, "→ stable"),
            TrendIndicator::Deteriorating => write!(f, "↑ deteriorating"),
        }
    }
}

pub fn calculate_trend(older_count: usize, recent_count: usize) -> TrendIndicator {
    if recent_count < older_count {
        TrendIndicator::Improving
    } else if recent_count > older_count {
        TrendIndicator::Deteriorating
    } else {
        TrendIndicator::Stable
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommitRecord {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
    pub changed_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryErosionEvidence {
    pub source_module: String,
    pub target_boundary: String,
    pub violation_count: usize,
    #[serde(default)]
    pub files: Vec<String>,
    pub trend: TrendIndicator,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionDriftEvidence {
    pub decision_id: String,
    pub title: String,
    pub governed_target: String,
    pub global_commit_distance: usize,
    pub governed_mutations: usize,
    pub threshold: usize,
    #[serde(default)]
    pub ratified_at_commit: Option<String>,
    pub status: String,
    pub trend: TrendIndicator,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleDecisionEvidence {
    pub decision_id: String,
    pub title: String,
    pub governed_target: String,
    pub governed_mutations: usize,
    pub threshold: usize,
    #[serde(default)]
    pub ratified_at_commit: Option<String>,
    pub status: String,
    pub trend: TrendIndicator,
    pub recommended_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleContractFailureEvidence {
    pub module: String,
    pub failure_count: usize,
    pub total_checks: usize,
    pub failure_frequency: String,
    pub trend: TrendIndicator,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeterministicDriftEvidence {
    pub evidence_type: String, // "BOUNDARY_VIOLATIONS", "DECISION_DRIFT", "STALE_DECISIONS", "REPEATED_CONTRACT_FAILURES"
    pub target: String,
    pub count: usize,
    pub trend: TrendIndicator,
    pub details: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftCategory {
    DecisionDrift,
    BoundaryErosion,
    ConstitutionalDrift,
    StaleDecision,
    ContractDrift,
    IncompleteEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftSeverity {
    Info,
    Warn,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftConfidence {
    Confirmed,
    Potential,
    IncompleteEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriftFinding {
    pub id: String,
    pub category: DriftCategory,
    pub severity: DriftSeverity,
    pub confidence: DriftConfidence,
    pub reference_id: String,
    #[serde(default)]
    pub affected_paths: Vec<String>,
    #[serde(default)]
    pub affected_symbols: Vec<String>,
    pub expected_state: String,
    pub observed_state: String,
    pub explanation: String,
    pub suggested_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftOptions {
    #[serde(default)]
    pub module: Option<String>,
    #[serde(default)]
    pub decision: Option<String>,
    #[serde(default)]
    pub since: Option<String>,
    #[serde(default = "default_threshold")]
    pub staleness_threshold: usize,
}

fn default_threshold() -> usize {
    DEFAULT_STALENESS_THRESHOLD
}

impl Default for DriftOptions {
    fn default() -> Self {
        Self {
            module: None,
            decision: None,
            since: None,
            staleness_threshold: DEFAULT_STALENESS_THRESHOLD,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftReport {
    pub protocol_version: String,
    pub status: String,
    pub repository_status: String,
    pub global_commits_analyzed: usize,
    pub boundary_violations: Vec<BoundaryErosionEvidence>,
    pub decision_drift: Vec<DecisionDriftEvidence>,
    pub stale_decisions: Vec<StaleDecisionEvidence>,
    pub repeated_contract_failures: Vec<ModuleContractFailureEvidence>,
    pub deterministic_evidence: Vec<DeterministicDriftEvidence>,
    #[serde(default)]
    pub findings: Vec<DriftFinding>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// Extract commit history with list of changed files from Git.
pub fn extract_commits(
    root: &Path,
    since: Option<&str>,
    max_count: Option<usize>,
) -> Result<Result<Vec<GitCommitRecord>, String>> {
    if !crate::git::is_git_repo(root) {
        return Ok(Ok(Vec::new()));
    }

    let is_shallow = crate::git::is_shallow_repository(root);

    let resolved_since_sha: Option<String> = if let Some(since_commit) = since {
        if since_commit.trim().eq_ignore_ascii_case("WORKTREE") {
            return Ok(Ok(Vec::new()));
        }
        match crate::git::resolve_baseline(root, since_commit)? {
            Ok(sha) => {
                if sha.eq_ignore_ascii_case("WORKTREE") {
                    return Ok(Ok(Vec::new()));
                } else {
                    Some(sha)
                }
            }
            Err(err) => {
                if err == crate::git::SHALLOW_HISTORY_TRUNCATED || is_shallow {
                    return Ok(Err(crate::git::SHALLOW_HISTORY_TRUNCATED.to_string()));
                }
                return Ok(Err(err));
            }
        }
    } else {
        None
    };

    let mut args = vec![
        "log".to_string(),
        "--no-merges".to_string(),
        "--name-only".to_string(),
        "--format=__ENGRAM_DRIFT_COMMIT__:%H|%an|%ad|%s".to_string(),
    ];

    if let Some(ref sha) = resolved_since_sha {
        args.push(format!("{sha}..HEAD"));
    } else {
        let n = max_count.unwrap_or(100);
        args.push(format!("-n{n}"));
    }

    let output = Command::new("git").args(&args).current_dir(root).output();

    let output = match output {
        Ok(out) => {
            if !out.status.success() {
                if is_shallow {
                    return Ok(Err(crate::git::SHALLOW_HISTORY_TRUNCATED.to_string()));
                }
                return Ok(Ok(Vec::new()));
            }
            out
        }
        Err(_) => return Ok(Ok(Vec::new())),
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let mut commits = Vec::new();
    let mut current_commit: Option<GitCommitRecord> = None;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(header) = line.strip_prefix("__ENGRAM_DRIFT_COMMIT__:") {
            if let Some(c) = current_commit.take() {
                commits.push(c);
            }
            let parts: Vec<&str> = header.splitn(4, '|').collect();
            let hash = parts.first().copied().unwrap_or("").to_string();
            let author = parts.get(1).copied().unwrap_or("").to_string();
            let date = parts.get(2).copied().unwrap_or("").to_string();
            let message = parts.get(3).copied().unwrap_or("").to_string();

            current_commit = Some(GitCommitRecord {
                hash,
                author,
                date,
                message,
                changed_files: Vec::new(),
            });
        } else if let Some(ref mut c) = current_commit {
            let norm = crate::git::normalize_git_path(line);
            if !crate::git::is_internal_metadata_path(&norm)
                && !crate::git_history::is_noise_file(&norm)
            {
                c.changed_files.push(norm);
            }
        }
    }

    if let Some(c) = current_commit {
        commits.push(c);
    }

    // Git outputs newest first; reverse so commits[0] is oldest, last is newest (chronological order)
    commits.reverse();

    Ok(Ok(commits))
}

/// Match whether a changed file touches the governed surface of a durable memory.
pub fn matches_governed_surface(root: &Path, memory: &DurableMemory, changed_file: &str) -> bool {
    let norm_path = changed_file.replace('\\', "/").to_ascii_lowercase();

    // Global scope governs all codebase changes (and Repository scope when target is None)
    if memory.scope.level == crate::memory::ScopeLevel::Global
        || (memory.scope.level == crate::memory::ScopeLevel::Repository
            && memory.scope.target.is_none())
    {
        return !crate::git_history::is_noise_file(changed_file);
    }

    // 1. Exact or suffix match on scope.files
    for f in &memory.scope.files {
        let norm_f = f.replace('\\', "/").to_ascii_lowercase();
        if norm_path == norm_f
            || norm_path.ends_with(&format!("/{norm_f}"))
            || norm_f.ends_with(&format!("/{norm_path}"))
            || norm_path.ends_with(&norm_f)
        {
            return true;
        }
    }

    // 2. Module matching
    let mod_name = extract_module(changed_file).to_ascii_lowercase();
    let file_stem = Path::new(changed_file)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    for m in &memory.scope.modules {
        let norm_m = m.to_ascii_lowercase();
        if mod_name == norm_m
            || file_stem == norm_m
            || norm_path.starts_with(&format!("{norm_m}/"))
            || norm_path.contains(&format!("/{norm_m}/"))
            || norm_path.starts_with(&format!("src/{norm_m}."))
            || norm_path.starts_with(&format!("{norm_m}."))
        {
            return true;
        }
    }

    // Helper checking whether a changed file contains or defines a given symbol
    let file_contains_symbol = |sym: &str| -> bool {
        // Noise files and documentation/plain text cannot define code symbols
        if crate::git_history::is_noise_file(changed_file) {
            return false;
        }

        let ext = Path::new(changed_file)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if matches!(
            ext.as_str(),
            "md" | "markdown"
                | "txt"
                | "rst"
                | "svg"
                | "png"
                | "jpg"
                | "toml"
                | "yaml"
                | "yml"
                | "json"
        ) {
            return false;
        }

        let file_stem = Path::new(changed_file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if file_stem == sym.to_ascii_lowercase() {
            return true;
        }

        // Query AST database if available
        if let Ok(conn) = db(root) {
            let norm_cf = changed_file.replace('\\', "/");
            let pattern = format!("%{norm_cf}%");
            let exists = conn
                .prepare(
                    "SELECT 1 FROM entity_versions WHERE name = ?1 AND path LIKE ?2 AND invalidated_at_revision IS NULL LIMIT 1",
                )
                .and_then(|mut stmt| stmt.exists(rusqlite::params![sym, &pattern]))
                .unwrap_or(false);
            if exists {
                return true;
            }
        }

        // Check file content on disk for code files
        let abs = root.join(changed_file);
        if std::fs::read_to_string(&abs).is_ok_and(|c| c.contains(sym)) {
            return true;
        }

        false
    };

    // 3. Target matching
    if let Some(ref target) = memory.scope.target {
        let norm_t = target.to_ascii_lowercase();
        let file_stem = Path::new(changed_file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match memory.scope.level {
            crate::memory::ScopeLevel::Module | crate::memory::ScopeLevel::Package => {
                if mod_name == norm_t
                    || norm_path.starts_with(&format!("{norm_t}/"))
                    || norm_path.contains(&format!("/{norm_t}/"))
                    || norm_path == norm_t
                    || norm_path.ends_with(&norm_t)
                    || norm_path.ends_with(&format!("/{norm_t}"))
                    || file_stem == norm_t
                {
                    return true;
                }
            }
            crate::memory::ScopeLevel::Symbol => {
                if file_contains_symbol(target) {
                    return true;
                }
            }
            _ => {
                if mod_name == norm_t
                    || norm_path.starts_with(&format!("{norm_t}/"))
                    || norm_path.contains(&format!("/{norm_t}/"))
                    || norm_path == norm_t
                    || norm_path.ends_with(&norm_t)
                    || norm_path.ends_with(&format!("/{norm_t}"))
                    || file_stem == norm_t
                    || file_contains_symbol(target)
                {
                    return true;
                }
            }
        }
    }

    // 4. Scope symbols matching
    for sym in &memory.scope.symbols {
        if file_contains_symbol(sym) {
            return true;
        }
    }

    false
}

/// Helper matching either glob boundary or module directory name.
pub fn matches_boundary_or_module(path: &str, pattern: &str) -> bool {
    let norm_path = crate::contract::normalize_rel_path(path).to_ascii_lowercase();
    let norm_pat = crate::contract::normalize_rel_path(pattern).to_ascii_lowercase();
    let mod_name = extract_module(&norm_path).to_ascii_lowercase();
    let file_stem = Path::new(&norm_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    crate::contract::matches_boundary(&norm_path, &norm_pat)
        || mod_name == norm_pat
        || file_stem == norm_pat
        || norm_path.contains(&format!("/{norm_pat}/"))
        || norm_path.starts_with(&format!("{norm_pat}/"))
        || norm_path.ends_with(&format!("/{norm_pat}"))
        || norm_path.starts_with(&format!("src/{norm_pat}."))
        || norm_path.starts_with(&format!("{norm_pat}."))
}

#[derive(Default)]
struct BoundaryViolationStat {
    older: usize,
    recent: usize,
    total: usize,
    files: HashSet<String>,
}

#[derive(Default)]
struct ModuleContractStat {
    older_fails: usize,
    recent_fails: usize,
    total_fails: usize,
    total_checks: usize,
}

/// Perform comprehensive architectural drift, boundary erosion, and scoped staleness analysis.
pub fn analyze_drift(root: &Path, options: &DriftOptions) -> Result<DriftReport> {
    let root = root_path(root)?;
    let is_shallow = crate::git::is_shallow_repository(&root);

    let mut warnings = Vec::new();
    let mut status = "OK".to_string();
    let mut repository_status = "NORMAL".to_string();

    if is_shallow {
        status = crate::git::SHALLOW_HISTORY_TRUNCATED.to_string();
        repository_status = crate::git::SHALLOW_HISTORY_TRUNCATED.to_string();
        warnings.push(format!(
            "{}: Repository history is shallow (commit history truncated). Partial drift analysis performed.",
            crate::git::SHALLOW_HISTORY_TRUNCATED
        ));
    }

    // Extract Git commit history
    let commits = match extract_commits(&root, options.since.as_deref(), None)? {
        Ok(c) => c,
        Err(err) => {
            if err == crate::git::SHALLOW_HISTORY_TRUNCATED || is_shallow {
                status = crate::git::SHALLOW_HISTORY_TRUNCATED.to_string();
                repository_status = crate::git::SHALLOW_HISTORY_TRUNCATED.to_string();
                warnings.push(format!(
                    "{}: Commit history is truncated in shallow repository.",
                    crate::git::SHALLOW_HISTORY_TRUNCATED
                ));
                Vec::new()
            } else {
                warnings.push(format!("Git log warning: {err}"));
                Vec::new()
            }
        }
    };

    let global_commits_analyzed = commits.len();

    // 1. Scoped Decision Drift & Staleness Analysis
    let (mut doc, mem_load_err) = match load_durable_memories(&root) {
        Ok(d) => (d, None),
        Err(e) => {
            warnings.push(format!("Memory database unavailable or corrupt: {e}"));
            (
                crate::memory::DurableMemoryDocument::default(),
                Some(e.to_string()),
            )
        }
    };
    let mut doc_modified = false;

    let mut decision_drift = Vec::new();
    let mut stale_decisions = Vec::new();

    for mem in &mut doc.memories {
        // Only evaluate active Durable or already StaleReview decisions
        if mem.lifecycle_state != MemoryLifecycleState::Durable
            && mem.lifecycle_state != MemoryLifecycleState::StaleReview
        {
            continue;
        }

        // Apply filters if requested
        if options.decision.as_ref().is_some_and(|d| &mem.id != d) {
            continue;
        }

        if let Some(ref m_filter) = options.module {
            let m_lower = m_filter.to_ascii_lowercase();
            let matches_mod = mem.scope.level == crate::memory::ScopeLevel::Global
                || mem
                    .scope
                    .modules
                    .iter()
                    .any(|m| m.to_ascii_lowercase() == m_lower)
                || mem
                    .scope
                    .target
                    .as_ref()
                    .is_some_and(|t| t.to_ascii_lowercase() == m_lower);
            if !matches_mod {
                continue;
            }
        }

        let ratified_sha = mem.ratified_commit().map(str::to_string);

        let resolved_ratified_sha: Option<String> = ratified_sha.as_deref().and_then(|sha| {
            if sha == "HEAD" {
                crate::git::current_revision(&root).ok()
            } else if sha.eq_ignore_ascii_case("WORKTREE") {
                Some("WORKTREE".to_string())
            } else if sha.len() < 40 {
                crate::git::resolve_baseline(&root, sha)
                    .ok()
                    .and_then(|r| r.ok())
            } else {
                Some(sha.to_string())
            }
        });

        let target_sha = resolved_ratified_sha.as_deref().or(ratified_sha.as_deref());

        // Slice commits that occurred strictly after ratification
        let commits_since_ratification: &[GitCommitRecord] = if let Some(sha) = target_sha {
            if sha.eq_ignore_ascii_case("WORKTREE") {
                &[]
            } else if let Some(pos) = commits
                .iter()
                .position(|c| c.hash.starts_with(sha) || sha.starts_with(&c.hash))
            {
                // commits are chronological; take everything after pos
                &commits[pos + 1..]
            } else {
                // If ratified commit is older than our window or not found, evaluate window
                &commits[..]
            }
        } else {
            &commits[..]
        };

        let global_dist = commits_since_ratification.len();

        // Count governed mutations: commits touching files matching governed surface
        let mut governed_mutations = 0;
        let mut older_window_mutations = 0;
        let mut recent_window_mutations = 0;

        let half_point = commits_since_ratification.len() / 2;

        for (idx, commit) in commits_since_ratification.iter().enumerate() {
            let touched = commit
                .changed_files
                .iter()
                .any(|f| matches_governed_surface(&root, mem, f));
            if touched {
                governed_mutations += 1;
                if commits_since_ratification.len() > 1 {
                    if idx < half_point {
                        older_window_mutations += 1;
                    } else {
                        recent_window_mutations += 1;
                    }
                }
            }
        }

        let trend = if commits_since_ratification.len() <= 1 {
            TrendIndicator::Stable
        } else {
            calculate_trend(older_window_mutations, recent_window_mutations)
        };
        let target_desc = mem
            .scope
            .target
            .clone()
            .or_else(|| mem.scope.modules.first().cloned())
            .unwrap_or_else(|| "global".to_string());

        let is_stale = governed_mutations >= options.staleness_threshold;

        if is_stale {
            if mem.lifecycle_state != MemoryLifecycleState::StaleReview {
                mem.lifecycle_state = MemoryLifecycleState::StaleReview;
                doc_modified = true;
            }

            stale_decisions.push(StaleDecisionEvidence {
                decision_id: mem.id.clone(),
                title: mem.title.clone(),
                governed_target: target_desc.clone(),
                governed_mutations,
                threshold: options.staleness_threshold,
                ratified_at_commit: ratified_sha.clone(),
                status: "STALE_REVIEW".to_string(),
                trend,
                recommended_action: format!(
                    "Run 'engram memory re-ratify {} --commit <commit> --by <name>' to review and re-ratify.",
                    mem.id
                ),
            });
        }

        decision_drift.push(DecisionDriftEvidence {
            decision_id: mem.id.clone(),
            title: mem.title.clone(),
            governed_target: target_desc,
            global_commit_distance: global_dist,
            governed_mutations,
            threshold: options.staleness_threshold,
            ratified_at_commit: ratified_sha.clone(),
            status: if is_stale {
                "STALE_REVIEW".to_string()
            } else {
                "ACTIVE".to_string()
            },
            trend,
            details: format!(
                "Governed surface mutated {} time(s) across {} global commit(s) since {}.",
                governed_mutations,
                global_dist,
                ratified_sha.as_deref().unwrap_or("genesis")
            ),
        });
    }

    if doc_modified {
        let _ = save_durable_memories(&root, &doc);
    }

    // 2. Boundary Erosion and Historical Contract Violations Analysis
    let all_contracts = list_all_contracts(&root).unwrap_or_default();
    let events = load_events(&root).unwrap_or_default();

    let mut boundary_violations = Vec::new();
    let mut repeated_contract_failures = Vec::new();

    // Map: (source_module, target_boundary) -> BoundaryViolationStat
    let mut boundary_stats: HashMap<(String, String), BoundaryViolationStat> = HashMap::new();

    // Module -> ModuleContractStat
    let mut module_stats: HashMap<String, ModuleContractStat> = HashMap::new();

    // Track violations directly across commits from contract forbidden boundaries
    for contract in &all_contracts {
        let exp_modules = if !contract.boundary.expected_modules.is_empty() {
            contract.boundary.expected_modules.clone()
        } else if !contract.boundary.expected_files.is_empty() {
            let mut mods: Vec<String> = contract
                .boundary
                .expected_files
                .iter()
                .map(|f| extract_module(f))
                .collect();
            mods.sort();
            mods.dedup();
            mods
        } else {
            vec!["root".to_string()]
        };

        for exp in &exp_modules {
            let norm_exp = exp.to_ascii_lowercase();
            let mod_commits: Vec<&GitCommitRecord> = commits
                .iter()
                .filter(|c| {
                    c.changed_files.iter().any(|f| {
                        let m = extract_module(f).to_ascii_lowercase();
                        let stem = Path::new(f)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_ascii_lowercase();
                        m == norm_exp || stem == norm_exp || f.contains(&format!("/{norm_exp}/"))
                    })
                })
                .collect();

            if mod_commits.is_empty() {
                continue;
            }

            let stat = module_stats.entry(exp.clone()).or_default();
            stat.total_checks += mod_commits.len();

            let half_idx = mod_commits.len() / 2;

            for (m_idx, commit) in mod_commits.iter().enumerate() {
                let is_recent = m_idx >= half_idx;
                let mut commit_violated = false;

                for forbidden in &contract.boundary.forbidden_boundaries {
                    let violated_files: Vec<String> = commit
                        .changed_files
                        .iter()
                        .filter(|f| matches_boundary_or_module(f, forbidden))
                        .cloned()
                        .collect();

                    if !violated_files.is_empty() {
                        commit_violated = true;
                        let key = (exp.clone(), forbidden.clone());
                        let b_stat = boundary_stats.entry(key).or_default();
                        b_stat.total += 1;
                        if is_recent {
                            b_stat.recent += 1;
                        } else {
                            b_stat.older += 1;
                        }
                        for vf in violated_files {
                            b_stat.files.insert(vf);
                        }
                    }
                }

                if commit_violated {
                    if is_recent {
                        stat.recent_fails += 1;
                    } else {
                        stat.older_fails += 1;
                    }
                    stat.total_fails += 1;
                }
            }
        }
    }

    // Incorporate telemetry BOUNDARY_CHECK events
    let boundary_events: Vec<&crate::telemetry::TelemetryEvent> = events
        .iter()
        .filter(|e| e.event_type == "BOUNDARY_CHECK")
        .collect();

    let half_event_idx = boundary_events.len() / 2;

    for (idx, ev) in boundary_events.iter().enumerate() {
        let is_recent = idx >= half_event_idx;
        let b_violations = ev.payload["boundary_violations"].as_u64().unwrap_or(0) as usize;
        let exit_code = ev.payload["exit_code"].as_i64().unwrap_or(0);

        if let Some(c) = ev
            .contract_id
            .as_deref()
            .and_then(|c_id| crate::contract::get_contract(&root, c_id).ok())
        {
            let mod_name = c
                .boundary
                .expected_modules
                .first()
                .cloned()
                .unwrap_or_else(|| "general".to_string());

            let stat = module_stats.entry(mod_name.clone()).or_default();
            stat.total_checks += 1;

            if exit_code != 0 || b_violations > 0 {
                stat.total_fails += 1;
                if is_recent {
                    stat.recent_fails += 1;
                } else {
                    stat.older_fails += 1;
                }

                if b_violations > 0 {
                    let target_b = c
                        .boundary
                        .forbidden_boundaries
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "forbidden_boundary".to_string());
                    let key = (mod_name, target_b);
                    let b_stat = boundary_stats.entry(key).or_default();
                    b_stat.total += b_violations;
                    if is_recent {
                        b_stat.recent += b_violations;
                    } else {
                        b_stat.older += b_violations;
                    }
                }
            }
        }
    }

    // Build BoundaryErosionEvidence
    for ((source_mod, target_b), stat) in boundary_stats {
        if options
            .module
            .as_ref()
            .is_some_and(|m| !source_mod.eq_ignore_ascii_case(m))
        {
            continue;
        }

        let trend = calculate_trend(stat.older, stat.recent);
        let mut sorted_files: Vec<String> = stat.files.into_iter().collect();
        sorted_files.sort();

        boundary_violations.push(BoundaryErosionEvidence {
            source_module: source_mod.clone(),
            target_boundary: target_b.clone(),
            violation_count: stat.total,
            files: sorted_files,
            trend,
            details: format!(
                "Boundary erosion: {} violation(s) crossing from '{}' into '{}' (trend: {}).",
                stat.total, source_mod, target_b, trend
            ),
        });
    }

    // Build ModuleContractFailureEvidence
    for (module_name, stat) in module_stats {
        if options
            .module
            .as_ref()
            .is_some_and(|m| !module_name.eq_ignore_ascii_case(m))
        {
            continue;
        }

        if stat.total_fails > 0 {
            let trend = calculate_trend(stat.older_fails, stat.recent_fails);
            let total_chk = stat.total_checks.max(stat.total_fails);
            repeated_contract_failures.push(ModuleContractFailureEvidence {
                module: module_name.clone(),
                failure_count: stat.total_fails,
                total_checks: total_chk,
                failure_frequency: format!("{}/{total_chk}", stat.total_fails),
                trend,
                details: format!(
                    "Module '{}' failed contract checks in {} of {} checks/commits (trend: {}).",
                    module_name, stat.total_fails, total_chk, trend
                ),
            });
        }
    }

    // Sort outputs deterministically
    boundary_violations.sort_by_key(|b| std::cmp::Reverse(b.violation_count));
    decision_drift.sort_by_key(|d| std::cmp::Reverse(d.governed_mutations));
    stale_decisions.sort_by_key(|s| std::cmp::Reverse(s.governed_mutations));
    repeated_contract_failures.sort_by_key(|r| std::cmp::Reverse(r.failure_count));

    // Construct unified Deterministic Evidence list
    let mut deterministic_evidence = Vec::new();

    for bv in &boundary_violations {
        deterministic_evidence.push(DeterministicDriftEvidence {
            evidence_type: "BOUNDARY_VIOLATIONS".to_string(),
            target: format!("{} -> {}", bv.source_module, bv.target_boundary),
            count: bv.violation_count,
            trend: bv.trend,
            details: bv.details.clone(),
        });
    }

    for dd in &decision_drift {
        if dd.governed_mutations > 0 {
            deterministic_evidence.push(DeterministicDriftEvidence {
                evidence_type: "DECISION_DRIFT".to_string(),
                target: format!("{} ({})", dd.decision_id, dd.governed_target),
                count: dd.governed_mutations,
                trend: dd.trend,
                details: dd.details.clone(),
            });
        }
    }

    for sd in &stale_decisions {
        deterministic_evidence.push(DeterministicDriftEvidence {
            evidence_type: "STALE_DECISIONS".to_string(),
            target: format!("{} ({})", sd.decision_id, sd.governed_target),
            count: sd.governed_mutations,
            trend: sd.trend,
            details: format!(
                "Decision '{}' reached {} mutations without re-ratification (threshold: {}) -> {}.",
                sd.decision_id, sd.governed_mutations, sd.threshold, sd.status
            ),
        });
    }

    for rf in &repeated_contract_failures {
        deterministic_evidence.push(DeterministicDriftEvidence {
            evidence_type: "REPEATED_CONTRACT_FAILURES".to_string(),
            target: rf.module.clone(),
            count: rf.failure_count,
            trend: rf.trend,
            details: rf.details.clone(),
        });
    }

    // Construct structured findings
    let mut findings = Vec::new();

    if let Some(ref err_msg) = mem_load_err {
        findings.push(DriftFinding {
            id: "DRIFT-ANALYSIS-MEMORY-UNAVAILABLE".to_string(),
            category: DriftCategory::IncompleteEvidence,
            severity: DriftSeverity::Info,
            confidence: DriftConfidence::IncompleteEvidence,
            reference_id: "MEMORY_STORE".to_string(),
            affected_paths: Vec::new(),
            affected_symbols: Vec::new(),
            expected_state: "Durable memory store readable and well-formed".to_string(),
            observed_state: format!("Failed reading memory store: {err_msg}"),
            explanation:
                "Could not evaluate decision drift due to unavailable or corrupt memory store."
                    .to_string(),
            suggested_action: "Inspect .engram/memory.json or reinitialize with 'engram init'"
                .to_string(),
        });
    }

    if is_shallow {
        findings.push(DriftFinding {
            id: "DRIFT-INCOMPLETE-SHALLOW-HISTORY".to_string(),
            category: DriftCategory::IncompleteEvidence,
            severity: DriftSeverity::Info,
            confidence: DriftConfidence::IncompleteEvidence,
            reference_id: "GIT_HISTORY".to_string(),
            affected_paths: Vec::new(),
            affected_symbols: Vec::new(),
            expected_state: "Complete Git commit history available for audit".to_string(),
            observed_state: "Shallow Git clone detected (commit history truncated)".to_string(),
            explanation: "Shallow Git history limits mutation distance calculation.".to_string(),
            suggested_action:
                "Run 'git fetch --unshallow' to enable complete historical drift analysis"
                    .to_string(),
        });
    }

    if db(&root).is_err() {
        findings.push(DriftFinding {
            id: "DRIFT-INCOMPLETE-INDEX".to_string(),
            category: DriftCategory::IncompleteEvidence,
            severity: DriftSeverity::Info,
            confidence: DriftConfidence::IncompleteEvidence,
            reference_id: "SQLITE_INDEX".to_string(),
            affected_paths: Vec::new(),
            affected_symbols: Vec::new(),
            expected_state: "SQLite symbol index and entity relationship graph initialized"
                .to_string(),
            observed_state: "SQLite index is unavailable or uninitialized".to_string(),
            explanation:
                "AST entity relationships could not be queried; relying on commit diff analysis."
                    .to_string(),
            suggested_action: "Run 'engram analyze' to build code intelligence database"
                .to_string(),
        });
    }

    // Check Project Constitution rules for boundary violations
    if let Ok(constitution_rules) = crate::constitution::load_constitution(&root) {
        for rule in &constitution_rules {
            if let Some(ref forbidden) = rule.forbidden {
                let mut violated_files = HashSet::new();
                for commit in &commits {
                    for file in &commit.changed_files {
                        if matches_boundary_or_module(file, forbidden) {
                            violated_files.insert(file.clone());
                        }
                    }
                }
                if !violated_files.is_empty() {
                    let mut sorted_files: Vec<String> = violated_files.into_iter().collect();
                    sorted_files.sort();
                    findings.push(DriftFinding {
                        id: format!("DRIFT-CONST-{}", rule.id),
                        category: DriftCategory::ConstitutionalDrift,
                        severity: DriftSeverity::Block,
                        confidence: DriftConfidence::Confirmed,
                        reference_id: rule.id.clone(),
                        affected_paths: sorted_files.clone(),
                        affected_symbols: Vec::new(),
                        expected_state: format!(
                            "Constitution rule '{}' strictly forbids modifications in '{}'",
                            rule.name, forbidden
                        ),
                        observed_state: format!(
                            "Code modified in forbidden boundary: {}",
                            sorted_files.join(", ")
                        ),
                        explanation: format!(
                            "Constitution rule '{}' ({}) was breached across analyzed commits.",
                            rule.name, rule.id
                        ),
                        suggested_action:
                            "Revert forbidden modifications or request human constitutional amendment"
                                .to_string(),
                    });
                }
            }
        }
    }

    // Check for Provisional decisions
    for mem in &doc.memories {
        if mem.lifecycle_state == MemoryLifecycleState::Provisional {
            if options.decision.as_ref().is_some_and(|d| &mem.id != d) {
                continue;
            }
            if let Some(ref m_filter) = options.module {
                let m_lower = m_filter.to_ascii_lowercase();
                let matches_mod = mem.scope.level == crate::memory::ScopeLevel::Global
                    || mem
                        .scope
                        .modules
                        .iter()
                        .any(|m| m.to_ascii_lowercase() == m_lower)
                    || mem
                        .scope
                        .target
                        .as_ref()
                        .is_some_and(|t| t.to_ascii_lowercase() == m_lower);
                if !matches_mod {
                    continue;
                }
            }
            let mut paths = mem.scope.files.clone();
            paths.sort();
            let mut symbols = mem.scope.symbols.clone();
            symbols.sort();
            findings.push(DriftFinding {
                id: format!("DRIFT-DEC-PROVISIONAL-{}", mem.id),
                category: DriftCategory::DecisionDrift,
                severity: DriftSeverity::Info,
                confidence: DriftConfidence::Potential,
                reference_id: mem.id.clone(),
                affected_paths: paths,
                affected_symbols: symbols,
                expected_state: "Architectural decision is ratified by human authority".to_string(),
                observed_state: "Decision is PROVISIONAL (proposed automatically or by agent without human ratification)".to_string(),
                explanation: format!(
                    "Provisional decision '{}' (\"{}\") is unratified and pending human review.",
                    mem.id, mem.title
                ),
                suggested_action: format!(
                    "Run 'engram memory ratify {} --by <name> --reason <reason>' to ratify, or 'engram memory reject {}' to reject.",
                    mem.id, mem.id
                ),
            });
        }
    }

    // Stale Decisions
    for sd in &stale_decisions {
        let (paths, symbols) =
            if let Some(mem) = doc.memories.iter().find(|m| m.id == sd.decision_id) {
                let mut p = mem.scope.files.clone();
                p.sort();
                let mut s = mem.scope.symbols.clone();
                s.sort();
                (p, s)
            } else {
                (Vec::new(), Vec::new())
            };

        findings.push(DriftFinding {
            id: format!("DRIFT-DEC-STALE-{}", sd.decision_id),
            category: DriftCategory::StaleDecision,
            severity: DriftSeverity::Warn,
            confidence: DriftConfidence::Confirmed,
            reference_id: sd.decision_id.clone(),
            affected_paths: paths,
            affected_symbols: symbols,
            expected_state: format!(
                "Decision '{}' expected re-ratification before reaching {} mutations",
                sd.decision_id, sd.threshold
            ),
            observed_state: format!(
                "Governed surface mutated {} times without re-ratification",
                sd.governed_mutations
            ),
            explanation: format!(
                "Decision '{}' (\"{}\") entered STALE_REVIEW after {} governed mutations.",
                sd.decision_id, sd.title, sd.governed_mutations
            ),
            suggested_action: sd.recommended_action.clone(),
        });
    }

    // Boundary Erosion
    for bv in &boundary_violations {
        let clean_target = bv
            .target_boundary
            .replace(['/', '*', '.', ' '], "-")
            .trim_matches('-')
            .to_string();
        findings.push(DriftFinding {
            id: format!(
                "DRIFT-BOUNDARY-EROSION-{}-{}",
                bv.source_module, clean_target
            ),
            category: DriftCategory::BoundaryErosion,
            severity: DriftSeverity::Warn,
            confidence: DriftConfidence::Confirmed,
            reference_id: format!("{}->{}", bv.source_module, bv.target_boundary),
            affected_paths: bv.files.clone(),
            affected_symbols: Vec::new(),
            expected_state: format!(
                "Module '{}' must not breach forbidden boundary '{}'",
                bv.source_module, bv.target_boundary
            ),
            observed_state: format!(
                "Observed {} cross-boundary violation(s) (trend: {})",
                bv.violation_count, bv.trend
            ),
            explanation: bv.details.clone(),
            suggested_action:
                "Review cross-boundary coupling or formally ratify an expanded boundary contract"
                    .to_string(),
        });
    }

    findings.sort_by(|a, b| a.id.cmp(&b.id));

    // Determine final status
    if status != crate::git::SHALLOW_HISTORY_TRUNCATED {
        let has_confirmed_drift = findings.iter().any(|f| {
            (f.severity == DriftSeverity::Warn || f.severity == DriftSeverity::Block)
                && f.confidence == DriftConfidence::Confirmed
        });
        if has_confirmed_drift
            || !stale_decisions.is_empty()
            || !boundary_violations.is_empty()
            || !repeated_contract_failures.is_empty()
        {
            status = "DRIFT_DETECTED".to_string();
        } else {
            status = "OK".to_string();
        }
    }

    let _ = crate::telemetry::record_event(
        &root,
        "DRIFT_EVALUATION",
        None,
        "drift_analyzer",
        serde_json::json!({
            "status": &status,
            "findings_count": findings.len(),
            "stale_decisions_count": stale_decisions.len(),
            "boundary_violations_count": boundary_violations.len(),
        }),
    );

    Ok(DriftReport {
        protocol_version: PROTOCOL_VERSION.to_string(),
        status,
        repository_status,
        global_commits_analyzed,
        boundary_violations,
        decision_drift,
        stale_decisions,
        repeated_contract_failures,
        deterministic_evidence,
        findings,
        warnings,
    })
}

/// Format human-readable drift report for terminals.
pub fn format_drift_report(report: &DriftReport) -> String {
    let mut out = String::new();
    out.push_str("🌊 Engram Architectural Drift & Boundary Analysis\n");
    out.push_str("──────────────────────────────────────────\n");
    out.push_str(&format!(
        "  Protocol:          {}\n",
        report.protocol_version
    ));
    out.push_str(&format!("  Status:            {}\n", report.status));
    out.push_str(&format!(
        "  Repository:        {}\n",
        report.repository_status
    ));
    out.push_str(&format!(
        "  Commits Analyzed:  {}\n",
        report.global_commits_analyzed
    ));
    out.push_str("──────────────────────────────────────────\n");

    if !report.warnings.is_empty() {
        out.push_str("\nAdvisories & Notices:\n");
        for w in &report.warnings {
            out.push_str(&format!("  ⚠️  {}\n", w));
        }
    }

    if !report.stale_decisions.is_empty() {
        out.push_str("\nStale Decisions Requiring Re-ratification (STALE_REVIEW):\n");
        for s in &report.stale_decisions {
            out.push_str(&format!("  ⚠️  {} (\"{}\")\n", s.decision_id, s.title));
            out.push_str(&format!("      Governed Target:  {}\n", s.governed_target));
            out.push_str(&format!(
                "      Mutations:        {} (threshold: {})\n",
                s.governed_mutations, s.threshold
            ));
            out.push_str(&format!("      Trend:            {}\n", s.trend));
            out.push_str(&format!(
                "      Action:           {}\n",
                s.recommended_action
            ));
        }
    }

    if !report.findings.is_empty() {
        out.push_str("\nStructured Architectural Findings:\n");
        for f in &report.findings {
            let icon = match f.severity {
                DriftSeverity::Block => "❌ BLOCK",
                DriftSeverity::Warn => "⚠️  WARN ",
                DriftSeverity::Info => "ℹ️  INFO ",
            };
            out.push_str(&format!(
                "  {} [{}] ({:?}) {}\n",
                icon, f.id, f.confidence, f.explanation
            ));
            if !f.affected_paths.is_empty() {
                out.push_str(&format!(
                    "      Paths:    {}\n",
                    f.affected_paths.join(", ")
                ));
            }
            if !f.affected_symbols.is_empty() {
                out.push_str(&format!(
                    "      Symbols:  {}\n",
                    f.affected_symbols.join(", ")
                ));
            }
            out.push_str(&format!("      Action:   {}\n", f.suggested_action));
        }
    }

    if !report.deterministic_evidence.is_empty() {
        out.push_str("\nDeterministic Evidence:\n");
        for e in &report.deterministic_evidence {
            out.push_str(&format!(
                "  • [{}] {} (count: {}) [{}]\n",
                e.evidence_type, e.target, e.count, e.trend
            ));
            out.push_str(&format!("      → {}\n", e.details));
        }
    } else if report.findings.is_empty() {
        out.push_str("\n  ✓ No architectural drift or boundary erosion detected.\n");
    }

    out
}
