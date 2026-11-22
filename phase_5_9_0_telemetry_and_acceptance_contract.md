# Engram Phase 5.9.0: Telemetry Contract & Benchmark Acceptance Specification

**Execution Phase:** 5.9.0  
**Target Schema:** `benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json`  
**Execution Mode:** SPECIFICATION & AUDIT CONTRACT  

---

## 1. Authoritative Telemetry Field Registry

Every task-arm execution must record and validate the following schema-compliant telemetry fields:

| Category | Telemetry Field | Data Type | Source / Provenance |
| :--- | :--- | :---: | :--- |
| **Identity** | `run_id` | `string` | Unique per-run UUID |
| | `pair_id` | `string` | Block/pair identifier |
| | `task_id` | `string` | `"task_a_typo"` or `"task_b_remember_me"` |
| | `arm` | `string` | Authoritative arm identifier from `ALL_PENTAD_ARMS` |
| | `execution_order_in_pair` | `integer` | 1 to 5 |
| **Model & Runtime** | `model.provider` | `string` | `"cloud"` |
| | `model.name` | `string` | `"gpt-5.6-luna"` |
| | `model.reasoning_effort` | `string` | `"medium"` |
| | `model.harness_executable` | `string` | Canonical path to Codex executable |
| | `model.harness_version` | `string` | Version string queried via `--version` |
| **Isolation & Binary** | `isolation.baseline_commit` | `string` | `"1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2"` |
| | `isolation.sandbox_path` | `string` | Dedicated ephemeral directory |
| | `isolation.engram_binary_actual_sha256` | `string` | Bit-for-bit pre-dispatch hash |
| | `isolation.resolved_candidate_version` | `string` | `"native"`, `"cand3"`, `"cand4_2"`, `"cand5"` |
| | `isolation.dependency_fingerprint_verified` | `boolean` | Must be `true` |
| **Token Usage** | `telemetry.total_model_tokens` | `integer` | Authoritative provider usage sum |
| | `telemetry.prompt_tokens_total` | `integer` | Total input prompt tokens |
| | `telemetry.prompt_tokens_cached` | `integer` | Ingested via prompt cache |
| | `telemetry.prompt_tokens_uncached` | `integer` | Billed uncached prompt tokens |
| | `telemetry.completion_tokens` | `integer` | Model completion output |
| | `telemetry.reasoning_tokens` | `integer` | Internal chain-of-thought tokens |
| | `telemetry.peak_single_request_context` | `integer` | Maximum context size in any single turn |
| **Turn & Tool Metrics** | `telemetry.turns_count` | `integer` | Total turns executed |
| | `telemetry.mcp_tool_calls_count` | `integer` | Total Engram tool calls made |
| | `telemetry.replacement_accounting` | `object` | Serialized bytes, excerpts count, auxiliary payload |
| **Correctness & Status** | `execution_status` | `string` | `"SUCCESS"`, `"HALTED"`, `"SKIPPED"`, `"TIMEOUT"` |
| | `correctness.score` | `number` | 0.00 to 1.00 |
| | `correctness.passed_all_requirements`| `boolean` | True if all REQs pass |
| | `telemetry_valid` | `boolean` | True if hash chain & schemas valid |

---

## 2. Distinction: Provider-Reported vs Locally Calculated Telemetry

1. **Provider-Reported Usage:**
   `prompt_tokens_total`, `prompt_tokens_cached`, `prompt_tokens_uncached`, `completion_tokens`, and `reasoning_tokens` are extracted directly from the Codex JSONL session stream. They represent billing reality.
2. **Locally Calculated Usage:**
   `peak_single_request_context`, `auxiliary_tool_payload_bytes`, `serialized_jsonrpc_response_bytes`, and `effective_cost` are computed by `TokenTracker` from stdout character lengths and pricing coefficients.
3. **Requirement:** Telemetry records must never report estimated values in place of provider-reported fields. If provider usage is missing, `telemetry_valid` must be set to `False`.

---

## 3. Fail-Closed Handling of Incomplete or Missing Telemetry

The benchmark harness enforces strict fail-closed boundaries:
- **Missing Token Telemetry:** Run marked invalid; correctness score overridden to 0.00.
- **Binary Hash Mismatch:** Dispatch aborted immediately via `SafetyViolationError`.
- **Incomplete / Halted Arm:** Any halted arm is marked `accepted = False`.
- **Mutual Acceptance Rule:** In `compute_comparative_analysis`, if *either* the baseline arm or the candidate arm failed acceptance, the comparative verdict is recorded as `MUTUAL_ACCEPTANCE_FAILURE`. All token savings claims are strictly **INVALIDATED**.
