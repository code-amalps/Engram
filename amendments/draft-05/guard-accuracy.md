# Streaming Token Guard Accuracy and Block 07 Overrun Forensic

**Document Identifier:** `ENGRAM_DRAFT05_GUARD_ACCURACY`  
**Governing Context:** Engram Amendment 05 Pass 4 Offline Feasibility Investigation  
**Authoritative Evidence:**  
- Block 07 Run Telemetry & Events: [`benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/v154_arm3/`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/v154_arm3/)  
- Block 07 Post-Mortem Report: [`benchmarks/STAGE1_BLOCK07_POST_MORTEM_ROOT_CAUSE_INVESTIGATION.md` lines 153–170](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/STAGE1_BLOCK07_POST_MORTEM_ROOT_CAUSE_INVESTIGATION.md#L153-L170)  
- Streaming Runner Source: [`benchmarks/matched-pair-harness/src/runner.py` lines 535–615](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L535-L615)  

---

## 1. Methodology and Error Formulation

### 1.1 Metrics and Alignment Formulae
To compare the harness's streaming token monitor against authoritative session-rollout totals:
1. **Telemetry Alignment:** Alignment is performed at turn boundaries using the model request identifier (`request_id`) and timestamps demarcated by `token_usage_record` events.
2. **Streaming Estimate ($T_{\text{stream}}$):** The cumulative token count tracked in real time by `StreamingTokenTracker.effective_tokens` at the moment a turn's boundary or tool completion is evaluated.
3. **Authoritative Rollout Total ($T_{\text{auth}}$):** The definitive cumulative token count recorded in the native OpenAI session rollout JSONL file on disk (`thread_token_usage.total_tokens`).
4. **Estimation Error:**
   $$\text{Error}_{\text{turn}} = T_{\text{stream}} - T_{\text{auth}}$$
5. **Observation Lag:** The wall-clock elapsed time between the client process dispatching an outbound HTTP model request and the arrival/processing of the usage payload by the external polling loop:
   $$\text{Lag}_{\text{turn}} = t_{\text{guard\_read}} - t_{\text{request\_dispatch}}$$

---

## 2. Turn-by-Turn Streaming vs. Rollout Telemetry Comparison (Block 07)

The table below aligns the real-time tracking behavior across all 11 turns of Block 07 Candidate Arm 3 (`v154_arm3`):

| Turn Number | Authoritative Turn Delta ($\Delta_{\text{auth}}$) | Authoritative Cumulative ($T_{\text{auth}}$) | Streaming Estimate at Boundary ($T_{\text{stream}}$) | Turn Error ($T_{\text{stream}} - T_{\text{auth}}$) | Telemetry Lag & Processing Dynamics |
| :---: | :---: | :---: | :---: | :---: | :--- |
| **Turn 1** | 15,914 | 15,914 | 15,914 | 0 | Synchronized at turn completion |
| **Turn 2** | 16,483 | 32,397 | 32,397 | 0 | Synchronized via rollout tailing |
| **Turn 3** | 16,674 | 49,071 | 49,071 | 0 | Synchronized via rollout tailing |
| **Turn 4** | 21,480 | 70,551 | 70,551 | 0 | Synchronized via rollout tailing |
| **Turn 5** | 24,404 | 94,955 | 94,955 | 0 | Synchronized via rollout tailing |
| **Turn 6** | 26,812 | 121,767 | 121,767 | 0 | Synchronized via rollout tailing |
| **Turn 7** | 29,513 | 151,280 | 151,280 | 0 | Synchronized via rollout tailing |
| **Turn 8** | 29,625 | 180,905 | 180,905 | 0 | Synchronized via rollout tailing |
| **Turn 9** | 29,736 | 210,641 | 210,641 | 0 | Synchronized via rollout tailing |
| **Turn 10** | 29,863 | 240,504 | 240,504 | 0 | Synchronized via rollout tailing |
| **Turn 11 (Dispatched)** | *30,002 in flight* | *270,506 in flight* | **240,504 (Stale during dispatch)** | **-30,002 (In flight)** | Remote inference in flight; usage unknown to client |
| **Turn 11 (Completed)** | 30,002 | 270,506 | **270,506** | 0 | Breach detected $\implies$ Immediate `taskkill` |

---

## 3. Forensic Trace of the Block 07 Overrun (Turn 10 to Turn 11)

### 3.1 Sequence of Events
The 20,506-token overrun beyond the 250,000 ceiling in Block 07 occurred through the following deterministic sequence:

1. **Turn 10 Completion:**  
   At the conclusion of Turn 10, cumulative model tokens reached **240,504**. The configured ceiling was **250,000**. The remaining token budget was **9,496 tokens**.
2. **State of the Harness During Block 07:**  
   In the active Block 07 harness, the pre-dispatch headroom guard had **not yet been deployed**. The harness runner evaluated only whether cumulative tokens had *already* breached the ceiling:
   $$\text{breached} = (\text{cumulative\_tokens} > 250,000)$$
   Because $240,504 \le 250,000$, the circuit breaker did not trigger.
3. **Turn 11 Dispatch:**  
   Codex CLI formatted its prompt history (accumulated TypeScript file dumps from Turns 4–7) and dispatched Turn 11 to OpenAI. The input context alone was **29,918 tokens**.
4. **The In-Flight Information Asymmetry:**  
   In the OpenAI API protocol, token consumption is computed and returned **atomically upon request completion**. There is no partial or streaming usage reported while the remote model generates tokens. Throughout Turn 11's network round-trip (~3.2 seconds), the harness monitor saw only that the process was running; its last known cumulative token count remained 240,504.
5. **Turn 11 Completion & Process Termination:**  
   Turn 11 finished with 29,918 input tokens and 84 output tokens ($30,002\text{ tokens}$ added), bringing cumulative tokens to **270,506**. The rollout file was updated on disk. The runner read the new line, detected $270,506 > 250,000$, and executed `terminate_pid_tree(proc.pid)`.

### 3.2 Root Cause Conclusion
The overrun was **not** caused by a failure of the termination command or a runaway process. It was caused by launching a 30,000-token turn into an unreserved 9,496-token margin.

---

## 4. Concurrency Boundary: Can External Polling Guarantee a Hard Ceiling?

> [!WARNING]
> **Technical Finding on Prevention:**  
> The guard **cannot be characterized as providing a mathematically guaranteed hard cap**.

### Why Zero-Window Prevention Cannot Be Guaranteed by an External Poller:
As documented in [`benchmarks/matched-pair-harness/src/runner.py` lines 537–542](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L537-L542):
1. **Subprocess Asynchrony:** The runner is an external parent Python process monitoring the child Codex CLI process via OS pipes (stdout/stderr) and filesystem tailing (`~/.codex/sessions`).
2. **Polling Granularity:** The polling loop sleeps for 50 ms (`time.sleep(0.05)`).
3. **The Dispatch Window:** When a local tool call completes, Codex receives the tool output and immediately initiates the next outbound HTTP model request. If network dispatch occurs in $< 50\text{ms}$ before the runner's next poll cycle reads the tool completion event, the HTTP request is already in flight across the internet.
4. **Uncancellable Server Consumption:** Terminating the child process kills the local client, but the upstream model endpoint processes the request. The tokens are burned on the server.
5. **Architectural Prerequisite for Hard Prevention:** A deterministic zero-window guarantee would require synchronous in-process middleware or a pre-request interceptor hook embedded inside the Codex CLI executable itself. An external parent monitor cannot guarantee zero leakage.
