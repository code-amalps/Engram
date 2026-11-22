# Milestone 5A: Statistical Analysis Plan
## Preregistered Outcome Metrics, Paired Analysis Methodology, and Censoring Rules

**Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/async-precommit-guard`  
**HEAD Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`  
**Document ID:** `ENGRAM-A05-M5A-STATISTICAL-PLAN`  
**Classification:** Statistical Methodology Specification  

---

### 1. Inferential Status: Exploratory Evaluation

The planned initial sample size for the Milestone 5 live experiment is:

$$N = 3 \text{ Matched Pairs (6 Total Executions)}$$

#### Methodological Declaration:
With $N = 3$, this experiment is formally classified as **EXPLORATORY**. 
- The sample size is insufficient to support asymptotic confirmatory significance claims ($p < 0.05$).
- Statistical analyses will focus on **descriptive estimation, paired effect sizes, and non-parametric directional consistency**.
- No exaggerated claims of statistical significance will be asserted.

---

### 2. Primary and Secondary Outcome Measures

#### 2.1 Primary Outcome Measure
The primary outcome is the **Paired Token Savings Rate** ($\Delta\%_i$) for matched pairs where both arms achieve full functional acceptance ($C = 1.00$):

$$\Delta\%_i = \frac{T_{\text{baseline}, i} - T_{\text{engram}, i}}{T_{\text{baseline}, i}} \times 100\%$$

- **Positive Value ($\Delta\% > 0$):** Indicates token reduction (efficiency gain).
- **Negative Value ($\Delta\% < 0$):** Indicates token overhead (efficiency loss).
- **Aggregate Metric:** Median and Mean of $\Delta\%_i$ across all accepted pairs.

#### 2.2 Secondary Outcome Measures
1. **Task Completion Rate:**
   $$\text{CR}_{\text{baseline}} = \frac{N_{\text{success, baseline}}}{N_{\text{total}}}, \quad \text{CR}_{\text{engram}} = \frac{N_{\text{success, engram}}}{N_{\text{total}}}$$
2. **Turn Difference ($\Delta K_i$):**
   $$\Delta K_i = K_{\text{baseline}, i} - K_{\text{engram}, i}$$
3. **Cumulative Input Token Delta:**
   $$\Delta I_i = I_{\text{baseline}, i} - I_{\text{engram}, i}$$
4. **Cumulative Output Token Delta:**
   $$\Delta O_i = O_{\text{baseline}, i} - O_{\text{engram}, i}$$
5. **Tool Call Delta ($\Delta N_{\text{tools}}$):**
   $$\Delta N_{\text{tools}, i} = N_{\text{tools, baseline}, i} - N_{\text{tools, engram}, i}$$
6. **Agent Wall-Clock Duration Delta ($\Delta W_i$):**
   $$\Delta W_i = W_{\text{baseline}, i} - W_{\text{engram}, i}$$

---

### 3. Handling of Failed, Censored, and Breached Runs

To preserve experimental integrity and prevent cherry-picking:

```
[ Matched Pair Executed ]
          |
          +---> Both Arms C = 1.00?
                   |
                   |-- YES --> Included in Primary Savings Calculation (Δ%)
                   |
                   +-- NO  --> Excluded from Ratio Savings
                               Recorded in Full Intent-to-Treat (ITT) Table
```

#### Rules for Non-Accepted Runs:
1. **Zero Silent Exclusions:** Every initiated run must be documented in the final report, regardless of outcome.
2. **Exclusion from Savings Ratio:** If either Arm A or Arm B fails functional acceptance ($C < 1.00$), reaches a token ceiling, or times out, the pair **must not be included in the quantitative percentage savings metric** ($\Delta\%$). Calculating percentage savings against an incomplete or broken implementation is mathematically invalid.
3. **Intent-to-Treat (ITT) Reporting:** Non-accepted runs will be reported in a comprehensive ITT summary table detailing:
   - Arm identifier
   - Cumulative tokens at termination
   - Completed turns
   - Termination reason (`TOKEN_CEILING_BREACH`, `TIMEOUT`, `TEST_FAILURE`)
4. **Completion Supremacy Invariant:** If the treatment arm achieves lower token consumption but a lower completion rate than the baseline ($\text{CR}_{\text{engram}} < \text{CR}_{\text{baseline}}$), the experimental outcome **must be classified as a REGRESSION**, not a success.

---

### 4. Randomization, Counterbalancing, and Order Effects

To neutralize order-dependent confounds (system cache warming, transient network congestion):

1. **Counterbalanced Schedule ($N = 3$):**
   - Pair 1: Arm A (Baseline) $\rightarrow$ Arm B (Engram)
   - Pair 2: Arm B (Engram) $\rightarrow$ Arm A (Baseline)
   - Pair 3: Arm A (Baseline) $\rightarrow$ Arm B (Engram)
2. **Order-Effect Diagnostics:**
   The analysis will compare $\Delta\%_i$ between A-first and B-first pairs to evaluate whether execution sequence had a measurable influence on token consumption.

---

### 5. Descriptive Statistics & Reporting Standards

The final report must present:
- **Sample Distribution:** Minimum, 25th percentile, Median, 75th percentile, Maximum, Mean, Standard Deviation.
- **Parametric vs. Non-Parametric:** Because sample sizes are small, **Median and Interquartile Range (IQR)** are the authoritative summary statistics; Mean and Standard Deviation are provided for descriptive completeness only.
- **Confidence Intervals:** 95% bootstrap or exact permutation confidence intervals will be reported only if $N \ge 5$; for $N = 3$, exact individual values are tabulated directly.
