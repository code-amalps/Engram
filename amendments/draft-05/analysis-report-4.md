# Engram Amendment 05 Comprehensive Offline Analysis Report (Pass 4)

**Document Identifier:** `ENGRAM_DRAFT05_ANALYSIS_REPORT_4`  
**Execution Timestamp:** 2026-10-29T17:47:00Z (UTC) / 23:17:00 (IST)  
**Parent Directory:** `C:\Users\Amal P S\OneDrive\Projects\Engram\amendments\draft-05/`  
**Operational Status:** STRICTLY OFFLINE — ZERO INFERENCE CREDITS CONSUMED — NO LIVE RUNS  

---

## 1. Executive Summary

This report consolidates the offline forensic findings, telemetry breakdowns, guard accuracy analyses, and protocol drafting completed under Pass 4 of Engram's Amendment 05 preparation.

In strict compliance with project constraints:
- Zero model/API inference calls were dispatched.
- Active harness configuration files, frozen binaries, golden repository files, Amendment 04, Block 07 artifacts, and existing `draft-05` files (including `amendment-05.md`) were preserved bit-for-bit.
- All new documentation was authored exclusively under `amendments/draft-05/`.

---

## 2. Findings Per Task

### 2.1 Task 1: Engram MCP Usage and Autonomy Forensic
From native rollout and event records across Blocks 06, 07, and 08:
- **Block 06 Baseline Arm 1:** Exactly **0 Engram MCP calls** ("none"). MCP was disabled in baseline control. Agent used native PowerShell commands.
- **Block 07 Candidate Arm 3:** Exactly **1 attempted Engram MCP call** on Turn 2 (`engram_inspect_symbol` targeting `core.module.ts`, cumulative tokens: **32,397**).
  - *Call Lifecycle:* Attempted: YES; Approved: NO; Executed: NO; Successful: NO.
  - *Failure Cause:* Runner client approval policy was set to `"never"`, returning a **65-byte error** (`"MCP tool call requires approval, but approval policy is never"`).
  - *Agent Behavior Post-Failure:* Agent completely abandoned Engram MCP tools, regressed to recursive PowerShell `rg` searches, and dumped multiple entire TypeScript files (~1,500 lines) into context via `Get-Content`.
  - *Autonomous Model Intent:* **YES**. The model decided to invoke Engram autonomously without prompt guidance.
- **Block 08 Candidate Arm 3:** Exactly **0 Engram MCP calls** ("none"). The agent was provided an active Engram server with remediated approval policies, but autonomously chose to use PowerShell `Get-Content` and `rg` from Turn 1.

### 2.2 Task 2: Exploration Breakdown Before First Edit
- **First-Edit Timing:**
  - Block 06 Baseline: Turn 7 (**180,866 cumulative tokens**; 145,999 tokens in Turns 1–6).
  - Block 07 Candidate: **NO EDIT OCCURRED** across all 11 turns (**270,506 tokens** spent entirely in exploration).
  - Block 08 Candidate: Turn 9 (**177,588 cumulative tokens**; 151,470 tokens in Turns 1–8).
- **Tool Call Partitioning Before First Edit:**
  - *File Reads:* B06: 4 calls (92,783 tokens, 63.6%); B07: 2 calls (45,884 tokens); B08: 4 calls (78,295 tokens, 51.7%). File reading is the single largest consumer of context.
  - *Searches:* B06: 1 call (23,084 tokens); B07: 1 call (16,674 tokens); B08: 1 call (18,048 tokens).
  - *Directory Listing & Compound Search:* B06: 1 call (30,132 tokens); B07: 1 call (29,513 tokens); B08: 1 call (22,152 tokens).
  - *Engram MCP Calls:* B06: 0; B07: 1 call (16,483 tokens); B08: 0.

### 2.3 Task 3: Streaming Guard Accuracy & Block 07 Overrun Forensic
- **Alignment & Error:** Streaming estimates and authoritative rollout totals are synchronized at turn boundaries when rollout tailing reads `token_usage_record`.
- **Block 07 Overrun Mechanism:**  
  At Turn 10 completion, cumulative usage stood at **240,504 tokens** (9,496 tokens below 250k). In the unremediated harness, the guard evaluated only whether the ceiling was *already* breached. Turn 11 was launched with **29,918 prompt tokens**. Because OpenAI reports usage atomically upon turn completion, Turn 11 completed at **270,506 tokens** before the runner could intervene. The 20,506 overshoot occurred because a 30,000-token turn was dispatched into a 9,496-token margin.
- **Concurrency Boundary:** External polling monitors child processes at 50 ms intervals. If Codex dispatches an HTTP request across a local tool return in $< 50\text{ms}$, external process termination cannot cancel or recall that in-flight request. **The guard cannot provide a guaranteed hard cap.**

### 2.4 Task 4: Amendment 05 Draft v2 Proposal
Authored [`amendment-05-draft-v2.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/amendment-05-draft-v2.md) based on Variant B:
- Accurately describes the dynamic estimator (`remaining < max(last_turn, reserve)`).
- Acknowledges that external polling cannot guarantee a hard cap.
- Parameterizes reserve as `OWNER TO SET`, presenting nominal triggers for 30k (145k trigger), 35k (140k trigger), and 40k (135k trigger).
- Establishes the **Stop-at-First-Edit Rule** (terminating upon `file_change` / `apply_patch` to source files, verified by git).
- Lists primary descriptive metrics, strictly forbids comparative savings claims, and explicitly notes the small sample size ($N \approx 3$).

### 2.5 Task 5: Billing Checklist v2
Authored [`billing-checklist-v2.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/billing-checklist-v2.md):
- Formalizes the shift from dollar-based billing to ChatGPT subscription allocation windows.
- Replaces financial cost formulae with subscription percentage capacity equations.
- Marks every field `UNVERIFIED, owner to fill in` with source and date.
- Explicitly establishes the verified absence of auto-purchase as a strict prerequisite before live execution.

---

## 3. Inconsistencies Identified in Earlier Drafts

1. **Static vs. Dynamic Stop Trigger:**  
   Earlier draft `amendment-05.md` describes a static "160,000 stop trigger". The actual harness implementation is dynamic, evaluating `remaining < max(last_turn_tokens, reserve)`. When turn deltas reach ~29,000 tokens, the code triggers stopping at **~145,000 to ~151,000 cumulative tokens**, not at 160,000.
2. **Billing Paradigm Mismatch:**  
   Earlier draft `billing-checklist.md` formulated exposure in USD dollars based on OpenAI API token prices. In reality, the Codex CLI is authenticated via `auth_mode = "chatgpt"`, which is governed by subscription rate-limit windows rather than metered dollar billing.

---

## 4. Open Owner Decisions

1. **Protocol Confirmation:** Confirm whether to proceed with **Variant B (Exploratory Protocol with First-Edit Stop)** or **Variant A (Bounded Task Protocol)**.
2. **Headroom Reserve Sizing:** Authorize the reserve size ($R = 30,000$, $35,000$, or $40,000$).
3. **Account Plan Limits:** Complete `billing-checklist-v2.md` by verifying the subscription plan tier, reset window, and confirming that auto-purchase is disabled.
4. **Live Step Authorization:** Explicitly decide whether to authorize any future live execution or maintain the current offline state.

---

## 5. Offline Limitations & Boundaries

1. **Subscription Window State:** Active sliding-window usage percentages and reset timestamps cannot be inspected offline without network requests to OpenAI.
2. **Sub-50ms Network Dispatch Trace:** Exact network dispatch latency cannot be empirically measured without running an instrumented live process.
3. **No Execution Recommendation:** In strict accordance with instructions, this report remains neutral and does not recommend authorization.
