# Engram Phase 8.1.1.2: Statistical Reconciliation & Mathematical Evidence Report

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**GitHub Target:** `code-amalps/Engram` (`origin`), branch `feature/phase2-stage5-remediation`  
**Dataset:** `docs/phases/phase_8_1_1_2_raw_results.json` (150 paired iterations / 300 executions)  
**Transport:** Live OS Stdio Pipes (`ChildStdin` / `ChildStdout`)  
**Platform:** Windows x64 AMD64  
**Date:** 23 November 2026  

---

## 1. Executive Reconciliation Summary

Phase 8.1.1.2 formally reconciles all previously ambiguous statistical quantities and provides complete mathematical clarity across two distinct dimensions:
1. **Per-Workload vs Cross-Workload Aggregations:** Explains the relationship between per-workload figures (e.g. EQ-01: 701.7 ms / 1,092.0 ms) and aggregate cross-workload figures (704.1 ms / 1,093.5 ms).
2. **Mean Paired Savings vs Aggregate-Mean Changes:** Distinguishes the average of individual paired ratios ($\frac{1}{N}\sum \frac{B_i - E_i}{B_i}$) from the ratio of sample means ($\frac{\overline{B} - \overline{E}}{\overline{B}}$).

---

## 2. Reconciling Phase 8.1.1 Discrepancies

### 2.1 Reconciling Latency Impact Percentages (`-63.0%`, `-54.7%`, `-48.6%`, `-117.3%`)

In Phase 8.1.1, the summary benchmark table reported latency savings as:
- **EQ-01:** `-63.0%`
- **EQ-02:** `-54.7%`
- **EQ-03:** `-48.6%`
- **EQ-04:** `-117.3%`

However, calculating percentage change from the table's reported aggregate mean durations yielded:
- **EQ-01:** $\frac{701.7 - 1092.0}{701.7} \times 100 = \mathbf{-55.6\%}$ (discrepancy of 7.4%)
- **EQ-02:** $\frac{794.5 - 1186.6}{794.5} \times 100 = \mathbf{-49.4\%}$ (discrepancy of 5.3%)
- **EQ-03:** $\frac{891.6 - 1264.4}{891.6} \times 100 = \mathbf{-41.8\%}$ (discrepancy of 6.8%)
- **EQ-04:** $\frac{428.5 - 831.1}{428.5} \times 100 = \mathbf{-93.9\%}$ (discrepancy of 23.4%)

**Mathematical Resolution:**
The reported figures (`-63.0%`, etc.) were the **Mean Paired Latency Savings** ($\frac{1}{30}\sum_{i=1}^{30} \frac{B_i - E_i}{B_i} \times 100$). Because duration distributions exhibit variance across individual iterations, the arithmetic mean of individual ratios is mathematically not equal to the ratio of arithmetic means:
$$\frac{1}{N}\sum_{i=1}^N \frac{B_i - E_i}{B_i} \neq \frac{\frac{1}{N}\sum B_i - \frac{1}{N}\sum E_i}{\frac{1}{N}\sum B_i}$$
In iterations where baseline completed unusually quickly (e.g., fast cache hit or minimal scheduling delay), $\frac{B_i - E_i}{B_i}$ yielded large negative percentages (e.g., $\frac{200 - 800}{200} = -300\%$), heavily weighting the paired mean downward. In Phase 8.1.1, the table column was ambiguously titled `Latency Savings`, causing reviewers to compare it directly against the aggregate means.

### 2.2 Reconciling Overall Duration Figures (`704.1 ms` / `1,093.5 ms` vs `701.7 ms` / `1,092.0 ms`)

In Phase 8.1.1 prose, the text stated:
> "Consolidated execution in unoptimized debug mode averages 1,093.5 ms vs 704.1 ms for sequential calls"

While the table row for EQ-01 stated:
> Baseline: `701.7 ms`, Engage: `1,092.0 ms`

**Resolution:**
- `701.7 ms` and `1,092.0 ms` were the mean durations for **EQ-01 specifically**.
- `704.1 ms` and `1,093.5 ms` were the unweighted arithmetic mean across **all four Category 1 equivalent workloads combined**:
  $$\text{Baseline Mean} = \frac{701.7 + 794.5 + 891.6 + 428.5}{4} = \frac{2816.3}{4} = \mathbf{704.08\text{ ms}} \approx 704.1\text{ ms}$$
  $$\text{Engage Mean} = \frac{1092.0 + 1186.6 + 1264.4 + 831.1}{4} = \frac{4374.1}{4} = \mathbf{1093.52\text{ ms}} \approx 1093.5\text{ ms}$$
This prose was intended as a cross-workload summary but was misread as an inconsistent report of EQ-01.

---

## 3. Live Stdio OS-Pipe Empirical Benchmark Results (Phase 8.1.1.2)

The newly recorded benchmark executed 150 paired iterations (300 executions) over real OS pipes (`tests/engage_bench_test.rs`).

### 3.1 Comprehensive Metric Table (30 Paired Iterations per Workload)

| Metric | EQ-01 (Single-File) | EQ-02 (Multi-File) | EQ-03 (Subsystem) | EQ-04 (Constitution) | BYPASS-01 (Doc Leaf) |
|---|:---:|:---:|:---:|:---:|:---:|
| **Workload Category** | `EQUIVALENT_WORK` | `EQUIVALENT_WORK` | `EQUIVALENT_WORK` | `EQUIVALENT_WORK` | `TRIVIAL_BYPASS` |
| **Sample Count ($N$)** | 30 | 30 | 30 | 30 | 30 |
| **Baseline Tool Calls** | 2 | 2 | 2 | 2 | 2 |
| **Engage Tool Calls** | 1 | 1 | 1 | 1 | 1 |
| **Baseline Wire Mean** | **5,772.9 B** | **5,870.9 B** | **3,568.9 B** | **3,534.6 B** | **2,382.9 B** |
| Baseline Wire Std Dev | 1.3 B | 1.3 B | 1.3 B | 17.0 B | 1.3 B |
| Baseline Wire Median | 5,773.0 B | 5,871.0 B | 3,569.0 B | 3,538.0 B | 2,383.0 B |
| Baseline Wire Min / Max | 5,769 / 5,777 B | 5,867 / 5,875 B | 3,565 / 3,573 B | 3,443 / 3,538 B | 2,379 / 2,387 B |
| Baseline Wire p95 | 5,773.0 B | 5,871.0 B | 3,569.0 B | 3,538.0 B | 2,383.0 B |
| **Engage Wire Mean** | **3,141.9 B** | **3,258.9 B** | **2,821.9 B** | **3,078.7 B** | **1,874.9 B** |
| Engage Wire Std Dev | 0.5 B | 0.5 B | 0.5 B | 17.6 B | 0.5 B |
| Engage Wire Median | 3,142.0 B | 3,259.0 B | 2,822.0 B | 3,082.0 B | 1,875.0 B |
| Engage Wire Min / Max | 3,140 / 3,142 B | 3,257 / 3,259 B | 2,820 / 2,822 B | 2,984 / 3,082 B | 1,873 / 1,875 B |
| Engage Wire p95 | 3,142.0 B | 3,259.0 B | 2,822.0 B | 3,082.0 B | 1,875.0 B |
| **Mean Paired Wire Savings** | **45.6%** | **44.5%** | **20.9%** | **12.9%** | **21.3%** |
| **Aggregate-Mean Wire Change**| **45.6%** | **44.5%** | **20.9%** | **12.9%** | **21.3%** |
| **Baseline Duration Mean** | **434.6 ms** | **362.8 ms** | **404.0 ms** | **179.0 ms** | **419.6 ms** |
| Baseline Duration Std Dev | 224.4 ms | 93.2 ms | 119.8 ms | 126.8 ms | 86.6 ms |
| Baseline Duration Median | 355.6 ms | 344.8 ms | 379.2 ms | 155.3 ms | 400.2 ms |
| Baseline Duration Min / Max | 302.3 / 1535.9 ms | 288.8 / 847.2 ms | 343.0 / 1036.7 ms | 132.4 / 858.8 ms | 344.6 / 856.7 ms |
| Baseline Duration p95 | 658.7 ms | 403.4 ms | 465.1 ms | 183.8 ms | 505.5 ms |
| **Engage Duration Mean** | **642.0 ms** | **553.3 ms** | **592.0 ms** | **379.3 ms** | **218.4 ms** |
| Engage Duration Std Dev | 105.7 ms | 29.0 ms | 25.3 ms | 50.7 ms | 16.6 ms |
| Engage Duration Median | 614.1 ms | 562.7 ms | 592.7 ms | 368.0 ms | 219.1 ms |
| Engage Duration Min / Max | 513.3 / 930.6 ms | 479.4 / 595.8 ms | 527.7 / 673.6 ms | 318.3 / 617.3 ms | 188.5 / 254.3 ms |
| Engage Duration p95 | 833.6 ms | 590.1 ms | 623.6 ms | 456.1 ms | 246.6 ms |
| **Mean Paired Latency Savings**| **-60.4%** | **-57.5%** | **-52.0%** | **-134.4%** | **+46.8%** |
| **Aggregate-Mean Latency Change**| **-47.7%** | **-52.5%** | **-46.5%** | **-111.9%** | **+47.9%** |

---

## 4. Cross-Workload Aggregate Analysis

### 4.1 Category 1: Equivalent-Work Overall (120 Paired Runs / 240 Executions)
- **Baseline Wire Mean:** $4,686.8\text{ B}$
- **Engage Wire Mean:** $3,075.3\text{ B}$
- **Mean Paired Wire Savings:** **31.0%**
- **Aggregate-Mean Wire Change:** **34.4%**
- **Baseline Duration Mean:** $345.1\text{ ms}$
- **Engage Duration Mean:** $541.6\text{ ms}$
- **Mean Paired Latency Savings:** **-76.1%**
- **Aggregate-Mean Latency Change:** **-56.9%**

### 4.2 Category 2: Trivial-Task Avoided Work (30 Paired Runs / 60 Executions)
- **Baseline Wire Mean:** $2,382.9\text{ B}$
- **Engage Wire Mean:** $1,874.9\text{ B}$
- **Mean Paired Wire Savings:** **21.3%**
- **Aggregate-Mean Wire Change:** **21.3%**
- **Baseline Duration Mean:** $419.6\text{ ms}$
- **Engage Duration Mean:** $218.4\text{ ms}$
- **Mean Paired Latency Savings:** **+46.8%** (Speedup)
- **Aggregate-Mean Latency Change:** **+47.9%** (Speedup)

---

## 5. Comparison: In-Process vs Live Stdio Pipe Transport

| Dimension | Phase 8.1.1 (In-Process Framing) | Phase 8.1.1.2 (Live OS Stdio Pipes) | Variance / Delta | Interpretation |
|---|---|---|:---:|---|
| **EQ-01 Baseline Wire** | 5,776.1 B | 5,772.9 B | -3.2 B (-0.05%) | Monotonic request ID digit width variations (1 vs 2 digits). |
| **EQ-01 Engage Wire** | 3,143.4 B | 3,141.9 B | -1.5 B (-0.05%) | Monotonic request ID digit width variations. |
| **EQ-01 Baseline Latency** | 701.7 ms | 434.6 ms | -267.1 ms (-38.1%) | Faster in live stdio pipe due to persistent server instance. |
| **EQ-01 Engage Latency** | 1,092.0 ms | 642.0 ms | -450.0 ms (-41.2%) | Faster in live stdio pipe due to persistent server instance. |
| **BYPASS-01 Speedup** | +52.8% (paired) | +46.8% (paired) / +47.9% (agg) | Consistent | Avoided AST parsing yields ~47-53% speedup across all transports. |

---

## 6. Mathematical Dataset Validation

The dataset integrity test `tests/engage_bench_test.rs::test_validate_raw_results_integrity_and_statistics` deterministically asserts:
1. Exactly 30 records per workload ($5 \times 30 = 150$).
2. Unique iteration indices (zero duplicates).
3. Alternating execution order (`baseline_then_engage` vs `engage_then_baseline`).
4. Exact call counts (2 baseline, 1 engage).
5. Exact wire summation ($Total = Req + Resp$).
6. Strict positivity and finiteness for all measured durations.
7. Independent recomputation of paired savings percentages matches within $\epsilon < 0.1\%$.
8. Independent recomputation of aggregate metrics matches summary tables within $\epsilon < 0.1\%$.
