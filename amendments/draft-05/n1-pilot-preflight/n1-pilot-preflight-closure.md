# N=1 Matched-Pair Pilot: Preflight Closure Report

**Milestone:** Engram N=1 Matched-Pair Pilot Final Preflight  
**Document ID:** `amendments/draft-05/n1-pilot-preflight/n1-pilot-preflight-closure.md`  
**Evaluation Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Active Branch:** `feature/async-precommit-guard`  
**Baseline Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`  
**Designated Trial:** Matched Pair ($N=1$, `pilot_01`)  
**Mandatory Final Decision:** `CONDITIONAL — OWNER ACTION REQUIRED`  

---

## 1. Executive Summary

This closure report synthesizes the findings of the final operational preflight for the first controlled Engram token-efficiency experiment. The preflight audit covered execution symmetry, cryptographic binary mapping, token guard mechanics, task acceptance criteria, filesystem isolation, and CLI launch command validation.

The assessment confirms that the harness infrastructure is largely ready, secure, and physically isolated. However, because live execution depends on affirmative owner confirmation of subscription capacity and model entitlements, and because an evaluator static check deficiency was identified that would falsely penalize task correctness, the final readiness classification is formally designated:

$$\mathbf{CONDITIONAL\ —\ OWNER\ ACTION\ REQUIRED}$$

---

## 2. Multi-Dimensional Readiness Assessment

| Audit Dimension | Preflight Status | Key Evidence & Findings |
|---|---|---|
| **1. Execution Configuration** | **VERIFIED WITH DISCREPANCY** | `--mode pair` establishes symmetric, isolated arms. Control A is genuinely unassisted (0 MCP, 0 Turn-0 injection). Candidate binary validated via SHA-256 (`F56347EF...`). CLI parser does not define `--engram-bin`; binary bound via verified default path and `--expected-treatment-sha256`. |
| **2. Token Controls** | **VERIFIED & QUALIFIED** | 175k nominal budget, 45k headroom reserve, 130k pre-dispatch stop verified. In-flight request overrun risk, remote generation persistence, and subscription rate-limit mechanics are rigorously documented. Nominal figures are planning limits, not provider hard caps. |
| **3. Functional Acceptance** | **CONDITIONAL / DEFECT IDENTIFIED** | L0 (`cargo check`), L2 (`cargo test`), L3 (`cargo build`), boundary diffs, and dependency mutation checks verified. Defect: `evaluator.py` `_check_static_requirement` lacks handlers for `task_commit_cache_invalidation`, causing REQ-1 and REQ-3 to fail and capping score at $C = 0.50$. |
| **4. Workspace & Output** | **VERIFIED & PRISTINE** | Output directory `reports/runs/pilot_01` tested; does not exist; zero collision. Detached Git worktrees provision independent cold-start environments under `%TEMP%`. Dependency strategy `none` confirmed safe for Cargo task. |
| **5. Owner-Dependent Conditions** | **OWNER ACTION REQUIRED** | 10-point checklist generated. Subscription allowance, model entitlement, budget acceptance, and live authorization require affirmative owner sign-off. |
| **6. Launch Command** | **VERIFIED & TESTED** | Command line verified against `argparse`. Multi-line and single-line PowerShell commands prepared and checked. Execution strictly withheld. |

---

## 3. Detailed Identification of Conditions and Dependencies

### 3.1 Owner-Dependent Actions (Prerequisites for Execution)
1. **Subscription Quota Confirmation:** Confirm available capacity in active ChatGPT Plus/Team 3-hour window.
2. **Model Entitlement Check:** Confirm subscription entitlement to model `gpt-5.6-luna`.
3. **Budget and Overrun Acceptance:** Explicitly accept nominal budget of 175,000 tokens per arm (350,000 total) and acknowledge that in-flight turn bursts may exceed nominal reserves.
4. **Live Authorization Instruction:** Issue a separate, explicit command authorizing live inference.

### 3.2 Technical Harness Dependency (Recommended Pre-Execution Fix)
- **Evaluator Handler Gap:** Before running a live trial where functional correctness ($C = 1.0$) is required to unlock comparative savings, `evaluator.py`'s `_check_static_requirement` should be updated with static inspection patterns for `task_commit_cache_invalidation` (verifying SQLite `index.db` commit metadata and `engram clean --cache` CLI presence). Otherwise, both arms will pass compilation and tests but fail static evaluation, resulting in an automatic `MUTUAL_ACCEPTANCE_FAILURE`.

---

## 4. Preservation & Safety Adherence Record

Throughout this preflight milestone, all safety and preservation rules have been strictly respected:
- **Zero Live Inference:** $0.00 spent, 0 API calls initiated, 0 tokens consumed.
- **Zero Process Launch:** Codex agent runtime was not executed.
- **Git State Preserved:** Active branch remains `feature/async-precommit-guard` at commit `c70845b4d5913457f1b176fb0370b80dd792711f`. Zero commits, pushes, merges, stashes, or resets were performed.
- **Binaries Preserved:** Candidate Phase 1 binary (`F56347EF...`) and Frozen binaries remain unaltered.
- **Golden Repository Preserved:** Source repository state untouched.
- **Deliverables Isolated:** All artifacts written exclusively to `amendments/draft-05/n1-pilot-preflight/`.

---

## 5. Final Readiness Classification

$$\mathbf{CONDITIONAL\ —\ OWNER\ ACTION\ REQUIRED}$$

This preflight evaluation is concluded. No further automated actions will be taken. The system awaits explicit instructions from the owner.
