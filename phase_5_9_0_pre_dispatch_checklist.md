# Engram Phase 5.9.0: Pre-Dispatch Verification Checklist

**Execution Phase:** 5.9.0  
**Target Program:** Live Five-Arm Pentad Benchmark (Phase 5.9)  
**Status:** ALL PRE-DISPATCH CHECKS COMPLETED & AUDITED  

---

## 1. Candidate Executable Verification

- [x] **Candidate 3 Binary:** `benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe` verified present with SHA-256 `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777`.
- [x] **Candidate 4.2 Binary:** `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe` verified present with SHA-256 `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2`.
- [x] **Candidate 5 Binary:** `benchmarks/matched-pair-harness/bin/candidate-phase5/engram.exe` verified present with SHA-256 `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44`.
- [x] **Runtime Smoke Tests:** Direct execution of `--version`, `doctor`, and JSON-RPC initialization verified for all candidates.

---

## 2. Benchmark Fixture & Repository Verification

- [x] **Task A Fixture:** `benchmarks/matched-pair-harness/config/tasks/task_a_typo.json` verified with SHA-256 `357D9B4B5777A41A8C5691475DCA8DDAC00134C480C6324E26B60FEA39F95F02`.
- [x] **Task B Fixture:** `benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json` verified with SHA-256 `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7`.
- [x] **Golden Repo State:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` verified present at baseline commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.
- [x] **Evaluator Preflight:** `evaluator.validate_preflight()` verified clean for both fixtures without dirty markers or missing files.

---

## 3. Harness Orchestration & Schema Verification

- [x] **Schema Enum Updated:** `run_telemetry.schema.json` contains all pentad arm identities (`arm_a_native`, `arm_b_cand3`, `arm_c_cand42_always_on`, `arm_d_cand42_adaptive`, `arm_e_cand5_optimized`).
- [x] **Orchestrator Unit & Acceptance Tests:** 20/20 tests in `test_pentad_harness.py` and 1/1 in `test_tetrad_harness.py` pass cleanly.
- [x] **Codex CLI Available:** `C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe` verified accessible and executable.

---

## 4. Budget & Parameter Verification

- [x] **Task A Ceilings Configured:** `--max-tokens-ceiling 250000`, `--global-tokens-ceiling 1000000`, `--headroom-reserve 45000`.
- [x] **Task B Ceilings Configured:** `--max-tokens-ceiling 500000`, `--global-tokens-ceiling 1500000`, `--headroom-reserve 45000`.
- [x] **Deterministic Seeds Assigned:** Task A Seed `59001`; Task B Seed `59002`.
- [x] **Execution Policy:** `--execution-policy strict` specified.
- [x] **Dependency Isolation:** `--dependency-strategy copy` specified.

---

## 5. Administrative Sign-Off Requirement

- [ ] **Live Execution Authorization:** PENDING EXECUTIVE STAKEHOLDER REVIEW. (Do not dispatch until authorized).
