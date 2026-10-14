# Engram Architecture

Engram is a local repository intelligence engine and change boundary system built in Rust. It runs locally as a standalone binary or JSON-RPC 2.0 Model Context Protocol (MCP) server for AI coding assistants.

```text
 ┌─────────────────────────────────────────────────────────────────────────────┐
 │                             AI Coding Assistant                             │
 │                  (Cursor, Claude Desktop, Windsurf, Copilot)                │
 └──────────────────────────────────────┬──────────────────────────────────────┘
                                        │ stdio / CLI
                                        ▼
 ┌─────────────────────────────────────────────────────────────────────────────┐
 │                         Engram Intelligence Core                            │
 ├──────────────────────────────────────┬──────────────────────────────────────┤
 │  1. Context Intelligence             │  2. Change-Impact Intelligence       │
 │     • BM25 Lexical Grounding         │     • 7 Relationship Categories      │
 │     • Token Budget Slicing           │     • Multi-Hop Blast Radius         │
 │     • Relevance Provenance           │     • Causal Breadcrumbs             │
 ├──────────────────────────────────────┼──────────────────────────────────────┤
 │  3. Change Contracts & Change Guard  │  4. Stateful Automated Recovery      │
 │     • Preflight Boundary Handshake   │     • Bounded Retry (Max 2 attempts) │
 │     • Git Diff Working-Tree Guard    │     • Revert & Preserve Plans        │
 │     • Constitution Precedence        │     • Never directly modifies code   │
 ├──────────────────────────────────────┼──────────────────────────────────────┤
 │  5. Engineering Memory               │  6. Architecture Drift               │
 │     • Scoped Hierarchy (Symbol→Repo) │     • Scoped Staleness Tracking      │
 │     • Immutable Supersession Chains  │     • Explicit Human Ratification    │
 │     • Zero-Poisoning Gate            │     • Boundary Erosion Trends        │
 └──────────────────────────────────────┴──────────────────────────────────────┘
                                        │
                                        ▼
 ┌─────────────────────────────────────────────────────────────────────────────┐
 │                              Storage & Cache                                │
 │   • Local SQLite WAL (.engram/index.db) — Bi-temporal entity graph          │
 │   • Two-Tier Invalidation: Tier 1 (Content Hash) + Tier 2 (Relational Link)  │
 │   • Append-Only Cryptographic Telemetry Hash Chain (.engram/events/)        │
 └─────────────────────────────────────────────────────────────────────────────┘
```

---

## 1. Storage & State Model

Engram persists its knowledge in the local `.engram/` folder inside your repository:

- `.engram/index.db`: An embedded SQLite database using Write-Ahead Logging (WAL). It stores bi-temporal entity versions (`entities`, `entity_versions`, `entity_identity`), relationships (`relationships`), AST observations (`observations`), and confidence-scored evidence (`evidence`).
- `.engram/memory.yaml`: Durable engineering decisions and architectural rationale. Kept in human-readable, Git-versionable YAML.
- `.engram/constitution.yaml`: Team-wide architectural boundary rules and dependency restrictions that take absolute precedence over contract boundaries.
- `.engram/events/`: Append-only cryptographic hash-chained audit events (`previous_event_hash` $\to$ `event_hash`).
- `.engram/cache/`: Local ephemeral runtime caches, including active recovery sessions (`.engram/cache/recovery/<contract-id>.json`).

---

## 2. Two-Tier Incremental Invalidation

Rather than rescanning the whole repository on every prompt, Engram uses a two-tier cache invalidation pipeline:

1. **Authoritative Content Hashing (Tier 1 - Syntactic):**
   - For Git-tracked files, Git blob hashes and working-tree content SHA-256 hashes are authoritative.
   - `mtime` and file size are strictly fast-path optimizations: if both are unchanged, file byte reading is short-circuited.
   - Only files whose content hash actually changes are reparsed via Tree-sitter.

2. **Relational Graph Invalidation (Tier 2 - Relational):**
   - When a file is reparsed, Engram queries SQLite for all entities that import, implement, or call the modified symbols.
   - Only those related edges are updated in the relational graph.

---

## 3. Tree-Sitter Polyglot AST Extractors

Engram embeds Tree-sitter parsers directly into the native binary for 7 languages:
- **Rust** (`tree-sitter-rust`)
- **Go** (`tree-sitter-go` + native `go.mod` parser)
- **TypeScript** (`tree-sitter-typescript`)
- **JavaScript** (`tree-sitter-javascript`)
- **Python** (`tree-sitter-python`)
- **C#** (`tree-sitter-c-sharp`)
- **Java** (`tree-sitter-java` + native `pom.xml` & `build.gradle` parsers)

Extractors capture:
- Module and package declarations
- Class, interface, struct, record, enum, and trait definitions
- Method, function, and constructor signatures
- Import and dependency declarations
- Structural interface satisfaction (Go and Java)
- Test-to-implementation naming conventions

---

## 4. Change Contracts & The Change Guard

Change Contracts establish formal, machine-readable boundaries for AI code modifications.

### Lifecycle:
1. **Preflight Handshake:** Before the AI begins writing code, `engram preflight` or `engram contract create` generates a contract anchored to the current Git baseline commit. The contract declares `expected_files`, `expected_modules`, and `expected_symbols`, along with `forbidden_boundaries`.
2. **AI Mutation:** The agent edits code in the working tree.
3. **Postflight Verification:** `engram postflight` or `engram contract check` evaluates the Git diff against the contract.
4. **Constitution Precedence:** Rules in `.engram/constitution.yaml` take absolute precedence over contract allowances. If a rule forbids `src/auth/**`, an edit to that path is rejected as a boundary violation even if an agent's contract declared it expected.

---

## 5. Stateful Automated Recovery

When a Change Guard check blocks an agent with a boundary violation, Engram does not leave the agent stranded or force an infinite retry loop:

- **Safety Invariant:** Engram **never** modifies source code directly. It emits a structured, machine-readable recovery plan containing:
  - `revert_mutation`: Specific files to revert that breached boundaries.
  - `preserve_requested_change`: Authorized changes that should be retained.
  - `rerun`: Instructions to re-evaluate compliance.
- **Bounded Retry Invariant:** Recovery is strictly bounded to a maximum of 2 authorized execution attempts:
  - 1st violation: Attempt 1 plan generated.
  - 2nd violation: Attempt 2 retry plan generated.
  - 3rd violation: Request rejected with status `BLOCKED_AWAITING_HUMAN`. Engram will not execute a 3rd recovery.
- **Concurrency Protection:** Recovery sessions use an atomic compare-and-update (CAS) lock on `(session_version, lock_state)` to prevent race conditions when multiple agent sub-tasks run simultaneously.

---

## 6. Engineering Memory Lifecycle

Engram preserves architectural rationale across sessions while defending against memory poisoning:

- **Scoped Hierarchy:** Memories are attached to specific scopes: `Global` < `Repository` < `Module` < `Package` < `Symbol`.
- **Immutable Supersession:** Older decisions are not silently deleted or rewritten. A newer decision points to predecessor records via explicit supersession chains (e.g. `ADR-014` $\to$ `ADR-029`).
- **Zero-Poisoning Gate:** Proposed agent decisions remain ephemeral candidates. No decision enters durable memory (`.engram/memory.yaml`) without explicit human promotion (`engram memory promote <id>`).

---

## 7. Model Context Protocol (MCP) Integration

Engram implements the JSON-RPC 2.0 MCP standard over `stdio`. Supported tools:
- `engram_context`: Task-grounded context compilation within token budget.
- `engram_inspect`: Blast radius and downstream dependency impact cone.
- `engram_preflight`: Contract generation and boundary handshake.
- `engram_postflight`: Working-tree boundary verification.
- `engram_recover`: Structured recovery plan generation for boundary violations.
- `engram_drift`: Scoped architectural staleness and boundary erosion trends.
- `engram_memory_record` / `engram_memory_list`: Decision recording and querying.
