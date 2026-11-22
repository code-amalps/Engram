# Engram Amendment 05 — Milestone 2C: Command Readiness Specification

**Document Version:** 1.0.0  
**Date:** 2026-10-30  
**Status:** Remediated & Verified (Offline Preflight Complete)  
**Classification:** `REMEDIATION COMPLETE — READY FOR FINAL OFFLINE AUDIT`

> [!CAUTION]
> **STRICT EXECUTION BOUNDARY:**  
> The command documented below is **NOT AUTHORIZED FOR EXECUTION**. It is provided strictly as a verified execution template following the remediation of the 5 blockers identified in Milestone 2B. Live execution requires explicit written owner authorization across all governance gates.

---

## 1. Verified Smoke-Test Command Template

The following command has been validated offline via Python unit tests, argument parser introspection, schema verification, and filesystem integrity checks:

```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --mode single `
  --task smoke_test_read_only `
  --arms v154_arm3 `
  --engram-v154-bin "benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe" `
  --expected-v154-sha256 "F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE" `
  --max-tokens-ceiling 10000 `
  --headroom-reserve 2000 `
  --model gpt-5.6-luna `
  --reasoning low `
  --dependency-strategy none `
  --output-dir "benchmarks/matched-pair-harness/reports/smoke_test"
```

---

## 2. Argument-by-Argument Technical Specification

| Argument Flag | Supplied Value | Technical Routing & Implementation Behavior | Remediation Status |
|:---|:---|:---|:---:|
| `--mode` | `single` | Directs `orchestrator.py` to enter `run_single(arm)`. Validates that exactly 1 arm is passed. Bypasses paired/triad comparative analysis. Produces `single_summary.json` and `single_report.md`. | **REMEDIATED** |
| `--task` | `smoke_test_read_only` | Loads `benchmarks/matched-pair-harness/config/tasks/smoke_test_read_only.json`. Pinned to commit `1c285608...`, targets `authLogout` in `auth.actions.ts`. Declares `read_only=true` and `requires_dependencies=false`. | **REMEDIATED** |
| `--arms` | `v154_arm3` | Selects Candidate Phase 1 execution arm. Validated by `_validate_single_arm_selection`. Rejects multi-arm lists or empty values. Only arm 3 is provisioned and executed; arms 1 and 2 are never touched. | **REMEDIATED** |
| `--engram-v154-bin` | `"benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe"` | Path to the candidate Phase 1 binary. Orchestrator verifies file existence prior to sandbox dispatch. | **VERIFIED** |
| `--expected-v154-sha256` | `"F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE"` | Cryptographic binding invariant. Orchestrator computes SHA-256 of candidate binary and fails closed if any mismatch occurs. | **VERIFIED** |
| `--max-tokens-ceiling` | `10000` | Maximum token expenditure ceiling for the single smoke-test run. Enforced by `StreamingTokenTracker`. | **VERIFIED** |
| `--headroom-reserve` | `2000` | Pre-dispatch headroom reserve. Nominal stop threshold = $10,000 - 2,000 = 8,000$ tokens. If remaining budget is less than 2,000 tokens before dispatch, run halts proactively. | **VERIFIED** |
| `--model` | `gpt-5.6-luna` | Model identifier requested for Codex agent execution. Preserved from protocol baseline. | **VERIFIED** |
| `--reasoning` | `low` | Reasoning effort level passed to Codex CLI. Preserved from protocol baseline. | **VERIFIED** |
| `--dependency-strategy` | `none` | Instructs harness to skip `node_modules` copy/junction. Accepted by `_execute_single_arm` because `requires_dependencies=false` and `read_only=true`. Prevents directory pollution and package installation. | **REMEDIATED** |
| `--output-dir` | `"benchmarks/matched-pair-harness/reports/smoke_test"` | Destination directory for trial artifacts. Validated against protected roots. If prior evidence exists, automatically provisions `run_YYYYMMDD_HHMMSS` subdirectory. | **REMEDIATED** |

---

## 3. Actual Execution-Path Routing

### 3.1 Single-Arm Workflow Routing (`Orchestrator.run_single`)
1. **Validation Phase:**
   - Evaluates `--mode single`.
   - Confirms `self.selected_arms == ["v154_arm3"]`.
   - Validates that candidate binary exists and matches expected SHA-256 (`F56347...`).
   - Validates `output_dir` safety (rejects repo root, source tree, or golden repository).
2. **Sandbox Initialization:**
   - Provisions isolated ephemeral worktree from golden repo commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.
   - Evaluates `--dependency-strategy none`. Since task has `read_only=true`, dependency copying is skipped.
3. **Execution Dispatch:**
   - Detects `task_config.read_only == true` $\rightarrow$ sets `sandbox_policy = "read-only"`.
   - `CodexRunner.build_codex_command` emits `-s read-only` and **omits `--add-dir`**.
   - `StreamingTokenTracker` initialized with `ceiling=10000, reserve=2000`.
   - Invokes only `v154_arm3` with candidate binary `candidate-phase1/engram.exe`.
   - Companion arms (`baseline_arm1`, `v151_arm2`, `control_A`) are completely bypassed.
4. **Post-Execution Evaluation & Defense-in-Depth:**
   - Evaluator inspects prompt output for `authLogout` and `auth.actions.ts` (`REQ-1`).
   - Orchestrator checks Git diff in sandbox: if any file was modified, created, or deleted, `REQ-2` fails and score is set to 0.0.
   - Writes `single_summary.json`, `single_report.md`, and `run_telemetry.json` to the target output directory.

---

## 4. Protected Artifact Bindings

- **Authorized Executable:** Candidate Phase 1 Binary
- **Executable Path:** `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`
- **Authoritative SHA-256:** `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`
- **Execution Arm:** `v154_arm3`
- **Frozen Reference Binaries (Preserved & Unexecuted):**
  - Frozen v1.5.4: `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798`
  - Frozen v1.5.1: `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0`

---

## 5. Token Guard and Budget Parameters

- **Smoke-Test Hard Ceiling:** `10,000 tokens`
- **Pre-Dispatch Headroom Reserve:** `2,000 tokens`
- **Nominal Stop Threshold:** `8,000 tokens`
- **Pre-Dispatch Invariant:** If `(ceiling - tokens_used) < reserve`, the orchestrator refuses to dispatch further agent commands and triggers a preventive fail-fast halt.
- **Overrun Governance Note:** As established in Milestones 1 and 2A, the token guard is an external polling monitor, not a zero-leakage kernel barrier. An in-flight completion during an active command turn can exceed the ceiling before polling terminates the process. The 2,000-token reserve mitigates this risk but does not eliminate in-flight token overrun.

---

## 6. Status of Owner Authorization Gates

All 5 technical blockers have been remediated in code. However, live execution remains strictly blocked pending owner resolution of the remaining governance and account gates:

| Gate # | Description | Technical Harness Status | Owner Action Required |
|:---:|:---|:---:|:---|
| **Gate 1** | Token ceiling (10,000) and reserve (2,000) approval, acknowledging in-flight external polling overrun risk. | **IMPLEMENTED & VERIFIED** | Owner approval required. |
| **Gate 2** | Independent owner login to platform dashboard to verify subscription tier, account entitlement, plan quota, and automatic purchase disablement. | *External to Harness* | Independent owner action required. |
| **Gate 3** | Explicit written authorization for a single live smoke-test run. | *External to Harness* | Explicit written owner authorization required. |
| **Gate 4** | Confirmation that account has entitled access to `gpt-5.6-luna` without silent fallback/substitution. | *External to Harness* | Owner verification required. |
| **Gate 5** | Candidate Phase 1 binary path and cryptographic SHA-256 verification (`F56347...`). | **VERIFIED (100% MATCH)** | *Completed offline*. |
| **Gate 6** | Read-only sandbox isolation and dependency strategy (`--dependency-strategy none`). | **REMEDIATED & VERIFIED** | *Completed offline*. |
| **Gate 7** | Output directory overwrite protection and path safety. | **REMEDIATED & VERIFIED** | *Completed offline*. |

---

## 7. Execution Readiness Conclusion

The smoke-test execution specification is **technically complete and ready for final offline audit**.

No further code modifications are required for the smoke-test command to execute properly once authorized. However, **zero live commands shall be executed until explicit owner authorization is granted**.
