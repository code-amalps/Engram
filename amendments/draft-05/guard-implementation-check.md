# Pre-Dispatch Headroom Guard Implementation Audit

**Document Identifier:** `ENGRAM_DRAFT05_GUARD_IMPLEMENTATION_CHECK`  
**Governing Evaluation:** Engram Amendment 05 Pass 3 Offline Feasibility Investigation  
**Harness Source Files Audited:**  
- [`benchmarks/matched-pair-harness/src/orchestrator.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py)  
- [`benchmarks/matched-pair-harness/src/runner.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py)  
**Parent Document:** [`amendments/draft-05/amendment-05.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/amendment-05.md)

---

## 1. Executive Summary

This audit cross-references the proposed language in `amendment-05.md` against the actual Python implementation in `orchestrator.py` and `runner.py`.

### Primary Audit Findings:
1. **Dynamic Estimator vs. Static Trigger Mismatch:**  
   `amendment-05.md` describes a static "160,000 stop trigger" based on subtracting a 15,000 reserve from 175,000. In reality, the codebase implements a **dynamic cost estimator** (`estimate_next_request_tokens()`), where the required headroom is $\max(\text{last\_turn\_tokens}, 15000)$. Because late-stage turns in Task B consume 26,000 to 37,000 tokens, the code triggers pre-dispatch stopping at **~145,000 to ~151,000 cumulative tokens**, not at 160,000.
2. **50 ms Polling Interval & In-Flight Latency Window:**  
   The runner polls child process stdout every 50 ms (`time.sleep(0.05)`). If the Codex CLI dispatches an outbound model request within $< 50\text{ms}$ of receiving tool output, that turn cannot be intercepted locally.
3. **Deterministic Tree Termination:**  
   Upon guard triggering, the runner immediately terminates the entire child process hierarchy using `taskkill /F /T /PID`, records `stop_reason = "PRE_DISPATCH_HEADROOM_EXHAUSTED"`, and sets `token_ceiling_breached = False` if cumulative tokens are below the ceiling.

---

## 2. Detailed Technical Audit (Six Codebase Questions)

### 2.1 Next-Turn Cost Estimator vs. Cumulative Reserve Logic
- **Code Reference:** [`benchmarks/matched-pair-harness/src/runner.py` lines 199–218](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L199-L218)
- **Finding:** The guard does **not** rely solely on a fixed cumulative threshold. It utilizes a dynamic estimator:
  ```python
  def estimate_next_request_tokens(self) -> int:
      if self.last_turn_request_tokens > 0:
          return max(self.last_turn_request_tokens, self.conservative_reserve)
      return self.conservative_reserve

  def check_pre_dispatch_headroom(self, ceiling: int) -> Tuple[bool, int, int]:
      remaining = ceiling - self.effective_tokens
      estimated = self.estimate_next_request_tokens()
      return (remaining >= estimated, remaining, estimated)
  ```
  The headroom check ensures that `remaining >= estimated`.

### 2.2 Event Types Triggering Guard Evaluation
- **Code Reference:** [`benchmarks/matched-pair-harness/src/runner.py` lines 168–197](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L168-L197)
- **Finding:** Pre-dispatch boundary detection (`is_pre_dispatch_boundary`) is triggered on:
  1. `turn.started` (Initial turn preflight).
  2. `custom_tool_call` and `custom_tool_call_output` (MCP tool execution lifecycle).
  3. `item.started` and `item.completed` for item types `command_execution`, `mcp_tool_call`, `file_edit`, `web_search`, and `tool_call`.
  4. `token_usage_record`, `event_msg`, and `turn.completed` whenever `has_pending_tool == True`.

### 2.3 Polling Interval and Implementation
- **Code Reference:** [`benchmarks/matched-pair-harness/src/runner.py` lines 535–544](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L535-L544)
- **Finding:** Polling runs inside `while proc.poll() is None:` with an explicit `time.sleep(0.05)` (50 ms responsive sleep). Stderr and stdout streams are read continuously by dedicated background daemon threads (`_read_stream`) appending to thread-safe memory buffers.

### 2.4 Threshold Calculation and Missing/Stale Telemetry Behavior
- **Code Reference:** [`benchmarks/matched-pair-harness/src/runner.py` lines 105–143, 216–218](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L105-L143)
- **Finding:**
  - Token tracking is calculated as `effective_tokens = max(self.thread_cumulative_max, self.turn_deltas_sum)`.
  - **Missing / Malformed Telemetry:** Handled safely via `try ... except Exception: pass`. Malformed lines are discarded without crashing the monitor.
  - **Stale Telemetry:** If Codex stdout delays emission of `token_usage_record`, `effective_tokens` retains its last known valid maximum.
  - **Initial State / Zero Telemetry:** If no usage event has been parsed, `effective_tokens` is `0`, `remaining` equals `ceiling`, and `estimated` equals `conservative_reserve` (15,000), allowing the initial turn to proceed.
  - **Delayed Overrun Catch:** If delayed telemetry flushes reveal that tokens already breached the ceiling, line 564 activates `token_ceiling_breached = True`, immediately escalating from pre-dispatch halt to a hard ceiling breach halt.

### 2.5 Process Stopping and Result Classification
- **Code Reference:** [`benchmarks/matched-pair-harness/src/runner.py` lines 625–658](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L625-L658)
- **Finding:**
  - Process termination is executed via `terminate_pid_tree(proc.pid)`, invoking Windows `taskkill /F /T /PID {pid}` to terminate Codex and all spawned child shells/tools, followed by `proc.kill()`.
  - Outcomes are strictly segregated:
    - If halted prior to dispatch with cumulative tokens $\le$ ceiling: `stop_reason = "PRE_DISPATCH_HEADROOM_EXHAUSTED"`, `token_ceiling_breached = False`.
    - If actual cumulative tokens exceed ceiling: `stop_reason = "TOKEN_CEILING_BREACHED"`, `token_ceiling_breached = True`.
    - If wall-clock elapsed time exceeds timeout: `stop_reason = "TIMEOUT"`.

### 2.6 Interception Boundary for Already-Dispatched Requests
- **Code Reference:** [`benchmarks/matched-pair-harness/src/runner.py` lines 537–542](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L537-L542)
- **Finding:** **The guard cannot recall or cancel an HTTP model request already dispatched across the network.**  
  If the child Codex process dispatches an HTTP inference request to the model provider endpoint within $< 50\text{ms}$ before the runner's polling loop reads the tool completion event, that turn will complete on upstream servers. Terminating the child tree prevents subsequent turns, but the in-flight turn's tokens are consumed.

---

## 3. Discrepancies Between Code and `amendment-05.md` Text

The following specific mismatches exist between the proposed draft text in `amendment-05.md` and the verified code implementation:

| Code Feature | Minimal Quote from `amendment-05.md` | Line Citation in `amendment-05.md` | Code Finding & Discrepancy Description |
| :--- | :--- | :--- | :--- |
| **Stop Trigger Type** | `"the 160,000 stop trigger"` | Line 72 | **Code is dynamic, text is static.** The code evaluates `remaining < max(last_turn, reserve)`. When `last_turn` $\approx 29,000$, the halt occurs at ~146,000 tokens, not 160,000. |
| **Headroom Reservation** | `"Pre-Dispatch Stop Trigger: 160,000 tokens (15,000 headroom reserve inclusive)"` | Line 99 | **15,000 is a minimum floor, not a fixed reserve.** Under the dynamic estimator in `runner.py:202`, the actual reserve expands to match the preceding turn cost ($26\text{k}\text{--}37\text{k}$). |
| **Rollback Trigger** | `"Pre-dispatch stop trigger: 235,000 tokens ($250\text{k} - 15\text{k}$)"` | Line 115 | **Same static oversimplification for Amendment 04.** Under Amendment 04, the code also uses dynamic estimation; 235,000 is only an upper bound. |

### Technical Conclusion
The actual implementation is **more protective** than the text of `amendment-05.md`. A static 160,000 cutoff allows large turns to breach the ceiling. The dynamic code stops early. However, `amendment-05.md` should be amended in a future pass to document the dynamic nature of the headroom calculation.
