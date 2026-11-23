# Engram Next-Version Release-Readiness Audit Report

**Target Release:** Engram v1.5.5 Candidate 1 (`v1.5.5-cand1`)  
**Audit Date:** 2026-11-23  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**GitHub Target:** `code-amalps/Engram` (`origin`), branch `feature/phase2-stage5-remediation`  
**Certified Baseline:** `v1.5.4-cand5` at commit `2df1cac683435314ffd7ad52369e6d20198fae0d`  
**Auditor Classification:** Release Engineering & Systems Verification  

---

## 1. Executive Summary & Verdict

This document presents a comprehensive, evidence-based release-readiness assessment for the upcoming candidate release of Engram. The codebase has advanced through Phase 7.0 (release governance), Phase 7.1 (multi-platform CI), Phase 7.2 (Windows reproducibility), Phase 8.0/8.0.1 (Turn-0 static context injection), and Phase 8.1/8.1.1.2/8.1.1.3 (consolidated `engram_engage` endpoint, fail-closed Git detection, and live stdio benchmark).

### Audit Verdict: **`RELEASE_CANDIDATE_READY_WITH_DOCUMENTED_LIMITATIONS`**
- **P0 Release Blockers:** **0**
- **P1 Candidate Blockers:** **0**
- **P2 Documented Operational Limitations:** **8** (all documented in Limitations Register and Draft Release Notes)
- **P3 Deferred Backlog Items:** **2**
- **Test Suite Pass Rate:** **100%** (all unit, integration, and live stdio benchmark tests pass)
- **CI Matrix Pass Rate:** **100%** (6/6 jobs green on GitHub Actions run `#45150284124`)
- **Legal Compliance Status:** `TECHNICAL_SOURCE_CORRESPONDENCE_VERIFIED_LEGAL_REVIEW_PENDING`

---

## 2. Area A: Product Functionality & Regressions

### 2.1 Baseline CLI Subcommands
All core CLI subcommands maintain full functional parity and backward compatibility:
- `engram init`: Initializes the `.engram/` metadata directory, schema tables, and SQLite WAL storage (`src/setup.rs:1-450`, `src/storage.rs:1-150`).
- `engram index`: Orchestrates Tree-sitter AST extraction across 7 supported languages (Rust, TypeScript, JavaScript, Python, Go, Java, C#) into local SQLite storage (`src/compiler.rs:1-350`).
- `engram context`: Compiles token-budgeted AST summaries, symbol excerpts, and Project Constitution rules (`src/compiler.rs:200-380`).
- `engram check`: Validates planned or uncommitted file modifications against Project Constitution boundaries (`src/constitution.rs:1-200`).
- `engram plan`: Analyzes natural language tasks and infers file targets and blast radii (`src/prompt.rs:1-400`).
- `engram contract`: Manages the formal Change Contract state machine (`src/contract.rs:1-500`).
- `engram verify`: Validates modified files against an active Change Contract (`src/flywheel.rs:1-300`).
- `engram preflight`: Provides one-shot boundary validation, AST context assembly, and Change Contract issuance (`src/flywheel.rs:200-400`).

### 2.2 MCP Surface & Consolidated Endpoint
- **Lean Tool Surface:** By default, `engram mcp` provides a focused 5-tool surface (`engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight`, and `engram_engage`).
- **Specialist Tool Access:** 12 specialist diagnostic tools (`engram_get_impact`, `engram_query_graph`, `engram_why`, etc.) are exposed when passing `--all-tools` (`ENGRAM_SPECIALIST_TOOLS=1`, `src/main.rs:1544-1550`).
- **Phase 8.1.1.2 Consolidated Endpoint (`engram_engage`):**
  - Unified workflow: binds repository Git baseline, detects working tree state, compiles AST context, validates Project Constitution boundaries, and issues/reuses Change Contracts in a single JSON-RPC round trip (`src/engage.rs:87-350`).
  - Input validation: enforces task string length (1–4096 characters), rejects empty tasks, and validates planned/forbidden paths with workspace containment (`src/engage.rs:98-135`).
  - Adaptive bypass: automatically classifies low-complexity leaf edits (e.g. documentation typos, isolated comments) and issues immediate `BYPASS_TRIVIAL` responses without heavy AST compilation or contract overhead (`src/engage.rs:149-197`).
  - Verified empirical savings: reduces MCP wire payload by **31.0% mean paired reduction** over live OS stdio pipes (`tests/engage_bench_test.rs`).

### 2.3 Turn-0 Static Context Injection
- Managed instruction blocks (`<!-- BEGIN ENGRAM MANAGED BLOCK -->` ... `<!-- END ENGRAM MANAGED BLOCK -->`) implemented in `src/instructions.rs:1-606`.
- Supports all major coding agent configurations: `.github/copilot-instructions.md`, `.cursorrules`, `.windsurfrules`, `CLAUDE.md`, and `AGENTS.md`.
- Non-destructive atomic replacement: preserves user instructions outside the managed block using atomic temporary file replacement (`src/instructions.rs:66-105`).
- Zero runtime overhead: static markdown files are ingested by IDE agents at prompt initialization without generating MCP tool calls.
- Verified by unit and integration tests in `tests/turn0_injection_test.rs` and `tests/turn0_phase1_test.rs`.

### 2.4 Contract Enforcement Lifecycle
- States: `Created` -> `Active` -> `Closed` (clean verify) or `Broken` (out-of-bounds edit).
- Fail-closed behavior: missing, corrupted, or tampered contract files return strict errors and reject modifications (`src/contract.rs:250-380`).

---

## 3. Area B: Security & Hardening

### 3.1 Workspace Containment & Path Traversal Prevention
Path handling across all CLI and MCP tools is strictly guarded via `validate_workspace_path` (`src/mcp.rs:71-155`):
1. **Null Byte Check:** Paths containing null characters (`\0`) are immediately rejected.
2. **Absolute Path Verification:** Absolute paths are checked to ensure they reside strictly within the canonical workspace root.
3. **Existing Path Containment:** Existing paths are canonicalized and verified against `normalize_for_containment` (`src/mcp.rs:93-105`).
4. **Ancestor Traversal for New Paths:** For paths that do not yet exist, the algorithm walks up directory ancestors until an existing directory is found, canonicalizes that ancestor, and validates containment (`src/mcp.rs:107-143`). This prevents symlink escape attacks via non-existent leaf paths.

### 3.2 Symlink Handling & Sandboxing
- Workspace traversal avoids unbounded recursion across symlinks.
- Empirical test runner features dedicated Windows AppContainer and JobObject isolation sandboxing (`src/empirical_live/security.rs:730-1350`).

### 3.3 Subprocess Execution & Command Injection Audit
- Every subprocess invocation across `src/` uses direct binary execution via `Command::new(...)` with sliced argument arrays (`.args([...])`).
- Zero shell invocations (`sh -c`, `bash -c`, `cmd.exe /c` with arbitrary user string concatenation) exist in the codebase.
- User input is never interpolated into command strings.

### 3.4 Memory Safety & Unsafe Code Audit
A forensic audit for `unsafe` across `src/` revealed:
1. `src/main.rs:1546`: A single `unsafe { std::env::set_var(...) }` called during single-threaded CLI initialization for the `--all-tools` flag before worker threads are spawned.
2. `src/empirical_live/security.rs`: Low-level Win32 API bindings for Windows AppContainer profiles, JobObjects, and ACL security descriptors used strictly in empirical isolation tests.
3. **Core Engine:** **Zero `unsafe` blocks** in SQLite storage, Tree-sitter AST parsing, MCP JSON-RPC protocol framing, Change Contract engine, or CLI subcommands.

### 3.5 Git Subprocess Timeout & Reliability
- `src/git.rs:102-177`: `detect_git_state_with_timeout` executes `git status --porcelain=v1 -uall` with a bounded **5-second timeout** managed via `mpsc::channel::recv_timeout`.
- If Git hangs, Engram terminates the process (`child.kill()`), reaps it (`child.wait()`), and returns `GitStateError::Timeout`, failing closed rather than hanging the MCP server.

---

## 4. Area C: Multi-Platform Compatibility

The cross-platform CI matrix established in Phase 7.1/8.1.1.2 validates Engram across 5 target environments. Results from GitHub Actions Run **`#45150284124`**:

| Target Platform | Runner Image | Architecture | ABI / Target Triple | CI Status |
|---|---|---|---|:---:|
| **Windows MSVC** | `windows-latest` | x86_64 | `x86_64-pc-windows-msvc` | **PASSED** |
| **Linux glibc** | `ubuntu-latest` | x86_64 | `x86_64-unknown-linux-gnu` | **PASSED** |
| **Linux musl** | `ubuntu-latest` | x86_64 | `x86_64-unknown-linux-musl` | **PASSED** |
| **macOS Apple Silicon** | `macos-14` | aarch64 | `aarch64-apple-darwin` | **PASSED** |
| **macOS Intel** | `macos-15-intel` | x86_64 | `x86_64-apple-darwin` | **PASSED** |
| **Lint / Rustfmt** | `ubuntu-latest` | x86_64 | Standard Lint Suite | **PASSED** |

### Platform Compatibility Findings:
- Path separators: Normalized to `/` across all MCP wire payloads and JSON responses.
- Line endings: `src/instructions.rs:46` automatically detects and preserves `\r\n` (Windows) and `\n` (POSIX).
- Bundled SQLite: Compiles from embedded C sources via `rusqlite` bundled feature, eliminating external SQLite shared library dependencies on all 5 platforms.

---

## 5. Area D: Build, Toolchain & Dependency Integrity

### 5.1 Toolchain Specification
- **Edition:** Rust 2024 (`edition = "2024"` in `Cargo.toml`).
- **Channel:** Pinned to `stable` channel via `rust-toolchain.toml`.
- **Target Baseline:** MSVC x86_64 on Windows (`x86_64-pc-windows-msvc`).

### 5.2 Dependency Audit
- **Lockfile Integrity:** `Cargo.lock` is fully versioned and reproducible in offline builds (`cargo build --release --locked --offline`).
- **Tree-sitter Parsing:** Modern 0.25 core engine with pinned language grammars (0.23).
- **SQLite Engine:** `rusqlite 0.32` with `bundled` feature.
- **Vulnerabilities:** Zero reported critical CVEs in production dependencies.

### 5.3 Release Compilation Profile
Configured in `Cargo.toml`:
```toml
[profile.release]
lto = "thin"
codegen-units = 1
strip = true
```
Linker flags in `.cargo/config.toml`: `-C link-arg=/BREPRO` guarantees reproducible PE headers on Windows MSVC.

---

## 6. Area E: Distribution, Packaging & Installation

### 6.1 Package Assembly
- **Package Archive:** `releases/engram-preview-v1.5.5-cand1-windows-x64.zip`
- **Archive Contents:**
  - `engram.exe`: 64-bit Windows release executable.
  - `README.md`: Product documentation and quick-start guide.
  - `LICENSE`: Full GNU Affero General Public License v3.0 text.
  - `RELEASE_NOTES.md`: Detailed release notes, feature highlights, and limitations catalog.

### 6.2 Installation & Upgrade Verification
- **Clean Installation:** Verified extracting the archive to an isolated directory and executing `engram --version` and `engram init`.
- **In-Place Upgrade from v1.5.4-cand5:** Verified binary replacement in existing repository worktree. Existing `.engram/` metadata, contracts, and SQLite WAL indexes operate seamlessly without migration errors.

---

## 7. Area F: Documentation, Governance & Legal

### 7.1 Documentation Alignment
- `README.md` accurately describes CLI usage, MCP integration with Cursor / VS Code / Claude Desktop, and Project Constitution rules.
- `docs/governance/KNOWN_LIMITATIONS_REGISTER.md` cataloged and up-to-date.

### 7.2 AGPL-3.0 Compliance & Source Correspondence
- Under AGPL-3.0 Section 6, the Corresponding Source code is publicly available at `https://github.com/code-amalps/Engram`.
- Every source file in `src/` and `tests/` corresponds 100% bit-for-bit with the committed Git tree.
- Full license text included in repository root and distribution package.

### 7.3 Legal Review Posture
- Status maintained strictly as:  
  **`TECHNICAL_SOURCE_CORRESPONDENCE_VERIFIED_LEGAL_REVIEW_PENDING`**
- Distribution is explicitly governed as an **Experimental Developer Preview**. Formal qualified legal counsel review remains pending prior to final non-preview production distribution.

---

## 8. Version Determination & Rationale

- **Previous Certified Release:** `v1.5.4-cand5` (commit `2df1cac`)
- **Evaluated Options:**
  - Option A: `v1.5.5-cand1` (Patch release candidate)
  - Option B: `v1.6.0-alpha1` (Minor release candidate)
- **Selected Version:** **`v1.5.5-cand1`**
- **Rationale:**  
  While Phase 8 introduced significant architectural capabilities (`engram_engage`, Turn-0 context injection), the v1.6 roadmap dictates that Phase 8.2 (local postflight enforcement) and Phase 9 (ecosystem integration and live agent validation) must complete before tagging `v1.6.0`. Designating this release candidate as `v1.5.5-cand1` accurately communicates an incremental preview candidate bridging toward v1.6.0.
- **Cargo.toml Update Decision:**  
  `Cargo.toml` is updated from `1.5.4` to `1.5.5` to ensure `engram --version` reports `engram 1.5.5` and build provenance metadata strictly matches the release candidate package identity.
