# Amendment 05 (Draft v2): Exploratory Diagnostic Protocol & Early-Exit Guard

**Document Identifier:** `ENGRAM_AMENDMENT_05_DRAFT_V2`  
**Parent Protocol:** [`benchmarks/AMENDMENT_04_STAGE1_TOKEN_EFFICIENCY_PROTOCOL_REVISION.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/AMENDMENT_04_STAGE1_TOKEN_EFFICIENCY_PROTOCOL_REVISION.md)  
**Governing Architecture:** Variant B (Exploratory Diagnostic Protocol on Task B)  
**Status:** DRAFT PROPOSAL FOR OWNER REVIEW — REQUIRES OWNER CONFIRMATION BEFORE ADOPTION  

---

## 1. Objective and Empirical Rationale

Historical live benchmarking of `task_b_remember_me` across Blocks 06, 07, and 08 demonstrated that:
1. Reaching end-to-end task completion ($C = 1.0$) within a restricted 175,000 ceiling has a **0% empirical completion rate (0/3)**.
2. Initial codebase exploration across Angular sibling directories consumes between **145,999 and 270,506 tokens** before an agent attempts its first code edit.
3. Late-stage verification and interactive tool polling loops burn **26,000 to 37,651 tokens per turn**, creating extreme budget overrun risks.

**Amendment 05 Draft v2** establishes a controlled, low-risk **Exploratory Diagnostic Pilot**. It bounds execution strictly to the exploration and initial implementation phase, collecting rich comparative telemetry while capping model consumption well before late-stage turn inflation can occur.

> [!WARNING]
> **Sample Size & Generalizability Limitation:**  
> All empirical baselines in this document are derived from a small sample size ($N = 3$ historical live runs). Observed metrics are strictly **directional** and do not constitute a statistically powered proof of general model behavior across other tasks or codebases.

---

## 2. Dynamic Guard Architecture & Hard Cap Boundaries

### 2.1 Dynamic Headroom Estimator
Rather than relying on a static cumulative threshold, the harness pre-dispatch guard implements a **dynamic cost estimator** ([`runner.py` lines 199–218](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L199-L218)):
```python
estimated_next_turn = max(last_turn_request_tokens, conservative_reserve)
remaining_headroom = max_tokens_ceiling - current_cumulative_tokens
has_headroom = remaining_headroom >= estimated_next_turn
```
Execution is halted prior to dispatching any request if remaining headroom is insufficient to absorb the estimated cost of the next turn.

### 2.2 Realistic Concurrency Boundary (No Guaranteed Hard Cap)
The test harness operates as an external parent Python process polling the child Codex process at 50 ms intervals:
- If Codex dispatches an HTTP inference request to OpenAI across a tool return in $< 50\text{ms}$ before the runner's polling loop reads the event, that request is in flight across the internet.
- Terminating the child process halts local execution, but cannot cancel or recall an in-flight server-side inference.
- Therefore, **the external guard cannot guarantee a zero-leakage hard cap**. A buffer is mandatory to prevent ceiling breaches.

---

## 3. Parameter Specifications and Proposed Reserve Sizing

Informed by historical turn-size statistics ($N=20$ turns: Median $= 28,163$; $P_{90} = 34,867$; Maximum $= 37,651$ tokens), the reserve is parameterized for owner selection:

| Parameter | Specification / Value | Status |
| :--- | :--- | :--- |
| **Operational Token Ceiling ($T_{\text{max}}$)** | **175,000 tokens** | Parameterized |
| **Headroom Reserve ($R$)** | **`[OWNER TO SET]`** | Proposed options below |
| **Combined Block Exposure Limit** | **350,000 tokens maximum** | Two arms ($2 \times 175\text{k}$) with Stop-on-Failure |
| **Per-Arm Timeout** | **600 seconds** | Wall-clock timer |

### Unadopted Headroom Reserve Options:
- **Option 1 (30,000 Reserve):** Nominal trigger $= 175,000 - 30,000 = \mathbf{145,000\text{ tokens}}$. (Aligned with candidate $P_{90} = 29,863$).
- **Option 2 (35,000 Reserve):** Nominal trigger $= 175,000 - 35,000 = \mathbf{140,000\text{ tokens}}$. (Aligned with overall $P_{90} = 34,867$).
- **Option 3 (40,000 Reserve):** Nominal trigger $= 175,000 - 40,000 = \mathbf{135,000\text{ tokens}}$. (Exceeds historical worst-case maximum $\Delta = 37,651$).

---

## 4. The Stop-at-First-Edit Execution Rule

To guarantee that runs terminate before entering expensive late-stage compilation and polling loops, this amendment introduces the **Stop-at-First-Edit Rule**:

1. **Rule Definition:** The harness immediately terminates the child process tree upon detection of the first successful source code modification.
2. **Exact Defining Event:**  
   The rule triggers upon the emission of an `item.completed` event where:
   - `item.type == "file_change"` OR tool call is `tools.apply_patch` / `tools.write_file`, AND
   - The file path belongs to tracked task source files (e.g. `core.module.ts`), AND
   - A subsequent local git inspection confirms a non-empty working tree modification.
3. **Execution Classification:**  
   Runs halted by the first-edit rule are recorded as `COMPLETED_FIRST_EDIT` (not an error or failure).

---

## 5. Primary Telemetry Metrics & Exclusion of Savings Claims

### 5.1 Primary Exploratory Metrics
1. **Tokens Consumed Before First Edit:** Cumulative model tokens at the moment of the first edit.
2. **Turn Count to First Edit:** Total model turns dispatched prior to authoring a patch.
3. **Exploration Tool Calls by Category:** Number of file reads (`Get-Content`), directory listings (`git status`), and searches (`rg`).
4. **Engram MCP Calls:** Number of attempted, approved, executed, and successful calls to `engram_inspect_symbol` or `engram_get_context`.
5. **Engram Response Payload Size:** Exact serialized UTF-8 bytes returned by Engram MCP tools.
6. **Threshold-Stop and Overrun Incidence:** Incidence of `PRE_DISPATCH_HEADROOM_EXHAUSTED` vs `OVERRUN_CENSORED`.

### 5.2 Prohibition of Comparative Savings Claims
> [!IMPORTANT]
> Because runs terminate at the first edit and functional acceptance ($C = 1.0$) across full unit tests and production build is waived, **ALL COMPARATIVE TOKEN SAVINGS CLAIMS ($S$), EFFICIENCY PERCENTAGES, AND SUPERIORITY CLAIMS ARE STRICTLY FORBIDDEN**.
> 
> No comparative percentage savings may be published or claimed. Data is reported purely as descriptive telemetry.

---

## 6. Preserved Protocol Invariants

1. **Candidate-First Counterbalancing:** Engram candidate (`v154_arm3`) executes in Order 1. Native baseline executes in Order 2.
2. **Stop-on-Failure:** If Arm 1 exceeds the ceiling or triggers an unhandled harness error, Arm 2 is marked `SKIPPED_STOP_ON_FAILURE` and skipped immediately.
3. **Authoritative Provenance:** All token reporting is governed by native OpenAI session rollouts (`SESSION_ROLLOUT` provenance).
4. **Overrun Handling (Owner Decision 2):** Any arm exceeding 175,000 tokens is classified as `OVERRUN_CENSORED` and analyzed strictly as right-censored observation.

---

## 7. Rollback Specification

If Amendment 05 Draft v2 is rejected or revoked:
- Execution protocol immediately reverts to **Ratified Amendment 04**:
  - Per-arm ceiling: 250,000 tokens
  - Pre-dispatch reserve: 15,000 tokens
  - End-to-end task completion required ($C = 1.0$) for savings claims
  - Combined block exposure: 500,000 tokens maximum

---

## 8. Ratification Block

```
================================================================================
AMENDMENT 05 DRAFT V2 RATIFICATION
================================================================================

[   ] RATIFIED — Adopt Amendment 05 Draft v2 (Exploratory Protocol with First-Edit Stop)
      Selected Protocol Variant:   Variant B (Exploratory Task B)
      Operational Ceiling:         175,000 tokens per arm
      Headroom Reserve Floor:      [OWNER TO SET: 30k / 35k / 40k] tokens
      Stop-at-First-Edit Rule:     ENABLED (Item type: file_change / apply_patch)
      Comparative Savings Claims:  EXPLICITLY FORBIDDEN (Descriptive only)

[   ] REJECTED — Maintain Ratified Amendment 04 (250,000 Ceiling)

Owner Signature: ___________________________________  Date: _________________
================================================================================
```
