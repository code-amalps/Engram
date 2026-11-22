# Milestone 5B-Prep: Token Accounting Audit

## 1. Objective and Audit Scope

This audit reviews the real-time token tracking and telemetry reconciliation engine (`StreamingTokenTracker`) in `benchmarks/matched-pair-harness/src/runner.py`.

The audit addresses:
1. Cumulative thread snapshots vs incremental turn delta accounting.
2. Deduplication of streaming JSONL events.
3. Fail-safe handling of malformed or missing telemetry.
4. Token classification (cached input, uncached input, output, and reasoning tokens).

---

## 2. Token Invariants and Mathematical Rules

### Rule 1: Cumulative Snapshot Accounting
Authoritative cumulative thread snapshots are reported via `token_usage_record` (or `event_msg token_count`).
- Snapshots are monotonically tracked:
  $$\text{thread\_cumulative\_max} = \max(\text{thread\_cumulative\_max}, \text{snapshot\_total})$$
- Snapshots from the same thread are NEVER summed together.

### Rule 2: Incremental Turn Delta Accounting
Incremental per-turn consumption is reported via `turn.completed` events.
- Distinct turn deltas are tracked and summed:
  $$\text{turn\_deltas\_sum} = \sum_{t \in \text{distinct\_turns}} \Delta_t$$
- Events with identical `turn_id` or identical line hashes are deduplicated and rejected from secondary summation.

### Rule 3: Reconciled Effective Usage
The effective cumulative usage at any point in time $t$ is calculated as:
$$\text{effective\_tokens}(t) = \max(\text{thread\_cumulative\_max}(t), \text{turn\_deltas\_sum}(t))$$
This dual-track invariant ensures that:
- If `turn.completed` events are missing or delayed, snapshots capture full usage.
- If snapshot events are delayed or sparse, incremental turn deltas maintain real-time ceiling protection.

### Rule 4: Sub-Metric Invariance (Reasoning Tokens)
Reasoning tokens are an internal breakdown of `output_tokens`:
$$\text{total\_tokens} = \text{input\_tokens} + \text{output\_tokens}$$
$$\text{reasoning\_tokens} \le \text{output\_tokens}$$
Reasoning tokens are NEVER added to `total_tokens` as an independent term.

---

## 3. Telemetry Schema Validation

All completed runs generate a structured `run_telemetry.json` adhering to `schemas/run_telemetry.schema.json`.
The schema validates:
- `telemetry_valid`: Boolean flag indicating whether all telemetry records were well-formed.
- `telemetry_status`: `"VALID"`, `"INVALID"`, `"UNKNOWN"`, or `"UNAVAILABLE"`.
- `total_model_tokens`: Positive integer.
- `token_breakdown`: Dict containing `cached_input_tokens`, `uncached_input_tokens`, `output_tokens`, `reasoning_tokens`.
- `peak_turn_context`: Maximum context window exposure observed across all turns.

---

## 4. Empirical Test Verification

The unit test suite in `test_milestone_5b_remediation.py` verified the following audit scenarios:

| Test Scenario | Input / Stimulation | Expected Result | Observed | Status |
|---|---|---|---|---|
| **Deduplication** | Duplicate `turn.completed` event | Sum does not increase | `turn_deltas_sum = 1200` | **PASS** |
| **Negative Token Protection** | `input_tokens = -500` | `telemetry_valid = False` | `telemetry_valid = False` | **PASS** |
| **Malformed Payload** | Missing `thread_token_usage` | `telemetry_valid = False` | `telemetry_valid = False` | **PASS** |
| **Monotonic Snapshots** | Sequence of 500, 1500, 1200 tokens | `thread_cumulative_max = 1500` | `thread_cumulative_max = 1500` | **PASS** |

The accounting engine maintains mathematically sound and fail-safe operation under all tested conditions.
