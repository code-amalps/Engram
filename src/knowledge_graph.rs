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
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
};

use crate::storage::{cache_dir, hash, root_path};

pub const GRAPH_SCHEMA_VERSION: &str = "2.0.0";
pub const GRAPH_PARSER_VERSION: &str = "ts0.23-cs0.23.5-py0.23.6-rs0.23.3-go-java";
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
    Inherits,
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
            Self::Inherits => "INHERITS",
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
            "IMPORTS" | "IMPORTS_SYMBOL" | "REEXPORTS" | "USE" => Some(Self::Imports),
            "DEPENDS_ON" => Some(Self::DependsOn),
            "CALLS" => Some(Self::Calls),
            "REFERENCES" => Some(Self::References),
            "IMPLEMENTS" => Some(Self::Implements),
            "INHERITS" => Some(Self::Inherits),
            "EXTENDS" => Some(Self::Extends),
            "DECLARES" | "DECLARES_MODULE" => Some(Self::Declares),
            "TESTS" => Some(Self::Tests),
            "GOVERNED_BY" => Some(Self::GovernedBy),
            "DOCUMENTS" => Some(Self::Documents),
            "CONFIGURES" => Some(Self::Configures),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResolutionStatus {
    #[default]
    Resolved,
    PartiallyResolved,
    Unresolved,
    External,
    Ambiguous,
}

impl ResolutionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Resolved => "RESOLVED",
            Self::PartiallyResolved => "PARTIALLY_RESOLVED",
            Self::Unresolved => "UNRESOLVED",
            Self::External => "EXTERNAL",
            Self::Ambiguous => "AMBIGUOUS",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "RESOLVED" => Some(Self::Resolved),
            "PARTIALLY_RESOLVED" => Some(Self::PartiallyResolved),
            "UNRESOLVED" => Some(Self::Unresolved),
            "EXTERNAL" => Some(Self::External),
            "AMBIGUOUS" => Some(Self::Ambiguous),
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
    Failed,
}

impl GraphLifecycleState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::Indexing => "INDEXING",
            Self::PartiallyIndexed => "PARTIALLY_INDEXED",
            Self::Stale => "STALE",
            Self::Unavailable => "UNAVAILABLE",
            Self::Corrupt => "CORRUPT",
            Self::Failed => "FAILED",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "READY" => Some(Self::Ready),
            "INDEXING" => Some(Self::Indexing),
            "PARTIALLY_INDEXED" => Some(Self::PartiallyIndexed),
            "STALE" => Some(Self::Stale),
            "UNAVAILABLE" => Some(Self::Unavailable),
            "CORRUPT" => Some(Self::Corrupt),
            "FAILED" => Some(Self::Failed),
            _ => None,
        }
    }
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
    #[serde(default)]
    pub resolution_status: ResolutionStatus,
    #[serde(default)]
    pub target_hint: Option<String>,
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
    pub failed_files_count: usize,
    pub unsupported_files_count: usize,
    pub last_error: Option<String>,
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
    conn.busy_timeout(std::time::Duration::from_secs(30))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    let is_v2_ready = conn
        .query_row(
            "SELECT 1 FROM kg_schema_migrations WHERE version = '2.0.0' LIMIT 1",
            [],
            |r| r.get::<_, i64>(0),
        )
        .is_ok();
    if !is_v2_ready {
        init_graph_tables(&conn)?;
        run_schema_migrations(&conn)?;
    }
    Ok(conn)
}

pub fn run_schema_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS kg_schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL,
            description TEXT NOT NULL
        );",
    )?;

    let applied: HashSet<String> = {
        let mut stmt = conn.prepare("SELECT version FROM kg_schema_migrations")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        let mut set = HashSet::new();
        for r in rows {
            set.insert(r?);
        }
        set
    };

    if !applied.contains("1.0.0") {
        init_graph_tables(conn)?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO kg_schema_migrations (version, applied_at, description) VALUES ('1.0.0', ?1, 'Initial graph schema tables')",
            params![now],
        )?;
    }

    if !applied.contains("1.1.0") {
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_kg_nodes_label ON kg_nodes(label);
             CREATE INDEX IF NOT EXISTS idx_kg_edges_path ON kg_edges(source_path);
             DELETE FROM kg_edges WHERE source_id NOT IN (SELECT id FROM kg_nodes) OR target_id NOT IN (SELECT id FROM kg_nodes);",
        )?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO kg_schema_migrations (version, applied_at, description) VALUES ('1.1.0', ?1, 'Phase 9.0.1 integrity indexes and referential sweep')",
            params![now],
        )?;
    }

    if !applied.contains("2.0.0") {
        ensure_column_exists(conn, "kg_nodes", "scope_chain", "TEXT")?;
        ensure_column_exists(
            conn,
            "kg_nodes",
            "resolution_status",
            "TEXT NOT NULL DEFAULT 'RESOLVED'",
        )?;
        ensure_column_exists(conn, "kg_edges", "source_end_line", "INTEGER")?;
        ensure_column_exists(
            conn,
            "kg_edges",
            "resolution_status",
            "TEXT NOT NULL DEFAULT 'RESOLVED'",
        )?;
        ensure_column_exists(conn, "kg_edges", "target_hint", "TEXT")?;
        ensure_column_exists(
            conn,
            "kg_file_fingerprints",
            "parse_status",
            "TEXT NOT NULL DEFAULT 'COMPLETE'",
        )?;
        ensure_column_exists(
            conn,
            "kg_file_fingerprints",
            "analysis_key",
            "TEXT NOT NULL DEFAULT ''",
        )?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS kg_file_extractions (
                relative_path TEXT PRIMARY KEY,
                parse_status TEXT NOT NULL DEFAULT 'COMPLETE',
                rels_json TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_kg_edges_status ON kg_edges(resolution_status);
            CREATE INDEX IF NOT EXISTS idx_kg_nodes_status ON kg_nodes(resolution_status);",
        )?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO kg_schema_migrations (version, applied_at, description) VALUES ('2.0.0', ?1, 'Phase 10.1 resolution status, inherits relation, raw extraction cache and provenance metadata')",
            params![now],
        )?;
    }

    Ok(())
}

fn ensure_column_exists(conn: &Connection, table: &str, column: &str, col_def: &str) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let cols: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .filter_map(Result::ok)
        .collect();
    if !cols.iter().any(|c| c == column) {
        conn.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {col_def};"
        ))?;
    }
    Ok(())
}

pub fn init_graph_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS kg_schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL,
            description TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS kg_nodes (
            id TEXT PRIMARY KEY,
            category TEXT NOT NULL,
            label TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            module_path TEXT,
            scope_chain TEXT,
            start_line INTEGER,
            end_line INTEGER,
            signature TEXT,
            fingerprint TEXT NOT NULL,
            resolution_status TEXT NOT NULL DEFAULT 'RESOLVED',
            metadata_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS kg_edges (
            id TEXT PRIMARY KEY,
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            category TEXT NOT NULL,
            source_path TEXT NOT NULL,
            source_line INTEGER,
            source_end_line INTEGER,
            content_hash TEXT NOT NULL,
            extraction_method TEXT NOT NULL,
            classification TEXT NOT NULL,
            resolution_status TEXT NOT NULL DEFAULT 'RESOLVED',
            confidence REAL NOT NULL,
            target_hint TEXT
        );

        CREATE TABLE IF NOT EXISTS kg_file_fingerprints (
            relative_path TEXT PRIMARY KEY,
            fingerprint TEXT NOT NULL,
            parse_status TEXT NOT NULL DEFAULT 'COMPLETE',
            analysis_key TEXT NOT NULL DEFAULT '',
            last_indexed_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS kg_file_extractions (
            relative_path TEXT PRIMARY KEY,
            parse_status TEXT NOT NULL DEFAULT 'COMPLETE',
            rels_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS kg_metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_kg_nodes_path ON kg_nodes(relative_path);
        CREATE INDEX IF NOT EXISTS idx_kg_nodes_cat ON kg_nodes(category);
        CREATE INDEX IF NOT EXISTS idx_kg_nodes_label ON kg_nodes(label);
        CREATE INDEX IF NOT EXISTS idx_kg_nodes_mod ON kg_nodes(module_path);
        CREATE INDEX IF NOT EXISTS idx_kg_edges_source ON kg_edges(source_id);
        CREATE INDEX IF NOT EXISTS idx_kg_edges_target ON kg_edges(target_id);
        CREATE INDEX IF NOT EXISTS idx_kg_edges_cat ON kg_edges(category);
        CREATE INDEX IF NOT EXISTS idx_kg_edges_path ON kg_edges(source_path);",
    )?;
    Ok(())
}

fn compute_edge_id(source_id: &str, category: RelationCategory, target_id: &str) -> String {
    let input = format!("{source_id}|{}|{target_id}", category.as_str());
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())[..24].to_string()
}

fn normalize_edge_confidence(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn default_provenance(
    source_path: String,
    source_line: Option<usize>,
    content_hash: String,
    extraction_method: &str,
    classification: EvidenceClassification,
    confidence: f64,
    resolution_status: ResolutionStatus,
    target_hint: Option<String>,
) -> EdgeProvenance {
    EdgeProvenance {
        source_path,
        source_line,
        content_hash,
        extraction_method: extraction_method.to_string(),
        classification,
        confidence: normalize_edge_confidence(confidence),
        resolution_status,
        target_hint,
    }
}

fn compute_ignore_fingerprint(root: &Path) -> String {
    let mut hasher = Sha256::new();
    for rel in [".gitignore", ".engramignore", ".git/info/exclude"] {
        let p = root.join(rel);
        hasher.update(rel.as_bytes());
        hasher.update(b"\0");
        if let Ok(bytes) = fs::read(&p) {
            hasher.update(&(bytes.len() as u64).to_be_bytes());
            hasher.update(&bytes);
        } else {
            hasher.update(b"<missing>");
        }
    }
    format!("{:x}", hasher.finalize())
}

fn compute_manifest_fingerprint(root: &Path) -> String {
    let mut hasher = Sha256::new();
    for rel in ["Cargo.toml", "package.json", "go.mod", "pyproject.toml"] {
        let p = root.join(rel);
        hasher.update(rel.as_bytes());
        hasher.update(b"\0");
        if let Ok(bytes) = fs::read(&p) {
            hasher.update(&(bytes.len() as u64).to_be_bytes());
            hasher.update(&bytes);
        } else {
            hasher.update(b"<missing>");
        }
    }
    format!("{:x}", hasher.finalize())
}

fn compute_governance_fingerprint(root: &Path) -> String {
    let mut hasher = Sha256::new();
    for rel in [".engram/constitution.yaml", ".engram/contracts/active.json"] {
        let p = root.join(rel);
        hasher.update(rel.as_bytes());
        hasher.update(b"\0");
        if let Ok(bytes) = fs::read(&p) {
            hasher.update(&(bytes.len() as u64).to_be_bytes());
            hasher.update(&bytes);
        } else {
            hasher.update(b"<missing>");
        }
    }
    format!("{:x}", hasher.finalize())
}

fn compute_source_tree_fingerprint(files: &BTreeMap<String, String>) -> String {
    let mut hasher = Sha256::new();
    for (rel, fp) in files {
        hasher.update(rel.as_bytes());
        hasher.update(b"\0");
        hasher.update(fp.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

fn load_rust_crate_name(root: &Path) -> Option<String> {
    let text = fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let mut in_pkg = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_pkg = line == "[package]";
            continue;
        }
        if in_pkg {
            if let Some(rest) = line.strip_prefix("name") {
                let rest = rest.trim();
                if let Some(val) = rest.strip_prefix('=') {
                    let clean = val.trim().trim_matches(|c| c == '"' || c == '\'');
                    if !clean.is_empty() {
                        return Some(clean.replace('-', "_"));
                    }
                }
            }
        }
    }
    None
}

fn acquire_graph_repo_lock(root: &Path) -> std::sync::Arc<std::sync::Mutex<()>> {
    use std::sync::{Arc, Mutex, OnceLock};
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let map_lock = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut guard = map_lock.lock().unwrap_or_else(|p| p.into_inner());
    guard
        .entry(canon)
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

fn is_corruption_error(msg: &str) -> bool {
    let low = msg.to_ascii_lowercase();
    low.contains("corrupt")
        || low.contains("malformed")
        || low.contains("not a database")
        || low.contains("file is encrypted")
        || low.contains("database disk image is malformed")
}

fn open_graph_db_recovering(root: &Path) -> Result<(Connection, bool)> {
    let db_file = graph_db_path(root)?;
    let needs_reset = if db_file.exists() {
        match open_graph_db(root) {
            Ok(conn) => {
                match conn.query_row("SELECT COUNT(*) FROM kg_nodes", [], |r| r.get::<_, i64>(0)) {
                    Ok(_) => match conn
                        .query_row("PRAGMA quick_check(1);", [], |r| r.get::<_, String>(0))
                    {
                        Ok(s) => !s.eq_ignore_ascii_case("ok"),
                        Err(e) => is_corruption_error(&e.to_string()),
                    },
                    Err(e) => is_corruption_error(&e.to_string()),
                }
            }
            Err(e) => is_corruption_error(&format!("{e:#}")),
        }
    } else {
        false
    };

    if needs_reset {
        let _ = fs::remove_file(&db_file);
        let _ = fs::remove_file(db_file.with_extension("db-wal"));
        let _ = fs::remove_file(db_file.with_extension("db-shm"));
    }
    let conn = open_graph_db(root)?;
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    Ok((conn, needs_reset))
}

#[derive(Debug, Clone)]
struct SymEntry {
    id: String,
    rel_path: String,
    label: String,
    kind: String,
    line: usize,
    end_line: usize,
}

#[derive(Debug, Clone)]
struct FileWorkItem {
    rel_path: String,
    file_id: String,
    fp: String,
    lang: Option<&'static str>,
    symbols: Vec<SymEntry>,
    rels: Vec<crate::analyzer::Relationship>,
}

fn extract_qualifier_and_bare(target: &str) -> (Option<String>, String) {
    let clean = target.trim();
    if let Some((q, b)) = clean.rsplit_once("::") {
        return (Some(q.to_string()), b.to_string());
    }
    if let Some((q, b)) = clean.rsplit_once('.') {
        return (Some(q.to_string()), b.to_string());
    }
    (None, clean.to_string())
}

fn strip_ext(path: &str) -> &str {
    if let Some(dot) = path.rfind('.') {
        let slash = path.rfind('/').unwrap_or(0);
        if dot > slash {
            return &path[..dot];
        }
    }
    path
}

fn emit_dir_chain(
    repo_id: &str,
    rel_path: &str,
    fp: &str,
    nodes_to_insert: &mut BTreeMap<String, GraphNode>,
    edges_to_insert: &mut BTreeMap<String, GraphEdge>,
) {
    let mut parent_id = repo_id.to_string();
    if let Some(parent) = Path::new(rel_path).parent() {
        let mut acc = String::new();
        for comp in parent.components() {
            if let std::path::Component::Normal(seg) = comp {
                let s = seg.to_string_lossy();
                if !acc.is_empty() {
                    acc.push('/');
                }
                acc.push_str(&s);
                let dir_id = format!("dir:{acc}");
                nodes_to_insert
                    .entry(dir_id.clone())
                    .or_insert_with(|| GraphNode {
                        id: dir_id.clone(),
                        category: NodeCategory::Directory,
                        label: s.to_string(),
                        relative_path: "".to_string(),
                        module_path: Some(acc.replace('/', "::")),
                        start_line: None,
                        end_line: None,
                        signature: None,
                        fingerprint: hash(acc.as_bytes()),
                        metadata: serde_json::json!({ "dir": acc }),
                    });
                let cid = compute_edge_id(&parent_id, RelationCategory::Contains, &dir_id);
                edges_to_insert
                    .entry(cid.clone())
                    .or_insert_with(|| GraphEdge {
                        id: cid,
                        source_id: parent_id.clone(),
                        target_id: dir_id.clone(),
                        category: RelationCategory::Contains,
                        provenance: default_provenance(
                            "".to_string(),
                            None,
                            hash(acc.as_bytes()),
                            "filesystem",
                            EvidenceClassification::DirectFact,
                            1.0,
                            ResolutionStatus::Resolved,
                            None,
                        ),
                    });
                parent_id = dir_id;
            }
        }
    }
    let file_id = format!("file:{rel_path}");
    let cid = compute_edge_id(&parent_id, RelationCategory::Contains, &file_id);
    edges_to_insert
        .entry(cid.clone())
        .or_insert_with(|| GraphEdge {
            id: cid,
            source_id: parent_id,
            target_id: file_id,
            category: RelationCategory::Contains,
            provenance: default_provenance(
                rel_path.to_string(),
                None,
                fp.to_string(),
                "filesystem",
                EvidenceClassification::DirectFact,
                1.0,
                ResolutionStatus::Resolved,
                None,
            ),
        });
}

fn insert_placeholder_node(
    nodes_to_insert: &mut BTreeMap<String, GraphNode>,
    id: &str,
    cat: NodeCategory,
    label: &str,
    res_status: ResolutionStatus,
    meta: serde_json::Value,
) {
    nodes_to_insert
        .entry(id.to_string())
        .or_insert_with(|| GraphNode {
            id: id.to_string(),
            category: cat,
            label: label.to_string(),
            relative_path: "".to_string(),
            module_path: None,
            start_line: None,
            end_line: None,
            signature: None,
            fingerprint: hash(id.as_bytes()),
            metadata: serde_json::json!({
                "external": res_status == ResolutionStatus::External,
                "resolution_status": res_status.as_str(),
                "detail": meta
            }),
        });
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
/// Guarantee: when `force_full = false`, detects any config/ignore/schema/repo-root/parser/corruption
/// mismatch and automatically escalates to atomic full rebuild; otherwise performs Tier 1 + Tier 2
/// incremental update producing differential equivalence with `force_full = true`.
pub fn sync_knowledge_graph(root: &Path, force_full: bool) -> Result<GraphStatus> {
    let root = root_path(root)?;
    let repo_lock = acquire_graph_repo_lock(&root);
    let _repo_guard = repo_lock.lock().unwrap_or_else(|p| p.into_inner());
    let start_time = std::time::Instant::now();
    let (mut conn, recovered_corrupt) = open_graph_db_recovering(&root)?;

    let root_norm = crate::discovery::normalize_path(&root)
        .to_string_lossy()
        .replace('\\', "/");
    let root_canon = crate::discovery::normalize_path(
        &fs::canonicalize(&root).unwrap_or_else(|_| root.to_path_buf()),
    );
    let repo_root_hash = hash(root_norm.as_bytes());
    let config_fp = crate::storage::config_hash(&root);
    let ignore_fp = compute_ignore_fingerprint(&root);
    let manifest_fp = compute_manifest_fingerprint(&root);
    let governance_fp = compute_governance_fingerprint(&root);
    let rust_crate_name = load_rust_crate_name(&root);

    let mut meta_map: BTreeMap<String, String> = BTreeMap::new();
    {
        let mut stmt = conn.prepare("SELECT key, value FROM kg_metadata")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            if let Ok((k, v)) = row {
                meta_map.insert(k, v);
            }
        }
    }

    let prev_generation: usize = meta_map
        .get("graph_generation")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let mut effective_full = force_full || recovered_corrupt;
    if !effective_full {
        let meta_valid = meta_map.get("schema_version").map(|s| s.as_str())
            == Some(GRAPH_SCHEMA_VERSION)
            && meta_map.get("repo_root_hash") == Some(&repo_root_hash)
            && meta_map.get("config_fingerprint") == Some(&config_fp)
            && meta_map.get("ignore_fingerprint") == Some(&ignore_fp)
            && meta_map.get("parser_version").map(|s| s.as_str()) == Some(GRAPH_PARSER_VERSION)
            && meta_map.get("manifest_fingerprint") == Some(&manifest_fp)
            && matches!(
                meta_map.get("state").map(|s| s.as_str()),
                Some("READY") | Some("PARTIALLY_INDEXED")
            );
        if !meta_valid {
            effective_full = true;
        }
    }

    let mut stored_fingerprints: BTreeMap<String, String> = BTreeMap::new();
    if !effective_full {
        let fp_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM kg_file_fingerprints", [], |r| {
                r.get(0)
            })?;
        let ext_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM kg_file_extractions", [], |r| r.get(0))?;
        let root_exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM kg_nodes WHERE id = 'repo:root'",
            [],
            |r| r.get(0),
        )?;
        if fp_count != ext_count || root_exists == 0 {
            effective_full = true;
        } else {
            let mut stmt =
                conn.prepare("SELECT relative_path, fingerprint FROM kg_file_fingerprints")?;
            let rows =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            for row in rows {
                let (p, fp) = row?;
                stored_fingerprints.insert(p, fp);
            }
        }
    }

    // Discover repository source files with symlink escape containment protection
    let mut current_files: BTreeMap<String, String> = BTreeMap::new();
    let mut failed_files_count = 0usize;
    let mut partial_files_count = 0usize;
    let mut unsupported_files_count = 0usize;
    let mut last_error: Option<String> = None;

    let engine = crate::discovery::DiscoveryEngine::new_shared(&root);
    for entry in engine.build_walk().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() {
            if engine.is_excluded(path, false) {
                continue;
            }
            // Enforce canonical workspace boundary to reject escaping symlinks
            if let Ok(canon_file) = fs::canonicalize(path) {
                let norm_canon = crate::discovery::normalize_path(&canon_file);
                if !norm_canon.starts_with(&root_canon) {
                    continue;
                }
            } else {
                continue;
            }

            let p_norm = crate::discovery::normalize_path(path);
            if let Ok(rel) = p_norm
                .strip_prefix(engine.root())
                .or_else(|_| path.strip_prefix(&root))
            {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let is_code = crate::analyzer::language(path).is_some();
                let is_doc = rel_str.ends_with(".md");
                let is_cfg = rel_str.ends_with(".toml")
                    || rel_str.ends_with(".yaml")
                    || rel_str.ends_with(".yml");

                if !is_code && !is_doc && !is_cfg {
                    unsupported_files_count += 1;
                    continue;
                }

                match fs::read(path) {
                    Ok(bytes) => {
                        if bytes.len() > 1_000_000 || bytes.contains(&0) {
                            unsupported_files_count += 1;
                            continue;
                        }
                        current_files.insert(rel_str, hash(&bytes));
                    }
                    Err(e) => {
                        failed_files_count += 1;
                        last_error = Some(format!("Failed to read '{}': {e}", path.display()));
                    }
                }
            }
        }
    }

    let source_tree_fp = compute_source_tree_fingerprint(&current_files);

    let mut added: Vec<String> = Vec::new();
    let mut modified: Vec<String> = Vec::new();
    let mut deleted: Vec<String> = Vec::new();

    if effective_full {
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
    added.sort();
    modified.sort();
    deleted.sort();

    // Fast clean exit when literally nothing changed and graph state is READY
    if !effective_full
        && added.is_empty()
        && modified.is_empty()
        && deleted.is_empty()
        && failed_files_count == 0
        && meta_map.get("source_tree_fingerprint") == Some(&source_tree_fp)
        && meta_map.get("governance_fingerprint") == Some(&governance_fp)
        && meta_map.get("state").map(|s| s.as_str()) == Some("READY")
    {
        let node_count: usize =
            conn.query_row("SELECT COUNT(*) FROM kg_nodes", [], |r| r.get(0))?;
        let edge_count: usize =
            conn.query_row("SELECT COUNT(*) FROM kg_edges", [], |r| r.get(0))?;
        return Ok(GraphStatus {
            state: GraphLifecycleState::Ready,
            schema_version: GRAPH_SCHEMA_VERSION.to_string(),
            index_generation: prev_generation.max(1),
            node_count,
            edge_count,
            last_indexed_commit: meta_map.get("last_indexed_commit").cloned(),
            indexed_files_count: current_files.len(),
            stale_files_count: 0,
            failed_files_count: 0,
            unsupported_files_count,
            last_error: None,
        });
    }

    // Load pre-existing symbols, modules & extractions when incremental so we can compute Tier 2
    // and ensure differential equivalence with full rebuild without re-parsing unchanged files.
    let tier1_changed_paths: BTreeSet<String> = added
        .iter()
        .chain(modified.iter())
        .chain(deleted.iter())
        .cloned()
        .collect();
    let files_to_remove: BTreeSet<String> =
        deleted.iter().chain(modified.iter()).cloned().collect();

    let mut surviving_symbols_by_file: BTreeMap<String, Vec<SymEntry>> = BTreeMap::new();
    let mut old_changed_symbol_names: BTreeSet<String> = BTreeSet::new();
    let mut old_changed_module_names: BTreeSet<String> = BTreeSet::new();
    let mut surviving_extractions: BTreeMap<String, (String, Vec<crate::analyzer::Relationship>)> =
        BTreeMap::new();
    let mut surviving_edges_by_src_path: BTreeMap<
        String,
        Vec<(String, String, String, Option<String>, String)>,
    > = BTreeMap::new();
    let mut node_id_to_rel_path: BTreeMap<String, String> = BTreeMap::new();
    let mut node_id_to_label: BTreeMap<String, String> = BTreeMap::new();

    if !effective_full {
        let mut corrupt_cache = false;
        {
            let mut stmt = conn.prepare(
                "SELECT id, category, label, relative_path, start_line, end_line, metadata_json
                 FROM kg_nodes",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                    r.get::<_, String>(6)?,
                ))
            })?;
            for row in rows {
                let (id, cat, label, rel_path, sl, el, meta_str) = row?;
                node_id_to_rel_path.insert(id.clone(), rel_path.clone());
                node_id_to_label.insert(id.clone(), label.clone());
                let is_sym = matches!(
                    cat.as_str(),
                    "SYMBOL" | "TYPE" | "FUNCTION" | "METHOD" | "API_ENDPOINT"
                );
                if is_sym && !rel_path.is_empty() {
                    if files_to_remove.contains(&rel_path) {
                        old_changed_symbol_names.insert(label);
                    } else {
                        let meta_val: serde_json::Value =
                            serde_json::from_str(&meta_str).unwrap_or(serde_json::Value::Null);
                        let kind = meta_val
                            .get("kind")
                            .and_then(|k| k.as_str())
                            .unwrap_or(&cat)
                            .to_string();
                        surviving_symbols_by_file
                            .entry(rel_path.clone())
                            .or_default()
                            .push(SymEntry {
                                id,
                                rel_path,
                                label,
                                kind,
                                line: sl.unwrap_or(1) as usize,
                                end_line: el.unwrap_or(1) as usize,
                            });
                    }
                }
            }
        }

        // Load cached raw extractions for surviving files
        {
            let mut stmt = conn.prepare(
                "SELECT relative_path, parse_status, rels_json FROM kg_file_extractions",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?;
            for row in rows {
                let (rel_path, pstatus, rels_json) = row?;
                match serde_json::from_str::<Vec<crate::analyzer::Relationship>>(&rels_json) {
                    Ok(rels) => {
                        if files_to_remove.contains(&rel_path) {
                            for r in &rels {
                                if r.kind == "DECLARES_MODULE" {
                                    old_changed_module_names.insert(r.target.clone());
                                }
                            }
                        } else if current_files.contains_key(&rel_path) {
                            surviving_extractions.insert(rel_path, (pstatus, rels));
                        }
                    }
                    Err(_) => {
                        corrupt_cache = true;
                        break;
                    }
                }
            }
        }

        if corrupt_cache {
            effective_full = true;
            added = current_files.keys().cloned().collect();
            modified.clear();
            deleted.clear();
            surviving_symbols_by_file.clear();
            surviving_extractions.clear();
        } else {
            let mut stmt = conn.prepare(
                "SELECT source_path, target_id, category, resolution_status, target_hint
                 FROM kg_edges",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            })?;
            for row in rows {
                let (sp, tid, cat, res_status, thint) = row?;
                if !tier1_changed_paths.contains(&sp) && current_files.contains_key(&sp) {
                    surviving_edges_by_src_path.entry(sp).or_default().push((
                        tid,
                        cat,
                        res_status,
                        thint,
                        String::new(),
                    ));
                }
            }
        }
    }

    let files_to_parse: Vec<String> = if effective_full {
        current_files.keys().cloned().collect()
    } else {
        added.iter().chain(modified.iter()).cloned().collect()
    };

    let mut nodes_to_insert: BTreeMap<String, GraphNode> = BTreeMap::new();
    let mut edges_to_insert: BTreeMap<String, GraphEdge> = BTreeMap::new();
    let mut parsed_extractions_to_save: BTreeMap<
        String,
        (String, Vec<crate::analyzer::Relationship>),
    > = BTreeMap::new();
    let mut file_parse_status_updates: BTreeMap<String, String> = BTreeMap::new();

    // Ensure Repository root node always up-to-date
    let repo_id = "repo:root".to_string();
    nodes_to_insert.insert(
        repo_id.clone(),
        GraphNode {
            id: repo_id.clone(),
            category: NodeCategory::Repository,
            label: "RepositoryRoot".to_string(),
            relative_path: "".to_string(),
            module_path: None,
            start_line: None,
            end_line: None,
            signature: None,
            fingerprint: "repo".to_string(),
            metadata: serde_json::json!({
                "root": root_norm,
                "schema_version": GRAPH_SCHEMA_VERSION
            }),
        },
    );

    let mut tier1_items: Vec<FileWorkItem> = Vec::new();
    let mut new_changed_symbol_names: BTreeSet<String> = BTreeSet::new();
    let mut new_changed_module_names: BTreeSet<String> = BTreeSet::new();

    // Pass 1: Parse Tier 1 files (all files when full, added+modified when incremental)
    for rel_path in &files_to_parse {
        let abs_path = root.join(rel_path);
        let fp = current_files.get(rel_path).cloned().unwrap_or_default();
        let file_id = format!("file:{rel_path}");
        let lang = crate::analyzer::language(&abs_path);

        let file_name = Path::new(rel_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let is_test = rel_path.starts_with("tests/")
            || rel_path.contains("/tests/")
            || file_name.contains(".test.")
            || file_name.contains(".spec.")
            || file_name.starts_with("test_")
            || file_name.ends_with("_test.py")
            || file_name.ends_with("_test.rs")
            || file_name.ends_with("Tests.cs")
            || file_name.ends_with("Test.cs")
            || rel_path.contains("test");
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

        let mod_path = Path::new(rel_path)
            .parent()
            .map(|p| {
                let s = p.to_string_lossy().replace('\\', "/");
                if s.is_empty() {
                    "root".to_string()
                } else {
                    s.replace('/', "::")
                }
            })
            .unwrap_or_else(|| "root".to_string());

        nodes_to_insert.insert(
            file_id.clone(),
            GraphNode {
                id: file_id.clone(),
                category: cat,
                label: if file_name.is_empty() {
                    rel_path.clone()
                } else {
                    file_name.to_string()
                },
                relative_path: rel_path.clone(),
                module_path: Some(mod_path.clone()),
                start_line: Some(1),
                end_line: None,
                signature: None,
                fingerprint: fp.clone(),
                metadata: serde_json::json!({ "is_test": is_test, "is_doc": is_doc }),
            },
        );
        emit_dir_chain(
            &repo_id,
            rel_path,
            &fp,
            &mut nodes_to_insert,
            &mut edges_to_insert,
        );

        let mut sym_entries: Vec<SymEntry> = Vec::new();
        let mut raw_rels: Vec<crate::analyzer::Relationship> = Vec::new();
        let mut parse_status_str = "COMPLETE".to_string();

        if lang.is_some() {
            match fs::read(&abs_path) {
                Ok(code) => match crate::analyzer::parse_file_ast(&root, rel_path, &code, "head") {
                    Ok(mut parsed) => {
                        parse_status_str = parsed.parse_status.as_str().to_string();
                        if parsed.parse_status == crate::storage::ParseStatus::Partial {
                            partial_files_count += 1;
                        }
                        parsed.symbols.sort_by(|a, b| {
                            a.line
                                .cmp(&b.line)
                                .then_with(|| a.end_line.cmp(&b.end_line))
                                .then_with(|| a.kind.cmp(&b.kind))
                                .then_with(|| a.name.cmp(&b.name))
                                .then_with(|| a.signature.cmp(&b.signature))
                        });

                        let mut dup_counts: BTreeMap<(String, String, String), usize> =
                            BTreeMap::new();
                        for sym in &parsed.symbols {
                            let kind_up = sym.kind.to_ascii_uppercase();
                            let fp8 = sym.fingerprint[..8.min(sym.fingerprint.len())].to_string();
                            let key = (kind_up.clone(), sym.name.clone(), fp8.clone());
                            let ord = dup_counts.entry(key).or_insert(0);
                            *ord += 1;
                            let sym_id = if *ord == 1 {
                                format!("symbol:{rel_path}::{kind_up}::{}::{fp8}", sym.name)
                            } else {
                                format!("symbol:{rel_path}::{kind_up}::{}::{fp8}#{ord}", sym.name)
                            };

                            new_changed_symbol_names.insert(sym.name.clone());

                            let sym_cat = match kind_up.as_str() {
                                "FUNCTION" => NodeCategory::Function,
                                "METHOD" | "CONSTRUCTOR" => NodeCategory::Method,
                                "STRUCT" | "CLASS" | "TYPE" | "INTERFACE" | "TRAIT" | "ENUM"
                                | "RECORD" | "IMPL" => NodeCategory::Type,
                                _ => NodeCategory::Symbol,
                            };

                            nodes_to_insert.insert(
                                sym_id.clone(),
                                GraphNode {
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
                                },
                            );

                            let decl_id =
                                compute_edge_id(&file_id, RelationCategory::Declares, &sym_id);
                            edges_to_insert.insert(
                                decl_id.clone(),
                                GraphEdge {
                                    id: decl_id,
                                    source_id: file_id.clone(),
                                    target_id: sym_id.clone(),
                                    category: RelationCategory::Declares,
                                    provenance: default_provenance(
                                        rel_path.clone(),
                                        Some(sym.line),
                                        sym.fingerprint.clone(),
                                        "ast_tree_sitter",
                                        EvidenceClassification::DirectFact,
                                        1.0,
                                        ResolutionStatus::Resolved,
                                        None,
                                    ),
                                },
                            );

                            sym_entries.push(SymEntry {
                                id: sym_id,
                                rel_path: rel_path.clone(),
                                label: sym.name.clone(),
                                kind: sym.kind.clone(),
                                line: sym.line,
                                end_line: sym.end_line,
                            });
                        }

                        parsed.relationships.sort_by(|a, b| {
                            a.line
                                .cmp(&b.line)
                                .then_with(|| a.kind.cmp(&b.kind))
                                .then_with(|| a.target.cmp(&b.target))
                                .then_with(|| a.evidence.cmp(&b.evidence))
                        });
                        for r in &parsed.relationships {
                            if r.kind == "DECLARES_MODULE" {
                                new_changed_module_names.insert(r.target.clone());
                            }
                        }
                        raw_rels = parsed.relationships;
                    }
                    Err(e) => {
                        failed_files_count += 1;
                        parse_status_str = "FAILED".to_string();
                        last_error = Some(format!("Failed to parse '{rel_path}': {e}"));
                    }
                },
                Err(e) => {
                    failed_files_count += 1;
                    parse_status_str = "FAILED".to_string();
                    last_error = Some(format!("Failed to read '{rel_path}': {e}"));
                }
            }
        }

        file_parse_status_updates.insert(rel_path.clone(), parse_status_str.clone());
        parsed_extractions_to_save.insert(rel_path.clone(), (parse_status_str, raw_rels.clone()));
        tier1_items.push(FileWorkItem {
            rel_path: rel_path.clone(),
            file_id,
            fp,
            lang,
            symbols: sym_entries,
            rels: raw_rels,
        });
    }

    // Build complete global lookup maps for Pass 2 (Tier 1 + surviving unchanged files)
    let mut symbol_name_to_entries: BTreeMap<String, Vec<SymEntry>> = BTreeMap::new();
    let mut symbol_lower_to_entries: BTreeMap<String, Vec<SymEntry>> = BTreeMap::new();
    let mut file_symbols_map: BTreeMap<String, Vec<SymEntry>> = BTreeMap::new();

    for (rel_path, syms) in &surviving_symbols_by_file {
        if current_files.contains_key(rel_path) && !tier1_changed_paths.contains(rel_path) {
            file_symbols_map.insert(rel_path.clone(), syms.clone());
            for s in syms {
                symbol_name_to_entries
                    .entry(s.label.clone())
                    .or_default()
                    .push(s.clone());
                symbol_lower_to_entries
                    .entry(s.label.to_ascii_lowercase())
                    .or_default()
                    .push(s.clone());
            }
        }
    }
    for item in &tier1_items {
        file_symbols_map.insert(item.rel_path.clone(), item.symbols.clone());
        for s in &item.symbols {
            symbol_name_to_entries
                .entry(s.label.clone())
                .or_default()
                .push(s.clone());
            symbol_lower_to_entries
                .entry(s.label.to_ascii_lowercase())
                .or_default()
                .push(s.clone());
        }
    }
    for list in symbol_name_to_entries.values_mut() {
        list.sort_by(|a, b| a.id.cmp(&b.id));
    }
    for list in symbol_lower_to_entries.values_mut() {
        list.sort_by(|a, b| a.id.cmp(&b.id));
    }

    let all_changed_sym_names: BTreeSet<String> = old_changed_symbol_names
        .union(&new_changed_symbol_names)
        .cloned()
        .collect();
    let all_changed_mod_names: BTreeSet<String> = old_changed_module_names
        .union(&new_changed_module_names)
        .cloned()
        .collect();
    let mut changed_stems: BTreeSet<String> = BTreeSet::new();
    for p in &tier1_changed_paths {
        changed_stems.insert(p.to_ascii_lowercase());
        let no_ext = strip_ext(p).to_ascii_lowercase();
        changed_stems.insert(no_ext);
        if let Some(fname) = Path::new(p).file_name().and_then(|s| s.to_str()) {
            changed_stems.insert(fname.to_ascii_lowercase());
            if let Some(stem) = Path::new(p).file_stem().and_then(|s| s.to_str()) {
                changed_stems.insert(stem.to_ascii_lowercase());
            }
        }
    }
    let has_go_change = tier1_changed_paths.iter().any(|p| p.ends_with(".go"));

    // Determine Tier 2 surviving files requiring edge re-resolution
    let mut tier2_files: BTreeSet<String> = BTreeSet::new();
    if !effective_full {
        for (src_path, edges) in &surviving_edges_by_src_path {
            if has_go_change && src_path.ends_with(".go") {
                tier2_files.insert(src_path.clone());
                continue;
            }
            for (tid, _cat, res_status, thint, _) in edges {
                if let Some(tgt_rel) = node_id_to_rel_path.get(tid) {
                    if !tgt_rel.is_empty() && tier1_changed_paths.contains(tgt_rel) {
                        tier2_files.insert(src_path.clone());
                        break;
                    }
                }
                if let Some(tlabel) = node_id_to_label.get(tid) {
                    if all_changed_sym_names.contains(tlabel)
                        || all_changed_mod_names.contains(tlabel)
                    {
                        tier2_files.insert(src_path.clone());
                        break;
                    }
                }
                if res_status != "RESOLVED"
                    || tid.starts_with("unresolved:")
                    || tid.starts_with("ext:")
                    || tid.starts_with("symbol:ext::")
                    || tid.starts_with("file:ext::")
                    || tid.starts_with("mod:")
                {
                    let tid_low = tid.to_ascii_lowercase();
                    let hint_low = thint.as_deref().unwrap_or("").to_ascii_lowercase();
                    let bare_hint = extract_qualifier_and_bare(thint.as_deref().unwrap_or(tid))
                        .1
                        .to_ascii_lowercase();

                    if all_changed_sym_names.iter().any(|s| {
                        let sl = s.to_ascii_lowercase();
                        sl == bare_hint || tid_low.ends_with(&sl) || hint_low == sl
                    }) || all_changed_mod_names.iter().any(|m| {
                        let ml = m.to_ascii_lowercase();
                        tid_low.contains(&ml) || hint_low.contains(&ml)
                    }) || changed_stems
                        .iter()
                        .any(|st| tid_low.contains(st) || hint_low.contains(st))
                    {
                        tier2_files.insert(src_path.clone());
                        break;
                    }
                }
            }
        }

        // Propagate transitive re-export dependents until fixed point
        loop {
            let mut added_any = false;
            for (src_path, (_ps, rels)) in &surviving_extractions {
                if tier1_changed_paths.contains(src_path) || tier2_files.contains(src_path) {
                    continue;
                }
                for r in rels {
                    if r.kind == "IMPORTS" || r.kind == "IMPORTS_SYMBOL" {
                        let base_t = r.target.split("::").next().unwrap_or(&r.target);
                        if tier1_changed_paths.contains(base_t) || tier2_files.contains(base_t) {
                            if let Some((_, tgt_rels)) = surviving_extractions.get(base_t) {
                                if tgt_rels
                                    .iter()
                                    .any(|tr| tr.evidence.starts_with("TREE_SITTER_REEXPORT"))
                                {
                                    tier2_files.insert(src_path.clone());
                                    added_any = true;
                                    break;
                                }
                            } else {
                                tier2_files.insert(src_path.clone());
                                added_any = true;
                                break;
                            }
                        }
                    }
                }
            }
            if !added_any {
                break;
            }
        }
    }

    // Build all_module_declaring_files & global reexport map across ALL current files
    let mut module_to_files: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut reexport_map: BTreeMap<String, Vec<(Option<String>, String)>> = BTreeMap::new();

    for (rel_path, (_, rels)) in &surviving_extractions {
        if !tier1_changed_paths.contains(rel_path) {
            for r in rels {
                if r.kind == "DECLARES_MODULE" {
                    module_to_files
                        .entry(r.target.clone())
                        .or_default()
                        .insert(rel_path.clone());
                } else if r.evidence == "TREE_SITTER_REEXPORT" {
                    reexport_map
                        .entry(rel_path.clone())
                        .or_default()
                        .push((None, r.target.clone()));
                } else if r.evidence == "TREE_SITTER_REEXPORT_SYMBOL" {
                    if let Some((f, s)) = r.target.split_once("::") {
                        reexport_map
                            .entry(rel_path.clone())
                            .or_default()
                            .push((Some(s.to_string()), f.to_string()));
                    }
                }
            }
        }
    }
    for item in &tier1_items {
        for r in &item.rels {
            if r.kind == "DECLARES_MODULE" {
                module_to_files
                    .entry(r.target.clone())
                    .or_default()
                    .insert(item.rel_path.clone());
            } else if r.evidence == "TREE_SITTER_REEXPORT" {
                reexport_map
                    .entry(item.rel_path.clone())
                    .or_default()
                    .push((None, r.target.clone()));
            } else if r.evidence == "TREE_SITTER_REEXPORT_SYMBOL" {
                if let Some((f, s)) = r.target.split_once("::") {
                    reexport_map
                        .entry(item.rel_path.clone())
                        .or_default()
                        .push((Some(s.to_string()), f.to_string()));
                }
            }
        }
    }

    let mut file_lookup_lower: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in current_files.keys() {
        file_lookup_lower
            .entry(p.to_ascii_lowercase())
            .or_default()
            .push(p.clone());
        let no_ext = strip_ext(p).to_ascii_lowercase();
        file_lookup_lower.entry(no_ext).or_default().push(p.clone());
        if let Some(fname) = Path::new(p).file_name().and_then(|s| s.to_str()) {
            file_lookup_lower
                .entry(fname.to_ascii_lowercase())
                .or_default()
                .push(p.clone());
        }
    }
    for list in file_lookup_lower.values_mut() {
        list.sort();
        list.dedup();
    }

    // Assemble Pass 2 worklist: Tier 1 items + Tier 2 items sorted canonically.
    // When files are added, deleted, or renamed, re-run AST path enrichment for affected Tier 2 files
    // so path-dependent import hints stay bit-for-bit identical to a full rebuild.
    let fs_topology_changed = !added.is_empty() || !deleted.is_empty();
    let mut pass2_items: Vec<FileWorkItem> = tier1_items;
    for t2_path in &tier2_files {
        let abs = root.join(t2_path);
        let fp = current_files.get(t2_path).cloned().unwrap_or_default();
        let lang = crate::analyzer::language(&abs);

        let refreshed = if fs_topology_changed && lang.is_some() {
            fs::read(&abs)
                .ok()
                .and_then(|bytes| {
                    crate::analyzer::parse_file_ast(&root, t2_path, &bytes, "head").ok()
                })
                .map(|parsed| {
                    let ps = parsed.parse_status.as_str().to_string();
                    parsed_extractions_to_save
                        .insert(t2_path.clone(), (ps.clone(), parsed.relationships.clone()));
                    (ps, parsed.relationships)
                })
        } else {
            None
        };

        if let Some((_, rels)) = refreshed.or_else(|| surviving_extractions.get(t2_path).cloned()) {
            let mut sorted_rels = rels;
            sorted_rels.sort_by(|a, b| {
                a.line
                    .cmp(&b.line)
                    .then_with(|| a.kind.cmp(&b.kind))
                    .then_with(|| a.target.cmp(&b.target))
                    .then_with(|| a.evidence.cmp(&b.evidence))
            });
            pass2_items.push(FileWorkItem {
                rel_path: t2_path.clone(),
                file_id: format!("file:{t2_path}"),
                fp,
                lang,
                symbols: file_symbols_map.get(t2_path).cloned().unwrap_or_default(),
                rels: sorted_rels,
            });
        }
    }
    pass2_items.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    let resolve_workspace_file = |spec: &str| -> Option<ResolutionMatch> {
        let norm = spec.replace('\\', "/");
        if current_files.contains_key(&norm) {
            return Some(ResolutionMatch::Unique(norm));
        }
        // Check stripped crate name (`use my_crate::foo::Bar`)
        if let Some(ref crate_nm) = rust_crate_name {
            let prefix = format!("{crate_nm}::");
            if let Some(rest) = norm.strip_prefix(&prefix) {
                let segs: Vec<&str> = rest.split("::").collect();
                for len in (1..=segs.len()).rev() {
                    let sub = segs[..len].join("/");
                    for cand in [format!("src/{sub}.rs"), format!("src/{sub}/mod.rs")] {
                        if current_files.contains_key(&cand) {
                            return Some(ResolutionMatch::Unique(cand));
                        }
                    }
                }
            }
        }
        // Check Python dotted module against workspace `.py` / `__init__.py`
        if !norm.contains('/')
            && norm.contains('.')
            && !norm.ends_with(".rs")
            && !norm.ends_with(".ts")
        {
            let mod_slash = norm.replace('.', "/");
            for prefix in ["", "src/", "app/"] {
                for suffix in [".py", "/__init__.py"] {
                    let cand = format!("{prefix}{mod_slash}{suffix}");
                    if current_files.contains_key(&cand) {
                        return Some(ResolutionMatch::Unique(cand));
                    }
                }
            }
        }
        // Exact lowercase match or unique filename/stem match
        let low = norm.to_ascii_lowercase();
        if let Some(cands) = file_lookup_lower.get(&low) {
            if cands.len() == 1 {
                return Some(ResolutionMatch::Unique(cands[0].clone()));
            } else if cands.len() > 1 {
                return Some(ResolutionMatch::Ambiguous(cands.clone()));
            }
        }
        None
    };

    // Pass 2: Connect relationships deterministically with strict ambiguity/unresolved discipline
    for info in &pass2_items {
        let lang_str = info.lang.unwrap_or("unknown");
        let is_test_source = {
            let fname = Path::new(&info.rel_path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            info.rel_path.starts_with("tests/")
                || info.rel_path.contains("/tests/")
                || fname.contains(".test.")
                || fname.contains(".spec.")
                || fname.starts_with("test_")
                || fname.ends_with("_test.py")
                || fname.ends_with("_test.rs")
                || fname.ends_with("Tests.cs")
                || fname.ends_with("Test.cs")
        };

        // Precompute direct imported files, explicitly imported symbol hints, and imported modules
        let mut imported_files: BTreeSet<String> = BTreeSet::new();
        let mut imported_modules: BTreeSet<String> = BTreeSet::new();
        let mut explicit_sym_imports: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

        for r in &info.rels {
            match r.kind.as_str() {
                "IMPORTS" => {
                    if let Some(ResolutionMatch::Unique(wf)) = resolve_workspace_file(&r.target) {
                        imported_files.insert(wf);
                    } else if module_to_files.contains_key(&r.target) {
                        imported_modules.insert(r.target.clone());
                        if let Some(mfiles) = module_to_files.get(&r.target) {
                            for mf in mfiles {
                                imported_files.insert(mf.clone());
                            }
                        }
                    }
                }
                "IMPORTS_SYMBOL" => {
                    if let Some((f_or_mod, sym)) = r.target.split_once("::") {
                        let resolved_file = match resolve_workspace_file(f_or_mod) {
                            Some(ResolutionMatch::Unique(wf)) => Some(wf),
                            _ => None,
                        };
                        if let Some(rf) = resolved_file {
                            imported_files.insert(rf.clone());
                            explicit_sym_imports
                                .entry(sym.to_string())
                                .or_default()
                                .insert(rf);
                        } else if f_or_mod.starts_with("crate::")
                            || f_or_mod.starts_with("self::")
                            || f_or_mod.starts_with("super::")
                        {
                            if let Some((_, tail)) = r.target.rsplit_once("::") {
                                explicit_sym_imports.entry(tail.to_string()).or_default();
                            }
                        } else if let Some(ref crate_nm) = rust_crate_name {
                            if r.target.starts_with(&format!("{crate_nm}::")) {
                                if let Some(ResolutionMatch::Unique(wf)) =
                                    resolve_workspace_file(&r.target)
                                {
                                    imported_files.insert(wf.clone());
                                    if let Some((_, tail)) = r.target.rsplit_once("::") {
                                        explicit_sym_imports
                                            .entry(tail.to_string())
                                            .or_default()
                                            .insert(wf);
                                    }
                                }
                            }
                        }
                    }
                }
                "DECLARES_MODULE" => {
                    imported_modules.insert(r.target.clone());
                    if let Some(mfiles) = module_to_files.get(&r.target) {
                        for mf in mfiles {
                            imported_files.insert(mf.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        // Expand barrel re-exports for imported files up to depth 4
        let follow_barrel_symbol = |start_file: &str, sym_name: &str| -> BTreeSet<String> {
            let mut visited: BTreeSet<String> = BTreeSet::new();
            let mut queue: VecDeque<(String, usize)> = VecDeque::new();
            let mut hits: BTreeSet<String> = BTreeSet::new();
            queue.push_back((start_file.to_string(), 0));
            while let Some((cur, depth)) = queue.pop_front() {
                if !visited.insert(cur.clone()) {
                    continue;
                }
                if let Some(syms) = file_symbols_map.get(&cur) {
                    let non_impl: Vec<&SymEntry> = syms
                        .iter()
                        .filter(|s| s.label == sym_name && !s.kind.eq_ignore_ascii_case("IMPL"))
                        .collect();
                    if !non_impl.is_empty() {
                        for s in non_impl {
                            hits.insert(s.id.clone());
                        }
                    } else {
                        for s in syms {
                            if s.label == sym_name {
                                hits.insert(s.id.clone());
                            }
                        }
                    }
                }
                if depth >= 4 {
                    continue;
                }
                if let Some(reexps) = reexport_map.get(&cur) {
                    for (opt_sym, next_raw) in reexps {
                        if opt_sym.as_deref() == Some(sym_name) || opt_sym.is_none() {
                            if let Some(ResolutionMatch::Unique(next_f)) =
                                resolve_workspace_file(next_raw)
                            {
                                queue.push_back((next_f, depth + 1));
                            }
                        }
                    }
                }
            }
            hits
        };

        for r in &info.rels {
            let target_clean = r.target.trim();
            if target_clean.is_empty() {
                continue;
            }

            if r.kind == "DECLARES_MODULE" {
                let mod_id = format!("mod:{lang_str}:{target_clean}");
                nodes_to_insert
                    .entry(mod_id.clone())
                    .or_insert_with(|| GraphNode {
                        id: mod_id.clone(),
                        category: NodeCategory::Module,
                        label: target_clean.to_string(),
                        relative_path: "".to_string(),
                        module_path: Some(target_clean.to_string()),
                        start_line: None,
                        end_line: None,
                        signature: None,
                        fingerprint: hash(mod_id.as_bytes()),
                        metadata: serde_json::json!({ "language": lang_str }),
                    });
                let eid = compute_edge_id(&info.file_id, RelationCategory::Declares, &mod_id);
                edges_to_insert
                    .entry(eid.clone())
                    .or_insert_with(|| GraphEdge {
                        id: eid,
                        source_id: info.file_id.clone(),
                        target_id: mod_id,
                        category: RelationCategory::Declares,
                        provenance: default_provenance(
                            info.rel_path.clone(),
                            Some(r.line),
                            info.fp.clone(),
                            &r.evidence,
                            EvidenceClassification::DirectFact,
                            r.confidence,
                            ResolutionStatus::Resolved,
                            Some(target_clean.to_string()),
                        ),
                    });
                continue;
            }

            let mut edge_cat = match r.kind.to_ascii_uppercase().as_str() {
                "CALLS" => RelationCategory::Calls,
                "IMPORTS" | "IMPORTS_SYMBOL" | "USE" => RelationCategory::Imports,
                "IMPLEMENTS" => RelationCategory::Implements,
                "INHERITS" => RelationCategory::Inherits,
                "EXTENDS" => RelationCategory::Extends,
                "TESTS" => RelationCategory::Tests,
                _ => RelationCategory::References,
            };

            let enclosing_caller = |prefer_types: bool| -> String {
                if prefer_types {
                    info.symbols
                        .iter()
                        .filter(|s| {
                            s.line <= r.line
                                && s.end_line >= r.line
                                && matches!(
                                    s.kind.to_ascii_uppercase().as_str(),
                                    "CLASS" | "STRUCT" | "INTERFACE" | "TRAIT" | "IMPL" | "RECORD"
                                )
                        })
                        .min_by_key(|s| s.end_line.saturating_sub(s.line))
                        .map(|s| s.id.clone())
                } else {
                    info.symbols
                        .iter()
                        .filter(|s| {
                            s.line <= r.line
                                && s.end_line >= r.line
                                && matches!(
                                    s.kind.to_ascii_uppercase().as_str(),
                                    "FUNCTION" | "METHOD" | "CONSTRUCTOR"
                                )
                        })
                        .min_by_key(|s| s.end_line.saturating_sub(s.line))
                        .map(|s| s.id.clone())
                }
                .or_else(|| {
                    info.symbols
                        .iter()
                        .filter(|s| s.line <= r.line && s.end_line >= r.line)
                        .min_by_key(|s| s.end_line.saturating_sub(s.line))
                        .map(|s| s.id.clone())
                })
                .unwrap_or_else(|| info.file_id.clone())
            };

            let (source_node_id, target_node_id, res_status, classification, confidence, hint_opt) =
                if r.kind == "IMPORTS_SYMBOL" {
                    let (file_part, sym_name) = if let Some((f, s)) = target_clean.split_once("::")
                    {
                        (f.to_string(), s.to_string())
                    } else {
                        (String::new(), target_clean.to_string())
                    };
                    if let Some(ResolutionMatch::Unique(wf)) = resolve_workspace_file(&file_part) {
                        let sym_hits = follow_barrel_symbol(&wf, &sym_name);
                        if sym_hits.len() == 1 {
                            let sid = sym_hits.into_iter().next().unwrap();
                            (
                                info.file_id.clone(),
                                sid,
                                ResolutionStatus::Resolved,
                                EvidenceClassification::DirectFact,
                                0.98,
                                Some(target_clean.to_string()),
                            )
                        } else if sym_hits.len() > 1 {
                            let amb_id = format!("unresolved:ambiguous:{target_clean}");
                            insert_placeholder_node(
                                &mut nodes_to_insert,
                                &amb_id,
                                NodeCategory::Symbol,
                                &sym_name,
                                ResolutionStatus::Ambiguous,
                                serde_json::json!({ "candidate_count": sym_hits.len() }),
                            );
                            (
                                info.file_id.clone(),
                                amb_id,
                                ResolutionStatus::Ambiguous,
                                EvidenceClassification::Inferred,
                                0.45,
                                Some(target_clean.to_string()),
                            )
                        } else {
                            // File resolved, specific symbol not directly AST-matched -> partially resolved to file
                            (
                                info.file_id.clone(),
                                format!("file:{wf}"),
                                ResolutionStatus::PartiallyResolved,
                                EvidenceClassification::DerivedFact,
                                0.82,
                                Some(target_clean.to_string()),
                            )
                        }
                    } else if r.is_external
                        || crate::ast_deps::is_known_external_package(&file_part, lang_str)
                    {
                        let ext_id = format!("ext:pkg:{target_clean}");
                        insert_placeholder_node(
                            &mut nodes_to_insert,
                            &ext_id,
                            NodeCategory::Symbol,
                            &sym_name,
                            ResolutionStatus::External,
                            serde_json::json!({ "package": file_part }),
                        );
                        (
                            info.file_id.clone(),
                            ext_id,
                            ResolutionStatus::External,
                            EvidenceClassification::DirectFact,
                            0.85,
                            Some(target_clean.to_string()),
                        )
                    } else {
                        let unres_id = format!("unresolved:missing:{target_clean}");
                        insert_placeholder_node(
                            &mut nodes_to_insert,
                            &unres_id,
                            NodeCategory::Symbol,
                            &sym_name,
                            ResolutionStatus::Unresolved,
                            serde_json::json!({ "target": target_clean }),
                        );
                        (
                            info.file_id.clone(),
                            unres_id,
                            ResolutionStatus::Unresolved,
                            EvidenceClassification::Inferred,
                            0.30,
                            Some(target_clean.to_string()),
                        )
                    }
                } else {
                    match edge_cat {
                        RelationCategory::Imports => {
                            if let Some(m) = resolve_workspace_file(target_clean) {
                                match m {
                                    ResolutionMatch::Unique(wf) => {
                                        if is_test_source
                                            && wf != info.rel_path
                                            && !wf.contains("test")
                                        {
                                            let teid = compute_edge_id(
                                                &info.file_id,
                                                RelationCategory::Tests,
                                                &format!("file:{wf}"),
                                            );
                                            edges_to_insert.entry(teid.clone()).or_insert_with(
                                                || GraphEdge {
                                                    id: teid,
                                                    source_id: info.file_id.clone(),
                                                    target_id: format!("file:{wf}"),
                                                    category: RelationCategory::Tests,
                                                    provenance: default_provenance(
                                                        info.rel_path.clone(),
                                                        Some(r.line),
                                                        info.fp.clone(),
                                                        "graph_resolved_test_import",
                                                        EvidenceClassification::DerivedFact,
                                                        0.90,
                                                        ResolutionStatus::Resolved,
                                                        Some(target_clean.to_string()),
                                                    ),
                                                },
                                            );
                                        }
                                        (
                                            info.file_id.clone(),
                                            format!("file:{wf}"),
                                            ResolutionStatus::Resolved,
                                            EvidenceClassification::DirectFact,
                                            r.confidence,
                                            Some(target_clean.to_string()),
                                        )
                                    }
                                    ResolutionMatch::Ambiguous(cands) => {
                                        let amb_id = format!("unresolved:ambiguous:{target_clean}");
                                        insert_placeholder_node(
                                            &mut nodes_to_insert,
                                            &amb_id,
                                            NodeCategory::File,
                                            target_clean,
                                            ResolutionStatus::Ambiguous,
                                            serde_json::json!({ "candidate_count": cands.len() }),
                                        );
                                        (
                                            info.file_id.clone(),
                                            amb_id,
                                            ResolutionStatus::Ambiguous,
                                            EvidenceClassification::Inferred,
                                            0.45,
                                            Some(target_clean.to_string()),
                                        )
                                    }
                                }
                            } else if module_to_files.contains_key(target_clean) {
                                let mod_id = format!("mod:{lang_str}:{target_clean}");
                                nodes_to_insert.entry(mod_id.clone()).or_insert_with(|| {
                                    GraphNode {
                                        id: mod_id.clone(),
                                        category: NodeCategory::Module,
                                        label: target_clean.to_string(),
                                        relative_path: "".to_string(),
                                        module_path: Some(target_clean.to_string()),
                                        start_line: None,
                                        end_line: None,
                                        signature: None,
                                        fingerprint: hash(mod_id.as_bytes()),
                                        metadata: serde_json::json!({ "language": lang_str }),
                                    }
                                });
                                (
                                    info.file_id.clone(),
                                    mod_id,
                                    ResolutionStatus::Resolved,
                                    EvidenceClassification::DirectFact,
                                    0.96,
                                    Some(target_clean.to_string()),
                                )
                            } else if r.is_external
                                || crate::ast_deps::is_known_external_package(
                                    target_clean,
                                    lang_str,
                                )
                            {
                                let ext_id = format!("ext:pkg:{target_clean}");
                                insert_placeholder_node(
                                    &mut nodes_to_insert,
                                    &ext_id,
                                    NodeCategory::Module,
                                    target_clean,
                                    ResolutionStatus::External,
                                    serde_json::json!({ "language": lang_str }),
                                );
                                (
                                    info.file_id.clone(),
                                    ext_id,
                                    ResolutionStatus::External,
                                    EvidenceClassification::DirectFact,
                                    0.85,
                                    Some(target_clean.to_string()),
                                )
                            } else {
                                let unres_id = format!("unresolved:missing:{target_clean}");
                                insert_placeholder_node(
                                    &mut nodes_to_insert,
                                    &unres_id,
                                    NodeCategory::Module,
                                    target_clean,
                                    ResolutionStatus::Unresolved,
                                    serde_json::json!({ "language": lang_str }),
                                );
                                (
                                    info.file_id.clone(),
                                    unres_id,
                                    ResolutionStatus::Unresolved,
                                    EvidenceClassification::Inferred,
                                    0.30,
                                    Some(target_clean.to_string()),
                                )
                            }
                        }
                        RelationCategory::Tests => {
                            if let Some(m) = resolve_workspace_file(target_clean) {
                                match m {
                                    ResolutionMatch::Unique(wf) => (
                                        info.file_id.clone(),
                                        format!("file:{wf}"),
                                        ResolutionStatus::Resolved,
                                        EvidenceClassification::DerivedFact,
                                        r.confidence,
                                        Some(target_clean.to_string()),
                                    ),
                                    ResolutionMatch::Ambiguous(cands) => {
                                        let amb_id =
                                            format!("unresolved:ambiguous:test:{target_clean}");
                                        insert_placeholder_node(
                                            &mut nodes_to_insert,
                                            &amb_id,
                                            NodeCategory::File,
                                            target_clean,
                                            ResolutionStatus::Ambiguous,
                                            serde_json::json!({ "candidate_count": cands.len() }),
                                        );
                                        (
                                            info.file_id.clone(),
                                            amb_id,
                                            ResolutionStatus::Ambiguous,
                                            EvidenceClassification::Inferred,
                                            0.40,
                                            Some(target_clean.to_string()),
                                        )
                                    }
                                }
                            } else {
                                let unres_id = format!("unresolved:missing:test:{target_clean}");
                                insert_placeholder_node(
                                    &mut nodes_to_insert,
                                    &unres_id,
                                    NodeCategory::File,
                                    target_clean,
                                    ResolutionStatus::Unresolved,
                                    serde_json::json!({ "target": target_clean }),
                                );
                                (
                                    info.file_id.clone(),
                                    unres_id,
                                    ResolutionStatus::Unresolved,
                                    EvidenceClassification::Inferred,
                                    0.30,
                                    Some(target_clean.to_string()),
                                )
                            }
                        }
                        RelationCategory::Calls
                        | RelationCategory::Implements
                        | RelationCategory::Inherits
                        | RelationCategory::Extends
                        | RelationCategory::References => {
                            let prefer_types = matches!(
                                edge_cat,
                                RelationCategory::Implements
                                    | RelationCategory::Inherits
                                    | RelationCategory::Extends
                            );
                            let src_id = enclosing_caller(prefer_types);
                            let (qual_opt, bare_name) = extract_qualifier_and_bare(target_clean);
                            let candidates: Vec<SymEntry> = symbol_name_to_entries
                                .get(&bare_name)
                                .cloned()
                                .unwrap_or_default();

                            // Exclude `IMPL` container nodes when resolving inheritance/call targets
                            // unless nothing else matches (for constructor/struct targets prefer TYPE/FUNCTION/METHOD).
                            let filtered_cands: Vec<SymEntry> = {
                                let non_impl: Vec<SymEntry> = candidates
                                    .iter()
                                    .filter(|c| !c.kind.eq_ignore_ascii_case("IMPL"))
                                    .cloned()
                                    .collect();
                                if non_impl.is_empty() {
                                    candidates
                                } else {
                                    non_impl
                                }
                            };

                            // Step 1: check same-file match (with optional qualifier class scope)
                            let same_file: Vec<SymEntry> = filtered_cands
                                .iter()
                                .filter(|c| c.rel_path == info.rel_path)
                                .cloned()
                                .collect();

                            let pick_result: (
                                Option<SymEntry>,
                                usize,
                                ResolutionStatus,
                                EvidenceClassification,
                                f64,
                            ) = if same_file.len() == 1 && qual_opt.is_none() {
                                (
                                    Some(same_file[0].clone()),
                                    1,
                                    ResolutionStatus::Resolved,
                                    EvidenceClassification::DirectFact,
                                    0.98,
                                )
                            } else {
                                // Step 2: check explicit symbol imports or imported files / namespaces
                                let imported_scope: Vec<SymEntry> = filtered_cands
                                    .iter()
                                    .filter(|c| imported_files.contains(&c.rel_path))
                                    .cloned()
                                    .collect();

                                let qualified_scope: Vec<SymEntry> =
                                    if let Some(ref qual) = qual_opt {
                                        let q_bare = extract_qualifier_and_bare(qual).1;
                                        // Match symbol inside class/struct named `q_bare`, or in file whose stem matches `q_bare`
                                        let type_matches: Vec<SymEntry> = filtered_cands
                                            .iter()
                                            .filter(|c| {
                                                imported_files.contains(&c.rel_path)
                                                    || c.rel_path == info.rel_path
                                            })
                                            .filter(|c| {
                                                file_symbols_map
                                                    .get(&c.rel_path)
                                                    .map(|f_syms| {
                                                        f_syms.iter().any(|p| {
                                                            p.label == q_bare
                                                                && matches!(
                                                                    p.kind
                                                                        .to_ascii_uppercase()
                                                                        .as_str(),
                                                                    "CLASS"
                                                                        | "STRUCT"
                                                                        | "INTERFACE"
                                                                        | "TRAIT"
                                                                        | "MODULE"
                                                                )
                                                                && c.line >= p.line
                                                                && c.end_line <= p.end_line
                                                        })
                                                    })
                                                    .unwrap_or(false)
                                                    || strip_ext(
                                                        Path::new(&c.rel_path)
                                                            .file_name()
                                                            .and_then(|s| s.to_str())
                                                            .unwrap_or(""),
                                                    )
                                                    .eq_ignore_ascii_case(&q_bare)
                                            })
                                            .cloned()
                                            .collect();
                                        if !type_matches.is_empty() {
                                            type_matches
                                        } else {
                                            imported_scope.clone()
                                        }
                                    } else {
                                        imported_scope.clone()
                                    };

                                if qualified_scope.len() == 1 {
                                    let conf = if explicit_sym_imports.contains_key(&bare_name)
                                        || qual_opt.is_some()
                                    {
                                        0.96
                                    } else {
                                        0.90
                                    };
                                    let cls = if explicit_sym_imports.contains_key(&bare_name) {
                                        EvidenceClassification::DirectFact
                                    } else {
                                        EvidenceClassification::DerivedFact
                                    };
                                    (
                                        Some(qualified_scope[0].clone()),
                                        1,
                                        ResolutionStatus::Resolved,
                                        cls,
                                        conf,
                                    )
                                } else if qualified_scope.len() > 1 {
                                    (
                                        None,
                                        qualified_scope.len(),
                                        ResolutionStatus::Ambiguous,
                                        EvidenceClassification::Inferred,
                                        0.45,
                                    )
                                } else if same_file.len() == 1 {
                                    (
                                        Some(same_file[0].clone()),
                                        1,
                                        ResolutionStatus::Resolved,
                                        EvidenceClassification::DirectFact,
                                        0.95,
                                    )
                                } else if filtered_cands.len() == 1 && !r.is_external {
                                    // Single unique workspace candidate across un-imported files
                                    (
                                        Some(filtered_cands[0].clone()),
                                        1,
                                        ResolutionStatus::Resolved,
                                        EvidenceClassification::DerivedFact,
                                        0.78,
                                    )
                                } else if filtered_cands.len() > 1 && !r.is_external {
                                    // Ambiguous across multiple workspace files — never pick candidates[0]!
                                    (
                                        None,
                                        filtered_cands.len(),
                                        ResolutionStatus::Ambiguous,
                                        EvidenceClassification::Inferred,
                                        0.40,
                                    )
                                } else if r.is_external
                                    || crate::ast_deps::is_known_external_package(
                                        target_clean,
                                        lang_str,
                                    )
                                {
                                    (
                                        None,
                                        0,
                                        ResolutionStatus::External,
                                        EvidenceClassification::DirectFact,
                                        0.85,
                                    )
                                } else {
                                    (
                                        None,
                                        0,
                                        ResolutionStatus::Unresolved,
                                        EvidenceClassification::Inferred,
                                        0.30,
                                    )
                                }
                            };

                            if let Some(chosen) = pick_result.0 {
                                // Refine C# inheritance vs interface implementation once target symbol kind is known
                                if matches!(
                                    edge_cat,
                                    RelationCategory::Inherits | RelationCategory::Implements
                                ) && lang_str == "csharp"
                                {
                                    let k = chosen.kind.to_ascii_uppercase();
                                    if k == "INTERFACE" || k == "TRAIT" {
                                        edge_cat = RelationCategory::Implements;
                                    } else if k == "CLASS" || k == "STRUCT" || k == "RECORD" {
                                        edge_cat = RelationCategory::Inherits;
                                    }
                                }
                                (
                                    src_id,
                                    chosen.id,
                                    pick_result.2,
                                    pick_result.3,
                                    pick_result.4,
                                    Some(target_clean.to_string()),
                                )
                            } else if pick_result.2 == ResolutionStatus::Ambiguous {
                                let amb_id = format!("unresolved:ambiguous:{target_clean}");
                                insert_placeholder_node(
                                    &mut nodes_to_insert,
                                    &amb_id,
                                    NodeCategory::Symbol,
                                    &bare_name,
                                    ResolutionStatus::Ambiguous,
                                    serde_json::json!({ "candidate_count": pick_result.1 }),
                                );
                                (
                                    src_id,
                                    amb_id,
                                    ResolutionStatus::Ambiguous,
                                    EvidenceClassification::Inferred,
                                    pick_result.4,
                                    Some(target_clean.to_string()),
                                )
                            } else if pick_result.2 == ResolutionStatus::External {
                                let ext_id = format!("ext:pkg:{target_clean}");
                                insert_placeholder_node(
                                    &mut nodes_to_insert,
                                    &ext_id,
                                    NodeCategory::Symbol,
                                    &bare_name,
                                    ResolutionStatus::External,
                                    serde_json::json!({ "target": target_clean }),
                                );
                                (
                                    src_id,
                                    ext_id,
                                    ResolutionStatus::External,
                                    EvidenceClassification::DerivedFact,
                                    0.80,
                                    Some(target_clean.to_string()),
                                )
                            } else {
                                let unres_id = format!("unresolved:missing:{target_clean}");
                                insert_placeholder_node(
                                    &mut nodes_to_insert,
                                    &unres_id,
                                    NodeCategory::Symbol,
                                    &bare_name,
                                    ResolutionStatus::Unresolved,
                                    serde_json::json!({ "target": target_clean }),
                                );
                                (
                                    src_id,
                                    unres_id,
                                    ResolutionStatus::Unresolved,
                                    EvidenceClassification::Inferred,
                                    0.30,
                                    Some(target_clean.to_string()),
                                )
                            }
                        }
                        _ => {
                            let ext_id = format!("ext:pkg:{target_clean}");
                            insert_placeholder_node(
                                &mut nodes_to_insert,
                                &ext_id,
                                NodeCategory::Symbol,
                                target_clean,
                                ResolutionStatus::External,
                                serde_json::json!({ "target": target_clean }),
                            );
                            (
                                info.file_id.clone(),
                                ext_id,
                                ResolutionStatus::External,
                                EvidenceClassification::DerivedFact,
                                0.75,
                                Some(target_clean.to_string()),
                            )
                        }
                    }
                };

            let eid = compute_edge_id(&source_node_id, edge_cat, &target_node_id);
            edges_to_insert
                .entry(eid.clone())
                .or_insert_with(|| GraphEdge {
                    id: eid,
                    source_id: source_node_id,
                    target_id: target_node_id,
                    category: edge_cat,
                    provenance: default_provenance(
                        info.rel_path.clone(),
                        Some(r.line),
                        info.fp.clone(),
                        &r.evidence,
                        classification,
                        confidence,
                        res_status,
                        hint_opt,
                    ),
                });
        }
    }

    // Ingest Project Constitution if present (Policy nodes & Governance edges)
    if let Ok(mut rules) = crate::constitution::load_constitution(&root) {
        rules.sort_by(|a, b| a.id.cmp(&b.id));
        let const_file_id = "file:.engram/constitution.yaml".to_string();
        for rule in rules {
            let policy_id = format!("policy:{}", rule.id);
            nodes_to_insert.insert(
                policy_id.clone(),
                GraphNode {
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
                },
            );

            if current_files.contains_key(".engram/constitution.yaml") {
                let eid = compute_edge_id(&const_file_id, RelationCategory::Contains, &policy_id);
                edges_to_insert.insert(
                    eid.clone(),
                    GraphEdge {
                        id: eid,
                        source_id: const_file_id.clone(),
                        target_id: policy_id,
                        category: RelationCategory::Contains,
                        provenance: default_provenance(
                            ".engram/constitution.yaml".to_string(),
                            None,
                            "constitution".to_string(),
                            "constitution_parser",
                            EvidenceClassification::DirectFact,
                            1.0,
                            ResolutionStatus::Resolved,
                            None,
                        ),
                    },
                );
            }
        }
    }

    // Ingest Active Change Contract if present (Contract node & GovernedBy edges)
    if let Ok(Some(mut contract)) = crate::contract::get_active_contract(&root) {
        contract.boundary.expected_files.sort();
        contract.boundary.expected_files.dedup();
        let contract_id = format!("contract:{}", contract.contract_id);
        nodes_to_insert.insert(
            contract_id.clone(),
            GraphNode {
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
            },
        );

        for exp_file in &contract.boundary.expected_files {
            if current_files.contains_key(exp_file) {
                let target_file_id = format!("file:{exp_file}");
                let eid =
                    compute_edge_id(&target_file_id, RelationCategory::GovernedBy, &contract_id);
                edges_to_insert.insert(
                    eid.clone(),
                    GraphEdge {
                        id: eid,
                        source_id: target_file_id,
                        target_id: contract_id.clone(),
                        category: RelationCategory::GovernedBy,
                        provenance: default_provenance(
                            ".engram/contracts/active.json".to_string(),
                            None,
                            "contract".to_string(),
                            "contract_manager",
                            EvidenceClassification::DirectFact,
                            1.0,
                            ResolutionStatus::Resolved,
                            None,
                        ),
                    },
                );
            }
        }
    }

    let next_generation = if effective_full
        || !added.is_empty()
        || !modified.is_empty()
        || !deleted.is_empty()
        || !tier2_files.is_empty()
    {
        prev_generation + 1
    } else {
        prev_generation.max(1)
    };

    // Atomic SQLite Transaction Commit (Immediate prevents read->write upgrade deadlocks)
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    if effective_full {
        tx.execute_batch(
            "DELETE FROM kg_edges;
             DELETE FROM kg_nodes;
             DELETE FROM kg_file_fingerprints;
             DELETE FROM kg_file_extractions;
             DELETE FROM kg_metadata;",
        )?;
    } else {
        for path in &files_to_remove {
            let mut node_ids: Vec<String> = Vec::new();
            {
                let mut stmt = tx.prepare("SELECT id FROM kg_nodes WHERE relative_path = ?1")?;
                let rows = stmt.query_map(params![path], |r| r.get::<_, String>(0))?;
                for nid in rows {
                    if let Ok(id) = nid {
                        node_ids.push(id);
                    }
                }
            }
            node_ids.push(format!("file:{path}"));
            for nid in &node_ids {
                tx.execute(
                    "DELETE FROM kg_edges WHERE source_id = ?1 OR target_id = ?1",
                    params![nid],
                )?;
            }
            tx.execute("DELETE FROM kg_edges WHERE source_path = ?1", params![path])?;
            tx.execute(
                "DELETE FROM kg_nodes WHERE relative_path = ?1",
                params![path],
            )?;
            tx.execute(
                "DELETE FROM kg_file_fingerprints WHERE relative_path = ?1",
                params![path],
            )?;
            tx.execute(
                "DELETE FROM kg_file_extractions WHERE relative_path = ?1",
                params![path],
            )?;
        }

        // Remove stale non-structural outgoing edges of Tier 2 affected files prior to re-insertion
        // (Preserve CONTAINS and file->symbol DECLARES edges emitted in Pass 1.)
        for t2 in &tier2_files {
            tx.execute(
                "DELETE FROM kg_edges
                 WHERE source_path = ?1
                   AND category != 'CONTAINS'
                   AND NOT (category = 'DECLARES' AND target_id LIKE 'symbol:%')",
                params![t2],
            )?;
        }

        // Refresh shared policy/contract nodes cleanly
        tx.execute(
            "DELETE FROM kg_edges WHERE source_id LIKE 'policy:%' OR target_id LIKE 'policy:%' OR source_id LIKE 'contract:%' OR target_id LIKE 'contract:%'",
            [],
        )?;
        tx.execute(
            "DELETE FROM kg_nodes WHERE id LIKE 'policy:%' OR id LIKE 'contract:%'",
            [],
        )?;
    }

    {
        let mut stmt_node = tx.prepare(
            "INSERT OR REPLACE INTO kg_nodes (
                id, category, label, relative_path, module_path, scope_chain, start_line, end_line, signature, fingerprint, resolution_status, metadata_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )?;
        for n in nodes_to_insert.values() {
            let res_str = n
                .metadata
                .get("resolution_status")
                .and_then(|v| v.as_str())
                .unwrap_or("RESOLVED");
            stmt_node.execute(params![
                n.id,
                n.category.as_str(),
                n.label,
                n.relative_path,
                n.module_path,
                Option::<String>::None,
                n.start_line.map(|l| l as i64),
                n.end_line.map(|l| l as i64),
                n.signature,
                n.fingerprint,
                res_str,
                serde_json::to_string(&n.metadata).unwrap_or_else(|_| "{}".to_string())
            ])?;
        }
    }

    {
        let mut stmt_edge = tx.prepare(
            "INSERT OR REPLACE INTO kg_edges (
                id, source_id, target_id, category, source_path, source_line, source_end_line, content_hash, extraction_method, classification, resolution_status, confidence, target_hint
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )?;
        for e in edges_to_insert.values() {
            stmt_edge.execute(params![
                e.id,
                e.source_id,
                e.target_id,
                e.category.as_str(),
                e.provenance.source_path,
                e.provenance.source_line.map(|l| l as i64),
                Option::<i64>::None,
                e.provenance.content_hash,
                e.provenance.extraction_method,
                e.provenance.classification.as_str(),
                e.provenance.resolution_status.as_str(),
                e.provenance.confidence,
                e.provenance.target_hint
            ])?;
        }
    }

    // Referential sweep & bottom-up orphan pruning (directories, modules, external/unresolved placeholders)
    tx.execute(
        "DELETE FROM kg_edges WHERE source_id NOT IN (SELECT id FROM kg_nodes) OR target_id NOT IN (SELECT id FROM kg_nodes)",
        [],
    )?;
    loop {
        let deleted_dirs = tx.execute(
            "DELETE FROM kg_nodes WHERE category = 'DIRECTORY' AND id NOT IN (SELECT source_id FROM kg_edges)",
            [],
        )?;
        tx.execute(
            "DELETE FROM kg_edges WHERE source_id NOT IN (SELECT id FROM kg_nodes) OR target_id NOT IN (SELECT id FROM kg_nodes)",
            [],
        )?;
        if deleted_dirs == 0 {
            break;
        }
    }
    tx.execute(
        "DELETE FROM kg_nodes
         WHERE (
             category IN ('MODULE')
             OR id LIKE 'ext:%'
             OR id LIKE 'unresolved:%'
             OR id LIKE 'symbol:ext::%'
             OR id LIKE 'file:ext::%'
         )
         AND id NOT IN (SELECT source_id FROM kg_edges UNION SELECT target_id FROM kg_edges)",
        [],
    )?;

    // Save fingerprints & raw extractions for parsed files
    {
        let mut stmt_fp = tx.prepare(
            "INSERT OR REPLACE INTO kg_file_fingerprints (relative_path, fingerprint, parse_status, analysis_key, last_indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let now_str = chrono::Utc::now().to_rfc3339();
        for p in &files_to_parse {
            if let Some(fp) = current_files.get(p) {
                let pstatus = file_parse_status_updates
                    .get(p)
                    .map(|s| s.as_str())
                    .unwrap_or("COMPLETE");
                stmt_fp.execute(params![p, fp, pstatus, config_fp, now_str])?;
            }
        }

        let mut stmt_ext = tx.prepare(
            "INSERT OR REPLACE INTO kg_file_extractions (relative_path, parse_status, rels_json)
             VALUES (?1, ?2, ?3)",
        )?;
        for (p, (pstatus, rels)) in &parsed_extractions_to_save {
            let rels_json = serde_json::to_string(rels).unwrap_or_else(|_| "[]".to_string());
            stmt_ext.execute(params![p, pstatus, rels_json])?;
        }
    }

    // Recompute authoritative partial/failed counts across all indexed files in store
    let db_partial_count: usize = tx.query_row(
        "SELECT COUNT(*) FROM kg_file_fingerprints WHERE parse_status = 'PARTIAL'",
        [],
        |r| r.get(0),
    )?;
    let db_failed_count: usize = tx.query_row(
        "SELECT COUNT(*) FROM kg_file_fingerprints WHERE parse_status = 'FAILED'",
        [],
        |r| r.get(0),
    )?;
    let effective_failed = failed_files_count.max(db_failed_count);
    let effective_partial = partial_files_count.max(db_partial_count);

    // Update integrity metadata
    let head_sha = crate::git::resolve_baseline(&root, "HEAD")
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_else(|| "HEAD".to_string());

    let final_state = if effective_failed > 0 || effective_partial > 0 {
        GraphLifecycleState::PartiallyIndexed
    } else {
        GraphLifecycleState::Ready
    };

    for (k, v) in [
        ("schema_version", GRAPH_SCHEMA_VERSION.to_string()),
        ("graph_generation", next_generation.to_string()),
        ("repo_root_hash", repo_root_hash),
        ("source_tree_fingerprint", source_tree_fp),
        ("config_fingerprint", config_fp),
        ("ignore_fingerprint", ignore_fp),
        ("parser_version", GRAPH_PARSER_VERSION.to_string()),
        ("manifest_fingerprint", manifest_fp),
        ("governance_fingerprint", governance_fp),
        ("last_indexed_commit", head_sha.clone()),
        ("state", final_state.as_str().to_string()),
        ("failed_files_count", failed_files_count.to_string()),
        ("partial_files_count", partial_files_count.to_string()),
        (
            "unsupported_files_count",
            unsupported_files_count.to_string(),
        ),
    ] {
        tx.execute(
            "INSERT OR REPLACE INTO kg_metadata (key, value) VALUES (?1, ?2)",
            params![k, v],
        )?;
    }
    if let Some(ref err) = last_error {
        tx.execute(
            "INSERT OR REPLACE INTO kg_metadata (key, value) VALUES ('last_error', ?1)",
            params![err],
        )?;
    } else {
        tx.execute("DELETE FROM kg_metadata WHERE key = 'last_error'", [])?;
    }

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
            "tier2_re_resolved": tier2_files.len(),
            "full_rebuild": effective_full,
            "nodes": node_count,
            "edges": edge_count,
            "elapsed_ms": elapsed
        }),
    );

    Ok(GraphStatus {
        state: final_state,
        schema_version: GRAPH_SCHEMA_VERSION.to_string(),
        index_generation: next_generation,
        node_count,
        edge_count,
        last_indexed_commit: Some(head_sha),
        indexed_files_count: current_files.len(),
        stale_files_count: 0,
        failed_files_count,
        unsupported_files_count,
        last_error,
    })
}

enum ResolutionMatch {
    Unique(String),
    Ambiguous(Vec<String>),
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
                 FROM kg_nodes
                 ORDER BY id",
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
                if !node.relative_path.is_empty() {
                    let qual =
                        format!("{}::{}", node.relative_path, node.label).to_ascii_lowercase();
                    label_to_nodes
                        .entry(qual)
                        .or_default()
                        .push(node.id.clone());
                }
                nodes_by_id.insert(node.id.clone(), node);
            }
        }

        {
            let mut stmt = conn.prepare(
                "SELECT id, source_id, target_id, category, source_path, source_line, content_hash, extraction_method, classification, resolution_status, confidence, target_hint
                 FROM kg_edges
                 ORDER BY id",
            )?;
            let rows = stmt.query_map([], |r| {
                let cat_str: String = r.get(3)?;
                let class_str: String = r.get(8)?;
                let res_str: String = r.get(9)?;
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
                        resolution_status: ResolutionStatus::from_str_opt(&res_str)
                            .unwrap_or(ResolutionStatus::Resolved),
                        confidence: r.get(10)?,
                        target_hint: r.get(11)?,
                    },
                })
            })?;

            for row in rows {
                let edge = row?;
                // Referential validation: drop dangling edges
                if !nodes_by_id.contains_key(&edge.source_id)
                    || !nodes_by_id.contains_key(&edge.target_id)
                {
                    continue;
                }
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
        let norm_lower = norm.to_ascii_lowercase();
        let mut matches = Vec::new();
        for (id, n) in &self.nodes_by_id {
            if n.label.to_ascii_lowercase() == norm_lower
                || id.to_ascii_lowercase().contains(&norm_lower)
            {
                matches.push(id.clone());
            }
        }
        matches.sort();
        matches.dedup();
        matches
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

    /// Fast indexed direct neighborhood edge query avoiding full graph deserialization.
    pub fn query_file_neighborhood_fast(
        conn: &Connection,
        seed_path: &str,
        limit: usize,
    ) -> Vec<GraphEdge> {
        let file_id = format!("file:{seed_path}");
        let mut stmt = match conn.prepare(
            "SELECT id, source_id, target_id, category, source_path, source_line, content_hash, extraction_method, classification, resolution_status, confidence, target_hint
             FROM kg_edges
             WHERE source_id = ?1 OR target_id = ?1 OR source_path = ?2
             ORDER BY id
             LIMIT ?3",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let rows = match stmt.query_map(rusqlite::params![file_id, seed_path, limit as i64], |r| {
            let cat_str: String = r.get(3)?;
            let class_str: String = r.get(8)?;
            let res_str: String = r.get(9)?;
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
                    resolution_status: ResolutionStatus::from_str_opt(&res_str)
                        .unwrap_or(ResolutionStatus::Resolved),
                    confidence: r.get(10)?,
                    target_hint: r.get(11)?,
                },
            })
        }) {
            Ok(rows) => rows,
            Err(_) => return Vec::new(),
        };

        rows.flatten().collect()
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

        for s in &start_nodes {
            visited_nodes.insert(s.clone());
            queue.push_back((s.clone(), 0));
            // Include edges from/to symbols (and declared modules in reverse lookups) owned by a starting file node at depth 0
            if let Some(node) = self.nodes_by_id.get(s) {
                if matches!(node.category, NodeCategory::File | NodeCategory::Test) {
                    if let Some(out) = self.outgoing.get(s) {
                        for e in out {
                            if e.category == RelationCategory::Declares
                                && (e.target_id.starts_with("symbol:")
                                    || (incoming_direction && e.target_id.starts_with("module:")))
                                && visited_nodes.insert(e.target_id.clone())
                            {
                                queue.push_back((e.target_id.clone(), 0));
                            }
                        }
                    }
                }
            }
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
                            | RelationCategory::Implements
                            | RelationCategory::Inherits
                            | RelationCategory::Extends
                            | RelationCategory::Tests
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

    pub fn reverse_dependencies(&self, target: &str, max_depth: usize) -> Vec<GraphEdge> {
        self.dependencies(target, max_depth, true)
    }

    /// Explains why a target node/file/symbol is connected in the graph with resolution status & provenance.
    pub fn explain_target(&self, target: &str, max_depth: usize) -> serde_json::Value {
        let seeds = self.resolve_node_ids(target);
        let deps = self.dependencies(target, max_depth, false);
        let rev_deps = self.reverse_dependencies(target, max_depth);
        let impact_report = self.impact(target, max_depth);

        let edge_to_json = |e: &GraphEdge| -> serde_json::Value {
            serde_json::json!({
                "id": e.id,
                "source_id": e.source_id,
                "target_id": e.target_id,
                "category": e.category.as_str(),
                "source_path": e.provenance.source_path,
                "source_line": e.provenance.source_line,
                "extraction_method": e.provenance.extraction_method,
                "classification": e.provenance.classification.as_str(),
                "resolution_status": e.provenance.resolution_status.as_str(),
                "confidence": e.provenance.confidence,
                "target_hint": e.provenance.target_hint
            })
        };

        let resolved_nodes: Vec<serde_json::Value> = seeds
            .iter()
            .filter_map(|id| self.nodes_by_id.get(id))
            .map(|n| {
                serde_json::json!({
                    "id": n.id,
                    "category": n.category.as_str(),
                    "label": n.label,
                    "relative_path": n.relative_path,
                    "start_line": n.start_line,
                    "end_line": n.end_line
                })
            })
            .collect();

        let unresolved_or_ambiguous: Vec<serde_json::Value> = deps
            .iter()
            .filter(|e| {
                matches!(
                    e.provenance.resolution_status,
                    ResolutionStatus::Unresolved
                        | ResolutionStatus::Ambiguous
                        | ResolutionStatus::PartiallyResolved
                )
            })
            .map(edge_to_json)
            .collect();

        serde_json::json!({
            "query": target,
            "resolved_nodes": resolved_nodes,
            "outgoing_dependencies": deps.iter().map(edge_to_json).collect::<Vec<_>>(),
            "reverse_dependencies": rev_deps.iter().map(edge_to_json).collect::<Vec<_>>(),
            "unresolved_or_ambiguous": unresolved_or_ambiguous,
            "impact": impact_report
        })
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

            // If seed node at depth 0 is a file, also evaluate blast-radius from its declared symbols
            if curr_depth == 0 {
                if let Some(curr_node) = self.nodes_by_id.get(&curr) {
                    if curr_node.category == NodeCategory::File
                        || curr_node.category == NodeCategory::Test
                    {
                        if let Some(edges) = self.outgoing.get(&curr) {
                            for e in edges {
                                if e.category == RelationCategory::Declares {
                                    if visited_nodes.insert(e.target_id.clone()) {
                                        queue.push_back((e.target_id.clone(), curr_depth));
                                    }
                                }
                            }
                        }
                    }
                }
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

        // Direct candidate file path lexical match
        for f in candidate_files {
            if f.to_ascii_lowercase().contains(term) {
                *scored_files.entry(f.clone()).or_insert(0.0) += 4.0;
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
    let estimate_file_tokens = |file: &str| -> usize {
        let abs = root.join(file);
        if let Ok(meta) = fs::metadata(&abs) {
            ((meta.len() as usize) / 4).clamp(300, 4000)
        } else {
            300
        }
    };

    let mut prioritized = Vec::new();
    let mut accumulated_tokens = 0;

    for (f, _) in ranked_files {
        let ft = estimate_file_tokens(&f);
        if accumulated_tokens + ft <= token_budget {
            accumulated_tokens += ft;
            prioritized.push(f);
        } else {
            break;
        }
    }

    let baseline_files_count = candidate_files.len().max(prioritized.len());
    let estimated_baseline: usize = candidate_files
        .iter()
        .map(|f| estimate_file_tokens(f))
        .sum();
    let estimated_graph: usize = prioritized.iter().map(|f| estimate_file_tokens(f)).sum();
    let reduction = if estimated_baseline > 0 && estimated_baseline > estimated_graph {
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
