# Engram Amendment 05 — Candidate Readiness Decision

**Document Identifier:** `ENGRAM_DRAFT05_CANDIDATE_READINESS_DECISION`  
**Governing Context:** Executive Decision on Live Pilot Authorization & Next Experimental Steps  
**Operational Status:** Strictly Offline — Zero Model/API Inference Credits Consumed  
**Companion Document:** [`amendments/draft-05/candidate-readiness-review.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/candidate-readiness-review.md)  
**Parent Directory:** `amendments/draft-05/`  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram` (`feature/async-precommit-guard`)  
**Verified Git HEAD:** `c70845b4d5913457f1b176fb0370b80dd792711f`  

---

## 1. Formal Authorization Status Statement

> [!CAUTION]
> **LIVE INFERENCE EXECUTION IS NOT AUTHORIZED.**
> 
> In strict accordance with project safety rules, preflight boundaries, and the scope of this investigation, **no live model inference, API requests, smoke tests, benchmarks, or agent runs are authorized or initiated by this document.**
> 
> All live benchmark execution remains strictly paused until the project owner explicitly signs off on protocol calibration, billing verification, and harness readiness.

---

## 2. Executive Recommendation

### Recommended Action:
**Adopt Option 3 — Defer live inference trials until pre-dispatch guard safety, MCP server approval mechanics, and task scoping are hardened and verified entirely offline.**

Neither Variant A (Full Task B Functional Run) nor Variant B (Exploration / First-Edit Diagnostic) is ready or viable for live execution under the currently proposed operational parameters.

---

## 3. Justification for Deferral Based on Evidence

The forensic findings documented in [`candidate-readiness-review.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/candidate-readiness-review.md) establish five decisive disqualifying factors:

1. **The Insoluble 175,000-Token Ceiling Dilemma on Task B:**
   - Historical empirical evidence demonstrates that exploratory codebase search across Angular directories consumes **145,999 to 151,470 tokens** before an agent attempts its first code edit ([`task-b-feasibility.md#L89-L96`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/task-b-feasibility.md#L89-L96)).
   - Under a 175,000-token ceiling with a safe reserve (30,000–40,000 tokens), retrospective simulation proves that **100% of historical runs are halted before Turn 7 or Turn 9**, stopping the agent with **zero files edited** ($C = 0.00$) ([`reserve-simulation.md#L41-L96`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/reserve-simulation.md#L41-L96)).
   - Under the decided 15,000 reserve (trigger 160,000), the edit turn is dispatched, but because late turns cost 26,000–35,000 tokens, execution inevitably exceeds 175,000 tokens, resulting in **`OVERRUN_CENSORED`**. Under Owner Decision 2, this run is strictly ineligible to support any token savings claim ([`amendment-05.md#L22`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/amendment-05.md#L22)).
   - Attempting Variant A now guarantees spending up to 350,000 tokens with zero chance of validating comparative efficiency.

2. **Absence of Variant B Stopping Logic in Harness:**
   - The "Stop-at-First-Edit" rule proposed in `amendment-05-draft-v2.md` **does not exist in the codebase** ([`first-edit-rule-audit.md#L29-L43`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/first-edit-rule-audit.md#L29-L43)).
   - Neither `runner.py` nor `orchestrator.py` contains hooks to monitor `item.type == "file_change"` or terminate child processes upon code edits. Attempting Variant B without authoring and testing these hooks would result in an unconstrained full run.

3. **Pre-Dispatch Guard Unproven in Live Execution:**
   - The historical runner executed `proc.communicate()` synchronously without streaming monitoring ([`guard-evidence-audit.md#L27-L34`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/guard-evidence-audit.md#L27-L34)).
   - The `StreamingTokenTracker` poller in `runner.py` is uncommitted remediation code authored offline after Block 07. It operates as an external 50 ms poller, subject to network dispatch race conditions, and has never been proven to halt an in-flight turn under live execution.

4. **Candidate MCP Tool Invocation Never Validated in Codex:**
   - In Block 07, the candidate's MCP call failed immediately because Codex CLI rejected it with `"MCP tool call requires approval, but approval policy is never"` ([`exploration-attribution-audit.md#L71`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/exploration-attribution-audit.md#L71)).
   - In Block 08, the model never called MCP.
   - The remediation arguments configured in `runner.py` (`-c mcp_servers.engram.default_tools_approval_mode="approve"`) have never been validated in an end-to-end integration test.

5. **Subscription Billing & Auto-Purchase Uncertified:**
   - The project owner has not yet completed, verified, or signed [`billing-checklist-v2.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/billing-checklist-v2.md).
   - The active ChatGPT subscription tier, quota reset cycle, and verified absence of automated overage charges remain unverified.

---

## 4. Required Action Plan (Offline Hardening Phase)

Before any future live inference run can be responsibly authorized, the following three-step offline hardening phase must be completed:

```
[ Step 1: Offline Guard Validation ]
  ├── Run automated unit tests on StreamingTokenTracker with synthetic streams.
  └── Measure process termination latency on Windows under load.
       │
[ Step 2: Offline MCP Integration Verification ]
  ├── Run headless JSON-RPC smoke test on candidate-phase1/engram.exe over stdio.
  └── Verify Codex CLI argument parsing for MCP approval flags without model inference.
       │
[ Step 3: Owner Calibration & Billing Sign-Off ]
  ├── Owner decides between Bounded Task (Variant A) vs. Higher Ceiling for Task B.
  └── Owner verifies ChatGPT subscription settings and signs billing-checklist-v2.md.
```

### Action Items:
1. **Offline Unit & Integration Harness Tests:**
   Author and execute offline test suites verifying:
   - That `StreamingTokenTracker` correctly parses rollout files and triggers `terminate_pid_tree()` when simulated tokens exceed the stop trigger.
   - That the candidate binary (`candidate-phase1/engram.exe`) responds correctly to standard MCP JSON-RPC protocol messages (`initialize`, `tools/list`, `tools/call`).
   - If Variant B is desired, author and validate the `is_first_edit_boundary` hook in `runner.py` using archived Block 06/08 event fixtures.
2. **Owner Strategic Protocol Decision:**
   The owner must select one of two viable experimental designs:
   - **Alternative 1 (Calibrated Bounded Task):** Apply the 175,000 ceiling to a localized task ($\le 3$ files) where functional completion ($C = 1.00$) can realistically occur in 4–5 turns (~60k–90k tokens), providing a true test of token savings ($S$).
   - **Alternative 2 (Full Task B with Realistic Ceiling):** Retain `task_b_remember_me`, but calibrate the operational ceiling to **300,000–350,000 tokens** (with a 40,000 reserve and 260k–310k stop trigger), providing sufficient budget for exploration, editing, and compilation recovery.
3. **Owner Billing Certification:**
   The account owner must inspect ChatGPT subscription settings and sign [`amendments/draft-05/billing-checklist-v2.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/billing-checklist-v2.md).

---

## 5. Summary Verdict

| Evaluation Item | Status |
| :--- | :--- |
| **Current Live Inference Authorization** | **`NOT AUTHORIZED (PAUSED)`** |
| **Recommended Protocol Option** | **`OPTION 3: DEFER LIVE TESTS UNTIL HARDENED`** |
| **Variant A (Full Task B @ 175k) Viability** | **`INFEASIBLE (Guaranteed Overrun or Pre-Edit Halt)`** |
| **Variant B (First-Edit Diagnostic) Viability** | **`UNREADY (Requires Unimplemented Harness Hooks)`** |
| **Immediate Next Focus** | **`OFFLINE UNIT TESTS & OWNER BILLING/SCOPE SIGN-OFF`** |
