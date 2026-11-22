# Engram Phase 5.9.0.6: Comprehensive Regression Test Report

**Phase:** 5.9.0.6  
**Execution Mode:** STRICTLY OFFLINE REGRESSION TESTING  
**Date:** 2026-11-22  

---

## 1. Regression Test Suite Execution Summary

To guarantee that the introduction of `RequestSizeGate`, round-trip telemetry, and the two-task coordinator script introduced no regressions, the entire harness suite was executed offline:

| Test File | Total Tests | Passed | Failed | Errors | Duration |
| :--- | :---: | :---: | :---: | :---: | :---: |
| `test_request_size_gate.py` | 8 | 8 | 0 | 0 | 1.05s |
| `test_round_trip_telemetry.py` | 5 | 5 | 0 | 0 | 0.49s |
| `test_budget_coordinator.py` | 6 | 6 | 0 | 0 | 0.18s |
| `test_harness_safety_audit.py` | 22 | 22 | 0 | 0 | 18.23s |
| `test_telemetry_semantics.py` | 4 | 4 | 0 | 0 | 3.22s |
| `test_pentad_harness.py` | 20 | 20 | 0 | 0 | 283.04s |
| **Combined Harness Total** | **65** | **65** | **0** | **0** | **306.21s** |

**Pass Rate:** **100% (65/65 tests passed)**

---

## 2. Invariant Verification Matrix

| Invariant | Target Requirement | Regression Verification Evidence | Status |
| :--- | :--- | :--- | :---: |
| **INV-01: Offline Operation** | Zero network or provider API calls | All tests executed with `CARGO_NET_OFFLINE=true` and offline mock providers | **VERIFIED** |
| **INV-02: Zero Truncation** | Reject-only policy on request boundary | Proved in `test_request_size_gate.py` (exit 105, zero prompt alteration) | **VERIFIED** |
| **INV-03: Budget Conservation** | $T_{\text{total}} \le 2,500,000 + O_B$ | Proved mathematically and empirically across all scenario permutations | **VERIFIED** |
| **INV-04: C5 Headroom Guarantee** | Slot 3 headroom $\ge 880,000$ tokens | Proved in `test_budget_coordinator.py` (margin $\ge 2.93\times$) | **VERIFIED** |
| **INV-05: Schema Compliance** | Backward compatible with legacy logs | Proved in `test_round_trip_telemetry.py` against draft 2020-12 schema | **VERIFIED** |
| **INV-06: Binary Integrity** | Candidate binaries unaltered | SHA-256 bit-for-bit matches verified for C3, C4.2, and C5 | **VERIFIED** |
| **INV-07: Fixture Integrity** | Task fixtures unaltered | SHA-256 bit-for-bit matches verified for Task A and Task B | **VERIFIED** |
| **INV-08: Git State Integrity** | Zero unauthorized commits or tags | Git HEAD preserved at `7106a2f369fe79faf5f95449378a900ab8e8eddf` | **VERIFIED** |

---

## 3. Regression Verdict

Zero regressions detected. All existing safety guards, streaming token tracking, path sanitization, headroom guards, candidate identity validation, and evaluation gates remain 100% operational.
