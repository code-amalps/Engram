# Engram Phase 5.9.0.6: Starting State and Scope Report

**Phase:** 5.9.0.6  
**Execution Mode:** STRICTLY OFFLINE IMPLEMENTATION, TESTING & FORENSIC VERIFICATION  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Git HEAD Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Date:** 2026-11-22  

---

## 1. Executive Summary

Phase 5.9.0.6 was commissioned following the forensic determinations of Phase 5.9.0.5. The objective of Phase 5.9.0.6 is strictly implementation-driven and forensic:
1. **Workstream A:** Implement defensible pre-dispatch request-size protection in `benchmarks/matched-pair-harness/src/runner.py` that measures complete outbound requests, operates fully offline via `tiktoken` (`o200k_base` with `cl100k_base` and conservative heuristic fallback), enforces a hard reject-only boundary (~60,000 tokens), and fails closed without spawning child processes.
2. **Workstream B:** Reconcile aggregate budget conservation under $C_B = \min(1,500,000, 2,500,000 - T_A)$ without reintroducing the 62,768 double-counting error, proving maximum exposure $E_{\max} = 2,500,000 + O_B \le 2,531,384$ tokens, and provide a comprehensive coordinator script `benchmarks/matched-pair-harness/scripts/run_two_task_pentad.ps1` enforcing a 14-point validation gate.
3. **Workstream C:** Implement standardized round-trip telemetry in `benchmarks/matched-pair-harness/src/telemetry.py` distinguishing agent turns, Engram MCP tool calls, standard tool executions, round trips, retries, payload byte sizes, and statistical tool execution latency (mean, median/p50, p95, min, max).
4. **Workstream D:** Update `benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json` to ensure full validation of request-size gate and round-trip telemetry while maintaining 100% backward compatibility.
5. **Workstream E:** Execute full offline test suites (unit, budget, round-trip, safety, regression) with zero network calls and zero live provider dispatch.

---

## 2. Baseline Repository State Verification

Prior to modifying any files in Phase 5.9.0.6, the environment was audited:

| Component | Expected Baseline | Actual Verified Value | Verification Status |
| :--- | :--- | :--- | :---: |
| **Git HEAD** | `7106a2f369fe79faf5f95449378a900ab8e8eddf` | `7106a2f369fe79faf5f95449378a900ab8e8eddf` | **VERIFIED MATCH** |
| **Branch** | `feature/phase2-stage5-remediation` | `feature/phase2-stage5-remediation` | **VERIFIED MATCH** |
| **Tracked Modified Files** | Exactly 17 files from earlier phases | 1 staged (`Cargo.toml`), 16 unstaged | **VERIFIED MATCH** |
| **Golden Testbed HEAD** | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` | **VERIFIED MATCH** |
| **Candidate 3 Binary SHA-256** | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | **VERIFIED MATCH** |
| **Candidate 4.2 Binary SHA-256** | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | **VERIFIED MATCH** |
| **Candidate 5 Binary SHA-256** | `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` | `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` | **VERIFIED MATCH** |
| **Task A Fixture SHA-256** | `357D9B4B5777A41A8C5691475DCA8DDAC00134C480C6324E26B60FEA39F95F02` | `357D9B4B5777A41A8C5691475DCA8DDAC00134C480C6324E26B60FEA39F95F02` | **VERIFIED MATCH** |
| **Task B Fixture SHA-256** | `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` | `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` | **VERIFIED MATCH** |

---

## 3. Scope of Work

The scope of Phase 5.9.0.6 is strictly bounded:
1. **Source Code Modifications:** Confined strictly to `benchmarks/matched-pair-harness/src/runner.py`, `src/telemetry.py`, `src/orchestrator.py`, and `schemas/run_telemetry.schema.json`.
2. **Scripts Created:** `benchmarks/matched-pair-harness/scripts/run_two_task_pentad.ps1`.
3. **Tests Created:** `test_request_size_gate.py`, `test_round_trip_telemetry.py`, and `test_budget_coordinator.py`.
4. **Prohibited Activities:**
   - Zero live network access (`CARGO_NET_OFFLINE=true`, no OpenAI API requests, no Codex live server calls).
   - Zero modifications to candidate binaries or task fixtures.
   - Zero Git commits, tags, pushes, or merges.
   - Zero grant of live benchmark authorization (final verdict must be `IMPLEMENTATION_COMPLETE_READY_FOR_OFFLINE_FORENSIC_AUDIT`).
