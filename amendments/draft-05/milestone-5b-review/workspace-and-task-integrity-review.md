# Milestone 5B-Review: Workspace & Task Integrity Review

## 1. Executive Summary

This review independently inspects the task configuration (`task_commit_cache_invalidation.json`) and the dual-arm workspace isolation pipeline to verify that experimental arms operate under strict cold-start parity, zero cross-contamination, and genuine scientific isolation.

---

## 2. Task Configuration Verification (`task_commit_cache_invalidation.json`)

The task JSON file located at `benchmarks/matched-pair-harness/config/tasks/task_commit_cache_invalidation.json` was inspected:

| Configuration Property | Value in Configuration | Audit Verification | Status |
|---|---|---|---|
| **Task Identifier** | `"task_commit_cache_invalidation"` | Matches preregistered Task C identifier | **PASS** |
| **Repository Type** | `"cargo"` | Correctly recognized by `isolation.py` & `orchestrator.py` | **PASS** |
| **Baseline Commit** | `"c70845b4d5913457f1b176fb0370b80dd792711f"` | Exactly matches git HEAD of `feature/async-precommit-guard` | **PASS** |
| **Read-Only Mode** | `false` | Read-write engineering task requiring implementation | **PASS** |
| **External Dependencies** | `requires_dependencies: false` | Self-contained Cargo workspace; no npm/node_modules | **PASS** |
| **Authorized Scope** | `src/compiler.rs`, `src/analyzer.rs`, `src/main.rs`, `tests/commit_cache_invalidation_test.rs` | Fully specified; matches engineering requirements | **PASS** |
| **Forbidden Scope** | `Cargo.toml`, `Cargo.lock`, `src/mcp.rs`, `src/guard.rs`, `src/ci_guard.rs`, `src/hook.rs`, `.engram/memories.json` | Explicitly whitelists non-target files to prevent blast-radius sprawl | **PASS** |
| **Automated Gates** | L0 (`cargo check --offline`), L2 (`cargo test --offline --lib compiler::tests`), L3 (`cargo build --offline`) | Fully offline verification commands | **PASS** |
| **Model Configuration** | None embedded | Model, temperature, and reasoning parameters are strictly separated into harness CLI arguments | **PASS** |

---

## 3. Dual-Arm Workspace Isolation Architecture

The isolation mechanics across Baseline (Arm A) and Treatment (Arm B) were reviewed against the code in `isolation.py` and `orchestrator.py`:

```
Golden Repository (Read-Only)
[C:\Users\...\Engram @ HEAD c70845b4d...]
       │
       ├─── (git worktree add --detach) ───► Arm A Sandbox: runs/task_5b_control_A_<ts>
       │                                     - Native agent exploration
       │                                     - Zero Engram tools / context
       │                                     - Independent working tree & index
       │
       └─── (git worktree add --detach) ───► Arm B Sandbox: runs/task_5b_treatment_B_<ts>
                                             - Pre-generated Engram context package
                                             - Candidate Phase 1 Engram binary
                                             - Independent working tree & index
```

### Invariant 1: Source Snapshot Identity
Both sandboxes are provisioned using `git worktree add --detach <path> c70845b4d5913457f1b176fb0370b80dd792711f`. `verify_sandbox_identity` runs `git rev-parse HEAD` immediately post-creation, verifying identical starting commits.

### Invariant 2: Independent Mutable State
Each arm resides in a separate physical directory on disk inside `AppData\Local\Temp\engram_pilot\runs\`. Edits made in Arm A cannot modify files in Arm B or the golden repository.

### Invariant 3: Zero Cross-Arm Source Access
Neither sandbox has symbolic links or junctions pointing to the other sandbox. The `--add-dir` parameter in Codex is omitted, preventing the agent from mounting paths outside its assigned sandbox root.

### Invariant 4: Zero Conversation History Leakage
Codex is invoked with `--ephemeral` and a dedicated temporary conversation directory. Session logs and rollout files are unique per arm. Arm B has zero access to Arm A's reasoning traces, tool outputs, or failed attempts.

### Invariant 5: Context Package Asymmetry Controls
- In **Baseline (Control A)**: No Engram context package exists in the sandbox. The prompt contains only standard task instructions and requirements.
- In **Treatment (Arm B)**: Engram context is pre-compiled offline prior to execution and provided via the designated prompt injection or local MCP tool. The treatment agent does not receive unrecorded prompt advantages.

### Invariant 6: Hidden Benchmark Output Scrubbing
The sandboxes are cold-start worktrees from git HEAD. Neither sandbox contains past reports from `amendments/draft-05/` or `benchmarks/reports/runs/`.

### Invariant 7: Comprehensive Post-Run Mutation Checks
After execution, `evaluator.analyze_git_impact` inspects `git status --porcelain` and `git diff HEAD`. Any mutation to `Cargo.toml`, `Cargo.lock`, or files outside `allowed_file_patterns` immediately flags `dependency_mutated=True` or increments `unrelated_files_count`, triggering automatic requirement failure.

---

## 4. Verification Verdict

The task configuration and workspace isolation mechanisms are completely verified. Both arms operate under rigorous scientific controls with zero risk of cross-contamination or hidden leakage.
