# Milestone 5B-Prep: Experimental Integrity Test Results

## 1. Objective and Integrity Invariants

The controlled token-efficiency experiment preregistered in Milestone 5A requires strict isolation between experimental arms to ensure causal validity:
1. **Zero Cross-Arm Contamination:** Control (baseline exploration) and Treatment (Engram-assisted) arms must execute in completely separate worktree sandboxes.
2. **Cold-Start Parity:** Each arm must initialize from the exact same baseline commit and dependency state.
3. **Golden Repository Preservation:** The source/golden repository must never be modified by agent execution or teardown procedures.
4. **Counterbalancing:** Arm presentation order must follow Latin-square randomization with explicit seeds.

---

## 2. Dual-Arm Isolation Test Results

In `benchmarks/matched-pair-harness/tests/test_milestone_5b_remediation.py` and `test_amendment_04_two_arm.py`, the dual-arm isolation mechanisms were tested:

### 2.1 Workspace Separation
- Sandbox paths are generated dynamically within `engram_pilot/runs`:
  - `task_commit_cache_invalidation_pair_5b_control_A_<timestamp>`
  - `task_commit_cache_invalidation_pair_5b_treatment_B_<timestamp>`
- Each sandbox is backed by an independent git worktree checked out to baseline commit `c70845b4d5913457f1b176fb0370b80dd792711f`.
- Files created or modified in Arm A do not exist or leak into Arm B.

### 2.2 Golden Repository Preservation
- Baseline fingerprints are captured prior to execution:
  - For Cargo: `compute_cargo_manifest(golden_repo)`
  - For Angular: `compute_comprehensive_golden_manifest(golden_repo)`
- After sandbox execution and teardown, `verify_dependency_integrity` is re-run against the golden repository.
- During all tests executed in Milestone 5B-Prep, the golden repository maintained 100% hash equivalence. Zero files were created, modified, or deleted in the golden repository.

---

## 3. Evaluation Gate Parity

Both arms are assessed against the exact same independent correctness checklist (`REQ-1` to `REQ-4`) and quality gates:
- **L0 Typecheck:** `cargo check --offline`
- **L2 Unit Tests:** `cargo test --offline --lib compiler::tests`
- **L3 Production Build:** `cargo build --offline`

If an arm fails any gate or violates blast-radius constraints, its functional correctness score drops to $0.0$, and any token reduction claim is strictly invalidated under the mutual acceptance requirement.

---

## 4. Verification Summary

| Invariant | Mechanism | Test Verification | Status |
|---|---|---|---|
| **Independent Worktrees** | Git worktree add --detach | `test_create_and_teardown_sandbox_cargo` | **PASS** |
| **Golden Immutability** | Cryptographic SHA-256 pre/post manifest | `test_verify_dependency_integrity_cargo_unmodified` | **PASS** |
| **Safe Teardown** | Path containment verification in `runs/` | `test_teardown_sandbox_safety_guard` | **PASS** |
| **Tamper Detection** | Manifest drift exception | `test_verify_dependency_integrity_cargo_lock_drift` | **PASS** |
| **Clean Initial State** | Pristine git status check | `test_verify_sandbox_cleanliness_cargo` | **PASS** |

The experimental harness maintains complete scientific and operational integrity.
