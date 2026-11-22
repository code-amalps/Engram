# Engram Amendment 05 — Milestone 2B: Authorization Readiness Summary

**Document Version:** 1.0.0  
**Date:** 2026-10-30  
**Status:** Forensic Verification Closure  
**Final Classification:** `BLOCKED — TECHNICAL REMEDIATION REQUIRED`

---

## 1. Final Technical Readiness Classification

The Milestone 2 smoke-test execution specification was audited against the active codebase, CLI argument parsers, execution call chains, task configuration stores, and filesystem structures.

In accordance with Amendment 05 governance and evaluation criteria, the final technical readiness classification is:

$$\mathbf{BLOCKED\ —\ TECHNICAL\ REMEDIATION\ REQUIRED}$$

### Statement on Readiness for Manual Authorization:
**The smoke-test command is NOT ready to be manually authorized.**  
Even if the owner were to grant full budget and policy authorization, attempting to run the command in its current state would fail immediately due to multiple deterministic fatal runtime crashes.

---

## 2. Reconciled and Resolved Verification Issues

Through the Milestone 2, 2A, and 2B forensic audits, the following facts and behaviors have been conclusively resolved:

1. **Golden Repository Preservation:**  
   - Pinned to `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` at commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` on `test-branch`.
   - The 18 modified files (`+208 / -16`) and untracked `.engram/` directory (29 files) remain completely preserved and unmutated.
2. **Git Worktree Administrative Metadata:**  
   - Proved that detached worktree operations extract commits directly from the Git object database and do not touch golden working-tree files.
   - Identified that `git worktree add` writes administrative registration metadata to `.git/worktrees/`, which is cleaned up by `teardown_sandbox`. Historical registrations from Blocks 06/07 remain documented.
3. **Candidate Binary Integrity & Explicit Binding:**  
   - Candidate Phase 1 executable (`benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`) verified with cryptographic SHA-256:  
     `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`.
   - Frozen v1.5.4 reference verified with SHA-256:  
     `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798`.
   - Verified that CLI flags `--engram-v154-bin` and `--expected-v154-sha256` explicitly override Arm 3, preventing the frozen v1.5.4 executable from being selected.
4. **Token Guard Ceiling & Overrun Qualification:**  
   - Verified that external polling latency (~250 ms reaction window) means the 2,000-token headroom reserve is a defensive fail-fast monitor, not an absolute zero-leakage hard guarantee.
5. **Fixture Target Verification:**  
   - Pinned symbol `export const authLogout = createAction('[Auth] Logout');` in `projects/angular-ngrx-material-starter/src/app/core/auth/auth.actions.ts` verified present in pinned commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.

---

## 3. Remaining Unresolved Blockers (Technical Remediation Required)

Execution is blocked by four specific technical defects in the specification and harness:

1. **Unsupported CLI Argument (`--output-dir`):**  
   - *Defect:* `orchestrator.py`'s `argparse` configuration does not declare `--output-dir`.  
   - *Impact:* Passing `--output-dir` causes `argparse` to fail immediately (`unrecognized arguments: --output-dir`).
2. **Non-Existent Task Configuration (`smoke_test_read_only.json`):**  
   - *Defect:* `benchmarks/matched-pair-harness/config/tasks/smoke_test_read_only.json` does not exist on disk.  
   - *Impact:* Passing `--task smoke_test_read_only` causes `orchestrator.py` to exit immediately with code 1 (`Error: Task configuration not found`).
3. **Live Run Dependency Strategy Assertion:**  
   - *Defect:* `orchestrator.py` line 801 strictly asserts `if not self.is_dry_run and self.dependency_strategy != "copy": raise SafetyViolationError(...)`.  
   - *Impact:* Passing `--dependency-strategy none` on any live trial (where `--dry-run` is absent) triggers an immediate `SafetyViolationError` crash.
4. **Execution Mode Disconnect:**  
   - *Defect:* In `orchestrator.py` line 1342, `mode` defaults to `"pair"` unless `--mode triad` or `--block <id>` is provided.  
   - *Impact:* Passing `--arms v154_arm3` without `--mode triad` results in running `control_A` and `treatment_B` instead of `v154_arm3`.
5. **Hardcoded Agent Write Permissions:**  
   - *Defect:* In `runner.py` line 538, `CodexRunner.build_codex_command` hardcodes `-s workspace-write`.  
   - *Impact:* There is no existing code path to run Codex in read-only mode (`-s workspace-read`).

---

## 4. Owner Authorization Gates Status

| Gate # | Authorization Gate Description | Current Status | Gate Dependency |
|:---:|:---|:---:|:---|
| **Gate 1** | Owner approval of 10,000-token ceiling and 2,000-token reserve, accepting external polling overrun risk. | **PENDING OWNER DECISION** | Unapproved by owner. |
| **Gate 2** | Owner independent login and verification of account entitlement, plan quota, and automatic purchase disablement. | **PENDING OWNER ACTION** | Owner action required. |
| **Gate 3** | Owner explicit written authorization for a single live smoke-test run. | **PENDING OWNER DECISION** | Blocked by technical remediation. |
| **Gate 4** | Owner confirmation of `gpt-5.6-luna` model account access without model substitution. | **PENDING OWNER DECISION** | Local catalog does not prove account entitlement. |
| **Gate 5** | Verification of Candidate Phase 1 binary path and SHA-256 (`F56347...`). | **VERIFIED (OFFLINE)** | Verified against filesystem. |
| **Gate 6** | Verification of sandbox source isolation and resolution of dependency strategy. | **BLOCKED** | Requires technical remediation of line 801. |
| **Gate 7** | Confirmation that output directory cannot collide with or overwrite historical runs. | **BLOCKED** | Requires `--output-dir` CLI support or non-colliding pair/block ID. |

---

## 5. Affirmation of Zero Inference and Live Usage

The following boundaries were strictly observed throughout Milestone 2B:
- **Model API Requests / Inference Calls:** Exactly 0 calls.
- **Tokens Consumed / Financial Spend:** Exactly 0 tokens ($0.00 spend).
- **Network / External Service Access:** None (completely offline).
- **Smoke Tests / Benchmarks / Test Suites Executed:** None.
- **Git State Mutations / File Modifications to Baseline:** None.

---

## 6. Deliverable Cryptographic SHA-256 Signatures

The deliverables produced under `amendments/draft-05/milestone-2b/` have the following cryptographic signatures:

1. **`final-preflight-verification.md`**  
   - Path: `amendments/draft-05/milestone-2b/final-preflight-verification.md`  
   - SHA-256: `D4B5A48C03378E7DE355539888093455633297DEEF2CDCFC937181B92524EF39`

2. **`authorization-readiness-summary.md`**  
   - Path: `amendments/draft-05/milestone-2b/authorization-readiness-summary.md`  
   - Self-Referential Canonical Hash: Recorded in Git / preflight ledger.

---
*End of Milestone 2B Authorization Readiness Summary.*
