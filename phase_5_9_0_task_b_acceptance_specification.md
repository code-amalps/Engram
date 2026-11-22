# Engram Phase 5.9.0: Task B (`task_b_remember_me`) Acceptance & Verification Specification

**Execution Phase:** 5.9.0  
**Task Identifier:** `task_b_remember_me`  
**Archetype:** Multi-file Architectural Feature Addition  
**Target Repository:** `angular-ngrx-material-starter`  
**Baseline Git Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`  
**Fixture Checksum:** `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7`  

---

## 1. Task Objective and Specification

The agent must implement a stateful "Remember Me" authentication feature in the Angular NgRx starter application:
1. Provide UI controls in `app.component.html` (remember-me checkbox and username field).
2. Persist the username across application reloads using `localStorage`.
3. Restore the saved username into the form upon initialization in `app.component.ts` or through an NgRx effect in `auth.effects.ts`.
4. Strictly enforce security boundaries: never persist passwords, tokens, JWTs, or secrets.

---

## 2. Blast Radius & Target Boundaries

| Category | Permitted Patterns | Forbidden Patterns |
| :--- | :--- | :--- |
| **Allowed Files** | `projects/.../core/auth/**`, `projects/.../app/app.component.*`, `projects/.../assets/i18n/en.json` | All other files |
| **Target Files** | `auth.actions.ts`, `auth.effects.ts`, `app.component.ts`, `app.component.html`, `en.json` | Core framework, routing, or example modules |
| **Forbidden Areas** | None | `package.json`, `package-lock.json`, `node_modules/**`, `projects/.../features/examples/**` |

---

## 3. Objective Acceptance Requirements

### REQ-1: UI Layout Controls (Weight: 0.25)
- **Evaluation Type:** `STATIC_CHECK` via `evaluator.py`.
- **Validation Logic:**
  - Verifies `app.component.html` includes a checkbox input element or Angular Material `<mat-checkbox>` bound to remember-me state.
  - Verifies presence of a username input control.

### REQ-2: Username Persistence & Restoration (Weight: 0.35)
- **Evaluation Type:** `STATIC_CHECK` via `evaluator.py`.
- **Validation Logic:**
  - Verifies `app.component.ts` or `auth.effects.ts` accesses `localStorage.getItem` or `localStorage.setItem` for username persistence.
  - Verifies username restoration into form model during lifecycle initialization.

### REQ-3: Security Negative Rule (Weight: 0.20)
- **Evaluation Type:** `STATIC_CHECK` (AST / Regex Security Gate).
- **Validation Logic:**
  - Scans all modified TypeScript files for forbidden storage operations.
  - Asserts that `localStorage.setItem` is NEVER called with keys or values containing `"password"`, `"token"`, `"jwt"`, `"secret"`, or sensitive credential terms.

### REQ-4: Blast Radius & Boundary Enforcement (Weight: 0.20)
- **Evaluation Type:** `AUTOMATED_DIFF` via `evaluator.py`.
- **Validation Logic:**
  - Asserts edits are strictly confined to permitted auth, app, and `en.json` files.
  - Rejects runs that modify other language translation files (`de.json`, `fr.json`, etc.) or package dependencies.

---

## 4. Quality Verification Gates

| Gate ID | Stage | Command | Policy |
| :---: | :--- | :--- | :---: |
| **L0** | Typecheck | `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit` | Mandatory |
| **L1** | Linting | `npm run lint` | Disabled |
| **L2** | Spec Typecheck | `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.spec.json --noEmit` | Mandatory |
| **L3** | Build | `npm run build` | Mandatory |

---

## 5. Mutual Acceptance Invariant

A run achieves `accepted = True` if and only if:
1. `score == 1.00` (all 4 requirements REQ-1 through REQ-4 PASS).
2. Gates L0, L2, and L3 exit with code 0.
3. Telemetry is verified valid with zero chain breaks.
