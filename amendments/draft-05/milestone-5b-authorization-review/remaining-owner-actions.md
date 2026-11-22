# Milestone 5B Authorization Review: Remaining Owner Actions

## 1. Context and Purpose

This document outlines the exact, mandatory human owner actions required prior to granting authorization for the Milestone 5B $N=1$ pilot trial.

The repository harness is fully verified, isolated, and hardened offline. However, external model access, account rate limits, and billing tolerances cannot be validated autonomously by the agent.

---

## 2. Action Item 1: ChatGPT Subscription & Account Verification

- **Action:** Open the OpenAI account dashboard or ChatGPT interface associated with the logged-in session in `C:\Users\Amal P S\.codex\auth.json`.
- **Confirm:**
  - Active subscription tier (Plus, Team, Enterprise, or Pro).
  - Current message rate limit usage and reset schedule.
  - Absence of account-level restrictions or temporary suspensions.

---

## 3. Action Item 2: Confirm Model Entitlement for `gpt-5.6-luna`

- **Action:** Verify that the account currently has active access to `gpt-5.6-luna`.
- **Offline Evidence:** `gpt-5.6-luna` appears in `models_cache.json` with `"supported_in_api": true`.
- **Owner Confirmation:** Confirm that launching a session with `gpt-5.6-luna` is permitted under the subscription and will not trigger unexpected tier upgrades.

---

## 4. Action Item 3: Acknowledge Financial & Overrun Tolerances

- **Action:** Review the budget boundaries specified in `n1-pilot-authorization-specification.md`.
- **Acknowledge:**
  - Combined nominal planned exposure is **350,000 tokens** across 2 arms.
  - The local pre-dispatch monitor halts subsequent turn dispatches when usage exceeds 130,000 tokens per arm.
  - A worst-case single terminal turn overrun can push per-arm consumption to **~180,000 – 190,000 tokens** (~$5.00 – $10.00 equivalent).
  - In-flight cloud generation is not canceled synchronously upon local process termination.

---

## 5. Action Item 4: Issue Formal Authorization Prompt

If all three preceding actions are verified and satisfactory, the owner may authorize the trial by submitting an explicit instruction:

> *"I have verified my subscription entitlement, rate limits, and spending tolerances. I authorize the Milestone 5B pilot trial ($N=1$) under Task C according to the n1-pilot-authorization-specification."*

Until this prompt is received, the agent and harness will remain at rest.
