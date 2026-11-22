# Engram Phase 5.9.0: Failure Recovery & Exception Handling Protocol

**Execution Phase:** 5.9.0  
**Target Systems:** Five-Arm Pentad Benchmark Harness  
**Execution Mode:** SPECIFICATION & RECOVERY PROTOCOL  

---

## 1. Exception Handling Matrix

| Failure Mode | Interception Layer | Detection Mechanism | Immediate Action | Final Run Status |
| :--- | :--- | :--- | :--- | :---: |
| **Provider Rate Limiting (429)** | Subprocess Runtime | Stderr JSON error / retry loop | Exponential backoff (max 3 tries) | `RATE_LIMITED` |
| **Provider Outage / Network Drop**| Subprocess Runtime | Stream socket closure | Terminate process tree safely | `NETWORK_DISCONNECT` |
| **Authentication Failure (401)** | Preflight / Turn 1 | API key rejected | Immediate abort of entire block | `AUTH_FAILURE` |
| **Model Identifier Invalid** | Preflight | CLI error from Codex | Immediate pre-dispatch halt | `INVALID_MODEL` |
| **Execution Timeout (>600s)** | External Watcher | `runner.py` line 967 elapsed timer | Send SIGTERM, wait 1s, force kill | `TIMEOUT` |
| **MCP Server Process Crash** | Stdio Transport | Subprocess pipe EOF | Log diagnostic crash payload | `MCP_CRASH` |
| **Per-Arm Budget Exhaustion** | Pre-Dispatch Guard | `runner.py` headroom check | Safe preventative turn halt | `HEADROOM_EXHAUSTED` |
| **Global Budget Exhaustion** | Orchestrator | Cumulative token accumulator | Immediate abort of all downstream arms | `GLOBAL_BUDGET_HALT` |
| **Fixture Contamination** | Sandbox Cleanup | Post-run git status on golden repo | Log contamination alert & fail | `DIRTY_FIXTURE` |
| **Candidate Binary Hash Mismatch**| Pre-dispatch | SHA-256 validation vs ledger | Throw `SafetyViolationError` | `SAFETY_VIOLATION` |

---

## 2. In-Session Retry Prohibition Policy

> [!CRITICAL]
> **No Automated In-Session Retries:** The harness will **NEVER** automatically restart a failed arm within the same run session. Automatically re-dispatching against live cloud endpoints risks recursive billing loops and creates unmetered shadow token consumption.

Every failure results in:
1. Complete serialization of all partial telemetry captured up to the failure point.
2. Clean teardown and unlinking of the temporary sandbox folder.
3. Marking the affected arm with explicit failure metadata (`error_message`, `stop_reason`).
4. Halting downstream arms under `--execution-policy strict`.

---

## 3. Resumption & Re-Execution Protocol

If a block halts prematurely due to a transient infrastructure issue (e.g., local network blip or provider 500):
1. **Forensic Inspection:** The auditor inspects `pentad_summary.json` and raw logs in the output run folder.
2. **Clean Slate Principle:** To maintain matched-pair validity, a failed task cannot be repaired by "patching" one arm. The entire 5-arm block must be re-dispatched under a new pair identifier (`pentad_live_pilot_task_a_retry01`) and a distinct PRNG seed.
3. **Budget Accounting:** Any tokens consumed during an aborted attempt remain accounted for in the cumulative project ledger and cannot be expunged.
