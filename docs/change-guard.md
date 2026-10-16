# Engram CI/CD Change Guard

The **Engram CI/CD Change Guard** is a policy-driven change governance subsystem designed to prevent risky, non-compliant, or unexpected mutations from entering local repositories and GitHub pull requests.

It unifies three core Engram pillars into an offline, fail-closed enforcement engine:
1. **Elastic Change Contracts:** Enforces module and file boundaries, forbidden boundary rules, and expected modifications.
2. **Sovereign Project Constitution:** Evaluates architectural rules, forbids unratified policy mutations, and checks structural boundaries.
3. **Multi-Hop Blast Radius:** Analyzes downstream caller and reverse-dependency cascades against configured blast radius thresholds.

---

## 1. Core Workflow

```text
Repository Changes (Working Tree / Staged / PR)
                   │
                   ▼
       Engram Incremental Analysis
                   │
                   ▼
  Policy Evaluation (.engram/change-guard.toml)
    ├── Change Contracts (Allowed / Forbidden / Missing)
    ├── Project Constitution (.engram/constitution.yaml)
    └── Downstream Blast Radius (Max Affected Symbols)
                   │
                   ▼
       Structured Machine Findings
    ├── Pass (Exit Code 0)
    ├── Warn (Exit Code 0, advisory review)
    ├── Block (Exit Code 1, merge/commit rejected)
    └── Error (Exit Code 2, configuration or execution failure)
```

---

## 2. Policy Configuration Schema

Engram Change Guard is configured via `.engram/change-guard.toml`.

### Schema Specification

```toml
version = "1"

[checks.contracts]
# Enable or disable Change Contract boundary checking
enabled = true
# When true, rejects any pull request or commit that lacks an active contract
required = false
# Finding severity if contract rules fail: "warn" or "block"
severity = "block"
# When true, unexpected changes outside the contract boundary block the guard
strict = false

[checks.constitution]
# Enable or disable Project Constitution rule checks
enabled = true
# When true, Project Constitution is mandatory
required = true
# Finding severity for architectural violations
severity = "block"

[checks.impact]
# Enable or disable downstream blast radius analysis
enabled = true
# Maximum number of downstream caller and dependent symbols allowed
max_affected_symbols = 50
# Finding severity when blast radius exceeds threshold: "warn" or "block"
severity = "warn"

[exclusions]
# Relative glob patterns excluded from change governance
# Note: Broad catch-alls such as "*" or "**" are forbidden for safety
paths = [
    "docs/**",
    "*.md",
    "tests/fixtures/**"
]
```

---

## 3. CLI Usage and Deterministic Exit Codes

### CLI Command

```bash
# Run Change Guard against local working tree changes
engram guard

# Run Change Guard against staged files (pre-commit mode)
engram guard --staged

# Run Change Guard in PR mode against a base commit
engram guard --base origin/main

# Enforce strict boundary enforcement (treat unexpected files as blocking)
engram guard --strict

# Output machine-readable JSON
engram guard --format json

# Diagnostic mode: explains which checks ran, which were skipped, and why
engram guard --diagnostic

# Custom policy path
engram guard --policy custom-policy.toml

# Opt-in pre-commit hook installation and uninstallation
engram guard --install-hook
engram guard --uninstall-hook
engram guard --install-hook --dry-run
```

### Deterministic Exit Codes

| Exit Code | Status | Meaning | Action Required |
| :---: | :---: | :--- | :--- |
| **0** | `pass` | All executed checks conformed to policy | Safe to proceed |
| **0** | `warn` | Findings exist, but policy treats them as advisory | Continuation allowed; human review recommended |
| **1** | `block` | A mandatory check failed (forbidden boundary, strict expansion, or constitution violation) | Execution halted; commit or merge rejected |
| **2** | `error` | Configuration error, invalid policy version, syntax error, or unrecoverable Git failure | Fail-closed diagnostic; fix policy and rerun |

---

## 4. Local Git Pre-Commit Hook Integration

The pre-commit hook integration is **strictly opt-in and non-destructive**:
- It will never overwrite foreign hooks (such as Husky, Lefthook, or custom developer scripts).
- It injects a clearly delimited marker block:

```sh
# --- BEGIN ENGRAM CHANGE GUARD ---
if command -v engram >/dev/null 2>&1; then
  engram guard --staged || exit 1
fi
# --- END ENGRAM CHANGE GUARD ---
```

### Installation
```bash
engram guard --install-hook
```
If `.git/hooks/pre-commit` already exists, Engram appends the guard block cleanly.

### Removal
```bash
engram guard --uninstall-hook
```
Engram strips only its marker block, leaving all other user and tool hooks intact.

---

## 5. GitHub Actions Integration

A reusable GitHub Actions workflow is provided at `.github/workflows/engram-guard.yml`.

```yaml
name: "CI Change Guard"

on:
  pull_request:
    branches: ["main", "Development"]

permissions:
  contents: read
  pull-requests: read

jobs:
  guard:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: dtolnay/rust-toolchain@stable

      - name: Build Engram
        run: cargo build --release --bin engram

      - name: Enforce Change Guard
        run: target/release/engram guard --base origin/${{ github.base_ref }} --diagnostic
```

---

## 6. Execution Examples

### Example 1: Passing Result (`pass`, Exit Code 0)
```text
======================================================================
                     ENGRAM CI/CD CHANGE GUARD                        
======================================================================

  Status:          PASS
  Exit Code:       0
  Target Snapshot: working_tree
  Change Contract: ctr_a91b40fe
  Changed Files:   2

--- Checks Overview ---
  ✅ [EXECUTED]       contracts
  ✅ [EXECUTED]       constitution
  ✅ [EXECUTED]       impact

--- Findings ---
  No compliance findings or violations detected.

----------------------------------------------------------------------
VERDICT: PASSED: All 3 check(s) passed successfully across 2 changed file(s).
======================================================================
```

### Example 2: Advisory Warning (`warn`, Exit Code 0)
```text
======================================================================
                     ENGRAM CI/CD CHANGE GUARD                        
======================================================================

  Status:          WARN
  Exit Code:       0
  Target Snapshot: working_tree
  Changed Files:   3

--- Checks Overview ---
  ⚪ [NOT_APPLICABLE] contracts (No active Change Contract present (optional in policy))
  ✅ [EXECUTED]       constitution
  ✅ [EXECUTED]       impact

--- Findings ---
  ⚠️  WARN  [GUARD-IMPACT-001] Blast radius threshold exceeded: 64 affected downstream entities (max allowed: 50)
           Remediation: Review downstream dependencies or increase max_affected_symbols threshold in policy

----------------------------------------------------------------------
VERDICT: ADVISORY: All required checks passed, but 1 warning(s) were flagged for review.
======================================================================
```

### Example 3: Blocked Result (`block`, Exit Code 1)
```text
======================================================================
                     ENGRAM CI/CD CHANGE GUARD                        
======================================================================

  Status:          BLOCK
  Exit Code:       1
  Target Snapshot: origin/main..HEAD
  Change Contract: ctr_c142b910
  Changed Files:   4

--- Checks Overview ---
  ✅ [EXECUTED]       contracts
  ✅ [EXECUTED]       constitution
  ✅ [EXECUTED]       impact

--- Findings ---
  ❌ BLOCK [GUARD-CONTRACT-002] File 'src/payments/vault.rs' violates forbidden contract boundary
           File:        src/payments/vault.rs
           Remediation: Revert forbidden changes or update contract boundary with approval

----------------------------------------------------------------------
VERDICT: BLOCKED: 1 mandatory check failure(s) detected. Merge or commit rejected.
======================================================================
```

---

## 7. Security Guarantees & Assumptions

- **100% Offline ($0 spend):** Never invokes live LLM APIs or loads remote credentials.
- **Fail-Closed Execution:** Malformed policies, unsupported schema versions, or invalid exclusion patterns immediately fail with exit code `2`.
- **Sovereign Policy Protection:** Any PR modifying `.engram/constitution.yaml` or `.engram/change-guard.toml` is blocked unless accompanied by explicit human governance approval.
- **No Broad Exclusions:** Global wildcards (`*`, `**`) are rejected during policy validation.
