# Engram Pass 4 Evidence Claims Audit

**Document Identifier:** `ENGRAM_DRAFT05_PASS4_CLAIM_AUDIT`  
**Governing Standard:** Forensic Evaluation of Pass 4 Reports Against Raw Historical Evidence  
**Documents Reviewed:** `engram-usage-analysis.md`, `exploration-breakdown.md`, `guard-accuracy.md`, `amendment-05-draft-v2.md`, `billing-checklist-v2.md`, `analysis-report-4.md`  
**Status:** Strictly Offline Audit — No Reviewed Files Mutated; Corrections Proposed Herein  

---

## 1. Audit Matrix of Material Pass 4 Claims

| # | Topic / Target Area | Source Document & Location | Claim Under Review | Supporting Evidence & Source Identifier | Verdict | Minimal Required Correction |
| :-: | :--- | :--- | :--- | :--- | :---: | :--- |
| **1** | **Engram Autonomous Invocation in Block 07** | `engram-usage-analysis.md#L18, L89`, `analysis-report-4.md#L30` | The agent autonomously decided to invoke Engram (`engram_inspect_symbol`) on Turn 2. | [`v154_arm3/events.jsonl#L6-L7`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/v154_arm3/events.jsonl#L6-L7), rollout L20–L24. | **`PARTIALLY SUPPORTED`** | The model did dispatch the call autonomously, but it was rejected locally by the Codex client (`approval policy is never`). Clarify that this was an *attempted autonomous invocation aborted locally prior to server execution*. |
| **2** | **Exact Streaming Guard Accuracy** | `guard-accuracy.md#L30-L44`, `analysis-report-4.md#L45-L50` | The streaming token estimator tracked cumulative tokens with 0 error at every turn boundary. | Historical [`runner.py#L171-L183`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L171-L183) (used `proc.communicate()`), `events.jsonl`. | **`UNSUPPORTED`** | In the live Block 07 run, the historical runner had NO streaming tracker. The 0-error claim was derived from a post-hoc replay of the uncommitted remediation code. Label live streaming estimates as `UNKNOWN` and replay estimates as `RECONSTRUCTED (SIMULATED)`. |
| **3** | **Turn-Size Statistics ($N=20$)** | `turn-size-analysis.md#L45-L55`, `analysis-report-4.md#L40-L44` | Overall Task B turn increments: $N=20$, Median 28,162.5 / 28,163, $P_{90}$ 34,867, Max 37,651. | Rollouts for B06 (9 turns), B07 (11 turns), and B08 (16 turns). | **`PARTIALLY SUPPORTED`** | The $N=20$ figures are mathematically accurate for B06+B07 under Nearest Rank, but omitted Block 08 ($N=16$). Incorporating all Task B turns ($N=36$) yields Median 26,151.5 and $P_{90}$ 32,030. State the autoregressive context dependency caveat. |
| **4** | **Exploration Token Attribution to Commands** | `exploration-breakdown.md#L47-L50, L100-L104`, `analysis-report-4.md#L32-L38` | File reads consumed 92,783 tokens in B06; subprocess polling consumed 32,975 tokens in B08. | Rollout token payloads (`usage` vs. `thread_token_usage`). | **`UNSUPPORTED`** | OpenAI meters tokens per request, where $>90\%$ of late turn tokens represent re-ingested conversational prompt history. In B08 Turns 2–3, polling output was only 40 and 45 tokens. Mark category-specific token allocations as `UNKNOWN`. |
| **5** | **Reserve Sufficiency & Feasibility** | `amendment-05-draft-v2.md#L42-L48`, `turn-size-analysis.md#L75-L85`, `analysis-report-4.md#L52-L56` | A 30k–40k reserve is essential and sufficient to prevent overruns while executing Task B. | Replay simulations in `reserve-simulation.md`. | **`PARTIALLY SUPPORTED`** | The reserve does prevent overruns on historical turns, but it halts runs during exploration (145k–151k) *before any code edit can occur*. Furthermore, external polling cannot guarantee hard safety. State that 175k cap makes task completion infeasible. |
| **6** | **ChatGPT Authentication vs. Plan Limits** | `billing-checklist-v2.md#L12-L17`, `amendment-05-draft-v2.md#L62-L68`, `analysis-report-4.md#L58-L62` | Authentication mode is `chatgpt`, eliminating platform USD balance drawdown. | `~/.codex/auth.json` (`auth_mode: "chatgpt"`), rollout L82 (`plan_type: "go"`). | **`SUPPORTED` (Config) / `UNKNOWN` (Plan)** | Local config evidence confirms `auth_mode: "chatgpt"`, but actual account plan tier, rate limits, and overage billing terms cannot be determined offline and remain `UNVERIFIED, owner to fill in`. |

---

## 2. Detailed Technical Audit of Core Claims

### 2.1 Autonomous Model Intent vs. MCP Server Invocation
- **Pass 4 Statement:** Pass 4 asserted that the agent "autonomously invoked Engram in Block 07".
- **Evidence Review:** Rollout line L21 confirms the assistant generated a call to `tools.mcp__engram__engram_inspect_symbol`. However, stdout event log [`v154_arm3/events.jsonl#L7`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/v154_arm3/events.jsonl#L7) shows:
  ```json
  {"type":"item.completed","item":{"id":"call_0de14df1458eb60f016ae34c219fc08191bc1e12bfcf4aea06","type":"mcp_tool_call","server":"engram","tool":"engram_inspect_symbol","error":{"message":"MCP tool call requires approval, but approval policy is never"}}}
  ```
- **Correction:** The model demonstrated autonomous *intent* to invoke Engram, but the invocation was aborted by the Codex CLI client. The Engram process never received the request, never inspected the symbol, and never returned context. In Block 08, the agent did not attempt to invoke Engram at all.

### 2.2 Streaming Estimator Accuracy Conflation
- **Pass 4 Statement:** `guard-accuracy.md` presented a table showing turn errors of exactly `0` across Turns 1–10 in Block 07.
- **Evidence Review:** The historical runner (`runner.py` at commit `e341108`) did not read stdout line-by-line, did not tail session rollouts, and contained no token estimation code.
- **Correction:** The table in `guard-accuracy.md` simulated the behavior of the uncommitted `StreamingTokenTracker` on recorded data. Live historical streaming estimates were non-existent (`UNKNOWN`). In live runs, stdout contained no token information.

### 2.3 Command-Level Token Cost Fallacy
- **Pass 4 Statement:** `exploration-breakdown.md` reported category token allocations by summing turn deltas (e.g. 92,783 tokens for file reads in B06; 32,975 tokens for polling in B08).
- **Evidence Review:** In Block 08 Turns 2 and 3:
  - Turn 2: Input tokens: 16,411 (16,000 cached); Output tokens: 40; Delta: 16,451 tokens.
  - Turn 3: Input tokens: 16,479 (11,392 cached); Output tokens: 45; Delta: 16,524 tokens.
- **Correction:** Over 99.7% of the tokens in these turns represented re-ingested prior conversation history. The incremental completion generated by the polling tool was under 50 tokens. Category token allocations must be marked `UNKNOWN` or presented with explicit caveats that turn totals reflect cumulative conversational context.

### 2.4 Reserve Feasibility Under 175,000 Ceiling
- **Pass 4 Statement:** Pass 4 proposed a 30k–40k reserve under a 175,000 ceiling as a viable protocol variant for Amendment 05.
- **Evidence Review:** As proven in `reserve-simulation.md`:
  - Minimum exploration cost before first edit in historical runs was **145,999 tokens** (B06) and **151,470 tokens** (B08).
  - Stop triggers for 30k, 35k, and 40k reserves are **145,000**, **140,000**, and **135,000 tokens**.
- **Correction:** While these reserves prevent overruns, they ensure that Task B **cannot be completed or even edited**. Halting at 135k–145k terminates runs during exploration with 0 files modified. If a 175,000 ceiling is maintained, task completion cannot be expected without prompt retrieval optimizations that compress exploration context below 100k tokens.
