# Milestone 5B Authorization Review: Provider & Billing Verification

## 1. Executive Summary

This report performs an offline, read-only inspection of the active agent runner, CLI environment, model provider configuration, and authentication state for Engram Amendment 05.

The investigation was conducted strictly offline with zero live model inference ($0.00 spend), zero external API calls, and zero exposure of sensitive secrets or tokens.

---

## 2. Provider and Execution Architecture

Inspection of `benchmarks/matched-pair-harness/src/orchestrator.py` (lines 45–75) and `runner.py` (lines 460–520) establishes the exact execution pipeline:

```
Harness Orchestrator
   │
   ▼
CodexRunner (runner.py)
   │
   ▼ launches subprocess
Executable: C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe
Arguments: exec --ephemeral --model gpt-5.6-luna --full-auto ...
   │
   ▼ Authenticates using
Configuration Root: C:\Users\Amal P S\.codex\
   ├── auth.json
   ├── config.toml
   └── models_cache.json
   │
   ▼ Dispatches outbound HTTPS requests to
Provider: OpenAI (ChatGPT Backend Service)
```

- **Target Executable:** `C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe`
- **Execution Mode:** `codex exec --ephemeral --full-auto`
- **Target Model Identifier:** `gpt-5.6-luna`
- **Provider:** OpenAI

---

## 3. Authentication & Billing State Inspection

Inspection of `C:\Users\Amal P S\.codex\auth.json` reveals the following properties:

```json
{
  "auth_mode": "chatgpt",
  "OPENAI_API_KEY": null,
  "tokens": {
    "account_id": "<redacted>",
    "access_token": "<redacted>",
    "refresh_token": "<redacted>",
    "id_token": "<redacted>"
  }
}
```

### Key Findings on Billing:

1. **Authentication Mode (`auth_mode = "chatgpt"`):**
   - The CLI authenticates via **ChatGPT OAuth bearer tokens** linked to a user account ID.
   - It **DOES NOT** use a metered OpenAI Platform developer API key (`OPENAI_API_KEY = null`).
2. **Subscription vs. Metered API:**
   - Because `auth_mode` is `"chatgpt"`, usage is billed against the user's **ChatGPT Subscription Plan** (e.g., Plus, Team, or Pro message allowance / usage pool), rather than deducting pay-per-token dollar balances from an API credit card or prepaid credit pool.
3. **Absence of a Local $10 Dollar Cap:**
   - Previous review documents suggested setting a "$10.00 hard cap". Inspection confirms that **no dollar spending limit exists in local configuration**.
   - OpenAI Platform project spend limits apply only to Platform API keys (`sk-...`), which are **not** configured in `auth.json`.
   - ChatGPT subscriptions enforce **rate limits** (e.g. maximum messages per 3-hour window or dynamic weekly allowance), not token-metered dollar caps.
4. **Rate Limit vs Hard Limit Behavior:**
   - When a ChatGPT subscription rate limit is reached, OpenAI returns HTTP 429 (`rate_limit_exceeded`), and Codex halts or waits for cooldown.
   - If an in-flight request is already being processed remotely when a limit threshold is reached, the server generates the full completion for that turn.
   - Killing the local `codex.exe` process terminates the local client, but does not instantaneously abort remote GPU processing; remote inference continues until the remote socket detects closure.

---

## 4. Verification Classification Ledger

| Billing Dimension | Inspected Value | Determination | Status |
|---|---|---|---|
| **Underlying Provider** | OpenAI | Authoritatively established | **VERIFIED** |
| **Execution Binary** | `C:\Users\...\.plugin-appserver\codex.exe` | Binary exists on disk | **VERIFIED** |
| **Authentication Mode** | `chatgpt` (OAuth session tokens) | Uses subscription OAuth | **VERIFIED** |
| **Metered API Key** | `OPENAI_API_KEY: null` | No metered key configured | **VERIFIED** |
| **Account-Level Hard Cap** | Not available in local configuration | Dependent on ChatGPT tier | **OWNER VERIFICATION REQUIRED** |
| **Subscription Quota / Tier** | Not verifiable from offline file inspection | User account dashboard check required | **OWNER VERIFICATION REQUIRED** |
| **Remote In-Flight Termination** | Server-side generation continues until reset | Cannot guarantee zero remote overrun | **QUALIFIED (LOCAL ONLY)** |

---

## 5. Conclusion & Owner Actions Required

The CLI is configured to run via OpenAI ChatGPT subscription OAuth, not metered API credits.
Because the offline harness cannot query the live ChatGPT subscription status or remaining weekly message quota:
- **The owner must verify their ChatGPT subscription status, remaining message caps, and renewal timing prior to authorizing execution.**
- **Status:** **`OWNER VERIFICATION REQUIRED`**.
