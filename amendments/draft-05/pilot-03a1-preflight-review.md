# Engram — Pilot 03A.1: Pre-Pilot 03B Technical Review

## 1. Executive Summary

- **Milestone:** Pilot 03A.1 — Pre-Pilot 03B Technical Review
- **Status:** **COMPLETE (REVIEW PASSED)**
- **Scope Compliance:** Strictly offline. Zero software installations, zero model downloads, zero server processes spawned, zero live model calls, and zero candidate binary rebuilds.
- **Purpose:** Rigorous preflight review of the completed Pilot 03A local runtime harness compatibility implementation prior to authorizing host environment setup (Pilot 03B).

---

## 2. Telemetry Integrity & Serialization Audit

An exhaustive audit of `telemetry.py`, `runner.py`, and `orchestrator.py` was conducted to verify that missing, inferred, and measured token metrics are mathematically and semantically distinct.

### 2.1 Distinction Between Measured Zero, Inferred, and Unavailable
The harness enforces strict multi-tier classification across all token telemetry fields:

| Field | Measured State | Inferred / Missing Fallback State | Unavailable State |
| :--- | :--- | :--- | :--- |
| **`total_model_tokens`** | Directly measured from provider ($>0$). | Cannot be inferred. A count of 0 is flagged as `ZERO_TOKEN_ANOMALY`. | `0` with `telemetry_valid = False` and `provenance = "UNAVAILABLE"`. |
| **`prompt_tokens_cached`** | Present in rollout record; `prompt_tokens_cached_inferred = False`. | When omitted (standard in Ollama/LM Studio), defaults to `0`, but explicitly marked with **`prompt_tokens_cached_inferred = True`**. Never falsely presented as measured zero. | `0` with `telemetry_valid = False`. |
| **`prompt_tokens_uncached`** | Directly measured when cached is present; `prompt_tokens_uncached_inferred = False`. | Computed as `prompt_tokens_total - prompt_tokens_cached`; marked **`prompt_tokens_uncached_inferred = True`**. | `0` with `telemetry_valid = False`. |
| **`reasoning_tokens`** | Measured from `reasoning_output_tokens` or `reasoning_tokens`. | For models without reasoning tokens (e.g. standard local coding models), set to `0` as an informative subset of `completion_tokens`. | `0` with `telemetry_valid = False`. |
| **`peak_single_request_context`**| Maximum of individual request input tokens from session records. | None. | Recorded explicitly as **`None` / `null`** (never falsely substituted with cumulative thread tokens). |
| **`replacement_accounting`** | Observed MCP tool calls and file accesses; status: **`"MEASURED"`**. | Estimated from observed shell commands; status: **`"ESTIMATED"`**. | Zero observed activity; status: **`"NOT_OBSERVABLE"`**. `tokens_per_successful_task` is strictly `None` unless $C = 1.0$. |

### 2.2 Token Savings Calculation Guards
In `Orchestrator.compute_comparative_analysis`:
1. **Invalid / Missing Telemetry Invalidation:**
   If `telemetry_valid` is `False`, or required token fields are null/zero on either arm, comparative savings claims are **strictly ineligible** (`savings_claim_eligible: false`, `savings_claim_status: "INVALIDATED_BY_TELEMETRY_INVALID"`, all percentage metrics suppressed to `null`). Missing telemetry cannot artificially improve or fabricate token savings.
2. **Peak Context Handling:**
   If `peak_single_request_context` is `None` on either arm, `peak_delta` and `peak_savings_pct` are set to `None`, with status marked explicitly as **`"UNAVAILABLE"`**.
3. **Acceptance Gate Dependency:**
   Savings claims are strictly conditioned on both arms achieving functional acceptance ($C = 1.0$). If either arm fails, savings metrics are suppressed to `null` with status `INVALIDATED_UNACCEPTED`.

### 2.3 Historical Cloud Telemetry Backward Compatibility
- For cloud runs (`--provider cloud`), `run_telemetry.json` serializes `"provider": "openai"` and records `reasoning_effort`.
- All existing JSON schemas (`run_telemetry.schema.json` and `matched_pair_summary.schema.json`) validate successfully without modification.

---

## 3. Context Window Assessment (16K vs. 32K vs. 64K)

The initial assessment noted that Ollama defaults to a 2,048-token context window unless configured via `Modelfile`. A 65,536-token (64K) window was proposed. We evaluated 16K, 32K, and 64K configurations against the hardware constraints of the test machine (11th Gen Intel Core i7-1185G7 @ 3.00GHz, 4 cores / 8 threads, Intel Iris Xe graphics, 32 GB RAM, ~6.75 GB free RAM).

### 3.1 Architectural Parameters (`Qwen2.5-Coder-7B-Instruct`, Q4_K_M)
- **Parameters:** 7.61 billion.
- **Model Weights (Q4_K_M):** ~4.68 GB RAM.
- **Architecture:** 28 layers ($L$), 4 KV heads ($n_{\text{kv}}$) via Grouped Query Attention (GQA), head dimension 128 ($d_{\text{head}}$).
- **FP16 KV Cache Formula:**
  $$\text{KV bytes per token} = 2 \times L \times n_{\text{kv}} \times d_{\text{head}} \times 2 = 2 \times 28 \times 4 \times 128 \times 2 = 57,344 \text{ bytes} \approx 56.0 \text{ KB/token}$$

### 3.2 Quantitative Comparison

| Dimension | 16K (16,384 tokens) | 32K (32,768 tokens) — **RECOMMENDED** | 64K (65,536 tokens) — **REJECTED** |
| :--- | :---: | :---: | :---: |
| **Model Weights** | ~4.7 GB | ~4.7 GB | ~4.7 GB |
| **KV Cache Footprint** | ~0.92 GB | ~1.83 GB | ~3.67 GB |
| **Total Memory Required** | **~5.62 GB** | **~6.53 GB** | **~8.37 GB** |
| **Memory Headroom** | Fits entirely inside currently free physical RAM (~6.75 GB). | Fits cleanly within available RAM with minor OS cache eviction. | Exceeds currently free physical RAM; forces aggressive OS paging. |
| **Prefill Speed (AVX-512 CPU)** | ~35–50 tokens/sec. | ~35–50 tokens/sec. | Degrades significantly at high context due to quadratic self-attention. |
| **Turn Ingestion Latency** | ~20–40 sec for typical turns. | ~45–90 sec for late-stage turns. | **> 300–800 sec** (> 5–13 minutes per turn!). |
| **Harness Timeout Risk** | Very Low (well within 600s limit). | Low (stays well within 600s limit). | **CRITICAL: High risk of breaching 600s timeout.** |
| **Native Training Window** | Supported (native is 32k). | **Native window** (highest attention fidelity). | Requires RoPE YaRN extrapolation beyond native training length. |
| **Benchmark Task Suitability** | Borderline: Turn-0 context is ~4k tokens, leaving ~12k tokens for 4–6 tool turns. | **Ample:** Turn-0 payload (~4k) + 6–10 turns of bounded excerpts fit comfortably. | Excessive: Engram exists specifically to prevent 64k token explosions. |

### 3.3 Context Recommendation
> [!IMPORTANT]
> **RECOMMENDATION: 32,768 tokens (32K)**
> 
> - **Why NOT 64K:** On a 4-core mobile CPU with no dedicated GPU, quadratic self-attention at 64k tokens introduces catastrophic ingestion latencies exceeding 10 minutes per turn, directly triggering the harness process timeout (`timeout_seconds = 600`) and aborting the benchmark. Furthermore, 64k exceeds Qwen2.5-Coder's native 32k training window, requiring RoPE scaling that degrades code syntax precision.
> - **Why NOT 16K:** While fast, 16k leaves insufficient buffer if the agent inspects multiple source files alongside Turn-0 prompt payloads.
> - **Why 32K is Optimal:** 32,768 tokens is Qwen2.5-Coder's **native trained context window**, requires only ~1.83 GB KV cache, keeps total model memory under 6.6 GB, completes turn prefill within 45–90 seconds, and provides more than enough capacity for Engram-guided task execution.

---

## 4. Codex CLI Compatibility Verification

### 4.1 Static Verification (Completed)
- **Executable:** `C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe`
- **Version:** `codex-cli 0.166.0-alpha.5.1`
- **Documented Options Verified:**
  - `--oss`: "Use open-source provider"
  - `--local-provider <OSS_PROVIDER>`: specifically accepts `ollama` and `lmstudio`
  - `--json`: formats turn events as JSONL on `stdout`
  - `-m <model>`: specifies target model identifier
- **Command Construction:** Verified in `CodexRunner.build_codex_command()`:
  ```powershell
  codex.exe exec --json --oss --local-provider ollama -m qwen2.5-coder:7b -s workspace-write --add-dir <sandbox> -C <sandbox> ...
  ```

### 4.2 Runtime Verification (Unverified Boundary)
> [!WARNING]
> **Static Verification $\ne$ Runtime Handshake**
> 
> The Codex CLI command syntax has been verified statically against the binary's help output and strings. However, **actual runtime behavior under `--oss` remains unverified** because no live Ollama daemon is currently running on the host. Specifically:
> 1. It is unverified whether Codex AppServer's `--oss` runtime successfully exposes local external MCP servers (configured in `.codex/config.toml`).
> 2. It is unverified whether Codex AppServer correctly parses tool calls generated by `qwen2.5-coder:7b` when operating through Ollama's local OpenAI-compatible endpoint.
> 
> These boundaries can only be tested during Pilot 03B via a controlled, non-inferencing server handshake.

---

## 5. Experimental Integrity Confirmation

1. **Arm Symmetry:**
   In `Orchestrator`, both `control_A` (Engram OFF) and `treatment_B` (Engram ON) are executed by the same runner instance using the identical provider, model, and isolation policies. Verified in `test_control_and_treatment_configuration_consistency`.
2. **Evaluator Gates Unchanged:**
   The evaluation pipeline in `evaluator.py` (L0 typecheck, L1 lint, L2 unit tests, L3 production build, REQ-1 through REQ-4) was completely untouched.
3. **Historical Evidence Untouched:**
   Historical benchmark results and manifests in `benchmarks/matched-pair-harness/results/pilot_01/` and `pilot_02/` remain unmodified.
4. **Candidate Binary Frozen:**
   The candidate binary remains `candidate-phase1/engram.exe` with SHA-256:
   `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`.

---

## 6. Pilot 03B Readiness & Staging Plan

### Readiness Verdict:
**HARNESS READY FOR OFFLINE PREFLIGHT & LOCAL SETUP**

### Authorized Prerequisites for Pilot 03B:
Before executing any matched-pair benchmark runs under Pilot 03:
1. **Software Installation (Host):**
   Install Ollama for Windows (`https://ollama.com/download/windows`).
2. **Model Download & Context Tuning:**
   Pull `qwen2.5-coder:7b` and configure a custom Modelfile:
   ```dockerfile
   FROM qwen2.5-coder:7b
   PARAMETER num_ctx 32768
   ```
   Create tuned model: `ollama create qwen2.5-coder-32k -f Modelfile`.
3. **Dry-Run Handshake Test:**
   Execute a zero-token dry-run handshake confirming Codex detects `ollama` on `http://localhost:11434`.
4. **Owner Authorization:**
   Obtain explicit written owner authorization specifying the local model, provider, and ceiling parameters.
