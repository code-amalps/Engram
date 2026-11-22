# Proposed Amendment 05: Stage 1 Operational Token Ceiling Calibration & Risk Reduction

**Status:** PROPOSED (DRAFT — INCORPORATING OWNER DECISIONS — PENDING FINAL RATIFICATION SIGN-OFF)  
**Document Path:** `amendments/draft-05/amendment-05.md`  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Target Branch:** `feature/async-precommit-guard`  
**Parent Protocols:**
- Amendment 02 (Stage 1 Core Triad Protocol)
- Amendment 04 (Two-Arm Candidate-First Protocol Revision, `benchmarks/AMENDMENT_04_STAGE1_TOKEN_EFFICIENCY_PROTOCOL_REVISION.md`)

---

## 1. Executive Summary and Decided Operational Parameters

Proposed Amendment 05 establishes calibrated operational boundaries for the next live experimental block of Engram Stage 1 (Task B: `task_b_remember_me`).

Based on formal owner decisions, the operational parameters are fixed as follows:
1. **Per-Arm Token Ceiling:** **`175,000` model tokens**.
2. **Headroom Reserve (Inclusive):** **`15,000` model tokens**, included *within* the 175,000 ceiling.
3. **Pre-Dispatch Headroom Stop Trigger:** **`160,000` cumulative tokens**. The pre-dispatch guard halts execution before dispatching any model turn when cumulative tokens reach 160,000 or when remaining headroom ($175,000 - \text{cumulative}$) is less than the estimated next turn cost.
4. **Combined Planned Block Exposure:** **`350,000` model tokens** ($2 \times 175,000$) across the candidate-first two-arm schedule (`v154_arm3` followed by `baseline_arm1`).
5. **Overrun Handling Rule:** Any run that exceeds 175,000 tokens due to an in-flight final turn is flagged **`OVERRUN_CENSORED`**, analyzed strictly as right-censored data, and is **NOT eligible to support a comparative token savings claim** ($S$ strictly suppressed).

---

## 2. Pre-Registration Integrity Note

### 2.1 What Is Being Changed and Why
- **Change:** The per-arm operational token ceiling is reduced from **250,000 to 175,000 tokens**, with the pre-dispatch stop trigger set to **160,000 tokens**. The maximum planned block exposure is reduced from **500,000 to 350,000 tokens**.
- **Why:** This reduction is driven strictly by budget constraints and financial risk containment to prevent excessive quota consumption during live inference trials.

### 2.2 Influence of Prior Outcome Data (Explicit Disclosure)
Scientific integrity requires explicit disclosure: **This change is directly informed by observed empirical data from prior benchmark blocks**:
- In Block 06, the unassisted baseline arm (`baseline_arm1`) accumulated **253,748 tokens** across 9 turns before being halted by the 250,000 ceiling.
- In Block 07, the candidate arm (`v154_arm3`) accumulated **270,506 tokens** across 11 turns before being halted.
- **Protocol Impact:** Lowering the ceiling to 175,000 (stop trigger 160,000) after observing that both baseline and candidate arms required >250,000 tokens to attempt Task B substantially increases the probability that runs will terminate prematurely due to budget exhaustion ($C = 0.00$). This change weakens the preregistered discovery envelope in exchange for strict expenditure capping.

### 2.3 Comparability with Amendment 04
- **Comparable Elements:**
  - Candidate-first two-arm schedule (`v154_arm3` Order 1 $\rightarrow$ `baseline_arm1` Order 2, `v151_arm2` deferred).
  - Stop-on-failure rule: Candidate failure immediately aborts the block, skipping baseline.
  - Strict suppression of savings claims ($S$ suppressed unless mutual functional acceptance $C=1.00$).
  - 100% write isolation via `copy` dependency strategy.
  - Telemetry provenance requirement (`SESSION_ROLLOUT` extraction only).
- **Non-Comparable Elements:**
  - Functional task completion rate ($C$) and raw cumulative token expenditure are **NOT directly comparable** across the 250,000 and 175,000 regimes. Runs under Amendment 05 operate under a 30% tighter stopping horizon and must be evaluated as a distinct, budget-constrained trial series.

---

## 3. Protocol Comparison: Amendment 04 vs. Decided Amendment 05

| Protocol Element | Amendment 04 (Ratified) | Proposed Amendment 05 (Decided) | Reason for Change / Rationale |
| :--- | :--- | :--- | :--- |
| **Arm Schedule** | Two-Arm: `v154_arm3` $\rightarrow$ `baseline_arm1` (`v151_arm2` deferred) | Two-Arm: `v154_arm3` $\rightarrow$ `baseline_arm1` (`v151_arm2` deferred) | **Unchanged:** Candidate-first schedule avoids wasting baseline quota if candidate fails. |
| **Per-Arm Token Ceiling** | `250,000` model tokens | **`175,000` model tokens** | Budget-driven risk containment (-30% per arm). |
| **Headroom Reserve** | `15,000` tokens | **`15,000` tokens (Inclusive)** | **Decided:** Reserve is part of the 175k ceiling. |
| **Pre-Dispatch Stop Trigger** | `235,000` tokens ($250\text{k} - 15\text{k}$) | **`160,000` tokens ($175\text{k} - 15\text{k}$)** | **Decided:** Halts execution when headroom drops below estimated turn cost. |
| **Maximum Planned Exposure** | `500,000` model tokens ($2 \times 250\text{k}$) | **`350,000` model tokens ($2 \times 175\text{k}$)** | Caps combined two-arm expenditure at 350,000 tokens. |
| **Overrun Handling** | Standard `TOKEN_CEILING_BREACHED` halt | **`OVERRUN_CENSORED` classification** | **Decided:** Flagged, analyzed as censored data, ineligible for savings claims. |
| **Stop-on-Failure Rule** | Candidate failure immediately halts block; skips Arm 2 | Candidate failure immediately halts block; skips Arm 2 | **Unchanged:** Conserves baseline tokens if candidate fails. |
| **Comparative Savings ($S$)** | Strictly suppressed unless $C_{\text{treat}} = 1.00 \land C_{\text{ctrl}} = 1.00$ | Strictly suppressed unless $C_{\text{treat}} = 1.00 \land C_{\text{ctrl}} = 1.00$ | **Unchanged:** Zero comparative claims on incomplete or overrun tasks. |
| **Telemetry Provenance** | Authoritative `SESSION_ROLLOUT` parsing | Authoritative `SESSION_ROLLOUT` parsing | **Unchanged:** Prevents synthetic or fabricated token accounting. |
| **Dependency Isolation** | Mandatory `copy` strategy | Mandatory `copy` strategy | **Unchanged:** Guarantees 100% write isolation. |

---

## 4. Token Definitions and Authoritative Accounting

All token accounting under Amendment 05 adheres to the following definitions:
1. **Total Model Tokens ($T_{\text{model}}$):** Sum of input tokens (cached + uncached), completion tokens, and reasoning tokens:
   $$T_{\text{model}} = T_{\text{input\_uncached}} + T_{\text{input\_cached}} + T_{\text{completion}} + T_{\text{reasoning}}$$
2. **Cached Tokens:** Counted at **100% face value** towards the operational ceiling $T_{\text{model}}$ and the 160,000 stop trigger. Financial billing discounts apply only to monetary reconciliation, NOT to context window accounting.
3. **Authoritative Source:** Token numbers are extracted exclusively from native OpenAI session rollout files (`~/.codex/sessions/**/rollout-*.jsonl`) via `SESSION_ROLLOUT` provenance. Streaming stdout estimates are used strictly for real-time monitoring and pre-dispatch termination.

---

## 5. Overrun Mechanics and Decided Policy

### 5.1 Race Condition Mechanics
The test harness operates externally to the Codex CLI child process:
- Subprocess stdout flushing and asynchronous session rollout writing have a non-zero latency of **100 ms to 500 ms**.
- When Codex receives a tool response (e.g., shell output), it may dispatch the next HTTP inference request in $< 50\text{ms}$.
- If a turn is already dispatched before telemetry flushes, the request completes on remote servers before the runner can issue `taskkill`.
- For example, in Block 07, Turn 10 finished at 240,504 tokens. Turn 11 completed at 270,506 tokens before process termination (a 20,506-token overrun beyond 250k).

### 5.2 Decided Overrun Rule (Owner Decision 2)
In the event that an arm exceeds the 175,000-token ceiling due to an in-flight final turn:
1. **Execution Status:** Marked as **`OVERRUN_CENSORED`**.
2. **Savings Eligibility:** **STRICTLY INELIGIBLE**. The arm cannot be used to support any comparative token savings or efficiency claim ($S$ is suppressed).
3. **Data Analysis:** The run data is retained and analyzed strictly as right-censored observation in descriptive reports, documenting the exact overrun delta without presenting it as an accepted benchmark arm.

---

## 6. Ratification Block

```
[   ] RATIFIED — Adopt Amendment 05
      Per-Arm Token Ceiling:       175,000 tokens
      Pre-Dispatch Stop Trigger:   160,000 tokens (15,000 headroom reserve inclusive)
      Combined Planned Exposure:   350,000 tokens maximum
      Overrun Classification:      OVERRUN_CENSORED (Savings claims ineligible)

[   ] REJECTED — Maintain Ratified Amendment 04 (250,000 tokens per arm)

Signature: ___________________________________  Date: ________________________
```

---

## 7. Rollback Note

If Amendment 05 is rejected or revoked:
- Execution parameters immediately revert to **Amendment 04**:
  - Per-arm ceiling: `250,000` tokens
  - Pre-dispatch stop trigger: `235,000` tokens ($250\text{k} - 15\text{k}$)
  - Maximum planned block exposure: `500,000` tokens
- Active harness configuration files (`orchestrator.py`, `runner.py`) require no modifications, as they remain pinned to Amendment 04 defaults.

---

## Appendix: Options Considered (Historical Record)

To preserve architectural provenance, the alternative options evaluated prior to the owner's determination are recorded below:

### Threshold Interpretation Options Considered
- **Option 1A (Adopted):** 175,000 per arm *includes* the 15,000 reserve; stop trigger is 160,000 tokens. Combined planned exposure: 350,000 tokens.
- **Option 1B (Rejected):** 175,000 per arm base threshold *plus* 15,000 reserve on top; stop trigger is 175,000 tokens; hard cap at 190,000 tokens ($380,000$ combined).

### Overrun Handling Options Considered
- **Option 2A (Rejected):** Strict immediate failure with discard of all telemetry.
- **Option 2B (Rejected):** Manual review exception allowing savings claims if functional gates passed despite the overrun.
- **Option 2C (Adopted with Stringency):** Flagged, analyzed strictly as right-censored descriptive data, and completely barred from supporting comparative savings claims.
