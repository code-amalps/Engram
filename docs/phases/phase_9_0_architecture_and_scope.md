# Engram Phase 9.0: Native Micro-Knowledge Graph Architecture & Scope Specification

**Phase:** 9.0  
**Document Identifier:** `ENGRAM-DOC-PHASE-9-0-ARCH-20261123`  
**Phase Title:** Native Micro-Knowledge Graph, Repository Intelligence & Token-Efficient Context Engineering  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**GitHub Repository:** `code-amalps/Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Baseline Commit:** `c27261bf00f90f3d23c7ab168bb9adf36dc39b85` (equivalent to `d01f84d4ac358d8f6c9ec396730b5eec0d877929`)  
**Target Release Date:** 27 November 2026  
**Status:** `ARCHITECTURE_APPROVED`  

---

## 1. Executive Summary & Core Mission

The mission of Phase 9.0 is to design and implement an independent, native **Engram Micro-Knowledge Graph** that equips AI coding agents with deep, persistent, structured understanding of a software repository without relying on external dependencies or third-party graph tools (such as Graphify or AMstraLift).

### Primary Product Objective:
**Reduce repeated repository exploration and unnecessary AI context consumption** by providing Engram with a reliable, incrementally maintained, deterministic graph index of the codebase.

The native micro-knowledge graph serves as an intelligent retrieval and ranking engine for Engram's existing context compiler (`compiler::compile_context`) and unified engagement workflow (`engage::engram_engage`), enabling agents to retrieve the exact AST symbols, dependencies, tests, and contracts required for a task while filtering out thousands of irrelevant tokens.

---

## 2. Architectural Principles & Boundaries

1. **Zero External Runtime Dependencies:**
   - Pure Rust implementation leveraging Engram's existing SQLite database infrastructure (`rusqlite`) and Tree-sitter AST parsers.
   - Zero external Python daemons, zero external graph databases, zero cloud dependencies, and zero mandatory LLM extraction calls.
2. **Deterministic & Offline-First:**
   - 100% offline and repository-local (`.engram/graph.db` or dedicated SQLite tables).
   - Deterministic AST extraction ensuring identical repository inputs produce identical graph states.
3. **Change-Aware Incremental Indexing:**
   - Detects file changes, additions, renames, and deletions via SHA-256 fingerprints.
   - Re-indexes only affected files and invalidates dependent edges without requiring full repository scans for every task.
4. **Strict Evidence & Provenance Tiers:**
   - Distinguishes directly extracted facts (AST symbols, explicit imports) from derived relations (transitive dependencies) and inferred relationships (naming conventions).
   - Every graph node and edge is traceable to its relative path, line range, and content hash.
5. **Context Integration Without Duplication:**
   - Augments the existing context compilation pipeline; does not create a competing context format or parallel endpoint.
   - Preserves source verification: graph intelligence indicates *what* code to inspect; agents always receive verified source snippets.

---

## 3. Seven-Layer System Architecture

```mermaid
flowchart TD
    subgraph Layer A: Deterministic Code Intelligence
        P_RS["Rust Parser (Tree-sitter)"]
        P_PY["Python Parser (Tree-sitter)"]
        P_TS["TS/JS Parser (Tree-sitter)"]
        P_GO["Go Parser (Tree-sitter)"]
        P_JV["Java Parser (Tree-sitter)"]
        P_CS["C# Parser (Tree-sitter)"]
        P_CFG["Config & Policy Parsers"]
    end

    subgraph Layer B: Typed Graph Data Model
        Nodes["Nodes: Repo, Dir, File, Module, Symbol, Type, Function, Method, Endpoint, Test, Contract, Policy"]
        Edges["Edges: CONTAINS, IMPORTS, DEPENDS_ON, CALLS, REFERENCES, IMPLEMENTS, EXTENDS, DECLARES, TESTS, GOVERNED_BY, DOCUMENTS, CONFIGURES"]
    end

    subgraph Layer C: Evidence & Provenance Engine
        Prov["Source Path, Line Range, Content SHA-256, Extraction Method, Tier (Direct/Derived/Inferred), Confidence"]
    end

    subgraph Layer D: Incremental Maintenance & Persistence
        Inc["Change-Aware Invalidator & SQLite Graph Engine (.engram/index.db)<br>States: READY, INDEXING, PARTIALLY_INDEXED, STALE, UNAVAILABLE, CORRUPT"]
    end

    subgraph Layer E: Query Engine
        Q_Neigh["Neighborhood Query (Bounded BFS)"]
        Q_Dep["Dependency Query (Direct & Transitive)"]
        Q_Call["Call-Path Query (Tracing Call Chains)"]
        Q_Imp["Impact Query (Blast Radius & Dependents)"]
        Q_Sub["Subsystem Query (Architecture & Modules)"]
    end

    subgraph Layer F: Context Compiler & engram_engage Integration
        Compiler["src/compiler.rs & src/engage.rs<br>• Task Grounding & Symbol Identification<br>• Graph-Assisted Pruning & Context Ranking<br>• 40%+ Token Budget Savings"]
    end

    subgraph Layer G: Lean MCP & CLI Exposure
        MCP["MCP: engram_engage (Enriched)<br>CLI: engram graph (query, impact, status)"]
    end

    Layer A --> Layer B
    Layer B --> Layer C
    Layer C --> Layer D
    Layer D --> Layer E
    Layer E --> Layer F
    Layer F --> Layer G
```

---

## 4. Parser Inventory & Supported Languages

Engram's Tree-sitter infrastructure currently supports 7 core programming languages and configuration formats:

| Language / Format | Extensions | Parser Engine | Extracted Constructs |
| :--- | :--- | :--- | :--- |
| **Rust** | `.rs` | `tree-sitter-rust` | Structs, enums, traits, functions, methods, impl blocks, calls, imports, tests (`#[test]`) |
| **Python** | `.py` | `tree-sitter-python` | Classes, functions, methods, imports (`from/import`), calls, test functions (`test_*`) |
| **TypeScript / JS** | `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs` | `tree-sitter-typescript` / `javascript` | Classes, interfaces, types, functions, arrow functions, methods, imports, exports, calls |
| **Go** | `.go` | `tree-sitter-go` | Structs, interfaces, functions, methods, package imports, calls, test functions (`Test*`) |
| **Java** | `.java` | `tree-sitter-java` | Classes, interfaces, enums, methods, constructors, imports, annotations (`@Test`) |
| **C#** | `.cs` | `tree-sitter-c-sharp` | Classes, interfaces, structs, methods, properties, using directives, calls |
| **Config & Policy** | `.yaml`, `.yml`, `.toml`, `.json` | Serde Parsers | Constitution rules, package manifests (`Cargo.toml`, `package.json`), contracts |

*Unsupported language constructs are explicitly classified with `ParseStatus::Partial` rather than guessed.*

---

## 5. Token-Efficiency Evaluation Strategy

To validate that native micro-knowledge graph intelligence achieves measurable token reductions without sacrificing correctness, a reproducible benchmark framework evaluates 5 standard coding tasks:

1. **Simple Bug Fix (Single-file leaf edit):** Measure token savings through immediate graph-assisted isolation vs full file AST dumps.
2. **Function-Level Change (Local call-chain):** Measure accuracy of 1-hop caller/callee context compilation.
3. **Cross-Module Feature (Multi-file interface implementation):** Measure reduction in exploratory turn inflation.
4. **API Endpoint Change (Route-to-handler propagation):** Measure blast-radius context precision.
5. **Contract / Security-Sensitive Change:** Measure inclusion of governing policies without irrelevant code bloat.

### Metrics Recorded:
- Serialized context bytes ($B_{\text{graph}}$ vs $B_{\text{base}}$).
- Estimated token count ($T_{\text{graph}}$ vs $T_{\text{base}}$) using standard 4 chars/token heuristic.
- Number of files inspected and pruned.
- Query latency in milliseconds ($ms$).
- Context completeness (% necessary symbols preserved).
