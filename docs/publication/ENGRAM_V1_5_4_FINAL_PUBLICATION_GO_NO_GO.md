# Engram v1.5.4 Developer Preview Publication Go/No-Go Decision Document

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**GitHub Upstream:** `code-amalps/Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Release Version:** `1.5.4`  
**Candidate:** Candidate 5 (Windows x64)  
**Proposed Release Tag:** `v1.5.4-cand5`  
**Target Commit:** `2df1cac683435314ffd7ad52369e6d20198fae0d`  
**Release Asset:** `releases/engram-preview-v1.5.4-cand5-windows-x64.zip`  
**Asset Size:** `4502442` bytes | **Asset SHA-256:** `7C355109EEB8BE7B08301080FD660DE4A4A23DD12AFE89F008546C32FB3C1741`  
**Publication Verdict:** `PUBLICATION_READY_WITH_LEGAL_REVIEW_REQUIRED`  
**Execution State:** Pre-Publication Evaluation Completed. Zero Remote Publication Dispatched.

---

## 1. Publication Readiness Assessment

The technical release engineering, integration testing, package validation, and forensic audits for Engram v1.5.4 Candidate 5 (Windows x64) have completed with zero defects. All technical requirements for releasing an Experimental Developer Preview have been met.

Publication is categorized as **READY WITH LEGAL REVIEW REQUIRED** solely due to two business/legal governance decisions that require explicit repository owner sign-off prior to publication.

---

## 2. Comprehensive Go/No-Go Decision Matrix

| Gate | Evaluation Domain | Decision | Critical Evidence / Notes |
| :--- | :--- | :---: | :--- |
| **Gate 1** | **Technical & Integration Validation** | **GO** | 235 total offline assertions passed (0 failed). Real child process stdio MCP server verified across all 8 scenarios (`test_actual_mcp_integration.py`). Clean offline compilation confirmed. |
| **Gate 2** | **Asset Integrity & Checksum Verification** | **GO** | Archive `engram-preview-v1.5.4-cand5-windows-x64.zip` exists, measures 4,502,442 bytes, matches SHA-256 `7C355109...`, internal ZIP integrity verified, all 14 payload hashes verified. |
| **Gate 3** | **Security & Boundary Containment** | **GO** | 6 directory traversal attack vectors blocked. Secret redaction verified. Graceful recovery under corrupted configuration. Zero orphaned processes after rapid kill cycles. Zero archive secrets. |
| **Gate 4** | **Benchmark Methodology & Disclosure** | **GO** | Strict `SYNTHETIC_SIMULATED_USAGE` disclosures maintained. Zero live LLM billing savings or latency reductions claimed. Workload F single-turn +0.71% token overhead disclosed. |
| **Gate 5** | **Git Tag Collision & Identity** | **CONDITIONAL GO** | Remote `origin` contains legacy tag `v1.5.4` on commit `a75a2c6`. Resolved by proposed tag `v1.5.4-cand5` (0 collisions locally and remotely). |
| **Gate 6** | **AGPL-3.0 Source Correspondence** | **CONDITIONAL GO** | Complete Corresponding Source is reachable at commit `2df1cac` (ancestor `3be26e9`). Packaged binary embeds `commit: 7106a2f... (dirty: true)`. Requires owner selection between Option A (audited binary) and Option B (clean build). |

---

## 3. Owner Decisions Required Prior to Publication

The repository owner must resolve two specific decisions:

### Owner Decision 1: Tag Selection
- **Option 1 (Recommended): `v1.5.4-cand5`**  
  Creates a clean, non-colliding annotated tag on commit `2df1cac`. Preserves the historical October 2026 `v1.5.4` tag untouched. Accurately designates Candidate 5 of the 1.5.4 line.
- **Option 2: Force-Update `v1.5.4`**  
  Overwrites the existing `v1.5.4` tag on GitHub. *Not recommended as it alters public Git tag history for existing users.*

### Owner Decision 2: Distribution Binary Selection
- **Option A (Current Package - Recommended for Benchmark Continuity):**  
  Conveys the packaged Candidate 5 binary (`bin/engram.exe`, SHA-256: `E4E53216...`, embedded `7106a2f... (dirty: true)`).  
  *Advantage:* 100% cryptographic identity with the binary audited in Phase 5.9.2.5 and Phase 5.9.2.6.  
  *Compliance Requirement:* Conveyed alongside `RELEASE_MANIFEST.json` and release notes explicitly documenting that uncommitted working-tree changes correspond to commit `3be26e9` / `2df1cac`.
- **Option B (Clean Build Binary):**  
  Conveys the reproducible clean worktree build (`target/release/engram.exe`, SHA-256: `3A43F98F...`, embedded `625461b... (dirty: false)`).  
  *Advantage:* 1:1 metadata alignment between Git tree and embedded binary stamping. Proven functionally identical in all integration tests.

---

## 4. Final Verdict

```
PUBLICATION_READY_WITH_LEGAL_REVIEW_REQUIRED
```

The publication package is fully prepared locally. The repository owner and legal counsel may proceed with publication immediately upon confirming Decision 1 (Tag: `v1.5.4-cand5`) and Decision 2 (Binary: Option A vs Option B).

---

## 5. Step-by-Step Publication Procedure (Manual Execution Only)

Following owner review, execute the publication steps manually:

```bash
# 1. Create and push the non-colliding release tag:
git -C "C:\Users\Amal P S\OneDrive\Projects\Engram" tag -a v1.5.4-cand5 2df1cac683435314ffd7ad52369e6d20198fae0d -m "Engram v1.5.4 Candidate 5 Windows x64 Experimental Developer Preview"
git -C "C:\Users\Amal P S\OneDrive\Projects\Engram" push origin v1.5.4-cand5

# 2. In GitHub Web Interface (code-amalps/Engram -> Releases -> Draft a new release):
#    - Tag: v1.5.4-cand5
#    - Title: Engram v1.5.4 - Candidate 5 | Windows x64 Experimental Developer Preview
#    - Description: Paste verbatim from docs/publication/ENGRAM_V1_5_4_GITHUB_RELEASE_DRAFT.md
#    - Checkbox: [x] Set as a pre-release
#    - Checkbox: [x] Save as a draft

# 3. Upload Asset:
#    - File: releases/engram-preview-v1.5.4-cand5-windows-x64.zip
#    - Expected Checksum: 7C355109EEB8BE7B08301080FD660DE4A4A23DD12AFE89F008546C32FB3C1741

# 4. Final Verification & Publish:
#    - Verify asset download hash matches 7C355109...
#    - Click 'Publish release'
```
