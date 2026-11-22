# Engram Phase 0–2 Measurement Pilot — Execution Checklist & Run Protocol

**Document Version:** 1.0.0  
**Status:** READY FOR OPERATIONAL EXECUTION  
**Sealed Candidate Binary:** `C:\Users\Amal P S\OneDrive\Projects\Engram\target\release\engram.exe`  
**Binary SHA-256:** `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0`  
**Git HEAD Revision:** `17fdf4a2e98f98e6d192afc28271cb8de47aa497` (Branch `Development`)  
**Golden Master Repository:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` (Commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`)  
**Sealed Preregistration:** [`benchmarks/cost-baseline/PREREGISTRATION.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/PREREGISTRATION.md) (`db3ab2956a4da40aad8f1f376aa72a50bebc96f3e1ecc3120f405f0a294ea8bb`)  
**Sealed Build Manifest:** [`benchmarks/cost-baseline/BUILD_MANIFEST.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/BUILD_MANIFEST.md) (`92147717d23d8c4fa033230b65f0e1329a674fa7dcbfbebe60f38b55639f727c`)  
**Freeze Seal Record:** [`benchmarks/cost-baseline/FREEZE_SEAL.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/FREEZE_SEAL.md) (`91bb4dff545b78cb47231454ee110ddc2957f8674d82b79a6d88f6159baeb8fe`)

---

## 1. Exact Preregistered 20-Run Schedule

Generated deterministically from Preregistered Seed `20261018`.  
All 20 runs are arranged in **ten matched pairs** (5 pairs for Task A, 5 pairs for Task B). Each pair contains exactly one Control run and one Treatment run.

| Global Seq | Task ID | Pair ID | Run in Pair | Configuration | Description | Target Sandbox Directory |
|:---:|:---:|:---:|:---:|:---:|---|---|
| **01** | **Task A** | Pair A1 | Run 1 | **Treatment (B)** | Engram Enabled | `%TEMP%\engram_pilot\run_taskA_pair1_B` |
| **02** | **Task A** | Pair A1 | Run 2 | **Control (A)** | Native Copilot | `%TEMP%\engram_pilot\run_taskA_pair1_A` |
| **03** | **Task A** | Pair A2 | Run 1 | **Control (A)** | Native Copilot | `%TEMP%\engram_pilot\run_taskA_pair2_A` |
| **04** | **Task A** | Pair A2 | Run 2 | **Treatment (B)** | Engram Enabled | `%TEMP%\engram_pilot\run_taskA_pair2_B` |
| **05** | **Task A** | Pair A3 | Run 1 | **Treatment (B)** | Engram Enabled | `%TEMP%\engram_pilot\run_taskA_pair3_B` |
| **06** | **Task A** | Pair A3 | Run 2 | **Control (A)** | Native Copilot | `%TEMP%\engram_pilot\run_taskA_pair3_A` |
| **07** | **Task A** | Pair A4 | Run 1 | **Control (A)** | Native Copilot | `%TEMP%\engram_pilot\run_taskA_pair4_A` |
| **08** | **Task A** | Pair A4 | Run 2 | **Treatment (B)** | Engram Enabled | `%TEMP%\engram_pilot\run_taskA_pair4_B` |
| **09** | **Task A** | Pair A5 | Run 1 | **Treatment (B)** | Engram Enabled | `%TEMP%\engram_pilot\run_taskA_pair5_B` |
| **10** | **Task A** | Pair A5 | Run 2 | **Control (A)** | Native Copilot | `%TEMP%\engram_pilot\run_taskA_pair5_A` |
| **11** | **Task B** | Pair B1 | Run 1 | **Treatment (B)** | Engram Enabled | `%TEMP%\engram_pilot\run_taskB_pair1_B` |
| **12** | **Task B** | Pair B1 | Run 2 | **Control (A)** | Native Copilot | `%TEMP%\engram_pilot\run_taskB_pair1_A` |
| **13** | **Task B** | Pair B2 | Run 1 | **Treatment (B)** | Engram Enabled | `%TEMP%\engram_pilot\run_taskB_pair2_B` |
| **14** | **Task B** | Pair B2 | Run 2 | **Control (A)** | Native Copilot | `%TEMP%\engram_pilot\run_taskB_pair2_A` |
| **15** | **Task B** | Pair B3 | Run 1 | **Control (A)** | Native Copilot | `%TEMP%\engram_pilot\run_taskB_pair3_A` |
| **16** | **Task B** | Pair B3 | Run 2 | **Treatment (B)** | Engram Enabled | `%TEMP%\engram_pilot\run_taskB_pair3_B` |
| **17** | **Task B** | Pair B4 | Run 1 | **Control (A)** | Native Copilot | `%TEMP%\engram_pilot\run_taskB_pair4_A` |
| **18** | **Task B** | Pair B4 | Run 2 | **Treatment (B)** | Engram Enabled | `%TEMP%\engram_pilot\run_taskB_pair4_B` |
| **19** | **Task B** | Pair B5 | Run 1 | **Treatment (B)** | Engram Enabled | `%TEMP%\engram_pilot\run_taskB_pair5_B` |
| **20** | **Task B** | Pair B5 | Run 2 | **Control (A)** | Native Copilot | `%TEMP%\engram_pilot\run_taskB_pair5_A` |

---

## 2. Per-Run Setup & Safe Workspace Isolation Protocol

> [!CRITICAL]
> **Zero Workspace Contamination Invariant:**  
> The main Engram repository (`C:\Users\Amal P S\OneDrive\Projects\Engram`) and the golden master repository (`C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`) must NEVER be modified, cleaned, or used as execution directories. Every run operates strictly in an isolated disposable sandbox.

### Step-by-Step Sandbox Preparation (Per Run)
1. **Directory Teardown:**
   - If `%TEMP%\engram_pilot\run_{id}` exists from a prior attempt, recursively remove it.
   - Purge corresponding cache at `%LOCALAPPDATA%\Engram\cache\<sandbox_project_id>`.
2. **Fresh Golden Copy:**
   - Copy the clean golden master (`angular-ngrx-material-starter` at commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`) into `%TEMP%\engram_pilot\run_{id}`.
   - Verify `git status --short` in the sandbox is completely clean.
3. **Configuration Toggling:**
   - **For Control (A) Runs:** Ensure `.vscode/mcp.json` is absent or contains `"servers": {}`. Verify no Engram tools are available to the agent.
   - **For Treatment (B) Runs:** Create `.vscode/mcp.json` pointing strictly to the sealed release binary:
     ```json
     {
       "servers": {
         "engram": {
           "type": "stdio",
           "command": "C:\\Users\\Amal P S\\OneDrive\\Projects\\Engram\\target\\release\\engram.exe",
           "args": ["-C", "${workspaceFolder}", "mcp"]
         }
       }
     }
     ```
4. **Process Cleanup Check:**
   - Verify no background `engram.exe` processes from previous sessions remain active before starting a new run (`tasklist /FI "IMAGENAME eq engram.exe"`).

---

## 3. Model & Prompt Parity Protocol

### Model Governance Rule
- **Target Model:** `Claude 3.5 Sonnet` in Copilot Agent Mode.
- **Availability Check:** Confirm `Claude 3.5 Sonnet` is selected in the VS Code Copilot Chat model dropdown.
- **Halt Rule:** If `Claude 3.5 Sonnet` is unavailable, **STOP IMMEDIATELY**. Do not silently substitute another model. Report available choices for human authorization.
- **Parity:** The identical model must be used for both arms of every matched pair.

### Verbatim Prompts (No Conversational Steering)

#### Task A (Typo Fix):
```text
In projects/angular-ngrx-material-starter/src/environments/environment.ts, correct the branding typo in appName from 'Angular Ngrx Material Starter' to 'Angular NgRx Material Starter' so that 'NgRx' matches the official casing. Do not change any other file or configuration.
```

#### Task B (Remember-Me):
```text
Add a remember-me option to the authentication flow that persists the username across reloads.
```

---

## 4. Evidence Capture & Telemetry Accounting Protocol

> [!IMPORTANT]
> **Strict Metric Separation Rule:** Measured client tokens, estimated context tokens, and account-level billing deltas are recorded in separate data fields and NEVER summed together into a single combined token number.

### Data Capture Locations (Per Run)
All run artifacts must be archived immediately upon completion:
- **Directory:** `benchmarks/cost-baseline/runs/{seq}_{task}_{config}_{pair}/`
- **Stored Artifacts:**
  1. `transcript.md` — Full exported Copilot chat session transcript.
  2. `copilot_debug.log` — Exported client debug logs (`copilotLanguageModelWrapper_*.copilotmd`).
  3. `git_diff.patch` — `git diff HEAD` from sandbox.
  4. `git_status.txt` — `git status --short` from sandbox.
  5. `acceptance_report.txt` — Output and exit codes from verification gates.
  6. `run_telemetry.json` — Structured JSON containing timestamps, auxiliary tokens, turn counts, MCP byte counts, and credit readings.

### Telemetry Recording Procedure
1. Record VS Code status-bar / billing dashboard credit value immediately before prompt submission: $C_{\text{before}}$, $T_{\text{start}}$.
2. Execute run with zero human conversational intervention.
3. Record status-bar / billing dashboard credit value immediately after agent completion: $C_{\text{after}}$, $T_{\text{end}}$.
4. Record whether account telemetry has updated or remains pending (delayed). Do not infer exact per-run credits from wrapper tokens.

### Interrupted Run Policy
- **Replacement Eligibility:** A run is eligible for replacement **only** if interrupted by an external infrastructure abort (host crash, network loss, HTTP 429 quota exhaustion). Voluntary agent halts or broken code submissions count as **Task Failures** and are never replaced.
- **Replacement Cap:** Maximum of 2 replacements per arm (maximum 4 across the entire pilot).
- **Accounting:** Interrupted-run expenditure is recorded in `benchmarks/cost-baseline/runs/interrupted/` and included in total pilot cumulative expenditure.

---

## 5. Meaningful Acceptance Procedure

### Task A Acceptance Gates
1. **Scope Check:** `scripts/verify_task_a.ps1` exits with code `0`. Confirms exact casing and strictly 1 modified file.
2. **Build / Typecheck Gate:** `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit` exits with code `0`.

### Task B Acceptance Gates
1. **Scope Check:** `scripts/verify_task_b.ps1` exits with code `0`. Confirms authorized component boundaries and zero bulk i18n sprawl.
2. **Security Negative Audit:** Zero occurrences of `localStorage.setItem` persisting `password`, `token`, `jwt`, or `secret`.
3. **Application Build Gate:** `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit` exits with code `0`.
4. **Test Suite Typecheck Gate:** `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.spec.json --noEmit` exits with code `0`.
5. **Runtime Browser Gate Status:** Explicitly recorded as **`UNVERIFIED AT RUNTIME`** in accordance with the pre-pilot probe outcome. Static checks and TypeScript compilation are **never** described as proof of end-to-end browser persistence.

### Blinded Diff Review Protocol
- Anonymized diffs (`git diff --no-prefix` with author, timestamps, and Copilot metadata stripped) are prepared after all 20 runs conclude.
- Human reviewer (User) reviews the diffs blindly without arm labels.
- Any tool-marker leaks (`<!-- BEGIN ENGRAM MANAGED BLOCK -->`) are recorded as unavoidable blinding leaks.

---

## 6. Pre-Run Operator Checklist (Tick Off Before Each Run)

- [ ] Current run sequence number, task ID, pair ID, and configuration verified against Section 1 schedule.
- [ ] Golden master repository verified clean (`git status --short` is empty) and untouched.
- [ ] Disposable sandbox `%TEMP%\engram_pilot\run_{id}` freshly created and verified clean.
- [ ] For Treatment runs: `.vscode/mcp.json` verified pointing to `target\release\engram.exe`.
- [ ] For Control runs: `.vscode/mcp.json` confirmed absent / disabled.
- [ ] VS Code opened exclusively on the disposable sandbox directory.
- [ ] No other VS Code windows or background AI tasks running.
- [ ] `Claude 3.5 Sonnet` selected in Copilot Agent Mode dropdown.
- [ ] Initial credit counter and $T_{\text{start}}$ timestamp recorded.
- [ ] Verbatim prompt submitted with zero conversational follow-ups.
