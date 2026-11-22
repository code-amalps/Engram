# Owner Authorization Checklist: N=1 Matched-Pair Pilot

**Milestone:** Engram N=1 Matched-Pair Pilot Final Preflight  
**Document ID:** `amendments/draft-05/n1-pilot-preflight/owner-authorization-checklist.md`  
**Evaluation Date:** 2026-10-30  
**Target Trial:** Controlled Matched-Pair Experiment ($N=1$, `pilot_01`)  
**Status:** READY FOR OWNER REVIEW  

---

## 1. Operational Authorization Checklist

The following 10-point checklist evaluates all operational, technical, and subscription prerequisites required prior to authorizing live inference for the N=1 matched-pair pilot.

Statuses are strictly categorized as:
- `VERIFIED`: Confirmed by static inspection, unit test, or filesystem validation.
- `OWNER CONFIRMATION REQUIRED`: Requires affirmative owner review of subscription status, risk tolerance, or live authorization.
- `NOT VERIFIED`: Condition could not be established offline.
- `BLOCKED`: Technical blocker prevents valid execution.

| # | Item Description | Status | Verification Evidence / Owner Action Required |
|---|---|---|---|
| **1** | **Current ChatGPT/Codex Subscription Allowance** | `OWNER CONFIRMATION REQUIRED` | Local runtime uses ChatGPT Subscription OAuth (`auth_mode = "chatgpt"`, `OPENAI_API_KEY: null`). Owner must confirm that the ChatGPT account has sufficient rolling message/token allowance in its current 3-hour window. |
| **2** | **Current Model Access** | `OWNER CONFIRMATION REQUIRED` | Model `gpt-5.6-luna` is configured in `orchestrator.py` and `runner.py`. Owner must verify that their active ChatGPT subscription plan includes access to this model identifier. |
| **3** | **Owner Acceptance of Possible Usage Overrun** | `OWNER CONFIRMATION REQUIRED` | In-flight turns cannot be halted mid-generation. Single-turn bursts (historically up to 47k tokens) may cause terminal usage to overshoot the nominal 175k/350k envelope before local process shutdown. Owner must acknowledge this behavior. |
| **4** | **Owner Acceptance of 175k Nominal Per-Arm Budget** | `OWNER CONFIRMATION REQUIRED` | Per-arm budget is configured at 175,000 tokens with a 45,000 reserve (pre-dispatch stop at 130,000 tokens). Owner must approve this planning threshold. |
| **5** | **Owner Acceptance of Combined Planned Exposure** | `OWNER CONFIRMATION REQUIRED` | Combined nominal planned exposure across Control A and Treatment B is 350,000 tokens. Owner must approve this allocation against subscription limits. |
| **6** | **Candidate Binary Identity** | `VERIFIED` | Binary `target/release/engram.exe` matches `candidate-phase1/engram.exe` with SHA-256 `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`. Verified by preflight hash checks. |
| **7** | **Exact Model Identifier** | `VERIFIED` | Pinned to `gpt-5.6-luna` with reasoning effort `medium`. Invariants verified across task config, runner arguments, and orchestrator schemas. |
| **8** | **Experiment Task & Baseline Commit** | `VERIFIED` | Task `task_commit_cache_invalidation` verified; Git repository HEAD confirmed at `c70845b4d5913457f1b176fb0370b80dd792711f` on branch `feature/async-precommit-guard`. |
| **9** | **Output Directory Availability** | `VERIFIED` | Target directory `benchmarks/matched-pair-harness/reports/runs/pilot_01` tested; confirmed absent and 100% collision-free. |
| **10** | **Explicit Live Execution Authorization** | `OWNER CONFIRMATION REQUIRED` | Under absolute safety rules, no model-backed processes or inference commands may be executed without explicit, separate owner instruction. |

---

## 2. Technical Findings Requiring Owner Awareness

In addition to the 10 authorization items above, preflight inspection identified two technical findings in the harness:

1. **CLI Argument Option Discrepancy:**
   - The CLI argument parser in `orchestrator.py` does not define `--engram-bin`. Passing this flag causes `argparse` to exit with code 2.
   - However, `DEFAULT_ENGRAM_BIN` in `orchestrator.py` resolves to `target/release/engram.exe` (SHA-256 `F56347EF...`), and passing `--expected-treatment-sha256 F56347EF...` successfully validates the Candidate Phase 1 binary.
2. **Evaluator Static Check Gaps:**
   - `evaluator.py` lacks domain-specific checks in `_check_static_requirement` for `task_commit_cache_invalidation`.
   - Without updating `evaluator.py`, REQ-1 and REQ-3 will evaluate to `FAIL`, capping correctness at $C = 0.50$ and suppressing comparative token savings.

---

## 3. Owner Authorization Form

Prior to issuing the live launch command, please confirm the following:

- [ ] I confirm my ChatGPT/Codex subscription is active with available rolling capacity.
- [ ] I confirm my account has access to model `gpt-5.6-luna`.
- [ ] I accept the nominal planning budget of 175,000 tokens per arm (350,000 tokens total) and understand that in-flight requests may cause usage overruns.
- [ ] I authorize execution of the verified launch command for pair `pilot_01`.
