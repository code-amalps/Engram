# Engram Phase 5.4: Initial Evidence Preservation & Repository Audit

**Document Path:** `docs/phase-5-4/PHASE_5_4_INITIAL_EVIDENCE_AUDIT.md`  
**Milestone:** Phase 5.4 — Protocol Overhead, Adaptive Context Optimization & Live Benchmark Validation  
**Auditor:** Principal Systems Engineer & Independent Code Auditor  
**Date:** November 19, 2026  
**Status:** **PASSED**  

---

## 1. Executive Summary

This document establishes the verified baseline for Phase 5.4. It audits the state of the repository, verifies release candidate provenance, and preserves all Phase 5.3 live empirical artifacts without mutation or omission.

---

## 2. Repository State Audit

### 2.1 Git Topology & Working Tree
- **Current Branch:** `feature/phase2-stage5-remediation`
- **HEAD Commit SHA-1:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`
- **Working Tree State:**
  - Tracked modifications: Exactly 1 file modified in the benchmark harness:
    `benchmarks/matched-pair-harness/src/runner.py` (Pre-dispatch boundary condition and streaming token tracker deduplication fix ratified during Phase 5.3 live validation).
  - Untracked artifacts:
    - Phase 5.3 reports: `PHASE_5_3_INITIAL_AUDIT.md`, `PHASE_5_3_PROVIDER_COMPATIBILITY.md`, `PHASE_5_3_BENCHMARK_PROTOCOL.md`, `PHASE_5_3_CORRECTNESS_RESULTS.md`, `PHASE_5_3_FINAL_ACCEPTANCE.md`.
    - Phase 5.3 benchmark run logs and summaries in `benchmarks/matched-pair-harness/reports/runs/`.
    - Historical amendment drafts and milestone records in `benchmarks/` and `amendments/`.
- **Commit / Push Status:** All Phase 5.3 artifacts remain uncommitted in the local tree, strictly adhering to Non-Negotiable Rule 7 (no commit/push without explicit user approval).

---

## 3. Release Candidate Binary Provenance

All release candidate binaries have been independently hashed and verified using SHA-256 against their registered seal manifests:

| Candidate ID | Exact File Path | File Size (Bytes) | SHA-256 Checksum (Upper) | Status |
| :--- | :--- | :---: | :--- | :---: |
| **Candidate 1** | `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` | 18,285,568 | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | **VERIFIED (INTACT)** |
| **Candidate 2** | `benchmarks/matched-pair-harness/bin/candidate-phase2/engram.exe` | 18,667,008 | `A58B57D343502F2F4E9C328F20841527D573259FC9B305D5F006671427FD99DE` | **VERIFIED (INTACT)** |
| **Candidate 3** | `benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe` | 18,668,032 | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | **VERIFIED (INTACT)** |
| **Target Build** | `target/release/engram.exe` | 18,668,032 | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | **IDENTICAL TO C3** |

### Environment & Toolchain Versions
- **Host Operating System:** Microsoft Windows 11 Pro (x86_64)
- **Codex CLI Executable:** `C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe`
- **Codex CLI Version:** `codex-cli 0.167.0-alpha.4.1`
- **Rust Toolchain:** `rustc 1.91.1 (pre-release)` / `cargo 1.91.1`
- **Python Runtime:** Python 3.14.7 (`pytest 9.1.1`, `pluggy 1.6.0`, `anyio 4.15.1`)
- **Golden Test Repository:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` at commit `1c285608` (clean working tree).

---

## 4. Phase 5.3 Evidence Inventory & Preservation

All underlying telemetry, logs, patches, and summary records from Phase 5.3 have been inspected and verified intact on disk:

| Artifact Description | Disk Location | Size (Bytes) | Integrity Status |
| :--- | :--- | :---: | :---: |
| **Task A Matched Pair Summary** | `benchmarks/matched-pair-harness/reports/runs/task_a_typo_live_pair_01/run_20261119_094618/matched_pair_summary.json` | 10,164 | **VERIFIED** |
| **Task A Treatment B Events Stream** | `benchmarks/matched-pair-harness/reports/runs/task_a_typo_live_pair_01/run_20261119_094618/treatment_B/events.jsonl` | 47,311 | **VERIFIED** |
| **Task A Treatment B Patch** | `benchmarks/matched-pair-harness/reports/runs/task_a_typo_live_pair_01/run_20261119_094618/treatment_B/changes.patch` | 465 | **VERIFIED** |
| **Task A Control A Events Stream** | `benchmarks/matched-pair-harness/reports/runs/task_a_typo_live_pair_01/run_20261119_094618/control_A/events.jsonl` | 51,353 | **VERIFIED** |
| **Task A Control A Patch** | `benchmarks/matched-pair-harness/reports/runs/task_a_typo_live_pair_01/run_20261119_094618/control_A/changes.patch` | 465 | **VERIFIED** |
| **Task B Matched Pair Summary** | `benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_live_pair_01/matched_pair_summary.json` | 19,752 | **VERIFIED** |
| **Task B Treatment B Events Stream** | `benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_live_pair_01/treatment_B/events.jsonl` | 1,438,005 | **VERIFIED** |
| **Task B Control A Single Summary** | `benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_live_single_ctrl/single_summary.json` | 4,496 | **VERIFIED** |
| **Task B Control A Events Stream** | `benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_live_single_ctrl/control_A/events.jsonl` | 138,453 | **VERIFIED** |
| **Phase 5.3 Initial Audit** | `PHASE_5_3_INITIAL_AUDIT.md` | 7,595 | **VERIFIED** |
| **Phase 5.3 Provider Compatibility** | `PHASE_5_3_PROVIDER_COMPATIBILITY.md` | 5,775 | **VERIFIED** |
| **Phase 5.3 Benchmark Protocol** | `PHASE_5_3_BENCHMARK_PROTOCOL.md` | 5,871 | **VERIFIED** |
| **Phase 5.3 Correctness Results** | `PHASE_5_3_CORRECTNESS_RESULTS.md` | 10,254 | **VERIFIED** |
| **Phase 5.3 Final Acceptance Report** | `PHASE_5_3_FINAL_ACCEPTANCE.md` | 11,712 | **VERIFIED** |

---

## 5. Audit Classifications

### 5.1 Verified Facts
1. **Candidate 3 Invariance:** `candidate-phase3/engram.exe` and `target/release/engram.exe` match `97C5EC47...` exactly.
2. **Task A Correctness Parity:** Both Control A and Treatment B scored $1.00 / 1.00$, passing L0 typecheck, L3 production build, REQ-1 (exact typo fix), and REQ-2 (zero blast radius).
3. **Task A Peak Context Reduction:** Peak single-request context in Treatment B was lower by 4.85% (27,897 tokens vs 29,318 tokens).
4. **Task A Uncached Prompt Reduction:** Treatment B required 3.55% fewer uncached prompt tokens (29,069 vs 30,139 tokens).
5. **Task A Cumulative Token Regression:** Treatment B consumed 172,512 total model tokens vs Control A's 151,039 total model tokens (-14.22% regression).
6. **Task B Token Ceiling Breach:** Treatment B consumed 250,102 tokens across 7 requests, breaching the 250,000 ceiling. Control A consumed 223,884 tokens and halted via pre-dispatch headroom exhaustion ($26{,}116 < 45{,}000$ reserve).
7. **Task B MCP Invocations:** Treatment B made 3 MCP calls to `engram_get_context`, receiving 199,126 bytes of auxiliary payload.

### 5.2 Reported but Unverified Claims
1. **Source of Protocol Overhead:** Phase 5.3 hypothesized that MCP tool schema injection (~3,500 tokens/turn $\times 7 = 24.5\text{k tokens}$) was the primary driver of Task A's regression. This is an inference based on prompt length, not an experimentally isolated measurement.
2. **Payload Redundancy in Task B:** The 199 KB payload has not yet been audited for duplicate symbols, excessive AST depth, or transitive dependency bloat.

### 5.3 Contradictions & Discrepancies
1. **Test Suite Status in Phase 5.3 Activity Log vs Report:** The Phase 5.3 log recorded an initial `cargo test` exit code 1 (3 failures in `query_router_test`), while the final report stated 399/399 tests passing.
   - *Investigation:* The 3 failures were `database is locked` errors caused by running parallel test threads (`cargo test` default) against SQLite in `query_router_test`. Running with `--test-threads=1` passed 15/15 tests cleanly in 128s. This environmental/concurrency characteristic must be permanently addressed or documented.
2. **Token Ceiling Metric Semantics:** Comparing cumulative token expenditure across 7 turns ($250{,}102$) against single-request context limits ($48{,}458$) introduces ambiguity unless strictly segregated.

---

## 6. Stage 0 Audit Verdict

- **Stage 0 Status:** **PASSED**
- **Evidence Intact:** Yes (All files present, hashes verified).
- **Next Permitted Action:** Proceed to **Stage 1 — Protocol Overhead Investigation**.
