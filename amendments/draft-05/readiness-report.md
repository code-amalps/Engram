# Engram Pilot Readiness and Offline Audit Report (Proposed Amendment 05)

**Document Path:** `amendments/draft-05/readiness-report.md`  
**Phase:** Offline Preparation and Preflight Audit  
**Target Proposal:** Proposed Amendment 05 (175,000 tokens/arm ceiling, 160,000 stop trigger, 350,000 planned block exposure)  
**Authorization Status:** **STRICTLY PENDING HUMAN OWNER REVIEW AND GATES**  
*(Note: In accordance with protocol guidelines, this report presents verified evidence and open gates without recommending authorization.)*

---

## 1. Verified Offline

The following artifacts, components, and safeguards have been inspected, tested, and verified entirely offline without network access or model inference:

1. **Candidate and Frozen Binary Integrity:**
   - Candidate Phase 1 binary (`benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`) matches exact SHA-256: `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`.
   - Frozen v1.5.1 baseline binary matches SHA-256: `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0`.
   - Frozen v1.5.4 candidate binary matches SHA-256: `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798`.
2. **Golden Repository State:**
   - Golden repository pinned commit verified at `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.
   - Tracked files: exactly 18 modified user files preserved without alteration.
   - Untracked `.engram/` directory: all 29 event and database files verified against exact SHA-256 hash inventory.
3. **Historical Benchmark Artifacts:**
   - `AMENDMENT_04_...`, `STAGE1_BLOCK07_...`, and historical run records preserved without modification.
4. **Harness Remediation Regression Tests:**
   - `test_block07_remediation.py`: **14/14 tests passing** (verifying safe non-interactive MCP approval flags, pre-dispatch headroom calculations, boundary conditions, and distinct stop reasons).
   - `test_amendment_04_two_arm.py`: **9/9 tests passing** (verifying candidate-first two-arm schedule, stop-on-failure, and comparative claims suppression).
5. **Schema and Protocol Compliance:**
   - `run_telemetry.schema.json` and `correctness_checklist.schema.json` validated.
   - Pre-dispatch guard halts execution before an arm dispatches a turn that would breach the headroom reserve.
   - Comparative claims suppression rule validated: savings claims are suppressed unless both arms achieve full functional acceptance ($C = 1.00$).

---

## 2. Pending Account and Billing Verification

The following items cannot be determined offline and are recorded as UNVERIFIED in [`amendments/draft-05/billing-checklist.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/billing-checklist.md). They require direct owner verification:

1. **Billing Provider Verification:** Harness configuration executes `codex.exe exec` without in-repo API keys, inheriting ambient credentials. The active provider (OpenAI Platform, Azure OpenAI, ChatGPT subscription, etc.) is **inconclusive from repo config** and requires owner verification.
2. **Billing Tier and Credit Balance:** Current credit balance and monthly spending limits in the provider account dashboard.
3. **Model Unit Pricing:** Verified rates per 1M tokens for uncached input, cached input, completion, and reasoning for `gpt-5.6-luna`.
4. **Hard Spend Caps:** Confirmation of hard spending limits configured on the API key / project organization.
5. **Calculated Worst-Case Exposure:** Application of the symbolic cost formula to determine worst-case financial liability prior to any live run.

---

## 3. Pending Explicit Smoke-Test Authorization

The pre-benchmark smoke test procedure has been specified in [`amendments/draft-05/smoke-test-procedure.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/smoke-test-procedure.md). It is currently **deferred and unauthorized**:

1. **Token Cap Allocation:** The owner must assign a strict operational token cap (placeholder `[ OWNER TO SET ]`, recommended 5,000–10,000 tokens).
2. **Explicit Sign-Off:** Execution requires an explicit command and operator sign-off.
3. **Scope:** The smoke test will run a trivial read-only query on a disposable sandbox to verify model ID acceptance and unprompted autonomous MCP invocation across the five defined checks.

---

## 4. Pending Full Pilot Authorization

Proposed Amendment 05 is documented in [`amendments/draft-05/amendment-05.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/amendment-05.md). Full pilot execution remains **strictly deferred** pending completion of all sequential authorization gates.

---

## 5. Authorization Gates (Required Sequential Owner Actions)

Before any live inference step may be initiated, the human owner must complete the following six actions in strict sequential order:

1. **Gate 1 — Approve or Reject Proposed Amendment 05:**  
   Sign the ratification block in [`amendments/draft-05/amendment-05.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/amendment-05.md), formally adopting the 175,000 ceiling, 160,000 stop trigger, 350,000 block cap, and `OVERRUN_CENSORED` rule (or rejecting and reverting to Amendment 04).
2. **Gate 2 — Complete Billing Verification Checklist:**  
   Verify and fill in all UNVERIFIED fields in [`amendments/draft-05/billing-checklist.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/billing-checklist.md), confirming active billing provider, credit balance, model pricing, and hard spend caps.
3. **Gate 3 — Set the Smoke-Test Token Cap:**  
   Define and record the exact integer token ceiling (replacing `[ OWNER TO SET ]`) in [`amendments/draft-05/smoke-test-procedure.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/smoke-test-procedure.md).
4. **Gate 4 — Authorize Smoke-Test Execution:**  
   Issue a separate, explicit command authorizing the headless execution of the smoke test on an isolated disposable sandbox.
5. **Gate 5 — Review Smoke-Test Evidence:**  
   Inspect all five saved evidence files (`reports/smoke_test/check1_cli_init.log` through `check5_sandbox_diff.txt`) and verify that all five pass criteria were satisfied, including unprompted autonomous tool invocation and zero golden repo modifications.
6. **Gate 6 — Authorize Full Pilot Block Execution:**  
   Upon successful smoke-test evidence review, issue a separate, explicit command authorizing the two-arm Task B benchmark block under ratified Amendment 05.

---

## 6. Items Not Checked & Open Questions

### Not Checked (Offline Boundary)
- Live API connectivity, authentication, and model availability.
- Actual live token pricing for `gpt-5.6-luna`.
- Live autonomous agent behavior when invoking Engram MCP tools.

### Open Questions for Owner
1. **Billing Provider:** Which provider and credential mechanism powers `codex.exe` on this system (direct OpenAI Platform API, ChatGPT subscription, Azure, etc.)?
2. **Smoke Test Budget:** What numeric token cap is authorized for Gate 3 in [`smoke-test-procedure.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/smoke-test-procedure.md) (e.g., 5,000 or 10,000 tokens)?
3. **Billing Checklist Sign-Off:** Have all fields in [`billing-checklist.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/billing-checklist.md) been verified against provider dashboards?
