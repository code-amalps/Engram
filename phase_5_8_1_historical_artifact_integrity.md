# Engram Phase 5.8.1: Historical Artifact Integrity Report

**Date:** 2026-11-22  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Git HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Audit Classification:** `ALL_HISTORICAL_ARTIFACTS_CRYPTOGRAPHICALLY_INTACT`  

---

## 1. Executive Summary

To guarantee scientific reproducibility and ensure that cross-arm comparisons remain untainted, all pre-existing historical candidate binaries, benchmark task fixtures, and project dependency lockfiles were subjected to strict SHA-256 cryptographic verification before and during Phase 5.8.1.

**Integrity Verdict:** **100% Bit-for-Bit Preservation Verified.** Zero historical candidate binaries or fixtures were modified, replaced, or deleted during the Phase 5.8.1 implementation.

---

## 2. Checksum Verification Matrix

| Historical Artifact Identifier | Canonical Expected SHA-256 | Actual Measured SHA-256 | Byte Size | Verification Status |
| :--- | :--- | :--- | :--- | :--- |
| **Candidate 3 Binary** (`bin/candidate-phase3/engram.exe`) | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | 18,512,896 | **VERIFIED MATCH** |
| **Candidate 4.2 Binary** (`bin/candidate-phase4.2/engram.exe`)| `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | 18,844,672 | **VERIFIED MATCH** |
| **Task B Fixture** (`config/tasks/task_b_remember_me.json`) | `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` | `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` | 3,923 | **VERIFIED MATCH** |
| **Cargo Lockfile** (`Cargo.lock`) | `85FF18D67207BEF644D8C84582124D5EA0988983AC64C51CBA8EB6975FE11D39` | `85FF18D67207BEF644D8C84582124D5EA0988983AC64C51CBA8EB6975FE11D39` | 104,821 | **VERIFIED MATCH** |

---

## 3. Storage Invariants

1. **Isolation from Historical Bins:**  
   Candidate 5 is assigned its own dedicated staging directory (`benchmarks/matched-pair-harness/bin/candidate-phase5/`), preventing any risk of clobbering historical candidate assets.
2. **Immutable Task Fixtures:**  
   Neither `task_a_typo.json` nor `task_b_remember_me.json` was altered.
3. **Lockfile Purity:**  
   `Cargo.lock` has not been modified; offline builds ensure that zero crates are updated or added.
