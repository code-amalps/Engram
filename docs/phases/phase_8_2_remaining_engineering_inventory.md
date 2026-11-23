# Engram Phase 8.2: Remaining Engineering & Feature Completion Inventory

**Phase:** 8.2  
**Document Identifier:** `ENGRAM-DOC-PHASE-8-2-INVENTORY-20261123`  
**Phase Type:** Implementation-only / Coding Completion  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**GitHub Repository:** `code-amalps/Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Governing Baseline Candidate:** `v1.5.5-cand1` (source commit: `ded9ca521024d18248235a33e2e292964b79f9b0`)  
**Starting HEAD:** `8a7f1411516e873b4004c0ae3a7bcfd85a156e9c`  
**Target Delivery:** Before November 26, 2026  
**Certification Day Target:** November 26, 2026  
**Release Decision Target:** November 27, 2026  
**Legal Governance Posture:** `TECHNICAL_SOURCE_CORRESPONDENCE_VERIFIED_LEGAL_REVIEW_PENDING`

---

## 1. Executive Summary & Phase Mandate

Phase 8.2 is dedicated to engineering completion. Its sole objective is to implement the remaining planned coding tasks required for the next Engram release before the comprehensive certification day on November 26, 2026.

In accordance with strict operational boundaries:
1. **Zero CI Pipeline Invocations:** No GitHub Actions workflows are triggered, pushed to trigger, watched, or waited upon.
2. **Zero Release Operations:** No Git tags are created or moved, no candidate archives are overwritten, and no public releases are published.
3. **Preservation of Certified Baselines:** The candidate baseline `v1.5.5-cand1` (`ded9ca5`) and candidate archive (`DD67AB9F...`) remain permanently anchored.
4. **Focused Local Verification:** Local verification during Phase 8.2 is strictly focused (`cargo check`, targeted unit/integration tests for touched components, `cargo fmt`). The full regression suite and certification is reserved for November 26.

---

## 2. Inventory Categorization & Prioritization

The remaining engineering inventory has been compiled through comprehensive forensic inspection of:
- `docs/roadmap/ENGRAM_V1_6_EXECUTION_ROADMAP_AND_METRICS.md`
- `docs/release/next_release_blockers.md` (specifically LIM-04 and LIM-06)
- Existing source code in `src/mcp.rs`, `src/hook.rs`, `src/ci_guard.rs`, `src/prompt.rs`, `src/flywheel.rs`
- Candidate release notes and test coverage

### Priority 1 (P1): Mandatory for Functional Completeness & Safety

| Task ID | Component | Title | Description & Rationale |
| :--- | :--- | :--- | :--- |
| **8.2-P1-01** | `src/mcp.rs` | Expose `engram_engage` in default Core MCP surface | In Phase 8.1, `engram_engage` was registered in `all_tools_definitions()` (`--all-tools`), but omitted from `core_tools_definitions()`. Agents running default `engram mcp` could not discover `engram_engage` in `tools/list`. Promoting `engram_engage` into `core_tools_definitions()` expands the lean default surface to 5 tools, directly fulfilling release specification LIM-04 and ensuring immediate agent discoverability. |
| **8.2-P1-02** | `src/hook.rs`<br>`src/ci_guard.rs` | Local Postflight Git Pre-Commit Hook Integration | Fulfills roadmap requirement to shift postflight verification into the local Git pre-commit hook (0 model tokens consumed). When an active Change Contract exists and staged changes satisfy contract boundaries and Project Constitution rules, the pre-commit hook marks postflight verified, resolves recovery sessions, and permits the commit with zero remote API overhead. |
| **8.2-P1-03** | `src/hook.rs`<br>`src/ci_guard.rs`<br>`src/prompt.rs` | Zero-Cost Bypass Protocol & Recovery Guarantees | Implements safe bypass in pre-commit hook for tasks classified as `Trivial` (e.g. single markdown/doc edits or authorized trivial leaf changes) without requiring an active contract, while strictly enforcing sovereign constitution rules. Ensures 0.0% false bypass rate (Metric 6) by failing closed to full contract enforcement if multi-file, architecture, test, or security changes are detected, providing deterministic remediation instructions (`engram preflight`, `engram recover`). |

### Priority 2 (P2): High-Value Engineering Improvements

| Task ID | Component | Title | Description & Rationale |
| :--- | :--- | :--- | :--- |
| **8.2-P2-01** | `.cargo/config.toml` | Build Reproducibility Path Invariance Assessment | Investigate and document compiler path remapping (`--remap-path-prefix`) for Windows MSVC to address detached worktree hash divergence without altering the established `v1.5.5-cand1` compilation baseline. |
| **8.2-P2-02** | Touched Modules | Targeted Rust 2024 / Clippy Cleanliness | Eliminate trivial clippy warnings (such as redundant closures, manual case-folding comparisons, and missing docs) within touched files without destabilizing existing functionality. |

### Excluded / Deferred Items (Phase 9+ Ecosystem Scope)

| Item ID | Target Milestone | Title | Reason for Deferral |
| :--- | :--- | :--- | :--- |
| **8.2-DEF-01** | Phase 9.0 | Graphify Macro-Graph Integration | AMstraLift modernization pipeline and Graphify macro-graph boundary ingestion involve external codebase dependencies and must be validated in an isolated integration environment. |
| **8.2-DEF-02** | Phase 9.0 / 9.1 | Live Cloud Provider Billing Trials | Controlled trials measuring actual input/output token costs across Claude Desktop, Cursor, and VS Code Copilot require live network credentials and credit burn, out of scope for local engineering completion. |
| **8.2-DEF-03** | Phase 10.0 | Multi-Platform Packaging Automation | Cross-platform release archive generation (Linux tarballs, macOS PKG) belongs to the production release packaging milestone. |
| **8.2-DEF-04** | Future (v1.7+) | Embedded Vector / Semantic Search Index | Dynamic semantic embeddings require heavy external ML runtimes or cloud APIs, which contradict Engram's zero-dependency, local-first architecture. |

---

## 3. Implementation Plan & Sequenced Execution

1. **Step 1:** Implement Task 8.2-P1-01 in `src/mcp.rs`. Update `core_tools_definitions()` to include `engram_engage`. Update corresponding tests in `tests/engage_test.rs`.
2. **Step 2:** Implement Task 8.2-P1-02 and 8.2-P1-03 in `src/hook.rs` and `src/ci_guard.rs`. Add trivial bypass recognition and local postflight contract validation. Add targeted tests in `tests/hook_pre_commit_test.rs` and `tests/engage_test.rs`.
3. **Step 3:** Evaluate Task 8.2-P2-01 and Task 8.2-P2-02. Document build reproducibility findings and apply targeted lint cleanups.
4. **Step 4:** Maintain implementation ledger (`docs/phases/phase_8_2_implementation_ledger.md`) and document deferrals (`docs/phases/phase_8_2_deferred_items.md`).
5. **Step 5:** Complete Phase 8.2 completion report (`docs/phases/phase_8_2_completion_report.md`) with explicit handoff for November 26 certification.
6. **Step 6:** Finalize Git commit and push to `feature/phase2-stage5-remediation`.
