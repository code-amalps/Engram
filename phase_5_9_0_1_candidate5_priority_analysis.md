# Engram Phase 5.9.0.1: Candidate 5 Priority Analysis & Headroom Guarantee Remediation

**Phase:** 5.9.0.1  
**Candidate:** Candidate 5 (Phase 5.5 Seven-Optimization Context Engine + Phase 5.8.1 Cache Remediation)  
**Task Under Analysis:** Task B (`task_b_remember_me`)  
**Scope:** Forensic Review of Execution Priority vs Completion Guarantees  

---

## 1. Remediation of Unsupported Completion Claims

The Phase 5.9.0 report contained the following claim:
> *"Placing Candidate 5 in Slot 3 guarantees Candidate 5 executes and collects complete telemetry even if Candidate 3 subsequently halts on headroom exhaustion."*

### 1.1 Forensic Correction
This statement conflates **execution priority** with an **unsupported completion guarantee**:
1. **Execution Priority is Guaranteed:** Under seed `59002`, Candidate 5 is deterministic in **Slot 3**, scheduled strictly before Candidate 3 in **Slot 4**. Candidate 5 will begin execution before Candidate 3 consumes any tokens in Task B.
2. **Completion is NOT Guaranteed:** Candidate 5's execution and completion depend on:
   - Whether Slot 1 (`arm_a_native`) and Slot 2 (`arm_c_cand42_always_on`) leave sufficient headroom to dispatch Candidate 5.
   - Whether remaining headroom allows Candidate 5 to execute all required turns without hitting the pre-dispatch headroom guard ($H_{\text{rem}} < 45,000$).
   - Whether Candidate 5 itself converges within its per-arm limits (25 turns, $500,000$ tokens).

---

## 2. Mathematical Headroom Analysis for Task B Candidate 5

In Task B, the global invocation ceiling is $B_{\text{inv, B}} = 1,500,000\text{ tokens}$ with a headroom reserve of $R = 45,000\text{ tokens}$.

The execution sequence is:
- **Slot 1:** `arm_a_native` (Ceiling: $500,000$)
- **Slot 2:** `arm_c_cand42_always_on` (Ceiling: $500,000$)
- **Slot 3:** `arm_e_cand5_optimized` (Ceiling: $500,000$)
- **Slot 4:** `arm_b_cand3` (Ceiling: $500,000$)
- **Slot 5:** `arm_d_cand42_adaptive` (Ceiling: $500,000$)

### 2.1 Headroom Available at Candidate 5 Dispatch (Slot 3)

Let $T_1$ be the tokens consumed by Slot 1, and $T_2$ be the tokens consumed by Slot 2.
The headroom available upon reaching Slot 3 is:
$$H_3 = B_{\text{inv, B}} - (T_1 + T_2)$$

#### Case A: Maximum Saturated Consumption of Slots 1 & 2
If both Native and Candidate 4.2 Always-On run to their full per-arm ceilings:
$$T_1 = 500,000, \quad T_2 = 500,000 \implies T_1 + T_2 = 1,000,000\text{ tokens}$$
$$H_3 = 1,500,000 - 1,000,000 = 500,000\text{ tokens}$$

Because $H_3 = 500,000 \ge R_{\text{reserve}} (45,000)$, **Candidate 5 dispatch is guaranteed to occur**, even under maximum saturation of Slots 1 and 2!

#### Case B: In-Flight Response Overshoot on Slots 1 & 2
If both preceding arms experience the maximum single-turn overshoot bound ($16,384\text{ tokens}$ each):
$$T_1 + T_2 = 1,000,000 + (2 \times 16,384) = 1,032,768\text{ tokens}$$
$$H_3 = 1,500,000 - 1,032,768 = 467,232\text{ tokens}$$

Because $467,232 \ge 45,000$, dispatch remains guaranteed.

---

## 3. Minimum Headroom Required for Candidate 5 Completion

To complete a task run, Candidate 5 requires:
$$H_3 \ge T_{\text{cand5\_actual}} + R_{\text{reserve}}$$

Where:
- $T_{\text{cand5\_actual}}$ is the actual total tokens Candidate 5 will consume to solve Task B.
- $R_{\text{reserve}} = 45,000\text{ tokens}$.

### 3.1 Empirical Upper-Bound Limitations
- In synthetic benchmarks, Candidate 5 demonstrated a $68.2\%$ token reduction against Candidate 3.
- In historical Live 02 benchmarks, Task B consumed between $253,748$ tokens (Native) and $512,449$ tokens (Cand 3 halted).
- If Candidate 5 consumes its projected nominal budget of $\approx 150,000$ tokens:
  $$\text{Required Headroom } = 150,000 + 45,000 = 195,000\text{ tokens}$$
  Under worst-case preceding consumption ($H_3 = 467,232$), $467,232 \ge 195,000$, and Candidate 5 completes.
- **HOWEVER:** Because Candidate 5 has never run against live provider APIs on Task B, **no empirical upper-bound exists for actual live Candidate 5 consumption on Task B**.

### 3.2 Formal Protocol Finding
If Candidate 5 requires $> 455,000$ tokens (or encounters an unexpected tool loop), it will halt on the headroom reserve before finishing.

Therefore, the protocol certifies:
1. **Dispatch Guarantee:** Candidate 5 is guaranteed to be dispatched in Slot 3 with at least $467,232$ tokens of remaining headroom.
2. **Priority Protection:** Candidate 5 is fully insulated from Candidate 3's historical $>500,000$-token budget exhaustion.
3. **No Completion Guarantee:** Candidate 5 **cannot be guaranteed to complete** because live performance under real provider reasoning and model non-determinism cannot be proven offline.
