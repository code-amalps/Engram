# Engram Amendment 05 — Milestone 2D: Track B Engram Self-Hosting Capability Report

**Document Version:** 1.0.0  
**Date:** 2026-10-30  
**Target Executable:** Candidate Phase 1 Binary  
**Classification:** `PARTIALLY CAPABLE — LIMITATIONS IDENTIFIED`

---

## 1. Executive Summary

This report evaluates whether the existing Candidate Phase 1 Engram executable can analyze, understand, and assist in the development of its own codebase.

Using a clean, disposable analysis workspace (`engram_self_hosting_2d`) containing an isolated snapshot of the current Engram repository (including Rust source code and the Python matched-pair harness), the candidate binary was evaluated strictly offline across 8 core capability domains.

The evaluation concludes that Engram is **`PARTIALLY CAPABLE — LIMITATIONS IDENTIFIED`**:
- **Strong Capabilities:** Deterministic offline repository indexing, multi-language tree-sitter AST parsing (Rust and Python), reverse dependency blast radius extraction, symbol discovery, and Turn-0 structured prompt generation.
- **Key Limitations:** `engram contract check` and `validate-local` require an active local `.git` repository commit history to compute diffs; without Git metadata in disposable sandboxes, contract validation fails closed. Furthermore, semantic token budget allocation can suffer from lexical ambiguity between the Rust internal benchmark runner (`empirical_runner.rs`) and the Python matched-pair harness (`orchestrator.py`).

---

## 2. Candidate Binary Identity & Command Inventory

### 2.1 Cryptographic & Version Verification
- **Binary Path:** `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`
- **Reported CLI Version:** `engram 1.5.4`
- **Expected SHA-256:** `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`
- **Verified SHA-256:** `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` (100% Match)

### 2.2 Supported Command Inventory
Introspection of `engram.exe --help` identified 29 subcommands:
1. `init`: Initialize `.engram` configuration and rules in repository.
2. `analyze`: Analyze repository structure, symbols, and dependencies (with `--incremental` support).
3. `context`: Compile minimum sufficient context for a given task (`--token-budget`, `--explain`, `--json`).
4. `analyze-task`: Ground a task, detect gaps, and generate task records.
5. `contract`: Manage elastic Change Contracts (`create`, `check`, `ratify`, `reconcile`, `install-hook`, `uninstall-hook`).
6. `memory`: Manage durable engineering memory (`.engram/memory.yaml`).
7. `history`: Mine and query Git historical co-change patterns.
8. `impact`: Calculate change impact or query reverse dependencies for a symbol.
9. `inspect`: Deeply inspect symbol change impact, causal provenance, active contract status, and memories.
10. `validate`: Validate working-tree changes against architectural rules.
11. `decision`: Record a developer decision or rule override with rationale.
12. `update-memory`: Promote accepted task knowledge into local engineering memory.
13. `ecosystem`: Inspect declared and resolved ecosystem packages and scripts.
14. `setup`: Configure and inspect editor and MCP integrations.
15. `prepare`: Prepare task-aware context, lock preflight contract to baseline commit, and emit Turn-0 prompt.
16. `inject`: Inject task-aware Turn-0 context and contract directives into workspace instruction files.
17. `prompt`: Generate structured prompt context with AST symbols, boundaries, and completeness tracking.
18. `mcp`: Start the Model Context Protocol (MCP) server over stdio (`--all-tools`).
19. `bench`: Run scientific empirical benchmarks evaluating discovery, grounding, contracts, and memory lift.
20. `why`: Architectural archaeology: explain rationale, governing decisions, and commit history for a symbol.
21. `doctor`: Inspect memory health, scope resolution, and cryptographic audit chains.
22. `gc`: Controlled garbage collection with dry-run inspection.
23. `preflight`: Orchestrated Pre-Flight Handshake: compile context, predict blast radius, lock contract.
24. `postflight`: Orchestrated Post-Flight Guard: evaluate mutation against contract and constitution.
25. `validate-local`: Local deterministic postflight validation.
26. `recover`: Formulate structured recovery repair plan for a contract violation.
27. `drift`: Analyze architectural decision drift, boundary erosion, and scoped staleness.
28. `guard`: Policy-driven Change Guard for local Git and CI pull-request workflows.
29. `hook`: Manage Git pre-commit hooks.

---

## 3. Capability Matrix

| Capability Domain | Implemented? | Offline Verified? | Pass / Qualify / Fail | Empirical Verification Evidence |
|:---|:---:|:---:|:---:|:---|
| **Repository Indexing** | YES | YES | **PASS** | Indexed 160 files (Rust + Python), extracted 2,334 active AST symbols into SQLite WAL database in 1.4s. |
| **Symbol Discovery** | YES | YES | **PASS** | Parsed syntax trees for Rust (`tree-sitter-rust`) and Python (`tree-sitter-python`). Extracted functions, classes, enums, structs, impl blocks. |
| **Context Compilation** | YES | YES | **PASS** | `engram context` and `engram prompt` scored relevance via BM25 + graph traversal; assembled 1,200 and 4,000 token packages. |
| **Dependency Analysis** | YES | YES | **PASS** | `engram inspect Orchestrator` mapped 1-hop and 2-hop reverse dependencies across 14 modules and 12 test suites. |
| **Change-Impact Analysis** | YES | YES | **PASS** | Identified direct and transitive blast radius for `Orchestrator` and `CodexRunner` without running live agent executions. |
| **Change Contracts** | YES | PARTIAL | **QUALIFIED** | `contract create` produced elastic boundary definition; `contract check` failed (exit code 4) due to missing `.git` history in clean sandbox. |
| **Architectural Validation** | YES | PARTIAL | **QUALIFIED** | `validate` and `contract check` are tightly coupled to Git commit references; non-Git workspaces cannot verify diffs. |
| **MCP Integration** | YES | YES | **PASS** | Verified stdio MCP server architecture supporting 4 lean default tools (`prepare`, `inspect`, `preflight`, `postflight`) and 16 specialist tools via `--all-tools`. |

---

## 4. Self-Repository Analysis (The 10 Key Assessment Questions)

### Q1: Can Engram index its own repository?
**YES.** When pointed at the disposable workspace containing Engram's Rust codebase (`src/`, `tests/`) and Python harness (`benchmarks/matched-pair-harness/`), `engram analyze` completed successfully:
```text
Indexed Files:  160
Active Symbols: 2,334
Storage:        SQLite WAL database in %LOCALAPPDATA%\Engram\cache\<project_id>\derived_symbols.db
Execution Time: 1.4 seconds
```

### Q2: Can it identify its core architecture?
**YES.** Engram parsed the modular layout from `.engram/config.yaml` and identified major architectural layers:
- CLI command dispatcher (`src/main.rs`).
- MCP protocol server (`src/mcp.rs`).
- AST analysis and tree-sitter bindings (`src/analyzer.rs`).
- Elastic Change Contract engine (`src/contract.rs`).
- CI & Pre-commit guard policies (`src/guard.rs`, `src/ci_guard.rs`).
- Python matched-pair harness components (`orchestrator.py`, `runner.py`, `evaluator.py`, `isolation.py`, `telemetry.py`).

### Q3: Can it discover relationships between modules?
**YES.** Through `engram inspect`, Engram mapped import edges:
- `orchestrator.py` imports `runner.py`, `evaluator.py`, `isolation.py`, `telemetry.py`.
- `main.rs` dispatches into `analyzer.rs`, `compiler.rs`, `contract.rs`, `mcp.rs`, `prompt.rs`.

### Q4: Can it locate important symbols?
**YES.** Engram correctly indexed and retrieved:
- `Orchestrator` (`benchmarks/matched-pair-harness/src/orchestrator.py:65`)
- `CodexRunner` (`benchmarks/matched-pair-harness/src/runner.py:461`)
- `StreamingTokenTracker` (`benchmarks/matched-pair-harness/src/runner.py:77`)
- `MultiArmOrchestrator` (`src/empirical_runner.rs:817`)
- `Sandbox` (`src/empirical_runner.rs:234`)
- `RunnerSecurityError` (`src/empirical_runner.rs:51`)

### Q5: Can it identify relevant source files for a requested change?
**YES.** For the query *"Explain the execution architecture of Engram's matched-pair harness and identify the minimum set of files involved in adding a new execution mode"*, Engram surfaced:
1. `benchmarks/matched-pair-harness/tests/test_milestone_2c_remediation.py` (Score: 659.89)
2. `src/empirical_runner.rs` (Score: 695.42)
3. `benchmarks/matched-pair-harness/src/orchestrator.py`

### Q6: Can it identify dependency relationships?
**YES.** Reverse dependency queries for `Orchestrator` identified that modifying it impacts:
- All test suites in `benchmarks/matched-pair-harness/tests/` (12 test modules).
- Evaluator and Runner modules via shared type definitions and execution telemetry schemas.

### Q7: Can it explain the likely impact of changing a component?
**YES.** `engram inspect Orchestrator` produced a comprehensive multi-hop blast radius tree distinguishing:
- **Direct Impact (Hop 0):** `orchestrator.py`.
- **First-Order Dependencies (Hop 1):** `evaluator`, `isolation`, `runner`, `telemetry`.
- **Second-Order Dependencies (Hop 2):** Dependent test harnesses and benchmark execution scripts.

### Q8: Can it produce a useful context package for a coding agent?
**YES.** `engram prompt --stdout --no-clipboard` generated a clean Turn-0 context package containing:
- Structured task objective.
- Relevant AST symbol outlines with line numbers.
- Source code excerpts.
- Package ecosystem constraints from `Cargo.toml`.
- Token budget accounting (1,200 tokens allocated; overage files identified).

### Q9: Can it detect missing or incomplete context?
**YES.** In the prompt output, Engram reported:
```text
Status: PARTIAL_GROUNDING (1 files omitted due to budget)
Omitted Files due to budget ceiling:
  - src/empirical_runner.rs: Budget limit exceeded (+1,154 tokens would exceed 1,200)
```
This demonstrates active budget awareness and prevents context truncation.

### Q10: Can it validate proposed changes through existing change-contract functionality?
**PARTIALLY.** 
- `engram contract create` successfully created an elastic Change Contract specifying expected modules, target files, and forbidden boundaries.
- However, `engram contract check` failed closed with exit code 4:
  `✗ Baseline commit 'WORKTREE' is unavailable in current Git history (shallow clone or missing commit).`
  Because the disposable workspace did not include a `.git` folder, the contract checker could not compute diffs. Contract validation requires an active Git commit graph.

---

## 5. Identified Limitations & Technical Gaps

1. **Tight Coupling to Git Commit History:**  
   Contract checking (`contract check`) and local validation (`validate-local`) rely directly on Git CLI commands (`git diff`, `git rev-parse`). Disposable analysis environments created without `.git` cannot execute contract validation.
2. **Lexical Ambiguity in Dual-Language Repositories:**  
   Because Engram contains both a Rust empirical benchmark runner (`src/empirical_runner.rs`) and a Python matched-pair harness (`benchmarks/matched-pair-harness/src/orchestrator.py`), generic queries like *"benchmark orchestrator execution mode"* score both files highly. Additional prompt engineering or sub-path scoping (`-C benchmarks/matched-pair-harness`) is required to prevent cross-language context pollution.
3. **Configuration Language Declaration:**  
   The default `.engram/config.yaml` did not originally list `rust` in `languages`. Adding `rust` enabled full multi-language indexing, confirming that the binary natively contains `tree-sitter-rust`.
