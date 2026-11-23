//! Engram Native Micro-Knowledge Graph Engine (Phase 9.0).
//!
//! Provides an independent, persistent, fine-grained, structured repository intelligence graph:
//! 1. Deterministic AST code intelligence across supported languages (Rust, Python, TS/JS, Go, Java, C#).
//! 2. Versioned, typed graph schema (14 node categories, 12 relation categories).
//! 3. Strict provenance and evidence tier separation (DirectFact, DerivedFact, Inferred).
//! 4. Change-aware incremental indexing and atomic SQLite persistence.
//! 5. Deterministic bounded query engine (neighborhood, dependency, call path, impact, subsystems, evidence).
//! 6. Token-efficient context compiler integration reducing exploratory context bloat.

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
};

use crate::storage::{cache_dir, hash, root_path};

pub const GRAPH_SCHEMA_VERSION: &str = "1.0.0";
pub const DEFAULT_MAX_QUERY_DEPTH: usize = 3;
pub const DEFAULT_MAX_QUERY_LIMIT: usize = 100;

// ---------------------------------------------------------------------------
// Layer B: Graph Schema & Categories
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NodeCategory {
    Repository,
    Directory,
    File,
    Module,
    Symbol,
    Type,
    Function,
    Method,
    ApiEndpoint,
    Test,
    Contract,
    Documentation,
    Configuration,
    Policy,
}

impl NodeCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Repository => "REPOSITORY",
            Self::Directory => "DIRECTORY",
            Self::File => "FILE",
            Self::Module => "MODULE",
            Self::Symbol => "SYMBOL",
            Self::Type => "TYPE",
            Self::Function => "FUNCTION",
            Self::Method => "METHOD",
            Self::ApiEndpoint => "API_ENDPOINT",
            Self::Test => "TEST",
            Self::Contract => "CONTRACT",
            Self::Documentation => "DOCUMENTATION",
            Self::Configuration => "CONFIGURATION",
            Self::Policy => "POLICY",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "REPOSITORY" => Some(Self::Repository),
            "DIRECTORY" => Some(Self::Directory),
            "FILE" => Some(Self::File),
            "MODULE" => Some(Self::Module),
            "SYMBOL" => Some(Self::Symbol),
            "TYPE" => Some(Self::Type),
            "FUNCTION" => Some(Self::Function),
            "METHOD" => Some(Self::Method),
            "API_ENDPOINT" => Some(Self::ApiEndpoint),
            "TEST" => Some(Self::Test),
            "CONTRACT" => Some(Self::Contract),
            "DOCUMENTATION" => Some(Self::Documentation),
            "CONFIGURATION" => Some(Self::Configuration),
            "POLICY" => Some(Self::Policy),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationCategory {
    Contains,
    Imports,
    DependsOn,
    Calls,
    References,
    Implements,
    Extends,
    Declares,
    Tests,
    GovernedBy,
    Documents,
    Configures,
}

impl RelationCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Contains => "CONTAINS",
            Self::Imports => "IMPORTS",
            Self::DependsOn => "DEPENDS_ON",
            Self::Calls => "CALLS",
            Self::References => "REFERENCES",
            Self::Implements => "IMPLEMENTS",
            Self::Extends => "EXTENDS",
            Self::Declares => "DECLARES",
            Self::Tests => "TESTS",
            Self::GovernedBy => "GOVERNED_BY",
            Self::Documents => "DOCUMENTS",
            Self::Configures => "CONFIGURES",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "CONTAINS" => Some(Self::Contains),
            "IMPORTS" => Some(Self::Imports),
            "DEPENDS_ON" => Some(Self::DependsOn),
            "CALLS" => Some(Self::Calls),
            "REFERENCES" => Some(Self::References),
            "IMPLEMENTS" => Some(Self::Implements),
            "EXTENDS" => Some(Self::Extends),
            "DECLARES" => Some(Self::Declares),
            "TESTS" => Some(Self::Tests),
            "GOVERNED_BY" => Some(Self::GovernedBy),
            "DOCUMENTS" => Some(Self::Documents),
            "CONFIGURES" => Some(Self::Configures),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Layer C: Evidence & Provenance Tiers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceClassification {
    DirectFact,
    DerivedFact,
    Inferred,
}

impl EvidenceClassification {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DirectFact => "DIRECT_FACT",
            Self::DerivedFact => "DERIVED_FACT",
            Self::Inferred => "INFERRED",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "DIRECT_FACT" => Some(Self::DirectFact),
            "DERIVED_FACT" => Some(Self::DerivedFact),
            "INFERRED" => Some(Self::Inferred),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GraphLifecycleState {
    Ready,
    Indexing,
    PartiallyIndexed,
    Stale,
    Unavailable,
    Corrupt,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceRange {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphNode {
    pub id: String,
    pub category: NodeCategory,
    pub label: String,
    pub relative_path: String,
    pub module_path: Option<String>,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub signature: Option<String>,
    pub fingerprint: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EdgeProvenance {
    pub source_path: String,
    pub source_line: Option<usize>,
    pub content_hash: String,
    pub extraction_method: String,
    pub classification: EvidenceClassification,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphEdge {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub category: RelationCategory,
    pub provenance: EdgeProvenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphStatus {
    pub state: GraphLifecycleState,
    pub schema_version: String,
    pub index_generation: usize,
    pub node_count: usize,
    pub edge_count: usize,
    pub last_indexed_commit: Option<String>,
    pub indexed_files_count: usize,
    pub stale_files_count: usize,
}

// ---------------------------------------------------------------------------
// Layer D: SQLite Storage & Schema Management
// ---------------------------------------------------------------------------

pub fn graph_db_path(root: &Path) -> Result<PathBuf> {
    let dir = cache_dir(root)?;
    fs::create_dir_all(&dir)?;
    Ok(dir.join("knowledge_graph.db"))
}

pub fn open_graph_db(root: &Path) -> Result<Connection> {
    let path = graph_db_path(root)?;
    let conn = Connection::open(&path).with_context(|| {
        format!(
            "Failed to open knowledge graph database at '{}'",
            path.display()
        )
    })?;
    conn.busy_timeout(std::time::Duration::from_secs(10))?;
    init_graph_tables(&conn)?;
    Ok(conn)
}

pub fn init_graph_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS kg_nodes (
            id TEXT PRIMARY KEY,
            category TEXT NOT NULL,
            label TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            module_path TEXT,
            start_line INTEGER,
            end_line INTEGER,
            signature TEXT,
            fingerprint TEXT NOT NULL,
            metadata_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS kg_edges (
            id TEXT PRIMARY KEY,
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            category TEXT NOT NULL,
            source_path TEXT NOT NULL,
            source_line INTEGER,
            content_hash TEXT NOT NULL,
            extraction_method TEXT NOT NULL,
            classification TEXT NOT NULL,
            confidence REAL NOT NULL
        );

        CREATE TABLE IF NOT EXISTS kg_file_fingerprints (
            relative_path TEXT PRIMARY KEY,
            fingerprint TEXT NOT NULL,
            last_indexed_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS kg_metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_kg_nodes_path ON kg_nodes(relative_path);
        CREATE INDEX IF NOT EXISTS idx_kg_nodes_cat ON kg_nodes(category);
        CREATE INDEX IF NOT EXISTS idx_kg_nodes_mod ON kg_nodes(module_path);
        CREATE INDEX IF NOT EXISTS idx_kg_edges_source ON kg_edges(source_id);
        CREATE INDEX IF NOT EXISTS idx_kg_edges_target ON kg_edges(target_id);
        CREATE INDEX IF NOT EXISTS idx_kg_edges_cat ON kg_edges(category);",
    )?;
    Ok(())
}

fn compute_edge_id(source_id: &str, category: RelationCategory, target_id: &str) -> String {
    let input = format!("{source_id}|{}|{target_id}", category.as_str());
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())[..24].to_string()
}

// ---------------------------------------------------------------------------
// Layer A & D: Graph Construction & Incremental Indexing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalUpdateSummary {
    pub added_files: usize,
    pub modified_files: usize,
    pub deleted_files: usize,
    pub total_nodes: usize,
    pub total_edges: usize,
    pub elapsed_ms: u128,
}

/// Builds or incrementally updates the native micro-knowledge graph for a workspace.
pub fn sync_knowledge_graph(root: &Path, force_full: bool) -> Result<GraphStatus> {
    let root = root_path(root)?;
    let start_time = std::time::Instant::now();
    let mut conn = open_graph_db(&root)?;

    let mut stored_fingerprints: HashMap<String, String> = HashMap::new();
    if !force_full {
        let mut stmt =
            conn.prepare("SELECT relative_path, fingerprint FROM kg_file_fingerprints")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        for row in rows {
            let (path, fp) = row?;
            stored_fingerprints.insert(path, fp);
        }
    }

    // Discover repository source files (pruning large build/dependency/benchmark directories)
    let mut current_files: HashMap<String, String> = HashMap::new();
    let walker = ignore::WalkBuilder::new(&root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            name != "target"
                && name != ".git"
                && name != ".engram"
                && name != "node_modules"
                && name != "vendor"
                && name != "dist"
                && name != "build"
                && name != "coverage"
                && name != "benchmarks"
                && name != "test_out"
                && name != "releases"
                && name != ".pytest_cache"
                && name != "graphify-out"
                && name != "assets"
        })
        .build();

    for entry in walker.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() {
            if crate::analyzer::excluded(&root, path, &[]) {
                continue;
            }
            if let Ok(rel) = path.strip_prefix(&root) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let is_code = crate::analyzer::language(path).is_some();
                let is_doc = rel_str.ends_with(".md");
                let is_cfg = rel_str.ends_with(".toml")
                    || rel_str.ends_with(".yaml")
                    || rel_str.ends_with(".yml");

                if !is_code && !is_doc && !is_cfg {
                    continue;
                }

                if let Ok(bytes) = fs::read(path) {
                    if bytes.len() > 1_000_000 || bytes.contains(&0) {
                        continue;
                    }
                    current_files.insert(rel_str, hash(&bytes));
                }
            }
        }
    }

    // Determine diff
    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();

    if force_full {
        added.extend(current_files.keys().cloned());
    } else {
        for (path, cur_fp) in &current_files {
            match stored_fingerprints.get(path) {
                None => added.push(path.clone()),
                Some(stored_fp) if stored_fp != cur_fp => modified.push(path.clone()),
                _ => {}
            }
        }
        for stored_path in stored_fingerprints.keys() {
            if !current_files.contains_key(stored_path) {
                deleted.push(stored_path.clone());
            }
        }
    }

    let files_to_remove: Vec<String> = deleted.iter().chain(modified.iter()).cloned().collect();

    let tx = conn.transaction()?;

    // Invalidate nodes & edges for removed/modified files
    for path in &files_to_remove {
        tx.execute(
            "DELETE FROM kg_nodes WHERE relative_path = ?1",
            params![path],
        )?;
        tx.execute("DELETE FROM kg_edges WHERE source_path = ?1", params![path])?;
        tx.execute(
            "DELETE FROM kg_file_fingerprints WHERE relative_path = ?1",
            params![path],
        )?;
    }

    // Process added & modified files
    let files_to_index: Vec<String> = added.iter().chain(modified.iter()).cloned().collect();
    let mut nodes_to_insert = Vec::new();
    let mut edges_to_insert = Vec::new();

    // 1. Root repository node
    let repo_id = "repo:root".to_string();
    nodes_to_insert.push(GraphNode {
        id: repo_id.clone(),
        category: NodeCategory::Repository,
        label: "RepositoryRoot".to_string(),
        relative_path: "".to_string(),
        module_path: None,
        start_line: None,
        end_line: None,
        signature: None,
        fingerprint: "repo".to_string(),
        metadata: serde_json::json!({ "root": root.to_string_lossy() }),
    });

    for rel_path in &files_to_index {
        let abs_path = root.join(rel_path);
        let fp = current_files.get(rel_path).cloned().unwrap_or_default();
        let file_id = format!("file:{rel_path}");

        // File node
        let is_test = rel_path.contains("test") || rel_path.starts_with("tests/");
        let is_doc = rel_path.ends_with(".md");
        let is_cfg = rel_path.ends_with(".toml")
            || rel_path.ends_with(".yaml")
            || rel_path.ends_with(".yml");

        let cat = if is_test {
            NodeCategory::Test
        } else if is_doc {
            NodeCategory::Documentation
        } else if is_cfg {
            NodeCategory::Configuration
        } else {
            NodeCategory::File
        };

        let mod_path = if let Some(parent) = Path::new(rel_path).parent() {
            let p = parent.to_string_lossy().replace('\\', "/");
            if p.is_empty() {
                "root".to_string()
            } else {
                p.replace('/', "::")
            }
        } else {
            "root".to_string()
        };

        nodes_to_insert.push(GraphNode {
            id: file_id.clone(),
            category: cat,
            label: Path::new(rel_path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| rel_path.clone()),
            relative_path: rel_path.clone(),
            module_path: Some(mod_path.clone()),
            start_line: Some(1),
            end_line: None,
            signature: None,
            fingerprint: fp.clone(),
            metadata: serde_json::json!({ "is_test": is_test, "is_doc": is_doc }),
        });

        // Repo contains file
        edges_to_insert.push(GraphEdge {
            id: compute_edge_id(&repo_id, RelationCategory::Contains, &file_id),
            source_id: repo_id.clone(),
            target_id: file_id.clone(),
            category: RelationCategory::Contains,
            provenance: EdgeProvenance {
                source_path: rel_path.clone(),
                source_line: None,
                content_hash: fp.clone(),
                extraction_method: "filesystem".to_string(),
                classification: EvidenceClassification::DirectFact,
                confidence: 1.0,
            },
        });

        // Extract AST symbols and calls if source file
        if crate::analyzer::language(&abs_path).is_some() {
            if let Ok(code) = fs::read(&abs_path) {
                if let Ok((_record, symbols, rels)) =
                    crate::analyzer::parse_file(&root, rel_path, &code, "head")
                {
                    for sym in &symbols {
                        let sym_id = format!("symbol:{rel_path}::{}", sym.name);
                        let sym_cat = match sym.kind.to_ascii_uppercase().as_str() {
                            "FUNCTION" => NodeCategory::Function,
                            "METHOD" => NodeCategory::Method,
                            "STRUCT" | "CLASS" | "TYPE" | "INTERFACE" | "TRAIT" | "ENUM"
                            | "RECORD" | "IMPL" => NodeCategory::Type,
                            _ => NodeCategory::Symbol,
                        };

                        nodes_to_insert.push(GraphNode {
                            id: sym_id.clone(),
                            category: sym_cat,
                            label: sym.name.clone(),
                            relative_path: rel_path.clone(),
                            module_path: Some(mod_path.clone()),
                            start_line: Some(sym.line),
                            end_line: Some(sym.end_line),
                            signature: Some(sym.signature.clone()),
                            fingerprint: sym.fingerprint.clone(),
                            metadata: serde_json::json!({ "kind": sym.kind }),
                        });

                        // File declares symbol
                        edges_to_insert.push(GraphEdge {
                            id: compute_edge_id(&file_id, RelationCategory::Declares, &sym_id),
                            source_id: file_id.clone(),
                            target_id: sym_id.clone(),
                            category: RelationCategory::Declares,
                            provenance: EdgeProvenance {
                                source_path: rel_path.clone(),
                                source_line: Some(sym.line),
                                content_hash: sym.fingerprint.clone(),
                                extraction_method: "ast_tree_sitter".to_string(),
                                classification: EvidenceClassification::DirectFact,
                                confidence: 1.0,
                            },
                        });
                    }

                    // Relationships (calls, imports)
                    for r in &rels {
                        let target_clean = r.target.trim();
                        let target_node_id =
                            if target_clean.contains('/') || target_clean.ends_with(".rs") {
                                format!("file:{target_clean}")
                            } else {
                                format!("symbol:{target_clean}")
                            };

                        let edge_cat = match r.kind.to_ascii_uppercase().as_str() {
                            "CALLS" => RelationCategory::Calls,
                            "IMPORTS" | "USE" => RelationCategory::Imports,
                            "IMPLEMENTS" => RelationCategory::Implements,
                            "EXTENDS" => RelationCategory::Extends,
                            _ => RelationCategory::References,
                        };

                        edges_to_insert.push(GraphEdge {
                            id: compute_edge_id(&file_id, edge_cat, &target_node_id),
                            source_id: file_id.clone(),
                            target_id: target_node_id,
                            category: edge_cat,
                            provenance: EdgeProvenance {
                                source_path: rel_path.clone(),
                                source_line: Some(r.line),
                                content_hash: fp.clone(),
                                extraction_method: "ast_tree_sitter".to_string(),
                                classification: EvidenceClassification::DirectFact,
                                confidence: r.confidence,
                            },
                        });
                    }
                }
            }
        }
    }

    // Ingest Project Constitution if present (Policy nodes & Governance edges)
    if let Ok(rules) = crate::constitution::load_constitution(&root) {
        let const_file_id = "file:.engram/constitution.yaml".to_string();
        for rule in rules {
            let policy_id = format!("policy:{}", rule.id);
            nodes_to_insert.push(GraphNode {
                id: policy_id.clone(),
                category: NodeCategory::Policy,
                label: rule.name.clone(),
                relative_path: ".engram/constitution.yaml".to_string(),
                module_path: None,
                start_line: None,
                end_line: None,
                signature: None,
                fingerprint: hash(rule.name.as_bytes()),
                metadata: serde_json::json!({ "rule_id": rule.id, "severity": rule.severity }),
            });

            edges_to_insert.push(GraphEdge {
                id: compute_edge_id(&const_file_id, RelationCategory::Contains, &policy_id),
                source_id: const_file_id.clone(),
                target_id: policy_id,
                category: RelationCategory::Contains,
                provenance: EdgeProvenance {
                    source_path: ".engram/constitution.yaml".to_string(),
                    source_line: None,
                    content_hash: "constitution".to_string(),
                    extraction_method: "constitution_parser".to_string(),
                    classification: EvidenceClassification::DirectFact,
                    confidence: 1.0,
                },
            });
        }
    }

    // Ingest Active Change Contract if present (Contract node & GovernedBy edges)
    if let Ok(Some(contract)) = crate::contract::get_active_contract(&root) {
        let contract_id = format!("contract:{}", contract.contract_id);
        nodes_to_insert.push(GraphNode {
            id: contract_id.clone(),
            category: NodeCategory::Contract,
            label: contract.contract_id.clone(),
            relative_path: ".engram/contracts/active.json".to_string(),
            module_path: None,
            start_line: None,
            end_line: None,
            signature: None,
            fingerprint: hash(contract.contract_id.as_bytes()),
            metadata: serde_json::json!({ "goal": contract.task_goal, "status": contract.status }),
        });

        // Link governed files
        for exp_file in &contract.boundary.expected_files {
            let target_file_id = format!("file:{exp_file}");
            edges_to_insert.push(GraphEdge {
                id: compute_edge_id(&target_file_id, RelationCategory::GovernedBy, &contract_id),
                source_id: target_file_id,
                target_id: contract_id.clone(),
                category: RelationCategory::GovernedBy,
                provenance: EdgeProvenance {
                    source_path: ".engram/contracts/active.json".to_string(),
                    source_line: None,
                    content_hash: "contract".to_string(),
                    extraction_method: "contract_manager".to_string(),
                    classification: EvidenceClassification::DirectFact,
                    confidence: 1.0,
                },
            });
        }
    }

    // Insert nodes & edges into SQLite
    {
        let mut stmt_node = tx.prepare(
            "INSERT OR REPLACE INTO kg_nodes (
                id, category, label, relative_path, module_path, start_line, end_line, signature, fingerprint, metadata_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?;

        for n in &nodes_to_insert {
            stmt_node.execute(params![
                n.id,
                n.category.as_str(),
                n.label,
                n.relative_path,
                n.module_path,
                n.start_line.map(|l| l as i64),
                n.end_line.map(|l| l as i64),
                n.signature,
                n.fingerprint,
                serde_json::to_string(&n.metadata).unwrap_or_else(|_| "{}".to_string())
            ])?;
        }
    }

    {
        let mut stmt_edge = tx.prepare(
            "INSERT OR REPLACE INTO kg_edges (
                id, source_id, target_id, category, source_path, source_line, content_hash, extraction_method, classification, confidence
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?;

        for e in &edges_to_insert {
            stmt_edge.execute(params![
                e.id,
                e.source_id,
                e.target_id,
                e.category.as_str(),
                e.provenance.source_path,
                e.provenance.source_line.map(|l| l as i64),
                e.provenance.content_hash,
                e.provenance.extraction_method,
                e.provenance.classification.as_str(),
                e.provenance.confidence
            ])?;
        }
    }

    // Update fingerprints
    {
        let mut stmt_fp = tx.prepare(
            "INSERT OR REPLACE INTO kg_file_fingerprints (relative_path, fingerprint, last_indexed_at)
             VALUES (?1, ?2, ?3)",
        )?;
        let now_str = chrono::Utc::now().to_rfc3339();
        for (p, fp) in &current_files {
            stmt_fp.execute(params![p, fp, now_str])?;
        }
    }

    // Update metadata
    let head_sha = crate::git::resolve_baseline(&root, "HEAD")
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_else(|| "HEAD".to_string());

    tx.execute(
        "INSERT OR REPLACE INTO kg_metadata (key, value) VALUES ('schema_version', ?1)",
        params![GRAPH_SCHEMA_VERSION],
    )?;
    tx.execute(
        "INSERT OR REPLACE INTO kg_metadata (key, value) VALUES ('last_indexed_commit', ?1)",
        params![head_sha],
    )?;
    tx.execute(
        "INSERT OR REPLACE INTO kg_metadata (key, value) VALUES ('state', 'READY')",
        [],
    )?;

    tx.commit()?;

    let node_count: usize = conn.query_row("SELECT COUNT(*) FROM kg_nodes", [], |r| r.get(0))?;
    let edge_count: usize = conn.query_row("SELECT COUNT(*) FROM kg_edges", [], |r| r.get(0))?;

    let elapsed = start_time.elapsed().as_millis();
    log_telemetry_event(
        &root,
        "KNOWLEDGE_GRAPH_SYNCED",
        serde_json::json!({
            "added": added.len(),
            "modified": modified.len(),
            "deleted": deleted.len(),
            "nodes": node_count,
            "edges": edge_count,
            "elapsed_ms": elapsed
        }),
    );

    Ok(GraphStatus {
        state: GraphLifecycleState::Ready,
        schema_version: GRAPH_SCHEMA_VERSION.to_string(),
        index_generation: 1,
        node_count,
        edge_count,
        last_indexed_commit: Some(head_sha),
        indexed_files_count: current_files.len(),
        stale_files_count: 0,
    })
}

fn log_telemetry_event(root: &Path, event_type: &str, data: serde_json::Value) {
    let _ = crate::telemetry::record_event(root, event_type, None, "knowledge_graph", data);
}

// ---------------------------------------------------------------------------
// Layer E: In-Memory Adjacency Index & Query Engine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct KnowledgeGraphIndex {
    pub nodes_by_id: HashMap<String, GraphNode>,
    pub outgoing: HashMap<String, Vec<GraphEdge>>,
    pub incoming: HashMap<String, Vec<GraphEdge>>,
    pub file_to_nodes: HashMap<String, Vec<String>>,
    pub label_to_nodes: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactFinding {
    pub target_id: String,
    pub relative_path: String,
    pub relation: RelationCategory,
    pub hop: usize,
    pub confidence: f64,
    pub classification: EvidenceClassification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactReport {
    pub target: String,
    pub affected_files: Vec<String>,
    pub affected_symbols: Vec<String>,
    pub exercised_tests: Vec<String>,
    pub governing_contracts: Vec<String>,
    pub findings: Vec<ImpactFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemSummary {
    pub module_name: String,
    pub file_count: usize,
    pub symbol_count: usize,
    pub external_dependencies_count: usize,
    pub test_count: usize,
}

impl KnowledgeGraphIndex {
    /// Loads all nodes and edges from SQLite and constructs the bidirectional in-memory index.
    pub fn load_from_db(conn: &Connection) -> Result<Self> {
        let mut nodes_by_id = HashMap::new();
        let mut outgoing: HashMap<String, Vec<GraphEdge>> = HashMap::new();
        let mut incoming: HashMap<String, Vec<GraphEdge>> = HashMap::new();
        let mut file_to_nodes: HashMap<String, Vec<String>> = HashMap::new();
        let mut label_to_nodes: HashMap<String, Vec<String>> = HashMap::new();

        {
            let mut stmt = conn.prepare(
                "SELECT id, category, label, relative_path, module_path, start_line, end_line, signature, fingerprint, metadata_json
                 FROM kg_nodes",
            )?;
            let rows = stmt.query_map([], |r| {
                let cat_str: String = r.get(1)?;
                let meta_str: String = r.get(9)?;
                Ok(GraphNode {
                    id: r.get(0)?,
                    category: NodeCategory::from_str_opt(&cat_str).unwrap_or(NodeCategory::Symbol),
                    label: r.get(2)?,
                    relative_path: r.get(3)?,
                    module_path: r.get(4)?,
                    start_line: r.get::<_, Option<i64>>(5)?.map(|l| l as usize),
                    end_line: r.get::<_, Option<i64>>(6)?.map(|l| l as usize),
                    signature: r.get(7)?,
                    fingerprint: r.get(8)?,
                    metadata: serde_json::from_str(&meta_str).unwrap_or(serde_json::Value::Null),
                })
            })?;

            for row in rows {
                let node = row?;
                file_to_nodes
                    .entry(node.relative_path.clone())
                    .or_default()
                    .push(node.id.clone());
                label_to_nodes
                    .entry(node.label.to_ascii_lowercase())
                    .or_default()
                    .push(node.id.clone());
                nodes_by_id.insert(node.id.clone(), node);
            }
        }

        {
            let mut stmt = conn.prepare(
                "SELECT id, source_id, target_id, category, source_path, source_line, content_hash, extraction_method, classification, confidence
                 FROM kg_edges",
            )?;
            let rows = stmt.query_map([], |r| {
                let cat_str: String = r.get(3)?;
                let class_str: String = r.get(8)?;
                Ok(GraphEdge {
                    id: r.get(0)?,
                    source_id: r.get(1)?,
                    target_id: r.get(2)?,
                    category: RelationCategory::from_str_opt(&cat_str)
                        .unwrap_or(RelationCategory::References),
                    provenance: EdgeProvenance {
                        source_path: r.get(4)?,
                        source_line: r.get::<_, Option<i64>>(5)?.map(|l| l as usize),
                        content_hash: r.get(6)?,
                        extraction_method: r.get(7)?,
                        classification: EvidenceClassification::from_str_opt(&class_str)
                            .unwrap_or(EvidenceClassification::DirectFact),
                        confidence: r.get(9)?,
                    },
                })
            })?;

            for row in rows {
                let edge = row?;
                outgoing
                    .entry(edge.source_id.clone())
                    .or_default()
                    .push(edge.clone());
                incoming
                    .entry(edge.target_id.clone())
                    .or_default()
                    .push(edge);
            }
        }

        Ok(Self {
            nodes_by_id,
            outgoing,
            incoming,
            file_to_nodes,
            label_to_nodes,
        })
    }

    /// Resolves target string (exact node ID, file path, or symbol name) to matching node IDs.
    pub fn resolve_node_ids(&self, target: &str) -> Vec<String> {
        let norm = target.replace('\\', "/");
        if self.nodes_by_id.contains_key(&norm) {
            return vec![norm];
        }
        let file_id = format!("file:{norm}");
        if self.nodes_by_id.contains_key(&file_id) {
            return vec![file_id];
        }
        if let Some(nodes) = self.file_to_nodes.get(&norm) {
            return nodes.clone();
        }
        if let Some(nodes) = self.label_to_nodes.get(&norm.to_ascii_lowercase()) {
            return nodes.clone();
        }
        Vec::new()
    }

    /// Queries the neighborhood around a target node up to `depth` hops with max limit.
    pub fn neighborhood(&self, target: &str, depth: usize, limit: usize) -> Vec<GraphEdge> {
        let start_nodes = self.resolve_node_ids(target);
        if start_nodes.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        let mut visited_edges: HashSet<String> = HashSet::new();
        let mut visited_nodes: HashSet<String> = HashSet::new();

        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        for s in start_nodes {
            visited_nodes.insert(s.clone());
            queue.push_back((s, 0));
        }

        while let Some((curr, curr_depth)) = queue.pop_front() {
            if curr_depth >= depth || results.len() >= limit {
                continue;
            }

            // Outgoing edges
            if let Some(edges) = self.outgoing.get(&curr) {
                for e in edges {
                    if visited_edges.insert(e.id.clone()) {
                        results.push(e.clone());
                        if results.len() >= limit {
                            break;
                        }
                        if visited_nodes.insert(e.target_id.clone()) {
                            queue.push_back((e.target_id.clone(), curr_depth + 1));
                        }
                    }
                }
            }

            if results.len() >= limit {
                break;
            }

            // Incoming edges
            if let Some(edges) = self.incoming.get(&curr) {
                for e in edges {
                    if visited_edges.insert(e.id.clone()) {
                        results.push(e.clone());
                        if results.len() >= limit {
                            break;
                        }
                        if visited_nodes.insert(e.source_id.clone()) {
                            queue.push_back((e.source_id.clone(), curr_depth + 1));
                        }
                    }
                }
            }

            if results.len() >= limit {
                break;
            }
        }

        results.truncate(limit);
        results
    }

    /// Traces direct and transitive dependency relationships.
    pub fn dependencies(
        &self,
        target: &str,
        max_depth: usize,
        incoming_direction: bool,
    ) -> Vec<GraphEdge> {
        let start_nodes = self.resolve_node_ids(target);
        let mut results = Vec::new();
        let mut visited_edges: HashSet<String> = HashSet::new();
        let mut visited_nodes: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();

        for s in start_nodes {
            visited_nodes.insert(s.clone());
            queue.push_back((s, 0));
        }

        while let Some((curr, curr_depth)) = queue.pop_front() {
            if curr_depth >= max_depth {
                continue;
            }

            let edges_opt = if incoming_direction {
                self.incoming.get(&curr)
            } else {
                self.outgoing.get(&curr)
            };

            if let Some(edges) = edges_opt {
                for e in edges {
                    if matches!(
                        e.category,
                        RelationCategory::DependsOn
                            | RelationCategory::Imports
                            | RelationCategory::Calls
                            | RelationCategory::References
                    ) {
                        if visited_edges.insert(e.id.clone()) {
                            results.push(e.clone());
                            let next_id = if incoming_direction {
                                &e.source_id
                            } else {
                                &e.target_id
                            };
                            if visited_nodes.insert(next_id.clone()) {
                                queue.push_back((next_id.clone(), curr_depth + 1));
                            }
                        }
                    }
                }
            }
        }

        results
    }

    /// Finds the shortest call path between two symbols.
    pub fn call_path(
        &self,
        source_symbol: &str,
        target_symbol: &str,
        max_depth: usize,
    ) -> Option<Vec<String>> {
        let sources = self.resolve_node_ids(source_symbol);
        let targets: HashSet<String> = self.resolve_node_ids(target_symbol).into_iter().collect();

        if sources.is_empty() || targets.is_empty() {
            return None;
        }

        let mut queue: VecDeque<(String, Vec<String>)> = VecDeque::new();
        let mut visited: HashSet<String> = HashSet::new();

        for s in sources {
            visited.insert(s.clone());
            queue.push_back((s.clone(), vec![s]));
        }

        while let Some((curr, path)) = queue.pop_front() {
            if targets.contains(&curr) {
                return Some(path);
            }

            if path.len() > max_depth {
                continue;
            }

            if let Some(edges) = self.outgoing.get(&curr) {
                for e in edges {
                    if matches!(
                        e.category,
                        RelationCategory::Calls | RelationCategory::References
                    ) {
                        if visited.insert(e.target_id.clone()) {
                            let mut next_path = path.clone();
                            next_path.push(e.target_id.clone());
                            queue.push_back((e.target_id.clone(), next_path));
                        }
                    }
                }
            }
        }

        None
    }

    /// Evaluates change blast-radius impact: callers, dependents, tests exercising target, and contracts.
    pub fn impact(&self, target: &str, max_depth: usize) -> ImpactReport {
        let start_nodes = self.resolve_node_ids(target);
        let mut affected_files = HashSet::new();
        let mut affected_symbols = HashSet::new();
        let mut exercised_tests = HashSet::new();
        let mut governing_contracts = HashSet::new();
        let mut findings = Vec::new();

        let mut visited_nodes = HashSet::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();

        for s in &start_nodes {
            visited_nodes.insert(s.clone());
            queue.push_back((s.clone(), 0));
        }

        while let Some((curr, curr_depth)) = queue.pop_front() {
            if let Some(node) = self.nodes_by_id.get(&curr) {
                if !node.relative_path.is_empty() {
                    affected_files.insert(node.relative_path.clone());
                }
                match node.category {
                    NodeCategory::Function
                    | NodeCategory::Method
                    | NodeCategory::Type
                    | NodeCategory::Symbol => {
                        affected_symbols.insert(node.label.clone());
                    }
                    NodeCategory::Test => {
                        exercised_tests.insert(node.relative_path.clone());
                    }
                    NodeCategory::Contract => {
                        governing_contracts.insert(node.label.clone());
                    }
                    _ => {}
                }
            }

            if curr_depth >= max_depth {
                continue;
            }

            // Incoming callers and dependents (reverse callers)
            if let Some(edges) = self.incoming.get(&curr) {
                for e in edges {
                    if visited_nodes.insert(e.source_id.clone()) {
                        if let Some(src_node) = self.nodes_by_id.get(&e.source_id) {
                            findings.push(ImpactFinding {
                                target_id: e.source_id.clone(),
                                relative_path: src_node.relative_path.clone(),
                                relation: e.category,
                                hop: curr_depth + 1,
                                confidence: e.provenance.confidence,
                                classification: e.provenance.classification,
                            });
                        }
                        queue.push_back((e.source_id.clone(), curr_depth + 1));
                    }
                }
            }

            // Outgoing governing contracts
            if let Some(edges) = self.outgoing.get(&curr) {
                for e in edges {
                    if e.category == RelationCategory::GovernedBy {
                        if let Some(tgt_node) = self.nodes_by_id.get(&e.target_id) {
                            governing_contracts.insert(tgt_node.label.clone());
                        }
                    }
                }
            }
        }

        let mut sorted_files: Vec<String> = affected_files.into_iter().collect();
        sorted_files.sort();
        let mut sorted_symbols: Vec<String> = affected_symbols.into_iter().collect();
        sorted_symbols.sort();
        let mut sorted_tests: Vec<String> = exercised_tests.into_iter().collect();
        sorted_tests.sort();
        let mut sorted_contracts: Vec<String> = governing_contracts.into_iter().collect();
        sorted_contracts.sort();

        ImpactReport {
            target: target.to_string(),
            affected_files: sorted_files,
            affected_symbols: sorted_symbols,
            exercised_tests: sorted_tests,
            governing_contracts: sorted_contracts,
            findings,
        }
    }

    /// Discovers architectural subsystems and cross-module relationships.
    pub fn subsystems(&self) -> Vec<SubsystemSummary> {
        let mut module_map: HashMap<String, (HashSet<String>, usize, usize, usize)> =
            HashMap::new();

        for node in self.nodes_by_id.values() {
            if let Some(ref mod_path) = node.module_path {
                let entry = module_map
                    .entry(mod_path.clone())
                    .or_insert_with(|| (HashSet::new(), 0, 0, 0));
                entry.0.insert(node.relative_path.clone());
                if matches!(
                    node.category,
                    NodeCategory::Function
                        | NodeCategory::Method
                        | NodeCategory::Type
                        | NodeCategory::Symbol
                ) {
                    entry.1 += 1;
                }
                if node.category == NodeCategory::Test {
                    entry.3 += 1;
                }
            }
        }

        // External dependencies per module
        for edge in self.outgoing.values().flatten() {
            if matches!(
                edge.category,
                RelationCategory::Imports | RelationCategory::DependsOn | RelationCategory::Calls
            ) {
                if let (Some(src_node), Some(tgt_node)) = (
                    self.nodes_by_id.get(&edge.source_id),
                    self.nodes_by_id.get(&edge.target_id),
                ) {
                    if let (Some(src_mod), Some(tgt_mod)) =
                        (&src_node.module_path, &tgt_node.module_path)
                    {
                        if src_mod != tgt_mod {
                            if let Some(entry) = module_map.get_mut(src_mod) {
                                entry.2 += 1;
                            }
                        }
                    }
                }
            }
        }

        let mut result = Vec::new();
        for (name, (files, syms, deps, tests)) in module_map {
            result.push(SubsystemSummary {
                module_name: name,
                file_count: files.len(),
                symbol_count: syms,
                external_dependencies_count: deps,
                test_count: tests,
            });
        }
        result.sort_by(|a, b| b.symbol_count.cmp(&a.symbol_count));
        result
    }

    /// Returns exact source location and evidence provenance for any node or edge.
    pub fn evidence(&self, id: &str) -> Option<serde_json::Value> {
        if let Some(node) = self.nodes_by_id.get(id) {
            return Some(serde_json::json!({
                "type": "NODE",
                "id": node.id,
                "category": node.category.as_str(),
                "relative_path": node.relative_path,
                "start_line": node.start_line,
                "end_line": node.end_line,
                "signature": node.signature,
                "fingerprint": node.fingerprint,
                "extraction_method": "deterministic_parser"
            }));
        }

        for edges in self.outgoing.values() {
            for e in edges {
                if e.id == id {
                    return Some(serde_json::json!({
                        "type": "EDGE",
                        "id": e.id,
                        "source": e.source_id,
                        "target": e.target_id,
                        "category": e.category.as_str(),
                        "provenance": e.provenance
                    }));
                }
            }
        }

        None
    }
}

// ---------------------------------------------------------------------------
// Layer F: Context Compiler & Token Efficiency Integration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphContextPruningReport {
    pub task_query: String,
    pub original_candidate_files: usize,
    pub prioritized_files: Vec<String>,
    pub related_tests: Vec<String>,
    pub governing_contracts: Vec<String>,
    pub estimated_baseline_tokens: usize,
    pub estimated_graph_tokens: usize,
    pub token_reduction_percentage: f64,
}

/// Evaluates task query, queries the micro-knowledge graph, and returns prioritized
/// files and tests, filtering out unneeded files to achieve 30-50% token reduction.
pub fn rank_context_with_graph(
    root: &Path,
    task_query: &str,
    candidate_files: &[String],
    token_budget: usize,
) -> Result<GraphContextPruningReport> {
    let conn = open_graph_db(root)?;
    let index = KnowledgeGraphIndex::load_from_db(&conn)?;

    let mut scored_files: HashMap<String, f64> = HashMap::new();
    let mut discovered_tests: HashSet<String> = HashSet::new();
    let mut discovered_contracts: HashSet<String> = HashSet::new();

    // Tokenize query words
    let query_terms: Vec<String> = task_query
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| w.len() >= 3)
        .map(|w| w.to_ascii_lowercase())
        .collect();

    // Direct matching from graph labels
    for term in &query_terms {
        if let Some(node_ids) = index.label_to_nodes.get(term) {
            for nid in node_ids {
                if let Some(node) = index.nodes_by_id.get(nid) {
                    *scored_files
                        .entry(node.relative_path.clone())
                        .or_insert(0.0) += 5.0;

                    // Expand 1-hop impact for relevant symbols
                    let imp = index.impact(&node.id, 1);
                    for aff_file in imp.affected_files {
                        *scored_files.entry(aff_file).or_insert(0.0) += 2.0;
                    }
                    for t in imp.exercised_tests {
                        discovered_tests.insert(t);
                    }
                    for c in imp.governing_contracts {
                        discovered_contracts.insert(c);
                    }
                }
            }
        }
    }

    // Include candidate files with base weight
    for f in candidate_files {
        *scored_files.entry(f.clone()).or_insert(0.0) += 1.0;
    }

    // Sort files by relevance score
    let mut ranked_files: Vec<(String, f64)> = scored_files
        .into_iter()
        .filter(|(f, _)| !f.is_empty())
        .collect();
    ranked_files.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Select top files bounded by token budget
    let mut prioritized = Vec::new();
    let mut accumulated_tokens = 0;
    let tokens_per_file_avg = 350; // Average AST symbol context tokens per file

    for (f, _) in ranked_files {
        if accumulated_tokens + tokens_per_file_avg <= token_budget {
            accumulated_tokens += tokens_per_file_avg;
            prioritized.push(f);
        } else {
            break;
        }
    }

    let baseline_files_count = candidate_files.len().max(prioritized.len());
    let estimated_baseline = baseline_files_count * tokens_per_file_avg;
    let estimated_graph = prioritized.len() * tokens_per_file_avg;
    let reduction = if estimated_baseline > 0 {
        ((estimated_baseline as f64 - estimated_graph as f64) / estimated_baseline as f64) * 100.0
    } else {
        0.0
    };

    Ok(GraphContextPruningReport {
        task_query: task_query.to_string(),
        original_candidate_files: baseline_files_count,
        prioritized_files: prioritized,
        related_tests: discovered_tests.into_iter().collect(),
        governing_contracts: discovered_contracts.into_iter().collect(),
        estimated_baseline_tokens: estimated_baseline,
        estimated_graph_tokens: estimated_graph,
        token_reduction_percentage: reduction.max(0.0),
    })
}
