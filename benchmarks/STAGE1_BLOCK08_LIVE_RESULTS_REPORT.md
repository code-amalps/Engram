# Engram Stage 1 Core Triad — Block 08 Live Execution Results Report

**Document Identifier:** `ENGRAM_STAGE1_BLOCK08_LIVE_RESULTS_REPORT`  
**Execution Timestamp:** 2026-10-29T07:15:38Z – 2026-10-29T07:21:28Z (UTC) / 12:45 – 12:51 (IST)  
**Task Identifier:** `task_b_remember_me`  
**Block Identifier:** `block_08` (Scheduled Seed: `2026102808`)  
**Parent Protocol:** [`benchmarks/cost-baseline/PREREGISTRATION.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/PREREGISTRATION.md) & [`benchmarks/AMENDMENT_02_STAGE1_CORE_TRIAD.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/AMENDMENT_02_STAGE1_CORE_TRIAD.md)  
**Governing Amendment:** [`benchmarks/AMENDMENT_03_TASK_B_OPERATIONAL_CEILING_CALIBRATION.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/AMENDMENT_03_TASK_B_OPERATIONAL_CEILING_CALIBRATION.md) (Approved for Block 08 Only)  
**Execution Command:** `python benchmarks/matched-pair-harness/src/orchestrator.py --block block_08 --max-tokens-ceiling 350000`

---

## 1. Executive Summary & Primary Findings

Under explicit authorization and approved **Amendment 03**, Stage 1 Block 08 was executed on the live benchmark harness with the calibrated 350,000-token operational ceiling and 600-second per-arm timeout.

Block 08 counterbalanced schedule placed **Engram v1.5.4 (`v154_arm3`) in Execution Order 1**, followed by native baseline (`baseline_arm1`, Order 2) and Engram v1.5.1 (`v151_arm2`, Order 3).

### Key Empirical Findings:
1. **Engram v1.5.4 Breached the 350,000 Token Ceiling:**  
   Arm 1 (`v154_arm3`) accumulated **365,176 total model tokens** across 16 model turns (elapsed time: 260.34s) before the streaming runner circuit breaker terminated the process tree. Execution status was recorded as `TOKEN_CEILING_BREACHED`.
2. **Turn-0 Context Injection Succeeded but Was Not Self-Sufficient:**  
   Engram Turn-0 context injection correctly provided Change Contract `ctr_aae670aeb0684ac59d0750dc5618c49e` and AST excerpts for `core.module.ts`. However, because authentication state and storage logic resided in sibling subdirectories (`app/core/auth/` and `app/core/local-storage/`), the model conducted 6 sequential PowerShell `Get-Content` and `rg` exploration commands, expanding context from 16,187 to 25,340 tokens.
3. **Relevant Patch Implemented by Turn 9 (177,588 Tokens):**  
   At Turn 9, the agent authored a 33-line patch to `core.module.ts` implementing `RememberMeService` utilizing `LocalStorageService`.
4. **Failure Mode Identified: Deadlock Polling Loop on `tsc` (Turns 13–16):**  
   At Turn 12, the model invoked `npx tsc ...` with an overly aggressive `yield_time_ms: 1000` (1 second). Because TypeScript compilation in Angular takes >5 seconds, the command spawned a background interactive session (`session_id: 79526`). Across Turns 13–16, the model repeatedly polled `tools.write_stdin` every second, re-ingesting its ~27k-token context window on each turn. This 4-turn polling loop burned **108,723 tokens** in 30 seconds, driving cumulative consumption from 256,453 to 365,176 tokens.
5. **Mandatory Stop-on-Failure Halting:**  
   In strict compliance with Amendment 02 Section 6 and Amendment 03 Section 2.3, the orchestrator immediately halted the triad upon Arm 1's ceiling breach. Downstream arms (`baseline_arm1` and `v151_arm2`) were marked `SKIPPED_STOP_ON_FAILURE`.
6. **Comparative Claims Suppressed:**  
   All comparative token savings and efficiency claims are strictly **SUPPRESSED** (`MUTUAL_ACCEPTANCE_FAILURE`) as no arm completed with $C = 1.0$.
7. **Golden Repository Integrity 100% Preserved:**  
   Golden repository commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` and its dependency manifest hash (`f3366ca2ca59f8a3...`) were verified bit-for-bit intact before, during, and after execution.

---

## 2. Triad Arm Execution Summary

| Metric | Arm 3: Engram v1.5.4 (`v154_arm3`) | Arm 1: Native Baseline (`baseline_arm1`) | Arm 2: Engram v1.5.1 (`v151_arm2`) |
| :--- | :---: | :---: | :---: |
| **Execution Order** | **1 (Ran First)** | 2 (Downstream) | 3 (Downstream) |
| **Execution Status** | **`TOKEN_CEILING_BREACHED`** | `SKIPPED_STOP_ON_FAILURE` | `SKIPPED_STOP_ON_FAILURE` |
| **Elapsed Time (s)** | 260.34 s | 0.00 s | 0.00 s |
| **Total Model Tokens** | **365,176** | 0 (Skipped) | 0 (Skipped) |
| **Prompt Tokens (Total)** | 362,518 | 0 | 0 |
| **Prompt Tokens (Cached)** | 328,448 (90.6%) | 0 | 0 |
| **Prompt Tokens (Uncached)** | 34,070 (9.4%) | 0 | 0 |
| **Completion Tokens** | 2,658 | 0 | 0 |
| **Reasoning Tokens** | 761 | 0 | 0 |
| **Peak Context Window** | 27,283 tokens | N/A | N/A |
| **Model Requests / Turns** | 16 | 0 | 0 |
| **MCP Tool Calls** | 0 | 0 | 0 |
| **Files Changed** | 1 (`core.module.ts`) | 0 | 0 |
| **Insertions / Deletions** | +31 / -2 | 0 / 0 | 0 / 0 |
| **Correctness Score ($C$)** | **0.00** | **0.00** | **0.00** |
| **Functional Acceptance** | **FAIL ($C < 1.0$)** | **FAIL (Skipped)** | **FAIL (Skipped)** |
| **L0 Compilation Gate** | BLOCKED | SKIPPED | SKIPPED |
| **L2 Unit Tests Gate** | BLOCKED | SKIPPED | SKIPPED |
| **L3 Production Build Gate** | BLOCKED | SKIPPED | SKIPPED |
| **Telemetry Provenance** | `SESSION_ROLLOUT` (Valid) | `UNAVAILABLE` | `UNAVAILABLE` |

---

## 3. Turn-by-Turn Telemetry & Token Progression (`v154_arm3`)

The following data is extracted from the native session rollout `rollout-2026-10-29T12-46-05-01a18684-12ec-7e42-b098-386e03b049a2.jsonl` and verified against `events.jsonl`:

| Turn | Input (Total / Cached) | Output (Comp / Reason) | Turn Total | Cumulative Tokens | Action / Tool Invocations | Phase |
| :---: | :---: | :---: | :---: | :---: | :--- | :--- |
| **1** | 16,187 / 11,392 | 196 / 43 | 16,383 | 16,383 | `Get-Content core.module.ts` (Cell 1 initiated) | Exploration |
| **2** | 16,411 / 16,000 | 40 / 7 | 16,451 | 32,834 | `wait(cell_id: "1", yield: 10s)` | Exploration |
| **3** | 16,479 / 11,392 | 45 / 12 | 16,524 | 49,358 | `wait(cell_id: "1", yield: 10s)` | Exploration |
| **4** | 16,552 / 16,256 | 129 / 13 | 16,681 | 66,039 | `Get-Content core.module.ts -Raw` | Exploration |
| **5** | 17,904 / 16,256 | 144 / 33 | 18,048 | 84,087 | `rg -n "remember\|login\|authLogin..."` | Exploration |
| **6** | 20,242 / 17,664 | 214 / 32 | 20,456 | 104,543 | `Get-Content app.component.ts; local-storage...` | Exploration |
| **7** | 21,760 / 20,096 | 392 / 186 | 22,152 | 126,695 | `git status; rg -n "CoreModule\|..."` | Exploration |
| **8** | 24,615 / 21,632 | 160 / 12 | 24,775 | 151,470 | `Get-Content auth.effects.ts; core.state.ts` | Exploration |
| **9** | 25,340 / 24,448 | 778 / 308 | 26,118 | 177,588 | `tools.apply_patch(core.module.ts)` (Cell 7) | Implementation |
| **10** | 26,146 / 25,216 | 39 / 6 | 26,185 | 203,773 | `wait(cell_id: "7", yield: 10s)` | Implementation |
| **11** | 26,213 / 25,984 | 46 / 13 | 26,259 | 230,032 | `wait(cell_id: "7", yield: 10s)` | Implementation |
| **12** | 26,283 / 16,384 | 138 / 12 | 26,421 | 256,453 | `git diff; npx tsc ...` (`yield: 1000ms` -> `session: 79526`) | Verification Init |
| **13** | 26,922 / 26,112 | 55 / 7 | 26,977 | 283,430 | `tools.write_stdin(session: 79526, chars: "")` | Polling Loop 1 |
| **14** | 27,035 / 26,752 | 54 / 6 | 27,089 | 310,519 | `tools.write_stdin(session: 79526, chars: "")` | Polling Loop 2 |
| **15** | 27,146 / 26,880 | 79 / 31 | 27,225 | 337,744 | `tools.write_stdin(session: 79526, chars: "")` | Polling Loop 3 |
| **16** | 27,283 / 25,984 | 149 / 40 | 27,432 | **365,176** | `tools.write_stdin(session: 79526, chars: "")` [BREACH] | Polling Loop 4 |

---

## 4. In-Depth Forensic Analysis

### 4.1 Token Accumulation Mechanics
- **Context Re-Ingestion vs Completion:**  
  Of the 365,176 total tokens, **362,518 (99.27%)** were prompt tokens, and only **2,658 (0.73%)** were completion tokens. Prompt caching saved 328,448 tokens (90.6% cached), but total model tokens charged to quota still count cached prompt tokens.
- **Pre-Implementation Consumption (Turns 1–8):**  
  Context grew from 16,187 to 25,340 tokens. Cumulative usage reached 151,470 tokens before any patch was written.
- **The Polling Loop Multiplier (Turns 13–16):**  
  At Turn 12, cumulative tokens were 256,453 (well within the 350,000 ceiling). The agent executed `npx tsc -p projects/.../tsconfig.app.json --noEmit` with `yield_time_ms: 1000`. Because TypeScript compilation did not finish in 1,000 milliseconds, the runner created a background interactive session. The agent polled this session four consecutive times with empty stdin inputs. Each poll re-sent the 27,000-token context history, adding ~27,000 tokens per turn:
  $$\Delta T_{\text{poll}} = 365,176 - 256,453 = 108,723\text{ tokens in } 4 \text{ turns (28 seconds)}$$
  This caused the ceiling breach at Turn 16.

### 4.2 Why Zero MCP Tool Calls Were Recorded
1. **Engram Turn-0 Directive:**  
   The Turn-0 injection generated by Engram v1.5.4 explicitly directed the agent:
   > *"Change Contract `ctr_aae670aeb0684ac59d0750dc5618c49e` is ALREADY LOCKED to baseline revision... DO NOT CALL `engram_get_context` OR `engram_preflight`. All required context and boundaries are provided below."*
2. **Environment Tool Selection:**  
   Codex was executed with native JavaScript orchestration tools (`tools.exec_command`, `tools.apply_patch`, `tools.write_stdin`). The model followed the Turn-0 instruction not to call preflight/context tools and relied on PowerShell commands for investigation.

### 4.3 Git Impact & Code Quality
The patch generated at Turn 9 modified exactly one file:
- **File:** `projects/angular-ngrx-material-starter/src/app/core/core.module.ts`
- **Changes:** +31 insertions, -2 deletions
- **Content:** Added `REMEMBERED_USERNAME_KEY = 'AUTH.USERNAME'` and `@Injectable({ providedIn: 'root' }) export class RememberMeService`.
- **Target Boundary Compliance:** 100% compliant with authorized target file list.
- **Functional Acceptance:** Not achieved ($C = 0.0$) because the process was terminated before `npx tsc` completed and automated gates were blocked.

---

## 5. Comparative Savings Claims Status

Per Amendment 02 Section 6 and Amendment 03 Section 5:
- **Verdict:** `MUTUAL_ACCEPTANCE_FAILURE`
- **Claim Status:** **STRICTLY SUPPRESSED / INVALIDATED**
- **Explanation:** Neither Arm 3 (`v154_arm3`) nor Arm 1 (`baseline_arm1`) achieved functional acceptance ($C = 1.0$). Downstream arms were halted under mandatory stop-on-failure. No comparative token savings, latency reductions, or efficiency gains are claimed.

---

## 6. Audit & Environmental Verification

- **Harness & Binary Invariants:**
  - Frozen Engram v1.5.4 SHA-256: `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` (Verified)
  - Frozen Engram v1.5.1 SHA-256: `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` (Verified)
  - Preregistration Seal: `db3ab2956a4da40aad8f1f376aa72a50bebc96f3e1ecc3120f405f0a294ea8bb` (Verified)
  - Golden Repository Commit: `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` (Verified)
  - Golden Dependency Hash: `f3366ca2ca59f8a313b839369d1a3a258aeb1d784cfb7764f9b5c00eecce4be3` (Verified)
- **Schema Validation:**
  - `triad_summary.json`: PASS (`triad_summary.schema.json`)
  - `v154_arm3/run_telemetry.json`: PASS (`run_telemetry.schema.json`)
  - `baseline_arm1/run_telemetry.json`: PASS (`run_telemetry.schema.json`)
  - `v151_arm2/run_telemetry.json`: PASS (`run_telemetry.schema.json`)

---

## 7. Next Steps & Scope Governance

- **Block 08 Scope Concluded:** Execution of Block 08 is finished. No further actions will be taken on Block 08.
- **Blocks 07, 09, and 10 Remain Unexecuted:** In accordance with the user's explicit instructions, no subsequent blocks have been launched or scheduled.
- **Standing State:** Awaiting human review and further explicit guidance.
