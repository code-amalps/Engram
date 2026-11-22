# Engram Phase 5.4.11: Per-Arm Candidate Identity Metadata Correction & Regression Verification Report

**Phase:** Engram Phase 5.4.11  
**Timestamp:** 2026-11-19T21:35:00+05:30  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Git Branch:** `feature/phase2-stage5-remediation`  
**Golden Repository Root:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` (`1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`)  
**Task B Fixture Hash:** `cd55db0dfdb5758275b262b8b330858e9673533aa6faa182897edae83d69a2b7`  
**Status:** `VERIFIED & READY FOR EXPLICIT AUTHORIZATION`

---

## 1. Executive Summary

During the Phase 5.4.10 Final Live Task B Dispatch Readiness Audit, an engineering discrepancy was identified in the matched-pair/tetrad benchmark harness:
In `arm_b_cand3_always_on`'s telemetry record (`run_telemetry.isolation`), the version metadata string `resolved_candidate_version` recorded the global benchmark-level CLI argument (`"cand4_2"`), even though the actual executable assigned and executed was strictly Candidate 3 (`bin/candidate-phase3/engram.exe`, SHA-256: `97C5EC...`). While the path and SHA-256 were factually correct, the metadata label misrepresented the binary assigned to the individual arm.

In Phase 5.4.11, this defect was definitively remediated:
1. **Three Distinct Candidate Concepts:** Established explicit separation between:
   - `benchmark_candidate_version`: Global CLI target (e.g. `cand4_2`).
   - `requested_candidate_version`: Per-arm intentional target (`native`, `cand3`, `cand4_2`, `cand4_2`).
   - `resolved_candidate_version`: Per-arm verified target (`native`, `cand3`, `cand4_2`, `cand4_2`).
2. **Authoritative Immutable Model:** Implemented `ArmCandidateIdentity` dataclass with fail-closed ledger validation (`validate_against_ledger`) and physical binary verification (`verify_binary_integrity`).
3. **Fail-Closed Consistency Enforcement:** Mismatches between arm configuration, candidate version, binary path, and expected hash trigger immediate `SafetyViolationError` halts prior to sandbox provisioning or agent dispatch.
4. **Comprehensive Regression Coverage:** Created `test_per_arm_candidate_identity.py` covering Tests A through J (10 test cases, 100% pass rate in 187.59s).
5. **Historical Telemetry Integrity:** Verified historical telemetry artifacts from Phase 5.4.7, 5.4.8, 5.4.9, and 5.4.10 remain completely unmutated, valid against `run_telemetry.schema.json`, and legacy missing fields are gracefully exposed as unavailable (`None`) rather than guessed.
6. **Zero Live Dispatches:** Zero live model API requests (Codex/Gemini/OpenAI) were executed. Candidate binaries remain 100% bitwise intact.

---

## 2. Original Metadata Defect & Root Cause

### 2.1 The Defect
When orchestrating a 4-arm matched benchmark with `--candidate-version cand4_2`, the telemetry emission in `_execute_single_arm` previously populated:
```python
"requested_candidate_version": self.requested_candidate_version,
"resolved_candidate_version": self.resolved_candidate_version if is_engram else "native",
```
Because `self.resolved_candidate_version` held the global CLI candidate (`"cand4_2"`), any arm where `is_engram=True` (including Arm B) received `"cand4_2"` as its `resolved_candidate_version`, creating a contradiction with `engram_binary_path` (`.../candidate-phase3/engram.exe`) and `engram_binary_actual_sha256` (`97C5EC...`).

### 2.2 Root Cause Analysis
The orchestrator previously conflated the benchmark-level CLI selection with per-arm candidate assignment. While `self.arm_binaries["arm_b_cand3_always_on"]` and `self.arm_expected_hashes["arm_b_cand3_always_on"]` correctly resolved to Candidate 3, there was no per-arm identity record linking the version name string to the specific arm's binary.

---

## 3. Before-and-After Telemetry Comparison

### Before (Phase 5.4.10 State)
```json
// Arm B: arm_b_cand3_always_on (Phase 5.4.10 Defective State)
{
  "arm": "arm_b_cand3_always_on",
  "isolation": {
    "engram_enabled": true,
    "engram_binary_path": "C:\\...\\bin\\candidate-phase3\\engram.exe",
    "engram_binary_expected_sha256": "97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777",
    "engram_binary_actual_sha256": "97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777",
    "requested_candidate_version": "cand4_2",  // <-- DEFECT: Global CLI override
    "resolved_candidate_version": "cand4_2",   // <-- DEFECT: Conflated with global version
    "candidate_verification_status": "VERIFIED",
    "mcp_registration_verified": true,
    "mcp_server_status": "ACTIVE"
  }
}
```

### After (Phase 5.4.11 Corrected State)
```json
// Arm B: arm_b_cand3_always_on (Phase 5.4.11 Corrected State)
{
  "arm": "arm_b_cand3_always_on",
  "isolation": {
    "engram_enabled": true,
    "engram_binary_path": "C:\\...\\bin\\candidate-phase3\\engram.exe",
    "engram_binary_expected_sha256": "97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777",
    "engram_binary_actual_sha256": "97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777",
    "benchmark_candidate_version": "cand4_2",  // <-- CORRECT: Global CLI selection
    "requested_candidate_version": "cand3",    // <-- CORRECT: Arm-specific request
    "resolved_candidate_version": "cand3",     // <-- CORRECT: Arm-specific resolved version
    "candidate_verification_status": "VERIFIED",
    "mcp_registration_verified": true,
    "mcp_server_status": "ACTIVE"
  }
}
```

---

## 4. Corrected Four-Arm Identity Table

The following authoritative resolution table is enforced simultaneously across all 4 arms during tetrad execution:

| Arm Key | Benchmark Candidate | Requested Candidate | Resolved Candidate | Binary Path | Expected SHA-256 | Actual SHA-256 | Candidate Verification Status | MCP Registration Verified | MCP Server Status |
|---|---|---|---|---|---|---|---|---|---|
| `arm_a_native` | `cand4_2` | `native` | `native` | `null` | `null` | `null` | `NOT_APPLICABLE` | `false` | `DISABLED` |
| `arm_b_cand3_always_on` | `cand4_2` | `cand3` | `cand3` | `.../candidate-phase3/engram.exe` | `97C5EC...A777` | `97C5EC...A777` | `VERIFIED` | `true` | `ACTIVE` |
| `arm_c_cand4_always_on` | `cand4_2` | `cand4_2` | `cand4_2` | `.../candidate-phase4.2/engram.exe` | `78B848...9DD2` | `78B848...9DD2` | `VERIFIED` | `true` | `ACTIVE` |
| `arm_d_cand4_adaptive` | `cand4_2` | `cand4_2` | `cand4_2` | `.../candidate-phase4.2/engram.exe` | `78B848...9DD2` | `78B848...9DD2` | `VERIFIED` | `true` | `ACTIVE` / `ADAPTIVE_BYPASS` |

---

## 5. Source Files Changed

1. **`benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json`**
   - Added `"benchmark_candidate_version": { "type": ["string", "null"] }` under `isolation.properties`.
   - Preserves backward compatibility: `required` list remains unchanged, permitting historical telemetry without modification.

2. **`benchmarks/matched-pair-harness/src/orchestrator.py`**
   - Defined immutable `@dataclass(frozen=True) class ArmCandidateIdentity` with:
     - `validate_against_ledger(ledger)`: strictly checks arm configuration, candidate version, binary path, and expected hash against `CANDIDATE_LEDGER`.
     - `verify_binary_integrity()`: asserts physical binary existence and verifies actual SHA-256 against expected hash fail-closed.
   - Defined `audit_arm_candidate_identity(telemetry_record)`: forensic analysis helper for downstream telemetry inspection and defect detection.
   - In `Orchestrator.__init__`:
     - Populated `self.arm_identities: Dict[str, ArmCandidateIdentity]` covering all native, candidate-3, candidate-4, candidate-4.1, and candidate-4.2 arms.
     - Validated all identities against `CANDIDATE_LEDGER` upon orchestrator instantiation.
   - In `_create_skipped_arm_record`:
     - Populated `benchmark_candidate_version`, `requested_candidate_version`, `resolved_candidate_version`, and `candidate_verification_status` (`"SKIPPED"` or `"NOT_APPLICABLE"`).
   - In `_execute_single_arm`:
     - Pre-launch verification calls `arm_identity.verify_binary_integrity()`.
     - Serialized telemetry strictly pulls from `arm_identity` rather than the global `self.resolved_candidate_version`.
   - In `generate_tetrad_markdown_report`:
     - Dynamically resolves candidate label (`Candidate 3`, `Candidate 4.2`, `Native (None)`) from the arm's `resolved_candidate_version`.

3. **`benchmarks/matched-pair-harness/tests/test_per_arm_candidate_identity.py`**
   - New dedicated test suite implementing Tests A through J.

---

## 6. Telemetry Schema Changes

Under `isolation.properties` in `benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json`:
```json
        "engram_binary_path": { "type": ["string", "null"] },
        "benchmark_candidate_version": { "type": ["string", "null"] },
        "requested_candidate_version": { "type": ["string", "null"] },
        "resolved_candidate_version": { "type": ["string", "null"] },
        "candidate_verification_status": { "type": ["string", "null"] },
        "mcp_registration_verified": { "type": ["boolean", "null"] },
```

---

## 7. Regression Tests & Execution Evidence

All regression tests were executed offline in the Engram workspace (`c:\Users\Amal P S\OneDrive\Projects\Engram`).

### 7.1 New Per-Arm Identity Regression Suite (Tests A - J)
**Command:** `python -u benchmarks/matched-pair-harness/tests/test_per_arm_candidate_identity.py`  
**Exit Code:** `0`  
**Duration:** `187.590s`  
**Result:** `Ran 10 tests in 187.590s - OK`

| Test Case | Scenario Description | Status | Evidence |
|---|---|---|---|
| **Test A** | Native arm candidate identity | **PASS** | `requested="native"`, `resolved="native"`, `binary=None`, `hash=None`, `mcp=False`, `status="NOT_APPLICABLE"` |
| **Test B** | Candidate 3 arm candidate identity | **PASS** | `benchmark="cand4_2"`, `requested="cand3"`, `resolved="cand3"`, `bin=cand3_bin`, `sha=97C5EC...`, `status="VERIFIED"` |
| **Test C** | Candidate 4.2 always-on arm identity | **PASS** | `benchmark="cand4_2"`, `requested="cand4_2"`, `resolved="cand4_2"`, `bin=cand4_2_bin`, `sha=78B848...`, `status="VERIFIED"` |
| **Test D** | Candidate 4.2 adaptive arm identity | **PASS** | `benchmark="cand4_2"`, `requested="cand4_2"`, `resolved="cand4_2"`, `bin=cand4_2_bin`, `sha=78B848...`, adaptive metadata retained |
| **Test E** | Binary identity mismatch fails closed | **PASS** | Corrupted bytes raise `SafetyViolationError`; telemetry reports `"FAILED"`, never `"VERIFIED"` |
| **Test F** | Path/version inconsistency rejected | **PASS** | Mismatched ledger pairings raise `SafetyViolationError` fail-closed |
| **Test G** | Missing candidate binary fails closed | **PASS** | Missing binary raises `FileNotFoundError` fail-closed |
| **Test H** | Schema validation across all 4 arms | **PASS** | Emitted dry-run and skipped records for Arms A-D strictly satisfy `run_telemetry.schema.json` |
| **Test I** | Historical telemetry compatibility | **PASS** | Phase 5.4.7/5.4.8/5.4.9/5.4.10 records validate; files not mutated; missing fields exposed as `None`; simulated defect flagged |
| **Test J** | Four-arm integration offline tetrad | **PASS** | Simultaneous tetrad execution verifies all 4 identities, generates summary, and outputs dynamic Markdown report |

### 7.2 Existing Regression Suites

| Suite Name | Command | Tests Run | Failures | Errors | Duration | Status |
|---|---|---|---|---|---|---|
| Candidate Selection | `python benchmarks/.../test_candidate_selection.py` | 8 | 0 | 0 | 0.280s | **PASS** |
| Telemetry Semantics | `python benchmarks/.../test_telemetry_semantics.py` | 4 | 0 | 0 | 0.477s | **PASS** |
| Long-Path Evaluator | `python benchmarks/.../test_longpath_evaluator.py` | 4 | 0 | 0 | 2.060s | **PASS** |
| Cand 4.2 Offline Cert | `python benchmarks/.../test_candidate_4_2_offline_certification.py` | 10 | 0 | 0 | 6.018s | **PASS** |
| Task B Budget Sim | `python benchmarks/.../test_task_b_budget_simulation.py` | 3 | 0 | 0 | 0.027s | **PASS** |
| Arm C TS2554 Repro | `python benchmarks/.../test_arm_c_ts2554_reproduction.py` | 3 | 0 | 0 | 0.001s | **PASS** |
| Tetrad Harness | `python benchmarks/.../test_tetrad_harness.py` | 1 | 0 | 0 | 44.928s | **PASS** |
| Adaptive Activation | `python benchmarks/.../test_adaptive_activation.py` | 12 | 0 | 0 | 59.524s | **PASS** |
| **TOTAL** | **8 Suites** | **45** | **0** | **0** | **113.315s** | **ALL PASS** |

Combined with the 10 new identity tests: **55 total tests executed with 100% pass rate**.

---

## 8. Historical Telemetry Compatibility Findings

1. **Non-Mutation of Evidence:** Historical telemetry records in `benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_tetrad_live_01/` were verified before and after test execution via SHA-256 comparison. No byte modifications occurred.
2. **Schema Validity:** Historical Phase 5.4.7 records strictly validate against `run_telemetry.schema.json` without requiring retroactive patching.
3. **Graceful Handling of Legacy Telemetry:** In historical records where `benchmark_candidate_version`, `requested_candidate_version`, and `resolved_candidate_version` do not exist, `audit_arm_candidate_identity` flags the record as `is_legacy: True` and exposes these fields as `None` rather than inventing data.
4. **Defect Detection:** When fed a telemetry record with the historical metadata defect (`resolved_candidate_version="cand4_2"` while actual SHA-256 matches Candidate 3), `audit_arm_candidate_identity` flags `is_consistent: False` and outputs:
   `"Metadata defect: arm 'arm_b_cand3_always_on' resolved_candidate_version='cand4_2', but actual binary is Candidate 3 (97C5EC...)"`.

---

## 9. Candidate Binary Integrity Verification

All candidate binaries were independently hashed and measured:

| Candidate | Expected SHA-256 | Actual SHA-256 | Expected Size | Actual Size | Status |
|---|---|---|---|---|---|
| **Candidate 3** | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | 18,668,032 B | 18,668,032 B | **VERIFIED** |
| **Candidate 4** | `FC605BC0CF3B6C9C5FDB1E7C83EE666FD7E049FD9AE25C893D478578681699F8` | `FC605BC0CF3B6C9C5FDB1E7C83EE666FD7E049FD9AE25C893D478578681699F8` | 18,724,864 B | 18,724,864 B | **VERIFIED** |
| **Candidate 4.1** | `300ACE35A31E8E4AF6DD531E8B83B0A5BA9F03E70B27F5E7823E3650CE494015` | `300ACE35A31E8E4AF6DD531E8B83B0A5BA9F03E70B27F5E7823E3650CE494015` | 18,801,152 B | 18,801,152 B | **VERIFIED** |
| **Candidate 4.2** | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | 18,802,176 B | 18,802,176 B | **VERIFIED** |

- Golden repository commit remains: `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.
- Task B fixture hash remains: `cd55db0dfdb5758275b262b8b330858e9673533aa6faa182897edae83d69a2b7`.
- Zero candidate binaries were rebuilt or overwritten.

---

## 10. Git Working-Tree Status

```
Baseline Commit: 7106a2f369fe79faf5f95449378a900ab8e8eddf
Branch: feature/phase2-stage5-remediation
Modified files in working tree:
 M benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json
 M benchmarks/matched-pair-harness/src/orchestrator.py
(All pre-existing user modifications in working tree preserved intact)
Untracked files added:
?? benchmarks/matched-pair-harness/tests/test_per_arm_candidate_identity.py
?? PHASE_5_4_11_PER_ARM_CANDIDATE_IDENTITY_CORRECTION_REPORT.md
```
Zero commits, pushes, merges, or tags were created.

---

## 11. Remaining Defects and Uncertainties

1. **Live Task B Execution Remains Pending:** The harness is fully certified and offline regression verification has passed. Live dispatch has not occurred and requires separate explicit authorization.
2. **Historical Telemetry Defect Remains Preserved as Evidence:** Historical Phase 5.4.7 records preserve their original records intact; downstream analytics must use `audit_arm_candidate_identity` or `engram_binary_actual_sha256` when evaluating historical runs.

---

## 12. Final Acceptance Verdict

| Acceptance Criterion | Result | Status |
|---|---|---|
| Benchmark-level candidate selection distinct from per-arm identity | `benchmark_candidate_version` separated from `requested` & `resolved` | **VERIFIED** |
| Arm B correctly records Candidate 3 | `requested="cand3"`, `resolved="cand3"`, `sha=97C5EC...` | **VERIFIED** |
| Arms C & D correctly record Candidate 4.2 | `requested="cand4_2"`, `resolved="cand4_2"`, `sha=78B848...` | **VERIFIED** |
| Arm A correctly records native/no-binary status | `requested="native"`, `resolved="native"`, `binary=null`, `mcp=false` | **VERIFIED** |
| Version, path, and hash inconsistencies fail closed | Tested via Tests E, F, G; raises `SafetyViolationError` / `FileNotFoundError` | **VERIFIED** |
| All required offline regression tests pass | 55/55 tests passed (100% pass rate) across 9 test suites | **VERIFIED** |
| Historical artifacts remain unchanged and readable | Unmutated; validate against schema cleanly | **VERIFIED** |
| All candidate hashes remain unchanged | Bitwise SHA-256 and byte sizes verified intact | **VERIFIED** |
| Zero live model dispatches occurred | Purely offline deterministic verification | **VERIFIED** |

### Verdict:
`READY FOR EXPLICIT LIVE TASK B AUTHORIZATION`

The matched-pair benchmark harness now guarantees that every arm's telemetry accurately represents the executable that actually ran.
