# Codex Execution Authentication and Billing Identification Report

**Document Identifier:** `ENGRAM_DRAFT05_BILLING_IDENTIFICATION`  
**Configuration Inspected:** `~/.codex/auth.json` (Local Codex configuration)  
**Parent Document:** [`amendments/draft-05/amendment-05.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/amendment-05.md)

---

## 1. Executive Summary

This report establishes the authentication mode, account billing mechanism, and quota implications for Codex CLI executions under the Engram benchmark harness.

### Confidentiality and Safety Assurance:
In strict compliance with authorization boundaries, **no API keys, tokens, session secrets, or bearer credentials are printed, logged, or reproduced in this document**.

---

## 2. Authentication Configuration Findings

Offline inspection of the Codex authentication store (`~/.codex/auth.json`) reveals the following structural configuration:

| Configuration Property | Observed Setting | Description & Quota Implication |
| :--- | :--- | :--- |
| **`auth_mode`** | `"chatgpt"` | The Codex CLI is authenticated via **ChatGPT Web/Subscription OAuth**, not a direct developer API key. |
| **`OPENAI_API_KEY` Environment Variable** | Not Set / Null | No developer Platform API key is active in the environment or harness process context. |
| **Session Authentication Store** | Present (`tokens` object) | Contains OAuth session authentication tokens linked to the user's active ChatGPT subscription. |

---

## 3. Billing & Quota Impact for Benchmark Pilots

1. **Absence of Direct Metered API Consumption:**  
   Because `auth_mode` is `"chatgpt"`, inferences executed by `codex exec` do NOT consume pay-as-you-go OpenAI API credits (e.g. usage billed in USD on `platform.openai.com`).
2. **Subscription Rate Limit Accounting:**  
   Instead, execution volume consumes subscription allocation windows (e.g., standard ChatGPT Plus/Team 3-hour sliding window caps for advanced models like `gpt-5.6-luna`).
3. **Relevance to Token Ceilings:**  
   While monetary billing in dollars does not apply per token, the harness token ceiling ($175,000$ per arm) and pre-dispatch headroom guard ($15,000$) are essential to:
   - Prevent depleting the user's ChatGPT subscription rolling usage window.
   - Prevent hitting model context length limits (200k/256k window saturation).
   - Prevent multi-turn polling loops (such as the 108k-token loop observed in Block 08) from starving interactive development quota.
   - Maintain parity between native baseline and Engram-assisted arms under identical resource bounds.
