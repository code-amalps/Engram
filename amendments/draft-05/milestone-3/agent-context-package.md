# Engram Amendment 05 — Milestone 3: Agent-Ready Engineering Context Package

**Document Version:** 1.0.0  
**Date:** 2026-10-30  
**Target Task:** Implement Subpath- and Language-Scoped Query Routing in Engram Context Compiler and Prompt Generator  
**Context Engine:** Candidate Phase 1 Binary (`engram 1.5.4`)  
**Status:** Generated & Formatted for Agent Handoff

---

## 1. Information Provenance & Classification Schema

To prevent AI coding agents from conflating verified facts with heuristic assumptions, all information in this package is strictly categorized into four provenance tiers:

| Tier Symbol | Provenance Category | Definition |
|:---:|:---|:---|
| `[FACT]` | **Retrieved Repository Fact** | Directly extracted from SQLite WAL AST symbol database or raw source code files. Verified line numbers, method signatures, and file paths. |
| `[ENGRAM]` | **Engram-Generated Analysis** | Outputs computed by Engram algorithms (BM25 relevance scores, AST token estimations, reverse dependency blast radius trees, ecosystem package manifests). |
| `[HUMAN]` | **Human-Authored Requirement** | Task objectives, boundary constraints, and acceptance criteria specified by the engineer. |
| `[ASSUMPTION]` | **Unverified Assumption** | Hypotheses regarding secondary effects or optional extensions that require empirical test verification. |

---

## 2. Agent Engineering Handoff

### 2.1 Task Objective
- `[HUMAN]` **Goal:** Add optional `--subpath <PATH>` and `--language <LANG>` filter flags to `engram context` and `engram prompt` to allow developers and agents to scope repository candidate retrieval to a specific directory subtree or programming language.
- `[HUMAN]` **Target Architecture:** Monorepo and polyglot repository support (e.g. isolating Rust `src/` from Python `benchmarks/matched-pair-harness/`).

### 2.2 Target Files & Line References
- `[FACT]` **CLI Entry Point & Argument Parsing:**  
  [src/main.rs](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/main.rs#L56-L135)
  - Line 56: `struct Cli`
  - Line 69: `enum CommandLine`
  - Line 75: `Commands::Context { task, token_budget, explain, json }` $\rightarrow$ Needs `subpath: Option<String>` and `language: Option<String>`.
  - Line 115: `Commands::Prompt { task, token_budget, format, json, stdout, no_clipboard, lock_contract }` $\rightarrow$ Needs `subpath: Option<String>` and `language: Option<String>`.
  - Lines 490–520: CLI command dispatch handlers forwarding arguments to `compiler::context` and `prompt::generate_prompt`.

- `[FACT]` **Context Compilation & SQL Querying:**  
  [src/compiler.rs](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/compiler.rs#L45-L80)
  - Line 45: `pub fn context(root: &Path, task: &str, budget: usize) -> Result<ContextResult>` $\rightarrow$ Needs signature update to accept `subpath: Option<&str>` and `language: Option<&str>`.
  - Lines 60–66: Current unconditional SQL query:
    ```rust
    let mut s = conn.prepare("SELECT path, language, excerpt FROM files")?;
    ```
    Needs scoped candidate selection:
    ```rust
    // Candidate filtering by subpath prefix and language identifier
    ```
  - Lines 18–43: `ContextResult` struct $\rightarrow$ Optionally record active `subpath_filter` and `language_filter`.

- `[FACT]` **Prompt Generator Parameter Routing:**  
  [src/prompt.rs](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/prompt.rs#L563-L600)
  - Line 563: `pub fn generate_prompt(root: &Path, task: &str, budget: usize, format: &str, lock_contract: bool) -> Result<String>` $\rightarrow$ Forward `subpath` and `language` to `compiler::context`.
  - Line 713: `pub fn prepare_task_context(...)` $\rightarrow$ Forward scoping parameters.

### 2.3 Direct & Reverse Dependencies
- `[ENGRAM]` **Direct Dependencies of Target Modules:**
  - `src/compiler.rs` imports: `crate::analyzer` (AST token estimation), `crate::constitution` (rules), `crate::manifest` (ecosystem guard), `crate::storage` (`db`, `root_path`).
  - `src/prompt.rs` imports: `crate::compiler::context`, `crate::flywheel`.
  - `src/main.rs` dispatches into: `compiler`, `prompt`, `mcp`, `contract`.
- `[ENGRAM]` **Reverse Dependency Blast Radius:**
  - `compiler.rs` is imported by: `src/main.rs`, `src/prompt.rs`, `src/mcp.rs` (`engram_get_context`).
  - Modifying `pub fn context` signature requires updating `src/mcp.rs:381` (`context(&root, task, budget)?`) to pass `None, None`.

### 2.4 Existing Tests & Regression Suites
- `[FACT]` **Target Unit Tests:**
  - `tests/compiler_test.rs`: Unit tests for BM25 ranking, graph expansion, and budget allocation.
  - `tests/prompt_test.rs`: Turn-0 prompt structure, ecosystem guard formatting, and token budgeting.
  - `tests/mcp_consolidation_test.rs`: MCP stdio tools regression suite.

### 2.5 Architectural Constraints & Ecosystem Guard
- `[ENGRAM]` **Ecosystem Package Constraints (from `Cargo.toml`):**
  Only the 19 declared packages are permitted: `anyhow@1.0.104`, `chrono@0.4.45`, `clap@4.6.7`, `ignore@0.4.33`, `regex@1.13.1`, `rusqlite@0.32.1`, `serde@1.0.229`, `serde_json@1.0.151`, `serde_yaml@0.9.34`, `sha2@0.10.9`, `tree-sitter@0.25.10`, `uuid@1.27.0`, and language grammars.
- `[HUMAN]` **Forbidden Boundary:**
  Do NOT modify `benchmarks/matched-pair-harness/bin/`, `benchmarks/AMENDMENT*`, or frozen benchmark reports.

### 2.6 Acceptance Criteria
- `[HUMAN]` **REQ-1:** `engram context --subpath src/ "<task>"` evaluates only files residing within `src/`.
- `[HUMAN]` **REQ-2:** `engram context --language python "<task>"` evaluates only files where `language == "python"`.
- `[HUMAN]` **REQ-3:** When neither filter is passed, behavior is 100% backwards-compatible with existing whole-repo candidate scanning.
- `[HUMAN]` **REQ-4:** `cargo test --offline` passes 100% across all unit and integration test suites.

---

## 3. Comparative Context Budget Analysis

Engram was evaluated under two offline context budgets (1,200 tokens vs. 4,000 tokens) for this exact task:

| Evaluation Metric | Budget: 1,200 Tokens | Budget: 4,000 Tokens | Comparative Assessment |
|:---|:---:|:---:|:---|
| **Estimated Package Tokens** | 1,200 tokens | 4,000 tokens | Both budgets packed up to their configured limit. |
| **Files Selected** | 1 file (`src/main.rs`) | 1 file (`tests/empirical_live_test.rs`) | **Critical divergence:** Budget expansion caused BM25 to favor a massive test mock file. |
| **BM25 Relevance Score** | 751.87 | 801.10 | Test mock scored higher due to dense keyword repetition. |
| **AST Symbols Extracted** | 35 symbols (`Cli`, `CommandLine`, modules) | 58 symbols (mock test structures) | 1,200-token package provided better architectural orientation. |
| **Core Source Coverage** | Partial (`main.rs` included, `compiler.rs` omitted) | Poor (test mock included, `compiler.rs` omitted) | Demonstrates the exact need for the proposed `--subpath` feature! |
| **Omission Signals** | `tests/empirical_live_test.rs` (+3,391 tokens overage) | None (consumed full budget) | Engram accurately signaled the budget ceiling breach in 1,200 mode. |
| **Explainability** | High (Clear CLI dispatch context) | Low (Distracted by live test framework) | 1,200 budget was more actionable for an implementer. |

---

## 4. Context Size Reduction vs. Inference Token Savings

> [!NOTE]
> **Measurement Qualification:**  
> The 1,200-token context package represents an **88.6% reduction in input context size** compared to reading the full source files (`main.rs` + `compiler.rs` + `prompt.rs` = ~10,500 tokens).  
> 
> However, **context size reduction is not identical to total inference token savings**. In an interactive agent session, total token spend includes cached prefix tokens, tool call responses, reasoning tokens, and multi-turn exchanges. True token efficiency can only be verified through authorized live paired benchmarking.
