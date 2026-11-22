# Binary Selection Integrity Report: N=1 Matched-Pair Pilot

**Milestone:** Engram Amendment 05 — Milestone 5C  
**Document ID:** `amendments/draft-05/milestone-5c/binary-selection-integrity-report.md`  
**Evaluation Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Expected Candidate SHA-256:** `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`  
**Status:** VERIFIED & FAIL-CLOSED  

---

## 1. Executive Summary

This report evaluates the binary selection, path resolution, and cryptographic verification mechanisms in `benchmarks/matched-pair-harness/src/orchestrator.py` and `isolation.py`.

The audit establishes that:
1. `DEFAULT_ENGRAM_BIN` in `orchestrator.py` deterministically resolves to `target/release/engram.exe`.
2. This executable matches the Candidate Phase 1 binary byte-for-byte with SHA-256 `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`.
3. In `_execute_single_arm`, hash validation is performed immediately before sandbox creation and model execution.
4. Any discrepancy, missing binary, or attempted substitution of the frozen v1.5.4 binary fails closed with `IsolationError`.
5. The existing CLI interface is safely preserved without introducing fragile argument aliases.

---

## 2. Binary Path Resolution and Treatment B Mapping

### 2.1 Default Path Resolution
In `orchestrator.py` (`lines 50–55`):
```python
DEFAULT_ENGRAM_BIN = Path(r"C:\Users\Amal P S\OneDrive\Projects\Engram\target\release\engram.exe")
```

### 2.2 Orchestrator Treatment Arm Binding
In `Orchestrator.__init__` (`lines 135–185`):
```python
self.engram_bin = Path(engram_bin) # Defaults to DEFAULT_ENGRAM_BIN
self.expected_treatment_sha256 = expected_treatment_sha256 or FROZEN_ENGRAM_SHA256

self.arm_binaries: Dict[str, Optional[Path]] = {
    "control_A": None,
    "treatment_B": self.engram_bin,
    "baseline_arm1": None,
    "v151_arm2": self.engram_v151_bin,
    "v154_arm3": self.engram_v154_bin,
}
self.arm_expected_hashes: Dict[str, Optional[str]] = {
    "control_A": None,
    "treatment_B": self.expected_treatment_sha256,
    "baseline_arm1": None,
    "v151_arm2": self.expected_v151_sha256,
    "v154_arm3": self.expected_v154_sha256,
}
```
When `--expected-treatment-sha256 F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` is passed, `self.arm_expected_hashes["treatment_B"]` is set to the Candidate Phase 1 hash.

---

## 3. Pre-Execution Cryptographic Enforcement

In `_execute_single_arm` (`lines 265–278`):
```python
binary = self.arm_binaries.get(arm)
expected_sha = self.arm_expected_hashes.get(arm)
is_engram = arm in ("treatment_B", "v151_arm2", "v154_arm3")

if is_engram:
    if not binary or not binary.exists():
        raise FileNotFoundError(f"Engram binary missing for arm '{arm}': {binary}")
    if expected_sha:
        verify_frozen_binary(binary, expected_sha)
```
- Verification occurs **before** sandbox creation (`create_sandbox`), before prompt preparation, and before spawning Codex.
- `verify_frozen_binary` streams the binary in 64 KiB chunks, computes the actual SHA-256 digest, and performs a case-insensitive equality check.
- If hashes diverge, `IsolationError` is raised and execution aborts instantly.
- No fallback logic exists; execution cannot fall back to any alternative binary.

---

## 4. Offline Verification Test Suite Results

A dedicated test suite in `benchmarks/matched-pair-harness/tests/test_milestone_5c_remediation.py` (`TestBinarySelectionIntegrity`) was executed to empirically verify all five security properties:

| Test Case | Scenario Evaluated | Expected Behavior | Observed Result | Status |
|---|---|---|---|---|
| `test_correct_candidate_hash_passes` | Valid candidate binary tested against expected SHA-256 `F56347EF...` | Returns `True` | Returns `True` (0.001s) | **PASSED** |
| `test_incorrect_expected_hash_fails_closed` | Valid binary tested against corrupted hash `0000...` | Raises `IsolationError` with hash mismatch message | Raises `IsolationError` | **PASSED** |
| `test_missing_executable_fails_closed` | Missing file path passed to validator | Raises `IsolationError: Frozen binary not found` | Raises `IsolationError` | **PASSED** |
| `test_wrong_binary_path_fails_closed` | Arbitrary third-party executable tested against candidate hash | Raises `IsolationError` | Raises `IsolationError` | **PASSED** |
| `test_accidental_frozen_v154_selection_fails_closed` | Frozen v1.5.4 binary (`CF807EAC...`) tested against candidate hash (`F56347EF...`) | Raises `IsolationError` (prevents binary confusion) | Raises `IsolationError` | **PASSED** |
| `test_orchestrator_default_binary_mapping` | Verifies `DEFAULT_ENGRAM_BIN` matches absolute path to `target/release/engram.exe` | Path name matches `engram.exe` | Verified | **PASSED** |

---

## 5. Interface Decision: Preserving CLI Grammar

During preflight, it was identified that `--engram-bin` was not defined in `orchestrator.py`'s `argparse`.

### Decision: Preserve Existing Interface
Adding an ad-hoc `--engram-bin` flag solely to mirror older documentation would introduce unnecessary risk and diverge from the frozen CLI grammar. Because:
1. `DEFAULT_ENGRAM_BIN` already defaults to `target/release/engram.exe`,
2. That executable is byte-for-byte identical to `candidate-phase1/engram.exe`, and
3. Pre-execution SHA-256 validation (`--expected-treatment-sha256`) guarantees cryptographic identity,

the orchestrator interface is preserved in its current, proven form.
