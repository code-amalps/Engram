# Engram — Stage 1 Block 06 Live Triad Execution Report

**Execution Timestamp:** 2026-10-29T06:01:30Z – 2026-10-29T06:03:46Z (UTC)  
**Local Timestamp:** 2026-10-29T11:31:30 IST – 2026-10-29T11:33:46 IST  
**Task Under Test:** `task_b_remember_me` (Authentication flow with username persistence)  
**Block ID:** `block_06`  
**Planned Counterbalanced Order:** `["baseline_arm1", "v151_arm2", "v154_arm3"]` (Seed: `2026102806`)  
**Overall Triad Status:** **HALTED ON BREACHED LIMIT (ARM 1 TOKEN CEILING EXCEEDED)**  
**Comparative Token Savings Claims:** **STRICTLY SUPPRESSED (INVALIDATED)**

---

## 1. Executive Summary & Triad Outcomes

Stage 1 Core Triad Block 06 was executed under explicit authorization using the approved command:
```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py --block block_06
```

### Key Execution Highlights:
1. **Streaming Token Monitoring & Enforcement:**
   - During Turn 9 of Arm 1 (`baseline_arm1`), the cumulative token count reached **253,748 tokens**, exceeding the **250,000-token per-arm ceiling**.
   - Within **355 milliseconds** of the turn completion event being emitted (at `06:03:19.147Z`), `StreamingTokenTracker` detected the ceiling breach and triggered immediate process termination (`06:03:19.502Z`).
2. **Safe Process-Tree Termination:**
   - The harness invoked `terminate_pid_tree(proc.pid)` (`taskkill /F /T /PID`), safely and cleanly shutting down Codex and all child processes.
3. **Structured Partial Telemetry Preservation:**
   - Rather than raising an unhandled exception or losing telemetry, the harness recovered the complete 176-event stream from the native rollout directory (`.codex/sessions`), validated the telemetry schema, and recorded a complete, valid telemetry record.
4. **Deterministic Stop-on-Failure:**
   - In accordance with the preregistered stop-on-failure rule, the orchestrator immediately halted execution before launching Arm 2 (`v151_arm2`) and Arm 3 (`v154_arm3`), preventing unmetered quota consumption.
   - Structured `SKIPPED_STOP_ON_FAILURE` records were generated for both subsequent arms adhering to `run_telemetry.schema.json`.
5. **Golden Dependency Integrity:**
   - Post-execution verification confirmed the golden test repository (`angular-ngrx-material-starter`) remained 100% unmutated (`node_modules` file count: 68,274; recursive digest: `f63d2e4c069e14aa3cc4765fa327c6b7e6603d19b3a6945a9dd34eec8f074476`).

---

## 2. Per-Arm Results Breakdown

| Metric / Property | Arm 1: Native Baseline (`baseline_arm1`) | Arm 2: Engram v1.5.1 (`v151_arm2`) | Arm 3: Engram v1.5.4 (`v154_arm3`) |
| :--- | :---: | :---: | :---: |
| **Execution Order** | 1 (Executed) | 2 (Skipped) | 3 (Skipped) |
| **Execution Status** | `TOKEN_CEILING_BREACHED` | `SKIPPED_STOP_ON_FAILURE` | `SKIPPED_STOP_ON_FAILURE` |
| **Elapsed Duration** | **109.26 s** (1m 49s) | 0.0 s | 0.0 s |
| **Correctness Score ($C$)** | **0.00** / 1.00 | **0.00** / 1.00 | **0.00** / 1.00 |
| **Requirements Passed** | False (0/4 REQs) | False (Not Evaluated) | False (Not Evaluated) |
| **Automated Gates** | `L0: BLOCKED`, `L2: BLOCKED`, `L3: BLOCKED` | `ALL SKIPPED` | `ALL SKIPPED` |
| **Total Model Tokens** | **253,748** | **0** (*Not Measured*) | **0** (*Not Measured*) |
| **Prompt Tokens (Total)** | **250,026** | **0** (*Not Measured*) | **0** (*Not Measured*) |
| ↳ *Cached Prompt Tokens* | **221,824** (Directly Measured) | **0** (*Not Measured*) | **0** (*Not Measured*) |
| ↳ *Uncached Prompt Tokens* | **28,202** (Inferred) | **0** (*Not Measured*) | **0** (*Not Measured*) |
| **Completion Tokens** | **3,722** | **0** (*Not Measured*) | **0** (*Not Measured*) |
| ↳ *Reasoning Tokens* | **945** | **0** (*Not Measured*) | **0** (*Not Measured*) |
| **Peak Single-Request Context** | **37,231** tokens | None (*Not Measured*) | None (*Not Measured*) |
| **Model Requests / Turns** | 9 requests / 9 turns | 0 / 0 | 0 / 0 |
| **MCP Tool Calls** | 0 (No MCP registered) | 0 (*Not Run*) | 0 (*Not Run*) |
| **Files Modified in Sandbox** | 6 files (+45 / -7 lines) | 0 files | 0 files |
| **Telemetry Validity** | **VALID (`true`)** | **INVALID (`false`)** | **INVALID (`false`)** |
| **Telemetry Provenance** | `SESSION_ROLLOUT` | `UNAVAILABLE` | `UNAVAILABLE` |
| **Sandbox Integrity** | Verified Intact | Clean / Not Created | Clean / Not Created |

> [!NOTE]
> **Measurement Distinction:**  
> - For **Arm 1**, all token metrics were directly extracted from authoritative provider telemetry in the verified rollout session (`01a1863f-cd55-7ea0-a9fd-7c567ddfc3d5`).
> - For **Arms 2 and 3**, the reported values of `0` reflect unexecuted, skipped runs where no model calls were made. They represent **unmeasured/unavailable** telemetry, not zero-token executions.

---

## 3. Comparative Analysis & Scientific Claims

```
========================================================================================
Stage 1 Core Triad Result: [block_06]
  - Baseline Arm 1 Accepted: False (Score: 0.00)
  - v1.5.1 Arm 2 Accepted:   False (Score: 0.00, Skipped)
  - v1.5.4 Arm 3 Accepted:   False (Score: 0.00, Skipped)
  - Primary Savings Claim:   SUPPRESSED [MUTUAL_ACCEPTANCE_FAILURE]
  - Reason: Both arms failed functional correctness requirements.
            Comparative token savings and efficiency claims are STRICTLY INVALIDATED.
========================================================================================
```

In accordance with Section 6 of Amendment 02 and preregistered scientific safeguards:
1. **Primary Predeclared Outcome (Arm 3 vs Arm 1):** `MUTUAL_ACCEPTANCE_FAILURE`.
   - Comparative token deltas and percentage savings are strictly suppressed (`null`).
2. **Architectural Evolution (Arm 3 vs Arm 2):** `MUTUAL_ACCEPTANCE_FAILURE`.
   - Comparative claims suppressed (`null`).
3. **First-Generation Benchmark (Arm 2 vs Arm 1):** `MUTUAL_ACCEPTANCE_FAILURE`.
   - Comparative claims suppressed (`null`).

---

## 4. Resilience Repair Verification: Before vs After

| Dimension | Previous Unhardened Run (2026-10-28) | Hardened Run (2026-10-29) | Improvement / Verdict |
| :--- | :--- | :--- | :--- |
| **Ceiling Detection** | Checked only post-process exit | Streaming line-by-line inspection | **Active Real-Time Monitoring** |
| **Execution Duration** | 600.0s (Hard Timeout) | **109.26s** | **81.8% time saved** |
| **Token Accumulation** | 1,254,712 tokens (Overrun) | **253,748 tokens** | **79.8% token quota saved** |
| **Termination Method** | Unguarded process timeout | `terminate_pid_tree` (`taskkill /F /T`) | **Clean child-tree kill** |
| **Telemetry Capture** | Crashed (`FileNotFoundError`) | Partial rollout safely recovered | **Valid Schema Record** |
| **Subsequent Arms** | Crashed harness before handling | Handled via `stop-on-failure` | **Structured Skip Records** |
| **Integrity Post-Run** | Verified manually | Fully automated & verified | **Deterministic Parity** |

---

## 5. Artifact & Log References

- **Triad Summary JSON:** [`triad_summary.json`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/triad_summary.json)
- **Triad Markdown Report:** [`triad_report.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/triad_report.md)
- **Arm 1 Telemetry:** [`run_telemetry.json`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/baseline_arm1/run_telemetry.json)
- **Arm 1 Events Stream:** [`events.jsonl`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/baseline_arm1/events.jsonl)
- **Arm 1 Code Diff:** [`changes.patch`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/baseline_arm1/changes.patch)
- **Arm 2 Skipped Record:** [`run_telemetry.json`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/v151_arm2/run_telemetry.json)
- **Arm 3 Skipped Record:** [`run_telemetry.json`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/v154_arm3/run_telemetry.json)
- **Native Codex Rollout:** `C:\Users\Amal P S\.codex\sessions\2026\10\29\rollout-2026-10-29T11-31-31-01a1863f-cd55-7ea0-a9fd-7c567ddfc3d5.jsonl`
