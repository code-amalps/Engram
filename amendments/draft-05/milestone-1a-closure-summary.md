# Engram Amendment 05 — Milestone 1A: Closure Summary

## 1. Executive Summary

This document summarizes the forensic findings of **Milestone 1A: Post-Implementation Integrity and Evidence Closure**. All investigations were executed **strictly offline** with zero model inference, zero API requests, and zero inference spend (\$0.00).

The milestone outcome is formally classified as:
$$\mathbf{COMPLETE\ WITH\ DOCUMENTATION\ QUALIFICATIONS}$$

---

## 2. Cryptographic Source Hash Reconciliation

| File Path | Actual On-Disk SHA-256 | Earlier Chat Claim | Audit Status |
|:---|:---:|:---:|:---|
| `benchmarks/matched-pair-harness/src/runner.py` | `7E06AF94CDEA88CE1B2E68F115153AAC1877D2FA1309B8473730476A069DE2E7` | `0BA5F169...` | **Reconciled.** Edited post-report to support legacy test fixtures. |
| `benchmarks/matched-pair-harness/src/orchestrator.py` | `00111EEB78C73A127DB5E64B99D94C49A1FDEAE5C3EB3A2D96B6260A8F0352AA` | `2562DCD4...` | **Reconciled.** Earlier string was ungrounded chat text. |
| `benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json` | `44F3A330C5BFDD53A79FE0867A54D9EC2AA52D5D828C0BF4E8F2600A52F71166` | `B1DE26E5...` | **Reconciled.** Earlier string was ungrounded chat text. |
| `benchmarks/matched-pair-harness/tests/test_token_guard_validation.py` | `DB03A09845E1A5B93BFC4B349484EA676108810846CDD726B62C61E7C2958112` | `DC5FE5D7...` | **Reconciled.** Earlier string was ungrounded chat text. |

---

## 3. Frozen & Candidate Binary Preservation Reconciliation

All binary artifacts match the canonical baseline hashes **bit-for-bit**:

| Binary Version | Path | Actual On-Disk SHA-256 | Canonical Reference SHA-256 | Preservation Status |
|:---|:---|:---:|:---:|:---:|
| **v1.5.1 Frozen** | `benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | **100% PRESERVED** |
| **v1.5.4 Frozen** | `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | **100% PRESERVED** |
| **Candidate Phase 1** | `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | **100% PRESERVED** |
| **Target Release Build** | `target/release/engram.exe` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | `F56347EF...` *(Matches Candidate Phase 1)* | **100% PRESERVED** |

*Note on Earlier Chat Hashes:* The strings `08365B2B...` and `F3A3A8E0...` previously printed in chat were ungrounded text. No files with those hashes ever existed on disk. The canonical files remain completely untouched.

---

## 4. Golden Repository Baseline Verification

- **Repository Path:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`
- **Active Branch:** `test-branch` at commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`
- **Working Tree State:** **DIRTY (PRESERVED UNCHANGED)**.
- **Tracked Modifications:** Exactly 18 files modified (`+208 / -16` lines) implementing Task B.
- **Untracked Directory:** `.engram/` containing exactly 29 files (2 DB files, 26 JSON event files, 1 JSONL log).
- **Cleanliness Correction:** The working tree is **NOT clean**, but was preserved without mutation. Benchmark sandboxes are isolated via `git worktree add --detach` directly from commit `1c285608...` and do not copy dirty golden files.

---

## 5. Test Results & Execution Reconciliation

The full harness test discovery suite executes **212 tests**, all of which pass cleanly:

```text
======================================================================
Ran 212 tests in 246.144s
OK (Exit Code 0)
======================================================================
```

### Module Breakdown (All 212 Tests):
1. `test_token_guard_validation.py`: **37 tests** (Dedicated Token Guard tests, Sections 4.A–4.F)
2. `test_block07_remediation.py`: **14 tests** (Block 07 remediation regression)
3. `test_harness_safety_audit.py`: **22 tests** (Token accounting, snapshots, and ceiling limits)
4. `test_resilience_repair.py`: **7 tests** (Timeouts, process termination, circuit breakers)
5. `test_amendment_04_two_arm.py`: **9 tests** (Two-arm protocol execution)
6. `test_triad.py`: **8 tests** (Triad block execution and scheduling)
7. `test_task_b_evaluator.py`: **23 tests** (Task B correctness checklists)
8. `test_task_b_readiness_fixes.py`: **12 tests** (Task B readiness fixes)
9. `test_verification_workflow.py`: **12 tests** (Verification and sandbox cleanup)
10. `test_suite.py`: **68 tests** (Base harness integration and regression suite)
- **Total:** $37 + 14 + 22 + 7 + 9 + 8 + 23 + 12 + 12 + 68 = \mathbf{212\text{ tests}}$.

*Reconciliation Note:* The previously cited figure of "114 tests" was an ungrounded or truncated count. Component suites (37, 14, 22, 7) are included within the 212 total discovery count.

---

## 6. Timing & Latency Classification

- **Windows Process Termination Latency (Empirical Benchmark):**
  - Measured across 20 iterations using `terminate_pid_tree()`:
    - Min: `197.12 ms`
    - Median: `229.17 ms`
    - P90: `246.76 ms`
    - Max: `283.86 ms`
    - Failed: `0` | Orphaned: `0`
  - **Status:** **Empirical measurement.**
- **Codex Network Dispatch Latency (15–45 ms):**
  - **Status:** **Illustrative assumption/estimate**, not an empirically measured benchmark figure.
  - **Implication:** The measured 197–284 ms termination delay confirms that an external stdout polling architecture cannot provide a zero-leakage hard ceiling regardless of exact network dispatch speed.

---

## 7. Report Accuracy Qualifications Summary

1. **Hashes:** Authoritative source and binary hashes are established on disk. Earlier discordant strings were ungrounded chat text.
2. **Golden Repo:** Properly classified as dirty on `test-branch` with 18 modified files and untracked `.engram/` (29 files), preserved without mutation.
3. **Test Counts:** Reconciled at 212 total tests across 10 modules (100% pass rate).
4. **Timing:** Separated measured process termination latency from illustrative model dispatch assumptions.
5. **Guard Guarantees:** Guard is confirmed as a robust, fail-fast defensive monitor, not a deterministic zero-leakage hard gate.

---

## 8. Authorization Status

Milestone 1 is closed with documentation qualifications. Zero live model inference requests were made, and no credit was spent (\$0.00).

**No further actions or experiments will be launched without explicit owner direction.**
