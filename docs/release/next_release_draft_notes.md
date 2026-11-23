# Engram v1.5.5 Candidate 1 (Developer Preview) — Release Notes

**Version Identifier:** `v1.5.5-cand1`  
**Target Release Date:** 2026-11-27  
**Previous Certified Baseline:** `v1.5.4-cand5` (`2df1cac683435314ffd7ad52369e6d20198fae0d`)  
**Package Artifact:** `engram-preview-v1.5.5-cand1-windows-x64.zip`  
**License:** GNU Affero General Public License v3.0 or later (`AGPL-3.0-or-later`)  
**Legal Status:** `TECHNICAL_SOURCE_CORRESPONDENCE_VERIFIED_LEGAL_REVIEW_PENDING`  

---

## 1. Overview & Highlights

Engram v1.5.5 Candidate 1 represents a major milestone on the roadmap toward the v1.6 efficiency architecture. This candidate packages substantial feature additions, architectural consolidations, multi-platform verification, and hardened operational reliability.

### Key Features & Improvements:

1. **Consolidated Engagement Endpoint (`engram_engage`):**
   - Introduces a single, unified MCP entry point that binds Git baseline, checks working tree state, compiles AST context, validates Project Constitution boundaries, and manages Change Contracts in a single JSON-RPC tool call.
   - Eliminates multi-turn protocol coordination friction between `engram_get_context` and `engram_preflight`.
   - Empirically verified to achieve a **31.0% mean paired reduction in MCP wire-level byte payload** across 150 live OS stdio pipe evaluations.

2. **Adaptive Trivial Task Bypass:**
   - Automatically classifies low-complexity leaf tasks (documentation updates, single-comment fixes) and issues an immediate `BYPASS_TRIVIAL` response.
   - Bypasses heavy AST compilation and Change Contract locking, yielding **+46.8% mean paired latency speedup** and 21.3% wire savings for trivial edits.

3. **Turn-0 Static Context Injection:**
   - Adds non-destructive managed block injection (`<!-- BEGIN ENGRAM MANAGED BLOCK -->` ... `<!-- END ENGRAM MANAGED BLOCK -->`) for agent instruction files.
   - Natively supports `.github/copilot-instructions.md`, `.cursorrules`, `.windsurfrules`, `CLAUDE.md`, and `AGENTS.md`.
   - Injects core architecture context and task rules at Turn 0, eliminating agent exploratory tool calls with zero runtime MCP token overhead.

4. **Multi-Platform CI Matrix:**
   - Established a 6-job automated CI pipeline validating Linux glibc (x86_64), Linux musl (static x86_64), macOS Apple Silicon (ARM64), macOS Intel (x86_64), Windows MSVC (x86_64), and strict linter/formatting suites.

5. **Fail-Closed Git Working Tree Detection:**
   - Hardened `src/git.rs` with bounded 5-second execution timeouts and fail-closed error handling, ensuring unresponsive Git processes cannot hang the MCP server.

6. **Deterministic Windows Reproducibility:**
   - Integrated `-C link-arg=/BREPRO` into release compilation, eliminating non-deterministic linker timestamps and CodeView GUID variances on Windows MSVC.

---

## 2. Documented Operational Limitations Register

All users and evaluating teams should note the following documented operational boundaries:

- **LIM-01 (Single-Turn Overhead):** In single-turn tasks with single-file edits and zero blast radius, AST parsing and JSON-RPC framing carry a fixed ~850-byte baseline (+0.71% synthetic overhead) when bypass is disabled.
- **LIM-02 (Benchmark Provenance):** Token reduction figures from Phase 5.9.2.5 were collected using an offline deterministic simulation harness; live commercial LLM billing validation is scheduled for Phase 9.
- **LIM-03 (Packaging Scope):** Pre-compiled binary archives are currently packaged for 64-bit Windows (`x86_64-pc-windows-msvc`). Linux and macOS users can build from source via `cargo build --release --locked`.
- **LIM-04 (Dual MCP Surface):** The default MCP server exposes 5 core tools (`engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight`, and `engram_engage`). The 12 specialist tools require passing `--all-tools`.
- **LIM-05 (Reproducibility):** Linker timestamps resolved via `/BREPRO`.
- **LIM-06 (Clippy Style Diagnostics):** Non-fatal pre-v1.6 Rust 2024 style warnings on historical files are scheduled for cleanup in Phase 8.2.
- **LIM-07 (Legal Review Requirement):** Technical source correspondence is certified, but formal qualified external legal counsel review of AGPL-3.0 remains pending. Distributed as an Experimental Developer Preview under `TECHNICAL_SOURCE_CORRESPONDENCE_VERIFIED_LEGAL_REVIEW_PENDING`.
- **LIM-08 (Live Benchmark Daemon Scope):** Reported live stdio benchmark timings reflect warm-session persistent daemon execution over OS pipes; cold-process startup latency was not separately isolated.

---

## 3. Installation & Getting Started

### Clean Installation
1. Extract `engram-preview-v1.5.5-cand1-windows-x64.zip` to a folder in your `PATH` (e.g. `C:\bin`).
2. Verify installation:
   ```powershell
   engram --version
   ```
3. Initialize Engram in your repository:
   ```powershell
   cd \path\to\your\repo
   engram init
   engram index
   ```

### MCP Server Integration (Cursor / VS Code / Claude Desktop)
Add the following configuration to your MCP settings:
```json
{
  "mcpServers": {
    "engram": {
      "command": "engram",
      "args": ["mcp"]
    }
  }
}
```

### Upgrading from v1.5.4-cand5
1. Replace `engram.exe` with the new binary.
2. No repository schema migrations are required. Existing `.engram/` metadata and Change Contracts are automatically recognized and fully compatible.
