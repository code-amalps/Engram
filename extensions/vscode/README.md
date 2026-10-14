# Engram — VS Code Extension

Official Visual Studio Code extension for [Engram](https://github.com/code-amalps/Engram): Local engineering memory, context compilation, and Change Contract guard for AI coding agents.

## Overview

The Engram extension connects your Visual Studio Code IDE to the authoritative, local `engram` Rust engine.

### Milestone 1 Features:
- **Deterministic Binary Discovery:** Locates and validates an existing Engram binary across Windows, macOS, and Linux.
- **Strict Compatibility Enforcement:** Verifies that the detected binary satisfies semantic version requirements (`>= 1.4.0, < 2.0.0`).
- **Secure Process Execution:** Executes CLI commands using argument vectors with `shell: false` (zero shell interpolation).
- **Native Status Bar:** Real-time visibility into the Engram binary status.
- **Health Check Command:** Run `Engram: Check Health (Doctor)` to inspect memory liveness, scope resolution, and cryptographic hash chain integrity.
- **Dedicated Output Channel:** Clean, structured logging with runtime sanitization.

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

Alternatively, configure the explicit path in VS Code Settings:
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
