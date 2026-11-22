# Engram Milestone 5G — Pilot Readiness Assessment
## Comprehensive Technical, Verification, and Safety Audit

**Date:** 2026-11-04  
**Milestone:** 5G — Live Pilot Readiness Package  
**Status:** REMEDIATED / READY WITH QUALIFICATIONS — OWNER REVIEW REQUIRED  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Git Branch:** `feature/async-precommit-guard`  
**Git HEAD:** `c70845b4d5913457f1b176fb0370b80dd792711f`  

---

## 1. Executive Summary & Readiness Verdict

Following the completion of the Milestone 5G independent audit and subsequent remediation of identified documentation blockers, all technical and operational prerequisites for a matched-pair live pilot have been reconciled and verified against the actual source implementation:

1. **Candidate Binary Path Verified:** Fixed path to `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` with verified SHA-256 `F56347EF...`.
2. **Task Specification Aligned:** Protocol and evaluator documentation fully rewritten to reflect the actual Cargo/Rust task (`task_commit_cache_invalidation`) targeting `src/compiler.rs`, `src/analyzer.rs`, `src/main.rs`, and `tests/commit_cache_invalidation_test.rs`.
3. **Repository Integrity & Worktree Isolation Proven:** Golden repository paths and working tree states are documented transparently without false cleanliness claims. Detached Git worktree provisioning from commit `c70845b4...` guarantees execution against a pristine baseline.
4. **Token Headroom Configuration Reconciled:** The launch command explicitly incorporates `--headroom-reserve 15000` to override the 45k harness default, enforcing the verified 160k pre-dispatch threshold against the 175k ceiling.
5. **Runtime Qualifications Preserved:** The unverifiability of `gpt-5-codex` offline and the necessity of live runtime observation for MCP process exposure are explicitly maintained.

### Formal Readiness Verdict
$$\mathbf{READY\ WITH\ QUALIFICATIONS\ —\ OWNER\ REVIEW\ REQUIRED}$$

*(Live execution remains strictly prohibited. Only the repository owner can grant explicit authorization after reviewing this package.)*

---

## 2. Evidence Verification Summary

### 2.1 Git Branch & Primary Repository Integrity
- **Active Git Branch:** `feature/async-precommit-guard`
- **Current HEAD Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`
- **Primary Repository State:** The primary Engram repository working tree contains active, uncommitted working changes (harness schemas, isolation logic, evaluator methods, compiler modifications, and milestone documentation).
- **Integrity Guarantee:** No commits, resets, cleanups, stashes, or branch switches were executed during this milestone. All user changes are fully preserved.

### 2.2 Golden Repository State & Task Source Resolution
- **Task Source Repository:** For `task_commit_cache_invalidation` (`repository_type: "cargo"`), `orchestrator.py` (lines 104–105) resolves `self.golden_repo` to `HARNESS_ROOT.parent.parent.resolve()`, which is `C:\Users\Amal P S\OneDrive\Projects\Engram`.
- **Baseline Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`.
- **Golden Angular Repository State (Reference):** Located at `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`. Baseline commit and current commit are `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`. Its working tree is **dirty** with 18 modified tracked files and an untracked `.engram/` directory.
- **Cleanliness Transparency:** Neither the Engram working tree nor the Angular working tree is clean. However, `isolation.py` provisions sandboxes via `git worktree add --detach <sandbox_path> c70845b4...`, which checks out files directly from the Git object database at the immutable commit hash, completely bypassing working tree dirt.
- **Repository Invariance:** Zero modifications were made to either repository during this remediation.

### 2.3 Frozen Binary Integrity
All candidate and baseline binaries are frozen on disk and verified via cryptographic hashes:

| Binary Identity | Filesystem Location | Verified SHA-256 Hash | Integrity Status |
| :--- | :--- | :--- | :---: |
| **Candidate Phase 1** | `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | **MATCH** |
| **Release Build** | `target/release/engram.exe` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | **MATCH** |
| **v1.5.1 Baseline** | `benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | **MATCH** |
| **v1.5.4 Baseline** | `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | **MATCH** |

### 2.4 Test Verification Records

#### Fresh Milestone 5G Test Re-execution:
- **Test File:** `benchmarks/matched-pair-harness/tests/test_milestone_5f_remediation.py`
- **Execution Command:** `pytest "C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness\tests\test_milestone_5f_remediation.py"`
- **Result:** **7 passed in 0.61s** (0 failures, 0 errors).
- **Verified Subsystems:** Token guard variable attribution, MCP config generation, skipped-arm serialization, Turn-0 injection constants, and replacement event parsing.

#### Historical Comprehensive Test Baseline (Milestone 5F):
- Full Python Harness Suite: 302 passed, 0 failed.
- Full Rust Engine Suite: 63 passed, 0 failed (`cargo check --tests` clean).

---

## 3. Remediated Systems & Architecture Matrix

| Subsystem | Discovered Defect / Discrepancy | Remediation Applied | Operational Reality |
| :--- | :--- | :--- | :--- |
| **Candidate Binary Path** | Documented path `engram-candidate-phase1.exe` did not exist. | Corrected to `bin/candidate-phase1/engram.exe` across all documents. | Verified hash `F56347EF...`. Matches `target/release/engram.exe`. |
| **Task Contract & Scope** | Protocol described an Angular task with `npm test`. | Fully rewritten to Cargo task `task_commit_cache_invalidation` (`src/compiler.rs`, SQLite metadata, `cargo test`). | Eliminates codebase confusion; aligns protocol with `evaluator.py`. |
| **Repository Integrity** | Erroneously labeled golden repo "clean". | Documented true state (18 modified files in Angular, dirty Engram tree) and proved worktree isolation from commit `c70845b4`. | Guarantees audit transparency; detached worktrees prevent dirt leakage. |
| **Headroom Reserve CLI** | Default reserve is 45,000 (triggering at 130k), but 160k threshold was claimed. | Added `--headroom-reserve 15000` to documented launch command. | Accurately enforces 160k pre-dispatch threshold against 175k ceiling. |
| **Semantic Retrieval Reality** | Semantic retrieval assumed without evaluator check. | Clarified that task requires cross-module discovery, but evaluator tests functional code; replacement measured via telemetry. | Prevents unsupported claims regarding evaluator scope. |

---

## 4. Preserved Runtime Qualifications

The three operational qualifications remain active and require owner awareness:

1. **Model Runtime Identifier Unverifiable Offline:**
   - *Status:* `gpt-5-codex` is specified in protocol; harness defaults to `gpt-5.6-luna`. Whether `gpt-5-codex` is an active API model string cannot be verified without live inference.
   - *Plan:* Verified at Checkpoint 1 of the Runtime Observation Plan upon authorized launch.
2. **Runtime MCP Exposure vs. Offline Config:**
   - *Status:* Generating `.codex/config.toml` in the sandbox is verified offline. Actual process spawning and tool discovery by the proprietary Codex CLI must be verified during runtime.
   - *Plan:* Verified at Checkpoints 2 and 3 of the Runtime Observation Plan.
3. **Turn-0 Instruction Duplication Overhead:**
   - *Status:* Writing `.github/copilot-instructions.md` and prepending prompt text could result in redundant token consumption in Turn 1.
   - *Plan:* Verified at Checkpoint 5 of the Runtime Observation Plan.

---

## 5. Final Recommendation to Repository Owner

All four blockers identified in the independent audit have been resolved with empirical source evidence.

The engineering team recommends that the repository owner review the **Authorization Checklist** (`amendments/draft-05/milestone-5g/authorization-checklist.md`) and **Remediation Report** (`amendments/draft-05/milestone-5g/remediation-report.md`) to consider formal authorization for a single, controlled N=1 live matched pair.
