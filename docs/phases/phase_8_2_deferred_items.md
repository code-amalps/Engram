# Engram Phase 8.2: Documented Deferred Items

**Phase:** 8.2  
**Document Identifier:** `ENGRAM-DOC-PHASE-8-2-DEFERRED-20261123`  
**Phase Type:** Implementation-only / Coding Completion  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**GitHub Repository:** `code-amalps/Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Governing Baseline Candidate:** `v1.5.5-cand1` (source commit: `ded9ca521024d18248235a33e2e292964b79f9b0`)  
**Starting HEAD:** `8a7f1411516e873b4004c0ae3a7bcfd85a156e9c`  
**Date:** 2026-11-23  
**Status:** `ACCEPTED_DEFERRALS`  
**Legal Governance Posture:** `TECHNICAL_SOURCE_CORRESPONDENCE_VERIFIED_LEGAL_REVIEW_PENDING`

---

## 1. Governance Policy on Deferrals

In accordance with the Phase 8.2 mission boundaries:
1. Only tasks strictly necessary for functional completeness, security, and the next-version baseline have been implemented in Phase 8.2.
2. Ecosystem integrations, external API billing trials, packaging automations, and architectural expansions requiring external dependencies are explicitly documented and deferred to their scheduled roadmap milestones (Phase 9.0, Phase 9.1, Phase 10.0, or post-v1.6).

---

## 2. Register of Deferred Engineering Items

### Item 8.2-DEF-01: AMstraLift Modernization & Graphify Macro-Graph Integration
- **Target Milestone:** Phase 9.0 (Ecosystem Integration)
- **Component:** External Engine Bridges (`engram-graphify`, AMstraLift pipeline)
- **Description:** Investigation of bidirectional macro-graph boundary ingestion between Engram AST graph storage and Graphify knowledge graphs, alongside AMstraLift enterprise modernization pipeline verification.
- **Technical Rationale for Deferral:**
  - Graphify schema and external tool contracts require dedicated integration testing in an isolated staging environment.
  - Ingestion of external graph schemas must not introduce dependencies into the standalone Engram Rust core binary.
- **Handoff Condition:** Scheduled for implementation during Phase 9.0 ecosystem validation on Thursday, November 26, 2026.

### Item 8.2-DEF-02: Live Cloud Provider Billing & Telemetry Trials
- **Target Milestone:** Phase 9.0 / 9.1 (Live Provider Validation)
- **Component:** Telemetry & Empirical Accounting Subsystem
- **Description:** Controlled live trials with Claude Desktop, Cursor, and VS Code Copilot measuring actual token and dollar credit burn across standard multi-turn coding sessions.
- **Technical Rationale for Deferral:**
  - Live trials require network access, active third-party cloud billing accounts, and real model invocations.
  - Phase 8.2 is strictly an offline implementation phase. Synthetic benchmark wire-level accounting was certified in Phase 8.1.1.2; empirical live trials belong to Phase 9.1.
- **Handoff Condition:** Scheduled for execution on November 26, 2026.

### Item 8.2-DEF-03: Multi-Platform Release Packaging Automation
- **Target Milestone:** Phase 10.0 (Production Release & Certification)
- **Component:** Release Packaging Subsystem (`.github/workflows/`, release scripts)
- **Description:** Automated cross-compilation and packaging of official release assets for Linux x86_64 (`.tar.gz`), Linux musl static binaries, and macOS universal binaries (`.tar.gz` / `.pkg`), complementing the existing Windows x64 ZIP package.
- **Technical Rationale for Deferral:**
  - Multi-platform compilation and test execution are already fully verified in CI (Phase 7.1 / Phase 8.1.1.4).
  - Binary asset packaging, signing, and release drafting are release operations strictly scheduled for Phase 10.0 on Friday, November 27, 2026.
- **Handoff Condition:** Scheduled for Phase 10.0 publication gate.

### Item 8.2-DEF-04: Cross-Directory Compiler Path Remapping (`--remap-path-prefix`)
- **Target Milestone:** Phase 10.0 (Release Automation Scripts)
- **Component:** `.cargo/config.toml` / CI Build Harness
- **Description:** Normalization of embedded source file paths in debuginfo and panic symbols across arbitrary detached worktree paths.
- **Technical Rationale for Deferral:**
  - Static configuration in `.cargo/config.toml` cannot dynamically expand `$PWD` across different developer OS platforms or CI runner environments.
  - Local MSVC rebuild reproducibility within the repository root is already bit-for-bit identical (`440845D3...`).
  - Path remapping flags are best injected dynamically via `RUSTFLAGS="--remap-path-prefix=$PWD=/engram"` inside the automated release build script rather than hardcoded statically.
- **Handoff Condition:** Scheduled for release build script integration in Phase 10.0.

### Item 8.2-DEF-05: Embedded Semantic Vector Index
- **Target Milestone:** Post-v1.6 / v1.7+ Future Roadmap
- **Component:** Core Search Engine
- **Description:** Dynamic vector embeddings and nearest-neighbor search for fuzzy semantic symbol discovery.
- **Technical Rationale for Deferral:**
  - Embedded vector models (e.g. ONNX runtimes, sentence-transformers) introduce massive binary bloat (50MB–200MB+) and external C/C++ runtime dependencies.
  - Engram's deterministic AST graph parser, exact-match symbol indices, and co-change mining currently deliver sub-10ms response times with zero external dependencies.
- **Handoff Condition:** Re-evaluated for v1.7 after v1.6 production stability.
