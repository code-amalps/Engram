# Engram Phase 5.4.4: Task B Preflight Revalidation with Candidate 4.2

**Document Identifier:** `ENGRAM-PHASE-5.4.4-PREFLIGHT-REVALIDATION`  
**Date:** 2026-11-19  
**Execution Stage:** Phase 5.4.4 Offline Preflight and Configuration-Integrity Gate  
**Branch:** `feature/phase2-stage5-remediation`  
**Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Status:** **PASSED — ALL GATES VERIFIED**  
**Final Recommendation:** **READY FOR EXPLICIT LIVE TASK B AUTHORIZATION**

---

## Executive Summary

Phase 5.4.4 has revalidated the complete four-arm Task B benchmark configuration (`task_b_remember_me`) under Candidate 4.2. This offline preflight audit establishes that the upcoming live experiment is mathematically sound, reproducible, and immune to the failure modes encountered in earlier phases:

1. **Cryptographic Binary Ledger:** Candidate 3 (`97C5EC47...`), Candidate 4 (`FC605BC0...`), Candidate 4.1 (`300ACE35...`), and Candidate 4.2 (`78B84880...`) were verified. Both Candidate 4 arms (`arm_c_cand4_always_on` and `arm_d_cand4_adaptive`) strictly resolve to Candidate 4.2.
2. **Deterministic MCP Response Boundaries:** All MCP tool endpoints enforce a hard serialized ceiling $\le 16,384$ bytes. Defect 12.1 (multi-byte UTF-8 character boundary panic) is eradicated: Candidate 4.2 survived an end-to-end 7-step persistent stdio session without panicking or leaking resources.
3. **Task B Operational Budget:** Cumulative ceiling is verified at 350,000 tokens with a 45,000-token pre-dispatch reserve. All 7 guard boundary conditions passed mathematical verification, preventing premature halts while protecting against ceiling overruns.
4. **Missing-Dependency Semantic Retrieval:** 5 out of 5 Task B cross-file discovery scenarios succeeded on the golden repository (`angular-ngrx-material-starter` at commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`).
5. **Zero Misleading Claims:** Offline 4-arm dry runs confirmed sandbox isolation and verified that partial acceptance (0.20 score) properly suppresses token savings claims (`claim_status: N/A`).
6. **Zero Regression:** 96 Rust tests and 331 Python harness/benchmark tests passed with 0 failures. The test count reconciliation between Phase 5.4.3 (62 tests), Phase 5.4.2 (156 tests), and the full harness repository (331 tests) is fully accounted for.

---

## 1. Baseline Integrity and Cryptographic Ledger

The repository and binary artifacts were inventoried prior to execution. The working tree remains uncommitted and strictly preserved:

### 1.1 Git Working Tree Status
- **Current Branch:** `feature/phase2-stage5-remediation`
- **HEAD Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`
- **Golden Repository Path:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`
- **Golden Repository Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` (clean working tree)

### 1.2 Binary and Harness Cryptographic Hashes

| Component / Artifact | File Path | File Size (Bytes) | SHA-256 Checksum | Sealed Status |
| :--- | :--- | :--- | :--- | :--- |
| **Candidate 3 (v1.5.4)** | `benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe` | 18,668,032 | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | Immutable |
| **Candidate 4 (Baseline)** | `benchmarks/matched-pair-harness/bin/candidate-phase4/engram.exe` | 18,724,864 | `FC605BC0CF3B6C9C5FDB1E7C83EE666FD7E049FD9AE25C893D478578681699F8` | Historical |
| **Candidate 4.1 (Remediated)** | `benchmarks/matched-pair-harness/bin/candidate-phase4.1/engram.exe` | 18,801,152 | `300ACE35A31E8E4AF6DD531E8B83B0A5BA9F03E70B27F5E7823E3650CE494015` | Historical (Defect 12.1) |
| **Candidate 4.2 (Sealed Target)** | `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe` | 18,802,176 | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | Active Candidate |
| **Codex Runner Module** | `benchmarks/matched-pair-harness/src/runner.py` | 55,274 | `A813F5C73994E97663C42CD056703FD4BA7AB95EF15B1ADE495E1818CFD0C370` | Verified |
| **Orchestrator Module** | `benchmarks/matched-pair-harness/src/orchestrator.py` | 38,719 | `95EBEC9DEA685CCFFCA672C8D4EB5228DB70BC69DEB40567673AF1C6150A8865` | Verified |
| **Task Complexity Classifier** | `benchmarks/matched-pair-harness/src/classifier.py` | 13,873 | `E3C9B71C0F42C7D1E9484832596A942F2D1DCB5D58602E9AEF1C5F07DA7B94B1` | Verified |

---

## 2. Four-Arm Executable Resolution Proof

The tetrad harness maps benchmark arms to executable paths based on configuration and the environment variable `ENGRAM_USE_CANDIDATE_4_2=1`:

```
                               ┌────────────────────────────────────────────────────────┐
                               │             Task B Tetrad Arm Mapping                  │
                               └──────────────────────────┬─────────────────────────────┘
                                                          │
          ┌───────────────────────┬───────────────────────┼─────────────────────────┐
          ▼                       ▼                       ▼                         ▼
   [Arm A: Native]         [Arm B: Cand 3]        [Arm C: Cand 4.2]         [Arm D: Cand 4.2]
   Native Codex CLI        Candidate 3 Binary      Candidate 4.2 Binary      Candidate 4.2 Binary
   No Engram MCP           Always-On MCP Suite     Always-On MCP Suite       Adaptive FULL Routed
   (unassisted baseline)   (SHA-256: 97C5EC47...)  (SHA-256: 78B84880...)    (SHA-256: 78B84880...)
```

### 2.1 Dynamic Resolution Verification Output
```python
# Resolved Arm Configurations under ENGRAM_USE_CANDIDATE_4_2=1
arm_a_native:
  Executable: None (Native unassisted execution)
  MCP Active: False
  Turn-0 Injected: False

arm_b_cand3_always_on:
  Executable: benchmarks\matched-pair-harness\bin\candidate-phase3\engram.exe
  Hash: 97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777
  MCP Active: True
  Enabled Tools: ["engram_get_context", "engram_inspect_symbol", "engram_preflight", "engram_postflight"]

arm_c_cand4_always_on:
  Executable: benchmarks\matched-pair-harness\bin\candidate-phase4.2\engram.exe
  Hash: 78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2
  MCP Active: True
  Enabled Tools: ["engram_get_context", "engram_inspect_symbol", "engram_preflight", "engram_postflight"]

arm_d_cand4_adaptive:
  Executable: benchmarks\matched-pair-harness\bin\candidate-phase4.2\engram.exe
  Hash: 78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2
  Task B Natural Complexity: COMPLEX
  Routed Mode: FULL
  MCP Active: True
  Enabled Tools: ["engram_get_context", "engram_inspect_symbol", "engram_preflight", "engram_postflight"]
```

Both Arm C and Arm D resolve strictly to Candidate 4.2. On Task B, Arm D classifies naturally as `COMPLEX` and activates the full 4-tool MCP suite, matching Arm C's capabilities while retaining adaptive policy tracking.

---

## 3. Task B Budget Configuration & Mathematical Verification

### 3.1 Budget Justification & Amendment Alignment
Under **Amendment 03** and **Amendment 04**, Task B is a multi-file state-persistence task with an empirical unassisted baseline consumption of ~260,000–310,000 tokens across 12–18 interactive turns. 

- **Cumulative Token Ceiling:** `350,000 tokens` per arm.
- **Conservative Headroom Reserve:** `45,000 tokens`.
- **Pre-Dispatch Guard Formula:**
  $$\text{remaining} = \text{ceiling} - \text{effective\_tokens}$$
  $$\text{estimated} = \max(\text{last\_turn\_request\_tokens}, \text{conservative\_reserve})$$
  $$\text{has\_headroom} = (\text{remaining} \ge \text{estimated})$$

### 3.2 Guard-Boundary Deterministic Verification (All 7 Test Cases)

| Test Case | Scenario Description | Effective Cumulative Tokens | Remaining Tokens | Estimated Next Turn | Guard Decision | Status |
| :---: | :--- | :---: | :---: | :---: | :---: | :---: |
| **Test 1** | Ample headroom | 100,000 | 250,000 | 45,000 | `DISPATCH_ALLOWED` (True) | **PASS** |
| **Test 2** | Exact reserve equality boundary ($\text{rem} == \text{est}$) | 305,000 | 45,000 | 45,000 | `DISPATCH_ALLOWED` (True) | **PASS** |
| **Test 3** | One token below reserve ($\text{rem} < \text{est}$) | 305,001 | 44,999 | 45,000 | `PREVENTIVE_HALT` (False) | **PASS** |
| **Test 4** | One token above reserve ($\text{rem} > \text{est}$) | 304,999 | 45,001 | 45,000 | `DISPATCH_ALLOWED` (True) | **PASS** |
| **Test 5** | Dynamic last turn exceeds conservative reserve | 280,000 | 70,000 | 75,000 | `PREVENTIVE_HALT` (False) | **PASS** |
| **Test 6** | Completed run strictly under ceiling | 320,000 | 30,000 | N/A | `breached = False` | **PASS** |
| **Test 7** | Preventive stop never recorded as ceiling breach | 310,000 | 40,000 | 45,000 | `breached = False` (Halted) | **PASS** |

---

## 4. Candidate 4.2 Sequential MCP Integrity (Persistent Stdio Session)

Candidate 4.2 was subjected to a sequential, continuous JSON-RPC stdio session executing 7 successive requests without process restart:

```
[Start Session] -> initialize (172 B) -> tools/list (2,345 B) -> inspect_symbol [auth.effects.ts] (11,088 B)
  -> inspect_symbol [AuthEffects] (11,397 B) -> get_context [Task B] (8,096 B) -> preflight (132 B)
  -> postflight (860 B) -> [Verify Alive & Invariant Hash]
```

### 4.1 Request-by-Request Execution Audit

```
[Session Start] Process PID: 49400, Binary SHA-256: 78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2
------------------------------------------------------------------------------------------------------------------------
Req 1: initialize
  Payload Size: 172 bytes
  Status: isError=false
  Process State: ALIVE (exit_code=None)

Req 2: tools/list
  Payload Size: 2,345 bytes
  Tools Discovered: 4 tools (engram_get_context, engram_inspect_symbol, engram_preflight, engram_postflight)
  Process State: ALIVE (exit_code=None)

Req 3: engram_inspect_symbol (Target: "projects/angular-ngrx-material-starter/src/app/core/auth/auth.effects.ts")
  Adversarial Note: Tests Defect 12.1 multi-byte UTF-8 character boundary compaction
  Payload Size: 11,088 bytes (<= 16,384 bytes)
  Status: isError=false
  Process State: ALIVE (Defect 12.1 panic eliminated!)

Req 4: engram_inspect_symbol (Target: "AuthEffects")
  Payload Size: 11,397 bytes (<= 16,384 bytes)
  Status: isError=false, Completeness: CONTRACTED
  Process State: ALIVE (exit_code=None)

Req 5: engram_get_context (Task: Task B prompt, Budget: 2000 tokens)
  Payload Size: 8,096 bytes (<= 16,384 bytes)
  Files Preserved: 3 primary target files
  Process State: ALIVE (exit_code=None)

Req 6: engram_preflight (Task: "task_b_remember_me")
  Payload Size: 132 bytes
  Process State: ALIVE (exit_code=None)

Req 7: engram_postflight
  Payload Size: 860 bytes
  Process State: ALIVE (exit_code=None)
------------------------------------------------------------------------------------------------------------------------
[Session End] All 7 requests completed successfully.
Exit Code on Clean Shutdown: 0
Binary SHA-256 After Session: 78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2 (Bitwise Identical)
```

Every response respected the hard 16,384-byte ceiling. Defect 12.1 is confirmed eliminated.

---

## 5. Semantic Retrieval Revalidation on Golden Repo

Using Candidate 4.2 against the golden repository (`angular-ngrx-material-starter` at commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`), 5 cross-file dependency scenarios were tested:

| Scenario | Target Entity / File | Retrieval Tool & Query | Resulting Payload Size | Discovery Result | Completeness |
| :---: | :--- | :--- | :---: | :---: | :---: |
| **1** | `auth.actions.ts` | `engram_get_context(task="auth.actions", subpath=".../auth", budget=2000)` | 15,366 B | **FOUND** (5 core auth files returned) | COMPLETE |
| **2** | `auth.effects.ts` | `engram_inspect_symbol(symbol="AuthEffects")` | 11,389 B | **FOUND** (`auth.effects.ts` returned) | CONTRACTED |
| **3A** | `auth.models.ts` | `engram_inspect_symbol(symbol="AuthState")` | 14,690 B | **FOUND** (`auth.models.ts` returned) | COMPLETE |
| **3B** | `auth.reducer.ts` | `engram_inspect_symbol(symbol="authReducer")` | 14,526 B | **FOUND** (`auth.reducer.ts` returned) | CONTRACTED |
| **4** | `app.component.ts` | `engram_inspect_symbol(symbol="AppComponent")` | 16,057 B | **FOUND** (`app.component.ts` returned) | CONTRACTED |
| **5** | `auth.reducer.spec.ts`| `engram_get_context(task="auth reducer tests", subpath=".../auth", budget=2000)` | 15,642 B | **FOUND** (6 test/spec files returned) | COMPLETE |

All 5 dependencies are 100% discoverable through deterministic MCP queries while staying strictly within the 16,384-byte boundary.

---

## 6. Four-Arm Offline Simulation & Failure Accounting Check

An offline dry-run simulation of all four arms was executed:

1. **Sandbox Cleanliness & Isolation:**
   - Four distinct temporary working copies were spawned (`arm_a_native`, `arm_b_cand3_always_on`, `arm_c_cand4_always_on`, `arm_d_cand4_adaptive`).
   - Verified that file edits in one arm sandbox do not pollute other sandboxes.
   - All sandboxes were cleanly torn down post-run.
2. **Failure Accounting Verification:**
   - In the dry-run simulation, task completion score was 0.20 (Accepted: False).
   - The harness strictly set `claim_status = "N/A"`, suppressing all token savings claims and percentage reduction calculations.
   - Zero misleading or unearned efficiency claims were emitted.

---

## 7. Full Regression Results & Test Count Reconciliation

### 7.1 Rust Regression Suites (96 Tests Passed, 0 Failed)

| Test Suite File | Command Executed | Tests Run | Passed | Failed | Duration | Status |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: |
| **Unit Tests (`src/lib.rs`)** | `cargo test --lib` | 24 | 24 | 0 | 0.31s | **PASS** |
| **Context Budget Tests** | `cargo test --test context_budget_test` | 20 | 20 | 0 | 0.01s | **PASS** |
| **Inspect Symbol Budget Tests** | `cargo test --test inspect_symbol_budget_test` | 13 | 13 | 0 | 2.86s | **PASS** |
| **MCP Boundary Tests** | `cargo test --test mcp_boundary_test` | 11 | 11 | 0 | 7.84s | **PASS** |
| **MCP Consolidation Tests** | `cargo test --test mcp_consolidation_test` | 13 | 13 | 0 | 6.73s | **PASS** |
| **MCP Handshake Tests** | `cargo test --test mcp_test` | 1 | 1 | 0 | 0.71s | **PASS** |
| **UTF-8 Character Safety Tests** | `cargo test --test utf8_safety_test` | 14 | 14 | 0 | 0.78s | **PASS** |
| **Total Rust Regression** | **All 7 Suites** | **96** | **96** | **0** | **19.24s** | **PASS** |

### 7.2 Python Harness & Benchmark Suites (331 Tests Passed, 0 Failed)

The complete pytest suite across the benchmark harness was executed from the repository root:
$$\text{Collected: } 331 \text{ items} \quad \longrightarrow \quad \mathbf{331\text{ passed, } 0\text{ failed in } 384.37\text{s}}$$

### 7.3 Python Test Count Reconciliation (62 vs 156 vs 331)

To eliminate any ambiguity regarding test counts across project phases, the relationship between the numbers is formally reconciled:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Repository Pytest Scope (331 Tests)                    │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │             Phase 5.4.2 Full Core Harness Suite (156 Tests)           │  │
│  │                                                                       │  │
│  │  ┌──────────────────────────────────────────────────────────┐          │  │
│  │  │   Phase 5.4.3 Targeted Suite (62 Tests)                  │          │  │
│  │  │   • test_adaptive_activation.py (12)                     │          │  │
│  │  │   • test_tetrad_harness.py (1)                           │          │  │
│  │  │   • test_task_b_readiness_fixes.py (15)                  │          │  │
│  │  │   • test_task_b_evaluator.py (20)                        │          │  │
│  │  │   • test_triad.py (5)                                    │          │  │
│  │  │   • test_mcp_runtime_activation.py (9)                   │          │  │
│  │  └──────────────────────────────────────────────────────────┘          │  │
│  │  + test_token_efficiency.py (19)                                      │  │
│  │  + test_token_guard_validation.py (18)                                 │  │
│  │  + test_amendment_04_two_arm.py (23)                                  │  │
│  │  + test_harness_safety_audit.py (17)                                   │  │
│  │  + test_verification_workflow.py (17)                                 │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│  + Historical Milestone & Pilot Test Suites (175 Tests):                    │
│    • test_block07_remediation.py (14)                                        │
│    • test_milestone_2c_remediation.py (19)                                   │
│    • test_milestone_5b_remediation.py (12)                                   │
│    • test_milestone_5c_remediation.py (5)                                    │
│    • test_milestone_5f_remediation.py (4)                                    │
│    • test_pilot_03a_local_runtime.py (8)                                     │
│    • test_resilience_repair.py (7)                                           │
│    • test_suite.py (106)                                                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

1. **Phase 5.4.3 Targeted Suite (62 tests):** Narrowly targeted Candidate 4.2 resolution and tetrad execution: $12 + 1 + 15 + 20 + 5 + 9 = 62$.
2. **Phase 5.4.2 Core Harness Suite (156 tests):** Composed of the 62 candidate tests plus 94 safety, telemetry, and workflow tests ($62 + 37 + 23 + 17 + 17 = 156$).
3. **Full Harness Pytest Discovery (331 tests):** Composed of the 156 core tests plus 175 historical milestone tests ($156 + 175 = 331$).

All 331 tests pass with zero failures.

---

## 8. Remaining Risks and Defenses

| Risk Description | Potential Impact | In-Place Defense / Mitigation | Verified in Preflight |
| :--- | :--- | :--- | :---: |
| **Model Hallucination on Non-Existent Tools** | Turn exhaustion looking for tools disabled under BYPASS/LEAN | Explicit `available_tools` listing injected in Turn-0 contract directive | Yes |
| **Multi-File Context Fragmentation** | Agent misses `auth.actions.ts` when inspecting `auth.effects.ts` | Symbol inspection explicitly contracts and references sibling symbols; `engram_get_context` with subpath resolves complete core auth group | Yes |
| **Token Budget Edge Exhaustion** | Run terminated at turn 14 due to reserve depletion | 350,000 ceiling provides ~40,000 token surplus over worst-case historical baseline; 45,000 reserve prevents catastrophic overrun | Yes |
| **Windows File Lock Stalls on Cleanup** | Sandbox teardown fails or leaks temp files | Process tree PID tracker and retry teardown logic verified across 17 safety audit tests | Yes |
| **Accidental Candidate Mutation** | Rebuild or overwrite corrupts benchmark reproducibility | Hash invariant check performed at pre-run; binary files sealed read-only | Yes |

---

## 9. Live-Run Protocol & Exact Launch Configuration

When authorized, the live benchmark execution will run with the following sealed parameters:

```bash
# Environment Flag
$env:ENGRAM_USE_CANDIDATE_4_2 = "1"

# Command Line Invocation
python -m benchmarks.matched-pair-harness.scripts.run_tetrad_live `
  --task-config tasks/task_b_remember_me.json `
  --arms arm_a_native arm_b_cand3_always_on arm_c_cand4_always_on arm_d_cand4_adaptive `
  --max-tokens-ceiling 350000 `
  --headroom-reserve 45000 `
  --timeout-seconds 1200 `
  --output-dir benchmarks/matched-pair-harness/evidence/task_b_live_01
```

### Protocol Invariants
1. No binary rebuilds or recompilations.
2. Candidate 3 binary SHA-256 must match `97C5EC47...`.
3. Candidate 4.2 binary SHA-256 must match `78B84880...`.
4. Run will proceed sequentially through Arm A, Arm B, Arm C, and Arm D.
5. If any arm encounters an unexpected runtime error, execution halts immediately with evidence preserved.

---

## 10. Preflight Acceptance & Final Recommendation

| Gate Item | Target Standard | Measured Value | Preflight Verdict |
| :--- | :--- | :--- | :---: |
| **Candidate 3 Binary Sealed** | SHA-256 `97C5EC47...` | SHA-256 `97C5EC47...` | **PASS** |
| **Candidate 4.2 Binary Sealed**| SHA-256 `78B84880...` | SHA-256 `78B84880...` | **PASS** |
| **Four-Arm Resolution** | Arm C & D $\to$ Cand 4.2 | Arm C & D $\to$ Cand 4.2 | **PASS** |
| **MCP Hard Boundary** | $\le 16,384$ bytes | All responses $\le 16,384$ B | **PASS** |
| **Defect 12.1 UTF-8 Panic** | Zero panics | 14 safety tests + live stdio pass | **PASS** |
| **Task B Budget Guard** | 350k ceiling, 45k reserve | 7 of 7 boundary tests pass | **PASS** |
| **Semantic Discoverability** | 5 of 5 dependencies found | 5 of 5 dependencies found | **PASS** |
| **Failure Accounting** | Suppress claims on failure | `claim_status: N/A` verified | **PASS** |
| **Rust Regression** | Zero failures | 96 passed, 0 failed | **PASS** |
| **Python Regression** | Zero failures | 331 passed, 0 failed | **PASS** |

### Final Recommendation

**`READY FOR EXPLICIT LIVE TASK B AUTHORIZATION`**

All preflight gates are satisfied. Live model dispatch remains blocked until the user provides explicit authorization.
