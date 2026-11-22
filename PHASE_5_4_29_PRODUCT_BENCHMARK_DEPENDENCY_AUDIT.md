# Phase 5.4.29: Product-versus-Benchmark Dependency and Runtime Separation Audit

**Phase:** 5.4.29  
**Component:** Architectural Boundary & Runtime Dependency Audit  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Authorization Status:** `NOT_AUTHORIZED`  

---

## 1. Executive Summary

Phase 5.4.29 conducted a line-by-line dependency and runtime invocation audit to verify that the Engram product executable (`engram.exe`) has **zero dependency on Python, zero dependency on the internal benchmark harness, and zero runtime reliance on benchmark assets**.

The audit examined the Rust crate manifest, source code module graph, build script, CI release packaging script, installer logic, and process execution behavior.

**Audit Verdict:** Complete separation confirmed. Engram is a 100% self-contained, statically linked native binary that operates without Python or external runtime components.

---

## 2. Seven-Layer Dependency and Boundary Audit

### Layer 1: Cargo Dependencies (`Cargo.toml`)
- The dependencies declared in [`Cargo.toml`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/Cargo.toml) were audited:
  - `anyhow`, `chrono`, `clap`, `ignore`, `regex`, `rusqlite` (with `features = ["bundled"]`), `serde`, `serde_json`, `serde_yaml`, `sha2`, `tree-sitter` (+ language parsers: C#, Go, Java, JS, Python, Rust, TS), `uuid`.
- **Finding:** Zero benchmark crates, zero testbed frameworks, and zero Python FFI bindings (`pyo3`, `inline-python`) exist.
- SQLite is statically bundled via `rusqlite`'s C-amalgamation, eliminating external dynamic library requirements.

### Layer 2: Rust Module Graph (`src/`)
- A regex sweep across all 51 `.rs` files in `src/` for references to `matched-pair` or `benchmarks/` revealed:
  - `src/compiler.rs`: One string literal in a unit test verifying path normalization logic (`normalize_subpath("benchmarks\\matched-pair-harness\\")`).
  - `src/main.rs`: CLI help text documenting that `--scope` can accept arbitrary repository folders (e.g. `'src/'` or `'benchmarks/'`).
  - `src/bench.rs`: Internal command `engram bench` implementing native Rust micro-benchmarks of internal AST indexing data structures.
- **Finding:** No module in `src/` imports or delegates to any Python script or file in the `benchmarks/` directory.

### Layer 3: Build-Script Dependencies (`build.rs`)
- `build.rs` uses strictly Rust standard library modules: `std::process::Command`, `std::path::PathBuf`, `std::env`.
- It executes `git` and `rustc` at compile time and queries environment variables.
- **Finding:** `build.rs` has zero external dependencies and does not interact with the benchmark harness.

### Layer 4: Release Workflow Boundary (`.github/workflows/release.yml`)
- Lines 124–163 of the release workflow specify the packaging steps:
  - Archives include: `target/.../engram.exe` (or `engram`), `README.md`, `LICENSE`.
- **Finding:** The packaging step explicitly copies only the compiled binary and documentation files into `dist_staging/`. The entire `benchmarks/` directory tree is excluded from release archives.

### Layer 5: Installer Scripts (`install.ps1` & `install.sh`)
- Both installer scripts unpack the release archive into the user's local bin folder (e.g. `~/.engram/bin` or `%LOCALAPPDATA%\Programs\Engram\bin`).
- **Finding:** Installers neither install Python nor check for a Python interpreter.

### Layer 6: Runtime Process Invocation
- Ingestion, indexing, AST analysis, Change Guard evaluation, and MCP server communication occur entirely within native machine code threads.
- `engram` never spawns Python subprocesses during standard operation.

### Layer 7: Runtime File Dependencies
- At runtime, Engram interacts only with:
  1. The target workspace repository specified by `--root` (defaulting to current directory).
  2. The local user cache directory: `~/.cache/engram/` (or `%LOCALAPPDATA%\Engram\cache\`).
- It does not read, write, or expect any configuration files from `benchmarks/`.

---

## 3. Dependency Classification Summary

| Component | Nature of Dependency | Present in Release Binary? | Requires Python at Runtime? |
|---|---|---|---|
| **Rust Standard Library & Crates** | Compile-Time / Statically Linked | Yes | No |
| **Bundled SQLite Engine** | Compile-Time / Statically Linked C | Yes | No |
| **Tree-Sitter C/C++ Parsers** | Compile-Time / Statically Linked C | Yes | No |
| **`build.rs` Compile Script** | Build-Time Only | No (metadata only) | No |
| **`tests/` Test Suites** | Test-Time Only | No | No |
| **`benchmarks/matched-pair-harness`** | Internal Benchmark Only | **No (Completely Excluded)** | **No** |
