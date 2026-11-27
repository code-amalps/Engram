# Contributing to Engram

Thank you for your interest in contributing to Engram! We welcome contributions from developers, researchers, and open-source engineers interested in local repository intelligence, Tree-sitter AST extraction, Model Context Protocol tools, and change boundary controls.

---

## 1. Codebase Architecture & Module Map

Engram is structured as a modular Rust binary crate:

```text
src/
├── main.rs            # Application entrypoint & CLI routing
├── cli.rs             # CLI definition & argument parsing (clap)
├── analyzer.rs        # Tree-sitter AST parser coordination
├── ast_deps.rs        # AST cross-reference & dependency extraction
├── knowledge_graph.rs # Schema 2.0.0 dependency graph engine
├── storage.rs         # SQLite WAL persistence & schema migrations
├── incremental.rs     # Two-tier invalidation (mtime + SHA-256 hash)
├── compiler.rs        # Context compiler & prompt boundary tagging
├── budget.rs          # Token budget slicing & prioritization
├── contract.rs        # Change Contract lifecycle state machine
├── guard.rs           # Git diff boundary inspection & enforcement
├── recovery.rs        # Max-2 retry bounded recovery engine
├── memory.rs          # Engineering Memory (memory.yaml) persistence
└── mcp.rs             # JSON-RPC 2.0 Model Context Protocol stdio server
```

---

## 2. Development Setup & Building

### Prerequisites
- **Rust Toolchain:** Rust 1.85+ (or 1.99+) with `cargo`.
- **C/C++ Build Tools:** Required for Tree-sitter grammar compilation (MSVC on Windows, GCC/Clang on Unix).
- **Git:** Required for working tree diff checks.

### Workflow Commands
```bash
# Clone the repository
git clone https://github.com/code-amalps/Engram.git
cd Engram

# Compile in debug mode
cargo build

# Run formatting checks
cargo fmt --check

# Run linter
cargo clippy

# Run all unit and integration tests
cargo test
```

---

## 3. Adding a New Language Analyzer

Engram welcomes support for additional programming languages. To add an analyzer:

1. Add the corresponding Tree-sitter parser crate to `Cargo.toml` (e.g., `tree-sitter-kotlin`, `tree-sitter-ruby`).
2. Implement the `LanguageAnalyzer` trait in `src/analyzer.rs`:
   - Define node queries for symbol declarations (classes, functions, methods, interfaces).
   - Define relation queries for dependency extraction (`Imports`, `Calls`, `UsesType`).
   - Implement docstring and signature extraction.
3. Register the file extensions and language identifier in `src/analyzer.rs`.
4. Add unit test fixtures covering language constructs in `tests/`.

---

## 4. Code Standards & Style

- **Formatting:** Code must adhere to standard `cargo fmt` formatting.
- **Deterministic Behavior:** Context compilation and graph queries must be 100% deterministic given the same inputs and repository state.
- **No Background Daemons:** Engram runs on demand via CLI or MCP stdio. Do not introduce background threads or persistent daemons.
- **Privacy & Security:** Never introduce network dependencies or telemetry crates. Engram must remain 100% local.

---

## 5. Licensing & AGPL-3.0 Compliance

Engram is licensed under the **GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later)**.

### Contribution Requirements:
1. **License Agreement:** By submitting a pull request, you agree that your contributions will be licensed under the project's AGPL-3.0-or-later terms.
2. **Third-Party Code:** If introducing third-party dependencies, ensure their licenses are compatible with AGPL-3.0 (e.g., MIT, Apache-2.0, BSD-3-Clause) and add their attribution to `THIRD_PARTY_NOTICES.md`.
3. **No Proprietary Linking:** Do not link proprietary or license-incompatible libraries that violate AGPL-3.0 terms.\n