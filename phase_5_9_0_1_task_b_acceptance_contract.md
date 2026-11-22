# Engram Phase 5.9.0.1: Task B Functional Acceptance Specification & Contract

**Phase:** 5.9.0.1  
**Task Identifier:** `task_b_remember_me`  
**Task Fixture:** `benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json`  
**Evaluator Implementation:** `benchmarks/matched-pair-harness/src/evaluator.py`  
**Execution Mode:** Strictly Offline Specification & Forensic Audit  

---

## 1. Path & Target File Discrepancy Reconciliation

Phase 5.9.0 documentation erroneously cited target paths under `src/app/core/core.module.ts`, `src/app/core/auth/auth.models.ts`, and `src/app/settings/settings.reducer.ts`.
Forensic inspection of the actual golden repository (`angular-ngrx-material-starter`) and [task_b_remember_me.json](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/config/tasks/task_b_remember_me.json) establishes the authoritative file locations:

| Erroneous Phase 5.9.0 Path | Authoritative Repository / Fixture Path | File Exists in Golden Repo? |
|---|---|---|
| `src/app/core/core.module.ts` | `projects/angular-ngrx-material-starter/src/app/core/core.module.ts` | **YES** |
| `src/app/core/auth/auth.models.ts` | `projects/angular-ngrx-material-starter/src/app/core/auth/auth.actions.ts` / `auth.effects.ts` | **YES** |
| `src/app/settings/settings.reducer.ts` | `projects/angular-ngrx-material-starter/src/app/core/settings/settings.reducer.ts` | **YES** |
| *(Omitted in 5.9.0)* | `projects/angular-ngrx-material-starter/src/app/app/app.component.html` | **YES** |
| *(Omitted in 5.9.0)* | `projects/angular-ngrx-material-starter/src/app/app/app.component.ts` | **YES** |
| *(Omitted in 5.9.0)* | `projects/angular-ngrx-material-starter/src/assets/i18n/en.json` | **YES** |

All actual application code resides within the Angular multi-project sub-path `projects/angular-ngrx-material-starter/src/`.

---

## 2. Comprehensive Functional Acceptance Contract

The feature requires adding a `rememberMe` option to the authentication flow that persists the username across browser sessions without persisting credentials.

```
+---------------------------------------------------------------------------------------------------+
| Task B Acceptance Architecture                                                                    |
+--------------------------+------------------------------+-------------------------+---------------+
| REQ-1: UI Elements       | REQ-2: Persistence & Hydrate | REQ-3: Security Negative| REQ-4: Scope  |
| (Weight: 0.25)           | (Weight: 0.35)               | (Weight: 0.20)          | (Weight: 0.20)|
+--------------------------+------------------------------+-------------------------+---------------+
| app.component.html has   | app.component.ts or effects  | ZERO persistence of     | Only allowed  |
| rememberMe checkbox and  | restores username from       | password, token, jwt, or| auth/app/i18n |
| username input field     | localStorage on load         | secrets to localStorage | files touched |
+--------------------------+------------------------------+-------------------------+---------------+
```

### 2.1 Domain Specifications

#### A. Data Model & State
1. **Property Definition:** A `rememberMe: boolean` field must be supported in the login state or component form model.
2. **Default Value:** Must default to `false` (opt-in principle).
3. **Contract Preservation:** Existing NgRx auth state structure (`isAuthenticated: boolean`) must remain valid.

#### B. Initialization & Hydration
1. **Restore on Load:** `app.component.ts` (via `ngOnInit`) or `auth.effects.ts` (via an init effect) must check `localStorage.getItem(...)` for the persisted username.
2. **Safe Fallback:** If `localStorage` is empty, malformed, null, or throws (e.g., security restricted in private browsing), the application must fail-safe to an empty username without runtime crashes.

#### C. Persistence Logic
1. **Save on Login:** When `rememberMe` is checked during login, the username is written to `localStorage`.
2. **Clear on Uncheck / Logout:** If `rememberMe` is unchecked or upon logout, the persisted username key must be removed or overwritten.

#### D. Security Contract (Strict Negative Rule)
1. **Zero Secret Persistence:** Under NO circumstances may passwords, access tokens, refresh tokens, or JWTs be written to `localStorage` under the guise of "remember me".
2. **Automated Enforcement:** `evaluator.py` executes:
   ```python
   re.search(r'localStorage\s*\.\s*setItem\s*\([^)]*(?:password|token|secret|jwt)', ts_content, re.IGNORECASE)
   ```
   Any match triggers an **immediate 0.0 score on REQ-3** and causes task failure.

#### E. Blast Radius & Blast Boundary (`REQ-4`)
1. **Strict File Containment:** Changes are restricted to `core/auth/**`, `app/app.component.*`, and `assets/i18n/en.json`.
2. **Zero Dependency Drift:** Modifying `package.json`, `package-lock.json`, or running `npm install` immediately fails the run (`git_impact.dependency_mutated`).
3. **i18n Containment:** Modifying $\ge 3$ internationalization files triggers an automated sprawl failure (`len(i18n_files) >= 3`).

---

## 3. Evaluator Implementation Audit & Limitations

### 3.1 What the Current Evaluator Checks
1. **Static Regex Search:** Checks for `rememberMe|remember|mat-checkbox` and `username|matInput` in `app.component.html`.
2. **Hydration Regex Search:** Checks for `localStorage` and `getItem|AUTH_USERNAME|remember` in `app.component.ts` or `auth.effects.ts`.
3. **TypeScript Compilation Gates:**
   - `L0_typecheck`: `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit`
   - `L2_unit_tests`: `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.spec.json --noEmit`

### 3.2 Key Technical Limitation: No Runtime Browser Testing
> [!WARNING]
> **Static Regex vs End-to-End Runtime Execution:**
> `evaluator.py` does not execute headless Karma unit tests or live browser automation (Karma and linting are explicitly disabled in `gates.L1_lint` and `gates.L2_unit_tests` to prevent non-deterministic hangs in headless environments).
> 
> Therefore, acceptance certifies that the code **syntactically contains the required architecture, passes strict TypeScript typechecking, respects security negative constraints, and preserves repository blast boundaries**. Full behavioral interaction is verified via compilation and static semantic inspection.
