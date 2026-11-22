# Functional Acceptance Readiness: N=1 Matched-Pair Pilot

**Milestone:** Engram N=1 Matched-Pair Pilot Final Preflight  
**Document ID:** `amendments/draft-05/n1-pilot-preflight/functional-acceptance-readiness.md`  
**Evaluation Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Task ID:** `task_commit_cache_invalidation`  
**Status:** AUDITED — TECHNICAL DEFECT IDENTIFIED IN EVALUATOR STATIC CHECKS  

---

## 1. Executive Summary

This audit assesses the functional acceptance machinery in `benchmarks/matched-pair-harness/config/tasks/task_commit_cache_invalidation.json` and `benchmarks/matched-pair-harness/src/evaluator.py`.

The inspection confirms that automated compilation gates (L0, L2, L3), boundary enforcement (allowed/forbidden file diffs), and dependency mutation checks are active and operational. However, static inspection reveals a **critical evaluator deficiency**: `evaluator.py` currently lacks domain-specific static check handlers for `task_commit_cache_invalidation`. As a result, REQ-1 and REQ-3 would unconditionally evaluate to `FAIL` under live evaluation, capping the maximum correctness score at $C = 0.50$ and artificially suppressing comparative savings claims.

---

## 2. Configured Acceptance Gates & Evaluation Architecture

### 2.1 Automated Compilation & Test Gates
In `task_commit_cache_invalidation.json` (`lines 57–76`), the verification gates are configured as follows:

| Gate | Category | Configured Command | Status |
|---|---|---|---|
| **L0** | Typecheck | `cargo check --offline` | **Enabled** |
| **L1** | Lint | `cargo clippy --offline` | **Disabled** |
| **L2** | Unit Tests | `cargo test --offline --lib compiler::tests` | **Enabled** |
| **L3** | Production Build | `cargo build --offline` | **Enabled** |

In `evaluator.py` (`run_automated_gates`, `lines 120–185`), each enabled gate is invoked inside the provisioned sandbox. If any gate fails with a non-zero exit code or times out, the gate result is recorded as `FAIL`.

### 2.2 Task Checklist Requirements
The task specifies four weighted requirements totaling 1.0 (100%):

| Requirement ID | Weight | Evaluation Type | Specification Description & Criteria |
|---|---|---|---|
| **REQ-1** | 0.25 | `STATIC_CHECK` | Record active Git commit HEAD in SQLite meta table (`index.db` contains meta table recording `git_commit_head`). |
| **REQ-2** | 0.25 | `UNIT_TEST` | Commit drift detection invalidates stale cache entries (`ensure_fresh` triggers re-index when HEAD diverges). |
| **REQ-3** | 0.25 | `STATIC_CHECK` | Add `engram clean --cache` subcommand preserving memories (clean subcommand purges AST SQLite cache without deleting `memories.json`). |
| **REQ-4** | 0.25 | `AUTOMATED_DIFF` | Boundary adherence & regression prevention (zero edits to Cargo.toml, Cargo.lock, or unauthorized modules; compiler tests pass). |

---

## 3. Boundary Adherence and Mutation Detection

### 3.1 Authorized Blast-Radius Boundaries
In `task_commit_cache_invalidation.json`:
- **Allowed File Patterns:**
  - `src/compiler.rs`
  - `src/analyzer.rs`
  - `src/main.rs`
  - `tests/commit_cache_invalidation_test.rs`
- **Forbidden File Patterns:**
  - `Cargo.toml`
  - `Cargo.lock`
  - `src/mcp.rs`
  - `src/guard.rs`
  - `src/ci_guard.rs`
  - `src/hook.rs`
  - `.engram/memories.json`

### 3.2 Automated Git Impact Analysis
In `evaluator.py` (`analyze_git_impact`, `lines 45–115`):
- Git impact is assessed against the clean detached worktree commit `c70845b4d5913457f1b176fb0370b80dd792711f`.
- The evaluator runs `git status --porcelain` and `git diff HEAD --numstat`.
- If changes touch any file not matching `allowed_file_patterns`, `git_impact.unrelated_files_count` increments, causing REQ-4 to evaluate to `FAIL`.
- If no files are changed (`files_changed_count == 0`), REQ-4 evaluates to `FAIL`.

### 3.3 Dependency Integrity Enforcement
In `isolation.py` (`compute_cargo_manifest` and `verify_dependency_integrity`) and `evaluator.py`:
- SHA-256 manifests of `Cargo.toml` and `Cargo.lock` are computed prior to sandbox provisioning and compared post-execution.
- Any modification to `Cargo.toml` or `Cargo.lock` sets `dependency_mutated = True`.
- Under `calculate_score` (`lines 460–480`), `dependency_mutated = True` immediately forces:
  - Correctness Score $C = 0.0$
  - `passed_all_requirements = False`
  - All requirements overridden to `FAIL`.

---

## 4. Evaluator Static Check Deficiency Analysis

### 4.1 Requirement Evaluation Flow in Evaluator
In `evaluator.py` (`evaluate_requirements`, `lines 200–245`):
```python
elif eval_type == "STATIC_CHECK":
    passed = self._check_static_requirement(sandbox_path, req_id)
    req_results[req_id] = "PASS" if passed else "FAIL"
```

### 4.2 Inspection of `_check_static_requirement`
Inspection of `evaluator.py` (`lines 255–440`) reveals that `_check_static_requirement` implements branches strictly for:
1. `if self.task_id == "task_a_typo":`
2. `if self.task_id == "task_b_remember_me":`
3. `if self.task_id == "task_c_theme_toggle":`

There is **no branch** for `self.task_id == "task_commit_cache_invalidation"`.

### 4.3 Direct Impact on Pilot Execution
If live execution were launched under the current codebase:
1. **REQ-1** (`evaluation_type: STATIC_CHECK`) calls `_check_static_requirement(sandbox_path, "REQ-1")`, falls through all branches, and returns `False` $\rightarrow$ Result: `FAIL`.
2. **REQ-2** (`evaluation_type: UNIT_TEST`) checks `gate_results["L2_unit_tests"]`. If unit tests pass, returns $\rightarrow$ Result: `PASS` ($+0.25$).
3. **REQ-3** (`evaluation_type: STATIC_CHECK`) calls `_check_static_requirement(sandbox_path, "REQ-3")`, falls through, and returns `False` $\rightarrow$ Result: `FAIL`.
4. **REQ-4** (`evaluation_type: AUTOMATED_DIFF`) checks file bounds. If bounds respected, returns $\rightarrow$ Result: `PASS` ($+0.25$).
5. **Net Correctness Score:** $C = 0.25 + 0.25 = 0.50$.
6. **Acceptance Gate Failure:** Because $C < 1.0$ and REQ-1/REQ-3 failed, `passed_all_requirements` evaluates to `False`.

---

## 5. Mutual Acceptance Enforcement

In `orchestrator.py` (`lines 430–445`, `compute_comparative_analysis`):
```python
ctrl_acc = bool(ctrl_corr.get("passed_all_requirements", False) and (ctrl_corr.get("score", 0.0) == 1.0))
treat_acc = bool(treat_corr.get("passed_all_requirements", False) and (treat_corr.get("score", 0.0) == 1.0))
```
- If either `control_A` or `treatment_B` achieves $C < 1.0$, `pair_acceptance_parity` fails and primary savings claims are strictly suppressed with reason:
  `MUTUAL_ACCEPTANCE_FAILURE: Both arms failed functional correctness requirements`.
- Under the current `evaluator.py` implementation, even if Codex implements perfect code that compiles and passes all unit tests, the harness will falsely record $C = 0.50$ and suppress token savings.

---

## 6. Verification Verdict & Preflight Assessment

1. **Gating Parity:** Verified. L0, L2, L3 commands are verified against `Cargo.toml`.
2. **Boundary Enforcement:** Verified. Allowed/forbidden file lists are active and enforce strict blast-radius containment.
3. **Savings Suppression Invariant:** Verified. Unaccepted arms cannot generate valid token savings claims.
4. **Evaluator Readiness:** **BLOCKED / CONDITIONAL**. `_check_static_requirement` in `evaluator.py` must be populated with domain checks for `task_commit_cache_invalidation` (validating SQLite `git_commit_head` metadata and `engram clean --cache` CLI registration) before a live trial can achieve a valid $C = 1.0$ outcome.
