# Engram — Pilot 03: Free Local Runtime Compatibility & Feasibility Assessment

## 1. Executive Summary

This document performs an independent, offline compatibility assessment for executing Engram matched-pair benchmark runs using a free, locally hosted runtime (Ollama or LM Studio) instead of ChatGPT web subscription quota or paid API credits.

### Final Readiness Verdict:
> [!WARNING]
> **VERDICT: NOT READY — BLOCKERS REMAIN**
> 
> While Codex CLI natively supports local open-source runtimes (`--oss --local-provider ollama|lmstudio`), **neither Ollama nor LM Studio is installed**, zero local model weights exist on disk, the machine has no dedicated GPU (relying on a 4-core mobile CPU), and the Engram benchmark harness (`runner.py`) currently lacks command-line plumbing to pass `--oss` and `--local-provider` flags to Codex.

---

## 2. Hardware Inventory

An offline discovery scan of the host environment via Windows CIM/WMI revealed the following specifications:

| Hardware Component | Detected Specification | Constraint Analysis for Local Inference |
| :--- | :--- | :--- |
| **CPU** | 11th Gen Intel(R) Core(TM) i7-1185G7 @ 3.00GHz (4 physical cores, 8 logical processors) | CPU-only inference bound by AVX2/AVX-512 throughput on 4 cores. Prompt processing and generation will experience significant latency for large contexts. |
| **System RAM** | 32.0 GB (33,226,972 KB visible); ~6.75 GB currently free physical RAM | Sufficient to host a 7B–8B quantized model (4.5–5.0 GB) plus an intermediate 32k–64k KV cache (4.0–7.5 GB). 14B+ models will encounter severe memory pressure and swapping. |
| **GPU / VRAM** | Intel(R) Iris(R) Xe Graphics (Integrated, Driver 32.0.101.7088, ~2.0 GB shared memory) | **No dedicated NVIDIA CUDA or high-bandwidth VRAM.** GPU offloading via Vulkan/OpenCL on Iris Xe offers very limited speedup over CPU AVX2 and cannot hold large model weights. |
| **Storage (C:)** | 475.78 GB total; **252.09 GB free** (NTFS SSD) | Ample disk space to install Ollama and download candidate model weights (5–15 GB). |
| **Operating System** | Windows 11 Pro 64-bit | Fully compatible with Windows Ollama installer and LM Studio for Windows. |

---

## 3. Installed Local Providers & Existing Models

A comprehensive filesystem and registry audit checked standard user and system paths (`AppData`, `Program Files`, registry uninstall keys, and PATH):

- **Ollama Installation:** **NOT INSTALLED** (`ollama.exe` not found on PATH, `C:\Program Files\Ollama`, or `AppData\Local\Programs\Ollama`).
- **LM Studio Installation:** **NOT INSTALLED** (`lms.exe` not found on PATH or `AppData\Local\Programs\LM Studio`).
- **Locally Downloaded Models:** **NONE (0 models)**. No GGUF weights, Safetensors, or model manifests exist in `~/.ollama`, `~/.cache`, or `AppData`.
- **Existing Context Window Configurations:** **NONE**.

---

## 4. Codex CLI Compatibility Assessment

Inspection of `C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe` (version `codex-cli 0.166.0-alpha.5.1`) via CLI help and binary string disassembly confirmed the following:

### 4.1 Native Open-Source Provider Support
Codex CLI natively provides dedicated options for non-interactive local execution:
- `--oss`: Flag to enable the open-source provider backend.
- `--local-provider <OSS_PROVIDER>`: Explicitly accepts `ollama` or `lmstudio`.
- Fallback configuration: If not passed on the CLI, Codex checks `oss_provider` or `model_provider` in `config.toml` (`oss_provider = "ollama"`).

### 4.2 Endpoint & Protocol Handling
- **Ollama Integration:** Codex connects to Ollama via default port `11434` (`http://localhost:11434`), targeting `/api/version`, `/api/tags`, and OpenAI-compatible `/v1/models`.
  - Binary inspection confirms: `"Local Ollama server (Responses API, default port 11434) ollama"`.
  - Codex verifies that Ollama is running and rejects outdated versions (`"Ollama is too old. Codex requires Ollama ... or newer"`).
- **LM Studio Integration:** Codex connects to LM Studio via default port `1234` (`http://localhost:1234/v1`).
  - Binary inspection confirms: `"Local LM Studio server (default port 1234) lmstudio"`.

### 4.3 Non-Interactive JSON Event Output (`--json`)
- Codex supports `codex exec --json --oss --local-provider ollama -m <model>`.
- In non-interactive mode, Codex formats internal turn events into JSONL on `stdout` and continues to persist native session rollouts in `~/.codex/sessions/<year>/<month>/<day>/rollout-...jsonl`.

---

## 5. Engram Benchmark Harness Compatibility

Inspection of `benchmarks/matched-pair-harness/src/orchestrator.py`, `runner.py`, and `telemetry.py` revealed several compatibility gaps:

### 5.1 Command Construction Gap (`runner.py`)
In `CodexRunner.build_codex_command()` (lines 569–576):
```python
cmd = [
    str(self.codex_executable),
    "exec",
    "--json",
    "-m", self.model,
    "-c", f'collaboration_mode.settings.reasoning_effort="{self.reasoning_effort}"',
    "-s", self.sandbox_policy,
]
```
- **Missing OSS Flags:** `runner.py` does not currently accept or emit `--oss` or `--local-provider <provider>`.
- **Incompatible Configuration Override:** It unconditionally emits `-c collaboration_mode.settings.reasoning_effort="..."`. While this was tolerated by Codex in Pilot 02 with a warning, local models do not recognize this setting and it pollutes the event stream.

### 5.2 Orchestrator CLI Gap (`orchestrator.py`)
- `orchestrator.py` currently exposes `--model` and `--reasoning`, but lacks `--oss` or `--local-provider` flags to pass through to `runner.py`.

### 5.3 Token Telemetry Compatibility (`telemetry.py`)
- `TelemetryParser` extracts tokens from `token_usage_record` in the rollout file.
- Local OpenAI-compatible endpoints in Ollama and LM Studio report `input_tokens` (or `prompt_eval_count`) and `output_tokens` (or `eval_count`).
- **Telemetry Invariant:** Local endpoints typically do **not** report `cached_input_tokens` or `reasoning_output_tokens`.
- **Harness Behavior:** `telemetry.py` lines 376–379 already contain safe fallback handling:
  ```python
  prompt_tokens_cached = 0
  prompt_tokens_cached_inferred = True
  prompt_tokens_uncached = prompt_tokens_total
  prompt_tokens_uncached_inferred = True
  ```
  Therefore, missing cached token fields will not crash the telemetry parser or invalidate schemas.
- **Headroom Guard (`runner.py`):** `StreamingTokenTracker` will track cumulative usage based on reported input/output token counts.

---

## 6. Recommended Local Model

Because the host system lacks a dedicated GPU with VRAM, model selection must balance coding competence, tool-calling precision, and CPU inference latency.

### 6.1 Primary Recommendation: `Qwen2.5-Coder-7B-Instruct` (via Ollama `qwen2.5-coder:7b`)

| Evaluation Dimension | Assessment |
| :--- | :--- |
| **Coding Ability** | State-of-the-art for open-weight 7B parameter models. Strongly capable in Rust, AST comprehension, and surgical refactoring. |
| **Agentic Tool Calling** | Specially trained on OpenAI-compatible function calling and tool invocation schemas; reliable in multi-turn tool loops. |
| **Structured Output** | Follows strict JSON and diff instructions without extraneous conversational preamble. |
| **Context Window** | Native 32k context window (extensible to 64k via Ollama Modelfile `PARAMETER num_ctx 65536`). |
| **Memory Footprint** | Model weights (Q4_K_M): **~4.7 GB**. KV cache at 32k: **~3.6 GB**. Total RAM: **~8.3 GB** (Comfortably fits in 32 GB system RAM). |
| **Inference Speed** | On 4-core i7-1185G7: Generation ~5–8 tokens/sec; prompt ingestion ~25–40 tokens/sec. Usable, though prompt ingestion on long turns will take 15–30 seconds. |
| **Installation Status** | **NOT INSTALLED. Requires downloading ~4.7 GB.** |

### 6.2 Secondary Alternative: `Llama-3.1-8B-Instruct` (via Ollama `llama3.1:8b`)
- **Native Context:** 128k context window.
- **Memory Footprint:** ~4.9 GB weights + ~4.5 GB KV cache at 32k.
- **Tradeoff:** Strong general reasoning and tool use, but slightly lower Rust-specific coding precision compared to Qwen2.5-Coder.

### 6.3 Models Evaluated and Rejected:
- **Qwen2.5-Coder-14B / 32B:** Rejected due to CPU inference speed. On 4 cores, generation drops to 1–3 tokens/sec; processing a 20k context turn would take 2–4 minutes per turn, risking harness timeout.
- **DeepSeek-Coder-V2-Lite (16B MoE):** High memory bandwidth requirement (~9 GB weights) causes sluggish prompt processing on Intel mobile CPU.
- **3B Models (Qwen2.5-Coder-3B, Llama-3.2-3B):** Fast on CPU (~18 tok/s), but **inadequate** for complex multi-turn agentic refactoring. They frequently fail tool-calling syntax and break task contracts.

---

## 7. Experimental Comparability & Scientific Integrity

1. **Incomparability to GPT-5.6-Luna Baselines:**
   - Results obtained from `qwen2.5-coder:7b` cannot be compared numerically to historical GPT-5.6-Luna results (e.g. Milestone 5E/5G baselines).
   - Local models have different tokenization ratios, context capacities, and instruction adherence profiles.
2. **Matched-Pair Internal Validity:**
   - Both `treatment_B` (Engram ON) and `control_A` (Engram OFF) will execute against the **identical local model, quantization, and Ollama server instance**.
   - The comparative token savings formula:
     $$\Delta = 1 - \frac{\text{Tokens}_B}{\text{Tokens}_A}$$
     remains scientifically valid and internally sound for measuring Engram's context replacement efficiency.
3. **Preservation of Protocol Invariants:**
   - Task: `task_commit_cache_invalidation` (commit `c70845b4`).
   - Candidate binary: `candidate-phase1/engram.exe` (`F56347EF...`).
   - Acceptance gate: L0 typecheck, L2 unit tests, REQ-1 through REQ-4 ($C = 1.0$).
   - Ceilings: Per-arm ceiling of 175,000 tokens, 15,000 reserve.
   - Stop-on-failure policy enforced.

---

## 8. Required Changes Before Local Pilot Execution

To enable local execution without violating harness invariants, the following engineering steps must be taken (pending explicit owner authorization):

1. **Host Environment Setup (External to Repo):**
   - Install Ollama for Windows (`https://ollama.com/download/windows`).
   - Verify Ollama service is active on `http://localhost:11434`.
   - Download the target model: `ollama pull qwen2.5-coder:7b`.
   - Configure a custom Modelfile setting `num_ctx 65536` to ensure the context window is not artificially capped at Ollama's default 2,048 tokens.
2. **Harness Configuration Support (Repo):**
   - Add `--oss` and `--local-provider <ollama|lmstudio>` arguments to `orchestrator.py`.
   - Update `runner.py` to conditionally pass `--oss` and `--local-provider` to `codex.exe exec` when a local provider is selected.
   - Conditionally suppress `-c collaboration_mode.settings.reasoning_effort=...` when running under local providers.
3. **Offline Dry-Run Preflight:**
   - Perform a dry-run check confirming Codex can establish a handshake with `http://localhost:11434` without error.

---

## 9. Blockers & Final Readiness Verdict

| # | Requirement | Status | Details |
| :-: | :--- | :---: | :--- |
| **B1** | **Local Provider Installed** | **BLOCKED** | Neither Ollama nor LM Studio is installed on the machine. |
| **B2** | **Model Weights Available** | **BLOCKED** | Zero local model weights exist on disk. |
| **B3** | **Hardware Acceleration** | **QUALIFIED** | System lacks dedicated GPU; CPU-only execution is viable only with a 7B model. |
| **B4** | **Harness CLI & Runner Plumbing** | **BLOCKED** | `orchestrator.py` and `runner.py` do not support `--oss` / `--local-provider` flags. |
| **B5** | **Explicit Owner Authorization** | **PENDING** | Written authorization required before installing software, pulling models, or modifying harness. |

### Final Verdict:
**NOT READY — BLOCKERS REMAIN**
