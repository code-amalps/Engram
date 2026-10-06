# 🧠 Engram

> **Persistent engineering memory and change intelligence for AI coding agents.**

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built_with-Rust-orange.svg)](https://www.rust-lang.org/)
[![MCP Ready](https://img.shields.io/badge/Protocol-MCP-purple.svg)](https://modelcontextprotocol.io/)
[![Tests](https://img.shields.io/badge/Tests-17%20Passed-brightgreen.svg)]()

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

---

## ⚡ 1-Click Verified Agent Setup (MCP)

No more manually editing JSON files! Engram automatically configures MCP integrations while safely preserving all third-party servers:

```bash
# List supported integrations
engram setup --list

# Auto-configure Cursor (.cursor/mcp.json)
engram setup cursor

# Auto-configure Claude Desktop
engram setup claude

# Auto-configure Windsurf
engram setup windsurf

# Check configuration & live MCP health
engram setup cursor --status
```

---

## 📦 Package Ecosystem & Reverse Dependency Intelligence

```bash
# Inspect declared vs resolved versions and runnable scripts:
engram ecosystem

# Reverse dependency query: "Who will break if I change this file/symbol?"
engram impact projects/src/app/settings.actions.ts

# Context compilation with full provenance reasons:
engram context "settings effect" --explain
```

---

## 🏛️ Project Constitution & Team Rules

Define architectural constraints in `.engram/constitution.yaml`:

```yaml
schema_version: 1
architecture:
  pattern: modular-clean
rules:
  - id: RULE-001
    name: "No direct fetch in components"
    type: PATTERN
    severity: WARNING
    target_files: "**/*.component.ts"
    pattern: "fetch\\("
    description: "Use dedicated ApiService or state effects instead of direct window.fetch."
  - id: RULE-002
    name: "Domain boundary isolation"
    type: DEPENDENCY
    severity: ERROR
    source: "src/domain/**"
    forbidden: "src/infrastructure/**"
```

* **Advisory Checks:** Run `engram validate <task_id>` to check working tree changes against active rules.
* **Auditable Overrides:** Record an explicit override with rationale:
  ```bash
  engram decision <task_id> OVERRIDE "Migration exception approved by team lead" --rule-id RULE-001
  ```
  Overrides are permanently preserved in Engineering Memory.

---

## 💻 CLI Commands Reference

| Command | Description | Example |
| :--- | :--- | :--- |
| `engram init` | Initializes `.engram/` config & constitution | `engram init` |
| `engram .` or `engram analyze` | Analyzes repository AST & updates SQLite WAL | `engram .` |
| `engram setup <client>` | 1-Click MCP setup (Cursor, Claude, Windsurf) | `engram setup cursor` |
| `engram ecosystem` | Discovers frameworks, lockfile versions & scripts | `engram ecosystem` |
| `engram context "<task>"` | Compiles context under token budget (`--explain`) | `engram context "add login" --explain` |
| `engram analyze-task "<task>"` | Starts a tracked task and returns a `task_id` | `engram analyze-task "refactor auth"` |
| `engram contract <task_id>` | Generates pre-edit Change Contract | `engram contract task_abc123` |
| `engram impact <id_or_file>` | Compares live diff or queries reverse dependencies | `engram impact OrderService` |
| `engram validate <task_id>` | Advisory checks for scope expansion & rules | `engram validate task_abc123` |
| `engram decision <id> <dec> <msg>`| Records approval/override rationale | `engram decision task_1 OVERRIDE "Reason"` |
| `engram update-memory <task_id>` | Promotes change to durable historical memory | `engram update-memory task_abc123` |
| `engram mcp` | Starts background JSON-RPC 2.0 MCP server | `engram mcp` |

---

## 🔒 Security & Privacy

* **100% Local:** Engram runs entirely on your local machine. No code, tokens, or project files are ever sent to external cloud servers.
* **Secret Redaction:** Deterministic regex pattern matching redacts API keys (`sk-...`, `ghp_...`), JWTs, database connection strings, and passwords before any file excerpt enters the index.
* **Smart Ignores:** `.engramignore` excludes vendored folders (`node_modules`), build output (`dist/`, `target/`), and sensitive `.env*` files by default.

---

## 🧪 Testing

Engram includes a comprehensive automated test suite covering Tree-sitter parsers, token budgeting, SQLite WAL bi-temporal storage, graph traversal, and MCP stdio protocols:

```bash
cargo test
```

All **17 tests** run and pass in ~3 seconds:
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
test test_setup_targets_list ... ok
test test_setup_dry_run_does_not_modify_disk ... ok
test test_package_json_and_lockfile_intelligence ... ok
test test_constitution_rules_and_overrides ... ok
test test_setup_safe_merge_preserves_unrelated_servers ... ok
test test_graph_traversal_and_reverse_dependencies ... ok
```

---

## 📜 Copyright & License

Copyright (C) 2026 **Amal P S**.

This software is licensed under the **[GNU Affero General Public License v3.0 (AGPL-3.0-or-later)](LICENSE)**.

### Freedom for Developers & Open Source
* **100% Free & Open Source:** Developers, hobbyists, students, and open-source contributors are completely free to inspect, download, use, modify, and build upon Engram without paying any fee.
* **Network & Copyleft Protection:** Any modified versions hosted as a network service or redistributed must remain free and open-source under the AGPLv3.
* **Attribution Required:** Under Sections 5(d) and 7 of the AGPLv3, all author attributions and copyright notices for **Amal P S** must remain intact in all copies, distributions, and interactive interfaces.

### Commercial & Enterprise Licensing
If your organization wishes to use, embed, or distribute Engram under proprietary terms without being bound by the copyleft provisions of the AGPLv3, a separate commercial license is required.

For enterprise inquiries, custom deployments, or commercial licensing:
* **Author:** Amal P S
* **GitHub:** [https://github.com/code-amalps](https://github.com/code-amalps)
