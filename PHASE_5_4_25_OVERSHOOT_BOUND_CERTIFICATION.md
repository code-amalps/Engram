# Engram Phase 5.4.25: Overshoot Bound Certification

**Phase:** 5.4.25  
**Execution Mode:** STRICTLY OFFLINE, READ-ONLY FORENSIC AUDIT  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Previous Phase:** 5.4.24  
**Previous Audit Verdict:** `AUDIT_DETECT_ONLY`  
**Live Benchmark Authorization:** `NOT_AUTHORIZED`

---

## 1. Objective and Scope

In Phase 5.4.24, an analytical formula was introduced to quantify the maximum token overshoot that could occur on a benchmark arm before process termination:
$$\Omega_{\max} = P_{\max} + C_{\max} - R_{\min} = 60,000 + 16,384 - 45,000 = 31,384 \text{ tokens}$$

The mandate of Phase 5.4.25 is to rigorously audit this formula against production execution code, evaluate whether it constitutes a mathematical guarantee, evaluate worst-case scenario boundaries, formally withdraw any unqualified absolute claims, and certify a defensible bound classification.

---

## 2. Evaluation of Theoretical Formula

### 2.1 Parameter Decomposition

The formula decomposes the overshoot into three variables:
1. $P_{\max}$: The maximum prompt/input tokens submitted in a single turn request.
2. $C_{\max}$: The maximum completion/reasoning tokens generated in a single turn.
3. $R_{\min}$: The minimum token headroom reserve required before dispatching any subsequent turn ($R_{\min} = 45,000$).

### 2.2 Mathematical Mechanics
When an arm approaches its ceiling:
- The last permitted turn is dispatched when remaining headroom $G_{\text{remaining}} \ge R_{\min} = 45,000$.
- In the worst case, $G_{\text{remaining}}$ is exactly equal to $R_{\min} = 45,000$ (e.g. 45,001 tokens remaining).
- The dispatched turn then consumes $T_{\text{turn}} = P + C$ tokens.
- The resulting overshoot beyond the ceiling is:
$$\Omega = T_{\text{turn}} - G_{\text{remaining}} \le P + C - R_{\min}$$

If $P \le 60,000$ and $C \le 16,384$, then $\Omega \le 60,000 + 16,384 - 45,000 = 31,384$ tokens.

---

## 3. Worst-Case Parameter Scenarios

As established in `PHASE_5_4_25_PROMPT_CONTEXT_AND_COMPLETION_LIMIT_AUDIT.md`, neither $P_{\max}$ nor $C_{\max}$ is enforced by code or CLI flags; they are observed historical values. The table below illustrates the overshoot under varying operational conditions:

| Scenario ID | Scenario Description | Prompt Tokens ($P$) | Completion Tokens ($C$) | Reserve ($R_{\min}$) | Maximum Overshoot ($\Omega_{\max}$) | Bound Status |
|---|---|---|---|---|---|---|
| **S01** | Nominal Historical (Phase 5.4.24 Baseline) | 60,000 | 16,384 | 45,000 | **31,384** | Conditional |
| **S02** | Deep Repository Exploration (Expanded Context) | 100,000 | 16,384 | 45,000 | **71,384** | Empirical |
| **S03** | Heavy Reasoning Output (Extended CoT) | 60,000 | 32,768 | 45,000 | **47,768** | Empirical |
| **S04** | Large Context + Extended CoT Combination | 100,000 | 32,768 | 45,000 | **87,768** | Empirical |
| **S05** | Stress Task B Multi-File Ingestion | 120,000 | 32,768 | 45,000 | **107,768** | Stress Boundary |
| **S06** | Full Architecture Context Limit (`gpt-5.6-luna`) | 200,000 | 65,536 | 45,000 | **220,536** | Architectural Max |
| **S07** | Zero-Reserve Pre-Dispatch (Defect State $R=0$) | 60,000 | 16,384 | 0 | **76,384** | Unprotected |

### Key Observation
In all scenarios where $R_{\min} = 45,000$ is active, the pre-dispatch reserve successfully absorbs the first 45,000 tokens of the terminal turn's consumption, reducing total overshoot by exactly $R_{\min}$.

However, because $P$ and $C$ can expand up to model architectural limits, claiming an absolute 31,384-token bound across all possible prompts is invalid without qualifying the conditions.

---

## 4. Formal Withdrawal of Unqualified Claims

### Statement of Retraction
> **FORMAL RETRACTION:**  
> The Phase 5.4.24 characterization of $\Omega_{\max} = 31,384 \text{ tokens}$ as an unconditional or mathematically proven global maximum upper bound is hereby **RETRACTED**.  
>  
> Forensic source code audit confirms that $P_{\max} = 60,000$ and $C_{\max} = 16,384$ are observed empirical quantities from prior benchmark runs rather than enforced software constraints in `runner.py`, `orchestrator.py`, or the Codex CLI.  
>  
> Henceforth, $\Omega_{\max} = 31,384 \text{ tokens}$ is certified strictly as a **CONDITIONAL UPPER BOUND**, subject to explicit preconditions.

---

## 5. Certified Bound Classification and Preconditions

### 5.1 Bound Classification
The overshoot bound is certified under the following classification:
$$\mathbf{AUDIT\_CONDITIONAL\_BOUND}$$

### 5.2 Preconditions for the 31,384-Token Bound
The conditional bound $\Omega_{\max} \le 31,384 \text{ tokens}$ holds if and only if the following preconditions are met:
1. **Precondition 1 (Prompt Context Clamping):** Single-turn input context does not exceed $P \le 60,000$ tokens (empirically validated for Task B standard fixtures).
2. **Precondition 2 (Completion Generation Clamping):** Single-turn completion tokens do not exceed $C \le 16,384$ tokens (the nominal completion ceiling for non-extended generation).
3. **Precondition 3 (Headroom Enforcement Active):** `StreamingTokenTracker.check_pre_dispatch_headroom()` enforces $R_{\min} \ge 45,000$ prior to dispatching each turn.
4. **Precondition 4 (Single-Arm Non-Compounding):** Downstream arms are skipped upon detection of a ceiling breach (`SafetyViolationError` / `TOKEN_CEILING_BREACHED`).

---

## 6. Non-Compounding Guarantee Certification

### 6.1 Multi-Arm Compounding Analysis
A critical safety concern is whether an overshoot on Arm 1 could cascade into Arms 2, 3, or 4, compounding the total benchmark token excess:
$$\Omega_{\text{total}} \stackrel{?}{=} \sum_{i=1}^{4} \Omega_i$$

### 6.2 Code-Level Non-Compounding Verification
Forensic inspection of `benchmarks/matched-pair-harness/src/orchestrator.py` verifies:
1. In `run_arm()`, if `effective_tokens > effective_arm_ceiling`, the execution status is forced to:
   ```python
   arm_result["execution_status"] = "TOKEN_CEILING_BREACHED"
   ```
2. In `run_tetrad()` lines 2256–2265:
   ```python
   if arm_result.get("execution_status") == "TOKEN_CEILING_BREACHED":
       raise SafetyViolationError(
           f"Arm {arm} breached token ceiling: "
           f"effective_tokens={arm_effective_tokens} > ceiling={effective_arm_ceiling}"
       )
   ```
3. Raising `SafetyViolationError` terminates the execution loop immediately:
   ```python
   except SafetyViolationError as sve:
       tetrad_result["execution_status"] = "ABORTED_SAFETY_FAILURE"
       # Downstream arms in order are marked SKIPPED_SAFETY_FAILURE
   ```
4. Even if `--no-stop-on-failure` is active, `SafetyViolationError` halts the loop unconditionally.

### 6.3 Certified Non-Compounding Bound
Because zero downstream arms can be dispatched following a ceiling breach:
$$\Omega_{\text{tetrad}} \equiv \Omega_{\text{breach\_arm}}$$
The total benchmark tetrad overshoot is strictly bounded by the single breached arm's overshoot. Compounding across arms is **IMPOSSIBLE** in the production execution path.

---

## 7. Final Summary Table

| Metric | Certified Value | Nature of Guarantee |
|---|---|---|
| Nominal Overshoot Bound | **31,384 tokens** | `CONDITIONAL_UPPER_BOUND` ($P \le 60k, C \le 16,384$) |
| Maximum Empirical Task B Overshoot | **20,000 tokens** | Demonstrated in Phase 5.4.22 Scenario 13 |
| Worst-Case Extended Overshoot | **87,768 tokens** | Theoretical ($P = 100k, C = 32,768$) |
| Multi-Arm Compounding Multiplier | **1.0x (Zero Compounding)** | `PROVEN_HARD_GUARANTEE` (Immediate Tetrad Abort) |
| Post-Breach Downstream Arms Dispatched | **0 arms** | `PROVEN_HARD_GUARANTEE` (`SKIPPED_SAFETY_FAILURE`) |
| Acceptance Gate on Breached Run | **REJECTED** | `PROVEN_HARD_GUARANTEE` (Criterion 7) |
| Phase 5.4.25 Verdict | **`AUDIT_CONDITIONAL_BOUND`** | Certified Forensic Verdict |
