# Troubleshooting Engram

This guide covers common diagnostic commands, error codes, and fixes for Engram.

---

## 1. Quick Diagnostics: `engram doctor`

Run `engram doctor` to perform an automated health check on your repository setup:

```bash
engram doctor
```

What `engram doctor` inspects:
- **SQLite Database Integrity:** Checks WAL state and table schemas in `.engram/index.db`.
- **Telemetry Event Chain:** Validates cryptographic integrity of `.engram/events/` (`EVENT_CHAIN_VALID`, `EVENT_CHAIN_BROKEN`, `EVENT_MISSING`).
- **Git Repository State:** Checks for detached HEAD, shallow history, or untracked changes.
- **Constitution Schema:** Validates `.engram/constitution.yaml` syntax.
- **Durable Memory Integrity:** Validates YAML structure in `.engram/memory.yaml`.

---

## 2. Common Issues & Solutions

### A. CI Fails with Exit Code 4 (`BASELINE_UNAVAILABLE`)
- **Cause:** CI runner used a shallow Git clone (`git clone --depth 1`), so the baseline commit hash is missing from history.
- **Solution:** In GitHub Actions, configure `actions/checkout` with `fetch-depth: 0`:
  ```yaml
  - uses: actions/checkout@v4
    with:
      fetch-depth: 0
  ```

### B. MCP Tools Not Showing Up in Cursor / Claude / Windsurf
- **Check 1: Verify MCP server runs manually:**
  ```bash
  engram setup cursor --status
  ```
- **Check 2: Ensure binary is in your system PATH:**
  - Linux / macOS: `~/.engram/bin/engram`
  - Windows: `%USERPROFILE%\.engram\bin\engram.exe`
- **Check 3: Inspect client configuration file:**
  - Cursor: `.cursor/mcp.json`
  - Claude Desktop: `%APPDATA%\Claude\claude_desktop_config.json` (Windows) or `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS)
  - Windsurf: `~/.codeium/windsurf/mcp_config.json`

### C. Resetting the Local Index Without Losing Memory
If you suspect cache corruption or changed `.engramignore` rules:
- **Safe to delete:** `.engram/index.db`, `.engram/index.db-wal`, `.engram/index.db-shm`, `.engram/cache/`.
- **Permanent files to KEEP:** `.engram/memory.yaml` (your team's engineering decisions) and `.engram/constitution.yaml` (your architectural rules).
- **To re-index:**
  ```bash
  engram .
  ```
  This completely rebuilds the SQLite symbol graph from scratch without losing any saved memories.

### D. Recovery Session Locked / CAS Mismatch
- If an agent task aborted mid-execution, a lock may remain on `.engram/cache/recovery/<contract_id>.json`.
- Run:
  ```bash
  engram recover <contract_id> --force
  ```
  or delete the specific temporary JSON file in `.engram/cache/recovery/`.
