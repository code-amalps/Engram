# Engram Phase 5.9.0.1: Aggregate Budget Enforcement & Cross-Invocation Protocol

**Phase:** 5.9.0.1  
**Scope:** Architecture Analysis of Cross-Invocation Budget Enforcement  
**Target:** `orchestrator.py`  
**Execution Mode:** Strictly Offline Analysis & Safe Operational Specification  

---

## 1. Architectural Reality: Independent Process Invocations

An audit of `orchestrator.py` reveals that each invocation of the script runs as an isolated, short-lived Python process:
- Invocation 1 (`task_a_typo`) starts, executes its scheduled arms, records `pentad_summary.json`, and terminates.
- Invocation 2 (`task_b_remember_me`) starts as a completely fresh OS process with zero inherited memory state.

There is **no shared daemon, no IPC socket, no shared SQLite database, and no global state file** connecting two independent `orchestrator.py` executions.

### 1.1 The False Assumption of Automatic Shared Enforcement
Phase 5.9.0 configured:
- Task A `--global-tokens-ceiling 1000000`
- Task B `--global-tokens-ceiling 1500000`
- Combined sum = $2,500,000\text{ tokens}$

It is mathematically invalid to claim that a single 2,500,000-token budget is "enforced" merely because the two statically configured numbers sum to 2.5 million:
1. **No Downward Reallocation:** If Task A consumes only $350,000$ tokens, Task B does not automatically gain access to the unused $650,000$ tokens; Task B remains constrained to its hardcoded $1,500,000$ ceiling.
2. **No Upward Offset / Overshoot Guard:** If Task A were to experience an unexpected surge or overshoot, Task B has no mechanism to reduce its own ceiling to compensate.
3. **Double Reserve Allocation:** Because each process applies its own `--headroom-reserve 45000`, the total protected reserve across the two runs is $45,000 + 45,000 = 90,000$ tokens, rather than a single shared 45,000 buffer.

---

## 2. Rigorous Distinction of Aggregate Metrics

| Metric | Value | Definition & Mechanism |
|---|---:|---|
| **Aggregate Planned Budget** | **2,500,000 tokens** | The intended target ceiling for the complete 10-arm pentad evaluation. |
| **Aggregate Enforced Budget (Static)** | **2,500,000 tokens** | The sum of two hard process-level limits ($1.0\text{M} + 1.5\text{M}$); each process strictly aborts if its local ceiling is reached. |
| **Aggregate Enforced Reserve** | **90,000 tokens** | Two distinct 45,000-token reserves enforced independently at the end of each task's usable headroom. |
| **Usable Aggregate Budget** | **2,410,000 tokens** | $2,500,000 - (45,000 \times 2)$. Dispatch of new turns is blocked across both tasks once $2.41\text{M}$ usable tokens are reached. |
| **Maximum Theoretical Exposure** | **2,532,768 tokens** | Static ceilings ($2.5\text{M}$) plus maximum single-turn in-flight response overshoots ($16,384 \times 2$). |

---

## 3. Safe Operational Protocol for Dynamic Aggregate Enforcement

To guarantee true aggregate budget enforcement without modifying production Python code (strictly adhering to the `NOT_AUTHORIZED` boundary for source modifications), the execution protocol must use a **Dynamic Reconciled Two-Stage Dispatch Protocol**:

```mermaid
sequenceDiagram
    autonumber
    actor Stakeholder as Operator / Stakeholder
    participant TaskA as Invocation 1 (Task A Orchestrator)
    participant Telemetry as Task A pentad_summary.json
    participant Calc as Budget Reconciler (PowerShell / Python wrapper)
    participant TaskB as Invocation 2 (Task B Orchestrator)

    Stakeholder->>TaskA: Dispatch Task A (Ceiling = 1,000,000; Reserve = 45,000)
    Note over TaskA: Runs 5 arms under deterministic seed 59001
    TaskA->>Telemetry: Writes pentad_summary.json (cumulative_global_tokens = T_A)
    TaskA-->>Stakeholder: Process Exits (Code 0 or Halt)

    Stakeholder->>Calc: Read T_A from pentad_summary.json
    Note over Calc: Calculate Dynamic Task B Ceiling:<br/>C_B = min(1,500,000, 2,500,000 - T_A)
    Calc-->>Stakeholder: Output C_B

    alt Remaining Budget C_B < 95,000 tokens (Arm ceiling + reserve)
        Stakeholder-->>Stakeholder: ABORT: Insufficient remaining aggregate budget for Task B
    else Remaining Budget C_B >= 95,000 tokens
        Stakeholder->>TaskB: Dispatch Task B with --global-tokens-ceiling C_B
        Note over TaskB: Runs 5 arms under deterministic seed 59002
        TaskB-->>Stakeholder: Process Exits
    end
```

### 3.1 Concrete PowerShell Reconciled Wrapper Script

An operator executes Invocation 1, reads the exact tokens reported in `results/live_pentad_pilot_phase590_task_a/pentad_summary.json`, and dynamically injects the remaining balance into Invocation 2:

```powershell
# Step 1: Execute Task A with its maximum allocation
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --mode pentad `
  --task task_a_typo `
  --seed 59001 `
  --global-tokens-ceiling 1000000 `
  --max-tokens-ceiling 250000 `
  --headroom-reserve 45000 `
  --output-dir results/live_pentad_pilot_phase590_task_a `
  --execution-policy diagnostic_continue

# Step 2: Extract authoritative tokens consumed from Task A summary
$summaryPath = "results/live_pentad_pilot_phase590_task_a/pentad_summary.json"
$taskAData = Get-Content $summaryPath | ConvertFrom-Json
$tokensSpentA = $taskAData.cumulative_global_tokens

# Step 3: Compute dynamically bounded Task B ceiling
$aggregateCeiling = 2500000
$remainingAggregate = $aggregateCeiling - $tokensSpentA
$taskBCeiling = [Math]::Min(1500000, $remainingAggregate)

Write-Host "Task A Spent: $tokensSpentA tokens"
Write-Host "Dynamic Task B Ceiling: $taskBCeiling tokens (Aggregate limit: $aggregateCeiling)"

# Safety Gate: Ensure sufficient tokens remain for at least Candidate 5
if ($taskBCeiling -lt 95000) {
    Write-Error "ABORTING: Remaining aggregate budget ($taskBCeiling) insufficient for safe Task B execution."
    exit 1
}

# Step 4: Dispatch Task B with the dynamically bounded ceiling
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --mode pentad `
  --task task_b_remember_me `
  --seed 59002 `
  --global-tokens-ceiling $taskBCeiling `
  --max-tokens-ceiling 500000 `
  --headroom-reserve 45000 `
  --output-dir results/live_pentad_pilot_phase590_task_b `
  --execution-policy diagnostic_continue
```

### 3.2 Protocol Safety Guarantees
1. Under this operational protocol, the sum of `tokensSpentA + tokensSpentB` is **strictly bounded by $2,500,000$ tokens**.
2. If Task A finishes in fewer tokens (e.g., $380,000$ tokens), Task B's ceiling is capped at $1,500,000$ (its natural saturation limit), preventing runaway consumption on legacy arms.
3. If Task A encounters higher consumption (e.g., $950,000$ tokens), Task B's ceiling is strictly throttled to $2,500,000 - 950,000 = 1,550,000$, ensuring zero leakage beyond the authorized ceiling.
