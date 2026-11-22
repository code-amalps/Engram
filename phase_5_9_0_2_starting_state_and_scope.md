# Engram Phase 5.9.0.2: Starting State and Scope of Final Certification

**Phase:** 5.9.0.2  
**Execution Mode:** STRICTLY OFFLINE, READ-ONLY ADVERSARIAL CERTIFICATION  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Expected Git HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Expected Branch:** `feature/phase2-stage5-remediation`  
**Target Candidate:** Candidate 5 (Phase 5.5 Optimized Context Engine + Phase 5.8.1 Cache Invalidation Fix)  
**Live Dispatch Authorization:** `NOT_AUTHORIZED`  
**Network / Cargo Offline:** `CARGO_NET_OFFLINE=true`  
**Source / Test Modifications:** `NOT_AUTHORIZED`  
**Binary Modification / Rebuild:** `NOT_AUTHORIZED`  
**Git Commit / Push / Tag:** `NOT_AUTHORIZED`  

---

## 1. Mission and Adversarial Objectives

Phase 5.9.0.2 is the final adversarial verification and evidence-closure phase prior to any separate human decision on live pentad benchmark dispatch.

The mission is to independently verify every factual and mathematical claim made in Phase 5.9.0 and Phase 5.9.0.1, specifically:
1. **Candidate 5 Headroom Arithmetic Correction:** Resolve the arithmetic flaw in Phase 5.9.0.1 regarding the "467,232 tokens" claim and distinguish `global_remaining` from `dispatchable_headroom`.
2. **Actual Enforcement of Dynamic Aggregate Budget:** Trace CLI arguments from `main()` to runtime execution in `orchestrator.py` and `runner.py`, proving how `--global-tokens-ceiling`, `--max-tokens-ceiling`, and `--headroom-reserve` operate across processes.
3. **Reconciliation of Verification Script Failures:** Fully investigate the two exit code 1 failures in Phase 5.9.0.1 and establish their exact technical root causes.
4. **Authoritative Fixture & Path Authority:** Audit the physical JSON fixture bytes and establish historical continuity between the cost-baseline tasks and the modern matched-pair fixtures.
5. **Acceptance Contract Audit:** Distinguish static regex/typecheck inspection from dynamic runtime browser execution in `evaluator.py`.
6. **Provider Telemetry & Fail-Closed Recovery:** Audit streaming event deduplication, token extraction, and process crash recovery.
7. **Financial Exposure Recalculation:** Clearly separate subscription quota deductions from commercial-equivalent pay-as-you-go rates.

---

## 2. Invariants & Repository Baseline Snapshot

| Invariant Check | Required Specification | Measured Actual Value | Forensic Status |
|---|---|---|---|
| **Repository Root** | `C:\Users\Amal P S\OneDrive\Projects\Engram` | `C:\Users\Amal P S\OneDrive\Projects\Engram` | **VERIFIED** |
| **Git Commit HEAD** | `7106a2f369fe79faf5f95449378a900ab8e8eddf` | `7106a2f369fe79faf5f95449378a900ab8e8eddf` | **VERIFIED** |
| **Branch** | `feature/phase2-stage5-remediation` | `feature/phase2-stage5-remediation` | **VERIFIED** |
| **Tracked Modifications** | Exactly 17 files modified | Exactly 17 files modified (`git status --short`) | **VERIFIED** |
| **Staged Diff Bytes** | 0 bytes (`git diff --cached` empty) | 0 bytes | **VERIFIED** |
| **Golden Repo Commit** | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` | **VERIFIED** |
| **Candidate 3 Hash** | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | **VERIFIED** |
| **Candidate 4.2 Hash** | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | **VERIFIED** |
| **Candidate 5 Hash** | `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` | `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` | **VERIFIED** |
| **Task A Fixture Hash** | `357D9B4B5777A41A8C5691475DCA8DDAC00134C480C6324E26B60FEA39F95F02` | `357D9B4B5777A41A8C5691475DCA8DDAC00134C480C6324E26B60FEA39F95F02` | **VERIFIED** |
| **Task B Fixture Hash** | `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` | `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` | **VERIFIED** |
| **Codex CLI Binary** | `~/.codex/plugins/.plugin-appserver/codex.exe` | Version: `codex-cli 0.167.0-alpha.4.1` | **VERIFIED** |

---

## 3. Strict Boundary Discipline

No production source code, unit tests, candidate binaries, or historical fixtures will be modified during Phase 5.9.0.2. All evidence will be documented through non-destructive, read-only analysis and persisted in explicit Phase 5.9.0.2 deliverables.
