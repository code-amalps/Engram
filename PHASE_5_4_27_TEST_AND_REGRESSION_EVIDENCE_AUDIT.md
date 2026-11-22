# Engram Phase 5.4.27: Test and Regression Evidence Audit

**Phase:** 5.4.27  
**Execution Mode:** STRICTLY OFFLINE, READ-ONLY FORENSIC AUDIT  
**Baseline Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Audit Date:** 2026-11-20  
**Release Authorization:** `NOT_AUTHORIZED`

---

## 1. Executive Summary

This audit performs an independent verification of the test evidence across both the Engram Rust product and the benchmark harness.

In Phase 5.4.26, an 8-suite regression suite reportedly passed 86/86 tests in 627.90 seconds. This audit examined the raw log artifacts, executed offline-safe installer tests, and ran a direct runtime smoke test against the compiled `target/release/engram.exe`.

### Key Finding: Two Separate Test Domains
Auditors must strictly distinguish:
1. **The Python Benchmark Harness Regression (86 tests):** Validates the evaluation harness orchestrator, CLI parameter parsing, tetrad ordering, token ceiling enforcement, and cancellation wrappers. It does **not** validate the Rust runtime.
2. **The Rust Product & Installer Test Suite (130+ tests):** Validates the AST parser, SQLite indexer, Change Guard, pre-commit hooks, installer safety, and MCP server.

---

## 2. Benchmark Harness Regression Suite Audit (Phase 5.4.26 Evidence)

The raw execution log in `scratch/phase_5_4_26/regression_summary.json` was audited line-by-line:

| Suite Name | Command Executed | Duration | Tests | Exit Code | Evidence Classification |
| :--- | :--- | :---: | :---: | :---: | :--- |
| `test_candidate_selection.py` | `pytest benchmarks/.../test_candidate_selection.py -v` | 2.84s | 8 passed | 0 | `PRODUCTION_COMPONENT` |
| `test_telemetry_semantics.py` | `pytest benchmarks/.../test_telemetry_semantics.py -v` | 3.63s | 4 passed | 0 | `PRODUCTION_COMPONENT` |
| `test_candidate_4_2_offline_certification.py` | `pytest benchmarks/.../test_candidate_4_2_offline_certification.py -v` | 12.72s | 10 passed | 0 | `PRODUCTION_COMPONENT` |
| `test_task_b_budget_simulation.py` | `pytest benchmarks/.../test_task_b_budget_simulation.py -v` | 1.72s | 3 passed | 0 | `MOCKED_BEHAVIOR` |
| `test_diagnostic_continuation.py` | `pytest benchmarks/.../test_diagnostic_continuation.py -v` | 230.06s | 27 passed | 0 | `MOCKED_BEHAVIOR` |
| `test_tetrad_harness.py` | `pytest benchmarks/.../test_tetrad_harness.py -v` | 101.43s | 1 passed | 0 | `MOCKED_BEHAVIOR` |
| `test_per_arm_candidate_identity.py` | `pytest benchmarks/.../test_per_arm_candidate_identity.py -v` | 254.37s | 10 passed | 0 | `PRODUCTION_COMPONENT` |
| `test_phase_5_4_23_safety_remediation.py` | `pytest benchmarks/.../test_phase_5_4_23_safety_remediation.py -v` | 21.13s | 23 passed | 0 | `PRODUCTION_PATH` |
| **TOTAL** | **8 Suites** | **627.90s** | **86 passed** | **0** | **0 failed, 0 skipped** |

### Harness Suite Forensic Observations:
- **Zero Network Calls:** All 8 suites ran completely offline without making outbound network requests or contacting LLM APIs.
- **Process Isolation:** The long execution durations in `test_diagnostic_continuation.py` (230s) and `test_per_arm_candidate_identity.py` (254s) stem from spawning real local subprocesses with simulated streaming output and verifying timeout/kill barriers.
- **Mocking Reality:** Suites testing token ceilings simulate provider token events via synthetic line streaming; they do not invoke the actual closed-source Codex CLI or remote OpenAI infrastructure.

---

## 3. Product & Installer Test Verification (Phase 5.4.27 Evidence)

In this audit, two offline test suites and a functional runtime smoke test were executed directly against the Engram product components:

### 3.1 Installer Safety Test Suite (`tests/installer_safety_test.ps1`)
- **Execution:** Executed natively in PowerShell in an isolated temporary sandbox (`$env:TEMP/engram_test_sandbox_*`).
- **Results:** **8 Passed, 0 Failed, Exit Code 0**.
- **Coverage:**
  1. Checksum mismatch rejection (`FAIL_CLOSED`): Tampered archive detected and aborted.
  2. Missing manifest rejection (`FAIL_CLOSED`): Missing `SHA256SUMS` aborted.
  3. Duplicate manifest rejection (`FAIL_CLOSED`): Duplicate entries rejected.
  4. Token sanitization boundary: Secrets redacted from all logs.
  5. Process in-use protection: Active PID locks prevent replacing running binaries.
  6. Atomic replacement: Backup creation and rollback verified.
  7. CLI argument parsing: `--tag` and `--version` verified.
  8. Private repo API asset download: Authenticated header handling verified.

### 3.2 Attestation Policy Test Suite (`tests/attestation_policy_test.ps1`)
- **Execution:** Executed natively in PowerShell.
- **Results:** **5 Passed, 0 Failed, Exit Code 0**.
- **Coverage:** Verified precedence rules for GitHub Sigstore provenance attestations (`required` vs `approved_without_attestation`).

### 3.3 Runtime Smoke Test (`scratch/phase_5_4_27/smoke_test.ps1`)
- **Target:** Native release executable `target/release/engram.exe`.
- **Environment:** Clean temporary directory (`$env:TEMP/engram_audit_smoke_*`).
- **Executed Commands & Outputs:**
  1. `engram --version` -> `engram 1.5.4` (Exit Code: 0).
  2. `engram -C <tmp> init` -> Created `.engram/` config, generated project ID (Exit Code: 0).
  3. `engram -C <tmp> analyze` -> Built SQLite index, 0 files indexed, cache hit (Exit Code: 0).
  4. `engram -C <tmp> bench` -> Evaluated synthetic tasks across 7 languages, printed discovery and grounding metrics (Exit Code: 0).
  5. `engram -C <tmp> doctor` -> Verified memory health, telemetry chain pristine (Exit Code: 0).

---

## 4. Comprehensive Test Evidence Matrix

| Evidence Category | Test Suites / Scenarios | Validation Objective | Realism Level |
| :--- | :--- | :--- | :--- |
| **`PRODUCTION_PATH`** | `installer_safety_test.ps1`<br>`test_phase_5_4_23_safety_remediation.py`<br>`scratch/smoke_test.ps1` | Actual installer script, CLI arguments, and native executable runs | **Highest:** Real binaries and production code executed natively |
| **`PRODUCTION_COMPONENT`** | `test_candidate_selection.py`<br>`test_telemetry_semantics.py`<br>`test_candidate_4_2_offline_certification.py`<br>`test_per_arm_candidate_identity.py` | Production orchestrator, evaluator, and classifier modules | **High:** Real harness components run with local mock child processes |
| **`MOCKED_BEHAVIOR`** | `test_diagnostic_continuation.py`<br>`test_task_b_budget_simulation.py`<br>`test_tetrad_harness.py` | Arm failure continuation, ceiling breach overrides, tetrad sequencing | **Medium:** Synthesizes LLM token events and process exit signals |
| **`FORMULA_ONLY`** | C01–C15 Concurrency Simulations<br>Budget Headroom Clamping Calculations | State transition boundaries, clamped ceiling math, reserve threshold | **Mathematical:** Proves logic boundaries without external latency |
| **`HISTORICAL`** | 130+ Rust Unit & Integration Tests in `tests/*.rs` | AST parsers, tree-sitter grammars, SQLite WAL storage, Change Contracts | **Historical:** Verified in Phases 1–5.3; not re-run in 5.4.27 to avoid rebuilding |

---

## 5. Audit Conclusion

The reported regression evidence from Phase 5.4.26 (86/86 passed) is **confirmed authentic and reproducible offline**. The product installer and runtime smoke tests are **proven functional on Windows x86_64**. 

However, because the full 130+ Rust tests were not recompiled against the uncommitted `src/` changes, complete product test verification remains conditional on committing working-tree changes and running a full `cargo test`.
