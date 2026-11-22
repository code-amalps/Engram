# Engram Phase 5.9.0: Budget Model, Token Accounting & Stopping Policy

**Execution Phase:** 5.9.0  
**Target Systems:** `benchmarks/matched-pair-harness/src/runner.py`, `orchestrator.py`  
**Execution Mode:** SPECIFICATION & POLICY AUDIT  

---

## 1. Mathematical Budget Formulation

Total benchmark token exposure is governed by the formal model:

$$B_{\text{global}} = \sum_{t \in T} \sum_{a \in A} B_{t,a} + B_{\text{reserve}}$$

Where:
- $T = \{\text{task\_a\_typo}, \text{task\_b\_remember\_me}\}$ ($|T| = 2$)
- $A = \{\text{arm\_a\_native}, \text{arm\_b\_cand3}, \text{arm\_c\_cand42\_always\_on}, \text{arm\_d\_cand42\_adaptive}, \text{arm\_e\_cand5\_optimized}\}$ ($|A| = 5$)
- $B_{t,a}$ = Allocated per-arm token ceiling for task $t$ and arm $a$.
- $B_{\text{reserve}}$ = Headroom reserve buffer required before dispatching the next model inference turn ($B_{\text{reserve}} = 45,000\text{ tokens}$).

---

## 2. Token Accounting & Categories

To avoid double-counting while capturing complete provider billing, the harness relies exclusively on the authoritative `turn.completed` usage payload streamed by the Codex execution runner:

```
Total Billed Model Tokens = Prompt Tokens Total + Completion Tokens
```

Where:
1. **Prompt Tokens Total (`prompt_tokens_total`):**
   - **Cached Prompt Tokens (`prompt_tokens_cached`):** Ingested from provider prompt cache at discounted billing rate.
   - **Uncached Prompt Tokens (`prompt_tokens_uncached`):** Incurred on new context insertions, novel instructions, and tool outputs.
2. **Completion Tokens (`completion_tokens`):**
   - Generated model output text and tool-call JSON arguments.
   - **Reasoning Tokens (`reasoning_tokens`):** Internal chain-of-thought tokens (subsumed within completion tokens by OpenAI API specification).
3. **Auxiliary Payload Accounting:**
   - Inner tool response content bytes and JSON-RPC overhead are tracked independently in telemetry under `replacement_accounting` to measure serialization efficiency without inflating model token counts.

---

## 3. Deterministic Stopping Rules

### 3.1 Pre-Dispatch Headroom Guard (Per-Arm)
Before initiating any new model turn $k$, the runner calculates:
$$\text{Estimated Next Turn Cost } (E_{k}) = \max(\text{turn}_{k-1}\text{ prompt tokens}, 25,000)$$
$$\text{Remaining Budget } (R_{k}) = \text{Per-Arm Ceiling} - \sum_{i=1}^{k-1} \text{Billed Tokens}_{i}$$

The runner executes an immediate preventative halt if:
$$R_{k} < \max(E_{k}, B_{\text{reserve}})$$

- **Outcome:** The child process is cleanly signaled to exit, and the arm is recorded as `PRE_DISPATCH_HEADROOM_EXHAUSTED` (or `TOKEN_CEILING_BREACHED`).
- **Safety Guarantee:** Prevents runaway conversational loops from incurring unbudgeted provider expenses.

### 3.2 Global Ceiling Guard (Block-Level)
After every turn across all arms, the orchestrator tracks:
$$C_{\text{global}} = \sum \text{Tokens across all completed and running arms}$$

If $C_{\text{global}} \ge B_{\text{global}}$:
- All active subprocesses are immediately terminated.
- All remaining downstream arms in the schedule are marked `SKIPPED_GLOBAL_BUDGET_EXHAUSTED`.
- Block status is recorded as `HALTED_CEILING_BREACHED`.

### 3.3 Inter-Arm Budget Transfer Prohibition
> [!IMPORTANT]
> Unused budget from an efficient arm (e.g., Candidate 5 completing in 75,000 tokens against a 250,000 ceiling) **CANNOT** be transferred to expand another arm's ceiling. Each arm operates under fixed, independent limits to ensure strict cross-arm comparability and experimental isolation.

---

## 4. Scenario Evaluation & Final Recommendation

| Scenario Dimension | Scenario 1: Conservative | Scenario 2: Balanced (Recommended) | Scenario 3: Minimal Pilot |
| :--- | :---: | :---: | :---: |
| **Task A Per-Arm Ceiling** | 300,000 tokens | 250,000 tokens | 150,000 tokens |
| **Task B Per-Arm Ceiling** | 650,000 tokens | 450,000–550,000 tokens | 250,000 tokens |
| **Headroom Reserve** | 45,000 tokens | 45,000 tokens | 45,000 tokens |
| **Task A Block Ceiling** | 1,200,000 tokens | 1,000,000 tokens | 600,000 tokens |
| **Task B Block Ceiling** | 2,600,000 tokens | 1,500,000 tokens | 900,000 tokens |
| **Global Hard Ceiling ($B_{\text{global}}$)**| **3,800,000 tokens** | **2,500,000 tokens** | **1,500,000 tokens** |
| **Estimated Cost (@ \$15/M)** | \$57.00 | \$37.50 | \$22.50 |
| **Completion Probability (Task A)**| 100% | 100% | 80% (Always-on arms halt) |
| **Completion Probability (Task B)**| 90% (Cand 3 finishes) | 70% (Cand 3 halts; Cand 5 finishes) | 20% (Native & Cand 3 halt early) |

### Formal Recommendation:
**Scenario 2 (Balanced)** is officially recommended:
1. Allocates **1,000,000 tokens for Task A** (`--max-tokens-ceiling 250000`, `--global-tokens-ceiling 1000000`).
2. Allocates **1,500,000 tokens for Task B** (`--max-tokens-ceiling 500000`, `--global-tokens-ceiling 1500000`).
3. Total combined exposure is capped at **2,500,000 tokens**.
4. This ensures that optimized treatments (Candidate 4.2 Adaptive and Candidate 5) have ample headroom to complete fully and demonstrate true savings, while legacy unpruned arms (Candidate 3) are halted safely if they exceed reasonable boundaries.
