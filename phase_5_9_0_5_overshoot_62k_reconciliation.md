# Engram Phase 5.9.0.5: Resolution of the 62,768-Token Aggregate Overshoot Claim

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Phase:** 5.9.0.5 Workstream C  
**Execution Mode:** Strictly Offline, Read-Only Adversarial Audit  
**Prior Claim:** $2,500,000 + 62,768 = 2,562,768$ Tokens Aggregate Exposure  
**Audit Finding:** $62,768$ Overshoot Double-Counts Task A's In-Flight Overrun  
**Certified Conclusion:** `62_768_INVALID_DUE_TO_DOUBLE_COUNTING`

---

## 1. Purpose

This deliverable provides the definitive mathematical and code-level audit of the $62,768$-token aggregate overshoot claim asserted in Phases 5.9.0.3 and 5.9.0.4. It examines the interaction between single-turn in-flight overshoots and the cross-process dynamic ceiling formula, proves that adding both Task A and Task B overshoots double-counts Task A's overrun, simulates eight multi-arm overrun scenarios, and establishes the corrected aggregate exposure model.

---

## 2. Reconstruction of Original Exposure Figures

| Parameter / Figure | Formula / Derivation | True Nature / Origin | Formal Classification |
|---|---|---|---|
| **$31,384$ Single-Turn Overshoot** | $P_{\max} (60\text{k}) + C_{\max} (16.384\text{k}) - R_{\min} (45\text{k})$ | Calculated from empirical $60\text{k}$ prompt assumption | **Conditional Upper Bound** (Unenforced in code) |
| **$62,768$ Aggregate Overshoot** | $31,384 \text{ (Task A)} + 31,384 \text{ (Task B)}$ | Assumed linear compounding across invocations | **Invalid Due to Double-Counting** |
| **$2,562,768$ Theoretical Exposure** | $2,500,000 \text{ nominal} + 62,768$ | Derived from compounding model | **Invalid Compound Exposure** |
| **$2,540,000$ Prior Empirical Exposure**| $2,500,000 \text{ nominal} + 40,000 \text{ (20k + 20k)}$ | Assumed linear empirical compounding | **Invalid Compound Exposure** (Actual: $2,520,000$) |
| **$20,000$ Demonstrated Overshoot** | Observed max burst in Phase 5.4.24 | Maximum single-turn burst in offline test runs | **Historical Observed Maximum** |

---

## 3. Mathematical Proof of Double-Counting in $E_{\max} = B_G + O_A + O_B$

The previous phases evaluated total aggregate exposure using:
$$E_{\max} = B_G + O_A + O_B = 2,500,000 + 31,384 + 31,384 = 2,562,768 \text{ tokens}$$
where $O_A$ and $O_B$ represent the single-turn overshoots of Task A and Task B.

### 3.1 The Algebraic Flaw
Let:
- $B_A = 1,000,000$ (Task A nominal ceiling).
- $T_A = B_A + O_A = 1,031,384$ (Task A spending with overshoot $O_A$).
- The coordinator script dynamically sets Task B's ceiling:
  $$C_B = \min(B_B, \; B_G - T_A) = B_G - (B_A + O_A) = 1,500,000 - O_A$$
- Inside Task B, the orchestrator enforces ceiling $C_B$.
- When Task B completes, its final arm may overshoot $C_B$ by $O_B$:
  $$T_B = C_B + O_B = (1,500,000 - O_A) + O_B$$
- Now calculate the combined total spending ($T_{\text{total}}$):
  $$T_{\text{total}} = T_A + T_B = (B_A + O_A) + ((B_G - B_A - O_A) + O_B)$$
  $$T_{\text{total}} = B_G + O_B$$

Notice that **$O_A$ is eliminated entirely from the sum!**
Because Task B's ceiling was reduced by $O_A$, Task B absorbed Task A's overshoot. Adding $O_A$ again to the aggregate exposure on top of the dynamic ceiling counted $O_A$ **twice**: once as a deduction against Task B, and once as an expansion of the aggregate ceiling.

**Mathematical Law:**
Under the dynamic ceiling formula $C_B = \min(B_B, B_G - T_A)$, the maximum aggregate overshoot beyond $B_G = 2,500,000$ is **strictly $O_B$ alone ($31,384$ tokens conditional, $20,000$ tokens empirical)**, NOT $O_A + O_B$.

---

## 4. Simulation of the 8 Mandated Multi-Arm Scenarios

Pursuant to Section 6.3 of the directive, we simulate all eight execution cases:

| Case # | Scenario Description | Task A Usage ($T_A$) | Task B Ceiling ($C_B$) | Task B Prior Arm Spending (Slots 1 & 2) | Gross Remaining Before Slot 3 | Usable Headroom ($H_u$) | Candidate 5 Assigned Ceiling | Total Pilot Consumption | Aggregate Overrun Beyond $2.5\text{M}$ |
|---|---|---|---|---|---|---|---|---|---|
| **Case 1** | Task A nominal, Task B prior arms nominal | $1,000,000$ | $1,500,000$ | $1,000,000$ ($500\text{k} \times 2$) | $500,000$ | **$455,000$** | $455,000$ | $2,500,000$ | **$0$ tokens** |
| **Case 2** | Task A final overshoot, Task B prior arms nominal | $1,031,384$ | $1,468,616$ | $1,000,000$ ($500\text{k} \times 2$) | $468,616$ | **$423,616$** | $423,616$ | $2,500,000$ | **$0$ tokens** |
| **Case 3** | Task A nominal, Task B Slot 1 overshoot | $1,000,000$ | $1,500,000$ | $1,020,000$ ($520\text{k} + 500\text{k}$) | $480,000$ | **$435,000$** | $435,000$ | $2,500,000$ | **$0$ tokens** |
| **Case 4** | Task A nominal, Task B Slots 1 & 2 both overshoot | $1,000,000$ | $1,500,000$ | $1,062,768$ ($531.384\text{k} \times 2$) | $437,232$ | **$392,232$** | $392,232$ | $2,500,000$ | **$0$ tokens** |
| **Case 5** | Task A overshoot, Task B Slot 1 overshoot | $1,031,384$ | $1,468,616$ | $1,020,000$ ($520\text{k} + 500\text{k}$) | $448,616$ | **$403,616$** | $403,616$ | $2,500,000$ | **$0$ tokens** |
| **Case 6** | Task A overshoot, Task B Slots 1 & 2 both overshoot | $1,031,384$ | $1,468,616$ | $1,062,768$ ($531.384\text{k} \times 2$) | $405,848$ | **$360,848$** | $360,848$ | $2,500,000$ | **$0$ tokens** |
| **Case 7** | Candidate 5 is final executing arm & overshoots | $1,031,384$ | $1,468,616$ | $1,062,768$ | $405,848$ | **$360,848$** | $360,848$ | **$2,531,384$** | **$31,384$ tokens** |
| **Case 8** | Telemetry uncertain after Slot 1 or 2 | Any | Any | Unknown / Corrupted | Unknown | **$0$** | $0$ (`HALT`) | Halted safely | **$0$ tokens** |

### 4.1 Observations from the 8 Scenarios
1. In Cases 1 through 6, total pilot consumption **never exceeds $2,500,000$ tokens**. Intermediate arm overshoots in Slots 1 and 2 simply increment `cumulative_global_tokens`, reducing the remaining balance for subsequent arms without breaching the invocation ceiling.
2. Only in Case 7, where Candidate 5 is the final executing arm and generates a maximum in-flight turn burst at the reserve boundary, does the invocation ceiling breach, yielding an aggregate total of **$2,531,384$ tokens**.
3. In Case 8, corrupt or uncertain telemetry immediately triggers Gate 10, halting execution with zero aggregate breach.

---

## 5. Required Conclusion

Pursuant to Section 6.4 of the directive, we formally select:

# `62_768_INVALID_DUE_TO_DOUBLE_COUNTING`

### Formal Rationale:
The $62,768$-token figure ($31,384 \times 2$) assumed that Task A's overshoot and Task B's overshoot compound additively on top of the $2.5\text{M}$ budget. However, the dynamic coordinator formula $C_B = \min(B_B, B_G - T_A)$ explicitly deducts Task A's actual spending ($T_A = B_A + O_A$) from Task B's ceiling. This dynamically absorbs $O_A$. Consequently, only Task B's final turn ($O_B$) can breach the aggregate ceiling. The maximum aggregate overshoot is $31,384$ tokens (conditional) or $20,000$ tokens (empirical), and the $62,768$-token figure is mathematically invalid due to double-counting.
