//! Durable Engineering Memory: human rationale, promotion lifecycle, poisoning protection, and Git persistence.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::model::EvidenceTier;
use crate::storage::{cache_dir, root_path};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MemoryLifecycleState {
    Provisional,
    Candidate,
    Review,
    Accepted,
    Rejected,
    Durable,
    Superseded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryScope {
    pub modules: Vec<String>,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySource {
    pub source_type: String, // "human_decision", "pr_review", "adr"
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DurableMemory {
    pub id: String,
    pub title: String,
    pub statement: String,
    pub reason: String,
    pub source: MemorySource,
    pub accepted_by: String,
    pub accepted_at: DateTime<Utc>,
    pub scope: MemoryScope,
    pub evidence_tier: EvidenceTier,
    pub lifecycle_state: MemoryLifecycleState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DurableMemoryDocument {
    pub version: u32,
    pub memories: Vec<DurableMemory>,
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
    fs::write(&path, serialized)?;
    Ok(())
}

/// Record an explicit human / architectural decision directly into durable memory.
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
    let root = root_path(root)?;
    let mut doc = load_durable_memories(&root)?;

    let memory = DurableMemory {
        id: format!("mem_{}", Uuid::new_v4().simple()),
        title: title.to_string(),
        statement: statement.to_string(),
        reason: reason.to_string(),
        source: MemorySource {
            source_type: "human_decision".to_string(),
            reference: source_ref,
        },
        accepted_by: accepted_by.to_string(),
        accepted_at: Utc::now(),
        scope: MemoryScope { modules, files },
        evidence_tier: EvidenceTier::Explicit,
        lifecycle_state: MemoryLifecycleState::Durable,
    };

    doc.memories.push(memory.clone());
    save_durable_memories(&root, &doc)?;
    Ok(memory)
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
    let candidate = MemoryCandidate {
        candidate_id: candidate_id.clone(),
        task_id: task_id.to_string(),
        title: title.to_string(),
        statement: statement.to_string(),
        provisional_reason: reason.to_string(),
        proposed_by: proposed_by.to_string(),
        scope: MemoryScope { modules, files },
        created_at: Utc::now(),
        lifecycle_state: MemoryLifecycleState::Candidate,
    };

    let dir = candidates_cache_dir(&root)?;
    let file_path = dir.join(format!("{candidate_id}.json"));
    let json = serde_json::to_string_pretty(&candidate)?;
    fs::write(&file_path, json)?;

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

    let reason = reason_override.unwrap_or(&candidate.provisional_reason).to_string();

    let mut doc = load_durable_memories(&root)?;
    let durable = DurableMemory {
        id: format!("mem_{}", Uuid::new_v4().simple()),
        title: candidate.title.clone(),
        statement: candidate.statement.clone(),
        reason,
        source: MemorySource {
            source_type: "promoted_task".to_string(),
            reference: Some(format!("task:{}", candidate.task_id)),
        },
        accepted_by: accepted_by.to_string(),
        accepted_at: Utc::now(),
        scope: candidate.scope.clone(),
        evidence_tier: EvidenceTier::Verified,
        lifecycle_state: MemoryLifecycleState::Durable,
    };

    doc.memories.push(durable.clone());
    save_durable_memories(&root, &doc)?;

    // Update candidate status to Accepted
    candidate.lifecycle_state = MemoryLifecycleState::Accepted;
    fs::write(&file_path, serde_json::to_string_pretty(&candidate)?)?;

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

/// Query relevant durable memories for a task or set of files.
pub fn query_relevant_memories(
    root: &Path,
    task: &str,
    files: &[String],
) -> Result<Vec<DurableMemory>> {
    let root = root_path(root)?;
    let doc = load_durable_memories(&root)?;
    let task_lower = task.to_ascii_lowercase();

    let mut relevant = Vec::new();
    for mem in doc.memories {
        if mem.lifecycle_state != MemoryLifecycleState::Durable {
            continue;
        }

        let mut matches = false;

        // Check text match in title or statement
        if task_lower.contains(&mem.title.to_ascii_lowercase())
            || mem.statement.to_ascii_lowercase().contains(&task_lower)
        {
            matches = true;
        }

        // Check module scope
        for m in &mem.scope.modules {
            let m_lower = m.to_ascii_lowercase();
            if task_lower.contains(&m_lower)
                || files.iter().any(|f| f.to_ascii_lowercase().contains(&m_lower))
            {
                matches = true;
                break;
            }
        }

        // Check file scope
        for f in &mem.scope.files {
            let f_lower = f.to_ascii_lowercase();
            if files.iter().any(|file| file.to_ascii_lowercase().contains(&f_lower)) {
                matches = true;
                break;
            }
        }

        if matches {
            relevant.push(mem);
        }
    }

    Ok(relevant)
}
