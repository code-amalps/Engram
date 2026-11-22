# Engram Phase 6.0.1: Release Identity, Build Provenance & Package Inventory Reconciliation Report

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Expected HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Actual HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Existing Release ID:** `engram-preview-v1.5.3-cand5-windows-x64`  
**Executable Build Version:** `engram 1.5.4`  
**Existing Archive:** `releases/engram-preview-v1.5.3-cand5-windows-x64.zip`  
**Audit Mode:** Strictly Offline, Read-Only Forensic Reconciliation  
**Audit Timestamp:** 2026-11-22T14:35:00Z  

---

## 1. Executive Summary & Certified Primary Verdict

A comprehensive, strictly read-only forensic audit was conducted on the Phase 6.0 Experimental Developer Preview package to resolve reported release identity discrepancies, build provenance metadata, and package inventory counts.

### Certified Primary Verdict
```
RELEASE_BLOCKED_VERSION_MISMATCH
```

### Forensic Justification
1. **Version Inconsistency:** The release package directory (`engram-preview-v1.5.3-cand5-windows-x64`), archive name, and `RELEASE_MANIFEST.json` label the release as `v1.5.3`. However, the compiled Candidate 5 binary (`engram.exe --version`) reports `engram 1.5.4`, `Cargo.toml` explicitly declares `version = "1.5.4"`, and the benchmark harness (`orchestrator.py:2779`) documents Candidate 5 as a `v1.5.4 engine`. Git log inspection proves that `1.5.3` was an obsolete intermediate version superseded on October 28, 2026 by commit `a75a2c6` (`1.5.4`). The `v1.5.3` package naming was an accidental mismatch originating from an assistant chat typo in Phase 5.9.2.5.
2. **Package Inventory Reconciliation:** The package directory and ZIP archive each contain **16 physical regular files**, whereas `RELEASE_MANIFEST.json` and `SHA256SUMS.txt` list **14 files**. This 2-file delta represents `RELEASE_MANIFEST.json` and `SHA256SUMS.txt` themselves (which excluded themselves from hashing to avoid circular dependencies). While mathematically valid, the Phase 6.0 report ambiguously reported "14 files" without disclosing the self-referential exclusion.
3. **Dirty Build Provenance Explained:** The binary reports `dirty: true` because `build.rs` executes `git status --porcelain`. When Candidate 5 was compiled on 2026-11-22 10:54 UTC, the repository working tree contained untracked benchmark outputs and test workspaces. The embedded commit hash `7106a2f369fe79faf5f95449378a900ab8e8eddf` strictly matches the repository HEAD.

---

## 2. Release Version Reconciliation & Canonical Identity

### Authoritative Version Source Audit

| Version Source | Declared / Reported Version | Commit / Origin | Forensic Finding |
| :--- | :--- | :--- | :--- |
| **`Cargo.toml`** | `version = "1.5.4"` | Commit `a75a2c6` (2026-10-28) | Authoritative package version |
| **`build.rs`** | Reads `CARGO_PKG_VERSION` | Standard Cargo build-time env | Injects `1.5.4` into binary |
| **`src/lib.rs` (`VERSION_FULL`)** | `env!("CARGO_PKG_VERSION")` | Compile-time constant | Formats version as `1.5.4` |
| **`engram.exe --version`** | `engram 1.5.4` | Built 2026-11-22 10:54 UTC | Primary CLI output |
| **`orchestrator.py` (Harness)** | `arm_e_cand5_optimized (v1.5.4 engine)` | Line 2779 | Candidate 5 specification |
| **`tests/build_provenance_test.rs`** | `assert_eq!(build_info::VERSION, "1.5.4")` | Line 7 | Provenance test assertion |
| **Git Commit History** | `1.5.2` $\to$ `1.5.3` (commit `467a55f`)<br>`1.5.3` $\to$ `1.5.4` (commit `a75a2c6`) | 2026-10-28 10:35 UTC<br>2026-10-28 12:36 UTC | `1.5.3` was superseded |
| **Phase 6.0 Package Name** | `engram-preview-v1.5.3-cand5-windows-x64` | Improvised in Phase 6.0 | **MISMATCH (Typo)** |

### Recommended Canonical Release Identity
```
engram-preview-v1.5.4-cand5-windows-x64
```

### Artifacts Requiring Correction in Next Remediation Phase
1. `releases/engram-preview-v1.5.3-cand5-windows-x64/` $\to$ rename directory to `engram-preview-v1.5.4-cand5-windows-x64/`.
2. `releases/engram-preview-v1.5.3-cand5-windows-x64.zip` $\to$ regenerate archive as `engram-preview-v1.5.4-cand5-windows-x64.zip`.
3. `RELEASE_MANIFEST.json` $\to$ update `release_id` to `engram-preview-v1.5.4-cand5-windows-x64` and `candidate_identity` to `Candidate 5 (v1.5.4)`.
4. `RELEASE_NOTES.md` $\to$ update title and header to `Engram v1.5.4 (Candidate 5)`.
5. `INSTALLATION.md` $\to$ update package name references.
6. `BENCHMARK_DISCLOSURE.md` $\to$ update package header reference.
7. `SHA256SUMS.txt` $\to$ recompute checksums with updated documentation.

---

## 3. Dirty Build Provenance Investigation

### Exact Mechanism in `build.rs`
Lines 42–56 of `build.rs`:
```rust
let git_dirty = Command::new("git")
    .current_dir(&manifest_dir)
    .args(["status", "--porcelain"])
    .output()
    .ok()
    .and_then(|out| {
        if out.status.success() {
            let status_str = String::from_utf8_lossy(&out.stdout);
            Some(!status_str.trim().is_empty())
        } else {
            None
        }
    });
```
`build.rs` evaluates `git status --porcelain`. Because `git status --porcelain` by default includes untracked files (`??`), any benchmark output file, log, or temporary workspace in the repository causes `!status_str.trim().is_empty()` to evaluate to `true`.

### Findings
1. **Compile-Time Embedding:** The value `"true"` was embedded at compile time into `ENGRAM_BUILD_GIT_DIRTY` and compiled into `src/lib.rs` `VERSION_FULL`.
2. **Commit Hash Fidelity:** The embedded commit hash `7106a2f369fe79faf5f95449378a900ab8e8eddf` strictly matches the authoritative baseline HEAD.
3. **Working-Tree State at Build Time:** On 2026-11-22 at 10:54 UTC, the repository working tree contained untracked benchmark outputs (`benchmarks/matched-pair-harness/runs/...`) and stage audit markdown reports.
4. **Build Provenance Classification:**
   ```
   BUILD_PROVENANCE_DOCUMENTED_WITH_DIRTY_TREE_LIMITATION
   ```

---

## 4. Package Inventory Reconciliation (14 vs 16 Files)

| Item | In Directory | In ZIP Archive | In `SHA256SUMS.txt` | In `RELEASE_MANIFEST.json` | Classification |
| :--- | :---: | :---: | :---: | :---: | :--- |
| `bin/engram.exe` | Yes | Yes | Yes | Yes | Primary executable |
| `BENCHMARK_DISCLOSURE.md` | Yes | Yes | Yes | Yes | Documentation |
| `INSTALLATION.md` | Yes | Yes | Yes | Yes | Documentation |
| `KNOWN_LIMITATIONS.md` | Yes | Yes | Yes | Yes | Documentation |
| `RELEASE_NOTES.md` | Yes | Yes | Yes | Yes | Documentation |
| `USAGE.md` | Yes | Yes | Yes | Yes | Documentation |
| `config/config.example.yaml` | Yes | Yes | Yes | Yes | Configuration template |
| `config/constitution.example.yaml` | Yes | Yes | Yes | Yes | Configuration template |
| `sample_workspace/.engram/config.yaml` | Yes | Yes | Yes | Yes | Sample workspace file |
| `sample_workspace/.engram/constitution.yaml` | Yes | Yes | Yes | Yes | Sample workspace file |
| `sample_workspace/src/__init__.py` | Yes | Yes | Yes | Yes | Sample workspace file |
| `sample_workspace/src/calculator.py` | Yes | Yes | Yes | Yes | Sample workspace file |
| `sample_workspace/tests/__init__.py` | Yes | Yes | Yes | Yes | Sample workspace file |
| `sample_workspace/tests/test_calculator.py` | Yes | Yes | Yes | Yes | Sample workspace file |
| `RELEASE_MANIFEST.json` | **Yes** | **Yes** | *Excluded* | *Excluded* | Package metadata manifest |
| `SHA256SUMS.txt` | **Yes** | **Yes** | *Excluded* | *Excluded* | Checksums file |
| **Total Regular Files** | **16** | **16** | **14** | **14** | **Reconciled** |

**Clarification:** There are zero missing files. The 14 entries in `SHA256SUMS.txt` and `RELEASE_MANIFEST.json` represent the payload files, excluding the manifest and checksums file themselves to avoid circular dependencies. The package physically contains 16 regular files.

---

## 5. Failed-Command Recovery Forensic Review

| Phase 6.0 Failed Operation | Exit Code | Failure Root Cause | Recovery Operation | Recovery Verification | Artifact State |
| :--- | :---: | :--- | :--- | :--- | :--- |
| **Step 64799: Candidate binary hash check** | 1 | Script checked `Engram/bin/` instead of `Engram/benchmarks/matched-pair-harness/bin/` | Step 64802 located true binary paths; Step 64803 verified hashes | Candidate 5 SHA-256 confirmed `E4E53216...` | Clean. Read-only script. Zero artifacts left. |
| **Step 64993: Release documentation write** | 1 | Inline PowerShell Python string containing multi-line quotes and PowerShell snippet syntax broke shell string parsing | Steps 64994–64998 used agent `write_file` tool directly for each markdown document | Read back and verified all 5 markdown files; checksums matched | Clean. Overwritten completely by `write_file`. |
| **Step 65005: Pattern regex secret scan** | 1 | PowerShell intercepted regex quantifiers `{8,}`, `{20,}` as PowerShell scriptblocks | Step 65007 rewrote scan using list iteration of plain substring patterns | Exit code 0; clean scan across all text files | Clean. Read-only script. Zero artifacts left. |

---

## 6. Archive & Binary Integrity Verification

1. **ZIP Archive Hash:** `3A04FD53AB1D2ED6716B186A2DF28A301C61CAB538CC422547ED4A2AA1C6A9EF` (verified 100% intact).
2. **Packaged Binary Hash:** `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` (verified exact match with Candidate 5).
3. **Payload File Checksums:** All 14 payload files recomputed and verified against `SHA256SUMS.txt`. All 14 passed.

---

## 7. Next Steps for Remediation (Phase 6.0.2)

1. Rename release package directory and zip to canonical `engram-preview-v1.5.4-cand5-windows-x64`.
2. Update documentation and manifest headers to reference `v1.5.4` / `Candidate 5 (v1.5.4)`.
3. Clarify 16 total files (14 payload files + manifest + checksums) in manifest description.
4. Certify package as `RELEASE_IDENTITY_AND_INVENTORY_VERIFIED`.
