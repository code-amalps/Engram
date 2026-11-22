# Milestone 5B-Prep: Token Ceiling Safety Review

## 1. Safety Background and Historical Context

In Milestone 5A, an operational safety risk was analyzed:
In previous multi-turn trials on the Angular starter repository (Stage 1 Block 06, Task B), the single largest observed turn delta was **37,651 tokens** (Turn 9).

The pre-existing safety reserve was:
$$\text{Headroom Reserve}_{\text{historical}} = 35,000 \text{ tokens}$$

Because $37,651 > 35,000$, a theoretical vulnerability existed:
If an agent was at 139,999 tokens (just below the nominal stop threshold of 140,000 against a 175,000 ceiling) and executed a turn consuming 37,651 tokens, the total final consumption would be:
$$139,999 + 37,651 = 177,650 \text{ tokens}$$
This would breach the approved 175,000-token per-arm ceiling by 2,650 tokens.

---

## 2. Conservative Planning Scenario Architecture

To eliminate this hazard deterministically, Milestone 5B-Prep formally evaluated and implemented the **Conservative Planning Scenario**:

### Parameter Specification
- **Hard Exposure Ceiling ($C$):** $175,000 \text{ tokens per arm}$
- **Conservative Planning Reserve ($R$):** $45,000 \text{ tokens}$
- **Pre-Dispatch Stop Threshold ($T_{\text{stop}} = C - R$):** $130,000 \text{ tokens}$
- **Combined Two-Arm Exposure Budget:** $350,000 \text{ tokens}$

---

## 3. Mathematical Proof of Worst-Case Containment

Let $U_{\text{pre}}$ be the cumulative tokens consumed prior to dispatching Turn $N$.
Under the pre-dispatch guard algorithm:
$$\text{has\_dispatch\_headroom} = (C - U_{\text{pre}} \ge R)$$

Therefore, a dispatch is permitted if and only if:
$$U_{\text{pre}} \le C - R = 175,000 - 45,000 = 130,000 \text{ tokens}$$

Let $\Delta_{\max}$ be the worst-case single-turn token delta observed in any benchmark trial:
$$\Delta_{\max} = 37,651 \text{ tokens}$$

The maximum possible final post-turn token exposure $U_{\text{final}}$ occurs when an agent is dispatched at the exact threshold boundary:
$$U_{\text{pre}} = 130,000 \text{ tokens}$$
$$U_{\text{final}} = U_{\text{pre}} + \Delta_{\max} = 130,000 + 37,651 = 167,651 \text{ tokens}$$

### Safety Margin Under Ceiling:
$$\text{Margin} = C - U_{\text{final}} = 175,000 - 167,651 = 7,349 \text{ tokens}$$

Because $\text{Margin} = +7,349 \text{ tokens} > 0$, the hard exposure ceiling of 175,000 tokens is **strictly mathematically guaranteed** against any single-turn jump equal to or less than the historical maximum.

---

## 4. Polling Latency and Terminal Turn Invariants

The token guard is a **pre-dispatch preventive fail-fast monitor**, NOT a mid-flight deterministic circuit breaker.

### Fundamental System Invariant:
1. When an agent dispatches an inference request to a remote model provider (e.g. OpenAI/Anthropic API), the request is processed remotely in full.
2. Local harness monitors cannot halt remote token generation in the middle of a generation turn.
3. Overruns on an in-flight request are fundamentally bounded by the size of the single terminal turn.
4. By sizing the reserve at 45,000 tokens (greater than the worst-case single turn delta of 37,651 tokens), the pre-dispatch check prevents the agent from initiating any turn that could breach the hard ceiling.

### Pre-Dispatch Check Invariant:
```python
def check_pre_dispatch_headroom(self, ceiling: int) -> Tuple[bool, int, int]:
    remaining = ceiling - self.effective_tokens
    estimated_next_turn = max(self.last_turn_request_tokens, self.conservative_reserve)
    has_headroom = (remaining >= estimated_next_turn)
    return (has_headroom, remaining, estimated_next_turn)
```
If `has_headroom` is `False`, the process is terminated cleanly prior to dispatch, setting `stop_reason = "PRE_DISPATCH_HEADROOM_EXHAUSTED"` and logging full audit telemetry.
