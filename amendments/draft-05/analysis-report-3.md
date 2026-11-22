# Engram Amendment 05 Comprehensive Offline Analysis Report (Pass 3)

**Document Identifier:** `ENGRAM_DRAFT05_ANALYSIS_REPORT_3`  
**Governing Context:** Engram Amendment 05 Pass 3 Offline Feasibility Investigation  
**Execution Timestamp:** 2026-10-29T17:25:00Z (UTC) / 22:55:00 (IST)  
**Parent Directory:** `C:\Users\Amal P S\OneDrive\Projects\Engram\amendments\draft-05/`  
**Operational Status:** STRICTLY OFFLINE — ZERO INFERENCE CREDITS CONSUMED — NO LIVE RUNS

---

## 1. Executive Summary

This report consolidates all empirical data, codebase audits, and protocol evaluations completed under Pass 3 of Engram's Amendment 05 preparation. All tasks were conducted entirely offline with zero model/API inference calls. Active harness code, golden repository files, and historical benchmark artifacts were inspected in read-only mode and preserved bit-for-bit. In strict accordance with instructions, `amendment-05.md` was left untouched.

---

## 2. Category I: Observed Empirical Evidence

### 2.1 Per-Turn Historical Token Increments ($N=20$ turns)
From native session rollouts of Block 06 Baseline Arm 1 ($N=9$ turns, total: 253,748 tokens) and Block 07 Candidate Arm 3 ($N=11$ turns, total: 270,506 tokens):
- **Observed Turn Increments ($\Delta$):**
  - **Block 06 (Baseline Arm 1):** Turn 1: 13,457; Turn 2: 21,828; Turn 3: 23,084; Turn 4: 25,468; Turn 5: 30,132; Turn 6: 32,030; Turn 7: 34,867; Turn 8: 35,231; Turn 9: 37,651.
  - **Block 07 (Candidate Arm 3):** Turn 1: 15,914; Turn 2: 16,483; Turn 3: 16,674; Turn 4: 21,480; Turn 5: 24,404; Turn 6: 26,812; Turn 7: 29,513; Turn 8: 29,625; Turn 9: 29,736; Turn 10: 29,863; Turn 11: 30,002.
- **Monotonic Expansion:** Later turns invariably cost between 26,000 and 37,651 tokens due to full context window re-ingestion.

### 2.2 Task B Historical Milestones and Completion
Across all 3 historical live runs of Task B:
- **Completed Successfully $\le 175,000$ tokens:** **Exactly 0 / 3 (0.0%)**.
- **Halted or Incomplete by 175,000 tokens:** **3 / 3 (100.0%)**.
- **First MCP Tool Call:** Block 06: None; Block 07: Turn 2 (32,397 tokens); Block 08: None.
- **First Edit Attempt:** Block 06: Turn 7 (**180,866 tokens**); Block 07: **None** (0 edits); Block 08: Turn 9 (**177,588 tokens**).
- **First Test Execution:** Block 06: None; Block 07: Turn 7 (**151,280 tokens**, pre-edit typecheck); Block 08: Turn 12 (**256,453 tokens**).
- **Completion / Halt:** Block 06: Turn 9 (253,748 tokens); Block 07: Turn 11 (270,506 tokens); Block 08: Turn 16 (365,176 tokens). All runs terminated due to ceiling breach.

### 2.3 Authentication and Billing Configuration
- **Auth Mode:** Offline inspection of `~/.codex/auth.json` conclusively identifies `auth_mode = "chatgpt"`.
- **Environment API Key:** `OPENAI_API_KEY` is not present in the environment.
- **Credential Storage:** `~/.codex/auth.json` contains OAuth session tokens (`tokens` object) tied to a ChatGPT Plus/Team subscription. Zero secrets or keys were printed or recorded.
- **Billing Nature:** Inferences consume rolling subscription allocation quotas, not dollar-denominated metered API credits.

---

## 3. Category II: Derived Calculations and Audits

### 3.1 Statistical Distribution of Turn Increments
- **Block 06 Baseline Arm 1:** Median = **30,132**; $P_{90}$ = **37,651**; Max = **37,651**; Mean = 28,194.2.
- **Block 07 Candidate Arm 3:** Median = **26,812**; $P_{90}$ = **29,863**; Max = **30,002**; Mean = 24,591.5.
- **Overall Combined ($N=20$):**
  - **Median Turn Increment:** **28,162.5 tokens**
  - **90th Percentile ($P_{90}$):** **34,867 tokens**
  - **Maximum Turn Delta:** **37,651 tokens**
  - **Mean Turn Increment:** **26,212.7 tokens**

### 3.2 Exploration Token Share Before First Edit
- **Block 06 Baseline:** Exploration consumed **145,999 tokens** out of 253,748 total $\implies \mathbf{57.54\%}$ exploration share.
- **Block 07 Candidate:** Exploration consumed **270,506 tokens** out of 270,506 total $\implies \mathbf{100.0\%}$ exploration share (or `UNKNOWN` if edit milestone is strictly required).
- **Block 08 Candidate:** Exploration consumed **151,470 tokens** out of 365,176 total $\implies \mathbf{41.48\%}$ exploration share.

### 3.3 Guard Code Implementation Audit & Inconsistencies in Draft Documents
Auditing [`runner.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L199-L218) and [`orchestrator.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L85-L86) identified three key mismatches against `amendment-05.md`:
1. **Dynamic vs. Static Stop Trigger:** `amendment-05.md` (lines 72, 99) asserts a static "160,000 stop trigger". The code implements a dynamic estimator: `remaining < max(last_turn_tokens, reserve)`. With turn deltas reaching ~29,000 tokens, the code triggers stopping at **145,000 to 151,000 tokens**, not at 160,000.
2. **Headroom Reservation Nature:** `amendment-05.md` treats 15,000 as a fixed reserve. In `runner.py:202`, 15,000 is merely a minimum floor; the effective reserve expands to match preceding turn cost.
3. **Rollback Trigger Description:** Line 115 describes Amendment 04's trigger as 235,000 ($250\text{k} - 15\text{k}$). In reality, Amendment 04 also uses dynamic estimation.

### 3.4 Configuration Feasibility Without Modifying Frozen Files
- **CLI Runtime Overrides:** Running `python orchestrator.py --max-tokens-ceiling 175000 --headroom-reserve 15000` requires **zero file modifications**.
- **Permanent Code Defaults:** If defaults are changed, only `orchestrator.py` (lines 85, 1318) and `runner.py` (line 235) need updates.
- **Frozen File Status:** Neither file belongs to the frozen binary set (`v1.5.1`, `v1.5.4`, `candidate-phase1`). Zero frozen files are affected.

---

## 4. Category III: Unadopted Proposals & Protocols

### 4.1 Headroom Reserve Proposals
- **Proposal 1:** 30,000 reserve $\implies$ nominal stop trigger at 145,000 tokens (Candidate $P_{90}$ aligned).
- **Proposal 2:** 35,000 reserve $\implies$ nominal stop trigger at 140,000 tokens (Overall $P_{90}$ aligned).
- **Proposal 3:** 40,000 reserve $\implies$ nominal stop trigger at 135,000 tokens (Worst-case max aligned).

### 4.2 Protocol Variants
- **Variant A (Bounded Task Protocol):** Scopes the task to $\le 3$ files and $\le 6$ turns, allowing functional completion ($C=1.0$) and valid token savings claims ($S$).
- **Variant B (Exploratory Task B Protocol):** Retains Task B, formalizing it as purely descriptive, waiving functional completion ($C=1.0$), and forbidding comparative savings claims.

---

## 5. Open Owner Decisions

The following policy and technical decisions remain open for project owner resolution before any future live testing:

1. **Protocol Variant Selection:** Decide between Variant A (new bounded task, savings claimable) and Variant B (Task B retained, descriptive only, no savings claims).
2. **Ceiling & Headroom Parameters:** Set authoritative values for `max_tokens_ceiling` and `headroom_reserve` in the ratification block.
3. **Amendment Text Alignment:** Update `amendment-05.md` to document the dynamic headroom guard trigger (~145k–151k tokens) rather than the simplified static 160k threshold.
4. **Billing Quota Authorization:** Acknowledge that Codex runs against ChatGPT subscription quota windows.
5. **Future Live Execution Authorization:** Determine whether, when, and under which amendment live benchmarking will be authorized.

---

## 6. Offline Limitations & Unverified Claims

1. **No Real-Time Quota Checking:** Quota window reset times cannot be verified without network access.
2. **Residual Network Dispatch Race:** Offline audits confirm that if Codex dispatches an HTTP request across a local tool return in $< 50\text{ms}$, external process termination cannot recall that request.
3. **No Recommendation for Authorization:** In accordance with project boundaries, this report remains strictly neutral and does not recommend or solicit execution authorization.
