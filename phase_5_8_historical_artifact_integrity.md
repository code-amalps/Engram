# Engram Phase 5.8: Historical Artifact Integrity Audit Report

**Date:** 2026-11-22  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Git HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Auditor:** Adversarial Forensic Audit Team  
**Audit Classification:** `ALL_HISTORICAL_ARTIFACTS_CRYPTOGRAPHICALLY_INTACT`

---

## 1. Executive Summary

To guarantee that benchmark comparisons across historical phases remain scientifically valid and immune to silent drift or artifact tampering, this audit performed cryptographic checksum verifications against all frozen candidate binaries, benchmark configuration fixtures, and toolchain lockfiles.

**Audit Verdict:**  
**100% Bit-for-Bit Integrity Confirmed.** Every frozen candidate binary, historical benchmark task fixture, and project dependency lockfile perfectly matches its canonical pre-registered SHA-256 cryptographic digest. Zero tampering, zero silent mutation, and zero environmental corruption was detected.

---

## 2. Cryptographic Checksum Verification Matrix

| Artifact Identifier | Canonical Expected SHA-256 | Actual Measured SHA-256 | Byte Size | Verification Status |
| :--- | :--- | :--- | :--- | :--- |
| **Candidate 3 Binary** (`bin/candidate-phase3/engram.exe`) | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | 18,512,896 | **VERIFIED MATCH** |
| **Candidate 4 Binary** (`bin/candidate-phase4/engram.exe`) | `787680D1D230FBE5316AC038AE9082F995C26300AEBC8C33EC7E3BE98DAF22EC` | `787680D1D230FBE5316AC038AE9082F995C26300AEBC8C33EC7E3BE98DAF22EC` | 18,824,192 | **VERIFIED MATCH** |
| **Candidate 4.1 Binary** (`bin/candidate-phase4.1/engram.exe`)| `300ACE35A31E8E4AF6DD531E8B83B0A5BA9F03E70B27F5E7823E3650CE494015` | `300ACE35A31E8E4AF6DD531E8B83B0A5BA9F03E70B27F5E7823E3650CE494015` | 18,838,528 | **VERIFIED MATCH** |
| **Candidate 4.2 Binary** (`bin/candidate-phase4.2/engram.exe`)| `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | 18,844,672 | **VERIFIED MATCH** |
| **Task B Fixture** (`config/tasks/task_b_remember_me.json`) | `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` | `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` | 3,923 | **VERIFIED MATCH** |
| **Task A Fixture** (`config/tasks/task_a_typo.json`) | `357D9B4B5777A41A8C5691475DCA8DDAC00134C480C6324E26B60FEA39F95F02` | `357D9B4B5777A41A8C5691475DCA8DDAC00134C480C6324E26B60FEA39F95F02` | 2,874 | **VERIFIED MATCH** |
| **Task C Fixture** (`config/tasks/task_c_theme_toggle.json`) | `47CF388CCBA08A653AFF556983F5C52AE42EE1C4073F235156B54F229E6CDA8B` | `47CF388CCBA08A653AFF556983F5C52AE42EE1C4073F235156B54F229E6CDA8B` | 3,115 | **VERIFIED MATCH** |
| **Cargo Lockfile** (`Cargo.lock`) | `85FF18D67207BEF644D8C84582124D5EA0988983AC64C51CBA8EB6975FE11D39` | `85FF18D67207BEF644D8C84582124D5EA0988983AC64C51CBA8EB6975FE11D39` | 104,821 | **VERIFIED MATCH** |

---

## 3. Forensic Inspection Details

### 3.1 Candidate 3 (Phase 3 Baseline)
- Canonical SHA-256 recorded in Phase 3/4 ledger: `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777`.
- Local path: `benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe`.
- Result: Bit-for-bit identical. The historical v1.5.1 baseline remains untampered.

### 3.2 Candidate 4.2 (Phase 4.2 Baseline)
- Canonical SHA-256 recorded in Phase 4.2 ledger: `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2`.
- Local path: `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe`.
- Result: Bit-for-bit identical. Arms C and D benchmark binaries are verified frozen and intact.

### 3.3 Task B Benchmark Fixture
- Canonical SHA-256: `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7`.
- Local path: `benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json`.
- Result: Bit-for-bit identical. The benchmark prompt, test expectations, and ground-truth validation rules have not been modified.

### 3.4 Toolchain Dependency Lock (`Cargo.lock`)
- Canonical SHA-256: `85FF18D67207BEF644D8C84582124D5EA0988983AC64C51CBA8EB6975FE11D39`.
- Local path: `Cargo.lock`.
- Result: Bit-for-bit identical. No dependencies have been upgraded, downgraded, or replaced.

---

## 4. Audit Conclusion

All historical assets, candidate binaries, and benchmark fixtures in the Engram repository are cryptographically pure and uncorrupted. The comparative baseline against Candidates 3, 4, 4.1, and 4.2 is fully trustworthy.
