# Engram v1.5.4 Candidate 5 Publication Execution Report

**Repository:** `code-amalps/Engram`  
**Execution Date:** 2026-11-22  
**Final Verdict:** `RELEASE_PUBLISHED_WITH_DOCUMENTED_LIMITATIONS`  

---

## A. Publication Identity

- **Repository:** `https://github.com/code-amalps/Engram`
- **Release Tag:** `v1.5.4-cand5`
- **Peeled Tag Target Commit:** `2df1cac683435314ffd7ad52369e6d20198fae0d`
- **Release Branch:** `feature/phase2-stage5-remediation`
- **Release Title:** `Engram v1.5.4 Candidate 5`
- **Release URL:** `https://github.com/code-amalps/Engram/releases/tag/v1.5.4-cand5`
- **Publication Timestamp:** `2026-11-22T22:50:35+05:30` (`2026-11-22T17:20:35Z`)
- **Release Classification:** Experimental Developer Preview (`prerelease: true`, `draft: false`)

---

## B. Binary Provenance & Resolution of Hash Discrepancy

### 1. Selected Binary Specifications
- **Selected Distribution Binary:** Option B (Clean Worktree Build)
- **Packaged Executable:** `bin/engram.exe` (inside clean ZIP)
- **Executable Size:** `18,883,584` bytes
- **Executable SHA-256:** `1C5DD68B499298ACE1F6BFFBB4028951DD30C63500604CB355CC0A307E09AD38`
- **Embedded Git Commit:** `625461beb76d1b87027f9687e2fa010ee2973f2c`
- **Embedded Dirty Flag:** `false` (Pristine clean worktree build)
- **Rust Toolchain:** `rustc 1.99.0 (b940084d7 2026-09-28)`
- **Build Profile:** `release` (`opt-level = 3`, `lto = true`, `codegen-units = 1`, `panic = "abort"`)
- **Target Triple:** `x86_64-pc-windows-msvc`

### 2. Forensic Investigation of the Option B Hash Discrepancy
The hash divergence between the previously reported Option B build (`3A43F98F...` / `8EA5ED6E...`) and the final packaged executable (`1C5DD68B...`) was forensically analyzed down to the individual PE section level:

```
PE Section Table Comparison (18,883,584 total bytes):
  Section .text   (Executable Code)  : Size =  7,760,896 B | Match = True  | Diff = 0 bytes (100% BIT-FOR-BIT IDENTICAL)
  Section .data   (Initialized Data) : Size =     59,904 B | Match = True  | Diff = 0 bytes (100% BIT-FOR-BIT IDENTICAL)
  Section .pdata  (Exception Info)   : Size =    251,904 B | Match = True  | Diff = 0 bytes (100% BIT-FOR-BIT IDENTICAL)
  Section .reloc  (Relocations)      : Size =     29,696 B | Match = True  | Diff = 0 bytes (100% BIT-FOR-BIT IDENTICAL)
  Section .rdata  (Read-Only Data)   : Size = 10,780,160 B | Match = False | Diff = 22 bytes (Debug timestamps & PDB GUID)
  COFF Header     (IMAGE_FILE_HEADER): Size =        136 B | Match = False | Diff = 2 bytes (TimeDateStamp at offset 0x108)
```

**Root Cause:** MSVC linker (`link.exe`) default non-determinism embeds a 32-bit linker timestamp into the COFF header and generates a random 128-bit (16-byte) CodeView PDB 7.0 GUID into the debug directory unless `/BREPRO` is explicitly supplied. Exactly **24 bytes out of 18,883,584 bytes** differed. All 7.76 MB of compiled x86-64 machine instructions and all application data are bit-for-bit identical.

---

## C. Archive Integrity

- **Archive Filename:** `engram-preview-v1.5.4-cand5-clean-windows-x64.zip`
- **Archive Size:** `4,303,797` bytes
- **Archive SHA-256:** `CDD34117E217C44DE0C0F2BD860C04A64350EAEF500A4FC1B39FCABE9B85FF2E`
- **ZIP CRC-32 Integrity:** Verified via `testzip()` with 0 errors.
- **Inventory Count:** 16 total files (14 payload files + 2 metadata files).
- **Internal Checksums:** All 14 payload items match `SHA256SUMS.txt` 100%.
- **Secret Scanning:** 0 tokens, private keys, or credentials detected across all files.
- **Fresh Extraction:** Verified in an isolated temporary directory; byte-for-byte payload match.
- **Historical Option A Archive:** Preserved unmodified in `releases/engram-preview-v1.5.4-cand5-windows-x64.zip` (`7C355109...`, Option A executable `E4E53216...`).

---

## D. GitHub Verification

All 10 post-publication verification checks were performed and passed:

| Item | Verification Target | Verified Remote Evidence | Result |
| :---: | :--- | :--- | :---: |
| **1** | Remote Git Tag Exists | `refs/tags/v1.5.4-cand5` exists on `origin` | **PASS** |
| **2** | Tag Resolves to Expected Commit | `refs/tags/v1.5.4-cand5^{}` points to `2df1cac683435314ffd7ad52369e6d20198fae0d` | **PASS** |
| **3** | GitHub Release Exists | Release ID `454614885` retrieved from GitHub API | **PASS** |
| **4** | Release Title & Tag Match | Name: `Engram v1.5.4 Candidate 5`, Tag: `v1.5.4-cand5` | **PASS** |
| **5** | Classification & Notes | `prerelease: true`, `draft: false`, comprehensive notes | **PASS** |
| **6** | Assets Attached | 3 assets attached (ZIP archive, `.sha256`, `SHA256SUMS.txt`) | **PASS** |
| **7** | Uploaded Asset Size | `4,303,797` bytes matches local archive byte-for-byte | **PASS** |
| **8** | Downloaded Asset SHA-256 | Downloaded from GitHub and hashed: `CDD34117E217C44DE0C0F2BD860C04A64350EAEF500A4FC1B39FCABE9B85FF2E` | **PASS** |
| **9** | Checksum Instructions | PowerShell `Get-FileHash` commands verified | **PASS** |
| **10** | Existing Tag Preservation | `refs/tags/v1.5.4` on origin remains unchanged at `6608e96...` / peeled `a75a2c6...` | **PASS** |
| **11** | Public Web Accessibility | `https://github.com/code-amalps/Engram/releases/tag/v1.5.4-cand5` returns HTTP 200 | **PASS** |

---

## E. Compliance & Legal Disclosures

1. **License:** GNU Affero General Public License v3.0 or later (`AGPL-3.0-or-later`). Root `LICENSE` present.
2. **Source Correspondence:** Complete Corresponding Source is public on GitHub at tag `v1.5.4-cand5` (`2df1cac683435314ffd7ad52369e6d20198fae0d`). Forensic audit established 100% blob correspondence across all 88 source and test files.
3. **Legal Review Status:** `LEGAL_REVIEW_PENDING`. Technical source correspondence has been established. Formal review by qualified legal counsel remains pending.
4. **Synthetic Benchmark Limitations:**
   - Benchmark data is derived from deterministic offline simulation under the `SYNTHETIC_SIMULATED_USAGE` test harness.
   - Zero live LLM billing savings or latency reductions are claimed.
   - Single-turn workflows (Workload F) carry a documented +0.71% synthetic token overhead.
5. **Platform Scope:** Certified exclusively for Windows x64 AMD64 (`x86_64-pc-windows-msvc`).
6. **Validation Scope:** The reported 235 passing checks represent heterogeneous test assertions, runtime harness checks, headless MCP scenarios, and security boundaries, not 235 unit tests.

---

## F. Final Verdict

### `RELEASE_PUBLISHED_WITH_DOCUMENTED_LIMITATIONS`

The Engram v1.5.4 Candidate 5 Windows x64 Developer Preview has been successfully published to GitHub with complete cryptographic integrity, reproducible clean-build provenance, and thorough empirical disclosures.
