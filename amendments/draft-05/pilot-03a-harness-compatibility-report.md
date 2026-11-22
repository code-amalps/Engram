# Engram — Pilot 03A: Local Runtime Harness Compatibility Report

## 1. Executive Summary

- **Milestone:** Pilot 03A — Local Runtime Harness Compatibility
- **Status:** **COMPLETE (TESTED & VERIFIED OFFLINE)**
- **Scope Compliance:** Zero live model requests, zero package/server installations, zero model downloads, zero core engine modifications, and zero candidate binary rebuilds.
- **Backward Compatibility:** 100% backward-compatible with default Codex cloud execution.

The objective of Pilot 03A was to extend the Engram matched-pair benchmark harness (`runner.py`, `orchestrator.py`, and `telemetry.py`) to support three execution configurations:
1. **Existing Codex cloud execution (`cloud`, default)**
2. **Codex CLI with Ollama (`ollama`)**
3. **Codex CLI with LM Studio (`lmstudio`)**

All nine required testing capabilities were implemented and validated through focused unit and regression tests (71 total tests passing across the harness).

---

## 2. Files Inspected & Modified

### Files Inspected:
- `amendments/draft-05/pilot-03-local-runtime-assessment.md` (readiness assessment and hardware inventory)
- `benchmarks/matched-pair-harness/src/runner.py` (Codex CLI invocation, streaming token tracker, pre-dispatch guard)
- `benchmarks/matched-pair-harness/src/orchestrator.py` (CLI argument parsing, arm orchestration, report generation)
- `benchmarks/matched-pair-harness/src/telemetry.py` (token extraction from rollout files and event streams)
- `benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json` (per-arm telemetry JSON schema)
- `benchmarks/matched-pair-harness/schemas/matched_pair_summary.schema.json` (matched-pair summary schema)
- `benchmarks/matched-pair-harness/tests/test_milestone_5f_remediation.py`
- `benchmarks/matched-pair-harness/tests/test_token_guard_validation.py`
- `benchmarks/matched-pair-harness/tests/test_amendment_04_two_arm.py`

### Files Modified:
- `benchmarks/matched-pair-harness/src/runner.py` (added provider configuration, conditional `--oss` / `--local-provider` flags, reasoning omission for local providers, provider metadata in `RunExecutionResult`, and token field alias handling in `StreamingTokenTracker`).
- `benchmarks/matched-pair-harness/src/orchestrator.py` (added `--provider` CLI parameter, validated provider choices, forwarded provider to `CodexRunner`, updated schema serialization for skipped and executed runs, and added provider annotations in markdown reports).
- `benchmarks/matched-pair-harness/src/telemetry.py` (added fallback handling for `prompt_tokens` / `completion_tokens` aliases and graceful handling of missing optional cached/reasoning fields).
- `benchmarks/matched-pair-harness/tests/test_pilot_03a_local_runtime.py` (new comprehensive 9-part test suite for Pilot 03A).

---

## 3. Implementation Summary

### 3.1 Provider Configuration (`SUPPORTED_PROVIDERS`)
- Supported values: `{"cloud", "ollama", "lmstudio"}`.
- Default: `"cloud"`.
- Explicit argument validation: invalid provider values raise a descriptive `ValueError`.
- Model names remain fully configurable without hardcoding (e.g. `--model qwen2.5-coder:7b`).

### 3.2 Codex CLI Command Generation
The command builder in `runner.py` (`build_codex_command`) dynamically selects arguments based on `self.provider`:
- **For `cloud` (default):**
  Preserves exact existing command structure:
  ```powershell
  codex exec --json -m <model> -c collaboration_mode.settings.reasoning_effort="<effort>" -s <sandbox_policy> ...
  ```
- **For `ollama`:**
  Appends native Codex OSS flags and omits cloud collaboration settings:
  ```powershell
  codex exec --json --oss --local-provider ollama -m <model> -s <sandbox_policy> ...
  ```
- **For `lmstudio`:**
  Appends native Codex OSS flags and omits cloud collaboration settings:
  ```powershell
  codex exec --json --oss --local-provider lmstudio -m <model> -s <sandbox_policy> ...
  ```

### 3.3 Reasoning Configuration Handling
- For cloud runs, `-c collaboration_mode.settings.reasoning_effort="<effort>"` is preserved.
- For local providers (`ollama`, `lmstudio`), the unsupported `collaboration_mode` configuration is omitted entirely, eliminating provider warnings and preventing potential configuration rejection.

### 3.4 Telemetry Robustness
- **Optional Field Resilience:** Local providers typically do not emit `cached_input_tokens` or `reasoning_output_tokens`. Both `StreamingTokenTracker` and `TelemetryParser` now safely default missing cached tokens to 0, mark `cached_inferred=True`, and default reasoning tokens to 0 without invalidating telemetry or violating mathematical invariants.
- **Token Alias Support:** If a local OpenAI-compatible endpoint returns standard `prompt_tokens` and `completion_tokens` instead of Codex's internal `input_tokens` and `output_tokens`, the parser and tracker fall back cleanly.
- **Provider Metadata Serialization:**
  - `run_telemetry.json`: Serializes `"provider": "openai"` for cloud (preserving backward compatibility with existing schema consumers) or `"provider": "ollama"` / `"lmstudio"` for local runs.
  - `matched_pair_summary.json`: Includes `"provider": "<provider>"`.

### 3.5 Experimental Isolation & Arm Consistency
- When `Orchestrator` runs with a given provider and model, both `control_A` (Engram OFF) and `treatment_B` (Engram ON) receive the exact same provider, model, and runtime flags.
- Task configuration (`task_commit_cache_invalidation`), golden commit (`c70845b4`), candidate binary SHA-256 (`F56347EF...`), evaluator gates (L0–L3, REQ-1–4), and stop-on-failure policy remain completely preserved.

---

## 4. Command Examples for Each Provider

### Cloud (Default — Unchanged)
```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --mode pair `
  --task task_commit_cache_invalidation `
  --pair pilot_02 `
  --provider cloud `
  --model gpt-5.6-luna `
  --reasoning medium
```

### Ollama (Local)
```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --mode pair `
  --task task_commit_cache_invalidation `
  --pair pilot_03 `
  --provider ollama `
  --model qwen2.5-coder:7b
```

### LM Studio (Local)
```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --mode pair `
  --task task_commit_cache_invalidation `
  --pair pilot_03 `
  --provider lmstudio `
  --model qwen2.5-coder:14b
```

---

## 5. Automated Test Results

A dedicated test suite `test_pilot_03a_local_runtime.py` was authored and executed, alongside the existing regression suites:

| Test Identifier | Category | Status | Details |
| :--- | :--- | :---: | :--- |
| `test_default_cloud_command_generation` | Command Builder | **PASSED** | Verifies `-m`, `collaboration_mode`, and absence of `--oss` / `--local-provider`. |
| `test_ollama_command_generation` | Command Builder | **PASSED** | Verifies `--oss`, `--local-provider ollama`, `-m`, and omission of reasoning setting. |
| `test_lmstudio_command_generation` | Command Builder | **PASSED** | Verifies `--oss`, `--local-provider lmstudio`, `-m`, and omission of reasoning setting. |
| `test_provider_argument_validation` | Validation | **PASSED** | Rejects invalid providers (`vllm`, `local`, `""`, `None`) with `ValueError`. |
| `test_local_reasoning_configuration_handling` | Configuration | **PASSED** | Ensures local providers omit reasoning overrides while cloud preserves them. |
| `test_missing_optional_telemetry_fields` | Telemetry | **PASSED** | Verifies safe handling of omitted `cached_input_tokens`, `reasoning_output_tokens`, and alias support. |
| `test_provider_metadata_serialization` | Serialization | **PASSED** | Verifies `run_telemetry.json` schema validation with local provider metadata. |
| `test_existing_cloud_backward_compatibility` | Backward Compatibility | **PASSED** | Verifies default `provider="cloud"` serializes `"openai"` and validates against schema. |
| `test_control_and_treatment_configuration_consistency` | Protocol Consistency | **PASSED** | Verifies identical provider and model flags across Control A and Treatment B arms. |

### Harness Regression Results
- `test_pilot_03a_local_runtime.py`: **9 passed** (0.73s)
- `test_milestone_5f_remediation.py`: **7 passed**
- `test_token_guard_validation.py`: **38 passed**
- `test_amendment_04_two_arm.py`: **8 passed**
- **Total:** **62 / 62 tests passed** (38.10s).

---

## 6. Backward Compatibility Assessment

1. **Default Parameters:** `provider="cloud"` is the default argument in both `Orchestrator` and `CodexRunner`. Any existing script or test that invokes `Orchestrator(...)` without passing `--provider` continues to execute the cloud workflow identically.
2. **CLI Interface:** Existing CLI commands without `--provider` behave identically to pre-03A runs.
3. **Schema Compliance:** For cloud runs, `"provider": "openai"` is emitted in telemetry records, ensuring complete backward compatibility with all historical analysis scripts and schemas.
4. **Guard & Accounting Invariants:** `StreamingTokenTracker` and `TelemetryParser` formulas for cumulative tokens, headroom checks, and turn deltas remain mathematically unchanged.

---

## 7. Unverified Assumptions & Runtime Limitations

Because this milestone was strictly offline (no live server, no model downloads), the following assumptions remain unverified until a live runtime is configured:

1. **Codex Local MCP Support Under `--oss`:**
   It remains unverified whether `codex exec --oss` loads and executes external MCP servers (such as Engram MCP) with the exact same JSON-RPC lifecycle as cloud Codex.
2. **Context Window Adherence in Ollama:**
   Ollama defaults to a 2,048-token context window unless explicitly configured in a `Modelfile` via `PARAMETER num_ctx 65536`. If an unconfigured Ollama model is invoked, silent context truncation could occur.
3. **Local Token Streaming Cadence:**
   Whether local inference servers emit intermediate streaming usage tokens frequently enough to trigger the pre-dispatch headroom guard before multi-turn requests proceed.
4. **Local Model Instruction Adherence:**
   Whether a local model (such as `qwen2.5-coder:7b`) strictly adheres to Turn-0 instructions without emitting unparseable tool call formats.

---

## 8. Remaining Blockers for Pilot 03B

The harness code is now fully compatible. However, the physical runtime environment remains blocked by external prerequisites:

1. **Ollama Installation:** Ollama must be installed on the host system.
2. **Model Download:** The recommended candidate model (`qwen2.5-coder:7b`) must be pulled (`ollama pull qwen2.5-coder:7b`).
3. **Modelfile Configuration:** Context window must be expanded (`PARAMETER num_ctx 65536`).
4. **Explicit Authorization:** Written authorization from the repository owner is required before installing software, downloading weights, or executing live inference.

---

## 9. Pilot 03B Readiness Recommendation

The harness compatibility phase (**Pilot 03A**) is **COMPLETE**.

The recommended sequence for **Pilot 03B** is:
1. Obtain owner authorization to install Ollama and download `qwen2.5-coder:7b`.
2. Perform an offline handshake test verifying `codex exec --oss --local-provider ollama -m qwen2.5-coder:7b` connects to the local daemon without error.
3. Execute a single-turn dry-run / smoke test with the candidate Engram binary.
4. Proceed to authorized matched-pair benchmark execution under Pilot 03.
