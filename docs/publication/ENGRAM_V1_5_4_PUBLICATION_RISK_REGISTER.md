# Engram v1.5.4 Candidate 5 Publication Risk Register

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Evaluation Date:** 2026-11-22  
**Target Release:** `v1.5.4-cand5`  
**Classification:** Experimental Developer Preview (Windows x64)  

---

## 1. Risk Register Matrix

| Risk ID | Risk Description | Pre-Mitigation Severity | Post-Mitigation Status | Mitigation & Resolution Evidence |
| :--- | :--- | :---: | :---: | :--- |
| **RSK-01** | **Tag Collision with Existing `v1.5.4` Tag** | **HIGH** | **RESOLVED** | Selected distinct tag `v1.5.4-cand5`. Remote check confirmed zero collisions. Existing `v1.5.4` tag preserved untouched. |
| **RSK-02** | **Option A vs. Option B Provenance Discrepancy** | **HIGH** | **RESOLVED** | Option B clean build (`1C5DD68B...`, `dirty: false`) selected as distribution binary. Option A preserved for historical benchmark reproduction. Section-level PE analysis proved 100% executable machine code match. |
| **RSK-03** | **Linker Timestamp Non-Determinism (3A43... vs 1C5D...)** | **MEDIUM** | **RESOLVED** | PE section diffing proved that out of 18,883,584 bytes, exactly 24 bytes differ (PE `TimeDateStamp` and random CodeView PDB GUID). Executable code in `.text`, `.data`, `.pdata`, and `.reloc` is bit-for-bit identical. |
| **RSK-04** | **Synthetic Benchmark Token Misinterpretation** | **HIGH** | **RESOLVED** | Prominently stamped `SYNTHETIC_SIMULATED_USAGE` in release notes, manifest, and disclosures. Zero live billing/latency savings claimed. Disclosed Workload F +0.71% single-turn overhead. |
| **RSK-05** | **Platform Scope Misunderstanding** | **MEDIUM** | **RESOLVED** | Prominently stated in release title, release notes, and documentation that this package is certified exclusively for Windows x64 AMD64 (`x86_64-pc-windows-msvc`). |
| **RSK-06** | **Legal Review of AGPL-3.0 Source Correspondence** | **MEDIUM** | **DOCUMENTED** | Complete corresponding source committed and pushed to GitHub at release commit `2df1cac`. Formally documented as `LEGAL_REVIEW_PENDING` to clarify that technical source audit does not constitute formal legal counsel advice. |
| **RSK-07** | **Credential or Private Key Leakage in Package** | **HIGH** | **RESOLVED** | Regex secret scanning across all 16 files inside the release ZIP returned 0 matches. Zero API keys, passwords, or tokens in package. |
| **RSK-08** | **Validation Metric Misattribution** | **MEDIUM** | **RESOLVED** | Explicitly documented that the 235 passing validation results are heterogeneous test assertions, runtime harness checks, headless MCP scenarios, and security boundaries, not 235 unit tests. |

---

## 2. Risk Sign-Off Verdict

All identified risks are either fully resolved or documented with clear disclosures and owner authorization. The release package is cleared for GitHub publication.
