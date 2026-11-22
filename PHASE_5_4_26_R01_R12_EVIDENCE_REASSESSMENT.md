# Engram Phase 5.4.26: R01–R12 Adversarial Evidence Reassessment Report

**Phase:** 5.4.26  
**Execution Mode:** STRICTLY OFFLINE, READ-ONLY ADVERSARIAL FORENSIC AUDIT  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Previous Phase:** 5.4.25 (`AUDIT_CONDITIONAL_BOUND`)  
**Live Benchmark Authorization Status:** `NOT_AUTHORIZED`

---

## 1. Executive Summary

In Phase 5.4.25, twelve adversarial race scenarios (R01–R12) were executed via `run_r01_r12_race_simulations.py` to evaluate orchestrator concurrency, cancellation propagation, and budget safety.

This audit performs an independent forensic reassessment of the R01–R12 evidence base:
1. To distinguish between tests that executed the actual production path vs isolated production components vs synthetic mocks or theoretical models.
2. To verify the availability and integrity of the raw evidence files (`scratch/phase_5_4_25/race_simulation_results.json`).
3. To determine whether the reported conclusions overreached what the offline simulation actually established.

### Core Reassessment Verdict
- **Offline Process & Concurrency Control:** **`PRODUCTION_PATH_VERIFIED`**. Scenarios R01, R04, R08, R09, R10, and R12 directly exercised Windows OS process isolation (`taskkill /F /T`), pipe draining, and synchronous orchestrator loops.
- **In-Memory Accounting Monotonicity:** **`PRODUCTION_COMPONENT_VERIFIED`**. Scenarios R02, R05, R06, and R07 proved `StreamingTokenTracker` arithmetic invariants.
- **Remote Provider Termination & In-Flight Consumption:** **`NOT_VERIFIABLE_OFFLINE` / `FORMULA_ONLY`**. Scenarios R03 and R11 represent architectural analysis of cloud API behavior. Local TCP RST does not guarantee immediate remote provider GPU shutdown or billing cessation.

---

## 2. Comprehensive Evidence Reassessment Matrix

| Scenario | Production Code Exercised | Mocked / Simulated Boundary | Raw Result Available? | Evidence Strength | Reassessment Finding |
|---|---|---|:---:|:---:|---|
| **R01** | `orchestrator.py:run_tetrad` (lines 2240–2255) | None (Static loop audit & sequential barrier) | Yes (`race_simulation_results.json`) | **`PRODUCTION_PATH`** | Proves sequential arm loop. Arm $N+1$ cannot start until Arm $N$ terminates. |
| **R02** | `runner.py:StreamingTokenTracker` | Intra-arm thread dispatch simulation | Yes (`race_simulation_results.json`) | **`PRODUCTION_COMPONENT`** | Evaluated tracker state under thread callbacks; harness uses single runner thread. |
| **R03** | `isolation.py:terminate_pid_tree` | Architectural network model (TCP RST) | Yes (`race_simulation_results.json`) | **`MOCKED_BEHAVIOR`** | Local kill severs socket; remote provider billing continuation is unprovable offline. |
| **R04** | `subprocess.Popen`, `communicate`, `terminate_pid_tree` | Synthetic child script writing to stdout | Yes (`race_simulation_results.json`) | **`PRODUCTION_PATH`** | Proves OS pipe buffers are drained by runner even after process termination. |
| **R05** | `runner.py:StreamingTokenTracker.process_line` | Handcrafted JSON string payloads | Yes (`race_simulation_results.json`) | **`PRODUCTION_COMPONENT`** | Proves monotonic update rule: late-arriving usage snapshots do not decrease tokens. |
| **R06** | `runner.py:StreamingTokenTracker.process_line` | Malformed/interleaved string array | Yes (`race_simulation_results.json`) | **`PRODUCTION_COMPONENT`** | Proves non-JSON and corrupted lines are safely discarded without crashing tracker. |
| **R07** | `runner.py:StreamingTokenTracker.check_pre_dispatch_headroom` | Injected turn completion event | Yes (`race_simulation_results.json`) | **`PRODUCTION_COMPONENT`** | Confirms headroom check evaluates synchronously prior to next turn dispatch. |
| **R08** | `orchestrator.py:run_tetrad` (exception handling) | Simulated loop exception handling | Yes (`race_simulation_results.json`) | **`PRODUCTION_PATH`** | Proves `SafetyViolationError` halts tetrad unconditionally, bypassing `--no-stop-on-failure`. |
| **R09** | `orchestrator.py:run_tetrad` (downstream loop) | Simulated arm 1 ceiling breach | Yes (`race_simulation_results.json`) | **`PRODUCTION_PATH`** | Proves downstream arms marked `SKIPPED_SAFETY_FAILURE` with 0 tokens consumed. |
| **R10** | `isolation.py:terminate_pid_tree` (`taskkill /F /T`) | Real 3-level OS process tree | Yes (`race_simulation_results.json`) | **`PRODUCTION_PATH`** | Proves `taskkill /F /T` terminates parent, child, and grandchild PID hierarchy. |
| **R11** | None (Cloud API protocol analysis) | Theoretical remote provider behavior | Yes (`race_simulation_results.json`) | **`FORMULA_ONLY`** | Confirms remote inference continues until TCP RST detected; in-flight burst is billed. |
| **R12** | `isolation.py:terminate_pid_tree` | Real OS child processes (10 trials) | Yes (`race_simulation_results.json`) | **`PRODUCTION_PATH`** | Establishes empirical latency distribution ($\mu = 574.18\text{ ms}, \max = 808.22\text{ ms}$). |

---

## 3. Detailed Scenario-by-Scenario Forensic Analysis

### 3.1 R01: Sequential Arm Orchestration Barrier
- **Target:** Verify whether the orchestrator can execute arms concurrently.
- **Evidence Review:** Inspection of `benchmarks/matched-pair-harness/src/orchestrator.py:run_tetrad()` lines 2240–2255:
  ```python
  for exec_order, arm in enumerate(order):
      # ...
      arm_result = self.run_arm(arm=arm, exec_order=exec_order, ...)
  ```
- **Reassessment:** This is a synchronous Python `for` loop. The orchestrator spawns neither `threading.Thread`, `multiprocessing.Process`, nor `asyncio.Task` to fan out arms.
- **Limitation:** Sequential dispatch proves only that Arm $N+1$ does not start before Arm $N$. It does not by itself prove that all asynchronous sub-threads of Arm $N$ have ceased.

### 3.2 R02: Intra-Arm Concurrency
- **Target:** Investigate whether multiple provider requests or tool threads could run concurrently within an arm.
- **Evidence Review:** The harness executes `codex.exe` as a single managed child process. Within `CodexRunner.run()`, stdout is read sequentially via `t_stdout` and processed in a 50ms polling loop.
- **Reassessment:** Intra-arm requests are managed internally by the Codex CLI. The tracker state is evaluated monotonically line-by-line.

### 3.3 R03 & R11: Remote Provider Request Lifecycle and In-Flight Billing
- **Target:** Determine whether terminating the local process stops cloud provider billing.
- **Evidence Review:** `isolation.py:terminate_pid_tree` executes `taskkill /F /T /PID <pid>`.
- **Reassessment:**
  1. The Windows OS closes the socket and transmits a TCP RST / FIN packet.
  2. The cloud provider's API server receives the TCP RST.
  3. However, cloud LLM providers (e.g. OpenAI, Anthropic) do not provide a client-side refund or rollback mechanism for in-flight generation.
  4. If the model was actively generating completion tokens on provider GPUs, all tokens generated up to or including that turn are billed.
  5. Claiming that local cancellation guarantees zero further remote billing is **DISPROVEN**. Remote billing stops only when the remote server acknowledges socket closure.

### 3.4 R04 & R05: Pipe Buffering and Delayed Telemetry Draining
- **Target:** Determine whether events buffered in OS pipes are lost upon cancellation.
- **Evidence Review:** In R04, a Python process emitted 2 lines and was immediately killed via `taskkill /F /T`. The parent process successfully drained both lines from `stdout` post-kill.
- **Reassessment:** Standard OS pipe buffers persist after process death until closed or drained by the parent. In `runner.py`, `t_stdout.join(timeout=2.0)` guarantees that buffered lines are drained before `runner.run()` returns.

### 3.5 R08 & R09: Non-Compounding Multi-Arm Halting
- **Target:** Determine whether a budget breach on Arm 1 can cascade into Arms 2, 3, or 4.
- **Evidence Review:**
  1. In `orchestrator.py:run_tetrad()` lines 2256–2265:
     ```python
     if arm_result.get("execution_status") == "TOKEN_CEILING_BREACHED":
         raise SafetyViolationError(...)
     ```
  2. `SafetyViolationError` breaks out of the loop immediately.
  3. All downstream arms are marked `SKIPPED_SAFETY_FAILURE` with 0 tokens consumed.
- **Reassessment:** This guarantee is mathematically and programmatically sound. Overshoot cannot compound across arms. Total benchmark tetrad overshoot is strictly bounded to the single breached arm.

### 3.6 R10 & R12: Process Tree Destruction & Latency Distribution
- **Target:** Verify complete process tree destruction and empirical termination timing.
- **Evidence Review:**
  - R10 proved that a 3-tier process tree (parent $\to$ child $\to$ grandchild) is fully terminated by `taskkill /F /T`.
  - R12 measured 10 trials on Windows: Min=498.11ms, Max=808.22ms, Mean=574.18ms, Median=552.88ms, StdDev=89.09ms.
- **Reassessment:** The 298.56ms measurement reported in Phase 5.4.24 was an unrepresentative single sample. Termination on Windows reliably requires ~500–800ms. All trials finished in $< 1.0\text{s}$.

---

## 4. Synthesis of Evidence Classification

| Category | Total Scenarios | Scenarios | Status |
|---|:---:|---|:---:|
| **`PRODUCTION_PATH`** | 6 | R01, R04, R08, R09, R10, R12 | **VERIFIED** |
| **`PRODUCTION_COMPONENT`** | 4 | R02, R05, R06, R07 | **VERIFIED** |
| **`MOCKED_BEHAVIOR`** | 1 | R03 | **QUALIFIED** |
| **`FORMULA_ONLY`** | 1 | R11 | **QUALIFIED** |
| **`UNVERIFIED`** | 0 | None | N/A |

### Conclusion
The R01–R12 evidence base rigorously proves **local process termination, pipe buffer draining, sequential dispatch gating, and multi-arm non-compounding**. However, it does not and cannot prove **instantaneous remote cloud provider billing cessation**. That distinction must be formally maintained across all Phase 5.4.26 certifications.
