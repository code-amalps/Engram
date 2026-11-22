# Engram Phase 5.9.0: Task A (`task_a_typo`) Acceptance & Verification Specification

**Execution Phase:** 5.9.0  
**Task Identifier:** `task_a_typo`  
**Archetype:** Micro-edit / Targeted Template Repair  
**Target Repository:** `angular-ngrx-material-starter`  
**Baseline Git Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`  
**Fixture Checksum:** `357D9B4B5777A41A8C5691475DCA8DDAC00134C480C6324E26B60FEA39F95F02`  

---

## 1. Task Objective and Specification

The agent must correct a specific typographical capitalization error in the About component header template:
- **Original Text:** `"Angular Cli Tips"`
- **Required Text:** `"Angular CLI Tips"`
- **Target File:** `projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html`

---

## 2. Blast Radius & Modification Boundaries

| Category | Permitted Patterns | Forbidden Patterns |
| :--- | :--- | :--- |
| **Allowed Files** | `projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html` | All other files |
| **Forbidden Areas** | None | `package.json`, `package-lock.json`, `node_modules/**`, `projects/.../app/core/**` |

---

## 3. Objective Acceptance Requirements

### REQ-1: Exact Typo Correction (Weight: 0.50)
- **Evaluation Type:** `STATIC_CHECK` via `evaluator.py`.
- **Validation Logic:**
  1. File `about.component.html` exists and is accessible.
  2. Strips HTML comments (`<!-- ... -->`) to eliminate commented-out false positives.
  3. Normalizes whitespace across word boundaries.
  4. Asserts that normalized content contains `"Angular CLI Tips"` AND does NOT contain `"Angular Cli Tips"`.

### REQ-2: Boundary & Blast Radius Integrity (Weight: 0.50)
- **Evaluation Type:** `AUTOMATED_DIFF` via `evaluator.py`.
- **Validation Logic:**
  1. Inspects `git diff --name-only` against baseline commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.
  2. Asserts that the modified files list contains exactly one file (`about.component.html`).
  3. Fails immediately if any core, package, or configuration file was touched.

---

## 4. Quality Verification Gates

| Gate ID | Stage | Command | Policy |
| :---: | :--- | :--- | :---: |
| **L0** | Typecheck | `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit` | Mandatory |
| **L1** | Linting | `npm run lint` | Disabled |
| **L2** | Unit Tests | `npm test -- --watch=false --browsers=ChromeHeadlessNoSandbox` | Disabled |
| **L3** | Build | `npm run build` | Mandatory |

---

## 5. Mutual Acceptance Invariant

A run achieves `accepted = True` if and only if:
1. `score == 1.00` (both REQ-1 and REQ-2 PASS).
2. Gates L0 and L3 exit with code 0.
3. Telemetry is verified valid with zero sequence gaps or hash chain breaks.
