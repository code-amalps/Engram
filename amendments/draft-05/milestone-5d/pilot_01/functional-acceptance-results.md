# Functional Acceptance Results: Task Invalidation Audit

**Experiment ID:** `pilot_01`  
**Task Target:** `task_commit_cache_invalidation`  
**Evaluator Module:** `benchmarks/matched-pair-harness/src/evaluator.py`  
**Evaluation Standard:** 4-Tier Gate Hierarchy (L0..L3) & Task Requirements (REQ-1..4)  
**Status:** MUTUAL ACCEPTANCE FAILURE ($C = 0.00$)  

---

## 1. Quality Gates Evaluation Matrix

| Gate ID | Name | Command | Treatment Arm B | Control Arm A | Acceptance Impact |
| :--- | :--- | :--- | :---: | :---: | :--- |
| **L0** | Typecheck | `cargo check --offline` | `BLOCKED` | `SKIPPED` | Non-Compliant |
| **L1** | Lint | `cargo clippy --offline` (Disabled) | `SKIPPED` | `SKIPPED` | Informational |
| **L2** | Unit Tests | `cargo test --offline --lib compiler::tests && cargo test --offline --test commit_cache_invalidation_test` | `BLOCKED` | `SKIPPED` | Non-Compliant |
| **L3** | Build | `cargo build --offline` | `BLOCKED` | `SKIPPED` | Non-Compliant |

### Gate Execution Details
- **Treatment Arm B:** Quality gates were marked `BLOCKED` because the agent run was terminated by the pre-dispatch token headroom guard before completing its turn loop. The sandbox was cleaned up without executing post-run test suites.
- **Control Arm A:** All quality gates were `SKIPPED` because the arm was never dispatched under the Amendment 04 stop-on-failure rule.

---

## 2. Task Requirements Conformance (REQ-1..4)

| Requirement ID | Description | Weight | Evaluation Mechanism | Treatment Arm B | Control Arm A |
| :--- | :--- | :---: | :--- | :---: | :---: |
| **REQ-1** | Record active Git commit HEAD in SQLite meta table | 0.25 | Physical SQLite inspection / AST check | `FAIL` | `NOT_EVALUATED` |
| **REQ-2** | Commit drift detection invalidates stale cache entries | 0.25 | Composite L2 Cargo Gate + AST check | `FAIL` | `NOT_EVALUATED` |
| **REQ-3** | Add `engram clean --cache` subcommand preserving memories | 0.25 | Executable CLI subprocess / AST check | `FAIL` | `NOT_EVALUATED` |
| **REQ-4** | Boundary adherence and regression prevention | 0.25 | `AUTOMATED_DIFF` (Git impact analysis) | `FAIL` | `NOT_EVALUATED` |

### Requirement Failure Root Causes
- **REQ-1:** No `.engram/index.db` was generated and no commit insertion logic was written to `src/analyzer.rs` or `src/compiler.rs`.
- **REQ-2:** L2 gate did not run (`BLOCKED`), and no drift detection logic was authored in `src/analyzer.rs`.
- **REQ-3:** No `clean --cache` CLI flag or handler was added to `src/main.rs`.
- **REQ-4:** `git_impact.files_changed_count == 0`. An empty diff does not satisfy the task objective.

---

## 3. Git Impact and Sandbox Integrity

| Parameter | Treatment Arm B | Control Arm A | Permitted Boundary | Compliance |
| :--- | :---: | :---: | :---: | :---: |
| **Files Changed** | 0 | 0 | Authorized files only | YES (Zero modification) |
| **Insertions** | 0 | 0 | N/A | YES |
| **Deletions** | 0 | 0 | N/A | YES |
| **Unrelated Files** | 0 | 0 | 0 | YES |
| **Shared Dependency Mutation** | False | False | False | YES |
| **Patch Path** | `treatment_B/changes.patch` (0 bytes) | `control_A/changes.patch` (None) | Recorded | YES |

---

## 4. Overall Correctness Score ($C$)

$$\text{Score } (C) = \sum_{i=1}^{4} w_i \times \mathbb{I}(\text{REQ}_i = \text{PASS}) = 0.00$$

- **Treatment Arm B:** $C = 0.00$, `passed_all_requirements: False`
- **Control Arm A:** $C = 0.00$, `passed_all_requirements: False`
- **Pair Acceptance Parity:** `True` (Both failed)
- **Quality-Adjusted Effective Cost:**  
  $$\text{Effective Cost} = \frac{\text{Total Tokens}}{\max(C, 0.1)} = \frac{117,135}{0.1} = 1,171,350.0$$

Both arms failed to achieve functional acceptance ($C < 1.00$). Under experimental preregistration, this strictly precludes any token efficiency or savings claim.
