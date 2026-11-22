# Engram Token Threshold Locations and Invariants Audit

**Document:** `amendments/draft-05/threshold-locations.md`  
**Purpose:** Comprehensive inventory of all hardcoded token threshold references (`250k`, `250,000`, `250_000`, `235k`, `500k`, etc.) across the Engram repository, classified by functional role, with freeze-set verification and mapping to the proposed Amendment 05 **175,000 ceiling** and **160,000 stop trigger**.  
**Safety Constraint:** **Strictly read-only inspection. NONE of these files may be modified under the proposed draft phase.**

---

## 1. Classification Overview

Threshold values in the repository serve four distinct roles:
1. **Config Value:** Active operational defaults in the harness execution runner or CLI parser.
2. **Derived Constant:** Mathematical or logical derivations based on threshold bounds (e.g. combined block exposure = $2 \times \text{ceiling}$; stop trigger = $\text{ceiling} - \text{reserve}$).
3. **Test Fixture:** Hardcoded boundary assertions or mock inputs in unit/regression test suites.
4. **Documentation:** Historical narratives, protocol amendments, preflight audits, and post-mortem investigations.

---

## 2. Operational Threshold Calibration Mapping (Amendment 04 vs. Decided Amendment 05)

Under Amendment 04, the operational ceiling was 250,000 with a 15,000 reserve, producing a 235,000 pre-dispatch stop trigger and 500,000 maximum block exposure.  
Under Decided Amendment 05 (Owner Decision 1), the parameters calibrate as follows:

| Parameter Role | Amendment 04 Value | Proposed Amendment 05 (Decided) | Operational Function |
| :--- | :--- | :--- | :--- |
| **Per-Arm Token Ceiling** | `250,000` tokens | **`175,000` tokens** | Hard process termination boundary (`token_ceiling_breached = True` if crossed). |
| **Pre-Dispatch Headroom Reserve** | `15,000` tokens | **`15,000` tokens (Inclusive)** | Safety buffer required before launching the next model turn. |
| **Pre-Dispatch Stop Trigger** | `235,000` tokens | **`160,000` tokens** | Trigger point where remaining headroom ($175\text{k} - \text{cumulative} \le 15\text{k}$) halts execution before dispatch (`PRE_DISPATCH_HEADROOM_EXHAUSTED`). |
| **Planned Block Exposure** | `500,000` tokens | **`350,000` tokens** | $2 \times \text{Per-Arm Ceiling}$ across two-arm candidate-first schedule. |

---

## 3. Inventory of Threshold Occurrences

### A. Active Harness Implementation (`config value` & `derived constant`)

> [!WARNING]
> These files define active runtime defaults. Under Draft Amendment 05, they **remain completely unmodified** until explicit owner ratification.

| File Path | Line(s) | Role | Matched Text / Context | Frozen Hash Set Status |
| :--- | :--- | :--- | :--- | :--- |
| `benchmarks/matched-pair-harness/src/orchestrator.py` | 85 | `config value` | `max_tokens_ceiling: int = 250_000,` (Default in `BenchmarkOrchestrator.__init__`) | Not frozen (Active working tree) |
| `benchmarks/matched-pair-harness/src/orchestrator.py` | 1318 | `config value` | `parser.add_argument("--max-tokens-ceiling", type=int, default=250_000, help="Per-arm token ceiling limit (default: 250,000)")` | Not frozen (Active working tree) |
| `benchmarks/matched-pair-harness/src/runner.py` | 235 | `config value` | `max_tokens_ceiling: int = 250_000,` (Default in `CodexRunner.__init__`) | Not frozen (Active working tree) |

*Note for Ratification:* Upon owner sign-off of Amendment 05, the default `--max-tokens-ceiling` in these locations would be parameterized to `175_000`, which automatically produces the `160_000` stop trigger when combined with the active `15_000` headroom reserve.

---

### B. Test Suites and Fixtures (`test fixture`)

> [!NOTE]
> Tests in `test_block07_remediation.py` are part of the frozen Block 07 post-mortem verification hash set and must not be altered.

| File Path | Line(s) | Role | Matched Text / Context | Frozen Hash Set Status |
| :--- | :--- | :--- | :--- | :--- |
| `benchmarks/matched-pair-harness/tests/test_block07_remediation.py` | 296, 298 | `test fixture` | `tracker.check_pre_dispatch_headroom(ceiling=250_000)` -> `self.assertEqual(rem, 250_000)` | **FROZEN** (`44E9F25F...`) |
| `benchmarks/matched-pair-harness/tests/test_block07_remediation.py` | 318, 324 | `test fixture` | Headroom check with 240,504 used against 250k ceiling | **FROZEN** (`44E9F25F...`) |
| `benchmarks/matched-pair-harness/tests/test_block07_remediation.py` | 346, 382 | `test fixture` | Headroom evaluation against 250k ceiling | **FROZEN** (`44E9F25F...`) |
| `benchmarks/matched-pair-harness/tests/test_block07_remediation.py` | 406, 483, 563 | `test fixture` | `max_tokens_ceiling=250_000` | **FROZEN** (`44E9F25F...`) |
| `benchmarks/matched-pair-harness/tests/test_block07_remediation.py` | 470 | `test fixture` | Expected error string: `"budget: 250,000"` | **FROZEN** (`44E9F25F...`) |
| `benchmarks/matched-pair-harness/tests/test_block07_remediation.py` | 557 | `test fixture` | True ceiling breach test docstring (`> 250,000`) | **FROZEN** (`44E9F25F...`) |
| `benchmarks/matched-pair-harness/tests/test_amendment_04_two_arm.py` | 5 | `test fixture` | "2. Per-arm token ceiling propagation (250,000 default)." | Not frozen (Active working tree) |
| `benchmarks/matched-pair-harness/tests/test_amendment_04_two_arm.py` | 91, 133 | `test fixture` | `max_tokens_ceiling=250_000` | Not frozen (Active working tree) |
| `benchmarks/matched-pair-harness/tests/test_amendment_04_two_arm.py` | 147, 152 | `test fixture` | Synthetic error message and `prompt_tokens_total: 250000` | Not frozen (Active working tree) |
| `benchmarks/matched-pair-harness/tests/test_amendment_04_two_arm.py` | 176, 181, 183 | `test fixture` | `max_tokens_ceiling=250_000` runner enforcement test | Not frozen (Active working tree) |
| `benchmarks/matched-pair-harness/tests/test_amendment_04_two_arm.py` | 191, 214 | `test fixture` | Two-arm runner ceiling propagation | Not frozen (Active working tree) |
| `benchmarks/matched-pair-harness/tests/test_amendment_04_two_arm.py` | 237, 254, 273 | `test fixture` | Mock telemetry with `prompt_tokens_total: 235000` (Amendment 04 trigger) | Not frozen (Active working tree) |
| `benchmarks/matched-pair-harness/tests/test_amendment_04_two_arm.py` | 325, 328, 331, 334, 337 | `test fixture` | CLI parsing test for `--max-tokens-ceiling 250000` | Not frozen (Active working tree) |
| `benchmarks/matched-pair-harness/tests/test_harness_safety_audit.py` | 5, 97, 125, 169, 175, 183, 197, 219, 237, 252, 270 | `test fixture` | Boundary testing at 249,999, 250,000, 250,001 | Not frozen (Active working tree) |
| `benchmarks/matched-pair-harness/tests/test_harness_safety_audit.py` | 207, 214 | `test fixture` | Single jump test from 50k to 500k tokens (`total_tokens: 500000`) | Not frozen (Active working tree) |
| `benchmarks/matched-pair-harness/tests/test_resilience_repair.py` | 74 | `test fixture` | `max_tokens_ceiling=250_000` | Not frozen (Active working tree) |

---

### C. Historical Artifacts & Documentation (`documentation`)

| File Path | Line(s) | Role | Matched Text / Context | Frozen Hash Set Status |
| :--- | :--- | :--- | :--- | :--- |
| `benchmarks/AMENDMENT_04_STAGE1_TOKEN_EFFICIENCY_PROTOCOL_REVISION.md` | 30, 37, 38, 103, 106, 136, 141, 142, 150, 153 | `documentation` | Ratified Amendment 04 defining 250k ceiling and 500k max planned block exposure | **FROZEN** (`03C4B278...`) |
| `benchmarks/AMENDMENT_04_STAGE1_TOKEN_EFFICIENCY_PROTOCOL_REVISION_DRAFT.md` | 25, 32, 33, 97, 99, 102, 131, 134, 135, 146, 147, 148, 161, 164, 176 | `documentation` | Preliminary draft of Amendment 04 | **FROZEN** (`516924EC...`) |
| `benchmarks/STAGE1_BLOCK07_COST_RISK_ANALYSIS.md` | 25, 30, 36, 38, 41, 46, 72, 74, 88, 113, 119, 120, 124, 131, 140, 152, 158, 160, 164, 184, 192 | `documentation` | Cost-risk analysis analyzing 250k vs 350k vs two-arm | **FROZEN** (`03E4E3EC...`) |
| `benchmarks/STAGE1_BLOCK07_FINAL_LIVE_REPORT.md` | 9, 10, 20, 24, 39, 40, 72, 121 | `documentation` | Record of candidate breach at 270,506 tokens against 250,000 ceiling | **FROZEN** (`325EF0EC...`) |
| `benchmarks/STAGE1_BLOCK07_POST_MORTEM_ROOT_CAUSE_INVESTIGATION.md` | 19, 156, 157, 161 | `documentation` | Root-cause analysis of 20,506 overrun beyond 250k ceiling | **FROZEN** (`EF34E71F...`) |
| `benchmarks/STAGE1_BLOCK07_REMEDIATION_MILESTONE_CLOSURE_REPORT.md` | 22, 138 | `documentation` | Closure report referencing 250k per-arm ceiling and 500k block exposure | **FROZEN** (`FA3C9B43...`) |
| `benchmarks/AMENDMENT_03_TASK_B_OPERATIONAL_CEILING_CALIBRATION.md` | 16, 27, 30, 43, 105, 107, 161, 180 | `documentation` | Proposed Amendment 03 (350k calibration proposal for Block 08) | Not frozen |
| `benchmarks/FINAL_OFFLINE_PREFLIGHT_MILESTONES_2_5_REPORT.md` | 30, 33, 82, 144, 150, 151, 202, 213, 214, 216, 224, 263, 279 | `documentation` | Milestones 2-5 preflight audit | Not frozen |
| `benchmarks/STAGE1_BLOCK06_HARDENED_RESULTS_REPORT.md` | 22 | `documentation` | Block 06 baseline termination at 253,748 vs 250,000 | Not frozen |
| `benchmarks/STAGE1_BLOCK06_LIVE_RESULTS_REPORT.md` | 21 | `documentation` | Unhardened harness overrun at 1,254,712 vs 250,000 | Not frozen |
| `benchmarks/STAGE1_BLOCK06_PREFLIGHT_REPORT.md` | 181 | `documentation` | Block 06 triad planned exposure (750k tokens) | Not frozen |
| `benchmarks/STAGE1_BLOCK06_RESILIENCE_FIX_REPORT.md` | 18, 38, 50, 59, 82, 199 | `documentation` | Implementation of 250k hard cutoff in runner | Not frozen |
| `benchmarks/STAGE1_BLOCK06_SAFETY_AUDIT_REPORT.md` | 18, 27, 64, 71, 74, 75, 87, 89, 211 | `documentation` | Formal verification of 250,000 boundary condition | Not frozen |
| `benchmarks/STAGE1_BLOCK08_FINAL_PREFLIGHT_REPORT.md` | 32, 34, 35, 52, 83, 89, 147 | `documentation` | Preflight verification of ceiling CLI overrides | Not frozen |
| `benchmarks/STAGE1_BLOCK08_PREFLIGHT_REPORT.md` | 14, 34, 44 | `documentation` | Block 08 preflight checks | Not frozen |
| `benchmarks/STAGE1_BLOCK08_PROTOCOL_DECISION_REVIEW.md` | 14, 17, 19, 20, 21, 24, 32, 33, 43, 45, 47, 49, 70, 84, 95, 107, 110, 115 | `documentation` | Review of 250k origin vs preregistration constraints | Not frozen |
| `benchmarks/STAGE1_SMOKE_TEST_AUDIT.md` | 233 | `documentation` | Token exposure calculations | Not frozen |
| `benchmarks/STAGE1_TASK_A_COMPLETE_5BLOCK_REPORT.md` | 25, 138 | `documentation` | Task A multi-turn token growth (up to 235k) | Not frozen |
| `benchmarks/STAGE1_TASK_A_POST_MORTEM_AND_TASK_B_READINESS.md` | 22, 64 | `documentation` | Context growth analysis (116k-235k tokens) | Not frozen |
| `benchmarks/STAGE1_TASK_B_BLOCK06_FORENSIC_ANALYSIS.md` | 17, 174, 185, 193, 209, 244 | `documentation` | Block 06 forensic investigation of 250k breach | Not frozen |
| `benchmarks/STAGE1_VERIFICATION_OFFLINE_REGRESSION_REPORT.md` | 197 | `documentation` | Regression testing of ceiling boundary | Not frozen |

---

### D. Historical Run Records (`test fixture` / `historical output`)

| File Path | Line(s) | Role | Matched Text / Context | Frozen Hash Set Status |
| :--- | :--- | :--- | :--- | :--- |
| `benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/triad_summary.json` | 190 | `historical output` | `"Execution terminated: Token ceiling exceeded (253,748 > 250,000 tokens)"` | Historical artifact |
| `benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/baseline_arm1/run_telemetry.json` | 9 | `historical output` | `"Execution terminated: Token ceiling exceeded (253,748 > 250,000 tokens)"` | Historical artifact |
| `benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/triad_summary.json` | 189 | `historical output` | `"Execution terminated: Token ceiling exceeded (270,506 > 250,000 tokens)"` | Historical artifact |
| `benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/v154_arm3/run_telemetry.json` | 9 | `historical output` | `"Execution terminated: Token ceiling exceeded (270,506 > 250,000 tokens)"` | Historical artifact |

---

## 4. Implementation Guidance for Ratification

When Amendment 05 is formally signed off by the owner:
1. **Config Update:** Update default parameter in `orchestrator.py` (lines 85, 1318) and `runner.py` (line 235) to `--max-tokens-ceiling 175000`.
2. **Headroom Reserve Verification:** Maintain `--headroom-reserve 15000`, ensuring the pre-dispatch stop trigger automatically evaluates at $175,000 - 15,000 = 160,000$ cumulative tokens.
3. **Immutable History:** Frozen artifacts (`AMENDMENT_04_...`, `STAGE1_BLOCK07_...`, `test_block07_remediation.py`) must **NEVER** be updated.
