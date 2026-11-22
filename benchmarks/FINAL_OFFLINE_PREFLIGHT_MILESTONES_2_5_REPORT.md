# Final Offline Preflight Audit & Milestones 2–5 Acceptance Report

**Document Identifier:** `FINAL_OFFLINE_PREFLIGHT_MILESTONES_2_5_REPORT`  
**Document Version:** `1.0.0-FINAL`  
**Audit Date:** October 29, 2026 (21:28:00+05:30)  
**Lead Auditor:** Antigravity AI Systems & Verification Engineering  
**Primary Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram` (Branch: `feature/async-precommit-guard`, Commit: `c70845b4d5913457f1b176fb0370b80dd792711f`)  
**Golden Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` (Pinned Baseline Commit: `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`)  
**Candidate Executable:** `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`  
**Candidate Binary SHA-256:** `f56347ef2f0bfd4854176a3ec3c86d8d20b4c0e5954f04e4464998f623d85bee`  
**Frozen v1.5.1 SHA-256:** `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0`  
**Frozen v1.5.4 SHA-256:** `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798`  
**Operational Authorization Mode:** **STRICTLY OFFLINE** (Zero live model/API tokens consumed; zero live benchmarks executed)

---

## 1. Executive Summary

This report delivers the comprehensive, coordinated offline preflight audit of Engram following the engineering remediation of Milestones 2 through 4. The audit establishes the exact cryptographic baseline of the candidate binary, frozen historical references, and golden target repository; evaluates the three functional tiers of Milestone 2; inspects the bounded context retrieval and headroom enforcement mechanisms of Milestone 3; independently evaluates the Remember Me reference implementation against baseline acceptance gates in Milestone 4; and rigorously assesses the configuration readiness of Milestone 5 under ratified **Amendment 04**.

### Key Findings:
1. **Milestone 2 (Reliable Integration):**  
   - Tier 1 (Server & Protocol) and Tier 2 (Codex CLI Registration) are **100% PASS**. All four consolidated tools (`engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight`) execute cleanly via stdio JSON-RPC without error, returning structured payloads. The installed Codex CLI accepts the 4-tool approval configuration and reports the server enabled.  
   - Tier 3 (Autonomous Client Invocation by Model) is **PENDING AUTHORIZATION**. Autonomous invocation by an agent during `codex exec` requires live inference. In strict adherence to safety rules, execution halted at this boundary.  
   - **Overall Milestone 2 Status: PARTIAL** (Offline portion PASS; live agent invocation pending authorization).
2. **Milestone 3 (Bounded Context Retrieval):**  
   - **Status: PASS**. Depth clamping strictly caps `engram_inspect_symbol` traversal at depth 3 (payload: 1,432 chars / ~358 tokens). Shell retrieval guidance is verified injected into the candidate arm prompt and omitted from the baseline. Multi-turn pre-dispatch headroom enforcement was proven to halt execution at Turn 5 pre-dispatch (`stop_reason = PRE_DISPATCH_HEADROOM_EXHAUSTED`, `token_ceiling_breached = False`) prior to ceiling breach. The practical OS write-buffering race of external rollout files (100–500ms) is documented honestly.
3. **Milestone 4 (Functional Task Completion):**  
   - **Reference-Patch Validation: PASS**. The Remember Me implementation was applied in an isolated temporary clone checked out to clean baseline commit `1c285608`. It passed `L0_typecheck` (exit code 0), `L2_unit_tests` (exit code 0), `L3_production_build` (exit code 0), and all checklist criteria (REQ-1..4), earning a perfect Correctness Score $C = 1.00$. The golden repository was 100% preserved.  
   - **Autonomous Completion: UNPROVEN / PENDING LIVE VALIDATION**. Autonomous generation of these edits by an LLM agent within 250,000 tokens remains to be demonstrated during live benchmarking.  
   - **Overall Milestone 4 Status: PARTIAL**.
4. **Milestone 5 (End-to-End Matched-Pair Validation):**  
   - **Status: READY FOR EXPLICIT AUTHORIZATION (Execution Deferred)**. The ratified Amendment 04 two-arm schedule (`v154_arm3` followed by `baseline_arm1`, with `v151_arm2` deferred), 250,000 per-arm ceiling, 15,000 headroom reserve, and comparative claims suppression rules are fully wired and verified. Zero live model calls were made.
5. **Recommendation:**  
   **EXPLICIT GO FOR LIVE BENCHMARK AUTHORIZATION** under the ratified Amendment 04 protocol.

---

## 2. Audit Baseline & Cryptographic Integrity

### 2.1 Git Repository Provenance
- **Primary Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`
- **Active Branch:** `feature/async-precommit-guard`
- **HEAD Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f` (`feat(guard): add one-click engram guard install/uninstall subcommands...`)
- **Working Tree State:** Exactly 13 tracked modified files (harness scripts, Rust instructions/mcp, tests) and untracked benchmark documentation. Zero unstaged regressions.

### 2.2 Golden Repository Provenance
- **Golden Repository:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`
- **Pinned Baseline Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`
- **Working Tree State:** Exactly 18 tracked modified files and untracked `.engram/`.  
  Audited pre-test and post-test: **100% IDENTICAL BIT-FOR-BIT**. Zero modifications introduced during the audit.

### 2.3 Cryptographic Hash Manifest

| Executable Target | File Location | Expected SHA-256 | Actual Measured SHA-256 | Verification Status |
| :--- | :--- | :--- | :--- | :--- |
| **Candidate Phase 1** | `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` | `f56347ef2f0bfd4854176a3ec3c86d8d20b4c0e5954f04e4464998f623d85bee` | `f56347ef2f0bfd4854176a3ec3c86d8d20b4c0e5954f04e4464998f623d85bee` | **MATCH (100% PRESERVED)** |
| **Frozen v1.5.1** | `benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe` | `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` | `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` | **MATCH (100% PRESERVED)** |
| **Frozen v1.5.4** | `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe` | `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` | `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798` | **MATCH (100% PRESERVED)** |

*(Note on Discrepancy: The user prompt string omitted a `5` in the v1.5.1 hash (`...aa1a...` vs disk `...aa51a...`). The physical file on disk exactly matches the canonical preregistration seal in `benchmarks/cost-baseline/PREREGISTRATION.md` and `src/isolation.py`).*

---

## 3. Integrated Milestone Matrix

| Milestone | Sub-Component / Tier | Offline Evaluation Status | Evidence & Test Verification | Audit Verdict |
| :--- | :--- | :--- | :--- | :--- |
| **Milestone 2** | **Tier 1: MCP Server & Protocol** | Verified via stdio JSON-RPC | `initialize` (v1.5.4); `tools/list` returns 4 tools; `tools/call` returns valid JSON for all 4 tools | **PASS** |
| | **Tier 2: Codex CLI Registration** | Verified via Codex CLI inspection | `codex mcp get --json engram` exit code 0; 4 tools enabled; per-tool approval mode `approve` | **PASS** |
| | **Tier 3: Autonomous Invocation** | Stopped at model inference boundary | Requires `codex exec` with live LLM tokens; direct execution verified, agent triggering unexercised | **PENDING AUTHORIZATION** |
| | **Milestone 2 Overall** | Coordinated Offline Status | Tier 1 & 2 complete; Tier 3 awaiting authorized live benchmark | **PARTIAL** |
| **Milestone 3** | **Depth Clamping & Output Bounds** | Clamped to depth $\le 3$ | `engram_inspect_symbol` output bounded to 1,432 chars (~358 tokens) across depth 1..10 | **PASS** |
| | **Shell Retrieval Guidance** | Prompt injection verified | `SHELL_RETRIEVAL_GUIDANCE` confirmed in candidate prompt; omitted from baseline | **PASS** |
| | **Pre-Dispatch Guard & Stop Types**| Multi-turn lifecycle verified | Headroom stop at Turn 5 pre-dispatch (`PRE_DISPATCH_HEADROOM_EXHAUSTED`, breach=False) | **PASS** |
| | **External-Polling Race Analysis** | Operating system buffering audited | Synchronous stdout stream coupled with 15k reserve; documented 100-500ms OS file flush latency | **PASS** |
| | **Milestone 3 Overall** | Coordinated Offline Status | All offline tests passing; practical limits documented honestly | **PASS** |
| **Milestone 4** | **Baseline Fixture Health** | Verified in clean isolated clone | Baseline gates L0, L2, L3 pass; REQ-1/2 fail as expected (unimplemented baseline) | **PASS** |
| | **Reference Patch Validation** | Evaluated in isolated clone | `L0_typecheck`: PASS; `L2_unit_tests`: PASS; `L3_production_build`: PASS; REQ-1..4: PASS ($C = 1.00$) | **PASS** |
| | **Autonomous Agent Completion** | Stopped at model inference boundary | Synthesizing changes autonomously within token ceiling requires live model generation | **UNPROVEN / PENDING LIVE VALIDATION** |
| | **Milestone 4 Overall** | Coordinated Offline Status | Reference implementation verified; autonomous agent completion unproven | **PARTIAL** |
| **Milestone 5** | **Readiness Audit Only** | amendment 04 governance audited | Two-arm schedule, 250k ceiling, 15k reserve, stopping & claims suppression rules verified | **READY FOR EXPLICIT AUTHORIZATION** |
| | **Milestone 5 Execution** | Execution strictly deferred | Zero live model tokens consumed | **NOT RUN** |

---

## 4. Milestone 2 Deep-Dive: Reliable Integration

### 4.1 Tier 1 — MCP Server and Protocol
The candidate executable (`benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`) was spawned with `engram.exe -C <sandbox> mcp` and exercised over stdio JSON-RPC 2.0:
1. **Handshake:** Responded to `initialize` with protocol version `2024-11-05` and server info `{"name": "engram", "version": "1.5.4"}`.
2. **Tool Discovery (`tools/list`):** Returned exactly four core tools:
   - `engram_get_context`
   - `engram_inspect_symbol`
   - `engram_preflight`
   - `engram_postflight`
3. **Genuine `tools/call` Invocations:**
   - `engram_get_context` (task: `fix auth bug`, budget: 500): Returned structured context with target files, governing rules, and co-change alerts (`isError=False`, length: 397 chars).
   - `engram_inspect_symbol` (target: `test_auth.ts`, depth: 1): Returned target symbol AST node, contract evaluation, and explanation (`isError=False`, length: 1,432 chars).
   - `engram_preflight` (task: `add remember me`, planned: `["test_auth.ts"]`): Locked contract `ctr_dcfa1843a1034c428ada2486ae2c8c83` (`isError=False`, `status: READY_FOR_IMPLEMENTATION`).
   - `engram_postflight` (strict: false, test_evidence: true): Evaluated diff and returned verdict `PASS` (`isError=False`).
- **Tier 1 Status: PASS.**

### 4.2 Tier 2 — Codex CLI Registration and Configuration
The installed Codex CLI (`C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe`) was audited by passing candidate configuration flags:
- `-c mcp_servers.engram.command="C:/Users/.../candidate-phase1/engram.exe"`
- `-c mcp_servers.engram.args=["mcp"]`
- `-c mcp_servers.engram.default_tools_approval_mode="approve"`
- `-c mcp_servers.engram.enabled_tools=["engram_get_context", "engram_inspect_symbol", "engram_preflight", "engram_postflight"]`
- `-c mcp_servers.engram.tools.<tool>.approval_mode="approve"`

**Observations:**
- `codex mcp list --json` returned exit code 0, displaying `engram` with `enabled: true`.
- `codex mcp get --json engram` confirmed `enabled_tools` contains strictly the 4 consolidated tools.
- Sandboxing was confirmed: `-s workspace-write` is enforced by `CodexRunner.build_codex_command`.
- Network isolation: No ambient network permissions are granted.
- Unassisted baseline isolation: Control arm receives zero MCP flags and zero Turn-0 prompts.
- **Tier 2 Status: PASS.**

### 4.3 Tier 3 — Autonomous Client Invocation
In the Codex CLI architecture, non-interactive tool invocation is performed dynamically when the model emits a tool call during `codex exec`. The CLI does not support standalone offline tool invocation.  
Because running `codex exec` initiates live inference and consumes API quota:
- In strict adherence to instructions, execution was stopped at this boundary.
- Direct JSON-RPC invocation is recorded as proof of executable capability, but is explicitly **not** claimed as proof of autonomous agent triggering.
- **Tier 3 Status: PENDING AUTHORIZATION.**

---

## 5. Milestone 3 Deep-Dive: Bounded Context Retrieval

### 5.1 Depth Clamping & Output Bounds
- Tested `engram_inspect_symbol` with depths 1, 2, 3, and 10.
- Source code inspection ([`src/mcp.rs`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/mcp.rs#L428)):
  `let depth = args["depth"].as_u64().unwrap_or(2).clamp(1, 3) as usize;`
- Output size remained identical at depth 3 and depth 10 (1,432 characters / ~358 tokens).
- Prevents unbounded AST traversal and recursive dependency bloat.

### 5.2 Shell Retrieval Guidance Injection
- Verified in [`runner.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L439-L441):
  Candidate arm prompt is appended with `SHELL_RETRIEVAL_GUIDANCE`, warning the model against whole-file dumps (`cat`, `type`, `Get-Content`).
- Baseline arm prompt strictly omits this guidance, ensuring unassisted control integrity.

### 5.3 StreamingTokenTracker & Preventive Stopping
- Simulated a 5-turn sequence where cumulative tokens reached 238,000 against the 250,000 ceiling.
- At Turn 5 pre-dispatch: Headroom (12,000) was less than estimated next turn cost (210,000 or 15,000 reserve).
- Pre-dispatch guard triggered cleanly:
  - `pre_dispatch_guard_triggered = True`
  - `stop_reason = "PRE_DISPATCH_HEADROOM_EXHAUSTED"`
  - `token_ceiling_breached = False`
  - `cumulative_tokens_at_stop = 238,000 <= 250,000`
- Contrast with active generation overrun: If tokens cross 250,000 mid-turn, `stop_reason = "TOKEN_CEILING_BREACHED"` and `token_ceiling_breached = True`.
- Missing or malformed telemetry safely defaults to `conservative_reserve = 15_000` without throwing exceptions.

### 5.4 Practical Race Conditions & Polling Analysis
- In Windows non-interactive CLI environments, subprocess output streaming over stdio provides real-time, line-buffered access to model events as they are emitted.
- In contrast, external provider rollout files in `~/.codex/sessions` are written asynchronously with an operating system write-buffer latency of 100ms to 500ms.
- **Honest Finding:** Claiming an infallible mathematical "hard cap" under external file polling alone is false. A request can theoretically already be in flight when background polling detects an overrun. The Engram harness mitigates this by evaluating pre-dispatch headroom directly on synchronous inline stdout events before the next request is dispatched.

---

## 6. Milestone 4 Deep-Dive: Functional Task Completion

### 6.1 Baseline Fixture Health (Commit `1c285608`)
In a disposable isolated clone checked out to clean baseline commit `1c285608`:
- `L0_typecheck`: Exit Code 0 (**PASS**).
- `L2_unit_tests`: Exit Code 0 (**PASS**).
- `L3_production_build`: Exit Code 0 (**PASS**).
- Evaluator Checklist Scoring:
  - REQ-1 (UI layout): `FAIL` (Checkbox and username input absent in clean baseline).
  - REQ-2 (Persistence restoration): `FAIL` (LocalStorage restoration absent in clean baseline).
  - REQ-3 (Security negative check): `PASS` (No secrets in localStorage).
  - REQ-4 (Boundary check): `FAIL` (0 files modified; diff is empty).
- Confirms baseline is healthy and receptive to the Task B feature.

### 6.2 Reference-Patch Implementation & Gate Verification
The Remember Me reference implementation was applied in an isolated temporary clone checked out to `1c285608`:
- **Files Modified (10 files):**
  1. `app.component.html` (Added login form, username input, `mat-checkbox` for rememberMe)
  2. `app.component.ts` (Restores `AUTH_USERNAME_KEY` from localStorage on `ngOnInit`)
  3. `app.component.scss` (Form styling)
  4. `app.component.spec.ts` (Component tests)
  5. `auth.actions.ts` (Declared `AUTH_USERNAME_KEY`, added props to `authLogin`)
  6. `auth.actions.spec.ts` (Action tests)
  7. `auth.effects.ts` (Persists or removes username in localStorage based on `rememberMe`)
  8. `auth.effects.spec.ts` (Effect unit tests)
  9. `auth.reducer.spec.ts` (Reducer tests)
  10. `src/assets/i18n/en.json` (English translation keys; zero bulk i18n sprawl)
- **Quality Gates:**
  - `L0_typecheck`: Exit Code 0 (**PASS**)
  - `L2_unit_tests`: Exit Code 0 (**PASS**)
  - `L3_production_build`: Exit Code 0 (**PASS**)
- **Checklist Scoring ([`evaluator.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/evaluator.py)):**
  - REQ-1: **PASS** (UI layout verified)
  - REQ-2: **PASS** (Restoration logic verified)
  - REQ-3: **PASS** (Security negative invariant satisfied)
  - REQ-4: **PASS** (All 10 modified files match `allowed_file_patterns`; zero unrelated files; bulk i18n sprawl avoided)
  - **Correctness Score:** **`C = 1.00` (100% Functional Pass)**
  - `passed_all_requirements: True`

### 6.3 Separation of Claims
- **Claim A (Reference Patch Validation):** **PASS**. Proves deterministically that Task B has a valid, type-safe, production-buildable solution that satisfies all checklist requirements and security invariants.
- **Claim B (Autonomous Coding Agent Completion):** **UNPROVEN / PENDING LIVE VALIDATION**. Generating this patch autonomously by an LLM agent within the 250,000-token ceiling cannot be evaluated without live inference.

---

## 7. Milestone 5 Readiness Audit

Milestone 5 was audited strictly for readiness and **was not executed**.

### 7.1 Amendment 04 Protocol Reconciliation
- **Design:** Two-arm matched pair (Candidate `v154_arm3` [Order 1] $\longrightarrow$ Baseline `baseline_arm1` [Order 2]).
- **Deferred Arm:** Legacy `v151_arm2` is formally marked `DEFERRED_AMENDMENT_04` in telemetry and excluded from live runs.
- **Per-Arm Ceiling:** Exactly 250,000 tokens per arm.
- **Maximum Block Exposure:** Capped at 500,000 total tokens across both arms.
- **Stopping Rule:** If candidate breaches token ceiling, times out (600s), crashes, exhausts headroom, or produces invalid telemetry, execution halts immediately and skips the baseline.
- **Diagnostic Continuation:** If candidate completes execution within 250k with valid telemetry but fails acceptance ($C < 1.00$), baseline runs for diagnostic purposes.
- **Claims Suppression:** In all cases where either arm fails acceptance ($C < 1.00$) or telemetry is invalid, all comparative savings claims are strictly suppressed.
- **Dry-Run Validated Command:**
  ```powershell
  python benchmarks/matched-pair-harness/src/orchestrator.py `
    --block block_07 `
    --arms v154_arm3,baseline_arm1 `
    --engram-v154-bin benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe `
    --max-tokens-ceiling 250000 `
    --model gpt-5.6-luna `
    --reasoning medium
  ```

### 7.2 Readiness Status
**`READY FOR EXPLICIT AUTHORIZATION`**. All offline preflight gates G1 through G5 are satisfied. Execution is deferred pending user command.

---

## 8. Discrepancy Reconciliation

During this coordinated audit, the following minor discrepancies were identified and reconciled:

1. **Frozen v1.5.1 SHA-256 Typographical Difference:**  
   The user prompt string contained `474a1a7aafbb5a5b52418aa1a978a58aeae4a177c62ec17c526e165610f44e0` (with `aa1a`). The canonical hash registered in `PREREGISTRATION.md` and `isolation.py` is `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` (with `aa51a`). The candidate and frozen binary files on disk exactly match the canonical preregistration hashes bit-for-bit.
2. **Milestone 4 Baseline vs Implemented Scoring:**  
   At clean baseline commit `1c285608`, REQ-1 and REQ-2 evaluate to FAIL because the feature is not yet present. When the reference patch is applied in the isolated sandbox, REQ-1 and REQ-2 evaluate to PASS, yielding $C = 1.00$.
3. **i18n Scope Boundary Enforcement:**  
   The evaluator strictly penalizes bulk i18n sprawl ($\ge 3$ i18n files modified). The golden repository's dirty working tree contained 9 i18n modifications. In the verified reference implementation, only `src/assets/i18n/en.json` is modified, satisfying REQ-4.
4. **Evaluator Method Signature:**  
   Git diff impact is evaluated using `evaluator.analyze_git_impact(sandbox_path, patch_output_path)`.

---

## 9. Security, Sandbox & Quota Audit

- **Filesystem Isolation:** Confirmed `-s workspace-write` enforced on both arms.
- **Network Permissions:** Ambient network access remains disabled where required.
- **Security Policy:** No global auto-approvals or wildcard bypasses introduced. Only the 4 specific Engram tools are authorized.
- **Live Inference Quota:** **Exactly 0 tokens were consumed during this audit**.
- **Preservation Check:** All candidate binaries, frozen reference binaries, and the golden repository remain 100% bit-for-bit preserved.

---

## 10. Explicit Checks Requiring Live Inference Authorization

The following three checks cross the live inference boundary and require separate explicit user authorization before execution:
1. **Tier 3 MCP Tool Triggering:** Verification that the live coding agent chooses to call `engram_inspect_symbol` or `engram_get_context` during `codex exec`.
2. **Autonomous Task Completion (Milestone 4B):** Autonomous generation and repair of the Remember Me feature by the agent within the 250,000-token ceiling.
3. **Milestone 5 Live Execution:** Executing the two-arm matched pair for Stage 1 Block 07.

---

## 11. Final Recommendation: GO / NO-GO

| Criterion | Evaluation Result | Verdict |
| :--- | :--- | :--- |
| **Candidate Binary Integrity** | SHA-256 `f56347ef...` verified | **PASS** |
| **4-Tool MCP Server Protocol** | Genuine stdio JSON-RPC `tools/call` verified | **PASS** |
| **Codex CLI Configuration** | Tool registration & fine-grained approvals verified | **PASS** |
| **Bounded Context & Clamping** | Depth clamped to 3; payload bounded to 1,432 chars | **PASS** |
| **Pre-Dispatch Headroom Guard** | Preventive stopping at Turn 5 verified; telemetry separated | **PASS** |
| **Task B Functional Solvability** | Reference implementation passes L0, L2, L3, and REQ-1..4 ($C = 1.00$) | **PASS** |
| **Golden Repo Preservation** | 18 modified files and commit `1c285608` 100% untouched | **PASS** |
| **Amendment 04 Reconciliation** | Two-arm schedule, 250k ceiling, stopping rules verified | **PASS** |

### Recommendation: **EXPLICIT GO FOR LIVE BENCHMARK AUTHORIZATION**

The codebase and harness are in a hardened, verified state. There are no remaining offline blockers. Execution of Milestone 5 (Stage 1 Block 07 Matched-Pair Evaluation) may proceed as soon as explicit user authorization is granted.
