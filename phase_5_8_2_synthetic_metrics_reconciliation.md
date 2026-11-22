# Engram Phase 5.8.2: Synthetic Performance Metrics & Benchmark Scope Reclassification

**Audit Phase:** 5.8.2  
**Audit Target:** Mathematical recalculation, source provenance, and empirical classification of reported pentad token metrics.  
**Execution Mode:** STRICTLY OFFLINE RE-AUDIT  

---

## 1. Mathematical Recalculation of Reported Metrics

Phase 5.8.1 reported the following model token values from the Task A typo dry run:
- **Arm A (Native Baseline):** 28,720 tokens
- **Arm B (Candidate 3):** 12,650 tokens
- **Arm C (Candidate 4.2 Always-On):** 4,280 tokens
- **Arm D (Candidate 4.2 Adaptive):** 4,280 tokens
- **Arm E (Candidate 5 Optimized Adaptive):** 2,910 tokens

### Recalculation of Stated Relative Reductions:

1. **Candidate 5 vs Candidate 4.2:**
   $$1 - \frac{2910}{4280} = 1 - 0.6799065... = 0.320093... \approx 32.01\% \quad (\text{Reported: } 32.0\%)$$
   **Status: Mathematically Verified Exact.**

2. **Candidate 5 vs Native Baseline:**
   $$1 - \frac{2910}{28720} = 1 - 0.1013231... = 0.898676... \approx 89.87\% \quad (\text{Reported: } 89.9\%)$$
   **Status: Mathematically Verified Exact.**

---

## 2. Source Provenance: What the Numbers Actually Represent

Forensic code inspection of `benchmarks/matched-pair-harness/src/runner.py` lines 1252-1275 reveals the precise mechanism behind these numbers:

```python
elif arm == "arm_e_cand5_optimized":
    events.extend([
        {
            "type": "item.completed",
            "item": {
                "id": "item_1",
                "type": "mcp_tool_call",
                "server": "engram",
                "tool": "engram_get_context",
                "arguments": {"task": "Task execution", "budget": 2000},
                "result": {"content": [{"type": "text", "text": "Optimized context compiled. Seven token-efficiency passes active."}]}
            }
        },
        {
            "type": "turn.completed",
            "usage": {
                "input_tokens": 2850,
                "cached_input_tokens": 2200,
                "cache_write_input_tokens": 0,
                "output_tokens": 60,
                "reasoning_output_tokens": 8
            }
        }
    ])
```

- Total model tokens: $2850 + 60 = 2910$.
- These numbers originate from **`_mock_dry_run_events` in `runner.py`**.
- They are **Synthetic Mock Event Telemetry** designed specifically for dry-run offline validation of orchestrator logic, JSON-RPC schema compliance, comparative metric computation, and report rendering.
- **THEY ARE NOT LIVE PROVIDER TOKEN MEASUREMENTS.**

---

## 3. Benchmark Scope and Task Archetype Truth

| Evaluation Dimension | Stated / Implied Elsewhere | Actual Measured Reality | Forensic Classification |
| :--- | :---: | :---: | :---: |
| **Tasks in Full Benchmark** | 28 tasks | 28 task configurations exist | Planned Full Scope |
| **Tasks Represented in Dry-Run** | 1 task | 1 task (`task_a_typo`) | Single Archetype Only |
| **Tasks Executed Live with LLM** | 0 tasks | 0 tasks | Strictly Prohibited (Offline) |
| **Synthetic Tasks Executed** | 1 task | 1 task (`task_a_typo`) | Verified Dry-Run |
| **Arms Executed in Dry-Run** | 5 arms | 5 arms | Verified All Five Executed |
| **Metric Classification** | "Measured Tokens" | Synthetic Mock Ingestion | **SYNTHETIC_MOCK_TELEMETRY** |

---

## 4. Prohibition of Extrapolation

> [!CAUTION]
> The 32.0% token reduction observed in the Phase 5.8.1 dry run is an offline synthetic fixture calculation on a single task (`task_a_typo`). It cannot and must not be extrapolated as an empirical token savings guarantee across the full 28-task benchmark suite until a live, authorized matched-pair benchmark is conducted under strict statistical controls.
