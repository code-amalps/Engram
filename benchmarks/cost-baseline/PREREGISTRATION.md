# Engram Phase 0–2 Measurement Pilot — Formal Preregistration Document

**Document Version:** 1.3.0  
**Status:** SEALED PREREGISTRATION  
**Seal Timestamp:** 2026-10-18T06:33:00Z  
**Target Golden Repository:** `angular-ngrx-material-starter` (Commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`)  
**Engram Candidate Binary:** `target/release/engram.exe` (SHA-256: `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0`)  
**Build Manifest:** [`benchmarks/cost-baseline/BUILD_MANIFEST.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/BUILD_MANIFEST.md)  
**Randomization Seed:** `20261018`

---

## 1. Experimental Objective & Hypotheses

### Primary Research Question
Does providing AI coding agents with compact, task-relevant Engram context (`details: false`), incremental preflight boundary locking, and scope-drift enforcement reduce or increase the total cost (model tokens, auxiliary calls, and account credits) of completing coding tasks compared to native Copilot exploration?

### Hypotheses
- **$H_0$ (Null Hypothesis):** There is no significant difference in total token usage, conversational turns, or credit consumption between Configuration A (Without Engram) and Configuration B (Current Candidate Engram).
- **$H_1$ (Efficiency Hypothesis):** Configuration B reduces total prompt tokens and conversational turns on multi-file feature tasks (Task B) by preventing unnecessary exploratory file-reading and scope drift.
- **$H_2$ (Trivial Task Overhead Hypothesis):** Configuration B introduces measurable overhead on trivial single-file edits (Task A) unless small-task bypass is engaged.

> [!IMPORTANT]
> **Scientific Integrity Principle:** No claim of cost savings or token efficiency is assumed or asserted in advance. The experiment evaluates whether the current optimized Engram build reduces or increases the real usage cost of completing coding tasks.

---

## 2. Tested Configurations & Model Selection

### Model Governance Rule (Ratified)
- **Primary Model Choice:** `Claude 3.5 Sonnet` in Copilot Agent Mode is preferred for both arms, contingent on actual availability in the Copilot UI at session start.
- **Availability Safeguard:** If `Claude 3.5 Sonnet` is unavailable, the runner must **STOP and report** the available model choices before selecting a replacement.
- **Parity Invariant:** The selected model must remain strictly identical across all runs of both Configuration A and Configuration B.

### Configuration A — Control Baseline (Without Engram)
- **Agent:** GitHub Copilot in VS Code (Agent Mode).
- **Model:** `Claude 3.5 Sonnet` (contingent on availability).
- **MCP Configuration:** Disabled (`.vscode/mcp.json` removed or empty; `.vscode/settings.json` has `engram.binaryPath` unset/disabled).
- **Available Tools:** Native VS Code tools only (`read_file`, `replace_string_in_file`, `run_in_terminal`, `grep_search`, `find_files`).
- **Workspace:** Fresh disposable sandbox cloned from golden snapshot `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.

### Configuration B — Engram Treatment (Current Candidate)
- **Agent:** GitHub Copilot in VS Code (Agent Mode).
- **Model:** `Claude 3.5 Sonnet` (contingent on availability).
- **MCP Configuration:** `.vscode/mcp.json` configured with frozen candidate release binary (`target/release/engram.exe`).
- **MCP Response Mode:** Compact by default (`details: false`, payload size $\sim 700\text{--}900$ bytes; detailed mode preserved on request).
- **Available Tools:** Native VS Code tools + Engram MCP tools (`engram_context`, `engram_preflight`, `engram_postflight`, etc.).
- **Workspace:** Fresh disposable sandbox cloned from golden snapshot `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` with cold cache.

---

## 3. Usage Telemetry & Accounting Architecture (Ratified)

> [!IMPORTANT]
> **Strict Metric Separation Rule:** Measured client tokens, estimated context tokens, and account-level billing deltas are recorded in separate data fields and NEVER summed together into a single combined token number.

### Isolation & Timestamping Protocol
- Every run is strictly isolated and timestamped ($T_{\text{start}}$, $T_{\text{end}}$).
- **Zero Concurrent Activity:** No overlapping Copilot requests, secondary chat tabs, or background AI operations may run concurrently during a benchmark run.
- **Account Credit Observations:**
  * Record exact before/after credit values from the VS Code status bar / billing dashboard along with precise timestamps for each run.
  * If account telemetry is delayed or cumulative, the limitation is reported explicitly. Under **no circumstances** will exact per-run credit deductions be inferred or imputed from wrapper token logs.

### Metric Categories

| Category | Specific Metric | Evidence Source | Tag | Description |
|---|---|---|---|---|
| **Authoritative Measured** | Auxiliary Prompt Tokens | `copilotLanguageModelWrapper_*.copilotmd` | **Measured** | `gpt-4o-mini` prompt tokens (~1,634 tokens/call) |
| **Authoritative Measured** | Auxiliary Output Tokens | `copilotLanguageModelWrapper_*.copilotmd` | **Measured** | `gpt-4o-mini` completion tokens (~8 tokens/call) |
| **Authoritative Measured** | Model Turns / Steps | Chat Transcript / Wrapper count | **Measured** | Conversational iterations taken by agent |
| **Authoritative Measured** | Tool Call Counts | Chat Transcript / Events | **Measured** | Breakdown: Engram calls vs native file/terminal calls |
| **Authoritative Measured** | Engram Response Bytes | Server / Event logs | **Measured** | Exact payload characters returned by MCP server |
| **Authoritative Measured** | Blast Radius (Files) | `git status --short` in sandbox | **Measured** | Count of tracked & untracked files modified |
| **Authoritative Measured** | Elapsed Wall Time | Timestamps ($T_{\text{end}} - T_{\text{start}}$) | **Measured** | Wall-clock seconds for run |
| **Authoritative Measured** | Typecheck Exit Code | `tsc -p tsconfig.app.json` | **Measured** | Exit code (0 = success) |
| **Estimated Proxy** | Primary Context Tokens | Heuristic ($\approx \text{bytes} / 4$) | **Estimated** | Context tokens fed to Claude 3.5 Sonnet |
| **Account Billing** | Copilot AI Credits | Billing Dashboard / Status Display | **Account** | Account-level delta; delay and batch scope tagged |

---

## 4. Task Specifications & Acceptance Procedures (Ratified)

> [!WARNING]
> **No Static Regex Passing as Functional Proof:** Preliminary static scripts (`verify_task_a.ps1`, `verify_task_b.ps1`) verify file boundaries and string presence, but are NOT presented as proof of functional correctness. Identical acceptance criteria apply to both arms.

### Task A: Branding Typo Fix (Trivial Single-File Configuration)
- **Prompt:**
  ```text
  In projects/angular-ngrx-material-starter/src/environments/environment.ts, correct the branding typo in appName from 'Angular Ngrx Material Starter' to 'Angular NgRx Material Starter' so that 'NgRx' matches the official casing. Do not change any other file or configuration.
  ```
- **Allowed Scope:** Strictly 1 file: `projects/angular-ngrx-material-starter/src/environments/environment.ts`.
- **Acceptance Procedure:**
  1. *Preliminary Static Check:* `verify_task_a.ps1` exits with code 0 (casing match and exactly 1 file modified).
  2. *Build / Typecheck Gate:* `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit` exits with code 0.
  3. *Functional Verdict:* Because Task A is a pure configuration string without runtime state, passing both gates confirms correctness.

### Task B: Remember-Me Feature (Multi-File Architectural Task)
- **Prompt:**
  ```text
  Add a remember-me option to the authentication flow that persists the username across reloads.
  ```
- **Allowed Scope:** `projects/angular-ngrx-material-starter/src/app/core/auth/*`, `app/app.component.*`, `assets/i18n/en.json`.
- **Acceptance Procedure:**
  1. *Preliminary Static Scope Check:* `verify_task_b.ps1` exits with code 0 (confirms authorized component boundary, no bulk i18n sprawl).
  2. *Security Negative Rule:* Zero occurrences of `localStorage.setItem` persisting `password`, `token`, `jwt`, or `secret`.
  3. *Application Build & Typecheck Gate:* `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit` exits with code 0 (confirms full TypeScript typing and syntax validity).
  4. *Test Suite Typecheck Gate:* `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.spec.json --noEmit` exits with code 0.
  5. *Functional Runtime Browser Status (Pre-Pilot Probe Outcome):*
     - A read-only probe of `npx ng test --watch=false --browsers=ChromeHeadless` in the golden workspace confirmed that Karma starts and Chrome Headless connects, but throws `TypeError: __webpack_require__(...).context is not a function` in `src/test.ts:28` due to Webpack builder deprecation in this repository.
     - Because modifying the golden repository is prohibited, headless Karma runtime execution is **unavailable**.
     - **Preregistered Acceptance Outcome:** In accordance with the preregistered rule, runtime browser verification for Task B will be preserved and reported as **UNVERIFIED AT RUNTIME**. Static regex checks will not be substituted as functional proof.

---

## 5. Blinded Evaluation Protocol (Ratified)

To prevent confirmation bias in qualitative code quality and blast-radius evaluation:
1. **Evaluator:** Human reviewer (User).
2. **Timing & Procedure:** Blinded diffs are prepared only after all 20 runs have concluded. Diffs are exported using `git diff --no-prefix` with author, timestamps, commit hashes, and Copilot metadata stripped, preserving sufficient code context for inspection.
3. **Double-Blind Protection:** Arm labels (Control vs Treatment) are completely withheld from the evaluator until all qualitative diff reviews are formally recorded.
4. **Objective Checks Recorded Separately:** Objective compiler exit codes, assertion counts, file counts, and regex outputs are logged independently in structured data files.
5. **Documentation of Blinding Leaks:** If an agent outputs visible tool markers (e.g. `<!-- BEGIN ENGRAM MANAGED BLOCK -->`), this is documented as an unavoidable blinding leak.

---

## 6. Sample Size, Randomization, Interrupted Runs & Stopping Criteria

### Fixed Sample Size (Ratified)
- **Sample Plan:** Exactly **five valid matched pairs** per task.
  * Task A: 5 pairs = 10 valid runs (5 Control, 5 Treatment).
  * Task B: 5 pairs = 10 valid runs (5 Control, 5 Treatment).
  * **Total Planned Valid Runs:** 20 runs.
- **Stopping Invariant:** Sample size is fixed. Additional runs are **NOT** added based on observed metric variance.

### Deterministic Randomization Plan & Order Analysis (Seed: `20261018`)
Matched pairs are block-randomized using Seed `20261018`:

#### Task A Matched Pairs Execution Order:
- **Pair 1:** Run 1 = Treatment (B), Run 2 = Control (A)
- **Pair 2:** Run 1 = Control (A), Run 2 = Treatment (B)
- **Pair 3:** Run 1 = Treatment (B), Run 2 = Control (A)
- **Pair 4:** Run 1 = Control (A), Run 2 = Treatment (B)
- **Pair 5:** Run 1 = Treatment (B), Run 2 = Control (A)

#### Task B Matched Pairs Execution Order:
- **Pair 1:** Run 1 = Treatment (B), Run 2 = Control (A)
- **Pair 2:** Run 1 = Treatment (B), Run 2 = Control (A)
- **Pair 3:** Run 1 = Control (A), Run 2 = Treatment (B)
- **Pair 4:** Run 1 = Control (A), Run 2 = Treatment (B)
- **Pair 5:** Run 1 = Treatment (B), Run 2 = Control (A)

#### Order Imbalance & Potential Order Effect Documentation:
- In Task A: Treatment (B) runs first in 3 of 5 pairs (60%).
- In Task B: Treatment (B) runs first in 3 of 5 pairs (60%).
- Overall: Treatment (B) runs first in 6 of 10 pairs (60%); Control (A) runs first in 4 of 10 pairs (40%).
- **Documented Potential Order Effect:** The first run of a pair bears any initial cold-start latency (OS disk caches, initial Copilot API connection setup). Because B runs first in 60% of pairs, Treatment B bears slightly more cold-start overhead than Control A.
- **Integrity Rule:** In accordance with preregistration ethics, this randomization order is strictly locked and will **NOT** be altered after observing pilot outcomes.

### Interrupted Runs & Replacement Policy (Ratified)
- **Artifact Preservation:** All artifacts, debug logs, partial diffs, and usage telemetry from interrupted runs are permanently preserved in `benchmarks/cost-baseline/transcripts/interrupted/`.
- **Replacement Eligibility (Strict Pre-Execution Rule):**
  * A run is eligible for replacement **only** if interrupted by an external infrastructure abort: Copilot host crash, OS out-of-memory, network loss, or HTTP 429 quota exhaustion.
  * If the agent voluntarily halts, claims task completion with broken code, or runs out of turns, that constitutes a valid **Task Failure**, NOT an interrupted run. It is NEVER replaced.
- **Replacement Cap:** A maximum of **2 replacements per arm** (maximum 4 replacements across the entire pilot) is permitted. If an arm encounters a third infrastructure abort, the entire pilot stops for human review.
- **Accounting Invariant:** Interrupted-run expenditure (tokens, API turns, elapsed time) is recorded in a dedicated ledger and included in total pilot cumulative expenditure. Failed or interrupted runs are **never silently discarded**.

### Safe Disposable Workspace Protocol
- Golden master reference at `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` (`1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`) remains read-only.
- All runs execute strictly in disposable sandboxes:
  `C:\Users\Amal P S\AppData\Local\Temp\engram_pilot\run_{task}_{config}_{rep}`
- Caches in `%LOCALAPPDATA%\Engram\cache\<sandbox_id>` and sandbox `.engram/` directories are wiped cold before each repetition.

---

## 7. Status of Decisions & Blockers

All four methodological decisions have been ratified and incorporated:
1. **Model:** `Claude 3.5 Sonnet` preferred for both arms, contingent on availability. [RATIFIED]
2. **Telemetry:** Isolated timestamped runs; per-run credit logging with delay warnings; no imputation from wrapper logs. [RATIFIED]
3. **Acceptance:** Real build/typecheck required; Task B browser runtime unverified due to probe outcome; no static regex as functional proof. [RATIFIED]
4. **Blinded Evaluation:** Human reviewer (User) evaluates anonymized diffs after pilot completion; objective gates recorded separately. [RATIFIED]
5. **Sample & Replacement:** Fixed 5 matched pairs (20 runs total); max 2 replacements per arm for infrastructure aborts only; interrupted expenditure tracked cumulatively. [RATIFIED]

There are **zero remaining methodological blockers**. The specification is ready for freeze sealing upon your explicit approval.
