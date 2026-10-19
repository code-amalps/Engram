# Engram — VS Code Extension

Official Visual Studio Code extension for [Engram](https://github.com/code-amalps/Engram): Local engineering memory, context compilation, and Change Contract guard for AI coding agents.

## Overview

The Engram extension provides **zero-friction AI agent integration** connecting Visual Studio Code to the authoritative local `engram` Rust engine.

### Zero-Friction Features:
- **Zero Manual Configuration:** When you open any supported project (Angular, .NET, Python, Rust, Go, Java, TypeScript), Engram automatically identifies the workspace and offers 1-click MCP setup.
- **Native VS Code MCP Server Registration:** Automatically registers Engram in `.vscode/mcp.json` using VS Code's native `servers` schema and `${workspaceFolder}` workspace expansion.
- **Idempotent Configuration Preservation:** Preserves all existing third-party MCP servers (GitHub Copilot, PostgreSQL, SQLite, etc.) and file comments with zero data loss.
- **Live MCP Handshake Diagnostics:** Spawns and verifies the stdio MCP protocol (`2024-11-05`), validating that all 15 governed engineering memory tools are discovered and ready for agent invocation.
- **1-Click Setup & Repair (`engram.fixSetup`):** Single-click automated repair diagnosing missing registrations, path mismatches, or corrupted configurations.
- **Opt-in Copilot Instructions (`engram.syncAgentInstructions`):** Safely adds or updates bounded, reversible agent instructions in `.github/copilot-instructions.md`.
- **Deterministic Binary Discovery:** Locates and validates the Engram binary across PATH, standard OS locations, environment variables, or explicit settings.
- **Context Compilation & Memory Explorer:** Compile rich engineering context budgets and inspect durable architectural decisions directly from the Activity Bar.

## Supported Coding Agents
- **GitHub Copilot Agent Mode / VS Code MCP:** Direct integration via `.vscode/mcp.json` (`servers` stdio schema).
- **Cursor IDE:** Compatible via `engram setup --client cursor` (`.cursor/mcp.json`).
- **Windsurf IDE:** Compatible via `engram setup --client windsurf` (`mcp_config.json`).
- **Claude Desktop:** Compatible via `engram setup --client claude` (`claude_desktop_config.json`).

## Commands

- `Engram: Check Setup & Agent Health` (`engram.checkSetup`) — Evaluates binary discovery, workspace detection, MCP registration, and live tool discovery.
- `Engram: 1-Click Setup & Repair` (`engram.fixSetup`) — Automatically registers the MCP server and verifies live tool availability.
- `Engram: Configure Copilot Instructions` (`engram.syncAgentInstructions`) — Configures bounded project guidelines for GitHub Copilot in `.github/copilot-instructions.md`.
- `Engram: Compile Context` (`engram.compileContext`) — Compiles project context and invariants for a specific task.
- `Engram: Check Health (Doctor)` (`engram.checkHealth`) — Inspects memory liveness, scope resolution, and cryptographic hash chain integrity.
- `Engram: Locate Binary` (`engram.locateBinary`) — Re-runs binary discovery and displays the active version and path.
- `Engram: Show Output Channel` (`engram.showOutputChannel`) — Opens detailed execution and diagnostic logs.

## Requirements

The extension requires the `engram` CLI binary installed on your system.

To install Engram:
- **Windows (PowerShell):**
  ```powershell
  irm https://github.com/code-amalps/Engram/releases/latest/download/install.ps1 | iex
  ```
- **macOS / Linux:**
  ```bash
  curl -fsSL https://github.com/code-amalps/Engram/releases/latest/download/install.sh | bash
  ```

Alternatively, configure an explicit binary path in VS Code Settings:
`"engram.binaryPath": "/absolute/path/to/engram"`

## Extension Settings

* `engram.binaryPath`: Explicit absolute path to the `engram` executable. If specified and invalid, the extension fails closed and will not fall back to other paths.
* `engram.timeoutMs`: Timeout in milliseconds for CLI commands (default: 30000).

## Development & Testing

```bash
cd extensions/vscode
npm install
npm run compile
npm test
npm run package
```
