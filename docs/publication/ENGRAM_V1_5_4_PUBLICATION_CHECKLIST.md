# Engram v1.5.4 Candidate 5 Publication Execution Checklist

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**GitHub Target:** `code-amalps/Engram`  
**Target Commit:** `2df1cac683435314ffd7ad52369e6d20198fae0d`  
**Release Tag:** `v1.5.4-cand5`  
**Proposed Distribution Asset:** `releases/engram-preview-v1.5.4-cand5-clean-windows-x64.zip`  
**Asset Size:** `4303797` bytes | **Asset SHA-256:** `CDD34117E217C44DE0C0F2BD860C04A64350EAEF500A4FC1B39FCABE9B85FF2E`  
**Option B Executable SHA-256:** `1C5DD68B499298ACE1F6BFFBB4028951DD30C63500604CB355CC0A307E09AD38`  
**Historical Certified Archive:** `releases/engram-preview-v1.5.4-cand5-windows-x64.zip` (`7C355109...`)  
**Historical Option A Executable:** `E4E53216...`  
**Execution Gate:** Owner Authorization Granted for GitHub Release Execution.

---

## 1. Classification of Evidentiary Statements

| Category | Description | Verification State |
| :--- | :--- | :--- |
| **[FACT-VERIFIED]** | Independently rechecked and computed during Phase 6.4.3 & 6.5 | Archive Size (`4303797` bytes), Archive SHA-256 (`CDD34117...`), Executable SHA-256 (`1C5DD68B...`), Git HEAD `2df1cac`, remote synchronization, zero archive secrets, tag collision checks |
| **[FACT-REPORTED]** | Certified by prior regression and test executions | 235 heterogeneous validation assertions passed, 8 actual headless MCP scenarios, 5 security scenarios, clean build provenance |
| **[DISCLOSURE]** | Required transparency boundaries on empirical methodology | `SYNTHETIC_SIMULATED_USAGE` marker, zero live LLM billing savings claimed, predefined simulation action sequences |
| **[OWNER-AUTHORIZED]** | Owner authorization explicit grant | Explicitly granted for GitHub release creation, creation of tag `v1.5.4-cand5`, and asset upload |

---

## 2. Pre-Publication Verification Matrix

- [x] **[FACT-VERIFIED] Git Target Reached:** Local HEAD equals remote `origin/feature/phase2-stage5-remediation` at `2df1cac683435314ffd7ad52369e6d20198fae0d`.
- [x] **[FACT-VERIFIED] Distribution Archive Exists & Intact:** `releases/engram-preview-v1.5.4-cand5-clean-windows-x64.zip` exists; `testzip()` passed with 0 corruptions.
- [x] **[FACT-VERIFIED] Distribution Archive Size Matches:** Exact byte count `4303797` verified.
- [x] **[FACT-VERIFIED] Distribution Archive Hash Matches:** SHA-256 `CDD34117E217C44DE0C0F2BD860C04A64350EAEF500A4FC1B39FCABE9B85FF2E` verified.
- [x] **[FACT-VERIFIED] Executable Hash Matches:** Packaged `bin/engram.exe` SHA-256 `1C5DD68B499298ACE1F6BFFBB4028951DD30C63500604CB355CC0A307E09AD38` verified.
- [x] **[FACT-VERIFIED] Binary Provenance Reconciled:** PE section diffing proved 0 byte difference across `.text`, `.data`, `.pdata`, `.reloc`. 24 bytes diff isolated to linker timestamp and random PDB GUID.
- [x] **[FACT-VERIFIED] Payload Inventory Intact:** 14 payload files inside archive match `SHA256SUMS.txt` hashes 100%.
- [x] **[FACT-VERIFIED] Zero Archive Secrets:** Regex scan across all archive files found 0 credentials, tokens, or private keys.
- [x] **[FACT-VERIFIED] License Present:** Root `LICENSE` contains GNU AGPL-3.0; `Cargo.toml` specifies `AGPL-3.0-or-later`.
- [x] **[FACT-VERIFIED] Security Disclosure Present:** `docs/security-and-privacy.md` documents zero outbound telemetry and local subprocessing.
- [x] **[FACT-VERIFIED] Historical Preservation:** Historical Option A archive (`7C355109...`) and binary (`E4E53216...`) remain preserved and untouched in `releases/`.
- [x] **[FACT-VERIFIED] Tag Disambiguation Checked:** Local and remote tags queried. `v1.5.4-cand5` does not exist locally or remotely. Existing remote tag `v1.5.4` preserved untouched.
- [x] **[FACT-VERIFIED] GitHub Credentials Verified:** Git Credential Manager provides OAuth token with full `repo` and admin permissions on `code-amalps/Engram`.

---

## 3. Publication Execution Protocol

1. **Create Annotated Git Tag:**
   ```bash
   git tag -a v1.5.4-cand5 2df1cac683435314ffd7ad52369e6d20198fae0d -m "Engram v1.5.4 Candidate 5 Windows x64 Developer Preview"
   ```
2. **Push Git Tag to Origin:**
   ```bash
   git push origin v1.5.4-cand5
   ```
3. **Publish GitHub Release via API:**
   Target commit `2df1cac683435314ffd7ad52369e6d20198fae0d`, tag `v1.5.4-cand5`, title `Engram v1.5.4 Candidate 5`, prerelease `true`.
4. **Upload Release Assets:**
   - `engram-preview-v1.5.4-cand5-clean-windows-x64.zip`
   - `engram-preview-v1.5.4-cand5-clean-windows-x64.zip.sha256`
   - `SHA256SUMS-v1.5.4-cand5.txt`
5. **Post-Publication Verification:**
   - Verify tag resolves to `2df1cac`.
   - Download uploaded asset and verify SHA-256 independently.
