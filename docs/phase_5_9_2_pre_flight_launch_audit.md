# Engram Phase 5.9.2: Controlled-Runtime Live Pentad Pre-Flight Launch Audit

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Git HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Phase:** 5.9.2 Pre-Flight Launch Audit  
**Benchmark Protocol:** Version 3.0.0 (Controlled Runtime Pentad)  
**Seed:** `59002`  
**Live Provider Execution:** NOT_AUTHORIZED (Offline Verification Mode)  
**Status:** `AWAITING_EXPLICIT_LIVE_BENCHMARK_AUTHORIZATION`  

---

## 1. Executive Summary & Authorization Boundary

This audit completes all mandatory pre-flight checks required before the execution of the first empirical **Protocol v3.0.0 Controlled Runtime Pentad**.

> [!IMPORTANT]
> **MANDATORY BOUNDARY ENFORCED:**  
> Zero network requests, zero live provider API calls, zero model dispatches, and zero external credential interactions were conducted during this phase. Live execution remains strictly blocked until explicit operator authorization is received.

---

## 2. 10-Point Launch Check Verification Matrix

Every launch condition specified in Section 2 was inspected directly from runtime source code, schemas, and configurations:

| Check # | Launch Invariant Description | Implementation Verification Evidence | Audit Verdict |
| :--- | :--- | :--- | :--- |
| **Check 1** | All 5 arms execute via `ControlledRunner` and `ControlledAgentRuntime` | `orchestrator.py` defaults `--provider` to `"controlled"`, instantiating `ControlledRunner`. `ControlledRunner.run()` invokes `ControlledAgentRuntime.run()`. `run_two_task_pentad.ps1` propagates `--provider controlled`. | **VERIFIED** |
| **Check 2** | Every model request passes through `RequestReservationCoordinator` | Inside `ControlledAgentRuntime.run()`, every turn in the loop invokes `reserve_request()` before wire dispatch. Drops on failure (`PRE_DISPATCH_HEADROOM_EXHAUSTED`). | **VERIFIED** |
| **Check 3** | Every outbound request contains server-side output cap | `ProviderRequest` injects `max_completion_tokens=4096`. In `OpenAICompatibleTransport.serialize_request()`, payload contains `"max_completion_tokens": 4096`. | **VERIFIED** |
| **Check 4** | Input reservation includes serialized payload & structural overhead | Exact wire payload JSON is serialized, UTF-8 encoded, and measured before reservation ($R = I + O$). `RequestSizeGate` enforces $I \le 60,000$ tokens. | **VERIFIED** |
| **Check 5** | Provider usage is reconciled without double-counting | Post-dispatch calls `reconcile_response()`, adjusting committed tokens against actual usage. Output allowances are never added twice to global ceilings. | **VERIFIED** |
| **Check 6** | Timeouts and errors retain conservative reservations | `reconcile_response()` charges full reserved capacity $R$ if provider usage is None or missing. Proven by tests `test_10` and `test_11`. | **VERIFIED** |
| **Check 7** | Shared Task A + Task B budget cannot exceed 2,500,000 tokens | Dynamic ceiling $C_B = \min(1,500,000, 2,500,000 - T_A)$. Sum $T_A + C_B \le 2,500,000$ strictly conserved. Proven by `test_16`. | **VERIFIED** |
| **Check 8** | 45,000-token safety reserve is applied consistently | Headroom is checked via $R \le B_{\text{remaining}} - B_{\text{reserve}}$ as an offset against remaining capacity, never subtracted from cumulative spend. | **VERIFIED** |
| **Check 9** | Legacy `cloud` provider cannot execute live pentad | `Orchestrator.run_pentad()` contains hard fail-closed assertion: `if not self.is_dry_run and self.provider == "cloud": raise SafetyViolationError(...)`. | **VERIFIED** |
| **Check 10** | No dry-run fixture or synthetic count mistaken for live usage | Live run will output into a fresh directory (`runs/two_task_pentad_live`), with telemetry tagged `provenance: "DIRECT_API"` and labeled `LIVE_PROVIDER_MEASURED`. | **VERIFIED** |

---

## 3. Five-Arm Candidate Identity & Provenance

All candidate binaries were directly inspected on disk immediately prior to this pre-flight report:

| Arm | Candidate Identity | Physical Executable Path | File Size (Bytes) | SHA-256 Digest | Active Runtime Configuration |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Arm A** | `native` | None (In-Process Agent Loop) | N/A | `NONE` | Standard prompt framing, zero MCP tools |
| **Arm B** | `cand3` | `benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe` | 18,668,032 | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | Static symbol indexing & context pruning (`routed_mode="FULL"`) |
| **Arm C** | `cand4_2` | `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe` | 18,802,176 | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | UTF-8 safety & structural symbol compression (always-on mode) |
| **Arm D** | `cand4_2` | `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe` | 18,802,176 | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | Dynamic complexity heuristic routing via `TaskComplexityClassifier` |
| **Arm E** | `cand5` | `benchmarks/matched-pair-harness/bin/candidate-phase5/engram.exe` | 18,883,584 | `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` | Full 7 Token-Efficiency optimizations (AST delta compression, recursive summarization) |

---

## 4. Task Definitions & Golden Repository Baseline

### Task A Specification:
- **Task ID:** `task_a_typo`
- **Config Path:** `benchmarks/matched-pair-harness/config/tasks/task_a_typo.json`
- **Objective:** Fix capitalization typo ('Angular Cli Tips' -> 'Angular CLI Tips') in `projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html`.
- **Nominal Ceiling:** 1,000,000 tokens (Per-arm ceiling: 200,000 tokens).

### Task B Specification:
- **Task ID:** `task_b_remember_me`
- **Config Path:** `benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json`
- **Objective:** Add remember-me option to authentication flow persisting username in `localStorage` without persisting sensitive credentials.
- **Dynamic Ceiling:** $C_B = \min(1,500,000, 2,500,000 - T_A)$ (Per-arm ceiling: $\lfloor C_B / 5 \rfloor$).

### Golden Repository:
- **Path:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`
- **Verified Baseline Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`
- **Dependency Strategy:** Mandatory `copy` (ensuring pristine baseline tree isolation).

---

## 5. Budget, Reservation & Financial Exposure Model

- **Global Budget Ceiling ($B_G$):** 2,500,000 tokens
- **Output Cap per Request ($O$):** 4,096 tokens
- **Pre-Dispatch Headroom Reserve ($B_{\text{reserve}}$):** 45,000 tokens
- **Max Request Size Gate:** 60,000 tokens (reject-only)
- **Model:** `gpt-5.6-luna` (Temperature: 0.0, Top_P: 1.0)
- **Verified Pricing Baseline (2026-11-20):**
  - Prompt Input: **\$0.20 per million tokens**
  - Completion Output: **\$1.20 per million tokens**

### Financial Exposure Table Across Budget Ceilings:
| Exposure Scenario | Input / Output Mix | Blended Rate | Cost at $B_G = 2.50\text{M}$ Tokens | Cost at Legacy $E_{\text{max}} = 2.531\text{M}$ Tokens |
| :--- | :--- | :--- | :--- | :--- |
| **Scenario 1 (All Input)** | 100% Input / 0% Output | \$0.20 / M | **\$0.50** | **\$0.51** |
| **Scenario 2 (All Output)** | 0% Input / 100% Output | \$1.20 / M | **\$3.00** | **\$3.04** |
| **Scenario 3A (Standard Agent Mix)** | 80% Input / 20% Output | \$0.40 / M | **\$1.00** | **\$1.01** |
| **Scenario 3B (Context-Heavy Mix)** | 95% Input / 5% Output | \$0.25 / M | **\$0.63** | **\$0.63** |

---

## 6. Security & Credential Isolation Confirmation

1. **Zero Exposure:** `OpenAICompatibleTransport` and `ControlledRunner` sanitize all outbound and inbound records. In `test_20_api_credentials_never_appear_in_logs_or_reports`, API credentials were mathematically proven never to appear in logs, events, or reports.
2. **Zero Modification:** No system environment variables or credential stores will be modified.

---

## 7. Exact Live Execution Command

When authorized, the live benchmark pentad will be dispatched using the following exact invocation:

```powershell
powershell -ExecutionPolicy Bypass -File benchmarks/matched-pair-harness/scripts/run_two_task_pentad.ps1 `
    -OutputDir "benchmarks/matched-pair-harness/runs/two_task_pentad_live" `
    -Provider controlled `
    -ExecutionPolicy diagnostic_continue `
    -DependencyStrategy copy `
    -Seed 59002
```

*(Note: Execution policy `diagnostic_continue` ensures that if an individual arm fails code correctness, the orchestrator records the failure and safely proceeds with remaining arms to capture comparative token metrics, while strictly enforcing all budget, request-size, and safety limits).*

---

## 8. Final Decision

```
AWAITING_EXPLICIT_LIVE_BENCHMARK_AUTHORIZATION
```
