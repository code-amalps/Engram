# Engram Phase 5.9.1.2: Financial Exposure & Subscription-Quota Governance Disclosure

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Phase:** 5.9.1.2 Workstream A  
**Execution Mode:** Strictly Offline, Read-Only Audit & Forensic Accounting  
**Benchmark Protocol:** v3.0.0, Controlled Runtime Pentad  
**Pricing Baseline Date:** 2026-11-20 (Verified Provider Rate Schedule)  
**Recheck Obligation:** Rates must be rechecked immediately prior to live authorization.  
**Subscription Quota Impact Status:** `SUBSCRIPTION_QUOTA_IMPACT_NOT_VERIFIED`  

---

## 1. Executive Summary & Audit of Pricing References

Previous Phase 5.9.1 / 5.9.1.1 reports referenced an ad-hoc pricing figure of `$1.25 / M input` and `$5.00 / M output` with a `$2.50 / M` blended rate. That calculation is officially retracted as ungrounded.

This disclosure establishes the authoritative financial exposure model based on verified GPT-5.6 Luna standard developer API rates:
- **Standard Prompt Input:** **\$0.20 per 1,000,000 tokens** (\$0.00000020 per token)
- **Standard Completion Output:** **\$1.20 per 1,000,000 tokens** (\$0.00000120 per token)

### Distinguishing Metric Categories:
1. **Direct Cash Invoicing (Metered API Keys):** If executed using an API key (`OPENAI_API_KEY`), charges accrue based on actual metered tokens up to the hard pre-dispatch budget limit.
2. **Subscription Account Quota (Flat-Rate Auth):** If executed via subscription-authenticated tunnels, marginal cash cost is \$0.00, but rolling rate-limit window consumption occurs.
3. **Internal Effectiveness Metrics:** Harness logs reporting `Effective Cost: 574400.0` or `quality_adjusted_cost` represent **tokens per correctness score** ($T_{\text{tokens}} / C$), not financial currencies.

---

## 2. Authoritative Cost Scenarios

All scenarios are calculated across both the configured global budget ceiling ($B_G = 2,500,000$ tokens) and the legacy conservative exposure ceiling ($E_{\text{max}} = 2,531,384$ tokens).

### 2.1 Scenario Breakdown

| Scenario | Input / Output Mix | Blended Rate (\$/M Tokens) | Cost at Global Ceiling ($B_G = 2.50\text{M}$ Tokens) | Cost at Conservative Exposure ($E_{\text{max}} = 2.531\text{M}$ Tokens) | Description & Validity |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Scenario 1** | 100% Input / 0% Output | \$0.20 | **\$0.5000** | **\$0.5063** | Theoretical lower bound (read-only task, zero agent generation). |
| **Scenario 2** | 0% Input / 100% Output | \$1.20 | **\$3.0000** | **\$3.0377** | Theoretical absolute upper bound (worst-case all-completion). |
| **Scenario 3A** | 80% Input / 20% Output | \$0.40 | **\$1.0000** | **\$1.0126** | Standard illustrative agentic mix (4:1 prompt-to-completion ratio). |
| **Scenario 3B** | 95% Input / 5% Output | \$0.25 | **\$0.6250** | **\$0.6328** | Heavy-context repository analysis mix (typical coding agent). |
| **Dry-Run Observed** | Synthetic 57,440 Tokens | \$0.25 | **\$0.0144** | N/A | Calculated on actual synthetic dry-run token count. |

### 2.2 Mathematical Formulae for Illustrative Blended Rates
For any input fraction $f_{\text{in}}$ and output fraction $f_{\text{out}} = 1 - f_{\text{in}}$:
$$\text{Cost} = T_{\text{total}} \times \left( f_{\text{in}} \times \$0.20 \times 10^{-6} + f_{\text{out}} \times \$1.20 \times 10^{-6} \right)$$

For the standard 80/20 illustrative mix:
$$\text{Cost} = 2,500,000 \times (0.80 \times 0.00000020 + 0.20 \times 0.00000120) = 2,500,000 \times \$0.00000040 = \$1.0000$$

---

## 3. Disclosed Billing Unknowns & Provider Caveats

1. **Prompt Caching Discounts:** Frontier model providers often discount cached input tokens (e.g., 50% discount at \$0.10 / M tokens). However, cache hit rates and minimum prefix thresholds for `gpt-5.6-luna` cannot be empirically verified offline. All cached input savings are therefore disclosed as **UNVERIFIED UNKNOWNS** and are conservatively omitted from safety bounds.
2. **Cancellation-to-Billing Granularity:** While `ControlledTransport` aborts HTTP streaming chunk reception upon reaching reservation limits, provider-side GPU inference clusters may bill for tokens buffered prior to connection termination. The maximum billable completion tokens for any single turn is strictly bounded by the request-level `max_completion_tokens: 4096`.
3. **Rate Volatility:** API pricing is subject to provider schedule changes. Baseline verification date is 2026-11-20. The operator must confirm that GPT-5.6 Luna rates remain \$0.20 / \$1.20 prior to live benchmark dispatch.
4. **Subscription Quota Window Depletion:** Under subscription-authenticated execution, rate limits are governed by rolling token windows (e.g., 5-hour rolling windows). Offline testing cannot query remote quota balances. A human operator must verify $\ge 3.0\text{M}$ tokens of available rolling headroom on the web dashboard before live execution.
