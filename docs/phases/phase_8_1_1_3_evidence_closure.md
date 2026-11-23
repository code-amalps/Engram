# Engram Phase 8.1.1.3: Evidence Closure & Verification Baseline

**Document status:** Final Verified Baseline  
**Date:** 2026-11-23  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Remote:** `origin` (`code-amalps/Engram`)  
**Branch:** `feature/phase2-stage5-remediation`  

---

## 1. Commit Genealogy & Discrepancy Resolution

### 1.1 Linear Ancestry Verification
All Phase 8 commits form a strict, unbroken linear descent from the immutable certified release baseline:
1. **Release Baseline `v1.5.4-cand5`:** `2df1cac683435314ffd7ad52369e6d20198fae0d`
2. **Phase 8.1.1.2 Implementation Commit:** `bd8f9372254017e0380e8f310e967f23a604d77e` (Added live stdio OS pipe benchmark, Category 1/2 workloads, and in-repo raw dataset)
3. **Phase 8.1.1.2 Test Refinement Commit:** `2cd8bd2e0ffe2e36ba36a696f3af7e7058cc2593` (Corrected latency rounding assertion tolerances for multi-platform CI)
4. **Phase 8.1.1.2 Documentation Commit:** `c9ca585639282b470cbe00da3a1f8768fd5f4d20` (Formal certification report and benchmark deliverables index)

### 1.2 Discrepancy Resolution for Final Report Commit
- **Discrepancy Description:** The prompt noted a previously reported final report commit string of `c9ca585090f707f12bc8f3073173d1221b2d3581`, whereas Git log resolved to `c9ca585639282b470cbe00da3a1f8768fd5f4d20`.
- **Root Cause & Forensic Finding:** The first 7 characters (`c9ca585`) match identically. The trailing 33 characters in the earlier report string were an erroneous transcription artifact. The true Git object in the tree and on remote `origin` is confirmed as:
  `c9ca585639282b470cbe00da3a1f8768fd5f4d20`.
- **Ancestry Check:** `git merge-base --is-ancestor 2df1cac c9ca585` returned exit code `0`, confirming complete cryptographic descent.

---

## 2. GitHub Actions CI Matrix Verification (Phase 8.1.1.2 Baseline)

The multi-platform CI matrix ran on commit `2cd8bd2e0ffe2e36ba36a696f3af7e7058cc2593` under Workflow Run ID **`45150284124`** (`Cross-Platform CI & Verification`):

| Job Name | Platform | Runner OS | Status | Conclusion | Run ID |
|---|---|---|---|---|---|
| Linux glibc (x86_64) | x86_64-unknown-linux-gnu | `ubuntu-latest` | completed | **success** | `45150284124` |
| Linux musl (x86_64) | x86_64-unknown-linux-musl | `ubuntu-latest` | completed | **success** | `45150284124` |
| macOS Intel (x86_64) | x86_64-apple-darwin | `macos-15-intel` | completed | **success** | `45150284124` |
| macOS ARM64 (aarch64) | aarch64-apple-darwin | `macos-14` | completed | **success** | `45150284124` |
| Windows MSVC (x86_64) | x86_64-pc-windows-msvc | `windows-latest` | completed | **success** | `45150284124` |
| Rustfmt & Clippy | Lint & Static Analysis | `ubuntu-latest` | completed | **success** | `45150284124` |

**Matrix Result:** 6/6 jobs succeeded (100% green). Verified via GitHub REST API log capture.

---

## 3. Workstream A: Benchmark State-Reset Remediation

### 3.1 Defect Remediated
Previously, `tests/engage_bench_test.rs` suppressed filesystem errors during between-run state cleanup using `let _ = fs::remove_dir_all(...)`. If directory deletion or recreation failed silently, stale contracts or session metadata could leak into subsequent test runs.

### 3.2 Implemented Fix
1. **Fallible Signature & Error Propagation:** Converted `reset_workspace_contract_state` to return `std::io::Result<()>`, propagating all I/O errors directly using `?`.
2. **Fail-Closed Verification:** Explicitly checks that `.engram/contracts` and `.engram/sessions` exist and asserts via `fs::read_dir` that the directory item count is strictly `0`. If any stale artifact survives, an `io::ErrorKind::Other` error is raised immediately.
3. **Fail-Closed Benchmark Execution:** Updated every benchmark invocation site in `tests/engage_bench_test.rs` to invoke `.expect("Workspace state reset failed before baseline/engage")`.
4. **Unit Test Suite Added:**
   - `test_state_reset_success_and_staleness_verification`: Creates dummy active contracts and sessions, executes reset, and asserts post-condition emptiness.
   - `test_state_reset_fails_on_creation_error_blocking_file`: Tests failure mode when directory creation is blocked by an existing file, confirming fail-closed error return.
   - `test_state_reset_idempotency_on_empty_or_missing_dirs`: Verifies reset idempotency on clean, empty, or uninitialized directories.

### 3.3 Verification Results
Execution of `cargo test --test engage_bench_test` yielded:
```text
running 5 tests
test test_state_reset_idempotency_on_empty_or_missing_dirs ... ok
test test_state_reset_success_and_staleness_verification ... ok
test test_validate_raw_results_integrity_and_statistics ... ok
test test_state_reset_fails_on_creation_error_blocking_file ... ok
test test_rigorous_30_run_live_stdio_benchmark ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 135.66s
```

---

## 4. Architectural & Benchmark Limitations

As documented in `docs/phases/phase_8_1_1_2_final_certification_report.md` Section 4.1:
1. **Persistent Daemon Process:** The benchmark tests a persistent `engram mcp` server process communicating over OS pipes (`stdin`/`stdout`), accurately modeling IDE integrations (e.g. Cursor, VS Code, Antigravity).
2. **State Cleanup Scope:** Workspace state directories (`.engram/contracts` and `.engram/sessions`) were reset between workflows, but not between baseline individual calls within a single workflow.
3. **No Cold-Process Certification:** The empirical results do not measure cold process startup time (where a process is spawned fresh for a single tool call and terminates).
4. **Warm-Session Execution Timing:** All reported latencies measure warm-session JSON-RPC tool-call round-trips over live OS pipes.

---

## 5. Immutable Release Baseline & Tag Preservation

All previously certified tags and release assets remain completely untouched:
- **`v1.5.4-cand5` Tag:** Annotated tag `6b4beab25232d9154f3aeb5cba69074b1e42880e` pointing to commit `2df1cac683435314ffd7ad52369e6d20198fae0d` (preserved).
- **`v1.5.4` Tag:** Annotated tag `6608e96bfad246415a77eeef2e1cb15a44358896` pointing to commit `a75a2c668b5a03496667b9fe7924d67362df2f2a` (preserved).
- **Published ZIP Artifact:** `CDD34117B9ED1B0575306915AC888BECAE48EB0127F8A7DCEE9D504938D89A6A` (preserved).
- **Published Binary Asset:** `1C5DD68B2BD6498F46FD0F81119E15EB73C4CE3FDE2C3005EFD255EF7CDA0F67` (preserved).
- **Legal Governance Status:** Preserved as `TECHNICAL_SOURCE_CORRESPONDENCE_VERIFIED_LEGAL_REVIEW_PENDING`.
