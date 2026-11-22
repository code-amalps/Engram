# Engram Phase 5.4.27: Build Reproducibility and Binary Provenance Audit

**Phase:** 5.4.27  
**Execution Mode:** STRICTLY OFFLINE, READ-ONLY FORENSIC AUDIT  
**Baseline Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Classification:** `REPRODUCIBILITY_PARTIAL`  
**Release Authorization:** `NOT_AUTHORIZED`

---

## 1. Executive Summary

This forensic audit evaluates the build reproducibility, compiler toolchain governance, binary provenance, and cryptographic traceability of the Engram codebase and its pre-compiled binaries.

While dependency resolution is locked via `Cargo.lock` and release profiles are strictly configured, the current repository state achieves only **`REPRODUCIBILITY_PARTIAL`** due to four structural factors:
1. **Uncommitted Working Tree:** Nine tracked files in `src/` and `benchmarks/` contain unstaged modifications. Recompiling `target/release/engram.exe` today would compile an uncommitted dirty state rather than clean baseline commit `7106a2f3`.
2. **Missing Commit SHA in Binary:** The `engram` binary does not embed the Git commit hash, build timestamp, or compiler version (`build.rs` is absent; Clap merely reads static package version `1.5.4`).
3. **Unpinned Rust Toolchain:** No `rust-toolchain` or `rust-toolchain.toml` file exists in the repository root, allowing floating compiler toolchains across different developer environments.
4. **Benchmark Candidate Binaries:** Candidate binaries (Candidate 3, Candidate 4.2) are stored as pre-built binaries in git LFS/untracked folders without automated provenance attestation files.

---

## 2. Source-to-Binary Traceability Matrix

| Metric | Declared State | Verified Reality | Provenance Finding |
| :--- | :--- | :--- | :--- |
| **Package Name** | `engram` | `engram` in `Cargo.toml` | Matched |
| **Package Version** | `1.5.4` | `1.5.4` in `Cargo.toml` and CLI output | Matched |
| **Git Baseline Commit** | `7106a2f369fe79faf5f95449378a900ab8e8eddf` | `7106a2f369fe79faf5f95449378a900ab8e8eddf` | Verified via `git rev-parse HEAD` |
| **Working Tree Cleanliness** | "Clean working tree" (reported) | **9 modified tracked files (2,247 insertions, 136 deletions)** | **Discrepancy Documented** |
| **Git Commit in Binary** | Expected for traceable release | **ABSENT.** CLI prints bare `engram 1.5.4` | **Traceability Gap** |
| **Compiler Toolchain** | Stable Rust | `rustc 1.99.0 (b940084d7 2026-09-28)` on host | Unpinned in repo |
| **Dependency Lockfile** | `Cargo.lock` | Present, valid, and fully resolved | **Proven** |
| **Release Profile** | LTO thin, codegen-units 1, strip | Declared in `Cargo.toml:profile.release` | **Proven** |

---

## 3. Cryptographic Binary Hash Audit

The SHA-256 hashes of all prominent executables across the repository were measured independently:

```
Binary Location                                                                SHA-256 Checksum                                                    Provenance Status
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
target\release\engram.exe                                                      78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2     Candidate 4.2 Build
benchmarks\matched-pair-harness\bin\candidate-phase4.2\engram.exe             78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2     Exact Match
benchmarks\matched-pair-harness\bin\candidate-phase3\engram.exe               97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777     Candidate 3 (Frozen)
dist\installer\engram-setup.exe                                                6AEB32D84078190D5C519EE71703DBFABA1E25DA5C43A302EE3CF716F8229D76     Packaged Installer
```

### Forensic Hash Observations:
- `target\release\engram.exe` matches `benchmarks\matched-pair-harness\bin\candidate-phase4.2\engram.exe` with 100% cryptographic identity.
- Candidate 3 (`97C5EC...`) is an older binary built prior to the Phase 5.4.3 UTF-8 boundary remediation, preserved solely for historical benchmark comparisons.
- The Inno Setup installer package `engram-setup.exe` bundles `target\release\engram.exe`.

---

## 4. Working-Tree Discrepancy Investigation

Earlier phases (5.4.17 through 5.4.25) repeatedly reported preserving 9 modified tracked files. A subsequent summary in Phase 5.4.26 referred to a "clean working tree." This audit investigated the discrepancy forensically:

1. **Origin of the "Clean" Output:**
   - In the initial command execution of the prior turn, `git status` was run without `-C <path>`, defaulting to `c:\Users\Amal P S\OneDrive\Projects\AMstraLift`, which was on branch `Dev` and was clean.
2. **True State of Engram Repository:**
   - Running `git -C "c:\Users\Amal P S\OneDrive\Projects\Engram" status` confirms that the Engram repository has **never** been clean during this sequence.
   - The 9 modified tracked files have remained continuously unstaged and preserved:
     - `benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json`
     - `benchmarks/matched-pair-harness/src/classifier.py`
     - `benchmarks/matched-pair-harness/src/evaluator.py`
     - `benchmarks/matched-pair-harness/src/orchestrator.py`
     - `benchmarks/matched-pair-harness/src/runner.py`
     - `benchmarks/matched-pair-harness/src/telemetry.py`
     - `src/impact.rs`
     - `src/mcp.rs`
     - `src/storage.rs`
3. **Impact on Release Reproducibility:**
   - If an engineer runs `cargo build --release` right now, the resulting binary will incorporate unstaged edits in `src/impact.rs`, `src/mcp.rs`, and `src/storage.rs`.
   - Therefore, a third party checking out commit `7106a2f369fe79faf5f95449378a900ab8e8eddf` cannot reproduce the exact binary `78B84880...` unless those working-tree modifications are committed or isolated.

---

## 5. Toolchain & Build Environment Governance

### 5.1 Toolchain Specification
- **Host Compiler:** `rustc 1.99.0 (b940084d7 2026-09-28)`
- **Host Cargo:** `cargo 1.99.0 (5f94df478 2026-08-27)`
- **Host Target:** `x86_64-pc-windows-msvc`
- **Issue:** No `rust-toolchain.toml` file exists. Rust nightly or future stable releases may alter optimization output or symbol linking, introducing non-deterministic binary hashes.

### 5.2 Build Configuration
- `Cargo.toml` contains optimal release profile flags:
  ```toml
  [profile.release]
  lto = "thin"
  codegen-units = 1
  strip = true
  ```
  This configuration eliminates non-deterministic debug symbols and reduces binary size to ~18.8 MB.

### 5.3 Missing Build Information Crate
- `src/main.rs` does not include `build.rs` or crates such as `vergen` or `git-version`.
- Executing `engram --version` outputs only:
  ```
  engram 1.5.4
  ```
  For a production release, it should output:
  ```
  engram 1.5.4 (7106a2f3 2026-11-20 rustc-1.99.0)
  ```

---

## 6. Classification Verdict

**Reproducibility Classification:** **`REPRODUCIBILITY_PARTIAL`**

**Prerequisites to achieve `REPRODUCIBLE_BUILD_PROVEN`:**
1. Commit or isolate the 9 modified files in `src/` and `benchmarks/`.
2. Add a `rust-toolchain.toml` locking the Rust channel and version.
3. Add a `build.rs` script that injects Git commit hash, build timestamp, and target triple into the binary version string.
4. Record GitHub Actions build provenance attestations for all multi-platform release assets.
