# Phase 5.7: Installation, Onboarding & Agent Integration Guide

**Document ID:** `ENGRAM-PHASE-5.7-INSTALLATION-GUIDE`  
**Date:** 2026-11-20  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Target Release:** `1.5.4` (Public Alpha Candidate)

---

## 1. Safety-First Windows Installation

Engram provides two audited, zero-elevation installation methods on Windows: the native PowerShell installer script (`install.ps1`) and the Inno Setup executable installer (`installer/engram.iss`). Both operate strictly in user space and require **no administrator privileges**.

### 1.1 PowerShell Native Script (`install.ps1`)

The installer enforces five strict safety invariants verified in Phase 5.4.31:
1. **User Scope Only:** Installs binary to `$env:LOCALAPPDATA\Programs\engram\bin` (or `$env:USERPROFILE\.engram\bin`).
2. **In-Use Process Detection:** Checks if `engram.exe` is currently running, prompting the user or halting safely rather than crashing or corrupting files.
3. **Staged Atomic Deployment:** Copies files to a temporary staging directory (`engram.tmp.exe`), performs an integrity check, and atomically swaps it into place.
4. **Automated Rollback:** If the staging check fails, the installer restores previous files and aborts cleanly.
5. **Safe User PATH Registration:** Appends the installation path to the user's personal environment block without touching system-wide keys, creating a timestamped backup before modification.

#### Installation Command:
```powershell
# From local repository or release folder:
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

### 1.2 Inno Setup Graphical Installer (`installer/engram.iss`)

For GUI installations, the Inno Setup script is configured with:
```pascal
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
DefaultDirName={userpf}\Programs\engram
```
This guarantees that standard non-elevated user accounts can safely install Engram without administrative prompts or UAC escalation.

---

## 2. Fast Verification

After installation, verify the binary from any terminal:
```bash
engram --version
```
Expected output:
```
engram 1.5.4
commit: 7106a2f369fe79faf5f95449378a900ab8e8eddf
rustc: rustc 1.99.0 (b940084d7 2026-09-28)
profile: release
```

---

## 3. Integrating with AI Coding Agents (MCP Configuration)

Engram runs as a local Model Context Protocol (MCP) server over standard input/output (`stdio`), making it instantly compatible with all major MCP-enabled coding agents.

### 3.1 Claude Desktop Configuration
Add the following snippet to your Claude Desktop configuration file:
- **Windows:** `%APPDATA%\Claude\claude_desktop_config.json`
- **macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "engram": {
      "command": "engram",
      "args": ["mcp"],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

### 3.2 Cursor / VS Code / Cline Configuration
Add to your project or global `.cursor/mcp.json` or `cline_mcp_settings.json`:
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

### 3.3 Antigravity / Gemini Agent Configuration
Engram can be invoked directly by agents via the command line or registered as a local tool provider:
```bash
engram context --task "Explain auth flow" --budget 4000
```

---

## 4. MCP Tools Exposed by Engram

When running in MCP mode (`engram mcp`), Engram exposes three high-efficiency tools:

### `engram_get_context`
Compiles an optimized, AST-ranked context window tailored to the agent's task.
- **Parameters:**
  - `task` (string, required): Description of the agent's current objective.
  - `detail_level` (string, optional: `"full"` | `"compact"` | `"summary"`): Controls token density. Default is `"full"`, while `"compact"` delivers up to 77% token savings for exploratory turns.
  - `symbols` (array of strings, optional): Specific symbol names to prioritize.
  - `max_tokens` (integer, optional): Maximum token budget ceiling (default: 4000).
- **Adaptive Bypass:** Automatically returns a trivial bypass response (`0 tokens`) if the task is self-contained or a simple query.

### `engram_record_decision`
Records an architectural or engineering decision into persistent memory (`.engram/memory.yaml`), enabling future turns to maintain architectural consistency without re-reading extensive documentation.
- **Parameters:**
  - `topic` (string, required): Short category or domain (e.g., `"auth"`, `"database"`).
  - `decision` (string, required): The agreed pattern or rule.
  - `rationale` (string, required): The reason for this decision.

### `engram_index_repository`
Forces an incremental re-index of the repository's AST symbol tree.
- **Parameters:**
  - `path` (string, optional): Subpath to reindex (default: workspace root).

---

## 5. First-Time User Quickstart

In any repository directory:

```bash
# 1. Initialize Engram in the workspace (creates .engram directory)
engram init

# 2. Index the workspace (parses Rust, Python, TypeScript, Go files)
engram index

# 3. Test context compilation directly in terminal
engram context --task "Implement payment gateway retry logic" --budget 2000

# 4. View persistent architectural memory
engram memory list
```
