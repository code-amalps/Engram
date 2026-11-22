# Engram Amendment 05 — Milestone 1A: Final Forensic Reconciliation Report

## Executive Objective

This report delivers a strictly read-only, final forensic reconciliation of Engram Amendment 05 Milestone 1A, addressing three specific evidentiary topics:
1. **Resolution of the Git HEAD discrepancy** between Pass 5 and Milestone 1A records.
2. **Verification of the supporting evidence for the 212-test discovery execution**, including exact execution logs, exit codes, elapsed times, and module uniqueness.
3. **Rigorous qualification of the ~250 ms vulnerability-window timing claims**, explicitly distinguishing empirical operating-system measurements, code-configured polling intervals, and unmeasured end-to-end dispatch and network latencies.

All investigations were conducted **strictly offline** with zero model inference, zero API calls, zero network access, zero test reruns, and zero spend (\$0.00). No Git write operations were performed, and all protected artifacts (binaries, Amendment 04, historical reports, and golden repository state) remain 100% preserved.

---

## 1. Resolution of Git HEAD Discrepancy

### 1.1 Candidate Commit Hash Comparison
Two divergent commit hashes appear in the historical Amendment 05 documentation:
- **Hash A (Pass 3–5 Snapshot Record):** `c70845b410522197609f98485cb487f5d08409e5`
- **Hash B (Milestone 1 / 1A Observed Record):** `c70845b4d5913457f1b176fb0370b80dd792711f`

### 1.2 Read-Only Git Object Database & Reflog Inspection
A forensic query was executed against the local Git object database:

```powershell
cd "C:\Users\Amal P S\OneDrive\Projects\Engram"
git cat-file -t c70845b410522197609f98485cb487f5d08409e5
# Output: fatal: git cat-file: could not get object info (Exit code 128)

git cat-file -t c70845b4d5913457f1b176fb0370b80dd792711f
# Output: commit (Exit code 0)
```

The Git commit metadata for Hash B confirms:
- **Object Type:** `commit`
- **Author/Commit Date:** Mon Oct 19 19:15:37 2026 +0530
- **Commit Subject:** `feat(guard): add one-click engram guard install/uninstall subcommands with worktree and custom hooksPath support`
- **Active Branch:** `feature/async-precommit-guard`

Inspecting `git reflog -n 30` confirms:
- `HEAD@{0}` has pointed to `c70845b4d5913457f1b176fb0370b80dd792711f` continuously since commit creation.
- Zero commits, zero branch checkouts, zero resets, and zero merges have occurred on `feature/async-precommit-guard` throughout Passes 1 through 5, Pass 5A, Milestone 1, and Milestone 1A.

### 1.3 Forensic Determination & Origin Trace
1. **Hash A (`c70845b41052...`) is not a Git commit.** It does not exist in the object database and has no parentage or tree objects.
2. **Historical Origin:** In Pass 3 Step 8 (transcript step 41295), the assistant executed `git status --short` but omitted `git rev-parse HEAD`. When composing `snapshot-after-3.md`, the string `c70845b410522197609f98485cb487f5d08409e5` was generated via transcription error / hallucination.
3. **Propagation:** This errant string was subsequently copied into `snapshot-before-4.md`, `snapshot-after-4.md`, and `snapshot-before-5.md`.
4. **Exposure:** In Pass 5 Step 8, an explicit Python command queried `git rev-parse HEAD` directly from Git, recording the true hash `c70845b4d591...` in `snapshot-after-5.md` and exposing the discrepancy.
5. **Conclusive Classification:** **EARLIER REPORTING / TRANSCRIPTION ERROR.** The repository HEAD was never modified or transitioned; `c70845b4d5913457f1b176fb0370b80dd792711f` is the true, unchanging HEAD of `feature/async-precommit-guard`.

---

## 2. Supporting Evidence for the 212-Test Result

### 2.1 Final Test Execution Log Identification
The reported 212-test result was captured during Milestone 1 regression validation:
- **Task Identifier:** `task-42121`
- **Exact Log File Path:**  
  `C:\Users\Amal P S\.gemini\antigravity\brain\96df9171-c910-4b4f-ae52-2add01693b56\.system_generated\tasks\task-42121.log`
- **Log URI:**  
  `file:///C:/Users/Amal%20P%20S/.gemini/antigravity/brain/96df9171-c910-4b4f-ae52-2add01693b56/.system_generated/tasks/task-42121.log`

### 2.2 Execution Metadata and Log Verification
Inspecting `task-42121.log` and system task metadata confirms:
- **Working Directory:** `C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness`
- **Exact Command Executed:** `python -m unittest discover -s tests -p "test_*.py"`
- **Start Time:** 2026-10-30T04:52:40Z (10:22:40 IST)
- **Completion Time:** 2026-10-30T04:56:54Z (10:26:54 IST)
- **Elapsed Execution Time:** `246.144s` (~4 minutes 6 seconds)
- **Process Exit Code:** `0` (Success)
- **Authoritative Output String (Lines 93–95):**
  ```text
  ----------------------------------------------------------------------
  Ran 212 tests in 246.144s

  OK
  ```
- **Failures / Errors:** `0` failures, `0` errors, `0` skipped.

### 2.3 Verification of the 10-Module Discovery Breakdown
To establish whether the test discovery output represents independent tests or imported duplicates, a programmatic inspection of the `unittest.defaultTestLoader` discovery structure was conducted.

#### Module-by-Module Breakdown:
| Discovery Module Name | Test File Path | TestCase Classes | Number of Tests |
|:---|:---|:---|:---:|
| `test_token_guard_validation` | `tests/test_token_guard_validation.py` | `TestTokenAccounting`, `TestHeadroomCalculation`, `TestEventBoundaryDetection`, `TestProcessTermination`, `TestStopClassification`, `TestTelemetryIntegrity` | 37 |
| `test_block07_remediation` | `tests/test_block07_remediation.py` | `TestDynamicTokenPolling`, `TestCandidateBoundaryDetection`, `TestFastPathMicroEdits`, `TestProcessIsolationTeardown` | 14 |
| `test_harness_safety_audit` | `tests/test_harness_safety_audit.py` | `TestHarnessSafetyAudit` | 22 |
| `test_resilience_repair` | `tests/test_resilience_repair.py` | `TestResilienceRepair` | 7 |
| `test_amendment_04_two_arm` | `tests/test_amendment_04_two_arm.py` | `TestAmendment04TwoArmProtocol` | 9 |
| `test_triad` | `tests/test_triad.py` | `TestTriadOrchestration` | 8 |
| `test_task_b_evaluator` | `tests/test_task_b_evaluator.py` | `TestTaskBEvaluator` | 23 |
| `test_task_b_readiness_fixes` | `tests/test_task_b_readiness_fixes.py` | `TestTaskBReadinessFixes` | 12 |
| `test_verification_workflow` | `tests/test_verification_workflow.py` | `TestVerificationWorkflow` | 12 |
| `test_suite` | `tests/test_suite.py` | `TestSchemaValidator`, `TestDependencyIsolationAndIntegrity`, `TestProcessLifecycleSafety`, `TestTelemetryAccounting`, `TestReproducibilityAndControlIsolation`, `TestComparativeSavingsGuard`, `TestTokenProvenanceAndCacheIntegrity`, `TestSandboxFilesystemAndCommandAccess`, `TestTaskAFixtureAndAcceptanceEvaluator`, `TestCodeReviewFindingsRemediation` | 68 |
| **Total Discovered** | **10 test files** | **23 test classes** | **212** |

#### Test ID Independence & Collision Analysis:
- Total test instances collected by `unittest`: **212**
- Distinct `test.id()` strings collected: **212**
- **Collision / Duplicate Count:** **0**
- **Import Duplication Check:** `test_suite.py` (68 tests) contains standalone test classes authored for earlier benchmark milestones. None of the tests in `test_suite.py` are re-imported or aliased into the newer test modules.
- **Why `test_token_efficiency.py` is not included:** `tests/test_token_efficiency.py` contains standalone `pytest` test functions (`def test_...`) rather than `unittest.TestCase` subclasses. Python's standard `unittest` test discovery automatically ignores functions that do not inherit from `unittest.TestCase`. Hence, it does not contribute to the 212 count.
- **Conclusion:** The 212 tests are strictly unique, distinct unit and integration tests. The component test counts cited in earlier reports (e.g. 37, 14, 22, 7) are **integral subsets** of this single 212-test suite.

---

## 3. Review of the ~250 ms Vulnerability-Window Statement

The Milestone 1 and 1A documents referenced an "approximately 250 ms vulnerability window" (or 198–350 ms). To ensure rigorous scientific accuracy, this statement is formally decomposed into measured empirical components, code-enforced bounds, and unmeasured theoretical assumptions.

### 3.1 Measured Component: Windows Process-Tree Termination Latency
- **Mechanism:** `terminate_pid_tree(pid)` in `isolation.py` executing `taskkill /F /T /PID <pid>` on Windows 11.
- **Empirical Measurement:** 20 sequential trials conducted on disposable process trees (spawning nested Python processes and terminating the root PID):
  - **Minimum Latency:** `197.12 ms`
  - **Median Latency:** `229.17 ms`
  - **90th Percentile (P90):** `246.76 ms`
  - **Maximum Latency:** `283.86 ms`
  - **Failed Terminations:** `0` (100% success rate)
  - **Orphaned Processes:** `0` (verified clean OS process table)
- **Scientific Classification:** **EMPIRICAL BENCHMARK MEASUREMENT.** This is a verified property of the Windows OS process manager deallocating tree resources.

### 3.2 Configured Component: Parent Polling Interval Delay
- **Mechanism:** `CodexRunner` main polling loop in `runner.py`:
  ```python
  time.sleep(self.poll_interval_seconds) # poll_interval_seconds = 0.05
  ```
- **Delay Contribution:** A discrete delay between $0\text{ ms}$ (if an event arrives just as sleep finishes) and $50\text{ ms}$ (if an event arrives immediately after sleep begins).
- **Average Delay:** $25.0\text{ ms}$.
- **Scientific Classification:** **DETERMINISTIC CODE BOUND.**

### 3.3 Derived Reactive Response Latency (Parent Interception Window)
Summing the code polling interval with the empirical termination latency yields the time required for the *harness* to observe an event on stdout and kill the process:
$$\text{Best-Case Interception Time} = 0\text{ ms (poll)} + 197.12\text{ ms (kill)} = 197.12\text{ ms}$$
$$\text{Median Interception Time} = 25\text{ ms (mean poll)} + 229.17\text{ ms (kill)} = \mathbf{254.17\text{ ms}\ (\approx 250\text{ ms})}$$
$$\text{Worst-Case Interception Time} = 50\text{ ms (poll)} + 283.86\text{ ms (kill)} = 333.86\text{ ms}$$

### 3.4 Unmeasured Components (Must NOT Be Presented as Measured Data)
The following parameters are **theoretical estimates or unmeasured external latencies**:
1. **Stdout Pipe Buffering ($1\text{--}15\text{ ms}$):** In Windows anonymous pipes and Node/Python runtimes, stdout lines may experience small OS buffer delays before becoming readable by `readline()`. This was not clocked with microsecond kernel tracing.
2. **Codex Client Outbound Dispatch Latency ($15\text{--}45\text{ ms}$):** The time required for Codex to process tool completion, assemble JSON payload for the next turn, and invoke `POST /v1/chat/completions`. Because all testing in Milestone 1 was offline with synthetic mocks, **this was never measured empirically**.
3. **Network Transmission and Upstream LLM Ingestion Latency:** The time required for TCP packets to travel across the internet and be accepted by OpenAI/model servers. This is unmeasured and variable.
4. **End-to-End Token Leakage Window:** The total time from when an agent model decides to trigger a turn until the model provider bills tokens.

### 3.5 Formal Qualification
> **The "~250 ms vulnerability window" is an empirical measurement of local operating-system process-tree termination ($229\text{ ms}$) combined with harness polling delay ($25\text{ ms}$). It is NOT a measured end-to-end network dispatch or leakage latency.**
>
> However, because the measured local termination delay alone ($\ge 197\text{ ms}$) is substantially larger than typical local client HTTP socket establishment times ($< 50\text{ ms}$), the architectural conclusion remains valid: **the external polling architecture cannot guarantee zero-leakage hard caps**, and must be paired with an adequate reserve buffer.

---

## 4. Reconciled Baseline and Protected Artifact Verification

| Component | Path / Location | Baseline Hash / State | Observed Verification State | Status |
|:---|:---|:---:|:---:|:---:|
| **Engram HEAD** | `Projects/Engram` | `c70845b4d591...` | `c70845b4d5913457f1b176fb0370b80dd792711f` | **MATCH (UNTOUCHED)** |
| **v1.5.1 Binary** | `matched-pair-harness/bin/v1.5.1/engram.exe` | `474A1A7AAFBB...` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | **MATCH (UNTOUCHED)** |
| **v1.5.4 Binary** | `matched-pair-harness/bin/v1.5.4/engram.exe` | `CF807EACDEB3...` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | **MATCH (UNTOUCHED)** |
| **Candidate Phase 1** | `matched-pair-harness/bin/candidate-phase1/engram.exe` | `F56347EF2F0B...` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | **MATCH (UNTOUCHED)** |
| **Golden Repository** | `Projects/Test/angular-ngrx-material-starter` | `1c2856086f3a...` (dirty) | `18 files modified` (`+208/-16`), `.engram/` untracked (29 files) | **PRESERVED DIRTY BASELINE** |
| **Amendment 04 & Historical Reports** | `benchmarks/*.md` | Historical baselines | All historical files preserved without edits | **PRESERVED** |

---

## 5. Conclusion & Audit Sign-Off

The three reconciliation targets are completely resolved:
1. **Git HEAD:** Confirmed as an earlier reporting error in Pass 3–5 snapshot composing; true HEAD has always been `c70845b4d5913457f1b176fb0370b80dd792711f`.
2. **Test Evidence:** Confirmed by `task-42121.log` (212 tests, 0 failures, 246.144s, exit code 0) across 10 distinct, non-overlapping modules.
3. **Vulnerability Window:** Confirmed as an empirical OS termination and polling latency measurement ($197\text{--}334\text{ ms}$, median $254\text{ ms}$), with network and dispatch latencies properly designated as unmeasured illustrative assumptions.
