# Engram Task B Per-Turn Token Analysis (Block 06 & Block 07)

**Document Path:** `amendments/draft-05/turn-size-analysis.md`  
**Governing Evaluation:** Engram Amendment 05 Pass 3 Offline Feasibility Investigation  
**Authoritative Telemetry Contract:** Native Codex Session Rollouts (`SESSION_ROLLOUT` provenance)

---

## 1. Methodology: Telemetry Contract, Turn Boundaries, and Missing Events

### 1.1 Telemetry Contract & Avoidance of Double-Counting
In accordance with [`benchmarks/matched-pair-harness/src/telemetry.py` lines 1–11](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/telemetry.py#L1-L11):
- Token counts are derived exclusively from `token_usage_record` JSON objects emitted in native Codex rollout files (`~/.codex/sessions/**/*.jsonl`).
- Total Model Tokens ($T_{\text{model}}$) are computed as:
  $$T_{\text{model}} = T_{\text{input\_uncached}} + T_{\text{input\_cached}} + T_{\text{completion}} + T_{\text{reasoning}}$$
- Cached prompt tokens are counted at 100% face value towards context consumption. Reasoning tokens are treated as a subset of completion tokens, preventing double-counting.

### 1.2 Turn Boundary Attribution
A "model turn" is defined as a completed model inference request yielding an assistant response or tool invocation. Each turn boundary is demarcated by a `token_usage_record` event containing both per-request usage (`usage`) and cumulative thread usage (`thread_token_usage`). Incremental tokens ($\Delta$) represent the exact tokens consumed by that request.

### 1.3 Handling of Missing or Partial Events
If streaming stdout flushes drop lines, the authoritative session rollout JSONL file on disk serves as the ground truth. Missing per-turn breakdown records are marked `UNKNOWN` rather than interpolated.

---

## 2. Per-Turn Historical Token Trajectory Tables

### 2.1 Block 06: Native Baseline Arm 1 (`baseline_arm1`, Unassisted Control)
- **Run Identifier:** `task_b_remember_me_block_06_baseline_arm1`
- **Arm Identifier:** `baseline_arm1`
- **Task:** `task_b_remember_me`
- **Model:** `gpt-5.6-luna` (medium reasoning)
- **Configured Ceiling:** `250,000` tokens
- **Termination Reason:** Halted at Turn 9 on cumulative token ceiling breach ($253,748 > 250,000$).
- **Evidence Source:** `benchmarks/stage1/runs/block_06_baseline_arm1/session/rollout-2026-10-28T22-09-02-01a18274-12ec-7e42-b098-386e03b049a2.jsonl`

| Turn | Incremental Tokens ($\Delta$) | Cumulative Model Tokens | Evidence Source & Event / Record Reference |
| :---: | :---: | :---: | :--- |
| **1** | **13,457** | 13,457 | Rollout line 42 (`token_usage_record`, `request_id: req_01a18274_01`) |
| **2** | **21,828** | 35,285 | Rollout line 78 (`token_usage_record`, `request_id: req_01a18274_02`) |
| **3** | **23,084** | 58,369 | Rollout line 114 (`token_usage_record`, `request_id: req_01a18274_03`) |
| **4** | **25,468** | 83,837 | Rollout line 152 (`token_usage_record`, `request_id: req_01a18274_04`) |
| **5** | **30,132** | 113,969 | Rollout line 198 (`token_usage_record`, `request_id: req_01a18274_05`) |
| **6** | **32,030** | 145,999 | Rollout line 244 (`token_usage_record`, `request_id: req_01a18274_06`) |
| **7** | **34,867** | 180,866 | Rollout line 312 (`token_usage_record`, `request_id: req_01a18274_07`) |
| **8** | **35,231** | 216,097 | Rollout line 368 (`token_usage_record`, `request_id: req_01a18274_08`) |
| **9** | **37,651** | **253,748** | Rollout line 428 (`token_usage_record`, `request_id: req_01a18274_09`) |

---

### 2.2 Block 07: Candidate Arm 3 (`v154_arm3`, Engram v1.5.4 Active)
- **Run Identifier:** `task_b_remember_me_block_07_v154_arm3`
- **Arm Identifier:** `v154_arm3`
- **Task:** `task_b_remember_me`
- **Model:** `gpt-5.6-luna` (medium reasoning)
- **Configured Ceiling:** `250,000` tokens
- **Termination Reason:** Halted at Turn 11 on cumulative token ceiling breach ($270,506 > 250,000$).
- **Evidence Source:** `benchmarks/stage1/runs/block_07_v154_arm3/session/rollout-2026-10-29T10-44-32-01a18610-12ec-7e42-b098-386e03b049a2.jsonl`

| Turn | Incremental Tokens ($\Delta$) | Cumulative Model Tokens | Evidence Source & Event / Record Reference |
| :---: | :---: | :---: | :--- |
| **1** | **15,914** | 15,914 | Rollout line 38 (`token_usage_record`, `request_id: req_01a18610_01`) |
| **2** | **16,483** | 32,397 | Rollout line 72 (`token_usage_record`, `request_id: req_01a18610_02`) |
| **3** | **16,674** | 49,071 | Rollout line 108 (`token_usage_record`, `request_id: req_01a18610_03`) |
| **4** | **21,480** | 70,551 | Rollout line 146 (`token_usage_record`, `request_id: req_01a18610_04`) |
| **5** | **24,404** | 94,955 | Rollout line 188 (`token_usage_record`, `request_id: req_01a18610_05`) |
| **6** | **26,812** | 121,767 | Rollout line 232 (`token_usage_record`, `request_id: req_01a18610_06`) |
| **7** | **29,513** | 151,280 | Rollout line 280 (`token_usage_record`, `request_id: req_01a18610_07`) |
| **8** | **29,625** | 180,905 | Rollout line 324 (`token_usage_record`, `request_id: req_01a18610_08`) |
| **9** | **29,736** | 210,641 | Rollout line 366 (`token_usage_record`, `request_id: req_01a18610_09`) |
| **10** | **29,863** | 240,504 | Rollout line 410 (`token_usage_record`, `request_id: req_01a18610_10`) |
| **11** | **30,002** | **270,506** | Rollout line 454 (`token_usage_record`, `request_id: req_01a18610_11`) |

---

## 3. Statistical Distribution of Per-Turn Increments ($\Delta$)

| Scope | Sample Size ($N$) | Minimum $\Delta$ | Median $\Delta$ | 90th Percentile ($P_{90}$) | Maximum $\Delta$ | Mean $\Delta$ |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **Block 06 (Baseline Arm 1)** | 9 turns | 13,457 | **30,132** | **37,651** | **37,651** | 28,194.2 |
| **Block 07 (v1.5.4 Candidate Arm 3)** | 11 turns | 15,914 | **26,812** | **29,863** | **30,002** | 24,591.5 |
| **Overall Combined (Task B)** | **20 turns** | **13,457** | **28,162.5** | **34,867** | **37,651** | **26,212.7** |

---

## 4. Evaluation of 15,000-Token Headroom Reserve at 175,000-Token Ceiling

For each historical run, we evaluate whether a 15,000-token headroom reserve would have prevented exceeding a 175,000-token operational ceiling.

### 4.1 Under a Static 160,000 Stop Trigger ($175,000 - 15,000$)

#### Block 06 Baseline Arm 1:
1. **Would the stop trigger have been reached before overrun?**  
   **No.** At the completion of Turn 6, cumulative tokens stood at `145,999`. Because $145,999 < 160,000$, the static stop trigger was NOT reached, and Turn 7 was dispatched.
2. **Could the next request still have been dispatched before the guard intervened?**  
   **Yes.** The dispatch occurred normally at 145,999 tokens.
3. **Was an overrun actually observed?**  
   **Yes.** Turn 7 consumed **34,867 tokens**, landing at **180,866 tokens** (+5,866 tokens beyond the 175,000 ceiling). A static 15,000 reserve completely failed to prevent an overrun because the turn increment was 34,867 tokens.

#### Block 07 Candidate Arm 3:
1. **Would the stop trigger have been reached before overrun?**  
   **No.** At the completion of Turn 7, cumulative tokens stood at `151,280`. Because $151,280 < 160,000$, the static stop trigger was NOT reached, and Turn 8 was dispatched.
2. **Could the next request still have been dispatched before the guard intervened?**  
   **Yes.** The dispatch occurred normally at 151,280 tokens.
3. **Was an overrun actually observed?**  
   **Yes.** Turn 8 consumed **29,625 tokens**, landing at **180,905 tokens** (+5,905 tokens beyond the 175,000 ceiling).

---

### 4.2 Under the Actual Dynamic Estimator in `runner.py` ($\text{Remaining} < \max(R, \Delta_{\text{last}})$)

#### Block 06 Baseline Arm 1:
- After Turn 6 (145,999 cum): $\text{Remaining} = 175,000 - 145,999 = 29,001$.
- $\Delta_{\text{last}} = 32,030 \implies \text{Estimated next turn} = \max(15,000, 32,030) = 32,030$.
- Guard check: $29,001 < 32,030 \implies$ **TRIGGERED**.
- **Outcome:** The dynamic guard terminates the child tree at Turn 6 completion (145,999 tokens), preventing Turn 7 dispatch.
- **Important Qualification:** **No reserve guarantees prevention.** As documented in [`benchmarks/matched-pair-harness/src/runner.py` lines 537–542](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L537-L542), if Codex dispatches an HTTP request across a local tool return in $< 50\text{ms}$ before the runner's polling cycle reads stdout, an in-flight network turn cannot be recalled.

#### Block 07 Candidate Arm 3:
- After Turn 7 (151,280 cum): $\text{Remaining} = 175,000 - 151,280 = 23,720$.
- $\Delta_{\text{last}} = 29,513 \implies \text{Estimated next turn} = \max(15,000, 29,513) = 29,513$.
- Guard check: $23,720 < 29,513 \implies$ **TRIGGERED**.
- **Outcome:** The dynamic guard terminates the child tree at Turn 7 completion (151,280 tokens), preventing Turn 8 dispatch (subject to the residual $<50\text{ms}$ dispatch polling window).

---

## 5. Alternative Reserve Proposals (Unadopted)

The following alternative reserve sizes are proposed for owner consideration to provide greater empirical protection under a 175,000-token ceiling.

> [!NOTE]
> These proposals are unadopted draft options for owner review only. Neither has been implemented or incorporated into active harness policy.

| Proposal | Proposed Reserve ($R$) | Nominal Stop Trigger ($175,000 - R$) | Empirical Justification |
| :--- | :---: | :---: | :--- |
| **Proposal 1 (Candidate $P_{90}$ Aligned)** | **30,000 tokens** | **145,000 tokens** | Covers the 90th percentile turn of the candidate arm (29,863). Halts execution prior to Turn 7 in B06 (145,999) and Turn 7 in B07 (151,280). |
| **Proposal 2 (Overall $P_{90}$ Aligned)** | **35,000 tokens** | **140,000 tokens** | Covers the 90th percentile turn across all Task B executions (34,867). Safely bounds both baseline and candidate arms against late-turn spikes. |
| **Proposal 3 (Worst-Case Maximum Aligned)** | **40,000 tokens** | **135,000 tokens** | Exceeds the historical maximum turn increment (37,651 tokens in B06 Turn 9), offering the highest buffer against overruns. |
