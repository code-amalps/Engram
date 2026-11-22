# Phase 5.4 — Stage 3: Token Budget Semantics & Context Ceiling Specification

**Document Status:** Sealed Architectural & Accounting Audit  
**Execution Date:** 2026-11-19  
**Auditor:** Principal Systems Architect & Test Automation Engineer  
**Target Repository:** Engram (`feature/phase2-stage5-remediation`, HEAD `7106a2f`)  
**Components Audited:**
- Rust Engine: `src/compiler.rs`, `src/mcp.rs`, `src/analyzer.rs`, `src/budget.rs`
- Python Harness: `benchmarks/matched-pair-harness/src/runner.py` (`StreamingTokenTracker`, `CodexRunner`)
- Benchmark Suites: `tests/test_token_guard_validation.py`, `tests/test_block07_remediation.py`

---

## 1. Executive Summary

This Stage 3 audit examines the definition, measurement, and enforcement of token budgets and context ceilings across the Engram engine and the matched-pair evaluation harness.

### Key Audit Conclusions:
1. **Semantic Collision on "Token Budget":** The term `token_budget` is used with two entirely conflicting meanings across the system:
   - **Engine Definition (Per-Request Context Occupancy):** In `src/compiler.rs` and `src/mcp.rs`, `token_budget` (default: 1,200 tokens) specifies the maximum token size of the *injected context slice* produced by `BudgetPacker` for a single prompt or MCP response.
   - **Harness Definition (Cumulative Billed Session Tokens):** In `runner.py`, `max_tokens_ceiling` (default: 250,000 tokens) specifies the hard ceiling on *cumulative billable tokens* (prompt + completion across all turns) across the entire multi-turn thread.
2. **Tokenizer & Unit Discrepancy:**
   - The Rust engine estimates tokens using a crude heuristic `(chars + 3) / 4` (`src/analyzer.rs:2289`). For code-heavy JSON and AST structures, this underestimates actual BPE tokens by 30% to 45%.
   - The Python harness measures exact BPE tokens reported authoritatively by the model provider API (`token_usage_record` / `event_msg`).
3. **Cached Token Accounting:**
   - Provider telemetry reports `prompt_tokens_cached` separately.
   - The harness records cached vs. uncached tokens in telemetry, but treats cached prompt tokens at **100% full weight** when enforcing the 250,000 session ceiling and calculating `effective_tokens`.
4. **Effective Headroom Reduction:**
   - The harness enforces a conservative headroom reserve of `45,000` tokens.
   - As a result, execution is halted when cumulative tokens reach `205,000` (`250,000 - 45,000`), reducing the effective operational ceiling by 18%.
5. **The Quadratic Turn-History Dynamic:**
   - In multi-turn agent sessions, full conversation history is re-sent on every turn. Cumulative prompt tokens scale as $\sum_{k=1}^N S_k \approx O(N \cdot S_0 + \frac{1}{2} N^2 \Delta)$.
   - Consequently, turn count $N$ dominates cumulative token consumption far more than small single-turn context reductions.

---

## 2. Comparative Semantic Audit: Engine vs. Harness

| Dimension | Rust Engine (`src/`) | Benchmark Harness (`benchmarks/`) | Production Provider API |
|:---|:---|:---|:---|
| **Primary Metric** | `token_budget` | `max_tokens_ceiling` | `context_window` & `usage` |
| **Scope** | Single tool response / injection | Entire multi-turn session (per arm) | Per request & per billing period |
| **Default Value** | `1,200` tokens | `250,000` tokens | 128k–200k tokens per request |
| **Measurement Method** | Character heuristic: `(chars + 3) / 4` | Authoritative provider telemetry | Exact BPE tokenizer (e.g. o200k_base) |
| **Cached Token Handling** | Not applicable (unaware of cache) | Tracked in metadata, but 1.0x weight in ceiling | 80% discounted cost (0.2x billing) |
| **Enforcement Point** | Context compilation (`BudgetPacker`) | Pre-dispatch boundary poller | HTTP 400 Context Length Exceeded |
| **Failure Behavior** | Pruning / truncation with omission manifest | Process termination (`SIGKILL` / `taskkill`) | Request rejection |

---

## 3. Harness Token Accounting Audit (`StreamingTokenTracker`)

### 3.1 Effective Token Computation
In `benchmarks/matched-pair-harness/src/runner.py`:
```python
@property
def effective_tokens(self) -> int:
    return max(self.thread_cumulative_max, self.turn_deltas_sum)
```
- `thread_cumulative_max`: Sourced from authoritative `token_usage_record.payload.thread_token_usage.total_tokens` or `event_msg.payload.info.total_token_usage.total_tokens`.
- `turn_deltas_sum`: Sum of incremental `turn.completed` delta usage events.
- **Evaluation:** This dual-source reconciliation ensures robust handling of both snapshot-based and delta-based streaming feeds without double-counting.

### 3.2 Pre-Dispatch Headroom Guard Evaluation
In `runner.py`:
```python
remaining = ceiling - self.effective_tokens
estimated = max(self.last_turn_request_tokens, self.conservative_reserve)
has_headroom = (remaining >= estimated)
```
- **Operational Reserve:** Fixed at `45,000` tokens (`conservative_reserve`).
- **Estimation Logic:** Uses `max(last_turn_request_tokens, 45_000)`.
- **Behavior at Boundary:**
  - If a task finishes Turn 4 with cumulative tokens = 206,000:
  - `remaining = 250,000 - 206,000 = 44,000`.
  - `estimated = max(last_turn_request, 45,000) >= 45,000`.
  - `has_headroom = (44,000 >= 45,000) = False` $\rightarrow$ `PRE_DISPATCH_HEADROOM_EXHAUSTED` triggered.
  - The process is terminated before Turn 5 can dispatch, even though 44,000 tokens remained in the ceiling.

---

## 4. Discrepancies and Failure Modes

### 4.1 Engine Heuristic Underestimation
In `src/analyzer.rs`:
```rust
pub fn estimate_tokens(s: &str) -> usize {
    (s.chars().count() + 3) / 4
}
```
- **Code/AST Empirical Measurement:**
  - 10,000 characters of TypeScript AST JSON produces ~3,600 BPE tokens in `o200k_base` (2.77 chars/token).
  - The formula `(10000 + 3) / 4` predicts 2,500 tokens.
  - **Discrepancy:** The engine underestimates context size by ~30.5%. A response packed to an estimated 1,200 tokens actually occupies ~1,700 tokens in the model prompt.

### 4.2 Unbudgeted MCP Tool Endpoints
As established in Stage 2, while `engram_get_context` uses `BudgetPacker`, `engram_inspect_symbol` in `src/mcp.rs` contains zero budget checks. It returned 162,925 tokens in a single response.

### 4.3 Ceiling Scope Ambiguity in Task B
- Task B is an Angular full-stack task modifying 4 files (`auth.actions.ts`, `auth.reducer.ts`, `auth.effects.ts`, `app.component.html`, `app.component.ts`) with associated unit tests.
- Completing this workflow requires 8 to 14 turns.
- In Control A, prompt tokens grew from 17k (Turn 1) to 32k (Turn 5), reaching 222,063 cumulative tokens by Turn 5.
- Therefore, a 250,000 cumulative token ceiling is **structurally inadequate for complex multi-turn coding tasks** unless:
  1. The task is decomposed into smaller sub-tasks, OR
  2. The session token ceiling is calibrated to the complexity tier of the task (e.g., 500,000 for complex multi-file tasks), OR
  3. Turn-level history compaction / summarization is active.

---

## 5. Unambiguous Token Accounting Model

To eliminate semantic confusion across all future phases, Engram adopts the following four-tier token accounting model:

```
┌────────────────────────────────────────────────────────────────────────┐
│                      ENGRAM TOKEN ACCOUNTING MODEL                     │
└────────────────────────────────────────────────────────────────────────┘

1. Per-Request Context Occupancy (R_t)
   Definition: Total tokens contained in the single prompt sent to the LLM at turn t.
   Formula:    R_t = System_Prompt + Tool_Definitions + Conversation_History_t + Turn_Input_t
   Engine Contract: Injected Engram context MUST NOT exceed per_request_budget (default: 2,000 tokens).
   Hard Ceiling:    Must fit within model window (e.g. 128,000 tokens).

2. Injected Auxiliary Payload Size (P_t)
   Definition: Size of the specific payload returned by an Engram MCP tool or injection at turn t.
   Engine Contract: P_t <= tool_response_ceiling (Hard cap: 4,000 tokens / 16 KB).
   Enforcement:     Mandatory truncation manifest if raw graph/excerpt exceeds ceiling.

3. Turn Incremental Delta (D_t)
   Definition: Net new tokens billed for turn t.
   Formula:    D_t = R_t (Prompt) + C_t (Completion)

4. Cumulative Session Billed Tokens (C_total)
   Definition: Total billed tokens summed across all turns of an arm execution.
   Formula:    C_total = sum_{t=1}^N (R_t + C_t)
   Harness Contract: Evaluated against task-complexity calibrated session ceiling:
                     - Tier 1 (Simple):  150,000 tokens
                     - Tier 2 (Medium):  300,000 tokens
                     - Tier 3 (Complex): 500,000 tokens
```

### 5.1 Financial Cost Weighting (Quality-Adjusted Cost)
When reporting financial cost or token efficiency, cached prompt tokens must be reported with explicit billing weighting:
$$\text{Cost-Equivalent Tokens} = (\text{Prompt}_{\text{uncached}} \times 1.0) + (\text{Prompt}_{\text{cached}} \times 0.20) + (\text{Completion} \times 4.0)$$

---

## 6. Required Harness & Engine Adjustments

1. **Rust Engine (`src/mcp.rs` & `src/impact.rs`):**
   - Enforce `tool_response_ceiling` on `engram_inspect_symbol` (max 4,000 tokens).
   - Prune `import_dependent_entities` to top $N \le 10$ non-external entities.
   - Suppress redundant narrative explanation when structured JSON is requested.
2. **Harness (`src/runner.py`):**
   - Distinguish `session_cumulative_ceiling` from `per_request_context_occupancy` in telemetry reports.
   - Support task-complexity-aware ceilings in `task_definition.json`.

*Document finalized and ready for Stage 4.*
