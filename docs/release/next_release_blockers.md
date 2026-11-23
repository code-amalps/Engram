# Engram Next-Version Release Blockers & Operational Limitations Register

**Release Target:** Engram v1.5.5 Candidate 1 (`v1.5.5-cand1`)  
**Assessment Date:** 2026-11-23  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Current Governance Status:** `TECHNICAL_SOURCE_CORRESPONDENCE_VERIFIED_LEGAL_REVIEW_PENDING`  

---

## 1. Classification Policy

Findings are classified according to the following strict release governance criteria:
- **P0 (Release Blocker):** Fatal crash, data corruption, active security vulnerability, test failure on release target, or open-source licensing violation. Binary cannot be released.
- **P1 (Candidate Blocker):** Critical defect in new functionality, untested critical path, or unmitigated performance regression (>20%). Must be resolved or explicitly accepted with documented mitigation before candidate tagging.
- **P2 (Documented Limitation):** Known behavioral constraint, benchmark scope limitation, minor UI/diagnostic inconsistency, or pending external legal counsel review. Must be explicitly documented in release notes.
- **P3 (Deferred Backlog):** Code style cleanups, optimization opportunities, or feature enhancements scheduled for subsequent roadmap milestones.

---

## 2. Summary of Findings

| Severity | Active Count | Resolved Count | Release Decision Impact |
|:---:|:---:|:---:|---|
| **P0** | **0** | 0 | None. All release-critical gates passed. |
| **P1** | **0** | 2 | None. All previously identified P1 issues resolved in Phase 8.1.1.2/8.1.1.3. |
| **P2** | **8** | 0 | Accepted with mandatory documentation in release notes. |
| **P3** | **2** | 0 | Deferred to v1.6 roadmap milestones. |

---

## 3. P0 — Release Blockers (None Active)

*No active P0 blockers exist in the codebase.*
- All unit, integration, and empirical tests compile and pass with 100% success rate.
- Multi-platform CI matrix passes across all 5 target architectures (Windows MSVC, Linux glibc, Linux musl, macOS Intel, macOS ARM64).
- AGPL-3.0 source correspondence verified 100% against public GitHub repository.

---

## 4. P1 — Candidate Blockers (All Resolved)

### Resolved P1-01: Benchmark State Reset Error Suppression
- **Discovery:** In `tests/engage_bench_test.rs`, `reset_workspace_contract_state` suppressed filesystem cleanup errors with `let _ = fs::remove_dir_all(...)`.
- **Resolution:** Re-engineered in Phase 8.1.1.3 to return `std::io::Result<()>`, fail-closed, and verify post-condition directory emptiness (`fs::read_dir().count() == 0`). Added 3 comprehensive unit tests confirming error propagation and idempotency.
- **Status:** **RESOLVED** (Commit verified locally; 5/5 bench tests pass in 135.66s).

### Resolved P1-02: Multi-Platform Floating-Point Assertion Discrepancy
- **Discovery:** Benchmark assertion compared unrounded raw record means against rounded duration stats with an overly tight 0.1% tolerance, causing intermittent test failure on fast Linux runners.
- **Resolution:** Reconciled in Phase 8.1.1.2 commit `2cd8bd2` with separate dual-tolerance assertions (<0.2% for summary means, <1.0% for raw records).
- **Status:** **RESOLVED** (GitHub Actions run `#45150284124` verified green across Linux, macOS, and Windows).

---

## 5. P2 — Documented Operational Limitations (Accepted)

### LIM-01: Single-Turn Token Overhead on Low-Complexity Workflows
- **Description:** For single-turn tasks with single-file edits and zero blast radius, AST parsing and JSON-RPC protocol packaging carry a fixed ~850-byte baseline, resulting in a +0.71% synthetic token overhead.
- **Mitigation:** The `engram_engage` consolidated endpoint implements an automatic `BYPASS_TRIVIAL` classification that short-circuits full AST compilation for trivial leaf edits.
- **Disposition:** Accepted for v1.5.5-cand1; documented in release notes.

### LIM-02: Offline Benchmark Simulation Provenance
- **Description:** Phase 5.9.2.5 token reduction figures were collected using an offline, deterministic test harness across synthetic workloads A through F.
- **Constraint:** These figures demonstrate algorithmic efficiency but do not prove commercial LLM provider bill reductions.
- **Disposition:** Accepted; live provider validation scheduled for Phase 9.

### LIM-03: Windows x64 Binary Packaging Scope
- **Description:** While multi-platform CI validates compilation and tests for Linux and macOS, official pre-packaged binary releases are currently assembled exclusively for Windows x64 (`x86_64-pc-windows-msvc`).
- **Constraint:** Linux and macOS users must compile from source (`cargo install --locked engram`).
- **Disposition:** Accepted for v1.5.5-cand1; cross-platform packaging automation planned for v1.6.

### LIM-04: Dual MCP Surface Complexity
- **Description:** Default MCP server exposes 5 tools (`engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight`, and `engram_engage`). The remaining 12 specialist tools require passing `--all-tools`.
- **Disposition:** Accepted; `engram_engage` is recommended as the unified entry point.

### LIM-05: Reproducible Linker Timestamps
- **Description:** Standard MSVC rebuilds can produce hash variances in PE headers due to linker timestamps and PDB GUIDs.
- **Mitigation:** `-C link-arg=/BREPRO` is configured in `.cargo/config.toml`, guaranteeing bit-for-bit reproducible PE binaries.
- **Disposition:** Resolved and verified in Phase 7.2.

### LIM-06: Pre-v1.6 Clippy Diagnostics on Historical Code
- **Description:** Running `cargo clippy` on Rust 2024 toolchain flags 215 non-fatal style warnings on historical Phase 1–5 files.
- **Mitigation:** All fatal compilation errors and unsafe pointer warnings have been resolved. Clippy exits with code 0 on CI.
- **Disposition:** Accepted; strict `-D warnings` cleanup scheduled for Phase 8.2/8.3.

### LIM-07: External Legal Counsel Review Requirement
- **Description:** Technical source correspondence and license inventories are verified, but formal qualified external legal counsel review of AGPLv3 Section 13 remains pending.
- **Constraint:** Release candidate must be distributed with the explicit classification `EXPERIMENTAL DEVELOPER PREVIEW` under status `TECHNICAL_SOURCE_CORRESPONDENCE_VERIFIED_LEGAL_REVIEW_PENDING`.
- **Disposition:** Accepted; legal review remains open until formal counsel sign-off.

### LIM-08: Persistent MCP Daemon Scope in Live Benchmark
- **Description:** The 150-pair live stdio benchmark was executed against a persistent `engram mcp` daemon process over OS pipes, accurately modeling IDE integration but not measuring cold binary startup latency.
- **Disposition:** Accepted; cold-start performance is documented as uncertified.

---

## 6. P3 — Deferred Backlog Items

### DEF-01: Modernization of Historical Style Diagnostics
- Refactor historical Phase 1–5 source code to resolve all 215 Rust 2024 edition Clippy suggestions (e.g. `manual_div_ceil`, `collapsible_if`). Scheduled for v1.6 codebase cleanup.

### DEF-02: Automated Multi-Platform Release Packaging
- Enhance `.github/workflows/ci.yml` to automatically build, test, package (`.tar.gz`, `.zip`), and upload release artifacts for Linux x86_64, macOS ARM64, and macOS Intel upon release tag creation.

---

## 7. Sign-Off & Release Readiness Conclusion

With **0 active P0 release blockers** and **0 active P1 candidate blockers**, Engram satisfies all criteria for the candidate milestone:
### Final Disposition: **`PROCEED TO RELEASE CANDIDATE PACKAGING (v1.5.5-cand1)`**
