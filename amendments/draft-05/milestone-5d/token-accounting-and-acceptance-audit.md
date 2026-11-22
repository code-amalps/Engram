# Token Accounting and Acceptance Audit: N=1 Matched-Pair Pilot

**Milestone:** Engram Amendment 05 — Milestone 5D  
**Document ID:** `amendments/draft-05/milestone-5d/token-accounting-and-acceptance-audit.md`  
**Evaluation Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Accounting Engine:** `StreamingTokenTracker` & `orchestrator.py`  
**Status:** AUDITED, VERIFIED & RIGOROUSLY QUALIFIED  

---

## 1. Executive Summary

This audit assesses the mathematical integrity, telemetry parsing, and acceptance gating mechanics governing token accounting in the matched-pair harness.

The audit confirms that the harness prevents double-counting of cached input and reasoning tokens, deduplicates streaming events, retains consumed tokens on partial runs, and strictly invalidates comparative savings if telemetry is missing or functional acceptance fails. Furthermore, this report reaffirms that client-side token controls are nominal planning thresholds, not provider-enforced hard limits.

---

## 2. Mathematical Accounting Invariants

In `benchmarks/matched-pair-harness/src/runner.py` (`StreamingTokenTracker`) and `telemetry.py`:

### 2.1 Deduplication of Streaming Events
`StreamingTokenTracker` maintains internal sets of observed event IDs:
- `self._seen_turn_events: Set[str] = set()`
- `self._seen_snapshot_hashes: Set[str] = set()`
If an agent runtime replays an event or emits duplicate `turn.completed` messages, subsequent occurrences are deduplicated and do not inflate cumulative token metrics.

### 2.2 Cached Input Accounting (Zero Double-Counting)
Prompt tokens are partitioned into cached and uncached components:
$$\text{prompt\_tokens\_total} = \text{prompt\_tokens\_cached} + \text{prompt\_tokens\_uncached}$$
$$\text{total\_model\_tokens} = \text{prompt\_tokens\_total} + \text{completion\_tokens}$$
Cached tokens are an informative sub-metric of total prompt tokens. They are **never added twice** into total model tokens.

### 2.3 Reasoning Tokens Accounting (Zero Double-Counting)
For models utilizing internal reasoning (such as `gpt-5.6-luna`), reasoning tokens are reported as a component of output generation:
$$\text{completion\_tokens} = \text{reasoning\_tokens} + \text{standard\_completion\_tokens}$$
Reasoning tokens are treated as an informative breakdown and are **never summed on top of** completion tokens.

---

## 3. Telemetry and Acceptance Suppression Invariants

### 3.1 Missing or Corrupt Telemetry Suppression
In `orchestrator.py` (`validate_telemetry_integrity`):
- If telemetry is empty, unparseable, reports negative values, or shows zero total tokens on a completed run, `telemetry_valid` is set to `False`.
- In `compute_comparative_analysis`, if `not (ctrl_tel_valid and treat_tel_valid)`, the verdict is set to `MUTUAL_TELEMETRY_INVALID`, `CONTROL_TELEMETRY_INVALID`, or `TREATMENT_TELEMETRY_INVALID`.
- Comparative savings claims are strictly suppressed (`savings_claim_eligible: False`), even if both arms achieved $C = 1.00$.

### 3.2 Acceptance Failure Suppression
- If either arm fails any requirement ($C < 1.00$ or `passed_all_requirements == False`), comparative savings are strictly suppressed (`MUTUAL_ACCEPTANCE_FAILURE`, `CONTROL_FAILED_ACCEPTANCE`, or `TREATMENT_FAILED_ACCEPTANCE`).
- A defective or incomplete implementation cannot generate valid efficiency savings.

### 3.3 Partial Run Token Retention
- If an arm times out, encounters a fatal exception, or halts under the pre-dispatch headroom guard, `StreamingTokenTracker` retains the exact cumulative token count consumed up to the moment of termination.
- All consumed tokens are written to `events.jsonl` and `run_telemetry.json`.
- Stop-on-failure records preserve the partial run's token cost while marking downstream arms as skipped.

---

## 4. Technical Qualification: Enforceability of the 175,000 Nominal Budget

> [!CAUTION]
> The configured per-arm budget of $175,000$ tokens is a **nominal planning ceiling**, NOT a provider-enforced hard limit.

The following operational realities are explicitly reaffirmed:
1. **In-Flight Turn Exposure:** The pre-dispatch guard stops turns when remaining headroom is $\le 45,000$ tokens. However, a dispatched turn can consume up to $47,000$ tokens in flight. In a worst-case burst, a completed run can finish at $176,000$–$180,000$ tokens before local process termination occurs.
2. **Local Kill vs. Server Cancellation:** Terminating the local Codex process severs the client socket, but does not guarantee instantaneous cancellation of remote GPU inference on OpenAI infrastructure.
3. **Subscription Rate Limits:** The environment operates under ChatGPT Subscription OAuth (`auth_mode = "chatgpt"`). There is no metered API billing cap to cut off requests at exactly $175,000$ tokens.
4. **Non-Enforceability Disclosure:** Neither Antigravity nor the local harness claims that total usage will never exceed $175,000$ tokens.
