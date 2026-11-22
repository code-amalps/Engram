# Engram Amendment 05 — Milestone 2C: Security and Preservation Report

**Document Version:** 1.0.0  
**Date:** 2026-10-30  
**Status:** Complete Offline Verification  
**Classification:** `REMEDIATION COMPLETE — READY FOR FINAL OFFLINE AUDIT`

---

## 1. Executive Summary

This Security and Preservation Report provides formal verification that all offline harness remediations executed during Engram Amendment 05 Milestone 2C strictly complied with every preservation boundary, safety constraint, and cryptographic integrity invariant.

Crucially:
1. **Zero Model Inference / API Activity:** Exactly 0 tokens, 0 API calls, 0 network requests, and $0.00 spent. Neither Codex CLI nor any model endpoint was executed.
2. **Protected Binaries Intact:** The Candidate Phase 1 binary, frozen v1.5.4 binary, and frozen v1.5.1 binary match their authoritative SHA-256 hashes bit-for-bit.
3. **Historical Evidence Preserved:** Amendment 04, Block 07 reports/telemetry, and Milestone 2B verification documents remain 100% byte-identical.
4. **Golden Repository Untouched:** The golden repository at `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` remains anchored to commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` with identical tracked diffstat (`+208 / -16`) and untracked `.engram/` metadata.
5. **Read-Only Agent Sandbox Verified:** The local Codex CLI binary was verified via static help inspection (`-s read-only`), and the harness was instrumented to omit `--add-dir` and enforce a post-run zero-mutation failure check.
6. **Output Overwrite Protection Implemented:** The harness actively prevents overwriting historical run evidence by rejecting protected directory roots and automatically provisioning unique timestamped subdirectories on path collisions.

---

## 2. Before and After Git Status Comparison

### 2.1 Engram Repository Overview
- **Repository Root:** `C:\Users\Amal P S\OneDrive\Projects\Engram`
- **Active Branch:** `feature/async-precommit-guard`
- **Current HEAD Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f` (unmodified; zero Git write or commit operations performed)

### 2.2 Working Tree Comparison

| File | Pre-Change Status (`snapshot-before.md`) | Post-Remediation Status | Diff Description |
|:---|:---:|:---:|:---|
| `benchmarks/matched-pair-harness/config/tasks/smoke_test_read_only.json` | *Did not exist* | `??` (Untracked New File) | Authored dedicated read-only smoke-test task configuration |
| `benchmarks/matched-pair-harness/schemas/correctness_checklist.schema.json` | `M` (Modified) | `M` (Modified) | Added `read_only` and `requires_dependencies` boolean properties |
| `benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json` | `M` (Modified) | `M` (Modified) | Added `sandbox_policy` string property |
| `benchmarks/matched-pair-harness/src/evaluator.py` | `M` (Modified) | `M` (Modified) | Pre-existing baseline modifications preserved |
| `benchmarks/matched-pair-harness/src/isolation.py` | `M` (Modified) | `M` (Modified) | Pre-existing baseline modifications preserved |
| `benchmarks/matched-pair-harness/src/orchestrator.py` | `M` (Modified) | `M` (Modified) | Implemented `--mode single`, `run_single`, `--output-dir`, safe dependency guard, read-only mutation check |
| `benchmarks/matched-pair-harness/src/runner.py` | `M` (Modified) | `M` (Modified) | Added `sandbox_policy` support, `-s read-only`, omission of `--add-dir` |
| `benchmarks/matched-pair-harness/src/telemetry.py` | `M` (Modified) | `M` (Modified) | Pre-existing baseline modifications preserved |
| `benchmarks/matched-pair-harness/tests/test_milestone_2c_remediation.py` | *Did not exist* | `??` (Untracked New File) | 21 offline unit tests validating all 5 remediations |
| `src/instructions.rs` | `M` (Modified) | `M` (Modified) | Pre-existing Milestone 1 modifications preserved |
| `src/main.rs` | `M` (Modified) | `M` (Modified) | Pre-existing Milestone 1 modifications preserved |
| `src/mcp.rs` | `M` (Modified) | `M` (Modified) | Pre-existing Milestone 1 modifications preserved |
| `tests/mcp_consolidation_test.rs` | `M` (Modified) | `M` (Modified) | Pre-existing Milestone 1 modifications preserved |
| `tests/turn0_injection_test.rs` | `M` (Modified) | `M` (Modified) | Pre-existing Milestone 1 modifications preserved |
| `amendments/draft-05/milestone-2c/` | *Did not exist* | `??` (Untracked Directory) | Milestone 2C documentation deliverables |

**Conclusion:** All existing user changes from Milestones 1, 1A, and 2B have been strictly preserved. Zero commits, resets, or stashes occurred.

---

## 3. Cryptographic Binary Hash Preservation

All candidate and frozen binary executables were hashed using SHA-256 before and after remediation activities:

| Binary Role | Path | Expected Authoritative SHA-256 | Verified SHA-256 | Status |
|:---|:---|:---|:---|:---:|
| **Candidate Phase 1** | `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | **MATCH (100%)** |
| **Frozen v1.5.4** | `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | **MATCH (100%)** |
| **Frozen v1.5.1** | `benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | **MATCH (100%)** |

No binaries were rebuilt, recompiled, replaced, renamed, moved, or modified in any way.

---

## 4. Protected Historical Evidence Preservation

All historical benchmark reports, protocols, and prior milestone verification documents were verified against their pre-change SHA-256 hashes:

| Historical Document | File Path | Authoritative SHA-256 | Verified SHA-256 | Status |
|:---|:---|:---|:---|:---:|
| **Amendment 04 Protocol** | `benchmarks/AMENDMENT_04_STAGE1_TOKEN_EFFICIENCY_PROTOCOL_REVISION.md` | `03C4B2786E2F62A6435A7EA81C54E65889619296FD4ED9873546EF20573030E2` | `03C4B2786E2F62A6435A7EA81C54E65889619296FD4ED9873546EF20573030E2` | **MATCH** |
| **Block 07 Final Live Report** | `benchmarks/STAGE1_BLOCK07_FINAL_LIVE_REPORT.md` | `325EF0EC3F14155B02F388C584DE9F85B88D04ABA142E87C8DB8FDCACCFD4266` | `325EF0EC3F14155B02F388C584DE9F85B88D04ABA142E87C8DB8FDCACCFD4266` | **MATCH** |
| **Block 07 Triad Report** | `benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/triad_report.md` | `9CBD121CD500EFF13130430DB0F836BB24338C15C523E9EBE9F3209318F32507` | `9CBD121CD500EFF13130430DB0F836BB24338C15C523E9EBE9F3209318F32507` | **MATCH** |
| **Block 07 Triad Summary** | `benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/triad_summary.json` | `DAF183FC193DC63B95A623DE303C44334087F54177AC031F4F5ED31D33643785` | `DAF183FC193DC63B95A623DE303C44334087F54177AC031F4F5ED31D33643785` | **MATCH** |
| **Milestone 2B Verification** | `amendments/draft-05/milestone-2b/final-preflight-verification.md` | `D4B5A48C03378E7DE355539888093455633297DEEF2CDCFC937181B92524EF39` | `D4B5A48C03378E7DE355539888093455633297DEEF2CDCFC937181B92524EF39` | **MATCH** |
| **Milestone 2B Readiness Summary** | `amendments/draft-05/milestone-2b/authorization-readiness-summary.md` | `4A59E17C4075E848FBA274AD361A397EF01119D9958D4FA5C6E9C681DCC214EE` | `4A59E17C4075E848FBA274AD361A397EF01119D9958D4FA5C6E9C681DCC214EE` | **MATCH** |

Bit-for-bit historical preservation is verified at 100%.

---

## 5. Golden Repository State and Worktree Metadata

### 5.1 Golden Repository Forensic Verification
- **Path:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`
- **Active Branch:** `test-branch`
- **Current HEAD Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`
- **Tracked Dirty State (`git diff --stat`):**
  `18 files changed, 208 insertions(+), 16 deletions(-)`
- **Untracked Directories:**
  `.engram/` (29 files preserved; exactly identical to Milestone 2B and Pre-Change snapshot)
- **Status Line Count (`git status --porcelain`):** Exactly 19 lines (18 tracked modified files + 1 untracked `.engram/` directory).

### 5.2 Worktree Registry Verification (`git worktree list`)
```text
C:/Users/Amal P S/OneDrive/Projects/Test/angular-ngrx-material-starter                                       1c28560 [test-branch]
C:/Users/Amal P S/AppData/Local/Temp/engram_pilot/runs/task_b_remember_me_block_06_v151_arm2_20261029_112143 1c28560 (detached HEAD)
C:/Users/Amal P S/AppData/Local/Temp/engram_pilot/runs/task_b_remember_me_block_07_v154_arm3_20261029_173706 1c28560 (detached HEAD)
C:/Users/Amal P S/AppData/Local/Temp/engram_pilot/runs/task_b_remember_me_block_07_v154_arm3_20261029_173830 1c28560 (detached HEAD)
C:/Users/Amal P S/AppData/Local/Temp/engram_pilot/runs/task_b_remember_me_block_07_v154_arm3_20261029_173938 1c28560 (detached HEAD)
```
- **Total Worktrees:** Exactly 5 (1 primary working tree + 4 historical detached run worktrees).
- **Integrity Status:** Completely unchanged. Zero worktrees were added, pruned, locked, or mutated during Milestone 2C.

---

## 6. Read-Only Enforcement Evidence

### 6.1 Static Verification of Codex CLI Capabilities
Direct query of the local Codex binary (`C:\Users\Amal P S\.gemini\antigravity\bin\codex.exe exec --help`) established:
```text
-s, --sandbox <SANDBOX>
    Sandbox mode for execution [possible values: read-only, workspace-write, danger-full-access]
```
The `--sandbox read-only` argument is natively supported by the local installed Codex binary.

### 6.2 Runner Implementation Verification
In `benchmarks/matched-pair-harness/src/runner.py`:
1. `SUPPORTED_SANDBOX_POLICIES = {"read-only", "workspace-write", "danger-full-access"}`.
2. In `build_codex_command`:
   - Emits `-s read-only` when `sandbox_policy == "read-only"`.
   - **Omits `--add-dir` entirely** when in `read-only` mode, ensuring no directory is granted writable status.
   - Tested offline in `test_milestone_2c_remediation.py::test_runner_codex_command_read_only`.

### 6.3 Post-Run Defense-in-Depth Mutation Check
In `benchmarks/matched-pair-harness/src/orchestrator.py`:
- In `_execute_single_arm`:
  ```python
  if is_read_only_task:
      if git_impact.files_changed_count > 0 or git_impact.insertions > 0 or git_impact.deletions > 0:
          arm_result.score = 0.0
          arm_result.passed_all_requirements = False
          if "REQ-2" in arm_result.requirements_detail:
              arm_result.requirements_detail["REQ-2"]["passed"] = False
              arm_result.requirements_detail["REQ-2"]["actual"] = (
                  f"FAIL: Read-only task detected {git_impact.files_changed_count} modified files "
                  f"(+{git_impact.insertions}/-{git_impact.deletions})."
              )
  ```
- If an agent bypasses sandbox boundaries or mutates any file in the workspace, the harness automatically fails closed, voids the score, records `REQ-2 = FAIL`, and registers the invariant violation in run telemetry.

---

## 7. Output Directory Overwrite Protection

### 7.1 Path Traversal and Protected Root Rejection
In `benchmarks/matched-pair-harness/src/orchestrator.py`:
- `self.output_dir` is converted to an absolute path via `Path(output_dir).resolve()`.
- Explicitly verified that output directories resolving to protected roots (repo root, `src`, golden repository) raise `SafetyViolationError` immediately before any execution or file writing.

### 7.2 Automatic Timestamped Subdirectory Allocation on Evidence Collision
- If `output_dir` already contains benchmark run evidence (e.g. `triad_summary.json`, `run_telemetry.json`, `single_summary.json`, `single_report.md`):
  ```python
  evidence_markers = ["triad_summary.json", "single_summary.json", "run_telemetry.json", "triad_report.md", "single_report.md"]
  has_prior_evidence = any((self.output_dir / marker).exists() for marker in evidence_markers)
  if has_prior_evidence:
      timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
      self.output_dir = self.output_dir / f"run_{timestamp}"
      self.output_dir.mkdir(parents=True, exist_ok=True)
  ```
- This guarantees that prior trial runs (such as Block 06, Block 07, or Block 08) can never be overwritten by a subsequent smoke test or trial run.
- Tested offline in `test_milestone_2c_remediation.py::test_orchestrator_output_dir_timestamping_on_collision`.

---

## 8. Confirmation of Zero Model Inference / Spend

It is categorically certified that throughout Milestone 2C:
1. **Zero Model Inference:** No LLM or SLM inference was requested or executed.
2. **Zero Codex Agent Runs:** The Codex CLI was never invoked for code generation, task execution, or agent operations. (Only static `--help` flag introspection was executed).
3. **Zero Network Calls:** Zero network requests, HTTP/HTTPS sockets, or external API calls were initiated.
4. **Zero Financial Spend:** Zero API credits or quota were consumed ($0.00 spent).
5. **Strictly Offline:** 100% of all verification and testing was executed via local Python unit tests and read-only filesystem inspection.
