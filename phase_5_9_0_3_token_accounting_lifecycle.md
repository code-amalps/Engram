# Engram Phase 5.9.0.3: Token Accounting Lifecycle & Architecture Specification

**Phase:** 5.9.0.3  
**Target Files:** `benchmarks/matched-pair-harness/src/orchestrator.py`, `runner.py`, `telemetry.py`  
**Execution Mode:** Strictly Offline Source-Code Audit  

---

## 1. End-to-End Token Accounting Flow

We traced the complete token budget lifecycle across 14 distinct stages from CLI invocation to disk serialization:

```mermaid
flowchart TD
    S1["1. CLI Argument Parsing<br/>(--global-tokens-ceiling, --max-tokens-ceiling, --headroom-reserve)"] --> S2["2. Orchestrator Init<br/>(self.global_tokens_ceiling, self.headroom_reserve)"]
    S2 --> S3["3. Pentad Arm Ordering<br/>(random.Random(seed).shuffle(ALL_PENTAD_ARMS))"]
    S3 --> S4["4. Global Budget Init<br/>(cumulative_global_tokens = 0)"]
    
    S4 --> S5["5. Pre-Arm Dispatch Guard<br/>(global_remaining = G_ceil - cumulative; H_rem >= 45k?)"]
    S5 -- No --> S5_Abort["Halt Pentad: ABORTED_GLOBAL_BUDGET<br/>Skip remaining arms"]
    S5 -- Yes --> S6["6. Assign Arm Ceiling<br/>(effective_arm_ceiling = min(max_arm_ceiling, global_remaining))"]
    
    S6 --> S7["7. Launch Child Codex Process<br/>(runner.py: StreamingTokenTracker initialized)"]
    S7 --> S8["8. Pre-Turn Headroom Guard<br/>(remaining >= max(last_turn_request, 45k)?)"]
    S8 -- No --> S8_Halt["PREVENTIVE_HALT<br/>terminate_pid_tree; stop before next turn"]
    S8 -- Yes --> S9["9. Submit Turn Request to Provider"]
    
    S9 --> S10["10. Process Stream & Deduplicate<br/>(SHA-256 line hash; max(cumulative_max, turn_deltas_sum))"]
    S10 --> S11["11. Post-Turn Ceiling Check<br/>(effective_tokens > effective_arm_ceiling?)"]
    S11 -- Yes --> S11_Halt["TOKEN_CEILING_BREACHED<br/>terminate_pid_tree"]
    S11 -- No --> S8
    
    S8_Halt --> S12["12. Arm Post-Run Reconcile<br/>(validate_telemetry_integrity)"]
    S11_Halt --> S12
    S12 --> S13["13. Accumulate Global Total<br/>(cumulative_global_tokens += spent)"]
    S13 --> S14["14. Check Uncertainty / Halt<br/>(is_budget_uncertain -> ABORTED_UNCERTAIN_BUDGET)"]
    S14 --> S5
```

---

## 2. Definitive Inventory of Budget Counters and Accounting Layers

| Budget Variable / Counter | Scope | Unit | Cumulative or Delta? | Includes Cached Input? | Includes Completion Output? | Includes Reasoning Tokens? | Includes Tool Invocations? | Includes Provider Retries? | Includes Incomplete Requests? |
|---|---|---|---|---|---|---|---|---|---|
| `global_tokens_ceiling` | Block / Invocation | Tokens | Static ceiling | N/A | N/A | N/A | N/A | N/A | N/A |
| `max_tokens_ceiling` | Arm | Tokens | Static ceiling | N/A | N/A | N/A | N/A | N/A | N/A |
| `headroom_reserve` | Turn / Arm | Tokens | Static threshold | N/A | N/A | N/A | N/A | N/A | N/A |
| `effective_arm_ceiling` | Single Arm Execution | Tokens | Dynamic limit | N/A | N/A | N/A | N/A | N/A | N/A |
| `thread_cumulative_max` | Single Arm Session | Tokens | Cumulative monotonic | **YES** | **YES** | **YES (Sub-metric)**| **YES** | **YES** | If emitted prior to crash |
| `turn_deltas_sum` | Single Arm Session | Tokens | Deduplicated sum | **YES** | **YES** | **YES (Sub-metric)**| **YES** | **YES** | Only completed turns |
| `effective_tokens` | Single Arm Session | Tokens | $\max(\text{cum}, \text{deltas})$ | **YES** | **YES** | **YES** | **YES** | **YES** | Authoritative local count |
| `cumulative_global_tokens` | Invocation (5 Arms) | Tokens | Sum of arm spent | **YES** | **YES** | **YES** | **YES** | **YES** | Verified arm usage |

---

## 3. Disentangling Token Accounting Terminologies

To eliminate cross-layer ambiguities:
1. **Provider-Reported Usage:** The literal integers parsed from `token_usage_record.thread_token_usage` (`input_tokens`, `output_tokens`, `cached_input_tokens`, `reasoning_output_tokens`).
2. **Locally Estimated Turn Demand:** Calculated in `StreamingTokenTracker.estimate_next_request_tokens()` as:
   $$E_{\text{est}} = \max(\text{last\_turn\_request\_tokens}, 45,000)$$
3. **Locally Enforced Budget Counters:** `effective_arm_ceiling` in `runner.py` and `global_tokens_ceiling` in `orchestrator.py`.
4. **Post-Cancellation In-Flight Residual:** Tokens generated remotely on provider GPUs after local process termination (`taskkill`) before network socket teardown completes remotely.
