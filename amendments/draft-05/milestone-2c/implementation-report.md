# Engram Amendment 05 — Milestone 2C: Harness Remediation & Implementation Report

**Document Version:** 1.0.0  
**Date:** 2026-10-30  
**Status:** Completed Offline Remediation & Verification  
**Classification:** `REMEDIATION COMPLETE — READY FOR FINAL OFFLINE AUDIT`

---

## 1. Executive Summary

Milestone 2C resolved all five critical execution blockers identified during the Milestone 2B preflight audit. The test harness (`benchmarks/matched-pair-harness`) has been remediated so that the proposed single-arm, read-only smoke test can be authorized and executed deterministically, non-destructively, and under strict read-only security boundaries.

All remediation activities were conducted **strictly offline**. Exactly 0 live model inference calls, 0 API requests, 0 network requests, and 0 credits/spend ($0.00) occurred. No golden repository modifications or Git worktree mutations occurred.

---

## 2. Initial Blockers and Remediation Summary

| Blocker ID | Description | Root Cause in Code | Milestone 2C Resolution |
|:---|:---|:---|:---|
| **Blocker 1** | Missing task configuration file | `smoke_test_read_only.json` did not exist on disk | Authored `config/tasks/smoke_test_read_only.json`, verified against schema, pinned to commit `1c285608...` and symbol `authLogout`. |
| **Blocker 2** | Disconnect for single-arm execution | `orchestrator.py` CLI only had `--mode {pair, triad}`; defaulted to `pair` and ignored `--arms v154_arm3` | Implemented `--mode single` with strict validation (exactly 1 valid arm required; 0 or >1 rejected; dedicated `run_single` workflow). |
| **Blocker 3** | Live run rejected `dependency_strategy="none"` | `orchestrator.py:801` unconditionally rejected non-copy modes on live runs | Remediated guard: `dependency_strategy="none"` is accepted for read-only / zero-dependency tasks; strictly rejected for compile tasks. |
| **Blocker 4** | Missing `--output-dir` in CLI parser & risk of evidence overwrite | `argparse` omitted `--output-dir`; default path could overwrite historical blocks | Added `--output-dir` to `argparse`; added path traversal guards and automatic unique timestamped subdirectory generation on evidence collision. |
| **Blocker 5** | Hardcoded `-s workspace-write` agent permissions | `runner.py:548` hardcoded write access | Verified local Codex CLI support for `read-only`; added configurable `sandbox_policy` (`-s read-only`, omitting `--add-dir`); added post-run mutation defense-in-depth. |

---

## 3. Files Modified and Justification

| File Path | Nature of Change | Justification |
|:---|:---|:---|
| `benchmarks/matched-pair-harness/config/tasks/smoke_test_read_only.json` | **NEW FILE** | Created dedicated read-only smoke-test task configuration targeting `authLogout`. |
| `benchmarks/matched-pair-harness/schemas/correctness_checklist.schema.json` | **MODIFIED** | Added `read_only` and `requires_dependencies` boolean properties to schema. |
| `benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json` | **MODIFIED** | Added `sandbox_policy` string property to `isolation` in schema. |
| `benchmarks/matched-pair-harness/src/orchestrator.py` | **MODIFIED** | Implemented `--mode single`, `run_single()`, `--output-dir`, safe dependency strategy check, and read-only mutation defense-in-depth. |
| `benchmarks/matched-pair-harness/src/runner.py` | **MODIFIED** | Implemented `sandbox_policy` parameter, supported policies validation, and read-only CLI flags (`-s read-only`, omitting `--add-dir`). |
| `benchmarks/matched-pair-harness/tests/test_milestone_2c_remediation.py` | **NEW FILE** | 21 focused offline unit tests covering all 5 remediations, binary integrity, and token guard thresholds. |

---

## 4. Technical Remediation Details

### 4.1 Remediation A: Read-Only Smoke-Test Task Configuration
- **File:** `benchmarks/matched-pair-harness/config/tasks/smoke_test_read_only.json`
- **Design Invariants:**
  1. `task_id`: `"smoke_test_read_only"`.
  2. `read_only`: `true`.
  3. `requires_dependencies`: `false`.
  4. `baseline_commit`: `"1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2"`.
  5. `allowed_file_patterns`: `[]` (no file writes permitted).
  6. `forbidden_file_patterns`: `["**/*"]` (entire repository is protected).
  7. `gates`: All 4 compilation, lint, test, and build gates disabled (`enabled: false`).
  8. `requirements`:
     - `REQ-1`: Identify exact relative path and symbol name (`export const authLogout = createAction('[Auth] Logout');`).
     - `REQ-2`: Zero workspace mutation (strict read-only boundary).
  9. Validated 100% compliant with `correctness_checklist.schema.json`.

### 4.2 Remediation B: Explicit Single-Arm Execution Mode
- **CLI Interface:** `--mode single --arms <arm_id>`
- **Validation Rules:**
  1. If `--mode single` is provided, exactly one arm must be supplied via `--arms`.
  2. Zero arms (`--arms ""` or omitted) fails closed immediately with exit code 1.
  3. Multiple arms (`--arms v154_arm3,baseline_arm1`) fails closed immediately with exit code 1.
  4. Invalid arm names fail closed immediately with exit code 1.
- **Execution Workflow (`Orchestrator.run_single`):**
  1. Invokes only the requested arm via `_execute_single_arm(arm, exec_order=1)`.
  2. Companion arms (`baseline_arm1`, `v151_arm2`, `control_A`) are never provisioned or executed.
  3. Writes `single_summary.json` and `single_report.md` to `self.output_dir`.
  4. Existing pair and triad workflows remain completely intact.

### 4.3 Remediation C: Safe Dependency Strategy
- **Implementation in `_execute_single_arm`:**
  ```python
  task_requires_deps = self.task_config.get("requires_dependencies", True)
  is_read_only_task = self.task_config.get("read_only", False)

  if not self.is_dry_run:
      if self.dependency_strategy == "none":
          if task_requires_deps and not is_read_only_task:
              raise SafetyViolationError(
                  f"FATAL: Task '{self.task_id}' requires dependencies (requires_dependencies=True). "
                  f"Cannot use dependency_strategy='none' on live benchmark runs for tasks requiring compilation or testing."
              )
      elif self.dependency_strategy != "copy":
          raise SafetyViolationError(
              f"FATAL: Live benchmark runs must use dependency_strategy='copy' (or 'none' for read-only tasks). "
              f"Attempted to run live arm with '{self.dependency_strategy}'."
          )
  ```
- **Guarantees:**
  - Tasks marked `read_only=True` or `requires_dependencies=False` can safely run live with `--dependency-strategy none`.
  - Non-read-only tasks (e.g. `task_b_remember_me`, `task_a_typo`) strictly reject `none` on live runs.
  - Zero packages installed, zero `node_modules` copied or linked, zero NTFS junctions created.

### 4.4 Remediation D: Safe Output Directory Handling
- **CLI Parameter:** Added `--output-dir` to `argparse`.
- **Directory Protection:**
  1. Rejects output directories resolving to protected roots (repo root, `src`, golden repository).
  2. If the target directory already contains run evidence (`triad_summary.json`, `run_telemetry.json`, `single_summary.json`, etc.), the orchestrator automatically provisions a unique timestamped subdirectory:  
     `output_dir / "run_YYYYMMDD_HHMMSS"`
  3. Prevents overwriting historical benchmark evidence (`block_01` through `block_08`).

### 4.5 Remediation E: Enforce Read-Only Agent Permissions
- **Verified Local Codex Capability:**  
  Locally queried `codex.exe exec --help` (version `0.164.0-alpha.11.1`). Confirmed exact possible values for `-s, --sandbox`:  
  `[possible values: read-only, workspace-write, danger-full-access]`.
- **Implementation in `runner.py`:**
  1. Added `SUPPORTED_SANDBOX_POLICIES = {"read-only", "workspace-write", "danger-full-access"}`.
  2. In `build_codex_command`:
     - Emits `-s read-only` when `sandbox_policy="read-only"`.
     - **Omits `--add-dir <sandbox>`** in read-only mode so Codex does not mark the workspace writable.
  3. Orchestrator automatically defaults `sandbox_policy="read-only"` when `task_config.read_only == True`.
- **Defense in Depth (Post-Run Mutation Check):**  
  In `orchestrator._execute_single_arm`, if `is_read_only_task` is True:  
  Any modified, deleted, or created file (`git_impact.files_changed_count > 0` or `insertions > 0` or `deletions > 0`) immediately triggers `[READ-ONLY INVARIANT VIOLATED]`, forcing `score = 0.0` and `passed_all_requirements = False`.

---

## 5. Candidate Binary & Token Guard Verification

- **Candidate Binary Path:** `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`
- **Verified SHA-256:** `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` (100% Match).
- **Single-Arm Mapping:** `--engram-v154-bin` and `--expected-v154-sha256` explicitly bind Candidate Phase 1 to `v154_arm3`.
- **Token Guard Preservation:**  
  - Proposed smoke-test ceiling: `10,000 tokens`.
  - Proposed pre-dispatch reserve: `2,000 tokens`.
  - Nominal preventive halt threshold: `8,000 tokens`.
  - Fully tested across boundary conditions (`remaining == estimated` allowed; `remaining < estimated` halted).

---

## 6. Offline Test Results

### 6.1 Focused Remediation Suite
- **Command:** `pytest benchmarks/matched-pair-harness/tests/test_milestone_2c_remediation.py -v`
- **Result:** **21 passed in 0.84s (100% pass rate)**.

### 6.2 Full Repository Regression Suite
- **Command:** `pytest benchmarks/matched-pair-harness/tests -q`
- **Result:** Pre-change baseline of 228 passing tests preserved, plus 21 new tests = **249 total passing tests**.
- **Exit Code:** `0` (Zero failures, zero errors, zero skips).
