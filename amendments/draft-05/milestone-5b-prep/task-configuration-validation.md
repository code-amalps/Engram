# Milestone 5B-Prep: Task Configuration Validation

## 1. Objective and Registration

Milestone 5A established the prerequisite of a representative Rust engineering task (Task C) targeting Engram's self-hosting capabilities:
`task_commit_cache_invalidation.json`.

This deliverable records the validation of `benchmarks/matched-pair-harness/config/tasks/task_commit_cache_invalidation.json` against the harness schema, fixture preflight checks, and evaluation gates.

---

## 2. Task Specification Overview

- **Task Identifier:** `task_commit_cache_invalidation`
- **Schema Version:** `2.0.0` (Conforms to `schemas/correctness_checklist.schema.json`)
- **Title:** Commit-Bound AST Cache Invalidation and Stale Index Cleanup
- **Target Repository Type:** `cargo`
- **Baseline Git Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`
- **Allowed Edit Scope:**
  - `src/compiler.rs`
  - `src/analyzer.rs`
  - `src/main.rs`
  - `tests/commit_cache_invalidation_test.rs`
- **Forbidden Blast Radius Scope:**
  - `Cargo.toml`
  - `Cargo.lock`
  - `src/mcp.rs`
  - `src/guard.rs`
  - `src/ci_guard.rs`
  - `src/hook.rs`
  - `.engram/memories.json`

---

## 3. Schema Validation Results

The JSON task definition was validated using `schema_validator.validate_instance`:
```
Schema: benchmarks/matched-pair-harness/schemas/correctness_checklist.schema.json
Instance: benchmarks/matched-pair-harness/config/tasks/task_commit_cache_invalidation.json
Result: VALIDATION_SUCCESS (0 schema errors)
```

### Required Fields Check:
- `task_id`: PRESENT (`"task_commit_cache_invalidation"`)
- `schema_version`: PRESENT (`"2.0.0"`)
- `repository_type`: PRESENT (`"cargo"`)
- `read_only`: PRESENT (`false`)
- `requires_dependencies`: PRESENT (`false`)
- `baseline_commit`: PRESENT (`"c70845b4d5913457f1b176fb0370b80dd792711f"`)
- `fixture_preflight`: PRESENT (Asserts presence of `src/compiler.rs`, `src/analyzer.rs`, `src/main.rs`, and patterns `pub fn context`, `pub fn ensure_fresh`, `enum CommandLine`)
- `gates`: PRESENT (L0 `cargo check --offline`, L2 `cargo test --offline --lib compiler::tests`, L3 `cargo build --offline`)
- `requirements`: PRESENT (4 requirements, total weight = 1.0)

---

## 4. Requirement Breakdown & Evaluation Matrix

| ID | Description | Weight | Evaluation Method | Success Criteria |
|---|---|---|---|---|
| **REQ-1** | Record active Git commit HEAD in SQLite meta table | 0.25 | STATIC_CHECK | `index.db` contains meta table recording `git_commit_head` |
| **REQ-2** | Commit drift detection invalidates stale cache entries | 0.25 | UNIT_TEST | `ensure_fresh()` triggers re-index when repo HEAD diverges from cached commit |
| **REQ-3** | Add `engram clean --cache` subcommand preserving memories | 0.25 | STATIC_CHECK | Clean subcommand removes SQLite cache without deleting `memories.json` |
| **REQ-4** | Boundary adherence and regression prevention | 0.25 | AUTOMATED_DIFF | Zero edits to `Cargo.toml`, `Cargo.lock`, or unauthorized modules; existing compiler tests pass |

Total Weight: $\sum W_i = 0.25 + 0.25 + 0.25 + 0.25 = 1.00$.

---

## 5. Evaluator Integration Verification

In `benchmarks/matched-pair-harness/tests/test_milestone_5b_remediation.py`:
1. `test_evaluator_cargo_git_impact_allowed_edits`:
   - Changes restricted to `src/compiler.rs` and `src/analyzer.rs` resulted in:
     - `files_changed_count = 2`
     - `unrelated_files_count = 0`
     - `dependency_mutated = False`
2. `test_evaluator_cargo_git_impact_dependency_mutation`:
   - Adding modifications to `Cargo.lock` or `Cargo.toml` resulted in:
     - `unrelated_files_count = 1`
     - `dependency_mutated = True`
     - Immediate failure of all quality gates and requirement checklist.

This confirms that the task configuration is fully verified and ready for future execution.
