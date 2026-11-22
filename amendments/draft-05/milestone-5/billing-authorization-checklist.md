# Milestone 5A: Billing and Account Authorization Checklist
## Pre-Flight Fiscal Audit and Account Verification Protocol

**Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/async-precommit-guard`  
**HEAD Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`  
**Document ID:** `ENGRAM-A05-M5A-BILLING-CHECKLIST`  
**Classification:** Fiscal Governance and Authorization Gate  

---

### 1. Mandatory Owner Verification Standard

Prior to any live model inference, token-consuming smoke test, or benchmark execution, the human repository owner must independently verify and sign off on this checklist.

#### Strict Governance Invariant:
Local environment properties (such as configuration files, local CLI cache directories, environment variable names, or successful local `--dry-run` executions) **must never be used to infer billing status**. 
- The presence of `"auth_mode": "chatgpt"` in local config does **NOT** prove that requests will not trigger credit card billing.
- A successful local CLI invocation does **NOT** guarantee that usage falls within subscription quotas.
- Only direct inspection of the provider's active billing dashboard provides authoritative verification.

---

### 2. Pre-Flight Billing Verification Checklist

All items below must be personally reviewed by the owner and certified prior to granting live experiment authorization:

| Verification Dimension | Checklist Item | Status | Verification Source / Instructions |
| :--- | :--- | :---: | :--- |
| **Account Entitlement** | 1. Is the active account authorized for the selected experimental model (e.g., `gpt-5.6-luna` or equivalent)? | `OWNER VERIFICATION REQUIRED` | Inspect active subscription tier / API dashboard. |
| **Billing Mode** | 2. Is the model usage covered under an inclusive subscription (e.g. ChatGPT Pro/Team) or billed on metered API pay-as-you-go? | `OWNER VERIFICATION REQUIRED` | Check organization billing settings in OpenAI / Anthropic console. |
| **Direct Card Charges** | 3. Is there a credit card linked that could incur unexpected direct charges if subscription limits are exceeded? | `OWNER VERIFICATION REQUIRED` | Verify billing payment methods. |
| **Auto-Recharge Risk** | 4. Is automatic credit purchase ("auto-top-up" or "automatic recharge") disabled in the provider account? | `OWNER VERIFICATION REQUIRED` | Disable auto-reload to ensure hard spend boundaries. |
| **Available Quota** | 5. Is there sufficient quota remaining in the current billing cycle to absorb up to 350,000 tokens without throttling? | `OWNER VERIFICATION REQUIRED` | Check usage graphs and remaining rate limit quota. |
| **Usage Hard Cap** | 6. Has a hard usage spend limit (e.g. \$5.00 USD maximum) been configured in the provider console? | `OWNER VERIFICATION REQUIRED` | Set strict organizational spending cap in developer settings. |
| **API Key Segregation** | 7. If API keys are utilized, is a dedicated, restricted key generated solely for this experiment? | `OWNER VERIFICATION REQUIRED` | Ensure key has no access to production resources. |
| **Telemetry Provenance** | 8. Has the local harness been confirmed to extract provider-reported usage objects without local approximation? | `VERIFIED OFFLINE` | Audited in `token-accounting-specification.md`. |
| **Harness Token Ceiling**| 9. Is the harness configured with the 140k nominal stop threshold and 175k hard per-arm ceiling? | `VERIFIED OFFLINE` | Audited in `token-ceiling-and-stopping-policy.md`. |

---

### 3. Estimated Financial Exposure

Based on standard commercial API list-price equivalents (e.g., GPT-4o / GPT-5 class rates at ~\$2.50 / 1M prompt tokens and ~\$10.00 / 1M completion tokens):

$$\text{Estimated List-Price Exposure (Per Arm):} \quad \approx \$0.50 \text{ to } \$1.25 \text{ USD}$$

$$\text{Maximum Theoretical Exposure (Combined 350k tokens):} \quad \le \mathbf{\$3.50 \text{ USD}}$$

*(Note: Under subscription session-token authentication, direct marginal cash billing has historically been \$0.00 USD, but rate limits and hourly caps apply).*

---

### 4. Owner Certification Gate

This experiment **cannot proceed to live execution** until the repository owner completes and signs the following statement:

```
[ ] I have verified that the active provider account has sufficient quota.
[ ] I have confirmed the billing mode (inclusive subscription vs. metered API).
[ ] I have verified that automatic credit recharge is disabled.
[ ] I have authorized an experimental exposure ceiling of at most 350,000 tokens.
[ ] I explicitly authorize the execution of Milestone 5B.

Owner Signature / Confirmation: __________________________
Date: __________________________
```
