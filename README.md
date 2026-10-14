<p align="center">
  <img src="assets/engram-banner.png" alt="Engram — Repository context, engineering memory, and boundaries for AI coding agents" width="720" />
</p>

<p align="center">
  <strong>Repository context, engineering memory, and explicit boundaries for AI coding agents.</strong>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-AGPL_v3-blue.svg" alt="License: AGPL v3"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Built_with-Rust-orange.svg" alt="Built with Rust"></a>
  <a href="https://modelcontextprotocol.io/"><img src="https://img.shields.io/badge/Protocol-MCP-purple.svg" alt="MCP Server"></a>
  <img src="https://img.shields.io/badge/Tests-130%2B%20Passing-brightgreen.svg" alt="Tests: 130+ Passing">
</p>

---

## 1. What is Engram?

Engram is a standalone, local CLI tool and Model Context Protocol (MCP) server written in **Rust**. It acts as a repository intelligence engine and change boundary system for AI coding assistants such as **Cursor, Claude Desktop, and Windsurf**.

Instead of letting an AI agent burn thousands of tokens blindly searching your repository—or accidentally modifying unrelated files—Engram provides:
1. **Precise, AST-grounded context** tailored to the prompt and token budget.
2. **Change-impact visibility** into downstream callers, interface implementations, and tests.
3. **Change Contracts & Change Guard** to enforce explicit boundaries against unauthorized edits.
4. **Stateful Automated Recovery** to provide structured repair plans when boundaries are breached.
5. **Durable Engineering Memory** to preserve architectural decisions and rationale across sessions.

---

## 2. The Problems Engram Solves

AI coding agents are capable code writers, but they operate under three major structural limitations:

1. **Repository Discovery Amnesia:** On every new prompt, the agent starts from zero knowledge. It consumes thousands of tokens running broad searches (`find`, `grep`, `dir`), often filling context windows with whole files when only a few function signatures were relevant.
2. **Unintended Scope Expansion:** Coding agents frequently make unrequested edits outside their task—modifying configuration files, tweaking unrelated utility methods, or breaking downstream consumers because they cannot see the full blast radius.
3. **Forgotten Architectural Decisions:** When your team decides *"all database queries must use prepared statements"* or *"OrderService must stream CSV exports"*, that rationale is typically lost in chat history. The next agent session has no memory of past decisions and repeats past anti-patterns.

---

## 3. How Engram Helps

Engram divides repository intelligence into six distinct, cooperating capabilities:

- **Context Intelligence:** Uses Tree-sitter parsers to extract symbols (classes, interfaces, methods, structs) and combines BM25 lexical ranking with graph proximity to deliver minimal sufficient context under a strict token budget.
- **Change-Impact Intelligence:** Analyzes the repository graph across 7 deterministic relationship types (`DIRECT`, `IMPLEMENTATION`, `CALLER`, `TEST`, `IMPORT_DEPENDENT`, `CONFIGURATION`, `HISTORICAL`) to show downstream blast radius before code is written.
- **Change Contracts:** Establishes pre-edit boundaries declaring `expected_files`, `expected_symbols`, and `forbidden_boundaries`, anchored to the current Git baseline commit.
- **Change Guard:** Evaluates working-tree or staged Git diffs against active contracts and Project Constitution rules (`.engram/constitution.yaml`) through CLI commands, pre-commit hooks, or CI workflows.
- **Stateful Automated Recovery:** When an agent breaches a boundary, Engram generates a structured repair plan (`revert_mutation`, `preserve_requested_change`, `rerun`) with a strict 2-attempt limit to prevent infinite loops. Engram never modifies code directly.
- **Engineering Memory:** Preserves architectural decisions with scoped hierarchy (`Symbol` $\to$ `Package` $\to$ `Module` $\to$ `Repository` $\to$ `Global`). A mandatory human promotion gate permanently prevents memory poisoning.

---

## 4. A Real End-to-End Example

Here is a reproducible workflow showing how Engram guides an AI coding agent:

### Step 1: Initialize and Analyze Repository
```bash
engram init
engram .
```
Engram parses source files via Tree-sitter and builds a local SQLite symbol graph in `.engram/index.db`:
```json
{
  "revision": "c4f92a10",
  "indexed_files": 142,
  "changed_files": 142,
  "symbols": 226,
  "removed_files": 0
}
```

### Step 2: Compile Minimal Context for a Task
```bash
engram context "add refund capability to BillingService" --token-budget 800
```
Engram returns exact file excerpts, method signatures, and relevance breadcrumbs fitting inside 800 tokens without loading full unneeded files.

### Step 3: Establish a Change Contract Before Editing
```bash
engram contract create "add refund capability to BillingService" --forbidden "secrets/**"
```
Engram creates Change Contract `ctr_829104` declaring `src/billing.rs` as expected and `secrets/**` as forbidden.

### Step 4: Agent Edits Code & Change Guard Evaluates Diffs
Suppose the agent modifies `src/billing.rs` but also accidentally creates or modifies `secrets/vault.rs`:
```bash
engram contract check
```
Change Guard immediately detects the boundary breach:
```text
❌ Change Guard Check: FAILED (Exit Code 1)
Contract ID:         ctr_829104
Boundary Violations: [secrets/vault.rs]
Status:              UNAUTHORIZED_MUTATION_DETECTED
```

### Step 5: Agent Requests Recovery Plan
Instead of guessing or failing silently, the agent calls `engram recover`:
```bash
engram recover ctr_829104
```
Engram emits a structured, machine-readable recovery plan:
```json
{
  "contract_id": "ctr_829104",
  "attempt": 1,
  "status": "IN_PROGRESS",
  "plan": {
    "actions": [
      {
        "action_type": "revert_mutation",
        "target": "secrets/vault.rs",
        "description": "Revert unauthorized mutation at `secrets/vault.rs`"
      },
      {
        "action_type": "preserve_requested_change",
        "target": "src/billing.rs",
        "description": "Preserve authorized change within `src/billing.rs`"
      },
      {
        "action_type": "rerun",
        "target": "engram postflight",
        "description": "Rerun check to verify compliance"
      }
    ]
  }
}
```

### Step 6: Agent Repairs & Passes Guard
The agent reverts `secrets/vault.rs` while keeping the billing change:
```bash
engram contract check
# Output: ✅ PASS (Exit Code 0)
```

### Step 7: Record the Decision into Engineering Memory
Once verified by the engineer, promote the rationale to durable memory:
```bash
engram memory record "Billing Refund Policy" "Refunds must emit audit events and never touch vault directly" "Security & audit requirement" --files "src/billing.rs"
```
On subsequent prompts touching `BillingService`, Engram automatically includes this rationale.

---

## 5. Why is Engram Different?

| Dimension | Plain Grep / File Search | Vector Database / Code RAG | Generic MCP File Tools | **Engram** |
|---|---|---|---|---|
| **Understands Code Structure** | ❌ Text matching only | ⚠️ Approximate embeddings | ❌ Read/write tool only | **✅ Polyglot Tree-Sitter AST & Symbol Graph** |
| **Downstream Impact** | ❌ None | ❌ None | ❌ None | **✅ 7-category blast radius & call-path tracing** |
| **Change Boundaries** | ❌ None | ❌ None | ❌ None | **✅ Elastic Change Contracts & Guard** |
| **Violation Recovery** | ❌ None | ❌ None | ❌ None | **✅ Bounded recovery state machine (max 2 retries)** |
| **Architectural Rules** | ❌ None | ❌ None | ❌ None | **✅ Project Constitution (`.engram/constitution.yaml`)** |
| **Memory Poisoning Defense**| ❌ None | ❌ Automatically ingests hallucinations | ❌ None | **✅ Mandatory human-promotion gate** |
| **Network & Privacy** | ✅ Local | ⚠️ Often requires cloud embeddings | ⚠️ Depends on tool | **✅ 100% Local (Zero outbound telemetry)** |

---

## 6. Language & Integration Support

### Supported Languages

Engram features native Tree-sitter parsers embedded into the binary:

| Language | AST Extraction | Symbols Extracted | Manifest / Dependency Semantics | Interface & Test Association |
|---|:---:|---|---|---|
| **Rust** | `tree-sitter-rust` | Functions, structs, enums, traits, impls, modules | `Cargo.toml`, `use`, `mod` | `tests/*.rs` $\to$ `src/*.rs` |
| **Go** | `tree-sitter-go` | Structs, interfaces, methods, functions | Full `go.mod` directives (`require`, `replace`, `exclude`, `toolchain`) | Two-stage interface matching, `*_test.go` |
| **TypeScript** | `tree-sitter-typescript` | Classes, interfaces, types, functions, methods | `package.json`, ES imports, `require` | `*.spec.ts`, `*.test.ts` $\to$ `*.ts` |
| **JavaScript** | `tree-sitter-javascript` | Classes, functions, methods | `package.json`, ES imports, `require` | `*.spec.js`, `*.test.js` $\to$ `*.js` |
| **Python** | `tree-sitter-python` | Classes, functions, methods | `pyproject.toml`, `import`, `from ... import` | `test_*.py`, `*_test.py` |
| **C#** | `tree-sitter-c-sharp` | Classes, interfaces, structs, enums, methods | `.csproj`, `using` directives | `*Test.cs`, `*Tests.cs` $\to$ `*.cs` |
| **Java** | `tree-sitter-java` | Classes, interfaces, records, enums, methods, annotations | Maven `pom.xml` & Gradle `build.gradle` dependencies | `src/test/java/...` $\to$ `src/main/java/...` |

*See [`docs/supported-languages.md`](docs/supported-languages.md) for full capability details.*

### Supported AI Clients (MCP)

Engram runs a background JSON-RPC 2.0 MCP server over `stdio` (`engram mcp`):

- **Cursor:** Automated 1-click configuration via `engram setup cursor`.
- **Claude Desktop:** Automated 1-click configuration via `engram setup claude`.
- **Windsurf:** Automated 1-click configuration via `engram setup windsurf`.
- **Any MCP Client:** Connect by invoking `engram mcp` over `stdio`.

*`engram setup` safely preserves existing third-party MCP servers and never overwrites unrelated configurations.*

---

## 7. Quickstart

### Installation

No compiler, Node.js, or Python runtime required. Downloads the self-contained binary, verifies SHA256 integrity, and adds Engram to your `PATH`:

**Windows (PowerShell):**
```powershell
iwr -useb https://raw.githubusercontent.com/code-amalps/Engram/main/install.ps1 | iex
```

**macOS & Linux (Bash):**
```bash
curl -fsSL https://raw.githubusercontent.com/code-amalps/Engram/main/install.sh | sh
```

**Or Build from Source (Requires Rust 1.85+):**
```bash
git clone https://github.com/code-amalps/Engram.git
cd Engram
cargo build --release
# Binary will be at target/release/engram (or engram.exe on Windows)
```

---

### 5-Minute Safe Evaluation (Try on Any Repository)

```bash
# 1. Navigate to your repository
cd /path/to/your/project

# 2. Initialize configuration (creates .engram/ folder)
engram init

# 3. Analyze repository
engram .

# 4. Compile context for a task
engram context "fix authentication token validation" --token-budget 1000

# 5. Connect to your editor (e.g. Cursor)
engram setup cursor
```

---

## 8. Change Contracts & Change Guard

Engram prevents unintended modifications from silently crossing intended boundaries.

### Local Git Pre-Commit Hook
Install Change Guard into `.git/hooks/pre-commit` (never installed silently):
```bash
# Preview what would be installed
engram contract install-hook --dry-run

# Install or chain with existing pre-commit hooks (Lefthook, Husky)
engram contract install-hook

# Uninstall cleanly
engram contract uninstall-hook
```

When you commit, Change Guard validates staged changes against active contracts.

### GitHub Actions CI Enforcement
Validate pull requests automatically by adding `.github/workflows/engram-guard.yml`:
```yaml
name: Engram Change Guard
on: [pull_request]

jobs:
  change-guard:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - run: |
          curl -fsSL https://raw.githubusercontent.com/code-amalps/Engram/main/install.sh | sh
          engram contract check --base ${{ github.event.pull_request.base.sha }} --check-provenance
```

### Stable Numeric Exit Codes
Change Guard never forces scripts to parse human text:
- `0`: **PASS** — All changes match expected boundaries.
- `1`: **CONTRACT_VIOLATION** — Breached a forbidden boundary, unexpected edit in strict mode, or violated Constitution.
- `2`: **CONTRACT_NOT_FOUND** — No active contract in repository.
- `3`: **CONTRACT_INVALID** — Corrupt contract schema.
- `4`: **BASELINE_UNAVAILABLE** — Baseline commit missing (e.g. shallow clone).
- `5`: **INTERNAL_ERROR** — I/O or Git subprocess failure.

*See [`docs/change-contracts.md`](docs/change-contracts.md) for full lifecycle details.*

---

## 9. Engineering Memory & Architectural Rules

### Project Constitution (`.engram/constitution.yaml`)
Define immutable architectural constraints that override any agent request:
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
    description: "Use ApiService or store effects instead of direct window.fetch."
  - id: RULE-002
    name: "Domain isolation"
    type: DEPENDENCY
    severity: ERROR
    source: "src/domain/**"
    forbidden: "src/infrastructure/**"
```

### Engineering Memory (`.engram/memory.yaml`)
- **Scoped Hierarchy:** Decisions can govern an individual `Symbol`, `Package`, `Module`, or the entire `Repository`.
- **Immutable Supersession:** When a design changes (e.g., `ADR-014` buffered export superseded by `ADR-029` streaming export), the old record is preserved and linked in an audit chain.
- **Zero Poisoning:** Agent suggestions remain uncommitted candidates until explicitly promoted by a human (`engram memory promote <id>`).

---

## 10. Security & Privacy

- **100% Local Execution:** Engram makes **zero outbound network calls** and contains no cloud telemetry. Source code, tokens, and context payloads never leave your workstation.
- **Deterministic Secret Redaction:** Regex pattern matching redacts API keys (`sk-...`, `ghp_...`, `AKIA...`), passwords, connection URIs (`postgres://`, `mongodb://`), and emails before file excerpts enter SQLite storage.
- **Smart Ignores:** Automatically excludes `.env*`, `*.pem`, `*.key`, `.git/`, dependency folders (`node_modules/`, `vendor/`), and build output (`target/`, `dist/`).
- **Heuristic Redaction Notice:** Regex redaction cannot guarantee catching all proprietary or high-entropy secrets. Always keep secrets in `.env` files excluded by `.engramignore`.

*See [`docs/security-and-privacy.md`](docs/security-and-privacy.md) for full security disclosures.*

---

## 11. Benchmark Methodology & Evidence

Engram includes an empirical benchmark engine (`engram bench`). We strictly separate **empirically measured Engram outputs** from **estimated baseline models**:

| Dimension | Measured Engram Value | Baseline Comparison | Evidence Description |
|---|:---:|:---:|---|
| **Discovery Context Tokens** | **210 tokens** | 4,500 tokens (est. scan) | Measured from actual AST context compiler output vs. estimated full-file read baseline across 8 synthetic tasks. |
| **Tool Call Reduction** | **1 call** | 12 calls (est. discovery) | Single context compilation vs. estimated iterative search/grep loop. |
| **Grounding File Recall** | **95.2%** | Target $\ge$ 90% | Measured file recall against annotated ground-truth fixtures. |
| **Composite Grounding F1** | **0.813** | Precision: 61.4% | Conservative inclusion of interfaces/tests prioritizes high recall over strict precision. |
| **Change Contract Accuracy** | **100% (32/32)** | Synthetic mutation suite | Evaluated on defined deterministic mutation fixtures. Evaluates boundary classification logic, not real-world semantic correctness. |
| **Memory Recall** | **100% (vs 0% Control)** | Controlled A/B experiment | Unlearned decision returns 0% recall; promoted decision returns 100% recall on related follow-up tasks. |

*To reproduce locally: run `engram bench --verbose`. See [`docs/benchmarks.md`](docs/benchmarks.md) for the complete audit.*

---

## 12. Known Limitations

- **Boundary Enforcement $\ne$ Semantic Correctness:** Change Guard verifies *where* changes occur and *whether* they violate contracts. It cannot prove that the generated code compiles, is free of logic bugs, or meets functional specifications.
- **Syntactic vs Semantic Parsing:** Tree-sitter performs syntax extraction; it does not run a full compiler type-checker across unimported types.
- **Synthetic Benchmark Corpus:** The built-in benchmark numbers reflect controlled, deterministic fixture suites. Results on arbitrary, complex proprietary monorepos will vary depending on codebase size and modularity.

---

## 13. Testing Evidence

Engram maintains an extensive automated test suite covering Tree-sitter parsers, SQLite WAL storage, Change Contracts, Git hardening (detached HEAD, shallow clones), stateful recovery, and MCP protocols.

```bash
cargo test
```

All **130+ tests** pass across unit and integration suites:
- `analyzer`, `go_parser`, `java_parser`, `contract`, `guard`, `impact` unit tests
- `bench_test`: measured vs estimated separation, grounding recall, mutation accuracy
- `contract_guard_test`: pre-commit hooks, CI base commit comparison, boundary violations
- `recovery_test`: CAS concurrency locks, bounded retry exhaustion, recovery plan formulation
- `drift_test`: scoped staleness, explicit ratification, trend calculations
- `git_hardening_test`: detached HEAD, renames, untracked files, shallow clones
- `phase1_fixtures_bench_test`: all 7 language fixtures and adversarial edge cases
- `mcp_test`: JSON-RPC 2.0 handshake and tool execution

---

## 14. Help Test Engram

We are actively seeking technical feedback from developers using AI coding assistants (Cursor, Claude Desktop, Windsurf, Copilot).

If you test Engram on a real repository, please report:
- Your OS and Engram version (`engram --version`)
- Target repository language and ecosystem
- AI client and MCP setup used
- Task attempted
- Context relevance: Were the files and symbols useful?
- Change Guard evaluation: Did it catch unauthorized edits or report false positives?
- Where setup or execution stumbled

Use the repository's real issue tracker:
- 🐛 [Report a Bug](.github/ISSUE_TEMPLATE/bug_report.md)
- 🧠 [Report a Context or Impact Failure](.github/ISSUE_TEMPLATE/context_or_impact_failure.md)
- 🛡️ [Report a Change Guard Failure](.github/ISSUE_TEMPLATE/contract_guard_failure.md)
- 💡 [Request a Feature](.github/ISSUE_TEMPLATE/feature_request.md)

*Please sanitize all logs before sharing. Do NOT post credentials or proprietary source code.*

---

## 15. Architecture & Documentation Links

- **Architecture Deep-Dive:** [`docs/architecture.md`](docs/architecture.md)
- **Benchmark Methodology & Audit:** [`docs/benchmarks.md`](docs/benchmarks.md)
- **Security & Privacy Policy:** [`docs/security-and-privacy.md`](docs/security-and-privacy.md)
- **Supported Languages & Matrix:** [`docs/supported-languages.md`](docs/supported-languages.md)
- **Change Contracts Guide:** [`docs/change-contracts.md`](docs/change-contracts.md)
- **Troubleshooting & Diagnostics:** [`docs/troubleshooting.md`](docs/troubleshooting.md)
- **Contributing Guide:** [`docs/contributing.md`](docs/contributing.md)
- **Launch Announcement:** [`docs/launch-post.md`](docs/launch-post.md)

---

## 16. License

Copyright (C) 2026 **Amal P S**.

Engram is free software licensed under the **[GNU Affero General Public License v3.0 (AGPL-3.0-or-later)](LICENSE)**.

Under AGPL-3.0, you are free to inspect, run, modify, and redistribute the software. If you modify Engram and distribute it or run it as a network service, you must make the source code of your modified version available under the same license terms. Refer to the full [LICENSE](LICENSE) file for complete legal terms.
