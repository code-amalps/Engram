# Amendment 05 — Variant A: Bounded Task Protocol

**Document Identifier:** `ENGRAM_AMENDMENT_05_VARIANT_A`  
**Parent Protocol:** [`benchmarks/AMENDMENT_04_STAGE1_TOKEN_EFFICIENCY_PROTOCOL_REVISION.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/AMENDMENT_04_STAGE1_TOKEN_EFFICIENCY_PROTOCOL_REVISION.md)  
**Status:** DRAFT PROPOSAL FOR OWNER REVIEW (Strictly Offline)

---

## 1. Purpose and Suitability

### 1.1 Empirical Context & Rationale
Historical evidence from Blocks 06, 07, and 08 on `task_b_remember_me` demonstrates that codebase exploration across Angular sibling directories consumes 145,000 to 270,000 tokens before any edit occurs, resulting in a **0% historical completion rate (0/3)** under a 175,000-token ceiling.

**Variant A** replaces the diffuse Task B with a **localized, bounded engineering task**. The objective is to evaluate whether Engram provides statistically significant token savings on a task that can realistically reach full functional completion ($C = 1.0$) within a controlled inference budget.

### 1.2 Explicit Limitation & Boundary Acknowledgment
> [!WARNING]
> A smaller or localized task does **not** represent the complex architectural dispersion of Task B, nor does success on a bounded task prove that Engram achieves general token savings on large-scale architectural refactoring tasks. Results from Variant A apply strictly to bounded, localized edits.

---

## 2. Objective Selection Criteria for the Bounded Task

Informed by the observed turn-size distribution ($P_{90} = 34,867$ tokens; median $= 28,162$ tokens) and exploration burdens from Tasks 1 and 2, any candidate task under Variant A must satisfy five strict criteria before selection:

1. **Strictly Bounded File Scope:**  
   The solution trajectory must require reading and modifying no more than **3 files** within a single sub-module.
2. **Shallow AST Dependency Radius:**  
   The dependency tree from the target files must have an import hop depth $\le 2$, preventing recursive multi-directory search loops.
3. **Sub-15-Second Test Verification:**  
   The targeted unit test suite or compiler check must execute in $\le 15\text{ seconds}$ locally, precluding interactive child process background polling (such as the 108k-token loop seen in Block 08).
4. **Target Turn Budget:**  
   The complete trajectory must require $\le 6\text{ model turns}$, with an expected cumulative token consumption $\le 100,000\text{ tokens}$.
5. **No Invented Pre-Testing Claims:**  
   No specific task is claimed to be tested or pre-validated here; the owner must select an existing or newly defined task meeting these criteria.

---

## 3. Parameter Specifications

| Parameter | Specification | Enforcement Mechanism |
| :--- | :--- | :--- |
| **Target Task Identifier** | `[OWNER TO SELECT — Must meet Section 2 criteria]` | Harness task config JSON |
| **Per-Arm Token Ceiling ($T_{\text{max}}$)** | **`[OWNER TO SET]`** *(Informed by Task 1: Proposed $\le 175,000$)* | Hard subprocess tree termination |
| **Conservative Headroom Reserve ($R$)** | **`[OWNER TO SET]`** *(Informed by Task 1: Floor 15,000 to 30,000)* | Parameterized in `StreamingCodexRunner` |
| **Pre-Dispatch Stop Trigger** | Dynamic: $\text{Remaining} < \max(\Delta T_{\text{last}}, R)$ | Local pre-dispatch evaluation |
| **Combined Block Exposure Limit** | **`[OWNER TO SET]`** *(Proposed: $2 \times T_{\text{max}}$)* | Two-arm matched pair with stop-on-failure |
| **Per-Arm Timeout** | **600 seconds** | Wall-clock timer |

---

## 4. Success and Failure Criteria

### 4.1 Success Criteria
1. **Functional Correctness:** Both native baseline and Engram candidate arms achieve full functional acceptance:
   $$C_{\text{base}} = 1.0 \quad \text{and} \quad C_{\text{candidate}} = 1.0$$
2. **Verified Token Savings:** Cumulative model tokens consumed by the candidate arm are lower than the baseline arm:
   $$S = 1 - \frac{T_{\text{candidate}}}{T_{\text{base}}} > 0$$
3. **Telemetry Integrity:** Both runs produce valid `SESSION_ROLLOUT` telemetry with zero ceiling breaches.

### 4.2 Failure Criteria
1. **Functional Rejection:** Any arm achieves $C < 1.0$ (compilation error, test failure, or incomplete requirements).
2. **Pre-Dispatch Halting:** An arm halts on `PRE_DISPATCH_HEADROOM_EXHAUSTED`.
3. **Ceiling Overrun:** An arm breaches the configured ceiling (`TOKEN_CEILING_BREACHED` or `OVERRUN_CENSORED`).
4. **Harness Timeout:** Wall-clock time exceeds 600 seconds (`TIMEOUT`).

---

## 5. Permitted and Forbidden Claims

### 5.1 Permitted Claims
- If and only if $C_{\text{base}} = 1.0$ and $C_{\text{candidate}} = 1.0$, the comparative token savings percentage $S$ may be reported and claimed.
- Descriptive comparisons of turn count, tool calls, and prompt cache hit rates on the specific bounded task.

### 5.2 Forbidden Claims
- **No Generalization:** It is strictly forbidden to claim that savings on a bounded task extrapolate to complex multi-directory tasks like Task B.
- **No Incomplete Claims:** It is strictly forbidden to assert token efficiency or savings if either arm failed to achieve $C = 1.0$.
- **No Claims on Censored Runs:** If either arm is halted by the headroom guard or ceiling breach, all comparative savings claims are suppressed (`MUTUAL_ACCEPTANCE_FAILURE`).

---

## 6. Formal Ratification Block

```
================================================================================
AMENDMENT 05 — VARIANT A RATIFICATION
================================================================================

[   ] RATIFIED — Adopt Amendment 05 Variant A (Bounded Task Protocol)
      Selected Task Identifier:    [OWNER TO SET]
      Per-Arm Token Ceiling:       [OWNER TO SET] tokens
      Headroom Reserve Floor:      [OWNER TO SET] tokens
      Maximum Block Exposure:      [OWNER TO SET] tokens

[   ] REJECTED

Approver Signature: ___________________________________  Date: _________________
================================================================================
```
