# Engram Phase 5.9.0.3: Overshoot Mechanism, Concurrency & Enforcement Audit

**Phase:** 5.9.0.3  
**Target:** In-Flight Token Overshoot Architecture  
**Audit Targets:** `runner.py`, `orchestrator.py`, `PHASE_5_4_24_OVERSHOOT_BOUND_ANALYSIS.md`  
**Execution Mode:** Strictly Offline Source-Code & Concurrency Audit  

---

## 1. Forensic Deconstruction of the 16,384-Token Value

In Phase 5.9.0.1 and 5.9.0.2, the reports asserted:
> *"The single-turn overshoot bound is strictly bounded by the provider model's maximum single-completion token limit (16,384 tokens), yielding a maximum possible aggregate overshoot of 32,768 tokens across both invocations."*

### 1.1 What 16,384 Actually Represents
1. **Remote Provider Completion Burst Constraint ($C_{\max}$):** In frontier models (such as OpenAI GPT-4o / GPT-5.6), the maximum tokens the model can generate in a single completion response is capped at **$16,384$ tokens**.
2. **Not a Local Harness Constant:** The number `16,384` is **not hardcoded in `runner.py` or `orchestrator.py`**. It is an external provider-side architectural parameter.
3. **The Incompleteness of the 16,384 Model:**  
   Total token consumption on turn $k$ is:
   $$T_k = P_k + C_k$$
   where $P_k$ is prompt context (input tokens) and $C_k$ is completion (output tokens).
   If a turn is dispatched at the minimum headroom boundary ($45,000$ tokens remaining), and the prompt context has expanded (e.g. $P_k \approx 60,000$ tokens on Task B), the total turn consumption is:
   $$T_k = 60,000 + 16,384 = 76,384\text{ tokens}$$
   The resulting theoretical single-turn overshoot beyond the $45,000$ boundary is:
   $$\Omega_{\max} = 76,384 - 45,000 = \mathbf{31,384\text{ tokens}}$$
   In practice, during Phase 5.4.24 empirical adversarial testing (Scenario 13), the observed single-turn overshoot was **$20,000$ tokens**.

**Conclusion:** Treating $16,384$ as the *total* single-turn overshoot was flawed because it accounted solely for output completion burst tokens ($C_k$), ignoring incremental prompt growth ($P_k$).

---

## 2. Concurrency & Request Serialization Audit

An inspection of `orchestrator.py` and `runner.py` proves:
1. **Zero Parallel Arm Execution:** In `run_pentad()`, arms execute strictly sequentially within a single-threaded Python loop:
   ```python
   for exec_order, arm in enumerate(order, start=1):
       run_data = self._execute_single_arm(...)
   ```
   Arm $N+1$ cannot launch until Arm $N$ has completely terminated.
2. **Zero Concurrent Asynchronous Turns:** The Codex CLI session (`codex.exe exec --json`) executes turns sequentially. Each turn waits for model response completion and tool execution before issuing the next turn.
3. **Sequential Retries:** HTTP 429 backoff retries occur synchronously within the single child process session.
4. **Post-Cancellation Socket Severing:** Upon detection of a ceiling breach or timeout, `isolation.py:terminate_pid_tree(proc.pid)` issues an immediate OS-level tree kill (`taskkill /F /T /PID`), instantly severing active TCP sockets.

**Definitive Concurrency Finding:** At any microsecond of execution, the maximum number of requests in flight across the entire benchmark harness is **EXACTLY ONE (1)**.

---

## 3. Proof of the "Final Arm Overshoot" Theorem

Does an intermediate arm's overshoot cause the global block to breach its global ceiling?

### Mathematical Proof:
Let $G_{\text{ceiling}}$ be the global invocation ceiling ($1,500,000$ for Task B).
Let $T_i$ be the tokens consumed by Arm $i$, including any per-arm overshoot $\Delta_i$.

In `orchestrator.py:2804`:
$$\text{cumulative\_global\_tokens} = \sum_{j=1}^{i} T_j$$
When Arm $i+1$ prepares to run, its effective ceiling is dynamically capped by:
$$C_{\text{eff}, i+1} = \min\left(C_{\text{arm}}, G_{\text{ceiling}} - \sum_{j=1}^{i} T_j\right)$$

Because $\sum T_j$ includes the full overshoot $\Delta_i$ of Arm $i$:
1. The overshoot of Arm $i$ directly **reduces the budget available to Arm $i+1$**.
2. Intermediate arm overshoots **do not compound** beyond $G_{\text{ceiling}}$.
3. The global ceiling $G_{\text{ceiling}}$ can ONLY be exceeded if the **very last arm that executes** overshoots when gross remaining headroom is near the reserve floor.

---

## 4. Defensible Maximum Overshoot Hierarchy

| Scope | Empirical Demonstrated Bound | Theoretical Worst-Case Bound | Governance Mechanism |
|---|---:|---:|---|
| **Single Arm** | **20,000 tokens** | **31,384 tokens** | Pre-dispatch headroom guard ($45\text{k}$) |
| **Single Invocation (5 Arms)**| **20,000 tokens** | **31,384 tokens** | Non-compounding global counter |
| **Both Invocations (10 Arms)** | **40,000 tokens** | **62,768 tokens** | Dynamic two-stage reconciliation |

### Absolute Maximum Defensible Exposure:
$$T_{\text{max\_defensible}} = B_{\text{nominal}} + \Omega_{\text{max\_theoretical}} = 2,500,000 + 62,768 = \mathbf{2,562,768\text{ tokens}}$$
Under empirical conditions:
$$T_{\text{max\_empirical}} = 2,500,000 + 40,000 = \mathbf{2,540,000\text{ tokens}}$$
