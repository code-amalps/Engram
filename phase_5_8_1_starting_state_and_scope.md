# Engram Phase 5.8.1: Starting State and Scope Report

**Date:** 2026-11-22  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Git Branch:** `feature/phase2-stage5-remediation`  
**Git HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Previous Audit:** Phase 5.8 (`CANDIDATE5_NOT_READY`)  
**Execution Mode:** Controlled Offline Implementation, Regression Testing, Isolated Build, and Pentad Validation  

---

## 1. Baseline Git Identity & Preserved Working Tree State

The repository identity and state were verified prior to any source or test modifications:

- **Repository Path:** `C:\Users\Amal P S\OneDrive\Projects\Engram`
- **Active Branch:** `feature/phase2-stage5-remediation`
- **Git Commit (HEAD):** `7106a2f369fe79faf5f95449378a900ab8e8eddf`
- **Staged Diff Bytes:** Exactly `0` bytes (clean index).
- **Tracked Modified Files:** Exactly `15` files.

### Tracked Modified File Inventory (15 Files):
1. `Cargo.toml`
2. `README.md`
3. `benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json`
4. `benchmarks/matched-pair-harness/src/classifier.py`
5. `benchmarks/matched-pair-harness/src/evaluator.py`
6. `benchmarks/matched-pair-harness/src/orchestrator.py`
7. `benchmarks/matched-pair-harness/src/runner.py`
8. `benchmarks/matched-pair-harness/src/telemetry.py`
9. `src/budget.rs`
10. `src/compiler.rs`
11. `src/impact.rs`
12. `src/lib.rs`
13. `src/main.rs`
14. `src/mcp.rs`
15. `src/storage.rs`

*Note on Historical Discrepancy:* Phase 5.4.31 established 13 modified files. Phase 5.5 subsequently introduced token optimizations in `src/budget.rs` and `src/compiler.rs`, bringing the authoritative count to 15 modified tracked files.

---

## 2. Historical Artifact Invariants Verification

All canonical historical artifacts were verified using SHA-256 digests prior to starting Phase 5.8.1:

| Artifact | Canonical Expected SHA-256 | Actual Measured SHA-256 | Byte Size | Status |
| :--- | :--- | :--- | :--- | :--- |
| **Candidate 3 Binary** (`bin/candidate-phase3/engram.exe`) | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | 18,512,896 | **VERIFIED MATCH** |
| **Candidate 4.2 Binary** (`bin/candidate-phase4.2/engram.exe`)| `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | 18,844,672 | **VERIFIED MATCH** |
| **Task B Fixture** (`config/tasks/task_b_remember_me.json`) | `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` | `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` | 3,923 | **VERIFIED MATCH** |
| **Cargo Lockfile** (`Cargo.lock`) | `85FF18D67207BEF644D8C84582124D5EA0988983AC64C51CBA8EB6975FE11D39` | `85FF18D67207BEF644D8C84582124D5EA0988983AC64C51CBA8EB6975FE11D39` | 104,821 | **VERIFIED MATCH** |

---

## 3. Scope and Objectives of Phase 5.8.1

1. **Remediation of `DEFECT-5.8-01` in `src/compiler.rs`:**
   - Reproduce test failure `query_router_test::test_15_missing_or_invalidated_entities`.
   - Implement a robust cache invalidation design that handles database index updates, entity deletions, file modifications, file renames, and non-Git workspaces.
   - Prove 100% pass across all 50 integration test suites.
2. **Preservation of Optimizations A–F:**
   - Ensure the cache fix does not degrade or interfere with symbol ranking, type pruning, signature extraction, or token budget bounding.
3. **Five-Arm Pentad Integration in Benchmark Harness:**
   - Register `cand5` in `CANDIDATE_LEDGER` in `orchestrator.py`.
   - Register `arm_e_cand5_optimized` in `arm_identities`.
   - Implement `run_pentad()` execution logic with fail-closed validation.
   - Preserve existing four-arm tetrad execution (`run_tetrad`).
4. **Candidate 5 Release Build & Frozen Staging:**
   - Build Candidate 5 offline.
   - Stage binary to `benchmarks/matched-pair-harness/bin/candidate-phase5/engram.exe`.
   - Record immutable build manifest and source hash manifest.
5. **Synthetic Offline Pentad Dry Run:**
   - Execute mock 5-arm pentad across benchmark tasks offline to verify orchestration, telemetry parsing, and acceptance logic.
6. **Certification & Manifest Generation:**
   - Author all 22 Phase 5.8.1 deliverables and master evidence manifest.

---

## 4. Operational Boundaries

- **Live Benchmark Authorization:** `NOT_AUTHORIZED`
- **Online Security Audit Authorization:** `NOT_AUTHORIZED`
- **Commit / Push / Tag Authorization:** `NOT_AUTHORIZED`
- **Network Access:** Strictly forbidden (`CARGO_NET_OFFLINE=true`)
