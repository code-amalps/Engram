# Changelog

All notable changes to the Engram project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Important Disclosure on Performance Claims:**  
> Live LLM coding-agent token-reduction and execution-speed improvements have **not** yet been validated through a completed, accepted four-arm live benchmark. Performance metrics reported in synthetic microbenchmarks (`engram bench`) reflect deterministic fixture suites and must not be extrapolated to arbitrary live coding tasks.

---

## [1.5.5] - 2026-11-27

### Engram Stable Release

#### Added
- **Schema 2.0.0 Multi-File AST Dependency Graph:** Incremental structural symbol graph traversal, cross-file caller/callee resolution, and struct/trait relationship tracking.
- **Micro-Knowledge Graph & Context Compiler:** Adaptive BM25 and structural context assembly with verified 1,200-token budget ceiling.
- **Model Context Protocol (MCP) Integration:** 17 governed tools and live MCP resources (`engram://memories`, `engram://constitution`) supporting Cursor, Claude Desktop, and Windsurf.
- **Change Contract Governance & Guard:** Multi-hop blast radius impact analysis and pre-commit Git hook enforcement.
- **Build Identity Schema v2:** Complete provenance stamping including Git commit, compiler version, linker target, and tree hash.

#### Changed
- **Empirical Benchmarks Aligned:** Audited against canonical 18-run benchmark dataset; documented modeled ~85.0% complete agent prompt reduction (~84.0% task mean) under 4,600-token fixed host overhead.
- **Multi-Platform Support:** Windows x64 native binary distribution with standalone packaging.

---

## [1.5.4] - Proposed Technical Preview (Unreleased)

### Engram Product & Runtime

#### Added
- **Build Provenance Integration:** Compile-time `build.rs` exposes Git commit SHA, working-tree dirty status, Rust compiler version, and build profile via `engram --version`.
- **Toolchain Pinning:** Dedicated `rust-toolchain.toml` pins the host compiler channel and build target.
- **Model Context Protocol (MCP) Server:** Native stdio-based MCP server (`engram mcp`) providing real-time AST context, symbol queries, and boundary checks for Cursor, Claude Desktop, and Windsurf.
- **Change Contracts & Change Guard:** Pre-edit change boundary specifications (`engram contract create`) and diff validation (`engram contract check`) enforcing Project Constitution rules (`.engram/constitution.yaml`).
- **Two-Tier Incremental Engine:** Content-addressed AST hashing with warm analysis speedup across 7 supported languages (Rust, Go, TypeScript, JavaScript, Python, C#, Java).
- **Stateful Automated Recovery:** Directed repair directives (`engram recover`) with a strict 2-attempt limit to prevent infinite repair loops without directly mutating code.
- **Engineering Memory Graph:** Architectural decision ledger (`engram memory`) with mandatory human promotion gates preventing memory poisoning.
- **1-Click Multi-Platform Installation:** PowerShell installer (`install.ps1`) and Inno Setup Windows installer (`dist/installer/engram-setup.exe`) featuring fail-closed SHA-256 checksum verification, running process protection, and automatic rollback.

#### Changed
- **SQLite Concurrency Resilience (`src/storage.rs`):** Configured a 15-second busy timeout on SQLite connection pools to prevent database locks during concurrent CLI and MCP operations.
- **Deterministic Impact Graph Budgeting (`src/impact.rs`):** Introduced `ImpactBudgetOptions` to clamp traversal depth, maximum entities per category, and relationships per entity, recording structured omission manifests for pruned items.
- **Progressive Response Shedding (`src/mcp.rs`):** Implemented seven-level progressive payload condensation (`enforce_mcp_envelope_boundary`) to guarantee MCP tool responses never exceed configured byte limits.

#### Fixed
- **UTF-8 Character Boundary Safety (`src/mcp.rs`):** Replaced byte slicing with `safe_truncate_str` to prevent panics and character corruption when truncating strings containing multi-byte UTF-8 sequences.

#### Security
- **100% Local-First Architecture:** Zero outbound network calls, telemetry beacons, or code exfiltration.
- **Subprocess Sanitization:** Parameter arrays used for all external process invocations, eliminating shell command injection surfaces.
- **In-Use Process Protection:** Installers detect active Engram processes and abort before overwriting locked binaries.

#### Known Limitations
- **Boundary Enforcement $\ne$ Semantic Correctness:** Change Guard verifies *where* changes occur and *whether* boundaries are breached; it cannot guarantee that generated code compiles or passes functional tests.
- **Asynchronous Remote Provider Cancellation:** Local process termination (`taskkill /F /T /PID`) closes the client socket, but does not guarantee immediate GPU preemption or billing cessation on remote provider inference clusters.
- **Delayed Telemetry Observability:** LLM provider token usage is typically emitted at turn completion; budget breaches during an in-flight generation burst are detected post-facto.

---

### Internal Benchmark Harness (Evaluation Infrastructure Only)

*The components below are internal research tools and are not required for product operation.*

#### Added
- **Fail-Closed Tetrad Order Validation (`orchestrator.py`):** Rejects invalid, duplicate, or malformed `--arm-order` flags prior to dispatching any candidate.
- **Dynamic Global Budget Clamping (`orchestrator.py`):** Pre-dispatch gate enforces $G_{\text{remaining}} \ge 45,000$ and clamps effective arm ceilings to protect the global safety reserve.
- **Cross-Arm Dispatch Isolation (`orchestrator.py`):** Safety ceiling breaches immediately raise `SafetyViolationError`, converting all downstream arms to `SKIPPED_SAFETY_FAILURE` with 0 tokens consumed.
- **Post-Telemetry Ceiling Override (`runner.py`, `evaluator.py`):** Forces execution status to `TOKEN_CEILING_BREACHED` if reported token usage exceeds the effective ceiling.
- **PATH Sanitization (`runner.py`):** Ensures `rg` (ripgrep) is discoverable while preventing shadowing of system binaries.

---

## [1.5.0] - 2026-10-15

### Added
- Initial standalone Engram Rust CLI executable.
- Tree-sitter parsing engine for TypeScript, Python, and Rust.
- Local SQLite database storage in `.engram/index.db`.
- Change Guard pre-commit hook integration.
- Empirical benchmark engine (`engram bench`).
