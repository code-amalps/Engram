# Milestone 5B Authorization Review: Model Identifier Verification

## 1. Executive Summary

This report documents the verification of the target model identifier for the proposed Milestone 5B matched-pair token-efficiency experiment.

The investigation examined harness source code, command-line invocation parameters, and local runtime configuration caches to confirm the model slug, configuration provenance, alias behaviors, and entitlement status.

---

## 2. Model Identifier Specification

- **Exact Model Slug:** `gpt-5.6-luna`
- **Display Name:** `GPT-5.6-Luna`
- **Provider & Backend:** OpenAI (via Codex AppServer)
- **Execution Tool:** Codex CLI (`codex.exe`)
- **Default Reasoning Level:** `medium`
- **Supported Reasoning Levels:** `low`, `medium`, `high`, `xhigh`, `max`
- **Native Context Window:** 272,000 tokens (Maximum context window: 872,000 tokens)
- **Tool Mode:** `code_mode_only`

---

## 3. Configuration Sources and Lineage

The model identifier is specified and enforced across three distinct layers:

### Layer 1: Harness Orchestration Defaults
In `benchmarks/matched-pair-harness/src/orchestrator.py`:
- Line 74: `model: str = "gpt-5.6-luna"`
- Line 1520: `parser.add_argument("--model", default="gpt-5.6-luna", help="Explicit model identifier")`
- Line 1572: Passed directly into `Orchestrator(model=args.model)`

### Layer 2: Process Invocation Layer
In `benchmarks/matched-pair-harness/src/runner.py`:
- Line 465: `model: str = "gpt-5.6-luna"`
- Line 533: Constructs subprocess command:
  ```python
  cmd = [
      str(self.codex_executable),
      "exec",
      "--model", self.model,
      "--ephemeral",
      "--full-auto",
      "--sandbox", self.sandbox_policy,
      ...
  ]
  ```
  The `--model` flag explicitly passes `gpt-5.6-luna` to the Codex process.

### Layer 3: Local Runtime Model Cache
In `C:\Users\Amal P S\.codex\models_cache.json`:
- Timestamp: `"fetched_at": "2026-10-30T08:51:33.694271300Z"`
- Entry:
  ```json
  {
    "slug": "gpt-5.6-luna",
    "display_name": "GPT-5.6-Luna",
    "description": "Older fast and efficient model.",
    "default_reasoning_level": "medium",
    "shell_type": "unified_exec",
    "visibility": "list",
    "supported_in_api": true,
    "context_window": 272000,
    "max_context_window": 872000,
    "effective_context_window_percent": 95,
    "tool_mode": "code_mode_only"
  }
  ```
  This proves `gpt-5.6-luna` is recognized as an active, supported model by the installed Codex runtime.

---

## 4. Fallback and Alias Analysis

1. **Model Aliases:**
   - In `models_cache.json`, `gpt-5.6-luna` is a primary canonical slug, not a dynamic alias pointing to an unpredictable backend.
2. **Upgrade / Fallback Policies:**
   - The cache defines `"upgrade": null`.
   - The harness invokes `codex.exe exec` with explicit `--model gpt-5.6-luna` and does **not** specify any auto-upgrade or fallback options.
   - If `gpt-5.6-luna` is unavailable or throttled, Codex exits with an error code rather than silently falling back to a different model (e.g. `gpt-reserve` or `gpt-6-luna`).
   - The harness runner captures non-zero exit codes and records them fail-closed as execution failures.

---

## 5. Account Entitlement Verification

- **Offline Evidence:** The presence of `gpt-5.6-luna` with `"visibility": "list"` and `"supported_in_api": true` in the fresh `models_cache.json` (fetched today, October 30, 2026) indicates that the account token possessed access at cache refresh time.
- **Limitation:** Real-time quota availability and active rate limits on the ChatGPT subscription cannot be queried offline without initiating live network traffic.
- **Classification:** **`OWNER VERIFICATION REQUIRED`**.
  The owner must verify in the ChatGPT UI or developer portal that their account retains active access to `gpt-5.6-luna`.
