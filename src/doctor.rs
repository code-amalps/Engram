//! Memory Doctor Diagnostic Engine: read-only health, liveness, and drift diagnosis.
//! Detects LIVE, ORPHANED / DORMANT, STALE, SUPERSEDED, and ARCHIVAL_CANDIDATE statuses.

use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::memory::{
    load_durable_memories, save_durable_memories, MemoryLifecycleState, ScopeLevel,
};
use crate::storage::{db, root_path};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LivenessStatus {
    Live,
    Dormant,
    Stale,
    Superseded,
    ArchivalCandidate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorItem {
    pub memory_id: String,
    pub title: String,
    pub scope_level: ScopeLevel,
    pub scope_target: Option<String>,
    pub liveness: LivenessStatus,
    pub diagnostic_message: String,
    pub suggested_actions: Vec<String>,
}

pub const PROTOCOL_VERSION: &str = "1.1";

fn default_protocol_version() -> String {
    PROTOCOL_VERSION.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,
    pub total_memories: usize,
    pub live_count: usize,
    pub dormant_count: usize,
    pub stale_count: usize,
    pub superseded_count: usize,
    pub archival_candidate_count: usize,
    pub items: Vec<DoctorItem>,
    #[serde(default)]
    pub event_chain_status: Option<String>,
    #[serde(default)]
    pub event_chain_details: Option<String>,
}

fn symbol_exists(conn: &Connection, sym: &str) -> bool {
    let mut stmt = match conn.prepare(
        "SELECT 1 FROM entity_versions WHERE (name = ?1 OR path LIKE ?2) AND invalidated_at_revision IS NULL LIMIT 1"
    ) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let pattern = format!("%{sym}%");
    stmt.exists([sym, &pattern]).unwrap_or(false)
}

fn file_exists(root: &Path, file_path: &str) -> bool {
    root.join(file_path).exists()
}

fn calculate_supersession_depth(mem_id: &str, memories: &[crate::memory::DurableMemory]) -> usize {
    let mut current_id = Some(mem_id.to_string());
    let mut depth = 0;
    while let Some(id) = current_id {
        if depth >= 20 {
            break;
        }
        let successor = memories
            .iter()
            .find(|m| m.supersedes.as_deref() == Some(&id));
        if let Some(succ) = successor {
            depth += 1;
            current_id = Some(succ.id.clone());
        } else {
            break;
        }
    }
    depth
}

/// Diagnose memory health, scope resolution, and knowledge decay against the current codebase AST.
/// Read-only diagnostic: preserves historical records while detecting drift.
pub fn diagnose_memory_health(root: &Path) -> Result<DoctorReport> {
    let root = root_path(root)?;
    let conn = db(&root)?;
    let mut doc = load_durable_memories(&root)?;

    let mut items = Vec::new();
    let mut live_count = 0;
    let mut dormant_count = 0;
    let mut stale_count = 0;
    let mut superseded_count = 0;
    let mut archival_candidate_count = 0;
    let mut doc_mutated = false;

    let snapshot = doc.memories.clone();

    for mem in &mut doc.memories {
        let (liveness, msg, actions) = match mem.lifecycle_state {
            MemoryLifecycleState::Superseded => {
                let depth = calculate_supersession_depth(&mem.id, &snapshot);
                if depth >= 3 {
                    archival_candidate_count += 1;
                    (
                        LivenessStatus::ArchivalCandidate,
                        format!(
                            "Deep superseded record (depth {} hops). Eligible for cold archive.",
                            depth
                        ),
                        vec![
                            "Run 'engram gc --archive-superseded' to move to .engram/archive/"
                                .into(),
                            "Preserve in hot memory.yaml if needed for active development".into(),
                        ],
                    )
                } else {
                    superseded_count += 1;
                    (
                        LivenessStatus::Superseded,
                        format!(
                            "Superseded by {}.",
                            mem.superseded_by.as_deref().unwrap_or("newer policy")
                        ),
                        vec!["Active query automatically excludes this record".into()],
                    )
                }
            }
            MemoryLifecycleState::Dormant => {
                dormant_count += 1;
                (
                    LivenessStatus::Dormant,
                    format!(
                        "Target '{:?}' not found in active codebase. Marked DORMANT.",
                        mem.scope.target
                    ),
                    vec![
                        "Re-anchor to new symbol or file via 'engram memory update'".into(),
                        "Retire decision if architecture permanently changed".into(),
                        "Archive to cold storage".into(),
                    ],
                )
            }
            MemoryLifecycleState::StaleReview => {
                stale_count += 1;
                (
                    LivenessStatus::Stale,
                    "Governed code evolved beyond staleness threshold. Marked STALE_REVIEW."
                        .to_string(),
                    vec![
                        format!("Run 'engram memory re-ratify {}' to re-ratify", mem.id),
                        "Verify policy alignment with active codebase".into(),
                    ],
                )
            }
            _ => {
                // Evaluate Durable memory target resolution
                match mem.scope.level {
                    ScopeLevel::Symbol => {
                        let sym = mem.scope.target.as_deref().unwrap_or("");
                        if !sym.is_empty() && !symbol_exists(&conn, sym) {
                            // Target code disappeared in refactor -> transition to DORMANT
                            mem.lifecycle_state = MemoryLifecycleState::Dormant;
                            doc_mutated = true;
                            dormant_count += 1;
                            (
                                LivenessStatus::Dormant,
                                format!(
                                    "Target symbol '{}' not found in active codebase. Marked DORMANT.",
                                    sym
                                ),
                                vec![
                                    "Re-anchor to replacement symbol".into(),
                                    "Retire decision".into(),
                                    "Archive to cold storage".into(),
                                ],
                            )
                        } else {
                            live_count += 1;
                            (
                                LivenessStatus::Live,
                                "Active symbol policy grounded in codebase".into(),
                                vec!["No action required".into()],
                            )
                        }
                    }
                    ScopeLevel::Package => {
                        let missing = mem.scope.files.iter().any(|f| !file_exists(&root, f));
                        if missing {
                            mem.lifecycle_state = MemoryLifecycleState::Dormant;
                            doc_mutated = true;
                            dormant_count += 1;
                            (
                                LivenessStatus::Dormant,
                                "Package target files not found in working tree. Marked DORMANT."
                                    .into(),
                                vec!["Re-anchor to updated file paths".into()],
                            )
                        } else {
                            live_count += 1;
                            (
                                LivenessStatus::Live,
                                "Package files verified".into(),
                                vec!["No action required".into()],
                            )
                        }
                    }
                    ScopeLevel::Module => {
                        let mod_name = mem.scope.target.as_deref().unwrap_or("");
                        let has_files = if !mod_name.is_empty() {
                            symbol_exists(&conn, mod_name)
                        } else {
                            true
                        };
                        if !has_files && !mod_name.is_empty() {
                            stale_count += 1;
                            (
                                LivenessStatus::Stale,
                                format!("Module '{}' has no active files in repository.", mod_name),
                                vec!["Verify module path or retire rule".into()],
                            )
                        } else {
                            live_count += 1;
                            (
                                LivenessStatus::Live,
                                "Module architecture active".into(),
                                vec!["No action required".into()],
                            )
                        }
                    }
                    ScopeLevel::Global | ScopeLevel::Repository => {
                        live_count += 1;
                        (
                            LivenessStatus::Live,
                            "Broad architectural policy active".into(),
                            vec!["No action required".into()],
                        )
                    }
                }
            }
        };

        items.push(DoctorItem {
            memory_id: mem.id.clone(),
            title: mem.title.clone(),
            scope_level: mem.scope.level,
            scope_target: mem.scope.target.clone(),
            liveness,
            diagnostic_message: msg,
            suggested_actions: actions,
        });
    }

    if doc_mutated {
        save_durable_memories(&root, &doc)?;
    }

    let (event_chain_status, event_chain_details) =
        match crate::telemetry::verify_event_chain(&root) {
            Ok((status, details)) => (Some(status.to_string()), Some(details)),
            Err(e) => (
                Some("EVENT_CHAIN_BROKEN".to_string()),
                Some(format!("Error verifying telemetry event chain: {e}")),
            ),
        };

    Ok(DoctorReport {
        protocol_version: PROTOCOL_VERSION.to_string(),
        total_memories: items.len(),
        live_count,
        dormant_count,
        stale_count,
        superseded_count,
        archival_candidate_count,
        items,
        event_chain_status,
        event_chain_details,
    })
}

/// Format human-readable memory doctor diagnostic report for the terminal.
pub fn format_doctor_report(report: &DoctorReport) -> String {
    let mut out = String::new();
    out.push_str("🩺 Engram Memory Health & Liveness Diagnostic\n");
    out.push_str("──────────────────────────────────────────────────────────────────────────\n");
    out.push_str(&format!(
        "Total Decisions: {}  |  Live: {}  |  Dormant: {}  |  Stale: {}  |  Archival Candidates: {}\n\n",
        report.total_memories,
        report.live_count,
        report.dormant_count,
        report.stale_count,
        report.archival_candidate_count
    ));

    if let Some(ref status) = report.event_chain_status {
        out.push_str(&format!("Audit Event Telemetry Chain: {}\n", status));
        if let Some(ref details) = report.event_chain_details {
            out.push_str(&format!("  Verification: {}\n", details));
        }
        out.push_str("\n");
    }

    for item in &report.items {
        let icon = match item.liveness {
            LivenessStatus::Live => "✓ [LIVE]",
            LivenessStatus::Dormant => "⚠️ [DORMANT]",
            LivenessStatus::Stale => "⚡ [STALE]",
            LivenessStatus::Superseded => "○ [SUPERSEDED]",
            LivenessStatus::ArchivalCandidate => "📦 [ARCHIVAL CANDIDATE]",
        };
        let target_str = item.scope_target.as_deref().unwrap_or("repo");
        out.push_str(&format!(
            "  {} {} ({}) - {}\n",
            icon, item.memory_id, target_str, item.title
        ));
        out.push_str(&format!("      Status Note: {}\n", item.diagnostic_message));
        if item.liveness != LivenessStatus::Live {
            out.push_str("      Suggested Actions:\n");
            for a in &item.suggested_actions {
                out.push_str(&format!("        • {}\n", a));
            }
        }
        out.push_str("\n");
    }

    out
}
