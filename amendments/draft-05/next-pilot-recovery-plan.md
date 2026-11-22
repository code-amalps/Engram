# Engram — Next Pilot Recovery Plan & Runtime Flexibility Analysis

## 1. Executive Context

Pilot 02 was authorized as an isolated matched-pair experiment under Milestone 5G on 2026-11-04. The treatment arm terminated before inference due to a provider-level subscription usage limit (`usage_limit_exceeded`), resulting in zero tokens consumed and clean stop-on-failure triggering for the control arm.

This recovery plan synthesizes the offline learnings from Pilot 01 and Pilot 02, conducts an offline investigation into harness and Codex AppServer runtime flexibility, and outlines the precise criteria required before any future pilot (Pilot 03) can be authorized.

---

## 2. What Pilot 02 Established

Despite provider rejection preventing agent execution, Pilot 02 established several critical offline and harness behaviors:

1. **Worktree & Isolation Subsystem:**
   - Isolated ephemeral sandboxes were created from baseline commit `c70845b4d5913457f1b176fb0370b80dd792711f` without corrupting or touching the active working tree or golden repository.
   - Post-run teardown was executed cleanly without leftover background processes or dangling file locks.
2. **Turn-0 Context Compilation & Payload Injection:**
   - The candidate Phase 1 binary (`F56347EF...`) executed `inject` as specified.
   - AST symbol indexing, target file detection, and excerpt compilation occurred cleanly, writing directives to `.github/copilot-instructions.md`.
   - Inspection of rollout file `rollout-2026-11-04T14-18-22-01a1a5be-b1cb-7641-ae7a-e222c258c868.jsonl` confirmed the compiled Turn-0 block was fully integrated into the prompt payload sent to Codex.
3. **Stop-on-Failure Protection:**
   - When `treatment_B` encountered an unrecoverable provider process error, the orchestrator immediately aborted the matched-pair schedule and skipped `control_A`.
   - Prevented unnecessary expenditure of quota and preserved benchmark integrity.
4. **Accurate Telemetry and Schema Invariants:**
   - Telemetry schemas properly classified the run as `FAILED`, recorded token consumption as `0`, marked `control_A` as `SKIPPED`, and suppressed comparative token savings to `null` with verdict `TREATMENT_FAILED_CONTROL_SKIPPED`.
   - No false parity or misleading token metrics were emitted.

---

## 3. What Remains Untested

Due to zero model inference occurring during Pilot 02, the following critical boundaries remain unverified:

1. **Runtime MCP Server Exposure in Codex Exec:**
   - While `.codex/config.toml` was generated inside the sandbox, it remains unverified whether Codex AppServer successfully surfaces the MCP tools (`engram_get_context`, `engram_inspect_symbol`, etc.) into the active agent tool manifest during multi-turn execution.
2. **Agent Reliance on Excerpts vs Brute-Force Shell Dumping:**
   - It is unknown whether the Turn-0 AST excerpt guidance effectively suppresses the agent from issuing brute-force shell reads (e.g. `cat`, `Get-Content`, `rg`).
3. **End-to-End Replacement Accounting:**
   - `TokenReplacementAccounting` and the `tokens_per_successful_task` metric have not yet been observed against an active agent trajectory.
4. **Task Correctness Gate ($C = 1.0$):**
   - The functional correctness evaluator for `task_commit_cache_invalidation` (L0 typecheck, L2 unit tests, REQ-1 through REQ-4) has not evaluated an agent patch.

---

## 4. Offline Investigation: Harness & Codex Runtime Flexibility

An offline technical analysis of `orchestrator.py`, `runner.py`, and `codex.exe` reveals the following architectural capabilities regarding runtimes and authentication:

### 4.1 CLI Model Identifier Configurability
- **Finding:** The model identifier is **fully configurable via CLI without any source code changes**.
- **Implementation:** `orchestrator.py` exposes the `--model <MODEL>` flag (lines 1612, 1665), which passes directly to `CodexRunner(model=args.model)` and is emitted in the subprocess command line as `codex.exe exec --json -m <model> ...`.
- **Reasoning Effort:** Similarly, reasoning effort is configurable via `--reasoning <effort>` without source modifications.

### 4.2 Codex AppServer Authentication & Account Options
Inspection of `codex.exe login --help`, `codex.exe login status`, and `codex.exe exec --help` reveals multiple distinct operational runtimes:

1. **ChatGPT Web Subscription (Current Operational State):**
   - Currently authenticated via `Logged in using ChatGPT`.
   - Enforces ChatGPT subscription tiers (Free / Plus / Team / Pro) with sliding window rate limits.
   - Status: Blocked by rate limiter until Nov 27, 2026 (`usage_limit_exceeded`).
2. **OpenAI API Key Authentication:**
   - `codex.exe login --with-api-key` allows piping a standard OpenAI API key (`sk-...`).
   - Alternatively, setting standard environment variables (e.g. `OPENAI_API_KEY`) or passing custom configuration profiles.
   - Bypasses ChatGPT web subscription rate-limit windows, drawing instead from standard OpenAI API platform credit/tier quotas.
3. **Named Configuration Profiles:**
   - `codex.exe exec` supports `-p, --profile <CONFIG_PROFILE_V2>`.
   - Enables separate credentials or endpoints to be maintained in `~/.codex/config.toml` under distinct profile names without mutating global defaults.
4. **Local / Open-Source Providers:**
   - `codex.exe exec` supports `--oss` and `--local-provider <lmstudio|ollama>`.
   - Allows offline execution against local LLMs with zero cloud quota consumption.

### 4.3 Independence of Model Selection vs Authentication
- In Codex AppServer, model selection (`-m <model>`) and authentication mechanisms are **decoupled**:
  - The model flag selects the architecture/checkpoint requested from the backend.
  - The authentication layer selects the gateway (ChatGPT web token vs OpenAI API key).
  - An API key account can request models such as `gpt-4o`, `o3-mini`, or other supported variants, completely detached from the exhausted ChatGPT subscription quota.

### 4.4 Telemetry Equivalence Across Runtimes
- **Finding:** Any runtime executed via `codex.exe exec --json` produces structured JSON event streams on `stdout` and writes native rollout JSONL files into `C:\Users\Amal P S\.codex\sessions\...`.
- **Parsing Compatibility:**
  - `telemetry.py` extracts authoritative metrics from the `token_usage_record` and `thread_token_usage` structures.
  - As long as the backend reports standard token counts (`input_tokens`, `output_tokens`, `cached_input_tokens`, `reasoning_output_tokens`), `StreamingTokenTracker` and `TelemetryParser` operate with 100% equivalence.
  - OpenAI API key execution maintains exact schema and event-type parity with ChatGPT subscription runs.

### 4.5 Impact on Benchmark Validity & Baselines
- **Within-Pair Internal Validity:**
  - In matched-pair benchmarking, both arms (`control_A` and `treatment_B`) run on the **identical model, runtime, and seed**.
  - Therefore, the relative token delta:
    $$\Delta = 1 - \frac{\text{Tokens}_B}{\text{Tokens}_A}$$
    remains mathematically and scientifically valid for that chosen runtime, regardless of whether it is `gpt-5.6-luna`, `gpt-4o`, or `o3-mini`.
- **Cross-Triad / Cross-Amendment Comparability:**
  - If a different model is used (e.g. switching from `gpt-5.6-luna` to `o3-mini` or `gpt-4o`), direct numerical comparison against historical Amendment 02/03/04 absolute token baselines is invalid because agent capability, context capacity, and tokenizer parameters differ.
  - If an alternative model is selected for future benchmark blocks, a fresh matched-pair baseline must be established.
  - If standard OpenAI API key authentication is used with the **same underlying model family**, cross-comparability is maximally preserved.

---

## 5. Minimum Conditions for Next Pilot (Pilot 03)

Before any new pilot execution can be authorized, all of the following conditions must be satisfied:

| # | Condition | Verification Mechanism | Status |
| :-: | :--- | :--- | :-: |
| **C1** | **Active Quota / Unblocked Runtime** | Run offline preflight test with `codex login status` / credential verification confirming active quota. | **BLOCKED** (Requires quota reset or API key switch) |
| **C2** | **Task Contract Preservation** | Task remains `task_commit_cache_invalidation` on commit `c70845b4`. | **VERIFIED** |
| **C3** | **Candidate Binary Integrity** | Candidate binary remains `candidate-phase1/engram.exe` with SHA-256 `F56347EF...`. | **VERIFIED** |
| **C4** | **Token Ceiling & Reserve** | Token ceiling fixed at 175,000; headroom reserve fixed at 15,000 (160k threshold). | **VERIFIED** |
| **C5** | **Ephemeral Isolation** | Sandbox paths dynamically allocated under `%TEMP%\engram_pilot\runs\...` with copy or none dependency strategy. | **VERIFIED** |
| **C6** | **Stop-on-Failure Active** | Stop-on-failure enforced to halt control arm if treatment arm fails. | **VERIFIED** |
| **C7** | **Explicit Owner Authorization** | Written authorization from the repository owner specifying task, pair ID, model, and runtime credentials. | **PENDING** |

---

## 6. Actionable Next Steps

1. **Select Runtime Path (Owner Decision):**
   - *Option A (Wait for reset):* Defer live pilot execution until quota resets on Nov 27, 2026.
   - *Option B (OpenAI API Key):* Configure an active OpenAI API key via `codex login --with-api-key` or profile, enabling immediate resumption.
   - *Option C (Alternate Model):* Authorize an alternative supported model available under an active quota tier.
2. **Maintain Offline Readiness:**
   - Keep candidate Phase 1 binary frozen.
   - Keep task configuration and evaluators locked.
   - Keep Git working tree clean.
3. **Awaiting Owner Instruction:**
   - Stand down from any model inference until explicit runtime direction and fresh authorization are provided.
