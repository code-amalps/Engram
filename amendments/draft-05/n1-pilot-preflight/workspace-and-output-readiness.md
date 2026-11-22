# Workspace and Output Readiness: N=1 Matched-Pair Pilot

**Milestone:** Engram N=1 Matched-Pair Pilot Final Preflight  
**Document ID:** `amendments/draft-05/n1-pilot-preflight/workspace-and-output-readiness.md`  
**Evaluation Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Intended Output Directory:** `benchmarks/matched-pair-harness/reports/runs/pilot_01`  
**Status:** VERIFIED & PRISTINE  

---

## 1. Executive Summary

This audit assesses the physical filesystem readiness, workspace isolation guarantees, and output directory availability for the N=1 matched-pair pilot.

The evaluation confirms:
1. The designated output directory `benchmarks/matched-pair-harness/reports/runs/pilot_01` does not exist and is completely collision-free.
2. The Git worktree provisioning mechanism guarantees 100% physical filesystem separation between `control_A` and `treatment_B`.
3. Golden repository state is fully preserved with zero untracked files or staged changes introduced into production code.
4. No agent workspaces or execution processes were initiated during preflight.

---

## 2. Output Directory Availability & Collision Audit

### 2.1 Intended Output Vault
The designated directory for the pilot run evidence is:
`C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness\reports\runs\pilot_01`

### 2.2 Collision Check
The filesystem was queried for the existence of `pilot_01`:
```powershell
Test-Path "C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness\reports\runs\pilot_01"
# Output: False
```

### 2.3 Historical Run Preservation
The existing run reports under `benchmarks/matched-pair-harness/reports/runs/` were inspected:
- Existing runs correspond to historical smoke tests and calibration trials.
- None conflict with `pilot_01`.
- Safe Output Directory Handling in `orchestrator.py` (`lines 180–210`) enforces that existing directories cannot be silently overwritten without explicit owner intent.

---

## 3. Workspace Isolation and Provisioning Lifecycle

### 3.1 Git Worktree Provisioning Mechanics
In `benchmarks/matched-pair-harness/src/isolation.py` (`create_sandbox`):
1. **Isolated Root:** Sandboxes are created under the local system temporary directory:
   $$\text{Sandbox Path} = \texttt{\%TEMP\%\textbackslash engram\_pilot\textbackslash runs\textbackslash task\_commit\_cache\_invalidation\_pilot\_01\_<arm>\_<timestamp>}$$
2. **Path Traversal Protection:** The harness verifies that the sandbox path is strictly confined to `engram_pilot\runs`.
3. **Detached Worktree Add:** The harness provisions the workspace via:
   ```bash
   git -c core.longpaths=true worktree add --detach <sandbox_path> c70845b4d5913457f1b176fb0370b80dd792711f
   ```
4. **Independent Working Copy:** Both `control_A` and `treatment_B` operate in independent directory trees with their own index, working tree, and untracked files.

### 3.2 Dependency and Compilation Isolation
For `task_commit_cache_invalidation`:
- `repository_type = "cargo"`
- `requires_dependencies = false`
- `dependency_strategy = "none"`
- Unlike Angular/Node.js tasks which require `node_modules` provisioning, Cargo tasks operate offline using pre-cached local crates in Cargo's global cache (`~/.cargo/registry`).
- No NTFS junctions or shared symlinks are created between the sandboxes and the golden repository.
- Each sandbox creates its own isolated `target/` directory during `cargo check` and `cargo test`, preventing cross-arm build artifact pollution.

### 3.3 Sandbox Teardown and Cleanup
In `isolation.py` (`teardown_sandbox`):
- If `--preserve-sandboxes` is not passed, `teardown_sandbox` safely unlinks and purges the sandbox directory upon arm completion.
- Running `git worktree prune` cleans worktree administrative metadata in the golden `.git/worktrees` directory.
- Post-run dependency manifests of the golden repository are verified before the next arm is provisioned.

---

## 4. Cold-Start and State Cleansing Verification

| Isolation Dimension | Guarantee | Harness Mechanism |
|---|---|---|
| **Filesystem Separation** | 100% Physical Isolation | Detached Git worktree under `%TEMP%` |
| **Commit Parity** | Pinned to `c70845b4...` | `verify_sandbox_identity()` called immediately upon creation |
| **Clean Initial State** | Zero Untracked Cruft | `verify_sandbox_cleanliness()` asserts `git status --porcelain` is empty |
| **Process Separation** | Isolated Child Processes | Codex spawned as detached child with sanitized environment variables |
| **Conversation Memory** | Zero Cross-Arm History | Independent ephemeral Codex sessions; zero conversation persistence |
| **MCP State** | Ephemeral Process | Candidate Engram binary launched per-arm; separate SQLite index per sandbox |

---

## 5. Verification Verdict

The workspace isolation architecture and output directory configuration for `pilot_01` are verified as completely ready, fully isolated, and collision-free.
