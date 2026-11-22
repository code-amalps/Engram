# Engram Amendment 05 — Milestone 3: Self-Hosting Engineering Task Selection

**Document Version:** 1.0.0  
**Date:** 2026-10-30  
**Phase:** Track B — Candidate Engineering Task Evaluation  
**Status:** Evaluation Completed & Task Selected

---

## 1. Executive Summary

To establish an authentic self-hosting development loop, three genuine engineering challenges were identified directly from the architecture and limitations discovered during Milestone 2D and Milestone 3 Track A.

Rather than proposing superficial renames or cosmetic documentation edits, each candidate addresses a real structural friction point in Engram's developer workflow:
1. **Candidate Task 1:** Non-Git Fallback for Change Contract Diffing.
2. **Candidate Task 2:** Structured JSON/YAML Task Configuration and Schema Indexing.
3. **Candidate Task 3:** Subpath- and Language-Scoped Query Routing in Context Compilation.

Following comparative evaluation, **Candidate Task 3** was selected for full engineering handoff and context package generation.

---

## 2. Candidate Task 1: Non-Git Fallback for Change Contract Diffing

### 2.1 Problem Statement
`engram contract check` and preflight/postflight contract validation currently rely on Git CLI invocations (`git diff`, `git rev-parse`) to determine which files have changed relative to the baseline commit. In isolated test sandboxes, ephemeral CI containers, or environments where `.git` is omitted to preserve repository safety (as demonstrated in Track A), `contract check` fails closed with exit code 4:
```text
✗ Baseline commit 'WORKTREE' is unavailable in current Git history (shallow clone or missing commit).
```
This prevents agents in clean, non-Git sandboxes from validating their mutations against active Change Contracts.

### 2.2 Relevant Source Files
- `src/contract.rs` (contract lifecycle, evaluation logic, and baseline resolution)
- `src/guard.rs` (Change Guard diff inspection and violation classification)
- `src/storage.rs` (baseline commit hashing and workspace root resolution)

### 2.3 Expected Behavior
When `.git` is not detected in the workspace root, Engram should support a `--baseline-dir <PATH>` or snapshot tree digest fallback that compares the current directory against a clean baseline directory template via deterministic filesystem traversal, allowing contract validation to succeed in non-Git sandboxes.

### 2.4 Architectural Dependencies & Blast Radius
- **Dependencies:** Standard library `std::fs`, `sha2`, and SQLite WAL storage.
- **Blast Radius:** `contract.rs`, `guard.rs`, CLI command handlers in `main.rs`, and MCP tool `engram_postflight`.
- **Existing Tests:** `tests/contract_test.rs`, `tests/mcp_consolidation_test.rs`.

### 2.5 Acceptance Criteria & Offline Verifiability
- `contract check -C <non_git_workspace> --baseline-dir <baseline_snapshot>` returns exit code 0 when changes match the contract.
- 100% offline verifiable via local Rust unit tests.

---

## 3. Candidate Task 2: Structured JSON/YAML Task & Schema Indexing

### 3.1 Problem Statement
Engram's `analyzer.rs` only indexes languages supported by Tree-sitter grammars (`.rs`, `.py`, `.ts`, `.js`, `.go`, `.java`, `.cs`). Configuration files that define critical benchmark task contracts (such as `benchmarks/matched-pair-harness/config/tasks/*.json` and `schemas/*.schema.json`) are ignored during repository analysis. When a developer or agent queries for task properties (e.g., `smoke_test_read_only`), Engram cannot locate the configuration because it is absent from the `files` and `symbols` tables.

### 3.2 Relevant Source Files
- `src/analyzer.rs` (file extension mapping and AST symbol extraction)
- `src/storage.rs` (database schema for `files` and `symbols`)
- `src/compiler.rs` (lexical search and relevance scoring)

### 3.3 Expected Behavior
Extend `analyzer.rs` to detect `.json` and `.yaml` configuration files, parse them using `serde_json` / `serde_yaml`, and index top-level keys, task IDs, schema titles, and requirement IDs into `derived_symbols.db` with symbol kind `CONFIG_PROPERTY` or `SCHEMA_PROPERTY`.

### 3.4 Architectural Dependencies & Blast Radius
- **Dependencies:** `serde_json`, `serde_yaml`, rusqlite schema in `storage.rs`.
- **Blast Radius:** Database migrations in `storage.rs`, symbol querying in `impact.rs` and `inspect.rs`.
- **Existing Tests:** `tests/analyzer_test.rs`, `tests/empirical_test.rs`.

### 3.5 Acceptance Criteria & Offline Verifiability
- `engram analyze` indexes JSON task configurations.
- `engram inspect smoke_test_read_only` resolves to `benchmarks/matched-pair-harness/config/tasks/smoke_test_read_only.json`.
- 100% offline verifiable via local Rust unit tests.

---

## 4. Candidate Task 3: Subpath- and Language-Scoped Query Routing

### 4.1 Problem Statement
Engram is a dual-language polyglot repository containing both a core Rust binary in `src/` and a comprehensive Python benchmark harness in `benchmarks/matched-pair-harness/`. In both Milestone 2D and Milestone 3 Track A, broad queries like *"matched-pair harness orchestrator execution mode"* resulted in cross-component BM25 lexical competition:
- `src/empirical_runner.rs` (Rust) and `tests/empirical_live_test.rs` competed with `benchmarks/matched-pair-harness/src/orchestrator.py` (Python).
- In a 1,200-token context budget, this competition resulted in the selection of unrelated files or test mocks, crowding out the actual target implementation files.

Currently, `engram context` and `engram prompt` scan the entire repository unconditionally (`SELECT path, language, excerpt FROM files`) without any path or language constraints.

### 4.2 Relevant Source Files
- `src/main.rs`: CLI argument parsing for `Commands::Context` and `Commands::Prompt`.
- `src/compiler.rs`: `pub fn context(...)` SQL querying, candidate filtering, and budget packing.
- `src/prompt.rs`: `pub fn generate_prompt(...)` and `prepare_task_context(...)` parameter routing.

### 4.3 Expected Behavior
Introduce `--subpath <PATH>` and `--language <LANG>` options to `engram context` and `engram prompt`:
1. When `--subpath` is provided, candidate file selection is constrained to paths starting with that prefix (`path.starts_with(subpath)`).
2. When `--language` is provided, candidate file selection is constrained to files matching that language (`language == target_lang`).
3. Cross-language and cross-component lexical pollution is completely eliminated, allowing 100% of the token budget to focus on the target component.

### 4.4 Architectural Dependencies & Blast Radius
- **Dependencies:** `clap` CLI parser, SQLite query filtering in `compiler.rs`.
- **Blast Radius:** `main.rs`, `compiler.rs`, `prompt.rs`, and MCP tools `engram_get_context` and `engram_preflight`.
- **Existing Tests:** `tests/compiler_test.rs`, `tests/prompt_test.rs`.

### 4.5 Acceptance Criteria & Offline Verifiability
- `engram prompt --subpath benchmarks/matched-pair-harness "<query>"` selects only files within `benchmarks/matched-pair-harness/`.
- `engram prompt --language rust "<query>"` selects only Rust source files.
- 100% offline verifiable via deterministic CLI and unit tests without external inference.

---

## 5. Comparative Assessment & Final Selection

| Assessment Criterion | Candidate Task 1<br>(Non-Git Fallback) | Candidate Task 2<br>(JSON/YAML Indexing) | Candidate Task 3<br>(Subpath/Language Scoping) |
|:---|:---:|:---:|:---:|
| **Architectural Impact** | High (enables sandbox validation) | Medium (broadens symbol table) | **High (directly improves retrieval quality)** |
| **Complexity & Risk** | Medium (diff engine required) | High (database schema change) | **Low to Medium (additive query filters)** |
| **Token Reduction Value** | Indirect (safety guard) | Indirect (better coverage) | **Immediate & Direct (eliminates distractor files)** |
| **Safety / Hermetic Isolation** | High | High | **Extremely High (purely local query filtering)** |
| **Offline Testability** | 100% Offline | 100% Offline | **100% Offline** |

### Selected Task: Candidate Task 3
**"Implement Subpath- and Language-Scoped Query Routing in Engram Context Compiler and Prompt Generator"**

**Rationale for Selection:**  
Candidate Task 3 directly targets the primary retrieval limitation discovered during empirical self-hosting audits in Milestones 2D and 3. By providing subpath and language scoping, developers and coding agents can cleanly isolate specific architectural components in multi-language or monorepo environments, eliminating lexical false positives and maximizing the density of the compiled context package.
