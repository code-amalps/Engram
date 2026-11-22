# Engram Phase 5.4: Task B Preflight, Offline Validation & Experimental Readiness Report
## Rigorous Offline Audit, Architectural Context Bounding & Four-Arm Experimental Protocol

**Document Version:** 1.0.0-PREFLIGHT-TASK-B  
**Date:** November 19, 2026  
**Audited Benchmark:** Task B (`task_b_remember_me`) — Add Remember Me Option to Authentication Flow  
**Target Repository:** `angular-ngrx-material-starter` (Commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`)  
**Auditor:** Antigravity Autonomous Systems Engineer / Antigravity AI  
**Scope:** Offline harness preflight, budget calibration verification, fixture validation, response-bounding audit, classifier escalation testing, and deterministic simulation. No live LLM calls, no candidate rebuilds, no runtime code mutations.

---

## 1. Executive Summary

Following the completion of the forensic audit of Task A (Live 01 vs Live 02), this preflight phase evaluates the experimental readiness of **Task B (`task_b_remember_me`)** across the Phase 5.4 four-arm architecture:
- **Arm A:** Native Baseline (Unassisted)
- **Arm B:** Candidate 3 Always-On (Legacy unpruned impact engine)
- **Arm C:** Candidate 4 Always-On (Bounded 16 KB symbol inspection)
- **Arm D:** Candidate 4 Adaptive (Complexity-routed architecture)

### Key Preflight Findings:
1. **Amendment 03 Justification Verified:**
   Historical evidence from Block 06 demonstrates that an unassisted native baseline required 253,748 cumulative tokens across 9 turns before being interrupted by the 250,000 ceiling. Amendment 03 formally calibrated the operational token ceiling to **350,000 tokens** per arm for `task_b_remember_me`. Adopting 350,000 tokens (with a 45,000 headroom reserve) is empirically justified and prevents premature truncation of the baseline arm.
2. **Task Complexity Routing Validated:**
   `TaskComplexityClassifier` was evaluated against the authoritative Task B prompt. Triggered by the architectural keyword `"authentication"`, the task deterministically classified as **`COMPLEX` $\rightarrow$ `FULL`** in **4.04 ms**. In `FULL` mode, Arm D correctly enables the full tool suite (`engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight`), validating that Candidate 4 Adaptive does not attempt an inappropriate `BYPASS` on multi-file feature additions.
3. **Candidate 4 Symbol Bounding Succeeded:**
   For the primary authentication symbol `AuthEffects`, Candidate 4 bounded MCP response size to **6,518 bytes** (a 75.0% reduction from Candidate 3’s **26,102 bytes**), strictly adhering to the 16,384-byte ceiling while preserving target file location (`auth.effects.ts`), line ranges (`[13, 44]`), direct entities, and structured omission manifests.
4. **Critical Discovery — Unbounded `engram_get_context` Omission Manifest:**
   While Candidate 4 successfully bounded `engram_inspect_symbol` to 16 KB, offline JSON-RPC inspection revealed that `engram_get_context` still emits an unbounded `omission_manifest` dictionary containing every unselected repository file, bloating the response to **95,010 bytes (95 KB)**. Re-ingesting a 95 KB tool payload across multiple turns represents a material risk of context inflation.
5. **Live 01 Registration Defect Prevented:**
   Offline regression testing verified that `runner.py` now guarantees active MCP tool registration for Arms B and C even if `fast_path_active` evaluates to True. Always-on arms can no longer silently degrade into native baseline runs.

---

## 2. Repository and Candidate Binary Integrity

All candidate binaries, harness source files, and repository fixtures were verified on disk via SHA-256 cryptographic hashing:

| Component | Path | SHA-256 Hash | Integrity Status |
| :--- | :--- | :--- | :---: |
| **Candidate 3 Binary** | `benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe` | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | **SEALED / VERIFIED** |
| **Candidate 4 Binary** | `benchmarks/matched-pair-harness/bin/candidate-phase4/engram.exe` | `FC605BC0CF3B6C9C5FDB1E7C83EE666FD7E049FD9AE25C893D478578681699F8` | **SEALED / VERIFIED** |
| **Task B Definition** | `benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json` | `378F7B36EF05847B0108920C8E3253B82061ECBB32812C4119842BD995B90781` | **VERIFIED** |
| **Runner Source** | `benchmarks/matched-pair-harness/src/runner.py` | `A813F5C73994E97663C42CD056703FD4BA7AB95EF15B1ADE495E1818CFD0C370` | **VERIFIED** |
| **Orchestrator Source**| `benchmarks/matched-pair-harness/src/orchestrator.py` | `E5C8A0243D69001C1B989EE1CA7A1C20BC1394AAD7A8E3DDF7ABE98898A3428D` | **VERIFIED** |
| **Classifier Source** | `benchmarks/matched-pair-harness/src/classifier.py` | `5F9D51E1AB6F92754BC2159D163678CE90379F1B25E4F9CE8F5326E246ECC655` | **VERIFIED** |
| **Golden Repository** | `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` | Git HEAD: `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` | **CLEAN / VERIFIED** |

*Working Tree Audit:* Current branch is `feature/phase2-stage5-remediation` at commit `7106a2f`. Uncommitted working-tree modifications from prior milestones remain preserved without mutation.

---

## 3. Amendment 03 and Token Budget Verification

### 3.1 Historical Evidence and Approval Record
On October 29, 2026, the first live Task B execution (**Block 06**) was conducted under the default 250,000-token ceiling. The unassisted native baseline (`baseline_arm1`) reached 253,748 tokens across 9 turns while implementing feature logic across 6 files (+45 / -7 lines), but was terminated by the circuit-breaker immediately prior to running `npx tsc` compiler verification.

Following this incident, [`benchmarks/AMENDMENT_03_TASK_B_OPERATIONAL_CEILING_CALIBRATION.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/AMENDMENT_03_TASK_B_OPERATIONAL_CEILING_CALIBRATION.md) was formally drafted and approved:
- **Amendment Status:** `3.2.0-APPROVED` (2026-10-29).
- **Approved Operational Ceiling:** **350,000 tokens** per arm.
- **Rationale:** An unassisted baseline agent exploring a multi-file Angular/NgRx feature requires 10–12 turns (~280k–320k tokens) to finish code edits and execute non-interactive TypeScript compilation. Maintaining 250,000 tokens on Task B guarantees artificial baseline truncation.

### 3.2 Verification of Budget Parameters

| Parameter | Registered Value (Amendment 03) | Configured Value (Harness Default) | Proposed Phase 5.4 Value | Source / Evidence | Verdict |
| :--- | :---: | :---: | :---: | :--- | :---: |
| **Session Ceiling** | 350,000 tokens | 250,000 tokens | **350,000 tokens** | Amendment 03 Sec 2.1 | **JUSTIFIED** |
| **Headroom Reserve**| 45,000 tokens | 45,000 tokens | **45,000 tokens** | `runner.py` line 61 | **VERIFIED** |
| **Effective Threshold**| 305,000 tokens | 205,000 tokens | **305,000 tokens** | $350\text{k} - 45\text{k}$ | **VERIFIED** |
| **Per-Request Context Budget**| Model Context Window (250k+) | Model Context Window (250k+) | Model Context Window (250k+) | `gpt-5.6-luna` API spec | **VERIFIED** |
| **MCP Response-Size Limit** | 16,384 bytes | 16,384 bytes | **16,384 bytes** | Candidate 4 engine spec | **VERIFIED** |

*Methodological Rule:* The 350,000-token ceiling applies symmetrically to all four arms (`arm_a_native`, `arm_b_cand3_always_on`, `arm_c_cand4_always_on`, `arm_d_cand4_adaptive`). It is not adjusted dynamically during execution.

---

## 4. Task B Fixture and Authoritative Acceptance Contract

### 4.1 Task Specification
- **Task ID:** `task_b_remember_me`
- **Baseline Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`
- **Task Prompt:**
  > "Add a remember-me option to the authentication flow that persists the username across reloads. You may use any available tools (including registered Engram MCP tools if enabled, or standard search commands) appropriate to your environment. CRITICAL SAFETY INSTRUCTION: Do NOT run npm install or modify package.json or node_modules under any circumstances. VERIFICATION GUIDANCE: When verifying your changes, do NOT run full workspace linters (npm run lint) or interactive test watchers (ng test). To verify, run bounded non-interactive checks: 'npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit' and 'npx tsc -p projects/angular-ngrx-material-starter/tsconfig.spec.json --noEmit'. Ensure all commands are strictly non-interactive and complete promptly."

### 4.2 Target and Allowed Files
Target files span UI, state management, and localization:
1. `projects/angular-ngrx-material-starter/src/app/core/auth/auth.actions.ts` (NgRx actions)
2. `projects/angular-ngrx-material-starter/src/app/core/auth/auth.effects.ts` (State effects & localStorage persistence)
3. `projects/angular-ngrx-material-starter/src/app/app/app.component.ts` (Component state restoration)
4. `projects/angular-ngrx-material-starter/src/app/app/app.component.html` (UI checkbox & input)
5. `projects/angular-ngrx-material-starter/src/assets/i18n/en.json` (Localization strings)

### 4.3 Deterministic Quality & Acceptance Gates
To achieve full acceptance ($C = 1.00$), an arm must pass all four criteria:
- **L0 Typecheck (App):** `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit`
- **L2 Spec Typecheck (Tests):** `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.spec.json --noEmit`
- **L3 Production Build:** `npm run build`
- **REQ-1 (UI Layout, Weight 0.25):** `app.component.html` contains remember-me checkbox and username input.
- **REQ-2 (Auth Flow, Weight 0.35):** `app.component.ts` or `auth.effects.ts` restores username from localStorage.
- **REQ-3 (Security Negative Rule, Weight 0.20):** Strict check ensuring no password, token, jwt, or secret is written to localStorage.
- **REQ-4 (Boundary Blast Radius, Weight 0.20):** Edits confined strictly to auth/app components and `en.json`. Zero edits to `package.json`, `node_modules/`, or unrelated feature modules.

---

## 5. Four-Arm Configuration & Registration Audit

Every arm was independently validated against the Phase 5.4 harness implementation:

| Arm Identity | Candidate Binary | Intended Mode | Registered Tools Exposed in Model Session | Turn-0 Context Injection |
| :--- | :---: | :---: | :--- | :--- |
| **Arm A (Native)** | None | NATIVE | **None** (`mcp_server_status: DISABLED`) | None |
| **Arm B (Cand 3)** | Candidate 3 (`97C5EC47`) | ALWAYS_ON | `engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight` | `.github/copilot-instructions.md` |
| **Arm C (Cand 4)** | Candidate 4 (`FC605BC0`) | ALWAYS_ON | `engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight` | `.github/copilot-instructions.md` |
| **Arm D (Cand 4)** | Candidate 4 (`FC605BC0`) | ADAPTIVE | `engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight` (`FULL`) | Routed Turn-0 prompt injection |

### Regression Check: Live 01 Always-On Defect
- **Test:** Simulating `fast_path_active = True` during `runner.build_codex_command`.
- **Observed Result:** `build_codex_command` enforces `bypass_mcp = fast_path_active and not is_always_on`. For Arms B and C, `is_always_on` is True, ensuring that `-c mcp_servers.engram...` flags are strictly emitted.
- **Verdict:** **PASS.** Always-on arms cannot silently run as native baselines.

---

## 6. Adaptive Classification and Escalation Validation

`TaskComplexityClassifier` was audited across deterministic offline test suites:

### 6.1 Task B Prompt Classification
- **Input:** Authoritative Task B prompt from `task_b_remember_me.json`.
- **Classification Output:**
  - **Tier:** `COMPLEX`
  - **Routed Mode:** `FULL`
  - **Execution Latency:** **4.04 ms**
  - **Triggers Detected:** `"authentication"`
  - **Signals:** Word count: 98, Files mentioned: 3 (`package.json`, `tsconfig.app.json`, `tsconfig.spec.json`).
  - **Determinism:** Identical across 10 repeated offline executions.

### 6.2 Offline Multi-Scenario Suite Results

| Scenario | Input Prompt | Expected Tier | Actual Tier | Expected Mode | Actual Mode | Status |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: |
| **Scenario 1: Simple Typo** | "Fix capitalization typo in header template about.html" | `SIMPLE` | `SIMPLE` | `BYPASS` | `BYPASS` | **PASS** |
| **Scenario 2: Medium Feature** | "Update button styling and padding in nav.component.ts and nav.html" | `MEDIUM` | `MEDIUM` | `LEAN` | `LEAN` | **PASS** |
| **Scenario 3: Complex Architecture**| "Implement authentication remember-me feature across store and UI" | `COMPLEX` | `COMPLEX` | `FULL` | `FULL` | **PASS** |
| **Scenario 4: Task B Prompt** | Authoritative Task B prompt | `COMPLEX` | `COMPLEX` | `FULL` | `FULL` | **PASS** |
| **Scenario 5: Escalation (LEAN $\rightarrow$ FULL)** | Scoped navigation prompt escalated with cross-cutting auth dependency | `MEDIUM` $\rightarrow$ `COMPLEX` | `MEDIUM` $\rightarrow$ `COMPLEX` | `LEAN` $\rightarrow$ `FULL` | `LEAN` $\rightarrow$ `FULL` | **PASS** |

*Escalation Audit:* Calling `cl_res.escalate("FULL", "Discovered cross-cutting auth effects dependency")` successfully transitions `routed_mode` to `FULL`, increments `escalation_count` to 1, and appends a UTC-timestamped event to `escalation_history`.

---

## 7. Candidate 4 Context Quality and Response-Boundary Analysis

Symbol inspection was audited via live JSON-RPC MCP calls against the golden repository for Task B symbols:

### 7.1 Primary Symbol: `AuthEffects`

| Metric | Candidate 3 (Legacy) | Candidate 4 (Phase 5.4) | Delta | Evaluation |
| :--- | :---: | :---: | :---: | :--- |
| **MCP Response Bytes** | **26,102 bytes** | **6,518 bytes** | **-19,584 bytes (-75.0%)** | Strict compliance with 16 KB hard ceiling. |
| **Target File Path** | `.../auth.effects.ts` | `.../auth.effects.ts` | Identical | Preserved accurately. |
| **Line Range** | `[13, 44]` | `[13, 44]` | Identical | Preserved accurately. |
| **Direct Entities** | 1 (`AuthEffects`) | 1 (`AuthEffects`) | Identical | Key symbol retained. |
| **Impact Cone Keys** | 11 keys | 11 keys | Identical | Structural schema preserved. |
| **Omission Manifest** | Missing | Present (`omissions: []`) | Added | Transparent omissions metadata. |
| **JSON Syntactic Validity**| Valid JSON | Valid JSON | Identical | Clean parse; no truncation corruption. |

### 7.2 Secondary Symbol: `AppComponent`
- **Candidate 3 Response Size:** **175,893 bytes (175.8 KB)**
- **Candidate 4 Response Size:** **105,681 bytes (105.6 KB)** (via CLI unpruned)
- *Note:* While `AppComponent` is the root component of the Angular application with massive downstream imports, Candidate 4 reduces entity fanout by 40%.

### 7.3 Critical Vulnerability: `engram_get_context` Omission Manifest Bloat
When `engram_get_context` is invoked for Task B:
- **Budget Enforced on Code Excerpts:** 2 target files included (`core.module.ts`, `auth.effects.spec.ts`), totaling 2,975 bytes.
- **Unbounded Omission Manifest:** `omission_manifest` dictionary contains an array of every omitted file in the repository, totaling **70,556 bytes**.
- **Total Serialized Response Size:** **95,010 bytes (95 KB)**.
- **Impact:** While `engram_inspect_symbol` is strictly bounded to 16 KB in Candidate 4, `engram_get_context` still returns 95 KB of JSON. If the LLM invokes `engram_get_context` in Turn 1, it immediately ingests ~24,000 prompt tokens.

---

## 8. Offline Four-Arm Dry-Run Results

The 8 required offline simulation scenarios were executed using deterministic mocks and stubs:

```
[OFFLINE TEST SUITE: test_task_b_offline_preflight.py]
  Scenario A (Normal Completion):           PASS (All 4 arms initialize, execute, and generate artifacts)
  Scenario B (Context-Budget Pressure):     PASS (Halts at 40k remaining < 45k reserve under 350k ceiling)
  Scenario C (Oversized MCP Bounding):      PASS (AuthEffects bounded to 6,518 bytes <= 16,384 limit)
  Scenario D (MCP Tool Failure / Missing):  PASS (Returns structured "Target not found" without crash)
  Scenario E (Duplicate Tool Calls):        PASS (Deterministic byte-identical responses: 6,518 bytes)
  Scenario F (Adaptive Escalation):         PASS (Transitions LEAN -> FULL with recorded event)
  Scenario G (Always-On Registration):      PASS (Guarantees -c mcp_servers.engram even if fast_path=True)
  Scenario H (Incomplete Arm Suppression):  PASS (TREATMENT_FAILED_ACCEPTANCE; savings suppressed to None)
```

---

## 9. Budget Accounting and Fair Comparison Rules

### 9.1 Token Accounting Invariants
1. **No Double-Counting:** `StreamingTokenTracker` strictly separates `thread_cumulative_max` (billed session tokens) from single-turn context requests.
2. **Provider Authoritative Counts:** Halting decisions use streaming usage events (`token_usage_record`, `event_msg`) emitted directly by Codex. Payload byte-to-token estimates ($bytes / 4$) are used solely for reporting.
3. **Prompt Cache Accounting:** Cached prompt tokens and uncached prompt tokens are tracked separately. Billed usage reflects total prompt tokens ($cached + uncached$).
4. **Pre-Dispatch Intervention:** The guard intervenes on pre-dispatch boundaries before an API turn is sent.

### 9.2 Comparative Evaluation Rules
1. **Completed Arms Only:** Relative token reduction ($S$) is calculated strictly when both compared arms achieve $C = 1.00$.
2. **Halting Failure Suppression:** If an arm is terminated by `PRE_DISPATCH_HEADROOM_EXHAUSTED` or `TOKEN_CEILING_BREACHED`, its correctness is set to $C = 0.00$, and comparative savings claims are suppressed (`None`).
3. **Primary Evaluation Metrics:**
   - Functional Correctness ($C = 1.00$)
   - Total Model Tokens for Completed Tasks
   - Number of Conversational Turns
   - Peak Single-Request Context
   - Wall-Clock Latency (seconds)

---

## 10. Proposed Live Experiment Protocol (Task B)

When authorized, Task B shall be dispatched under the following strict configuration:

- **Block ID:** `tetrad_task_b_live_01`
- **Task Fixture:** `benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json`
- **Model:** `gpt-5.6-luna` (Reasoning Effort: `medium`, Provider: `cloud/openai`)
- **Baseline Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`
- **Execution Order (4 Arms):** `arm_a_native` $\rightarrow$ `arm_b_cand3_always_on` $\rightarrow$ `arm_c_cand4_always_on` $\rightarrow$ `arm_d_cand4_adaptive`
- **Session Token Ceiling:** **350,000 tokens** per arm (`--max-tokens-ceiling 350000`)
- **Headroom Reserve:** **45,000 tokens** (`--headroom-reserve 45000`)
- **Isolation Strategy:** Full filesystem write copy (`--dependency-strategy copy`)
- **Timeout:** 600 seconds per arm
- **Candidate Binaries:**
  - Candidate 3: `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777`
  - Candidate 4: `FC605BC0CF3B6C9C5FDB1E7C83EE666FD7E049FD9AE25C893D478578681699F8`

---

## 11. Remaining Risks and Preconditions

1. **Risk of `engram_get_context` 95 KB Payload:**
   Because Candidate 4 engine remediation was restricted to `engram_inspect_symbol`, `engram_get_context` still returns 95 KB of JSON due to the unpruned `omission_manifest`. In Task B, an arm invoking `engram_get_context` will consume ~24k prompt tokens on Turn 1.
2. **Always-On Arm Context Inflation:**
   Arms B and C (and Arm D in `FULL` mode) have access to `engram_get_context`. Even with the 350,000 ceiling, if an arm makes multiple tool calls across 10+ turns, it remains vulnerable to headroom exhaustion.

---

## 12. Final Authorization Gate Decision

### **`CONDITIONALLY READY FOR LIVE TASK B`**

### Mandatory Preconditions Before Live Execution:
1. **Explicit Protocol Approval:** The user must explicitly approve the live execution command with `--max-tokens-ceiling 350000`.
2. **Acknowledgment of `engram_get_context` Payload Exposure:** The user must confirm acceptance of the observed 95 KB `engram_get_context` payload risk, or alternatively authorize a Candidate 5 engine remediation to prune `omission_manifest` in `engram_get_context` prior to live dispatch.
3. **No Unapproved Live Dispatches:** No live model calls shall be initiated until the user provides explicit, separate approval.
