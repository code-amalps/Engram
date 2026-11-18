# Phase 5 Candidate Binary Build & Sealing Report

**Repository:** `Engram`
**Branch:** `feature/phase2-stage5-remediation`
**Execution Date:** 2026-11-18
**Profile:** `release`
**Target:** `x86_64-pc-windows-msvc`

---

## 1. Executive Summary

Phase 5 requires producing and sealing a production-grade release binary of the remediated Engram engine (incorporating all work from Phase 4A through Phase 4E), calculating cryptographic SHA-256 hashes, ensuring absolute preservation of the Pristine Baseline Candidate (`candidate-phase1/engram.exe`), and registering Candidate 2 (`candidate-phase2/engram.exe`) into the matched-pair evaluation harness.

Both candidate binaries have been successfully verified, sealed, and fingerprinted.

---

## 2. Compilation & Environment Details

- **Rust Compiler:** `rustc 1.87.0-nightly (07eb78263 2025-02-18)`
- **Cargo Command:** `cargo build --release`
- **Build Duration:** 3 minutes, 25 seconds
- **Compilation Output Path:** `target/release/engram.exe`
- **Compiler Flags:** Optimizations enabled (`opt-level = 3`), LTO enabled.

---

## 3. Cryptographic Verification & Sealing

### Pristine Baseline (Candidate 1)
- **Path:** `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`
- **Status:** **UNMODIFIED & PRESERVED**
- **Size:** 18,285,568 bytes
- **SHA-256 Hash:**
  ```
  F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE
  ```
- **Engine Capabilities:** Pre-remediation engine (Schema v1, legacy AST visitor, unbounded impact graph, no query router, no budget packer).

### Remediated Production Binary (Candidate 2)
- **Path:** `benchmarks/matched-pair-harness/bin/candidate-phase2/engram.exe`
- **Status:** **SEALED & ACTIVE**
- **Size:** 18,667,008 bytes
- **SHA-256 Hash:**
  ```
  A58B57D343502F2F4E9C328F20841527D573259FC9B305D5F006671427FD99DE
  ```
- **Engine Capabilities:** Fully remediated engine (Schema v2 `entity_members`, Tree-sitter AST extraction, workspace-bounded graph, decoupled query router with `AngularNgRxAdapter`, context budget guard with omission manifests, contracted signature generation).

---

## 4. Verification Checksum Manifest

Stored in `benchmarks/matched-pair-harness/reports/PHASE_5_SHA256SUMS.txt`:
```
f56347ef2f0bfd4854176a3ec3c86d8d20b4c0e5954f04e4464998f623d85bee *candidate-phase1/engram.exe
a58b57d343502f2f4e9c328f20841527d573259fc9b305d5f006671427fd99de *candidate-phase2/engram.exe
```

---

## 5. Harness Integration Status

The harness orchestrator (`benchmarks/matched-pair-harness/src/orchestrator.py`) and isolation module (`src/isolation.py`) enforce strict binary hash verification prior to executing treatment arms. Candidate 2 was registered with `--expected-treatment-sha256 a58b57d343502f2f4e9c328f20841527d573259fc9b305d5f006671427fd99de`, satisfying all harness runtime isolation and tamper-prevention assertions.
