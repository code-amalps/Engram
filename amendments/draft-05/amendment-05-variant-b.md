# Amendment 05 — Variant B: Exploratory Task B Protocol

**Document Identifier:** `ENGRAM_AMENDMENT_05_VARIANT_B`  
**Parent Protocol:** [`benchmarks/AMENDMENT_04_STAGE1_TOKEN_EFFICIENCY_PROTOCOL_REVISION.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/AMENDMENT_04_STAGE1_TOKEN_EFFICIENCY_PROTOCOL_REVISION.md)  
**Status:** DRAFT PROPOSAL FOR OWNER REVIEW (Strictly Offline)

---

## 1. Rationale and Descriptive Objective

Historical benchmarking on `task_b_remember_me` across Blocks 06, 07, and 08 established that reaching functional completion ($C = 1.0$) within a restricted ceiling ($\le 175,000$ tokens) has a 0% empirical completion rate (0/3).

**Variant B** retains `task_b_remember_me` in its authentic, complex state, but formally re-scopes the execution as a **purely descriptive diagnostic evaluation**. The objective is to study agent exploration behavior, context inflation dynamics, and MCP tool adoption under controlled budget limits, without requiring task completion.

---

## 2. Parameter Specifications

| Parameter | Specification | Enforcement Mechanism |
| :--- | :--- | :--- |
| **Benchmark Task** | `task_b_remember_me` (Unmodified) | Pinned harness task configuration |
| **Per-Arm Token Ceiling ($T_{\text{max}}$)** | **`[OWNER TO SET]`** *(Informed by Task 1: Proposed $\le 175,000$)* | Hard subprocess tree termination |
| **Conservative Headroom Reserve ($R$)** | **`[OWNER TO SET]`** *(Informed by Task 1: Floor 15,000 to 30,000)* | Parameterized in `StreamingCodexRunner` |
| **Pre-Dispatch Stop Trigger** | Dynamic: $\text{Remaining} < \max(\Delta T_{\text{last}}, R)$ | Local pre-dispatch evaluation |
| **Expected Termination Window** | Turns 6–8 (~145,000 – 151,000 tokens) | Natural pre-dispatch headroom exhaustion |
| **Combined Block Exposure Limit** | **`[OWNER TO SET]`** *(Proposed: $2 \times T_{\text{max}}$)* | Two-arm matched pair with stop-on-failure |
| **Per-Arm Timeout** | **600 seconds** | Wall-clock timer |

---

## 3. Primary Exploratory Evaluation Metrics

Because functional completion ($C = 1.0$) is waived, performance is quantified exclusively through six descriptive telemetry metrics:

1. **Turns to First Edit:**  
   The turn number where the agent authoring the first code modification tool call (`tools.apply_patch`) is dispatched (or recorded as `NONE` if terminated during exploration).
2. **Tokens Consumed Before First Edit:**  
   Cumulative model tokens consumed across all turns strictly preceding the first edit attempt.
3. **Number of Exploration Calls Before First Edit:**  
   Count of file reads (`Get-Content`), searches (`rg`), and directory queries issued before the first edit.
4. **Engram MCP Calls and Their Timing:**  
   Exact turn number, latency, and frequency of `engram_inspect_symbol` or `engram_query_context` calls versus native shell exploration.
5. **Total Tokens and Final Task Outcome:**  
   Authoritative total model tokens ($T_{\text{model}}$) at execution end and final execution status (`PRE_DISPATCH_HEADROOM_EXHAUSTED`, `TOKEN_CEILING_BREACHED`, or `COMPLETED`).
6. **Threshold-Stop and Overrun Incidence:**  
   Whether the run stopped cleanly within the pre-dispatch headroom margin or experienced an in-flight final turn overrun (`OVERRUN_CENSORED`).

---

## 4. Formal Exclusion of Comparative Claims

> [!IMPORTANT]
> Under Variant B, **all comparative token-savings claims ($S$), efficiency claims, and claims of model superiority are EXPLICITLY OUT OF SCOPE AND FORBIDDEN**.
> 
> No comparative percentage savings may be published, claimed, or inferred. Data from both arms is analyzed strictly as right-censored exploratory observations.

---

## 5. Success and Failure Criteria

### 5.1 Success Criteria
1. **Telemetry Capture:** Authoritative `SESSION_ROLLOUT` telemetry is successfully extracted and verified for all executed arms.
2. **Deterministic Halting:** The pre-dispatch headroom guard halts execution cleanly upon headroom exhaustion without unhandled exceptions or harness crashes.
3. **Exploratory Data Generation:** All six primary metrics are fully populated with zero fabricated values.

### 5.2 Failure Criteria
1. **Harness Telemetry Defect:** Telemetry is corrupted, missing, or invalidated (`telemetry_valid == False`).
2. **Uncontrolled Overrun:** Child process fails to terminate upon guard activation, resulting in an unconstrained token burn.
3. **Golden Repository Mutation:** Golden repository HEAD commit or tracked files are mutated during execution.

---

## 6. Formal Ratification Block

```
================================================================================
AMENDMENT 05 — VARIANT B RATIFICATION
================================================================================

[   ] RATIFIED — Adopt Amendment 05 Variant B (Exploratory Task B Protocol)
      Target Task:                 task_b_remember_me (Unchanged)
      Per-Arm Token Ceiling:       [OWNER TO SET] tokens
      Headroom Reserve Floor:      [OWNER TO SET] tokens
      Maximum Block Exposure:      [OWNER TO SET] tokens
      Comparative Savings Claims:  EXPLICITLY OUT OF SCOPE (Forbidden)

[   ] REJECTED

Approver Signature: ___________________________________  Date: _________________
================================================================================
```
