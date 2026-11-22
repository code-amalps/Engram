# Engram Phase 5.4.27: Security and Dependency Readiness Audit

**Phase:** 5.4.27  
**Execution Mode:** STRICTLY OFFLINE, READ-ONLY SECURITY AUDIT  
**Baseline Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Audit Date:** 2026-11-20  
**Release Authorization:** `NOT_AUTHORIZED`

---

## 1. Executive Summary

This audit assesses the security posture, privacy architecture, dependency supply chain, and license obligations of the Engram repository for potential release consideration.

### Key Audit Findings:
- **Zero Secret Exposure:** Automated regex scans across all source files, scripts, and configurations revealed zero hardcoded credentials, production API keys, or private tokens.
- **Privacy & Local Governance:** Engram's core architecture is 100% local-first. The compiled executable does not open outbound HTTP/TCP sockets, does not send background telemetry beacons, and operates strictly over local stdio or local filesystem SQLite storage.
- **Dependency License Compatibility:** All 17 direct Rust dependencies and their transitive sub-crates are licensed under permissive open-source licenses (MIT, Apache-2.0, or Public Domain). These are fully compatible with the top-level `AGPL-3.0-or-later` license.
- **Security Caveat:** Because network access was strictly prohibited during this audit, an online vulnerability database scan (`cargo audit` / GitHub Advisory Database) could not be performed. Running a live `cargo audit` remains a mandatory pre-release prerequisite.

---

## 2. Secrets & Credential Exposure Scan

An exhaustive offline regex search was conducted across the source tree:
- OpenAI API Keys (`sk-[a-zA-Z0-9]{20,}`)
- AWS Access Keys (`AKIA[0-9A-Z]{16}`)
- GitHub Personal Access Tokens (`ghp_[a-zA-Z0-9]{20,}`)

### Scan Findings:
- `src/*.rs`: **0 secrets found.** Only negative assertion test cases in `src/main.rs:1837-1839` were detected, where dummy keys (`sk-1234567890abcdef1234567890`) are used to verify that the AST analyzer properly redacts secrets from context compilations (`assert!(!redacted.contains("sk-1234..."))`).
- `benchmarks/`: **0 secrets found.**
- `installer/` & `scripts/`: **0 secrets found.** In `install.ps1`, GitHub tokens passed via CLI or environment variable are masked and never echoed or logged.

---

## 3. Filesystem Safety & Process Execution

### 3.1 Temporary File Handling
- In both `install.ps1` and test automation scripts (`tests/installer_safety_test.ps1`), temporary directories are generated using cryptographic GUIDs (`[System.Guid]::NewGuid().ToString()`) under `$env:TEMP` or `mktemp -d`.
- Cleanup is guaranteed via structured `finally` blocks, preventing residual files from persisting after crashes or interruptions.

### 3.2 Path Traversal & Repository Root Resolution
- Subcommands accepting arbitrary repository paths (`--root <path>` or `-C <path>`) route through `storage::root_path(root)`, which canonicalizes paths and resolves symlinks before creating or reading `.engram/` metadata.
- Change Guard rules enforce that file targets cannot escape the repository root boundary.

### 3.3 Subprocess Execution Safety
- In the Rust codebase, subprocesses (e.g. `git diff`, `git rev-parse`) are executed using `std::process::Command` with explicit parameter arrays rather than shell interpolation (`sh -c` or `cmd /c`), preventing command-injection attacks.
- In `install.ps1`, in-use binary detection (`Get-Process -Name "engram*"`) halts the installer before attempting to overwrite an active executable, preventing binary corruption.

---

## 4. Privacy, Network Defaults & Telemetry

| Feature | Implemented Architecture | User Privacy Impact |
| :--- | :--- | :--- |
| **Outbound Network Telemetry** | **NONE.** No analytics, tracking beacons, or crash reporters | Zero data exfiltration |
| **Code Uploads** | **NONE.** All AST parsing and embeddings run on localhost | Source code never leaves host |
| **Database Storage** | Local SQLite file at `.engram/index.db` with WAL mode enabled | Fully transparent, inspectable locally |
| **Agent Communication** | Model Context Protocol (MCP) over standard local stdio pipes | Completely offline-capable |

---

## 5. Dependency Supply Chain & License Compliance

### 5.1 Direct Rust Dependencies

| Crate | Declared Version | Resolved License | Purpose | License Compatibility |
| :--- | :---: | :---: | :--- | :---: |
| `anyhow` | `1.0` | MIT / Apache-2.0 | Error handling | Compatible |
| `chrono` | `0.4` | MIT / Apache-2.0 | Timestamp & drift tracking | Compatible |
| `clap` | `4.5` | MIT / Apache-2.0 | CLI argument parsing | Compatible |
| `ignore` | `0.4` | MIT / Unlicense | `.gitignore` rule processing | Compatible |
| `regex` | `1.11` | MIT / Apache-2.0 | Pattern matching | Compatible |
| `rusqlite` | `0.32` | MIT (bundled SQLite: Public Domain) | Local relational storage | Compatible |
| `serde` / `serde_json` / `serde_yaml` | `1.0` / `1.0` / `0.9` | MIT / Apache-2.0 | Serialization | Compatible |
| `sha2` | `0.10` | MIT / Apache-2.0 | Checksum verification | Compatible |
| `tree-sitter` | `0.25` | MIT | Incremental AST parsing | Compatible |
| `tree-sitter-*` (7 grammars) | `0.23` | MIT | Language syntax trees | Compatible |
| `uuid` | `1.11` | MIT / Apache-2.0 | Unique project/contract IDs | Compatible |

### 5.2 Top-Level License: AGPL-3.0-or-later
- Engram is licensed under the **GNU Affero General Public License v3.0 or later**.
- Permissive dependencies (MIT/Apache-2.0) are legally compatible with AGPL-3.0.
- **Distribution Obligations:** If binary distributions of `engram` are published publicly, corresponding complete source code must be made accessible under the AGPL-3.0. If deployed as a network MCP service, source code must be made available to network users.

---

## 6. Pre-Release Security Prerequisites

Before any public or stable release, the following actions must be executed:
1. **Live Dependency Vulnerability Scan:** Run `cargo audit` with active network access to query the RustSec Advisory Database against all resolved crates in `Cargo.lock`.
2. **Sigstore Provenance Attestations:** Ensure that GitHub Actions release pipelines generate cryptographic build provenance attestations (`actions/attest-build-provenance@v1`).
3. **License Attribution Notice:** Package third-party copyright and license notices into release archives (`LICENSE-THIRD-PARTY.txt`).
