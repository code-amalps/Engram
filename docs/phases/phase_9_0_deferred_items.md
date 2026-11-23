# Engram Phase 9.0: Documented Deferred Items

**Phase:** 9.0  
**Document Identifier:** `ENGRAM-DOC-PHASE-9-0-DEFERRED-20261123`  
**Phase Type:** Integration Implementation / Coding Completion  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**GitHub Repository:** `code-amalps/Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Governing Baseline Candidate:** `v1.5.5-cand1` (source commit: `ded9ca521024d18248235a33e2e292964b79f9b0`)  
**Starting HEAD:** `d01f84d4ac358d8f6c9ec396730b5eec0d877929`  
**Date:** 2026-11-23  
**Status:** `ACCEPTED_DEFERRALS`  
**Legal Governance Posture:** `TECHNICAL_SOURCE_CORRESPONDENCE_VERIFIED_LEGAL_REVIEW_PENDING`

---

## 1. Governance Policy on Deferrals

In accordance with the Phase 9.0 mission boundaries:
1. All core integration models, zero-trust validators, contract generators, evidence verifiers, macro-graph parsers, staleness evaluators, in-memory indexes, and CLI subcommands have been implemented in pure Rust without external runtime dependencies.
2. External live network execution, live GitHub API pull-request publishing, live cloud LLM trials, and background Python daemon daemons are explicitly documented and deferred to their scheduled milestones (Phase 9.1 or Phase 10.0).

---

## 2. Register of Deferred Engineering Items

### Item 9.0-DEF-01: Live AMstraLift Stage B Automated Remote PR Publishing
- **Target Milestone:** Phase 9.1 (Live Ecosystem Verification)
- **Component:** AMstraLift Publisher Integration (`amstralift.publisher.github`)
- **Description:** Executing an end-to-end modernization run where AMstraLift Stage B pushes a branch to GitHub and opens a pull request that triggers Engram's CI Change Guard (`engram guard --ci`).
- **Technical Rationale for Deferral:**
  - Phase 9.0 is strictly an offline implementation and integration phase.
  - Remote Git pushing and GitHub PR creation require live external credentials (e.g. `GITHUB_TOKEN`) and network connectivity.
  - Engram's contract ingestion (`generate_contract_for_plan`) and evidence verification (`verify_transformation_evidence`) are fully tested and proven locally with deterministic fixtures.
- **Handoff Condition:** Scheduled for live end-to-end trial during Phase 9.1 validation.

### Item 9.0-DEF-02: Real-Time Incremental Graphify Generation Daemon
- **Target Milestone:** Post-v1.6 Architecture (Engram v1.7 / Graphify v2)
- **Component:** Dynamic Knowledge Graph Synchronizer
- **Description:** A background daemon that automatically invokes Graphify extraction scripts on file mutation events and incrementally updates `graphify-out/graph.json`.
- **Technical Rationale for Deferral:**
  - Graphify is currently an on-demand static generation skill invoked by agents or developers via Python scripts.
  - Embedding a background Python execution daemon into Engram would violate Engram's lightweight zero-dependency architecture.
  - Engram's current implementation handles static Graphify artifacts cleanly, detects staleness against Git HEAD, and degrades gracefully to native SQLite AST when graph files are absent.
- **Handoff Condition:** Evaluated for post-v1.6 architecture after stability certification.

### Item 9.0-DEF-03: Multi-Platform Release Packaging Automation
- **Target Milestone:** Phase 10.0 (Production Release & Certification)
- **Component:** Release Packaging Subsystem (`.github/workflows/`, release scripts)
- **Description:** Automated cross-compilation and packaging of official release assets for Linux x86_64 (`.tar.gz`), Linux musl static binaries, and macOS universal binaries (`.tar.gz` / `.pkg`), complementing the existing Windows x64 ZIP package.
- **Technical Rationale for Deferral:**
  - Cross-compilation and multi-platform testing are already verified in CI.
  - Packaging automation is strictly scheduled for the Phase 10.0 release pipeline on November 27, 2026.
- **Handoff Condition:** Retained for Phase 10.0.
