# Milestone 5B-Prep: Synthetic Guard Test Results

## 1. Overview and Test Objectives

This document presents empirical test evidence from synthetic stimulation of the pre-dispatch token headroom safety monitor under the 45,000-token reserve and 175,000-token per-arm ceiling configuration.

All tests were executed strictly offline using synthetic event streams, without initiating external inference or network calls.

---

## 2. Test Scenarios and Execution Matrix

| Scenario ID | Description | Cumulative Tokens ($U_{\text{pre}}$) | Ceiling ($C$) | Reserve ($R$) | Remaining ($C - U$) | Expected Decision | Observed Decision | Verification Verdict |
|---|---|---|---|---|---|---|---|---|
| **SG-01** | Well below stop threshold | 50,000 | 175,000 | 45,000 | 125,000 | ALLOW_DISPATCH | ALLOW_DISPATCH | **PASS** |
| **SG-02** | Intermediate execution | 100,000 | 175,000 | 45,000 | 75,000 | ALLOW_DISPATCH | ALLOW_DISPATCH | **PASS** |
| **SG-03** | Boundary allowed dispatch | 129,999 | 175,000 | 45,000 | 45,001 | ALLOW_DISPATCH | ALLOW_DISPATCH | **PASS** |
| **SG-04** | Exact equality stop boundary | 130,000 | 175,000 | 45,000 | 45,000 | ALLOW_DISPATCH | ALLOW_DISPATCH | **PASS** |
| **SG-05** | First blocked token level | 130,001 | 175,000 | 45,000 | 44,999 | PREVENTIVE_HALT | PREVENTIVE_HALT | **PASS** |
| **SG-06** | Deep into reserve | 140,000 | 175,000 | 45,000 | 35,000 | PREVENTIVE_HALT | PREVENTIVE_HALT | **PASS** |
| **SG-07** | Exhausted budget | 175,000 | 175,000 | 45,000 | 0 | PREVENTIVE_HALT | PREVENTIVE_HALT | **PASS** |
| **SG-08** | In-flight ceiling overrun | 180,000 | 175,000 | 45,000 | -5,000 | OVERRUN_HALT | OVERRUN_HALT | **PASS** |
| **SG-09** | Worst-case turn jump | 130,000 + 37,651 | 175,000 | 45,000 | 7,349 | WITHIN_CEILING | 167,651 < 175k | **PASS** |
| **SG-10** | Duplicate event injection | 1,200 (x2) | 175,000 | 45,000 | 173,800 | NO_INFLATION | 1,200 | **PASS** |
| **SG-11** | Negative token injection | -500 | 175,000 | 45,000 | N/A | FAIL_SAFE_HALT | FAIL_SAFE_HALT | **PASS** |
| **SG-12** | Missing usage field | Missing | 175,000 | 45,000 | N/A | FAIL_SAFE_HALT | FAIL_SAFE_HALT | **PASS** |
| **SG-13** | Delayed snapshot after deltas| Deltas: 10k, Snap: 12k| 175,000 | 45,000 | 163,000 | RECONCILED_MAX | 12,000 | **PASS** |
| **SG-14** | Zero ceiling configuration | 0 | 0 | 45,000 | 0 | FAIL_SAFE_HALT | FAIL_SAFE_HALT | **PASS** |
| **SG-15** | None ceiling configuration | None | None | 45,000 | N/A | FAIL_SAFE_HALT | FAIL_SAFE_HALT | **PASS** |

---

## 3. Analysis of Critical Scenarios

### SG-04 & SG-05 (The 130,000 Stop Boundary)
At $U_{\text{pre}} = 130,000$, the remaining budget is $175,000 - 130,000 = 45,000$.
Because the conservative reserve is exactly $45,000$, $\text{remaining} \ge \text{estimated}$ evaluates to `True` ($45,000 \ge 45,000$).
At $U_{\text{pre}} = 130,001$, the remaining budget is $44,999$.
Because $44,999 < 45,000$, the monitor halts immediately before dispatch.
This verifies that $130,000$ functions as the precise stop threshold.

### SG-09 (The Turn 9 Worst-Case Delta Containment)
When an agent is allowed to execute from $U_{\text{pre}} = 130,000$, the worst-case single turn jump observed in previous trials ($37,651$ tokens) brings total consumption to:
$$130,000 + 37,651 = 167,651 \text{ tokens}$$
This remains $7,349$ tokens below the $175,000$ ceiling, empirically demonstrating the effectiveness of the $45,000$ planning reserve.

---

## 4. Conclusion

The synthetic guard test battery confirms that the remediated token tracker operates deterministically, fails safe on malformed or invalid inputs, and guarantees ceiling compliance under worst-case historical token consumption patterns.
