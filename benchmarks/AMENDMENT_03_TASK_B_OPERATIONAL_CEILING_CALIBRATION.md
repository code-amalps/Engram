# Amendment 03: Task B Operational Token Ceiling Calibration (Block 08 Only)

**Document Identifier:** `AMENDMENT_03_TASK_B_OPERATIONAL_CEILING_CALIBRATION`  
**Document Version:** 3.2.0-APPROVED (Restricted to Block 08)  
**Status:** **APPROVED FOR BLOCK 08 ONLY**  
**Parent Specification:** [`benchmarks/cost-baseline/PREREGISTRATION.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/PREREGISTRATION.md) (SHA-256: `db3ab2956a4da40aad8f1f376aa72a50bebc96f3e1ecc3120f405f0a294ea8bb`)  
**Preceding Protocol Amendment:** [`benchmarks/AMENDMENT_02_STAGE1_CORE_TRIAD.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/AMENDMENT_02_STAGE1_CORE_TRIAD.md)  
**Target Golden Repository:** `angular-ngrx-material-starter` (Commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`)  
**Proposed Effective Scope:** Prospective Execution of `task_b_remember_me`, **Block 08 Only**

---

## 1. Purpose and Rationale

### 1.1 Background & Observed Empirical Evidence (Block 06 Incident)
On 2026-10-29, the first live Task B triad (**Block 06: `task_b_remember_me`**) was executed under explicit authorization. Arm 1 (`baseline_arm1`) was subjected to the initial operational harness limit of 250,000 tokens.

The empirical forensic investigation ([`benchmarks/STAGE1_TASK_B_BLOCK06_FORENSIC_ANALYSIS.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/STAGE1_TASK_B_BLOCK06_FORENSIC_ANALYSIS.md)) established the following observed facts:
1. **Rapid Context Compounding:** Across 9 conversational turns, cumulative token consumption reached **253,748 tokens** in **109.26 seconds**, triggering immediate circuit-breaker termination.
2. **Prompt Re-Ingestion Dominance:** Of the 253,748 tokens consumed, **250,026 tokens (98.5%)** were prompt tokens re-sent on successive turns. Completion tokens totaled only 3,722 tokens (including 945 reasoning tokens), demonstrating that the agent was not generating degenerate text or runaway loops.
3. **Exploratory Ingestion Overhead:** In an unassisted baseline environment without architectural index tools (Engram MCP), the agent executed compound terminal commands (`Get-Content`, `rg`) dumping multiple raw TypeScript source files into stdout. This expanded the conversational context from 13,259 tokens (Turn 1) to 37,231 tokens (Turn 9).
4. **Substantive Code Progress:** The 6 modified files in `changes.patch` (+45 / -7 lines) represented direct, purposeful feature implementation (Angular Material login menu, NgRx action typing, effect persistence, and i18n localization). The model was terminated mid-stride while resolving module imports (`MatFormFieldModule` and `auth.effects.spec.ts`) immediately prior to executing the specified compilation checks (`npx tsc`).

### 1.2 Forecasts vs. Observed Evidence
- **Observed Fact:** The unassisted baseline agent reached Turn 9 at 253,748 tokens with near-complete feature code, but was interrupted before resolving template module imports (`MatCheckboxModule`) and running compiler verification.
- **Engineering Forecast:** Based on observed turn deltas (~35k tokens per turn at steady-state context), an unassisted baseline agent on Task B requires approximately **10 to 12 conversational turns** (~280,000 to 320,000 cumulative tokens) to complete file exploration, implementation, syntax repair, and non-interactive compiler verification.
- **Risk of Under-Sized Ceiling in Block 08:** In Block 08, Arm 1 is `v154_arm3` and Arm 2 is `baseline_arm1`. If `v154_arm3` completes successfully, maintaining a 250,000 ceiling on the subsequent baseline arm is forecast to trigger another premature termination at Turn 9, suppressing comparative savings claims ($S$) and leaving the triad without a completed baseline comparison.

### 1.3 Purpose of Calibration
This amendment formally proposes calibrating the operational token ceiling from **250,000 to 350,000 tokens per arm** strictly for the prospective execution of **Block 08**. This adjustment tests whether an additional 100,000 tokens of operational headroom allows the unassisted baseline arm to finish its verification turns while maintaining a strict circuit breaker.

---

## 2. Scope and Status (Block 08 Exclusivity)

### 2.1 Formal Status
- **Status:** **`APPROVED FOR BLOCK 08 ONLY`**
- Explicitly approved by User / Principal Investigator on 2026-10-29. Authorizes prospective execution of Block 08 only with `--max-tokens-ceiling 350000`. Does not authorize Blocks 07, 09, or 10.

### 2.2 Strict Scope Limitation (Block 08 Only)
- This proposed calibration applies **strictly and exclusively** to the prospective execution of `task_b_remember_me`, **Block 08**.
- **No Blanket Authorization for Blocks 07, 09, or 10:** Blocks 07, 09, and 10 are **NOT authorized** and will not automatically execute. Each remaining block requires a separate independent human decision and authorization.
- **Harness Defaults Untouched:** The harness default in [`orchestrator.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py) and [`runner.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py) remains **250,000 tokens**. The 350,000-token ceiling for Block 08 is passed via explicit command-line invocation (`--max-tokens-ceiling 350000`).
- **Task A Excluded:** Blocks 01–05 are completely excluded and maintain their existing operational behavior (<30k tokens).

### 2.3 Preserved Arm Order & Operational Limits for Block 08
Block 08 executes under its existing preregistered Latin-square schedule (Seed: `2026102808`):
1. **Order 1:** `v154_arm3` (Engram v1.5.4 with Turn-0 context injection + MCP server)
2. **Order 2:** `baseline_arm1` (Native baseline, no MCP, no Turn-0 injection)
3. **Order 3:** `v151_arm2` (Engram v1.5.1 interactive MCP)

- **Execution Timeout:** Remains **600 seconds (10 minutes) per arm** unchanged.
- **Stop-on-Failure:** Remains **strictly enabled** (`stop_on_failure = True`).
- **Non-Blocking Mode Prohibited:** The `--no-stop-on-failure` CLI flag is strictly forbidden.

---

## 3. Block 06 Historical Evidence Treatment

### 3.1 Preservation of Block 06
- All original artifacts, reports, run directories, and telemetry files for Block 06 remain permanently preserved and immutable:
  - Run directory: [`benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/)
  - Incident report: [`benchmarks/STAGE1_BLOCK06_LIVE_RESULTS_REPORT.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/STAGE1_BLOCK06_LIVE_RESULTS_REPORT.md)
  - Hardened report: [`benchmarks/STAGE1_BLOCK06_HARDENED_RESULTS_REPORT.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/STAGE1_BLOCK06_HARDENED_RESULTS_REPORT.md)
  - Forensic analysis: [`benchmarks/STAGE1_TASK_B_BLOCK06_FORENSIC_ANALYSIS.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/STAGE1_TASK_B_BLOCK06_FORENSIC_ANALYSIS.md)
  - Authoritative native rollout: `C:\Users\Amal P S\.codex\sessions\2026\10\29\rollout-2026-10-29T11-31-31-01a1863f-cd55-7ea0-a9fd-7c567ddfc3d5.jsonl`

### 3.2 Formal Recording of Block 06 Baseline Result
The baseline run in Block 06 is formally recorded as:
- **Observed Cumulative Token Usage:** 253,748 tokens.
- **Execution Status:** `TOKEN_CEILING_BREACHED`.
- **Functional Acceptance:** Not established ($C = 0.00$; automated gates `BLOCKED`).
- **Telemetry Validity:** Valid (`true`), directly extracted from provider rollout session logs.

### 3.3 Explicit Methodological Distinctions
- **Terminated Unsuccessful Attempt:** Block 06 Arm 1 was an aborted, unsuccessful attempt terminated by the operational ceiling—**not a completed baseline run**.
- **Observed Resource Usage:** The Block 06 baseline consumed 253,748 tokens before termination by the operational ceiling. The run did not establish successful task completion, so the token cost of a successfully completed baseline implementation remains unknown.
- **Analytical Limitation:** This observation may be reported descriptively as a terminated attempt. Do not insert it into the formal token-savings formula as an accepted baseline. Do not impute a completion cost. Use formal right-censoring terminology only if the preregistered analytical methodology explicitly supports the required assumptions; otherwise describe it as a terminated, incomplete observation.

---

## 4. Comparative Analysis Rules & Scientific Safeguards

This amendment modifies only the operational ceiling parameter for Block 08. All preregistered scientific gates, evaluation criteria, and telemetry safeguards remain 100% active and uncompromised:

1. **Functional Acceptance Invariant ($C = 1.00$):**  
   Full functional acceptance strictly requires $C = 1.00$. All enabled quality gates must pass:
   - `L0_typecheck`: `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit`
   - `L2_unit_tests`: `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.spec.json --noEmit`
   - `L3_production_build`: `npm run build`
   If any gate fails, `mandatory_gates_passed = False`, forcing $C = 0.00$ and acceptance rejection.
2. **Savings Suppression Invariant:**  
   In accordance with Amendment 02 Section 6, if either compared arm in a pairwise evaluation fails acceptance ($C < 1.0$) or produces invalid telemetry, **comparative token savings claims are strictly suppressed (`null`)**. Savings claims may NEVER be calculated against a failed or aborted run.
3. **Statistical Validity Guard:**  
   A common 350,000 ceiling within Block 08 ensures symmetric operational boundaries for arms within that block, but **does not, by itself, guarantee statistical validity or task success**.
4. **Standalone v1.5.4 Evidence Interpretation:**  
   In Block 08, `v154_arm3` executes first (Order 1). A successful v1.5.4 run alone provides **authoritative standalone feasibility and resource-use evidence** (proof of task completion, turns taken, tokens consumed), but **does NOT constitute a completed baseline-versus-Engram savings comparison** if the subsequent baseline arm fails.
5. **Stop-on-Failure Behavior:**  
   If any arm fails (`TOKEN_CEILING_BREACHED`, `TIMEOUT`, `CRASH`, or invalid telemetry), `stop_on_failure` halts the triad immediately. Downstream arms are marked `SKIPPED_STOP_ON_FAILURE` in accordance with the existing protocol.

---

## 5. Maximum Exposure Statement

| Scope | Existing Limit (250k) | Proposed Block 08 Limit (350k) | Maximum Additional Exposure |
| :--- | :---: | :---: | :---: |
| **Per Arm Execution** | 250,000 tokens | 350,000 tokens | **+100,000 tokens** |
| **Complete Triad (3 Scheduled Arms)** | 750,000 tokens | 1,050,000 tokens | **+300,000 tokens** |

> [!IMPORTANT]
> **Strict Upper-Bound Definition:**  
> These numbers represent the **maximum permitted quota exposure prior to circuit-breaker cutoff**, NOT predicted consumption:  
> - For **Arm 1 (`v154_arm3`)**, predicted consumption is **~30,000–60,000 tokens** (<20% of the ceiling).  
> - Because `stop_on_failure = True` is strictly enforced, if any arm fails, subsequent arms are never executed, preventing the triad from reaching the theoretical 1,050,000 token maximum.

---

## 6. Failure Handling & Telemetry Classification

| Failure Mode | Detection Point | Execution Status | Correctness Score ($C$) | Gate Results | Downstream Action |
| :--- | :--- | :---: | :---: | :---: | :--- |
| **Token Ceiling Breach ($> 350,000$)** | Streaming reader during turn event | `TOKEN_CEILING_BREACHED` | `0.00` | `BLOCKED` | `taskkill /F /T`, record partial telemetry, **halt triad immediately**. |
| **Execution Timeout ($> 600\text{ s}$)** | Runner timer check | `TIMEOUT` | `0.00` | `BLOCKED` | `taskkill /F /T`, record partial telemetry, **halt triad immediately**. |
| **Telemetry Invalid / Missing** | Telemetry parser validation | `TELEMETRY_INVALID` | `0.00` | `BLOCKED` | Suppress all savings claims, **halt triad immediately**. |
| **Compiler / Test Gate Failure** | Evaluator check (`L0`, `L2`, `L3`) | `SUCCESS` (Run exit) | `0.00` | `FAIL` | Record telemetry, **halt triad immediately** (if $C < 1.0$). |
| **Upstream Arm Failure** | Orchestrator loop | `SKIPPED_STOP_ON_FAILURE` | `0.00` | `SKIPPED` | Do not provision sandbox, record 0 tokens, **skip arm**. |

---

## 7. Cryptographic Verification & Reproducibility Seals

The candidate executables, golden repository commit, and parent preregistration remain bit-for-bit identical to the frozen baseline seals:

| Component | Path / Reference | Cryptographic SHA-256 Digest | Status |
| :--- | :--- | :--- | :---: |
| **Engram v1.5.1 (Arm 3)** | `benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe` | `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` | **SEALED** |
| **Engram v1.5.4 (Arm 1)** | `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe` | `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` | **SEALED** |
| **Preregistration Seal** | `benchmarks/cost-baseline/PREREGISTRATION.md` | `db3ab2956a4da40aad8f1f376aa72a50bebc96f3e1ecc3120f405f0a294ea8bb` | **SEALED** |
| **Golden Reference Commit** | `angular-ngrx-material-starter` | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` | **LOCKED** |

---

## 8. Explicit Approval Checklist & Governance Sign-Off

> [!NOTE]
> **GOVERNANCE STATUS: APPROVED FOR BLOCK 08 ONLY**  
> This protocol amendment was explicitly approved on 2026-10-29.  
> It authorizes the execution of Stage 1 Block 08 only under the specified operational parameters.

```
========================================================================================
FORMAL PROTOCOL AMENDMENT 03 APPROVAL CHECKLIST (BLOCK 08 SCOPE)

Amendment Identifier:  AMENDMENT_03_TASK_B_OPERATIONAL_CEILING_CALIBRATION
Effective Scope:       task_b_remember_me, Block 08 Only
Parent Specification:  PREREGISTRATION.md (db3ab295...) & AMENDMENT_02

Human Decision Checklist:
[ X ] 1. APPROVAL OF 350,000-TOKEN CEILING:
         Approve calibrating the operational token ceiling to 350,000 tokens for
         Block 08 only, while maintaining the harness default of 250,000 tokens.

[ X ] 2. CONFIRMATION OF TIMEOUT & STOP-ON-FAILURE:
         Confirm retention of the 600-second per-arm timeout and mandatory
         stop-on-failure policy (prohibiting --no-stop-on-failure).

[ X ] 3. ACCEPTANCE OF BLOCK 06 HISTORICAL TREATMENT:
         Confirm that Block 06 remains permanently preserved as a terminated,
         unsuccessful attempt (not an accepted baseline, not in savings formulas,
         completion cost unknown).

[ X ] 4. AUTHORIZATION TO EXECUTE BLOCK 08:
         Explicitly authorize live execution of Stage 1 Block 08 under this
         amendment using command:
         python benchmarks/matched-pair-harness/src/orchestrator.py --block block_08 --max-tokens-ceiling 350000

Approver Signature:   Amal P S (User / Principal Investigator)
Approval Date:         2026-10-29
Final Decision:        [ X ] APPROVED (All 4 Items Checked)
                       [   ] REJECTED (Maintain 250,000 Ceiling for Block 08)
========================================================================================
```
