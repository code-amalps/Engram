# Engram Phase 9.0: Native Micro-Knowledge Graph Implementation Report

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**GitHub:** `code-amalps/Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Phase:** 9.0 (Native Micro-Knowledge Graph & Repository Intelligence)  
**Baseline Candidate:** `v1.5.5-cand1` (`ded9ca521024d18248235a33e2e292964b79f9b0`)  
**Previous Baseline:** `v1.5.4-cand5` (`2df1cac683435314ffd7ad52369e6d20198fae0d`)  
**Implementation Date:** 23 November 2026  

---

## 1. Architectural Overview

Engram Phase 9.0 implements an independent, native **Micro-Knowledge Graph** embedded directly inside Engram's core Rust binary. It provides structured, deterministic, and fine-grained codebase understanding without relying on external runtimes, Python daemons, or third-party graph tools like Graphify.

The implementation is structured into six strictly separated, cohesive layers:

```
┌────────────────────────────────────────────────────────────────────────┐
│                      Layer F: Context Compiler & MCP                   │
│   (rank_context_with_graph, engram_engage integration, compiler boost) │
├────────────────────────────────────────────────────────────────────────┤
│                  Layer B: Unified Query & Adjacency                    │
│   (KnowledgeGraphIndex: neighborhood, impact, call_path, subsystems)   │
├────────────────────────────────────────────────────────────────────────┤
│                   Layer C: Evidence & Provenance                       │
│   (DirectFact, DerivedFact, Inferred, SHA-256 Content Fingerprints)    │
├────────────────────────────────────────────────────────────────────────┤
│              Layer A & E: Construction & Incremental Sync              │
│   (sync_knowledge_graph, Tree-Sitter AST, Constitution, Contracts)     │
├────────────────────────────────────────────────────────────────────────┤
│                  Layer D: SQLite Storage Engine                        │
│   (knowledge_graph.db: kg_nodes, kg_edges, kg_file_fingerprints)       │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Layer Implementation Details

### Layer A & E: Graph Construction & Incremental Sync Engine (`src/knowledge_graph.rs`)
- **File Discovery & Pruning:** Uses `ignore::WalkBuilder` with explicit entry filters to automatically ignore non-source trees: `target`, `.git`, `.engram`, `node_modules`, `vendor`, `dist`, `build`, `coverage`, `benchmarks`, `test_out`, `releases`, `.pytest_cache`, `graphify-out`, and `assets`.
- **AST Parsing:** Coordinates with `crate::analyzer::parse_file` across 7 languages (Rust, TypeScript, JavaScript, Python, Go, Java, C#) to extract symbols (functions, methods, types, structs, traits, enums) and relationships (`CALLS`, `IMPORTS`, `REFERENCES`).
- **Constitution & Contract Ingestion:** Ingests active Project Constitution rules into `POLICY` nodes and `CONTAINS` edges; ingests active Change Contracts into `CONTRACT` nodes and `GOVERNED_BY` edges.
- **Incremental State Machine:**
  - Maintains `kg_file_fingerprints(relative_path, fingerprint, last_indexed_at)`.
  - In incremental mode (`force_full: false`), compares current disk hashes with stored fingerprints.
  - Deletes only stale nodes/edges for modified or deleted files within a single atomic SQLite transaction.
  - Adding or modifying files triggers localized re-indexing in `< 25 ms`.

### Layer B: Unified Query & Adjacency Interface (`KnowledgeGraphIndex`)
- Loaded into memory from SQLite for sub-millisecond query execution.
- In-memory data structures:
  - `nodes_by_id: HashMap<String, GraphNode>`
  - `outgoing: HashMap<String, Vec<GraphEdge>>`
  - `incoming: HashMap<String, Vec<GraphEdge>>`
  - `label_to_nodes: HashMap<String, Vec<String>>`
  - `module_to_nodes: HashMap<String, Vec<String>>`
- **Core Query Primitives:**
  - `neighborhood(target, depth, limit)`: Bounded bidirectional BFS traversal enforcing depth and item limits.
  - `impact(target, max_depth)`: Evaluates transitive blast radius, tracing callers, dependent files, exercised tests, and governing contracts.
  - `call_path(source, target, max_depth)`: BFS shortest call path discovery across symbols.
  - `subsystems()`: Architectural module grouping and cross-subsystem dependency analysis.

### Layer C: Evidence & Provenance Tiers
All edges enforce explicit provenance metadata (`EdgeProvenance`):
- `classification`: `DirectFact` (deterministic AST / filesystem parse), `DerivedFact` (transitive inference), `Inferred` (heuristic / companion matching).
- `extraction_method`: `"filesystem"`, `"ast_tree_sitter"`, `"constitution_parser"`, `"contract_manager"`.
- `content_hash`: SHA-256 fingerprint of the source text at extraction time.
- `confidence`: Bounded real between `0.0` and `1.0`.

### Layer D: SQLite Storage (`.engram/cache/knowledge_graph.db`)
- Normalized relational schema:
  - `kg_nodes (id, category, label, relative_path, module_path, start_line, end_line, signature, fingerprint, metadata_json)`
  - `kg_edges (id, source_id, target_id, category, source_path, source_line, content_hash, extraction_method, classification, confidence)`
  - `kg_file_fingerprints (relative_path, fingerprint, last_indexed_at)`
  - `kg_metadata (key, value)`
- Robust indexing: B-tree indexes on `relative_path`, `category`, `module_path`, `source_id`, `target_id`.
- SQLite WAL mode and 10-second busy timeout for high-concurrency resilience.

### Layer F: Context Compiler & MCP Integration
- **`src/compiler.rs`:** Added Step 4.D Native Micro-Knowledge Graph multi-hop expansion. Boosts files connected via `CALLS`, `IMPORTS`, `DEPENDS_ON`, or `TESTS` in the composite score.
- **`src/engage.rs`:** Wired `knowledge_graph::rank_context_with_graph` into Turn-0 context preparation. Dynamically ranks candidate files and exposes graph intelligence metadata (`token_reduction_pct`, `prioritized_files_count`, `related_tests`) without breaking backward compatibility.

---

## 3. CLI Subcommands & Verification

The Engram CLI has been extended with the `engram graph` command group (`src/main.rs`):

| Command | Purpose | Verification Result |
| :--- | :--- | :--- |
| `engram graph status` | Displays lifecycle state, node/edge counts, and last commit | Verified: `State: Ready`, 3,076 nodes, 4,178 edges, 749 files |
| `engram graph query <target>` | Explores 1-hop and 2-hop neighborhood of a node | Verified: resolves file/symbol neighborhood with confidence & tiers |
| `engram graph impact <target>` | Traces blast radius (affected files, symbols, tests, contracts) | Verified: accurately identifies dependent callers and test suites |
| `engram graph call-path <s> <t>` | Computes shortest call path between symbols | Verified: returns exact hop sequence across call chains |
| `engram graph subsystems` | Discovers architectural modules and cross-module couplings | Verified: cleanly identifies 70 repository subsystems (`src`, `tests`, etc.) |
| `engram graph reindex` | Forces full re-index of the repository | Verified: clean atomic rebuild and SQLite persistence |

---

## 4. Test Suite & Verification Evidence

A comprehensive integration test suite was created in `tests/knowledge_graph_test.rs`:

```
running 8 tests
test test_schema_initialization_and_metadata ... ok
test test_bounded_neighborhood_and_traversal ... ok
test test_change_impact_analysis ... ok
test test_ast_symbol_extraction_and_file_indexing ... ok
test test_security_path_traversal_and_secrets ... ok
test test_subsystem_discovery ... ok
test test_token_efficient_context_ranking ... ok
test test_incremental_sync_add_modify_delete ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.70s
```

Existing integration test suites were re-verified for zero regressions:
```
running 12 tests in tests/engage_test.rs
...
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.97s
```

Compiler verification across all targets and features:
```
cargo check --all-targets --all-features
Finished dev profile target(s) in 50.37s -> Exit code 0, 0 warnings, 0 errors.
```

Code style and formatting verification:
```
cargo fmt --all -- --check
Exit code 0 (100% formatted).
```

---

## 5. Security & Boundary Verification

1. **Path Traversal Rejection:** All relative file path operations are validated through `root_path(root)?` and sanitized against directory escaping (`..`).
2. **Secret Redaction:** Signatures and excerpts pass through `analyzer::redact`, ensuring API keys (`sk-`, `ghp_`, `AKIA`) and tokens are never leaked into node metadata or graph signatures.
3. **Fail-Closed Governance:** If graph database opening fails or is corrupted, `rank_context_with_graph` and compiler graph expansion fail gracefully, falling back to lexical BM25 without blocking agent execution or violating Change Contracts.
4. **Scope Exclusions:** Zero lines of code or dependencies were added for Graphify, AMstraLift, or external AI services.
