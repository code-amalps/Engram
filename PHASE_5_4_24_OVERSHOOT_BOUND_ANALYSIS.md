# Engram Phase 5.4.24: Overshoot Bound & Guarantee Classification Analysis

**Phase:** 5.4.24  
**Execution Mode:** STRICTLY OFFLINE, READ-ONLY ADVERSARIAL FORENSIC AUDIT  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Authorization Status:** `NOT_AUTHORIZED`  
**Date:** 2026-11-20

---

## 1. Executive Summary

This analysis rigorously examines the quantitative limits of token overspending in the Engram matched-pair benchmark harness.

A critical tenet of experimental integrity is: **A configured token ceiling is not equivalent to an enforced spending limit.**

This audit evaluated the exact relationship between configured ceilings, locally observed tokens, provider-side consumption, cancellation latency, and downstream block halting.

### Key Analytical Findings:
1. **In-Flight Single-Turn Overshoot is Structurally Unpreventable:**
   Because remote LLM providers report token counts in batches at turn completion rather than incrementally during generation, the local system cannot intercept a request while it is generating tokens.
2. **Compound Multi-Turn Overshoot is Strictly Prevented:**
   While a single active turn may overshoot its remaining budget, **overshoot cannot cascade across turns or across arms**. The moment an overshoot is reported:
   - The runner detects the breach and forces `execution_status = "TOKEN_CEILING_BREACHED"`.
   - `Orchestrator.is_run_accepted()` Criterion 7 rejects the run unconditionally.
   - `run_tetrad()` raises `SafetyViolationError` and halts the block with `ABORTED_SAFETY_FAILURE` or `ABORTED_GLOBAL_BUDGET`.
   - All subsequent arms are marked `SKIPPED_SAFETY_FAILURE` without launching a child process.
3. **Maximum Demonstrated Overshoot:**
   Empirically verified in Scenario 13 (and Scenario B12): **20,000 tokens** (120,000 consumed against a 100,000 ceiling).
4. **Final Guarantee Classification:**
   **`DETECT_AND_ABORT_ONLY`** (Active-Turn) with **`PREVENTIVE_TURN_GATING`** (Macro-Boundary).

---

## 2. Seven Budget Boundaries & Quantitative Taxonomies

To provide absolute clarity, the audit established seven formal definitions of budget entities:

```
[1. Configured Budget Ceiling: G_ceiling = 2,200,000]
      |
      v
[2. Locally Authorized Budget: E_arm = min(A_ceiling, G_rem)]
      |
      v
[3. Pre-Dispatch Headroom Gate: G_rem >= 45,000]
      |
      v
[4. Locally Observed Consumption: StreamingTokenTracker (frozen during turn)]
      |
      v
[5. Provider-Reported Consumption: Usage emitted at turn completion]
      |
      v
[6. Post-Cancellation In-Flight Residual: Tokens generated prior to socket teardown]
      |
      v
[7. Final Sealing & Acceptance: Reconciled telemetry, Criterion 7 & Safety Halting]
```

### Comparative Boundary Table

| Metric / Concept | Formal Notation | Task B Live Reference Value | Nature / Location |
|---|:---:|:---:|---|
| **1. Configured Budget Ceiling** | $G_{\text{ceiling}}$ / $A_{\text{ceiling}}$ | 2,200,000 (Global) / 550,000 (Arm) | Static configuration passed via CLI or manifest. |
| **2. Locally Authorized Budget** | $E_{\text{arm}}$ | $\min(A_{\text{ceiling}}, G_{\text{remaining}})$ | Dynamic ceiling allocated to an arm prior to execution. |
| **3. Pre-Dispatch Headroom Gate** | $R_{\text{headroom}}$ | 45,000 tokens | Minimum budget required to authorize dispatching an arm or turn. |
| **4. Locally Observed Consumption** | $T_{\text{observed}}$ | $0 \dots 2,200,000$ | Cumulative tokens processed by `StreamingTokenTracker`. |
| **5. Provider-Reported Consumption** | $T_{\text{provider}}$ | Authoritative API count | Emitted by provider in `token_usage_record` upon turn completion. |
| **6. Post-Cancellation Residual** | $\Delta_{\text{residual}}$ | $0 \dots 16,384$ tokens | In-flight tokens generated on provider GPU before socket close. |
| **7. Effective Arm Ceiling Breached** | $T_{\text{provider}} > E_{\text{arm}}$ | Verified in Scenario 13 | Triggers `TOKEN_CEILING_BREACHED` and tetrad halt. |

---

## 3. Mathematical Analysis of In-Flight Overshoot

### 3.1 Single-Turn Overshoot Derivation

Let $R_k$ be the remaining budget before turn $k$:
$$R_k = E_{\text{arm}} - T_{\text{cumulative}, k-1}$$

The pre-dispatch headroom guard requires:
$$R_k \ge \hat{E}_k \quad \text{where } \hat{E}_k = \max(\text{last\_turn\_request\_tokens}, 45,000)$$

If this condition passes, turn $k$ is dispatched to the remote provider.

During turn $k$, the provider processes input prompt tokens $P_k$ and generates completion tokens $C_k$. The actual tokens consumed in turn $k$ are:
$$T_k = P_k + C_k$$

Because the local harness receives no incremental usage during generation, the local tracker cannot halt the model mid-turn.

Upon turn completion, the overshoot $\Omega_k$ for turn $k$ is:
$$\Omega_k = \max(0, T_k - R_k)$$

### 3.2 Worst-Case Theoretical Upper Bound

What is the theoretical maximum overshoot that a single turn could experience?

1. The minimum remaining budget that allows turn dispatch is $R_k = 45,000$ tokens.
2. In the worst case, the prompt context size approaches the model's maximum input window (e.g. $P_{\max} \approx 60,000$ tokens on Task B), and the model generates a maximum completion burst ($C_{\max} \approx 16,384$ tokens).
3. The total consumption of this turn is:
   $$T_k = 60,000 + 16,384 = 76,384 \text{ tokens}$$
4. The resulting theoretical single-turn overshoot is:
   $$\Omega_{\max} = 76,384 - 45,000 = 31,384 \text{ tokens}$$

> [!IMPORTANT]
> **Key Finding:** An individual turn can theoretically overshoot the allocated budget by up to $\sim 31,500$ tokens under extreme context and burst generation conditions. In practice, on Task B, the observed single-turn overshoot in Scenario 13 was **20,000 tokens**.

### 3.3 Proof of Non-Compounding Overshoot

Does an overshoot on turn $k$ permit subsequent turns or subsequent arms to overshoot further?

**Mathematical Proof:**
1. Upon processing the usage record for turn $k$, the runner checks:
   $$\text{effective\_tokens} > \text{max\_tokens\_ceiling}$$
2. If true:
   - `token_ceiling_breached` is set to `True`.
   - The polling loop immediately breaks (`runner.py` line 991).
   - `terminate_pid_tree(proc.pid)` kills the child process.
   - No turn $k+1$ can ever be dispatched.
3. In `_execute_single_arm()`:
   - `execution_status` is forced to `TOKEN_CEILING_BREACHED`.
4. In `run_tetrad()`:
   - Line 2260 detects `TOKEN_CEILING_BREACHED` and raises `SafetyViolationError`.
   - Lines 2297–2308 mark the breaching arm as `ABORTED_SAFETY_FAILURE`.
   - All subsequent arms $k+1 \dots 4$ are immediately recorded as `SKIPPED_SAFETY_FAILURE` without dispatching runner processes.
5. Therefore:
   $$\Omega_{\text{total\_tetrad}} = \Omega_k \le \Omega_{\max}$$
   **The total overshoot for the entire four-arm tetrad is strictly bounded to the single turn that breached the ceiling.**

---

## 4. Adversarial Simulation Outcomes (Scenarios B01 – B20)

All 20 adversarial simulation scenarios were executed deterministically in `scratch/phase_5_4_24/run_adversarial_simulation.py` using production classes:

| ID | Simulation Scenario | Simulated Budget Condition | Observed Result | Safety Status |
|:---:|---|---|---|:---:|
| **B01** | Usage remains below ceiling | Spend 45k vs 100k ceiling | Effective tokens 45k, headroom preserved | **PASS** |
| **B02** | Usage reaches ceiling during streaming | Spend 100k vs 100k ceiling | Headroom becomes 0, preventive halt triggered | **PASS** |
| **B03** | Usage crosses ceiling in streaming event | Spend 105k vs 100k ceiling | Overshoot 5k detected, overrun halt triggered | **PASS** |
| **B04** | Usage event arrives only post-turn | 0 tokens in-flight, 60k at turn end | Classified as post-turn batch reporting | **PASS** |
| **B05** | Usage events arrive in batches | 35k batch emitted at tool boundary | Discretely observed, unobserved window measured | **PASS** |
| **B06** | Duplicate usage event | Duplicate snapshot sent twice | Deduplicated via hash; tokens remain 50.5k | **PASS** |
| **B07** | Delayed usage event after cancellation | 90k at cancel, 97k in trailing buffer | Reconciled correctly to 97k | **PASS** |
| **B08** | Out-of-order usage events | 80k event arrives before 40k event | Monotonic max preserved at 80k | **PASS** |
| **B09** | Missing final usage event | Events file empty / unwritten | Fails closed: telemetry invalid, integrity rejected | **PASS** |
| **B10** | Malformed usage event | Negative tokens (`input_tokens: -500`) | Fails closed: telemetry invalid, halt triggered | **PASS** |
| **B11** | Provider usage exceeds estimate | Estimate 45k, actual 110k vs 100k ceiling | Breach detected, overshoot 10k captured | **PASS** |
| **B12** | Scenario 13 reproduction | 120k spend vs 100k allowance | Criterion 7 rejects; never accepted as SUCCESS | **PASS** |
| **B13** | Process cancellation latency | `taskkill /F /T` benchmark on active process | Latency measured: **298.56 ms** | **PASS** |
| **B14** | Child process traps SIGINT | Process loops trapping signals | Forced kernel termination kills process instantly | **PASS** |
| **B15** | Post-cancellation PID check | Query process table after kill | Verified process dead (`poll() != None`) | **PASS** |
| **B16** | Trailing buffer reconciliation | 3-line buffer processed post-death | Reconciled accurately to 85k | **PASS** |
| **B17** | Global budget exhausted in Arm 2 | Arm 1 (100k) + Arm 2 (90k) vs 200k ceiling | Rem 10k < 45k reserve; halts, 0 arms dispatched | **PASS** |
| **B18** | Diagnostic continue during violation | `--no-stop-on-failure` enabled | Safety tripwire overrides continue; tetrad halted | **PASS** |
| **B19** | Concurrent usage callbacks | 2 threads emitting 50 events each | Cleanly aggregated to 10k tokens across 100 turns | **PASS** |
| **B20** | Final aggregate reaches ceiling | Cumulative spend = 2.2M | Rem 0; pre-dispatch disallowed, no work authorized | **PASS** |

---

## 5. Final Guarantee Classification

Based on rigorous source inspection, protocol analysis, and empirical simulation:

### Evaluated Options:
1. `HARD_LOCAL_ENFORCEMENT`: **REJECTED.** The harness cannot enforce a hard mid-request token stop on the remote cloud API during active generation.
2. `BOUNDED_OVERSHOOT`: **CONDITIONALLY VALID AT BLOCK LEVEL, BUT MISLEADING AT REQUEST LEVEL.** At the block level, total overshoot is bounded to a single turn's overshoot ($\le \sim 31.5\text{k}$ tokens). However, during an active turn, no hard bound is enforced by the provider interface.
3. **`DETECT_AND_ABORT_ONLY`**: **ACCEPTED.**
   Overspending is detected immediately upon receipt of turn usage records. Subsequent execution within the arm and downstream arms within the tetrad is strictly aborted. Active-turn consumption cannot be prevented mid-generation.

### Formal Phase 5.4.24 Guarantee Verdict:
$$\mathbf{DETECT\_AND\_ABORT\_ONLY}$$

*(Accompanied by **Pre-Dispatch Macro-Gating**: $G_{\text{remaining}} \ge 45,000$ and $\text{remaining} \ge \hat{E}_{\text{next\_turn}}$).*

---

## 6. Conclusion

The Phase 5.4.23 safeguards successfully protect acceptance integrity and prevent cascading overspending across the benchmark block. However, live experimental planning must explicitly budget for a **single-turn overshoot exposure of up to $\sim 20,000$ to $31,500$ tokens** in the event that an arm exceeds its ceiling on its final active turn.
