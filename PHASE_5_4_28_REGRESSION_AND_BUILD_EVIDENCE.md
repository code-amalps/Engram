# Phase 5.4.28: Comprehensive Regression and Build Verification Evidence

**Phase:** 5.4.28  
**Execution Mode:** Strictly Offline Verification & Test Execution  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Authorization Status:** `NOT_AUTHORIZED`  

---

## 1. Executive Summary

Phase 5.4.28 executed comprehensive, multi-layer regression testing across product core code, installer scripts, and benchmark harness infrastructure.

All tests were executed in a **strictly offline environment** with **zero network access**, **zero live model/API calls**, and **zero Git mutations**.

### Test Execution Aggregate Results:
- **Rust Integration Test Suites:** 6 suites, 62 tests passing, 0 failed (100% pass rate).
- **Installer Safety Suite:** 1 suite, 8 tests passing, 0 failed (100% pass rate).
- **Python Harness Safety Suites:** 4 suites, 38 tests passing, 0 failed (100% pass rate).
- **Total Tests Executed:** **108 tests passing, 0 failed, 0 skipped**.

---

## 2. Test Suite Breakdown and Results

### 2.1 Rust Product Test Suites

| Test Target / File | Focus Area | Executed Command | Results | Execution Duration |
|---|---|---|---|---|
| `tests/build_provenance_test.rs` | CLI `--version` metadata, git commit, dirty state, rustc version, build profile | `cargo test --test build_provenance_test --offline` | **5 / 5 PASS** | 0.00s |
| `tests/storage_migration_test.rs` | SQLite connection, busy timeout (15s), schema migrations, table creation | `cargo test --test storage_migration_test --offline` | **11 / 11 PASS** | 0.21s |
| `tests/change_guard_test.rs` | AST symbol graph integrity, change guard rules, impact analysis, query router | `cargo test --test change_guard_test --offline` | **18 / 18 PASS** | 19.52s |
| `tests/mcp_test.rs` | MCP stdio JSON-RPC handshake, tool enumeration, analysis execution | `cargo test --test mcp_test --offline` | **1 / 1 PASS** | 1.49s |
| `tests/utf8_safety_test.rs` | UTF-8 multibyte boundary slicing (`safe_truncate_str`), emojis, Unicode bullets, Chinese/Japanese strings | `cargo test --test utf8_safety_test --offline` | **14 / 14 PASS** | 2.12s |
| `tests/inspect_symbol_budget_test.rs` | 16KB response budget shedding (Levels 0–7), omission tracking, payload bounding | `cargo test --test inspect_symbol_budget_test --offline` | **13 / 13 PASS** | 10.53s |
| **Rust Subtotal** | **Product Core & Transport Layer** | — | **62 / 62 PASS** | **33.87s** |

#### Rust Test Output Highlights:
```
test test_version_full_contains_pkg_version ... ok
test test_git_commit_metadata_present ... ok
test test_git_dirty_flag_boolean ... ok
test test_rustc_version_metadata_present ... ok
test test_build_profile_metadata_present ... ok
test test_utf8_14_mcp_inspect_symbol_end_to_end_no_panic ... ok
test test_inspect_symbol_oversized_response_bounded_and_valid_json ... ok
test test_inspect_symbol_immediately_above_limit_sheds_cleanly ... ok
```

---

### 2.2 Installer Safety Suite

| Test Target / File | Focus Area | Executed Command | Results | Duration |
|---|---|---|---|---|
| `tests/installer_safety_test.ps1` | Checksum verification, missing manifest fail-closed, duplicate entries, token sanitization, process-in-use protection, atomic extraction, CLI flag parsing | `powershell -ExecutionPolicy Bypass -File tests/installer_safety_test.ps1` | **8 / 8 PASS** | ~6.0s |

#### Installer Test Output:
```
[TEST] Running Engram Installer Safety Test Suite
Test 1: Checksum Mismatch Rejection (Fail-Closed) -> [PASS]
Test 2: Missing SHA256SUMS Manifest Rejection (Fail-Closed) -> [PASS]
Test 3: Duplicate/Ambiguous Manifest Entries Rejection -> [PASS]
Test 4: Token Sanitization Boundary -> [PASS]
Test 5: Process In-Use Protection -> [PASS]
Test 6: Staged Extraction and Atomic Replacement -> [PASS]
Test 7: Installer CLI Flag Parsing (--tag / --version) -> [PASS]
Test 8: Private Repository API Asset Resolution -> [PASS]
Test Results: 8 Passed, 0 Failed
```

---

### 2.3 Python Benchmark Harness Test Suites

| Test Target / File | Focus Area | Executed Command | Results | Duration |
|---|---|---|---|---|
| `test_candidate_selection.py` | Candidate identity verification, binary checksum matching, legacy env fail-closed | `python -m pytest test_candidate_selection.py` | **8 / 8 PASS** | ~10.2s |
| `test_telemetry_semantics.py` | Synthetic telemetry events, conflation prevention, byte measurements | `python -m pytest test_telemetry_semantics.py` | **4 / 4 PASS** | ~5.1s |
| `test_task_b_budget_simulation.py` | Candidate 4.2 compaction simulation, ceiling sensitivity, halt reproduction | `python -m pytest test_task_b_budget_simulation.py` | **3 / 3 PASS** | ~4.8s |
| `test_phase_5_4_23_safety_remediation.py` | Fail-closed tetrad order validation (T01–T18), budget pre-dispatch gate, streaming tracker deduplication | `python -m pytest test_phase_5_4_23_safety_remediation.py` | **23 / 23 PASS** | ~29.2s |
| **Python Subtotal** | **Harness Safety & Budget Gate** | — | **38 / 38 PASS** | **49.35s** |

#### Pytest Output Summary:
```
============================= 38 passed in 49.35s =============================
```

---

## 3. Build & Compilation Verification

1. **Compilation Check:**
   ```cmd
   cargo check --offline
   ```
   Completed in 0.28s with exit code 0.
2. **Binary Invocation:**
   ```cmd
   cargo run --bin engram -- --version
   ```
   Output:
   ```
   engram 1.5.4
   commit: 7106a2f369fe79faf5f95449378a900ab8e8eddf
   dirty: true
   rustc: rustc 1.99.0 (b940084d7 2026-09-28)
   profile: debug
   ```
3. **Zero Network Activity:** All builds and tests operated strictly from local caches.
4. **Binary Immutability Check:** The pre-existing release binary `target/release/engram.exe` was **not modified or recompiled**, preserving its exact SHA-256 hash (`78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2`).
