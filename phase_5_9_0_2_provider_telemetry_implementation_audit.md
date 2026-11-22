# Engram Phase 5.9.0.2: Provider Telemetry Implementation & Parsing Verification Audit

**Phase:** 5.9.0.2  
**Target Files:** `benchmarks/matched-pair-harness/src/runner.py`, `telemetry.py`  
**Execution Mode:** Strictly Offline Source-Code Audit  

---

## 1. Streaming JSONL Telemetry Implementation

The harness ingests real-time streaming events from Codex CLI sessions via `StreamingTokenTracker` (`runner.py:120–280`).

### 1.1 Snapshot Deduplication Proof (No Double-Counting)
Codex periodically emits cumulative thread snapshots (`token_usage_record`).
In `runner.py`:
```python
line_hash = hashlib.sha256(line.strip().encode("utf-8")).hexdigest()
if line_hash in self._seen_snapshot_hashes:
    return self.effective_tokens
self._seen_snapshot_hashes.add(line_hash)

# Cumulative thread snapshots are NEVER summed to each other:
self.thread_cumulative_max = max(self.thread_cumulative_max, tot)
```
- Snapshots monotonically update `thread_cumulative_max`.
- Cryptographic hashing prevents identical event lines from being parsed twice.
- Cumulative snapshots cannot double-count.

### 1.2 Incremental Turn Deltas Deduplication
When individual turn events (`turn.completed`) are parsed:
```python
turn_id = data.get("turn_id") or data.get("id")
if turn_id and str(turn_id) not in self._seen_turn_events:
    self._seen_turn_events.add(str(turn_id))
    self.turn_deltas_sum += delta
```
- Deduplicated by unique `turn_id`.

### 1.3 Effective Cumulative Usage Formula
The canonical usage reported by the tracker is:
$$T_{\text{effective}} = \max(\text{thread\_cumulative\_max}, \text{turn\_deltas\_sum})$$
This dual-ledger architecture guarantees that whichever telemetry stream is more complete or up-to-date governs the budget without summing them together.

---

## 2. Token Field Integrity & Sub-Metric Breakdown

| Usage Field in Codex Stream | Tracker Attribute | Accounting Rule | Double-Counting Guard |
|---|---|---|---|
| `usage.input_tokens` | `prompt_tokens_total` | Full prompt input tokens. | Base metric; included in total. |
| `usage.cached_input_tokens` | `prompt_tokens_cached` | Prompt tokens read from KV cache. | **Sub-metric of input_tokens.** NEVER added to total. |
| `usage.output_tokens` | `completion_tokens` | Full completion output tokens. | Base metric; included in total. |
| `usage.reasoning_output_tokens` | `reasoning_tokens` | Internal model reasoning tokens. | **Sub-metric of output_tokens.** NEVER added to total. |
| `usage.total_tokens` | `thread_cumulative_max` | Authoritative provider total. | Governs if $\ge \text{turn\_deltas\_sum}$. |

---

## 3. Answers to Specific Telemetry Audit Questions

1. **Can the parser double-count cumulative snapshots?**  
   **No.** Cryptographic SHA-256 line deduplication and `max()` updates prevent summing cumulative snapshots.
2. **Does it lose final usage events?**  
   **No.** `runner.py` polls and reads the stream until the child process terminates, followed by a final buffer flush before closing the file handle.
3. **Do failed requests still contribute to consumption?**  
   **Yes.** Any turn that completed before the failure is permanently recorded in `thread_cumulative_max`.
4. **Do retries retain the same arm budget?**  
   **Yes.** Retries occur within the same Codex process session; all consumed tokens accumulate onto the current arm's consumption counter.
5. **Does missing usage metadata cause a fail-closed halt?**  
   **Yes.** If both `total_tokens` and `input_tokens` are missing, or if negative values are encountered, `self.telemetry_valid` is set to `False`.
6. **Can a crashed process leave an unknown usage balance?**  
   If a crash occurs before usage is flushed to disk, `telemetry_valid` is marked `False`.
7. **Can subsequent arms start after incomplete telemetry?**  
   **No.** In `orchestrator.py:2820`, if `is_budget_uncertain` is True, `block_status` is set to `ABORTED_UNCERTAIN_BUDGET` and all remaining arms are **SKIPPED**.
8. **Does process termination reliably prevent additional provider activity?**  
   **Yes.** `terminate_pid_tree` forcefully kills the entire OS process tree using recursive PID termination, instantly severing any pending provider connections.
