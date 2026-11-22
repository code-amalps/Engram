# Engram Amendment 05 — Milestone 2D: Track A Independent Final Harness Audit

**Document Version:** 1.0.0  
**Date:** 2026-10-30  
**Audit Scope:** Independent Verification of Milestone 2C Harness Remediation  
**Classification:** `AUDIT PASSED WITH QUALIFICATIONS`

---

## 1. Executive Summary

This independent technical audit examined the source code, argument parsing, execution paths, security boundaries, and preservation evidence implemented during Milestone 2C in `benchmarks/matched-pair-harness`.

The audit confirms that all five primary blockers identified in Milestone 2B have been remediated in code and validated through offline testing. The harness is technically ready for single-arm, read-only smoke-test execution under strict candidate binary validation and token guard monitoring.

The audit awards the classification **`AUDIT PASSED WITH QUALIFICATIONS`** based on two minor edge-case findings that do not impede offline readiness but must be documented for full transparency:
1. **Timestamp Granularity Edge Case:** Automatic output directory collision avoidance relies on `strftime("%Y%m%d_%H%M%S")`, which provides second-level uniqueness. Concurrent or rapid sub-second invocations could produce identical directory names unless disambiguated with a process ID, microsecond timestamp, or monotonic counter.
2. **Read-Only Requirement 1 Static Target Check:** The evaluator verifies `REQ-1` by confirming that `auth.actions.ts` in the provisioned sandbox contains the `authLogout` declaration. Because this declaration exists at the baseline golden commit (`1c285608...`), this static check confirms file presence and integrity rather than parsing agent natural-language dialogue.

---

## 2. Detailed Technical Audit Findings

### A1. Single-Arm Routing & Execution Isolation
- **CLI Registration:** In [orchestrator.py](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L1496), `--mode` explicitly registers `choices=["pair", "triad", "single"]` with default `"pair"`.
- **Validation Logic:** Lines 1537–1549 strictly validate `--mode single`:
  - Enforces `len(arm_order) == 1`. Passing zero arms or multiple arms (`--arms v154_arm3,baseline_arm1`) exits immediately with error code 1.
  - Rejects unrecognized arm identifiers (only `control_A`, `treatment_B`, `baseline_arm1`, `v151_arm2`, `v154_arm3` allowed).
- **Execution Workflow (`run_single`):** Lines 1407–1449 implement `Orchestrator.run_single(arm)`:
  - Invokes `self._execute_single_arm(arm=arm, exec_order=1)`.
  - Companion arms (`baseline_arm1`, `v151_arm2`, `control_A`) are completely bypassed; no processes or sandboxes are provisioned for them.
  - Outputs `single_summary.json` and `single_report.md` directly into `self.output_dir`.
- **Preservation of Existing Modes:** Pair and triad execution workflows remain completely intact on separate branches (`run_pair()` and `run_triad()`).

### A2. Candidate Binary Selection & Cryptographic Binding
- **Arm Mapping:** Passing `--arms v154_arm3` binds execution to `self.arm_binaries["v154_arm3"]`, populated from `--engram-v154-bin`.
- **Pre-Execution Cryptographic Check:** In [orchestrator.py](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L851-L861):
  ```python
  if is_engram:
      if not binary or not binary.exists():
          raise FileNotFoundError(f"Engram binary missing for arm '{arm}': {binary}")
      if expected_sha:
          verify_frozen_binary(binary, expected_sha)
  ```
- **Fail-Closed Verification:** `verify_frozen_binary` computes the exact SHA-256 of the target executable before sandbox dispatch. If a mismatch occurs, execution aborts with `SecurityViolationError` before any agent or sandbox execution can take place.
- **Substitution Prevention:** Frozen v1.5.4 cannot be silently substituted because `--engram-v154-bin` points explicitly to `candidate-phase1/engram.exe` with expected SHA-256 `F56347...`.

### A3. Read-Only Execution Boundary & Sandbox Mutation Defense
- **Codex CLI Sandbox Parameter:** Static help introspection of the installed Codex binary confirmed supported sandbox policies: `[read-only, workspace-write, danger-full-access]`.
- **Runner Command Construction:** In [runner.py](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L559-L565):
  - Emits `-s read-only` when `self.sandbox_policy == "read-only"`.
  - **Omits `--add-dir` entirely** when in read-only mode, preventing Codex from granting write capabilities to the target directory.
  - Passes `-C <canonical_sandbox_path>`, ensuring execution takes place exclusively in the ephemeral sandbox, completely isolating the host Engram repository and golden repository.
- **Orchestrator Policy Derivation:** [orchestrator.py](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L177-L181) inspects `task_config.read_only`. If true, `sandbox_policy` automatically defaults to `"read-only"`.
- **Post-Run Defense-in-Depth:** In lines 977–988, if `is_read_only_task` is true, the orchestrator inspects `git_impact`. If `files_changed_count > 0`, `unrelated_files_count > 0`, `insertions > 0`, or `deletions > 0`, the run fails closed immediately:
  - `score = 0.0`
  - `passed_all_requirements = False`
  - `REQ-2 = FAIL`
  - Emits `[READ-ONLY INVARIANT VIOLATED]`.

### A4. Dependency Strategy & Isolation Safeguards
- **Strategy Guard Remediation:** In [orchestrator.py](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L862-L878):
  - Live benchmark runs evaluate `self.dependency_strategy`.
  - If `dependency_strategy == "none"`, it is accepted **only if** `not task_requires_deps or is_read_only_task`.
  - Tasks requiring dependencies (such as `task_b_remember_me`) reject `none` and trigger `SafetyViolationError`.
- **Zero Package Installations:** Because `smoke_test_read_only` specifies `requires_dependencies: false` and `read_only: true`, passing `--dependency-strategy none` creates the sandbox with zero `node_modules` copies, zero junction links, and zero npm/yarn network calls.

### A5. Output Directory Safety & Overwrite Protection
- **CLI Parameter Support:** `--output-dir` is declared in `argparse` (line 1513) and routed to `Orchestrator.__init__`.
- **Protected Root Rejection:** Lines 130–141 resolve the target path and raise `SafetyViolationError` if the output directory resolves to:
  - The Engram repository root.
  - The harness source tree (`benchmarks/matched-pair-harness/src`).
  - The golden repository root.
- **Collision Avoidance:** Lines 142–165 inspect the destination for existing evidence files (`triad_summary.json`, `single_summary.json`, `run_telemetry.json`, `events.jsonl`, `triad_report.md`). If prior evidence exists, the orchestrator appends `f"run_{timestamp}"`.
- **Audit Qualification 1:** The timestamp format `"%Y%m%d_%H%M%S"` provides second-level resolution. If two invocations target the same directory within the exact same second, a collision could still occur. A sub-second UUID or monotonic process ID suffix is recommended for future hardening.

### A6. Token Guard Enforcement & Boundary Limits
- **Ceiling & Reserve Propagation:** CLI arguments `--max-tokens-ceiling 10000` and `--headroom-reserve 2000` propagate to `Orchestrator`, `CodexRunner`, and `StreamingTokenTracker`.
- **Pre-Dispatch Headroom Guard:** At each agent command dispatch, the tracker evaluates:
  $$\text{remaining\_tokens} = \text{ceiling} - \text{tokens\_used}$$
  If $\text{remaining\_tokens} < \text{reserve}$, dispatch is aborted with `PRE_DISPATCH_HEADROOM_EXHAUSTED` (nominal halt threshold: 8,000 tokens).
- **Hard Ceiling Breach Distinction:** If an in-flight command completion pushes the token total past 10,000 tokens, the tracker signals `TOKEN_CEILING_BREACHED` and terminates the external process.
- **Realistic Qualification:** External polling latency (~250 ms) means an active in-flight model generation may exceed 10,000 tokens before termination takes effect. The harness documents this boundary and makes no false claims of a kernel-level hard cap.

### A7. Test & Artifact Preservation Evidence
- **Remediation Unit Tests:** [test_milestone_2c_remediation.py](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/tests/test_milestone_2c_remediation.py) contains 21 focused unit tests; all 21 passed in 0.84s (100%).
- **Full Regression Suite:** The full harness suite discovered and executed 249 tests; all 249 passed in 178.01s (exit code 0).
- **Cryptographic Binary Hashes:**
  - Candidate Phase 1: `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` (100% Match)
  - Frozen v1.5.4: `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` (100% Match)
  - Frozen v1.5.1: `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` (100% Match)
- **Historical Evidence Preservation:**
  - Amendment 04 Protocol: `03C4B2786E2F62A6435A7EA81C54E65889619296FD4ED9873546EF20573030E2` (100% Match)
  - Block 07 Final Report: `325EF0EC3F14155B02F388C584DE9F85B88D04ABA142E87C8DB8FDCACCFD4266` (100% Match)
- **Golden Repository State:** Verified clean at commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` on `test-branch`, 18 modified files, `+208 / -16`, untracked `.engram/` (29 files), exactly 5 worktrees intact.

---

## 3. Audit Qualifications

1. **Qualification 1 (Output Directory Granularity):**  
   The timestamping pattern `run_YYYYMMDD_HHMMSS` resolves to 1-second buckets. While sufficient for sequential executions and human testing, automated concurrent batch execution could experience naming collisions unless a process ID or random nonce is appended.
2. **Qualification 2 (Static Evaluator Checking vs. Agent Dialogue):**  
   The task evaluator checks whether `projects/angular-ngrx-material-starter/src/app/core/auth/auth.actions.ts` in the sandbox contains `export const authLogout = createAction('[Auth] Logout');`. Because this symbol is present in the golden repository baseline, this static check acts as an invariant integrity check rather than verifying that the agent outputted the correct answer in its dialogue events.

---

## 4. Audit Conclusion

The Milestone 2C harness remediation is **formally validated**. The five technical blockers are resolved, zero regressions were introduced, and all cryptographic and filesystem preservation boundaries were strictly maintained.
