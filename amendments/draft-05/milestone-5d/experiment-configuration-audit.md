# Experiment Configuration Audit: N=1 Matched-Pair Pilot

**Milestone:** Engram Amendment 05 — Milestone 5D  
**Document ID:** `amendments/draft-05/milestone-5d/experiment-configuration-audit.md`  
**Evaluation Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Target Pair:** `pilot_01` ($N=1$)  
**Status:** CONFIGURATION AUDITED & SYMMETRY VERIFIED  

---

## 1. Executive Summary

This audit evaluates the parameter configuration and execution symmetry of the planned $N=1$ matched-pair experiment (`pilot_01`) against `benchmarks/matched-pair-harness/src/orchestrator.py`, `runner.py`, and `task_commit_cache_invalidation.json`.

The audit verifies that all experimental settings conform to the immutable specification, that `control_A` is genuinely unassisted, that `treatment_B` receives context solely through verified Engram MCP tooling, and that both arms operate under identical environment parameters.

---

## 2. Experimental Parameters Audit

| Parameter | Specified Value | CLI Parser Argument | Runtime Value in `Orchestrator` | Verification Status |
|---|---|---|---|---|
| **Execution Mode** | `pair` | `--mode pair` | `self.mode = "pair"` | **VERIFIED** |
| **Task ID** | `task_commit_cache_invalidation` | `--task task_commit_cache_invalidation` | `self.task_id = "task_commit_cache_invalidation"` | **VERIFIED** |
| **Pair Identifier** | `pilot_01` | `--pair pilot_01` | `self.pair_id = "pilot_01"` | **VERIFIED** |
| **Model** | `gpt-5.6-luna` | `--model "gpt-5.6-luna"` | `self.model = "gpt-5.6-luna"` | **VERIFIED** |
| **Reasoning Effort** | `medium` | `--reasoning "medium"` | `self.reasoning_effort = "medium"` | **VERIFIED** |
| **Nominal Ceiling** | `175,000` tokens | `--max-tokens-ceiling 175000` | `self.max_tokens_ceiling = 175000` | **VERIFIED** |
| **Headroom Reserve** | `45,000` tokens | `--headroom-reserve 45000` | `self.headroom_reserve = 45000` | **VERIFIED** |
| **Stop Threshold** | `130,000` tokens | Derived ($175\text{k} - 45\text{k}$) | $175,000 - 45,000 = 130,000$ | **VERIFIED** |
| **Dependency Strategy** | `none` | `--dependency-strategy "none"` | `self.dependency_strategy = "none"` | **VERIFIED** |
| **Candidate SHA-256** | `F56347EF...` | `--expected-treatment-sha256 "F56347EF..."` | `self.expected_treatment_sha256 = "F563..."` | **VERIFIED** |
| **Output Vault** | `reports/runs/pilot_01` | `--output-dir "reports/runs/pilot_01"` | `self.output_dir = Path("...pilot_01")` | **VERIFIED** |

---

## 3. Arm Symmetry & Context Isolation Audit

### 3.1 Control A: Genuinely Unassisted Baseline
- **Zero Binary Linkage:** `self.arm_binaries["control_A"] = None`.
- **Zero MCP Registration:** `is_engram_arm` evaluates to `False`. The arguments `-c mcp_servers.engram...` are omitted from the process invocation.
- **Zero Turn-0 Injection:** `turn0_injected` is `False`. No `.github/copilot-instructions.md` file is created or read.
- **Verbatim Prompt:** The initial prompt from `task_commit_cache_invalidation.json` is passed directly to Codex without modification or augmentation.
- **Standard Capabilities:** The agent relies exclusively on standard built-in Codex tools (file search, view, edit, bash execution).

### 3.2 Treatment B: Candidate Engram Assisted
- **Binary Binding:** Uses Candidate Phase 1 binary (`target/release/engram.exe`), pre-verified via SHA-256.
- **MCP Server Configuration:** Registered as a local stdio MCP server:
  - Command: `target/release/engram.exe`
  - Arguments: `-C <sandbox_path> mcp`
  - Approval Mode: `approve`
  - Enabled Tools: `engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight`.
- **Dynamic Context Only:** The treatment agent receives no pre-seeded manual context packages. Context is retrieved dynamically on-demand when the agent invokes `engram_get_context` or `engram_inspect_symbol`.
- **Prompt Parity:** Initial prompt is identical to Control A.

---

## 4. Common Environmental Controls

Both arms operate under strictly matched environment parameters:
1. **Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`.
2. **Pinned Baseline Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f` on branch `feature/async-precommit-guard`.
3. **Workspace Isolation:** Completely detached Git worktrees created under `%TEMP%\engram_pilot\runs`.
4. **Compilation Sandbox:** Each arm creates its own private `target/` directory with zero cross-arm caching.
5. **Evaluation Engine:** Identical Cargo gates (L0 `cargo check`, L2 `cargo test`, L3 `cargo build`) and requirement evaluation.
