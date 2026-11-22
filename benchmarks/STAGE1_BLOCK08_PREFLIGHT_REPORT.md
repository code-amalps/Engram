# Engram — Stage 1 Block 08 Preflight Checklist & Verification Report

**Task Under Test:** `task_b_remember_me`  
**Block ID:** `block_08`  
**Preflight Mode:** Read-Only Verification  
**Status:** **READY FOR EXPLICIT AUTHORIZATION (WITH PROTOCOL LIMITATIONS NOTED)**

---

## 1. Preflight Checklist & Pass/Fail Verdicts

| # | Check Item | Status | Verification Evidence & Forensic Notes |
| :---: | :--- | :---: | :--- |
| **1** | **Block 08 Schedule, Seed, Task & Limits** | **PASS** | • **Task:** `task_b_remember_me` (preregistered JSON definition verified).<br>• **Order:** `["v154_arm3", "baseline_arm1", "v151_arm2"]` ([`orchestrator.py` lines 50–55](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L50-L55)).<br>• **Seed:** `2026102808`.<br>• **Per-Arm Limits:** `max_tokens_ceiling = 250,000` tokens; `timeout_seconds = 600` seconds.<br>• **Model / Reasoning:** `gpt-5.6-luna` / `medium`. |
| **2** | **First Arm Configuration (`v154_arm3`)** | **PASS** | • **Position:** Order 1 in Block 08.<br>• **Binary:** Frozen v1.5.4 executable at [`benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe).<br>• **Turn-0 Injection:** Active via [`runner.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L225-L255) (`engram inject -C <sandbox> <prompt>` prepends `.github/copilot-instructions.md` to initial model request; `turn0_injected = True`).<br>• **Tooling:** Engram v1.5.4 MCP server registered. |
| **3** | **Telemetry Preservation & Termination Safety** | **PASS** | • **Streaming Monitor:** [`StreamingTokenTracker`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L65-L125) monitors real-time cumulative token usage.<br>• **Process Termination:** Windows process tree killed via [`terminate_pid_tree(proc.pid)`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/isolation.py#L35-L55) (`taskkill /F /T /PID`).<br>• **Telemetry Recovery:** Partial rollout safely captured into `events.jsonl` and schema-valid `run_telemetry.json`.<br>• **Stop-on-Failure:** Orchestrator halts immediately upon non-SUCCESS arm exit. |
| **4** | **Evidence Capture Under Partial Triad Halt** | **PASS** | If `v154_arm3` (Order 1) succeeds and `baseline_arm1` (Order 2) breaches ceiling:<br>• `v154_arm3/run_telemetry.json`, `events.jsonl`, and `changes.patch` remain 100% intact with verified tokens, turns, and compiler gate results.<br>• `baseline_arm1/` records partial rollout and ceiling breach.<br>• `v151_arm2/` receives structured `SKIPPED_STOP_ON_FAILURE` record. |
| **5** | **Compiler & Test Acceptance Requirements** | **PASS** | In [`evaluator.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/evaluator.py#L570-L590), every enabled gate (`L0_typecheck`, `L2_unit_tests`, `L3_production_build`) must pass. If any gate fails, `mandatory_gates_passed = False`, forcing $C = 0.00$ and `passed_all_requirements = False`. A relevant patch without clean compilation cannot achieve acceptance. |
| **6** | **Single Block Termination Boundary** | **PASS** | [`main()`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L1085-L1130) executes `orchestrator.run_triad(block_id="block_08")` exactly once. There is no outer loop over subsequent blocks; process terminates immediately after Block 08 completes. |
| **7** | **Frozen Hashes, Seal & Block 06 Preservation** | **PASS** | • **v1.5.1 SHA-256:** `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` (Matches [`FREEZE_SEAL.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/FREEZE_SEAL.md)).<br>• **v1.5.4 SHA-256:** `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798`.<br>• **Preregistration SHA-256:** `db3ab2956a4da40aad8f1f376aa72a50bebc96f3e1ecc3120f405f0a294ea8bb` (Matches [`FREEZE_SEAL.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/FREEZE_SEAL.md)).<br>• **Block 06 Preserved:** Run folder `reports/runs/task_b_remember_me_block_06/` and forensic reports intact. |
| **8** | **Comparative Validity Analysis** | **FAIL\*** | *\*Conditional limitation:* See Section 2 below. |

---

## 2. Comparative Result Validity & Protocol Limitation

> [!WARNING]
> **Can the Current Protocol Produce a Valid Comparative Savings Result Under Existing Constraints?**
> **ANSWER: NO (Conditional on Baseline Ceiling Breach).**
> 
> - **Protocol Rule:** Under Amendment 02 Section 6 and preregistered criteria, comparative token savings ($S = \frac{T_{base} - T_{engram}}{T_{base}}$) are **strictly suppressed** unless **both** compared arms achieve functional acceptance ($C = 1.0$) and valid telemetry.
> - **Expected Block 08 Trajectory:**
>   1. **Arm 1 (`v154_arm3`):** Is expected to succeed ($C = 1.0$) with low token consumption (~30k–60k tokens) due to Turn-0 injection bypassing the multi-file exploration loop.
>   2. **Arm 2 (`baseline_arm1`):** As proven in Block 06, an unassisted agent exploring this 68k-file repository consumes >250,000 tokens before finishing typechecks. Under the mandated 250,000 ceiling, `baseline_arm1` is predicted to breach the ceiling again ($C = 0.00$).
>   3. **Triad Halt:** When `baseline_arm1` breaches the ceiling, stop-on-failure halts the triad, skipping Arm 3 (`v151_arm2`).
> - **Scientific Outcome:**
>   - The comparative analysis verdict will be `CONTROL_ACCEPTANCE_FAILURE`.
>   - Comparative percentage savings metrics will be **strictly invalidated (`null`)**.
>   - **What IS Achieved:** We will obtain **authoritative, standalone treatment telemetry for Engram v1.5.4 on Task B** under identical, reproducible conditions.

### Smallest Formal Protocol Decision Needed (For Comparative Delta)
If the scientific objective requires establishing a numerical comparative savings claim ($S$), the smallest formal protocol decision needed is:
- **Formal Protocol Amendment 03 (Task B Token Ceiling Calibration):**  
  Calibrate the per-arm ceiling for Task B from **250,000 to 350,000 tokens** to account for unassisted multi-file exploration in large Angular workspaces.  
  *(Note: This is an audit observation; no change has been implemented.)*

---

## 3. Proposed Execution Command

To execute Stage 1 Block 08 once authorized:

```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py --block block_08
```

---

*Preflight checklist complete. Standing by for explicit authorization before launching Block 08.*
