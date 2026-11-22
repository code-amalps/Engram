# Task B Feasibility Analysis Under Proposed 175,000-Token Ceiling

**Document Path:** `amendments/draft-05/task-b-feasibility.md`  
**Governing Evaluation:** Engram Amendment 05 Pass 3 Offline Feasibility Investigation  
**Historical Task B Executions Audited:**  
1. **Block 06 Baseline Arm 1:** `benchmarks/stage1/runs/block_06_baseline_arm1/session/rollout-2026-10-28T22-09-02-01a18274-12ec-7e42-b098-386e03b049a2.jsonl`
2. **Block 07 Candidate Arm 3:** `benchmarks/stage1/runs/block_07_v154_arm3/session/rollout-2026-10-29T10-44-32-01a18610-12ec-7e42-b098-386e03b049a2.jsonl`
3. **Block 08 Candidate Arm 3:** `benchmarks/stage1/runs/block_08_v154_arm3/session/rollout-2026-10-29T12-46-05-01a18684-12ec-7e42-b098-386e03b049a2.jsonl` (Report: [`benchmarks/STAGE1_BLOCK08_LIVE_RESULTS_REPORT.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/STAGE1_BLOCK08_LIVE_RESULTS_REPORT.md))

---

## 1. Methodology and Definitions

### 1.1 Definition of "Exploration" Event Types
An event or tool call is classified as **Exploration** if it inspects, searches, reads, or queries repository state without modifying project code or running functional acceptance tests. Specifically:
- **File Reading Commands:** PowerShell `Get-Content`, `cat`, `head`, `tail`.
- **Search and Grep Commands:** `rg` (ripgrep), `findstr`, `grep`.
- **MCP Retrieval & Symbol Invocations:** `mcp__engram__engram_inspect_symbol`, `mcp__engram__engram_query_context`.
- **Git State Inspection:** `git status`, `git log`, `git diff` (read-only queries).
- **Diagnostics Prior to Edits:** Pre-edit compiler queries (`npx tsc`) executed solely to survey errors.

### 1.2 Non-Exploration Event Types
- **Implementation Events:** `tools.apply_patch`, file creation, file modification scripts.
- **Verification Events:** Interactive build/test runs (`npm test`, `ng test`, production build) executed after an edit has been made.

### 1.3 Denominator and Treatment of Runs Without an Edit
- **Exploration Tokens Before First Edit:** Sum of tokens consumed in turns strictly preceding the turn where the first code edit was executed.
- **Exploration Share:** $\frac{\text{Exploration Tokens Before First Edit}}{\text{Total Model Tokens Consumed in Run}}$.
- **Treatment of Runs Without an Edit (Block 07):** In Block 07, the candidate agent never authored a patch or edited any file before the ceiling halted execution. Under the primary operational interpretation, **100.0% of consumed tokens (270,506 tokens)** were spent on exploration. Under a strict grammatical interpretation where an edit milestone is a required precondition, this metric is marked `UNKNOWN (No Edit Milestone Observed)`. Both perspectives are documented.

---

## 2. Historical Run Milestones & Token Progression

### 2.1 Block 06: Native Baseline Arm 1 (`baseline_arm1`)
- **Rollout File:** `benchmarks/stage1/runs/block_06_baseline_arm1/session/rollout-2026-10-28T22-09-02-01a18274-12ec-7e42-b098-386e03b049a2.jsonl`
- **Configured Ceiling:** 250,000 tokens
- **Total Model Tokens:** 253,748 tokens (9 turns)
- **Outcome:** Halted at Turn 9 (`TOKEN_CEILING_BREACHED`)
- **Milestones:**
  - **First Genuine Engram MCP Call:** `NONE` (MCP server disabled in baseline arm).
  - **First File Edit:** **Turn 7** (Cumulative tokens: **180,866**; tool call: `apply_patch` on `core.module.ts`, rollout line 312).
  - **First Test Execution:** `NONE` (Run terminated before any test command was issued).
  - **Completion / Halt:** **Turn 9** (Cumulative tokens: **253,748**; ceiling breach halt, rollout line 428).
- **Exploration Analysis:**
  - Turns 1–6 were exclusively exploratory (`Get-Content`, `rg`, `git status`).
  - Exploration tokens before first edit = **145,999 tokens**.
  - Exploration share = $\frac{145,999}{253,748} = \mathbf{57.54\%}$.

---

### 2.2 Block 07: Candidate Arm 3 (`v154_arm3`)
- **Rollout File:** `benchmarks/stage1/runs/block_07_v154_arm3/session/rollout-2026-10-29T10-44-32-01a18610-12ec-7e42-b098-386e03b049a2.jsonl`
- **Configured Ceiling:** 250,000 tokens
- **Total Model Tokens:** 270,506 tokens (11 turns)
- **Outcome:** Halted at Turn 11 (`TOKEN_CEILING_BREACHED`)
- **Milestones:**
  - **First Genuine Engram MCP Call:** **Turn 2** (Cumulative tokens: **32,397**; tool call: `mcp__engram__engram_inspect_symbol`, rollout line 72). Note: The call was rejected by the runner client's approval policy, but constitutes a genuine MCP dispatch attempt.
  - **First File Edit:** `NONE` (Agent authored zero patches and edited zero files across all 11 turns).
  - **First Test Execution:** **Turn 7** (Cumulative tokens: **151,280**; command: `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json`, rollout line 280; pre-edit diagnostic).
  - **Completion / Halt:** **Turn 11** (Cumulative tokens: **270,506**; ceiling breach halt, rollout line 454).
- **Exploration Analysis:**
  - Entire 11 turns spent exploring and inspecting code.
  - Exploration tokens before first edit = **270,506 tokens** (or `UNKNOWN` under strict edit prerequisite).
  - Exploration share = $\mathbf{100.0\%}$ (or `UNKNOWN`).

---

### 2.3 Block 08: Candidate Arm 3 (`v154_arm3`)
- **Rollout File:** `benchmarks/stage1/runs/block_08_v154_arm3/session/rollout-2026-10-29T12-46-05-01a18684-12ec-7e42-b098-386e03b049a2.jsonl`
- **Report Reference:** [`benchmarks/STAGE1_BLOCK08_LIVE_RESULTS_REPORT.md` lines 20–29, 68–86](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/STAGE1_BLOCK08_LIVE_RESULTS_REPORT.md#L20-L29)
- **Configured Ceiling:** 350,000 tokens (calibrated under Amendment 03)
- **Total Model Tokens:** 365,176 tokens (16 turns)
- **Outcome:** Halted at Turn 16 (`TOKEN_CEILING_BREACHED`)
- **Milestones:**
  - **First Genuine Engram MCP Call:** `NONE` (Agent opted for native shell `Get-Content` and `rg`).
  - **First File Edit:** **Turn 9** (Cumulative tokens: **177,588**; tool call: `tools.apply_patch` on `core.module.ts`, rollout line 366).
  - **First Test Execution:** **Turn 12** (Cumulative tokens: **256,453**; command: `npx tsc ...`, rollout line 482).
  - **Completion / Halt:** **Turn 16** (Cumulative tokens: **365,176**; ceiling breach halt during background stdin polling loop, rollout line 612).
- **Exploration Analysis:**
  - Turns 1–8 were exclusively exploratory (`Get-Content`, `rg`, `git status`).
  - Exploration tokens before first edit = **151,470 tokens**.
  - Exploration share = $\frac{151,470}{365,176} = \mathbf{41.48\%}$.

---

## 3. Feasibility Summary Across All Historical Task B Executions

| Metric | Block 06 Baseline | Block 07 Candidate | Block 08 Candidate | Aggregate Total |
| :--- | :---: | :---: | :---: | :---: |
| **Completed Successfully $\le 175,000$ tokens** | 0 | 0 | 0 | **0 / 3 (0.0%)** |
| **Halted or Incomplete by 175,000 tokens** | 1 | 1 | 1 | **3 / 3 (100.0%)** |
| **Tokens at First Edit** | 180,866 (Turn 7) | `NONE` (No edit) | 177,588 (Turn 9) | All observed edits $\mathbf{> 175,000}$ |
| **Pre-Edit Exploration Tokens** | 145,999 | 270,506 | 151,470 | **Median: 151,470 tokens** |
| **Exploration Share of Total Tokens** | 57.54% | 100.00% | 41.48% | **Range: 41.5% – 100.0%** |

---

## 4. Empirical Conclusion

1. **Zero Completion Probability:**  
   Across all 3 historical live executions of Task B, **zero runs (0/3)** completed at or below 175,000 tokens.
2. **First Edit Exceeds Ceiling:**  
   In both runs where an edit was authored (B06 and B08), the agent required **177,588 to 180,866 tokens** just to reach the first file edit. Both values exceed the entire 175,000 ceiling.
3. **Headroom Guard Guarantees Premature Halt:**  
   Under Amendment 05, the dynamic headroom guard triggers at ~145,000–151,000 tokens. Therefore, an agent running Task B under Amendment 05 will invariably be terminated during codebase exploration or immediately before its first edit, rendering Task B completely incapable of demonstrating functional completion ($C = 1.0$) or supporting comparative token savings claims.
