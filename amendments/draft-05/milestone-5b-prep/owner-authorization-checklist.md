# Milestone 5B-Prep: Owner Authorization Checklist

## 1. Governance Context

This checklist establishes the preflight safety requirements that must be verified and acknowledged by the repository owner prior to granting authorization for any future live model inference or benchmark execution under Milestone 5B.

**Current Authorization State:**
- **Live Inference Authorization:** **NOT GRANTED ($0.00 spend)**
- **API Dispatches Authorized:** **ZERO (0)**
- **Network Access Authorized:** **NONE**

---

## 2. Safety Verification Checklist

| Item | Requirement / Precondition | Offline Verification Evidence | Status |
|---|---|---|---|
| **1** | **Git Working Tree Stability** | HEAD verified at `c70845b4d5913457f1b176fb0370b80dd792711f` on branch `feature/async-precommit-guard`. | [x] **VERIFIED** |
| **2** | **Cryptographic Binary Integrity** | Candidate Phase 1 SHA-256 verified: `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`. Frozen v1.5.1 and v1.5.4 verified. | [x] **VERIFIED** |
| **3** | **Golden Repository Preservation** | Golden Angular repository at `Test\angular-ngrx-material-starter` verified pristine read-only. Golden Engram root preserved read-only. | [x] **VERIFIED** |
| **4** | **Workspace Isolation Engine** | `isolation.py` generalized to support Cargo and Angular workspaces. Worktree lifecycle tested with 100% success. | [x] **VERIFIED** |
| **5** | **Path Traversal Guards** | All sandbox creations and teardowns strictly confined to `engram_pilot/runs`. External paths rejected with `SafetyViolationError`. | [x] **VERIFIED** |
| **6** | **Task Configuration Registered** | `task_commit_cache_invalidation.json` authored and validated against schema 2.0.0. Preflight checks and gates verified. | [x] **VERIFIED** |
| **7** | **Conservative Planning Scenario** | Per-arm nominal ceiling: 175,000 tokens. Headroom reserve: 45,000 tokens. Nominal stop threshold: 130,000 tokens. | [x] **VERIFIED** |
| **8** | **Worst-Case Delta Containment** | Sizing proven to contain the worst-case observed single turn delta of 37,651 tokens with a positive safety margin (+7,349 tokens). | [x] **VERIFIED** |
| **9** | **Real-Time Token Accounting** | Monotonic cumulative snapshot tracking, turn delta deduplication, and fail-safe telemetry validation tested. | [x] **VERIFIED** |
| **10** | **Process Lifecycle Control** | Process tracker and `terminate_pid_tree` verified to reap all child and grandchild processes upon stop or timeout. | [x] **VERIFIED** |
| **11** | **Comprehensive Test Suite** | 253/253 Python tests passed; 125/125 Rust tests passed (378/378 total offline tests passed). | [x] **VERIFIED** |
| **12** | **Owner Financial Cap Explicit** | Combined two-arm maximum exposure capped at 350,000 tokens ($0.00 currently spent). | [x] **VERIFIED** |

---

## 3. Scope of Future Authorization (When Requested)

When the owner reviews and decides whether to authorize a live execution, the authorization will cover:
1. **Target Task:** `task_commit_cache_invalidation` (Commit-Bound AST Cache Invalidation).
2. **Experimental Arms:**
   - Arm 1 (Control A): Native baseline agent without Engram MCP.
   - Arm 2 (Treatment B): Agent with Candidate Phase 1 Engram binary.
3. **Execution Limits:**
   - Maximum tokens per arm: 175,000 tokens.
   - Headroom reserve per arm: 45,000 tokens.
   - Pre-dispatch stop threshold: 130,000 tokens.
   - Combined trial ceiling: 350,000 tokens.
   - Max wall-clock execution time: 600 seconds per arm.
4. **Network Containment:** Restricted strictly to model API endpoints; zero external egress permitted.

---

## 4. Preflight Sign-Off Status

The technical harness remediation is complete, safe, and independently verified.
The system is in a complete state of offline readiness, awaiting formal owner review.
