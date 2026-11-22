# Engram Amendment 05 — Milestone 2: Authorization Gates

**Document Identifier:** `ENGRAM_DRAFT05_MILESTONE2_AUTHORIZATION_GATES`  
**Governing Document:** Engram Amendment 05 (Controlled Smoke-Test Readiness)  
**Execution Status:** **BLOCKED PENDING OWNER DECISIONS — NO LIVE EXECUTION PERMITTED**  

---

## Executive Gating Policy

The offline preparation for the controlled pre-benchmark smoke test is complete. However, live execution consumes model inference quota and interacts with the upstream model provider.

In strict compliance with governance rules:
> **Zero live inference or model API requests may be initiated without explicit, written owner sign-off on all three authorization gates below.**

---

## Authorization Gate 1: Smoke-Test Token Budget Allocation

The full Amendment 05 pilot protocol specifies a 175,000-token per-arm ceiling. For the narrowly targeted single-arm read-only smoke test, allocating 175,000 tokens is excessive. The owner must formally select and authorize an operational smoke-test budget.

### Options Presented for Owner Selection:

```
[   ] OPTION A (RECOMMENDED — TIGHT BOUNDS)
      - Operational Token Ceiling: 10,000 tokens
      - Pre-Dispatch Headroom Reserve: 2,000 tokens
      - Nominal Stop Trigger: 8,000 tokens
      - Rationale: Provides a generous 2x–3x margin over expected read-only usage (~4,000 tokens)
        while capping total potential subscription window exposure to a negligible level.

[   ] OPTION B (CONSERVATIVE BUFFER)
      - Operational Token Ceiling: 15,000 tokens
      - Pre-Dispatch Headroom Reserve: 3,000 tokens
      - Nominal Stop Trigger: 12,000 tokens
      - Rationale: Accommodates potential multi-turn exploration if the agent inspects multiple symbols.

[   ] OPTION C (CUSTOM OWNER SPECIFICATION)
      - Operational Token Ceiling: ____________ tokens
      - Pre-Dispatch Headroom Reserve: ____________ tokens
      - Nominal Stop Trigger: ____________ tokens
```

---

## Authorization Gate 2: Subscription Billing & Auto-Purchase Certification

Because Codex is authenticated via ChatGPT Subscription OAuth (`auth_mode: "chatgpt"`), consumption draws down the rolling subscription window rather than metered API credits.

The owner must inspect the ChatGPT web account settings and certify:

```text
================================================================================
OWNER SUBSCRIPTION CERTIFICATION
================================================================================

1. Active Plan Tier:
   [   ] ChatGPT Plus    [   ] ChatGPT Team    [   ] ChatGPT Enterprise    [   ] ChatGPT Pro

2. Automatic Additional Usage / Purchasing Status:
   [   ] I CERTIFY THAT AUTOMATIC PURCHASING / ADD-ON BILLING IS STRICTLY DISABLED.

3. Subscription Capacity Assessment:
   [   ] I have verified the current rolling window status and confirmed that an
         experimental exposure of up to [ 10,000 / 15,000 ] tokens will not disrupt
         normal engineering workflows or deplete required development capacity.

================================================================================
```

---

## Authorization Gate 3: Explicit Live Execution Sign-Off

The owner must formally sign the execution authorization statement below:

```text
================================================================================
FORMAL LIVE SMOKE-TEST AUTHORIZATION SIGN-OFF
================================================================================

I hereby authorize the execution of a single-arm, strictly read-only pre-benchmark
smoke test under Engram Amendment 05 using:
- Candidate Executable: candidate-phase1/engram.exe (SHA-256: F56347EF...23D85BEE)
- Model: gpt-5.6-luna (low/medium reasoning effort)
- Approved Token Ceiling: ____________ tokens
- Approved Reserve: ____________ tokens
- Execution Workspace: Isolated disposable scratch sandbox

I confirm that all preconditions in smoke-test-execution-checklist.md are satisfied.

Owner Signature: ___________________________________  Date: ____________________
================================================================================
```

---

## Conditions that Block Execution

The automated harness and engineering agent are **strictly prohibited** from launching live execution if any of the following conditions exist:

1. **Unsigned Gates:** Any of Gates 1, 2, or 3 lacks explicit owner sign-off.
2. **Missing Token Cap:** The smoke-test token ceiling is not explicitly designated.
3. **Unverified Auto-Purchase:** Automatic purchasing or add-on billing is not certified as disabled.
4. **Binary Hash Mismatch:** `candidate-phase1/engram.exe` does not match `F56347EF...23D85BEE`.
5. **Golden Repo Access:** Any execution command targets or references the golden repository path.
6. **Network Failure / Rate Limit:** Codex returns an active HTTP 429 quota exhaustion code.
