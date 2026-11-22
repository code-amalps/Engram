# Engram Known Limitations & Operational Constraints Register

**Document Identifier:** `ENGRAM-GOVERNANCE-LIMITATIONS-20261122`  
**Applies To:** Engram v1.5.4 Candidate 5 & v1.6 Development Baseline  
**Classification:** Operational Governance Register  

---

## 1. Executive Summary

This register formally documents the known technical, empirical, and architectural limitations of the current Engram release baseline. Operating with total transparency ensures engineering teams and auditors understand where Engram excels and where its current operational boundaries lie.

---

## 2. Limitations Catalog

### LIM-01: Single-Turn Overhead in Low-Complexity Workflows (Workload F)
- **Observed Behavior:** In tasks involving single-file modifications with minimal or zero blast radius, Engram introduces a **+0.71% synthetic token overhead** compared to unguided native agent baseline.
- **Root Cause:** Tree-sitter AST extraction, JSON-RPC schema framing, and initial context packaging carry a fixed baseline overhead (~850 bytes). For tasks requiring only a single turn, this overhead cannot be amortized across subsequent conversational turns.
- **Remediation Target:** Phase 8.2 will implement an **Adaptive Task Complexity Classifier** that detects low-complexity tasks at Turn 0 and bypasses MCP tool execution entirely.

### LIM-02: Benchmark Provenance Boundary (`SYNTHETIC_SIMULATED_USAGE`)
- **Observed Behavior:** The reported token efficiency metrics from Phase 5.9.2.5 (e.g. simulated 72.8% reduction across multi-turn loops) were derived from an offline, deterministic simulation harness across 6 synthetic workloads (A through F).
- **Boundary Restriction:** These metrics **do not establish real-world commercial LLM billing savings** (e.g. OpenAI/Anthropic/Google API bills) or live agent latency reductions.
- **Remediation Target:** Phase 9 will conduct live human-in-the-loop trials with real coding agents (Claude Desktop, Cursor, Copilot) to capture actual provider token usage and user-visible turn counts.

### LIM-03: Platform Certification Scope
- **Current Support:** The v1.5.4 Developer Preview binary is packaged and certified exclusively for **Windows x64 (AMD64)** (`x86_64-pc-windows-msvc`).
- **Boundary Restriction:** Linux and macOS packages are not yet published in the release assets.
- **Remediation Target:** Phase 7.1 establishes cross-platform CI matrix builds for Linux x86_64 and macOS ARM64/Intel.

### LIM-04: Dual MCP Tool Surface Complexity
- **Observed Behavior:** By default, `engram mcp` exposes a consolidated 4-tool surface (`engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight`). The remaining 12 specialist tools require passing `--all-tools`.
- **Boundary Restriction:** Agents that attempt to call specialist endpoints (such as `engram_get_impact` or `engram_query_graph`) without `--all-tools` receive a JSON-RPC method not found error (`code -32601`).
- **Remediation Target:** Phase 8.1 will introduce `engram_engage` as a unified single-shot endpoint, eliminating dual-surface confusion while keeping context token overhead minimal.

### LIM-05: Non-Deterministic Linker Timestamps on Windows MSVC Rebuilds (Resolved)
- **Observed Behavior:** Rebuilding the clean Git worktree on Windows MSVC without explicit flags produces identical machine code in `.text` and data in `.data`, but causes a hash variance in SHA-256 due to `TimeDateStamp` and random CodeView PDB GUID (up to 24 bytes total).
- **Resolution:** Resolved in Phase 7.2 / 7.3 by configuring `-C link-arg=/BREPRO` in `.cargo/config.toml`, empirically verified to yield 100% bit-for-bit identical binary hashes (`A6E8AD9A...`).

### LIM-06: Pre-v1.6 Clippy Style Diagnostics on Historical Code
- **Observed Behavior:** On Rust 1.85+ / 2024 edition toolchains, 215 style lints (e.g. `empty_line_after_doc_comments`, `manual_div_ceil`, `collapsible_if`) are reported on pre-existing Phase 1–5 files.
- **Current Mitigation:** All genuine fatal compiler defects (such as raw pointer dereference errors in `src/empirical_live/security.rs`) have been resolved. `cargo clippy --all-targets --all-features` compiles with Exit Code 0. Strict `-D warnings` cleanup is scheduled as part of the v1.6 codebase modernization.

### LIM-07: Legal Counsel Review Requirement (`LEGAL_REVIEW_PENDING`)
- **Observed Behavior:** While technical source correspondence, public Git availability, and third-party license inventories are forensicly verified, qualified external legal counsel has not yet formally reviewed AGPLv3 Section 13 obligations.
- **Governing Posture:** All distribution remains classified as an **Experimental Developer Preview** under `LEGAL_REVIEW_PENDING`. Formal legal counsel sign-off is required prior to non-preview production distribution.

---

## 3. Risk Sign-Off Verdict

All known limitations are documented with explicit remediation milestones scheduled prior to the Friday, November 27, 2026 production release deadline.
