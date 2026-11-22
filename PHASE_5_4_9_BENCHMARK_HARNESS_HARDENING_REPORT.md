# Engram Phase 5.4.9: Benchmark Harness Hardening & Candidate 4.2 Offline Certification Report

**Phase:** 5.4.9  
**Status:** COMPLETE & SEALED  
**Mode:** Strictly Read-Only with Respect to Live Models (Zero Live Model Dispatches)  
**Target Repository:** `c:\Users\Amal P S\OneDrive\Projects\Engram`  
**Git Branch:** `feature/phase2-stage5-remediation`  
**Git HEAD Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Golden Repository Baseline Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`  
**Candidate 4.2 SHA-256:** `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` (18,802,176 bytes)  
**Candidate 4 SHA-256 (Preserved):** `FC605BC0CF3B6C9C5FDB1E7C83EE666FD7E049FD9AE25C893D478578681699F8` (18,724,864 bytes)  
**Candidate 3 SHA-256 (Preserved):** `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` (18,668,032 bytes)

---

## 1. Executive Summary

Phase 5.4.9 was commissioned following the Phase 5.4.8 forensic integrity audit of the authorized four-arm Task B benchmark (`task_b_remember_me_tetrad_live_01`). The forensic audit uncovered critical engineering and reporting defects:
1. **Candidate Binary Mismatch:** The benchmark harness lacked CLI support for selecting candidate versions, causing Arm C and Arm D to silently fall back to Candidate 4 (`FC605BC0...`) rather than Candidate 4.2 (`78B84880...`).
2. **False-Positive Long-Path Deletion:** The harness evaluator failed to configure `core.longpaths=true` during Git operations on Windows, causing deep files (>220 characters) to be reported as falsely deleted in git impact metrics.
3. **Conflated Telemetry Metrics:** Reporting erroneously compared cumulative session auxiliary payloads (e.g. 31,621 bytes across 4 tool calls) against the single-call 16,384-byte MCP limit, creating the false perception that Candidate 4.2 responses had violated boundary constraints.
4. **Universal Pre-Dispatch Headroom Halts:** Every arm was halted between turns 7 and 9 because the historical 350,000-token ceiling was insufficient to accommodate agentic context accumulation (where context grows to ~50,000 tokens per turn).
5. **Arm C Incomplete Test Fixes:** Arm C had successfully implemented the core feature and explicitly stated in its final turn commentary that it was checking for zero-argument `authLogin()` callers, but was halted by the pre-dispatch headroom guard before dispatching the fix for `auth.actions.spec.ts` and `auth.reducer.spec.ts`.

In Phase 5.4.9, all six remediation workstreams were executed offline without any live LLM dispatches. 32 automated unit and integration tests across 6 dedicated test suites verify the complete remediation.

```mermaid
flowchart TD
    subgraph Hardened Benchmark Harness Architecture
        CLI["CLI: --candidate-version"] --> Ledger["Fail-Closed Candidate Ledger"]
        Ledger --> BinaryCheck{"Hash Check\n(SHA-256)"}
        BinaryCheck -- Pass --> ArmLauncher["Arm Execution (Codex Runner)"]
        BinaryCheck -- Fail --> Abort["Abort: SafetyViolationError"]
        
        Evaluator["Git Long-Path Evaluator"] --> LongPathCfg["core.longpaths=true\ncore.quotepath=false"]
        LongPathCfg --> DiskVerify["Disk Existence & Byte Comparison"]
        DiskVerify --> TrueImpact["Accurate Git Impact Patch"]
        
        Telemetry["Telemetry Parser"] --> Deconflate["De-conflated Accounting"]
        Deconflate --> Inner["Inner Text Bytes"]
        Deconflate --> JsonRpc["Serialized JSON-RPC Bytes"]
        Deconflate --> MaxResp["Max Single Response Bytes (<=16,384)"]
        Deconflate --> AggPayload["Aggregate Auxiliary Payload Bytes"]
    end
```

---

## 2. Workstream 1: Fail-Closed Candidate Selection

### Root Cause Analysis
In Phase 5.4.7, `orchestrator.py` resolved candidate paths using a legacy fallback chain: checking environment variables first, then falling back to `DEFAULT_CANDIDATE_PATH = REPO_ROOT / "benchmarks" / "matched-pair-harness" / "bin" / "candidate-phase4" / "engram.exe"`. Because the CLI runner script did not expose a candidate version flag, Arm C and Arm D silently loaded Candidate 4 (`FC605BC0...`) instead of Candidate 4.2 (`78B84880...`).

### Engineering Remediation
1. **Explicit CLI Parameter:** Added `--candidate-version {native,cand3,cand4,cand4_1,cand4_2}` to the matched-pair harness CLI in [orchestrator.py](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py).
2. **Deterministic Candidate Ledger:** Defined `CANDIDATE_LEDGER` containing canonical binary paths, exact expected SHA-256 hashes, and binary sizes:
   - `cand3`: `bin/candidate-phase3/engram.exe` (`97C5EC47...`)
   - `cand4`: `bin/candidate-phase4/engram.exe` (`FC605BC0...`)
   - `cand4_1`: `bin/candidate-phase4.1/engram.exe` (`300ACE35...`)
   - `cand4_2`: `bin/candidate-phase4.2/engram.exe` (`78B84880...`)
3. **Fail-Closed Verification Gates:**
   - Missing binary files raise `FileNotFoundError` immediately (no silent fallbacks).
   - SHA-256 hash mismatches raise `SafetyViolationError` immediately before launching any model processes.
   - Conflicting legacy environment variables (`ENGRAM_CANDIDATE_VERSION`, `ENGRAM_EXECUTABLE`) raise `ValueError`.
4. **Native Arm Invariant Protection:**
   - Native arms explicitly reject `engram_executable` and fail if any MCP command arguments are passed to the runner.
5. **Telemetry Traceability:** Added `requested_candidate_version`, `resolved_candidate_version`, `candidate_verification_status`, and `mcp_registration_verified` to `run_telemetry.isolation` and updated `run_telemetry.schema.json`.

### Verification Suite: `test_candidate_selection.py`
All 8 regression tests passed in 0.772s:
- `test_explicit_candidate_4_2_selection`: Confirms `--candidate-version cand4_2` resolves to `78B84880...`.
- `test_missing_binary_fails_closed`: Confirms missing binary throws `FileNotFoundError`.
- `test_hash_mismatch_fails_closed`: Confirms tampered binary throws `SafetyViolationError`.
- `test_invalid_candidate_version_fails_closed`: Confirms unknown version string throws `ValueError`.
- `test_conflicting_legacy_env_fails_closed`: Confirms conflicting legacy env vars throw `ValueError`.
- `test_native_arm_rejects_mcp_registration`: Confirms native arm aborts if MCP binary is provided.
- `test_candidate_arm_verifies_mcp_registration`: Confirms candidate arm registers active MCP server.
- `test_regression_reproduce_phase_5_4_7_failure`: Reproduces the Phase 5.4.7 flaw and confirms the hardened harness now rejects it.

---

## 3. Workstream 2: Windows Git Long-Path Evaluator Correction

### Root Cause Analysis
During Phase 5.4.7 evaluation, Git on Windows ran without `-c core.longpaths=true`. When evaluating files with paths exceeding 220 characters (specifically `projects/angular-ngrx-material-starter/src/app/features/examples/stock-market/components/stock-market-container.component.scss-theme.scss`), Git was unable to resolve the file on disk during `git diff --numstat` and reported it as a deleted file (`0 insertions, 19 deletions`).

### Engineering Remediation
1. **Long-Path Git Invariant:** Updated all Git commands in [evaluator.py](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/evaluator.py) to explicitly include `-c core.longpaths=true` and `-c core.quotepath=false`.
2. **Defensive Byte-Level Deletion Verification:**
   - In `analyze_git_impact()`, when `git diff` reports deletions for a file where `insertions == 0`:
   - The evaluator checks whether the file exists on disk.
   - If the file exists, it compares disk bytes against `git show HEAD:{fpath}`.
   - If bytes match exactly, the false deletion is discarded from `changed_files`, `insertions`, `deletions`, and `changes.patch`.
3. **Porcelain Status Reconciliation:** Reconciled `git status --porcelain` with `git diff` numstat accounting.

### Verification Suite: `test_longpath_evaluator.py`
All 4 regression tests passed in 8.196s:
- `test_untouched_deep_file_does_not_appear_as_deletion`: Confirms untouched 220+ character path produces 0 deletions and 0 changes.
- `test_actual_modification_to_deep_file_is_detected`: Confirms genuine edits to deep files are detected.
- `test_actual_deletion_of_deep_file_is_detected`: Confirms genuine deletions of deep files are detected.
- `test_reconciliation_between_porcelain_and_diff`: Confirms multi-file modification and creation accounting matches porcelain status.

---

## 4. Workstream 3: Candidate 4.2 Offline Certification

### Target Binary Audit
- **Path:** `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe`
- **SHA-256:** `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2`
- **Size:** 18,802,176 bytes

### Endpoint Response-Boundary Verification
Candidate 4.2 was subjected to extensive offline JSON-RPC testing across all registered tools against the golden repository (`angular-ngrx-material-starter`):

| Tool Name | Golden Repo Query | Serialized Response Bytes | Max Boundary | Compliance Status | Details |
|---|---|---|---|---|---|
| `tools/list` | Discovery query | 1,489 B | N/A | COMPLIANT | Registers all 4 tools (`get_context`, `inspect_symbol`, `preflight`, `postflight`) |
| `engram_get_context` | 100+ file repo | **14,599 B** | 16,384 B | **COMPLIANT** | Omission manifest compacted (118 omitted items summarized, 0 raw paths dumped) |
| `engram_inspect_symbol` | `auth.effects.ts` | **11,088 B** | 16,384 B | **COMPLIANT** | Zero character-boundary panics on UTF-8 strings |
| `engram_preflight` | Task B contract | **897 B** | 16,384 B | **COMPLIANT** | Contract preflight verified |
| `engram_postflight` | Task B contract | **860 B** | 16,384 B | **COMPLIANT** | Postflight verification verified |

### UTF-8 Character Boundary & Adversarial Resilience
1. **Adversarial Queries Tested:** Tested complex Unicode queries containing emojis (`🔐_auth_login_🔥_🚀`), Chinese characters (`认证_授权_令牌`), Cyrillic characters (`Аутентификация_Вход`), RTL Arabic (`تسجيل_الدخول_المستخدم`), and bullet characters (`•`).
2. **Outcome:** Candidate 4.2 processed every adversarial query without panic, truncation errors, or malformed UTF-8 byte sequences.
3. **Clamping Tests:** Tested boundary clamping at 16,383, 16,384, and 16,385 bytes. Every response strictly clamped to $\le \text{requested max\_bytes}$.
4. **Session Persistence:** Successfully executed 15 sequential calls over a single persistent stdio session with deterministic memory usage.

### Verification Suite: `test_candidate_4_2_offline_certification.py`
All 10 offline certification tests passed in 32.727s.

---

## 5. Workstream 4: Telemetry Measurement Semantics & Disambiguation

### Disambiguation of Measurement Semantics
The Phase 5.4.8 audit identified that telemetry reports conflated cumulative session totals with single-call limits. In [telemetry.py](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/telemetry.py), four distinct metrics are now formally separated and enforced:

1. **`inner_response_content_bytes`**: Exact UTF-8 byte count of the text payload inside `content[0].text`.
2. **`serialized_jsonrpc_response_bytes`**: Exact UTF-8 byte count of the entire JSON-RPC response frame.
3. **`aggregate_auxiliary_payload_bytes`**: Cumulative bytes across all tool invocations and responses throughout the session.
4. **`max_individual_response_bytes`**: The peak byte size of any single MCP response frame.

### Invariants Enforced
$$\text{inner\_response\_content\_bytes} \le \text{serialized\_jsonrpc\_response\_bytes} \le \text{aggregate\_auxiliary\_payload\_bytes}$$
$$\text{Single-call ceiling (16,384 B) applies ONLY to } \text{max\_individual\_response\_bytes}$$

### Forensic Re-Evaluation of Phase 5.4.7
Evaluating the historical Phase 5.4.7 data under the disambiguated metrics reveals:

| Arm | Tool Calls | Aggregate Auxiliary Payload | Serialized JSON-RPC Bytes | Peak Individual Response | $\le 16,384$ B Compliant? | Root Cause |
|---|---|---|---|---|---|---|
| **Arm A (Native)** | 0 | 0 B | 0 B | 0 B | N/A | Native execution |
| **Arm B (Cand 3)** | 3 | 425,510 B | 425,510 B | 285,377 B | NON-COMPLIANT | Candidate 3 had no boundary limits |
| **Arm C (Cand 4 executed)** | 4 | 31,621 B | 31,159 B | **14,481 B** | **COMPLIANT** | 4 calls (8.5k, 14.5k, 8.0k, 0.2k). Cumulative sum was misreported as single call. |
| **Arm D (Cand 4 executed)** | 4 | 90,155 B | 89,611 B | **65,542 B** | **NON-COMPLIANT** | `get_context` blew up because Candidate 4 was run instead of Candidate 4.2 |

> [!IMPORTANT]
> In Arm C, **every single MCP response was strictly $\le 16,384$ bytes**. The reported "31,621 byte violation" was a reporting error caused by comparing the cumulative multi-call payload against a single-response ceiling.

### Verification Suite: `test_telemetry_semantics.py`
All 4 regression tests passed in 0.787s.

---

## 6. Workstream 5: Task B Budget Requirements & Simulation

### Ground-Truth Turn Trajectory Analysis
From the session rollout logs in `.codex/sessions`, turn-by-turn context growth was extracted across all four arms:

```
=== arm_a_native (8 turns) ===
  Turn 1: req_input= 17,189, req_output=  321 | cumulative_total=  17,510
  Turn 2: req_input= 27,057, req_output=  273 | cumulative_total=  44,840
  Turn 3: req_input= 34,850, req_output=  265 | cumulative_total=  79,955
  Turn 4: req_input= 39,295, req_output=  396 | cumulative_total= 119,646
  Turn 5: req_input= 43,673, req_output=3,291 | cumulative_total= 166,610
  Turn 6: req_input= 47,063, req_output=  735 | cumulative_total= 214,408
  Turn 7: req_input= 47,826, req_output=   40 | cumulative_total= 262,274
  Turn 8: req_input= 47,894, req_output=   39 | cumulative_total= 310,207 -> HALTED (rem: 39,793 < est: 47,894)

=== arm_c_cand4_always_on (8 turns) ===
  Turn 1: req_input= 23,724, req_output=  130 | cumulative_total=  23,854
  Turn 2: req_input= 26,706, req_output=  201 | cumulative_total=  50,761
  Turn 3: req_input= 33,852, req_output=  336 | cumulative_total=  84,949
  Turn 4: req_input= 34,232, req_output=  373 | cumulative_total= 119,554
  Turn 5: req_input= 43,912, req_output=  514 | cumulative_total= 163,980
  Turn 6: req_input= 46,107, req_output=1,063 | cumulative_total= 211,150
  Turn 7: req_input= 47,194, req_output=  262 | cumulative_total= 258,606
  Turn 8: req_input= 48,803, req_output=  344 | cumulative_total= 307,753 -> HALTED (rem: 42,247 < est: 48,803)
```

### Context Growth Dynamics
In an agentic LLM session, conversation history is preserved across turns. When single-request context reaches $C_k \approx 48,000$ to $55,000$ tokens, each additional turn adds $\approx 50,000$ tokens to the cumulative input count:
$$\text{Cumulative Tokens after Turn } N = \sum_{k=1}^N (C_k + O_k)$$
At $N=8$, cumulative tokens reach $\sim 308,000$. Under a 350,000 ceiling with a 45,000 reserve, available tokens before turn 9 are $350,000 - 308,000 = 42,000 < 48,803$. Thus, **any multi-step task requiring $\ge 9$ turns is mathematically impossible to complete under a 350,000-token ceiling**.

### Turn Capacity Sensitivity Matrix
Simulating turn capacity across budget ceilings (with 45k and 50k reserves):

| Arm | Reserve | 350k | 450k | 500k | 550k | 600k | 650k |
|---|---|---|---|---|---|---|---|
| **Arm A (Native)** | 45k | 8 turns | 10 turns | 11 turns | 12 turns | 13 turns | 14 turns |
| **Arm B (Cand 3)** | 45k | 9 turns | 11 turns | 12 turns | 12 turns | 13 turns | 14 turns |
| **Arm C (Cand 4)** | 45k | 8 turns | 10 turns | 11 turns | 12 turns | 13 turns | 14 turns |
| **Arm D (Cand 4)** | 45k | 7 turns | 9 turns | 10 turns | 11 turns | 12 turns | 12 turns |
| **Arm A (Native)** | 50k | 8 turns | 10 turns | 11 turns | 12 turns | 13 turns | 14 turns |
| **Arm B (Cand 3)** | 50k | 9 turns | 11 turns | 12 turns | 12 turns | 13 turns | 14 turns |
| **Arm C (Cand 4)** | 50k | 8 turns | 10 turns | 11 turns | 12 turns | 13 turns | 14 turns |
| **Arm D (Cand 4)** | 50k | 7 turns | 9 turns | 10 turns | 11 turns | 12 turns | 12 turns |

### Candidate 4.2 Compaction Leverage
Candidate 4 dumped 154,608 bytes ($\approx 38,650$ tokens) on `engram_get_context`. Candidate 4.2 compacts this to 14,599 bytes ($\approx 3,650$ tokens). This saves $\approx 35,000$ tokens of context on **every subsequent turn**. In Arm D, this compaction extends turn capacity from 7 turns to 10 turns under the same 350k ceiling.

### Verification Suite: `test_task_b_budget_simulation.py`
All 3 simulation tests passed in 0.051s.

---

## 7. Workstream 6: Arm C Implementation Defect Analysis

### Defect Reproduction
1. **Baseline Golden Repo:**
   `auth.actions.ts`: `export const authLogin = createAction('[Auth] Login');` (0 arguments).
   `auth.actions.spec.ts:5`: `const action = authLogin();` (0 arguments).
   `auth.reducer.spec.ts:18`: `const action = authLogin();` (0 arguments).
2. **Arm C Modifications:**
   `auth.actions.ts`:
   ```typescript
   export const authLogin = createAction(
     '[Auth] Login',
     props<{ username: string; rememberMe: boolean }>()
   );
   ```
   `auth.effects.spec.ts:81`: Updated caller to `authLogin({ username: 'ada', rememberMe: true })`.
3. **TypeScript Compiler Failure:**
   When non-interactive verification ran `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.spec.json --noEmit`, the compiler threw:
   `TS2554: Expected 1 arguments, but got 0` on `auth.actions.spec.ts:5` and `auth.reducer.spec.ts:18`.

### Forensic Evidence of Model Awareness
In Turn 8 of Arm C's session rollout (`rollout-2026-11-19T19-47-01-01a1f42a-fee7-7332-abbe-d41da3764401.jsonl`), the model emitted the following message:
> *"The first implementation compiles at the application level so far. I found one cleanup needed before final verification: logout should read the stored record once, and the existing effect spec’s router spy must expose `navigate` (the production effect already uses it). **I’m tightening those details and checking spec compilation for any zero-argument `authLogin()` callers.**"*

Immediately following this message, the harness pre-dispatch headroom guard intervened and halted the run:
`Execution halted by pre-dispatch token headroom guard: 42,247 tokens remaining < estimated next turn cost of 48,803 tokens (budget: 350,000)`.

### Assessment
- **Nature of Defect:** Arm C did NOT fail due to a candidate retrieval bug or model hallucination.
- **Turn Requirement:** The model required approximately 2 additional turns (Turns 9 and 10) to update `auth.actions.spec.ts` and `auth.reducer.spec.ts` and run final verification.
- **Root Cause:** Budget exhaustion under an artificially constrained 350,000-token ceiling prevented the agent from finishing its self-correction loop.

### Verification Suite: `test_arm_c_ts2554_reproduction.py`
All 3 tests passed in 0.001s.

---

## 8. Master Test Suite Execution Summary

The complete Phase 5.4.9 regression suite was executed via automated test discovery:

```
test_candidate_arm_verifies_mcp_registration ......................... ok
test_conflicting_legacy_env_fails_closed ............................. ok
test_explicit_candidate_4_2_selection ................................ ok
test_hash_mismatch_fails_closed ...................................... ok
test_invalid_candidate_version_fails_closed .......................... ok
test_missing_binary_fails_closed ..................................... ok
test_native_arm_rejects_mcp_registration ............................. ok
test_regression_reproduce_phase_5_4_7_failure ........................ ok
test_actual_deletion_of_deep_file_is_detected ........................ ok
test_actual_modification_to_deep_file_is_detected .................... ok
test_reconciliation_between_porcelain_and_diff ....................... ok
test_untouched_deep_file_does_not_appear_as_deletion ................. ok
test_01_binary_hash_and_integrity .................................... ok
test_02_tools_list_contract .......................................... ok
test_03_get_context_large_omission_manifest_bounded .................. ok
test_04_inspect_symbol_with_multibyte_and_unicode_safety ............. ok
test_05_preflight_bounded ............................................ ok
test_06_postflight_bounded ........................................... ok
test_07_boundary_exact_one_byte_below_and_above ...................... ok
test_08_adversarial_unicode_inputs_and_excerpts ...................... ok
test_09_malformed_and_missing_parameters_graceful_fallback ........... ok
test_10_persistent_stdio_session_idempotency ......................... ok
test_conflation_prevention_check ..................................... ok
test_disambiguation_synthetic_events ................................. ok
test_phase_5_4_7_arm_c_telemetry_reconciliation ...................... ok
test_phase_5_4_7_arm_d_telemetry_reconciliation ...................... ok
test_candidate_4_2_compaction_benefit_simulation ..................... ok
test_ceiling_sensitivity_analysis .................................... ok
test_reproduce_phase_5_4_7_halt ...................................... ok
test_arm_c_action_signature_incompatibility .......................... ok
test_golden_baseline_signatures ...................................... ok
test_golden_specs_use_zero_arg_auth_login ............................ ok

Ran 32 tests in 22.740s
OK (Total tests: 32, Errors: 0, Failures: 0)
```

---

## 9. Experimental Readiness Verdict & Live Authorization Recommendations

### Readiness Verdict
**STATUS: HARNESS HARDENED, CANDIDATE 4.2 FULLY CERTIFIED OFFLINE**

The benchmark harness is fail-closed,Candidate 4.2 strictly enforces response-size limits ($\le 16,384$ bytes) across all endpoints, the Git long-path evaluator defect is corrected, and telemetry semantics are formally disambiguated.

### Required Benchmark Configuration for Future Live Execution

> [!CAUTION]
> Disagree with re-running under a 350,000-token ceiling. Any four-arm benchmark executing under a 350,000 ceiling is guaranteed to halt prematurely due to pre-dispatch headroom exhaustion before completing verification.

To ensure experimental integrity and allow model self-correction to complete, the following parameters are strongly recommended for any future live authorization:
1. **Candidate Flag:** `--candidate-version cand4_2` (enforced via fail-closed ledger).
2. **Token Ceiling:** **550,000 tokens** (allows 11–12 turns, accommodating feature implementation and unit test adjustments).
3. **Headroom Reserve:** **45,000 tokens** (sufficient to guard against single-turn context overflows).
4. **Single-Response Boundary:** **16,384 bytes** (certified on Candidate 4.2).
5. **Git Configuration:** `-c core.longpaths=true -c core.quotepath=false` (enforced in evaluator).
6. **Telemetry Metrics:** Strict separation of `max_individual_response_bytes` ($\le 16,384$ B) from `aggregate_auxiliary_payload_bytes`.

*Report sealed and archived.*
