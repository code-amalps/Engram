# Engram Phase 5.8: Live Benchmark Readiness Report

**Date:** 2026-11-22  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Candidate Evaluated:** Candidate 5 (Phase 5.5 Optimization Engine)  
**Harness Evaluated:** `benchmarks/matched-pair-harness/`  
**Auditor:** Adversarial Forensic Audit Team  
**Readiness Verdict:** `NOT_READY_FOR_LIVE_BENCHMARK`

---

## 1. Executive Summary

Phase 5.8 evaluated whether Candidate 5 and the matched-pair benchmarking infrastructure are technically prepared, verifiable, and safe for live benchmark execution against remote LLM providers (e.g., OpenAI Codex / GPT-4o).

**Verdict:**  
**Live Benchmarking is BLOCKED.**  
Candidate 5 and the benchmark harness fail multiple mandatory readiness gates. Under no circumstances should live benchmark runs be initiated in the current state.

---

## 2. Readiness Evaluation Gate Matrix

| Readiness Gate | Requirement | Current Status | Gate Verdict |
| :--- | :--- | :--- | :--- |
| **Gate 1: Build Reproducibility** | Deterministic, isolated rebuild matches release candidate | Reproducible (PE header / metadata variances only; `.pdata` 100% identical) | **PASSED** |
| **Gate 2: Historical Integrity** | Historical binaries (Cand 3, 4.2) and fixtures 100% match SHA-256 | Bit-for-bit identical hashes verified across all frozen artifacts | **PASSED** |
| **Gate 3: Full Regression Suite** | Zero test failures across all 50 repository test suites | 1 test failed: `query_router_test::test_15_missing_or_invalidated_entities` | **FAILED (BLOCKED)** |
| **Gate 4: Cache Consistency** | Context compiler reliably invalidates modified/deleted entities | Optimization G serves stale context on entity deletion (`DEFECT-5.8-01`) | **FAILED (BLOCKED)** |
| **Gate 5: Harness Candidate Registration** | Candidate 5 registered in harness `CANDIDATE_LEDGER` | `cand5` missing from `CANDIDATE_LEDGER` in `orchestrator.py` | **FAILED (BLOCKED)** |
| **Gate 6: Five-Arm Orchestration** | Harness capable of dispatching 5-arm pentad execution | Harness only supports 4-arm tetrad (`run_tetrad`) | **FAILED (BLOCKED)** |
| **Gate 7: Frozen Binary Staging** | Candidate 5 frozen in `benchmarks/.../bin/candidate-phase5/` | Directory and staged binary do not exist | **FAILED (BLOCKED)** |
| **Gate 8: Live Authorization** | Explicit human authorization for live API spend | Strictly `NOT_AUTHORIZED` (Offline audit mode) | **BLOCKED (POLICY)** |

---

## 3. Detailed Blocker Analysis

### Blocker 1: Functional Regression in Context Cache (`DEFECT-5.8-01`)
- **Impact:** In non-git workspaces or repeated test runs, `COMPILED_CONTEXT_CACHE` in `src/compiler.rs` fails to detect filesystem changes because the cache key omits SQLite database index timestamps and file modification times.
- **Consequence:** In a live multi-turn benchmark where an agent edits or deletes files, Candidate 5 would serve stale context, degrading correctness and producing invalid evaluation outcomes.

### Blocker 2: Harness Candidate Ledger Omission
- **Impact:** `BenchmarkOrchestrator` in `orchestrator.py` strictly restricts candidate choices to `['native', 'cand3', 'cand4', 'cand4_1', 'cand4_2']`.
- **Consequence:** Invoking the benchmark harness with Candidate 5 fails immediately at CLI parsing (`argparse: invalid choice: 'cand5'`).

### Blocker 3: Lack of Five-Arm Pentad Orchestration
- **Impact:** Phase 5.6 formulated a 5-arm comparative framework, but `orchestrator.py` only implements a 4-arm tetrad loop (`run_tetrad`).
- **Consequence:** The harness cannot schedule, execute, isolate, or aggregate telemetry across five simultaneous arms.

### Blocker 4: Missing Frozen Candidate 5 Binary
- **Impact:** `benchmarks/matched-pair-harness/bin/` lacks a `candidate-phase5/` directory.
- **Consequence:** The harness enforces fail-closed binary verification; without a frozen binary and pre-registered SHA-256 hash, execution is rejected.

---

## 4. Remediation Roadmap: Path to Phase 5.8.1

Before live benchmarking can be authorized, the following sequential remediation steps must be completed in Phase 5.8.1:

1. **Remediate Optimization G Cache Invalidation:**
   - Modify `src/compiler.rs` to include database mtime or a sequence epoch counter in `COMPILED_CONTEXT_CACHE` key.
   - Verify that `cargo test --test query_router_test` passes with 15/15 tests.
2. **Re-compile & Freeze Candidate 5:**
   - Perform clean release build.
   - Create `benchmarks/matched-pair-harness/bin/candidate-phase5/`.
   - Copy `engram.exe` into this directory and compute canonical SHA-256.
3. **Extend Benchmark Harness for 5-Arm Pentad:**
   - Add `"cand5"` to `CANDIDATE_LEDGER` in `orchestrator.py`.
   - Define `arm_e_cand5_optimized` in `arm_identities`.
   - Implement `run_pentad()` with Latin-square block scheduling.
   - Update `schema_validator.py` for pentad summary schema.
4. **Offline Mock Pentad Verification:**
   - Execute mock pentad run offline to verify zero-error orchestration.

---

## 5. Audit Conclusion

Candidate 5 demonstrates outstanding theoretical token-efficiency optimizations, but it is not yet ready for live benchmarking due to an active cache invalidation bug and harness software limitations. Phase 5.8.1 remediation is required.
