# Engram Phase 5.4.25: Adversarial Test Authenticity & Production-Path Verification

**Phase:** 5.4.25  
**Execution Mode:** STRICTLY OFFLINE, READ-ONLY FORENSIC AUDIT  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Previous Phase:** 5.4.24  
**Live Benchmark Authorization:** `NOT_AUTHORIZED`

---

## 1. Objective

Phase 5.4.24 reported 20 adversarial simulation scenarios (B01–B20) in `run_adversarial_simulation.py` to evaluate streaming budget enforcement, process cancellation, and multi-arm safety.

This audit evaluates the **production-path authenticity** of those tests:
1. Did the tests exercise actual production classes and methods, or isolated simulation mocks?
2. Did B02 and B03 verify live child process streaming token tracking or merely unit-test string parsing?
3. What are the true mocked boundaries and evidence strengths for each scenario?

---

## 2. In-Depth Audit of B02 and B03 (Streaming Budget Enforcement)

### 2.1 The Claim in Phase 5.4.24
Phase 5.4.24 cited B02 and B03 as empirical demonstrations that:
- B02: Usage reaching ceiling during streaming halts work.
- B03: Usage crossing ceiling in a streaming event produces a measured overshoot.

### 2.2 Forensic Inspection of `run_adversarial_simulation.py`
Inspection of lines 50–96 of `run_adversarial_simulation.py` reveals:
```python
# B02:
tracker = StreamingTokenTracker(conservative_reserve=45_000)
ev_reach = json.dumps({
    "type": "token_usage_record",
    "payload": {"thread_token_usage": {"input_tokens": 90_000, "output_tokens": 10_000, "total_tokens": 100_000}}
})
tracker.process_line(ev_reach)
has_headroom, rem, est = tracker.check_pre_dispatch_headroom(ceiling)

# B03:
tracker = StreamingTokenTracker(conservative_reserve=45_000)
ev_cross = json.dumps({
    "type": "token_usage_record",
    "payload": {"thread_token_usage": {"input_tokens": 95_000, "output_tokens": 10_000, "total_tokens": 105_000}}
})
tracker.process_line(ev_cross)
```

### 2.3 Forensic Findings
1. **No Live Process Spawned:** Neither B02 nor B03 spawned a child process or piped standard output from `codex.exe` or any subshell.
2. **Synthetic Input Events:** Hand-crafted JSON strings (`ev_reach`, `ev_cross`) were fed directly to `StreamingTokenTracker.process_line()`.
3. **Provider Reality Gap:** Real LLM providers (e.g. OpenAI API, Codex CLI) do **not** emit real-time token counts as text streams during generation. Providers emit text tokens, and only emit usage counts in a batch upon turn completion or in final response metadata.
4. **Conclusion on B02 and B03:** B02 and B03 tested the internal arithmetic of `StreamingTokenTracker` under hypothetical streaming events. They did **not** test real-time in-flight process preemption. They are classified as **`SYNTHETIC_ONLY`**.

---

## 3. Comprehensive Matrix: Production Authenticity of B01–B20

The table below provides a forensic breakdown of all 20 scenarios from Phase 5.4.24:

| ID | Title / Target | Production Code Exercised | Mocked / Simulated Boundary | What Was Actually Tested | Evidence Strength |
|---|---|---|---|---|---|
| **B01** | Usage Below Ceiling | `runner.py:StreamingTokenTracker.process_line`, `check_pre_dispatch_headroom` | Manually constructed JSON event string | In-memory token accumulation and headroom calculation when under ceiling | `MODERATE` |
| **B02** | Usage Reaches Ceiling | `runner.py:StreamingTokenTracker.process_line`, `check_pre_dispatch_headroom` | Manually constructed JSON event string | State transition to `has_headroom=False` when tokens equal ceiling | `SYNTHETIC_ONLY` |
| **B03** | Usage Crosses Ceiling | `runner.py:StreamingTokenTracker.process_line`, `check_pre_dispatch_headroom` | Manually constructed JSON event string | Overshoot calculation and `last_decision='OVERRUN_HALT'` logic | `SYNTHETIC_ONLY` |
| **B04** | Post-Turn Event Arrival | `runner.py:StreamingTokenTracker.process_line` | Mock `turn.completed` event | Token assignment occurring only upon receiving turn completion event | `MODERATE` |
| **B05** | Batch Usage Arrival | `runner.py:StreamingTokenTracker.process_line` | Mock single discrete 35k token event | Discrete batch jump in effective tokens | `MODERATE` |
| **B06** | Duplicate Usage Event | `runner.py:StreamingTokenTracker.process_line` | Duplicate mock JSON string | Monotonic idempotency; duplicate events do not increase token count | `MODERATE` |
| **B07** | Delayed Event Post-Kill | `runner.py:StreamingTokenTracker.process_line` | Sequential mock events | Draining post-cancellation lines updates token total monotonically | `MODERATE` |
| **B08** | Out-of-Order Usage | `runner.py:StreamingTokenTracker.process_line` | Reordered mock JSON strings | Non-decreasing monotonic update rule (`max(current, reported)`) | `MODERATE` |
| **B09** | Missing Usage Event | `telemetry.py:TelemetryParser.parse_file`, `orchestrator.py:Orchestrator.validate_telemetry_integrity` | Empty file on disk | Production schema and integrity validation rejecting missing telemetry | **`STRONG`** |
| **B10** | Malformed Usage Event | `runner.py:StreamingTokenTracker.process_line` | Malformed JSON payload (negative tokens) | Parser setting `telemetry_valid=False` and headroom check rejecting | **`STRONG`** |
| **B11** | Usage Exceeds Estimate | `runner.py:StreamingTokenTracker.estimate_next_request_tokens`, `process_line` | Mock two-turn event sequence | Detection of ceiling breach when turn consumption exceeds conservative estimate | `MODERATE` |
| **B12** | Scenario 13 Reproduction | `orchestrator.py:Orchestrator.is_run_accepted` | Synthetic run dictionary with 120k tokens | Production Criterion 7 rejecting run with `total_model_tokens > configured_ceiling` | **`STRONG`** |
| **B13** | Process Termination Latency | `isolation.py:terminate_pid_tree`, Windows `taskkill` | Real OS child process (`python -c "import time; time.sleep(30)"`) | Empirical wall-clock duration of `taskkill /F /T /PID` on Windows | **`STRONG`** |
| **B14** | Unresponsive Child Process | `isolation.py:terminate_pid_tree`, Windows `taskkill` | Real OS process trapping `SIGINT` | Forced kernel termination (`taskkill /F`) overriding signal traps | **`STRONG`** |
| **B15** | Post-Termination Death Check | `isolation.py:terminate_pid_tree`, `proc.poll` | Real OS child process | Verification that `poll() is not None` following taskkill | **`STRONG`** |
| **B16** | Buffered Stdout Draining | `runner.py:StreamingTokenTracker.process_line` | Array of buffered strings | Sequential processing of lines accumulated in OS pipe buffer | `MODERATE` |
| **B17** | Global Budget Exhaustion | `orchestrator.py:run_tetrad` budget logic | Simulated cumulative token counter | Pre-dispatch gate condition `rem_global < 45_000` halting downstream arms | `MODERATE` |
| **B18** | Continuation Override | `orchestrator.py:run_tetrad` exception logic | Simulated execution loop | Proof that `SafetyViolationError` halts tetrad regardless of `stop_on_failure` | **`STRONG`** |
| **B19** | Concurrent Callbacks | `runner.py:StreamingTokenTracker.process_line` | Multi-threaded python threads calling `process_line` | Thread safety of token accumulator under concurrent event callbacks | **`STRONG`** |
| **B20** | Global Ceiling Reached | `orchestrator.py` pre-dispatch gate logic | Boundary arithmetic ($G=2.2M$) | Mathematical boundary where $G_{\text{rem}} = 0 \implies$ no dispatch | `MODERATE` |

---

## 4. Evidence Classification Summary

- **`STRONG` Production Evidence (7/20):** B09, B10, B12, B13, B14, B15, B18, B19.  
  *Directly exercised production code paths against real OS processes, production schema validators, real thread locks, or real acceptance criteria.*
- **`MODERATE` Production Evidence (9/20):** B01, B04, B05, B06, B07, B08, B11, B16, B17, B20.  
  *Exercised production classes (`StreamingTokenTracker`), but input data was injected via mock JSON strings.*
- **`SYNTHETIC_ONLY` Evidence (2/20):** B02, B03.  
  *Simulated in-flight streaming preemption that does not exist in live provider environments.*

---

## 5. Architectural Implications

The simulation suite successfully validates:
1. `StreamingTokenTracker` internal state machine correctness.
2. `Orchestrator.validate_telemetry_integrity()` fail-closed behavior.
3. `Orchestrator.is_run_accepted()` Criterion 7 budget rejection.
4. Windows kernel-level process termination reliability (`taskkill /F /T`).
5. Multi-arm halting upon safety violations.

However, the audit cautions against conflating in-memory unit simulations with live provider observability. In a live run, budget enforcement remains strictly **`DETECT_AND_ABORT_ONLY`** at turn completion boundaries.
