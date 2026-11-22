# Cost Baseline Specification

**Document Version:** 1.0.0  
**Phase:** Phase 0–2 Measurement Pilot  
**Status:** ACTIVE SPECIFICATION  

---

## 1. Configurations

### Configuration A — Baseline (Without Engram)
- **Agent:** GitHub Copilot in VS Code (Agent Mode).
- **Engram Status:** Disabled (MCP server removed from `.vscode/settings.json` or `mcpServers` disabled).
- **Environment:** Clean workspace snapshot restored from baseline commit.
- **Tools Available:** Native VS Code tools (`read_file`, `replace_string_in_file`, `run_in_terminal`, `grep_search`, `find_files`).

### Configuration B — Current Optimized Engram
- **Agent:** GitHub Copilot in VS Code (Agent Mode).
- **Engram Status:** Enabled, pointing to the optimized release executable (`target/release/engram.exe`).
- **Binary Verification:** Built with compact response formatting, SHA-256 preflight fingerprinting, and scope-drift enforcement.
- **MCP Response Mode:** Compact by default (`details: false` returning $\sim 700\text{--}900$ bytes; detailed mode preserved on request).
- **Tools Available:** Native VS Code tools plus Engram MCP tools (`preflight`, `get_context`, `postflight`).

---

## 2. Controlled Inputs (Identical Across Configurations)

To ensure comparability without bias:
- **Baseline Git Snapshot:** Commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` (clean working tree, identical across all runs).
- **Model Selection:** `Claude 3.5 Sonnet` (or `GPT-4o` where selectable). Must be held constant across all repetitions of A and B.
- **Agent Mode:** Copilot Workspace Agent Mode.
- **Session Isolation:** A fresh chat session and isolated workspace copy are created for each individual run.
- **Prompt Fidelity:** Prompts are pasted verbatim from `prompts/` without manual conversational steering or follow-up corrections.
- **Clean State:** Workspace caches, `.engram/` directories, and build artifacts are reset prior to every repetition.

---

## 3. Task Suite Specifications

| Task ID | Task Category | Pilot Active | Prompt File | Allowed Files | Acceptance Script |
|---|---|:---:|---|---|---|
| **Task A** | Trivial (Single-line Config) | **[x] YES** | `prompts/task_a.txt` | `projects/angular-ngrx-material-starter/src/environments/environment.ts` | `scripts/verify_task_a.ps1` |
| **Task B** | Multi-File Feature | **[x] YES** | `prompts/task_b.txt` | Auth core modules, `app.component`, `en.json` | `scripts/verify_task_b.ps1` |

---

### Task A Specification: Branding Typo Fix

- **Objective:** Fix capitalization of "NgRx" in the application configuration name.
- **Verbatim Prompt:**
  ```text
  In projects/angular-ngrx-material-starter/src/environments/environment.ts, correct the branding typo in appName from 'Angular Ngrx Material Starter' to 'Angular NgRx Material Starter' so that 'NgRx' matches the official casing. Do not change any other file or configuration.
  ```
- **Initial State:** Line 9 contains `appName: 'Angular Ngrx Material Starter'`.
- **Expected Change:** Line 9 updated to `appName: 'Angular NgRx Material Starter'`.
- **Allowed Scope:**
  - Strictly 1 file: `projects/angular-ngrx-material-starter/src/environments/environment.ts`.
  - Allowed files outside this set: **0**.
- **Automated Verification:**
  - Script: `benchmarks/cost-baseline/scripts/verify_task_a.ps1`
  - Pass Criteria: `environment.appName === 'Angular NgRx Material Starter'`, exactly 1 tracked file modified, compilation/lint succeeds.

---

### Task B Specification: Remember-Me Authentication Feature

- **Objective:** Add remember-me capability to the Angular NgRx starter application that persists the username across page reloads.
- **Verbatim Prompt:**
  ```text
  Add a remember-me option to the authentication flow that persists the username across reloads.
  ```
- **Architectural Requirements & Acceptance Checklist:**
  1. **Persistence & Prefill:**
     - When remember-me is checked on login, the username persists in local storage (`LocalStorageService`).
     - Upon subsequent page load/reload, the stored username is automatically restored and prefilled in the login field.
  2. **Opt-Out / Disabling Behavior:**
     - When remember-me is unchecked, the username is removed from persistent storage and not retained across reloads.
  3. **Security Safeguards:**
     - Under **no circumstances** may passwords, authentication tokens (JWTs), or private keys be persisted in local storage.
  4. **Existing Authentication Preservation:**
     - `authLogin` and `authLogout` actions continue to manage `isAuthenticated` correctly.
     - Logout clears active authentication session state.
  5. **UI Integration:**
     - The login toolbar/panel presents a username input and remember-me checkbox alongside the login button.
  6. **Test Verification:**
     - Unit tests in `auth.actions.spec.ts`, `auth.reducer.spec.ts`, `auth.effects.spec.ts`, and `app.component.spec.ts` pass without errors.
- **Diff-Scope Boundaries:**
  - **Allowed Files:**
    - `projects/angular-ngrx-material-starter/src/app/core/auth/auth.actions.ts`
    - `projects/angular-ngrx-material-starter/src/app/core/auth/auth.actions.spec.ts`
    - `projects/angular-ngrx-material-starter/src/app/core/auth/auth.models.ts`
    - `projects/angular-ngrx-material-starter/src/app/core/auth/auth.reducer.ts`
    - `projects/angular-ngrx-material-starter/src/app/core/auth/auth.reducer.spec.ts`
    - `projects/angular-ngrx-material-starter/src/app/core/auth/auth.effects.ts`
    - `projects/angular-ngrx-material-starter/src/app/core/auth/auth.effects.spec.ts`
    - `projects/angular-ngrx-material-starter/src/app/app/app.component.html`
    - `projects/angular-ngrx-material-starter/src/app/app/app.component.ts`
    - `projects/angular-ngrx-material-starter/src/app/app/app.component.scss`
    - `projects/angular-ngrx-material-starter/src/app/app/app.component.spec.ts`
  - **Conditionally Allowed:**
    - `projects/angular-ngrx-material-starter/src/assets/i18n/en.json` (for remember-me label / placeholder)
  - **Forbidden Sprawl Scope:**
    - Bulk translation across $\ge 3$ localization files (`ar.json`, `de.json`, `fr.json`, `es.json`, etc.).
    - Introducing unrequested full modal dialog components, new dialog services, or separate login routes.
    - Modifying feature example code in `projects/angular-ngrx-material-starter/src/app/features/examples/...`.
- **Automated Verification:**
  - Script: `benchmarks/cost-baseline/scripts/verify_task_b.ps1`

---

## 4. Run Protocol

For each repetition:
1. Restore clean snapshot of the target test workspace at baseline commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.
2. Clear any lingering `.engram/` directory in the target test workspace.
3. For Configuration B, ensure the running Engram MCP server process is active and loaded from `target/release/engram.exe`.
4. Open a new VS Code window on the workspace copy and start a fresh Copilot chat session.
5. Submit the stored prompt verbatim without follow-up questions.
6. Record start and completion timestamps.
7. Save session logs:
   - Chat transcript $\to$ `transcripts/<run_id>.txt`
   - Engram events $\to$ `engram-logs/<run_id>.jsonl`
   - Copilot debug traces $\to$ `transcripts/<run_id>_wrapper_traces/`
8. Execute the automated verification script (`verify_task_a.ps1` or `verify_task_b.ps1`).
9. Record run metrics in `results.csv`.
