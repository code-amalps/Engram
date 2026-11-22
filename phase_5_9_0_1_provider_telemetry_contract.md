# Engram Phase 5.9.0.1: Provider & Token Telemetry Contract

**Phase:** 5.9.0.1  
**Integration Target:** Codex CLI Headless Agent (`codex.exe`) & OpenAI Cloud Provider  
**Implementation Files:** `benchmarks/matched-pair-harness/src/runner.py`, `telemetry.py`  
**Execution Mode:** Strictly Offline Specification & Telemetry Contract Audit  

---

## 1. Provider & Agent Configuration Identity

| Parameter | Authoritative Value | Verification Evidence |
|---|---|---|
| **Execution Provider** | `cloud` (OpenAI via Headless Codex CLI) | `runner.py:SUPPORTED_PROVIDERS` |
| **Model Identifier** | `gpt-5.6-luna` | CLI argument `--model gpt-5.6-luna` |
| **Reasoning Effort** | `medium` | Passed as `-c collaboration_mode.settings.reasoning_effort="medium"` |
| **Codex CLI Executable** | `C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe` | Verified present |
| **Codex Version** | `codex-cli 0.167.0-alpha.4.1` | Query output via `codex.exe --version` |
| **Execution Policy** | `diagnostic_continue` | Continues subsequent arms on failure for complete comparison |
| **Sandbox Policy** | `workspace-write` | Enforces write isolation to sandbox copy |
| **Model Selection Guarantee** | **Guaranteed by Codex CLI session** | Fails closed if model is unavailable or unrecognized |

---

## 2. Token Accounting Architecture & Telemetry Invariants

The harness extracts token usage directly from the Codex event stream (`turn.completed` and `token_usage_record`).

### 2.1 Authoritative Usage Fields
Codex streams JSONL records with the following usage schema:
```json
{
  "type": "token_usage_record",
  "payload": {
    "thread_token_usage": {
      "input_tokens": 16815,
      "cached_input_tokens": 11136,
      "cache_write_input_tokens": 0,
      "output_tokens": 420,
      "reasoning_output_tokens": 128,
      "total_tokens": 17235
    }
  }
}
```

### 2.2 Canonical Token Metric Definition
The authoritative total token metric ($T_{\text{total}}$) is defined as:
$$T_{\text{total}} = \text{input\_tokens} + \text{output\_tokens}$$

To eliminate double-counting:
1. **Cached Input Tokens ($T_{\text{cached}}$):** `cached_input_tokens` is a **sub-metric** of `input_tokens` (representing prompt tokens served from KV cache at reduced rate). It is **NEVER** added to `total_tokens`.
   $$\text{Uncached Input Tokens} = \text{input\_tokens} - \text{cached\_input\_tokens}$$
2. **Reasoning Output Tokens ($T_{\text{reasoning}}$):** `reasoning_output_tokens` is a **sub-metric** of `output_tokens` (representing internal chain-of-thought tokens). It is **NEVER** added on top of `output_tokens`.
3. **Tool Call Tokens:** Input and output tokens consumed during tool invocation arguments and results are integrated directly into the conversational turns by Codex and accounted for in `input_tokens` of subsequent turns.

---

## 3. Streaming Telemetry Safeguards & Fail-Closed Boundaries

In `StreamingTokenTracker` (`runner.py:120–280`):
1. **Snapshot Deduplication:** Thread cumulative snapshots are tracked via cryptographic SHA-256 hashes. Repeated events cannot inflate token tallies.
2. **Turn Delta Reconciliation:** Effective cumulative usage is:
   $$T_{\text{effective}} = \max(\text{thread\_cumulative\_max}, \text{turn\_deltas\_sum})$$
3. **Negative / Non-Integer Validation:** Any event reporting negative or non-integer tokens immediately marks:
   ```python
   self.telemetry_valid = False
   self.telemetry_status = "INVALID"
   ```
4. **Missing Usage Metadata:** If a `token_usage_record` omits both `total_tokens` and `input_tokens`, telemetry is marked `INVALID`.
5. **Fail-Closed Audit Rule:** In `orchestrator.py`:
   If `telemetry_valid == False`, the arm is flagged as `TELEMETRY_INVALID`.
   > [!CRITICAL]
   > A task run that achieves code changes but reports incomplete or invalid token accounting is **STRICTLY REJECTED** and cannot be counted as a valid performance measurement.
