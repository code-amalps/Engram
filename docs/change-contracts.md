# Change Contracts & The Change Guard

Change Contracts establish auditable, machine-readable boundaries for AI code modifications. They answer a fundamental safety question:  
**Did the coding assistant stay within the intended scope, or did it make unauthorized modifications elsewhere in the codebase?**

---

## 1. What Change Guard Actually Guarantees (And What It Does Not)

### What It Guarantees:
- **Boundary Violation Detection:** Detects when an AI agent edits files or symbols marked as forbidden or outside authorized scopes.
- **Scope Expansion Auditing:** Flags unexpected file changes that were not part of the initial task grounding.
- **Architectural Policy Enforcement:** Enforces Project Constitution rules (e.g. "Domain layer must never import Infrastructure layer").
- **Audit Logging:** Creates a tamper-evident audit record of what was authorized vs what was changed.

### What It Does NOT Guarantee (Important Distinctions):
- **Semantic Correctness:** Change Guard verifies **where** changes occurred, not **whether the logic is correct**. It does not prove your code is free of bugs, performance regressions, or security flaws.
- **Type Checking / Compilation:** Change Guard is not a replacement for `cargo check`, `tsc`, `go test`, or unit test suites.
- **Universal Prevention:** Change Guard cannot physically prevent an AI from editing a file in your editor buffer; it catches and blocks unauthorized changes when evaluated via CLI, pre-commit hook, or CI.

---

## 2. Enforcement Tiers: Enforced vs. Advisory

Change Guard operates across two modes:

| Mode | Allowed Changes (`EXPECTED`) | Unexpected Changes (`UNEXPECTED`) | Explicit Violations (`BOUNDARY_VIOLATION`) | Constitution Errors |
|---|:---:|:---:|:---:|:---:|
| **Advisory Mode** (Default) | PASS (0) | WARNING (0) | **BLOCKED (1)** | **BLOCKED (1)** |
| **Strict Mode** (`--strict` / CI) | PASS (0) | **BLOCKED (1)** | **BLOCKED (1)** | **BLOCKED (1)** |

---

## 3. The Change Contract Lifecycle

```text
 ┌──────────────────────┐
 │   Preflight Task     │  <- Generates contract anchored to current Git commit
 └──────────┬───────────┘
            │
            ▼
 ┌──────────────────────┐
 │   Agent Mutates      │  <- Agent writes code
 └──────────┬───────────┘
            │
            ▼
 ┌──────────────────────┐
 │  Postflight / Hook   │  <- Change Guard evaluates Git diff
 └──────────┬───────────┘
            │
      ┌─────┴───────────────────────┐
      │                             │
 [Pass (0)]               [Violation (1)]
      │                             │
      ▼                             ▼
 Commit Allowed            ┌────────────────────────┐
                           │ Stateful Recovery      │ <- Bounded retry (max 2)
                           └────────┬───────────────┘
                                    │
                         ┌──────────┴───────────┐
                         ▼                      ▼
                   Agent Repairs          Human Reconciles
                   (Back to Pass)       (Ratifies Expansion)
```

1. **Creation:** Run `engram contract create "<task>"` or invoke `preflight_task` via MCP. Engram grounds the task to identify expected files and symbols, creates a unique contract ID (`ctr_...`), and records the baseline Git commit.
2. **Evaluation:** Run `engram contract check` (or automatically via `.git/hooks/pre-commit`).
3. **Reconciliation (Human-Authorized Expansion):** If the AI genuinely needs to expand into an unpredicted file, a human engineer can explicitly authorize the expansion:
   ```bash
   engram contract reconcile <contract_id> "Approved adding database migration file"
   ```
4. **Ratification in CI:** If a pull request modifies a contract's allowed boundary compared to the PR base commit, CI flags `CONTRACT_MODIFIED_IN_PR` until humanly ratified (`engram contract ratify <contract_id>`).

---

## 4. Hook & CI Enforcement Surfaces

### Local Git Pre-Commit Hook
Engram does **not** install Git hooks silently. To install:
```bash
# Preview what would be installed
engram contract install-hook --dry-run

# Install or chain cleanly with Lefthook / Husky
engram contract install-hook

# Uninstall cleanly
engram contract uninstall-hook
```
> **Bypass Notice:** Standard Git flags like `git commit --no-verify` will bypass the local hook, as with any Git hook.

### GitHub Actions CI Workflow
Add `.github/workflows/engram-guard.yml` to enforce boundaries on pull requests:
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

---

## 5. Exit Codes Reference

| Exit Code | Constant | Meaning |
|:---:|---|---|
| **`0`** | `PASS` | All changes match expected boundaries. |
| **`1`** | `CONTRACT_VIOLATION` | Breached a forbidden boundary, violated a Constitution rule, or strict unexpected change. |
| **`2`** | `CONTRACT_NOT_FOUND` | No active contract found in repository. |
| **`3`** | `CONTRACT_INVALID` | Corrupt contract schema or unparseable JSON. |
| **`4`** | `BASELINE_UNAVAILABLE` | Baseline commit missing from local Git history (e.g. shallow clone). |
| **`5`** | `INTERNAL_ERROR` | System I/O error or Git subprocess exception. |
