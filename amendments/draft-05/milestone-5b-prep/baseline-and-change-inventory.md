# Milestone 5B-Prep: Baseline and Change Inventory

## 1. Executive Summary

Milestone 5B-Prep addresses the two technical execution blockers identified in Milestone 5A and establishes a hardened, verified, offline experimental environment for the future controlled token-efficiency experiment.

### Governance State
- **Phase:** Engram Amendment 05 — Milestone 5B-Prep (Harness Remediation & Verification)
- **Status:** Complete (Offline Only)
- **Live Inference Authorization:** STRICTLY ZERO ($0.00 spend; zero API tokens dispatched)
- **Git HEAD Discrepancy Reconciliation:** Verified pre-existing commit `c70845b4d5913457f1b176fb0370b80dd792711f` on branch `feature/async-precommit-guard`.

---

## 2. Baseline Inventory (Prior to Milestone 5B-Prep)

| Component | State / Identifier | Integrity / Checksum |
|---|---|---|
| **Git Working Branch** | `feature/async-precommit-guard` | HEAD `c70845b4d5913457f1b176fb0370b80dd792711f` |
| **Candidate Phase 1 Binary** | `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` | SHA-256: `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` |
| **Frozen v1.5.1 Executable** | `benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe` | SHA-256: `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` |
| **Frozen v1.5.4 Executable** | `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe` | SHA-256: `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` |
| **Golden Angular Repository** | `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` | HEAD `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` (Preserved read-only) |
| **Harness Baseline Test Suite**| `benchmarks/matched-pair-harness/tests/` | 233 passing tests (109.12s elapsed) |
| **Rust Baseline Library Tests**| `cargo test --offline --lib` | 17 passing tests (0.67s elapsed) |
| **Rust Empirical Live Tests**  | `cargo test --offline --test empirical_live_test -- --test-threads=1` | 99 passing tests (268.60s elapsed) |
| **Rust Scoped Routing Tests** | `cargo test --offline --test scoped_query_routing_test` | 9 passing tests (7.98s elapsed) |

---

## 3. Targeted Remediation Inventory

Milestone 5A identified three specific gaps requiring technical intervention:

1. **Blocker 1 (Coupled Workspace Isolation):**
   `isolation.py` was hardcoded to Angular/Node.js expectations (`node_modules`, `package.json`, `package-lock.json`). Attempting to run tasks on the Engram codebase (a Rust/Cargo repository) failed because `node_modules` was absent.
2. **Blocker 2 (Missing Task Configuration):**
   `task_commit_cache_invalidation.json` was preregistered in Milestone 5A as the representative Task C scenario but did not exist on disk.
3. **Safety Concern (Headroom Reserve Sizing):**
   The historical 35,000-token headroom reserve was smaller than the largest observed turn delta in Stage 1 Block 06 (Task B, Turn 9: 37,651 tokens). Under worst-case single-turn jumps, this presented an overrun hazard against the 175,000 per-arm ceiling.

---

## 4. File-by-File Change Log

### 4.1 Modified Harness Source Files

1. **`benchmarks/matched-pair-harness/src/isolation.py`**
   - Implemented `compute_cargo_manifest(repo_path: Path) -> Dict[str, Any]` to compute SHA-256 cryptographic hashes for `Cargo.toml`, `Cargo.lock`, and recursive structural digests for `src/` and `tests/`.
   - Generalized `verify_dependency_integrity` to branch conditionally on `baseline.get("repo_type") == "cargo"`, comparing Cargo manifests without requiring `node_modules`.
   - Generalized `verify_sandbox_cleanliness` to accept `repo_type="cargo"`, allowing repository-standard artifacts (`.github`, `.engram`, `target/`).
   - Generalized `create_sandbox` and `teardown_sandbox` with explicit `repo_type` routing for NTFS junctions and directory cleanup.

2. **`benchmarks/matched-pair-harness/src/orchestrator.py`**
   - Imported `compute_cargo_manifest` and `compute_comprehensive_golden_manifest`.
   - Updated `Orchestrator.__init__` to infer `self.repo_type` automatically from `task_config["repository_type"]` or directory structure, defaulting to the Engram root if a Cargo task is configured.
   - Updated default `headroom_reserve` from `15_000` to `45_000` tokens across both class initializers and CLI entrypoint.
   - Added CLI arguments `--golden-repo` / `--source-repo` and passed `repo_type=self.repo_type` to `teardown_sandbox`.

3. **`benchmarks/matched-pair-harness/src/runner.py`**
   - Updated `RunExecutionResult.headroom_reserve` default to `45_000`.
   - Updated `StreamingTokenTracker.__init__` default `conservative_reserve` from `15_000` to `45_000`.
   - Updated `CodexRunner.__init__` default `headroom_reserve` from `15_000` to `45_000`.

4. **`benchmarks/matched-pair-harness/src/evaluator.py`**
   - Updated `Evaluator.analyze_git_impact` to evaluate dependency mutations according to repository type (`Cargo.toml` and `Cargo.lock` for Cargo; `package.json`, `package-lock.json`, and `node_modules/` for Angular).

### 4.2 Added Task Configuration Files

5. **`benchmarks/matched-pair-harness/config/tasks/task_commit_cache_invalidation.json`**
   - Formally authored and registered schema 2.0.0 task configuration.
   - Configured `repository_type: "cargo"`, `read_only: false`, `requires_dependencies: false`, and baseline commit `c70845b4d5913457f1b176fb0370b80dd792711f`.
   - Formulated 4 weighted checklist requirements (`REQ-1` to `REQ-4`, weight 0.25 each) evaluating SQLite commit tracking, AST cache invalidation, cache purge subcommand, and blast-radius boundary adherence.

### 4.3 Added Offline Remediation Test Files

6. **`benchmarks/matched-pair-harness/tests/test_milestone_5b_remediation.py`**
   - Implemented 20 offline unit tests covering Cargo manifest extraction, dependency drift detection, sandbox worktree isolation, task schema validation, Evaluator git impact, 45,000-token headroom guard behavior, worst-case turn delta exposure, and Orchestrator repository type inference.

---

## 5. Post-Remediation Verification Matrix

| Verification Check | Target / Invariant | Observed Status | Verdict |
|---|---|---|---|
| **Python Test Discovery** | Full harness suite | 253 / 253 passed (220.03s) | **PASS** |
| **Milestone 5B Test Suite** | 20 dedicated remediation tests | 20 / 20 passed (7.61s) | **PASS** |
| **Rust Library Tests** | `cargo test --offline --lib` | 17 / 17 passed (0.67s) | **PASS** |
| **Rust Scoped Routing Tests** | `tests/scoped_query_routing_test.rs` | 9 / 9 passed (7.98s) | **PASS** |
| **Rust Empirical Safety Tests**| `tests/empirical_live_test.rs` (`--test-threads=1`) | 99 / 99 passed (268.60s) | **PASS** |
| **Binary Hash Preservation** | Candidate Phase 1 SHA-256 | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | **PASS** |
| **Binary Hash Preservation** | Frozen v1.5.1 SHA-256 | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | **PASS** |
| **Binary Hash Preservation** | Frozen v1.5.4 SHA-256 | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | **PASS** |
| **Golden Angular Repo** | Zero modification during testing | HEAD `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` | **PASS** |
| **Zero Live Inference** | Token spend = $0.00 | Zero API calls dispatched | **PASS** |
