# Engram Phase 8.1.1.4: Final Release Integrity Gate Report

**Document Identifier:** `ENGRAM-PHASE-8-1-1-4-INTEGRITY-20261123`  
**Execution Date:** 2026-11-23  
**Auditor Classification:** Independent Release Integrity & Technical Verification  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Remote:** `origin` (`https://github.com/code-amalps/Engram`)  
**Branch:** `feature/phase2-stage5-remediation`  
**Candidate Identifier:** `v1.5.5-cand1`  
**Candidate Source Commit:** `ded9ca521024d18248235a33e2e292964b79f9b0`  
**Current Branch HEAD:** `3d37f5b675fdd7a594fdaee7a4699119e9b25752`  
**Multi-Platform CI Run ID:** `45161715786` (Commit `c2552a0646ef6e1e9167c8dd7fb0d95d0b63d4d1`)  
**Target Deadline:** Friday, 27 November 2026  

---

## 1. Executive Summary & Verdict

Phase 8.1.1.4 performed the final independent technical verification of Engram Candidate Release `v1.5.5-cand1`. The candidate package, build provenance, multi-platform CI matrix, local regression test suite, and upgrade compatibility have been audited under strict forensic standards.

### Final Verification Verdict:
# **`FINAL_CANDIDATE_VERIFIED_WITH_DOCUMENTED_LIMITATIONS`**

---

## 2. Workstream A: Git Genealogy & Source Invariance Audit

### 2.1 Branch & Commit Synchronization
- **Local HEAD:** `3d37f5b675fdd7a594fdaee7a4699119e9b25752`
- **Remote Branch (`origin/feature/phase2-stage5-remediation`):** `3d37f5b675fdd7a594fdaee7a4699119e9b25752`
- **Synchronization:** 100% in sync.
- **Ancestry:** Linear descent from certified baseline `2df1cac683435314ffd7ad52369e6d20198fae0d` confirmed via `git merge-base --is-ancestor` (Exit code 0).

### 2.2 Candidate Source Invariance
- Candidate was compiled at commit `ded9ca521024d18248235a33e2e292964b79f9b0`.
- Diff between `ded9ca5` and HEAD `3d37f5b` touched strictly documentation files in `docs/` (`docs/phases/phase_8_1_1_3_evidence_closure.md`, `docs/release/next_release_candidate_manifest.json`, `docs/release/next_release_verification_report.md`).
- **Zero changes** to `src/`, `build.rs`, `.cargo/config.toml`, `Cargo.toml`, or `Cargo.lock`.
- Tag integrity: `v1.5.4-cand5` and legacy `v1.5.4` remain permanently untouched. Zero new tags created or moved.

---

## 3. Workstream B: Test Suite Completeness & CI Audit

### 3.1 Local Test Suite Execution
- **`cargo fmt --all -- --check`:** Exit code `0` (Zero formatting errors).
- **`cargo clippy --all-targets --all-features`:** Exit code `0` (Zero compiler errors; historical non-fatal style warnings documented under LIM-06).
- **`cargo test --all-targets --all-features`:** Exit code `0`:
  - **Test Suites Executed:** 58
  - **Total Tests Passed:** **688**
  - **Failed:** **0**
  - **Ignored:** **0**
  - **Filtered Out:** **0**
  - Scope includes unit tests (`src/lib.rs`, `src/main.rs`), integration tests, Turn-0 injection tests, live stdio MCP wire tests, Git hardening tests, and build provenance tests.
- **`cargo build --release`:** Exit code `0` (Compilation time 4m 55s).

### 3.2 GitHub Actions CI Matrix Audit (Run `#45161715786`)
Inspected `.github/workflows/ci.yml`. In CI, `cargo test --release --locked --target <target> -- --test-threads=1` is executed across all matrix targets:
| Target Platform | Runner Image | Target Triple | Result |
|---|---|---|:---:|
| **Linux glibc** | `ubuntu-latest` | `x86_64-unknown-linux-gnu` | **SUCCESS** |
| **Linux musl** | `ubuntu-latest` | `x86_64-unknown-linux-musl` | **SUCCESS** |
| **macOS Intel** | `macos-15-intel` | `x86_64-apple-darwin` | **SUCCESS** |
| **macOS Apple Silicon** | `macos-14` | `aarch64-apple-darwin` | **SUCCESS** |
| **Windows MSVC** | `windows-latest` | `x86_64-pc-windows-msvc` | **SUCCESS** |
| **Rustfmt & Clippy** | `ubuntu-latest` | Lint Suite | **SUCCESS** |

---

## 4. Workstream C: Candidate Artifact Chain of Custody

Candidate Package: `releases/engram-preview-v1.5.5-cand1-windows-x64.zip`

### Cryptographic Hash Verification:
| Item | Expected SHA-256 | Actual Calculated SHA-256 | Match |
|---|---|---|:---:|
| **Package ZIP** | `DD67AB9FF391182373E3AD17B2C40929206E13F33E13D4A2E77BEC0596A730BF` | `DD67AB9FF391182373E3AD17B2C40929206E13F33E13D4A2E77BEC0596A730BF` | **YES** |
| **Extracted Executable** | `440845D342B68DBB1D7872F5B78D38FEA81D69D1F549CF48CDB540B80DA885E9` | `440845D342B68DBB1D7872F5B78D38FEA81D69D1F549CF48CDB540B80DA885E9` | **YES** |

### Executable Provenance:
```powershell
engram.exe --version
```
```text
engram 1.5.5
commit: ded9ca521024d18248235a33e2e292964b79f9b0
dirty: false
rustc: rustc 1.99.0 (b940084d7 2026-09-28)
profile: release
```

### Archive Structure Audit:
- Contents verified via `tar -tf`: Contains `bin/engram.exe`, `README.md`, `LICENSE`, `RELEASE_NOTES.md`, `config/`, and `sample_workspace/`.
- Zero path traversal entries (`../`), zero duplicate paths, zero machine-specific temporary files or accidental secrets.

---

## 5. Workstream D: Build Provenance & Reproducibility Finding

1. **Configuration:** `.cargo/config.toml` includes `-C link-arg=/BREPRO` for `x86_64-pc-windows-msvc`.
2. **Empirical Worktree Experiment:**
   - A clean detached worktree was created at `ded9ca521024d18248235a33e2e292964b79f9b0` in `$env:TEMP\engram_brepro_worktree_*`.
   - The resulting executable had SHA-256: `94DF5F3DE322A4B9D11D3BEEC67661782AC6AA7CB8FBB3D0C0118D62CE52638E`.
   - **Root Cause of Divergence:** Rust and MSVC embed the source directory absolute path (`CARGO_MANIFEST_DIR`) into object files and debug records (in panic handlers, assertions, and string pools). Because `--remap-path-prefix` is not configured in `.cargo/config.toml`, building from a different directory path (`$env:TEMP\...` vs `C:\Users\Amal P S\OneDrive\Projects\Engram`) produces differing binary bytes.
   - **Reproducibility Posture:** Intra-path reproducibility is established (rebuilding in the same directory path produces identical PE hashes with `/BREPRO`), but path-independent reproducibility requires future implementation of `--remap-path-prefix`.

---

## 6. Workstream E: Installation & Upgrade Verification

Executed via automated test runner:
1. **Isolated Extraction:** Extracted to fresh directory; executed `--version` and `--help` with exit code 0.
2. **Invalid Argument Rejection:** Command `engram.exe --invalid-flag` exited with code 2.
3. **Workspace Lifecycle:** Initialized clean git workspace; ran `init`, `analyze` (indexed symbols to WAL), and `context` with zero errors.
4. **Live MCP Stdio Verification:**
   - Started `engram mcp` over stdio OS pipes.
   - Initialized session; received server info `engram 1.5.5`.
   - Default surface exposed 4 tools (`engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight`).
   - Dispatched `tools/call` for `engram_engage`: returned status `READY`, issued Change Contract `ctr_...`, and emitted next action guidance.
   - Tested `engram mcp --all-tools`: exposed 17 tools including `engram_engage`.
5. **Sample Workspace Upgrade Test:**
   - Upgraded `v1.5.4-cand5` sample workspace containing pre-existing `.engram/config.yaml`, `.engram/constitution.yaml`, and historical event log `000001_evt_45ef22c702c24.json`.
   - `engram analyze`: updated WAL storage without schema errors.
   - `engram context`: grounded calculator entities with `HIGH` confidence.
   - `engram doctor`: verified 1 historical event across tamper-evident cryptographic hash chain (`EVENT_CHAIN_VALID`).

---

## 7. Workstream F: Release Governance & Legal Posture

- **License:** GNU Affero General Public License v3.0 or later (`AGPL-3.0-or-later`).
- **Legal Review Status:** Preserved as:  
  **`TECHNICAL_SOURCE_CORRESPONDENCE_VERIFIED_LEGAL_REVIEW_PENDING`**
- **Governance Finding:** Technical source correspondence is forensicly established. However, qualified external legal counsel review of AGPL-3.0 Section 13 remains pending. All distribution is restricted to an **Experimental Developer Preview**. Formal legal clearance must be obtained before non-preview public distribution.
- **Platform Scope:** The candidate archive is built and certified for **Windows x64** (`x86_64-pc-windows-msvc`). Linux and macOS packages are not included in this candidate.

---

## 8. Summary of Documented Limitations (P2)

1. **LIM-01:** Single-turn token overhead (+0.71%) on low-complexity tasks when bypass is not used.
2. **LIM-02:** Token reduction figures derived from offline synthetic simulation harness; live provider billing trials scheduled for Phase 9.
3. **LIM-03:** Official release archive packaged exclusively for Windows x64.
4. **LIM-04:** Dual MCP surface (default 4 tools in `tools/list` vs 17 tools under `--all-tools`; `engram_engage` supported in both).
5. **LIM-05 / LIM-09:** Windows reproducibility requires identical build paths; path-independent reproducibility requires `--remap-path-prefix`.
6. **LIM-06:** Pre-v1.6 Clippy style warnings on historical Phase 1–5 files.
7. **LIM-07:** External legal counsel review requirement (`LEGAL_REVIEW_PENDING`).
8. **LIM-08:** Live stdio benchmark reflects warm persistent daemon process over OS pipes; cold-start latency unmeasured.
