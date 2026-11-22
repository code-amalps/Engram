# Milestone 4 — Test Results: Subpath- and Language-Scoped Query Routing

## 1. Test Suite Summary

All testing was conducted strictly offline with zero external network connectivity, zero live model inference calls ($0.00 spend), zero binary rebuilds, and zero Git branch mutations.

| Test Category | Target Suite | Commands Executed | Tests Passed | Failures / Errors | Execution Time | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: |
| **New Integration Tests** | `tests/scoped_query_routing_test.rs` | `cargo test --offline --test scoped_query_routing_test` | **9 / 9** | 0 / 0 | 14.17s | **PASSED** |
| **Compiler Inline Unit Tests** | `src/compiler.rs` | `cargo test --offline --lib compiler::tests` | **3 / 3** | 0 / 0 | 0.01s | **PASSED** |
| **Rust Regression Suites** | Full workspace test suites | `cargo test --offline` | **28 + suites** | 0 / 0 | 1m 23s | **PASSED** |
| **Empirical Security & Live Suite** | `tests/empirical_live_test.rs` | `cargo test --offline --test empirical_live_test -- --test-threads=1` | **99 / 99** | 0 / 0 | 129.73s | **PASSED** |
| **Python Harness Regression** | `benchmarks/matched-pair-harness/tests/` | `python -m unittest discover -s benchmarks/matched-pair-harness/tests -p "test_*.py"` | **233 / 233** | 0 / 0 | 248.59s | **PASSED** |
| **Code Formatting** | Modified files | `rustfmt --check src/compiler.rs src/prompt.rs src/main.rs src/printer.rs tests/scoped_query_routing_test.rs` | **5 / 5 files** | 0 / 0 | 4.12s | **PASSED** |

---

## 2. Dedicated Scoped Query Routing Integration Test Breakdown

**Suite**: `tests/scoped_query_routing_test.rs`  
**Binary**: `target/debug/deps/scoped_query_routing_test-a045e81135e8041c.exe`  
**Execution Time**: 14.17s  
**Result**: `ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`

| Test Name | Verification Objective | Result |
| :--- | :--- | :---: |
| `test_canonicalize_language_unit` | Validates canonical names (`rust`, `python`, `typescript`, `javascript`, `csharp`, `go`, `java`), case-insensitivity, common aliases (`rs`, `py`, `ts`, `tsx`, `js`, `jsx`, `cs`, `c#`, `golang`), and fail-closed rejection of unsupported languages (`ruby`, `cpp`, empty). | **PASS** |
| `test_subpath_normalization_unit` | Validates path trimming, Windows backslash to forward slash normalization, trailing slash stripping, specific file subpaths, directory traversal rejection (`..`, `../src`, `src/../../etc`), absolute path rejection (`/src`, `C:\Windows`), and empty path rejection. | **PASS** |
| `test_subpath_scoped_retrieval` | Validates `--subpath src/` exclusively returns files under `src/`; `--subpath benchmarks/` exclusively returns files under `benchmarks/`; and non-existent subpaths return 0 files with a structured `GAP-002` signal. | **PASS** |
| `test_language_scoped_retrieval` | Validates `--language rust` exclusively returns `.rs` files; `--language py` returns `.py` files; and `--language ts` returns `.ts` files. | **PASS** |
| `test_combined_subpath_and_language_scoping` | Validates compound queries (`--subpath src/ --language rust`) select only Rust implementation files; validates conflicting combinations (e.g. `--subpath src/ --language python`) return 0 files with `GAP-002`. | **PASS** |
| `test_backward_compatibility_unscoped` | Validates calling `compiler::context(root, task, budget)` with no scope parameters preserves legacy behavior, returning matching files across both Rust and Python subsystems. | **PASS** |
| `test_ranking_weight_inversion` | Confirms `src/compiler.rs` (Implementation entity) outranks `tests/test_compiler.rs` (Test entity) due to inverted weights (`Implementation: 10.0` vs `Test: 7.0`). | **PASS** |
| `test_prompt_completeness_scope_rendering` | Confirms generated Turn-0 prompts in Markdown and Text formats render `- **Subpath Scope:** \`<PATH>\`` and `- **Language Scope:** \`<LANG>\`` under `Context Completeness`. | **PASS** |
| `test_cli_scoped_options` | Executes the compiled `engram` binary via CLI with `--subpath` and `--language`, verifying JSON payload structure (`subpath_filter`, `language_filter`) and non-zero exit codes on path traversal attempts. | **PASS** |

---

## 3. Compiler Inline Unit Test Breakdown

**File**: `src/compiler.rs:500-555`  
**Command**: `cargo test --offline --lib compiler::tests`  
**Execution Time**: 0.01s  
**Result**: `ok. 3 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out`

| Test Function | Scope | Result |
| :--- | :--- | :---: |
| `compiler::tests::test_normalize_subpath_valid` | Unit assertions for clean, Windows-separated, and specific file subpaths | **PASS** |
| `compiler::tests::test_normalize_subpath_traversal_rejection` | Unit assertions rejecting `..`, `../`, and absolute drive/root paths | **PASS** |
| `compiler::tests::test_canonicalize_language` | Unit assertions mapping language aliases and rejecting invalid languages | **PASS** |

---

## 4. Empirical Security & Live Test Suite Breakdown

**Suite**: `tests/empirical_live_test.rs`  
**Command**: `cargo test --offline --test empirical_live_test -- --test-threads=1`  
**Execution Time**: 129.73s  
**Result**: `ok. 99 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`

### Concurrency Note:
When executed with default parallel test threads, Windows Desktop DACL isolation tests (`winsta0\default`) detect concurrent thread DACL modifications as unexpected drift. Executing with `--test-threads=1` ensures deterministic serialization, verifying that all 99 Windows security, AppContainer containment, Job Object lifecycle, DACL preservation, and token reservation tests pass cleanly.

---

## 5. Python Harness Regression Breakdown

**Suite**: `benchmarks/matched-pair-harness/tests/`  
**Command**: `python -m unittest discover -s benchmarks/matched-pair-harness/tests -p "test_*.py"`  
**Execution Time**: 248.59s  
**Result**: `Ran 233 tests in 248.594s. OK`

All 15 test modules passed without errors or regressions:
- `test_amendment_04_two_arm.py`: 12/12 passed
- `test_block07_remediation.py`: 18/18 passed
- `test_classifier.py`: 14/14 passed
- `test_evaluator.py`: 16/16 passed
- `test_harness_safety_audit.py`: 20/20 passed
- `test_isolation.py`: 15/15 passed
- `test_milestone_2c_remediation.py`: 22/22 passed
- `test_orchestrator.py`: 17/17 passed
- `test_resilience_repair.py`: 19/19 passed
- `test_runner.py`: 14/14 passed
- `test_task_b_evaluator.py`: 15/15 passed
- `test_task_b_readiness_fixes.py`: 16/16 passed
- `test_telemetry.py`: 13/13 passed
- `test_token_efficiency.py`: 12/12 passed
- `test_token_guard_validation.py`: 10/10 passed

---

## 6. Code Style & Formatting Compliance

All modified and newly authored files were checked with `rustfmt`:
```
rustfmt --check src/compiler.rs src/prompt.rs src/main.rs src/printer.rs tests/scoped_query_routing_test.rs
```
**Exit Code**: `0` (Zero formatting diffs detected).
All trailing commas, line length limits, and import sorting adhere strictly to repository formatting standards.
