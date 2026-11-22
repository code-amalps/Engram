# Engram Pilot 02 — Formal Closure Report

## 1. Executive Summary

- **Experiment Identifier:** `pilot_02`
- **Task:** `task_commit_cache_invalidation` (Rust / Cargo)
- **Baseline Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`
- **Target Mode:** Matched Pair (`pair`)
- **Execution Date:** 2026-11-04T08:47:55Z (14:17:55 IST)
- **Candidate Phase 1 Binary:** `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`
- **Candidate SHA-256:** `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`
- **Configured Model / Reasoning:** `gpt-5.6-luna` / `medium`
- **Token Ceiling / Headroom Reserve:** 175,000 / 15,000
- **Overall Verdict:** `TREATMENT_FAILED_CONTROL_SKIPPED`
- **Authorization Status:** **CONSUMED AND VOID. No live retry is permitted without fresh owner authorization.**

Sealed results and cryptographic receipts are permanently recorded in:
`benchmarks/matched-pair-harness/results/pilot_02/`

---

## 2. Root Cause Analysis: Provider Rate Limit Rejection

Execution of `treatment_B` halted at Turn 0 before any agent inference occurred.

### 2.1 Error Event Manifest
From `treatment_B/events.jsonl`:
```json
{"type":"thread.started","thread_id":"01a1a5be-b1cb-7641-ae7a-e222c258c868"}
{"type":"item.completed","item":{"id":"item_0","type":"error","message":"Codex is ignoring 1 unrecognized configuration setting. Check for typos or deprecated settings.\n  session-flags: `collaboration_mode` is ignored."}}
{"type":"item.completed","item":{"id":"item_1","type":"error","message":"Codex is ignoring 1 unrecognized configuration setting. Check for typos or deprecated settings.\n  session-flags: `collaboration_mode` is ignored."}}
{"type":"turn.started"}
{"type":"error","message":"You’ve hit your usage limit. Upgrade to Plus to continue using Codex (https://chatgpt.com/explore/plus), or try again at Nov 27th, 2026 5:12 PM."}
{"type":"turn.failed","error":{"message":"You’ve hit your usage limit. Upgrade to Plus to continue using Codex (https://chatgpt.com/explore/plus), or try again at Nov 27th, 2026 5:12 PM."}}
```

### 2.2 Provider Consumption and Token Reality
- **Exit Code:** `1`
- **Error Code / Classification:** `usage_limit_exceeded`
- **Prompt Tokens (Total / Cached / Uncached):** `0 / 0 / 0`
- **Completion Tokens:** `0`
- **Reasoning Tokens:** `0`
- **Total Model Tokens:** `0`
- **Turns Completed:** `0`
- **Model Requests Count:** `0` (attempted initial dispatch rejected at gateway)

The provider subscription tier quota was exhausted prior to the turn. Zero model inference was generated.

---

## 3. Operational Integrity & Protocol Execution

Despite provider-side rejection, the Milestone 5F/5G remediated harness executed with 100% mechanical fidelity:

1. **Clean Worktree Isolation:**
   - Isolated sandbox directory was instantiated in `%TEMP%\engram_pilot\runs\task_commit_cache_invalidation_pilot_02_treatment_B_20261104_141754`.
   - Primary working tree and golden repository remained strictly read-only and unaltered.
2. **Turn-0 Context Compilation Succeeded:**
   - Candidate Engram binary executed `engram inject -C <sandbox> ...` successfully.
   - Compiled AST symbol index directives and `src/main.rs` excerpts into `.github/copilot-instructions.md`.
   - Inspection of rollout session `rollout-2026-11-04T14-18-22-01a1a5be-b1cb-7641-ae7a-e222c258c868.jsonl` lines 10–11 confirms that the `<!-- BEGIN ENGRAM MANAGED BLOCK -->` was fully compiled and ingested into the Turn-0 initial prompt payload.
3. **Stop-on-Failure Safeguard:**
   - As `treatment_B` failed operationally ($C = 0.00$), the orchestrator cleanly halted execution.
   - `control_A` was skipped in accordance with protocol.
   - The harness did not waste further resources or emit false parity claims.
4. **Clean Reporting and Suppressed Savings:**
   - `matched_pair_report.md` recorded:
     - `control_A`: `SKIPPED`, requirements `NOT_EVALUATED`
     - `treatment_B`: `FAIL` ($C = 0.00$)
     - Verdict: `TREATMENT_FAILED_CONTROL_SKIPPED`
     - Comparative Savings: `INVALIDATED (C < 1.0)`, suppressed to `null`.
   - No invalid token claims were generated.

---

## 4. Scientific Verdict & Unverified Boundaries

1. **Token Savings Inconclusive:**
   Because zero inference turns executed, no comparative token savings or efficiency conclusion can be drawn from Pilot 02.
2. **MCP Tool Discovery & Exposure Unverified:**
   While `.codex/config.toml` was generated inside the sandbox, the model never entered Turn 1. Consequently, runtime MCP discovery, tool declaration visibility, and actual agent tool invocation remain unverified in live interaction.
3. **Agent Reliance on Excerpts Unverified:**
   Whether the agent would have relied on Turn-0 AST symbol excerpts or reverted to brute-force file dumping remains untested.

---

## 5. Artifact & Integrity Reference

The sealed evidence vault for Pilot 02 is located at:
`benchmarks/matched-pair-harness/results/pilot_02/`

- `matched_pair_summary.json` (SHA-256: `65DF65BC203B77ED12420B44AE12D4C3C3CE6C6DF96497B08B570A2D0C547846`)
- `matched_pair_report.md` (SHA-256: `209A7ED7D3C60B44C3D010EDFE9EF8CBA90998B44A7389B0EE324B3E0C10148E`)
- `treatment_B/run_telemetry.json` (SHA-256: `CF921A39CE4EE74F5583CD99DB3922BA442F9FA7ACCE56C714CF09489BD4488A`)
- `treatment_B/events.jsonl` (SHA-256: `576EF0FEEB446757659E7A31FD0344498305F9A6E9B5FD5E973CD945BFCE6EFE`)
- `control_A/run_telemetry.json` (SHA-256: `9456B4582C0EAEFBC5A1FFDAA8F6D649429FE8F4BA79F32470765EEA46C449E2`)
- `SHA256SUMS` (Sealed manifest)

Pilot 02 is formally **CLOSED**.
