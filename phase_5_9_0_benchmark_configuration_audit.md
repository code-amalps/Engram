# Engram Phase 5.9.0: Benchmark Configuration Audit

**Execution Phase:** 5.9.0  
**Target Systems:** Five-Arm Pentad Benchmark Harness (`orchestrator.py`, `runner.py`, `isolation.py`, `evaluator.py`)  
**Execution Mode:** READ-ONLY OFFLINE AUDIT  

---

## 1. Executive Summary

This document establishes the exact, verified configuration parameters for the proposed Phase 5.9 live five-arm benchmark across Task A and Task B.

---

## 2. Configuration Parameter Specification Matrix

| Parameter # | Dimension | Concrete Specification | Source / Verification Path |
| :---: | :--- | :--- | :--- |
| **1** | **Provider Name** | `cloud` | `SUPPORTED_PROVIDERS` in `runner.py` (line 476) |
| **2** | **Model Identifier** | `gpt-5.6-luna` | Default model in `orchestrator.py` & `runner.py` (line 485) |
| **3** | **Model Version** | Upstream remote provider build | Exposed via Codex API session header |
| **4** | **Reasoning Level** | `medium` | Passed via CLI flag `-c collaboration_mode.settings.reasoning_effort="medium"` |
| **5** | **Execution Environment** | Sandboxed Windows 11 host | Sandboxes provisioned under `Temp/engram_pilot/runs/` |
| **6** | **Sampling Parameters** | Deterministic greedy decoding | Governed by Codex runner defaults (`temperature=0.0`) |
| **7** | **Tool Configuration** | Dynamic per arm | Arm A/D: 0 tools (Bypass); Arms B/C/E: 2 active tools (`engram_query`, `engram_inspect_symbol`) |
| **8** | **MCP Configuration** | JSON-RPC 2.0 stdio subprocess | Automated registration via Codex `-c mcp_servers.engram` flags |
| **9** | **Dependency Strategy** | `copy` | Full write isolation from `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` |
| **10** | **Per-Arm Timeout** | `600 seconds` (10 minutes) | Hard process-tree kill enforced by `runner.py` line 967 |
| **11** | **Per-Turn Timeout** | Sub-turn polling (<50 ms) | Active external reader thread monitoring stdout line boundaries |
| **12** | **Retry Policy** | Zero auto-retries (`retries=0`) | Fail-closed; each task-arm run has exactly 1 execution attempt |
| **13** | **Maximum Turns** | Bounded by token ceiling | Enforced by pre-dispatch headroom guard |
| **14** | **Per-Arm Token Budget** | Task A: 250,000; Task B: 450,000–550,000 | Configurable via `--max-tokens-ceiling` |
| **15** | **Global Token Budget** | Reconciled Scenario Ceiling | Configurable via `--global-tokens-ceiling` |
| **16** | **Telemetry Collection** | Real-time JSONL event stream | Line-by-line parsing of `item.completed` & `turn.completed` usage |
| **17** | **Candidate Binaries** | Cand 3, Cand 4.2, Cand 5 | Pre-registered in `isolation.py` & verified before dispatch |
| **18** | **Task Fixtures** | Task A & Task B JSON configs | Cryptographically hashed in `config/tasks/` |

---

## 3. Cryptographic Artifact Integrity Verification

### 3.1 Candidate Executables

| Candidate | Target File Path | Verified SHA-256 Hash |
| :--- | :--- | :--- |
| **Candidate 3** | `benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe` | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` |
| **Candidate 4.2** | `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe`| `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` |
| **Candidate 5** | `benchmarks/matched-pair-harness/bin/candidate-phase5/engram.exe` | `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` |

### 3.2 Task Fixtures

| Task | Fixture File Path | Verified SHA-256 Hash |
| :--- | :--- | :--- |
| **Task A (`task_a_typo`)** | `benchmarks/matched-pair-harness/config/tasks/task_a_typo.json` | `357D9B4B5777A41A8C5691475DCA8DDAC00134C480C6324E26B60FEA39F95F02` |
| **Task B (`task_b_remember_me`)** | `benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json` | `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` |

---

## 4. Harness CLI Capability Finding

The existing orchestrator CLI (`orchestrator.py`) supports all required parameters (`--mode pentad`, `--provider`, `--model`, `--reasoning`, `--dependency-strategy`, `--max-tokens-ceiling`, `--headroom-reserve`, `--global-tokens-ceiling`, `--execution-policy`). 

Because `--task` accepts one task per CLI invocation, evaluating Task A and Task B across 5 arms is executed via **two consecutive orchestrator CLI invocations** under isolated output directories.
