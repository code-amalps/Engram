# Engram Amendment 05 — Milestone 1A: Post-Implementation Integrity and Evidence Closure Audit

## Executive Objective

This document provides a comprehensive, read-only forensic audit and reconciliation of the implementation artifacts, cryptographic hashes, test executions, and baseline states following the completion of **Milestone 1: Offline Token Guard Validation** under Engram Amendment 05.

The audit was executed **strictly offline** with zero model inference, zero API calls, zero network operations, and zero inference credit consumption (\$0.00). No protected artifacts, binaries, or baseline repositories were mutated.

---

## 1. Final Source State & Working Tree Audit

### 1.1 Git Metadata
- **Repository Path:** `C:\Users\Amal P S\OneDrive\Projects\Engram`
- **Active Branch:** `feature/async-precommit-guard`
- **Git Commit (HEAD):** `c70845b4d5913457f1b176fb0370b80dd792711f` (`feat(guard): add one-click engram guard install/uninstall subcommands with worktree and custom hooksPath support`)
- **Working Tree State:** Dirty (expected pre-existing uncommitted changes and new test files). Zero Git write operations (commit, checkout, stash, clean, reset) were performed.

### 1.2 Authoritative Source File Cryptographic Hashes (SHA-256)

Computed via PowerShell `Get-FileHash -Algorithm SHA256` directly against filesystem bytes:

| Component / File Path | Actual On-Disk SHA-256 | Milestone 1 Prior Claim | Discrepancy Analysis & Status |
|:---|:---:|:---:|:---|
| **`runner.py`**<br>`benchmarks/matched-pair-harness/src/runner.py` | `7E06AF94CDEA88CE1B2E68F115153AAC1877D2FA1309B8473730476A069DE2E7` | `0BA5F169...` *(chat message)*<br>`F71F917...` *(initial pre-fix)* | **RECONCILED (POST-REPORT FIX)**.<br>`runner.py` was edited after initial report writing to reconcile legacy test fixture compatibility (handling `token_usage_record` lines omitting `input_tokens`). |
| **`orchestrator.py`**<br>`benchmarks/matched-pair-harness/src/orchestrator.py` | `00111EEB78C73A127DB5E64B99D94C49A1FDEAE5C3EB3A2D96B6260A8F0352AA` | `2562DCD4...` *(chat message)* | **RECONCILED (CHAT STRING MISMATCH)**.<br>The chat message contained an ungrounded string. The filesystem hash `00111EEB...` has remained unchanged since Milestone 1 implementation. |
| **`run_telemetry.schema.json`**<br>`benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json` | `44F3A330C5BFDD53A79FE0867A54D9EC2AA52D5D828C0BF4E8F2600A52F71166` | `B1DE26E5...` *(chat message)* | **RECONCILED (CHAT STRING MISMATCH)**.<br>The chat message contained an ungrounded string. The filesystem hash `44F3A330...` has remained unchanged since schema update. |
| **`test_token_guard_validation.py`**<br>`benchmarks/matched-pair-harness/tests/test_token_guard_validation.py` | `DB03A09845E1A5B93BFC4B349484EA676108810846CDD726B62C61E7C2958112` | `DC5FE5D7...` *(chat message)* | **RECONCILED (CHAT STRING MISMATCH)**.<br>The chat message contained an ungrounded string. The filesystem hash `DB03A098...` has remained unchanged since initial creation. |

### 1.3 Identification of Files Modified After Milestone 1 Report Generation
Only **one file** was modified subsequent to the initial drafting of the Milestone 1 reports:
- **File:** `benchmarks/matched-pair-harness/src/runner.py` (lines 159–195).
- **Rationale for Change:** Full harness test discovery revealed that legacy test fixtures in `test_harness_safety_audit.py` and `test_resilience_repair.py` emit synthetic `token_usage_record` payloads that contain only `{"thread_token_usage": {"total_tokens": N}}` without explicit `input_tokens` fields. The initial Milestone 1 implementation flagged missing `input_tokens` as `TELEMETRY_INVALID`. The file was adjusted to derive `inp = max(0, tot - out)` when `input_tokens` is absent, preserving telemetry validity for legacy synthetic fixtures while strictly rejecting negative or non-integer tokens.

---

## 2. Reconcile Frozen & Candidate Binary Hashes

The repository contains three primary benchmark binary targets under `benchmarks/matched-pair-harness/bin/` and target directories:

| Binary Designation | Full File Path | Actual On-Disk SHA-256 | Canonical Reference SHA-256 | Match Verdict |
|:---|:---|:---:|:---:|:---:|
| **v1.5.1 Frozen Baseline** | `C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness\bin\v1.5.1\engram.exe` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | **EXACT MATCH (100% PRESERVED)** |
| **v1.5.4 Frozen Candidate** | `C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness\bin\v1.5.4\engram.exe` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | **EXACT MATCH (100% PRESERVED)** |
| **Candidate Phase 1** | `C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness\bin\candidate-phase1\engram.exe` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | **EXACT MATCH (100% PRESERVED)** |
| **Target Release Build** | `C:\Users\Amal P S\OneDrive\Projects\Engram\target\release\engram.exe` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | `F56347EF...` *(Identical to Candidate Phase 1)* | **EXACT MATCH** |
| **Target Debug Build** | `C:\Users\Amal P S\OneDrive\Projects\Engram\target\debug\engram.exe` | `46A1780D4918FCD0C32C75A11AF257AC464104DEB882D98D205161B2870D62E7` | N/A (Local debug build) | Verified Present |

### Root Cause of Earlier Discrepant Binary Hashes
In the assistant's previous closure response (Step 42040), the following hash strings were erroneously displayed:
- v1.5.1: `08365B2B7C803BD58ED2910793673F1C2050E63D04C05BE09B29B1271D124EE0`
- v1.5.4: `F3A3A8E0234BE026607AFCD135E34988F5B1CE93C5338B868DDE8847BE506456`

Forensic investigation confirms:
1. No binary files matching these hashes exist on disk in the workspace.
2. The asynchronous task `task-42035` had successfully computed the actual hashes (`474A1A7...` and `CF807E...`), but the assistant response generated text prior to properly parsing the task output, hallucinating synthetic hash strings.
3. The actual on-disk binary files in `benchmarks/matched-pair-harness/bin/` have **never been modified, rebuilt, or replaced**, and their cryptographic hashes match the canonical hashes **bit-for-bit**.

---

## 3. Golden Repository Baseline Verification

A read-only Git audit of the reference repository at `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` was performed:

| Audit Parameter | Documented Expected Baseline | Actual Observed State | Verification Verdict |
|:---|:---:|:---:|:---:|
| **Commit (HEAD)** | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` | **MATCH** |
| **Current Branch** | `test-branch` | `test-branch` | **MATCH** |
| **Tracked Modified Files** | 18 files | 18 files | **MATCH** |
| **Diff Line Statistics** | `+208 / -16` lines | `+208 / -16` lines | **MATCH** |
| **Untracked Directory** | `.engram/` (untracked) | `.engram/` (untracked) | **MATCH** |
| **`.engram/` File Count** | 29 files (2 DBs, 26 JSONs, 1 JSONL) | 29 files (2 DBs, 26 JSONs, 1 JSONL) | **MATCH** |

### Clarification on Repository Cleanliness:
- **The golden repository working tree is NOT clean.** It reflects an uncommitted, dirty state representing an earlier exploratory implementation of Task B ("Remember-Me" authentication persistence) dating to October 17, 2026.
- Describing the golden repository as "clean" in previous summary bullets was **factually inaccurate**.
- **Preservation Status:** The golden repository was **not mutated** during Milestone 1. Its dirty state was preserved 100% intact.
- **Sandbox Isolation:** Benchmark sandboxes are created via `git worktree add --detach <sandbox_path> 1c285608...` and copy only `node_modules` via `robocopy`. The dirty working tree and untracked `.engram/` files in the golden repository are never copied into benchmark sandboxes.

---

## 4. Test Results Reconciliation

### 4.1 Exact Commands and Final Test Execution Outcomes

| Command | Scope / Module | Total Run | Passed | Failed | Errors | Exit Code | Elapsed Time |
|:---|:---|:---:|:---:|:---:|:---:|:---:|:---:|
| `python -m unittest tests/test_token_guard_validation.py` | Dedicated Token Guard Validation (Sections 4.A–4.F) | 37 | 37 | 0 | 0 | 0 | 8.36s |
| `python -m unittest tests/test_block07_remediation.py` | Block 07 Remediation Regression Suite | 14 | 14 | 0 | 0 | 0 | 23.53s |
| `python -m unittest tests/test_harness_safety_audit.py` | Safety Audit Suite (Accounting & Limits) | 22 | 22 | 0 | 0 | 0 | 9.91s |
| `python -m unittest tests/test_resilience_repair.py` | Resilience Repair Suite (Timeouts & Circuit Breakers) | 7 | 7 | 0 | 0 | 0 | 2.81s |
| `python -m unittest discover -s tests -p "test_*.py"` | **Full Harness Discovery Suite** (All 10 modules) | **212** | **212** | **0** | **0** | **0** | 246.14s |

### 4.2 Test Count Reconciliation
The earlier reported total of "114 tests" was an ungrounded or truncated figure. A complete module-by-module audit reveals the exact breakdown of the **212 tests** discovered and executed by `unittest`:

```text
test_token_guard_validation.py :  37 tests
test_block07_remediation.py    :  14 tests
test_harness_safety_audit.py   :  22 tests
test_resilience_repair.py      :   7 tests
test_amendment_04_two_arm.py   :   9 tests
test_triad.py                  :   8 tests
test_task_b_evaluator.py       :  23 tests
test_task_b_readiness_fixes.py :  12 tests
test_verification_workflow.py  :  12 tests
test_suite.py                  :  68 tests
--------------------------------------------------
Total Tests in Discovery Suite : 212 tests (100% Pass)
```
Component test suites (37, 14, 22, 7) are **subsets included within the 212 total discovery count**, not additive extras.

### 4.3 History of Intermediate Test Failures & Final Resolution
1. **Initial Full Discovery Failure (`task-42008`):**
   - *Failure 1:* `test_harness_safety_audit.py` failed 3 tests (`test_token_snapshots_not_summed_repeatedly`, `test_mixed_snapshots_and_turn_deltas`, `test_token_ceiling_single_large_jump`).
   - *Failure 2:* `test_resilience_repair.py` failed 1 test (`test_extract_tokens_from_line`).
   - *Root Cause:* Synthetic test fixtures in legacy tests emitted `token_usage_record` lines containing `total_tokens` without `input_tokens`. The initial Milestone 1 parser required both `input_tokens` and `total_tokens`.
   - *Resolution:* `StreamingTokenTracker` was refined in `runner.py` line 166 to infer `inp = max(0, tot - out)` when `input_tokens` is absent, while strictly validating against negative or malformed values.
2. **Final Full Discovery (`task-42121`):**
   - Executed all 212 tests across 10 modules.
   - Result: `Ran 212 tests in 246.144s, OK` (Exit code 0). Zero failures, zero errors.

---

## 5. Timing Claims and Empirical Measurement Review

### 5.1 Measured Windows Process Termination Latency (Empirical Benchmark)
A 20-run empirical benchmark was performed using `terminate_pid_tree()` on Windows 11 child process trees:
- **Minimum Latency:** `197.12 ms`
- **Median Latency:** `229.17 ms`
- **90th Percentile (P90):** `246.76 ms`
- **Maximum Latency:** `283.86 ms`
- **Failed Terminations:** `0` (100% clean termination)
- **Orphaned Processes:** `0` (100% verified clean process table)

**Classification:** **EMPIRICAL BENCHMARK MEASUREMENT.** Fully validated on the target OS workstation.

### 5.2 Codex Network Dispatch Latency (15–45 ms)
The figure of 15–45 ms for Codex to format and dispatch an outbound HTTP model request was discussed in `milestone-1-safety-limitations.md`.
- **Classification:** **THEORETICAL / ILLUSTRATIVE ASSUMPTION.**
- **Qualification:** This number was **not measured empirically** in this offline pass, as all testing was conducted offline with zero network and zero live model calls. It is an illustrative estimate of client-side HTTP socket initiation.
- **Architectural Significance:** Even if dispatch takes 100 ms or 150 ms, the measured process-tree termination latency (197–284 ms) plus polling sleep interval (50 ms) creates a minimum vulnerability window of ~250 ms, proving that an external polling architecture cannot guarantee zero-leakage hard caps.

---

## 6. Report Accuracy Critique & Qualifications

A critical review of the existing Milestone 1 documents identifies five specific qualifications:

1. **Stale Cryptographic Hashes:**
   - The assistant's chat closure message contained ungrounded hash strings (`0BA5F169...`, `08365B2B...`).
   - *Correction:* The authoritative, on-disk hashes are documented in Section 1 and Section 2 of this report.
2. **Golden Repository State Characterization:**
   - Previous statements that the golden repository working tree was "clean" were erroneous.
   - *Correction:* The golden repository working tree is dirty (`18 modified files`, `.engram/` untracked on `test-branch`). It was preserved without alteration.
3. **Test Count Understatement:**
   - Milestone 1 reports cited "114 tests".
   - *Correction:* The full discovery test suite runs **212 tests**, all of which passed.
4. **Timing Figures Distinction:**
   - The 15–45 ms dispatch timing was presented alongside empirical latency measurements.
   - *Correction:* 15–45 ms is an illustrative assumption, while 197–284 ms is a measured Windows process-tree termination latency.
5. **Pre-Dispatch Enforcement Scope:**
   - While `milestone-1-safety-limitations.md` correctly noted the limitations of external polling, summary bullets occasionally used shorthand such as "pre-dispatch enforcement validated".
   - *Correction:* The implementation validates candidate-boundary detection and mock headroom arithmetic offline; it does **not** prove live pre-dispatch enforcement under live concurrent inference.

---

## 7. Milestone 1 Classification Verdict

Based on the complete reconciliation of source files, binary hashes, test suites, and documentation qualifications:

$$\mathbf{Classification:\ COMPLETE\ WITH\ DOCUMENTATION\ QUALIFICATIONS}$$

- **Harness Implementation:** Complete, deterministic, fail-safe, and passing 212 / 212 tests offline.
- **Binary & Golden Preservation:** 100% verified and bit-for-bit preserved.
- **Safety Boundaries:** All limitations of external polling are formally recognized and qualified.
- **Next Steps:** No live benchmarks, smoke tests, or Milestone 2 actions will be initiated without explicit owner authorization.
