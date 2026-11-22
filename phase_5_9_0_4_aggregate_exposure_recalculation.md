# Engram Phase 5.9.0.4: Aggregate Token Exposure Recalculation

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Phase:** 5.9.0.4 Workstream C  
**Execution Mode:** Strictly Offline, Read-Only Adversarial Audit  
**Prior Theoretical Exposure Claim:** $2,562,768$ tokens ($2,500,000 + 62,768$)  
**Re-Audit Classification:** `MAXIMUM_AGGREGATE_EXPOSURE_NOT_PROVEN` (Theoretical Hard Bound)  
**Empirical Historical Exposure:** $2,540,000$ tokens ($2,500,000 + 40,000$)

---

## 1. Purpose

This deliverable provides the final mathematical and architectural recalculation of aggregate token exposure across the two-invocation five-arm matched-pair benchmark pilot (Task A + Task B). It incorporates the evidentiary findings of Workstream A (prompt input bound audit) and Workstream B (dynamic ceiling reconciliation) to establish the true defensible exposure model under the current software implementation.

---

## 2. Re-Assessment of the Theoretical 62,768-Token Overshoot Model

In Phase 5.9.0.3, the theoretical maximum aggregate exposure was derived as:
$$T_{\text{maximum, theoretical}} = 2,500,000 + 62,768 = 2,562,768 \text{ tokens}$$
This formula rested upon the assertion that single-turn overshoot for each of the two invocations is bounded to:
$$\Omega_{\text{turn}} = P_{\max} + C_{\max} - R_{\min} = 60,000 + 16,384 - 45,000 = 31,384 \text{ tokens}$$
and that two independent invocations could overshoot by at most $31,384 \times 2 = 62,768$ tokens.

### 2.1 The Fatal Defect in the Theoretical Bound
As proven in Workstream A (`phase_5_9_0_4_input_token_hard_limit_audit.md`):
1. **$P_{\max} = 60,000$ is an observed empirical estimate**, NOT an enforced client-side or provider-side software limit.
2. In `task_b_remember_me`, the agent can explore large TypeScript modules or bundles, expanding prompt context ($P_k$) beyond $60,000$ tokens up to the model's architectural window ($258,400$ tokens).
3. If an agent executes a turn at the $45,000$ reserve boundary with $P_k = 100,000$ tokens and generates $C_k = 16,384$ tokens, the single-turn overshoot is:
   $$\Omega_{\text{turn}} = 100,000 + 16,384 - 45,000 = 71,384 \text{ tokens}$$
4. Because the prompt input component is unbounded by the client software, **the $31,384$ single-turn bound and the $62,768$ aggregate overshoot bound cannot be proven as hard mathematical ceilings**.

Pursuant to Section 6 of the Phase 5.9.0.4 directive:
> *"If any term is unbounded or unknown, report: `MAXIMUM_AGGREGATE_EXPOSURE_NOT_PROVEN`"*

We formally report:
# `MAXIMUM_AGGREGATE_EXPOSURE_NOT_PROVEN`
*(as an unconditional theoretical software guarantee)*

---

## 3. Structural Properties That Remain Formally Proven

Although the theoretical upper bound is unbounded due to lack of client-side prompt truncation, the following structural invariants **are fully proven and enforced by code**:

1. **Synchronous Single-Threaded Turn Concurrency:**
   - In `runner.py`, every model request is executed synchronously via `subprocess.Popen` in a serial turn loop.
   - Turn $k+1$ is never initiated until turn $k$ finishes and emits its streaming JSONL events.
   - At no point can multiple concurrent turns be in-flight.
2. **Immediate Cumulative Accounting:**
   - As each event line is parsed by `StreamingTokenTracker.process_line()`, `cumulative_global_tokens` updates immediately.
   - Pre-dispatch headroom checks (`check_pre_dispatch_headroom`) evaluate effective cumulative tokens *prior* to authorizing the subsequent turn.
3. **Non-Compounding Intermediate Arms:**
   - Within an invocation, intermediate arm overshoots increment `cumulative_global_tokens`.
   - This immediately starves subsequent arms, strictly preventing multiple intermediate arms from creating a compounding breach of the invocation ceiling.
   - **Only the final executing arm in each invocation can breach the invocation ceiling.**
4. **Fail-Closed Cross-Process Coordination:**
   - If Invocation 1 crashes mid-turn or emits corrupt telemetry, the 14-point summary validation gate aborts, completely blocking Invocation 2 from launching.

---

## 4. Final Exposure Table

Pursuant to Section 6 of the directive, the complete exposure breakdown is presented below:

| Budget Component | Verified Amount | Evidentiary Basis & Enforcement Mechanism |
|---|---:|---|
| **Task A Nominal Ceiling** | $1,000,000$ tokens | Configured in `task_a_typo.json`; enforced by `orchestrator.py:246` pre-dispatch gate |
| **Task B Nominal Ceiling** | $1,500,000$ tokens | Configured in `task_b_remember_me.json`; enforced by `orchestrator.py:246` pre-dispatch gate |
| **Aggregate Nominal Ceiling** | $2,500,000$ tokens | Coordinator invariant; enforced by `$taskBCeiling = [Math]::Min(...)` |
| **Task A Maximum Overshoot** | **NOT PROVEN** (Theoretical)<br>$20,000$ tokens (Empirical) | Theoretical: Prompt size $P_k$ unbounded in code.<br>Empirical: Max observed single-turn burst in Phase 5.4.24 |
| **Task B Maximum Overshoot** | **NOT PROVEN** (Theoretical)<br>$20,000$ tokens (Empirical) | Theoretical: Prompt size $P_k$ unbounded in code.<br>Empirical: Max observed single-turn burst in Phase 5.4.24 |
| **Total Theoretical Overshoot** | **NOT PROVEN** | Unbounded due to lack of client-side prompt truncation gate |
| **Maximum Theoretical Exposure** | **`MAXIMUM_AGGREGATE_EXPOSURE_NOT_PROVEN`** | Formally reported pursuant to Section 6 directive |
| **Empirical Historical Exposure** | **$2,540,000$ tokens** | $2,500,000 \text{ nominal} + 40,000 \text{ demonstrated overshoot}$ (NOT a guaranteed maximum) |

---

## 5. Explicit Remediation Required to Prove Hard Theoretical Bound

To transform `MAXIMUM_AGGREGATE_EXPOSURE_NOT_PROVEN` into a mathematically proven hard upper bound of $2,562,768$ tokens, the system requires a single explicit software change:
- **Remediation Specification:** Implement an explicit pre-dispatch prompt length check in `benchmarks/matched-pair-harness/src/runner.py`. Before invoking `codex exec` or dispatching a turn payload, calculate prompt tokens (or a conservative character-based heuristic $\text{length} / 3.5$). If estimated prompt tokens exceed $60,000$, reject or truncate the prompt, preventing provider dispatch.
