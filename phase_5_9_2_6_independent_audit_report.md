# Engram Phase 5.9.2.6: Independent Offline Benchmark Integrity Audit & Candidate 5 Result Certification

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Expected HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Actual HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Audit Mode:** Strictly Offline, Read-Only, Independent Verification  
**Benchmark Provenance:** `SYNTHETIC_SIMULATED_USAGE`  
**Audit Timestamp:** 2026-11-22T14:18:00Z  

---

## 1. Executive Summary & Certified Primary Verdict

An independent forensic audit of the **Phase 5.9.2.5 Offline Deterministic Benchmark** was conducted across the target repository `C:\Users\Amal P S\OneDrive\Projects\Engram`. The audit examined all 30 raw execution records (6 workloads $\times$ 5 pentad arms), the provider simulator (`provider_simulator.py`), the benchmark runner (`offline_benchmark_runner.py`), the agent runtime (`agent_runtime.py`), the 6 workload fixture workspaces, and the 104-item regression test suite.

### Certified Primary Verdict
```
OFFLINE_BENCHMARK_VALID_WITH_DOCUMENTED_LIMITATIONS
```

### Forensic Justification
1. **Mathematical & Execution Validity:**
   All 30 benchmark runs executed inside clean, isolated sandboxes with zero cross-arm contamination. The raw `telemetry.json` records, serialized wire payloads, and unit test outputs are mathematically verifiable, internally consistent, and 100% reproducible under seed `59002`. Functional correctness achieved 100% parity across all 5 arms (30/30 runs passed concrete unit test suites).
2. **Documented Limitations & Synthetic Boundaries:**
   - **Simulation Nature:** The provider simulator (`DeterministicProviderSimulator`) executes predefined multi-turn action sequences (`_workload_plans`) modeling expected agent decisions rather than dynamic LLM inference.
   - **Synthesized MCP Responses:** When Engram MCP tools (`engram_get_context`, `engram_inspect_symbol`) are called, `ControlledAgentRuntime` synthesizes standardized responses without spawning the `engram.exe` binary over stdio.
   - **Workload E Weighting:** Workload E represents 65.1% of Native Baseline's aggregate tokens (19,601 of 30,113) due to multi-turn conversation memory accumulation. When Workload E is excluded, Candidate 5 still provides a **-37.69%** token reduction vs Native and **-47.88%** vs Candidate 3.
   - **Workload F Tool Schema Overhead:** In Workload F, Candidate 5 consumes 1,562 tokens (+0.71% / +11 tokens) vs Native's 1,551 tokens, demonstrating that tool definition schema overhead slightly exceeds turn consolidation gains on minimal 2-turn fallback tasks.
   - **No Live Provider Inferences:** The benchmark carries the `SYNTHETIC_SIMULATED_USAGE` provenance tag and provides zero empirical validation of upstream LLM provider billing, actual API pricing, or network latency.

---

## 2. Raw-Record Metric Reconciliation (30/30 Runs)

Every metric in the table below was independently recomputed directly from the 30 raw `telemetry.json` records located in `benchmarks/matched-pair-harness/runs/offline_pentad_deterministic_20261122_192518`:

| Workload | Pentad Arm | Wire Input Tokens | Wire Output Tokens | Total Synthetic Tokens | Peak Context (Wire) | Model Turns | Tool Calls | Functional Pass |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Workload A: Simple** | `arm_a_native` | 1,586 | 110 | 1,696 | 653 | 3 | 2 | **PASS** |
| | `arm_b_cand3` | 2,333 | 100 | 2,433 | 894 | 3 | 2 | **PASS** |
| | `arm_c_cand42_always_on` | 2,333 | 88 | 2,421 | 894 | 3 | 2 | **PASS** |
| | `arm_d_cand42_adaptive` | 1,009 | 60 | 1,069 | 569 | 2 | 1 | **PASS** |
| | `arm_e_cand5_optimized` | 1,011 | 55 | **1,066** | 571 | 2 | 1 | **PASS** |
| **Workload B: Cross-File** | `arm_a_native` | 2,935 | 205 | 3,140 | 1,023 | 4 | 4 | **PASS** |
| | `arm_b_cand3` | 2,563 | 170 | 2,733 | 1,103 | 3 | 3 | **PASS** |
| | `arm_c_cand42_always_on` | 2,563 | 165 | 2,728 | 1,103 | 3 | 3 | **PASS** |
| | `arm_d_cand42_adaptive` | 2,350 | 165 | 2,515 | 1,032 | 3 | 3 | **PASS** |
| | `arm_e_cand5_optimized` | 1,556 | 115 | **1,671** | 942 | 2 | 2 | **PASS** |
| **Workload C: Debugging** | `arm_a_native` | 1,618 | 105 | 1,723 | 681 | 3 | 2 | **PASS** |
| | `arm_b_cand3` | 2,363 | 110 | 2,473 | 919 | 3 | 2 | **PASS** |
| | `arm_c_cand42_always_on` | 2,363 | 100 | 2,463 | 919 | 3 | 2 | **PASS** |
| | `arm_d_cand42_adaptive` | 1,717 | 100 | 1,817 | 714 | 3 | 2 | **PASS** |
| | `arm_e_cand5_optimized` | 1,031 | 65 | **1,096** | 591 | 2 | 1 | **PASS** |
| **Workload D: Orchestration** | `arm_a_native` | 2,282 | 120 | 2,402 | 750 | 4 | 3 | **PASS** |
| | `arm_b_cand3` | 2,466 | 110 | 2,576 | 957 | 3 | 3 | **PASS** |
| | `arm_c_cand42_always_on` | 2,466 | 110 | 2,576 | 957 | 3 | 3 | **PASS** |
| | `arm_d_cand42_adaptive` | 1,628 | 90 | 1,718 | 656 | 3 | 2 | **PASS** |
| | `arm_e_cand5_optimized` | 1,080 | 75 | **1,155** | 640 | 2 | 2 | **PASS** |
| **Workload E: Context-Heavy** | `arm_a_native` | 19,441 | 160 | 19,601 | 7,120 | 4 | 5 | **PASS** |
| | `arm_b_cand3` | 2,445 | 105 | 2,550 | 939 | 3 | 3 | **PASS** |
| | `arm_c_cand42_always_on` | 2,445 | 100 | 2,545 | 939 | 3 | 3 | **PASS** |
| | `arm_d_cand42_adaptive` | 1,613 | 90 | 1,703 | 641 | 3 | 2 | **PASS** |
| | `arm_e_cand5_optimized` | 989 | 50 | **1,039** | 549 | 2 | 1 | **PASS** |
| **Workload F: Recovery** | `arm_a_native` | 1,476 | 75 | 1,551 | 578 | 3 | 2 | **PASS** |
| | `arm_b_cand3` | 2,283 | 70 | 2,353 | 847 | 3 | 2 | **PASS** |
| | `arm_c_cand42_always_on` | 2,283 | 70 | 2,353 | 847 | 3 | 2 | **PASS** |
| | `arm_d_cand42_adaptive` | 2,283 | 70 | 2,353 | 847 | 3 | 2 | **PASS** |
| | `arm_e_cand5_optimized` | 1,507 | 55 | **1,562** | 831 | 2 | 2 | **PASS** |

---

## 3. Six-Workload Equivalence Matrix

The 6 benchmark fixtures under `benchmarks/matched-pair-harness/workloads/` were inspected for structural parity:

| Workload ID | Task Complexity | Task Prompt | Target Files | Verification Harness | Assertions Tested |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Workload A** | `SIMPLE` | Fix `add_tax` in `src/calculator.py` | `src/calculator.py` | `python -m unittest tests/test_calculator.py` | Tax added to price: 100 @ 5% $\to$ 105.0; 50 @ 10% $\to$ 55.0 |
| **Workload B** | `MODERATE` | Add `is_active` to `Account` and filter in `AccountService` | `src/models.py`, `src/service.py` | `python -m unittest tests/test_service.py` | Inactive accounts filtered out; only active accounts returned |
| **Workload C** | `MODERATE` | Fix `safe_divide` for empty and zero-sum lists | `src/indexer.py` | `python -m unittest tests/test_indexer.py` | Non-zero list division; all-zero list returns zeros; empty list returns empty |
| **Workload D** | `COMPLEX` | Update `batch_size: 64, workers: 4, validated: True` | `config.json` | `python -m unittest tests/test_pipeline.py` | Validated JSON configuration values loaded by pipeline |
| **Workload E** | `COMPLEX` | Update `MAX_RETRIES: 5` without legacy clutter | `src/target_module.py` | `python -m unittest tests/test_target.py` | Target module constant updated: `MAX_RETRIES == 5` |
| **Workload F** | `MODERATE` | Verify loader fallback when `settings.json` is missing | `src/loader.py` | `python -m unittest tests/test_loader.py` | Loader gracefully falls back to `default_settings.json` |

---

## 4. Five-Arm Simulator Execution-Path Matrix

| Arm Identity | Configured Tools | MCP Spawning Behavior | Turn Sequence Generation | Input Token Accounting |
| :--- | :--- | :--- | :--- | :--- |
| **Arm A (Native)** | 4 Workspace tools (`read_file`, `write_file`, `list_dir`, `execute_command`) | Zero Engram MCP tools. Cannot have engram binary. | Plan models sequential exploratory reads & writes | Dynamically measured wire serialization |
| **Arm B (Cand 3)** | 4 Workspace tools + Engram MCP tools (`engram_get_context`, `engram_inspect_symbol`) | Binary path verified; text response synthesized | Plan models static symbol table query + file edit | Dynamically measured wire serialization (includes tool schemas) |
| **Arm C (Cand 4.2 Always)** | 4 Workspace tools + Compressed Engram MCP tools | Binary path verified; text response synthesized | Plan models compressed symbol index query + file edit | Dynamically measured wire serialization |
| **Arm D (Cand 4.2 Adapt)** | 4 Workspace tools + Dynamic MCP tools | Binary path verified; text response synthesized | Heuristic bypasses MCP on SIMPLE tasks; queries on others | Dynamically measured wire serialization |
| **Arm E (Cand 5 Opt)** | 4 Workspace tools + 7 Optimization MCP tools | Binary path verified; text response synthesized | Direct AST surgical edit; batched tool calls; context pruning | Dynamically measured wire serialization |

---

## 5. Token and Context Metric Calculations

$$\text{Percentage Change} = \frac{\text{Candidate 5} - \text{Comparison Arm}}{\text{Comparison Arm}} \times 100$$

| Comparison Arm | Baseline Synthetic Tokens | Cand 5 Synthetic Tokens | Synthetic Token Reduction | Turn Reduction | Tool Call Reduction | Acceptance Parity |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **vs Native Baseline** | 30,113 | 7,589 | **-74.80%** (-22,524) | **-42.86%** (12 vs 21) | **-50.00%** (9 vs 18) | 100% vs 100% |
| **vs Candidate 3 (v1.5.1)** | 15,118 | 7,589 | **-49.80%** (-7,529) | **-33.33%** (12 vs 18) | **-40.00%** (9 vs 15) | 100% vs 100% |
| **vs Cand 4.2 Always-On** | 15,086 | 7,589 | **-49.70%** (-7,497) | **-33.33%** (12 vs 18) | **-40.00%** (9 vs 15) | 100% vs 100% |
| **vs Cand 4.2 Adaptive** | 11,175 | 7,589 | **-32.09%** (-3,586) | **-29.41%** (12 vs 17) | **-25.00%** (9 vs 12) | 100% vs 100% |

---

## 6. Peak-Context Discrepancy Resolution

### Root-Cause Resolution
1. **Source of Initial Report (1,950 / 810 / 680 / 570 / 250):**  
   These values were taken directly from the hardcoded `synthetic_prompt_tokens` inside `SimulatedTurn` in `provider_simulator.py` for Workload E:
   - `arm_a_native`: `[550, 1100, 1650, 1950]` $\to$ max = **1,950**
   - `arm_b_cand3`: `[420, 650, 810]` $\to$ max = **810**
   - `arm_c_cand42_always_on`: `[360, 550, 680]` $\to$ max = **680**
   - `arm_d_cand42_adaptive`: `[310, 460, 570]` $\to$ max = **570**
   - `arm_e_cand5_optimized`: `[190, 250]` $\to$ max = **250**  
   $(250 - 1950) / 1950 = -87.18\% \approx -87.2\%$.
2. **Source of Chat Table (520 tokens for Candidates):**  
   The assistant in chat mistakenly entered 520 tokens to match a $-51.9\%$ calculation $((250 - 520) / 520 = -51.9\%)$.
3. **Ground Truth from Raw Execution Records (`telemetry.json`):**  
   The actual measured request wire tokens (`serialize_request(req)`) for Workload E are:
   - Native Baseline: **7,120 tokens**
   - Candidate 3: **939 tokens**
   - Candidate 4.2 Always-On: **939 tokens**
   - Candidate 4.2 Adaptive: **641 tokens**
   - Candidate 5: **549 tokens**  
   $$\frac{549 - 7120}{7120} = -92.29\% \quad (\text{vs Native})$$
   $$\frac{549 - 939}{939} = -41.53\% \quad (\text{vs Candidate 3})$$

---

## 7. Workload E Sensitivity Analysis

Workload E workspace contains `docs/architecture.md` (13,525 bytes), `src/legacy_v1.py` (6,920 bytes), `src/legacy_v2.py` (6,920 bytes), and `src/target_module.py` (39 bytes).
- **Native Baseline Token Explosion:** Native reads all files across 4 turns. Cumulative conversation memory carries forward the full contents of all read files (~27 KB), generating 19,441 input tokens.
- **Candidate 5 Context Pruning:** Candidate 5 executes a surgical edit directly on `src/target_module.py` without loading the legacy documentation files.

### Aggregate Comparison With and Without Workload E

| Metric | All 6 Workloads (With E) | 5 Workloads (Excluding E: A, B, C, D, F) |
| :--- | :---: | :---: |
| **Native Baseline Tokens** | 30,113 | 10,512 |
| **Candidate 3 Tokens** | 15,118 | 12,568 |
| **Cand 4.2 Always-On Tokens** | 15,086 | 12,541 |
| **Cand 4.2 Adaptive Tokens** | 11,175 | 9,472 |
| **Candidate 5 Tokens** | **7,589** | **6,550** |
| **Cand 5 vs Native (Tokens)** | **-74.80%** (-22,524) | **-37.69%** (-3,962) |
| **Cand 5 vs Cand 3 (Tokens)** | **-49.80%** (-7,529) | **-47.88%** (-6,018) |
| **Cand 5 vs Cand 4.2 Adapt (Tokens)** | **-32.09%** (-3,586) | **-30.85%** (-2,922) |
| **Cand 5 vs Native (Turns)** | **-42.86%** (12 vs 21) | **-41.18%** (10 vs 17) |
| **Cand 5 vs Native (Tools)** | **-50.00%** (9 vs 18) | **-38.46%** (8 vs 13) |

---

## 8. Workload F Regression Analysis

On Workload F (`workload_f_failure_recovery`):
- **Native Baseline:** 1,551 total synthetic tokens (1,476 in, 75 out, 3 turns, 2 tools).
- **Candidate 5:** 1,562 total synthetic tokens (1,507 in, 55 out, 2 turns, 2 tools).
- **Delta:** Candidate 5 consumes **+11 tokens (+0.71%)** more than Native Baseline.

### Forensic Cause
1. **Tool Schema Overhead:** Candidate 5 registers Engram MCP tools in `req.tools`. In JSON wire serialization, the tool schema definitions add ~250 bytes per request.
2. **Turn Consolidation Trade-off:** Candidate 5 consolidated 2 file reads into a single Turn 0 batch. The combined tool results in Turn 1 increased the input payload size slightly more than the single turn saved in this minimal scenario.
3. **Assessment:** This is an expected boundary condition: on minimal 2-turn tasks requiring trivial error fallback, MCP schema registration overhead slightly offsets turn consolidation gains.

---

## 9. Functional Acceptance Evidence

The 30 execution workspaces were verified against the unit test suites:
- **Workload A:** `test_calculator.py` passed in 0.000s across all 5 arms.
- **Workload B:** `test_service.py` passed in 0.000s across all 5 arms.
- **Workload C:** `test_indexer.py` passed in 0.000s across all 5 arms.
- **Workload D:** `test_pipeline.py` passed in 0.000s across all 5 arms.
- **Workload E:** `test_target.py` passed in 0.000s across all 5 arms.
- **Workload F:** `test_loader.py` passed in 0.001s across all 5 arms.
- **Total Functional Acceptance Rate:** **100% (30/30 runs passed)**. Zero functional regressions.

---

## 10. Reproducibility Assessment

- **Determinism:** Seed `59002` produces 100% byte-for-byte identical request serializations, token estimates, tool sequences, and test outcomes.
- **Sandbox Isolation:** Each arm executes in an independent subdirectory (`runs/.../<workload>/<arm>/workspace`) created from a clean fixture copy.
- **Historical Evidence Preserved:** Historical Phase 5.9.2.1 run directory (`runs/two_task_pentad_live_20261122_180013`) remains completely untouched.

---

## 11. Test-Suite Verification

- **Command:** `pytest test_phase_5_9_2_5_offline_benchmark.py test_phase_5_9_2_3_http_contract.py test_phase_5_9_2_2_live_transport_enforcement.py test_controlled_transport.py test_budget_coordinator.py test_request_boundary_enforcement.py test_request_size_gate.py test_round_trip_telemetry.py -q`
- **Results:** **104 passed in 8.77s**. Zero failures, zero warnings, zero skipped tests.
- **Binary Integrity Verified:**
  - Candidate 3 (`bin/candidate-phase3/engram.exe`): 18,668,032 bytes, SHA-256 `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777`
  - Candidate 4.2 (`bin/candidate-phase4.2/engram.exe`): 18,802,176 bytes, SHA-256 `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2`
  - Candidate 5 (`bin/candidate-phase5/engram.exe`): 18,883,584 bytes, SHA-256 `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44`

---

## 12. Limitations & Release Implications

1. **Synthetic Provenance Boundary:** The benchmark results confirm deterministic orchestration efficiency under simulated conditions. They must never be described as live LLM provider billing data.
2. **Release Authorization:** The codebase and Candidate 5 binary are cleared for **Experimental / Developer Preview Release Packaging** under explicit disclosure of synthetic benchmark methodology.
3. **Future Live Validation:** Validating actual upstream LLM token billing reductions and live provider latency remains an optional future validation track pending API credential availability.
