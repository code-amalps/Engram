# Engram Task B — Block 08 Protocol Decision Review (Offline Analysis)

**Document ID:** `STAGE1_BLOCK08_PROTOCOL_DECISION_REVIEW`  
**Date:** 2026-10-29  
**Review Target:** `benchmarks/STAGE1_BLOCK08_PREFLIGHT_REPORT.md`  
**Referenced Specifications:**
- Parent Specification: [`benchmarks/cost-baseline/PREREGISTRATION.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/PREREGISTRATION.md) (SHA-256: `db3ab295...`)
- Active Triad Protocol: [`benchmarks/AMENDMENT_02_STAGE1_CORE_TRIAD.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/AMENDMENT_02_STAGE1_CORE_TRIAD.md)
- Incident Forensic Report: [`benchmarks/STAGE1_TASK_B_BLOCK06_FORENSIC_ANALYSIS.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/STAGE1_TASK_B_BLOCK06_FORENSIC_ANALYSIS.md)
- Harness Orchestrator: [`benchmarks/matched-pair-harness/src/orchestrator.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py)

---

## 1. Origin of the 250,000-Token Ceiling: Preregistered Constraint vs. Operational Limit

### Forensic Finding
- **In `PREREGISTRATION.md`:** The string `250,000`, `250000`, or any fixed numeric token ceiling **does not appear anywhere in the ratified parent specification**. `PREREGISTRATION.md` defines hypothesis structures, metric separation rules, test acceptance gates, replacement caps for infrastructure crashes, and cumulative accounting invariants, but sets no numeric token cutoff.
- **In `AMENDMENT_02_STAGE1_CORE_TRIAD.md`:** Similarly, Amendment 02 contains **no fixed numeric token ceiling**. It defines the 3-arm Latin-square schedule, sample size (10 blocks / 30 runs), primary outcomes ($\Delta QAC$), and the rule that $C < 1.0$ invalidates savings claims, but specifies no token limit.
- **Actual Origin:** The 250,000-token ceiling was introduced as an **operational circuit breaker** in the test harness software:
  1. [`orchestrator.py` (line 1268)](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L1268): `parser.add_argument("--max-tokens-ceiling", type=int, default=250_000, help="Per-arm token ceiling limit (default: 250,000)")`.
  2. [`runner.py` (line 138)](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L138): `def __init__(..., max_tokens_ceiling: int = 250_000)`.
  3. Documented in `benchmarks/STAGE1_SMOKE_TEST_AUDIT.md` and `benchmarks/STAGE1_BLOCK06_RESILIENCE_FIX_REPORT.md` as an engineering safeguard to terminate runaway multi-turn loops.

**Verdict:** The 250,000 ceiling is a **configurable operational circuit breaker** in the execution harness, NOT a mathematically frozen constraint in the preregistration document.

---

## 2. Amendment Implications of Calibrating the Ceiling to 350,000 Tokens

### Methodological & Scientific Necessity
While modifying a CLI parameter does not alter the text of `PREREGISTRATION.md`, **scientific rigor requires a formal protocol amendment (Amendment 03)** for the following reasons:
1. **Asymmetric Stopping Boundary Avoidance:** Block 06 was executed under an explicit authorization enforcing a 250,000 ceiling. If subsequent blocks allow 350,000 tokens without formal documentation, baseline arms would be subjected to unequal stopping conditions across the block series.
2. **Pre-hoc Justification:** The Block 06 forensic analysis established that unassisted exploration of a 68,000-file Angular workspace accumulates ~250k prompt tokens across 9 turns purely from file dumps and conversational history re-ingestion. Sizing the operational ceiling to 350,000 provides the ~2–3 additional turns needed for an unassisted baseline to run `npx tsc` and resolve syntax imports.
3. **Audit Trail Integrity:** Transparently documenting this calibration via a sealed Amendment 03 ensures external reproducibility and prevents accusations of post-hoc parameter tweaking.

### Validity of Outcomes Under a 350,000-Token Calibration
- **Within-Block Pairwise Comparisons ($\Delta QAC_{\text{Arm3 vs Arm1}}$):** 100% Valid. All arms within the block execute under the identical 350,000 ceiling.
- **Task A Blocks (Blocks 01–05):** 100% Valid. Task A runs consume <30k tokens; the ceiling is never approached.
- **Block 06 Historical Evidence:** Remains fully valid and preserved as empirical proof of the baseline context explosion under a tighter boundary.

---

## 3. Token Exposure Analysis: 350,000 vs. 250,000

| Scope | 250,000 Ceiling | 350,000 Ceiling | Maximum Additional Permitted Exposure |
| :--- | :---: | :---: | :---: |
| **Per Arm (Single Execution)** | 250,000 | 350,000 | **+100,000 tokens** |
| **Per Block (3 Arms Scheduled)** | 750,000 | 1,050,000 | **+300,000 tokens** |
| **All Task B Blocks (Blocks 06–10, 15 Runs)** | 3,750,000 | 5,250,000 | **+1,500,000 tokens** |

> [!IMPORTANT]
> **Strict Exposure Labeling:**  
> These numbers represent the **maximum permitted upper-bound exposure before circuit-breaker termination**, NOT predicted actual usage.  
> - For **Arm 3 (`v154_arm3`)**, predicted usage is **~30,000–60,000 tokens**, utilizing <20% of either ceiling.  
> - Only an unassisted baseline agent exploring multiple files is expected to approach the 280,000–320,000 token range to reach completion.

---

## 4. Scientific Value of a Standalone Successful v1.5.4 Run

In Block 08, the Latin-square order is:
$$\text{Order 1: } \mathbf{v154\_arm3} \longrightarrow \text{Order 2: } \mathbf{baseline\_arm1} \longrightarrow \text{Order 3: } \mathbf{v151\_arm2}$$

### What a Standalone v1.5.4 Run Answers
1. **Feasibility on Complex Multi-File Tasks:** Proves whether Engram v1.5.4 can achieve 100% functional acceptance ($C = 1.0$) on `task_b_remember_me`.
2. **Turn-0 Context Injection Verification:** Directly tests whether Turn-0 injection eliminates the 6-turn exploratory file-dump cycle observed in baseline.
3. **Absolute Token Expenditure:** Establishes the empirical numerator for efficiency claims ($T_{\text{v1.5.4}}$). If $T_{\text{v1.5.4}} \approx 40,000$ tokens, it proves that Engram completed Task B using less than one-sixth of the tokens baseline consumed before failing.

### What It Cannot Answer (Methodological Limitation)
- Under Section 6 of Amendment 02, if `baseline_arm1` subsequently breaches the 250,000 ceiling, its verdict is `CONTROL_ACCEPTANCE_FAILURE`.
- **Comparative savings percentages ($S = \frac{T_{base} - T_{engram}}{T_{base}}$) are strictly suppressed (`null`).**
- Therefore, a standalone v1.5.4 run provides **authoritative descriptive and diagnostic evidence**, but **cannot calculate a formal comparative savings delta** unless baseline also achieves $C = 1.0$.

---

## 5. Comparison Rules & Stop-on-Failure Mechanics

- **Stop-on-Failure Implementation ([`orchestrator.py` lines 985–996](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L985-L996)):**
  - If any arm fails (`execution_status != "SUCCESS"` or `telemetry_valid == False`), execution halts immediately.
  - Remaining unexecuted arms receive `SKIPPED_STOP_ON_FAILURE` records.
- **Behavior in Block 08:**
  - Arm 1 (`v154_arm3`) executes first.
  - If Arm 1 succeeds, Arm 2 (`baseline_arm1`) executes next.
  - If Arm 2 breaches the 250k ceiling, stop-on-failure halts the triad, skipping Arm 3 (`v151_arm2`).
- **Policy on `--no-stop-on-failure`:**
  - Disabling stop-on-failure would permit Arm 3 (`v151_arm2`) to run even after baseline fails, allowing a pairwise comparison between v1.5.4 and v1.5.1.
  - However, in accordance with explicit user instructions, `--no-stop-on-failure` is **not recommended and not permitted**. Stop-on-failure remains active.

---

## 6. Strategic Options Evaluation

| Option | Description | Scientific Value | Quota Risk | Recommendation |
| :--- | :--- | :---: | :---: | :---: |
| **Option A: Run Block 08 Unchanged** (Ceiling = 250,000, Stop-on-Failure = True) | Executes `v154_arm3` first. If v1.5.4 succeeds and baseline breaches at ~250k, triad halts. | **High Descriptive Value:** Obtains clean, isolated v1.5.4 metrics on Task B. Comparative claims suppressed if baseline fails. | **Low:** Capped at 250k per arm. | **VIABLE (Fastest)** |
| **Option B: Formal Ceiling Amendment (Amendment 03)** | Drafts Amendment 03 calibrating Task B ceiling to 350,000 tokens for review before execution. | **High Comparative Value:** Maximizes likelihood that both baseline and v1.5.4 achieve $C = 1.0$, enabling valid comparative savings claims. | **Moderate:** Permitted exposure up to 350k per arm. | **RECOMMENDED (Most Rigorous)** |
| **Option C: Redesign Comparison Protocol** | Changes Task B prompt, task criteria, or evaluation gates. | **Low:** Disrupts preregistered continuity; invalidates prior baselines. | **High Overhead** | **REJECTED** |

---

## 7. Decision Memo & Recommendation

### Recommendation
**Adopt a two-step scientific progression:**

1. **Primary Recommendation (Option B — Most Rigorous for Comparative Claims):**  
   If the goal is to obtain a **formally claimable comparative savings delta ($S$)** between Engram and baseline in Block 08, prepare **Amendment 03 (Task B Operational Ceiling Calibration)** to raise the Task B per-arm ceiling from 250,000 to **350,000 tokens** for human review and authorization prior to execution.

2. **Alternative Recommendation (Option A — Fastest for Treatment Verification):**  
   If the user wishes to proceed immediately without drafting an amendment, **run Block 08 unchanged** (`--block block_08` with the existing 250,000 ceiling). This will immediately capture clean, empirical performance data for Engram v1.5.4 on Task B, with the explicit understanding that if baseline breaches the 250k ceiling, comparative savings claims will be strictly suppressed under the preregistered acceptance rules.

### Required Authorization
Awaiting your explicit selection between:
- **Path 1:** Authorize drafting **Amendment 03** (350,000 token ceiling for Task B) for review.
- **Path 2:** Authorize execution of **Block 08 unchanged** (250,000 token ceiling, Arm 3 runs first).
