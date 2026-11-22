# Engram Billing and Financial Risk Verification Checklist

**Document Path:** `amendments/draft-05/billing-checklist.md`  
**Purpose:** Preflight verification of billing provider, account credentials, rates, hard spending limits, and worst-case financial exposure prior to live benchmark authorization under Proposed Amendment 05.  
**Instruction:** **All entries are currently UNVERIFIED placeholders. The human owner must verify and fill in every field prior to authorizing any live inference.**

---

## 1. Billing Provider and Authentication Architecture Check

> [!IMPORTANT]
> **Repository Configuration Finding:**  
> Audit of `orchestrator.py` and `runner.py` shows that the test harness executes:  
> `C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe exec --json -m gpt-5.6-luna ...`  
> - **In-Repo Configuration:** The harness specifies model `gpt-5.6-luna` and reasoning `medium`, but specifies **NO** in-repo API key, organization ID, or billing portal URL. It inherits ambient `os.environ` and relies on Codex CLI's internal credential store.  
> - **Provider Inconclusive:** Whether `codex.exe` bills against an OpenAI Platform API key, a ChatGPT Team/Enterprise subscription, an Azure OpenAI endpoint, or another provider is **inconclusive from repository configuration alone**.  
> - **Owner Verification Required:** The owner must verify which backend account and billing entity is actually debited by `codex.exe`.

| Provider Verification Item | Current Status | Owner Verified Value | Verification Source | Date Verified |
| :--- | :--- | :--- | :--- | :--- |
| **Active Billing Provider** | UNVERIFIED (Config Inconclusive) | `[ OWNER TO FILL IN ]` (e.g. OpenAI Platform, Azure, Subscription) | `codex login status` or account portal | `[ YYYY-MM-DD ]` |
| **Billing Entity / Org ID** | UNVERIFIED, owner to fill in | `[ OWNER TO FILL IN ]` | Provider billing dashboard | `[ YYYY-MM-DD ]` |
| **Billing Tier / Account Type** | UNVERIFIED, owner to fill in | `[ OWNER TO FILL IN ]` (e.g., Tier 1-5, Pay-as-you-go, Subscription) | Provider billing dashboard | `[ YYYY-MM-DD ]` |
| **Available Prepaid Credits / Balance** | UNVERIFIED, owner to fill in | `$[ OWNER TO FILL IN ]` | Provider billing dashboard | `[ YYYY-MM-DD ]` |
| **Monthly Hard Spend Limit** | UNVERIFIED, owner to fill in | `$[ OWNER TO FILL IN ]` | Provider usage limits settings | `[ YYYY-MM-DD ]` |
| **Monthly Soft Spend Alert Threshold** | UNVERIFIED, owner to fill in | `$[ OWNER TO FILL IN ]` | Provider usage limits settings | `[ YYYY-MM-DD ]` |
| **Current Month-to-Date Spend** | UNVERIFIED, owner to fill in | `$[ OWNER TO FILL IN ]` | Provider usage dashboard | `[ YYYY-MM-DD ]` |
| **Remaining Spend Buffer Before Hard Cap** | UNVERIFIED, owner to fill in | `$[ OWNER TO FILL IN ]` | Calculated (`Hard Limit` - `MTD Spend`) | `[ YYYY-MM-DD ]` |

---

## 2. Model Pricing and Rate Matrix (`gpt-5.6-luna` / Target Model)

> [!NOTE]
> Rates must reflect the exact model ID and reasoning tier designated for the benchmark.

| Token Category | Status | Unit Rate ($ / 1M Tokens) | Owner Verified Source | Date Verified |
| :--- | :--- | :--- | :--- | :--- |
| **Uncached Input Tokens ($R_{\text{in}}$)** | UNVERIFIED, owner to fill in | `$[ OWNER TO FILL IN ]` / 1M | Provider pricing documentation | `[ YYYY-MM-DD ]` |
| **Cached Input Tokens ($R_{\text{cached}}$)** | UNVERIFIED, owner to fill in | `$[ OWNER TO FILL IN ]` / 1M | Provider pricing documentation | `[ YYYY-MM-DD ]` |
| **Output / Completion Tokens ($R_{\text{out}}$)** | UNVERIFIED, owner to fill in | `$[ OWNER TO FILL IN ]` / 1M | Provider pricing documentation | `[ YYYY-MM-DD ]` |
| **Reasoning Tokens (if billed separately)** | UNVERIFIED, owner to fill in | `$[ OWNER TO FILL IN ]` / 1M | Provider pricing documentation | `[ YYYY-MM-DD ]` |

---

## 3. Spending Controls and Risk Limits (Amendment 05 Aligned)

| Risk Control Parameter | Status | Owner Mandated Threshold | Rationale |
| :--- | :--- | :--- | :--- |
| **Maximum Acceptable Exposure for Smoke Test** | UNVERIFIED, owner to fill in | `$[ OWNER TO FILL IN ]` (tokens: `[ OWNER TO FILL IN ]`) | Trivial read-only preflight check |
| **Per-Arm Planned Stop Trigger** | FIXED (Amendment 05) | **160,000 tokens** | 175,000 ceiling minus 15,000 reserve |
| **Maximum Acceptable Exposure for Single Arm** | FIXED (Amendment 05) | **175,000 tokens** (`$[ OWNER TO FILL IN ]`) | Operational per-arm ceiling |
| **Maximum Planned Exposure for Block (2 Arms)** | FIXED (Amendment 05) | **350,000 tokens** (`$[ OWNER TO FILL IN ]`) | Planned two-arm candidate-first block |
| **Absolute Financial Loss Cap for Run** | UNVERIFIED, owner to fill in | `$[ OWNER TO FILL IN ]` | Hard kill line for entire experiment |

---

## 4. Worst-Case Exposure Formulas

To calculate the financial liability prior to launch, the owner must apply the following symbolic formulas:

### 4.1 Nominal Planned Exposure (No Overrun)
Assuming maximum context within the 175,000 per-arm ceiling without overrun across both arms:
$$\text{Cost}_{\text{planned}} = 2 \times \left( T_{\text{uncached}} \cdot R_{\text{in}} + T_{\text{cached}} \cdot R_{\text{cached}} + T_{\text{out}} \cdot R_{\text{out}} \right)$$
Where $T_{\text{uncached}} + T_{\text{cached}} + T_{\text{out}} \le 175,000$ tokens per arm ($350,000$ total).

### 4.2 Absolute Worst-Case Exposure (With Maximum Potential Overrun)
Accounting for an in-flight final turn overrun where the final turn request consumes maximum single-request context ($T_{\text{max\_turn}}$):
$$\text{Worst-Case Exposure} = 2 \times \left( 175,000 \cdot R_{\text{max}} + T_{\text{max\_turn}} \cdot R_{\text{max}} \right)$$
Where:
- $R_{\text{max}} = \max(R_{\text{in}}, R_{\text{out}})$
- $T_{\text{max\_turn}}$ is the maximum context window of a single request (empirically observed up to 35,000-40,000 tokens on Task B).
- *Policy Note:* Under Amendment 05 Owner Decision 2, any overrun beyond 175,000 is marked `OVERRUN_CENSORED` and barred from savings claims.

---

## 5. Post-Run Billing Reconciliation Procedure

Immediately following any authorized live arm or smoke test:
1. **Authoritative Rollout Extraction:**
   Run `telemetry.py` to extract exact `prompt_tokens_uncached`, `prompt_tokens_cached`, and `completion_tokens` from `~/.codex/sessions/**/rollout-*.jsonl`.
2. **Dashboard Invoice Check:**
   Navigate to the verified provider usage dashboard and record the exact dollar amount debited for the session time window.
3. **Reconciliation Variance Formula:**
   $$\text{Variance (\%)} = \frac{\left| \text{Dashboard Dollar Spend} - \text{Harness Calculated Spend} \right|}{\text{Dashboard Dollar Spend}} \times 100$$
4. **Discrepancy Reporting:**
   If $\text{Variance} > 5\%$, freeze all further testing and identify whether cached token discounts or reasoning multipliers explain the gap.
