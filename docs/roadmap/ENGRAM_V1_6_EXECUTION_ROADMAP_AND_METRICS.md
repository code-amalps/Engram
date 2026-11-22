# Engram v1.6 Execution Roadmap & Empirical Success Criteria

**Document Identifier:** `ENGRAM-V1-6-ROADMAP-20261122`  
**Current Baseline Commit:** `2df1cac683435314ffd7ad52369e6d20198fae0d` (`v1.5.4-cand5`)  
**Target Milestone:** Engram v1.6.0 Production Release  
**Target Deadline:** Friday, November 27, 2026  
**Core Thesis:** *Measurable task-level efficiency without sacrificing functional correctness or architectural safety.*

---

## 1. Executive Sequence

This roadmap implements the authoritative 7-phase sequence to take Engram from the current Windows-only Developer Preview to a fully verified, cross-platform, efficiency-first production engine:

1. **Phase 7.0: Release Governance & Operational Baseline**
   - Resolve legal review documentation under AGPL-3.0.
   - Formalize documented limitations register (single-turn overhead, synthetic benchmarks).
   - Establish a reproducible, automated clean-checkout validation harness.
2. **Phase 7.1: Cross-Platform CI Validation**
   - Add Linux x86_64 and macOS ARM64/Intel build-and-test matrix in GitHub Actions.
   - Maintain Windows x64 as the release baseline; validate compilation and test suites across all OS targets without auto-publishing.
3. **Phase 7.2: Windows Binary Reproducibility Investigation**
   - Implement `/BREPRO` linker flags and audit PDB CodeView GUID generation.
   - Achieve byte-for-byte reproducibility across clean rebuilds while preserving published historical hashes.
4. **Phase 8.0: Turn-0 Static Context Injection Prototype**
   - Prototype direct injection of workspace architecture, rules, and symbols into `.github/copilot-instructions.md`, `.cursorrules`, and `CLAUDE.md`.
   - Measure reduction in Turn-1 and Turn-2 exploratory MCP calls.
5. **Phase 8.1: Consolidated Atomic Endpoint (`engram_engage`)**
   - Prototype atomic context retrieval and preflight handshake in a single roundtrip.
   - Compare performance, turns, and context consumption against the lean 4-tool surface.
6. **Phase 8.2: Local Postflight Hook & Complexity Classification**
   - Shift postflight verification into a local Git pre-commit hook (0 model tokens consumed).
   - Implement zero-cost bypass for trivial single-file edits with strict fallback and recovery guarantees.
7. **Phase 9: Ecosystem Integration & Live Provider Validation**
   - Integrate AMstraLift modernization pipeline and investigate Graphify macro-graph boundaries.
   - Run controlled live trials with Claude Desktop, Cursor, and VS Code Copilot measuring actual token and credit consumption.
8. **Phase 10: v1.6.0 Production Release & Certification**
   - Package multi-platform release assets (Windows, Linux, macOS) and publish official GitHub Release `v1.6.0` by Friday, Nov 27.

---

## 2. Mandatory v1.6 Success Criteria (8 Core Empirical Metrics)

Every v1.6 optimization must be empirically measured against the baseline without Engram and against the v1.5.4 Developer Preview. Synthetic benchmark results cannot serve as proxy for live provider savings.

| Metric | Measurement Unit | Target Objective | What It Establishes |
| :--- | :---: | :---: | :--- |
| **1. Actual Model Input/Output Tokens** | Tokens (Prompt + Completion) | $\ge 40\%$ reduction on multi-turn tasks | **Real Token Consumption:** Closes the gap between synthetic wire savings and real LLM provider bills. |
| **2. Number of Model Turns** | Turn count $T$ | $T_{\text{engram}} \le T_{\text{baseline}}$ | **Interaction Overhead:** Eliminates conversational inflation caused by iterative tool polling. |
| **3. Number of Tool Calls** | Count per task | $\ge 50\%$ reduction vs v1.5.4 | **MCP Interaction Tax:** Eliminates redundant exploratory calls via Turn-0 injection and atomic endpoints. |
| **4. Task Acceptance Rate** | % tests passing | $100\%$ parity with baseline | **Correctness Preservation:** Ensures efficiency gains never break working code or tests. |
| **5. Missed-Impact Rate** | % unflagged breaking edits | $0.0\%$ (Zero missed impacts) | **Architectural Safety:** Ensures blast-radius analysis correctly flags callers and dependents. |
| **6. False Bypass Rate** | % complex tasks skipped | $0.0\%$ (Zero false bypasses) | **Classifier Reliability:** Ensures architectural edits are never mistakenly classified as trivial. |
| **7. Validation Overhead** | Milliseconds ($ms$) | $\le 15\text{ ms}$ per local invocation | **Local Execution Cost:** Ensures developer machine remains ultra-responsive with native Rust execution. |
| **8. Recovery Success Rate** | % breaches self-healed | $\ge 90\%$ surgical recovery | **Failure Resilience:** Proves the change-guard enables agents to recover cleanly from contract violations. |

---

## 3. Daily Timeline Matrix (Target: Friday, November 27, 2026)

| Target Date | Phases | Key Deliverables | Gate Condition |
| :--- | :--- | :--- | :--- |
| **Mon Nov 23** | **Phase 7.0, 7.1, 7.2** | Legal review documentation, Cross-platform CI workflow, Windows `/BREPRO` determinism audit. | CI tests pass on Windows, Linux, and macOS. |
| **Tue Nov 24** | **Phase 8.0** | `engram init-context`, Turn-0 instructions generation, baseline exploratory accounting. | $\ge 1$ turn eliminated on multi-turn loops. |
| **Wed Nov 25** | **Phase 8.1, 8.2** | `engram_engage` tool, Git pre-commit hook guard, Adaptive complexity classifier. | 0 false bypasses; 0 tokens burned for valid commits. |
| **Thu Nov 26** | **Phase 9.0** | AMstraLift modernization integration, Graphify bridge, Live Claude/Cursor A/B benchmark. | Empirical proof across the 8 mandatory metrics. |
| **Fri Nov 27** | **Phase 10.0** | v1.6.0 production binaries (Win/Linux/macOS), SHA256 manifests, certification report. | **Production Release Published [DEADLINE]** |
