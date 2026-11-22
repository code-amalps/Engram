# Phase 5.4.28: Public Alpha Claims, Capabilities and Limitations Matrix

**Phase:** 5.4.28  
**Component:** Release Governance & Technical Claims Specification  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Target Release:** Proposed Engram v1.5.4 Public Alpha  
**Authorization Status:** `NOT_AUTHORIZED`  

---

## 1. Executive Intent and Release Framing

To ensure absolute integrity and protect users, the proposed **Engram v1.5.4 Public Alpha** release must clearly articulate what is technically verified by deterministic evidence versus what remains experimental or unverified.

Engram is positioned as an **indexing engine and context-compacting MCP server for developer experimentation and preview**. Marketing or technical claims asserting guaranteed live LLM cost reduction or production-grade token savings are strictly prohibited until verified by authorized live empirical studies.

---

## 2. Capabilities, Verification Status, and Evidence Matrix

| Feature / Claim | Status | Verification Evidence | Scope & Limitations |
|---|---|---|---|
| **UTF-8 Character Boundary Safety** | **VERIFIED** | `tests/utf8_safety_test.rs` (14/14 PASS) | `safe_truncate_str` prevents byte-slicing crashes across all valid UTF-8 multibyte characters. |
| **16KB MCP Response Budget Shedding** | **VERIFIED** | `tests/inspect_symbol_budget_test.rs` (13/13 PASS) | MCP tool payloads degrade progressively (Levels 0–7) to fit strictly within 16,384 bytes. |
| **SQLite Concurrency & Lock Resilience** | **VERIFIED** | `tests/storage_migration_test.rs` (11/11 PASS) | 15-second `busy_timeout` absorbs transient write lock contention. Not intended for multi-process writer clusters. |
| **Binary Build Provenance CLI** | **VERIFIED** | `tests/build_provenance_test.rs` (5/5 PASS), `engram --version` | Outputs commit SHA, dirty status, rustc version, and build profile. |
| **Installer Integrity & Safety** | **VERIFIED** | `tests/installer_safety_test.ps1` (8/8 PASS) | SHA-256 manifest check, staged extraction, token redaction, and in-use process guards verified. |
| **Toolchain Pinning** | **VERIFIED** | `rust-toolchain.toml`, `cargo check --offline` | Pins stable channel, minimal profile, MSVC target for deterministic builds. |
| **AST Symbol Extraction & Graph** | **VERIFIED** | `tests/change_guard_test.rs` (18/18 PASS) | Tree-sitter parsing and symbol extraction pass on supported language grammars. |
| **Live LLM Token Reduction (Task B)** | **UNVERIFIED** | Task B live benchmark halted/aborted in Phase 5.4.7 & 5.4.13 | **NOT CLAIMED.** Candidate 4.2 reduces local JSON bytes, but live LLM multi-turn token efficiency has not achieved statistical significance against live APIs. |
| **In-Flight Provider Token Bounding** | **UNVERIFIED / CONDITIONAL** | Phases 5.4.24–5.4.26 forensic audits | Once a request is dispatched to a remote LLM provider, token consumption cannot be paused mid-stream by local cancellation. Overruns up to ~31k tokens can occur. |
| **Cross-Platform Tier 1 Binary Release** | **PREVIEW ONLY** | Built and tested on Windows MSVC x86_64 | Linux/macOS workflows are defined in CI, but local testing in this baseline occurred on Windows. |

---

## 3. Public Alpha User Advisory

When releasing v1.5.4, the following advisory must accompany the documentation:

> ### Notice to Users of Engram v1.5.4 Public Alpha
> 
> Engram v1.5.4 is a **Public Alpha** release intended for developers seeking to experiment with AST-driven context indexing and MCP tool integration.
> 
> **Important Considerations:**
> 1. **Do Not Rely on Token Guarantees:** While Engram compacts tool outputs to 16KB, actual token usage depends on how your LLM client parses and formats conversational history.
> 2. **Backup Storage:** While SQLite connection timeouts are configured, Engram uses local disk storage (`~/.cache/engram/`); backup your project indexes when testing critical workflows.
> 3. **Report Issues:** If you encounter encoding issues or tool response anomalies, please report them with `engram --version` output.
