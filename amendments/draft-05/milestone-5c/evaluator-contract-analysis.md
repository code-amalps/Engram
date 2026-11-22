# Evaluator Contract Analysis: `task_commit_cache_invalidation`

**Milestone:** Engram Amendment 05 — Milestone 5C  
**Document ID:** `amendments/draft-05/milestone-5c/evaluator-contract-analysis.md`  
**Evaluation Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Target Task:** `task_commit_cache_invalidation`  
**Status:** AUDITED & SPECIFICATION BOUND  

---

## 1. Executive Summary

During the N=1 pilot preflight audit, a critical gap was identified in `benchmarks/matched-pair-harness/src/evaluator.py`: while the task configuration for `task_commit_cache_invalidation` had been created and validated against JSON schemas in Milestone 5B, the evaluator's static inspection engine lacked domain-specific handlers for this task. Consequently, any candidate solution—regardless of correctness—would fail REQ-1 and REQ-3, capping the correctness score at $C = 0.50$ and artificially triggering mutual acceptance failure.

This document formally establishes the evaluator contract for `task_commit_cache_invalidation`, analyzing the four registered requirements, their verification mechanisms, and the invariants required to maintain scientific integrity.

---

## 2. Immutable Task Specification and Requirements

The contract is defined in `benchmarks/matched-pair-harness/config/tasks/task_commit_cache_invalidation.json`:

```json
{
  "schema_version": "2.0.0",
  "task_id": "task_commit_cache_invalidation",
  "repository_type": "cargo",
  "baseline_commit": "c70845b4d5913457f1b176fb0370b80dd792711f"
}
```

### 2.1 The Four Registered Requirements
The evaluation contract partitions functional correctness into four orthogonal requirements, each carrying a weight of $0.25$ (summing to $1.0$):

| Requirement ID | Weight | Evaluation Type | Specification Description | Evaluator Verification Criteria |
|---|---|---|---|---|
| **REQ-1** | 0.25 | `STATIC_CHECK` | Record active Git commit HEAD in SQLite meta table | SQLite `index.db` (or `derived_symbols.db`) contains `meta` table with active `git_commit_head` entry, backed by AST/SQL persistence in source code (`src/analyzer.rs` / `src/storage.rs`). |
| **REQ-2** | 0.25 | `UNIT_TEST` | Commit drift detection invalidates stale cache entries | In `ensure_fresh()`, the cached commit hash is compared against current repository HEAD; divergence triggers invalidation/re-indexing; verified via L2 unit tests and AST inspection. |
| **REQ-3** | 0.25 | `STATIC_CHECK` | Add `engram clean --cache` subcommand preserving memories | `CommandLine` enum in `src/main.rs` accepts `clean --cache` to purge AST SQLite database while strictly preserving `.engram/memories.json`. |
| **REQ-4** | 0.25 | `AUTOMATED_DIFF` | Boundary adherence and regression prevention | Modifications strictly confined to authorized files (`src/compiler.rs`, `src/analyzer.rs`, `src/main.rs`, `tests/commit_cache_invalidation_test.rs`); zero changes to `Cargo.toml`, `Cargo.lock`, or unauthorized modules. |

---

## 3. Verification Gates Contract

In `task_commit_cache_invalidation.json` (`lines 57–76`), automated compilation and testing gates are mapped directly to Cargo commands:

| Gate | Target Phase | Command | Gate Behavior |
|---|---|---|---|
| **L0** | Typecheck | `cargo check --offline` | Verifies syntactic and semantic compilation integrity without external network calls. Non-zero exit code halts gate sequence. |
| **L1** | Lint | `cargo clippy --offline` | Disabled (`enabled: false`) to avoid subjective stylistic gating. |
| **L2** | Unit Tests | `cargo test --offline --lib compiler::tests` | Executes core unit tests offline. Must pass cleanly for REQ-2 eligibility. |
| **L3** | Build | `cargo build --offline` | Verifies full production binary compilation, generating `target/debug/engram.exe` for executable acceptance testing. |

---

## 4. Blast Radius and Dependency Invariants

### 4.1 Authorized Files
The task contract strictly limits the agent's modification footprint to four files:
- `src/compiler.rs`
- `src/analyzer.rs`
- `src/main.rs`
- `tests/commit_cache_invalidation_test.rs`

### 4.2 Forbidden Files & Dependency Mutation
The following files are explicitly forbidden:
- `Cargo.toml`
- `Cargo.lock`
- `src/mcp.rs`
- `src/guard.rs`
- `src/ci_guard.rs`
- `src/hook.rs`
- `.engram/memories.json`

### 4.3 Evaluator Enforcement Mechanism
In `evaluator.py`:
1. `analyze_git_impact`: Checks `git status --porcelain` and `git diff HEAD --numstat`. Any modification to an unauthorized file increments `unrelated_files_count`, forcing REQ-4 to `FAIL`.
2. `compute_cargo_manifest`: Pre- and post-run SHA-256 manifests of `Cargo.toml` and `Cargo.lock` are compared. Any drift sets `dependency_mutated = True`.
3. `calculate_score`: If `dependency_mutated == True`, correctness score is overridden to $C = 0.0$, all requirements evaluate to `FAIL`, and quality-adjusted token cost is penalized $100\times$.

---

## 5. Root Cause of Preflight Evaluator Deficiency

Prior to Milestone 5C, `_check_static_requirement` in `evaluator.py` implemented branch handling only for `task_a_typo`, `task_b_remember_me`, and `task_c_theme_toggle`:
```python
if self.task_id == "task_a_typo": ...
elif self.task_id == "task_b_remember_me": ...
elif self.task_id == "task_c_theme_toggle": ...
return False
```
Because no handler existed for `task_commit_cache_invalidation`:
1. REQ-1 (`STATIC_CHECK`) fell through to `return False` $\rightarrow$ Result: `FAIL`.
2. REQ-3 (`STATIC_CHECK`) fell through to `return False` $\rightarrow$ Result: `FAIL`.
3. Even with REQ-2 and REQ-4 passing, total correctness was capped at:
   $$C = 0.00 + 0.25 + 0.00 + 0.25 = 0.50$$
4. Under `compute_comparative_analysis`, $C = 0.50 < 1.00$ caused `passed_all_requirements = False`, triggering:
   `MUTUAL_ACCEPTANCE_FAILURE: Both arms failed functional correctness requirements`.

Remediating `evaluator.py` to evaluate REQ-1, REQ-2, and REQ-3 via deterministic, executable, and AST-backed checks removes this false blocker while preserving absolute rigor.
