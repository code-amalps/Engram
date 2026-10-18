# Engram Reproducible Matched-Pair Benchmark Harness (v2.1)

Empirical evaluation harness for conducting scientifically rigorous, matched-pair benchmark trials comparing AI coding assistant performance (OpenAI Codex CLI / `gpt-5.6-luna`) with **Engram Active (Treatment Arm B)** versus **Engram Disabled (Control Arm A)** on standardized golden codebases.

---

## 1. Core Scientific Invariants & Safeguards

The harness implements five mandatory scientific and operational safeguards:

### 1. Acceptance-Conditioned Savings Guard
- **Principle:** A defective, broken, or non-functional patch cannot claim token efficiency savings.
- **Enforcement:** If either arm fails functional correctness ($C < 1.0$ or `passed_all_requirements == False`), comparative token savings percentages and deltas are strictly suppressed (`null`) and flagged as `INVALIDATED_BY_ACCEPTANCE_FAILURE`.

### 2. Explicit Token Provenance & Semantics
- **Principle:** Derived or estimated token counts must never appear as independently measured provider telemetry.
- **Enforcement:**
  - `prompt_tokens_cached_inferred` (`bool`): `false` when `cached_input_tokens` is directly reported by provider telemetry; `true` if inferred during normalization or defaulted.
  - `prompt_tokens_uncached_inferred` (`bool`): `true` when derived via `prompt_tokens_total - prompt_tokens_cached`.
  - **Invariant Checks:** Rejects zero-token anomalies, missing telemetry, negative values, and inconsistent sums (`prompt + completion != total`, `cached + uncached != prompt`). Non-boolean provenance flags are rejected as `TELEMETRY_MALFORMED`.
  - **Context Occupancy:** Rollout parsing distinguishes exact single-request peak context from cumulative thread totals. When only stdout events are available, peak context is marked `null` (`UNAVAILABLE`) rather than misrepresenting cumulative tokens as context size.

### 3. Physical Dependency Isolation
- **Principle:** Running agents must not mutate or poison shared dependencies or the golden repository baseline.
- **Enforcement:**
  - Default `--dependency-strategy copy` physically clones `node_modules` into an isolated temporary sandbox (`%TEMP%\engram_pilot\runs\...`).
  - Pre-execution and post-teardown recursive SHA-256 manifests verify that the golden repository remains bitwise intact.

### 4. Process Lifecycle & PID Tree Termination
- **Principle:** Timeout or termination must cleanly kill all spawned child and grandchild processes without leaving orphaned background processes or affecting unrelated user/editor tasks.
- **Enforcement:**
  - PID tree lifecycle tracking terminates process trees via `taskkill /F /T /PID` on Windows.
  - Pre-run PATH sanitization purges any directories containing `engram` executables from Control Arm A's environment to eliminate unmanaged discovery paths.

### 5. Frozen Artifact Pointers
- **Treatment Arm B:** Pinned strictly to frozen Engram binary SHA-256:
  `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0`
- **Dynamic Configuration:** MCP server configuration (`-c mcp_servers.engram...`) is passed dynamically per invocation. Control Arm A has no MCP configuration.

---

## 2. Directory Layout & Architecture

```text
benchmarks/matched-pair-harness/
├── .gitignore                      # Ignores __pycache__ and local trial run outputs
├── README.md                       # Harness documentation (this file)
├── config/
│   └── tasks/
│       ├── task_a_typo.json        # Task A: Documentation/typo fix
│       ├── task_b_remember_me.json # Task B: Remember-me toggle feature
│       └── task_c_theme_toggle.json# Task C: Dark/light theme toggle feature
├── schemas/
│   ├── correctness_checklist.schema.json # Task specification & requirements schema
│   ├── matched_pair_summary.schema.json  # Top-level pair comparative schema (Draft 2020-12)
│   └── run_telemetry.schema.json         # Single-run telemetry & isolation schema (Draft 2020-12)
├── src/
│   ├── evaluator.py                # L0-L3 gates, requirement verification, C score calculation
│   ├── isolation.py                # Sandbox creation, copy isolation, recursive digests, PID cleanup
│   ├── orchestrator.py             # Matched-pair orchestration, telemetry validation, reporting
│   ├── runner.py                   # Codex execution, PATH sanitization, MCP args injection
│   ├── schema_validator.py         # Zero-dependency Draft 2020-12 schema validator
│   └── telemetry.py                # Rollout & event telemetry parser with explicit provenance
├── tests/
│   ├── fixtures/                   # Real session rollouts & synthetic event streams
│   └── test_suite.py               # 33 unit tests covering isolation, accounting, and provenance
└── reports/
    ├── dry_run_validation_report.md
    └── preflight_remediation_report.md
```

---

## 3. Automated Verification Gates

Every run arm is evaluated against four strict gate tiers:

| Gate | Name | Command / Inspection | Status |
| :---: | :--- | :--- | :--- |
| **L0** | Typecheck | `npx ng build --configuration=development --no-watch` | Mandatory |
| **L1** | Lint & Style | `npm run lint` / `npx prettier --check ...` | Mandatory |
| **L2** | Unit Tests | `npm test -- --watch=false --browsers=ChromeHeadless` | Optional / Task-specific |
| **L3** | Production Build | `npx ng build --configuration=production` | Mandatory |

---

## 4. How to Run

### Running Offline Unit Tests (Zero Model Cost)
```powershell
cd "C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness"
python -m unittest discover -s tests -p "test_*.py"
```
All 33 tests run entirely offline against deterministic local fixtures.

### Running a Dry-Run Trial (Zero Model Cost)
Simulates end-to-end sandbox creation, execution, gate evaluation, and report generation:
```powershell
python src\orchestrator.py `
  --task task_c_theme_toggle `
  --pair pair_01 `
  --dry-run
```

### Running an Authorized Live Benchmark Trial
```powershell
python src\orchestrator.py `
  --task task_c_theme_toggle `
  --pair pair_01 `
  --model gpt-5.6-luna `
  --reasoning medium `
  --dependency-strategy copy
```

---

## 5. Generated Artifacts

Each trial execution writes sealed artifacts to `reports/runs/<task_id>_<pair_id>/`:
- `control_A/run_telemetry.json`: Control arm telemetry, git diff patch, gate outputs, and provenance flags.
- `treatment_B/run_telemetry.json`: Treatment arm telemetry, git diff patch, gate outputs, and provenance flags.
- `matched_pair_summary.json`: Top-level summary containing comparative deltas, savings percentages, and acceptance parity.
- `matched_pair_report.md`: Executive markdown report with GitHub-style warning alerts when claims are invalidated.
