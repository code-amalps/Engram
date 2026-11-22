# Engram Phase 5.4.27: Release Scope and Product Identity Audit

**Phase:** 5.4.27  
**Execution Mode:** STRICTLY OFFLINE, READ-ONLY AUDIT  
**Baseline Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Audit Date:** 2026-11-20  
**Release Authorization:** `NOT_AUTHORIZED`

---

## 1. Executive Summary & Objective

This audit establishes the concrete boundaries, packaging, and product identity of the Engram releaseable artifact. It resolves ambiguities between development artifacts, experimental benchmark candidates, the evaluation harness, and the intended end-user product.

---

## 2. Intended Distributable & Product Identity

### 2.1 The Distributable Product
- **Product Name:** Engram
- **Current Version:** `1.5.4` (defined in `Cargo.toml`)
- **Executable Name:** `engram` (Unix / macOS) or `engram.exe` (Windows)
- **Crate Type:** Single monolithic Rust executable defined by `Cargo.toml` (`[package] name = "engram"`, `[[bin]] name = "engram"`, `path = "src/main.rs"`).
- **Core Product Capabilities:**
  1. **Change Boundaries & Change Guard (`engram contract check`, `engram validate`):** Evaluates Git diffs and AST modifications against declared contracts and Project Constitution rules.
  2. **Model Context Protocol (MCP) Server (`engram mcp`):** Provides a local, stdio-based MCP server for AI coding assistants (Cursor, Claude Desktop, Windsurf, Factory AI / Droid).
  3. **Repository Analysis & Symbol Indexing (`engram analyze`, `engram .`):** Tree-sitter AST extraction across 7 supported languages into a local SQLite database (`.engram/index.db`).
  4. **Context Compilation (`engram context`):** AST-guided context extraction fitting within strict token budgets.
  5. **Stateful Automated Recovery (`engram recover`):** Bounded repair directives without direct file mutation.
  6. **Engineering Memory & Drift Intelligence (`engram memory`, `engram drift`, `engram doctor`).

---

## 3. Disentangling Product vs. Benchmark Infrastructure

A critical finding of this audit is that **Engram contains two distinct codebases with completely different lifecycles and release targets**:

```
Engram Repository
├── [PRODUCT / RUNTIME] ─────────────── Rust Source & Native Tooling
│   ├── Cargo.toml                      Package definition (v1.5.4, AGPL-3.0)
│   ├── src/                            Main CLI, AST engine, MCP server, Change Guard
│   ├── tests/                          Unit & integration test suites
│   ├── installer/                      Inno Setup Windows packaging (.iss)
│   ├── install.ps1 / install.sh        1-Click curl/iwr installation scripts
│   └── dist/installer/                 Packaged setup binaries
│
└── [BENCHMARK HARNESS] ─────────────── Internal Python Evaluation Suite
    ├── benchmarks/matched-pair-harness/
    │   ├── src/ (orchestrator, runner) Tetrad orchestrator, process wrappers
    │   ├── tests/                      Harness safety & race regression tests
    │   ├── candidates/ & bin/          Pre-compiled candidate binaries (Candidates 1-4.2)
    │   └── reports/                    Execution logs, JSON findings, manifests
```

### 3.1 Candidate Binaries are Benchmark Test Artifacts, Not Releases
- **Candidate 3 (`97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777`):**
  - Stored in `benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe`.
  - This is an immutable benchmark reference binary compiled during Phase 3 to serve as Arm C baseline.
  - It is **not** a distributable release candidate.
- **Candidate 4.2 (`78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2`):**
  - Stored in `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe` and identical to `target/release/engram.exe`.
  - This is the candidate compiled after the Phase 5.4.3 UTF-8 boundary remediation.
  - While it embodies the current source tree, the releaseable product is named **Engram v1.5.4**, not "Candidate 4.2".
  - The designation "Candidate 4.2" belongs strictly to experimental benchmark taxonomy.

### 3.2 The Benchmark Harness is NOT Required for Product Operation
- The Python matched-pair harness (`benchmarks/matched-pair-harness/`) is **internal research and validation tooling**.
- End users **do not need Python**, `pytest`, `psutil`, or the harness repository to run Engram.
- Distributing the benchmark harness with the runtime binary is unnecessary and risks confusing developers with internal test artifacts.

---

## 4. End-User Installation & Runtime Independence

### 4.1 Standalone Operation
- Can a user install and run Engram without the development repository? **YES.**
- The compiled `engram.exe` binary is self-contained.
- **Embedded Database:** SQLite is bundled statically via the `rusqlite` crate with the `bundled` feature. No external SQLite DLL or service is required.
- **Embedded Parsers:** Tree-sitter runtime and 7 grammar parsers (Rust, Go, TypeScript, JavaScript, Python, C#, Java) are compiled into the binary.
- **External Dependencies:**
  - `git`: Required on PATH if the user executes Git-dependent features (`engram contract check`, `engram drift`, `engram history`).
  - Terminal / AI Agent: An MCP client (Cursor, Claude Desktop, Windsurf) or command-line shell.

---

## 5. Scope Boundary Recommendations

| Component | In Release Scope? | Distribution Method | Intended Audience |
| :--- | :---: | :--- | :--- |
| `engram.exe` (Windows) | **YES** | GitHub Release ZIP / Inno Setup | Developers, AI Agent Users |
| `engram` (Linux x86_64 musl) | **YES** | GitHub Release tar.gz / `install.sh` | Linux Developers |
| `engram` (macOS Intel/ARM) | **YES** | GitHub Release tar.gz / `install.sh` | macOS Developers |
| Inno Setup (`engram-setup.exe`) | **YES** | Windows GUI installer | Windows Developers |
| PowerShell 1-Click (`install.ps1`) | **YES** | Hosted raw script via `iwr` | Windows Developers |
| POSIX 1-Click (`install.sh`) | **YES** | Hosted raw script via `curl` | Linux / macOS Developers |
| Python Benchmark Harness | **NO** | Internal Git tree only | Internal Researchers |
| Candidate Binaries (1, 2, 3, 4.1, 4.2) | **NO** | Internal Git tree only | Benchmark Harness |
| Benchmark Task Fixtures | **NO** | Internal Git tree only | Benchmark Harness |

---

## 6. Audit Verdict

- **Product Identity:** **UNAMBIGUOUS.** The product is `engram` v1.5.4, a standalone native binary.
- **Scope Isolation:** **VERIFIED.** The benchmark harness and candidate test binaries are separate internal test infrastructure.
- **Release Authorization:** Strictly **`NOT_AUTHORIZED`**.
