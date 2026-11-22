# Engram Amendment 05 — Milestone 2A: Smoke-Test Specification Corrections

## Executive Objective

This document provides a documentation-only forensic correction of the Engram Amendment 05 Milestone 2 smoke-test specification.

All investigations were conducted **strictly offline** with zero model inference, zero API calls, zero network operations, zero benchmark executions, and zero spend (\$0.00). In accordance with strict governance boundaries, no existing Milestone 2 artifacts were edited; all corrections, reconciliations, and qualifications are recorded herein.

---

## 1. Candidate Binary and Arm Mapping Correction

### 1.1 Forensic Analysis of Harness Arm Resolution
An inspection of `benchmarks/matched-pair-harness/src/orchestrator.py` (lines 94–112) reveals how arms map to executable binaries:

```python
self.engram_v151_bin = Path(engram_v151_bin) if engram_v151_bin else DEFAULT_ENGRAM_V151_BIN
self.engram_v154_bin = Path(engram_v154_bin) if engram_v154_bin else DEFAULT_ENGRAM_V154_BIN
self.expected_v154_sha256 = expected_v154_sha256 or (FROZEN_ENGRAM_V154_SHA256 if not engram_v154_bin else None)

self.arm_binaries: Dict[str, Optional[Path]] = {
    "control_A": None,
    "treatment_B": self.engram_bin,
    "baseline_arm1": None,
    "v151_arm2": self.engram_v151_bin,
    "v154_arm3": self.engram_v154_bin,
}
```

Where the default module constants are:
- `DEFAULT_ENGRAM_V154_BIN`: `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe`
- `FROZEN_ENGRAM_V154_SHA256`: `cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798`

### 1.2 Identified Specification Discrepancy in Milestone 2
- **Documented Intent in Milestone 2:** Evaluate the **Candidate Phase 1 binary** (`benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`, SHA-256 `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`).
- **Milestone 2 Command Specification:** Specified only `--arms v154_arm3` without setting explicit binary flags.
- **Consequence:** Invoking `orchestrator.py --arms v154_arm3` without flags causes the orchestrator to resolve `v154_arm3` to the **frozen v1.5.4 reference binary** (`bin/v1.5.4/engram.exe`), expecting hash `CF807EAC...`. If an operator expected it to run Candidate Phase 1, `verify_frozen_binary` would either fail with a checksum mismatch or mistakenly benchmark the frozen v1.5.4 binary.

### 1.3 Corrected Specification
To ensure the authorized executable, file path, cryptographic SHA-256, and execution arm all refer to the exact same artifact:

| Parameter | Corrected Value |
|:---|:---|
| **Target Artifact Name** | Candidate Phase 1 Engram Executable |
| **Exact Executable Path** | `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` |
| **Authoritative SHA-256** | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` |
| **Execution Arm Identifier** | `v154_arm3` (Candidate Arm under Alternative B protocol) |
| **Mandatory CLI Overrides** | `--engram-v154-bin "benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe"`<br>`--expected-v154-sha256 "F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE"` |

#### Corrected Invocation Command Template:
```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --task smoke_test_read_only `
  --arms v154_arm3 `
  --engram-v154-bin "benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe" `
  --expected-v154-sha256 "F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE" `
  --max-tokens-ceiling 10000 `
  --headroom-reserve 2000 `
  --model gpt-5.6-luna `
  --reasoning low `
  --dependency-strategy none `
  --output-dir "benchmarks/matched-pair-harness/reports/smoke_test"
```

---

## 2. Token Guard Qualification & Overrun Dynamics

### 2.1 Correction of Prior Guarantee Claims
Prior documentation in Milestone 2 asserted that the 2,000-token headroom reserve "prevents overrun beyond the 10,000-token ceiling." This assertion is **retracted**. No external polling mechanism can mathematically guarantee that total token consumption will not exceed the nominal ceiling.

### 2.2 Formal Documentation of Budget Architecture
- **Proposed Operational Smoke-Test Ceiling:** `10,000 tokens`
- **Proposed Pre-Dispatch Headroom Reserve:** `2,000 tokens`
- **Nominal Stop Threshold:** `8,000 tokens`

### 2.3 Limitations of External Polling & Process Termination
1. **Reaction Latency Window:**
   - Polling loop sleep interval: `time.sleep(0.05)` (0 to 50 ms).
   - Windows process tree recursive kill latency (`taskkill /F /T`): measured median **229.17 ms** (197.12 ms to 283.86 ms).
   - Derived parent reaction time: **~254 ms**.
2. **Whole-Request Overshoot Risk:**
   - The token guard operates **between turns** (pre-dispatch). It checks if $\text{remaining\_budget} \ge \text{reserve}$.
   - If cumulative usage after Turn 1 is 7,800 tokens, the remaining budget is $2,200\text{ tokens} > 2,000\text{ tokens}$. Turn 2 is dispatched.
   - If Turn 2 incurs 4,500 tokens (due to prompt context and model generation), the entire turn proceeds to completion on the model provider's API servers.
   - Upon Turn 2 completion, cumulative usage reaches $7,800 + 4,500 = \mathbf{12,300\text{ tokens}}$, exceeding the nominal 10,000-token ceiling by 2,300 tokens before the guard can halt further execution.
3. **Preventive Stop vs. Ceiling Breach:**
   - **Preventive Stop (`PRE_DISPATCH_HEADROOM_EXHAUSTED`):** Occurs before dispatch when remaining budget is less than the 2,000-token reserve (e.g. cumulative usage reaches 8,100 tokens; remaining budget 1,900 tokens $< 2,000$). The turn is not dispatched; execution halts cleanly within or near the nominal budget.
   - **Ceiling Breach (`TOKEN_CEILING_BREACHED`):** Occurs reactively when a completed turn pushes cumulative usage past 10,000 tokens. The harness kills the process tree immediately upon observing the breach on stdout.
4. **Qualification Statement:**
   > **The 2,000-token headroom reserve is a defensive heuristic estimate, not a mathematical guarantee. A smoke-test run MAY exceed the nominal 10,000-token ceiling if a single dispatched turn consumes more tokens than the remaining headroom.**

---

## 3. Source Workspace Specification

### 3.1 Identification of the `authLogout` Fixture
The smoke test queries:
> *"Locate and state the file path and exact action creator name for authLogout in the core auth module. Do not edit, format, or create any files."*

Forensic inspection of the golden repository establishes the exact location of the target symbol:
- **Repository:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`
- **Pinned Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` (`origin/master`)
- **Target File Path:** `projects/angular-ngrx-material-starter/src/app/core/auth/auth.actions.ts`
- **Target Definition (Line 14):** `export const authLogout = createAction('[Auth] Logout');`

### 3.2 Workspace Creation & Isolation Method
To guarantee that the dirty golden repository working tree and untracked `.engram/` directory are never accessed, exposed, or mutated:
1. **Creation Mechanism:** The harness provisions a dedicated sandbox via detached Git worktree:
   ```bash
   git -c core.longpaths=true worktree add --detach <sandbox_path> 1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2
   ```
2. **Isolation Guarantee:**
   - `git worktree add` extracts files directly from the local Git object store at commit `1c285608...`.
   - The 18 uncommitted modified files on `test-branch` are **never copied**.
   - The untracked `.engram/` directory (29 files) is **never copied**.
   - The initial sandbox is verified 100% clean via `git status --porcelain` immediately after creation.
3. **Dependency Strategy:**
   - `--dependency-strategy none` is specified. Because the smoke test is a read-only symbol lookup, Angular compilation and `node_modules` are not required. This eliminates junction or file copy overhead.
4. **Zero-Mutation Verification:**
   - Immediately prior to execution: Assert `git status --porcelain` in the sandbox is empty.
   - Immediately upon run completion: Assert `git status --porcelain` in the sandbox returns 0 lines.
   - Golden repository re-verification: Confirm `git rev-parse HEAD` remains `1c285608...` on `test-branch` and tracked/untracked diff statistics are identical.
   - Mandatory rollback: The temporary worktree is removed via `git worktree remove --force <sandbox_path>` and the directory discarded.

---

## 4. Model Compatibility & Entitlement Status

### 4.1 Offline Local Configuration Inspection
Offline inspection of `C:\Users\Amal P S\.codex\models_cache.json` confirms:
- **Model Slug:** `gpt-5.6-luna`
- **Display Name:** `GPT-5.6-Luna`
- **Description:** `"Older fast and efficient model."`
- **Default Reasoning Level:** `medium`
- **Supported Reasoning Levels:** `['low', 'medium', 'high', 'xhigh', 'max']`
- **Supported in API:** `True`
- **Context Window:** `272,000 tokens`

### 4.2 Account Entitlement Qualification
While `gpt-5.6-luna` is definitively present in the local Codex CLI model catalog:
- Whether the user's specific ChatGPT subscription account currently has active server-side access to this model cannot be determined offline without live API communication.
- **Qualification:** Active account entitlement for `gpt-5.6-luna` is classified as **`OWNER VERIFICATION REQUIRED`**.
- **Invariant:** No alternative model (e.g. `gpt-6-luna` or `gpt-5.6-terra`) may be substituted without explicit owner review and written approval.

---

## 5. Token Consumption Estimate Classification

The previously cited token estimate of **3,000 to 6,000 tokens** is formally classified:
- **Scientific Classification:** **UNVERIFIED PLANNING ESTIMATE (HEURISTIC CALCULATION).**
- **Basis:**
  - Turn 0 prompt injection + 4-tool lean manifest: ~1,500 to 2,000 tokens.
  - Candidate MCP tool context response (`engram_inspect_symbol`): ~1,000 to 2,500 tokens.
  - Final agent answer synthesis: ~100 to 300 tokens.
  - Total: ~3,000 to 6,000 tokens.
- **Qualification:** This estimate is **not a measured consumption figure and not a guaranteed operating range**. Actual token usage depends on model reasoning depth, exploration choices, and tool response lengths.

---

## 6. Document Cross-Reference & Modification Note

In strict observance of safety restrictions, the existing Milestone 2 documents ([`milestone-2-readiness-report.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-2/milestone-2-readiness-report.md), [`smoke-test-execution-checklist.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-2/smoke-test-execution-checklist.md), [`milestone-2-authorization-gates.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-2/milestone-2-authorization-gates.md), [`milestone-2-closure-summary.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-2/milestone-2-closure-summary.md)) have **not been edited**.

This document serves as the authoritative, ratified correction to the Milestone 2 specifications.
