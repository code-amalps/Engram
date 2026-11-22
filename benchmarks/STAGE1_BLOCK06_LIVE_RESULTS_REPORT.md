# Engram — Stage 1 Block 06 Live Triad Results & Post-Mortem Report

**Date:** 2026-10-28T21:30:00+05:30 (16:00:00 UTC)  
**Task Under Test:** `task_b_remember_me` (Authentication flow with username persistence)  
**Block ID:** `block_06`  
**Planned Schedule Order:** `["baseline_arm1", "v151_arm2", "v154_arm3"]`  
**Status:** **HALTED ON BREACHED LIMIT (ARM 1 TIMEOUT & TOKEN CEILING EXCEEDED)**  
**Token Savings Claims:** **STRICTLY SUPPRESSED (INVALID)**

---

## 1. Executive Summary & Verdict

In accordance with the explicit authorization granted for Stage 1 Block 06 only, live execution commenced at 21:11:49 IST (15:41:49 UTC) using the approved command:
```bash
python benchmarks/matched-pair-harness/src/orchestrator.py --block block_06
```

During execution of the first arm (**Arm 1: `baseline_arm1`**):
1. **Timeout Limit Breached:** Codex was terminated after hitting the hard **600-second execution timeout** (per-arm limit).
2. **Token Ceiling Breached:** Total accumulated turn tokens reached **1,254,712 tokens**, exceeding the **250,000-token per-arm ceiling**.
3. **Telemetry Capture Anomaly:** Because `proc.communicate(timeout=600)` raised `TimeoutExpired` inside `runner.py`, `stdout` was not returned to write `events.jsonl` in the run directory. When `orchestrator.py` unconditionally attempted `parser.parse_file(events_jsonl_path)`, an unhandled `FileNotFoundError` halted orchestrator execution before launching Arm 2 (`v151_arm2`) and Arm 3 (`v154_arm3`).
4. **Safety & Containment:** The child process tree was safely killed (`taskkill /F /T /PID`), the sandbox was torn down cleanly, and the golden repository dependency fingerprint was verified intact.
5. **Protocol Compliance:** In accordance with the mandatory instruction (*"Stop immediately on HTTP 429, missing/unreliable telemetry, integrity violations, or a breached limit. Do not retry failed executions automatically"*), the harness and agent stopped immediately. **Zero retries were performed.**

```
========================================================================================
Stage 1 Block 06 Triad Status: HALTED (Arm 1 Failed)
  - Baseline Arm 1 (`baseline_arm1`): FAILED (Timeout 600s, 1.25M tokens, C = 0.00)
  - v1.5.1 Arm 2 (`v151_arm2`):       NOT EXECUTED (Halted per stop-on-failure rule)
  - v1.5.4 Arm 3 (`v154_arm3`):       NOT EXECUTED (Halted per stop-on-failure rule)
  - Primary Savings Claim:            STRICTLY SUPPRESSED (C = 0.00 / Incomplete Run)
========================================================================================
```

---

## 2. Forensic Analysis of Arm 1 (`baseline_arm1`)

Although `events.jsonl` was not flushed to the run vault due to the Python subprocess timeout, the full event stream was safely preserved in the native Codex rollout directory:
`C:\Users\Amal P S\.codex\sessions\2026\10\28\rollout-2026-10-28T21-13-35-01a1832e-52cf-7280-bd92-a6c055a91249.jsonl` (176 JSONL events).

### 2.1 Exploration & Implementation Phase (Turns 1–9)
- **Prompt Delivery:** Codex received the exact Task B prompt with safety instructions and balanced discovery guidance.
- **Ripgrep Discovery (FIX-3):** Codex executed `rg -n --hidden -S "login|sign.?in|username|remember|auth"` on Turn 1. Ripgrep was discovered and executed cleanly without error, confirming FIX-3 was effective.
- **Code Inspection:** Codex inspected `app.component.ts`, `core.state.ts`, `auth.actions.ts`, and `auth.reducer.ts`.
- **Code Editing:** Codex authored two patches updating `auth.actions.ts`, `auth.reducer.ts`, and `app.component.ts`, adding a remember-me flag and local-storage persistence logic.

### 2.2 The Verification Spiral (Turns 10–23)
- **Turn 10 (`npm run lint`):** Instead of running a targeted file lint or build check, Codex executed `npm run lint` across the entire workspace via `tools.exec_command`.
- **Interactive Stdin Loop (Turns 11–17):** In Angular CLI projects, full linting on Windows took longer than the tool yield time, prompting Codex to attach via `tools.write_stdin({session_id: 55910, chars: ""})` in an interactive polling loop.
- **Token Inflation:** Because each turn in Codex re-transmits the complete conversation history (~72,510 tokens per turn), repeated polling caused turn-accumulated token usage to balloon rapidly:
  - Turn 1: 14,725 tokens
  - Turn 5: 129,257 tokens
  - Turn 10: 442,887 tokens (Ceiling breached)
  - Turn 18: 1,254,712 tokens
- **Karma Test Runner (Turns 19–23):** After terminating the lint process, Codex invoked `ng test --configuration=test --watch=false --browsers=ChromeHeadless` (Session `59365`). It entered another `write_stdin` loop waiting for Karma to finish.
- **Hard Timeout at 600s:** At 15:53:35 UTC (10 minutes elapsed), `runner.py`'s 600-second timeout expired. `terminate_pid_tree()` killed the Codex process tree.

### 2.3 Provider Telemetry Snapshot at Termination
- **Model:** `gpt-5.6-luna` (Reasoning Effort: `medium`)
- **Single-Turn Context Window:** 72,510 input tokens (72,192 cached, 99.6% cache hit rate).
- **Cumulative Thread Tokens:** 1,254,712 total model tokens (1,248,320 input, 6,392 output, 1,812 reasoning).
- **OpenAI Account Quota:** Primary used percentage: 6.0% (resets at 1795779772). Zero HTTP 429 errors occurred.

---

## 3. Harness Architecture Post-Mortem

### 3.1 Defect 1: Timeout Handling in `runner.py`
In `benchmarks/matched-pair-harness/src/runner.py` (lines 250–276):
```python
stdout_data, stderr_data = proc.communicate(timeout=self.timeout_seconds)
with open(output_events_path, "w", encoding="utf-8") as f:
    f.write(stdout_data)
```
When `subprocess.TimeoutExpired` is raised:
- Python raises before `proc.communicate()` returns `(stdout_data, stderr_data)`.
- The exception handler catches `TimeoutExpired` and terminates the process, but `stdout_data` is unbound.
- `output_events_path` (`events.jsonl`) is never created on disk.

### 3.2 Defect 2: Missing File Check in `orchestrator.py`
In `benchmarks/matched-pair-harness/src/orchestrator.py` (line 725):
```python
exec_res = self.runner.run(...)
telemetry = parser.parse_file(events_jsonl_path, thread_id=exec_res.thread_id)
```
- `orchestrator.py` assumed `events.jsonl` is always present, even when `exec_res.exit_code != 0`.
- Because `events.jsonl` was missing, `parser.parse_file()` raised `FileNotFoundError`, abruptly crashing the orchestrator instead of recording an arm failure ($C = 0.00$) and cleanly reporting the triad verdict.

---

## 4. Verification of System Invariants

1. **Golden Repository Integrity:**
   - Pre-run baseline fingerprint matched.
   - Post-run golden dependency check verified clean before teardown:
     `Sandbox cleaned up safely; golden dependency integrity verified.`
   - Golden repository commit remains `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.
2. **Protected Benchmark Assets:**
   - `benchmarks/cost-baseline/`: **Untouched** (verified via `git status`).
   - `benchmarks/profile/`: **Untouched** (verified via `git status`).
   - Frozen binaries (`v1.5.1`, `v1.5.4`): **Untouched** (`474a1a7a...`, `cf807eac...`).
   - Preregistration (`PREREGISTRATION.md`): **Untouched** (`db3ab295...`).
3. **Execution Boundaries:**
   - Blocks 07–10: **Zero executions**.
   - Block 06 Arms 2 and 3: **Zero executions** (halted immediately upon Arm 1 failure).
   - Retries: **Zero automatic retries attempted**.

---

## 5. Comparative Assessment

| Arm | Binary | Status | Correctness ($C$) | Model Tokens | Execution Time | Savings Claim |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: |
| **Arm 1 (`baseline_arm1`)** | Native Baseline | **FAILED (Timeout / Limit Breached)** | **0.00** | 1,254,712 | 600.0s | N/A |
| **Arm 2 (`v151_arm2`)** | Engram v1.5.1 | **NOT RUN** | N/A | 0 | 0.0s | N/A |
| **Arm 3 (`v154_arm3`)** | Engram v1.5.4 | **NOT RUN** | N/A | 0 | 0.0s | **STRICTLY SUPPRESSED** |

---

## 6. Recommended Next Actions

To maintain scientific integrity and harness reliability before any future authorized live runs:

1. **Harness Resilience Fix (Offline):**
   - Update `runner.py` to capture partial stdout or recover session events from `~/.codex/sessions` upon timeout.
   - Update `orchestrator.py` to handle `exec_res.exit_code != 0` gracefully: record a failed arm record with $C = 0.00$, timeout status, and skip subsequent evaluation gates without crashing with `FileNotFoundError`.
2. **Task B Verification Guidance:**
   - In Task A, the agent ran targeted checks (`npx jest ...`).
   - In Task B, the native baseline agent attempted full-workspace `npm run lint` and headless Karma tests, which hang on Windows when spawned through interactive subshells. Adding a note in the task description clarifying standard headless verification command patterns or constraints will prevent non-productive polling loops.
3. **Standby:**
   - All live operations are halted. Awaiting explicit user direction.
