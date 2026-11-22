# Engram Amendment 05 — Milestone 2C: Pre-Change Baseline Snapshot

**Document Version:** 1.0.0  
**Timestamp:** 2026-10-30T11:32:00+05:30  
**Status:** Pre-Change Integrity Seal (Offline)

---

## 1. Engram Repository Git State

- **Repository Path:** `C:\Users\Amal P S\OneDrive\Projects\Engram`
- **Active Branch:** `feature/async-precommit-guard`
- **Current Commit HEAD:** `c70845b4d5913457f1b176fb0370b80dd792711f`
- **Working Tree Status (`git status --porcelain`):**
  ```text
   M benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json
   M benchmarks/matched-pair-harness/schemas/correctness_checklist.schema.json
   M benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json
   M benchmarks/matched-pair-harness/src/evaluator.py
   M benchmarks/matched-pair-harness/src/isolation.py
   M benchmarks/matched-pair-harness/src/orchestrator.py
   M benchmarks/matched-pair-harness/src/runner.py
   M benchmarks/matched-pair-harness/src/telemetry.py
   M src/instructions.rs
   M src/main.rs
   M src/mcp.rs
   M tests/mcp_consolidation_test.rs
   M tests/turn0_injection_test.rs
  ?? amendments/
  ?? benchmarks/AMENDMENT_02_STAGE1_CORE_TRIAD.md
  ?? benchmarks/AMENDMENT_03_TASK_B_OPERATIONAL_CEILING_CALIBRATION.md
  ?? benchmarks/AMENDMENT_04_STAGE1_TOKEN_EFFICIENCY_PROTOCOL_REVISION.md
  ?? benchmarks/AMENDMENT_04_STAGE1_TOKEN_EFFICIENCY_PROTOCOL_REVISION_DRAFT.md
  ?? benchmarks/FINAL_OFFLINE_PREFLIGHT_MILESTONES_2_5_REPORT.md
  ?? benchmarks/PHASE_B_COMPLETION_REPORT.md
  ?? benchmarks/STAGE1_BLOCK06_FINAL_SAFETY_CHECK.md
  ?? benchmarks/STAGE1_BLOCK06_HARDENED_RESULTS_REPORT.md
  ?? benchmarks/STAGE1_BLOCK06_LIVE_RESULTS_REPORT.md
  ?? benchmarks/STAGE1_BLOCK06_PREFLIGHT_REPORT.md
  ?? benchmarks/STAGE1_BLOCK06_RESILIENCE_FIX_REPORT.md
  ?? benchmarks/STAGE1_BLOCK06_SAFETY_AUDIT_REPORT.md
  ?? benchmarks/STAGE1_BLOCK07_COST_RISK_ANALYSIS.md
  ?? benchmarks/STAGE1_BLOCK07_FINAL_LIVE_REPORT.md
  ?? benchmarks/STAGE1_BLOCK07_POST_MORTEM_ROOT_CAUSE_INVESTIGATION.md
  ?? benchmarks/STAGE1_BLOCK07_REMEDIATION_MILESTONE_CLOSURE_REPORT.md
  ?? benchmarks/STAGE1_BLOCK08_FINAL_PREFLIGHT_REPORT.md
  ?? benchmarks/STAGE1_BLOCK08_LIVE_RESULTS_REPORT.md
  ?? benchmarks/STAGE1_BLOCK08_PREFLIGHT_REPORT.md
  ?? benchmarks/STAGE1_BLOCK08_PROTOCOL_DECISION_REVIEW.md
  ?? benchmarks/STAGE1_BLOCKS_01_03_AGGREGATE_REPORT.md
  ?? benchmarks/STAGE1_SMOKE_TEST_AUDIT.md
  ?? benchmarks/STAGE1_SMOKE_TEST_REPORT.md
  ?? benchmarks/STAGE1_TASK_A_COMPLETE_5BLOCK_REPORT.md
  ?? benchmarks/STAGE1_TASK_A_FINAL_SCHEDULE_AUDIT.md
  ?? benchmarks/STAGE1_TASK_A_POST_MORTEM_AND_TASK_B_READINESS.md
  ?? benchmarks/STAGE1_TASK_A_READINESS_AND_TASK_B_REMEDIATION.md
  ?? benchmarks/STAGE1_TASK_B_BLOCK06_FORENSIC_ANALYSIS.md
  ?? benchmarks/STAGE1_TASK_B_HARNESS_FIXES_REPORT.md
  ?? benchmarks/STAGE1_VERIFICATION_OFFLINE_REGRESSION_REPORT.md
  ?? benchmarks/STAGE1_VERIFICATION_POLLING_FIX_REPORT.md
  ?? benchmarks/STAGE1_VERIFICATION_RUNNER_SAFETY_HARDENING_REPORT.md
  ?? benchmarks/TOKEN_REDUCTION_VERIFICATION_GATE_REPORT.md
  ?? benchmarks/cost-baseline/
  ?? benchmarks/matched-pair-harness/bin/
  ?? benchmarks/matched-pair-harness/schemas/triad_summary.schema.json
  ?? benchmarks/matched-pair-harness/src/classifier.py
  ?? benchmarks/matched-pair-harness/src/verification.py
  ?? benchmarks/matched-pair-harness/tests/test_amendment_04_two_arm.py
  ?? benchmarks/matched-pair-harness/tests/test_block07_remediation.py
  ?? benchmarks/matched-pair-harness/tests/test_harness_safety_audit.py
  ?? benchmarks/matched-pair-harness/tests/test_resilience_repair.py
  ?? benchmarks/matched-pair-harness/tests/test_task_b_evaluator.py
  ?? benchmarks/matched-pair-harness/tests/test_task_b_readiness_fixes.py
  ?? benchmarks/matched-pair-harness/tests/test_token_efficiency.py
  ?? benchmarks/matched-pair-harness/tests/test_token_guard_validation.py
  ?? benchmarks/matched-pair-harness/tests/test_triad.py
  ?? benchmarks/matched-pair-harness/tests/test_verification_workflow.py
  ?? benchmarks/profile/
  ?? scratch/
  ```

---

## 2. Protected Binary Hashes

| Executable | Path | Expected SHA-256 | Actual SHA-256 | Match |
|:---|:---|:---|:---|:---:|
| **Candidate Phase 1** | `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | **YES** |
| **Frozen v1.5.4** | `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | **YES** |
| **Frozen v1.5.1** | `benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | **YES** |

---

## 3. Protected Historical Evidence Hashes

| Artifact Description | File Path | Authoritative SHA-256 |
|:---|:---|:---|
| Amendment 04 Protocol Specification | `benchmarks/AMENDMENT_04_STAGE1_TOKEN_EFFICIENCY_PROTOCOL_REVISION.md` | `03C4B2786E2F62A6435A7EA81C54E65889619296FD4ED9873546EF20573030E2` |
| Block 07 Final Live Report | `benchmarks/STAGE1_BLOCK07_FINAL_LIVE_REPORT.md` | `325EF0EC3F14155B02F388C584DE9F85B88D04ABA142E87C8DB8FDCACCFD4266` |
| Block 07 Triad Report | `benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/triad_report.md` | `9CBD121CD500EFF13130430DB0F836BB24338C15C523E9EBE9F3209318F32507` |
| Block 07 Triad Summary JSON | `benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/triad_summary.json` | `DAF183FC193DC63B95A623DE303C44334087F54177AC031F4F5ED31D33643785` |
| Milestone 2B Verification | `amendments/draft-05/milestone-2b/final-preflight-verification.md` | `D4B5A48C03378E7DE355539888093455633297DEEF2CDCFC937181B92524EF39` |
| Milestone 2B Readiness Summary | `amendments/draft-05/milestone-2b/authorization-readiness-summary.md` | `4A59E17C4075E848FBA274AD361A397EF01119D9958D4FA5C6E9C681DCC214EE` |

---

## 4. Golden Repository Baseline State

- **Path:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`
- **Branch:** `test-branch`
- **Pinned Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`
- **Tracked Modifications:** Exactly 18 files
- **Diffstat:** `+208 / -16` lines
- **Untracked Directory:** `.engram/` containing exactly 29 files
- **Active Worktrees (`git worktree list`):** Main tree + 4 historical detached worktrees in `AppData/Local/Temp/engram_pilot/runs/`

---

## 5. Current Implementation Baseline of 5 Affected Areas

1. **Task Configuration Store (`config/tasks/`):**  
   Only contains `task_a_typo.json`, `task_b_remember_me.json`, and `task_c_theme_toggle.json`. The proposed task `smoke_test_read_only.json` is missing.
2. **Execution Mode (`orchestrator.py: main`):**  
   Line 1320 defines `--mode choices=["pair", "triad"]`. Line 1342 derives `mode = "triad" if (args.block or args.mode == "triad") else "pair"`. Passing `--arms v154_arm3` without `--mode triad` defaults to `pair` and executes `control_A` and `treatment_B`.
3. **Dependency Strategy Guard (`orchestrator.py:801`):**  
   `_execute_single_arm` asserts:  
   `if not self.is_dry_run and self.dependency_strategy != "copy": raise SafetyViolationError(...)`  
   Live runs cannot use `none` even for tasks that do not need dependencies.
4. **Output Directory Handling (`orchestrator.py`):**  
   `argparse` does not declare `--output-dir`. Default output path is `HARNESS_ROOT / "reports" / "runs" / f"{self.task_id}_{self.pair_id}"`, which risks colliding with and overwriting historical reports (`block_01` .. `block_08`).
5. **Sandbox Agent Permissions (`runner.py:548`):**  
   `CodexRunner.build_codex_command` hardcodes `-s workspace-write`. Local Codex CLI (`codex.exe 0.164.0-alpha.11.1`) was verified to support `[possible values: read-only, workspace-write, danger-full-access]`.

---

## 6. Pre-Change Harness Test Suite Baseline

- **Command:** `pytest benchmarks/matched-pair-harness/tests -q`
- **Test Result:** `228 passed in 102.17s (0:01:42)`
- **Exit Code:** `0`
- **Failures / Errors / Skips:** 0 / 0 / 0

---
*Baseline snapshot recorded and verified before any code modification.*
