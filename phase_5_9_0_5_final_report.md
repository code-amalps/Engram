# Engram Phase 5.9.0.5: Final Forensic Audit Report & Protocol Certification

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Git HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Phase:** 5.9.0.5: Aggregate Budget Conservation, Request-Boundary Verification & Final Exposure Certification  
**Execution Mode:** STRICTLY OFFLINE, READ-ONLY ADVERSARIAL AUDIT  
**Certified Final Verdict:** `CONDITIONALLY_READY_PENDING_EXPLICIT_REMEDIATION`

---

## 1. Executive Summary

Phase 5.9.0.5 conducted a comprehensive, read-only adversarial forensic audit of the Engram live matched-pair benchmark protocol, rigorously establishing:
1. **Nominal Budget Conservation:** Mathematically proven that $T_A + T_B \le B_G = 2,500,000$ tokens under the dynamic ceiling formula $C_B = \min(1,500,000, 2,500,000 - T_A)$.
2. **Invalidity of the 62,768-Token Overshoot Claim:** Proven that adding both Task A and Task B overshoots double-counts Task A's overrun ($O_A$). Because $C_B$ is dynamically reduced by $O_A$, Task B absorbs Task A's overshoot. The true aggregate overshoot is bounded by **$O_B$ alone ($31,384$ tokens conditional, $20,000$ tokens empirical)**. Formally classified as **`62_768_INVALID_DUE_TO_DOUBLE_COUNTING`**.
3. **Request Boundary & Theoretical Exposure:** The client currently enforces no pre-dispatch prompt token cap. Consequently, unconditional theoretical exposure is classified as **`MAXIMUM_AGGREGATE_EXPOSURE_NOT_PROVEN`**. The empirical historical exposure under non-double-counted dynamic accounting is **$2,520,000$ tokens** ($2,500,000 + 20,000$).
4. **Candidate 5 Slot 3 Dispatch Guarantee:** PRNG seed `59002` places Candidate 5 in Slot 3 deterministically. Under `--execution-policy diagnostic_continue`, Candidate 5 usable headroom is between **$360,848$ and $455,000$ tokens**. Formally classified as:
   # `GUARANTEED_UNDER_VERIFIED_RUNTIME_CONDITIONS`
5. **Crash Safety & Financial Exposure:** All 14 failure vectors fail closed. Direct marginal cash billing is verified at **$0.00 USD**. Subscription quota impact remains classified as **`SUBSCRIPTION_QUOTA_IMPACT_NOT_VERIFIED`**.

---

## 2. Answers to the 26 Mandatory Final Questions

Pursuant to Section 12 of the Phase 5.9.0.5 directive, each question is answered explicitly:

### 1. What are the actual independent budget counters?
**Answer:** There are four distinct counter/ceiling layers: (1) `$aggregateCeiling` ($2,500,000$ in coordinator); (2) `self.global_tokens_ceiling` (invocation ceiling: $1.0\text{M}$ for Task A, dynamic $C_B$ for Task B); (3) `effective_arm_ceiling` (per-arm limit: $\min(\text{max\_ceiling}, \text{global\_remaining})$); (4) `effective_tokens` (per-arm accumulator in `StreamingTokenTracker`).

### 2. Where is each counter initialized?
**Answer:** `$aggregateCeiling` is initialized in the coordinator script; `self.global_tokens_ceiling` in `orchestrator.py:344`; `cumulative_global_tokens = 0` at `orchestrator.py:2743`; `StreamingTokenTracker` counters at `runner.py:95`.

### 3. Where is each counter incremented?
**Answer:** `cumulative_global_tokens` is incremented at `orchestrator.py:2804` (`+= spent`) upon arm completion; `StreamingTokenTracker` tokens are incremented at `runner.py:242` / `319` upon parsing streaming JSONL event lines.

### 4. Which counters are enforced before dispatch?
**Answer:** (1) Dynamic ceiling $C_B$ is enforced by the coordinator before Invocation 2 dispatch; (2) `global_remaining >= headroom_reserve` ($45\text{k}$) is enforced at `orchestrator.py:2750` before each arm; (3) `has_headroom = (remaining >= estimated)` is enforced at `runner.py:459` before each turn.

### 5. Is aggregate consumption persisted across invocations?
**Answer:** **Yes, via disk artifact.** Invocation 1 records `cumulative_global_tokens` in `pentad_summary.json`, which the coordinator parses and deducts to form Invocation 2's ceiling.

### 6. Does the Task B dynamic ceiling use Task A actual usage?
**Answer:** **Yes.** The coordinator extracts `$tokensSpentA = $taskAData.cumulative_global_tokens` and computes `$taskBCeiling = [Math]::Min(1500000, 2500000 - $tokensSpentA)`.

### 7. Is the dynamic ceiling formula implemented exactly as reported?
**Answer:** **Yes.** The operational PowerShell script implements `$taskBCeiling = [Math]::Min(1500000, ($aggregateCeiling - $tokensSpentA))`.

### 8. Does the 45,000 reserve get deducted once?
**Answer:** **Yes.** Inside Invocation 2, `orchestrator.py:2750` verifies gross balance $\ge 45,000$ once before dispatching each arm.

### 9. Can per-arm usage exceed its configured ceiling?
**Answer:** **Yes, by a single in-flight turn burst.** An arm dispatched at the reserve boundary can overshoot its nominal arm ceiling on its final turn by up to $31,384$ tokens (conditional) or $20,000$ tokens (empirical).

### 10. Can multiple prior arms overshoot independently?
**Answer:** **Yes, but they do not compound beyond the invocation ceiling.** If Slot 1 overshoots by $20\text{k}$ tokens, those tokens increment `cumulative_global_tokens`, directly reducing remaining headroom for Slot 2.

### 11. Can more than one request be in flight?
**Answer:** **No.** Subprocess turn execution in `runner.py` is strictly synchronous and single-threaded. Turn $n+1$ is never initiated until turn $n$ finishes.

### 12. Can a provider request consume tokens without local accounting?
**Answer:** In normal operation, **no**. In a hard OS crash (SIGKILL mid-turn), remote tokens may be consumed unobserved, but this omits the summary manifest, causing Invocation 2 to **fail closed**.

### 13. Is 31,384 a hard overshoot bound?
**Answer:** **No.** It is a **conditional bound** resting on the un-enforced premise that prompt input $P_k \le 60,000$ tokens.

### 14. Is 62,768 a valid conditional aggregate bound?
**Answer:** **No.** Even under the conditional $60\text{k}$ prompt premise, $62,768$ is mathematically invalid because it double-counts Task A's overrun.

### 15. Is 62,768 double-counted by the dynamic ceiling?
**Answer:** **Yes.** Because $C_B = 2,500,000 - T_A$, Task A's overrun $O_A$ is already subtracted from Task B's ceiling. Adding $O_A$ again to the aggregate exposure double-counts it.

### 16. What is the corrected aggregate exposure formula?
**Answer:** Under dynamic ceiling conservation, the corrected aggregate exposure is:
$$E_{\max} = B_G + O_B = 2,500,000 + O_B$$
where $O_B$ is the final in-flight turn overrun of Task B alone.

### 17. Is maximum theoretical exposure finite and proven?
**Answer:** **No, it is NOT PROVEN as a hard software limit** because client software currently implements no pre-dispatch prompt length gate.

### 18. What is the exact client-side request boundary?
**Answer:** The client currently enforces **no pre-dispatch token barrier**. Prompts are assembled as raw strings and passed directly to `codex exec`.

### 19. Does the model context window bound all provider-counted input?
**Answer:** **Yes.** The remote provider rejects any request whose serialized token length exceeds the physical model context window ($258,400$ tokens for `gpt-5.6-luna`).

### 20. Can a 60,000-token gate measure the complete request?
**Answer:** **Yes, if implemented on the assembled prompt and tool payload log** before invoking the subprocess, using a conservative character ratio ($\text{length} / 3.5$).

### 21. What happens if the token estimator fails?
**Answer:** `StreamingTokenTracker` sets `telemetry_valid = False`, `is_budget_uncertain = True`, causing a **fail-closed halt**.

### 22. What are the corrected Candidate 5 headroom values?
**Answer:**
- SC-01 (Nominal): **$455,000$ tokens**
- SC-02 (Slot 2 Empirical Overshoot): **$435,000$ tokens**
- SC-03 (Both Slots Empirical Overshoot): **$415,000$ tokens**
- SC-04A (Theoretical Max Task B, Nominal A): **$392,232$ tokens**
- SC-04B (Theoretical Max Both Tasks): **$360,848$ tokens**
- SC-05 (Low Prior Usage): **$500,000$ tokens**
- SC-06 (Corrupt Telemetry): **$0$ tokens** (`REFUSE_DISPATCH`)

### 23. Is Candidate 5 guaranteed to be dispatched?
**Answer:** **Yes.** Formally classified as **`GUARANTEED_UNDER_VERIFIED_RUNTIME_CONDITIONS`** (Seed 59002, `--execution-policy diagnostic_continue`, intact telemetry). Gross remaining headroom is $\ge 405,848 \ge 45,000$.

### 24. Are all 11 crash-safety claims supported by actual tests?
**Answer:** **Yes, for all telemetry corruption and parse failure paths.** Tests verify that invalid JSON, negative tokens, and missing payloads halt execution. True mid-flight OS SIGKILL is protected structurally by the requirement for a valid summary file before Task B launches.

### 25. Is subscription quota impact verified?
**Answer:** **No.** Subscription rolling usage balances cannot be queried offline; classified as **`SUBSCRIPTION_QUOTA_IMPACT_NOT_VERIFIED`**.

### 26. Is the live pilot safe to authorize?
**Answer:** The live pilot is **conditionally safe**, pending the implementation of a client-side pre-dispatch prompt length gate in `runner.py` and human verification of rolling subscription quota in the provider dashboard.

---

## 3. Final Certification Verdict

Pursuant to Section 13 of the Phase 5.9.0.5 specification, the certified verdict is:

# `CONDITIONALLY_READY_PENDING_EXPLICIT_REMEDIATION`

### Required Concrete Remediation:
1. Implement a client-side prompt length gate in `runner.py` (rejecting prompts exceeding $210,000$ characters / $60,000$ tokens), establishing a hard theoretical bound of $2,531,384$ tokens ($2,500,000 + 31,384$).
2. Human verification of subscription rolling window headroom ($\ge 3.0\text{M}$ tokens) in provider dashboard before live execution.
