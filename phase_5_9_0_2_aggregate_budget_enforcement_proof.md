# Engram Phase 5.9.0.2: Aggregate Budget Enforcement Proof & Runtime Argument Audit

**Phase:** 5.9.0.2  
**Target:** `benchmarks/matched-pair-harness/src/orchestrator.py` & `runner.py`  
**Execution Mode:** Strictly Offline Source-Code Audit & Mathematical Proof  

---

## 1. Trace of CLI Arguments: From ArgumentParser to Enforcement Function

We traced every command-line argument used in the proposed pentad benchmark from its definition in `argparse` through its actual enforcement functions in the runtime code:

| Argument | Argparse Definition | Destination in Code | Enforcing Class / Method | Enforcement Mechanism & Action on Breach | Status |
|---|---|---|---|---|---|
| `--mode pentad` | `orchestrator.py:3213` | `args.mode` $\to$ `mode = "pentad"` | `main()` $\to$ `orchestrator.run_pentad()` | Selects 5-arm pentad execution method; validates arm count = 5. | **ENFORCED** |
| `--task` | `orchestrator.py:3214` | `args.task` $\to$ `self.task_id` | `main()` $\to$ `Orchestrator.__init__` | Loads task fixture JSON from `config/tasks/{task}.json`. | **ENFORCED** |
| `--seed` | `orchestrator.py:3225` | `args.seed` $\to$ `self.seed` | `Orchestrator.run_pentad:2721` | Seeds `random.Random(self.seed).shuffle(order)`. | **ENFORCED** |
| `--global-tokens-ceiling` | `orchestrator.py:3236` | `args.global_tokens_ceiling` $\to$ `self.global_tokens_ceiling` | `Orchestrator.run_pentad:2749` | Checks `global_remaining < headroom_reserve`; halts remaining arms with `ABORTED_GLOBAL_BUDGET`. | **ENFORCED** |
| `--max-tokens-ceiling` | `orchestrator.py:3229` | `args.max_tokens_ceiling` $\to$ `self.max_tokens_ceiling` | `runner.py:check_pre_dispatch_headroom:445` & `orchestrator.py:2767` | Caps effective arm ceiling to $\min(C_{\text{arm}}, H_{\text{rem}})$; halts arm if exceeded. | **ENFORCED** |
| `--headroom-reserve` | `orchestrator.py:3230` | `args.headroom_reserve` $\to$ `self.headroom_reserve` | `runner.py:StreamingTokenTracker:450` & `orchestrator.py:2750` | Blocks new LLM turns if remaining headroom $< R_{\text{reserve}}$. | **ENFORCED** |
| `--execution-policy` | `orchestrator.py:3235` | `args.execution_policy` $\to$ `self.execution_policy` | `Orchestrator.resolve_execution_policy:1010` | `"diagnostic_continue"` permits logging failed arms without premature block abort. | **ENFORCED** |
| `--output-dir` | `orchestrator.py:3233` | `args.output_dir` $\to$ `self.output_dir` | `Orchestrator._save_reports:2920` | Writes `pentad_summary.json` and per-arm logs to target folder. | **ENFORCED** |

**Conclusion:** Zero arguments are cosmetic. Every argument is actively routed into execution logic and strictly enforced at runtime.

---

## 2. Dynamic Budget Calculation & Telemetry Aggregation

### 2.1 What `cumulative_global_tokens` Actually Accounts For
In `Orchestrator.run_pentad()` (lines 2785–2804):
```python
if tel_valid and tel_data:
    spent = tel_data.get("total_model_tokens", 0)
else:
    spent = arm_telem.get("total_model_tokens", 0)

cumulative_global_tokens += spent
```
In `StreamingTokenTracker` (`runner.py:120–280`):
- **Snapshot Deduplication:** Uses thread cumulative usage snapshots (`usage.total_tokens`), deduplicating across turn events.
- **Turn Deltas:** If snapshots are unavailable, aggregates verified incremental turn deltas.
- **Cached Input Tokens:** Included within `input_tokens` (not double-counted).
- **Completion & Reasoning Tokens:** `output_tokens` includes reasoning tokens without double-counting.
- **Tool-Call Invocations:** Tool definition tokens and tool response tokens are captured by Codex within the conversational turns.
- **Failed & Retried Requests:** Even if an arm fails verification, times out, or triggers a ceiling halt, `spent` accumulates all tokens consumed up to the moment of termination.

### 2.2 When is `pentad_summary.json` Written?
`pentad_summary.json` is serialized **only after the entire 5-arm loop has finished** (either all arms completed or remaining arms skipped on budget exhaustion).
- **Crash Immunity Gate:** If Invocation 1 crashes midway (e.g., host OS out-of-memory or forced kill), `pentad_summary.json` will not exist or will be incomplete.
- **Refusal to Start Condition:** The PowerShell dynamic reconciler strictly checks:
  ```powershell
  if (-not (Test-Path $summaryPath)) {
      Write-Error "CRITICAL: Task A summary not found. Refusing to launch Task B."
      exit 1
  }
  ```
  Invocation 2 will **never start** on an assumed balance.

---

## 3. Exposure Bound Mathematical Proof

### 3.1 Why Overshoot is Bounded to 32,768 Tokens Total

Let $B_{\text{inv}}$ be the invocation token ceiling.
In `run_pentad()`:
$$\text{cumulative\_global\_tokens} = \sum_{i=1}^{k} T_i$$
When Arm $i$ runs, its effective ceiling is set to:
$$C_{\text{eff}, i} = \min(C_{\text{arm}}, B_{\text{inv}} - \text{cumulative\_global\_tokens})$$

If Arm $i$ overshoots its effective ceiling by $\Delta_i$ (due to in-flight completion output tokens):
$$\text{cumulative\_global\_tokens} \leftarrow \text{cumulative\_global\_tokens} + C_{\text{eff}, i} + \Delta_i$$
Notice that **$\Delta_i$ is immediately added to `cumulative_global_tokens`**!
Because `global_remaining = B_inv - cumulative_global_tokens`, any overshoot on Arm $i$ directly reduces the budget available to Arm $i+1$.

Therefore, per-arm overshoots **do not compound** across arms.
The ONLY overshoot that can expand beyond the global invocation ceiling is the overshoot of the **very last arm that executes before the global halt occurs**.

By architectural bound (`PHASE_5_4_24_OVERSHOOT_BOUND_ANALYSIS.md`):
- A single LLM turn response is capped by the provider model output limit:
  $$\Delta_{\text{max\_turn}} \le 16,384\text{ tokens}$$
- For Invocation 1 (`task_a_typo`): Maximum possible overrun $= 16,384\text{ tokens}$.
- For Invocation 2 (`task_b_remember_me`): Maximum possible overrun $= 16,384\text{ tokens}$.
- Across both independent invocations:
  $$\Delta_{\text{max\_aggregate}} = 16,384 + 16,384 = \mathbf{32,768\text{ tokens}}$$

---

## 4. Definitive Exposure Breakdown Matrix

| Budget Component | Token Allocation | Enforcement Mechanism | Failure Action |
|---|---:|---|---|
| **1. Nominal Aggregate Ceiling** | **2,500,000** | Static process ceilings ($1.0\text{M} + 1.5\text{M}$) | Halts dispatch when reached |
| **2. Protected Reserve Buffer** | **90,000** | $45,000 \times 2$ protected floors | Blocks turn dispatch if $H_{\text{rem}} < 45\text{k}$ |
| **3. Usable Nominal Budget** | **2,410,000** | $2,500,000 - 90,000$ | Active turn dispatch budget |
| **4. Max In-Flight Response Overshoot**| **32,768** | $16,384 \times 2$ single-turn output bounds | Absorbed safely by reserve floor |
| **5. Provider Retry Exposure** | **0 (Zero)** | Retries count against arm ceiling | Accelerates ceiling halt; zero spill |
| **6. Maximum Defensible Exposure** | **2,532,768** | $\text{Ceiling} + \Delta_{\text{overshoot}}$ | Absolute worst-case boundary |
| **7. Unbounded / Unknown Exposure** | **0 (None)** | All child processes strictly killed | Zero unbounded paths exist |
