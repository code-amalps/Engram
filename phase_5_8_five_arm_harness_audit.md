# Engram Phase 5.8: Five-Arm Benchmark Harness Audit Report

**Date:** 2026-11-22  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Git HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Target Architecture:** Five-Arm Matched-Pair Benchmark (Phase 5.6 Specification)  
**Harness Implementation Inspected:** `benchmarks/matched-pair-harness/src/orchestrator.py`  
**Auditor:** Adversarial Forensic Audit Team  
**Audit Verdict:** `HARNESS_UNPREPARED_TETRAD_ONLY`

---

## 1. Executive Summary

Phase 5.6 specified a five-arm benchmarking framework to rigorously measure the comparative performance of Engram across its generational iterations:
- **Arm A:** Native Codex (Vanilla Baseline, Zero Engram)
- **Arm B:** Candidate 3 Always-On (v1.5.1 Baseline Engine)
- **Arm C:** Candidate 4.2 Always-On (v1.5.4 Static Baseline Engine)
- **Arm D:** Candidate 4.2 Adaptive (v1.5.4 Adaptive Routing Engine)
- **Arm E:** Candidate 5 Optimized (v1.5.4 Seven-Optimization Engine)

This forensic audit conducted a line-by-line inspection of the benchmark harness implementation located in `benchmarks/matched-pair-harness/`.

**Core Forensic Finding:**  
The benchmark harness currently implemented in `benchmarks/matched-pair-harness/src/orchestrator.py` is strictly a **Four-Arm Tetrad Harness**. It possesses **zero operational support** for Arm E or Candidate 5. Attempting to execute a live 5-arm benchmark with the current codebase immediately terminates with fail-closed errors.

---

## 2. Forensic Gap Analysis: Specified 5-Arm Architecture vs Actual Harness Implementation

| Dimension | Phase 5.6 Specification (5-Arm Pentad) | Actual Codebase Implementation (`orchestrator.py`) | Forensic Classification |
| :--- | :--- | :--- | :--- |
| **Execution Function** | `run_pentad()` or 5-arm permutation dispatcher | `run_tetrad()` only (lines 2011–2250) | `MISSING_FEATURE` |
| **Candidate Ledger** | `native`, `cand3`, `cand4_2`, `cand5` | `native`, `cand3`, `cand4`, `cand4_1`, `cand4_2` (line 344) | `DEFICIENT_LEDGER` |
| **CLI Flag Support** | `--candidate-version cand5` | Choices: `['native', 'cand3', 'cand4', 'cand4_1', 'cand4_2']` | `UNSUPPORTED_CLI_CHOICE` |
| **Arm Identity Map** | Arm A, Arm B, Arm C, Arm D, Arm E | Arm A, Arm B, Arm C, Arm D (cand4, cand4_1, cand4_2) | `MISSING_ARM_IDENTITY` |
| **Candidate 5 Binary** | `benchmarks/.../bin/candidate-phase5/engram.exe` | Directory does not exist in `benchmarks/matched-pair-harness/bin/` | `MISSING_FROZEN_ASSET` |
| **Ordering Schedules** | 5! = 120 block permutations (or Latin Square 5x5) | Tetrad schedules (4-arm block permutations) | `UNSUPPORTED_SCHEDULE` |
| **Telemetry Aggregation** | 5-arm comparison matrices (Pentad summary) | Tetrad summary and triad summary schemas only | `SCHEMA_RESTRICTION` |

---

## 3. Detailed Codebase Inspection Findings

### 3.1 Candidate Ledger Defect (`orchestrator.py` Lines 344–351)

The authoritative ledger definition inside `BenchmarkOrchestrator.__init__` is:

```python
CANDIDATE_LEDGER = {
    "native": {"bin": None, "sha": None, "label": "Native"},
    "cand3": {"bin": cand3_bin, "sha": cand3_sha, "label": "Candidate 3"},
    "cand4": {"bin": cand4_bin, "sha": cand4_sha, "label": "Candidate 4"},
    "cand4_1": {"bin": cand4_1_bin, "sha": cand4_1_sha, "label": "Candidate 4.1"},
    "cand4_2": {"bin": cand4_2_bin, "sha": cand4_2_sha, "label": "Candidate 4.2"},
}
```

- `cand5` is completely missing from this ledger.
- If `--candidate-version cand5` is supplied, `argparse` rejects the argument before script execution begins.
- If bypassed programmatically, `validate_against_ledger()` throws:
  `ValueError: Invalid candidate version 'cand5'. Allowed choices: ['cand3', 'cand4', 'cand4_1', 'cand4_2', 'native']`.

### 3.2 Arm Identity Registration Defect (`orchestrator.py` Lines 450–510)

`self.arm_identities` only configures identities for:
- `arm_a_native`
- `arm_b_cand3_always_on`
- `arm_c_cand4_always_on`
- `arm_d_cand4_adaptive`
- `arm_c_cand4_1_always_on`
- `arm_d_cand4_1_adaptive`
- `arm_c_cand4_2_always_on`
- `arm_d_cand4_2_adaptive`

There is no entry for `arm_e_cand5_optimized` or any Candidate 5 identity.

### 3.3 Physical Asset Omission (`benchmarks/matched-pair-harness/bin/`)

Inspection of the filesystem reveals the following candidate directories:
```
benchmarks/matched-pair-harness/bin/
├── candidate-phase1
├── candidate-phase2
├── candidate-phase3
├── candidate-phase4
├── candidate-phase4.1
├── candidate-phase4.2
├── v1.5.1
└── v1.5.4
```
There is **no** `candidate-phase5` directory. Candidate 5 exists solely as the newly compiled output in `target/release/engram.exe` and has not been frozen, verified, or deployed into the harness candidate storage directory.

### 3.4 Tetrad Execution Logic (`orchestrator.py` Lines 2011–2250)

The harness orchestration loop is hardcoded to execute 4 arms:
```python
def run_tetrad(self, block_name: str, task_name: str, ...):
    arms = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]
    ...
```
There is no provision for a 5th arm in execution ordering, token budget tracking, cross-arm cleanup, or the downstream markdown/JSON reporting pipeline.

---

## 4. Remediation Requirements for Phase 5.8.1

To achieve full 5-arm benchmark readiness, the following engineering tasks must be performed in a dedicated remediation phase:

1. **Harness Ledger Extension:**
   - Add `"cand5"` to `CANDIDATE_LEDGER` in `orchestrator.py`.
   - Add `"cand5"` to `argparse` choices for `--candidate-version`.
   - Update `FROZEN_ENGRAM_SHA256` constants in `isolation.py`.
2. **Arm E Registration:**
   - Define `arm_e_cand5_optimized` in `self.arm_identities`.
3. **Candidate 5 Binary Staging:**
   - Create `benchmarks/matched-pair-harness/bin/candidate-phase5/`.
   - Copy the verified release binary into this path and record its immutable SHA-256 hash.
4. **Pentad Orchestration Engine:**
   - Implement `run_pentad()` supporting 5 arms with Latin-square block balance.
   - Extend `tetrad_summary.json` schema to `pentad_summary.json` (or generalize to n-arm schema).

---

## 5. Audit Conclusion

The claims in Phase 5.6 regarding the existence of a "Five-Arm Benchmark Harness" represent an architectural specification rather than an operational reality. The existing harness is exclusively a 4-arm tetrad harness. Live execution of a 5-arm benchmark is currently **blocked**.
