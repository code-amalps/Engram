# Engram Milestone 5F — Final Closure Report
## Complete Harness, Integration, Code Excerpt, and Telemetry Remediation

**Date:** 2026-11-04  
**Milestone:** 5F — Harness & Integration Remediation  
**Status:** ACCEPTED WITH QUALIFICATIONS  
**Independent Audit Artifact:** `amendments/draft-05/milestone-5f/independent-acceptance-audit.md`  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Active Git Branch:** `feature/async-precommit-guard`  
**HEAD Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`  

---

## 1. Milestone Mission & Objectives

Milestone 5F was commissioned following the Milestone 5E forensic investigation into `pilot_01`. Its purpose was to transition from theoretical diagnosis to concrete, verified implementation across four vital remediation workstreams:

1. **Workstream A (Harness & Integration)**: Fix Codex MCP tool registration via workspace `.codex/config.toml`, correct the token guard variable attribution defect in `StreamingTokenTracker`, restore Turn-0 context injection for `treatment_B`, and implement accurate skipped-arm reporting.
2. **Workstream B (Knowledge Engine Optimization)**: Enhance Engram compiler and MCP tools to return exact line-bounded code excerpts (`L{start}..L{end}`) with completeness indicators (`[COMPLETE_BLOCK]` / `[INCOMPLETE_EXCERPT]`) and uncertainty signaling, ensuring tokens returned genuinely replace file reading.
3. **Workstream C (Empirical Replacement Accounting)**: Build end-to-end token replacement observation into `telemetry.py` and JSON schemas to measure whether retrieved files were subsequently re-read via shell.
4. **Workstream D (Future Architecture Preservation)**: Document the 12 long-term roadmap principles in `future-architecture-preservation.md`.

All objectives have been fully implemented and independently audited via automated test suites without any live model inference.

---

## 2. Independent Acceptance Audit Record

Following independent inspection in `amendments/draft-05/milestone-5f/independent-acceptance-audit.md`, Milestone 5F was formally **ACCEPTED WITH QUALIFICATIONS**.

### 2.1 The 14 Audit Findings & Verdicts

| # | Inspection Criterion | Verdict | Verified Details |
| :-: | :--- | :---: | :--- |
| **1** | `.codex/config.toml` Contract & Runtime Exposure | **PASS** | Generated TOML conforms strictly to Codex configuration contract; distinction between offline generation and unobserved live exposure recorded. |
| **2** | Turn-0 Context Trace & Boundary Analysis | **PARTIAL** | Pipeline verified end-to-end; boundary identified regarding possible instruction duplication between `.github/copilot-instructions.md` and `effective_prompt`. |
| **3** | StreamingTokenTracker & Guard Variable Attribution | **PASS** | Attribution bug remediated (single-request input tokens 41k vs. cumulative thread tokens 115k separated); next-turn estimation evaluates accurately. |
| **4** | Token Metric Categorization Distinction | **PASS** | Context size, request input, cumulative usage, output, cached input, and reasoning tokens strictly segregated and invariants enforced. |
| **5** | TokenReplacementAccounting Observability Limits | **PARTIAL** | Standard shell reads (`cat`, `type`, `Get-Content`, `head`, `tail`) and MCP tools observable; non-shell scripts (`python -c`, `node -e`) remain unobservable. |
| **6** | Canonical Matching & Excerpt Completeness | **PARTIAL** | Normalized path matching verified; excerpt counter aggregates both complete blocks and partial excerpts into a single counter. |
| **7** | Skipped-Arm Reporting & Schema Compatibility | **PASS** | Skipped arms serialize `score: null` and `passed_all_requirements: null`; 2-arm verdict produces `TREATMENT_FAILED_CONTROL_SKIPPED`, passing Draft 2020-12 schema validation. |
| **8** | Headroom Reserve Reconciliation (15k vs. 45k) | **PASS** | 45k reserve sized for Task B; for Task Commit Invalidation, dynamic estimation ($\max(\text{request}, \text{reserve}) = 41k$) supersedes static reserve once data is observed. |
| **9** | Claimed Python and Rust Test Counts | **PASS** | Python: 7/7 targeted passed, 302/302 full harness passed. Rust: 63/63 unit/compiler/MCP passed. `cargo check --tests` clean (0 errors, 0 warnings). |
| **10** | Golden Repo Integrity & Failed Command Audit | **PASS** | Erroneous temp command reported; authoritative golden repo at `Projects\Test\angular-ngrx-material-starter` verified pristine at commit `1c285608`. |
| **11** | SHA-256 Manifest Recalculation | **PASS** | Checksums in `amendments/draft-05/milestone-5f/SHA256SUMS` match disk recalculations 100%. |
| **12** | Frozen Binary Hash Verification | **PASS** | Candidate Phase 1, v1.5.1, and v1.5.4 hashes verified identical to baseline certifications. |
| **13** | Git Branch & HEAD Invariance | **PASS** | Branch `feature/async-precommit-guard` at commit `c70845b4d5913457f1b176fb0370b80dd792711f` unchanged. |
| **14** | Scrutiny of Claims Exceeding Evidence | **PASS** | Claims qualified: offline generation verified, while live agent behavioral reliance remains for authorized observation. |

### 2.2 The Three Explicit Qualifications
1. **Runtime MCP Exposure & Reliance**: Runtime MCP exposure and actual agent reliance on excerpts remain unverified through live interaction (as live inference was strictly prohibited during Milestone 5F).
2. **Turn-0 Duplication Boundary**: Potential Turn-0 prompt duplication involving `.github/copilot-instructions.md` being parsed natively alongside `effective_prompt` remains to be empirically observed.
3. **Strict Authorization Enforcement**: Milestone 5G (Live Pilot Readiness) prepares the experiment only; live execution requires fresh, explicit owner authorization.

---

## 2. Workstream Completion Matrix

| Workstream | Scope & Deliverable | Implementation Files | Status |
| :--- | :--- | :--- | :---: |
| **Workstream A** | Codex MCP config generation | `isolation.py`, `runner.py` | **COMPLETE** |
| **Workstream A** | Sandbox cleanliness whitelist (`.codex`) | `isolation.py` | **COMPLETE** |
| **Workstream A** | Token guard attribution fix (41k vs 115k) | `runner.py` | **COMPLETE** |
| **Workstream A** | Turn-0 context injection for `treatment_B` | `runner.py` | **COMPLETE** |
| **Workstream A** | Skipped arm null schema & verdict logic | `schemas/`, `orchestrator.py` | **COMPLETE** |
| **Workstream B** | Bounded code excerpts (`L{start}..L{end}`) | `src/compiler.rs`, `src/mcp.rs` | **COMPLETE** |
| **Workstream B** | Completeness indicators (`COMPLETE_BLOCK`) | `src/compiler.rs`, `src/mcp.rs` | **COMPLETE** |
| **Workstream B** | Low-confidence uncertainty warnings | `src/compiler.rs` | **COMPLETE** |
| **Workstream C** | `TokenReplacementAccounting` dataclass | `telemetry.py` | **COMPLETE** |
| **Workstream C** | Rollout & stdout event stream parsing | `telemetry.py` | **COMPLETE** |
| **Workstream C** | Re-read tracking & tokens-per-task | `telemetry.py`, `orchestrator.py` | **COMPLETE** |
| **Workstream C** | JSON Schema update for replacement | `run_telemetry.schema.json` | **COMPLETE** |
| **Workstream D** | 12 Architectural Pillars Preservation | `future-architecture-preservation.md` | **COMPLETE** |

---

## 3. Offline Verification Evidence

### 3.1 Targeted Milestone 5F Regression Test Suite
A dedicated regression test file (`benchmarks/matched-pair-harness/tests/test_milestone_5f_remediation.py`) was implemented and executed:
```
tests/test_milestone_5f_remediation.py::TestMilestone5FRemediation::test_comparative_analysis_verdict_for_skipped_arm PASSED
tests/test_milestone_5f_remediation.py::TestMilestone5FRemediation::test_configure_sandbox_codex_mcp PASSED
tests/test_milestone_5f_remediation.py::TestMilestone5FRemediation::test_skipped_arm_telemetry_schema_validation PASSED
tests/test_milestone_5f_remediation.py::TestMilestone5FRemediation::test_streaming_token_tracker_correctly_halts_at_genuine_breach PASSED
tests/test_milestone_5f_remediation.py::TestMilestone5FRemediation::test_streaming_token_tracker_multi_turn_accounting PASSED
tests/test_milestone_5f_remediation.py::TestMilestone5FRemediation::test_telemetry_replacement_accounting_from_events PASSED
tests/test_milestone_5f_remediation.py::TestMilestone5FRemediation::test_turn0_guidance_present_in_runner_constants PASSED

7 passed in 1.64s
```

### 3.2 Full Python Test Suite
The comprehensive harness offline test suite passed with zero regressions:
- Total tests executed: 302
- Passed: 302
- Failed: 0
- Execution time: ~220 seconds

### 3.3 Rust Engine Test Suite
All compiler, CLI, and MCP unit/integration test suites passed:
- `mcp_consolidation_test`: 13 passed, 0 failed
- `turn0_injection_test`: 5 passed, 0 failed
- Library unit tests: 17 passed, 0 failed
- Binary unit tests: 28 passed, 0 failed
- `cargo check --tests`: Clean (0 errors, 0 warnings)

---

## 4. Safety & Authorization Status

1. **Zero Inference Consumed**: In strict adherence to experimental protocol and user instructions, zero live model API calls were initiated during Milestone 5F.
2. **Prior Authorization Status**: The single-experiment authorization granted for `pilot_01` was fully consumed during Milestone 5D/5E.
3. **No Automatic Dispatch**: The harness is currently in a safe, offline, non-executing state.
4. **Milestone 5G Readiness**: All technical prerequisites, configurations, guards, and accounting systems required for an authorized re-run are complete and verified. Execution of Milestone 5G strictly requires explicit new authorization from the repository owner.

---

## 5. Formal Recommendation

Milestone 5F is formally **CLOSED**. The harness and engine have achieved the technical maturity and verification rigor necessary for owner authorization review.
