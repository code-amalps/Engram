# Engram — Phase 5.3 Initial Audit: Repository & Candidate Integrity

**Document Identifier:** `PHASE_5_3_INITIAL_AUDIT.md`  
**Execution Timestamp:** 2026-11-19T09:28:00+05:30  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Current Branch:** `feature/phase2-stage5-remediation`  
**Head Commit:** `7106a2f feat(core): Phase 2 Stage 5 - AST member extraction, workspace-bounded graph, query router, context budget, and Candidate 3 release hardening`  
**Audit Status:** COMPLETE — All Baseline Invariants Confirmed  

---

## 1. Repository State & Branch Identity

| Property | Actual Value | Invariant Check |
|---|---|---|
| **Repository Root** | `C:/Users/Amal P S/OneDrive/Projects/Engram` | VERIFIED |
| **Current Branch** | `feature/phase2-stage5-remediation` | VERIFIED (Exact Match) |
| **Upstream Branch** | `origin/feature/phase2-stage5-remediation` | VERIFIED (In Sync, 0 commits ahead/behind) |
| **Latest Commit SHA** | `7106a2f` | VERIFIED |
| **Tracked Working Tree** | Clean (0 modified tracked files) | VERIFIED |
| **Untracked Entries** | Historical benchmarks/amendments documentation (preservation-only) | VERIFIED |

---

## 2. Release Candidate Checksum Verification

All physical executables in `benchmarks/matched-pair-harness/bin/` were independently hashed using SHA-256:

| Candidate Identifier | Relative File Path | File Size (Bytes) | Cryptographic SHA-256 Checksum | Match Status |
|---|---|---|---|:---:|
| **Candidate 1** (Baseline) | `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` | 18,285,568 | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | **EXACT MATCH** |
| **Candidate 2** (Phase 4) | `benchmarks/matched-pair-harness/bin/candidate-phase2/engram.exe` | 18,667,008 | `A58B57D343502F2F4E9C328F20841527D573259FC9B305D5F006671427FD99DE` | **EXACT MATCH** |
| **Candidate 3** (Production) | `benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe` | 18,668,032 | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | **EXACT MATCH** |
| **v1.5.1 Baseline** | `benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe` | 17,722,880 | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | **EXACT MATCH** |
| **v1.5.4 Baseline** | `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe` | 18,181,632 | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | **EXACT MATCH** |

*Integrity Confirmation:* Candidate 1, Candidate 2, and Candidate 3 are bit-for-bit identical to their release freeze manifests (`PHASE_5_2_SHA256SUMS.txt`, `PHASE_5_2_CANDIDATE_MANIFEST.json`). Zero binary drift has occurred.

---

## 3. Matched-Pair Benchmark Harness Architecture

The benchmark harness in `benchmarks/matched-pair-harness/` consists of seven specialized modules:

1. **`orchestrator.py`:** Multi-arm execution engine coordinating matched pairs (`control_A` vs `treatment_B`) and triads (`baseline_arm1`, `v151_arm2`, `v154_arm3`). Handles run vault creation, schema validation, and summary generation.
2. **`runner.py`:** `CodexRunner` driving the headless OpenAI Codex CLI (`codex.exe exec --json`). Implements process tree tracking, child process PATH sanitization (preventing accidental discovery of unmanaged Engram binaries), streaming token tracking, and pre-dispatch headroom guards.
3. **`isolation.py`:** Creates ephemeral Git worktrees from the golden repository into `%TEMP%\engram_pilot\runs`. Supports `copy` (multi-threaded robocopy of `node_modules` for write isolation) and `junction` modes. Asserts `git rev-parse HEAD` identity and porcelain cleanliness prior to each run.
4. **`evaluator.py`:** Objective multi-tier evaluation engine:
   - Gate L0: TypeScript compile check (`tsc -p ... --noEmit`).
   - Gate L1: ESLint / Prettier code quality.
   - Gate L2: Spec compilation and unit tests.
   - Gate L3: Angular production build (`npm run build`).
   - Task requirements scoring (REQ-1 through REQ-4).
5. **`telemetry.py`:** Parses JSONL event streams to extract provider-reported usage, turn counts, tool invocation latency, and error types.
6. **`classifier.py`:** Evaluates task complexity to determine fast-path eligibility.
7. **`verification.py`:** Independent verification workflow ensuring telemetry schema compliance and manifest validation.

---

## 4. Existing Provider Support & Execution Model

* **Codex CLI Engine:** The harness relies on `codex.exe` (`codex-cli 0.167.0-alpha.4.1`) located at `C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe`.
* **Authentication:** Codex uses session authentication stored in `C:\Users\Amal P S\.codex\auth.json` (associated with a ChatGPT Plus/Team account).
* **Supported Provider Options in Harness:**
  - `cloud`: OpenAI cloud execution via Codex CLI.
  - `ollama`: Local open-source execution via `--oss --local-provider ollama`.
  - `lmstudio`: Local open-source execution via `--oss --local-provider lmstudio`.
* **Model Compatibility:**
  - Cached available models: `gpt-5.6-luna`, `gpt-6-luna`, `gpt-5.6-terra`, `gpt-reserve`.
  - Standard API model names (e.g. `gpt-4o`) are rejected by Codex CLI under ChatGPT account mode (`"The 'gpt-4o' model is not supported when using Codex with a ChatGPT account."`).
  - Native Anthropic models: **Not supported** by Codex CLI runner. Codex CLI is exclusively an OpenAI/ChatGPT client and local OSS runner.
* **Token Reporting:** Live `turn.completed` events return authoritative token metadata:
  `{'input_tokens': int, 'cached_input_tokens': int, 'cache_write_input_tokens': int, 'output_tokens': int, 'reasoning_output_tokens': int}`.

---

## 5. Benchmark Task Inventory & Baseline Repositories

### 5.1 Golden Repository
* **Path:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`
* **Baseline Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`
* **Dependencies:** `node_modules` verified present (68,274 files).
* **Isolation Test:** `create_sandbox` and `teardown_sandbox` verified working cleanly via `git worktree add --detach`.

### 5.2 Task Definitions
1. **Task A (`task_a_typo.json`):**
   - Goal: Fix capitalization typo `'Angular Cli Tips'` $\to$ `'Angular CLI Tips'` in `about.component.html`.
   - Baseline Commit: `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.
   - Allowed Paths: `projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html`.
   - Gates: L0 Typecheck, L3 Build.
   - Requirements: REQ-1 (Text fix, weight 0.5), REQ-2 (Boundary containment, weight 0.5).
2. **Task B (`task_b_remember_me.json`):**
   - Goal: Add remember-me option to authentication flow that persists username in localStorage across reloads without persisting sensitive credentials.
   - Baseline Commit: `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.
   - Allowed Paths: `projects/angular-ngrx-material-starter/src/app/core/auth/**`, `app.component.*`, `assets/i18n/en.json`.
   - Gates: L0 Typecheck, L2 Spec Compile, L3 Build.
   - Requirements:
     - REQ-1 (UI checkbox & input, weight 0.25).
     - REQ-2 (Username restoration logic, weight 0.35).
     - REQ-3 (Negative security check: zero secret/password/token persistence, weight 0.20).
     - REQ-4 (Boundary containment, weight 0.20).

---

## 6. Audit Conclusion & Readiness Decision

- **Repository State:** Clean, verified, and at the designated commit `7106a2f`.
- **Candidate Hashes:** 100% verified without drift.
- **Provider Mechanism:** Headless Codex CLI with live OpenAI account credentials confirmed functional.
- **Readiness:** **PROCEED TO PHASE 1 (Provider Compatibility Audit).**
