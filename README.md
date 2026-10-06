# 🧠 Engram

> **Persistent engineering memory and change intelligence for AI coding agents.**

[![License: PolyForm Noncommercial 1.0.0](https://img.shields.io/badge/License-PolyForm_Noncommercial_1.0.0-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built_with-Rust-orange.svg)](https://www.rust-lang.org/)
[![MCP Ready](https://img.shields.io/badge/Protocol-MCP-purple.svg)](https://modelcontextprotocol.io/)
[![Tests](https://img.shields.io/badge/Tests-11%20Passed-brightgreen.svg)]()

Engram is a blazingly fast, standalone tool written in **Rust** that acts as an **external brain and safety guard** for AI coding assistants (like Cursor, Claude, Windsurf, and Copilot). 

Instead of letting an AI agent burn thousands of tokens rediscovering your repository on every prompt—or accidentally modifying unrelated files—Engram understands your codebase globally, gives the agent the exact minimal context it needs, and verifies that its edits stayed inside safe boundaries.

---

## 🧸 Explain Like I'm 5 (What is Engram?)

Imagine hiring a super-smart robot assistant to fix a toy in your bedroom:

1. **Without Engram:** Every single day, the robot wakes up with **amnesia**. It searches every drawer, closet, and toy box just to find a screwdriver (burning time and money). Then, while fixing the toy, it accidentally disassembles your television set because nobody told it where to stop.
2. **With Engram:** Engram is the house blueprint and memory journal. When you ask the robot to fix a toy, Engram says:  
   *"The toy is on the shelf. Here are the 2 screws you need to touch. Do not touch the television."*  
   And after the robot finishes, Engram checks its work and records what was fixed so the robot remembers tomorrow!

---

## ⚡ Key Highlights

* 🏎️ **Blazingly Fast:** Re-analyzes a 140+ file codebase in **< 15 milliseconds** using composite content-addressed caching.
* 🪶 **Featherweight:** Runs quietly in the background using **< 20 MB of RAM** with zero garbage collection pauses.
* 📦 **Single Static Binary:** Self-contained executable (`engram.exe` on Windows or `engram` on Mac/Linux) with **zero external dependencies**—no Node.js, Python, or .NET runtime needed.
* 🛡️ **Blast-Radius Guard:** Compares live `git diff` against a pre-change contract to catch scope expansion before broken code is merged.
* 🔌 **Native MCP Support:** Plugs directly into **Cursor, Claude Desktop, Claude Code, and Windsurf** via standard Model Context Protocol (MCP).
* 🌳 **Multi-Language AST:** Native Tree-sitter parsers for **TypeScript, JavaScript, Python, and C#**.

---

## 🔄 The Engram Loop

```text
       TASK ("Add CSV export")
                 │
                 ▼
       ┌───────────────────┐
       │  Task Grounding   │  <- BM25 lexical tokenization & symbol resolution
       └─────────┬─────────┘
                 │
                 ▼
       ┌───────────────────┐
       │  Change Contract  │  <- Pre-edit boundary (Allowed files & symbols)
       └─────────┬─────────┘
                 │
                 ▼
       ┌───────────────────┐
       │ Context Compiler  │  <- Delivers minimal sufficient context under token budget
       └─────────┬─────────┘
                 │
                 ▼
       ┌───────────────────┐
       │   AI Agent Edits  │  <- Cursor / Claude writes code
       └─────────┬─────────┘
                 │
                 ▼
       ┌───────────────────┐
       │   Change Guard    │  <- Validates Git diff against contract (Catch scope drift!)
       └─────────┬─────────┘
                 │
                 ▼
       ┌───────────────────┐
       │  Memory Promotion │  <- Stores accepted decisions into durable memory
       └───────────────────┘
```

---

## 🚀 Quickstart: Install in 60 Seconds

### Option A: Build from Source (Requires Rust)

```bash
git clone https://github.com/code-amalps/Engram.git
cd Engram
cargo build --release
```

The compiled standalone binary will be at:
* **Windows:** `target/release/engram.exe`
* **macOS / Linux:** `target/release/engram`

Copy `engram` to a directory on your system `PATH` (e.g. `~/.cargo/bin` or `C:\Windows\System32`), or alias it:

```powershell
# In PowerShell:
Set-Alias engram "C:\path\to\Engram\target\release\engram.exe"
```

---

## 📖 3-Minute Tutorial: Using Engram

Navigate to any repository you want to analyze:

```bash
cd /path/to/your/project
```

### 1. Initialize Engram
```bash
engram init
```
*Creates `.engram/constitution.yaml` (for team architectural rules), `.engram/config.yaml`, and `.engramignore`.*

### 2. Analyze the Repository
```bash
engram .
```
*Parses all source files using Tree-sitter, extracts symbols and imports, and stores them in a local SQLite database in under a second!*

```json
{
  "revision": "1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2",
  "indexed_files": 141,
  "changed_files": 141,
  "symbols": 224,
  "removed_files": 0
}
```

*Run `engram .` a second time, and it will return in 10ms with `"changed_files": 0` thanks to cache invalidation!*

### 3. Compile Context for a Task
```bash
engram context "settings effect dark theme" --token-budget 800
```
*Engram calculates BM25 relevance, locates the exact files and symbol signatures, and slices the context to fit within your token budget.*

### 4. Ground a Task & Generate a Change Contract
```bash
# Analyze the task to get a task_id:
engram analyze-task "toggle dark theme in settings"
```
*(Copy the `task_id` from the output, e.g. `task_0df9d0952aa64bdf`)*

```bash
# Create the Change Contract:
engram contract task_0df9d0952aa64bdf
```
*This defines the expected files and symbols allowed to be modified.*

### 5. Validate Edits (The Change Guard)
While the AI agent is working, run:
```bash
engram validate task_0df9d0952aa64bdf
```

* If the agent edited allowed files $\to$ **`warnings: []`, `valid: true`**.
* If the agent edited an unexpected file outside the contract $\to$ **`warnings: [{"code": "SCOPE_EXPANSION"}]`**!

### 6. Record Decisions & Promote Memory
Once satisfied with the change:
```bash
engram decision task_0df9d0952aa64bdf accepted "Clean implementation"
engram update-memory task_0df9d0952aa64bdf
```
*The decision and rationale are permanently promoted to local engineering memory.*

---

## 🔌 Connecting to AI Agents via MCP

Engram includes a high-performance **Model Context Protocol (MCP)** server over `stdio`. It connects directly to Cursor, Claude Desktop, Claude Code, and Windsurf.

### 1. Cursor (`.cursor/mcp.json`)
Add this to your project's `.cursor/mcp.json` or your global Cursor settings:

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

### 2. Claude Desktop (`claude_desktop_config.json`)
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

### 3. Claude Code CLI
```bash
claude mcp add engram engram mcp
```

### Native Tools Provided to Agents:
* `engram_get_context(task, token_budget)`: Fetches grounded symbols and file excerpts.
* `engram_analyze_task(task, token_budget)`: Grounds a task and logs initial state.
* `engram_create_change_contract(task_id)`: Generates the expected change boundary.
* `engram_get_impact(task_id)`: Measures live diff against contract.
* `engram_validate_change(task_id)`: Advisory check for unexpected files and architectural drift.
* `engram_record_decision(task_id, decision, reason)`: Records developer rationale.
* `engram_update_memory(task_id)`: Promotes accepted outcomes into durable memory.

---

## 💻 CLI Commands Reference

| Command | Description | Example |
| :--- | :--- | :--- |
| `engram init` | Initializes `.engram/` config & constitution | `engram init` |
| `engram .` or `engram analyze` | Analyzes repository AST & updates SQLite index | `engram .` |
| `engram context "<task>"` | Compiles context under a token budget | `engram context "add login" --token-budget 1000` |
| `engram analyze-task "<task>"` | Starts a tracked task and returns a `task_id` | `engram analyze-task "refactor auth"` |
| `engram contract <task_id>` | Generates pre-edit Change Contract | `engram contract task_abc123` |
| `engram impact <task_id>` | Compares live working tree against baseline | `engram impact task_abc123` |
| `engram validate <task_id>` | Checks for scope expansion and rule drift | `engram validate task_abc123` |
| `engram decision <id> <dec> <msg>`| Records approval/override rationale | `engram decision task_abc123 accepted "LGTM"` |
| `engram update-memory <task_id>` | Promotes change to durable historical memory | `engram update-memory task_abc123` |
| `engram mcp` | Starts background JSON-RPC MCP server | `engram mcp` |

---

## 🔒 Security & Privacy

* **100% Local:** Engram runs entirely on your local machine. No code, tokens, or project files are ever sent to external cloud servers.
* **Secret Redaction:** Deterministic regex pattern matching redacts API keys (`sk-...`, `ghp_...`), JWTs, database connection strings, and passwords before any file excerpt enters the index.
* **Smart Ignores:** `.engramignore` excludes vendored folders (`node_modules`), build output (`dist/`, `target/`), and sensitive `.env*` files by default.

---

## 🧪 Testing

Engram comes with a comprehensive automated test suite covering Tree-sitter parsers, token budgeting, SQLite bi-temporal versioning, and MCP stdio protocol handshakes:

```bash
cargo test
```

All 11 tests run and pass in ~2 seconds:
```text
test tests::test_language_detection ... ok
test tests::test_analysis_key_composition ... ok
test tests::test_evidence_decay_and_precedence ... ok
test tests::test_matches_pattern ... ok
test tests::test_tokenization_and_estimation ... ok
test tests::test_secret_redaction ... ok
test tests::test_tree_sitter_python_extraction ... ok
test tests::test_tree_sitter_csharp_extraction ... ok
test tests::test_tree_sitter_typescript_extraction ... ok
test test_end_to_end_lifecycle ... ok
test test_mcp_protocol_handshake_and_tools ... ok
```

---

## 📜 Copyright & License

Copyright (c) 2026 **Amal P S**.

This project is licensed under the **[PolyForm Noncommercial License 1.0.0](LICENSE)**.

* **Free for Individuals & Noncommercial Use:** You are completely free to inspect, download, modify, use, and run Engram for personal projects, open-source work, education, and non-commercial research without paying any fee.
* **Commercial Use:** Use of Engram by commercial entities or for activities primarily intended for commercial advantage or monetary compensation requires a separate commercial license.

For commercial licensing inquiries, please contact the author via [GitHub](https://github.com/code-amalps).
