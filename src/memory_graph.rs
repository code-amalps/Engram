//! Memory ↔ Code Relational Graph: bidirectional GOVERNS, EXPLAINS, MOTIVATED_BY, and SUPERSEDES relations.

use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::memory::{load_durable_memories, DurableMemory, ScopeLevel};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MemoryEdgeKind {
    Governs,
    Explains,
    MotivatedBy,
    Supersedes,
}

impl MemoryEdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Governs => "GOVERNS",
            Self::Explains => "EXPLAINS",
            Self::MotivatedBy => "MOTIVATED_BY",
            Self::Supersedes => "SUPERSEDES",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedEntity {
    pub name: String,
    pub entity_kind: String, // "SYMBOL", "FILE", "MODULE", "CONTRACT"
    pub path: Option<String>,
}

/// Initialize SQLite tables and indexes for the relational memory graph.
pub fn init_memory_graph_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS memory_nodes (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            statement TEXT NOT NULL,
            reason TEXT NOT NULL,
            scope_level TEXT NOT NULL,
            scope_target TEXT,
            team TEXT,
            lifecycle_state TEXT NOT NULL,
            evidence_tier TEXT NOT NULL,
            accepted_by TEXT NOT NULL,
            accepted_at TEXT NOT NULL,
            supersedes TEXT,
            superseded_by TEXT,
            data_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS memory_edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            provenance TEXT,
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_mem_edges_source ON memory_edges(source_id);
        CREATE INDEX IF NOT EXISTS idx_mem_edges_target ON memory_edges(target_id);
        CREATE INDEX IF NOT EXISTS idx_mem_edges_kind ON memory_edges(kind);",
    )?;
    Ok(())
}

/// Build and synchronize the relational memory graph in SQLite from .engram/memory.yaml and AST tables.
pub fn build_memory_graph(root: &Path, conn: &Connection) -> Result<()> {
    init_memory_graph_tables(conn)?;

    let doc = load_durable_memories(root)?;

    // Fetch existing symbols from entity_versions to resolve module-level governance
    let mut extracted_symbols: Vec<(String, String)> = Vec::new(); // (symbol_name, file_path)
    {
        let mut stmt = conn.prepare(
            "SELECT name, path FROM entity_versions WHERE invalidated_at_revision IS NULL"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for r in rows.flatten() {
            extracted_symbols.push(r);
        }
    }

    conn.execute("DELETE FROM memory_nodes", [])?;
    conn.execute("DELETE FROM memory_edges", [])?;

    for mem in &doc.memories {
        let serialized = serde_json::to_string(mem)?;
        let scope_level_str = format!("{:?}", mem.scope.level).to_uppercase();
        let state_str = format!("{:?}", mem.lifecycle_state).to_uppercase();
        let evidence_str = format!("{:?}", mem.evidence_tier).to_uppercase();
        let accepted_at_str = mem.accepted_at.to_rfc3339();

        conn.execute(
            "INSERT OR REPLACE INTO memory_nodes (
                id, title, statement, reason, scope_level, scope_target, team,
                lifecycle_state, evidence_tier, accepted_by, accepted_at,
                supersedes, superseded_by, data_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                mem.id,
                mem.title,
                mem.statement,
                mem.reason,
                scope_level_str,
                mem.scope.target,
                mem.scope.team,
                state_str,
                evidence_str,
                mem.accepted_by,
                accepted_at_str,
                mem.supersedes,
                mem.superseded_by,
                serialized,
            ],
        )?;

        // Edge 1: SUPERSEDES
        if let Some(ref old_id) = mem.supersedes {
            conn.execute(
                "INSERT INTO memory_edges (source_id, target_id, kind, provenance, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    mem.id,
                    old_id,
                    MemoryEdgeKind::Supersedes.as_str(),
                    "Supersession lineage link",
                    accepted_at_str,
                ],
            )?;
        }

        // Edge 2: GOVERNS
        match mem.scope.level {
            ScopeLevel::Symbol => {
                if let Some(ref sym) = mem.scope.target {
                    conn.execute(
                        "INSERT INTO memory_edges (source_id, target_id, kind, provenance, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![mem.id, sym, MemoryEdgeKind::Governs.as_str(), "Direct symbol target", accepted_at_str],
                    )?;
                }
                for sym in &mem.scope.symbols {
                    conn.execute(
                        "INSERT INTO memory_edges (source_id, target_id, kind, provenance, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![mem.id, sym, MemoryEdgeKind::Governs.as_str(), "Symbol target list", accepted_at_str],
                    )?;
                }
            }
            ScopeLevel::Module => {
                let mut mod_names = mem.scope.modules.clone();
                if let Some(ref t) = mem.scope.target {
                    if !mod_names.contains(t) {
                        mod_names.push(t.clone());
                    }
                }
                for m in &mod_names {
                    let mod_tag = format!("module:{m}");
                    conn.execute(
                        "INSERT INTO memory_edges (source_id, target_id, kind, provenance, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![mem.id, mod_tag, MemoryEdgeKind::Governs.as_str(), "Module target", accepted_at_str],
                    )?;

                    // Ground symbols belonging to this module
                    let m_lower = m.to_ascii_lowercase();
                    for (sym_name, path) in &extracted_symbols {
                        let p_lower = path.to_ascii_lowercase();
                        if p_lower.contains(&format!("/{m_lower}/"))
                            || p_lower.contains(&format!("\\{m_lower}\\"))
                            || p_lower.starts_with(&format!("{m_lower}/"))
                        {
                            conn.execute(
                                "INSERT INTO memory_edges (source_id, target_id, kind, provenance, created_at)
                                 VALUES (?1, ?2, ?3, ?4, ?5)",
                                params![mem.id, sym_name, MemoryEdgeKind::Governs.as_str(), "Module symbol extraction", accepted_at_str],
                            )?;
                        }
                    }
                }
            }
            ScopeLevel::Package => {
                for file in &mem.scope.files {
                    conn.execute(
                        "INSERT INTO memory_edges (source_id, target_id, kind, provenance, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![mem.id, file, MemoryEdgeKind::Governs.as_str(), "Package file target", accepted_at_str],
                    )?;
                }
            }
            ScopeLevel::Global => {
                conn.execute(
                    "INSERT INTO memory_edges (source_id, target_id, kind, provenance, created_at)
                     VALUES (?1, 'global:*', ?2, 'Global repository policy', ?3)",
                    params![mem.id, MemoryEdgeKind::Governs.as_str(), accepted_at_str],
                )?;
            }
            ScopeLevel::Repository => {
                let repo_target = mem.scope.target.as_deref().unwrap_or("repo:*");
                conn.execute(
                    "INSERT INTO memory_edges (source_id, target_id, kind, provenance, created_at)
                     VALUES (?1, ?2, ?3, 'Repository scope target', ?4)",
                    params![mem.id, repo_target, MemoryEdgeKind::Governs.as_str(), accepted_at_str],
                )?;
            }
        }

        // Edge 3: EXPLAINS (Contract provenance)
        if let Some(ref prov) = mem.provenance {
            if let Some(ref cid) = prov.source_contract {
                conn.execute(
                    "INSERT INTO memory_edges (source_id, target_id, kind, provenance, created_at)
                     VALUES (?1, ?2, ?3, 'Change contract authorization', ?4)",
                    params![mem.id, cid, MemoryEdgeKind::Explains.as_str(), accepted_at_str],
                )?;
            }
            // Edge 4: MOTIVATED_BY (Commit provenance)
            if let Some(ref sha) = prov.source_commit {
                conn.execute(
                    "INSERT INTO memory_edges (source_id, target_id, kind, provenance, created_at)
                     VALUES (?1, ?2, ?3, 'Originating commit motivation', ?4)",
                    params![mem.id, sha, MemoryEdgeKind::MotivatedBy.as_str(), accepted_at_str],
                )?;
            }
        }
    }

    Ok(())
}

/// Query active memories governing a specific symbol or target path (Code -> Why?).
pub fn query_governing_memories(conn: &Connection, target_symbol: &str) -> Result<Vec<DurableMemory>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT mn.data_json
         FROM memory_nodes mn
         JOIN memory_edges me ON me.source_id = mn.id
         WHERE me.kind = 'GOVERNS'
           AND (
               me.target_id = ?1
               OR me.target_id = 'global:*'
               OR (?1 LIKE '%' || REPLACE(me.target_id, 'module:', '') || '%')
           )
           AND mn.lifecycle_state = 'DURABLE'
         ORDER BY 
           CASE mn.scope_level
             WHEN 'SYMBOL' THEN 1
             WHEN 'PACKAGE' THEN 2
             WHEN 'MODULE' THEN 3
             WHEN 'REPOSITORY' THEN 4
             ELSE 5
           END"
    )?;

    let rows = stmt.query_map([target_symbol], |row| {
        let json_str: String = row.get(0)?;
        Ok(json_str)
    })?;

    let mut results = Vec::new();
    for r in rows.flatten() {
        if let Ok(mem) = serde_json::from_str::<DurableMemory>(&r) {
            if !results.iter().any(|m: &DurableMemory| m.id == mem.id) {
                results.push(mem);
            }
        }
    }

    Ok(results)
}

/// Query all code entities and files governed by a specific memory (Decision -> Affected Code?).
pub fn query_governed_entities(conn: &Connection, memory_id: &str) -> Result<Vec<GovernedEntity>> {
    let mut stmt = conn.prepare(
        "SELECT target_id, kind FROM memory_edges WHERE source_id = ?1 AND kind = 'GOVERNS'"
    )?;

    let rows = stmt.query_map([memory_id], |row| {
        let target_id: String = row.get(0)?;
        let kind: String = row.get(1)?;
        Ok((target_id, kind))
    })?;

    let mut entities = Vec::new();
    for r in rows.flatten() {
        let (target, _) = r;
        if target.starts_with("module:") {
            entities.push(GovernedEntity {
                name: target.clone(),
                entity_kind: "MODULE".into(),
                path: None,
            });
        } else if target.contains("::") || target.contains('.') {
            entities.push(GovernedEntity {
                name: target.clone(),
                entity_kind: "SYMBOL".into(),
                path: None,
            });
        } else {
            entities.push(GovernedEntity {
                name: target.clone(),
                entity_kind: "FILE".into(),
                path: Some(target),
            });
        }
    }

    Ok(entities)
}
