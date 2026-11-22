# Engram Phase 5.4.22: Global Reserve Semantics Audit

**Execution Mode:** STRICTLY OFFLINE, READ-ONLY FORENSIC AUDIT  
**Project:** Engram Benchmark Harness  
**Branch:** `feature/phase2-stage5-remediation`  
**Git Baseline HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Authorization Status:** `NOT_AUTHORIZED`  
**Phase Verdict:** `MANIFEST_BLOCKED`

---

## 1. Executive Summary

This forensic audit rigorously resolves the exact architectural and operational semantics of the global headroom reserve within the Engram matched-pair benchmark harness (`benchmarks/matched-pair-harness/src/orchestrator.py` and `runner.py`).

The primary finding is that the 45,000-token headroom reserve is implemented strictly as a **Pre-Dispatch Eligibility Threshold (Policy 1)** and an **In-Flight Per-Turn Headroom Guard**, but is **NOT a Continuously Protected Global Floor (Policy 2)**. The effective arm ceiling clamps against remaining global tokens ($E_{\text{arm}} = \min(A_{\text{ceiling}}, G_{\text{remaining}})$) without deducting the 45,000-token reserve. Consequently, if the final arm is dispatched when $G_{\text{remaining}} = 45,000$, it is permitted to consume the entirety of those 45,000 tokens, reducing the remaining global budget to zero upon successful completion.

Crucially, **Policy 3 (Absolute Global Ceiling)** is strictly enforced: under no circumstances can aggregate spend exceed the configured global ceiling ($2,200,000$ tokens) during normal execution.

---

## 2. Implementation Code References

### 2.1 Orchestrator Level (`orchestrator.py`)
- **Global Budget Initialization & Pre-Dispatch Gate (`orchestrator.py` Lines 2095–2113):**
  ```python
  if self.global_tokens_ceiling is not None:
      global_remaining = self.global_tokens_ceiling - cumulative_global_tokens
      if global_remaining < self.headroom_reserve:
          print(
              f"\n[ORCHESTRATOR GLOBAL-BUDGET HALT] Global remaining tokens ({global_remaining:,}) "
              f"below headroom reserve ({self.headroom_reserve:,}). Halting tetrad."
          )
          block_status = "ABORTED_GLOBAL_BUDGET"
          ...
          break
  ```
  *Finding:* Any arm attempting to dispatch when $G_{\text{remaining}} < 45,000$ is halted immediately.

- **Effective Arm Ceiling Calculation (`orchestrator.py` Line 2115):**
  ```python
  effective_arm_ceiling = min(self.max_tokens_ceiling, global_remaining)
  ```
  *Finding:* The global reserve `self.headroom_reserve` (45,000) is **not** subtracted from `global_remaining`. The effective ceiling provides access to all remaining global tokens up to `self.max_tokens_ceiling`.

- **Arm Ceiling Application (`orchestrator.py` Lines 1436–1439):**
  ```python
  orig_runner_ceiling = getattr(self.runner, "max_tokens_ceiling", self.max_tokens_ceiling)
  if arm_max_tokens_ceiling is not None:
      self.runner.max_tokens_ceiling = arm_max_tokens_ceiling
  ```
  *Finding:* The runner receives `effective_arm_ceiling` as its scalar `max_tokens_ceiling`. The runner has no visibility into `global_tokens_ceiling` or `cumulative_global_tokens`.

- **Over-Ceiling Safety Detection (`orchestrator.py` Lines 2136–2142):**
  ```python
  if effective_arm_ceiling is not None and tel_valid and tel_data:
      raw_spent = tel_data.get("total_model_tokens", 0)
      if raw_spent > effective_arm_ceiling and arm_status == "SUCCESS":
          raise SafetyViolationError(
              f"Arm '{arm}' reported consumption ({raw_spent:,}) exceeding effective ceiling "
              f"({effective_arm_ceiling:,}) while reporting execution_status SUCCESS."
          )
  ```
  *Finding:* Post-execution, the orchestrator verifies that no arm reporting `SUCCESS` exceeded its clamped effective ceiling.

### 2.2 Runner Level (`runner.py`)
- **Token Tracker Initialization (`runner.py` Line 946):**
  ```python
  token_tracker = StreamingTokenTracker(conservative_reserve=self.headroom_reserve)
  ```
- **Pre-Dispatch Turn Headroom Check (`runner.py` Lines 414–462):**
  ```python
  remaining = ceiling - self.effective_tokens
  estimated = self.estimate_next_request_tokens()
  has_headroom = (remaining >= estimated)
  ```
  *Finding:* On every turn boundary within an arm, the runner verifies that remaining tokens within the arm's ceiling equal or exceed $\max(T_{\text{next}}, 45000)$.

---

## 3. Classification of the Three Policies

1. **Policy 1: Global Pre-Dispatch Threshold (`IMPLEMENTED`)**  
   No arm begins execution if $G_{\text{remaining}} < 45,000$. Enforced at `orchestrator.py` line 2098.
2. **Policy 2: Continuously Protected Global Reserve (`NOT IMPLEMENTED`)**  
   The system does not withhold 45,000 tokens from being spent. The effective arm ceiling does not deduct 45,000. An arm dispatched at $G_{\text{remaining}} = 45,000$ can spend all 45,000 tokens.
3. **Policy 3: Absolute Global Ceiling (`IMPLEMENTED`)**  
   The harness strictly prevents planned or accepted aggregate spend from exceeding $2,200,000$ tokens across the four arms.

---

## 4. Numerical Scenarios Analysis

| Scenario | Initial Global Remaining | Per-Arm Ceiling | Can Start? | Effective Arm Ceiling | Max Possible Consumption | Min Remaining Global Tokens | Reserve Margin Guaranteed? |
|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| **A** | 550,000 | 550,000 | **YES** | 550,000 | 550,000 | 0 | **NO** (0 < 45k) |
| **B** | 100,000 | 550,000 | **YES** | 100,000 | 100,000 | 0 | **NO** (0 < 45k) |
| **C** | 45,000 | 550,000 | **YES** | 45,000 | 45,000 | 0 | **NO** (0 < 45k) |
| **D** | 44,999 | 550,000 | **NO** | N/A (Halt) | 0 | 44,999 | N/A (Did not start) |
| **E** | 50,000 | 30,000 | **YES** | 30,000 | 30,000 | 20,000 | **NO** (20k < 45k) |
| **F** | 80,000 | 80,000 | **YES** | 80,000 | 80,000 | 0 | **NO** (0 < 45k) |

---

## 5. Final-Arm Execution Deep Dive

Consider a tetrad execution where Arms 1, 2, and 3 consumed an aggregate of $2,155,000$ tokens:
1. Arm 4 pre-dispatch check evaluates:  
   $$G_{\text{remaining}} = 2,200,000 - 2,155,000 = 45,000$$
2. Condition `global_remaining < 45,000` evaluates to `False` ($45,000 \ge 45,000$). Arm 4 is **cleared for dispatch**.
3. Effective ceiling calculated:  
   $$E_{\text{arm}} = \min(550,000, 45,000) = 45,000$$
4. Runner initializes with `max_tokens_ceiling = 45,000`.
5. Turn 1 evaluation in `StreamingTokenTracker`:  
   $$\text{remaining} = 45,000 - 0 = 45,000$$  
   $$\text{estimated} = \max(0, 45,000) = 45,000$$  
   Condition `remaining >= estimated` ($45,000 \ge 45,000$) evaluates to `True`. **Turn 1 is dispatched**.
6. If Turn 1 consumes 45,000 tokens and solves the task, Codex reports `SUCCESS`.
7. Post-run orchestrator validation:  
   `raw_spent (45,000) > effective_arm_ceiling (45,000)` is `False`. Run is accepted.
8. Cumulative global spend updates:  
   $$\text{cumulative\_global\_tokens} = 2,155,000 + 45,000 = 2,200,000$$  
   $$G_{\text{remaining}} = 0$$

**Definitive Conclusion:** The 45,000-token headroom reserve is **not** a protected reserve. It is an arm-dispatch eligibility threshold that guarantees sufficient tokens to execute at least one turn, but permits full exhaustion of the budget down to zero.

---

## 6. Maximum Exposure Determination

| Metric | Verified Value | Justification |
|---|---:|---|
| **Absolute Global Ceiling** | `2,200,000` | Configured CLI/orchestrator limit (`--global-tokens-ceiling`) |
| **Maximum Permitted Aggregate Spend** | `2,200,000` | Clamped effective ceilings enforce sum $\le 2,200,000$ |
| **Minimum Guaranteed Remaining Tokens** | `0` | Any arm starting with $\ge 45,000$ can spend down to 0 |
| **Maximum Spend Preserving 45,000 Reserve** | `2,155,000` | $2,200,000 - 45,000$ |
| **Max Cumulative Spend Before Final Arm Dispatch** | `2,155,000` | Spend of 2,155,001 halts tetrad ($G_{\text{remaining}} = 44,999 < 45,000$) |
| **Maximum Possible Spend by Final Arm** | `550,000` | $\min(550,000, G_{\text{remaining}})$ |
| **Concurrency / Streaming Overshoot Exposure** | `< 1,000` | Single in-flight LLM stream chunk prior to child process kill |

---

## 7. Recommended Policy Decision

**Adopt Option A (Clarified Pre-Dispatch Eligibility Threshold).**

### Justification:
- **Fairness & Utility:** Withholding 45,000 tokens as an unspendable reserve (Option B) artificially lowers the global budget to $2,155,000$ and causes valid final arms to be starved of tokens when they only need a fraction of the remaining budget to succeed.
- **Predictability:** Total expenditure cannot exceed $2,200,000$. If a strict ceiling of $2,155,000$ is desired, the operator should simply set `--global-tokens-ceiling 2155000`.
- **Terminology Alignment:** The codebase and manifests should refer to $H_{\text{reserve}}$ as the `pre_dispatch_headroom_threshold` to prevent confusion.
