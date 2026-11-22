# Milestone 5B-Review: Token Accounting Integrity Review

## 1. Executive Summary

This review audits the token extraction, classification, and aggregation logic in `benchmarks/matched-pair-harness/src/runner.py` and `telemetry.py`.

The audit verifies that token accounting is mathematically rigorous, adheres to strict sub-metric classifications, eliminates double-counting, handles edge cases fail-closed, and accurately retains partial and failed runs under Intent-to-Treat (ITT) principles.

---

## 2. Token Extraction and Classification Architecture

The raw JSONL stream emitted by Codex or model runner contains multiple telemetry schemas across turns. The `StreamingTokenTracker` parses and classifies tokens according to the following invariants:

### 2.1 Token Categories & Relationships

```
Total Model Tokens = Input Tokens + Output Tokens
                       │                  │
         ┌─────────────┴─────────────┐    └─────────────┐
         ▼                           ▼                  ▼
Cached Input Tokens       Uncached Input Tokens    Reasoning Tokens
(Informative subset)      (Informative subset)    (Informative subset of Output)
```

1. **Input Tokens:** The complete prompt input.
   - If `cached_input_tokens` is reported, it represents the subset of input tokens retrieved from prompt cache.
   - Invariant: $\text{Cached Input} \le \text{Input Tokens}$.
   - **Crucial Invariant:** Cached input is **NEVER** added on top of total input tokens.
2. **Output / Completion Tokens:** Total generation tokens produced by the model.
   - If `reasoning_tokens` is reported, it represents the subset of completion tokens spent on hidden chain-of-thought.
   - Invariant: $\text{Reasoning Tokens} \le \text{Output Tokens}$.
   - **Crucial Invariant:** Reasoning tokens are **NEVER** added on top of total output tokens.
3. **Total Tokens:**
   $$\text{Total Tokens} = \text{Input Tokens} + \text{Output Tokens}$$

---

## 3. Aggregation Hierarchy

1. **Per-Request Level:**
   - Raw `turn.completed` or `token_usage_record` payloads provide turn-level usage deltas:
     $$\Delta_t = \text{input}_t + \text{output}_t$$
2. **Per-Turn Level:**
   - Incremental turn deltas are tracked in `turn_deltas_sum = \sum_{t \in \text{distinct\_turns}} \Delta_t$.
   - Cumulative thread snapshots update `thread_cumulative_max = \max(\text{thread\_cumulative\_max}, \text{snapshot}_t)$.
3. **Per-Arm Level:**
   - Effective tokens for the entire arm is calculated as:
     $$\text{Arm Total Tokens} = \max(\text{thread\_cumulative\_max}, \text{turn\_deltas\_sum})$$
   - This prevents divergence if one telemetry format drops packets.

---

## 4. Synthetic Stress Testing Matrix

To independently prove robustness against real-world event stream anomalies, synthetic event sequences were tested against `StreamingTokenTracker`:

### Case 1: Duplicate Event Injection
- **Stimulus:** Two identical `turn.completed` events with `turn_id="turn_01"` and `usage={"input_tokens": 1000, "output_tokens": 200}`.
- **Handling:** First event adds 1,200 tokens. Second event detects line hash in `_seen_turn_events` and is discarded.
- **Result:** `turn_deltas_sum = 1200`. Zero inflation.

### Case 2: Delayed Cumulative Snapshot
- **Stimulus:** Turns 1, 2, 3 report incremental deltas totaling 8,000 tokens. At Turn 4, an authoritative `token_usage_record` arrives reporting cumulative 9,500 tokens (accounting for unlogged tool calls).
- **Handling:** `thread_cumulative_max` updates to 9,500. `effective_tokens` evaluates to $\max(8000, 9500) = 9,500$.
- **Result:** Full usage captured despite earlier telemetry lag.

### Case 3: Out-of-Order Snapshot Ingestion
- **Stimulus:** Snapshot A (12,000 tokens) arrives, followed by an older re-flushed Snapshot B (8,000 tokens).
- **Handling:** `thread_cumulative_max = max(12000, 8000) = 12,000`.
- **Result:** Monotonicity preserved.

### Case 4: Negative Token Telemetry
- **Stimulus:** Corrupt event reporting `{"input_tokens": -500, "output_tokens": 100}`.
- **Handling:** `tracker.process_line` detects negative token value, sets `telemetry_valid = False` and `telemetry_status = "INVALID"`.
- **Result:** Fails closed. Prevents corrupt subtraction from the ceiling.

### Case 5: Missing Usage Payload
- **Stimulus:** `{"type": "token_usage_record", "payload": {}}` with missing `thread_token_usage`.
- **Handling:** Sets `telemetry_valid = False` and halts pre-dispatch headroom checks. Missing consumption is NOT treated as zero.
- **Result:** Halts run safely; flags run telemetry as invalid.

---

## 5. Intent-to-Treat (ITT) Reporting Integrity

The harness evaluator strictly adheres to Intent-to-Treat principles:
1. **Partial Runs:** If a run terminates prematurely due to timeout or pre-dispatch headroom exhaustion, all tokens consumed up to termination are captured and recorded in `run_telemetry.json`.
2. **Failed Acceptance:** If an arm fails functional correctness (score $< 1.0$), its token consumption is **retained in the analysis ledger**. It is never scrubbed or discarded.
3. **Savings Invalidation:** Any token reduction claim is strictly invalidated unless **both** arms achieve full functional acceptance ($\text{score} = 1.0$). A low-token failed run cannot claim efficiency.

---

## 6. Audit Verdict

The token accounting engine is mathematically consistent, resilient to streaming anomalies, eliminates double-counting, and preserves statistical integrity under ITT guidelines.
