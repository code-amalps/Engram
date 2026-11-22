# Engram Amendment 05 — Milestone 1: Safety & Architecture Limitations

## Executive Assessment

The token-guard remediation validated in Milestone 1 represents a significant engineering enhancement over the initial Block 07 post-run evaluation. By tracking cumulative token usage in real-time, accounting for cached/uncached prompt tokens and reasoning sub-metrics, and checking headroom against last-turn usage before turn dispatch, the harness successfully avoids running unconstrained loops.

However, based on empirical measurements and structural analysis of the external polling architecture, **the token guard CANNOT be claimed to provide a deterministic, zero-leakage hard ceiling.**

This document details the architectural boundaries, latency budget, race conditions, failure modes, and prerequisites for a true hard gate.

---

## 1. Architectural Architecture: External Polling vs. Internal Hooks

The current harness architecture operates entirely as an **external asynchronous observer**:

```text
+-----------------------------------------------------------------------------------+
| Matched-Pair Harness (Parent Process)                                             |
|                                                                                   |
|   +-----------------------+      Poll stdout (50 ms)     +--------------------+   |
|   | StreamingTokenTracker | <--------------------------- | Codex Process Tree |   |
|   +-----------------------+                              | (codex.exe, shell) |   |
|               |                                          +--------------------+   |
|               v                                                    ^              |
|   Headroom Check (< reserve)                                       |              |
|               |                                                    |              |
|               +----------> terminate_pid_tree() -------------------+              |
|                            (taskkill /F /T, 197-284 ms)                           |
+-----------------------------------------------------------------------------------+
```

### Key Differences Between External Polling and Internal Gating

| Architectural Attribute | External Poller (Current) | Internal Runtime Hook (Ideal) |
|:---|:---|:---|
| **Execution Point** | External parent reading buffered pipe | Synchronous callback inside Codex runtime |
| **Interception Timing** | Post-event observation (reactive) | Pre-request dispatch interception (proactive) |
| **Zero-Leakage Guarantee** | **NO** (Window of vulnerability exists) | **YES** (Outbound HTTP blocked before socket send) |
| **Process State Control** | Asynchronous SIGKILL via OS `taskkill` | Clean in-process state suspension / thread hold |
| **Dependency on Buffers** | Vulnerable to stdout line-buffering | Direct memory inspection of request state |

---

## 2. Quantified Latency Budget & Vulnerability Window

When Codex completes a tool call and prepares to invoke the LLM for the next turn, a strict race condition occurs between the agent's network stack and the harness's polling/kill loop.

### Empirical Latency Breakdown:

$$\text{Total Interception Delay} = T_{\text{event\_flush}} + T_{\text{poll\_interval}} + T_{\text{tracker\_eval}} + T_{\text{process\_kill}}$$

1. **Stdout Pipe Flush Latency ($T_{\text{event\_flush}}$):** $1 \text{ ms} - 15 \text{ ms}$
   - In Node.js / Rust subprocess runtimes, stdout lines may be chunk-buffered before flushing across the Windows anonymous pipe.
2. **Polling Loop Sleep Interval ($T_{\text{poll\_interval}}$):** $0 \text{ ms} - 50 \text{ ms}$
   - The runner loop executes `time.sleep(0.05)`. An event arriving immediately after sleep begins incurs the full 50 ms delay.
3. **Headroom Evaluation & Logic ($T_{\text{tracker\_eval}}$):** $< 1 \text{ ms}$
   - JSON parsing and headroom arithmetic are virtually instantaneous ($\approx 0.08\text{ ms}$).
4. **Process Tree Kill Latency ($T_{\text{process\_kill}}$):** $197 \text{ ms} - 284 \text{ ms}$
   - Measured across 20 trials on Windows 11 using `taskkill /F /T /PID <pid>`.
   - Windows must recursively walk the process tree, signal each child process handle, and deallocate memory.

### Total Window of Vulnerability:
$$\mathbf{T_{\text{total\_delay}} = 198\text{ ms to } 350\text{ ms (Median: } 280\text{ ms)}}$$

### The Race Condition:
- Time required for Codex to format and dispatch an outbound HTTPS request: **$\approx 15\text{ ms} - 45\text{ ms}$**.
- Because $T_{\text{dispatch}} (15-45\text{ ms}) \ll T_{\text{total\_delay}} (198-350\text{ ms})$, **the next inference request is already in flight across the internet and accepted by the model provider's API server before `taskkill` can terminate the process**.
- If that turn incurs 15,000 to 25,000 tokens, those tokens will be billed by the model provider even though the local process is killed seconds later.

---

## 3. Concurrency & High-Velocity Failure Modes

Beyond the standard latency budget, specific operational scenarios exacerbate leakage risk:

### 3.1 Fast-Path Tool Chains
When Codex executes rapid tool iterations without user interaction (e.g. searching 5 files in rapid succession), intermediate tool outputs are generated in fractions of a millisecond. If the token count crosses the threshold during turn $N$, the model call for turn $N+1$ may initiate before the harness thread wakes from `time.sleep()`.

### 3.2 Thread Starvation & Context Switching on Heavy Workloads
If the benchmark runner or child compile tools consume 100% CPU on all cores, the Python harness polling thread can be descheduled by the Windows OS scheduler for 50 ms to 100 ms. This extends $T_{\text{total\_delay}}$ past 400 ms.

### 3.3 Large Single-Turn Overshoot
If an arm is at 155,000 tokens (under the 160,000 stop trigger), and dispatches a turn with massive file context that costs 35,000 tokens, the turn will proceed to completion. The cumulative usage will reach 190,000 tokens, exceeding the 175,000 budget by 15,000 tokens despite the guard functioning exactly as designed.

---

## 4. Prerequisites for a Genuine Deterministic Hard Gate

To convert the current defensive monitor into a mathematically guaranteed zero-leakage hard cap, one of the following architectural mechanisms would be required:

1. **Local Forwarding HTTP Proxy with Quota Enforcement (Recommended)**:
   - Route all Codex outbound HTTP/HTTPS requests through a dedicated local proxy (e.g. custom Python `aiohttp` or Go proxy at `127.0.0.1:8888`).
   - The proxy tracks cumulative tokens directly from HTTP request/response headers.
   - When remaining budget $\le 0$ or headroom $< \text{reserve}$, the proxy **synchronously rejects** the outbound HTTP POST with an immediate HTTP 429 or 403, preventing the request from ever reaching the internet.
   - **Leakage: Exactly 0 tokens.**

2. **Windows OS Job Objects with Hard Limits**:
   - Assign the child process tree to a Windows Job Object.
   - While Job Objects can strictly restrict memory, CPU, and active process count, they do not inherently understand application-level token semantics without network packet filtering (Windows Filtering Platform / WFP).

3. **Codex Native Runtime Pre-Dispatch Interceptor**:
   - Implement a pre-request callback plugin directly inside Codex (`codex.exe`) that checks budget with the parent harness via synchronous IPC (e.g. named pipe or shared memory) before issuing `client.chat.completions.create()`.

---

## 5. Live Benchmark Pilot Readiness Verdict

### Operational Verdict:
> **The current token guard is SUITABLE for pilot authorization as a DEFENSIVE FAIL-FAST MONITOR, but MUST NOT be represented or relied upon as a deterministic zero-leakage hard cap.**

### Protocol Requirements for Amendment 05:
1. **Headroom Reserve Calibration**: The 15,000 token reserve is essential. Because $T_{\text{delay}} > T_{\text{dispatch}}$, the reserve absorbs the final in-flight turn rather than preventing it completely.
2. **Soft vs. Hard Cap Clarification**:
   - The 160,000 token stop trigger serves as an **operational dispatch barrier**.
   - The 175,000 token threshold serves as the **budget ceiling**.
   - Any run finishing between 160,001 and 175,000 is an expected consequence of final-turn latency and must be treated as valid censored data under Decision 2.
3. **Overrun Policy Enforcement**: Under Decision 2, any arm exceeding 175,000 tokens is classified as an overrun run, flagged, analyzed as censored data, and is strictly ineligible to support a savings claim.
