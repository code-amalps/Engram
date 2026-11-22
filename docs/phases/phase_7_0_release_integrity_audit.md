# Phase 7.0: Release Integrity Audit Report

**Audit Execution Date:** 2026-11-22T23:16:15+05:30  
**Target Release:** `v1.5.4-cand5`  
**GitHub Release Target:** `https://github.com/code-amalps/Engram/releases/tag/v1.5.4-cand5`  
**Audit Mode:** Independent Read-Only Integrity Verification  

---

## 1. Release Identity & Target Resolution

| Audit Parameter | Expected Target | Verified Actual Value | Status |
| :--- | :--- | :--- | :---: |
| **Release Tag** | `v1.5.4-cand5` | `v1.5.4-cand5` | **VERIFIED** |
| **Annotated Tag Commit** | `2df1cac683435314ffd7ad52369e6d20198fae0d` | `2df1cac683435314ffd7ad52369e6d20198fae0d` | **VERIFIED** |
| **Remote Tag Peeled Commit** | `2df1cac683435314ffd7ad52369e6d20198fae0d` | `2df1cac683435314ffd7ad52369e6d20198fae0d` | **VERIFIED** |
| **GitHub Release Title** | `Engram v1.5.4 Candidate 5` | `Engram v1.5.4 Candidate 5` | **VERIFIED** |
| **Release Classification** | Pre-release (`prerelease: true`) | `prerelease: true`, `draft: false` | **VERIFIED** |
| **Target Platform Statement** | Windows x64 (AMD64) | Explicitly documented in release body | **VERIFIED** |
| **Benchmark Disclosure** | `SYNTHETIC_SIMULATED_USAGE` | Prominently stated in release body | **VERIFIED** |
| **Single-Turn Limitation** | Workload F (+0.71% overhead) | Prominently disclosed in release body | **VERIFIED** |

---

## 2. Remote & Local Release Asset Verification

All 3 published release assets were audited against the GitHub API and verified locally:

| Asset Name | Expected Size | Actual Size | Expected SHA-256 | Verified Actual SHA-256 |
| :--- | :---: | :---: | :--- | :--- |
| `engram-preview-v1.5.4-cand5-clean-windows-x64.zip` | 4,303,797 B | 4,303,797 B | `CDD34117E217C44DE0C0F2BD860C04A64350EAEF500A4FC1B39FCABE9B85FF2E` | `CDD34117E217C44DE0C0F2BD860C04A64350EAEF500A4FC1B39FCABE9B85FF2E` |
| `engram-preview-v1.5.4-cand5-clean-windows-x64.zip.sha256` | 116 B | 116 B | `3214C7C2...` | Verified byte-for-byte |
| `SHA256SUMS-v1.5.4-cand5.txt` | 442 B | 442 B | `25BA117A...` | Verified byte-for-byte |

---

## 3. Embedded Binary Metadata Audit

The binary extracted from the certified release ZIP was inspected:
- **Executable Path:** `releases/engram-preview-v1.5.4-cand5-clean-windows-x64/bin/engram.exe`
- **Reported CLI Version:** `engram 1.5.4`
- **Embedded Git Commit:** `commit: 625461beb76d1b87027f9687e2fa010ee2973f2c`
- **Embedded Dirty State:** `dirty: false`
- **Embedded Compiler:** `rustc: rustc 1.99.0 (b940084d7 2026-09-28)`
- **Embedded Cargo Profile:** `profile: release`

---

## 4. Preservation of Legacy Tag & Historical Reference Binary

- **Legacy Tag `v1.5.4`:** Verified remotely at `6608e96d...` / peeled `a75a2c6e2c02acf7af578fd4c5f0b2751efd5988`. Untouched.
- **Historical Option A Archive:** Preserved in `releases/engram-preview-v1.5.4-cand5-windows-x64.zip` (SHA-256: `7C355109EEB8BE7B08301080FD660DE4A4A23DD12AFE89F008546C32FB3C1741`).
- **Historical Option A Executable:** Preserved in `releases/engram-preview-v1.5.4-cand5-windows-x64/bin/engram.exe` (SHA-256: `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44`).

---

## 5. Audit Verdict

### `RELEASE_INTEGRITY_VERIFIED_AND_IMMUTABLE`

The published v1.5.4 Candidate 5 release artifacts on GitHub match the local certified archives and executable bit-for-bit.
