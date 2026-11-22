# Engram Phase 5.9.0.4: Dynamic Ceiling Reconciliation & Arithmetic Resolution

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Phase:** 5.9.0.4 Workstream B  
**Execution Mode:** Strictly Offline, Read-Only Adversarial Audit  
**Target Invariant:** Task B Dynamic Ceiling $C_B = \min(1,500,000, 2,500,000 - T_A)$  
**Resolved Discrepancy:** The 22,768-Token (~23k) Conflation Artifact  
**Reconciliation Status:** `RECONCILED_AND_MATHEMATICALLY_CORRECTED`

---

## 1. Purpose

This deliverable provides the complete forensic reconciliation of the Task B dynamic ceiling formula and resolves the arithmetic discrepancy identified in Phase 5.9.0.3's scenario SC-04. It exposes the exact mathematical origin of the $1,477,232$-token ceiling and its associated $22,768$-token discrepancy, defines the authoritative dynamic ceiling formula, and establishes a single, internally consistent accounting model.

---

## 2. Reconstructing the Dynamic Ceiling Formula

The cross-process dynamic budget coordinator is designed around the invariant:
$$C_B = \min(1,500,000, \; 2,500,000 - T_A)$$

### 2.1 Term-by-Term Source Analysis
1. **$2,500,000$ (Aggregate Nominal Budget):** The combined ceiling for the two-invocation pilot (Task A + Task B).
2. **$1,500,000$ (Task B Nominal Ceiling):** The maximum nominal allocation for Task B.
3. **$T_A$ (Task A Actual Spending):** The actual cumulative tokens consumed by Invocation 1, extracted directly from `task_a_summary.json` (`cumulative_global_tokens`).
4. **Overshoot Treatment:** If Invocation 1 overshoots its $1,000,000$-token ceiling (e.g. consuming $1,020,000$ tokens), $T_A$ directly reflects this total. The coordinator formula subtracts $T_A$ from $2,500,000$, yielding $C_B = 1,480,000$. **The prior overshoot is already fully deducted and absorbed.**
5. **Reserve Deductions:** The coordinator does **not** deduct the $45,000$-token reserve. $C_B$ is passed directly to Invocation 2 as the gross global ceiling (`--global-ceiling $C_B`). The $45,000$ reserve is enforced locally inside Invocation 2 by `orchestrator.py:246` prior to dispatching each arm.

---

## 3. Dissecting the 23,000-Token Discrepancy in SC-04

Phase 5.9.0.3 presented contradictory figures for Scenario SC-04:
- The narrative report and dispatch proof stated:
  $$\text{Task B Dynamic Ceiling} = 1,477,232$$
  $$\text{Task B Prior Spending (Slots 1 & 2)} = 1,062,768$$
  $$\text{Gross Remaining} = 1,477,232 - 1,062,768 = 414,464$$
  $$\text{Usable Headroom} = 414,464 - 45,000 = \mathbf{369,464 \text{ tokens}}$$
- However, the accompanying CSV (`phase_5_9_0_3_candidate5_headroom_scenarios.csv`) recorded:
  $$\text{Gross Remaining} = 437,232$$
  $$\text{Usable Headroom} = \mathbf{392,232 \text{ tokens}}$$
- The difference between the two reported usable headrooms is:
  $$392,232 - 369,464 = \mathbf{22,768 \text{ tokens}}$$

### 3.1 The Forensic Origin of 22,768 Tokens
Why did Phase 5.9.0.3 use $1,477,232$ in its narrative?
1. Notice that:
   $$2,500,000 - 1,477,232 = \mathbf{1,022,768}$$
   $$1,022,768 - 1,000,000 = \mathbf{22,768}$$
   $$1,500,000 - 1,477,232 = \mathbf{22,768}$$
2. Now compare the maximum aggregate exposure figures from Phase 5.9.0.3:
   $$\text{Theoretical Maximum Exposure} = 2,562,768 \text{ tokens}$$
   $$\text{Empirical Maximum Exposure} = 2,540,000 \text{ tokens}$$
   $$\text{Difference} = 2,562,768 - 2,540,000 = \mathbf{22,768 \text{ tokens}}!$$
3. **The Root Cause:** In Phase 5.9.0.3, the author calculated the exposure difference between theoretical and empirical models ($22,768$ tokens) and mistakenly applied it as an overshoot deduction against Task A ($T_A = 1,000,000 + 22,768 = 1,022,768$), which yielded $C_B = 2,500,000 - 1,022,768 = 1,477,232$.
4. This was a **clerical conflation** between an aggregate cross-invocation delta and single-invocation usage! Neither Task A nor the coordinator ever produced a $22,768$ overshoot.

---

## 4. Reconciled SC-04 Models (The Two Valid Scenarios)

Because the $22,768$ artifact has no basis in code or runtime telemetry, SC-04 must be separated into its two genuine constituent scenarios:

### Sub-Scenario 4A: Task A Nominal, Task B Worst-Case Theoretical Overshoot
- **Task A Spending:** $T_A \le 1,000,000$ (Nominal completion).
- **Task B Dynamic Ceiling:** $C_B = \min(1,500,000, 2,500,000 - 1,000,000) = \mathbf{1,500,000 \text{ tokens}}$.
- **Task B Prior Usage (Slots 1 & 2):** $531,384 + 531,384 = \mathbf{1,062,768 \text{ tokens}}$.
- **Gross Remaining Before Slot 3:** $1,500,000 - 1,062,768 = \mathbf{437,232 \text{ tokens}}$.
- **Usable Headroom for Candidate 5:** $437,232 - 45,000 = \mathbf{392,232 \text{ tokens}}$.
- *(This matches the Phase 5.9.0.3 CSV calculation).*

### Sub-Scenario 4B: Task A Theoretical Overshoot, Task B Worst-Case Theoretical Overshoot
- **Task A Spending:** $T_A = 1,000,000 + 31,384 = \mathbf{1,031,384 \text{ tokens}}$ (Task A hits theoretical max overshoot).
- **Task B Dynamic Ceiling:** $C_B = \min(1,500,000, 2,500,000 - 1,031,384) = \mathbf{1,468,616 \text{ tokens}}$ (NOT $1,477,232$).
- **Task B Prior Usage (Slots 1 & 2):** $531,384 + 531,384 = \mathbf{1,062,768 \text{ tokens}}$.
- **Gross Remaining Before Slot 3:** $1,468,616 - 1,062,768 = \mathbf{405,848 \text{ tokens}}$.
- **Usable Headroom for Candidate 5:** $405,848 - 45,000 = \mathbf{360,848 \text{ tokens}}$.

**Conclusion:** Both valid sub-scenarios confirm that gross remaining tokens before Candidate 5 ($437,232$ and $405,848$) substantially exceed the $45,000$ reserve threshold. Candidate 5 is guaranteed dispatch under both.

---

## 5. Double-Counting Audit

We evaluated five potential double-counting vectors:
1. **Task A Overshoot Counted Twice:** In Phase 5.9.0.3, the $22,768$ delta was applied as an arbitrary deduction on Task B's ceiling while also maintaining $1,062,768$ prior spending on Task B. In the corrected model above, Task A overshoot is counted strictly once via $T_A$.
2. **Task B Prior Overshoot Counted Twice:** In Task B, Slot 1 and Slot 2 overshoots ($31,384 \times 2$) increment `cumulative_global_tokens` in `orchestrator.py`. They are counted once against the Task B ceiling.
3. **Double Reserve Deduction:** The reserve ($45,000$) is deducted strictly once ($H_u = G_{\text{rem}} - 45,000$).
4. **Dynamic Ceiling Reserve Deduction:** Verified that the coordinator formula does **not** deduct the reserve prior to passing the ceiling to `orchestrator.py`.
5. **Candidate 5 Ceiling as Additional Capacity:** Candidate 5's assigned ceiling is clamped to $\min(500,000, H_u)$ and remains strictly within the gross remaining headroom. No phantom tokens are created.
