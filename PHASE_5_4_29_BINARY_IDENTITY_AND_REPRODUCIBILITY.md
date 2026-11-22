# Phase 5.4.29: Binary Artifact Identity and Rebuild Reproducibility Audit

**Phase:** 5.4.29  
**Component:** Executable Binary Forensics & Bit-for-Bit Reproducibility Analysis  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Authorization Status:** `NOT_AUTHORIZED`  

---

## 1. Executive Summary

Phase 5.4.29 executed an independent rebuild reproducibility test. Two isolated clean release builds were performed from the identical source snapshot in two completely separate build directories with independent target caches:
- **Build 1:** Source in `engram_isolated_build_1`, Target in `engram_isolated_target_1`
- **Build 2:** Source in `engram_isolated_build_2`, Target in `engram_isolated_target_2`

Both builds completed with exit code 0 and produced executables of exactly **18,803,200 bytes**.

Forensic byte-by-byte diffing (`fc /B`) revealed that all executable code sections (`.text`), read-only data (`.rdata`), and data sections (`.data`) are **100% bitwise identical**. However, approximately 24 bytes in the PE/COFF header and debug directory differ due to the default behavior of the Microsoft Visual C++ Linker (`link.exe`), which injects a build timestamp and generates a random GUID for the PDB debug stream.

### Reproducibility Classification:
**`FUNCTIONALLY_EQUIVALENT_NOT_BIT_REPRODUCIBLE`**

---

## 2. Multi-Binary Comparative Identity Matrix

| Executable Artifact | Origin / Environment | File Size (Bytes) | SHA-256 Checksum | Embedded Version Output |
|---|---|---|---|---|
| **Candidate 3** | Baseline HEAD `7106a2f` (Phase 2 Stage 5) | 18,668,032 | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | `engram 1.5.4` |
| **Candidate 4.2** | Phase 5.4.3 (Added UTF-8 slice safety & 16KB budget ladder) | 18,802,176 | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | `engram 1.5.4` |
| **Existing Target Release** | Retained from Phase 5.4.3 build cache | 18,802,176 | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | `engram 1.5.4` |
| **Isolated Build 1** | Phase 5.4.29 Clean Build from Temp Workspace 1 | 18,803,200 | `AC88FEEF5E28641DE3F74E071031D318734EDD7FAB78B0A0A705D00DA0303110` | `engram 1.5.4\ncommit: unknown\ndirty: unknown\nrustc: rustc 1.99.0...\nprofile: release` |
| **Isolated Build 2** | Phase 5.4.29 Clean Rebuild from Temp Workspace 2 | 18,803,200 | `039C62C6BFB26A0A747DB77537D82FFFC8CCAA4F8293B5B263A9552DC2BCED85` | `engram 1.5.4\ncommit: unknown\ndirty: unknown\nrustc: rustc 1.99.0...\nprofile: release` |

---

## 3. Forensic Analysis: Why Existing Release Matches Candidate 4.2

1. **Exact Bitwise Identity:** `fc /B target\release\engram.exe candidate-phase4.2\engram.exe` reported `no differences encountered`.
2. **Historical Provenance:** In Phase 5.4.3, Candidate 4.2 was compiled into `target/release/engram.exe` to fix the TS2554 UTF-8 panic and copied into `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe`.
3. **No Intervening Release Compilation:** Between Phase 5.4.4 and Phase 5.4.27, all activities were read-only forensic audits. No command ran `cargo build --release` in the repository root. Thus, `target/release/engram.exe` remained the untouched output of Phase 5.4.3.
4. **Pre-`build.rs` Origin:** Because Phase 5.4.3 preceded the creation of `build.rs` in Phase 5.4.28, neither Candidate 4.2 nor the existing `target/release/engram.exe` contains the `build_info` module or multi-line `--version` strings.

---

## 4. Forensic Analysis: Isolated Build 1 vs Isolated Build 2

### 4.1 Byte-by-Byte Diff Trace (`fc /B`)
```
Comparing files engram_isolated_target_1\release\engram.exe and engram_isolated_target_2\release\engram.exe
00000108: E6 25
00000109: 01 03
010C2DD4: E6 25
010C2DD5: 01 03
010C2DF0: E6 25
010C2DF1: 01 03
010C2E0C: E6 25
010C2E0D: 01 03
010C2F74: FA 7F
010C2F75: 2A 09
010C2F76: 0D B4
010C2F77: 62 D6
010C2F78: 8F CD
010C2F79: 04 2C
010C2F7A: 30 35
010C2F7B: 48 4C
010C2F7C: A0 9F
010C2F7D: BF A9
010C2F7E: 1B 90
010C2F7F: F5 48
010C2F80: B3 BF
010C2F81: D7 FF
010C2F82: C4 2C
010C2F83: 6C 4A
```

### 4.2 Root-Cause Identification
- **Offset `0x00000108`:** COFF File Header `TimeDateStamp` field (32-bit UNIX timestamp). The difference (`E6 25` vs `01 03`) reflects the compilation wall-clock delta between the two builds.
- **Offsets `0x010C2DD4`, `0x010C2DF0`, `0x010C2E0C`:** The Debug Directory entry timestamps for IMAGE_DEBUG_TYPE_CODEVIEW and associated debug entries.
- **Offsets `0x010C2F74..0x010C2F83`:** The 16-byte CodeView PDB 7.0 (`RSDS`) GUID. By default, MSVC `link.exe` generates a cryptographically random GUID for each link invocation to link the PE to its corresponding `.pdb` file.
- **Code & Data Invariance:** Every single byte outside of these PE header and debug metadata fields—encompassing 18,803,176 out of 18,803,200 bytes—is 100% bitwise identical.

### 4.3 Why MSVC Defaults Prevent Bit-for-Bit Identity
In the MSVC toolchain, deterministic bit-for-bit linking requires passing explicit linker arguments:
- `/BREPRO`: Tells Microsoft `link.exe` to emit a deterministic timestamp and PDB signature based on hash of inputs rather than system time and random UUID.
- Without `-C link-arg=/BREPRO` in Cargo configuration, Rust on Windows MSVC generates timestamp-dependent binaries.

---

## 5. Reproducibility Classification Verdict

Because the two independent builds produced functionally identical machine code, data, and layout, but differed by timestamp and PDB UUID:

**Verdict:** **`FUNCTIONALLY_EQUIVALENT_NOT_BIT_REPRODUCIBLE`**
