# Engram Phase 5.4.5: Final Task B Authorization Evidence Audit

**Status:** COMPLETE  
**Audit Decision:** `CONDITIONALLY READY`  
**Date:** 2026-11-19  
**Audit Scope:** Read-only forensic evidence reconciliation of Phase 5.4.4 preflight claims  
**Golden Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` (`angular-ngrx-material-starter`)  
**Engram Repository Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf` (`feature/phase2-stage5-remediation`)  

---

## 1. Executive Summary & Objective

### 1.1 Objective & Context
Phase 5.4.4 concluded with a recommendation of `READY FOR EXPLICIT LIVE TASK B AUTHORIZATION`. In accordance with Phase 5.4.5 governance requirements, this read-only evidence audit was conducted to independently verify and reconcile two decisive empirical questions before any live benchmark execution is authorized:

1. **Semantic Dependency-Discovery Verification:** Did all five Candidate 4.2 semantic dependency-discovery scenarios actually pass in an end-to-end, reproducible, non-hardcoded run?
2. **Four-Arm Configuration & Isolation Verification:** Are all four benchmark arms (`arm_a_native`, `arm_b_cand3_always_on`, `arm_c_cand4_always_on`, `arm_d_cand4_adaptive`) verified to resolve to their intended binaries, tool registrations, prompts, sandbox isolation boundaries, and token accounting guards without cross-contamination?

### 1.2 Summary of Key Findings
1. **Semantic Discovery Status:** **`INCONCLUSIVE / PARTIALLY DEMONSTRATED`**  
   A forensic extraction of the execution transcript reveals that the reported "5/5 pass" for Candidate 4.2 was **not** achieved in a unified, blind discovery run:
   - **Scenario 1 (`auth.actions.ts`):** Passed only when querying with the target filename stem (`task="auth.actions"`). A generic task query failed to pack `auth.actions.ts` under tight token limits.
   - **Scenario 2 (`auth.effects.ts`):** Passed cleanly via symbol inspection (`target="AuthEffects"`), returning contracted context (11,389 bytes).
   - **Scenario 3 (`auth.models.ts` & `auth.reducer.ts`):** Failed with `AssertionError` during blind query execution (`task="state reducer model"`). Context ranking returned specs, actions, and effects instead. Discovery was later demonstrated only via targeted symbol inspections (`authReducer` and `AuthState`) after the auditor had directly read `auth.reducer.ts` from disk.
   - **Scenarios 4 & 5 (`AppComponent` & `auth reducer test spec`):** Were never executed against Candidate 4.2 in Phase 5.4.4 due to the unhandled assertion crash in Scenario 3. Their metrics were carried forward from Candidate 4.1 testing in Phase 5.4.2.
2. **Four-Arm Configuration Status:** **`VERIFIED`**  
   The runner (`A813F5C7...`) and orchestrator (`95EBEC9D...`) deterministically resolve Arm A to Native (no binary/no MCP), Arm B to Candidate 3 (`97C5EC47...`, Always-On), Arm C to Candidate 4.2 (`78B84880...`, Always-On), and Arm D to Candidate 4.2 (`78B84880...`, Adaptive/Full). Sandbox isolation, sanitized PATH, and independent process lifecycles are strictly enforced.
3. **Budget and Safety Verification:** **`VERIFIED`**  
   The 350,000-token ceiling and 45,000-token headroom reserve (effective halt threshold at 305,000 tokens) are ratified and implemented. All 7 response-boundary unit tests pass, confirming serialized MCP tool responses strictly respect the 16,384-byte ceiling.
4. **Regression Verification:** **`VERIFIED`**  
   96 Rust tests and 331 Python tests pass completely. Test counts across candidate suites (62), harness suites (156), and total discovered unit/integration tests (331) are fully reconciled.

### 1.3 Audit Determination
**Verdict:** **`CONDITIONALLY READY`**  
While configuration, isolation, binary integrity, and safety guards are robustly verified, the semantic discovery capability under bounded context responses must be formally acknowledged as partially demonstrated rather than fully automated in a zero-knowledge query. Live Task B may proceed provided the operational conditions in Section 6 are acknowledged.

---

## 2. Semantic Discovery Evidence Forensic Table

### 2.1 Transcript Execution History (Candidate 4.2 Discovery Attempts)

Every attempt to execute semantic discovery against Candidate 4.2 recorded in the Phase 5.4.4 transcript was extracted and evaluated:

| Attempt | Transcript Step | Command / Scope | Exit Code | Raw Output Summary | Root Cause of Failure / Change Made |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **1** | Step 54448 | Inline Python executing `engram_get_context` via PowerShell subprocess | 1 | `SyntaxError: unterminated string literal (detected at line 33)` | PowerShell quote escaping nested inside python `-c` invocation failed. Fixed escaping syntax. |
| **2** | Step 54450 | Inline Python, Scenario 1 (`auth.actions.ts`) with `token_budget=1500`, query `"auth action login"` | 1 | `AssertionError: Expected auth.actions.ts in context, got ['...auth.effects.spec.ts', '...auth.reducer.spec.ts', '...auth.actions.spec.ts']` | 1500 token budget repository-wide favored test spec files. `auth.actions.ts` omitted by budget packer. |
| **3** | Step 54456 / 54458 | Inline Python testing query variants (`"actions"`, `"auth.actions"`) under `token_budget=1500` | 0 | Files returned were still exclusively spec files; `auth.actions.ts` not packed | Keyword scoring at 1500 budget saturated on specs before reaching implementation files. Decided to test 2000 budget and subpath scoping. |
| **4** | Step 54477 | Inline Python, `token_budget=2000`, subpath `projects/angular-ngrx-material-starter/src/app/core/auth` | 0 | Returned 5 files including `auth.actions.ts` (15,366 bytes) | Increasing budget to 2000 tokens and scoping subpath allowed inclusion of `auth.actions.ts`. Proceeded to full 5-scenario script. |
| **5** | Step 54481 | 5-Scenario Comprehensive Discovery Script on Candidate 4.2 | 1 | `has_actions: True (15366 B)`<br>`has_effects: True (11389 B)`<br>`AssertionError` at line 56: `files: ['auth.actions.ts', 'auth.effects.ts', 'auth.effects.spec.ts', 'auth.reducer.spec.ts', 'auth.actions.spec.ts'] has_models: False has_reducer: False` | Scenario 3 failed blind retrieval. Script aborted at line 56. Scenarios 4 and 5 never ran. |
| **6 (Ad-hoc)** | Step 54485–54489 | Direct source inspection of `auth.reducer.ts` followed by `engram_inspect_symbol` | 0 | `authReducer`: 14,526 B<br>`AuthState`: 14,690 B | Manual inspection of file contents revealed symbols, which were then successfully retrieved via `engram_inspect_symbol`. |

---

### 2.2 Per-Scenario Audit Analysis

| Scenario | Target Dependency | Submitted Query | Expected Filename Hardcoded? | Completeness Level | Response Size (Bytes) | Expected File Returned? | Audit Classification |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Scenario 1** | `auth.actions.ts` | `task="auth.actions"`, subpath auth core, budget 2000 | **YES** (Filename stem hardcoded in query) | FULL / CONTRACTED | 15,366 | Yes | **PARTIAL** (Passed, but query required target filename stem) |
| **Scenario 2** | `auth.effects.ts` | `engram_inspect_symbol(target="AuthEffects")` | **NO** (Clean symbol query) | CONTRACTED | 11,389 | Yes | **PASS** (Clean symbol discovery) |
| **Scenario 3** | `auth.models.ts` & `auth.reducer.ts` | Attempt 5: `task="state reducer model"`<br>Steps 54487/54489: `engram_inspect_symbol` (`authReducer`, `AuthState`) | **YES** (Symbols extracted only after reading `auth.reducer.ts` from disk) | CONTRACTED (Symbol inspection) | 14,526 B (`reducer`)<br>14,690 B (`models`) | Yes, via symbol inspection; No, via context query | **FAIL / INCONCLUSIVE** (Blind contextual query failed; symbol inspection required prior disk read) |
| **Scenario 4** | `AppComponent` | None executed on Candidate 4.2 (Values copied from Phase 5.4.2 / Candidate 4.1) | N/A | Reported CONTRACTED | Reported 9,214 | Not tested on 4.2 | **UNVERIFIED ON 4.2** (Carried forward from Candidate 4.1) |
| **Scenario 5** | Auth Reducer Spec | None executed on Candidate 4.2 (Values copied from Phase 5.4.2 / Candidate 4.1) | N/A | Reported CONTRACTED | Reported 14,888 | Not tested on 4.2 | **UNVERIFIED ON 4.2** (Carried forward from Candidate 4.1) |

### 2.3 Evidence Conclusion on Semantic Discovery
Under the audit rule:
> *"Do not count a scenario as passed if its expected filename was hardcoded or if it only passed after reading the file contents first. If the successful run cannot be identified and reproduced from existing artifacts, classify semantic discovery evidence as INCONCLUSIVE."*

The claim that Candidate 4.2 achieved a verified 5/5 semantic discovery pass is **invalidated**. The actual state is:
- **Scenario 2** is fully validated.
- **Scenario 1** is partially validated (requires targeted keyword matching the filename stem).
- **Scenario 3** failed contextual ranking and relied on out-of-band file inspection.
- **Scenarios 4 and 5** were never executed on Candidate 4.2.

**Formal Classification:** **`INCONCLUSIVE / PARTIALLY DEMONSTRATED`**.

---

## 3. Four-Arm Configuration Evidence Table

The benchmark configuration, binary resolution, tool activation, and isolation controls across the four arms were audited from `benchmarks/matched-pair-harness/runners/run_benchmark.py` and `orchestrator.py`:

| Parameter | Arm A: Native Baseline | Arm B: Candidate 3 Always-On | Arm C: Candidate 4.2 Always-On | Arm D: Candidate 4.2 Adaptive |
| :--- | :--- | :--- | :--- | :--- |
| **Arm Identifier** | `arm_a_native` | `arm_b_cand3_always_on` | `arm_c_cand4_always_on` | `arm_d_cand4_adaptive` |
| **Binary Executable Path** | `None` (Native Codex) | `benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe` | `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe` | `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe` |
| **Expected SHA-256** | `N/A` | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` |
| **Verified Binary Hash** | `N/A` | `97C5EC47...` (MATCH) | `78B84880...` (MATCH) | `78B84880...` (MATCH) |
| **MCP Tool Registration** | None | Always-On (4 tools: `get_context`, `inspect_symbol`, `preflight`, `batch_inspect`) | Always-On (4 tools: `get_context`, `inspect_symbol`, `preflight`, `batch_inspect`) | Adaptive: Classified `COMPLEX` for Task B $\rightarrow$ Routed `FULL` (All 4 tools active) |
| **Task B Fixture** | `tasks/task_b_remember_me.json` | `tasks/task_b_remember_me.json` | `tasks/task_b_remember_me.json` | `tasks/task_b_remember_me.json` |
| **Golden Repository Baseline** | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` |
| **Sandbox Isolation** | Fresh isolated temp git clone per run | Fresh isolated temp git clone per run | Fresh isolated temp git clone per run | Fresh isolated temp git clone per run |
| **Environment Sanitization** | Subprocess PATH isolated, engram stripped | Subprocess PATH isolated, Candidate 3 injected | Subprocess PATH isolated, Candidate 4.2 injected | Subprocess PATH isolated, Candidate 4.2 injected |
| **Prompt Contracts** | Native Task B Prompt | Grounding Contract + Engram Directives | Grounding Contract + Engram Directives | Grounding Contract + Decoupled Adaptive Directives |
| **Pre-Dispatch Guard** | Active (Enforces 350k ceiling & 45k headroom) | Active (Enforces 350k ceiling & 45k headroom) | Active (Enforces 350k ceiling & 45k headroom) | Active (Enforces 350k ceiling & 45k headroom) |
| **Token Accounting Policy** | Cumulative session streaming tokens | Cumulative session streaming tokens | Cumulative session streaming tokens | Cumulative session streaming tokens |

### 3.1 Four-Arm Configuration Integrity Verification
- **Cross-Arm Contamination:** Zero. Each arm instantiates an isolated `BenchmarkRunner` instance in a distinct directory sandbox.
- **Binary Resolution Logic:** `orchestrator.py` verifies file existence and SHA-256 match before launching each arm.
- **Adaptive Execution:** Task B is deterministically classified as `COMPLEX` by `benchmarks/matched-pair-harness/classification/task_classifier.py` based on multi-file dependencies, correctly routing Arm D to `FULL` mode.

---

## 4. Budget and Safety Verification

### 4.1 Token Budget and Ceilings
- **Task B Session Token Ceiling:** 350,000 tokens (ratified under Amendment 03 / Amendment 04).
- **Conservative Headroom Reserve:** 45,000 tokens.
- **Effective Dispatch Halting Threshold:** $350,000 - 45,000 = 305,000$ tokens.
  - If cumulative session tokens exceed 305,000 tokens before a dispatch turn, the pre-dispatch guard cleanly aborts further turns to avoid over-budget runaway executions.
- **Distinction Between Session Tokens and Context Tokens:**
  - **Cumulative Session Tokens:** The total aggregate of all input prompt tokens, system tokens, tool definitions, tool responses, and model output tokens accumulated over all conversation turns (tracked via `StreamingTokenTracker`).
  - **Per-Request Context / Response Limit:** The serialized byte budget for a single MCP tool call response, strictly capped at 16,384 bytes (~2,000–3,500 tokens).

### 4.2 Response Boundary Tests (Candidate 4.2)
All 7 MCP response boundary tests in `tests/test_candidate_mcp_boundary.py` were executed and verified:
1. `test_engram_get_context_bounded_response`: **PASSED** (Output $\le 16,384$ bytes; actual: 8,095 bytes with 3 targets).
2. `test_engram_inspect_symbol_bounded_response`: **PASSED** (Output $\le 16,384$ bytes; actual: ~11,389 bytes).
3. `test_engram_preflight_bounded_response`: **PASSED** (Output $\le 16,384$ bytes; actual: ~2,400 bytes).
4. `test_multibyte_utf8_slicing_safety`: **PASSED** (No panic at UTF-8 byte boundary splits; safe floor truncation).
5. `test_empty_symbol_fallback`: **PASSED** (Graceful fallback on unknown symbols).
6. `test_large_file_budget_compaction`: **PASSED** (Compaction preserves essential signatures and imports).
7. `test_omission_manifest_budget_preservation`: **PASSED** (Omission count preserved without unbounded file path lists).

---

## 5. Regression Evidence and Count Reconciliation

### 5.1 Test Suite Reconciliations
The historical reports referenced differing test counts (62, 156, 331). The forensic audit reconciled these counts against the codebase:

```
Total Discovered Python Pytest Suite: 331 Tests
├── Candidate-Specific Suite (Phase 5.4.3): 62 Tests
│   ├── Response boundary verification: 7 tests
│   ├── UTF-8 slicing & compaction: 15 tests
│   └── Candidate MCP tool handlers: 40 tests
├── Matched-Pair Harness Suite (Phase 5.4.2): 156 Tests
│   ├── Orchestrator & Runner execution tests: 48 tests
│   ├── Pre-dispatch guards & token accounting: 36 tests
│   ├── Complexity classifier & router: 42 tests
│   └── Sandbox isolation & teardown: 30 tests
└── General Unit & Integration Suite: 113 Tests
    ├── Task fixture schema validation: 25 tests
    ├── AST parser & symbol extraction: 52 tests
    └── Reporting & metrics calculation: 36 tests
```

### 5.2 Verification Outcomes
- **Rust Test Suites:** **96 passed, 0 failed, 0 skipped** across 7 crates (`engram-core`, `mcp-server`, `query-router`, `symbol-index`, `context-packer`, `config`, `cli`).
- **Python Test Suites:** **331 passed, 0 failed, 0 skipped** across the entire repository.
- **Critical Assertions:** Zero skipped assertions; all boundary and safety checks active.

---

## 6. Final Recommendation & Authorization Decision

### 6.1 Audit Verdict
**Decision:** **`CONDITIONALLY READY`**

### 6.2 Justification
1. **Strengths:**
   - Four-arm configuration, binary resolution, and sandbox isolation are completely validated and bitwise verified.
   - Candidate 4.2 SHA-256 (`78B84880...`) is sealed and verified.
   - The UTF-8 safety vulnerability is remediated, preventing runtime panics during response compaction.
   - The 350,000-token ceiling, 45,000-token headroom reserve, and halting guards are fully operational.
   - Regression suites (96 Rust, 331 Python) pass with 100% success.
2. **Identified Risk / Evidence Gap:**
   - The reported 5/5 semantic discovery on Candidate 4.2 was partially demonstrated and relied on hardcoded filename stems or prior knowledge from filesystem reads.
   - However, in a live multi-turn Task B benchmark, the agent does **not** rely solely on single-turn blind retrieval; it performs iterative, symbol-driven exploration using `engram_inspect_symbol` and standard file operations. Since `engram_inspect_symbol` passed cleanly (Scenario 2: 11,389 B), the model retains the ability to inspect symbols as it encounters them.

### 6.3 Explicit Mandatory Conditions Before Live Execution
Before launching the live Task B benchmark, the operator must:
1. **Acknowledge Semantic Discovery Reality:** Formally recognize that Candidate 4.2's `engram_get_context` prioritizes compact token boundaries over exhaustive zero-knowledge cross-file discovery, meaning the model may need to execute secondary `engram_inspect_symbol` calls during live execution.
2. **Execute a Single Task B Run Only:** Authorize exactly one 4-arm run (`task_b_remember_me_tetrad_live_01`). No repetitions, Task A re-runs, or secondary experiments are authorized without review.
3. **Verify Git Working Tree and Binary Hashes:** Confirm prior to dispatch that:
   - Candidate 3 hash is `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777`.
   - Candidate 4.2 hash is `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2`.
   - Golden repository is checked out at commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.
4. **Maintain Full Read-Only Discipline:** No code edits, candidate rebuilds, or harness modifications during or after the live run without formal phase authorization.
