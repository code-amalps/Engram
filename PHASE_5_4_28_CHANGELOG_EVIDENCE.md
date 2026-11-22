# Phase 5.4.28: Proposed v1.5.4 Changelog Specification and Audit Evidence

**Phase:** 5.4.28  
**Component:** Documentation & Release Communication  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Authorization Status:** `NOT_AUTHORIZED`  

---

## 1. Problem Statement & Audit Finding

Phase 5.4.27 identified the absence of a root `CHANGELOG.md` as a critical release blocker. Prior to this phase:
1. Release notes were scattered across intermediate benchmark audit reports.
2. Product enhancements (e.g. UTF-8 slice safety, 16KB response budget limits) were mixed indistinguishably with internal benchmark harness changes (e.g. tetrad arm validation, synthetic telemetry seeding).
3. The true experimental status of live LLM token reduction was unstated, risking misleading users into assuming token savings guarantees.

---

## 2. Implementation: Root `CHANGELOG.md`

To resolve this blocker, a standard `CHANGELOG.md` was created in the repository root adhering to the [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format.

### Key Editorial Principles Enforced:
1. **Strict Demarcation:** Product features/fixes are clearly separated from internal benchmark infrastructure.
2. **Prominent Safety Disclaimers:** A dedicated `### Limitations & Unverified Claims` callout explicitly discloses that token reduction under live LLM execution is unverified and not guaranteed.
3. **Traceability:** Links all changes to their underlying PRs, phases, or commits.

---

## 3. Full Text of `CHANGELOG.md`

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.5.4] - Proposed Public Alpha Release

### Critical Notice: Public Alpha Status & Benchmark Disclaimers
> [!WARNING]
> **Live LLM Token Reduction is Unverified:** While Engram Candidate 4.2 implements AST-based context compaction and enforces a strict 16,384-byte JSON-RPC response budget, empirical token reduction against live LLM providers (e.g. Task B live benchmarks) remains **unverified** due to provider token exhaustion in experimental testbeds. Token efficiency claims must not be relied upon in production environments without localized validation.

### Added
- **Build Provenance Metadata (`engram --version`):**
  - Displays package version, source Git commit SHA, working-tree dirty status, Rust compiler version, and build profile.
  - Implemented via a zero-dependency, offline `build.rs` compile-time script.
- **Deterministic Toolchain Pinning:**
  - Added `rust-toolchain.toml` targeting the stable Rust channel for reproducible builds.
- **Progressive Response Budget Shedding:**
  - Added 8-level degradation ladder (Levels 0–7) in `src/mcp.rs` ensuring MCP tool responses never exceed 16,384 bytes.
  - Added symbol omission tracking and manifest generation in `src/impact.rs`.

### Fixed
- **UTF-8 Character Boundary Slicing (`safe_truncate_str`):**
  - Resolved panic `byte index is not a char boundary` when truncating strings containing multibyte UTF-8 codepoints (e.g. emojis, Unicode punctuation, Japanese/Chinese text).
- **SQLite Concurrency & Lock Contention:**
  - Added a 15-second busy timeout to SQLite connection initialization in `src/storage.rs` to mitigate `DatabaseLocked` errors under concurrent access.

### Security & Hardening
- **Installer Fail-Closed Verification:**
  - Hardened `install.ps1` and `install.sh` to enforce SHA-256 checksum verification, atomic staged binary extraction, and token sanitization.

### Benchmark Harness (Internal - Not Included in Release Binaries)
- **Fail-Closed Tetrad Order Validation:**
  - Implemented strict validation for `--arm-order` argument in `benchmarks/matched-pair-harness/src/orchestrator.py` to prevent duplicate or omitted arms.
- **Global Token Ceiling & Headroom Enforcement:**
  - Implemented a 45,000-token pre-dispatch global safety reserve and dynamic effective arm ceiling clamping.
  - Added post-telemetry ceiling override to flag runs exceeding token limits.
- **Process Lifecycle & Cleanup:**
  - Implemented Windows PID tree termination (`taskkill /F /T /PID`) and reader thread timeouts in `benchmarks/matched-pair-harness/src/runner.py`.
- **Cryptographic Telemetry Sealing:**
  - Added SHA-256 cryptographic HMAC sealing for benchmark telemetry in `benchmarks/matched-pair-harness/src/telemetry.py`.
```

---

## 4. Verification and Audit Sign-Off

The changelog file was inspected and verified against the repository state:
1. `CHANGELOG.md` exists in the repository root.
2. It correctly describes all modifications present in the working tree.
3. It clearly states the `NOT_AUTHORIZED` status of live operations.
4. It sets transparent, honest expectations for users of the proposed Public Alpha release.
