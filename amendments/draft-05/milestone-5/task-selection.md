# Milestone 5A: Engineering Task Selection & Specification
## Candidate Task Evaluation and Primary Task Selection

**Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/async-precommit-guard`  
**HEAD Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`  
**Document ID:** `ENGRAM-A05-M5A-TASK-SELECTION`  
**Classification:** Experimental Task Specification  

---

### 1. Task Selection Principles

To ensure that the controlled experiment provides an authentic, unbiased evaluation of Engram's context efficiency, tasks must adhere to six rigorous selection principles:

1. **Authentic Software Engineering:** Tasks must represent realistic implementation or refactoring work, not contrived synthetic puzzles.
2. **Architectural Exploration Required:** The task must require understanding symbols and relationships across multiple modules, preventing the agent from guessing the solution from the prompt alone.
3. **No Prior Ingestion Bias:** The task must not have been previously implemented or solved in the repository history, agent memory, or prompt examples.
4. **Independent Verifiability:** The task must have objective, automated verification criteria (unit tests, integration tests, compiler checks) that can be evaluated without human subjectivity.
5. **Bounded Blast Radius:** The task must have clear, authorized file boundaries to prevent unbounded scope drift.
6. **Zero External Dependencies:** The task must be completely buildable and testable offline using the existing toolchain.

---

### 2. Candidate Task 1: Small Localized Task
**Task Title:** Structured JSON Output for Query Explanation (`engram explain --json`)

#### 2.1 Overview & Requirements
Add a `--json` output formatting flag to the `engram explain` command. Currently, `engram explain` emits human-readable ASCII terminal diagrams. When `--json` is supplied, the command must serialize the AST explanation graph, entity dependencies, and confidence breakdown into a valid, schema-compliant JSON string.

#### 2.2 Technical Scope
- **Authorized Files:**
  - `src/main.rs` (CLI command argument parsing and JSON flag dispatch)
  - `src/printer.rs` (JSON serialization helper for `ExplainResult`)
  - `tests/explain_json_test.rs` (New integration test)
- **Relevant Dependencies:** `serde`, `serde_json`, `clap`.
- **Expected Complexity:** Low (2 source files, ~50 lines of code).
- **Acceptance Criteria:**
  - `cargo check --offline` passes with 0 warnings.
  - `engram explain <target> --json` outputs valid JSON parseable by `serde_json`.
  - JSON schema includes `target`, `confidence`, `dependencies`, and `rationale`.
  - Terminal human-readable output remains 100% backward-compatible when `--json` is omitted.
  - New integration test passes.
- **Suitability Assessment:**
  - *Strength:* Highly localized, fast verification.
  - *Weakness:* Similar to historical Task A, the prompt could easily give away the required changes, allowing an unassisted agent to solve it in 2–3 turns without exploring the codebase.

---

### 3. Candidate Task 2: Multi-File Implementation Task (RECOMMENDED PRIMARY TASK)
**Task Title:** Commit-Bound Cache Invalidation and Stale Index Cleanup

#### 3.1 Overview & Requirements
Currently, Engram's AST cache relies primarily on file modification timestamps (`mtime`) stored in `.engram/index.db`. When switching Git branches or applying stashes, file timestamps can appear fresh while the underlying Git commit has changed, leading to stale symbol graphs.
This task requires:
1. Recording the active Git commit HEAD hash in a new `meta` table in `.engram/index.db` during indexing.
2. In `ensure_fresh()`, checking whether the current repository HEAD matches the indexed commit hash.
3. If the commit differs, automatically triggering an incremental re-index of modified files between the cached commit and HEAD.
4. Adding an explicit CLI flag `engram clean --cache` that safely prunes orphaned index entries without corrupting user decision memories.

#### 3.2 Technical Scope
- **Authorized Files:**
  - `src/compiler.rs` (Updating `ensure_fresh` commit validation logic)
  - `src/analyzer.rs` (Updating SQLite `meta` table schema and read/write operations)
  - `src/main.rs` (Adding `engram clean --cache` CLI subcommand)
  - `tests/commit_cache_invalidation_test.rs` (New integration test)
- **Relevant Dependencies:** `rusqlite`, `git2` / standard process git, `chrono`.
- **Expected Complexity:** Medium (3 source files, 1 test file, ~150 lines of code).
- **Acceptance Criteria:**
  - SQLite database creates and maintains `meta` table with schema `(key TEXT PRIMARY KEY, val TEXT)`.
  - Switching commit HEAD triggers detection of commit drift and invalidates stale entries.
  - `engram clean --cache` purges AST cache while preserving `.engram/memories.json`.
  - All existing compiler tests (`cargo test --offline --lib compiler::tests`) pass.
  - New dedicated integration test passes with exit code 0.
- **Suitability Assessment:**
  - *Strength:* Requires understanding SQLite schema setup in `analyzer.rs`, cache freshness in `compiler.rs`, and CLI routing in `main.rs`. An unassisted agent must search across multiple files to locate where `index.db` is initialized and managed. Engram's context compiler can immediately supply these exact symbol definitions.

---

### 4. Candidate Task 3: Architectural Dependencies & High Regression Risk
**Task Title:** Worktree-Safe Pre-Commit Guard with Custom `hooksPath` Auto-Detection

#### 4.1 Overview & Requirements
Enhance Engram's `guard` and `hook` subsystem to automatically detect and support non-standard Git environments:
1. Git worktrees where `.git` is a pointer file rather than a directory.
2. Custom core `core.hooksPath` configurations.
3. Repositories with detached HEAD states or shallow clones.
The implementation must ensure that `engram guard install` installs hooks into the correct active worktree hook path without modifying global Git configurations or breaking shared bare repositories.

#### 4.2 Technical Scope
- **Authorized Files:**
  - `src/hook.rs` (Resolving worktree gitdir pointers and custom hook paths)
  - `src/guard.rs` (Contract evaluation on detached HEAD and shallow clones)
  - `src/ci_guard.rs` (CI exit code mapping for untracked worktrees)
  - `tests/worktree_guard_hardening_test.rs` (New integration test)
- **Relevant Dependencies:** `std::fs`, `std::path`, `git2`, `tempfile`.
- **Expected Complexity:** High (3 source files, 1 test file, ~250 lines of code).
- **Acceptance Criteria:**
  - Successfully installs hook in a secondary git worktree linked via `.git` pointer file.
  - Detects `core.hooksPath` if configured and installs hook to custom directory.
  - Reversible uninstallation cleans only Engram hooks, preserving pre-existing hooks.
  - Full hook integration test suite (`tests/hook_precommit_test.rs`, `tests/hook_pre_commit_test.rs`) passes with 0 regressions.
- **Suitability Assessment:**
  - *Strength:* High architectural complexity with substantial regression risk.
  - *Weakness:* High variability in agent exploration; high risk of ceiling overruns on both arms during initial experiments.

---

### 5. Candidate Evaluation & Comparison Matrix

| Evaluation Dimension | Task 1: JSON Explain | Task 2: Cache Invalidation (Selected) | Task 3: Worktree Hooks |
| :--- | :---: | :---: | :---: |
| **Architectural Scope** | Single subsystem (CLI/Printer) | Multi-module (Compiler/DB/CLI) | Core governance (Guard/Hook/Git) |
| **Exploration Necessity** | Low (Guessable from prompt) | **High (Requires symbol indexing)** | Very High (Plumbing intensive) |
| **Regression Risk** | Negligible | Low (Isolated caching logic) | High (May break git test suites) |
| **Expected Token Consumption** | ~40k–70k tokens | **~80k–140k tokens** | ~150k–260k tokens (Ceiling risk) |
| **Ceiling Overrun Risk** | Minimal | **Low (Well within 175k ceiling)** | Elevated (Likely to exceed 160k stop) |
| **Verification Objectivity** | Automated (CLI & JSON) | **Automated (SQLite & Tests)** | Automated (Git worktree tests) |
| **Context Compilation Value** | Marginal | **High (Directly targets DB & Compiler)** | High |

---

### 6. Primary Task Selection: Task 2

**Selection:** **Candidate Task 2: Commit-Bound Cache Invalidation and Stale Index Cleanup** is formally designated as the **Primary Experimental Task** for Milestone 5.

#### Rationale:
1. **Optimal Complexity Balance:** Task 2 is sufficiently complex to require architectural discovery (preventing Arm A from solving it in 2 turns), while bounded enough to comfortably complete within a conservative token ceiling (~100k tokens nominal).
2. **Direct Alignment with Engram Strengths:** Engram's AST context compiler shines specifically at indexing database access patterns, type declarations, and cross-file function calls—precisely what is required to navigate `analyzer.rs` and `compiler.rs`.
3. **Deterministic Automated Verification:** Can be completely verified via local SQLite assertions and offline Rust unit/integration tests without external network or API dependencies.
