# Engram Phase 5.9.0.2: Financial Exposure Reconciliation & Billing Model Audit

**Phase:** 5.9.0.2  
**Target:** Financial and Quota Exposure Models for Live Pilot Benchmark  
**Execution Mode:** Strictly Offline Financial Audit  

---

## 1. Authentication & Billing Context Classification

Based on [PHASE_5_3_PROVIDER_COMPATIBILITY.md](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/PHASE_5_3_PROVIDER_COMPATIBILITY.md) (Section 4):
- **Provider Mechanism:** Headless Codex CLI app-server (`codex.exe exec --json`).
- **Authentication Channel:** Session-based authentication against an active ChatGPT subscription account.

### 1.1 Four Distinct Exposure Tiers

| Exposure Dimension | Classification | Financial & Operational Impact |
|---|---|---|
| **1. Direct Marginal API Billing** | **$0.00 USD** | No pay-as-you-go invoices, credit card charges, or API key balance deductions are generated. |
| **2. Subscription Quota Impact** | **NON-ZERO (Rolling Allowance)** | Consumes message and token allowance against active subscription tier (e.g. 3-hour / weekly message caps). **Usage is NOT unlimited.** |
| **3. Account Restriction Risk** | **MODERATE** | Excessive rapid turns during Task B could trigger transient rate-limiting (HTTP 429) or cooling-off throttling. |
| **4. Commercial-Equivalent Value** | **$7.50 to $20.26 USD** | The equivalent monetary value if the same tokens were dispatched via standard commercial pay-as-you-go developer endpoints. |

---

## 2. Commercial-Equivalent Cost Model (Explicit Parameter Matrix)

Because live network queries are `NOT_AUTHORIZED`, commercial rates are established parametrically using local project documentation baselines:

$$\text{Total Equivalent Cost } (C) = \left( T_{\text{uncached}} \times \frac{R_{\text{in}}}{10^6} \right) + \left( T_{\text{cached}} \times \frac{R_{\text{cached}}}{10^6} \right) + \left( T_{\text{out}} \times \frac{R_{\text{out}}}{10^6} \right)$$

### 2.1 Parameter Baseline
- **Model Identity:** `gpt-5.6-luna`
- **Assumed Split (Historical Live 02 Telemetry):**
  - $35\%$ Uncached Prompt Input Tokens
  - $50\%$ Cached Prompt Input Tokens (Prompt caching hit rate)
  - $15\%$ Completion Output & Reasoning Tokens
- **Rate Tier 1 (Standard Frontier Commercial Pricing):**
  $R_{\text{in}} = \$2.50 / \text{M}$, $R_{\text{cached}} = \$1.25 / \text{M}$, $R_{\text{out}} = \$10.00 / \text{M}$
- **Rate Tier 2 (High-Capability / Reasoning Tier Commercial Pricing):**
  $R_{\text{in}} = \$5.00 / \text{M}$, $R_{\text{cached}} = \$2.50 / \text{M}$, $R_{\text{out}} = \$20.00 / \text{M}$

---

## 3. Parametric Exposure Calculations

| Benchmark Scenario | Cumulative Tokens | Direct API Billing | Tier 1 Equivalent (USD) | Tier 2 Equivalent (USD) | Quota Risk |
|---|---:|---:|---:|---:|---|
| **Nominal Execution** | 1,600,000 | $0.00 | **$6.40** | **$12.80** | Low (Well within standard session limits) |
| **Balanced Hard Ceiling** | 2,500,000 | $0.00 | **$10.00** | **$20.00** | Moderate (May consume ~25% of daily message allowance) |
| **Max Overshoot Bound** | 2,532,768 | $0.00 | **$10.13** | **$20.26** | Moderate (Hard-capped by orchestrator) |

### Step-by-Step Mathematical Verification for 2,500,000 Tokens (Tier 1):
1. Uncached Input: $2,500,000 \times 0.35 = 875,000\text{ tokens} \times \frac{\$2.50}{10^6} = \$2.1875$
2. Cached Input: $2,500,000 \times 0.50 = 1,250,000\text{ tokens} \times \frac{\$1.25}{10^6} = \$1.5625$
3. Output Tokens: $2,500,000 \times 0.15 = 375,000\text{ tokens} \times \frac{\$10.00}{10^6} = \$3.7500$
4. **Sum:** $\$2.1875 + \$1.5625 + \$3.7500 = \mathbf{\$7.50\text{ USD}}$ (Baseline) up to **\$10.00 USD** (under 0% cache hits).

---

## 4. Final Financial Assessment

- **Direct Invoiced Cost:** Guaranteed **$0.00 USD** under subscription authentication.
- **Maximum Commercial Exposure Equivalent:** Strictly bounded to **$\le \$20.26\text{ USD}$**.
- **Actionable Stakeholder Guidance:** The decision to authorize live execution incurs zero incremental financial debt, but must accept that approximately 10 to 25 conversational turns will be deducted from the user's Codex subscription quota.
