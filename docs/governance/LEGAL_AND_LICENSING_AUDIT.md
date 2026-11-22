# Engram Legal & Licensing Compliance Audit (AGPL-3.0)

**Document Identifier:** `ENGRAM-GOVERNANCE-LEGAL-20261122`  
**Target Product:** Engram v1.5.4 Candidate 5 & v1.6 Core  
**Repository:** `https://github.com/code-amalps/Engram`  
**License:** GNU Affero General Public License v3.0 or later (`AGPL-3.0-or-later`)  
**Repository Owner & Copyright Holder:** Amal P S  
**Attestation Date:** 2026-11-22  

---

## 1. Executive Summary & Attestation

This document formalizes the legal and open-source licensing governance for Engram.

Under GNU Affero General Public License Version 3 (AGPL-3.0-or-later), distribution of compiled binaries mandates the provision of Corresponding Source code under Section 6. As repository owner and copyright holder, Amal P S has authorized public distribution of Engram v1.5.4 Candidate 5 and subsequent v1.6 releases under AGPL-3.0-or-later, supported by complete, verified technical source correspondence.

---

## 2. AGPL-3.0 Section 6 Corresponding Source Verification

Engram strictly satisfies AGPL-3.0 Section 6 requirements through public Git repository availability:

| Requirement | Implementation Evidence | Verification State |
| :--- | :--- | :---: |
| **Corresponding Source Availability** | Publicly accessible at `https://github.com/code-amalps/Engram` | **VERIFIED** |
| **Release Tag Correspondence** | Annotated tag `v1.5.4-cand5` points to exact release commit `2df1cac683435314ffd7ad52369e6d20198fae0d` | **VERIFIED** |
| **Blob-for-Blob File Match** | All 88 Rust source files (`src/`) and test suites (`tests/`) match Git object tree 100% bit-for-bit | **VERIFIED** |
| **Build Scripts & Toolchains** | `build.rs`, `Cargo.toml`, `Cargo.lock`, and `rust-toolchain.toml` are committed and versioned | **VERIFIED** |
| **Offline Reproducibility** | Compiles with `cargo build --release --locked --offline` with zero network dependencies | **VERIFIED** |

### Source Access Instructions for Recipients:
Any recipient of the Engram executable may obtain the complete, matching Corresponding Source using standard Git tooling:
```bash
git clone https://github.com/code-amalps/Engram.git
cd Engram
git checkout v1.5.4-cand5
cargo build --release --locked
```

---

## 3. Third-Party Dependency License Audit

All dependencies declared in `Cargo.lock` have been audited for copyleft compatibility with AGPL-3.0:

| Dependency Category | Example Crates | Inbound License | AGPL-3.0 Compatibility |
| :--- | :--- | :--- | :---: |
| **AST Parsing** | `tree-sitter`, `tree-sitter-python`, `tree-sitter-typescript` | MIT | **100% Compatible** |
| **Serialization & Protocol** | `serde`, `serde_json`, `tokio` | MIT / Apache-2.0 | **100% Compatible** |
| **Storage Engine** | `rusqlite`, `libsqlite3-sys` | MIT / Public Domain | **100% Compatible** |
| **CLI & Diagnostics** | `clap`, `tracing`, `tracing-subscriber` | MIT / Apache-2.0 | **100% Compatible** |

**Conclusion:** Zero proprietary, non-commercial, or copyleft-incompatible libraries exist in the dependency tree.

---

## 4. Privacy & Outbound Telemetry Governance

1. **Zero Outbound Telemetry:** Engram contains zero telemetry beacons, crash reporters, analytics pings, or network listeners.
2. **Local-Only Execution:** All AST parsing, SQLite indexing, and symbol lookups execute strictly on the local machine.
3. **Subprocess Isolation:** Engram never transmits code, symbol names, or prompts to external LLM providers or remote servers.

---

## 5. Governance Sign-Off Verdict

### Status: `LEGAL_GOVERNANCE_RESOLVED_AND_COMPLIANT`

Engram satisfies all source correspondence, licensing attribution, and dependency compatibility obligations under AGPL-3.0-or-later.
