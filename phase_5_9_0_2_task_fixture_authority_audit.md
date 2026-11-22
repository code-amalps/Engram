# Engram Phase 5.9.0.2: Authoritative Task Fixture Audit

**Phase:** 5.9.0.2  
**Target:** Task Fixtures in `benchmarks/matched-pair-harness/config/tasks/`  
**Execution Mode:** Strictly Offline Byte and JSON Audit  

---

## 1. Cryptographic Hash & Byte-Level Verification

The physical bytes of both task fixtures were read and hashed independently:

| Task Fixture | File Path | File Size (Bytes) | SHA-256 Hash | Baseline Verification |
|---|---|---:|---|---|
| **Task A Fixture** | `benchmarks/matched-pair-harness/config/tasks/task_a_typo.json` | 2,284 | `357D9B4B5777A41A8C5691475DCA8DDAC00134C480C6324E26B60FEA39F95F02` | **VERIFIED (Bit-for-Bit Match)** |
| **Task B Fixture** | `benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json` | 4,654 | `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` | **VERIFIED (Bit-for-Bit Match)** |

---

## 2. Authoritative Task A Specification

- **Schema Version:** `2.0.0`
- **Task ID:** `task_a_typo`
- **Title:** "Fix Capitalization Typo in About Component Header"
- **Description:** "Fix a capitalization error in the about component header link text without modifying unrelated modules or breaking unit tests."
- **Baseline Git Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`
- **Allowed File Patterns:**  
  `["projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html"]`
- **Forbidden File Patterns:**  
  `["package.json", "package-lock.json", "node_modules/**", "projects/angular-ngrx-material-starter/src/app/core/**"]`
- **Prompt:**  
  `"Fix the capitalization typo 'Angular Cli Tips' -> 'Angular CLI Tips' in the about component header template (projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html). Ensure no other files or dependencies are changed. CRITICAL SAFETY INSTRUCTION: Do NOT run npm install or modify package.json or node_modules under any circumstances."`
- **Preflight Target:**  
  - Target File: `projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html`
  - Original Text: `"Angular Cli Tips"`
  - Expected Text: `"Angular CLI Tips"`
- **Gating Commands:**
  - `L0_typecheck`: `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit` (Enabled: true)
  - `L1_lint`: `npm run lint` (Enabled: false)
  - `L2_unit_tests`: `npm test -- --watch=false --browsers=ChromeHeadlessNoSandbox` (Enabled: false)
  - `L3_production_build`: `npm run build` (Enabled: true)
- **Requirements:**
  - `REQ-1` (Weight 0.50): Static check that `about.component.html` contains `'Angular CLI Tips'` and does not contain `'Angular Cli Tips'`.
  - `REQ-2` (Weight 0.50): Automated diff check that only the targeted about template was modified.

---

## 3. Authoritative Task B Specification

- **Schema Version:** `2.0.0`
- **Task ID:** `task_b_remember_me`
- **Title:** "Add Remember Me Option to Authentication Flow"
- **Description:** "Add a remember-me option to the authentication flow that persists the username across reloads without persisting sensitive credentials."
- **Baseline Git Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`
- **Allowed File Patterns:**  
  `["projects/angular-ngrx-material-starter/src/app/core/auth/**", "projects/angular-ngrx-material-starter/src/app/app/app.component.*", "projects/angular-ngrx-material-starter/src/assets/i18n/en.json"]`
- **Target Files:**
  - `projects/angular-ngrx-material-starter/src/app/core/auth/auth.actions.ts`
  - `projects/angular-ngrx-material-starter/src/app/core/auth/auth.effects.ts`
  - `projects/angular-ngrx-material-starter/src/app/app/app.component.ts`
  - `projects/angular-ngrx-material-starter/src/app/app/app.component.html`
  - `projects/angular-ngrx-material-starter/src/assets/i18n/en.json`
- **Forbidden File Patterns:**  
  `["package.json", "package-lock.json", "node_modules/**", "projects/angular-ngrx-material-starter/src/app/features/examples/**"]`
- **Prompt:**  
  `"Add a remember-me option to the authentication flow that persists the username across reloads. You may use any available tools (including registered Engram MCP tools if enabled, or standard search commands) appropriate to your environment. CRITICAL SAFETY INSTRUCTION: Do NOT run npm install or modify package.json or node_modules under any circumstances. VERIFICATION GUIDANCE: When verifying your changes, do NOT run full workspace linters (npm run lint) or interactive test watchers (ng test). To verify, run bounded non-interactive checks: 'npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit' and 'npx tsc -p projects/angular-ngrx-material-starter/tsconfig.spec.json --noEmit'. Ensure all commands are strictly non-interactive and complete promptly."`
- **Gating Commands:**
  - `L0_typecheck`: `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit` (Enabled: true)
  - `L1_lint`: `npm run lint` (Enabled: false)
  - `L2_unit_tests`: `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.spec.json --noEmit` (Enabled: true)
  - `L3_production_build`: `npm run build` (Enabled: true)
- **Requirements:**
  - `REQ-1` (Weight 0.25): UI layout contains remember-me checkbox and username input field (`app.component.html`).
  - `REQ-2` (Weight 0.35): Auth persistence restoration logic (`app.component.ts` or `auth.effects.ts` restores username from `localStorage`).
  - `REQ-3` (Weight 0.20): Security Negative Rule (zero password, token, jwt, or secret persistence in `localStorage`).
  - `REQ-4` (Weight 0.20): Boundary & Blast Radius (zero edits outside allowed files, zero bulk i18n sprawl).

---

## 4. Hierarchy of Authority

The authoritative source of truth for benchmark evaluation is:
1. **The physical JSON task fixtures** in `benchmarks/matched-pair-harness/config/tasks/`.
2. **The Python evaluator implementation** in `benchmarks/matched-pair-harness/src/evaluator.py`.

Any conflicting descriptions in high-level prose, reports, or documentation summaries are **subservient and non-authoritative**.
