# Milestone 5B-Review: Billing & Account Owner Verification Checklist

## 1. Important Governance Notice

**THE REPOSITORY AND BENCHMARK HARNESS CANNOT INDEPENDENTLY VERIFY OR PROVE ACCOUNT BILLING TERMS, SUBSCRIPTION ENTITLEMENTS, REMAINING CREDITS, OR FINANCIAL SPEND CONTROLS.**

Local execution artifacts, CLI configurations, cached models, or past successful runs **DO NOT** constitute evidence of zero-cost inference or active subscription coverage.

Every item below requires explicit review, verification, and sign-off by the human account owner before authorizing any live inference experiment.

---

## 2. Owner Verification Checklist

| # | Item Description | Verification Requirement | Status | Owner Sign-Off / Notes |
|---|---|---|---|---|
| **1** | **Active Subscription & Tier** | Confirm active developer tier / subscription (e.g., ChatGPT Plus/Team/Enterprise, Claude Pro/Team, or API Organization tier). | **OWNER VERIFICATION REQUIRED** | |
| **2** | **Model Availability & Entitlement** | Confirm that the target model (e.g., `gpt-5.6-luna` or equivalent designated frontier model) is authorized and accessible without tier-escalation blocks. | **OWNER VERIFICATION REQUIRED** | |
| **3** | **Subscription Inclusions vs Metered API** | Confirm whether model requests routed through the designated CLI/tooling are covered by a flat subscription or billed per token as metered API usage. | **OWNER VERIFICATION REQUIRED** | |
| **4** | **External API Credit Billing** | If metered API billing applies, confirm whether payment is backed by a credit balance or linked credit card. | **OWNER VERIFICATION REQUIRED** | |
| **5** | **Automatic Top-Up Controls** | Check whether automatic reload / auto-recharge is enabled. (Recommended: disable auto-recharge before running experiments). | **OWNER VERIFICATION REQUIRED** | |
| **6** | **Available Total Quota** | Confirm total monthly organization spend limit / token quota. | **OWNER VERIFICATION REQUIRED** | |
| **7** | **Remaining Real-Time Quota** | Verify remaining balance/quota prior to initiating any live trial. | **OWNER VERIFICATION REQUIRED** | |
| **8** | **Usage Reset Schedule** | Identify billing cycle rollover date to ensure experiment does not conflict with operational needs. | **OWNER VERIFICATION REQUIRED** | |
| **9** | **Per-Arm Spending Tolerance** | Confirm maximum dollar loss tolerated per experimental arm (Nominal: 175,000 tokens $\approx$ $1.75 – $3.50; Worst-case: ~$5.00). | **OWNER VERIFICATION REQUIRED** | |
| **10** | **Combined Spending Tolerance** | Confirm maximum dollar loss tolerated across two arms (Nominal: 350,000 tokens $\approx$ $3.50 – $7.00; Worst-case: ~$10.00). | **OWNER VERIFICATION REQUIRED** | |
| **11** | **Tooling Telemetry Reliability** | Verify that the specific CLI/agent tool version in use actually writes well-formed JSONL usage events to stdout or rollout files. | **OWNER VERIFICATION REQUIRED** | |
| **12** | **Provider-Enforced Spend Caps** | Set a hard dollar spend limit in the provider console (e.g. $10.00 monthly spend cap) to eliminate cloud-side overrun risk. | **OWNER VERIFICATION REQUIRED** | |

---

## 3. Explicit Prohibitions and Security Guidelines

1. **Zero Secret Exposure:** Never paste, print, or commit API keys, tokens, session cookies, or billing credentials into this repository, command prompts, or git branches.
2. **Zero Inferred Entitlement:** The AI assistant will not assume marginal cost is zero. The owner must establish whether metered charges apply.
3. **Fail-Closed on Unverified Billing:** If the account terms or spending limits are unconfirmed, the experiment must remain in `NO-GO` or `CONDITIONAL GO` status.
