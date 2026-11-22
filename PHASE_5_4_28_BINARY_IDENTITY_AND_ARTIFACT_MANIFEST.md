# Phase 5.4.28: Release Artifact Identity, Provenance and Manifest Investigation

**Phase:** 5.4.28  
**Component:** Binary Provenance & Artifact Verification  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Authorization Status:** `NOT_AUTHORIZED`  

---

## 1. Objective & Investigation Context

Phase 5.4.27 noted that `target/release/engram.exe` and `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe` shared an identical cryptographic hash. The audit raised two questions:
1. What was the origin and provenance of `target/release/engram.exe` relative to Candidate 4.2 and Candidate 3?
2. Does the repository release workflow package local candidate directories or build fresh release binaries from source?

Phase 5.4.28 conducted binary forensic comparison and CI/CD workflow analysis to answer these questions authoritatively.

---

## 2. Cryptographic and Binary Comparison Results

Forensic analysis was performed across the three binaries present on disk:

| Artifact | File Path | File Size | SHA-256 Hash | Byte Comparison vs Candidate 4.2 |
|---|---|---|---|---|
| **Release Target** | `target/release/engram.exe` | 13,878,272 bytes | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | **Identical (0 differences)** |
| **Candidate 4.2** | `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe` | 13,878,272 bytes | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | **Baseline (0 differences)** |
| **Candidate 3** | `benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe` | 13,858,816 bytes | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | 19,456 bytes delta (Lacks UTF-8 & 16KB budget fixes) |

### 2.1 Native Byte-by-Byte Diff
Execution of binary comparison:
```cmd
cmd.exe /c "fc /B target\release\engram.exe benchmarks\matched-pair-harness\bin\candidate-phase4.2\engram.exe"
Comparing files target\release\engram.exe and benchmarks\matched-pair-harness\bin\candidate-phase4.2\engram.exe
FC: no differences encountered
```
The two binaries are 100% bitwise identical.

---

## 3. Provenance Reconstruction

1. **Candidate 3 (Commit `7106a2f`):** Built at the close of Phase 2 Stage 5. It incorporates the AST extraction engine, change guard, and SQLite storage, but lacks UTF-8 boundary protection and response budget limits.
2. **Candidate 4.2 (Phase 5.4.3):** Built after implementing `safe_truncate_str` and the 16KB response degradation ladder in `src/mcp.rs`. The compilation was performed via `cargo build --release`, and the resulting executable was copied into `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe` as an immutable benchmark fixture.
3. **`target/release/engram.exe`:** Remained in the build cache from the Phase 5.4.3 build. Throughout Phases 5.4.4 through 5.4.27, this release binary was intentionally preserved without rebuilding to protect historical fidelity.
4. **Phase 5.4.28 Preservation:** In strict accordance with the execution protocol, `target/release/engram.exe` was **not modified or overwritten** during Phase 5.4.28.

---

## 4. Release Workflow Packaging Analysis

An audit of `.github/workflows/release.yml` confirms that the CI release pipeline:
1. **Builds from Source:** Always runs `cargo build --release --locked --bin engram` in a clean VM environment.
2. **Ignores Candidate Directories:** Does not package, inspect, or reference `benchmarks/matched-pair-harness/bin/`.
3. **Generates Fresh Hashes:** Computes `SHA256SUMS` dynamically from the newly compiled binary artifacts.
4. **Reproducibility Guarantee:** With `Cargo.lock` pinned (hash: `85FF18D6...`) and `rust-toolchain.toml` committed, CI builds will match the audited code baseline.

---

## 5. Artifact Manifest

| Category | Item Name | Path / Reference | Checksum (SHA-256) |
|---|---|---|---|
| Binary | Release Target | `target/release/engram.exe` | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` |
| Binary | Candidate 4.2 Fixture | `benchmarks/.../candidate-phase4.2/engram.exe` | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` |
| Binary | Candidate 3 Fixture | `benchmarks/.../candidate-phase3/engram.exe` | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` |
| Manifest | Cargo Lockfile | `Cargo.lock` | `85FF18D67207BEF644D8C84582124D5EA0988983AC64C51CBA8EB6975FE11D39` |
| Benchmark | Task B Fixture | `benchmarks/fixtures/task_b_fixture.tar.gz` | `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` |
| Benchmark | Golden Repo Commit | Task B Git HEAD | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` |
