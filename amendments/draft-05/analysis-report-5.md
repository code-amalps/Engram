# Engram Amendment 05 — Pass 5 Forensic Audit Consolidated Report

**Document Identifier:** `ENGRAM_DRAFT05_ANALYSIS_REPORT_5`  
**Governing Authorization:** Strict Offline Read-Only Forensic Audit  
**Operating Workspace:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Safety Status:** 0 Model / API Inference Credits Consumed  

---

## 1. Verified Offline Findings

1. **Repository & Binary Integrity:**  
   - Working tree HEAD: `c70845b410522197609f98485cb487f5d08409e5` (`feature/async-precommit-guard`).
   - Candidate binary (`candidate-phase1/engram.exe`): SHA-256 `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`.
   - Frozen v1.5.1 binary: SHA-256 `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0`.
   - Frozen v1.5.4 binary: SHA-256 `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798`.
   - Golden repository: Verified intact at commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` (18 modified user files, 29 `.engram/` metadata files verified bit-for-bit).
2. **Historical Task B Execution Record:**  
   Across all historical runs of Task B Remember Me, exactly three runs dispatched model requests:
   - `task_b_remember_me_block_06_baseline_arm1`: 9 turns, 253,748 tokens (breached 250k ceiling).
   - `task_b_remember_me_block_07_v154_arm3`: 11 turns, 270,506 tokens (breached 250k ceiling).
   - `task_b_remember_me_block_08_v154_arm3`: 16 turns, 365,176 tokens (breached 350k ceiling).
   All other arms were halted with 0 turns via protocol stop-on-failure or deferral rules.

---

## 2. Streaming Guard Evidence and Limitations

1. **Dual-Channel Telemetry Reality:**  
   - Codex CLI stdout (`events.jsonl`) emitted lifecycle events (`item.started`, `item.completed`), but **zero token events**.
   - Authoritative token data was written asynchronously by Codex CLI to disk rollouts (`~/.codex/sessions/**/*.jsonl`).
2. **Historical Runner vs. Remediation Implementation:**  
   - In Block 07, the historical runner called `proc.communicate(timeout=...)` synchronously. It had **no live streaming token tracking** and no live rollout tailing.
   - The `StreamingTokenTracker` and dynamic rollout poller represent **uncommitted remediation code** authored post-Block 07, not observed live historical execution.
3. **Concurrency and Zero-Window Limitations:**  
   - External 50ms polling cannot provide a mathematically guaranteed hard ceiling. Because model requests are dispatched internally by Codex CLI, a subsequent turn may already be in flight before an external poller terminates the child tree. True deterministic zero-window enforcement requires synchronous middleware embedded within the Codex binary.

---

## 3. Recomputed Turn-Size Statistics

Turn increments ($\Delta$) across historical Task B runs were recomputed across standardized percentile methodologies:

| Metric | B06 Baseline Arm 1 ($N=9$) | B07 Candidate Arm 3 ($N=11$) | Pass 4 Sample (B06+B07, $N=20$) | B08 Candidate Arm 3 ($N=16$) | Complete Task B (B06+B07+B08, $N=36$) |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Minimum $\Delta$** | 13,457 | 15,914 | **13,457** | 16,383 | **13,457** |
| **Median $\Delta$** | 30,132.0 | 26,812.0 | **28,162.5** | 25,446.5 | **26,151.5** |
| **$P_{90}$ (Nearest Rank)** | 37,651.0 | 29,863.0 | **34,867.0** | 27,225.0 | **32,030.0** |
| **$P_{90}$ (Linear Interp.)** | 35,715.0 | 29,863.0 | **34,903.4** | 27,157.0 | **31,081.0** |
| **Maximum $\Delta$** | 37,651 | 30,002 | **37,651** | 27,432 | **37,651** |
| **Mean $\Delta$** | 28,194.2 | 24,591.5 | **26,212.7** | 22,823.5 | **24,706.4** |

**Autoregressive Dependency Caveat:** Turns within a single run are strictly non-independent. Monotonic prompt context accumulation causes turn increments to grow from ~13k in Turn 1 to ~37k in Turn 9. Summary percentiles computed across all turns underestimate typical turn size at the ceiling boundary.

---

## 4. Exploration and First-Edit Findings

1. **First-Edit Timing Verification:**  
   - **B06 Baseline Arm 1:** First edit occurred at **Turn 7** (Cumulative tokens: **180,866**; 6 files modified via `apply_patch`). Pre-edit exploration spent **145,999 tokens**.
   - **B07 Candidate Arm 3:** **`NO EDIT OBSERVED`**. Entire run of 11 turns (**270,506 tokens**) was spent in exploration, an approval-blocked MCP call, and a hanging compiler.
   - **B08 Candidate Arm 3:** First edit occurred at **Turn 9** (Cumulative tokens: **177,588**; `core.module.ts` modified via `apply_patch`). Pre-edit exploration spent **151,470 tokens**.
2. **Exploration Token Attribution Fallacy:**  
   Equating whole-turn token deltas to individual shell commands (e.g. "File reads: 92k tokens") is methodologically invalid. In OpenAI API metering, $>90\%$ of late turn tokens represent re-ingested prior conversation history, not fresh command payload. In B08 Turns 2–3, subprocess polling added only 40 and 45 completion tokens, while prompt history accounted for $>16,400$ tokens per turn.

---

## 5. Reserve Simulation Results Under 175,000 Ceiling

Retrospective replay of candidate reserves (30k / 35k / 40k) against historical turn sequences reveals an identical outcome across all three options:

1. **Overrun Prevention:** In all runs, the simulated guard halts execution before an overrun occurs:
   - B06: Halts at **145,999 tokens** (before Turn 7 $\Delta=34,867$).
   - B07: Halts at **151,280 tokens** (before Turn 8 $\Delta=29,625$).
   - B08: Halts at **151,470 tokens** (before Turn 9 $\Delta=26,118$).
2. **Task Infeasibility:** Because Task B exploration historically required **146k–151k tokens** before authoring the first patch, halting at 135k–145k ensures that **zero runs reach their first code edit**. The agent is stopped purely as an exploration process with 0 files modified.

---

## 6. Material Pass 4 Claims Requiring Correction

1. **Engram Invocation in Block 07:** Pass 4 claimed the agent autonomously invoked Engram. *Correction:* The agent demonstrated autonomous intent, but the invocation was aborted locally by the Codex client (`approval policy is never`) and never reached the server.
2. **Streaming Estimator Accuracy:** Pass 4 claimed 0 estimation error at all turn boundaries in Block 07. *Correction:* Live streaming telemetry did not exist in Block 07; 0-error figures were derived from a post-hoc replay of remediation code.
3. **Turn Statistics Scope:** Pass 4 reported $N=20$ (omitting Block 08). *Correction:* Full Task B sample is $N=36$ (Median 26,151.5; $P_{90}$ 32,030); nearest-rank calculation method and non-independence caveat must be stated.
4. **Command Token Attribution:** Pass 4 attributed whole-turn token deltas to shell commands and polling. *Correction:* Token deltas reflect cumulative conversation history; isolated command costs are `UNKNOWN`.
5. **Reserve Feasibility:** Pass 4 presented 30k–40k reserves as a viable configuration for Task B under a 175k ceiling. *Correction:* While effective at stopping overruns, these reserves prevent the agent from ever reaching the first code edit.
6. **Authentication vs. Plan Limits:** Pass 4 established `auth_mode: "chatgpt"`. *Correction:* Local config confirms ChatGPT OAuth, but actual subscription tier, rate limits, and auto-purchase status remain `UNVERIFIED, owner to fill in`.

---

## 7. Open Owner Decisions

1. **Ceiling vs. Feasibility Tradeoff:**  
   - If per-arm ceiling is capped at 175,000 with a 30k–40k reserve, Task B cannot reach first edit unless prompt retrieval is optimized to complete exploration under 100k tokens.
   - Alternatively, evaluating complete task execution requires a higher operational ceiling (e.g. 250k–350k as in Blocks 06–08).
2. **Protocol Variant Selection:**  
   - **Variant A (Full Task B):** Retains full correctness gates ($C=1.0$), requiring higher token budgets.
   - **Variant B (First-Edit Stop):** Halts at first edit, waives downstream test execution, but requires authoring new harness event hooks in `runner.py`.
3. **Headroom Reserve Calibration:** Selection between 30k, 35k, or 40k reserve.
4. **Account Billing Verification:** Manual verification by the owner of the active ChatGPT subscription tier, usage reset window, and verified absence of auto-purchase / pay-as-you-go balance drawdowns.

---

## 8. Items Not Verifiable Offline

1. **Exact Outbound Dispatch Timestamps:** Sub-50ms HTTP dispatch timings are internal to the compiled Codex CLI binary and are not logged.
2. **Live Polling Race Outcomes:** Whether an external 50ms polling loop will reliably intercept an outbound request before socket creation during a live run cannot be verified offline.
3. **OpenAI Account Quotas:** Current subscription rate limits, plan tier (Plus vs Pro), and billing thresholds on `chatgpt.com` cannot be checked offline.
4. **Future Agent Exploration Trajectories:** Whether an agent with active Engram MCP tools will explore more concisely and reach first edit within 100k tokens cannot be determined without authorized live execution.
