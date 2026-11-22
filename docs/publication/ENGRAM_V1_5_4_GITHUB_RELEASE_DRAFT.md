# Engram v1.5.4 Candidate 5 (Windows x64 Developer Preview)

Engram v1.5.4 Candidate 5 is an **Experimental Developer Preview** of the local engineering memory and context engine for AI coding agents.

This release provides workspace-bounded code intelligence, AST symbol extraction, deterministic change impact analysis, and Model Context Protocol (MCP) stdio server integration. It enables coding agents (Claude, Cursor, Codex, Windsurf, Factory AI) to query targeted code boundaries and signatures rather than ingesting entire repository contents.

---

## What's Included in This Preview

- **Model Context Protocol (MCP) Stdio Server:** Native JSON-RPC 2.0 server (`engram mcp`) supporting standard protocol lifecycle (`initialize`, `tools/list`, `tools/call`, `resources`, `prompts`, `ping`).
- **Dual Tool Surface:**
  - **Lean Default Surface (4 tools):** `engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight` — designed to conserve agent context windows.
  - **Specialist Full Surface (16 tools):** Available by passing the `--all-tools` flag on launch.
- **Dynamic Context Compilation & Budgeting:** Dynamic context packaging (`engram_get_context`) respecting token budgets, payload byte limits, and signature-only compact tiers (`DetailLevel::Compact`).
- **Blast Radius & Impact Analysis:** Multi-hop dependency analysis (`engram_inspect_symbol`, `engram_get_impact`) calculating caller/callee graphs, classification, and file boundaries.
- **UTF-8 Character Boundary Safety:** Zero-panic character boundary truncation (`safe_truncate_str`) preventing multi-byte UTF-8 slice splits across symbol excerpts.
- **Index State Invalidation:** Deterministic workspace state fingerprinting (`IndexStateFingerprint`) invalidating compiled context caches upon source changes.
- **Preflight & Postflight Guard Contracts:** Deterministic pre-flight handshake and post-flight guard checks validating working tree mutations against architectural rules.

---

## Mandatory Provenance & Methodology Disclosures

> [!IMPORTANT]
> **Synthetic Simulation Disclosure:**
> All token reduction figures (e.g. simulated 72.8% reduction across multi-turn loops) were measured strictly within the Phase 5.9.2.5 offline deterministic benchmark simulator under the `SYNTHETIC_SIMULATED_USAGE` test harness.

- **Zero Live Provider Savings Claimed:** This release makes **no claims** of real-world LLM token savings, commercial provider cost reductions (OpenAI/Anthropic/Google), or live agent latency improvements.
- **Single-Turn Overhead (Workload F):** In single-turn workflows with minimal blast radius, Engram introduces a **+0.71% synthetic token overhead** due to baseline AST extraction and MCP tool framing.
- **Platform Scope:** This preview package is certified exclusively for **Windows x64 (AMD64)** (`x86_64-pc-windows-msvc`).
- **Validation Scope:** The reported 235 passing validation items represent **heterogeneous test assertions, runtime harness checks, headless MCP scenarios, and security containment vectors**, not 235 unit tests.

---

## Binary Provenance & AGPL-3.0 Correspondence

The primary release binary in this package is **Option B (Clean Build)**:
- **Packaged Executable:** `bin/engram.exe`
- **Executable SHA-256:** `1C5DD68B499298ACE1F6BFFBB4028951DD30C63500604CB355CC0A307E09AD38`
- **Embedded Git Commit:** `625461beb76d1b87027f9687e2fa010ee2973f2c`
- **Embedded Dirty Flag:** `false` (Pristine clean worktree build)
- **Rust Toolchain:** `rustc 1.99.0 (b940084d7 2026-09-28)`
- **Build Profile:** `release` (`opt-level = 3`, `lto = true`, `codegen-units = 1`, `panic = "abort"`)

*(Note: The historical Option A benchmark binary `E4E53216...` with `dirty: true` metadata is preserved in the repository source history for benchmark reproduction).*

---

## Asset Verification

Verify the integrity of your download before running:

```powershell
# Verify the ZIP archive checksum
Get-FileHash .\engram-preview-v1.5.4-cand5-clean-windows-x64.zip -Algorithm SHA256
# Expected SHA-256: CDD34117E217C44DE0C0F2BD860C04A64350EAEF500A4FC1B39FCABE9B85FF2E

# Verify the extracted executable
Get-FileHash .\bin\engram.exe -Algorithm SHA256
# Expected SHA-256: 1C5DD68B499298ACE1F6BFFBB4028951DD30C63500604CB355CC0A307E09AD38
```

---

## Quickstart & Installation

1. Download and extract `engram-preview-v1.5.4-cand5-clean-windows-x64.zip`.
2. Place `bin\engram.exe` on your system `PATH` or invoke it directly.
3. Configure your agent's MCP settings (e.g. Claude Desktop or Cursor `claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "engram": {
      "command": "engram.exe",
      "args": ["mcp"]
    }
  }
}
```

For full tool surface, pass `["mcp", "--all-tools"]`.

---

## License & Source Code

Engram is free software licensed under the **GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later)**.
Complete Corresponding Source is available in this repository at tag `v1.5.4-cand5` (commit `2df1cac683435314ffd7ad52369e6d20198fae0d`).
