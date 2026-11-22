# Engram Amendment 05 — Milestone 3: Execution and Verification Report

**Document Version:** 1.0.0  
**Date:** 2026-10-30  
**Phase:** Milestone 3 — Self-Hosting Engineering Loop  
**Mode:** Strictly Offline (Zero Model Inference, Zero Network, Zero Spend)  
**Classification:** `COMPLETE — OFFLINE SELF-HOSTING WORKFLOW DEMONSTRATED`

---

## 1. Executive Summary & Compliance Certification

Milestone 3 established an evidence-backed self-hosting engineering workflow in which Engram is utilized to analyze, index, investigate, and plan architectural improvements to its own repository.

All activities in this milestone complied strictly with offline safety boundaries:
1. **Zero Inference / External Model Usage:** Exactly 0 LLM/SLM inference requests, 0 API calls, 0 network sockets opened, and **$0.00 spent**.
2. **Zero Live Agent Invocations:** Neither Codex CLI nor any external coding agent was executed.
3. **Protected Binaries Intact:** The Candidate Phase 1 binary (`engram.exe`), frozen v1.5.4 binary, and frozen v1.5.1 binary were verified bit-for-bit against their authoritative SHA-256 hashes. Zero binaries were recompiled or replaced.
4. **Golden Repository Preservation:** The golden repository at `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` was strictly excluded and unaccessed.
5. **Engram Working Tree Preservation:** The primary repository at `C:\Users\Amal P S\OneDrive\Projects\Engram` remains anchored at commit `c70845b4d5913457f1b176fb0370b80dd792711f` on branch `feature/async-precommit-guard`. All pre-existing user modifications and historical benchmark artifacts were preserved bit-for-bit.

---

## 2. Pre-Execution Working-Tree Baseline

- **Repository Path:** `C:\Users\Amal P S\OneDrive\Projects\Engram`
- **Active Branch:** `feature/async-precommit-guard`
- **Current Commit HEAD:** `c70845b4d5913457f1b176fb0370b80dd792711f`
- **Tracked Modifications:** Exactly 13 files (preserved from Milestones 1, 1A, and 2C):
  - `benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json`
  - `benchmarks/matched-pair-harness/schemas/correctness_checklist.schema.json`
  - `benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json`
  - `benchmarks/matched-pair-harness/src/evaluator.py`
  - `benchmarks/matched-pair-harness/src/isolation.py`
  - `benchmarks/matched-pair-harness/src/orchestrator.py`
  - `benchmarks/matched-pair-harness/src/runner.py`
  - `benchmarks/matched-pair-harness/src/telemetry.py`
  - `src/instructions.rs`
  - `src/main.rs`
  - `src/mcp.rs`
  - `tests/mcp_consolidation_test.rs`
  - `tests/turn0_injection_test.rs`
- **Git Operation Boundary:** Zero commits, pushes, tags, checkouts, stashes, resets, or rebases were performed.

---

## 3. Candidate Binary Cryptographic Verification

The Candidate Phase 1 executable was verified using SHA-256 immediately before any execution:

| Executable | Path | Authoritative Expected SHA-256 | Verified SHA-256 | Status |
|:---|:---|:---|:---|:---:|
| **Candidate Phase 1** | `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | **100% MATCH** |
| **Frozen v1.5.4** | `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | **100% MATCH** |
| **Frozen v1.5.1** | `benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | **100% MATCH** |

The candidate executable reported CLI version `engram 1.5.4`.

---

## 4. Disposable Analysis Workspace Specification

To protect the host repository and prevent accidental metadata contamination, a disposable snapshot workspace was constructed under the Windows temporary directory:

- **Snapshot Path:** `C:\Users\AMALPS~1\AppData\Local\Temp\engram_milestone3_snapshot`
- **Composition:** Exactly 241 files across 94 directories.
- **Inclusions:**
  1. `src/`: Complete Rust source tree (36 modules, including all uncommitted changes).
  2. `tests/`: Rust test suites (unit, integration, adversarial).
  3. `Cargo.toml` & `Cargo.lock`: Full dependency manifest and locked versions.
  4. `README.md` & `LICENSE`: Root project documentation.
  5. `benchmarks/matched-pair-harness/src/`: Python harness modules (`orchestrator.py`, `runner.py`, `evaluator.py`, `isolation.py`, `telemetry.py`).
  6. `benchmarks/matched-pair-harness/config/`: Task JSON definitions (`smoke_test_read_only.json`, `task_b_remember_me.json`).
  7. `benchmarks/matched-pair-harness/schemas/`: Correctness checklist, telemetry, and triad JSON schemas.
  8. `benchmarks/matched-pair-harness/tests/`: Offline test suites (249 tests).
  9. `.engram/`: Configuration files (`config.yaml` with Rust/Python languages declared, `constitution.yaml`, `memory.yaml`).
- **Exclusions:**
  - `.git` administrative directory.
  - All `.env*` files, tokens, and credentials.
  - Build targets (`target/`, `.pytest_cache/`, `dist/`).
  - Historical benchmark run reports (`benchmarks/matched-pair-harness/reports/runs/`).
  - Standalone zipped release bundles and binary executables.
  - The golden Angular repository.

---

## 5. Track A: Activity Log & Empirical Command Evidence

All fifteen Track A self-analysis operations were executed against the disposable snapshot. Timings, exit codes, and outputs were recorded deterministically:

| Operation ID | Command Invocation | Exit Code | Elapsed Time | Observed Output & Status | Independent Verification |
|:---|:---|:---:|:---:|:---|:---:|
| **1. Indexing (Clean)** | `engram analyze -C <ws> --json` | `0` | 24.928s | Indexed 160 files (Rust & Python), extracted 2,334 active AST symbols into SQLite WAL database in `%LOCALAPPDATA%\Engram\cache\`. | **CONFIRMED** |
| **1b. Indexing (Incremental)** | `engram analyze -C <ws> --incremental --json` | `0` | 1.221s | Invalidation engine detected 0 modified files; 0 reparsed; completed in 1.2s (**20x speedup**). | **CONFIRMED** |
| **2. Symbol & Blast Radius** | `engram inspect -C <ws> Orchestrator --json` | `0` | 0.196s | Resolved target to `orchestrator.py`; mapped 1-hop imports (14 modules) and 2-hop dependents (12 test suites). | **CONFIRMED** |
| **3. Impact Query (CodexRunner)** | `engram impact -C <ws> CodexRunner --json` | `0` | 0.152s | Resolved target symbol to `runner.py`; reported direct entity. | **CONFIRMED** |
| **4. Impact Query (TokenTracker)** | `engram impact -C <ws> StreamingTokenTracker --json` | `0` | 0.168s | Resolved target symbol to `runner.py`; reported direct entity. | **CONFIRMED** |
| **5a. Context (1,200 Budget)** | `engram context -C <ws> --token-budget 1200 --json <task>` | `0` | 1.490s | Selected `src/main.rs` (score: 751.87); noted `tests/empirical_live_test.rs` excluded (+3,391 tokens overage). | **CONFIRMED** |
| **5b. Context (4,000 Budget)** | `engram context -C <ws> --token-budget 4000 --json <task>` | `0` | 1.449s | Selected `tests/empirical_live_test.rs` (score: 801.10) within expanded budget. | **CONFIRMED** |
| **6a. Prompt (1,200 Budget)** | `engram prompt -C <ws> --token-budget 1200 --stdout --no-clipboard <task>` | `0` | 3.342s | Generated Turn-0 Markdown prompt with AST symbols for `src/main.rs` and ecosystem guard. | **CONFIRMED** |
| **6b. Prompt (4,000 Budget)** | `engram prompt -C <ws> --token-budget 4000 --stdout --no-clipboard <task>` | `0` | 4.923s | Generated expanded Turn-0 prompt package with AST symbols. | **CONFIRMED** |
| **7a. Contract Creation** | `engram contract create -C <ws> m3-add-single-mode --modules ... --files ... --forbidden ... --json` | `0` | 0.159s | Emitted active Change Contract `ctr_d471...` with elastic boundaries. | **CONFIRMED** |
| **7b. Contract Check** | `engram contract check -C <ws> --json` | `4` | 0.265s | **FAIL-CLOSED:** Exited with code 4 (`Baseline commit 'WORKTREE' is unavailable in current Git history`). Confirms Git history dependency. | **CONFIRMED** |
| **7c. Validate Working Tree** | `engram validate -C <ws> m3-add-single-mode --json` | `0` | 0.465s | Advisory validation evaluated contract; identified missing expected changes. | **CONFIRMED** |
| **8a. Archaeology (Why)** | `engram why -C <ws> Orchestrator` | `0` | 0.425s | Resolved `Orchestrator` to `orchestrator.py`; reported active governing policy status. | **CONFIRMED** |
| **8b. Health Diagnostics** | `engram doctor -C <ws>` | `0` | 0.070s | Verified cryptographic audit chain (`EVENT_CHAIN_VALID`); reported memory liveness. | **CONFIRMED** |
| **9a. Ecosystem Intelligence** | `engram ecosystem -C <ws> --json` | `0` | 0.040s | Resolved all 19 declared Rust ecosystem packages from `Cargo.toml`. | **CONFIRMED** |
| **10a. MCP Tool Surface** | `engram mcp --help` | `0` | 0.043s | Verified stdio MCP server command supporting `--all-tools` (16 specialist tools) and 4 lean default tools. | **CONFIRMED** |

---

## 6. Post-Execution Integrity Verification

- Primary working tree status was checked via `git status --short`.
- Exactly 13 modified tracked files remain in the working tree (unmodified from pre-execution state).
- Zero production files in `src/` or `benchmarks/` were altered during Milestone 3.
- All new documentation and test records were strictly quarantined to `amendments/draft-05/milestone-3/` and temporary scratch directories.
