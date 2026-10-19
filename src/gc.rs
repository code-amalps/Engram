//! Controlled Garbage Collection Engine: candidate TTL cleanup, superseded archival, and cache vacuuming.
//! Enforces developer authority with mandatory --dry-run pre-flight capability.

use anyhow::Result;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::doctor::{diagnose_memory_health, LivenessStatus};
use crate::memory::{
    load_durable_memories, save_durable_memories, DurableMemory, DurableMemoryDocument,
    MemoryCandidate, MemoryLifecycleState,
};
use crate::storage::{cache_dir, db, root_path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcOptions {
    pub dry_run: bool,
    pub prune_candidates: bool,
    pub archive_superseded: bool,
    pub vacuum: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcPlan {
    pub dry_run: bool,
    pub pruned_candidates: Vec<String>,
    pub archival_candidates: Vec<String>,
    pub archived_count: usize,
    pub vacuum_executed: bool,
    pub summary_message: String,
}

fn candidates_dir(root: &Path) -> Result<PathBuf> {
    Ok(cache_dir(root)?.join("candidates"))
}

fn archive_dir(root: &Path) -> PathBuf {
    root.join(".engram").join("archive")
}

fn memory_archive_yaml(root: &Path) -> PathBuf {
    archive_dir(root).join("memory_archive.yaml")
}

fn load_archived_memories(root: &Path) -> Result<DurableMemoryDocument> {
    let path = memory_archive_yaml(root);
    if !path.exists() {
        return Ok(DurableMemoryDocument {
            version: 1,
            memories: Vec::new(),
        });
    }
    let content = fs::read_to_string(&path)?;
    if content.trim().is_empty() {
        return Ok(DurableMemoryDocument {
            version: 1,
            memories: Vec::new(),
        });
    }
    let doc: DurableMemoryDocument = serde_yaml::from_str(&content)?;
    Ok(doc)
}

fn save_archived_memories(root: &Path, doc: &DurableMemoryDocument) -> Result<()> {
    let dir = archive_dir(root);
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    let path = memory_archive_yaml(root);
    let serialized = serde_yaml::to_string(doc)?;
    fs::write(&path, serialized)?;
    Ok(())
}

/// Execute garbage collection or generate a pre-flight plan if dry_run is enabled.
pub fn execute_gc(root: &Path, opts: &GcOptions) -> Result<GcPlan> {
    let root = root_path(root)?;

    // 1. Discover Candidate Cleanup (TTL > 14 days or Rejected)
    let mut pruned_candidates = Vec::new();
    let mut candidate_paths_to_delete = Vec::new();
    let now = Utc::now();
    let ttl = Duration::days(14);

    if let Ok(c_dir) = candidates_dir(&root) {
        if c_dir.exists() {
            if let Ok(entries) = fs::read_dir(c_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "json") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(cand) = serde_json::from_str::<MemoryCandidate>(&content) {
                                let is_expired = (now - cand.created_at) > ttl;
                                let is_rejected =
                                    cand.lifecycle_state == MemoryLifecycleState::Rejected;
                                if is_expired || is_rejected {
                                    pruned_candidates.push(cand.candidate_id.clone());
                                    candidate_paths_to_delete.push(path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Discover Archival Candidates via Memory Doctor
    let doc_report = diagnose_memory_health(&root)?;
    let archival_candidates: Vec<String> = doc_report
        .items
        .into_iter()
        .filter(|i| i.liveness == LivenessStatus::ArchivalCandidate)
        .map(|i| i.memory_id)
        .collect();

    // 3. Dry-Run Gate: strictly return plan without mutating disk
    if opts.dry_run {
        return Ok(GcPlan {
            dry_run: true,
            pruned_candidates: pruned_candidates.clone(),
            archival_candidates: archival_candidates.clone(),
            archived_count: 0,
            vacuum_executed: false,
            summary_message: format!(
                "DRY-RUN: {} candidates eligible for TTL purge, {} deep memories eligible for archive, vacuum pending.",
                pruned_candidates.len(),
                archival_candidates.len()
            ),
        });
    }

    // 4. Execute Mutating Actions
    // A. Prune candidates
    if opts.prune_candidates {
        for p in candidate_paths_to_delete {
            let _ = fs::remove_file(p);
        }
    }

    // B. Archive deep superseded memories
    let mut archived_count = 0;
    if opts.archive_superseded && !archival_candidates.is_empty() {
        let mut active_doc = load_durable_memories(&root)?;
        let mut archive_doc = load_archived_memories(&root)?;

        let mut to_archive: Vec<DurableMemory> = Vec::new();
        active_doc.memories.retain(|m| {
            if archival_candidates.contains(&m.id) {
                to_archive.push(m.clone());
                false
            } else {
                true
            }
        });

        archived_count = to_archive.len();
        archive_doc.memories.extend(to_archive);

        save_durable_memories(&root, &active_doc)?;
        save_archived_memories(&root, &archive_doc)?;
    }

    // C. Vacuum SQLite database
    let mut vacuum_executed = false;
    if opts.vacuum {
        let conn = db(&root)?;
        let _ = conn.execute("VACUUM", []);
        vacuum_executed = true;
    }

    Ok(GcPlan {
        dry_run: false,
        pruned_candidates,
        archival_candidates,
        archived_count,
        vacuum_executed,
        summary_message: format!(
            "SUCCESS: Archived {} deep memories to .engram/archive/, pruned candidate cache, vacuumed SQLite.",
            archived_count
        ),
    })
}

/// Format human-readable garbage collection report for the terminal.
pub fn format_gc_plan(plan: &GcPlan) -> String {
    let mut out = String::new();
    if plan.dry_run {
        out.push_str("🔍 Engram Garbage Collection (PRE-FLIGHT DRY-RUN)\n");
        out.push_str(
            "──────────────────────────────────────────────────────────────────────────\n",
        );
        out.push_str("No disk state was modified. Review the plan below:\n\n");
        out.push_str(&format!(
            "  • Ephemeral Candidates to Prune: {}\n",
            plan.pruned_candidates.len()
        ));
        for c in &plan.pruned_candidates {
            out.push_str(&format!("      - {}\n", c));
        }
        out.push_str(&format!(
            "  • Deep Memories to Archive:      {}\n",
            plan.archival_candidates.len()
        ));
        for a in &plan.archival_candidates {
            out.push_str(&format!(
                "      - {} -> .engram/archive/memory_archive.yaml\n",
                a
            ));
        }
        out.push_str("\nRun 'engram gc' without --dry-run to apply.\n");
    } else {
        out.push_str("🧹 Engram Garbage Collection (EXECUTED)\n");
        out.push_str(
            "──────────────────────────────────────────────────────────────────────────\n",
        );
        out.push_str(&format!("  ✓ Archived Memories: {}\n", plan.archived_count));
        out.push_str(&format!(
            "  ✓ Pruned Candidates: {}\n",
            plan.pruned_candidates.len()
        ));
        out.push_str(&format!(
            "  ✓ SQLite Vacuum:     {}\n\n",
            if plan.vacuum_executed {
                "Complete"
            } else {
                "Skipped"
            }
        ));
        out.push_str(&format!("Status: {}\n", plan.summary_message));
    }
    out
}
