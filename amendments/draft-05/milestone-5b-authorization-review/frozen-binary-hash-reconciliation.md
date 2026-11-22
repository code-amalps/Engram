# Milestone 5B Authorization Review: Frozen Binary Hash Reconciliation

## 1. Executive Summary

This report conducts a forensic reconciliation of the binary executables used across the Engram Amendment 05 benchmarks:
1. **Candidate Phase 1 Binary**
2. **Frozen v1.5.1 Reference Binary**
3. **Frozen v1.5.4 Reference Binary**

The audit investigates the origin of the "apparent v1.5.4 checksum discrepancy" noted in historical documentation and confirms that all three physical binary files in the repository have remained **100% bit-for-bit preserved and untouched**.

---

## 2. Forensic Cryptographic Hash Ledger

The table below compares the physical file hashes calculated directly from disk with the historical manifests and code constants:

| Artifact Identifier | Physical Disk Path | Current Computed SHA-256 | Authoritative Manifest SHA-256 | Reconciliation Status |
|---|---|---|---|---|
| **Candidate Phase 1 Binary** | `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | **EXACT MATCH (100% Preserved)** |
| **Frozen v1.5.1 Binary** | `benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | **EXACT MATCH (100% Preserved)** |
| **Frozen v1.5.4 Binary** | `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | **EXACT MATCH (100% Preserved)** |

---

## 3. Investigation of the Apparent v1.5.4 Checksum Discrepancy

In several historical documentation passes (e.g. Milestone 2, Milestone 2A, and Milestone 1A reports), two conflicting SHA-256 hashes were associated with the label "v1.5.4":
- Hash A: `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798`
- Hash B: `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`

### Root Cause Analysis:

The forensic investigation reveals that this discrepancy was **purely a documentation and naming collision**, NOT physical file corruption or binary replacement:

1. **Two Distinct Binaries Existed for Different Purposes:**
   - **Artifact 1 (Frozen Release v1.5.4):** Built on October 28, 2026 as the frozen reference release for the 3-arm Triad benchmark (`v154_arm3`). Its physical path is `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe` and its SHA-256 is `CF807EAC...`.
   - **Artifact 2 (Candidate Phase 1 Binary):** Built on October 29, 2026 incorporating the Phase 1 token-efficiency optimizations and subpath/language routing. Its physical path is `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` and its SHA-256 is `F56347EF...`.
2. **The Naming Collision in Milestone 2:**
   - In Milestone 2 documentation, authors frequently referred to the Candidate Phase 1 binary colloquially as the *"Candidate v1.5.4 binary"* or *"Phase 1 candidate"*.
   - When authors inspected `orchestrator.py`, they found constant `DEFAULT_ENGRAM_V154_BIN` pointing to `bin/v1.5.4/engram.exe` with expected hash `CF807EAC...`.
   - When other authors inspected `bin/candidate-phase1/engram.exe`, they found hash `F56347EF...`.
   - Some reports conflated the two, asserting that the "v1.5.4 hash was either CF807... or F563...".
3. **Orchestrator Arm Resolution in Milestone 5B:**
   - In the two-arm matched pair protocol (`control_A` vs `treatment_B`), the treatment arm uses `self.engram_bin`.
   - To benchmark Candidate Phase 1, the orchestrator must explicitly point to `bin/candidate-phase1/engram.exe` with expected hash `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`.
   - If `v154_arm3` is invoked without override flags, the harness safely resolves to the frozen reference binary (`bin/v1.5.4/engram.exe`, `CF807EAC...`).

---

## 4. Manifest Integrity Proof

Inspection of historical checksum ledgers in `amendments/draft-05/` confirms exact continuity:
- `amendments/draft-05/milestone-1a-closure-summary.md` (Line 30):
  `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798`
- `amendments/draft-05/snapshot-integrity-closure-audit.md` (Line 115):
  `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798`
- `amendments/draft-05/milestone-5b-prep/baseline-and-change-inventory.md` (Section 2):
  Candidate: `F56347EF...` | Frozen v1.5.4: `CF807EAC...` | Frozen v1.5.1: `474A1A7A...`
- `benchmarks/matched-pair-harness/src/isolation.py` (Lines 26–28):
  ```python
  FROZEN_ENGRAM_V151_SHA256 = "474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0"
  FROZEN_ENGRAM_V154_SHA256 = "cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798"
  ```

---

## 5. Conclusion

There is **zero discrepancy in the physical binary artifacts**.
- The Frozen v1.5.4 reference binary is and has always been `CF807EAC...`.
- The Candidate Phase 1 binary is and has always been `F56347EF...`.
- Both binaries are intact, verified bit-for-bit, and require zero rebuilding or modification.
