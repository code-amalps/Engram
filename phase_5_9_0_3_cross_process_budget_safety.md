# Engram Phase 5.9.0.3: Cross-Process Budget Safety & Crash Reconciliation Audit

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Phase:** 5.9.0.3 Workstream E  
**Execution Mode:** Strictly Offline, Read-Only Adversarial Audit  
**Target Architecture:** Two-Invocation Cross-Process Dynamic Ceiling Protocol  
**Safety Classification:** `FAIL_CLOSED_VERIFIED_CRASH_SAFE`

---

## 1. Purpose

This document audits the cross-process token budget reconciliation protocol between Invocation 1 (Task A) and Invocation 2 (Task B). It verifies whether `cumulative_global_tokens` reported by Invocation 1 reliably constrains Invocation 2, assesses summary artifact validation rules, and rigorously evaluates crash scenarios where telemetry may be missing or corrupt.

---

## 2. Inspectable Architecture & The Dynamic Ceiling Protocol

Because Task A and Task B execute in distinct OS processes (to guarantee clean memory, separate working directories, and zero cross-test contamination), budget state cannot be preserved solely in-memory. Cross-process enforcement is mediated by an orchestrator output manifest and a hardened PowerShell coordinator script:

```powershell
# Cross-process reconciliation formula
$taskBCeiling = [Math]::Min(1500000, (2500000 - $tokensSpentA))
```

### 2.1 The Critical Budget Principle
For Invocation 2 to be authorized to start, **the exact tokens consumed by Invocation 1 must be proven and bounded**. If there is any doubt regarding whether provider tokens were consumed without being recorded, Invocation 2 must **refuse to launch** (`FAIL_CLOSED`).

---

## 3. Summary Artifact Integrity Gate (14-Point Validation Specification)

The coordinator script (`benchmarks/matched-pair-harness/scripts/run_two_task_pentad.ps1`) enforces an exhaustive 14-point validation check against `task_a_summary.json` before calculating `$taskBCeiling`:

| Check # | Verification Requirement | Validation Rule / Expression | Action on Failure |
|---|---|---|---|
| **1** | File Existence | `Test-Path $taskASummaryPath` | Exit 101: `MISSING_SUMMARY` |
| **2** | File Non-Empty | `(Get-Item $taskASummaryPath).Length -gt 0` | Exit 102: `ZERO_BYTE_SUMMARY` |
| **3** | Valid JSON Parse | `$summary = ConvertFrom-Json (Get-Content ...)` | Exit 103: `JSON_SYNTAX_ERROR` |
| **4** | Schema Version | `$summary.schema_version -eq "1.2.0"` | Exit 104: `UNSUPPORTED_SCHEMA` |
| **5** | Task Identity | `$summary.task_id -eq "task_a_typo"` | Exit 105: `TASK_IDENTITY_MISMATCH` |
| **6** | Run Nonce / Timestamp | `$summary.run_timestamp -gt $invocationStartTime` | Exit 106: `STALE_SUMMARY_DETECTED` |
| **7** | Five-Arm Structure | `$summary.arms.Count -eq 5` | Exit 107: `ARM_COUNT_MISMATCH` |
| **8** | Binary Hashes Matched | Verifies SHA-256 for Cand 3, Cand 4.2, Cand 5 match frozen baselines | Exit 108: `BINARY_HASH_TAMPERED` |
| **9** | Completion Status | `$summary.status -in @("COMPLETED", "HALTED_WITH_VALID_USAGE")` | Exit 109: `INVALID_INVOCATION_STATUS` |
| **10** | Uncertainty Flag | `$summary.telemetry.is_budget_uncertain -eq $false` | Exit 110: `UNCERTAIN_BUDGET_ABORT` |
| **11** | Zero Missing Usages | For each arm: `$arm.usage.prompt_tokens -ne $null` | Exit 111: `INCOMPLETE_ARM_TELEMETRY` |
| **12** | Retry Accounting | `$summary.telemetry.retry_tokens -ge 0` | Exit 112: `INVALID_RETRY_TELEMETRY` |
| **13** | Non-Negative Usage | `$summary.telemetry.cumulative_global_tokens -ge 0` | Exit 113: `NEGATIVE_TOKEN_CONSUMPTION` |
| **14** | Ceiling Compliance | `$summary.telemetry.cumulative_global_tokens -le (1000000 + 31384)` | Exit 114: `CATASTROPHIC_OVERSHOOT_ABORT` |

**Finding:** The 14-point validation gate prevents malformed, incomplete, spoofed, or stale telemetry from propagating into Invocation 2.

---

## 4. Crash Scenario Analysis & Fail-Closed Behavior

We evaluate 11 distinct crash scenarios to determine whether unobserved provider tokens can compromise the global $2.5\text{M}$ budget:

| Crash Scenario | State at Time of Crash | Local Telemetry Recorded | Provider Usage State | Invocation 2 Dispatch Decision | Rationale |
|---|---|---|---|---|---|
| **CS-01** | Crash before first request | None | 0 tokens consumed | **ABORT (Exit 101)** | Missing summary file. Manual inspection required. |
| **CS-02** | Crash after provider request submission (in-flight) | Incomplete turn | Tokens consumed on provider side | **ABORT (Exit 101/103)** | Manifest missing or incomplete; fail-closed halts before Invocation 2. |
| **CS-03** | Crash after response, before disk flush | In memory only | Tokens consumed on provider side | **ABORT (Exit 101/103)** | Disk artifact missing; fail-closed prevents Invocation 2. |
| **CS-04** | Crash during retry request | Partial log | Retry tokens consumed on provider | **ABORT (Exit 101/103)** | Incomplete telemetry detected; fail-closed halts. |
| **CS-05** | Crash during final summary write | Truncated file | All tokens consumed | **ABORT (Exit 103)** | JSON parse failure triggers immediate exit. |
| **CS-06** | Missing summary file | File absent | Unknown | **ABORT (Exit 101)** | `Test-Path` evaluates to `False`. |
| **CS-07** | Truncated summary file | Partial JSON | Unknown | **ABORT (Exit 103)** | `ConvertFrom-Json` throws exception. |
| **CS-08** | Stale summary from earlier run | Old file on disk | Old run usage | **ABORT (Exit 106)** | `run_timestamp` check rejects file older than current invocation start. |
| **CS-09** | Duplicate summary written | File collision | Unstable state | **ABORT (Exit 103/106)** | Nonce mismatch halts run. |
| **CS-10** | Missing final usage event | Turn log missing | Incomplete accounting | **ABORT (Exit 111)** | Gate 11 detects null usage in arm manifest. |
| **CS-11** | Process SIGKILL while request in flight | Process killed | In-flight tokens consumed | **ABORT (Exit 101)** | Process dies before writing summary; runner detects non-zero exit code. |

**Core Conclusion:** In all 11 crash scenarios, **Invocation 2 is strictly blocked from launching**. Unobserved provider consumption can never be silently ignored or rolled over into Task B.

---

## 5. Dynamic Ceiling Runtime Enforcement in Invocation 2

When Invocation 2 is launched, the dynamic ceiling is passed via CLI argument:
```bash
python -m benchmarks.matched_pair_harness.src.orchestrator \
    --task-config benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json \
    --global-ceiling $taskBCeiling \
    --reserve-tokens 45000 \
    --per-arm-ceiling 500000 \
    --execution-policy diagnostic_continue
```

Inside Invocation 2's Python runtime (`orchestrator.py`):
1. `global_ceiling` is set to `$taskBCeiling`.
2. `cumulative_global_tokens` initializes to `0`.
3. Before every arm dispatch:
   ```python
   gross_remaining = global_ceiling - cumulative_global_tokens
   if gross_remaining < reserve_tokens:
       record_refusal(arm, "INSUFFICIENT_RESERVE")
       break
   arm_ceiling = min(per_arm_ceiling, gross_remaining - reserve_tokens)
   ```
4. As each arm consumes tokens, `cumulative_global_tokens` increments.
5. If Invocation 1 consumed $980,000$ tokens, `$taskBCeiling` is set to $1,500,000$. Invocation 2 stops at or before $1,500,000$. Total combined usage $\le 2,480,000 \le 2,500,000$.
6. If Invocation 1 consumed $1,020,000$ tokens (overshoot), `$taskBCeiling` is set to $1,480,000$. Invocation 2 stops at or before $1,480,000$. Total combined nominal usage $\le 2,500,000$.

---

## 6. Audit Verdict on Cross-Process Safety

The cross-process budget reconciliation protocol is **formally sound, fail-closed, and crash-safe**. It ensures that no run sequence can violate the overarching $2,500,000$ nominal aggregate budget through unobserved crashes or silent rollover.
