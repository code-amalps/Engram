# Security Architecture, Privacy & Known Limitations

This document provides a transparent accounting of Engram's security model, data privacy guarantees, ignore policies, and current technical limitations.

---

## 1. Security & Privacy Model

### 100% Local Execution
- **Zero Remote Telemetry:** Engram contains no analytics, telemetry, or remote tracking libraries.
- **Zero Outbound HTTP Calls:** Engram's dependency tree (`Cargo.toml`) contains no HTTP client crates (`reqwest`, `hyper`, `curl`, `ureq`). The core indexing and retrieval engine does not make network calls.
- **Local Git Subprocesses:** Engram invokes local `git` CLI subcommands (`rev-parse`, `diff`, `status`) strictly to inspect working tree status. It never calls `git push`, `git fetch`, or network-bound commands.
- **Local MCP Transport:** Model Context Protocol communication operates exclusively via local standard input/output (`stdio`).

### Targeted Secret Redaction Engine
Before persisting extracted code excerpts to `.engram/index.db`, Engram's parser (`src/analyzer.rs`) executes targeted regex filtering to identify and redact sensitive credential assignments:
- Patterns matching `(?:password|secret|api[_-]?key|token)\s*[:=]\s*["'][^"']+["']` have their values replaced with `[REDACTED]`.
- In addition, standard secret files (`.env`, `.env.*`, `*.pem`, `*.key`) are excluded from file discovery.
- **Scope & Limitations:** Redaction is targeted to regex-matched key-value assignments and known secret filenames. It is not an arbitrary high-entropy cryptographic scanner (such as TruffleHog); developers should continue using dedicated secret management practices.

---

## 2. Ignore Policies (`.engramignore`)

Engram implements a 6-layer tiered configuration evaluation in `src/discovery.rs` to prune directories during filesystem traversal:
- **Layer 1:** Universal safety exclusions (non-overridable: `.git`, `.engram`, `node_modules`, `target`, binary archives).
- **Layer 2:** Tool configuration (`engram.yaml`).
- **Layer 3:** Explicit tool ignore (`.engramignore` with `!` negation support).
- **Layer 4:** Standard Git ignore (`.gitignore`, `.git/info/exclude`).
- **Layer 5:** Secret file exclusions.
- **Layer 6:** Legacy category-prefixed patterns (`SECRET:`, `VENDORED:`, `GENERATED:`).

Excluded files are **pruned at discovery time** during recursive directory walking and are never parsed or stored in SQLite.

### Symbolic Link & Junction Policy
To prevent directory traversal loops and path-escape attacks:
- `src/discovery.rs` explicitly configures `.follow_links(false)`.
- Workspace discovery and MCP path resolution execute canonical boundary validation checks to detect and reject escaping symlinks and junctions.

---

## 3. Known Limitations

Documented limitations for the `v1.5.5-rc2` release:

### 1. 403 Compiler Warnings in Release Build
- The release build emits **403 compiler warnings** during `cargo build --release`.
- **Cause:** Harmless unused struct fields, dead code warnings, and missing documentation comments across generated or auxiliary Tree-sitter grammar modules.
- **Impact:** Non-blocking. All 800+ test functions pass, and zero warnings cause runtime errors.

### 2. 82 Non-Code Repository Files Not Indexed
- The repository contains **82 non-code files** (documentation `.md` files, configuration `.json`/`.yaml` files, test fixtures, and shell scripts) that are skipped by AST parsers.
- **Impact:** Non-code files are not mapped into the AST symbol graph. Lexical queries covering markdown documents fall back to memory ADRs or standard file inspection.

### 3. Windows x64 Distribution Focus
- Official pre-compiled binary packages are currently provided primarily for **Windows x64**.
- macOS and Linux environments require building from source via `cargo build --release` (standard Rust compilation requirements apply).

### 4. Single-Writer SQLite Concurrency
- While SQLite WAL mode permits concurrent readers during MCP tool calls, write operations (e.g. initial `engram analyze`) require an exclusive write lock.
