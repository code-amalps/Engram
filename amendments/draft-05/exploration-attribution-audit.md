# Engram Exploration Cost Attribution Forensic Audit (Blocks 06, 07, and 08)

**Document Identifier:** `ENGRAM_DRAFT05_EXPLORATION_ATTRIBUTION_AUDIT`  
**Governing Standard:** Read-Only Audit of Pass 4 Attribution Claims  
**Authoritative Evidence:** Native Codex session rollouts and stdout event streams  
**Parent Directory:** `amendments/draft-05/`  
**Status:** Strictly Offline — Zero Inference Credits Consumed  

---

## 1. Executive Summary: The Prompt-History Attribution Fallacy

In Pass 4 (`amendments/draft-05/exploration-breakdown.md`), token costs were assigned to individual command categories (e.g. "File Reads: 92,783 tokens", "Polling: 32,975 tokens") by directly equating each model turn's total token increment ($\Delta$) with the command executed during that turn.

A forensic audit of the raw rollouts confirms that **this methodology is fundamentally flawed**:
1. **Autoregressive Prompt History Ingestion:**  
   In OpenAI's Responses / Chat Completions API, each model turn re-ingests the entire cumulative conversation history (system instructions, user prompts, past tool calls, and tool outputs).
   - In Block 06 Turn 6, the model increment was **32,030 tokens**. However, **31,681 tokens** (98.9%) was input prompt history re-sent to the model (including 29,696 cached tokens), while output tokens was only **349**. Attributing 32,030 tokens solely to `Get-Content` misattributes prior conversational context to a single shell execution.
2. **Compound Commands:**  
   When a turn executed compound shell pipelines (e.g. `git status --short; rg -n ...` or `Get-ChildItem; rg`), the relative token contribution of each sub-command is **`UNKNOWN`** because token counts are metered per model request, not per OS process.
3. **Subprocess Polling (`wait`):**  
   In Block 08 Turns 2 and 3, separate model requests were indeed dispatched to call `wait(cell_id: "1")`. However, each turn generated only **40** and **45** completion tokens, respectively. Re-ingesting prompt history accounted for >99.7% of the turn cost. Describing this as "32,975 tokens of polling overhead" creates the false impression that polling generated ~33k tokens of new data.

---

## 2. First-Edit Milestone Verification

In accordance with strict verification rules, a "first edit" is established only upon the first successful modification of a task source file in the isolated workspace, verified by a `file_change` or `apply_patch` completion:

| Run Identifier | Total Turns | First-Edit Turn | Cumulative Tokens Before First Edit | Cumulative Tokens After First Edit | First-Edit Target Files |
| :--- | :---: | :---: | :---: | :---: | :--- |
| **B06 Baseline Arm 1** | 9 | **Turn 7** | **145,999 tokens** (at Turn 6 end) | **180,866 tokens** | `core.actions.ts`, `core.effects.ts`, `app.component.ts`, `app.component.html`, `en.json`, `app.component.scss` |
| **B07 Candidate Arm 3** | 11 | **`NO EDIT OBSERVED`** | **270,506 tokens** (Entire run) | `N/A` (Halted with 0 files edited) | None. All 11 turns spent in exploration, failed MCP call, and hanging compiler. |
| **B08 Candidate Arm 3** | 16 | **Turn 9** | **151,470 tokens** (at Turn 8 end) | **177,588 tokens** | `core.module.ts` |

---

## 3. Detailed Turn-by-Turn Audit Prior to First Edit

### 3.1 Block 06: Baseline Arm 1 (Turns 1–6 Pre-Edit)

| Turn | Authoritative Delta ($\Delta$) | Input / Cached | Completion / Reasoning | Observed Actions / Tool Invocations | Observable Category | Exact Category Token Cost Attribution |
| :---: | :---: | :---: | :---: | :--- | :--- | :---: |
| **1** | 13,457 | 13,259 / 10,496 | 198 / 24 | `Get-ChildItem -Force; rg -n "login\|username..."` | Dir Listing + Search (Compound) | `UNKNOWN` (Turn total: 13,457) |
| **2** | 21,828 | 21,621 / 13,056 | 207 / 8 | `Get-Content .../core.module.ts` | File Read | `UNKNOWN` (Turn total: 21,828) |
| **3** | 23,084 | 22,934 / 21,504 | 150 / 16 | `rg -n "authLogin..."` | Search | `UNKNOWN` (Turn total: 23,084) |
| **4** | 25,468 | 25,247 / 22,784 | 221 / 45 | `Get-Content .../app.component.ts` | File Read | `UNKNOWN` (Turn total: 25,468) |
| **5** | 30,132 | 29,921 / 25,088 | 211 / 59 | `git status --short; Get-Content ...` | Dir Listing + File Read (Compound) | `UNKNOWN` (Turn total: 30,132) |
| **6** | 32,030 | 31,681 / 29,696 | 349 / 190 | `Get-Content .../core.module.spec.ts` | File Read | `UNKNOWN` (Turn total: 32,030) |

*Pass 4 Claim Discrepancy:* Pass 4 reported Turn 1 as `Get-Content core.module.ts`. The actual raw record in rollout line L16 confirms Turn 1 executed `Get-ChildItem -Force; rg -n "login|username|remember|auth"`.

---

### 3.2 Block 07: Candidate Arm 3 (All 11 Turns — No Edit Observed)

| Turn | Authoritative Delta ($\Delta$) | Input / Cached | Completion / Reasoning | Observed Actions / Tool Invocations | Observable Category | Exact Category Token Cost Attribution |
| :---: | :---: | :---: | :---: | :--- | :--- | :---: |
| **1** | 15,914 | 15,767 / 11,392 | 147 / 68 | `ALL_TOOLS.filter(x => /engram/i...)` | Tool Discovery / Agent Introspection | `UNKNOWN` (Turn total: 15,914) |
| **2** | 16,483 | 16,401 / 15,616 | 82 / 10 | `engram_inspect_symbol(core.module.ts)` | Engram MCP (Attempted; Rejected Locally) | `UNKNOWN` (Turn total: 16,483) |
| **3** | 16,674 | 16,518 / 16,256 | 156 / 19 | `rg -n "remember\|username..."` | Search | `UNKNOWN` (Turn total: 16,674) |
| **4** | 21,480 | 21,291 / 16,384 | 189 / 31 | `Get-Content .../core.module.ts` | File Read | `UNKNOWN` (Turn total: 21,480) |
| **5** | 24,404 | 24,182 / 21,120 | 222 / 66 | `Get-Content .../app.component.ts` | File Read | `UNKNOWN` (Turn total: 24,404) |
| **6** | 26,812 | 26,312 / 24,064 | 500 / 278 | `git status --short; rg -n "Login..."` | Dir Listing + Search (Compound) | `UNKNOWN` (Turn total: 26,812) |
| **7** | 29,513 | 29,195 / 26,112 | 318 / 65 | `npx tsc -p .../tsconfig.app.json` | Diagnostic Typecheck (Hung) | `UNKNOWN` (Turn total: 29,513) |
| **8** | 29,625 | 29,569 / 29,056 | 56 / 8 | `tools.write_stdin({chars: ""})` | Subprocess Polling / Output Wait | `UNKNOWN` (Turn total: 29,625) |
| **9** | 29,736 | 29,682 / 29,440 | 54 / 6 | `tools.write_stdin({chars: ""})` | Subprocess Polling / Output Wait | `UNKNOWN` (Turn total: 29,736) |
| **10**| 29,863 | 29,795 / 29,440 | 68 / 20 | `tools.write_stdin({chars: ""})` | Subprocess Polling / Output Wait | `UNKNOWN` (Turn total: 29,863) |
| **11**| 30,002 | 29,918 / 29,568 | 84 / 32 | `tools.write_stdin({chars: "\x03"})` | Subprocess Termination (Ctrl+C) | `UNKNOWN` (Turn total: 30,002) |

*Audit Verification of Engram Call:* On Turn 2, the agent autonomously dispatched `tools.mcp__engram__engram_inspect_symbol`. However, the Codex client approval policy was set to `"never"`, causing the call to be rejected locally with message `"MCP tool call requires approval, but approval policy is never"` ([`v154_arm3/events.jsonl#L7`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/v154_arm3/events.jsonl#L7)). The call never reached the Engram binary.

---

### 3.3 Block 08: Candidate Arm 3 (Turns 1–8 Pre-Edit)

| Turn | Authoritative Delta ($\Delta$) | Input / Cached | Completion / Reasoning | Observed Actions / Tool Invocations | Observable Category | Exact Category Token Cost Attribution |
| :---: | :---: | :---: | :---: | :--- | :--- | :---: |
| **1** | 16,383 | 16,187 / 11,392 | 196 / 43 | `Get-Content .../core.module.ts` | File Read | `UNKNOWN` (Turn total: 16,383) |
| **2** | 16,451 | 16,411 / 16,000 | 40 / 7 | `function_call wait(session_id)` | Subprocess Polling Request | `UNKNOWN` (Turn total: 16,451) |
| **3** | 16,524 | 16,479 / 11,392 | 45 / 12 | `function_call wait(session_id)` | Subprocess Polling Request | `UNKNOWN` (Turn total: 16,524) |
| **4** | 16,681 | 16,552 / 16,256 | 129 / 13 | `Get-Content .../core.module.ts -Raw` | File Read | `UNKNOWN` (Turn total: 16,681) |
| **5** | 18,048 | 17,904 / 16,256 | 144 / 33 | `rg -n "remember\|login..."` | Search | `UNKNOWN` (Turn total: 18,048) |
| **6** | 20,456 | 20,242 / 17,664 | 214 / 32 | `Get-Content .../app.component.ts` | File Read | `UNKNOWN` (Turn total: 20,456) |
| **7** | 22,152 | 21,760 / 20,096 | 392 / 186 | `git status --short; rg -n "CoreModule..."` | Dir Listing + Search (Compound) | `UNKNOWN` (Turn total: 22,152) |
| **8** | 24,775 | 24,615 / 21,632 | 160 / 12 | `Get-Content .../auth.effects.ts` | File Read | `UNKNOWN` (Turn total: 24,775) |

*Audit Verification of Polling:* In Turns 2 and 3, separate model requests were dispatched to execute `wait()`. While each request added ~16.5k tokens to the cumulative accounting, 97%–99% of that total represented pre-existing context cached by OpenAI. Fresh output tokens generated by the model during polling were only 40 and 45 tokens.

---

## 4. Reassessed Category Totals & Defensible Reporting

Because fine-grained token attribution to individual commands is physically confounded by conversational context accumulation, defensible reporting must present:
1. **Directly Observable Metrics:** Number of tool invocations by category.
2. **Turn-Level Increments:** Full token cost of the turn in which the activity occurred.
3. **Context Weight:** Clear acknowledgment that $>90\%$ of late exploration token weight is conversational history retention rather than fresh command payload.

| Category | B06 Invocations (Turns 1–6) | B07 Invocations (Turns 1–11) | B08 Invocations (Turns 1–8) | Token Attribution Feasibility |
| :--- | :---: | :---: | :---: | :--- |
| **File Reads (`Get-Content`)** | 4 | 2 | 4 | Invocations directly observable; exact isolated token cost is `UNKNOWN`. |
| **Code Searches (`rg`)** | 1 pure (+ 1 compound) | 1 pure (+ 1 compound) | 1 pure (+ 1 compound) | Invocations directly observable; exact isolated token cost is `UNKNOWN`. |
| **Directory / Git Listings** | 2 (compound) | 1 (compound) | 1 (compound) | Always executed as compound commands; isolated token cost is `UNKNOWN`. |
| **Engram MCP Invocations** | 0 | 1 (Failed locally) | 0 | Invocations directly observable; server execution cost was 0 tokens. |
| **Compiler / Diagnostic Runs** | 0 | 1 (`npx tsc`) | 0 | Observable; caused multi-turn polling hang in B07. |
| **Process Wait / Polling Requests** | 0 | 4 (`write_stdin`) | 2 (`wait`) | Distinct model requests observable; turn totals dominated by cached prompt history. |
