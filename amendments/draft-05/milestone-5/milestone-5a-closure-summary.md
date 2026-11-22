# Milestone 5A Closure Summary
## Controlled Token Efficiency Experiment — Offline Preparation and Preregistration

**Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/async-precommit-guard`  
**HEAD Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`  
**Milestone:** Engram Amendment 05 — Milestone 5A  
**Formal Classification:** `COMPLETE — LIVE EXPERIMENT PREREGISTERED AND READY FOR OWNER REVIEW`  

---

### Mandatory Governance Declaration

> **"Milestone 5A is an offline preparation and preregistration phase. No live model inference, token-consuming benchmark, or external API request was authorized or executed. Milestone 5B requires separate explicit owner authorization."**

---

### 1. Executive Summary

Milestone 5A establishes a rigorous, scientifically controlled, and methodologically reproducible experimental protocol to evaluate the central hypothesis:

$$\mathbf{H_1}: \text{Using Engram-generated engineering context reduces total AI-agent token consumption per successfully completed engineering task, compared with an otherwise identical coding-agent workflow without Engram.}$$

All 11 phases designated in the Milestone 5A specification were executed under strict offline constraints ($0.00 marginal spend, zero API calls, zero binary rebuilds, golden repository untouched, and zero Git modifications).

---

### 2. Key Audit and Methodological Findings

#### 2.1 Historical Evidence Reconciliation (Phase 1)
- **Task A (Blocks 01–05):** In all 15 executions across 3 arms, `mcp_tool_calls_count` was exactly 0. The prompt provided the exact file and target string, so the agent solved it using native shell commands. Task A evaluates tool schema definition overhead and edit retry variance, not context retrieval efficiency. 88.61% of tokens were cached inputs; token costs scaled quadratically with turn count.
- **Task B (Blocks 06 and 07):** Both baseline (Block 06, Turn 9, 253,748 tokens) and candidate treatment (Block 07, Turn 11, 270,506 tokens) breached the 250k ceiling and achieved $C = 0.00$. In Block 07, MCP tools failed on Turn 1 due to client security policies (`approval_policy: never`), and Turn-0 Change Contracts caused agent refusal.
- **Suitability Verdict:** Historical benchmark data cannot support claims of context token savings. Historical failures were driven by operational and client configuration deadlocks rather than Engram indexing flaws.

#### 2.2 Milestone 4 Implementation & Test Discrepancy Forensic Audit (Phase 2)
- Milestone 4 features (`--subpath`, `--language`, early candidate pruning at SQLite read, ranking inversion `Implementation: 10.0 / Test: 7.0`) were independently inspected and confirmed fully operational with 0 token leakage outside scope.
- **Test Exit Code 1 Discrepancy Resolved:** Aggregate `cargo test --offline` exits with code 1 solely due to multi-threaded Windows WindowStation DACL contention in `tests/empirical_live_test.rs`. Running with `--test-threads=1` passes 100% (99/99 passed). Combining 111 selective Rust tests with 233 Python tests obscured this multithreading requirement in Milestone 4 summary reporting.

#### 2.3 Experimental Design & Treatment Isolation (Phase 3)
- Selected **Generated Context Only (Option 1)** as the Primary Treatment.
- Both Arm A (Baseline) and Arm B (Engram-assisted) receive identical native shell and file editing tools, identical models, identical baselines, and identical timeouts.
- Arm B receives an AST-grounded context package compiled offline into its Turn-0 prompt.
- Background MCP daemons and restrictive Turn-0 Change Contracts are omitted from the primary treatment, eliminating client approval rejections and isolating the pure value of architectural context.

#### 2.4 Primary Task Selection (Phase 4)
- Evaluated three tasks: Task 1 (JSON Explain, localized), Task 2 (Commit-Bound Cache Invalidation, multi-file), and Task 3 (Worktree Hook Detection, architectural).
- Selected **Task 2: Commit-Bound Cache Invalidation and Stale Index Cleanup** as the Primary Task. It requires multi-module discovery across `compiler.rs`, `analyzer.rs`, and `main.rs`, while remaining bounded within ~100k tokens.

#### 2.5 Authoritative Token Accounting & Double-Counting Safeguards (Phase 5)
- Primary metric: Total provider-reported model tokens ($T_{\text{task}}$) for completed tasks where $C = 1.00$.
- Mathematical enforcement prevents double-counting: cached input tokens are recognized as a subset of prompt tokens; reasoning tokens are recognized as a subset of completion tokens.
- Engram local preprocessing is recorded separately in CPU and wall-clock units, with zero synthetic token attribution.

#### 2.6 Five-Gate Functional Acceptance Battery (Phase 6)
- Compilation (Gate 0), Baseline Regressions (Gate 1), Task Verification (Gate 2), Boundary Audit (Gate 3), and Blinded Code Review (Gate 4).
- Rule: A run that consumes fewer tokens but fails functional acceptance ($C < 1.00$) is classified as a **FAILURE**, not a saving.

#### 2.7 Re-Calibrated Token Ceiling & Overrun Bounds (Phase 7)
- Proposed thresholds: **175,000 token hard ceiling per arm**, **35,000 token reserve headroom**, and **140,000 nominal stop threshold** (Combined 350,000 tokens).
- Acknowledges that external polling monitors cannot halt remote in-flight turns mid-generation. The 35,000-token reserve mathematically absorbs worst-case single-turn context compounding within the 175,000 ceiling.

#### 2.8 Fiscal & Account Readiness (Phase 8)
- Comprehensive checklist prepared; all account-specific billing items marked `OWNER VERIFICATION REQUIRED`.
- Warnings against inferring billing mode from local config files.

#### 2.9 Statistical Analysis Plan (Phase 9)
- Formally designated as **EXPLORATORY** ($N = 3$ matched pairs, counterbalanced execution order).
- Primary metric: Paired percentage savings rate on accepted pairs.
- Completion supremacy invariant: Lower tokens with lower completion rate is classified as a regression.

#### 2.10 Harness Readiness & Implementation Blockers (Phase 10)
- Core telemetry extraction, streaming token tracking, and evidence vaulting verified operational.
- Two blockers identified for owner review: `isolation.py` coupling to Angular `node_modules` (must be adapted for Cargo repositories) and missing task config JSON for Task 2.

#### 2.11 Experiment Preregistration (Phase 11)
- Formally authored and conceptually frozen in `experiment-preregistration.md`.

---

### 3. Preservations and Governance Statement

| Security / Governance Invariant | Status | Evidence |
| :--- | :---: | :--- |
| **Model Inference / External Calls** | **ZERO ($0.00 SPEND)** | Strictly offline execution; zero tokens consumed. |
| **Candidate Phase 1 Binary** | **PRESERVED** | SHA-256 matches `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` |
| **Frozen v1.5.1 / v1.5.4 Binaries** | **PRESERVED** | Hashes match `474A1A7...` and `CF807EA...` |
| **Golden Repository** | **PRESERVED** | `angular-ngrx-material-starter` untouched. |
| **Git Repository State** | **PRESERVED** | Branch `feature/async-precommit-guard` at HEAD `c70845b4d5913457f1b176fb0370b80dd792711f`. |

---

### 4. Milestone Deliverables Manifest

All 12 required reports have been authored under `amendments/draft-05/milestone-5/`:

1. `historical-evidence-audit.md` (Reconciliation of Blocks 01–07 data)
2. `milestone-4-readiness-audit.md` (Implementation audit and test discrepancy resolution)
3. `experimental-design.md` (Matched-pair architecture and treatment isolation)
4. `task-selection.md` (Candidate task evaluation and selection of Task 2)
5. `token-accounting-specification.md` (Authoritative usage aggregation and double-counting prevention)
6. `functional-acceptance-protocol.md` (Five-gate verification battery and blinded review)
7. `token-ceiling-and-stopping-policy.md` (175k/35k/140k exposure guard and in-flight overrun bounds)
8. `billing-authorization-checklist.md` (Pre-flight fiscal verification for repository owner)
9. `statistical-analysis-plan.md` (Exploratory paired methodology and censoring rules)
10. `harness-readiness-audit.md` (Inspection of matched-pair harness and blocker identification)
11. `experiment-preregistration.md` (Authoritative conceptually frozen experiment protocol)
12. `milestone-5a-closure-summary.md` (Formal closure report, this document)

---

### 5. Milestone Classification

**`COMPLETE — LIVE EXPERIMENT PREREGISTERED AND READY FOR OWNER REVIEW`**
