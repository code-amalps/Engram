# Engram Benchmark — Matched-Pair Harness v2.0 Validation Report

**Document Version:** 2.0.0-VALIDATED  
**Date:** 2026-10-18  
**Target:** Headless Reproducible Matched-Pair Benchmark Harness  
**Runner Executable:** `C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe` (`codex-cli 0.163.0-alpha.10.2`)  
**Frozen Binary SHA-256:** `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0`  
**Baseline Git Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` (`angular-ngrx-material-starter`)

---

## 1. Executive Summary & Safeguard Adherence

The reproducible matched-pair benchmark harness (v2.0) has been fully implemented and verified against all required validation gates. This harness replaces exploratory GUI-based trials with an automated, strictly isolated, headless experimental apparatus designed to produce defensible, peer-review-grade empirical measurements of Engram's impact on coding agent token efficiency and functional correctness.

### Strict Safeguard Compliance Audit

| Requirement / Invariant | Status | Verification Evidence |
| :--- | :---: | :--- |
| **Frozen Artifact Integrity** | **PRESERVED** | `benchmarks/cost-baseline/` artifacts (PREREGISTRATION, BUILD_MANIFEST, FREEZE_SEAL) remain 100% bitwise intact. |
| **Product Repository Purity** | **PRESERVED** | Zero edits to `src/` in `Engram` product codebase. All harness code is isolated in `benchmarks/matched-pair-harness/`. |
| **Frozen Binary Hash Lock** | **VERIFIED** | SHA-256 of `target/release/engram.exe` verified as `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0`. |
| **Zero Model Usage During Setup** | **VERIFIED** | All tests and validation gates ran in simulated/dry-run mode or against recorded event logs. 0 live API tokens consumed. |
| **Filesystem Safety Invariant** | **ENFORCED** | Sandbox creation and teardown strictly enforce the invariant that targets must reside inside `%TEMP%\engram_pilot\runs`. Attempted cleanup outside this path immediately raises `SafetyViolationError`. |
| **Headless Execution Hygiene** | **VERIFIED** | Codex invocations enforce `stdin=subprocess.DEVNULL`, dynamic per-invocation MCP configuration, and automated process cleanup. |

---

## 2. Implemented Architecture & Modular Components

The harness is organized cleanly into dedicated, decoupled modules within `benchmarks/matched-pair-harness/`:

```text
benchmarks/matched-pair-harness/
├── config/
│   └── tasks/
│       ├── task_a_typo.json               # Task A: Scoped typo & localized text fix
│       ├── task_b_remember_me.json         # Task B: Multi-file state preference
│       └── task_c_theme_toggle.json        # Task C: Full dark-light theme toggle
├── schemas/
│   ├── run_telemetry.schema.json          # Strict JSON Schema (draft 2020-12) for run telemetry
│   └── correctness_checklist.schema.json  # Strict JSON Schema for task requirements & gates
├── src/
│   ├── schema_validator.py                # Zero-dependency schema validator
│   ├── telemetry.py                       # Tier 1 token parser & auxiliary tool payload meter
│   ├── isolation.py                       # Git worktree sandboxing, NTFS junction linking & safety guards
│   ├── evaluator.py                       # Git impact, quality gates L0-L3 & correctness score C
│   ├── runner.py                          # Headless Codex CLI runner with per-invocation MCP flags
│   └── orchestrator.py                    # Matched-pair coordinator, coin-flip randomizer & ledger
├── tests/
│   ├── fixtures/
│   │   ├── real_handshake.jsonl           # Captured JSONL from Option C Codex handshake
│   │   └── synthetic_multiturn.jsonl      # Multi-turn synthetic event stream
│   └── test_suite.py                      # 8/8 unit tests verifying all core invariants
└── reports/
    └── runs/                              # Vault output directory for run telemetry & patches
```

---

## 3. Telemetry Accounting & Invariants Verification

### 3.1 Four-Tier Telemetry Model
Harness v2.0 programmatically isolates token consumption directly from the model stream, eliminating reliance on coarse visual percentage bars:
- **Tier 1 (Programmatic Tokens):** Direct capture of `input_tokens`, `cached_input_tokens`, `output_tokens`, and `reasoning_output_tokens`.
- **Tier 2 (Context Occupancy):** Peak turn context window load and final context window size.
- **Tier 3 (Auxiliary Tool Overhead):** Exact wire bytes and tool call frequencies for all Engram MCP interactions.
- **Tier 4 (Macro Billing & Wall Time):** Non-blocking execution duration and account-level reference points.

### 3.2 Double-Counting Prevention
The parser enforces mathematical consistency across all turns:
$$\text{prompt\_tokens\_uncached} = \text{prompt\_tokens\_total} - \text{prompt\_tokens\_cached}$$
$$\text{total\_model\_tokens} = \text{prompt\_tokens\_total} + \text{completion\_tokens}$$
Cumulative thread summaries are distinguished from per-turn records to ensure multi-turn sessions never artificially multiply token counts.

---

## 4. Multi-Dimensional Quality & Correctness Evaluation

Diff size alone does not determine engineering quality. The evaluator tests functional requirements independently:

### 4.1 Independent Correctness Score ($C$)
$$C = \frac{\sum_{i=1}^{N} w_i \cdot \text{Pass}(REQ_i)}{\sum_{i=1}^{N} w_i} \in [0.0, 1.0]$$

For the Theme Toggle task (`task_c_theme_toggle`):
- **REQ-1 (Weight 0.20):** User-accessible toggle button in header.
- **REQ-2 (Weight 0.20):** Consistent theme application to CDK overlay and theme wrapper.
- **REQ-3 (Weight 0.20):** Reload persistence via localStorageService under `'SETTINGS'`.
- **REQ-4 (Weight 0.10):** Default theme preservation (light theme default).
- **REQ-5 (Weight 0.15):** Runtime asset validity (FontAwesome icons imported & registered in `FaIconLibrary`).
- **REQ-6 (Weight 0.05):** Feature override resilience (`autoNightMode` handling).
- **REQ-7 (Weight 0.10):** Boundary & blast radius (strictly zero edits to unrelated files).

### 4.2 Quality-Adjusted Efficiency (Effective Cost)
$$\text{Effective Cost} = \frac{\text{Total Model Tokens}}{C}$$
A run that saves tokens by omitting necessary icon imports or breaking night mode (e.g., $C = 0.70$) is penalized proportionally, ensuring fair comparison against complete implementations.

---

## 5. Unit Test Suite Execution Results

Running `python tests/test_suite.py` executed 8 comprehensive unit tests:

```text
........
----------------------------------------------------------------------
Ran 8 tests in 0.053s

OK
```

### Detailed Test Invariant Coverage:
1. `test_all_tasks_valid`: Confirmed all task JSON files (`task_a`, `task_b`, `task_c`) conform to `correctness_checklist.schema.json`.
2. `test_invalid_task_missing_field`: Confirmed schema validator rejects malformed task specifications.
3. `test_real_handshake_fixture`: Validated telemetry parser against Option C live handshake (`real_handshake.jsonl`):
   - `prompt_tokens_total`: 47,541
   - `prompt_tokens_cached`: 39,296
   - `prompt_tokens_uncached`: 8,245
   - `completion_tokens`: 83
   - `reasoning_tokens`: 9
   - `total_model_tokens`: 47,624
   - `auxiliary_tool_payload_bytes`: 431 bytes
4. `test_synthetic_multiturn_fixture`: Verified 3-turn multi-turn accumulation, caching invariants, and peak context calculation (`peak_turn_context`: 7,100).
5. `test_effective_cost_calculation`: Verified that a defective run ($C=0.80$) with 50K tokens correctly computes an effective cost of 62,500 tokens.
6. `test_frozen_binary_verification`: Verified release executable against freeze seal SHA-256.
7. `test_frozen_binary_mismatch_raises`: Verified isolation manager rejects modified or corrupt binaries.
8. `test_safety_guard_prevents_arbitrary_deletion`: Verified `teardown_sandbox` rejects any path outside `%TEMP%\engram_pilot\runs`.

---

## 6. End-to-End Dry-Run Demonstration

The orchestrator was executed in dry-run mode for a full matched pair on Task C (`pair_dry_run_01`):

```bash
python src/orchestrator.py --task task_c_theme_toggle --pair pair_dry_run_01 --dry-run
```

### Execution Log Summary:
- **Randomization Seed:** 8734
- **Determined Execution Order:** Arm B (Treatment, Order 1) $\rightarrow$ Arm A (Control, Order 2)
- **Arm B Execution:**
  - Sandbox created: `%TEMP%\engram_pilot\runs\task_c_theme_toggle_pair_dry_run_01_treatment_B_...`
  - Engram MCP: Verified Active (`ACTIVE`)
  - Simulated model tokens: 12,650 (Prompt: 12,500, Cached: 10,000, Uncached: 2,500, Completion: 150)
  - Correctness score: $C = 0.70$
  - Quality-adjusted cost: 18,071.43 tokens
  - Sandbox cleaned up safely via worktree removal.
- **Arm A Execution:**
  - Sandbox created: `%TEMP%\engram_pilot\runs\task_c_theme_toggle_pair_dry_run_01_control_A_...`
  - Engram MCP: Verified Disabled (`DISABLED`)
  - Simulated model tokens: 28,720 (Prompt: 28,400, Cached: 21,000, Uncached: 7,400, Completion: 320)
  - Correctness score: $C = 0.70$
  - Quality-adjusted cost: 41,028.57 tokens
  - Sandbox cleaned up safely via worktree removal.
- **Artifact Sealing:** Both run telemetry files validated against `run_telemetry.schema.json` with zero errors.

### Matched-Pair Comparison Summary

> [!WARNING]
> **COMPARATIVE TOKEN SAVINGS CLAIM INVALIDATED**  
> **Verdict:** `MUTUAL_ACCEPTANCE_FAILURE`  
> **Explanation:** Both arms failed functional correctness requirements (Control A score: 0.70, Treatment B score: 0.70; `passed_all_requirements: False`). Under the pre-registered harness protocol, comparative token efficiency percentages are STRICTLY INVALIDATED and suppressed whenever either arm fails acceptance ($C < 1.0$). Incomplete implementations that omit required logic cannot claim efficiency gains.

| Metric | Control Arm A (Engram OFF) | Treatment Arm B (Engram ON) | Comparative Delta / Savings | Status |
| :--- | :---: | :---: | :---: | :---: |
| **Execution Order** | Run 2 | Run 1 | Randomized (Coin Flip) | Complete |
| **Engram MCP Status** | `DISABLED` | `ACTIVE` | Verified Isolated | Enforced |
| **Prompt Tokens (Total)** | 28,400 | 12,500 | **INVALIDATED ($C < 1.0$)** | `INVALIDATED_UNACCEPTED` |
| **Prompt Tokens (Cached)** | 21,000 | 10,000 | **INVALIDATED ($C < 1.0$)** | `INVALIDATED_UNACCEPTED` |
| **Prompt Tokens (Uncached)** | 7,400 | 2,500 | **INVALIDATED ($C < 1.0$)** | `INVALIDATED_UNACCEPTED` |
| **Completion Tokens** | 320 | 150 | **INVALIDATED ($C < 1.0$)** | `INVALIDATED_UNACCEPTED` |
| **Total Model Tokens** | 28,720 | 12,650 | **INVALIDATED ($C < 1.0$)** | `INVALIDATED_UNACCEPTED` |
| **Peak Single-Request Context** | `None` (Unavailable) | `None` (Unavailable) | **INVALIDATED ($C < 1.0$)** | `INVALIDATED_UNACCEPTED` |
| **Engram Tool Calls** | 0 | 1 | +1 call | Recorded |
| **Auxiliary Wire Payload** | 0 B | 100 B | +100 B | Recorded |
| **Correctness Score ($C$)** | 0.70 | 0.70 | 0.70 vs 0.70 (Parity) | Both Failed ($C < 1.0$) |
| **Passed All Requirements** | `False` | `False` | `False` vs `False` | **UNACCEPTED** |
| **Quality-Adjusted Effective Cost**| 41,028.57 | 18,071.43 | **INVALIDATED ($C < 1.0$)** | `INVALIDATED_UNACCEPTED` |


---

## 7. Current Status & Review Gate

The benchmark harness v2.0 is fully assembled, unit tested, and dry-run validated.

**Status:** ALL VALIDATION GATES PASSED.  
**Execution Halting:** In accordance with the pre-approved plan, execution is halted immediately before launching any live model tasks or consuming model tokens. Awaiting review and authorization before proceeding to live paired trials.
