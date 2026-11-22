# Engram Phase 5.8.1 Live Benchmark Readiness Report

**Date:** 2026-11-22  
**Target Candidate:** Candidate 5 (Engram v1.5.4 Optimized Engine)  
**Live Benchmark Authorization Status:** `NOT_AUTHORIZED` (Offline Verification Mode Active)  
**Harness Mode:** Five-Arm Pentad Evaluation  

---

## 1. Executive Summary

Phase 5.8.1 successfully remediated the single critical blocker (`DEFECT-5.8-01`) that prevented Candidate 5 certification in Phase 5.8. Candidate 5 is now fully verified offline, successfully frozen in the benchmark staging directory, and integrated into a verified 5-arm pentad benchmark harness.

All prerequisite technical readiness criteria for future live matched-pair benchmarking have been established offline. However, live execution remains strictly **`NOT_AUTHORIZED`** pending formal administrative authorization.

---

## 2. Five-Arm Candidate Readiness Assessment

| Arm | Identity | Binary Path | Expected SHA-256 | Verification Status | Readiness |
| :--- | :--- | :--- | :---: | :---: | :---: |
| **Arm A** | `arm_a_native` | N/A (Standard CLI agent) | N/A | N/A | READY |
| **Arm B** | `arm_b_cand3` | `benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe` | `97C5EC47E3...` | Hash & CLI Verified | READY |
| **Arm C** | `arm_c_cand42_always_on` | `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe` | `78B84880AB...` | Hash & CLI Verified | READY |
| **Arm D** | `arm_d_cand42_adaptive` | `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe` | `78B84880AB...` | Hash & CLI Verified | READY |
| **Arm E** | `arm_e_cand5_optimized` | `benchmarks/matched-pair-harness/bin/candidate-phase5/engram.exe` | `E4E5321624...` | Hash & CLI Verified | READY |

All four binary artifacts are safely staged, verified against their registered ledger hashes, and smoke-tested.

---

## 3. Harness Architecture & Pentad Capabilities

1. **Protocol Rigor:**
   - Supports genuine 5-arm pentad blocks with randomized or seed-deterministic execution permutations.
   - Pre-dispatch binary hash verification blocks execution on 1-bit discrepancies.
   - Golden dependency fingerprint verification before and after sandbox provisioning prevents cross-run contamination.
2. **Defensive Safety Controls:**
   - Per-arm token ceiling enforcement overrides status to `TOKEN_CEILING_BREACHED` upon overflow.
   - Global cumulative budget ceiling (`--global-tokens-ceiling`) with headroom reserve halts downstream arms before incurring billing overruns.
   - Strict stop-on-failure policy skips downstream arms if an earlier arm fails acceptance.
   - Diagnostic continue mode allows end-to-end telemetry collection for diagnostic evaluation without certifying invalid savings.
3. **Comparative Analysis Invariant:**
   - Comparative claims between arms are unconditionally blocked (`MUTUAL_ACCEPTANCE_FAILURE`) unless all evaluated arms satisfy the correctness gate.

---

## 4. Benchmark Task Fixture Readiness

- **Task A (`task_a_typo`):** Angular project typo repair fixture verified and clean.
- **Task B (`task_b_remember_me`):** Angular project stateful feature fixture (`CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7`) verified intact.
- **Preflight Checks:** Automated preflight validates required files, present patterns, and dirty fixture markers prior to arm dispatch.

---

## 5. Outstanding Risks & Operational Limitations

1. **API Rate Limiting & Provider Jitter:** Remote LLM providers may introduce non-deterministic latency or sporadic connection drops. The harness requires exponential backoff retry for transient provider network errors.
2. **Live Budget Overhead:** A full 5-arm live run across all test tasks consumes an estimated 350,000–500,000 provider tokens. Tight global ceilings (`--global-tokens-ceiling 600000`) and headroom reserves (`--headroom-reserve 45000`) must be supplied.
3. **Dirty Workspace Isolation:** Sandboxes must be instantiated on temporary local disks with `junction` or clean copy strategies; `preserve-sandboxes` should only be enabled during debugging.

---

## 6. Pre-Dispatch Authorization Requirements

Before any live benchmark invocation:
1. Executive stakeholder approval for live API token expenditures.
2. Explicit provider model confirmation (`gpt-5.6-luna`, `reasoning_effort: medium`).
3. Fixed random seed assignment for block reproducibility.
4. Execution policy configured to `strict`.

---

## 7. Conclusion

Candidate 5 has satisfied all technical prerequisites. The benchmark harness is fully capable of executing a statistically sound, defensive, and reproducible 5-arm pentad evaluation when live authorization is granted.
