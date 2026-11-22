# Engram Milestone 5G — Audit Remediation Report
## Comprehensive Resolution of Independent Readiness Audit Findings

**Date:** 2026-11-04  
**Milestone:** 5G — Live Pilot Readiness Remediation  
**Status:** COMPLETE / READY WITH QUALIFICATIONS — OWNER REVIEW REQUIRED  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Git Branch:** `feature/async-precommit-guard`  
**Git HEAD:** `c70845b4d5913457f1b176fb0370b80dd792711f`  
**Live Inference Conducted:** None (0 tokens consumed, 0 API requests)  

---

## 1. Executive Summary & Remediation Outcome

Following the Milestone 5G Independent Readiness Audit (`independent-readiness-audit.md`), which concluded **`NOT READY — BLOCKERS REMAIN`**, all four identified critical blockers were systematically addressed, corrected across all Milestone 5G documentation, and verified against the underlying Python and Rust codebases.

### Final Remediated Verdict
$$\mathbf{READY\ WITH\ QUALIFICATIONS\ —\ OWNER\ REVIEW\ REQUIRED}$$

*(All four blockers have been resolved with empirical evidence. Execution remains strictly prohibited until the repository owner grants explicit written authorization.)*

---

## 2. Blockers Remediation Matrix

| # | Audit Finding / Blocker | Exact Remediation Applied | Supporting Source Evidence | Resolution Status |
| :-: | :--- | :--- | :--- | :---: |
| **1** | **Candidate Binary Path Error**<br>`engram-candidate-phase1.exe` did not exist. | Corrected to `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` across `pilot-protocol.md`, `authorization-checklist.md`, and `readiness-assessment.md`. | Verified file existence and recalculated SHA-256 digest: `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`. Matches `target/release/engram.exe`. | **RESOLVED** |
| **2** | **Task Scope Contradiction**<br>Protocol described an Angular task with `npm test`, contradicting the actual Cargo task. | Rewrote Sections 2.1, 2.2, and 2.3 of `pilot-protocol.md` to define the Cargo task (`task_commit_cache_invalidation`), targeting `src/compiler.rs`, `src/analyzer.rs`, `src/main.rs`, and `tests/commit_cache_invalidation_test.rs`. | Cross-referenced against `task_commit_cache_invalidation.json` and `evaluator.py` lines 462, 621–770 (`_check_commit_cache_req1_meta_persistence`, `_check_commit_cache_req2_drift_detection`, `_check_commit_cache_req3_clean_subcommand`). | **RESOLVED** |
| **3** | **Repository Integrity & Worktree Isolation**<br>Golden repo labeled "clean" despite 18 modified files; source repo unclarified. | 1. Clarified that for Cargo tasks, `orchestrator.py` selects the Engram repository as golden repo.<br>2. Documented the true state of both repositories without false clean claims.<br>3. Proved that `isolation.py` provisions detached Git worktrees from commit `c70845b4...`, guaranteeing pristine baseline isolation. | `orchestrator.py` lines 104–105 (`repo_type_hint in ("cargo", "rust") -> self.golden_repo = HARNESS_ROOT.parent.parent`). `isolation.py` line 431 (`git worktree add --detach <sandbox_path> baseline_commit`). | **RESOLVED** |
| **4** | **Headroom Reserve CLI Omission**<br>160k threshold claimed, but CLI default is 45k (yielding 130k). | Added `--headroom-reserve 15000` to the documented launch command in `authorization-checklist.md`. | `orchestrator.py` line 1623 (`--headroom-reserve` argument definition) and `runner.py` line 416 (`estimated_next_turn = max(last_turn_request_tokens, conservative_reserve)`). | **RESOLVED** |

---

## 3. Detailed Verification Records

### 3.1 Candidate Binary & Frozen Baseline Integrity
All binaries were independently verified on disk via SHA-256 recalculation:

```
Candidate Phase 1:
  Path: benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe
  Hash: F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE
  Release Match: target/release/engram.exe matches exact hash.

v1.5.1 Baseline:
  Path: benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe
  Hash: 474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0

v1.5.4 Baseline:
  Path: benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe
  Hash: CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798
```

### 3.2 Git Branch, HEAD, and Repository Status
1. **Primary Engram Repository:**
   - Path: `C:\Users\Amal P S\OneDrive\Projects\Engram`
   - Active Branch: `feature/async-precommit-guard`
   - Current HEAD: `c70845b4d5913457f1b176fb0370b80dd792711f`
   - Tree Status: Contains working changes relating to harness schema updates, isolation logic, evaluator methods, and milestone documentation. Zero changes made to Git history.
2. **Golden Angular Repository (Historical Baseline):**
   - Path: `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`
   - Baseline & Current Commit: `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`
   - Tree Status: 18 modified tracked files and untracked `.engram/` directory preserved untouched.
3. **Task Isolation Mechanism:**
   - Sandboxes for `task_commit_cache_invalidation` are provisioned under `%TEMP%\engram_pilot\runs\<sandbox_name>\`.
   - Command: `git -c core.longpaths=true worktree add --detach <sandbox_path> c70845b4d5913457f1b176fb0370b80dd792711f`.
   - The detached worktree extracts files directly from the Git object database at the commit hash, completely isolated from uncommitted working tree changes.

### 3.3 Test Verification
- **Re-executed Regression Suite:** `benchmarks/matched-pair-harness/tests/test_milestone_5f_remediation.py`
- **Command:** `pytest "C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness\tests\test_milestone_5f_remediation.py"`
- **Result:** **7 passed in 0.61 seconds** (0 failures, 0 errors).
- **Verified Subsystems:** `StreamingTokenTracker` multi-turn accounting, pre-dispatch headroom halting, `.codex/config.toml` generation, skipped-arm null schema validation, Turn-0 injection constants, and replacement event parsing.

---

## 4. Preserved Operational Qualifications

The following three qualifications cannot be resolved offline and are formally submitted for owner awareness:

1. **Model Runtime Identifier Unverifiable Offline:**  
   `gpt-5-codex` is cited in protocol documentation; the harness default is `gpt-5.6-luna`. Whether `gpt-5-codex` is an active API model alias recognized by OpenAI's backend cannot be verified without live inference.
2. **Live Runtime MCP Process Exposure:**  
   Offline generation and syntax of `.codex/config.toml` in the sandbox is verified. Whether the proprietary `codex.exe` child process spawns `engram.exe serve --stdio` and discovers MCP tools must be observed during live execution.
3. **Turn-0 Instruction Duplication Overhead:**  
   Simultaneous injection via `.github/copilot-instructions.md` and prompt prefix may create minor token duplication at Turn 1.

---

## 5. Verified Launch Command Specification

The launch command in `authorization-checklist.md` has been aligned with `orchestrator.py` `argparse` grammar:

```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --mode pair `
  --task task_commit_cache_invalidation `
  --pair pilot_02 `
  --expected-treatment-sha256 "F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE" `
  --model "gpt-5.6-luna" `
  --reasoning "medium" `
  --max-tokens-ceiling 175000 `
  --headroom-reserve 15000 `
  --dependency-strategy "none" `
  --output-dir "benchmarks/matched-pair-harness/results/pilot_02"
```

---

## 6. Safety Affirmation

**Explicit confirmation:** Zero live model API requests, zero Codex agent executions, zero network requests, and zero tokens were consumed during this remediation milestone. All work was strictly confined to documentation alignment, hash verification, and offline test execution.
