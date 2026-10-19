//! Durable Engineering Memory: human rationale, promotion lifecycle, poisoning protection, and Git persistence.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::model::EvidenceTier;
use crate::storage::{cache_dir, root_path};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MemoryLifecycleState {
    Provisional,
    Candidate,
    Review,
    Accepted,
    Rejected,
    Durable,
    Superseded,
    Dormant,
    StaleReview,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScopeLevel {
    Global,     // Applies across entire repository/project
    Repository, // Monorepo sub-repository / workspace root
    Module,     // Specific module or architectural layer (e.g. orders, payment)
    Package,    // Exact package path (e.g. internal/orders/export)
    Symbol,     // Exact symbol (e.g. OrderService.ExportCSV)
}

fn default_scope_level() -> ScopeLevel {
    ScopeLevel::Module
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryScope {
    #[serde(default = "default_scope_level")]
    pub level: ScopeLevel,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default)]
    pub modules: Vec<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub symbols: Vec<String>,
}

impl Default for MemoryScope {
    fn default() -> Self {
        Self {
            level: ScopeLevel::Global,
            target: None,
            team: None,
            modules: Vec::new(),
            files: Vec::new(),
            symbols: Vec::new(),
        }
    }
}

impl MemoryScope {
    pub fn global() -> Self {
        Self {
            level: ScopeLevel::Global,
            target: None,
            team: None,
            modules: Vec::new(),
            files: Vec::new(),
            symbols: Vec::new(),
        }
    }

    pub fn repository(target: &str) -> Self {
        Self {
            level: ScopeLevel::Repository,
            target: Some(target.to_string()),
            team: None,
            modules: Vec::new(),
            files: Vec::new(),
            symbols: Vec::new(),
        }
    }

    pub fn module(name: &str) -> Self {
        Self {
            level: ScopeLevel::Module,
            target: Some(name.to_string()),
            team: None,
            modules: vec![name.to_string()],
            files: Vec::new(),
            symbols: Vec::new(),
        }
    }

    pub fn package(path: &str) -> Self {
        Self {
            level: ScopeLevel::Package,
            target: Some(path.to_string()),
            team: None,
            modules: Vec::new(),
            files: vec![path.to_string()],
            symbols: Vec::new(),
        }
    }

    pub fn symbol(sym: &str) -> Self {
        Self {
            level: ScopeLevel::Symbol,
            target: Some(sym.to_string()),
            team: None,
            modules: Vec::new(),
            files: Vec::new(),
            symbols: vec![sym.to_string()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemorySource {
    pub source_type: String, // "human_decision", "pr_review", "adr"
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryProvenance {
    pub source_type: String, // "human_decision", "pr_review", "adr", "promoted_candidate", "supersession"
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub source_commit: Option<String>,
    #[serde(default)]
    pub source_contract: Option<String>,
    #[serde(default)]
    pub source_decision: Option<String>,
    #[serde(default)]
    pub ratified_by: Option<String>,
    #[serde(default)]
    pub ratified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DurableMemory {
    pub id: String,
    pub title: String,
    pub statement: String,
    pub reason: String,
    #[serde(default)]
    pub source: Option<MemorySource>,
    #[serde(default)]
    pub provenance: Option<MemoryProvenance>,
    #[serde(default)]
    pub accepted_by: String,
    #[serde(default = "Utc::now")]
    pub accepted_at: DateTime<Utc>,
    pub scope: MemoryScope,
    pub evidence_tier: EvidenceTier,
    pub lifecycle_state: MemoryLifecycleState,
    #[serde(default)]
    pub supersedes: Option<String>,
    #[serde(default)]
    pub superseded_by: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub ratified_at_commit: Option<String>,
    #[serde(default)]
    pub ratified_by: Option<String>,
    #[serde(default)]
    pub ratified_at: Option<DateTime<Utc>>,
}

impl DurableMemory {
    pub fn ratified_commit(&self) -> Option<&str> {
        self.ratified_at_commit.as_deref().or_else(|| {
            self.provenance
                .as_ref()
                .and_then(|p| p.source_commit.as_deref())
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DurableMemoryDocument {
    pub version: u32,
    pub memories: Vec<DurableMemory>,
}

impl Default for DurableMemoryDocument {
    fn default() -> Self {
        Self {
            version: 1,
            memories: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCandidate {
    pub candidate_id: String,
    pub task_id: String,
    pub title: String,
    pub statement: String,
    pub provisional_reason: String,
    pub proposed_by: String,
    pub scope: MemoryScope,
    pub created_at: DateTime<Utc>,
    pub lifecycle_state: MemoryLifecycleState,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConflictType {
    DirectContradiction,  // Same scope, contradictory rules without supersedes link
    ScopeShadowing,       // Specialized scoped exception shadowing broader rule
    UnresolvedPrecedence, // Divergent rules without clear ordering
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConflict {
    pub memory_a_id: String,
    pub memory_b_id: String,
    pub conflict_type: ConflictType,
    pub explanation: String,
    pub resolution: String,
    pub requires_human_resolution: bool,
}

fn engram_dir(root: &Path) -> PathBuf {
    root.join(".engram")
}

fn memory_yaml_path(root: &Path) -> PathBuf {
    engram_dir(root).join("memory.yaml")
}

fn candidates_cache_dir(root: &Path) -> Result<PathBuf> {
    let dir = cache_dir(root)?.join("candidates");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

/// Load the durable memory document from `.engram/memory.yaml`.
pub fn load_durable_memories(root: &Path) -> Result<DurableMemoryDocument> {
    let path = memory_yaml_path(root);
    if !path.exists() {
        return Ok(DurableMemoryDocument {
            version: 1,
            memories: Vec::new(),
        });
    }

    let contents = fs::read_to_string(&path)?;
    if contents.trim().is_empty() {
        return Ok(DurableMemoryDocument {
            version: 1,
            memories: Vec::new(),
        });
    }

    let doc: DurableMemoryDocument = serde_yaml::from_str(&contents)?;
    Ok(doc)
}

/// Save the durable memory document into `.engram/memory.yaml`.
pub fn save_durable_memories(root: &Path, doc: &DurableMemoryDocument) -> Result<()> {
    let dir = engram_dir(root);
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    let path = memory_yaml_path(root);
    let serialized = serde_yaml::to_string(doc)?;
    let tmp_path = dir.join(format!("memory.yaml.{}.tmp", Uuid::new_v4().simple()));
    fs::write(&tmp_path, serialized.as_bytes())?;

    if path.exists() {
        let bak_path = dir.join("memory.yaml.bak");
        let _ = fs::remove_file(&bak_path);
        if fs::rename(&path, &bak_path).is_ok() {
            if let Err(e) = fs::rename(&tmp_path, &path) {
                let _ = fs::rename(&bak_path, &path);
                let _ = fs::remove_file(&tmp_path);
                return Err(anyhow!(
                    "Failed to atomic rename temporary memory file: {e}"
                ));
            }
            let _ = fs::remove_file(&bak_path);
        } else {
            fs::write(&path, serialized.as_bytes())?;
            let _ = fs::remove_file(&tmp_path);
        }
    } else if let Err(_) = fs::rename(&tmp_path, &path) {
        fs::write(&path, serialized.as_bytes())?;
        let _ = fs::remove_file(&tmp_path);
    }
    Ok(())
}

/// Record a scoped architectural decision directly into durable memory.
pub fn record_scoped_decision(
    root: &Path,
    id: Option<&str>,
    title: &str,
    statement: &str,
    reason: &str,
    scope: MemoryScope,
    source_ref: Option<String>,
    accepted_by: &str,
) -> Result<DurableMemory> {
    let root = root_path(root)?;
    let mut doc = load_durable_memories(&root)?;

    let mem_id = id
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("mem_{}", Uuid::new_v4().simple()));

    let current_sha = if crate::git::is_git_repo(&root) {
        crate::git::current_revision(&root)
            .ok()
            .filter(|s| s != "WORKTREE" && !s.is_empty())
    } else {
        None
    };

    let provenance = MemoryProvenance {
        source_type: "human_decision".to_string(),
        source_id: id.map(|s| s.to_string()),
        source_commit: current_sha.clone(),
        source_contract: None,
        source_decision: id.map(|s| s.to_string()),
        ratified_by: Some(accepted_by.to_string()),
        ratified_at: Some(Utc::now()),
    };

    let memory = DurableMemory {
        id: mem_id,
        title: title.to_string(),
        statement: statement.to_string(),
        reason: reason.to_string(),
        source: Some(MemorySource {
            source_type: "human_decision".to_string(),
            reference: source_ref,
        }),
        provenance: Some(provenance),
        accepted_by: accepted_by.to_string(),
        accepted_at: Utc::now(),
        scope,
        evidence_tier: EvidenceTier::Explicit,
        lifecycle_state: MemoryLifecycleState::Durable,
        supersedes: None,
        superseded_by: None,
        tags: Vec::new(),
        ratified_at_commit: current_sha,
        ratified_by: Some(accepted_by.to_string()),
        ratified_at: Some(Utc::now()),
    };

    doc.memories.push(memory.clone());
    save_durable_memories(&root, &doc)?;
    Ok(memory)
}

/// Record an explicit human / architectural decision directly into durable memory (legacy helper).
pub fn record_decision(
    root: &Path,
    title: &str,
    statement: &str,
    reason: &str,
    modules: Vec<String>,
    files: Vec<String>,
    source_ref: Option<String>,
    accepted_by: &str,
) -> Result<DurableMemory> {
    let target = modules.first().cloned();
    let level = if !modules.is_empty() {
        ScopeLevel::Module
    } else if !files.is_empty() {
        ScopeLevel::Package
    } else {
        ScopeLevel::Global
    };
    let scope = MemoryScope {
        level,
        target,
        team: None,
        modules,
        files,
        symbols: Vec::new(),
    };
    record_scoped_decision(
        root,
        None,
        title,
        statement,
        reason,
        scope,
        source_ref,
        accepted_by,
    )
}

/// Record a provisional architectural decision proposed via MCP or automated agents.
/// Strictly marked as PROVISIONAL and requires explicit human CLI ratification to become DURABLE.
pub fn record_provisional_decision(
    root: &Path,
    title: &str,
    statement: &str,
    reason: &str,
    modules: Vec<String>,
    files: Vec<String>,
    source_ref: Option<String>,
    proposed_by: &str,
) -> Result<DurableMemory> {
    let target = modules.first().cloned();
    let level = if !modules.is_empty() {
        ScopeLevel::Module
    } else if !files.is_empty() {
        ScopeLevel::Package
    } else {
        ScopeLevel::Global
    };
    let scope = MemoryScope {
        level,
        target,
        team: None,
        modules,
        files,
        symbols: Vec::new(),
    };
    let root = root_path(root)?;
    let mut doc = load_durable_memories(&root)?;

    let mem_id = format!("mem_{}", Uuid::new_v4().simple());

    let current_sha = if crate::git::is_git_repo(&root) {
        crate::git::current_revision(&root)
            .ok()
            .filter(|s| s != "WORKTREE" && !s.is_empty())
    } else {
        None
    };

    let provenance = MemoryProvenance {
        source_type: "agent_mcp".to_string(),
        source_id: Some(mem_id.clone()),
        source_commit: current_sha.clone(),
        source_contract: None,
        source_decision: Some(mem_id.clone()),
        ratified_by: None,
        ratified_at: None,
    };

    let memory = DurableMemory {
        id: mem_id,
        title: title.to_string(),
        statement: statement.to_string(),
        reason: reason.to_string(),
        source: Some(MemorySource {
            source_type: "agent_mcp".to_string(),
            reference: source_ref,
        }),
        provenance: Some(provenance),
        accepted_by: proposed_by.to_string(),
        accepted_at: Utc::now(),
        scope,
        evidence_tier: EvidenceTier::Inferred,
        lifecycle_state: MemoryLifecycleState::Provisional,
        supersedes: None,
        superseded_by: None,
        tags: vec!["provisional".to_string(), "mcp".to_string()],
        ratified_at_commit: None,
        ratified_by: None,
        ratified_at: None,
    };

    doc.memories.push(memory.clone());
    save_durable_memories(&root, &doc)?;
    Ok(memory)
}

/// Supersede an existing durable memory with a new architectural policy.
/// The superseded memory record remains preserved (immutable statement) with lifecycle state SUPERSEDED.
pub fn supersede_memory(
    root: &Path,
    old_id: &str,
    new_title: &str,
    new_statement: &str,
    new_reason: &str,
    new_scope: MemoryScope,
    accepted_by: &str,
) -> Result<DurableMemory> {
    let root = root_path(root)?;
    let mut doc = load_durable_memories(&root)?;

    let old_idx = doc
        .memories
        .iter()
        .position(|m| m.id == old_id)
        .ok_or_else(|| anyhow!("Memory to supersede '{}' not found", old_id))?;

    let new_id = format!("mem_{}", Uuid::new_v4().simple());

    // Mark previous record as Superseded (statement remains immutable!)
    doc.memories[old_idx].lifecycle_state = MemoryLifecycleState::Superseded;
    doc.memories[old_idx].superseded_by = Some(new_id.clone());

    let provenance = MemoryProvenance {
        source_type: "supersession".to_string(),
        source_id: Some(new_id.clone()),
        source_commit: None,
        source_contract: None,
        source_decision: Some(new_id.clone()),
        ratified_by: Some(accepted_by.to_string()),
        ratified_at: Some(Utc::now()),
    };

    let current_sha = if crate::git::is_git_repo(&root) {
        crate::git::current_revision(&root)
            .ok()
            .filter(|s| s != "WORKTREE" && !s.is_empty())
    } else {
        None
    };

    let new_memory = DurableMemory {
        id: new_id,
        title: new_title.to_string(),
        statement: new_statement.to_string(),
        reason: new_reason.to_string(),
        source: Some(MemorySource {
            source_type: "supersession".to_string(),
            reference: Some(format!("supersedes:{}", old_id)),
        }),
        provenance: Some(provenance),
        accepted_by: accepted_by.to_string(),
        accepted_at: Utc::now(),
        scope: new_scope,
        evidence_tier: EvidenceTier::Explicit,
        lifecycle_state: MemoryLifecycleState::Durable,
        supersedes: Some(old_id.to_string()),
        superseded_by: None,
        tags: Vec::new(),
        ratified_at_commit: current_sha,
        ratified_by: Some(accepted_by.to_string()),
        ratified_at: Some(Utc::now()),
    };

    doc.memories.push(new_memory.clone());
    save_durable_memories(&root, &doc)?;
    Ok(new_memory)
}

/// Record a candidate memory derived from an agent task (isolated from durable memory).
pub fn record_task_candidate(
    root: &Path,
    task_id: &str,
    title: &str,
    statement: &str,
    reason: &str,
    modules: Vec<String>,
    files: Vec<String>,
    proposed_by: &str,
) -> Result<MemoryCandidate> {
    let root = root_path(root)?;
    let candidate_id = format!("cand_{}", Uuid::new_v4().simple());
    let target = modules.first().cloned();
    let level = if !modules.is_empty() {
        ScopeLevel::Module
    } else if !files.is_empty() {
        ScopeLevel::Package
    } else {
        ScopeLevel::Global
    };

    let candidate = MemoryCandidate {
        candidate_id: candidate_id.clone(),
        task_id: task_id.to_string(),
        title: title.to_string(),
        statement: statement.to_string(),
        provisional_reason: reason.to_string(),
        proposed_by: proposed_by.to_string(),
        scope: MemoryScope {
            level,
            target,
            team: None,
            modules,
            files,
            symbols: Vec::new(),
        },
        created_at: Utc::now(),
        lifecycle_state: MemoryLifecycleState::Candidate,
    };

    let dir = candidates_cache_dir(&root)?;
    let file_path = dir.join(format!("{candidate_id}.json"));
    let json = serde_json::to_string_pretty(&candidate)?;
    fs::write(&file_path, json)?;

    let _ = crate::telemetry::record_event(
        &root,
        "CANDIDATE_CREATION",
        None,
        proposed_by,
        serde_json::json!({
            "candidate_id": &candidate.candidate_id,
            "task_id": task_id,
            "title": title,
            "modules": &candidate.scope.modules,
        }),
    );

    Ok(candidate)
}

/// List all pending memory candidates awaiting human review.
pub fn list_candidates(root: &Path) -> Result<Vec<MemoryCandidate>> {
    let root = root_path(root)?;
    let dir = candidates_cache_dir(&root)?;
    let mut candidates = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(cand) = serde_json::from_str::<MemoryCandidate>(&content) {
                        if cand.lifecycle_state == MemoryLifecycleState::Candidate {
                            candidates.push(cand);
                        }
                    }
                }
            }
        }
    }

    Ok(candidates)
}

/// Promote a candidate to durable memory after human review (Memory Poisoning Protection Gate).
pub fn promote_candidate(
    root: &Path,
    candidate_id: &str,
    accepted_by: &str,
    reason_override: Option<&str>,
) -> Result<DurableMemory> {
    let root = root_path(root)?;
    let dir = candidates_cache_dir(&root)?;
    let file_path = dir.join(format!("{candidate_id}.json"));

    if !file_path.exists() {
        return Err(anyhow!("Memory candidate {candidate_id} not found"));
    }

    let content = fs::read_to_string(&file_path)?;
    let mut candidate: MemoryCandidate = serde_json::from_str(&content)?;

    let reason = reason_override
        .unwrap_or(&candidate.provisional_reason)
        .to_string();

    let mut doc = load_durable_memories(&root)?;
    let durable = DurableMemory {
        id: format!("mem_{}", Uuid::new_v4().simple()),
        title: candidate.title.clone(),
        statement: candidate.statement.clone(),
        reason,
        source: Some(MemorySource {
            source_type: "promoted_task".to_string(),
            reference: Some(format!("task:{}", candidate.task_id)),
        }),
        provenance: Some(MemoryProvenance {
            source_type: "promoted_task".to_string(),
            source_id: Some(format!("task:{}", candidate.task_id)),
            source_commit: None,
            source_contract: None,
            source_decision: None,
            ratified_by: Some(accepted_by.to_string()),
            ratified_at: Some(Utc::now()),
        }),
        accepted_by: accepted_by.to_string(),
        accepted_at: Utc::now(),
        scope: candidate.scope.clone(),
        evidence_tier: EvidenceTier::Verified,
        lifecycle_state: MemoryLifecycleState::Durable,
        supersedes: None,
        superseded_by: None,
        tags: Vec::new(),
        ratified_at_commit: if crate::git::is_git_repo(&root) {
            crate::git::current_revision(&root)
                .ok()
                .filter(|s| s != "WORKTREE" && !s.is_empty())
        } else {
            None
        },
        ratified_by: Some(accepted_by.to_string()),
        ratified_at: Some(Utc::now()),
    };

    doc.memories.push(durable.clone());
    save_durable_memories(&root, &doc)?;

    // Update candidate status to Accepted
    candidate.lifecycle_state = MemoryLifecycleState::Accepted;
    fs::write(&file_path, serde_json::to_string_pretty(&candidate)?)?;

    let _ = crate::telemetry::record_event(
        &root,
        "CANDIDATE_PROMOTION",
        None,
        accepted_by,
        serde_json::json!({
            "candidate_id": candidate_id,
            "durable_memory_id": &durable.id,
            "accepted_by": accepted_by,
        }),
    );

    Ok(durable)
}

/// Reject a candidate, permanently preventing promotion.
pub fn reject_candidate(
    root: &Path,
    candidate_id: &str,
    rejected_by: &str,
    reason: &str,
) -> Result<()> {
    let root = root_path(root)?;
    let dir = candidates_cache_dir(&root)?;
    let file_path = dir.join(format!("{candidate_id}.json"));

    if !file_path.exists() {
        return Err(anyhow!("Memory candidate {candidate_id} not found"));
    }

    let content = fs::read_to_string(&file_path)?;
    let mut candidate: MemoryCandidate = serde_json::from_str(&content)?;
    candidate.lifecycle_state = MemoryLifecycleState::Rejected;

    // Save updated status with rejection reason
    let json = serde_json::json!({
        "candidate": candidate,
        "rejected_by": rejected_by,
        "rejection_reason": reason,
        "rejected_at": Utc::now()
    });
    fs::write(&file_path, serde_json::to_string_pretty(&json)?)?;

    Ok(())
}

fn is_stop_word(w: &str) -> bool {
    matches!(
        w,
        "this"
            | "that"
            | "with"
            | "from"
            | "have"
            | "been"
            | "using"
            | "uses"
            | "order"
            | "service"
            | "services"
            | "instead"
            | "avoid"
            | "before"
            | "after"
            | "when"
            | "what"
            | "where"
    )
}

fn detect_statement_conflict(a: &DurableMemory, b: &DurableMemory) -> Option<bool> {
    let s_a = a.statement.to_ascii_lowercase();
    let s_b = b.statement.to_ascii_lowercase();

    // Check shared known architectural topics/terms
    let keywords = [
        "repository pattern",
        "connection pool",
        "token validation",
        "csv export",
        "raw sql",
        "direct connection",
        "streaming",
        "buffering",
        "security token",
        "database access",
    ];

    let mut topic_overlap = false;
    for kw in &keywords {
        if s_a.contains(kw) && s_b.contains(kw) {
            topic_overlap = true;
            break;
        }
    }

    if !topic_overlap {
        if let (Some(ta), Some(tb)) = (&a.scope.target, &b.scope.target) {
            if ta.eq_ignore_ascii_case(tb) {
                topic_overlap = true;
            }
        }
    }

    if !topic_overlap {
        let words_a: HashSet<_> = s_a
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| w.len() > 3 && !is_stop_word(w))
            .collect();
        let words_b: HashSet<_> = s_b
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| w.len() > 3 && !is_stop_word(w))
            .collect();
        let common = words_a.intersection(&words_b).count();
        if common >= 2 {
            topic_overlap = true;
        }
    }

    if !topic_overlap {
        return None;
    }

    // Direct opposition/negation checks
    let has_negation_a = s_a.contains("must not")
        || s_a.contains("forbidden")
        || s_a.contains("prohibit")
        || s_a.contains("never")
        || s_a.contains("bypass")
        || s_a.contains("disable");
    let has_negation_b = s_b.contains("must not")
        || s_b.contains("forbidden")
        || s_b.contains("prohibit")
        || s_b.contains("never")
        || s_b.contains("bypass")
        || s_b.contains("disable");

    let has_affirm_a = s_a.contains("must ")
        || s_a.contains("always")
        || s_a.contains("require")
        || s_a.contains("enforce")
        || s_a.contains("enable");
    let has_affirm_b = s_b.contains("must ")
        || s_b.contains("always")
        || s_b.contains("require")
        || s_b.contains("enforce")
        || s_b.contains("enable");

    if (has_negation_a && has_affirm_b) || (has_negation_b && has_affirm_a) {
        return Some(true);
    }

    if (s_a.contains("must use") && s_b.contains("must not use"))
        || (s_b.contains("must use") && s_a.contains("must not use"))
    {
        return Some(true);
    }

    if s_a.contains("bypass") || s_b.contains("bypass") {
        return Some(false);
    }

    None
}

/// Detect conflicts, contradictions, and scope shadowing across active durable memories.
pub fn detect_memory_conflicts(memories: &[DurableMemory]) -> Vec<MemoryConflict> {
    let mut conflicts = Vec::new();

    // Only compare active memories that have not already resolved via supersession
    let active: Vec<_> = memories
        .iter()
        .filter(|m| m.lifecycle_state == MemoryLifecycleState::Durable)
        .collect();

    for i in 0..active.len() {
        for j in (i + 1)..active.len() {
            let a = active[i];
            let b = active[j];

            // If explicitly linked by supersedes, not in conflict
            if a.supersedes.as_deref() == Some(&b.id)
                || b.supersedes.as_deref() == Some(&a.id)
                || a.superseded_by.as_deref() == Some(&b.id)
                || b.superseded_by.as_deref() == Some(&a.id)
            {
                continue;
            }

            if detect_statement_conflict(a, b).is_some() {
                if a.scope.level == b.scope.level && a.scope.target == b.scope.target {
                    // Exact scope match contradiction -> DirectContradiction (Requires human resolution)
                    conflicts.push(MemoryConflict {
                        memory_a_id: a.id.clone(),
                        memory_b_id: b.id.clone(),
                        conflict_type: ConflictType::DirectContradiction,
                        explanation: format!(
                            "Direct contradiction between '{}' ({}) and '{}' ({}) at identical scope {:?} ({:?}) without supersedes link",
                            a.title, a.id, b.title, b.id, a.scope.level, a.scope.target
                        ),
                        resolution: "Requires developer ratification or explicit supersession link before compile".to_string(),
                        requires_human_resolution: true,
                    });
                } else if a.scope.level != b.scope.level {
                    // Hierarchical scope -> ScopeShadowing (Handled automatically)
                    let (broader, specific) = if a.scope.level < b.scope.level {
                        (a, b)
                    } else {
                        (b, a)
                    };

                    conflicts.push(MemoryConflict {
                        memory_a_id: broader.id.clone(),
                        memory_b_id: specific.id.clone(),
                        conflict_type: ConflictType::ScopeShadowing,
                        explanation: format!(
                            "Specialized rule '{}' ({:?}) shadows broader rule '{}' ({:?}) for target '{:?}'",
                            specific.title, specific.scope.level, broader.title, broader.scope.level, specific.scope.target
                        ),
                        resolution: format!(
                            "Specialized rule {} shadows broader rule {} within target scope; broader rule remains active elsewhere.",
                            specific.id, broader.id
                        ),
                        requires_human_resolution: false,
                    });
                }
            }
        }
    }

    conflicts
}

/// Query relevant durable memories for a task, target files, and target symbols.
/// Handles supersession exclusion and hierarchical scope shadowing resolution.
pub fn query_scoped_memories(
    root: &Path,
    task: &str,
    files: &[String],
    symbols: &[String],
    include_superseded: bool,
) -> Result<Vec<DurableMemory>> {
    let root = root_path(root)?;
    let doc = load_durable_memories(&root)?;
    let task_lower = task.to_ascii_lowercase();

    let mut candidates = Vec::new();

    for mem in doc.memories {
        if !include_superseded && mem.lifecycle_state == MemoryLifecycleState::Superseded {
            continue;
        }
        if mem.lifecycle_state != MemoryLifecycleState::Durable
            && mem.lifecycle_state != MemoryLifecycleState::Superseded
            && mem.lifecycle_state != MemoryLifecycleState::StaleReview
        {
            continue;
        }

        let mut matches = false;

        match mem.scope.level {
            ScopeLevel::Global => {
                matches = true;
            }
            ScopeLevel::Repository => {
                if let Some(target) = &mem.scope.target {
                    let target_l = target.to_ascii_lowercase();
                    if task_lower.contains(&target_l)
                        || files
                            .iter()
                            .any(|f| f.to_ascii_lowercase().contains(&target_l))
                    {
                        matches = true;
                    }
                } else {
                    matches = true;
                }
            }
            ScopeLevel::Module => {
                let mut mod_names = mem.scope.modules.clone();
                if let Some(t) = &mem.scope.target {
                    if !mod_names.contains(t) {
                        mod_names.push(t.clone());
                    }
                }
                for m in &mod_names {
                    let m_l = m.to_ascii_lowercase();
                    if task_lower.contains(&m_l)
                        || files.iter().any(|f| {
                            let fl = f.to_ascii_lowercase();
                            fl.contains(&format!("/{m_l}/"))
                                || fl.contains(&format!("\\{m_l}\\"))
                                || fl.starts_with(&format!("{m_l}/"))
                                || fl.contains(&m_l)
                        })
                        || symbols
                            .iter()
                            .any(|s| s.to_ascii_lowercase().contains(&m_l))
                    {
                        matches = true;
                        break;
                    }
                }
            }
            ScopeLevel::Package => {
                for p in &mem.scope.files {
                    let pl = p.to_ascii_lowercase();
                    if files.iter().any(|f| f.to_ascii_lowercase().contains(&pl))
                        || task_lower.contains(&pl)
                    {
                        matches = true;
                        break;
                    }
                }
                if let Some(t) = &mem.scope.target {
                    let tl = t.to_ascii_lowercase();
                    if files.iter().any(|f| f.to_ascii_lowercase().contains(&tl))
                        || task_lower.contains(&tl)
                    {
                        matches = true;
                    }
                }
            }
            ScopeLevel::Symbol => {
                for s in &mem.scope.symbols {
                    let sl = s.to_ascii_lowercase();
                    if symbols
                        .iter()
                        .any(|sym| sym.to_ascii_lowercase().contains(&sl))
                        || task_lower.contains(&sl)
                        || files.iter().any(|f| f.to_ascii_lowercase().contains(&sl))
                    {
                        matches = true;
                        break;
                    }
                }
                if let Some(t) = &mem.scope.target {
                    let tl = t.to_ascii_lowercase();
                    if symbols
                        .iter()
                        .any(|sym| sym.to_ascii_lowercase().contains(&tl))
                        || task_lower.contains(&tl)
                        || files.iter().any(|f| f.to_ascii_lowercase().contains(&tl))
                    {
                        matches = true;
                    }
                }
            }
        }

        // Textual fallback match on title / statement
        if !matches {
            if task_lower.contains(&mem.title.to_ascii_lowercase())
                || mem.statement.to_ascii_lowercase().contains(&task_lower)
            {
                matches = true;
            }
        }

        if matches {
            candidates.push(mem);
        }
    }

    // Apply Scope Shadowing resolution
    let conflicts = detect_memory_conflicts(&candidates);
    let mut shadowed_ids = HashSet::new();

    for conflict in conflicts {
        if conflict.conflict_type == ConflictType::ScopeShadowing {
            let mem_a = candidates.iter().find(|m| m.id == conflict.memory_a_id);
            let mem_b = candidates.iter().find(|m| m.id == conflict.memory_b_id);
            if let (Some(a), Some(b)) = (mem_a, mem_b) {
                if a.scope.level < b.scope.level {
                    shadowed_ids.insert(a.id.clone());
                } else if b.scope.level < a.scope.level {
                    shadowed_ids.insert(b.id.clone());
                }
            }
        }
    }

    candidates.retain(|m| !shadowed_ids.contains(&m.id));

    // Sort order: Symbol > Package > Module > Repository > Global, then most recent first
    candidates.sort_by(|a, b| {
        b.scope
            .level
            .cmp(&a.scope.level)
            .then_with(|| b.accepted_at.cmp(&a.accepted_at))
    });

    Ok(candidates)
}

/// Query relevant durable memories for a task or set of files (legacy helper).
pub fn query_relevant_memories(
    root: &Path,
    task: &str,
    files: &[String],
) -> Result<Vec<DurableMemory>> {
    query_scoped_memories(root, task, files, &[], false)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    NotFound(String),
    InvalidTransition {
        id: String,
        from: MemoryLifecycleState,
        to: MemoryLifecycleState,
        reason: String,
    },
    MissingReviewer,
    MissingReason,
    ConsistencyError {
        id: String,
        action: String,
        state_persisted: bool,
        telemetry_recorded: bool,
        recovery_hint: String,
        details: String,
    },
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "Decision '{id}' not found"),
            Self::InvalidTransition {
                id,
                from,
                to,
                reason,
            } => {
                write!(
                    f,
                    "Invalid lifecycle transition for decision '{id}': cannot transition from {from:?} to {to:?} ({reason})"
                )
            }
            Self::MissingReviewer => {
                write!(
                    f,
                    "Ratification rejected: reviewer ('by') must be non-empty"
                )
            }
            Self::MissingReason => {
                write!(
                    f,
                    "Ratification rejected: ratification reason must be non-empty"
                )
            }
            Self::ConsistencyError {
                id,
                action,
                state_persisted,
                telemetry_recorded,
                recovery_hint,
                details,
            } => {
                write!(
                    f,
                    "Governance consistency error for decision '{id}' during '{action}': state_persisted={state_persisted}, telemetry_recorded={telemetry_recorded}. Recovery procedure: {recovery_hint}. Details: {details}"
                )
            }
        }
    }
}

impl std::error::Error for LifecycleError {}

/// Explicit human ratification of an architectural decision (provisional or existing).
/// Transitions PROVISIONAL or STALE_REVIEW decisions to DURABLE with explicit human provenance.
pub fn ratify_decision(
    root: &Path,
    decision_id: &str,
    by: &str,
    reason: &str,
    commit: Option<&str>,
) -> Result<DurableMemory> {
    if by.trim().is_empty() {
        return Err(LifecycleError::MissingReviewer.into());
    }
    if reason.trim().is_empty() {
        return Err(LifecycleError::MissingReason.into());
    }

    let root = root_path(root)?;
    let mut doc = load_durable_memories(&root)?;

    let idx = doc
        .memories
        .iter()
        .position(|m| m.id == decision_id)
        .ok_or_else(|| LifecycleError::NotFound(decision_id.to_string()))?;

    let original_doc = doc.clone();
    let prev_state = doc.memories[idx].lifecycle_state;
    let mem = &mut doc.memories[idx];

    match mem.lifecycle_state {
        MemoryLifecycleState::Superseded => {
            return Err(LifecycleError::InvalidTransition {
                id: decision_id.to_string(),
                from: MemoryLifecycleState::Superseded,
                to: MemoryLifecycleState::Durable,
                reason: "Superseded decisions cannot be ratified".to_string(),
            }
            .into());
        }
        MemoryLifecycleState::Rejected => {
            return Err(LifecycleError::InvalidTransition {
                id: decision_id.to_string(),
                from: MemoryLifecycleState::Rejected,
                to: MemoryLifecycleState::Durable,
                reason: "Rejected decisions cannot be ratified".to_string(),
            }
            .into());
        }
        _ => {}
    }

    let target_commit = match commit {
        Some(c) if !c.trim().is_empty() => {
            let c_trimmed = c.trim();
            if crate::git::is_git_repo(&root) {
                match crate::git::resolve_baseline(&root, c_trimmed)? {
                    Ok(full_sha) => full_sha,
                    Err(err) => return Err(anyhow!("Cannot ratify: {err}")),
                }
            } else {
                c_trimmed.to_string()
            }
        }
        _ => {
            if crate::git::is_git_repo(&root) {
                crate::git::current_revision(&root).unwrap_or_else(|_| "HEAD".to_string())
            } else {
                "HEAD".to_string()
            }
        }
    };

    mem.lifecycle_state = MemoryLifecycleState::Durable;
    mem.ratified_at_commit = Some(target_commit.clone());
    mem.ratified_by = Some(by.trim().to_string());
    mem.ratified_at = Some(Utc::now());

    if let Some(ref mut prov) = mem.provenance {
        prov.source_commit = Some(target_commit.clone());
        prov.ratified_by = Some(by.trim().to_string());
        prov.ratified_at = Some(Utc::now());
        prov.source_type = "human_ratification".to_string();
    } else {
        mem.provenance = Some(MemoryProvenance {
            source_type: "human_ratification".to_string(),
            source_id: Some(decision_id.to_string()),
            source_commit: Some(target_commit.clone()),
            source_contract: None,
            source_decision: Some(decision_id.to_string()),
            ratified_by: Some(by.trim().to_string()),
            ratified_at: Some(Utc::now()),
        });
    }

    let updated = mem.clone();
    save_durable_memories(&root, &doc)?;

    let telemetry_res = crate::telemetry::record_event(
        &root,
        "DECISION_RATIFIED",
        None,
        by.trim(),
        serde_json::json!({
            "decision_id": decision_id,
            "previous_state": format!("{:?}", prev_state),
            "new_state": "DURABLE",
            "ratified_at_commit": target_commit,
            "ratified_by": by.trim(),
            "reason": reason.trim(),
        }),
    );

    if let Err(telemetry_err) = telemetry_res {
        match save_durable_memories(&root, &original_doc) {
            Ok(_) => {
                return Err(LifecycleError::ConsistencyError {
                    id: decision_id.to_string(),
                    action: "ratify_decision".to_string(),
                    state_persisted: false,
                    telemetry_recorded: false,
                    recovery_hint: format!(
                        "Telemetry write failed ({telemetry_err}). State was cleanly rolled back to {prev_state:?}. Resolve telemetry storage issue and retry."
                    ),
                    details: telemetry_err.to_string(),
                }
                .into());
            }
            Err(rollback_err) => {
                return Err(LifecycleError::ConsistencyError {
                    id: decision_id.to_string(),
                    action: "ratify_decision".to_string(),
                    state_persisted: true,
                    telemetry_recorded: false,
                    recovery_hint: format!(
                        "INDETERMINATE_STATE: State persisted as DURABLE but telemetry failed ({telemetry_err}) and state rollback failed ({rollback_err}). Run 'engram doctor' to reconcile."
                    ),
                    details: format!("telemetry error: {telemetry_err}; rollback error: {rollback_err}"),
                }
                .into());
            }
        }
    }

    Ok(updated)
}

/// Explicit human rejection of an architectural decision proposal or provisional decision.
pub fn reject_decision(
    root: &Path,
    decision_id: &str,
    by: &str,
    reason: &str,
) -> Result<DurableMemory> {
    if by.trim().is_empty() {
        return Err(LifecycleError::MissingReviewer.into());
    }
    if reason.trim().is_empty() {
        return Err(LifecycleError::MissingReason.into());
    }

    let root = root_path(root)?;
    let mut doc = load_durable_memories(&root)?;

    let idx = doc
        .memories
        .iter()
        .position(|m| m.id == decision_id)
        .ok_or_else(|| LifecycleError::NotFound(decision_id.to_string()))?;

    let original_doc = doc.clone();
    let prev_state = doc.memories[idx].lifecycle_state;
    let mem = &mut doc.memories[idx];

    if mem.lifecycle_state == MemoryLifecycleState::Durable {
        return Err(LifecycleError::InvalidTransition {
            id: decision_id.to_string(),
            from: MemoryLifecycleState::Durable,
            to: MemoryLifecycleState::Rejected,
            reason:
                "Durable decisions must be superseded via 'engram memory supersede', not rejected"
                    .to_string(),
        }
        .into());
    }
    if mem.lifecycle_state == MemoryLifecycleState::Superseded {
        return Err(LifecycleError::InvalidTransition {
            id: decision_id.to_string(),
            from: MemoryLifecycleState::Superseded,
            to: MemoryLifecycleState::Rejected,
            reason: "Superseded decisions cannot be rejected".to_string(),
        }
        .into());
    }

    mem.lifecycle_state = MemoryLifecycleState::Rejected;

    let updated = mem.clone();
    save_durable_memories(&root, &doc)?;

    let telemetry_res = crate::telemetry::record_event(
        &root,
        "DECISION_REJECTED",
        None,
        by.trim(),
        serde_json::json!({
            "decision_id": decision_id,
            "previous_state": format!("{:?}", prev_state),
            "new_state": "REJECTED",
            "rejected_by": by.trim(),
            "reason": reason.trim(),
        }),
    );

    if let Err(telemetry_err) = telemetry_res {
        match save_durable_memories(&root, &original_doc) {
            Ok(_) => {
                return Err(LifecycleError::ConsistencyError {
                    id: decision_id.to_string(),
                    action: "reject_decision".to_string(),
                    state_persisted: false,
                    telemetry_recorded: false,
                    recovery_hint: format!(
                        "Telemetry write failed ({telemetry_err}). State was cleanly rolled back to {prev_state:?}. Resolve telemetry storage issue and retry."
                    ),
                    details: telemetry_err.to_string(),
                }
                .into());
            }
            Err(rollback_err) => {
                return Err(LifecycleError::ConsistencyError {
                    id: decision_id.to_string(),
                    action: "reject_decision".to_string(),
                    state_persisted: true,
                    telemetry_recorded: false,
                    recovery_hint: format!(
                        "INDETERMINATE_STATE: State persisted as REJECTED but telemetry failed ({telemetry_err}) and state rollback failed ({rollback_err}). Run 'engram doctor' to reconcile."
                    ),
                    details: format!("telemetry error: {telemetry_err}; rollback error: {rollback_err}"),
                }
                .into());
            }
        }
    }

    Ok(updated)
}

/// Re-ratify an architectural decision with current or specified commit and reviewer name.
/// Resets governed mutation count and clears STALE_REVIEW back to Durable.
pub fn re_ratify_decision(
    root: &Path,
    decision_id: &str,
    commit: Option<&str>,
    by: &str,
) -> Result<DurableMemory> {
    if by.trim().is_empty() {
        return Err(LifecycleError::MissingReviewer.into());
    }

    let root = root_path(root)?;
    let mut doc = load_durable_memories(&root)?;

    let idx = doc
        .memories
        .iter()
        .position(|m| m.id == decision_id)
        .ok_or_else(|| LifecycleError::NotFound(decision_id.to_string()))?;

    let original_doc = doc.clone();
    let prev_state = doc.memories[idx].lifecycle_state;
    let mem = &mut doc.memories[idx];

    // Provisional decisions cannot be re-ratified; they must be explicitly ratified first
    if mem.lifecycle_state == MemoryLifecycleState::Provisional {
        return Err(LifecycleError::InvalidTransition {
            id: decision_id.to_string(),
            from: MemoryLifecycleState::Provisional,
            to: MemoryLifecycleState::Durable,
            reason: "Provisional decisions require explicit human ratification ('engram memory ratify'), not re-ratification".to_string(),
        }
        .into());
    }

    if mem.lifecycle_state == MemoryLifecycleState::Rejected {
        return Err(LifecycleError::InvalidTransition {
            id: decision_id.to_string(),
            from: MemoryLifecycleState::Rejected,
            to: MemoryLifecycleState::Durable,
            reason: "Rejected decisions cannot be re-ratified".to_string(),
        }
        .into());
    }

    if mem.lifecycle_state == MemoryLifecycleState::Superseded {
        return Err(LifecycleError::InvalidTransition {
            id: decision_id.to_string(),
            from: MemoryLifecycleState::Superseded,
            to: MemoryLifecycleState::Durable,
            reason: "Superseded decisions cannot be re-ratified".to_string(),
        }
        .into());
    }

    let target_commit = match commit {
        Some(c) if !c.trim().is_empty() => {
            let c_trimmed = c.trim();
            if crate::git::is_git_repo(&root) {
                match crate::git::resolve_baseline(&root, c_trimmed)? {
                    Ok(full_sha) => full_sha,
                    Err(err) => return Err(anyhow!("Cannot re-ratify: {err}")),
                }
            } else {
                c_trimmed.to_string()
            }
        }
        _ => {
            if crate::git::is_git_repo(&root) {
                crate::git::current_revision(&root).unwrap_or_else(|_| "HEAD".to_string())
            } else {
                "HEAD".to_string()
            }
        }
    };

    mem.ratified_at_commit = Some(target_commit.clone());
    mem.ratified_by = Some(by.to_string());
    mem.ratified_at = Some(Utc::now());

    if let Some(ref mut prov) = mem.provenance {
        prov.source_commit = Some(target_commit.clone());
        prov.ratified_by = Some(by.to_string());
        prov.ratified_at = Some(Utc::now());
    }

    if mem.lifecycle_state == MemoryLifecycleState::StaleReview {
        mem.lifecycle_state = MemoryLifecycleState::Durable;
    }

    let updated = mem.clone();
    save_durable_memories(&root, &doc)?;

    let telemetry_res = crate::telemetry::record_event(
        &root,
        "DECISION_RE_RATIFIED",
        None,
        by,
        serde_json::json!({
            "decision_id": decision_id,
            "ratified_at_commit": target_commit,
            "ratified_by": by,
        }),
    );

    if let Err(telemetry_err) = telemetry_res {
        match save_durable_memories(&root, &original_doc) {
            Ok(_) => {
                return Err(LifecycleError::ConsistencyError {
                    id: decision_id.to_string(),
                    action: "re_ratify_decision".to_string(),
                    state_persisted: false,
                    telemetry_recorded: false,
                    recovery_hint: format!(
                        "Telemetry write failed ({telemetry_err}). State was cleanly rolled back to {prev_state:?}. Resolve telemetry storage issue and retry."
                    ),
                    details: telemetry_err.to_string(),
                }
                .into());
            }
            Err(rollback_err) => {
                return Err(LifecycleError::ConsistencyError {
                    id: decision_id.to_string(),
                    action: "re_ratify_decision".to_string(),
                    state_persisted: true,
                    telemetry_recorded: false,
                    recovery_hint: format!(
                        "INDETERMINATE_STATE: State persisted as DURABLE but telemetry failed ({telemetry_err}) and state rollback failed ({rollback_err}). Run 'engram doctor' to reconcile."
                    ),
                    details: format!("telemetry error: {telemetry_err}; rollback error: {rollback_err}"),
                }
                .into());
            }
        }
    }

    Ok(updated)
}
