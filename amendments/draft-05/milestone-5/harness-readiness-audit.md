# Milestone 5A: Telemetry and Harness Readiness Audit
## Inspection of Active Matched-Pair Harness Architecture and Execution Blockers

**Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/async-precommit-guard`  
**HEAD Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`  
**Document ID:** `ENGRAM-A05-M5A-HARNESS-AUDIT`  
**Classification:** Harness Architecture and Readiness Audit  

---

### 1. Executive Summary

This audit evaluates the active Python matched-pair harness located under `benchmarks/matched-pair-harness/src/` (`orchestrator.py`, `runner.py`, `telemetry.py`, `evaluator.py`, `isolation.py`, `classifier.py`).

The objective is to establish whether the harness in its current state can execute the preregistered Milestone 5 experiment, record all required telemetry without double-counting, enforce conservative token ceilings, and preserve evidence.

#### Audit Verdict:
The harness contains robust telemetry extraction, token tracking, and evidence preservation mechanisms, but **two concrete implementation blockers** prevent immediate execution of the primary task (Task 2) without authorized harness remediation:
1. **Target Repository Coupling in `isolation.py`:** The dependency integrity verifier is hardcoded to validate 68,274 files in `node_modules` for the Angular starter repository (`angular-ngrx-material-starter`). Executing a task on a Rust repository (such as Engram itself) triggers an immediate `GoldenDependencyCorruptionError`.
2. **Missing Task Configuration File:** The selected primary task (Task 2: Commit-Bound Cache Invalidation) does not yet have a corresponding task configuration file in `benchmarks/matched-pair-harness/config/tasks/`.

---

### 2. Systematic 11-Point Harness Inspection

| Dimension | Inspection Target | Verification Status | Forensic Findings & Implementation Details |
| :--- | :--- | :---: | :--- |
| **1. Arm Selection** | `orchestrator.py:115-120` | **VERIFIED** | Supports 2-arm (`control_A`, `treatment_B`), 3-arm triads (`baseline_arm1`, `v151_arm2`, `v154_arm3`), and single-arm mode (`--mode single`). |
| **2. Candidate Binary** | `orchestrator.py:97-100` | **VERIFIED** | Correctly maps `v154_arm3` to Candidate Phase 1 binary (`F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`). |
| **3. Task Selection** | `orchestrator.py:1528-1534` | **GAP IDENTIFIED** | Task parser looks for `config/tasks/<task>.json`. Only `task_a_typo`, `task_b_remember_me`, `task_c_theme_toggle`, and `smoke_test_read_only` exist. Task 2 config file is missing. |
| **4. Workspace Isolation** | `isolation.py:1-402` | **BLOCKER** | Hardcoded to Angular `node_modules`. Cannot create isolated worktree for Rust repository without triggering dependency integrity failure. |
| **5. Model Identity** | `runner.py:180-210` | **VERIFIED** | Forwards `-m <model>` and `-c collaboration_mode.settings.reasoning_effort=<effort>` directly to Codex CLI. |
| **6. Usage Extraction** | `telemetry.py:40-120` | **VERIFIED** | Correctly parses provider usage objects, distinguishing `cached_tokens` and `reasoning_tokens` without double-counting. |
| **7. Ceiling Propagation**| `orchestrator.py:85-86` | **VERIFIED** | CLI exposes `--max-tokens-ceiling` and `--headroom-reserve`, propagating to `StreamingTokenTracker`. |
| **8. Stop Reason** | `telemetry.py:150-185` | **VERIFIED** | Records `SUCCESS`, `TOKEN_CEILING_BREACH`, `TIMEOUT`, `CLIENT_APPROVAL_REJECTION`, `PROCESS_CRASH`. |
| **9. Failure Classifier**| `classifier.py:1-150` | **VERIFIED** | Deterministically categorizes errors based on telemetry events and exit codes. |
| **10. Acceptance Gate** | `evaluator.py:1-350` | **VERIFIED** | Executes automated test suites and scores $C \in [0.0, 1.0]$. |
| **11. Evidence Vault** | `orchestrator.py:125-165`| **VERIFIED** | Creates timestamped, non-overwriting run directories containing `run_telemetry.json`, `events.jsonl`, `changes.patch`. |

---

### 3. Detailed Forensic Analysis of Implementation Blockers

#### 3.1 Blocker 1: Golden Repository Coupling in `isolation.py`
In `benchmarks/matched-pair-harness/src/isolation.py`:
- Line 8: Enforces "Comprehensive Golden Repository Dependency Verification across all 68,274 files in `node_modules`".
- Line 880: `verify_dependency_integrity(self.golden_repo, self.golden_baseline_fingerprint)` explicitly asserts the structure of an npm/Node.js project.
- If `--task` points to an Engram-internal task requiring the Engram repository as the source workspace, `isolation.py` fails immediately because `node_modules` does not exist in a pure Rust codebase.

##### Remediation Path (For Milestone 5B Authorization):
1. Extend `isolation.py` to support an ecosystem-aware dependency strategy:
   - For `npm`/`angular`: Validate `node_modules`.
   - For `cargo`/`rust`: Validate `Cargo.lock` and offline vendor cache or read-only `target/` isolation.
   - For `git_only`: Perform clean `git worktree add` without dependency copying if the task is self-contained.
2. In accordance with Milestone 5A strict restrictions ("No changes to harness source code in this milestone"), this remediation must be documented here and executed only under separate owner authorization.

#### 3.2 Blocker 2: Missing Task Configuration for Primary Task
`config/tasks/` currently lacks `task_commit_cache_invalidation.json`. 
Passing `--task task_commit_cache_invalidation` to `orchestrator.py` causes an immediate fatal exit:
```
Error: Task configuration not found: task_commit_cache_invalidation
```

##### Remediation Path (For Milestone 5B Authorization):
Author `benchmarks/matched-pair-harness/config/tasks/task_commit_cache_invalidation.json` defining:
- Task ID: `task_commit_cache_invalidation`
- Baseline commit: `c70845b4d5913457f1b176fb0370b80dd792711f`
- Authorized files: `src/compiler.rs`, `src/analyzer.rs`, `src/main.rs`, `tests/commit_cache_invalidation_test.rs`
- Verification suite: `cargo test --offline --test commit_cache_invalidation_test`

---

### 4. Harness Readiness Audit Conclusion

- The core execution engine, streaming token monitor, and telemetry recording infrastructure are **technically sound, fully audited, and capable of high-precision measurement**.
- However, because the harness currently only provisions workspaces for the Angular starter repository, executing the primary Rust task requires a targeted harness adaptation.
- **Milestone 5A Classification Impact:** This constitutes a known, documented technical precondition that must be formally resolved prior to launching Milestone 5B live runs.
