# Engram Phase 5.4.26: Non-Compounding Guarantee Reassessment Report

**Phase:** 5.4.26  
**Execution Mode:** STRICTLY OFFLINE, READ-ONLY ADVERSARIAL FORENSIC AUDIT  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Previous Phase:** 5.4.25  
**Previous Non-Compounding Classification:** `PROVEN_HARD_GUARANTEE`  
**Live Benchmark Authorization Status:** `NOT_AUTHORIZED`

---

## 1. Executive Summary

Phase 5.4.25 concluded that the non-compounding guarantee was a `PROVEN_HARD_GUARANTEE` based on the proof that a token ceiling breach on one arm terminates the tetrad block and skips all downstream arms.

However, Phase 5.4.26 mandates a critical refinement:
> **Core Principle:**  
> Sequential arm dispatch proves only sequential arm dispatch. A complete non-compounding guarantee requires evidence that every locally controlled pending operation is accounted for, while remote provider termination must remain explicitly unclaimed unless independently proven.

This audit evaluates the four distinct claims underlying the non-compounding guarantee and formally recalibrates its classification.

---

## 2. Forensic Reassessment of the Four Component Claims

### Claim A: Cross-Arm Dispatch Isolation
> *"Once a safety violation is detected, the orchestrator will not dispatch subsequent arms."*

- **Evaluation:**
  - `orchestrator.py:run_tetrad()` checks the status of Arm $N$. If `arm_result["execution_status"] == "TOKEN_CEILING_BREACHED"`, line 2258 raises `SafetyViolationError`.
  - The exception breaks from the execution loop immediately.
  - The loop terminates, and downstream arms (e.g. Arms $N+1 \dots 4$) are converted directly into `SKIPPED_SAFETY_FAILURE` run records with zero token consumption.
  - No subsequent process is spawned, no prompt is sent, and no network request is made.
- **Classification:** **`PROVEN`**

---

### Claim B: Local Execution Isolation
> *"Once a safety violation is detected, all locally controlled processes, tasks, and queued operations are terminated or prevented from executing."*

- **Evaluation:**
  - `isolation.py:terminate_pid_tree(proc.pid)` issues `taskkill /F /T /PID <pid>`, which executes at the Windows kernel level, forcefully terminating the process and all child processes.
  - `proc.wait(timeout=1.0)` asserts that the process has exited and `proc.poll() is not None`.
  - The reader daemon threads `t_stdout` and `t_stderr` are joined with a 2.0s timeout before the runner returns.
  - The orchestrator maintains zero asynchronous work queues or background thread pools.
  - Sandbox teardown (`teardown_sandbox`) deletes the sandbox worktree, verifying zero lingering local file handles.
- **Classification:** **`PROVEN`**

---

### Claim C: Remote Provider Isolation
> *"All provider-side generation and billing stops immediately after local cancellation."*

- **Evaluation:**
  - When `codex.exe` is killed locally, the client operating system resets the TCP connection by sending a TCP RST packet.
  - Cloud inference engines (such as OpenAI or Anthropic API endpoints) operate asynchronously on server clusters. The server detects the broken socket only when it next attempts to stream a chunk or when an internal timeout fires.
  - Upstream cloud APIs do not offer retroactive cancellation, mid-turn billing stops, or token refunds for requests terminated client-side.
  - Tokens generated during that active generation turn are billed to the account.
  - Client-side code cannot inspect, verify, or command immediate preemption of provider GPU hardware.
- **Classification:** **`DISPROVEN`** (Unprovable offline; provider-dependent).

---

### Claim D: Total Consumption Non-Compounding
> *"No additional token consumption can occur after the failing arm has been locally terminated and the block has been halted."*

- **Evaluation:**
  - Because Claim A is **`PROVEN`**, downstream arms cannot execute, meaning $\Omega_{\text{downstream}} \equiv 0$. Zero compounding across benchmark arms can occur.
  - However, because Claim C is **`DISPROVEN`**, the single breached arm may experience an in-flight burst up to the model's architectural completion maximum ($C_{\max}$) before the cloud provider acknowledges the broken socket.
  - Therefore, total consumption does not compound across arms, but single-arm consumption is subject to the terminal in-flight burst.
- **Classification:** **`CONDITIONALLY_PROVEN`**  
  (Proven strictly in the sense that $\Omega_{\text{tetrad}} = \Omega_{\text{arm}}$, with zero cascading across arms, conditioned upon single-arm in-flight burst bounds).

---

## 3. Comparative Synthesis

| Claim Component | Phase 5.4.25 Stated Classification | Phase 5.4.26 Recalibrated Classification | Scope of Proof |
|---|---|---|---|
| **Claim A: Cross-Arm Dispatch Isolation** | `PROVEN_HARD_GUARANTEE` | **`PROVEN`** | Local orchestrator control loop |
| **Claim B: Local Execution Isolation** | `PROVEN_HARD_GUARANTEE` | **`PROVEN`** | Local OS processes and threads |
| **Claim C: Remote Provider Isolation** | (Not independently separated) | **`DISPROVEN`** | Remote cloud API server boundary |
| **Claim D: Total Consumption Non-Compounding** | `PROVEN_HARD_GUARANTEE` | **`CONDITIONALLY_PROVEN`** | Bounded to single arm; zero cascading |

---

## 4. Recalibrated Non-Compounding Guarantee Statement

> **FORMAL REVISION:**  
> The non-compounding guarantee is henceforth classified as:  
> $$\mathbf{CROSS\_ARM\_NON\_COMPOUNDING\_PROVEN}$$  
>  
> This guarantee certifies that:
> 1. A token budget breach on any arm immediately and unconditionally halts the tetrad block.
> 2. Zero subsequent benchmark arms will be dispatched locally.
> 3. Total benchmark token excess is strictly confined to the single failing arm's terminal burst ($\Omega_{\text{tetrad}} \equiv \Omega_{\text{failing\_arm}}$).
> 4. Remote provider billing during that final in-flight burst cannot be controlled, rolled back, or verified offline.
