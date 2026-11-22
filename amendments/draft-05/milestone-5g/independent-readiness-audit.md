# Engram Milestone 5G — Independent Readiness Audit Report

**Date:** 2026-11-04  
**Auditor:** Independent Systems & Benchmark Auditor  
**Audit Target:** `amendments/draft-05/milestone-5g/`  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Git Branch:** `feature/async-precommit-guard`  
**Git HEAD:** `c70845b4d5913457f1b176fb0370b80dd792711f`  
**Live Inference Conducted:** None (0 tokens consumed, 0 API requests)  

---

## 1. Executive Summary & Audit Recommendation

An exhaustive, strictly read-only audit of the Milestone 5G Live Pilot Readiness Package was conducted. The audit inspected all five planning documents, evaluated harness and evaluator implementations, recalculated cryptographic checksums, verified frozen binary integrity, inspected Git repository trees, and investigated command execution behavior.

### Final Audit Recommendation
$$\mathbf{NOT\ READY\ —\ BLOCKERS\ REMAIN}$$

While the harness code, knowledge engine optimizations, and mathematical token accounting in `runner.py` and `telemetry.py` demonstrate high technical rigor, the readiness package contains **four critical blockers and inconsistencies** that must be remediated before the repository owner can safely review and authorize a live pilot.

---

## 2. Summary of Identified Blockers & Qualifications

### 2.1 Critical Blockers (Must Be Resolved Prior to Authorization)
1. **Candidate Binary Path Error in Launch Command & Manifests:**  
   `authorization-checklist.md` (line 67) and `readiness-assessment.md` specify the candidate binary path as `benchmarks/matched-pair-harness/bin/engram-candidate-phase1.exe`. No such file exists. The actual certified binary is at `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`. Launching with the documented command will fail immediately with `FileNotFoundError`.
2. **Task Contract & Target Repository Contradiction in Protocol:**  
   `pilot-protocol.md` (lines 38–51) describes the task as an Angular application task targeting `angular-ngrx-material-starter` and editing `src/app/core/meta-reducers/init-state-from-local-storage.reducer.ts` with `npm test`. In direct contradiction, the actual task definition (`benchmarks/matched-pair-harness/config/tasks/task_commit_cache_invalidation.json`) and evaluator (`evaluator.py` lines 462, 621–680) configure a Cargo/Rust task targeting the Engram codebase (`src/compiler.rs`, `src/analyzer.rs`, `src/main.rs`) and executing `cargo check --offline` and `cargo test --offline`. Authorizing the pilot as currently described would execute a completely different codebase and test suite than documented in the protocol.
3. **Dirty Golden Repository vs. Erroneous Cleanliness Claim:**  
   `readiness-assessment.md` (Section 2.2) claims the golden repository at `Projects\Test\angular-ngrx-material-starter` has a clean tree ("zero uncommitted changes"). Inspection via `git status -s` revealed **18 modified files and an untracked `.engram/` directory**. Furthermore, for a Cargo task, `orchestrator.py` selects the Engram repository itself (`HARNESS_ROOT.parent.parent`), which also contains active working-tree modifications.
4. **Pre-Dispatch Headroom Reserve CLI Parameter Omission:**  
   The protocol and token accounting documents specify a 15,000 reserve yielding a 160,000 pre-dispatch threshold against a 175,000 ceiling ($175\text{k} - 15\text{k} = 160\text{k}$). However, `orchestrator.py` (line 1623) defaults `--headroom-reserve` to `45_000`. The launch command in `authorization-checklist.md` omits `--headroom-reserve 15000`, which will cause the orchestrator to trip at $175,000 - 45,000 = 130,000$ tokens, terminating the run prematurely.

### 2.2 Operational Qualifications (Owner Review Required)
1. **Model Identifier Unverifiable Offline:** `gpt-5-codex` is specified in documentation, but the harness defaults to `gpt-5.6-luna`. Whether `gpt-5-codex` is a valid, accepted runtime identifier for `codex exec` cannot be verified without live API/inference.
2. **Runtime MCP Process Exposure:** Generating `.codex/config.toml` in the sandbox is verified offline; whether `codex exec` actually spawns the MCP process and discovers tools remains an unverified runtime boundary.
3. **Turn-0 Instruction Duplication:** Simultaneous presence of `.github/copilot-instructions.md` and prompt prefix injection could inflate Turn-0 token usage.

---

## 3. Detailed Audit by Criterion

### Criterion 1: Milestone 5F Acceptance Recording
- **Verdict:** **PASS**
- **Evidence:**  
  [milestone-5f-closure-report.md](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5f/milestone-5f-closure-report.md) lines 6–10 and 27–54 accurately record the status as `ACCEPTED WITH QUALIFICATIONS`, cite the independent audit artifact `amendments/draft-05/milestone-5f/independent-acceptance-audit.md`, document the 14 audit criteria and verdicts, and record the three explicit qualifications verbatim.

### Criterion 2: Consistency Across Milestone 5G Documents
- **Verdict:** **FAIL**
- **Evidence:**  
  - *Task Scope Contradiction:* [pilot-protocol.md](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5g/pilot-protocol.md#L38-L51) describes an Angular task modifying `src/app/core/meta-reducers/init-state-from-local-storage.reducer.ts` tested via `npm test`. Conversely, [readiness-assessment.md](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5g/readiness-assessment.md) and [task_commit_cache_invalidation.json](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/config/tasks/task_commit_cache_invalidation.json) define a Cargo task modifying `src/compiler.rs` and `src/analyzer.rs` tested via `cargo test --offline`.
  - *Candidate Binary Path Discrepancy:* `pilot-protocol.md` (line 64), `authorization-checklist.md` (lines 30, 67), and `readiness-assessment.md` (line 52) list `bin/engram-candidate-phase1.exe`. The actual file on disk is `bin/candidate-phase1/engram.exe`.
  - *Launch Command Inconsistency:* `authorization-checklist.md` (line 67) omits `--headroom-reserve 15000`, contradicting the 160k threshold claim in `token-accounting-and-stop-conditions.md`.

### Criterion 3: Task & Evaluator Contract Inspection
- **Verdict:** **PARTIAL**
- **Evidence:**  
  Inspection of [task_commit_cache_invalidation.json](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/config/tasks/task_commit_cache_invalidation.json) and [evaluator.py](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/evaluator.py#L462-L466) confirms that the implementation genuinely tests semantic retrieval and code modification:
  - `REQ-1`: SQLite `.engram/index.db` metadata persistence (`_check_commit_cache_req1_meta_persistence`).
  - `REQ-2`: Commit drift detection triggering invalidation in `ensure_fresh` (`_check_commit_cache_req2_drift_detection`).
  - `REQ-3`: `engram clean --cache` subcommand implementation (`_check_commit_cache_req3_clean_subcommand`).
  - `REQ-4`: Diff blast-radius containment and cargo test gates.  
  Criteria are objective, programmatic, and static/test-backed. However, this is rated **PARTIAL** because `pilot-protocol.md` misrepresents the entire task as Angular.

### Criterion 4: Sequential Matched-Pair Design & Conditional Control Policy
- **Verdict:** **PASS**
- **Evidence:**  
  [pilot-protocol.md](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5g/pilot-protocol.md#L60-L88) clearly distinguishes functional validation ($C = 1.0$) from comparative efficiency measurement ($\text{Tokens}_B \text{ vs } \text{Tokens}_A$). It correctly details that skipping `control_A` when `treatment_B` fails prevents quota waste while acknowledging that a net savings ratio can only be established when both arms achieve $C = 1.0$.

### Criterion 5: Model & Runtime Identifier (`gpt-5-codex`)
- **Verdict:** **NOT VERIFIABLE**
- **Evidence:**  
  In the harness source code (`orchestrator.py` lines 74, 1612; `runner.py` line 481), the default model string is `gpt-5.6-luna`. `gpt-5-codex` appears solely in markdown documentation and CLI examples. Offline examination cannot determine whether `codex exec --model gpt-5-codex` is an accepted model alias or rejected by OpenAI's backend CLI.

### Criterion 6: Token Ceilings, Thresholds, and Guard Mechanics
- **Verdict:** **PARTIAL**
- **Evidence:**  
  `runner.py` ([StreamingTokenTracker](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L75-L115)) correctly separates single-request input ($I_{\text{last}}$) from cumulative thread tokens, and the dynamic formula $\text{remaining} \ge \max(I_{\text{last}}, \text{reserve})$ is mathematically sound. The document accurately explains that local guards operate between turns and cannot truncate in-flight network streaming. However, claiming an effective 160k threshold is ambiguous because `orchestrator.py` defaults to a 45k reserve (triggering at 130k) unless overridden by `--headroom-reserve 15000`.

### Criterion 7: Runtime Observation Plan
- **Verdict:** **PASS**
- **Evidence:**  
  [runtime-observation-plan.md](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5g/runtime-observation-plan.md) explicitly distinguishes offline synthetic proofs from live runtime requirements across 7 concrete checkpoints. It defines observable criteria from `raw_events.jsonl`, `session_events.jsonl`, process tables, and `run_telemetry.json`.

### Criterion 8: Owner Authorization Checklist
- **Verdict:** **PARTIAL**
- **Evidence:**  
  [authorization-checklist.md](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5g/authorization-checklist.md) prominently displays the notice that prior authorizations are void, defines the 10 inspection criteria, and includes an explicit authorization template. However, it is rated **PARTIAL** because the launch command includes an invalid binary path (`engram-candidate-phase1.exe`) and omits `--headroom-reserve 15000`.

### Criterion 9: 5G Checksum Manifest Verification
- **Verdict:** **PASS**
- **Evidence:**  
  Independent recalculation of all five markdown documents in `amendments/draft-05/milestone-5g/` matched `SHA256SUMS` exactly:
  - `authorization-checklist.md`: `e8c29c55ef83993b83ff60532561f28934534f9263301862be6ab381f0725696`
  - `pilot-protocol.md`: `73f1b77f818a9b0230176a046373c28e722d59b2c98316d9751e6eda94a675df`
  - `readiness-assessment.md`: `391862e9af9b3231287c64b35967a528dfa9eaf00044171963ae43aae511e5ea`
  - `runtime-observation-plan.md`: `bc7eaa26a8f80ca318465745dc2ff24f57924a2c51f8d6fa950e7b9da7cdf95e`
  - `token-accounting-and-stop-conditions.md`: `746b278a54ff15f58b10f6a1b318d5445394964926021d448acb15693d07eef1`

### Criterion 10: Frozen Binary Hash Verification
- **Verdict:** **PASS (WITH PATH QUALIFICATION)**
- **Evidence:**  
  Cryptographic hashes of frozen binaries on disk were verified against sealed records:
  - `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`: `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` (Match)
  - `benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe`: `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` (Match)
  - `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe`: `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` (Match)  
  *Qualification:* The candidate binary is located in subfolder `candidate-phase1/engram.exe`, not `engram-candidate-phase1.exe`.

### Criterion 11: Branch, HEAD, and Golden Repository Integrity
- **Verdict:** **FAIL**
- **Evidence:**  
  - Engram repository branch is verified as `feature/async-precommit-guard` at commit `c70845b4d5913457f1b176fb0370b80dd792711f`.
  - Golden repository at `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` is at commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`, but its working tree is **dirty** with 18 modified files and untracked `.engram/`, directly refuting the cleanliness claim in `readiness-assessment.md`.

### Criterion 12: Investigation of 5F Regression Test Failure
- **Verdict:** **PASS**
- **Evidence & Investigation:**
  1. *First Attempt Command:* `pytest "benchmarks/matched-pair-harness/tests/test_milestone_5f_remediation.py"`
  2. *First Attempt Working Directory:* `C:\Users\Amal P S\OneDrive\Projects\AMstraLift` (the IDE active workspace root).
  3. *Failure Cause & Exit Code:* Exited with code `1` (`ERROR: file or directory not found: benchmarks/matched-pair-harness/tests/test_milestone_5f_remediation.py`). The relative path failed because PowerShell was rooted in `AMstraLift` rather than `Engram`.
  4. *Subsequent Attempt:* `pytest "C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness\tests\test_milestone_5f_remediation.py"`
  5. *Subsequent Exit Code & Output:* Exited with code `0`. All 7 tests collected and passed in 1.20s.
  6. *Post-5G Verification:* Re-executed during this audit; all 7 tests passed cleanly in 0.61s.

### Criterion 13: Audit of Claims Exceeding Available Evidence
- **Verdict:** **FAIL**
- **Evidence:**  
  The following claims in [readiness-assessment.md](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5g/readiness-assessment.md) exceed or conflict with empirical evidence:
  1. *Cleanliness Claim:* Section 2.2 claims `angular-ngrx-material-starter` has a clean tree (`git status -s returns zero uncommitted changes`). Empirical check revealed 18 modified files.
  2. *Golden Repository Identification:* Section 2.2 cites `angular-ngrx-material-starter` as the golden repository, whereas the actual task being executed is a Cargo task that uses the Engram repository as its golden source.
  3. *Candidate Binary Path Claim:* Section 2.3 asserts candidate binary path is `benchmarks/matched-pair-harness/bin/engram-candidate-phase1.exe`, which does not exist.
  4. *Model Verifiability:* Section 2 cites `gpt-5-codex` as verified, whereas it cannot be verified offline.
  5. *Historical Test Suite Count Substitution:* Section 2.4 claims 302 Python tests and 63 Rust tests passed without re-running the full suites during 5G.

---

## 4. Required Remediation Steps to Reach Readiness

Before submitting the readiness package to the repository owner, the following corrections must be performed:

1. **Fix Candidate Binary Path:** Update `pilot-protocol.md`, `authorization-checklist.md`, and `readiness-assessment.md` to reference `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`.
2. **Correct Task Contract in Protocol:** Align `pilot-protocol.md` Sections 2.1 and 2.2 with the actual `task_commit_cache_invalidation.json` specification (Cargo/Rust, SQLite meta table, `ensure_fresh`, and `cargo test`).
3. **Clarify Golden Repository & Cleanliness:** Note that for `task_commit_cache_invalidation`, the source repo is `Engram`, and document the true state of `angular-ngrx-material-starter` without false clean claims.
4. **Update Launch Command Parameters:** Add `--headroom-reserve 15000` to the launch command in `authorization-checklist.md` to enforce the documented 160k pre-dispatch threshold.
5. **Recalculate Checksums:** Re-generate `SHA256SUMS` after updating the documentation.

---

## 5. Safety Declaration

**Confirming explicitly:** No live model inference, Codex agent launches, external network requests, or token consumption occurred during this independent readiness audit. All findings are derived strictly from local static inspection and non-inference test executions.
