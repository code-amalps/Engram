# Phase 5.8: Phase 5.5 Optimization Source & Correctness Audit

**Document ID:** `ENGRAM-PHASE-5.8-OPTIMIZATION-AUDIT`  
**Date:** 2026-11-22  
**Audit Scope:** Implementation audit of Optimizations A through G in `src/budget.rs`, `src/compiler.rs`, `src/mcp.rs`, and `src/retrieval.rs`.  

---

## 1. Optimization A: Task-Aware Intra-File Symbol Ranking

- **Implementation Location:** `src/compiler.rs` (`extract_symbols_and_excerpts`)
- **Mechanism:** Implements a 5-tier query-relevance sorting hierarchy:
  1. *Tier 1 (Exact Match):* Symbol name matches query token exactly (Score: 100).
  2. *Tier 2 (Targeted Symbols):* Explicit user request via `symbols: Vec<String>` (Score: 80).
  3. *Tier 3 (Sub-token Match):* Case-insensitive substring match (Score: 50).
  4. *Tier 4 (Exported / Public Interface):* Public struct, trait, or function (Score: 20).
  5. *Tier 5 (Private / Internal Helper):* Default base tier (Score: 5).
- **Audit Findings:**
  - Solves the intra-file symbol starvation problem where early helper functions at line 1..50 starved downstream critical symbols.
  - Excerpt generation prioritizes high-scoring symbols before token degradation ladders truncate the payload.
  - Tie-breaking defaults to natural source file line order, preserving stability.

---

## 2. Optimization B: Progressive Disclosure

- **Implementation Location:** `src/budget.rs`, `src/compiler.rs`, `src/mcp.rs`
- **Mechanism:** Introduces `DetailLevel::Compact` alongside `DetailLevel::Full` and `DetailLevel::Contracted`.
- **Payload Structure:**
  - Emits `SYMBOLS (COMPACT METADATA)` headers.
  - Includes symbol kind, name, line range, and function signature.
  - Omits complete function bodies and internal implementation blocks.
- **Audit Findings:**
  - Reduces exploratory prompt payload from 538 tokens to 124 tokens (**-77.0%**).
  - Source locations (line numbers) remain 100% accurate.
  - Receiving agents can request complete bodies in subsequent turns via `symbols: ["TargetSymbol"]` or `detail_level: "full"`.

---

## 3. Optimization C: Cross-File Context Deduplication

- **Implementation Location:** `src/compiler.rs` (`extract_symbols_and_excerpts`)
- **Mechanism:** Maintains an in-memory hash set of seen normalized symbol signatures across files within a single context compilation request.
- **Audit Findings:**
  - When an identical signature repeats (e.g. standard interface declarations across mock and real services), subsequent occurrences are tagged `[DEDUPLICATED]` and their bodies omitted.
  - Delivers a 50.0% token reduction on repeated type definitions.
  - Path preservation ensures the agent knows the symbol exists in both files.

---

## 4. Optimization D: Adaptive MCP Bypass

- **Implementation Location:** `src/mcp.rs` (`classify_task_complexity`, `engram_get_context`)
- **Mechanism:** Heuristic classifier evaluating task intent against single-file typo keywords (`typo`, `comment`, `spelling`, `rename variable`).
- **Audit Findings:**
  - Returns `status: "BYPASS_TRIVIAL"`, `estimated_tokens: 0`, and `target_files: []`.
  - Saves 100% of context tokens for self-contained edits.
  - **Risk Assessment:** False bypass risk exists if an ambiguous short task appears trivial but actually requires architecture memory. Verified that tasks mentioning multi-file keywords (`refactor`, `database`, `auth`, `storage`) correctly bypass the bypass, returning full context.

---

## 5. Optimization E: Token Budget Bounding

- **Implementation Location:** `src/budget.rs`
- **Mechanism:** Enforces a strict 16 KB hard ceiling via an 8-level degradation ladder (Levels 0 through 7).
- **Audit Findings:**
  - Ladder progressively strips private function bodies, companion templates, and non-essential imports before applying string truncation.
  - Safe UTF-8 truncation (`char_indices`) prevents byte splitting and invalid JSON payloads.

---

## 6. Optimization F: Telemetry & Measurement Isolation

- **Implementation Location:** `src/compiler.rs`, `src/mcp.rs`
- **Mechanism:** Local structured JSON metrics (`estimated_tokens`, `elapsed_ms`, `cache_hit`).
- **Audit Findings:**
  - Purely local telemetry; zero external network transmissions.
  - Credentials and sensitive paths are excluded from telemetry records.

---

## 7. Optimization G: In-Memory LRU Cache — CRITICAL DEFECT IDENTIFIED

- **Implementation Location:** `src/compiler.rs` (`COMPILED_CONTEXT_CACHE`)
- **Mechanism:** 32-entry in-memory LRU cache storing pre-compiled `CompiledContext` objects.
- **Cache Key Formulation:**
  ```rust
  let cache_key = format!(
      "{}:{}:{}:{}:{}:{:?}:{:?}:{}",
      root.display(),
      task,
      budget,
      subpath.unwrap_or(""),
      language.unwrap_or(""),
      detail_level,
      target_symbols,
      mem_ver
  );
  ```
- **CRITICAL DEFECT FINDING (DEFECT-5.8-01):**
  - **The cache key does NOT incorporate SQLite index revision, database mtime, or working-tree file hashes.**
  - In non-git workspaces, or in workspaces where files are created, modified, or deleted without a Git commit, `cache_key` remains invariant between successive calls.
  - **Empirical Demonstration:**
    - `tests/query_router_test.rs::test_15_missing_or_invalidated_entities` creates `temp_service.rs`, analyzes, calls `context()`, then deletes `temp_service.rs`, re-analyzes, and calls `context()` again.
    - Because `task`, `budget`, and `mem_ver` were unchanged, the cache returned the previous result containing the deleted file!
    - The test failed on assertion: `assert!(!res2.files.iter().any(|f| f.path.contains("temp_service.rs")))`.
  - **Severity:** HIGH (Stale Context Retrieval).
  - **Remediation Recommendation:** Must include `.engram/cache.db` mtime or index generation version in the cache key.
