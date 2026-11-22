# Engram Phase 5.9.0.5: Financial Exposure & Subscription-Quota Governance Disclosure

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Phase:** 5.9.0.5 Workstream G  
**Execution Mode:** Strictly Offline, Read-Only Adversarial Audit  
**Direct Marginal Billing:** $0.00 USD (Directly Verified under Subscription Auth)  
**Commercial Developer API Equivalent:** $7.50 to $20.26 USD (Derived Opportunity Valuation)  
**Subscription Quota Impact Status:** `SUBSCRIPTION_QUOTA_IMPACT_NOT_VERIFIED`

---

## 1. Purpose

This deliverable provides the final financial governance and subscription-quota disclosure for the two-invocation matched-pair benchmark pilot. It establishes clear distinctions between out-of-pocket cash billing, commercial API shadow valuations, empirical token expenditure, and subscription rate-limit quota consumption.

---

## 2. Summary Audit of Financial Claims

| Financial Metric | Certified Finding | Evidentiary Basis | Operational Implication |
|---|---|---|---|
| **Direct Marginal Cash Cost** | **$0.00 USD** | Authenticated via active local developer CLI subscription session. Zero metered pay-per-token API credit card keys are configured. | No credit card charges or marginal invoices will be generated |
| **Commercial API Equivalent** | **$7.50 to $20.26 USD** | Derived calculation using published commercial API rates ($3.00/M input, $15.00/M output). | Opportunity / shadow valuation of compute resources; NOT a payable invoice |
| **Empirical Historical Exposure** | **$2,520,000$ tokens** | $2,500,000 \text{ nominal} + 20,000 \text{ demonstrated max burst}$ under non-double-counted dynamic accounting. | Expected operational maximum under standard test conditions |
| **Subscription Quota Impact** | **`SUBSCRIPTION_QUOTA_IMPACT_NOT_VERIFIED`** | Offline sandbox prevents querying remote rolling usage limits or remaining window balances. | **Mandatory human dashboard verification required before live launch** |

---

## 3. Dissecting the $0.00 Marginal Billing Claim

An inspection of `benchmarks/matched-pair-harness/config/provider_config.json` and system environment configuration confirms:
1. No commercial metered API keys (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`) are present in the environment or passed via CLI flags.
2. The benchmark runner invokes `codex exec`, which authenticates through the local developer's active flat-rate subscription account.
3. Provider requests under subscription authentication do not generate per-token financial billing statements or metered credit card debits.
4. **Finding:** The $0.00 direct marginal billing claim is **evidenced and verified**.

---

## 4. Subscription Quota Governance: The Risk of HTTP 429

Subscription authentication is **not unlimited**:
1. Frontier model subscriptions enforce dynamic, rolling rate limits (e.g. 5-hour rolling token windows, weekly utilization metrics, or adaptive throttling).
2. If the user's account has already consumed a large portion of its rolling capacity prior to launching the benchmark, dispatching Task A could encounter an `HTTP 429 Rate Limit Exceeded` error mid-run.
3. Because rolling quota balances and window resets cannot be determined offline, this risk is formally classified as:
   # `SUBSCRIPTION_QUOTA_IMPACT_NOT_VERIFIED`

### Mandatory Human Pre-Flight Requirement:
Prior to executing any live pilot command, the human operator **must** open their provider web dashboard (e.g., Claude/Codex usage portal) and verify that the account has at least **$3.0\text{M}$ tokens of available rolling headroom**. Live execution must remain blocked until this human check is completed.
