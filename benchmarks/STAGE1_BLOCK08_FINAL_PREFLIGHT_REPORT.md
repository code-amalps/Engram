# Engram — Stage 1 Block 08 Final Preflight & Approval Preparation Report

**Document ID:** `STAGE1_BLOCK08_FINAL_PREFLIGHT_REPORT`  
**Date:** 2026-10-29  
**Task Under Test:** `task_b_remember_me`  
**Block Under Preflight:** `block_08`  
**Preflight Mode:** Offline Verification & Approval Preparation Only (**NO LIVE RUN PERFORMED**)  
**Proposed Live Execution Command (Reference Only — NOT EXECUTED):**
```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py --block block_08 --max-tokens-ceiling 350000
```

---

## 1. Executive Summary & Verification Verdict

An exhaustive offline audit of the Stage 1 Core Triad execution harness, candidate binaries, and Amendment 03 draft was conducted. All 11 operational safeguards, telemetry invariants, cryptographic seals, and termination mechanisms have been verified:

- **All 11 Safeguard Checks:** **PASS**
- **Amendment 03 Review:** **VERIFIED (Draft status `DRAFT — NOT APPROVED` preserved)**
- **Offline Test Suite:** **26/26 tests passed (100% offline, 0 live API tokens)**
- **Cryptographic Hashes & Seals:** **100% Exact Match**
- **Live Model Usage in Preflight:** **EXACTLY ZERO TOKENS / ZERO CALLS (VERIFIED)**
- **Operational Status:** **AWAITING EXPLICIT HUMAN APPROVAL**

---

## 2. Safeguard-by-Safeguard Verification Results

| # | Operational Safeguard / Integrity Check | Result | Implementation Evidence & Verification Method |
| :---: | :--- | :---: | :--- |
| **1** | **CLI accepts `--max-tokens-ceiling 350000`** | **PASS** | [`orchestrator.py` line 1268](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L1268): `parser.add_argument("--max-tokens-ceiling", type=int, default=250_000)`. Verified via offline test `scratch/test_cli_parsing.py` parsing integer `350000`. |
| **2** | **CLI value reaches runner for every arm** | **PASS** | `Orchestrator.__init__` forwards `self.max_tokens_ceiling` to `CodexRunner(max_tokens_ceiling=...)`. Runner loop enforces `current_cumulative_tokens > self.max_tokens_ceiling` identically across all arms. |
| **3** | **Default remains 250,000 without override** | **PASS** | Default parameter in `argparse` and `CodexRunner.__init__` is `250_000`. Verified via `scratch/test_cli_parsing.py` with `--block block_08` (yields `250000`). |
| **4** | **Ceiling measures total input + output tokens (`>` rule)** | **PASS** | `StreamingTokenTracker.effective_tokens` tracks cumulative prompt plus completion tokens. Breach condition is strictly `current_cumulative_tokens > self.max_tokens_ceiling`. Boundary tested: 250,000 allowed, 250,001 terminated. |
| **5** | **Streaming/session rollout telemetry monitored & PID tree killed** | **PASS** | Real-time reader tails streaming stdout and `.codex/sessions` rollout. Upon breach, immediately calls [`terminate_pid_tree(proc.pid)`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/isolation.py#L35-L55) (`taskkill /F /T /PID`). Tested in `test_resilience_repair.py`. |
| **6** | **600-second timeout & partial-output preservation** | **PASS** | `elapsed >= self.timeout_seconds` triggers safe termination. Recovered stdout and native rollout lines are flushed to `events.jsonl`. Tested in `test_resilience_repair.py`. |
| **7** | **Invalid/missing telemetry marked invalid (not genuine 0 tokens)** | **PASS** | [`Orchestrator.validate_telemetry_integrity`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L240-L330) catches `TELEMETRY_MISSING`, `ZERO_TOKEN_ANOMALY`, and invariant violations, setting `telemetry_valid = False` and suppressing savings claims. |
| **8** | **Failed arm triggers stop-on-failure and skips downstream arms** | **PASS** | When `is_failed = True`, orchestrator halts triad immediately, skips subsequent arms, and writes structured `SKIPPED_STOP_ON_FAILURE` records. Verified in `test_harness_safety_audit.py` and live in Block 06. |
| **9** | **Single block invocation cannot execute Blocks 09 or 10** | **PASS** | [`orchestrator.py` line 1335](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L1335) executes `orchestrator.run_triad(block_id=args.block)` once. There is zero looping or recursion over subsequent blocks. |
| **10** | **Arm order & binary hashes match Block 08 preflight** | **PASS** | Schedule: `["v154_arm3", "baseline_arm1", "v151_arm2"]` (Seed: `2026102808`). v1.5.1 SHA-256 (`474a...`) and v1.5.4 SHA-256 (`cf80...`) match `FREEZE_SEAL.md` bit-for-bit. |
| **11** | **Baseline, frozen artifacts, and golden repo unmutated** | **PASS** | `PREREGISTRATION.md` (`db3ab295...`), `FREEZE_SEAL.md`, golden repository commit (`1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`), and dependency manifest verified 100% intact. |

---

## 3. Important Operational Limitation: Streaming Telemetry Latency & Overshoot

> [!WARNING]
> **Asynchronous Model Turn Boundary Limitation:**  
> The 350,000-token ceiling is a **configured operational threshold**, not a guaranteed absolute cap on actual consumption. Model reasoning and generation occur remotely on OpenAI provider infrastructure. Codex flushes discrete token usage lines (`token_usage_record`, `turn.completed`, `event_msg`) only when an individual model turn finishes or a chunked usage packet is emitted.
> 
> - **Overshoot Mechanics:** A model turn in progress may overshoot the operational threshold before usage telemetry arrives and the runner can terminate the child process tree. For example, in Block 06, Turn 8 ended at 216,097 tokens; Turn 9 was generated on the server consuming 37,651 tokens, pushing cumulative tokens to 253,748 before the usage line was flushed and process termination executed 355ms later (an overshoot of 3,748 tokens beyond the 250,000 ceiling).
> - **Operational Boundary vs. Hard Hardware Clamp:** The ceiling terminates multi-turn accumulation loops immediately after the breaching turn completes, but cannot preemptively abort a remote HTTP response in mid-stream before token counts are reported.
> - **Stop-on-Failure Scope:** While `stop_on_failure` prevents downstream arms from executing after a breach, it does not eliminate in-flight overshoot within the active arm. Without empirical runtime evidence from Block 08, no speculative alternative worst-case figure is asserted.

---

## 4. Configured Operational Exposure & Quota Analysis

| Exposure Dimension | Preflight Verification (This Task) | Proposed Block 08 Execution (If Authorized) |
| :--- | :---: | :---: |
| **Offline Test Executions** | 26 unit tests (Ran in ~10s) | 0 (Live benchmark mode) |
| **Live Model Requests / Turns** | **0 calls** (Verified) | Estimated ~15–20 turns total (Not guaranteed) |
| **Live Model Tokens Consumed** | **0 tokens** (Verified) | Estimated ~30k–60k (Arm 1); ~280k–320k (Arm 2 if run) |
| **Configured Operational Ceiling** | N/A (Offline only) | **350,000 tokens/arm** (Operational threshold) |
| **Theoretical Aggregate Threshold** | N/A (Offline only) | **1,050,000 tokens** (3 scheduled arms $\times$ 350,000) |
| **Account Quota & Billing Status** | **0 quota consumed** (Verified) | **Unverified / Account-Dependent** (See details below) |

> [!IMPORTANT]
> **Exposure and Billing Clarifications:**  
> 1. **Operational Thresholds vs. Absolute Consumption:** The 350,000 tokens per arm and theoretical 1,050,000 tokens across three arms are configured operational stopping thresholds, not guaranteed absolute caps on actual consumption. In-flight turn overshoot may cause the active arm to exceed the threshold before termination.
> 2. **Preflight Verification Evidence:** Live model calls and live token usage during this preflight are strictly **zero**, supported by offline test execution using local mocks and synthetic event streams.
> 3. **Account Billing & Quota Independence:** Subscription coverage or $0.00 out-of-pocket cost is **not claimed or independently verified**. Actual quota deductions, rate limits, and any billing implications depend entirely on the user's configured Codex account tier, subscription limits, and billing arrangement with OpenAI. No live calls were made to inspect account billing.
> 4. **Projected Token Figures:** The projected ranges (~30k–60k for Arm 1; ~280k–320k for Arm 2) are engineering estimates derived from Task A and Block 06 trajectories, not guarantees.

---

## 5. Offline Tests Run and Verified

The following offline regression and safety suites were executed:
1. `benchmarks/matched-pair-harness/tests/test_harness_safety_audit.py`:
   - 19/19 tests passed (`Ran 19 tests in 7.715s, OK`).
   - Verified boundary token tracking (249,999 vs 250,000 vs 250,001), process tree termination, and stop-on-failure handling.
2. `benchmarks/matched-pair-harness/tests/test_resilience_repair.py`:
   - 7/7 tests passed (`Ran 7 tests in 2.363s, OK`).
   - Verified timeout handling (600s), partial stdout recovery, and missing telemetry schema validation.
3. `scratch/test_cli_parsing.py`:
   - 4/4 assertions passed.
   - Verified default ceiling (250,000), CLI override (350,000), Block 08 schedule parameters, and single-block execution boundary.

---

## 6. Working-Tree Status & Document Integrity

### Git Status (`git status --short`)
```
 M benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json
 M benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json
 M benchmarks/matched-pair-harness/src/evaluator.py
 M benchmarks/matched-pair-harness/src/isolation.py
 M benchmarks/matched-pair-harness/src/orchestrator.py
 M benchmarks/matched-pair-harness/src/runner.py
 M benchmarks/matched-pair-harness/src/telemetry.py
?? benchmarks/AMENDMENT_02_STAGE1_CORE_TRIAD.md
?? benchmarks/AMENDMENT_03_TASK_B_OPERATIONAL_CEILING_CALIBRATION.md
?? benchmarks/STAGE1_BLOCK08_FINAL_PREFLIGHT_REPORT.md
... (Historical reports preserved)
```
- **Pre-existing changes:** The 7 modified files above belong to the previously verified resilience repair; zero modifications were made to them during this task.
- **Untracked files:** `AMENDMENT_03_TASK_B_OPERATIONAL_CEILING_CALIBRATION.md` and this preflight report are the only newly written documents.
- **Zero Git Operations:** Zero files staged, committed, pushed, tagged, merged, stashed, or reset.

---

## 7. Cryptographic Seals

| Artifact | Path | SHA-256 Digest | Status |
| :--- | :--- | :--- | :---: |
| **Engram v1.5.1 Executable** | `benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | **SEALED** |
| **Engram v1.5.4 Executable** | `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | **SEALED** |
| **Preregistration Seal** | `benchmarks/cost-baseline/PREREGISTRATION.md` | `DB3AB2956A4DA40AAD8F1F376AA72A50BEBC96F3E1ECC3120F405F0A294EA8BB` | **SEALED** |
| **Golden Reference Commit** | `angular-ngrx-material-starter` | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` | **LOCKED** |

---

## 8. Proposed Execution Command (Reference Only)

```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py --block block_08 --max-tokens-ceiling 350000
```
**STATUS: STRICTLY NOT EXECUTED.**

---

## 9. Explicit Human Approval Checklist

> [!CAUTION]
> **GOVERNANCE NOTICE:**  
> Amendment 03 remains **`DRAFT — NOT APPROVED`**. Live execution of Block 08 requires your explicit authorization of the four items below:

```
========================================================================================
FORMAL PROTOCOL AMENDMENT 03 APPROVAL CHECKLIST (BLOCK 08 SCOPE)

[   ] 1. APPROVE 350,000-TOKEN CEILING:
         Approve the 350,000-token operational ceiling specifically for Block 08,
         leaving the harness default at 250,000 tokens.

[   ] 2. CONFIRM TIMEOUT & STOP-ON-FAILURE:
         Confirm retention of the 600-second per-arm timeout and mandatory
         stop-on-failure policy (prohibiting --no-stop-on-failure).

[   ] 3. ACCEPT BLOCK 06 HISTORICAL TREATMENT:
         Accept the documented treatment of Block 06 as an unsuccessful terminated
         attempt with unknown completion cost, excluded from formal savings formulas.

[   ] 4. AUTHORIZE LIVE BLOCK 08 EXECUTION:
         Explicitly authorize live execution of Stage 1 Block 08 using command:
         python benchmarks/matched-pair-harness/src/orchestrator.py --block block_08 --max-tokens-ceiling 350000
========================================================================================
```
