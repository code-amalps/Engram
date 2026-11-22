# Engram Milestone 5G — Final Remediation Acceptance Audit Report

**Date:** 2026-11-04  
**Auditor:** Independent Systems & Benchmark Auditor  
**Audit Target:** Milestone 5G Readiness Remediation (`amendments/draft-05/milestone-5g/`)  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Git Branch:** `feature/async-precommit-guard`  
**Git HEAD:** `c70845b4d5913457f1b176fb0370b80dd792711f`  
**Live Inference Conducted:** None (0 tokens consumed, 0 API requests, 0 agent launches)  

---

## 1. Executive Summary & Audit Recommendation

A strictly read-only final verification was conducted to inspect whether all four critical blockers identified during the initial Milestone 5G readiness audit have been resolved in the documentation and offline verification evidence.

### Final Audit Determination
$$\mathbf{ACCEPT\ 5G\ WITH\ QUALIFICATIONS}$$

All four blockers—candidate binary path errors, task contract contradictions, repository state misrepresentations, and launch command parameter omissions—have been thoroughly resolved and corroborated by direct codebase inspection. The readiness package is technically coherent, safe, and ready for explicit repository owner review.

> [!IMPORTANT]
> **Acceptance Scope Boundary:**  
> This acceptance confirms strictly that the offline documentation, checksum manifests, and harness safety controls are sound and verified. **It does NOT authorize live model execution or agent dispatch.** Only the repository owner can issue fresh authorization for live execution.

---

## 2. Item-by-Item Verification Findings

### Item 1: Candidate Binary Path, Existence, and SHA-256
- **Verdict:** **PASS**
- **Verified Path:** `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`
- **File Existence:** Confirmed present on disk.
- **SHA-256 Digest:**
  ```
  F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE
  ```
- **Documentation Alignment:** Accurately cited in `pilot-protocol.md` (line 64), `authorization-checklist.md` (line 30), `readiness-assessment.md` (line 52), and `remediation-report.md` (line 46).

### Item 2: Hash Equality with `target/release/engram.exe`
- **Verdict:** **PASS**
- **Verification Evidence:**
  Direct recalculation of both binary files confirmed identical cryptographic hashes:
  - `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`: `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`
  - `target/release/engram.exe`: `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`
- **Significance:** In `orchestrator.py` (lines 47, 129), `Orchestrator` binds `DEFAULT_ENGRAM_BIN` (`target/release/engram.exe`) as the treatment executable and validates it against `--expected-treatment-sha256`. Exact hash equality guarantees that the orchestrator executes the certified Candidate Phase 1 binary.

### Item 3: Task Contract, Evaluator, Target Files, and Validation Alignment
- **Verdict:** **PASS**
- **Verification Evidence:**
  [pilot-protocol.md](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5g/pilot-protocol.md#L35-L60) has been fully rewritten to reflect the Cargo/Rust task:
  - *Task ID:* `task_commit_cache_invalidation`
  - *Target Files:* `src/compiler.rs`, `src/analyzer.rs`, `src/main.rs`, and `tests/commit_cache_invalidation_test.rs`.
  - *Forbidden Patterns:* `Cargo.toml`, `Cargo.lock`, `src/mcp.rs`, `src/guard.rs`, `src/ci_guard.rs`, `src/hook.rs`, `.engram/memories.json`.
  - *Validation Commands:* `cargo check --offline` and `cargo test --offline --lib compiler::tests && cargo test --offline --test commit_cache_invalidation_test`.
  - *Evaluator Concordance:* Matches [evaluator.py](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/evaluator.py#L462-L470) requirements:
    - `REQ-1`: SQLite `.engram/index.db` metadata table `git_commit_head` persistence (`_check_commit_cache_req1_meta_persistence`).
    - `REQ-2`: Commit drift detection in `ensure_fresh()` and integration tests (`_check_commit_cache_req2_drift_detection`).
    - `REQ-3`: Safe cache purge via `engram clean --cache` preserving `memories.json` (`_check_commit_cache_req3_clean_subcommand`).
    - `REQ-4`: Boundary adherence and cargo test gates.
  - *Semantic Retrieval Realism:* Protocol explicitly notes that the evaluator checks functional code output rather than proving semantic retrieval occurred, and that true context replacement is measured via telemetry tracking of subsequent shell reads.

### Item 4: Treatment & Control Worktree Isolation and Cleanliness
- **Verdict:** **PASS**
- **Verification Evidence:**
  1. *Baseline Origin:* Both arms originate from `task_config["baseline_commit"]` (`c70845b4d5913457f1b176fb0370b80dd792711f`) via `create_sandbox` in `isolation.py` (lines 431–439).
  2. *Distinct Sandboxes:* Sandbox directory names incorporate `{arm}` and a timestamp:
     - Treatment: `%TEMP%\engram_pilot\runs\task_commit_cache_invalidation_pilot_02_treatment_B_<timestamp>\`
     - Control: `%TEMP%\engram_pilot\runs\task_commit_cache_invalidation_pilot_02_control_A_<timestamp>\`
  3. *Evaluator Target:* In `orchestrator.py` (lines 1044, 1104, 1109), `self.evaluator.analyze_git_impact`, `self.evaluator.run_automated_gates`, and `self.evaluator.evaluate_requirements` all execute against `sandbox_path`, never the primary working tree.
  4. *Agent Sandbox Bounds:* `runner.py` executes Codex subprocesses with `cwd=sandbox_path`.
  5. *Zero Contamination Mechanism:* Sandboxes are created as detached Git worktrees (`git worktree add --detach`) directly from the Git object database at the commit hash. After each arm completes, `teardown_sandbox` invokes `git worktree remove --force <sandbox_path>` and wipes the directory before post-run verification.

### Item 5: Launch Command CLI Argument Conformance
- **Verdict:** **PASS**
- **Verification Evidence:**
  Cross-referencing the launch command in [authorization-checklist.md](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5g/authorization-checklist.md#L61-L71) against `orchestrator.py` `main()` (lines 1605–1627):
  - `--mode pair`: Supported (line 1607).
  - `--task task_commit_cache_invalidation`: Supported (line 1608).
  - `--pair pilot_02`: Supported (line 1609).
  - `--expected-treatment-sha256 "F56347EF...":` Supported (line 1618).
  - `--model "gpt-5.6-luna":` Supported (line 1612).
  - `--reasoning "medium":` Supported (line 1613).
  - `--max-tokens-ceiling 175000`: Supported (line 1622).
  - `--headroom-reserve 15000`: Supported (line 1623).
  - `--dependency-strategy "none"`: Supported (line 1615).
  - `--output-dir "benchmarks/matched-pair-harness/results/pilot_02"`: Supported (line 1625).
  All 10 options conform to `argparse` definitions. Unsupported flags (`--treatment-bin`, `--max-tokens`, `--timeout`) have been eliminated.

### Item 6: Headroom Reserve (15,000) and 160k Pre-Dispatch Threshold
- **Verdict:** **PASS**
- **Verification Evidence:**
  Tracing parameter flow in the codebase:
  1. CLI `--headroom-reserve 15000` sets `args.headroom_reserve = 15000` (`orchestrator.py` line 1678).
  2. `Orchestrator` passes `self.headroom_reserve` to `CodexRunner` (line 229).
  3. `CodexRunner` instantiates `StreamingTokenTracker(conservative_reserve=15000)` (`runner.py` line 830).
  4. In `StreamingTokenTracker.check_pre_dispatch_headroom(ceiling)` (lines 411–428):
     $$\text{remaining} = 175,000 - \text{cumulative\_tokens}$$
     $$\text{estimated\_next} = \max(I_{\text{last}}, 15,000)$$
     $$\text{has\_headroom} = (\text{remaining} \ge \text{estimated\_next})$$
  5. For any turn where $I_{\text{last}} \le 15,000$ (or initial turn), dispatch is permitted if $\text{remaining} \ge 15,000 \iff \text{cumulative\_tokens} \le 160,000$. If cumulative tokens reach 160,001, dispatch is blocked with `PRE_DISPATCH_GUARD_EXCEEDED`. The documented 160k threshold is verified.

### Item 7: Model Identifier Inconsistency Resolution
- **Verdict:** **PASS (WITH NOT VERIFIABLE QUALIFICATION)**
- **Verification Evidence:**
  - *Protocol:* `pilot-protocol.md` (line 115) records `gpt-5-codex (Runtime alias gpt-5.6-luna; offline status: NOT VERIFIABLE)`.
  - *Checklist:* `authorization-checklist.md` (line 32) documents `gpt-5-codex via codex exec | Runtime alias gpt-5.6-luna. Marked NOT VERIFIABLE offline; live exposure observed at Checkpoint 1`.
  - *Readiness Assessment:* `readiness-assessment.md` (line 118) details the distinction.
  - *Launch Command:* Uses the concrete CLI argument `--model "gpt-5.6-luna"`, matching the harness default (`orchestrator.py` line 1612).
  - *Status:* Acceptance is granted with the explicit qualification that remote API validity of the model identifier cannot be checked offline without triggering live network calls.

### Item 8: Cryptographic Verification of SHA256SUMS Entries
- **Verdict:** **PASS**
- **Verification Evidence:**
  All 7 files listed in `amendments/draft-05/milestone-5g/SHA256SUMS` were independently hashed on disk:

| File Name | Manifest SHA-256 Hash | Disk Recalculated Hash | Match |
| :--- | :--- | :--- | :---: |
| `authorization-checklist.md` | `da8382ed49f12ae8cf4aed7d726110384c70cf8f533b5b1e43a43a84e8676f09` | `da8382ed49f12ae8cf4aed7d726110384c70cf8f533b5b1e43a43a84e8676f09` | **TRUE** |
| `independent-readiness-audit.md` | `9ff9acbcd3c5675b40e5427f49117b3918770dbcd956c527b3f2443b81a8fa21` | `9ff9acbcd3c5675b40e5427f49117b3918770dbcd956c527b3f2443b81a8fa21` | **TRUE** |
| `pilot-protocol.md` | `26893f7da8815d9b55d54b58300e3ecc811c61ba2851bb6b41c6a34ddb6d0c67` | `26893f7da8815d9b55d54b58300e3ecc811c61ba2851bb6b41c6a34ddb6d0c67` | **TRUE** |
| `readiness-assessment.md` | `7f19564391b4cc940142b0be47d6efa2d08b678f5b4ba4741701e0c3a711c9a4` | `7f19564391b4cc940142b0be47d6efa2d08b678f5b4ba4741701e0c3a711c9a4` | **TRUE** |
| `remediation-report.md` | `b8e05dfafaa43ed268c6a90161c5c2f822cf14b19ab2a25e42cec3a779519704` | `b8e05dfafaa43ed268c6a90161c5c2f822cf14b19ab2a25e42cec3a779519704` | **TRUE** |
| `runtime-observation-plan.md` | `e3c3757ce63e9d90e44e51c444e3fe90dc53be5b35f16102a0346f9b3f774721` | `e3c3757ce63e9d90e44e51c444e3fe90dc53be5b35f16102a0346f9b3f774721` | **TRUE** |
| `token-accounting-and-stop-conditions.md` | `eba7fa9691f1b4fec13af86c9cdee4235046e868be25ea48ce5b748db5c23553` | `eba7fa9691f1b4fec13af86c9cdee4235046e868be25ea48ce5b748db5c23553` | **TRUE** |

### Item 9: Milestone 5F Regression Test Audit
- **Verdict:** **PASS**
- **Investigation of Failed vs. Successful Invocations:**
  - *Failed Attempt:* Invoked via `pytest "benchmarks/matched-pair-harness/tests/test_milestone_5f_remediation.py"`. The terminal session had defaulted to `C:\Users\Amal P S\OneDrive\Projects\AMstraLift`. Because the relative path was invalid from `AMstraLift`, pytest exited with code `1` (`ERROR: file or directory not found`).
  - *Successful Invocations:* When invoked with the absolute path `pytest "C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness\tests\test_milestone_5f_remediation.py"`, all 7 tests passed (exit code `0`):
    - First run: 7 passed in 1.20s.
    - Audit re-execution: 7 passed in 0.61s.
  - *Subsystems Verified:* Guard variable attribution (41k turn input vs 115k thread), pre-dispatch headroom halting, sandbox `.codex/config.toml` creation, Turn-0 injection constants, skipped-arm null serialization, and replacement event parsing.

### Item 10: Git Branch, HEAD, and Repository Tree State Verification
- **Verdict:** **PASS**
- **Verification Evidence:**
  1. *Engram Repository:*
     - Branch: `feature/async-precommit-guard`
     - HEAD Commit: `c70845b4d5913457f1b176fb0370b80dd792711f`
     - Tree State: Contains uncommitted working files preserved untouched.
  2. *Golden Angular Repository:*
     - Path: `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`
     - Commit: `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`
     - Tree State: Exactly 18 modified files and untracked `.engram/` preserved untouched.
  3. *Invariance Guarantee:* Zero commits, resets, cleanups, stashes, reverts, or branch switches were performed across either repository.

---

## 3. Preserved Qualifications Summary

The following three operational qualifications remain active and must accompany any authorization request presented to the repository owner:

1. **Remote Model Identifier (`gpt-5-codex` / `gpt-5.6-luna`):**  
   Cannot be validated offline without initiating external network requests to OpenAI's backend. Observed at Checkpoint 1 upon authorized execution.
2. **Runtime MCP Exposure:**  
   Offline generation and schema syntax of `.codex/config.toml` in the sandbox is verified. Whether `codex.exe` successfully spawns `engram.exe serve --stdio` and discovers MCP tools must be observed at Checkpoints 2 and 3.
3. **Turn-0 Instruction Duplication Overhead:**  
   Coexistence of `.github/copilot-instructions.md` and prepended prompt context may create minor token inflation at Turn 1, to be monitored at Checkpoint 5.

---

## 4. Final Recommendation

All four blockers identified in the initial readiness audit are **remediated, verified, and sealed**. 

The Milestone 5G Live Pilot Readiness Package is formally **ACCEPTED WITH QUALIFICATIONS**.

```
========================================================================
FINAL AUDIT DECISION: ACCEPT 5G WITH QUALIFICATIONS
========================================================================
All offline readiness criteria, task contracts, binary bindings,
worktree isolation controls, and launch commands are fully verified.
Execution remains strictly blocked pending explicit owner authorization.
========================================================================
```

### Safety Affirmation
**Confirmed explicitly:** Zero live model API requests, zero agent launches, zero network requests, and zero tokens were consumed during this final acceptance audit. All findings are derived exclusively from offline static inspection and local unit test executions.
