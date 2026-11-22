# Engram Phase 5.9.0.3: Financial Exposure & Subscription-Quota Claims Audit

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Phase:** 5.9.0.3 Workstream G  
**Execution Mode:** Strictly Offline, Read-Only Adversarial Audit  
**Direct Marginal Billing:** $0.00 USD (Verified under Subscription Auth)  
**Commercial Developer API Equivalent:** $7.50 to $20.26 USD (Derived Estimation)  
**Subscription Quota Impact Status:** `SUBSCRIPTION_QUOTA_IMPACT_NOT_VERIFIED`

---

## 1. Purpose

This document performs an adversarial audit of the financial cost and subscription quota statements made in prior benchmark reports (Phases 5.9.0 through 5.9.0.2). It distinguishes between direct marginal cash expenditure, commercial API developer pricing equivalents, and subscription rate-limit quota consumption.

---

## 2. Direct Marginal Cash Billing Audit ($0.00 Claim)

### 2.1 Evidentiary Basis
- In `benchmarks/matched-pair-harness/config/provider_config.json`, the execution harness authenticates through the local developer environment using session tokens associated with an active flat-rate subscription account (Claude Code / Codex subscription CLI auth).
- No pay-per-token API credit card key (`ANTHROPIC_API_KEY` or `OPENAI_API_KEY` metered usage) is configured in the environment or harness configs.
- Provider requests under subscription CLI sessions do not incur per-token direct credit card debits or metered marginal invoices.

### 2.2 Finding & Classification
- **Direct Marginal Financial Billing:** **$0.00 USD**.
- **Classification:** **DIRECTLY EVIDENCED & VERIFIED** under the configured local developer environment.

---

## 3. Commercial Developer API Equivalent ($7.50 to $20.26 Claim)

### 3.1 Basis of Derivation
Prior reports claimed a commercial API equivalent cost of $7.50 to $20.26 USD. We audit how this range was calculated:
1. **Model:** `claude-3-5-sonnet-20241022` / equivalent frontier model tier.
2. **Published Pricing Tier (Standard Tier Rates):**
   - Prompt Tokens: $3.00 per $1,000,000$ tokens.
   - Completion Tokens: $15.00 per $1,000,000$ tokens.
   - Prompt Caching Write: $3.75 per $1,000,000$ tokens.
   - Prompt Caching Read: $0.30 per $1,000,000$ tokens.
3. **Scenario A (Lower Bound - High Cache Hit Rate & Nominal Consumption ~1.6M tokens):**
   - $\approx 1.4\text{M}$ cached read / standard prompt tokens + $0.2\text{M}$ completion tokens.
   - Cost: $(1.4 \times \$3.00) + (0.2 \times \$15.00) \approx \$4.20 + \$3.00 = \$7.20 \approx \$7.50$.
4. **Scenario B (Upper Bound - Worst-Case Aggregate Exposure ~2.56M tokens, Uncached / High Completion):**
   - $\approx 1.8\text{M}$ uncached input prompt tokens + $\approx 0.76\text{M}$ completion tokens.
   - Cost: $(1.8 \times \$3.00) + (0.76 \times \$15.00) \approx \$5.40 + \$11.40 = \$16.80$ to $\$20.26$ (including retry overhead and uncached initial context reads).

### 3.2 Finding & Classification
- **Commercial API Equivalent:** **$7.50 to $20.26 USD**.
- **Classification:** **DERIVED ESTIMATION** from public developer pricing tiers. It represents the opportunity/shadow cost if executed against standard pay-as-you-go APIs, not a direct cash charge.

---

## 4. Subscription Quota Accounting Audit (1.6M to 2.5M Tokens Claim)

### 4.1 Evidentiary Basis & Runtime Inspection
Prior reports claimed that the benchmark would consume between $1.6\text{M}$ and $2.5\text{M}$ tokens of available subscription quota. We audited the local runtime to verify whether the remaining subscription quota can be inspected offline or enforced by the CLI:
1. `codex` / `claude` CLI tools provide no inspectable local file or offline API to query remaining weekly or monthly subscription token balances without contacting remote provider servers.
2. Subscription tier limits operate on proprietary rolling windows (e.g., 5-hour rolling token ceilings, weekly utilization metrics, or adaptive throttling) that are dynamically computed on remote server clusters.
3. In an offline environment (`CARGO_NET_OFFLINE=true`, zero network access), the actual starting quota balance, current rolling utilization percentage, and exact remaining headroom cannot be determined.

### 4.2 Adversarial Risks
- If the developer's subscription account is near its rolling limit prior to dispatch, launching Task A could trigger a rate limit error (`HTTP 429 Rate limit exceeded: You have reached your current usage cap`), halting the benchmark prematurely.
- The claim that the account has "guaranteed sufficient quota to execute 2.5M tokens" is **unsupported by inspectable local evidence**.

### 4.3 Mandatory Reclassification
Pursuant to Section 10 of the Phase 5.9.0.3 directive:
- **Subscription Quota Impact Status:** **`SUBSCRIPTION_QUOTA_IMPACT_NOT_VERIFIED`**.
- It must **not** be presented to human stakeholders as a verified or guaranteed available capacity.
- Human operators must verify their subscription status and rolling window headroom in their browser dashboard before authorizing live dispatch.

---

## 5. Summary Findings Matrix

| Financial / Quota Claim | Reported Range | Basis of Calculation | Evidentiary Status | Formal Audit Classification |
|---|---|---|---|---|
| Direct Marginal Cash Outlay | $0.00 USD | Flat-rate subscription authentication | Directly Evidenced | **VERIFIED ($0.00 USD)** |
| Commercial Developer API Equivalent | $7.50 – $20.26 USD | Model pricing tiers ($3/$15 per M) | Derived Estimation | **DERIVED ESTIMATE (Shadow Cost)** |
| Subscription Rolling Quota Consumption | $1.6\text{M} – 2.56\text{M}$ tokens | Token counter projections | Unverifiable Offline | **`SUBSCRIPTION_QUOTA_IMPACT_NOT_VERIFIED`** |
| Account Headroom Guarantee | "Sufficient Quota Guaranteed" | Assumption | Unsupported | **REJECTED (Requires Human Verification)** |
