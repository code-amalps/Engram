//! Decision Archaeology Engine: answers "Why was this code designed this way?".
//! Traces chronological evolution, active governing policies, contract provenance, and commit rationale.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::analyzer::ensure_fresh;
use crate::memory::{load_durable_memories, DurableMemory};
use crate::memory_graph::{build_memory_graph, query_governed_entities, query_governing_memories};
use crate::storage::{db, root_path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchaeologyReport {
    pub target_symbol: String,
    pub target_file: Option<String>,
    pub current_governing_policy: Option<DurableMemory>,
    pub historical_evolution: Vec<DurableMemory>,
    pub rationale_summary: String,
    pub contract_provenance: Vec<String>,
    pub commit_provenance: Vec<String>,
    pub governed_entities: Vec<String>,
}

/// Execute decision archaeology for a given symbol or file (Answering: "Why is it this way?").
pub fn explain_why(root: &Path, target_symbol: &str) -> Result<ArchaeologyReport> {
    let root = root_path(root)?;
    ensure_fresh(&root)?;

    let conn = db(&root)?;
    build_memory_graph(&root, &conn)?;

    // 1. Resolve target file if available
    let target_file: Option<String> = {
        let mut stmt = conn.prepare(
            "SELECT path FROM entity_versions WHERE name = ?1 AND invalidated_at_revision IS NULL LIMIT 1"
        )?;
        let mut rows = stmt.query([target_symbol])?;
        if let Some(row) = rows.next()? {
            Some(row.get(0)?)
        } else {
            None
        }
    };

    // 2. Query governing memories (Active Hot Truth)
    let governing = query_governing_memories(&conn, target_symbol)?;
    let current_governing = governing.first().cloned();

    // 3. Trace historical supersession evolution
    let doc = load_durable_memories(&root)?;
    let mut evolution = Vec::new();

    if let Some(ref current) = current_governing {
        let mut current_id = current.supersedes.clone();
        let mut depth = 0;
        while let Some(prev_id) = current_id {
            if depth >= 20 {
                break; // Cycle guard
            }
            if let Some(prev_mem) = doc.memories.iter().find(|m| m.id == prev_id) {
                evolution.push(prev_mem.clone());
                current_id = prev_mem.supersedes.clone();
            } else {
                break;
            }
            depth += 1;
        }
    }

    // 4. Formulate rationale summary
    let rationale_summary = if let Some(ref current) = current_governing {
        format!(
            "{}: {} (Rationale: {})",
            current.title, current.statement, current.reason
        )
    } else {
        "No explicit architectural decision governs this symbol directly.".to_string()
    };

    // 5. Collect contract and commit provenance
    let mut contracts = Vec::new();
    let mut commits = Vec::new();
    let mut governed_entities = Vec::new();

    if let Some(ref current) = current_governing {
        if let Some(ref prov) = current.provenance {
            if let Some(ref c) = prov.source_contract {
                contracts.push(c.clone());
            }
            if let Some(ref s) = prov.source_commit {
                commits.push(s.clone());
            }
        }
        if let Ok(ents) = query_governed_entities(&conn, &current.id) {
            governed_entities = ents.into_iter().map(|e| e.name).collect();
        }
    }

    Ok(ArchaeologyReport {
        target_symbol: target_symbol.to_string(),
        target_file,
        current_governing_policy: current_governing,
        historical_evolution: evolution,
        rationale_summary,
        contract_provenance: contracts,
        commit_provenance: commits,
        governed_entities,
    })
}

/// Format human-readable archaeology report for the terminal.
pub fn format_archaeology_explanation(report: &ArchaeologyReport) -> String {
    let mut out = String::new();
    let file_label = report.target_file.as_deref().unwrap_or("unknown file");
    out.push_str(&format!(
        "ARCHAEOLOGY: {} ({})\n",
        report.target_symbol, file_label
    ));
    out.push_str("──────────────────────────────────────────────────────────────────────────\n");

    if let Some(ref current) = report.current_governing_policy {
        out.push_str("Active Governing Policy:\n");
        let target_label = current.scope.target.as_deref().unwrap_or("repository");
        out.push_str(&format!(
            "  • {} ({}) [{:?}]\n",
            current.id, target_label, current.lifecycle_state
        ));
        out.push_str(&format!("    Scope:     {:?}\n", current.scope.level));
        out.push_str(&format!(
            "    Ratified:  {} by {}\n",
            current.accepted_at.format("%Y-%m-%d"),
            current.accepted_by
        ));
        out.push_str(&format!("    Statement: {}\n", current.statement));
        out.push_str(&format!("    Rationale: {}\n\n", current.reason));
    } else {
        out.push_str("Active Governing Policy: None (Implicit architecture)\n\n");
    }

    if !report.historical_evolution.is_empty() {
        out.push_str("Historical Decision Evolution:\n");
        for (i, prev) in report.historical_evolution.iter().enumerate() {
            let is_last = i == report.historical_evolution.len() - 1;
            let branch = if is_last { "└──" } else { "├──" };
            let pipe = if is_last { "    " } else { "│   " };
            out.push_str(&format!("  {} {} ({})\n", branch, prev.id, prev.title));
            out.push_str(&format!(
                "  {}  Status:    {:?}\n",
                pipe, prev.lifecycle_state
            ));
            out.push_str(&format!("  {}  Statement: {}\n", pipe, prev.statement));
            out.push_str(&format!("  {}  Rationale: {}\n", pipe, prev.reason));
            if !is_last {
                out.push_str("  │\n");
            }
        }
        out.push_str("\n");
    }

    if !report.contract_provenance.is_empty() || !report.commit_provenance.is_empty() {
        out.push_str("Causal Provenance:\n");
        for c in &report.contract_provenance {
            out.push_str(&format!("  • Change Contract: {}\n", c));
        }
        for s in &report.commit_provenance {
            out.push_str(&format!("  • Motivated by Commit: {}\n", s));
        }
        out.push_str("\n");
    }

    out
}
