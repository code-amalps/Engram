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

Under GNU Affero General Public License Version 3 (AGPL-3.0-or-later), distribution of compiled binaries mandates the provision of Corresponding Source code under Section 6. As repository owner and copyright holder, Amal P S explicitly authorized public publication of the Engram v1.5.4 Candidate 5 developer preview. Any future public release (including v1.6 milestones) requires separate, fresh owner authorization. While technical source correspondence has been forensicly verified, open-source legal compliance remains subject to independent qualified legal counsel review.

---

## 2. AGPL-3.0 Section 6 Corresponding Source Verification

Engram provides technical source correspondence through public Git repository availability:

| Requirement | Implementation Evidence | Verification State |
| :--- | :--- | :---: |
| **Corresponding Source Availability** | Publicly accessible at `https://github.com/code-amalps/Engram` | **VERIFIED** |
| **Release Tag Correspondence** | Annotated tag `v1.5.4-cand5` points to exact release commit `2df1cac683435314ffd7ad52369e6d20198fae0d` | **VERIFIED** |
| **Blob-for-Blob File Match** | All 88 Rust source files (`src/`) and test suites (`tests/`) match Git object tree 100% bit-for-bit | **VERIFIED** |
| **Build Scripts & Toolchains** | `build.rs`, `Cargo.toml`, `Cargo.lock`, and `rust-toolchain.toml` are committed and versioned | **VERIFIED** |
| **Offline Reproducibility** | Compiles with `cargo build --release --locked --offline` with zero network dependencies | **VERIFIED** |

### Source Access Instructions for Recipients:
Any recipient of the Engram executable may obtain the matching source tree using standard Git tooling:
```bash
git clone https://github.com/code-amalps/Engram.git
cd Engram
git checkout v1.5.4-cand5
cargo build --release --locked
```

---

## 3. Third-Party Dependency License Audit

All dependencies declared in `Cargo.lock` have been inventoried for licensing terms:

| Dependency Category | Example Crates | Inbound License | Inventory Status |
| :--- | :--- | :--- | :---: |
| **AST Parsing** | `tree-sitter`, `tree-sitter-python`, `tree-sitter-typescript` | MIT | Permissive Open Source |
| **Serialization & Protocol** | `serde`, `serde_json`, `tokio` | MIT / Apache-2.0 | Permissive Open Source |
| **Storage Engine** | `rusqlite`, `libsqlite3-sys` | MIT / Public Domain | Permissive Open Source |
| **CLI & Diagnostics** | `clap`, `tracing`, `tracing-subscriber` | MIT / Apache-2.0 | Permissive Open Source |

**Technical Finding:** Zero proprietary or non-commercial licenses were discovered in the dependency inventory. Formal legal compatibility opinions remain subject to qualified legal counsel review.

---

## 4. Privacy & Outbound Telemetry Governance

1. **Zero Outbound Telemetry:** Engram contains zero telemetry beacons, crash reporters, analytics pings, or network listeners.
2. **Local-Only Execution:** All AST parsing, SQLite indexing, and symbol lookups execute strictly on the local machine.
3. **Subprocess Isolation:** Engram never transmits code, symbol names, or prompts to external LLM providers or remote servers.

---

## 5. Governance Sign-Off Verdict

### Status: `TECHNICAL_SOURCE_CORRESPONDENCE_VERIFIED_LEGAL_REVIEW_PENDING`

Technical source correspondence, public Git availability, and third-party license inventories are forensicly verified. Formal legal counsel review remains pending and must be resolved prior to final non-preview production distribution.
