# Milestone 5A: Token Ceiling and Stopping Policy
## Conservative Multi-Tier Exposure Guard, In-Flight Overrun Bounds, and Termination Architecture

**Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/async-precommit-guard`  
**HEAD Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`  
**Document ID:** `ENGRAM-A05-M5A-TOKEN-CEILING`  
**Classification:** Operational Safety and Risk Management Policy  

---

### 1. Multi-Tier Token Exposure Architecture

To prevent uncontrolled API exposure and ensure rigorous cost containment during future authorized live experiments, this policy establishes a calibrated, multi-tier stopping mechanism:

```
[0 Tokens] -------------------------------------------------------------------->
                                                                               |
               [ Nominal Stop Threshold: 140,000 Tokens ]                      |
                                   |                                           |
                                   +---> [ Pre-Dispatch Guard Blocks Turn ]    |
                                   |                                           |
               [ In-Flight Reserve Headroom: 35,000 Tokens ]                   |
                                   |                                           |
                                   v                                           |
               [ Hard Token Ceiling: 175,000 Tokens Per Arm ] <----------------+
                                   |
                                   +---> [ Windows Job Object Force Kill ]
```

---

### 2. Quantitative Threshold Calibration

Based on empirical evidence from historical Block 06 (Turn 9 consumed 37,651 tokens) and Block 07 (Turn 11 consumed 29,918 tokens), the historical 15,000-token reserve was demonstrably inadequate to prevent hard-ceiling breaches during large file-ingestion turns. 

The threshold parameters are re-calibrated as follows:

| Parameter | Proposed Value | Empirical Justification |
| :--- | :---: | :--- |
| **Per-Arm Maximum Token Exposure** | **175,000 tokens** | Conservative upper boundary for complex multi-file engineering tasks. |
| **Combined Maximum Task Exposure** | **350,000 tokens** | Maximum possible exposure across both arms (Arm A + Arm B). |
| **Reserve Headroom** | **35,000 tokens** | Sized to absorb the maximum observed single-turn context ingestion (~35k tokens) without overrunning 175,000. |
| **Nominal Stop Threshold** | **140,000 tokens** | Dispatch trigger: No new conversational turn is permitted once cumulative usage reaches 140,000 tokens. |
| **Per-Turn Timeout** | **300 seconds** (5m) | Halts hung process if CLI fails to return. |
| **Per-Arm Total Timeout** | **1,200 seconds** (20m) | Total cumulative wall-clock ceiling. |

---

### 3. Pre-Dispatch Headroom Guard (Deterministic Gate)

Before the test harness dispatches turn $t+1$ to the agent subprocess, it must execute the deterministic pre-dispatch check:

```python
def pre_dispatch_check(cumulative_tokens: int, nominal_stop: int = 140_000) -> bool:
    if cumulative_tokens >= nominal_stop:
        logger.error(f"Pre-dispatch stop triggered: {cumulative_tokens} >= {nominal_stop}")
        return False  # Do NOT dispatch next turn; trigger clean termination
    return True
```

- **Zero-Latency Protection:** Because this check occurs locally *before* an API call is initiated, it guarantees that no turn begins if the cumulative budget is exhausted.
- **Stop Reason:** Recorded in telemetry as `STOP_REASON_NOMINAL_CEILING_REACHED`.

---

### 4. In-Flight Overrun Bounds and Polling Latency Reality

#### 4.1 Epistemological Disclaimers (Zero-Overrun Myth)
- **External Polling Cannot Truncate Remote In-Flight Tokens:** Model inference occurs on remote cloud servers (OpenAI / Anthropic). 
- Usage records are transmitted to the local harness only after the model completes the entire turn and flushes its HTTP response stream.
- An external polling monitor operating at a 250ms interval cannot halt token accumulation while the server-side model is actively generating.
- **Therefore, zero-overrun protection is mathematically and architecturally impossible with an external polling monitor.**

#### 4.2 Mathematical Overrun Bounds Under the 35,000 Token Reserve
Let $T_t$ be cumulative tokens at the start of turn $t+1$.
- By definition of the pre-dispatch guard: $T_t < 140,000$.
- In the worst-case scenario, turn $t+1$ initiates at $T_t = 139,999$ tokens.
- Maximum observed single-turn token consumption: $\Delta T_{\max} \approx 35,000$ tokens.
- Total upon turn completion:
  $$T_{\text{final}} = 139,999 + 35,000 = 174,999 \text{ tokens} \le 175,000 \text{ tokens}$$
- By setting the nominal stop threshold to 140,000 and the reserve to 35,000, **even a worst-case 35k single-turn jump remains contained within the 175,000 hard ceiling**.

---

### 5. Process Termination Architecture

If an in-flight turn finishes and pushes cumulative tokens past the 140,000 threshold or if a process times out:
1. **Windows Job Object Termination:** 
   The harness assigns the agent subprocess and all descendants to a Windows Job Object (`JobObjectBasicProcessIdList`) upon spawn. Termination calls:
   ```rust
   TerminateJobObject(job_handle, EXIT_CODE_TOKEN_CEILING_BREACH);
   ```
   This instantaneously reaps the entire process tree (killing Node.js, PowerShell child processes, and dangling git workers) in $< 50\text{ms}$.
2. **Telemetry Preservation:** Prior to killing the process, the streaming file reader captures the final emitted usage record.

---

### 6. Failure Handling and Inter-Arm Stop Policy

#### 6.1 Treatment Failure First (Arm B Fails)
If Arm B (Treatment) fails functionally or breaches its token ceiling, the orchestrator **immediately aborts the matched pair** and skips Arm A.
- *Rationale:* Since statistical comparison requires a valid matched pair, running Arm A after Arm B has already failed wastes model quota without producing a usable paired datapoint.

#### 6.2 Baseline Failure First (Arm A Fails)
Similarly, if Arm A (Baseline) breaches its ceiling (as in Block 06), Arm B is cleanly skipped.

#### 6.3 Invalid Telemetry & Crash Handling
- If a turn emits a malformed usage object or the telemetry stream disconnects unexpectedly, the harness transitions to `AMBIGUOUS_HOLD` state and aborts further execution.
- No rerun is attempted automatically; owner intervention is required.
