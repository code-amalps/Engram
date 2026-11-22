# Pre-Flight Billing & Subscription Quota Checklist (v2)

**Document Identifier:** `ENGRAM_DRAFT05_BILLING_CHECKLIST_V2`  
**Governing Context:** Engram Amendment 05 Pass 4 Offline Feasibility Investigation  
**Authentication Paradigm:** ChatGPT Subscription OAuth (`auth_mode: "chatgpt"`)  
**Prerequisite Requirement:** MUST BE VERIFIED AND SIGNED BY OWNER BEFORE ANY LIVE INFERENCE AUTHORIZATION  

---

## 1. Executive Summary & Policy Shift

Prior billing checklists utilized an OpenAI Platform pay-as-you-go dollar balance model ($\text{Exposure} = \text{Tokens} \times \text{Rate}$). Offline forensic inspection of `~/.codex/auth.json` confirmed that the Codex CLI is authenticated via a **ChatGPT subscription account** (`auth_mode: "chatgpt"`).

Under subscription authentication:
- Consumption does not draw down USD credit balances on `platform.openai.com`.
- Consumption is governed by rolling subscription quota windows (e.g., 3-hour sliding window caps and reasoning allocations).
- **Mandatory Policy Rule:** In accordance with evidence-integrity rules, **no plan limits or quota numbers are estimated or stated by the agent**. Every field below remains `UNVERIFIED, owner to fill in`. The verified absence of auto-purchase is a strict prerequisite before live execution.

---

## 2. Subscription Exposure & Allocation Formula

Instead of a dollar-cost formula, quota exposure is governed by subscription window consumption:

$$\text{Projected Quota Impact (\%)} = \frac{\text{Estimated Tokens Consumed}}{\text{Active Rolling Window Token Capacity}} \times 100$$

$$\text{Post-Run Quota Remaining (\%)} = \text{Starting Quota (\%)} - \text{Projected Quota Impact (\%)} - \text{Smoke Test Allowance (\% aph)}$$

---

## 3. Quota & Account Verification Checklist (Owner to Complete)

All fields below must be directly verified in the OpenAI/ChatGPT user settings interface and completed by the project owner:

| Verification Item | Specification / Requirement | Owner Verification Status | Verification Source & Date |
| :--- | :--- | :---: | :--- |
| **Active Plan Tier** | Specific subscription tier (Plus, Team, Enterprise, Pro) | `UNVERIFIED, owner to fill in` | Source: ____________________<br>Date: ____________________ |
| **Codex Rate Limit Cap** | Request or token volume allowed per rolling window | `UNVERIFIED, owner to fill in` | Source: ____________________<br>Date: ____________________ |
| **Rolling Reset Window** | Reset duration (e.g. 3-hour window, 24-hour cycle) | `UNVERIFIED, owner to fill in` | Source: ____________________<br>Date: ____________________ |
| **Pre-Run Usage Reading** | Account usage percentage immediately prior to run | `UNVERIFIED, owner to fill in` | Source: ____________________<br>Date: ____________________ |
| **Post-Run Usage Reading** | Account usage percentage immediately after run completion | `UNVERIFIED, owner to fill in` | Source: ____________________<br>Date: ____________________ |
| **Limit Saturation Behavior** | What occurs upon 100% saturation (HTTP 429, queue throttle, downgrade) | `UNVERIFIED, owner to fill in` | Source: ____________________<br>Date: ____________________ |
| **Auto-Purchase Status** | Automatic billing / purchase of add-on usage **(MUST BE VERIFIED OFF)** | `UNVERIFIED, owner to fill in` | Source: ____________________<br>Date: ____________________ |
| **Maximum Acceptable Exposure** | Ceiling of subscription quota permitted for pilot (e.g. $\le 20\%$ of window) | `UNVERIFIED, owner to fill in` | Source: ____________________<br>Date: ____________________ |
| **Smoke-Test Quota Allowance** | Dedicated separate allocation for offline/online preflight checks | `UNVERIFIED, owner to fill in` | Source: ____________________<br>Date: ____________________ |

---

## 4. Mandatory Sign-off Gate

> [!CAUTION]
> **Strict Authorization Prerequisite:**  
> The absence of automatic payment or add-on purchase features cannot be inferred from configuration files. It must be explicitly checked and certified by the account owner. Under no circumstances may a live benchmark or smoke test be initiated while any item above remains `UNVERIFIED`.

```
================================================================================
OWNER SUBSCRIPTION QUOTA PRE-FLIGHT CERTIFICATION
================================================================================

I confirm that I have directly inspected the active ChatGPT subscription account,
verified that auto-purchase is OFF, and determined that the planned benchmark block
exposure is within acceptable operational and development limits.

Owner Signature: ___________________________________  Date: _________________
================================================================================
```
