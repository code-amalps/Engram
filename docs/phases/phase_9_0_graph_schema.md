# Engram Phase 9.0: Native Micro-Knowledge Graph Schema Specification

**Phase:** 9.0  
**Document Identifier:** `ENGRAM-DOC-PHASE-9-0-SCHEMA-20261123`  
**Schema Version:** `1.0.0`  
**Engine:** SQLite & In-Memory Bidirectional Traversal  
**Date:** 2026-11-23  
**Status:** `SCHEMA_LOCKED`  

---

## 1. Node Categories & Definitions

The knowledge graph models 14 distinct node categories representing structural, semantic, and governance elements in a software repository:

```rust
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
```

### Node Data Structure:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphNode {
    pub id: String,                  // Unique deterministic ID: e.g. "symbol:src/lib.rs::MyStruct"
    pub category: NodeCategory,      // Node category classification
    pub label: String,               // Human-readable identifier/symbol name
    pub relative_path: String,       // Relative file or directory path
    pub module_path: Option<String>, // Hierarchical module path (e.g. "engram::analyzer")
    pub range: Option<SourceRange>,  // Start line/col to end line/col
    pub signature: Option<String>,   // Function/type signature
    pub fingerprint: String,         // SHA-256 fingerprint of node content
    pub metadata: serde_json::Value, // Typed extensible metadata
}
```

---

## 2. Relationship Categories & Definitions

Relationships connect nodes with explicit semantic meaning and directional provenance:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationCategory {
    Contains,    // Directory -> File, File -> Function, Module -> Symbol
    Imports,     // File -> Module, File -> File
    DependsOn,   // Module -> Module, Package -> Package
    Calls,       // Function -> Function, Method -> Method
    References,  // Function -> Type/Symbol, Variable -> Type
    Implements,  // Struct -> Trait/Interface
    Extends,     // Class -> BaseClass
    Declares,    // File -> Symbol, Interface -> Method
    Tests,       // Test -> Function, Test -> Module
    GovernedBy,  // File/Symbol -> Contract, File -> ConstitutionRule
    Documents,   // Markdown/DocComment -> Symbol/Module
    Configures,  // ConfigFile -> Module/Setting
}
```

### Relationship Data Structure:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphEdge {
    pub id: String,                    // Deterministic edge ID: "hash(source|kind|target)"
    pub source_id: String,             // Source Node ID
    pub target_id: String,             // Target Node ID
    pub category: RelationCategory,    // Relationship kind
    pub provenance: EdgeProvenance,    // Full evidence and origin metadata
}
```

---

## 3. Evidence & Provenance Data Model

Every edge and node retains explicit provenance to distinguish factual, derived, and inferred relationships:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceClassification {
    DirectFact,   // Verified directly from AST parser (e.g. explicit function call)
    DerivedFact,  // Reconstructed transitively (e.g. module dependency via imports)
    Inferred,     // Heuristic or naming-based (e.g. test_foo tests foo)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EdgeProvenance {
    pub source_path: String,
    pub source_line: Option<usize>,
    pub content_hash: String,
    pub extraction_method: String, // e.g. "ast_tree_sitter", "contract_parser", "heuristic"
    pub classification: EvidenceClassification,
    pub confidence: f64,           // 1.0 for DirectFact, 0.9 for Derived, 0.7 for Inferred
}
```

---

## 4. SQLite Storage Schema (`.engram/index.db`)

The graph is persisted deterministically inside SQLite using indexed tables:

```sql
CREATE TABLE IF NOT EXISTS graph_nodes (
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

CREATE TABLE IF NOT EXISTS graph_edges (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    category TEXT NOT NULL,
    source_path TEXT NOT NULL,
    source_line INTEGER,
    content_hash TEXT NOT NULL,
    extraction_method TEXT NOT NULL,
    classification TEXT NOT NULL,
    confidence REAL NOT NULL,
    FOREIGN KEY(source_id) REFERENCES graph_nodes(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS graph_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_graph_nodes_path ON graph_nodes(relative_path);
CREATE INDEX IF NOT EXISTS idx_graph_nodes_cat ON graph_nodes(category);
CREATE INDEX IF NOT EXISTS idx_graph_nodes_mod ON graph_nodes(module_path);
CREATE INDEX IF NOT EXISTS idx_graph_edges_source ON graph_edges(source_id);
CREATE INDEX IF NOT EXISTS idx_graph_edges_target ON graph_edges(target_id);
CREATE INDEX IF NOT EXISTS idx_graph_edges_cat ON graph_edges(category);
```

---

## 5. Lifecycle States

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GraphLifecycleState {
    Ready,
    Indexing,
    PartiallyIndexed,
    Stale,
    Unavailable,
    Corrupt,
}
```
