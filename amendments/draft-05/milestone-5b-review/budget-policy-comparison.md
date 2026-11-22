# Milestone 5B-Review: Budget Policy Comparison

## 1. Overview and Evaluation Criteria

This document evaluates three candidate budget policies for the proposed controlled token-efficiency experiment. The objective is to identify the safest, most scientifically valid, and financially understood policy before seeking owner authorization.

The policies are evaluated across six dimensions:
1. **Financial Exposure:** Maximum potential cost in USD.
2. **Token Exposure:** Maximum possible token consumption per block.
3. **In-Flight Overrun Vulnerability:** Susceptibility to remote generation leaking past nominal limits.
4. **Premature Truncation Risk:** Probability of stopping an agent before it completes a legitimate solution.
5. **Experimental Validity:** Likelihood of measuring true engineering task completion versus artificial budget starvation.
6. **Operational Complexity:** Engineering overhead required to configure and monitor.

---

## 2. Policy Definitions

### Option A: Nominal Budget with Conservative Reserve (Current Proposed Baseline)
- **Per-Arm Nominal Ceiling:** 175,000 tokens
- **Per-Arm Headroom Reserve:** 45,000 tokens
- **Pre-Dispatch Stop Threshold:** 130,000 tokens
- **Two-Arm Nominal Budget:** 350,000 tokens

### Option B: Lower Nominal Exposure (Strictly Constrained)
- **Per-Arm Nominal Ceiling:** 100,000 tokens
- **Per-Arm Headroom Reserve:** 30,000 tokens
- **Pre-Dispatch Stop Threshold:** 70,000 tokens
- **Two-Arm Nominal Budget:** 200,000 tokens

### Option C: Provider-Enforced Usage Limit (Hard Remote Cap)
- **Per-Arm Token Ceiling:** 175,000 tokens (local)
- **Remote Hard Cap:** Configured via OpenAI Project Monthly Budget or Anthropic Workspace Limit (e.g., $10.00 hard limit) + `max_tokens` completion cap (4,096 tokens per request).

---

## 3. Comparative Evaluation Matrix

| Evaluation Dimension | Option A: Nominal 175k / 45k Reserve | Option B: Constrained 100k / 30k Reserve | Option C: Provider-Enforced Hard Cap |
|---|---|---|---|
| **Max Nominal Exposure (2 Arms)** | 350,000 tokens | 200,000 tokens | 350,000 tokens |
| **Max Overrun Exposure (Worst-Case)**| ~400,000 tokens (if turns exceed 45k) | ~260,000 tokens (if turns exceed 30k) | Strictly bounded by API hard dollar cap |
| **Estimated Financial Exposure** | ~$3.50 – $7.00 (model dependent) | ~$2.00 – $4.00 (model dependent) | Enforced ceiling (e.g., max $10.00 hard stop) |
| **In-Flight Overrun Risk** | Moderate (local polling only) | High (reserve is smaller: 30k < 37.6k hist) | Zero financial overrun past provider limit |
| **Premature Truncation Risk** | Low (historical Task B consumed ~112k) | **Very High** (Task B baseline used 112k) | Low (full budget available) |
| **Experimental Validity** | **High** (Task C is non-trivial AST task) | **Severely Compromised** (artificial halts)| **High** (full task run permitted) |
| **Operational Complexity** | Low (already implemented in harness) | Low (configuration change only) | High (requires external billing dashboard setup)|

---

## 4. In-Depth Trade-Off Analysis

### Option A Analysis
- **Pros:** Option A provides sufficient context headroom for a non-trivial Rust compilation and cache-invalidation task. Historical Task B runs completed within 112,000 tokens, meaning 130,000 is an adequate threshold. The 45,000-token reserve safely absorbs any single-turn jump matching historical observations ($37,651$ tokens).
- **Cons:** It relies solely on local poller process termination. If an agent takes an unprecedented 60,000-token turn, it will overrun the 175k ceiling.

### Option B Analysis
- **Pros:** Reduces total potential token burn by 43%.
- **Cons:** **Scientifically flawed**. In historical Task B trials, the baseline agent consumed 112,450 tokens. Under Option B (stopping at 70,000 tokens), the baseline agent would be prematurely terminated mid-task in 100% of runs. This creates artificial task failure, destroying experimental validity. Furthermore, a 30,000-token reserve is smaller than the observed 37,651-token turn jump, re-introducing the overrun hazard!

### Option C Analysis
- **Pros:** The only mechanism that guarantees a true zero-leakage financial hard ceiling, because the cloud provider server rejects tokens past the dollar threshold.
- **Cons:** Provider project limits cannot be easily adjusted on a granular 175,000-token ($2.50) per-run basis; they operate on a workspace or monthly level. Furthermore, if a run hits a provider dollar cap, the API returns HTTP 429, which must be handled gracefully by the runner.

---

## 5. Recommendation and Limitations

### Recommended Policy: Hybrid Option A + Option C Safeguards
1. **Primary Policy:** **Option A (Nominal 175,000 tokens, 45,000 reserve, 130,000 stop threshold)**.
   - Sizing the reserve at 45,000 is mandatory because reducing it to 30,000 creates an active overrun hazard against the 37,651 historical jump.
   - 130,000 tokens provides genuine headroom for Task C without premature starvation.
2. **Owner Safeguard (Option C Component):**
   - The repository owner must configure an explicit dollar spend cap on their OpenAI / Anthropic developer console (e.g., setting a monthly budget or temporary hard limit of $10.00).
   - This pairs local pre-dispatch monitoring with remote financial guarantees.

### Explicit Limitations:
- The local harness cannot guarantee zero in-flight token leakage.
- If a turn exceeds 45,000 tokens, total per-arm tokens can reach 180,000–190,000 tokens.
- Total two-arm financial exposure is estimated at $3.50 to $7.00 and must be tolerated by the owner.
