# Engram v1.5.4 Candidate 5 Final Owner Approval Decisions Document

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**GitHub Upstream:** `code-amalps/Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**HEAD Commit:** `2df1cac683435314ffd7ad52369e6d20198fae0d`  
**Phase:** 6.4.2 Final Approval Package  
**Execution Status:** Publication Preparation Only — Absolute Publication Stop Enforced  

---

## 1. Executive Summary & Purpose

All technical release engineering, integration validation, forensic source reconstruction, and release packaging preparations for the **Engram v1.5.4 Candidate 5 Developer Preview (Windows x64)** are complete.

In strict compliance with release safety rules, **zero remote publication actions have been taken**. No Git tags have been created, no tags pushed, no GitHub releases created, and no assets uploaded.

The deployment of this release is now formally submitted to the repository owner. Public distribution requires the owner to review the forensic findings and sign off on three specific governance decisions detailed below.

---

## 2. Owner Decision Items

### Decision Item 1: Git Release Tag Identity
- **Context:** The upstream remote `origin` (`code-amalps/Engram`) already contains a legacy tag `v1.5.4` on historical commit `a75a2c6e2c02acf7af578fd4c5f0b2751efd5988`. Modifying or overwriting existing tags is prohibited.
- **Proposed Tag:** `v1.5.4-cand5`
- **Target Commit:** `2df1cac683435314ffd7ad52369e6d20198fae0d`
- **Verification:** Audited across local and remote namespaces; 0 collisions found.
- **Action Required:**
  - [ ] **APPROVE** tag `v1.5.4-cand5` for the Candidate 5 Developer Preview release.
  - [ ] **REJECT** / Request alternate tag naming.

---

### Decision Item 2: Final Distribution Binary Selection
- **Context:** Forensic source reconstruction established that Candidate 5's source code was committed in commit `3be26e9` and remains 100% bit-for-bit identical in release commit `2df1cac`. Two binary distribution options are available:
  - **Option A (Original Benchmarked Binary):**
    - SHA-256: `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44`
    - Embedded Metadata: `commit: 7106a2f... (dirty: true)`
    - Advantage: 1:1 cryptographic identity with the Phase 5.9.2.5 empirical benchmark runs.
    - Disadvantage: Contains `dirty: true` metadata because it was compiled prior to Git commit staging.
  - **Option B (Clean-Build Binary — RECOMMENDED):**
    - SHA-256: `3A43F98F3D7BA5979A38A07F223D6DF84143E0423D0B2543CF92CD87A15724E9`
    - Embedded Metadata: `commit: 625461b... (dirty: false)`
    - Advantage: 100% clean Git provenance, pristine `dirty: false` metadata, perfectly aligned with the public Git repository and AGPL Section 6.
    - Test Record: 235/235 regression tests passed, 8/8 headless MCP scenarios passed, security tests passed.
- **Action Required:**
  - [ ] **SELECT OPTION B (RECOMMENDED):** Package the clean-build binary `3A43F98F...` into the public release archive, retaining Option A in documentation as the historical benchmark reference binary.
  - [ ] **SELECT OPTION A:** Retain the original benchmarked binary `E4E53216...` in the public release archive.

---

### Decision Item 3: Legal Counsel Review & AGPL Compliance Sign-Off
- **Context:** Engram is licensed under the GNU Affero General Public License v3.0 (AGPL-3.0). Section 6 mandates providing the "Corresponding Source" in a machine-readable format.
- **Forensic Status:**
  - Complete source code, build scripts (`build.rs`), configurations (`Cargo.toml`), dependencies (`Cargo.lock`), and toolchain specifications (`rust-toolchain.toml`) are committed and pushed to `feature/phase2-stage5-remediation`.
  - All 88 source and test files match the Git repository blobs with zero discrepancies.
  - Option B provides clean `dirty: false` build metadata directly traceable to Git commit history.
- **Action Required:**
  - [ ] **SELF-CERTIFY:** Owner determines that the technical evidence and Option B clean provenance provide full source correspondence compliance; approve immediate publication upon tagging.
  - [ ] **LEGAL REVIEW:** Owner requests brief formal review by qualified legal counsel prior to pushing the public release.

---

## 3. Post-Approval Manual Execution Commands (For Repository Owner)

Once the owner has made the decisions above, execute the following commands to complete publication:

### Step 1: If Option B Was Selected (Repackage Release ZIP)
```powershell
# Copy Option B executable into the release directory
Copy-Item "C:\Users\Amal P S\OneDrive\Projects\Engram\target\release\engram.exe" "C:\Users\Amal P S\OneDrive\Projects\Engram\releases\engram-preview-v1.5.4-cand5-windows-x64\engram.exe" -Force

# Regenerate release archive and checksums
Compress-Archive -Path "C:\Users\Amal P S\OneDrive\Projects\Engram\releases\engram-preview-v1.5.4-cand5-windows-x64\*" -DestinationPath "C:\Users\Amal P S\OneDrive\Projects\Engram\releases\engram-preview-v1.5.4-cand5-windows-x64.zip" -Force
Get-FileHash -Algorithm SHA256 "C:\Users\Amal P S\OneDrive\Projects\Engram\releases\engram-preview-v1.5.4-cand5-windows-x64.zip"
```

### Step 2: Create and Push the Release Git Tag
```powershell
cd "C:\Users\Amal P S\OneDrive\Projects\Engram"
git tag -a v1.5.4-cand5 2df1cac683435314ffd7ad52369e6d20198fae0d -m "Engram v1.5.4 Candidate 5 Developer Preview (Windows x64)"
git push origin v1.5.4-cand5
```

### Step 3: Publish GitHub Release
1. Navigate to: `https://github.com/code-amalps/Engram/releases/new`
2. Select Tag: `v1.5.4-cand5`
3. Target Branch: `feature/phase2-stage5-remediation`
4. Release Title: `Engram v1.5.4 Candidate 5 - Developer Preview (Windows x64)`
5. Release Notes: Copy contents from `docs/publication/ENGRAM_V1_5_4_GITHUB_RELEASE_DRAFT.md`.
6. Attach Asset: Upload `releases/engram-preview-v1.5.4-cand5-windows-x64.zip`.
7. Check: **"Set as a pre-release"** (Mandatory for developer preview).
8. Click: **"Publish release"**.
