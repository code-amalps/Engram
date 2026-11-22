# Engram Phase 5.9.0.4: Financial Exposure & Subscription-Quota Governance Disclosure

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Phase:** 5.9.0.4 Workstream E  
**Execution Mode:** Strictly Offline, Read-Only Adversarial Audit  
**Direct Marginal Billing:** $0.00 USD (Directly Evidenced under Subscription Auth)  
**Commercial Developer API Equivalent:** $7.50 to $20.26 USD (Derived Shadow Cost)  
**Subscription Quota Impact Status:** `SUBSCRIPTION_QUOTA_IMPACT_NOT_VERIFIED`

---

## 1. Purpose

This document provides the authoritative financial exposure and subscription-quota governance disclosure for the live two-invocation matched-pair benchmark pilot. It transparently separates direct marginal cash costs, commercial API shadow estimates, empirical token exposure, theoretical token exposure, and subscription rate-limit utilization.

---

## 2. Summary of Financial & Token Exposure Metrics

| Exposure Metric | Certified Value / Status | Evidentiary Basis & Scope | Governance Requirement |
|---|---|---|---|
| **Direct Marginal Billing** | **$0.00 USD** | Authenticated via active local developer CLI subscription session. Zero metered pay-per-token API keys configured. | No credit card charges incurred |
| **Commercial API Equivalent** | **$7.50 to $20.26 USD** | Derived shadow cost based on standard commercial pricing tiers ($3.00/M input, $15.00/M output). | Opportunity cost disclosure only; not a cash charge |
| **Empirical Historical Exposure** | **$2,540,000$ tokens** | $2,500,000 \text{ nominal} + 40,000 \text{ demonstrated max overshoot}$ across Phase 5.4.24 offline runs. | Expected operational ceiling under standard behavior |
| **Theoretical Exposure Status** | **`NOT_PROVEN`** | Theoretical bound of $2,562,768$ rests on $P_k \le 60,000$, which is an observed estimate, not a software limit. | Conditional upon prompt not exceeding $60,000$ tokens |
| **Subscription Quota Impact** | **`SUBSCRIPTION_QUOTA_IMPACT_NOT_VERIFIED`** | Provider rolling token quota balances and window resets cannot be inspected offline. | **Mandatory human dashboard verification before live run** |

---

## 3. Dissecting the Commercial Developer API Equivalent

The reported range of **$7.50 to $20.26 USD** represents a derived opportunity cost (shadow pricing) if executed against metered commercial pay-as-you-go APIs:

1. **Frontier Model Pricing Assumptions:**
   - Input Prompt Tokens: $3.00 per $1,000,000$ tokens.
   - Output Completion Tokens: $15.00 per $1,000,000$ tokens.
   - Cached Prompt Read: $0.30 per $1,000,000$ tokens.
2. **Lower Bound ($7.50 USD):**
   - Assumes nominal execution ($\approx 1.6\text{M}$ total tokens), high prompt caching hit rate ($>80\%$), and minimal completion reasoning bursts.
3. **Upper Bound ($20.26 USD):**
   - Assumes maximum exposure ($\approx 2.54\text{M}$ tokens), uncached initial context reads, and extended chain-of-thought completion bursts.
4. **Governance Rule:** This metric must **never** be presented to stakeholders as a cash invoice or guaranteed charge. It is purely a benchmark resource valuation.

---

## 4. Subscription Quota Accounting & Mandatory Human Verification

Pursuant to Section 8 of the Phase 5.9.0.4 directive:
- **Offline Unverifiability:** In an offline sandbox (`CARGO_NET_OFFLINE=true`), neither the Codex CLI nor the Antigravity IDE provides an inspectable local ledger of remaining rolling subscription quota.
- **Provider-Side Rate Limits:** The provider enforces a dynamic rolling usage window (e.g., 5-hour rolling token capacity). If the user's account has already consumed a substantial fraction of its window capacity prior to dispatch, launching Task A could encounter `HTTP 429 Rate Limit Exceeded`.
- **Mandatory Pre-Flight Instruction:**
  Prior to issuing explicit live authorization, the human operator **must** open their provider web dashboard (e.g. Anthropic Console / OpenAI Usage Portal) and confirm that the account has ample rolling headroom ($\ge 3.0\text{M}$ tokens recommended) to execute the two-invocation pentad.
