# Engram — Stage 1 Block 06 Final Pre-Execution Safety Check

**Document ID:** `ENGRAM-STAGE1-BLOCK06-FINAL-SAFETY-CHECK`  
**Date:** 2026-10-28  
**Scope:** Read-Only Verification of Invariants Prior to Live Execution Authorization  
**Target Benchmark:** Stage 1 Core Triad, Block 06 (`task_b_remember_me`)  
**Parent Specifications:** `benchmarks/cost-baseline/PREREGISTRATION.md`, `AMENDMENT_02_STAGE1_CORE_TRIAD.md`, `STAGE1_BLOCK06_PREFLIGHT_REPORT.md`  
**Overall Decision:** **`READY FOR EXPLICIT AUTHORIZATION`**  

---

## 1. Executive Summary

This read-only safety check establishes conclusive, verifiable evidence across all six pre-execution requirements for **Block 06** of the Stage 1 Core Triad benchmark.

**Key Findings:**
1. **Dry-Run Score ($C = 0.20$) Disambiguation:** Confirmed that $C = 0.20$ is a synthetic artifact resulting from evaluating an unedited baseline repo (which passes negative security rule `REQ-3` but fails functional requirements `REQ-1`, `REQ-2`, and `REQ-4`). The harness strictly marked all arms as unaccepted (`accepted: false`) and suppressed token savings claims.
2. **Execution Boundary:** The invocation command explicitly passes `--block block_06`. The orchestrator executes exactly three arms (`baseline_arm1`, `v151_arm2`, `v154_arm3`) and terminates cleanly without looping into Block 07.
3. **Treatment & Baseline Parity:** Frozen binary SHA-256 digests (`v1.5.1` and `v1.5.4`) match authoritative manifests bit-for-bit. Task prompts and requirements are equivalent. Sandboxes use 100% private physical `node_modules` copies.
4. **Telemetry & Acceptance Guards:** Full provider token extraction is active. Broken patches ($C < 1.0$), timeouts, or missing telemetry automatically invalidate comparative claims.
5. **Quota Ceiling:** Exposure is bounded between 270k and 540k tokens (750k strict ceiling). Timeout is locked at 600s with process tree termination (`taskkill /F /T /PID`).
6. **Artifact Integrity:** `cost-baseline/`, `profile/`, and preregistration hashes remain bit-for-bit identical to initial benchmark freeze.

**Verdict:** **`READY FOR EXPLICIT AUTHORIZATION`** for Block 06 only.

---

## 2. Evidence for the Six Preflight Safety Checks

### Check 1: Disambiguation of Dry-Run Output ($C = 0.20$)
- **Question:** Is $C = 0.20$ evidence of real task correctness or a synthetic dry-run artifact?
- **Evidence:**
  In dry-run mode, the orchestrator simulates Codex execution without applying any file edits. The pristine baseline repository is evaluated by `Evaluator.calculate_score()` against Task B's four criteria:
  - `REQ-1` (UI layout with remember-me checkbox): **FAIL** (0.0 / 0.25)
  - `REQ-2` (Auth persistence restoration logic): **FAIL** (0.0 / 0.35)
  - `REQ-3` (Security negative check: no password/token in `localStorage`): **PASS** (0.20 / 0.20) — *The pristine golden repo does not store credentials.*
  - `REQ-4` (Blast radius / scope containment: files changed count > 0): **FAIL** (0.0 / 0.20)
  - **Earned Score ($C$):** $0.0 + 0.0 + 0.20 + 0.0 = \mathbf{0.20}$.
- **Harness Invalidation Handling:**
  In `triad_summary.json`:
  ```json
  "acceptance": {
    "baseline_arm1_accepted": false,
    "v151_arm2_accepted": false,
    "v154_arm3_accepted": false,
    "all_arms_accepted": false
  },
  "comparative_analysis": {
    "v154_vs_baseline": {
      "savings_claim_eligible": false,
      "savings_claim_status": "INVALIDATED_BY_ACCEPTANCE_FAILURE",
      "verdict": "MUTUAL_ACCEPTANCE_FAILURE",
      "metrics": {
        "total_model_tokens": { "delta": null, "savings_pct": null, "status": "INVALIDATED_UNACCEPTED" }
      }
    }
  }
  ```
- **Conclusion:** $C = 0.20$ is confirmed to be an unaccepted failure state under synthetic dry-run conditions. It provides zero evidence of functional correctness and is correctly rejected by the harness.

---

### Check 2: Single-Block Execution Boundary
- **Command Structure:**
  `python benchmarks/matched-pair-harness/src/orchestrator.py --block block_06`
- **Code Inspection (`orchestrator.py`):**
  1. `main()` parses `--block block_06`.
  2. Resolves `TRIAD_BLOCK_SCHEDULES["block_06"]` $\rightarrow$ `task: task_b_remember_me`, `order: ["baseline_arm1", "v151_arm2", "v154_arm3"]`, `seed: 2026102806`.
  3. Executes `results = orchestrator.run_triad(block_id=args.block)`.
  4. `run_triad()` runs the three scheduled arms sequentially, exports `triad_summary.json` and `triad_report.md`, and returns.
  5. `main()` prints completion messages and exits.
- **Verification:** There is zero code or scheduling logic that can chain from `block_06` into `block_07`. The process halts completely after the third arm of Block 06 completes.

---

### Check 3: Treatment Parity, Binary Integrity & Sandbox Isolation
- **Binary Cryptographic Verification:**
  - `v1.5.1 engram.exe`: `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` (**MATCH**)
  - `v1.5.4 engram.exe`: `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` (**MATCH**)
- **Task Requirement Equivalence:**
  The core task prompt is identical across all three arms:
  > *"Add a remember-me option to the authentication flow that persists the username across reloads. You may use any available tools (including registered Engram MCP tools if enabled, or standard search commands) appropriate to your environment. CRITICAL SAFETY INSTRUCTION: Do NOT run npm install or modify package.json or node_modules under any circumstances."*
  For Arm 3 only, Turn-0 directives are prepended via `engram.exe inject`. Arms 1 and 2 receive zero Turn-0 injection.
- **Sandbox Isolation:**
  - Cold-start worktrees created from baseline commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.
  - Dependency strategy is locked to `copy` (100% private, physical copy of `node_modules`).
  - Pre/post golden repository SHA-256 and 68k file metadata integrity check runs before and after every arm.
- **Active Build & Acceptance Gates:**
  - `L0_typecheck`: `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit` (**ACTIVE**)
  - `L2_unit_tests`: `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.spec.json --noEmit` (**ACTIVE**)
  - `L3_production_build`: `npm run build` (**ACTIVE**)
  - Static evaluation: `REQ-1`, `REQ-2`, `REQ-3`, `REQ-4`.

---

### Check 4: Telemetry Accuracy & Acceptance Invalidation Guard
- **Telemetry Extraction:**
  - Raw stdout JSONL is captured in `events.jsonl`.
  - For live runs (`provenance = "DIRECT_API"`), `TelemetryParser` extracts per-request token usage records (`input_tokens`, `cached_input_tokens`, `output_tokens`, `reasoning_output_tokens`) from `.codex/sessions` using `thread_id`.
  - Fallback parses `turn.completed` events if sessions are inaccessible.
  - Multi-shape parser captures `mcp_tool_call`, `McpToolCall`, and `custom_tool_call` with ID deduplication.
- **Savings Invalidation Guard:**
  In `orchestrator.py`:
  - If $C < 1.0$ for either treatment or baseline, `savings_claim_eligible` evaluates to `False`.
  - All token deltas and percentage metrics are set to `None` (`null`).
  - `verdict` is set to `MUTUAL_ACCEPTANCE_FAILURE`, `TREATMENT_FAILED_ACCEPTANCE`, or `CONTROL_FAILED_ACCEPTANCE`.
  - Zero-token runs or missing telemetry are flagged as anomalies and strictly disqualified.

---

### Check 5: Quota Ceiling & Stop-on-429 Behavior
- **Block 06 Quota Ceiling:**
  - Estimated per-arm consumption: ~90k–180k tokens.
  - Estimated Block 06 total: ~270k–540k tokens.
  - Strict ceiling bound: **750,000 tokens**.
- **Process & Timeout Controls:**
  - Each arm is bound by `timeout_seconds = 600` (10 minutes).
  - Descendant processes are terminated via Windows `taskkill /F /T /PID`.
- **Stop-on-429 Rule:**
  If an API rate-limit (HTTP 429) or Codex CLI execution error occurs:
  - Codex exits with non-zero exit code.
  - Telemetry parser flags missing/zero tokens (`TELEMETRY_MISSING` / `ZERO_TOKEN_ANOMALY`).
  - Orchestrator aborts with explicit error banner and does not retry.

---

### Check 6: Frozen Repository & Baseline Invariant Verification
- **`benchmarks/cost-baseline/`:** **UNTOUCHED.** Pristine untracked files preserved.
- **`benchmarks/profile/`:** **UNTOUCHED.** Pristine untracked files preserved.
- **Cryptographic Checksums:**
  ```
  File: PREREGISTRATION.md
  Hash: DB3AB2956A4DA40AAD8F1F376AA72A50BEBC96F3E1ECC3120F405F0A294EA8BB  [VERIFIED]

  File: FREEZE_SEAL.md
  Hash: 08E8DA2EE3DE1A01E376218ECD83D1A71B2AD659AA89C49CEDE3080565761F5B  [VERIFIED]

  File: bin/v1.5.1/engram.exe
  Hash: 474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0  [VERIFIED]

  File: bin/v1.5.4/engram.exe
  Hash: CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798  [VERIFIED]
  ```

---

## 3. Pre-Execution Gate Decision

### **Final Decision: `READY FOR EXPLICIT AUTHORIZATION`**

All six preflight checks have passed with complete evidence:
1. Dry-run $C = 0.20$ is proven to be a synthetic test artifact, correctly treated as unaccepted.
2. The execution command is bound strictly to `block_06` and terminates automatically.
3. Treatment parity, binary SHA-256 hashes, and sandbox copy isolation are intact.
4. Telemetry and acceptance invalidation guards are fully active.
5. Quota exposure is bounded and stop-on-429 controls are enforced.
6. Baseline artifacts and seals remain untouched and uncommitted.

**Execution Boundary:** **Zero live model/API calls have been launched.** The harness is completely prepared and will execute Block 06 only upon your explicit authorization.
