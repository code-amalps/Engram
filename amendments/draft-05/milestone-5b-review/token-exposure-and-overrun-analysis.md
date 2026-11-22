# Milestone 5B-Review: Token Exposure & Overrun Analysis

## 1. Executive Summary & Epistemological Correction

A critical finding of this audit is that **the local token guard is a defensive, pre-dispatch fail-fast monitor—NOT a deterministic zero-leakage hard ceiling**.

The Milestone 5B-Prep report asserted that sizing the planning reserve at 45,000 tokens provided a "mathematical containment proof" that a hard ceiling of 175,000 tokens could not be breached. This audit **corrects and qualifies that claim**:
- Sizing the reserve at 45,000 tokens guarantees containment **only under the empirical assumption that no single turn exceeds 37,651 tokens** (the historical maximum observed in Task B Block 06).
- If an agent generates a single turn consuming 50,000 or 80,000 tokens, or if remote cloud inference continues after local process termination, consumption will exceed 175,000 tokens.
- In the absence of a provider-enforced server-side hard ceiling, local process polling cannot deterministically cap total financial or token exposure.

---

## 2. Deep-Dive Inspection of the Token Guard Execution Path

The execution path in `benchmarks/matched-pair-harness/src/runner.py` was inspected to address ten specific architectural questions:

### Q1: What usage value is available immediately before dispatch?
**Finding:** The available value is `self.effective_tokens`, defined as:
$$\text{effective\_tokens} = \max(\text{thread\_cumulative\_max}, \text{turn\_deltas\_sum})$$
This reflects only the events already written to stdout/rollout, read across OS pipes, and parsed by the 50ms polling loop prior to the decision point.

### Q2: Is it the latest provider-reported cumulative usage or a local estimate?
**Finding:** The *current consumption* is the latest provider-reported value parsed from streaming events. However, the *projected next-turn cost* is an estimate:
$$\text{estimated\_next\_turn} = \max(\text{last\_turn\_request\_tokens}, \text{conservative\_reserve})$$
If the agent dispatches a turn substantially larger than either the previous turn or the 45,000 reserve, the actual cost will exceed the projection.

### Q3: Can a new request be dispatched while the previous request's usage is unresolved?
**Finding:** Yes. Within an agent turn, multiple tool calls and model completions can occur. If token telemetry is reported only upon `turn.completed`, intermediate requests can be dispatched while the poller's visible usage remains at the turn baseline.

### Q4: Can multiple requests be in flight?
**Finding:** In the matched-pair harness (`codex exec --full-auto --ephemeral`), the dialogue turns are executed sequentially by Codex. However, the harness poller operates externally and asynchronously; it does not place a synchronous lock or interception proxy on Codex's outbound HTTP sockets.

### Q5: Can usage events arrive out of order?
**Finding:** Yes. File-system buffer flushing and stdout interleaving can cause `token_usage_record` snapshots and `turn.completed` events to be ingested slightly out of order. The `StreamingTokenTracker` reconciles this monotonically via `max()`, but real-time visible usage may temporarily lag actual provider generation.

### Q6: Can a completed turn report usage after the process has terminated?
**Finding:** No. Once `proc.terminate()` or `terminate_pid_tree(proc.pid)` is called, the process and its child processes are killed. If the child process has not flushed its final telemetry buffer to disk before termination, the final usage delta is unreadable.

### Q7: Does process termination cancel a remote generation?
**Finding:** **NO.** Terminating the local process (`codex.exe`) tears down the local process tree. Remote inference on provider servers (e.g., OpenAI or Anthropic API) may continue generating tokens until the server detects TCP socket reset or until generation finishes. Tokens generated remotely during this window are billed to the account.

### Q8: Is the token count based on cumulative snapshots or per-request deltas?
**Finding:** It is a dual-track hybrid. `thread_cumulative_max` tracks cumulative snapshots from `token_usage_record`. `turn_deltas_sum` sums individual `turn.completed` deltas. Effective tokens is the maximum of the two.

### Q9: Can one request exceed 45,000 tokens?
**Finding:** **YES.** Frontier model APIs support 128,000 to 1,000,000+ context windows. If an agent dumps a large repository file, AST index, or extensive compiler error log into the prompt, a single turn can consume 60,000, 100,000, or more tokens.

### Q10: Is there a provider-enforced usage limit?
**Finding:** **NO.** The benchmark harness does not wire in a provider-enforced hard ceiling via API headers or workspace spend caps. Account-level subscription quotas operate on monthly schedules, not per-experiment run ceilings.

---

## 3. Seven Operational Exposure Scenarios

The table below summarizes the behavior of the harness across the seven required operational scenarios:

| Scenario | Pre-Dispatch Usage ($U$) | Estimated Next Turn | Actual Next Turn | Observable Usage Post-Turn | Unknown / In-Flight Leakage | Guard Action | Final Classification |
|---|---|---|---|---|---|---|---|
| **Scenario A: Historical Max** | 130,000 | 45,000 | 37,651 | 167,651 | 0 | Allowed dispatch; stops at next boundary | **NOMINAL CONTAINMENT (Within 175k)** |
| **Scenario B: Larger Single Turn** | 130,000 | 45,000 | 50,000 | 180,000 | 0 | Allowed dispatch; halts post-turn on ceiling breach | **OVERRUN (Exceeds 175k by 5,000 tokens)** |
| **Scenario C: Delayed Telemetry** | 120,000 | 45,000 | 25,000 (delayed) | 120,000 | 25,000 in-flight | Dispatches Turn 2 based on stale 120k count | **LATENCY VULNERABILITY WINDOW** |
| **Scenario D: Multiple In-Flight** | 100,000 | 45,000 | $2 \times 30,000$ | 100,000 | 60,000 concurrent | Poller cannot intercept intermediate sockets | **CONCURRENT LEAKAGE HAZARD** |
| **Scenario E: Missing Telemetry** | 100,000 | 45,000 | Missing / Null | 100,000 | Unknown | Marks `telemetry_valid=False`; fails closed | **FAIL-SAFE HALT (Stop on invalid data)** |
| **Scenario F: Process Termination** | 130,000 | 45,000 | 40,000 (remote) | 130,000 | Remainder of remote turn (~40k) | Local tree killed; remote generation completes | **REMOTE INFERENCE RESIDUAL SPEND** |
| **Scenario G: Provider Limits** | N/A | N/A | N/A | N/A | Bound by provider quota | Remote API returns 429/quota error | **HARD CEILING (Provider level only)** |

---

## 4. Rigorous Conceptual Distinctions

To ensure honest engineering communication, the project adopts these explicit definitions:

1. **Historical Observed Maximum ($\Delta_{\text{hist}} = 37,651$ tokens):** The largest empirical turn delta observed in one specific Task B run. It is an empirical reference point, not a physical or mathematical ceiling.
2. **Assumed Maximum ($\Delta_{\text{assumed}} \le 45,000$ tokens):** The planning assumption underpinning the 45,000-token headroom reserve.
3. **Estimated Request Exposure:** The projected cost for the upcoming turn: $\max(\text{last\_turn}, 45,000)$.
4. **Pre-Dispatch Reserve ($R = 45,000$ tokens):** The buffer withheld from the agent to prevent starting a turn near the budget limit.
5. **Nominal Planning Ceiling ($C_{\text{nom}} = 175,000$ tokens per arm):** The governance threshold for normal block completion.
6. **Maximum Possible Financial Exposure:** Under unbounded model API conditions, the maximum financial exposure is bounded only by the account-level provider spend limit or model context window limits.

---

## 5. Conclusion

The 45,000-token headroom reserve significantly hardens the harness against historical multi-turn jumps, but it **cannot guarantee a hard zero-leakage ceiling**. All references to "mathematical containment proofs" must be treated as conditional on the assumption that no turn exceeds 45,000 tokens and that remote generation terminates with local processes.
