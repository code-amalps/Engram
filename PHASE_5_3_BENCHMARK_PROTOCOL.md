# Engram — Phase 5.3 Live Benchmark Protocol

**Document Identifier:** `PHASE_5_3_BENCHMARK_PROTOCOL.md`  
**Execution Timestamp:** 2026-11-19T09:30:00+05:30  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Evaluation Standard:** Amendment 04 Reproducible Two-Arm Matched-Pair Protocol  

---

## 1. Experimental Objective

Empirically evaluate whether Engram (sealed Candidate 3) delivers statistically verifiable reductions in LLM agent context consumption and conversational turn inflation on real-world engineering tasks without degrading task correctness, scope containment, or test pass rates.

---

## 2. Experimental Controls & Invariants

To eliminate confounding variables and ensure scientific rigor:

| Dimension | Control Arm (`control_A`) | Treatment Arm (`treatment_B`) |
|---|---|---|
| **Base Commit** | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` | `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` |
| **Golden Repository** | `realworld-angular` (`angular-ngrx-material-starter`) | `realworld-angular` (`angular-ngrx-material-starter`) |
| **Workspace Isolation** | Ephemeral worktree in `%TEMP%\engram_pilot\runs` | Ephemeral worktree in `%TEMP%\engram_pilot\runs` |
| **Dependency Mode** | Identical physical `copy` / validated `junction` | Identical physical `copy` / validated `junction` |
| **Agent Executable** | `codex-cli 0.167.0-alpha.4.1` | `codex-cli 0.167.0-alpha.4.1` |
| **Model Pin** | `gpt-5.6-luna` | `gpt-5.6-luna` |
| **Reasoning Effort** | `medium` | `medium` |
| **Sandbox Policy** | `workspace-write` | `workspace-write` |
| **Engram Binary** | **NONE** (PATH sanitized to strip any Engram binary) | **Candidate 3** (`97C5EC47...`) |
| **Context Delivery** | Native shell inspection (`Get-Content`, `rg`) | Turn-0 Static Injection (`.github/copilot-instructions.md`) + MCP tools |
| **Stopping Condition** | Model finishes task or breaches 250k token ceiling | Model finishes task or breaches 250k token ceiling |
| **Timeout Ceiling** | 600 seconds per arm | 600 seconds per arm |
| **Evaluation Gates** | Evaluator L0 (typecheck), L3 (build) | Evaluator L0 (typecheck), L3 (build) |

---

## 3. Benchmark Tasks Specification

### 3.1 Task A (`task_a_typo`) — Micro-Change / Small Task
* **Prompt:** `"Fix the capitalization typo 'Angular Cli Tips' -> 'Angular CLI Tips' in the about component header template (projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html). Do not modify any other files or dependencies."`
* **Target File:** `projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html`
* **Forbidden Patterns:** `package.json`, `package-lock.json`, `node_modules/**`, `projects/.../app/core/**`
* **Fast-Path Eligibility:** Single-file targeted micro-fix; eligible for Engram zero-turn fast path.
* **Evaluation Rubric:**
  - `REQ-1` (Weight 0.5): Template text correctly capitalized.
  - `REQ-2` (Weight 0.5): Strict blast radius containment (zero changes to core or package manifests).
  - Gate L0: TypeScript compile passes.
  - Gate L3: Production build succeeds.

### 3.2 Task B (`task_b_remember_me`) — Multi-File Feature Extension
* **Prompt:** `"Add a remember-me option to the authentication flow that persists the username across reloads. You may use any available tools (including registered Engram MCP tools if present). Do not persist passwords or sensitive credentials."`
* **Target Subgraph:** `auth.actions.ts`, `auth.effects.ts`, `app.component.ts`, `app.component.html`, `en.json`.
* **Forbidden Patterns:** `package.json`, `package-lock.json`, `node_modules/**`, `projects/.../features/examples/**`
* **Evaluation Rubric:**
  - `REQ-1` (Weight 0.25): UI layout contains remember-me checkbox and username input.
  - `REQ-2` (Weight 0.35): Authentication effects/component restores persisted username on load.
  - `REQ-3` (Weight 0.20): Negative Security Check — zero localStorage persistence of passwords, tokens, JWTs, or secrets.
  - `REQ-4` (Weight 0.20): Boundary containment — zero edits outside auth/app components and no bulk i18n sprawl across unused languages.
  - Gate L0: TypeScript compile passes.
  - Gate L2: Spec compilation passes.
  - Gate L3: Production build succeeds.

---

## 4. Execution Order & Order-Effect Mitigation

For each matched pair $(i)$, the arm execution order is randomized using a cryptographically seeded PRNG:
- Order $\alpha$: Control Arm (`control_A`) $\to$ Treatment Arm (`treatment_B`).
- Order $\beta$: Treatment Arm (`treatment_B`) $\to$ Control Arm (`control_A`).

All execution attempts, including any rate-limited or failed attempts, will be recorded in `matched_pair_summary.json` with verbatim error messages.

---

## 5. Token & Performance Metrics Formulas

1. **Total Provider-Reported Tokens ($T_{\text{total}}$):**
   $$T_{\text{total}} = \sum_{r=1}^{R} (\text{input\_tokens}_r + \text{output\_tokens}_r)$$
2. **Token Reduction Ratio ($R$):**
   $$R = \frac{\overline{T}_{\text{control}} - \overline{T}_{\text{treatment}}}{\overline{T}_{\text{control}}} \times 100\%$$
3. **Correctness Score ($C \in [0.0, 1.0]$):**
   $$C = \sum_{k} w_k \cdot \mathbb{I}(\text{REQ}_k = \text{PASS})$$
4. **Quality-Adjusted Effective Cost ($Q$):**
   $$Q = \frac{T_{\text{total}}}{\max(C, 0.01)}$$
   *(If an arm saves tokens by failing the task, $Q$ severely penalizes the incomplete result).*

---

## 6. Execution Sample Size Strategy & Budget Safeguard

* **Strategy:**
  1. Execute Task A matched pairs (fast micro-change validation).
  2. Execute Task B matched pairs (complex multi-file context validation).
  3. Monitor model latency, quota headroom, and process timeouts after each pair.
  4. If provider quota or rate limits constrain repetitions, document exact sample size, degrees of freedom, and confidence intervals without exaggerating statistical power.
