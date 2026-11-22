# Engram v1.5.4 Candidate 5 Final Release Package Certification Report

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**GitHub Upstream:** `code-amalps/Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Repository HEAD:** `2df1cac683435314ffd7ad52369e6d20198fae0d`  
**Phase:** 6.4.3 Final Candidate 5 Clean-Binary Packaging & Release Asset Certification  
**Final Verdict:** `FINAL_OPTION_B_PACKAGE_CERTIFIED_PENDING_LEGAL_REVIEW`  
**Execution Mode:** Controlled Local Packaging & Certification (Zero Remote Publication)  

---

## A. Identity

- **Repository HEAD:** `2df1cac683435314ffd7ad52369e6d20198fae0d`
- **Git Branch:** `feature/phase2-stage5-remediation` (synchronized with `origin`)
- **Proposed Release Tag:** `v1.5.4-cand5` (0 collisions locally and remotely)
- **Product Version:** `1.5.4`
- **Candidate Identity:** `Candidate 5`
- **Target Platform:** Windows x64 (AMD64, `x86_64-pc-windows-msvc`)
- **Distribution Binary:** Option B (Clean Worktree Build)
- **Binary Embedded Commit:** `625461beb76d1b87027f9687e2fa010ee2973f2c`
- **Binary Embedded Dirty State:** `false`
- **Rust Toolchain:** `rustc 1.99.0 (b940084d7 2026-09-28)`
- **Build Profile:** `release` (`opt-level = 3`, `lto = true`, `codegen-units = 1`, `panic = "abort"`)

---

## B. Historical Artifact Preservation

The historical benchmark binary (Option A) and original certified release archive from Phase 6.0/6.1 have been preserved unmodified:

| Historical Artifact | File Location | Expected SHA-256 | Verified Actual SHA-256 | Status |
| :--- | :--- | :--- | :--- | :---: |
| **Option A Binary** | `releases/engram-preview-v1.5.4-cand5-windows-x64/bin/engram.exe` | `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` | `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` | **UNMODIFIED** |
| **Certified Archive** | `releases/engram-preview-v1.5.4-cand5-windows-x64.zip` | `7C355109EEB8BE7B08301080FD660DE4A4A23DD12AFE89F008546C32FB3C1741` | `7C355109EEB8BE7B08301080FD660DE4A4A23DD12AFE89F008546C32FB3C1741` | **UNMODIFIED** |

Both historical artifacts remain intact in the repository to maintain 100% cryptographic continuity with the Phase 5.9.2.5 empirical benchmark results and Phase 5.9.2.6 independent audit certification.

---

## C. New Package Integrity (Option B Clean Package)

The final Option B release candidate was assembled into an independent staging directory and packaged into a fresh distribution archive:

- **New Archive Filename:** `engram-preview-v1.5.4-cand5-clean-windows-x64.zip`
- **New Archive Relative Path:** `releases/engram-preview-v1.5.4-cand5-clean-windows-x64.zip`
- **New Archive Size:** `4303797` bytes
- **New Archive SHA-256:** `CDD34117E217C44DE0C0F2BD860C04A64350EAEF500A4FC1B39FCABE9B85FF2E`
- **Packaged Executable SHA-256:** `1C5DD68B499298ACE1F6BFFBB4028951DD30C63500604CB355CC0A307E09AD38`
- **Total Archive Files:** 16 files
- **Payload Files:** 14 files
- **Metadata Files:** 2 files (`RELEASE_MANIFEST.json`, `SHA256SUMS.txt`)
- **Archive CRC Integrity:** PASSED (`testzip()` reported 0 corruptions)
- **Fresh Extraction Test:** PASSED (extracted into fresh isolated directory with 100% byte-for-byte match to staged payload)

### Complete Payload Inventory & Cryptographic Manifest:
| Relative Payload Path | Size (Bytes) | SHA-256 Checksum |
| :--- | :--- | :--- |
| `BENCHMARK_DISCLOSURE.md` | 1,489 | `2A7FFB5DAA76743A042FF13768B2BDBC8DBB754E8626AC390A0C57A256E1AC96` |
| `INSTALLATION.md` | 2,027 | `547A8CEDAC235C2BC827BC426309B730E9AA3F62BCDD93C88E3288BEE81E3E04` |
| `KNOWN_LIMITATIONS.md` | 3,248 | `E9C6032CC58F3EC8607D12BCCEC524284716737B2AB869C05CBA84B9AA1DA46B` |
| `RELEASE_NOTES.md` | 2,217 | `6A0F111E7385EC98BECE1B8E2BE786F9C7D4D73EE03DA965DF630C1FA7FA9130` |
| `USAGE.md` | 1,849 | `9ED805C25D97024C069DF54BFAA3D015E2F9787F42B6C79411C9BA8A5743521D` |
| `bin/engram.exe` | 18,883,584 | `1C5DD68B499298ACE1F6BFFBB4028951DD30C63500604CB355CC0A307E09AD38` |
| `config/config.example.yaml` | 289 | `71B2B7414D484FFD48ABA7300BA151209A00D27F55A01CBBFF2A24E26C0BA9B6` |
| `config/constitution.example.yaml` | 355 | `1353A915F64C28D427F01DC99C2FF74589B0F1C126D04EE7510EA7FFBA88CF42` |
| `sample_workspace/.engram/config.yaml` | 103 | `10859B0A8B6CDB8DDB70583EFD9D74E5776BDD1BE97CBDB8FAF22B78807C0B8E` |
| `sample_workspace/.engram/constitution.yaml` | 100 | `677E97A2CB9824D4F7A899C18A0DF42E494385D35A62F01865CA52BAC70735C9` |
| `sample_workspace/src/__init__.py` | 0 | `E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855` |
| `sample_workspace/src/calculator.py` | 216 | `6C483D07C8C1D302517EB98D4A0F21C60A27C9D10F02923E2C54D273556F07FC` |
| `sample_workspace/tests/__init__.py` | 0 | `E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855` |
| `sample_workspace/tests/test_calculator.py` | 503 | `EC1181AACCBBB05BBB49552DB51622FEDA70B22C2A73056BEA499492BEC089EA` |

---

## D. Functional Verification

Tests were executed directly against the binary extracted from `engram-preview-v1.5.4-cand5-clean-windows-x64.zip` (`target/verified_extracted_package/bin/engram.exe`).

### 1. Tests Executed in Phase 6.4.3:
| Test Domain | Scenario / Command | Verified Assertion | Result |
| :--- | :--- | :--- | :---: |
| **CLI Version & Metadata** | `engram --version` | `1.5.4`, `commit: 625461b...`, `dirty: false` | **PASS** |
| **MCP Scenario A** | JSON-RPC `initialize` | Server info `engram` v1.5.4, protocol 2024-11-05 | **PASS** |
| **MCP Scenario B** | JSON-RPC `tools/list` | All 16 tools discovered (including consolidated core) | **PASS** |
| **MCP Scenario C** | JSON-RPC `engram_get_context` | Budget packaging with task & token limits (3,155 chars) | **PASS** |
| **MCP Scenario D** | JSON-RPC `engram_inspect_symbol` | Symbol extraction on `Calculator` | **PASS** |
| **MCP Scenario E** | Request Correlation | JSON-RPC IDs 101 and 102 correlated 1:1 | **PASS** |
| **MCP Scenario F** | Method Not Found | Proper JSON-RPC error code `-32601` | **PASS** |
| **MCP Scenario G** | Tool Error Handling | Missing tool returns proper `isError: true` | **PASS** |
| **MCP Scenario H** | Graceful Shutdown | Clean stdin EOF shutdown, exit code 0 | **PASS** |
| **Security & Containment** | 5 Path Traversal Vectors | Directory traversal (`..`) blocked, root contained | **PASS** |

### 2. Historical Regression Results Carried Forward:
- **235 Offline Regression Assertions:** Passed during Phase 6.2/6.3 across Rust unit tests (24), Rust integration tests (37), Python runtime tests (155), and security boundary checks.
- **Deterministic Offline Benchmark Simulator:** 6 synthetic workloads (A through F) evaluated in Phase 5.9.2.5 under `SYNTHETIC_SIMULATED_USAGE`.
- **Benchmark Integrity Audit:** Certified in Phase 5.9.2.6 with zero deviations.

---

## E. Compliance & Provenance Disclosures

1. **AGPL-3.0 Source Correspondence:**
   Complete Corresponding Source is provided in the public Git repository at branch `feature/phase2-stage5-remediation`, anchored at commit `2df1cac683435314ffd7ad52369e6d20198fae0d` (and ancestor commits `3be26e9` and `625461b`). Forensic audit verified 100% bit-for-bit Git blob correspondence across all 88 source and test files.
2. **Binary Provenance Distinction:**
   - **Option B (Distribution Binary):** Stamped with `commit: 625461b... (dirty: false)`. SHA-256: `1C5DD68B...`.
   - **Option A (Historical Benchmark Binary):** Stamped with `commit: 7106a2f... (dirty: true)`. SHA-256: `E4E53216...`.
   - The two binaries share 100% identical functional source code logic. The hash divergence is strictly due to `build.rs` embedding Git commit strings and dirty state into the PE `.rdata` section, and MSVC `link.exe` writing the build timestamp into the COFF header.
3. **Synthetic Benchmark Limitations:**
   Reported token reductions derive from deterministic simulation (`SYNTHETIC_SIMULATED_USAGE`). No actual LLM provider billing savings or latency reductions are claimed.
4. **Platform Limitation:**
   Windows x64 (AMD64) is the only supported platform for this developer preview.
5. **Legal Review Status:**
   Technical source correspondence has been established. Formal review by qualified legal counsel remains pending owner sign-off prior to public distribution.

---

## F. Final Certification Verdict

### Verdict: `FINAL_OPTION_B_PACKAGE_CERTIFIED_PENDING_LEGAL_REVIEW`

The Option B package satisfies all technical requirements for an open-source Developer Preview release under AGPL-3.0. The release archive is packaged, verified, and preserved locally.

---

## G. Final Publication Parameters (For Owner Reference)

- **Final Distribution Archive Path:** `C:\Users\Amal P S\OneDrive\Projects\Engram\releases\engram-preview-v1.5.4-cand5-clean-windows-x64.zip`
- **Final Distribution Archive SHA-256:** `CDD34117E217C44DE0C0F2BD860C04A64350EAEF500A4FC1B39FCABE9B85FF2E`
- **Packaged Executable SHA-256:** `1C5DD68B499298ACE1F6BFFBB4028951DD30C63500604CB355CC0A307E09AD38`
- **Proposed Release Tag:** `v1.5.4-cand5`
- **Target Git Commit:** `2df1cac683435314ffd7ad52369e6d20198fae0d`
- **Outstanding Requirement:** Repository owner legal review sign-off.
