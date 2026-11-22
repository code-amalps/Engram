# Engram Phase 5.9.0.3: Maximum Aggregate Token Exposure Reconciliation

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Phase:** 5.9.0.3 Workstream F  
**Execution Mode:** Strictly Offline, Read-Only Adversarial Audit  
**Prior Claimed Exposure (Phase 5.9.0.2):** $2,532,768$ tokens  
**Reconciled Defensible Theoretical Exposure:** $2,562,768$ tokens  
**Reconciled Defensible Empirical Exposure:** $2,540,000$ tokens  
**Status:** `RECONCILED_AND_MATHEMATICALLY_BOUNDED`

---

## 1. Purpose

This deliverable provides the final mathematical reconciliation of maximum aggregate token exposure for the two-invocation five-arm matched-pair benchmark pilot (Task A + Task B). It resolves the prior contradiction surrounding the 32,768-token overshoot figure and establishes an internally consistent, inspectable exposure model based on actual provider turn mechanics.

---

## 2. Forensic Audit of Prior Exposure Claims

In Phase 5.9.0.2, the maximum aggregate exposure was reported as:
$$\text{Nominal Aggregate Ceiling } (2,500,000) + 32,768 = 2,532,768 \text{ tokens}$$
This calculation was derived by assuming an overshoot of $16,384$ tokens per invocation ($16,384 \times 2 = 32,768$).

### 2.1 The Critical Flaw in the 32,768-Token Model
As established in Workstream B (`phase_5_9_0_3_overshoot_enforcement_audit.md`):
- $16,384$ tokens is strictly the provider's max *output completion burst limit* ($C_{\max}$).
- A provider request consists of **both prompt tokens ($P_k$) and completion tokens ($C_k$)**:
  $$T_k = P_k + C_k$$
- When an arm dispatches its final turn at the reserve threshold (where gross remaining headroom is exactly $R_{\text{inv}} = 45,000$ tokens), the prompt context can contain up to $60,000$ tokens ($P_k \le 60,000$).
- If the completion generates the maximum burst of $16,384$ tokens, total turn consumption is:
  $$T_{\text{turn}} = 60,000 + 16,384 = 76,384 \text{ tokens}$$
- The single-turn overshoot beyond the $45,000$ threshold is therefore:
  $$\Omega_{\max, \text{turn}} = 76,384 - 45,000 = 31,384 \text{ tokens}$$
- The empirical maximum single-turn overshoot observed during extensive Phase 5.4.24 offline benchmarking was $20,000$ tokens.

---

## 3. Invocation Concurrency & Cumulative Accounting Proof

A vital question addressed in this audit is: **Can multiple arms within the same invocation compound overshoots beyond the invocation ceiling?**

**Answer: No.**
The runtime enforces a single cumulative global counter within each invocation:
```python
cumulative_global_tokens += turn_usage.total_tokens
```
If Arm 1 overshoots its nominal ceiling by $15,000$ tokens, those $15,000$ tokens are immediately absorbed into `cumulative_global_tokens`. This directly reduces the gross remaining headroom available for Arm 2, Arm 3, etc.
Intermediate arms cannot produce an uncontained cumulative breach of the invocation ceiling; they simply starve subsequent arms earlier.
**Only the final executing arm in an invocation can breach the invocation ceiling.**

---

## 4. Formal Exposure Equation

The total aggregate exposure across the two-invocation benchmark is defined by:
$$T_{\text{maximum}} = T_{\text{nominal}} + \Omega_{\text{aggregate}}$$
where:
- $T_{\text{nominal}} = T_{\text{TaskA, nominal}} + T_{\text{TaskB, nominal}} = 1,000,000 + 1,500,000 = 2,500,000 \text{ tokens}$.
- $\Omega_{\text{aggregate}} = \Omega_{\text{TaskA}} + \Omega_{\text{TaskB}}$.

Because each invocation runs as an independent OS process, the final arm of Invocation 1 can overshoot by $\Omega_{\text{inv}}$, and the final arm of Invocation 2 can independently overshoot by $\Omega_{\text{inv}}$.

### 4.1 Theoretical Maximum Exposure Bound
$$\Omega_{\text{aggregate, theoretical}} = 31,384 \times 2 = 62,768 \text{ tokens}$$
$$T_{\text{maximum, theoretical}} = 2,500,000 + 62,768 = \mathbf{2,562,768 \text{ tokens}}$$

### 4.2 Empirical Maximum Exposure Bound (Phase 5.4.24 Demonstrated)
$$\Omega_{\text{aggregate, empirical}} = 20,000 \times 2 = 40,000 \text{ tokens}$$
$$T_{\text{maximum, empirical}} = 2,500,000 + 40,000 = \mathbf{2,540,000 \text{ tokens}}$$

---

## 5. Component-by-Component Budget Breakdown

The table below provides the full decomposition of aggregate exposure:

| Budget Component | Token Allocation | Enforcement Mechanism | Failure / Boundary Mode |
|---|---|---|---|
| **Task A Nominal Ceiling** | $1,000,000$ | `orchestrator.py:246` pre-dispatch gate | Reached when sum of arm usage $\ge 1,000,000$. |
| **Task A Protected Reserve** | $45,000$ | `MIN_ARM_RESERVE` threshold check | Refuses arm dispatch if gross balance $< 45,000$. |
| **Task A Final Turn In-Flight Burst** | $\le 76,384$ ($60\text{k}$ prompt $+ 16.384\text{k}$ compl.) | Single synchronous provider request limit | Provider output completion hard limit is $16,384$. |
| **Task A Maximum Overshoot** | $\le 31,384$ theoretical ($\le 20,000$ empirical) | Absorbed into Task A summary manifest | Transmitted to PowerShell coordinator. |
| **Cross-Process Dynamic Ceiling** | $\min(1,500,000, 2,500,000 - \text{Spent}_A)$ | PowerShell wrapper validation gate | Clamps Task B ceiling; fail-closed on error. |
| **Task B Nominal Ceiling** | $\le 1,500,000$ (down to $1,468,616$) | `orchestrator.py:246` pre-dispatch gate | Ensures combined nominal usage $\le 2,500,000$. |
| **Task B Protected Reserve** | $45,000$ | `MIN_ARM_RESERVE` threshold check | Protects final candidate/baseline arm. |
| **Task B Final Turn In-Flight Burst** | $\le 76,384$ ($60\text{k}$ prompt $+ 16.384\text{k}$ compl.) | Single synchronous provider request limit | Provider output completion hard limit is $16,384$. |
| **Task B Maximum Overshoot** | $\le 31,384$ theoretical ($\le 20,000$ empirical) | Invocation 2 final halt | Halts execution; benchmark complete. |
| **Retry & Failed Request Overhead** | Included in cumulative usage | `telemetry.py` records retry tokens | Deducted from active arm and global budget. |
| **Unobserved Provider Usage** | 0 tokens (in normal flow); Fail-closed on crash | `is_budget_uncertain` flag | Blocks Invocation 2 if crash occurs. |
| **Total Maximum Exposure (Theoretical)** | **2,562,768 tokens** | Two-layer local gate + provider burst limit | **Hard mathematical ceiling.** |
| **Total Maximum Exposure (Empirical)** | **2,540,000 tokens** | Two-layer local gate + demonstrated burst | **Observed ceiling in offline tests.** |

---

## 6. Audit Verdict on Maximum Exposure

1. The previous $2,532,768$ token claim is **formally rejected** as mathematically incomplete due to the omission of prompt context in the reserve boundary turn.
2. The corrected and certified maximum aggregate exposure is **$2,562,768$ tokens (theoretical)** and **$2,540,000$ tokens (empirical)**.
3. Because both bounds are finite, strictly inspectable, and enforced by synchronous single-request concurrency and two-layer pre-dispatch clamping, aggregate exposure is **proven to be bounded**.
