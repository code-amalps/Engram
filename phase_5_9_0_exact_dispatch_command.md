# Engram Phase 5.9.0: Exact Live Dispatch Commands

**Execution Phase:** 5.9.0  
**Target Systems:** Five-Arm Pentad Benchmark Orchestrator CLI (`orchestrator.py`)  
**Status:** VALIDATED FOR FUTURE DISPATCH ONLY (EXECUTION PROHIBITED IN PHASE 5.9.0)  

---

## 1. Operating Environment & Working Directory

- **Working Directory:** `C:\Users\Amal P S\OneDrive\Projects\Engram`
- **Python Runtime:** Python 3.14 (pinned virtual environment)
- **Required Credentials (by variable name only):**
  - `OPENAI_API_KEY` or active Codex appserver authentication token.
- **System Settings:**
  - `CARGO_NET_OFFLINE = "true"`

---

## 2. Command 1: Task A (`task_a_typo`) Five-Arm Pentad Dispatch

```powershell
Set-Location "C:\Users\Amal P S\OneDrive\Projects\Engram"

python benchmarks/matched-pair-harness/src/orchestrator.py `
  --mode pentad `
  --task task_a_typo `
  --pair pentad_live_pilot_task_a `
  --provider cloud `
  --model gpt-5.6-luna `
  --reasoning medium `
  --candidate-version cand5 `
  --dependency-strategy copy `
  --seed 59001 `
  --max-tokens-ceiling 250000 `
  --headroom-reserve 45000 `
  --global-tokens-ceiling 1000000 `
  --execution-policy strict `
  --output-dir benchmarks/matched-pair-harness/reports/runs/phase_5_9_live_pilot_task_a
```

### Expected Behavior (Task A):
- **Seed:** `59001`
- **Execution Permutation:**
  1. `arm_a_native`
  2. `arm_d_cand42_adaptive`
  3. `arm_b_cand3`
  4. `arm_c_cand42_always_on`
  5. `arm_e_cand5_optimized`
- **Ceilings:** 250,000 tokens per arm; 1,000,000 tokens cumulative block limit.
- **Expected Artifacts:**
  - `benchmarks/matched-pair-harness/reports/runs/phase_5_9_live_pilot_task_a/pentad_summary.json`
  - `benchmarks/matched-pair-harness/reports/runs/phase_5_9_live_pilot_task_a/pentad_report.md`

---

## 3. Command 2: Task B (`task_b_remember_me`) Five-Arm Pentad Dispatch

```powershell
Set-Location "C:\Users\Amal P S\OneDrive\Projects\Engram"

python benchmarks/matched-pair-harness/src/orchestrator.py `
  --mode pentad `
  --task task_b_remember_me `
  --pair pentad_live_pilot_task_b `
  --provider cloud `
  --model gpt-5.6-luna `
  --reasoning medium `
  --candidate-version cand5 `
  --dependency-strategy copy `
  --seed 59002 `
  --max-tokens-ceiling 500000 `
  --headroom-reserve 45000 `
  --global-tokens-ceiling 1500000 `
  --execution-policy strict `
  --output-dir benchmarks/matched-pair-harness/reports/runs/phase_5_9_live_pilot_task_b
```

### Expected Behavior (Task B):
- **Seed:** `59002`
- **Execution Permutation:**
  1. `arm_a_native`
  2. `arm_c_cand42_always_on`
  3. `arm_e_cand5_optimized`
  4. `arm_b_cand3`
  5. `arm_d_cand42_adaptive`
- **Ceilings:** 500,000 tokens per arm; 1,500,000 tokens cumulative block limit.
- **Expected Artifacts:**
  - `benchmarks/matched-pair-harness/reports/runs/phase_5_9_live_pilot_task_b/pentad_summary.json`
  - `benchmarks/matched-pair-harness/reports/runs/phase_5_9_live_pilot_task_b/pentad_report.md`

---

## 4. Execution Guard Reminder

> [!WARNING]
> These commands are fully validated against the Python CLI parser, but **MUST NOT BE EXECUTED** until explicit, separate administrative authorization for live benchmarking is granted.
