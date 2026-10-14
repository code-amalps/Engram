# Show HN / Dev.to: Engram — Repository memory and change boundaries for AI coding agents

Hey everyone,

I built **Engram** to solve a problem that kept biting me when pair-programming with AI coding assistants (like Cursor, Claude Desktop, and Windsurf):

1. **Context Amnesia & Rediscovery Cost:** On almost every non-trivial prompt, the agent spends thousands of tokens running repeated searches just to locate files and symbol definitions it saw yesterday.
2. **Silent Scope Expansion:** While implementing a targeted feature in one module, the agent occasionally modifies an unrelated configuration file, breaks a downstream dependency, or touches files it was never asked to touch.
3. **Lost Architectural Decisions:** When the team agrees on a rule (e.g., "Always stream CSV exports rather than buffering in memory"), that rationale lives in a Slack thread or a closed PR, invisible to the AI the next day.

Instead of another cloud service or embedding database wrapper, I wanted a fast, single-binary local CLI and MCP server in Rust with zero cloud dependencies.

---

## What Engram Does

- **AST-Aware Context Compilation:** Uses Tree-sitter parsers to extract symbols, classes, interfaces, methods, and dependencies into a local SQLite WAL database. For any prompt, it BM25-ranks and slices code to fit an exact token budget with relevance provenance (`engram context "<task>"`).
- **Change Impact Cones:** Predicts blast radius across 7 deterministic relationship types (`DIRECT`, `IMPLEMENTATION`, `CALLER`, `TEST`, `IMPORT_DEPENDENT`, `CONFIGURATION`, `HISTORICAL`).
- **Elastic Change Contracts & Change Guard:** Establishes pre-edit boundaries (`EXPECTED`, `EXPANSION`, `UNEXPECTED`, `BOUNDARY_VIOLATION`). Evaluates working-tree or staged Git diffs via pre-commit hooks or CI (`engram contract check`).
- **Stateful Recovery:** When an agent breaches a boundary, Engram emits a machine-readable recovery plan (`engram recover`) with bounded retries (max 2 attempts) so agents can repair mistakes autonomously without infinite loops. Engram never edits source code directly.
- **Durable Engineering Memory:** Preserves architectural decisions and rationale with scoped inheritance (`Symbol` $\to$ `Module` $\to$ `Repo`). Has a strict human-promotion gate to permanently prevent memory poisoning.
- **1-Click MCP Setup:** Automatically configures Cursor (`.cursor/mcp.json`), Claude Desktop, and Windsurf while safely preserving existing third-party servers.

---

## A Real End-to-End Walkthrough

Here is what using Engram looks like in practice:

```bash
# 1. Initialize in your repo
engram init
engram .

# 2. Get focused context for a task
engram context "add refund capability to BillingService" --token-budget 800

# 3. Create a change boundary before the agent edits
engram contract create "add refund capability to BillingService"
# Output: Contract ctr_a7f92b created. Expected files: [src/billing.rs], Forbidden: [secrets/**]

# 4. Agent edits code...
# If the agent modifies src/billing.rs and accidentally touches secrets/vault.rs:
engram contract check
# Output:
# ❌ Change Guard Check: FAILED (Exit Code 1)
# Boundary Violations: [secrets/vault.rs]

# 5. Agent asks for a structured recovery plan
engram recover ctr_a7f92b
# Output:
# Plan: Revert unauthorized changes to secrets/vault.rs. Preserve authorized changes in src/billing.rs. Rerun check.

# 6. Agent reverts secrets/vault.rs and reruns:
engram contract check
# Output: ✅ PASS (Exit Code 0)

# 7. Record the rationale into durable memory
engram memory record "Billing Refund Policy" "Refunds must emit audit events" "Compliance requirement"
```

---

## Current Supported Languages

Engram currently has native Tree-sitter extractors for:
- **Rust, Go, TypeScript, JavaScript, Python, C#, and Java** (including Maven `pom.xml` and Gradle `build.gradle` dependency parsing).

---

## Known Limitations & Honest Trade-offs

- **Boundary Enforcement $\ne$ Semantic Correctness:** Change Guard verifies *where* changes happened and whether they violated contracts or Constitution rules. It cannot prove that the code compiles or is logically bug-free.
- **Regex Secret Redaction:** We use pattern matching for API keys and connection URIs, but non-standard proprietary secrets won't be caught by regex.
- **Precision vs. Recall:** Context compilation prioritizes high recall (~95%) over precision (~61%), so it may occasionally include related interface definitions or tests you don't strictly need.

---

## How to Try It & How You Can Help

Engram is 100% open source under the **AGPL-3.0-or-later** license.

### Install:
- **Windows (PowerShell):**
  ```powershell
  iwr -useb https://raw.githubusercontent.com/code-amalps/Engram/main/install.ps1 | iex
  ```
- **macOS / Linux (Bash):**
  ```bash
  curl -fsSL https://raw.githubusercontent.com/code-amalps/Engram/main/install.sh | sh
  ```
- **From Source:**
  ```bash
  cargo install --path .
  ```

### What I Need Feedback On:
If you use Cursor, Claude Desktop, or Windsurf:
1. Run `engram setup cursor` (or `claude` / `windsurf`).
2. Try a real coding task in your project.
3. Does the compiled context feel relevant? Did the Change Guard catch any unintended file edits? Where did the parser stumble?

Please open an issue on GitHub:
👉 **Repository:** [https://github.com/code-amalps/Engram](https://github.com/code-amalps/Engram)

Thanks for reading, and I'd love to hear your thoughts and critiques!
