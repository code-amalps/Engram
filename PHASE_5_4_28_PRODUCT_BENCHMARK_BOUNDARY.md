# Phase 5.4.28: Product-versus-Benchmark Boundary Architecture

**Phase:** 5.4.28  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Authorization Status:** `NOT_AUTHORIZED`  

---

## 1. Architectural Scope and Motivation

Engram is an offline code-intelligence indexing engine and Model Context Protocol (MCP) server. To empirically evaluate token efficiency and context compaction, the repository contains an internal benchmark harness (`benchmarks/matched-pair-harness/`).

A core finding of the Phase 5.4.27 audit was that while product code and benchmark code reside in distinct directories, their boundary had not been explicitly formalized. Without an architectural boundary contract, there is a risk that internal benchmark mechanisms (such as token counters, testbed fixtures, or synthetic mock telemetry) could leak into production releases, or conversely that product releases could unintentionally depend on Python runtimes.

This document formalizes the boundary across five architectural layers:
1. Directory Structure Boundary
2. Cargo and Build Dependency Boundary
3. Language and Runtime Environment Boundary
4. CI/CD Release Packaging Boundary
5. Process and IPC Boundary

---

## 2. Five-Layer Boundary Specification

### Layer 1: Directory Structure Boundary

```
Engram Repository Root
├── src/                          <-- [PRODUCT CORE] Rust crate source code
│   ├── main.rs                   <-- CLI entrypoint
│   ├── lib.rs                    <-- Crate root & build_info module
│   ├── mcp.rs                    <-- MCP server & transport boundary
│   ├── storage.rs                <-- SQLite connection & WAL engine
│   ├── impact.rs                 <-- Symbol impact graph & budget shedding
│   ├── ast.rs                    <-- Tree-sitter parser & AST extractors
│   └── ...                       <-- Other internal indexing modules
├── tests/                        <-- [PRODUCT TESTS] Rust integration tests
│   ├── build_provenance_test.rs  <-- CLI --version verification
│   ├── utf8_safety_test.rs       <-- UTF-8 boundary slice safety
│   ├── inspect_symbol_budget_test.rs <-- 16KB response budget shedding
│   ├── storage_migration_test.rs <-- SQLite schema migration & lock tests
│   └── installer_safety_test.ps1 <-- Install script fail-closed tests
├── Cargo.toml                    <-- [PRODUCT MANIFEST] Zero benchmark dependencies
├── Cargo.lock                    <-- [PRODUCT LOCKFILE] Pinned dependencies
├── build.rs                      <-- [PRODUCT BUILD] Git commit, rustc metadata
├── rust-toolchain.toml           <-- [PRODUCT TOOLCHAIN] Pinned stable compiler
├── install.ps1 / install.sh      <-- [PRODUCT INSTALLERS] Release distribution scripts
│
└── benchmarks/                   <-- [BENCHMARK INTERNAL ONLY] Separated harness
    ├── matched-pair-harness/     <-- Python tetrad evaluation framework
    │   ├── src/                  <-- Orchestrator, runner, evaluator, telemetry
    │   ├── schemas/              <-- JSON Schema specifications for telemetry
    │   ├── tests/                <-- Pytest test suites for harness safety
    │   └── bin/                  <-- Immutable candidate binaries for regression
    ├── fixtures/                 <-- Task A and Task B evaluation repositories
    └── reports/                  <-- Offline audit reports and telemetry outputs
```

**Rule 1.1:** No file within `src/` or `tests/` shall import, reference, or require any file in `benchmarks/`.  
**Rule 1.2:** The `benchmarks/` directory is strictly internal to repository development and verification; it is never included in release distributions.

---

### Layer 2: Cargo and Build Dependency Boundary

The Rust crate manifest (`Cargo.toml`) defines the complete compile-time and runtime dependency graph of the product binary.

- **Rust Dependencies:**
  - `rusqlite`, `tree-sitter`, `serde`, `serde_json`, `tokio`, `clap`, `sha2`, `hex`, `tracing`, `walkdir`.
  - Zero Python bindings (`pyo3`, `inline-python`) are present or permitted.
  - Zero benchmark-specific crates or mock harnesses are included in dependencies or dev-dependencies.
- **Cargo Build Script (`build.rs`):**
  - Zero external crate dependencies (no `vergen`, `git2`, or `shadow-rs`).
  - Standard Rust std-only execution.
  - Queries local `git` and `rustc` via `std::process::Command` without network access.
  - Fails open gracefully to `"unknown"` if git metadata is missing (e.g. in source tarball builds).

---

### Layer 3: Language and Runtime Environment Boundary

| Dimension | Product Binary (`engram.exe`) | Benchmark Harness |
|---|---|---|
| **Language** | Rust 2021 (Native binary) | Python 3.10+ |
| **Dependencies** | Self-contained static MSVC binary | `pytest`, `jsonschema`, `psutil` |
| **Operating Runtime** | Zero external runtime required | Python interpreter & virtualenv |
| **Network Access** | Zero network access (Strictly offline) | Zero external network in offline mode; provider API calls strictly bounded during authorized live runs |
| **Data Storage** | SQLite WAL file (`~/.cache/engram/`) | Sealed JSON telemetry logs in `benchmarks/` |

**Rule 3.1:** A user running `engram` requires only the compiled binary (`engram.exe`). No Python installation is needed, requested, or utilized.

---

### Layer 4: CI/CD Release Packaging Boundary

The GitHub Actions release workflow (`.github/workflows/release.yml`) defines the exact boundary of released distribution archives.

Inspection of `.github/workflows/release.yml`:
1. **Compilation:**
   ```yaml
   - name: Build binary
     run: cargo build --release --locked --bin engram
   ```
2. **Packaging:**
   The workflow archives:
   - `target/release/engram.exe` (or `target/release/engram` on Linux/macOS)
   - `README.md`
   - `LICENSE`
   - `install.ps1` / `install.sh`
3. **Exclusion:**
   The entire `benchmarks/` tree, `amendments/`, test fixtures, candidate directories, and Python test files are explicitly excluded from release archives.

---

### Layer 5: Process and IPC Boundary

When the benchmark harness tests or evaluates an Engram binary, it interacts across a strict process boundary:
1. **Subprocess Invocation:** The Python runner invokes `engram.exe` via `subprocess.Popen` with standard pipes (`stdin`, `stdout`, `stderr`).
2. **Protocol:** All interaction occurs over the Model Context Protocol (MCP) using JSON-RPC 2.0 messages over standard I/O.
3. **Zero Shared Memory:** There are no shared memory segments, DLL injections, or unmanaged pointers.
4. **Process Lifetime & Cancellation:** The benchmark runner manages candidate binaries strictly via OS Process IDs (PIDs), terminating child process trees (`taskkill /F /T /PID` on Windows) upon completion or timeout.

---

## 3. Boundary Audit Checklist

- [x] Product Rust code (`src/`) has zero imports from `benchmarks/`.
- [x] `Cargo.toml` has zero dependencies on Python or benchmark packages.
- [x] `build.rs` uses zero external crates and does not reference benchmark assets.
- [x] Packaging script packages only `target/release/engram.exe` and release docs.
- [x] Benchmark harness interacts with `engram` solely via CLI/stdio MCP JSON-RPC.
- [x] Candidate binaries in `benchmarks/matched-pair-harness/bin/` are immutable test fixtures, not release artifacts.
