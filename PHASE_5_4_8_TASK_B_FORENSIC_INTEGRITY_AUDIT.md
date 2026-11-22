# Engram Phase 5.4.8: Task B Forensic Integrity Audit
## Comprehensive Forensic Re-examination of Live Benchmark `task_b_remember_me_tetrad_live_01`

**Document Version:** 1.0.0-FINAL  
**Audit Date:** November 19, 2026  
**Auditor:** Antigravity Autonomous Systems Engineer / Antigravity AI  
**Benchmark Target:** `benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_tetrad_live_01/`  
**Core Repository:** `Engram` (Branch: `feature/phase2-stage5-remediation`, Commit: `7106a2f369fe79faf5f95449378a900ab8e8eddf`)  
**Golden Repository:** `angular-ngrx-material-starter` (Commit: `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`)  
**Audit Mode:** Strictly Read-Only Forensic Verification (Zero source changes, zero binary rebuilds, zero live LLM dispatches, zero commits/pushes).

---

## 1. Executive Conclusion

A rigorous, evidence-based forensic audit of the four-arm benchmark run `task_b_remember_me_tetrad_live_01` reveals a critical execution discrepancy alongside key empirical insights:

1. **Candidate 4.2 Was Never Executed:** Due to a PowerShell environment variable assignment error at launch (`=1 : The term '=1' is not recognized...`), the harness orchestrator failed to detect `ENGRAM_USE_CANDIDATE_4_2=1`. Consequently, Arms C and D executed the legacy **Candidate 4** binary (`FC605BC0...`), rather than the sealed, UTF-8-safe, envelope-bounded **Candidate 4.2** (`78B84880...`).
2. **MCP Boundary Behavior Was Incomplete in Candidate 4:** Because Candidate 4 was executed, universal response-boundary enforcement was absent on `engram_get_context`. While Arm C avoided this endpoint and bounded `engram_inspect_symbol` (max 14,491 bytes), Arm D called `engram_get_context`, receiving an uncompacted 89,617-byte (154,608 formatted bytes) response that inflated auxiliary payload to 90,161 bytes.
3. **The 76.5% Payload Reduction Claim Is Misattributed:** The arithmetic $(134,361 - 31,621) / 134,361 = 76.465\%$ holds strictly for Arm C vs Arm B, but this reduction was driven by the model's tool-selection choices in Arm C (calling only `inspect_symbol`), not universal engine bounding. In Arm D, the payload was 90,161 bytes (only a 32.9% reduction).
4. **Arm C Implementation Is Incomplete and Breaks Typecheck:** While Arm C modified the 6 requested feature files (+33 / -29 lines), it introduced a breaking signature change to `authLogin` requiring payload props. This caused `npx tsc -p tsconfig.spec.json` to fail with error `TS2554: Expected 1 arguments, but got 0` in `auth.actions.spec.ts` and `auth.reducer.spec.ts`. The arm was halted before the model could resolve these compilation errors. Functional correctness remains $C = 0.00$.
5. **Adaptive Routing (Arm D) Failed to Demonstrate Benefit:** Arm D classified Task B as `COMPLEX` $\rightarrow$ `FULL`, emitted 90,161 bytes of MCP responses, made zero task code modifications, and halted at Turn 7.
6. **Token Budget Inadequacy Confirmed:** All 4 arms halted prematurely between turns 7 and 9 due to the headroom guard. Context sizes grew to 48,000–58,000 tokens per turn. Under a 45,000-token headroom reserve, an effective budget of 305,000 tokens allows only 6 to 8 turns—insufficient for full-stack feature modification, typecheck verification, and test remediation.
7. **Budget Calibration:** The proposed 500,000-token ceiling provides only 9 to 10 turns, which remains dangerously tight. A minimum ceiling of 600,000 tokens is required for complex Angular tasks requiring compile/test triage cycles.

---

## 2. Actual Binary Identity by Arm

Inspection of the raw `run_telemetry.json` and `tetrad_summary.json` files establishes the actual binary provenance and configuration under which each arm executed:

| Arm Identifier | Arm Name | Recorded Executable Path | Actual Executable SHA-256 | MCP Status | Headroom Status |
| :--- | :--- | :--- | :--- | :---: | :---: |
| **Arm A** | `arm_a_native` | `None` | `NONE` | `DISABLED` | Halted Turn 8 |
| **Arm B** | `arm_b_cand3_always_on` | `.../bin/candidate-phase3/engram.exe` | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | `ACTIVE` | Halted Turn 9 |
| **Arm C** | `arm_c_cand4_always_on` | `.../bin/candidate-phase4/engram.exe` | `FC605BC0CF3B6C9C5FDB1E7C83EE666FD7E049FD9AE25C893D478578681699F8` | `ACTIVE` | Halted Turn 8 |
| **Arm D** | `arm_d_cand4_adaptive` | `.../bin/candidate-phase4/engram.exe` | `FC605BC0CF3B6C9C5FDB1E7C83EE666FD7E049FD9AE25C893D478578681699F8` | `ACTIVE` (`FULL`) | Halted Turn 7 |

### Root Cause of Binary Substitution:
In `benchmarks/matched-pair-harness/src/orchestrator.py` (lines 144–154):
```python
use_cand4_2 = os.environ.get("ENGRAM_USE_CANDIDATE_4_2", "").lower() in ("1", "true", "yes")
use_cand4_1 = os.environ.get("ENGRAM_USE_CANDIDATE_4_1", "").lower() in ("1", "true", "yes")
if use_cand4_2 and cand4_2_bin.exists():
    active_cand4_bin = cand4_2_bin
    active_cand4_sha = cand4_2_sha
elif use_cand4_1 and cand4_1_bin.exists():
    active_cand4_bin = cand4_1_bin
    active_cand4_sha = cand4_1_sha
else:
    active_cand4_bin = cand4_bin
    active_cand4_sha = cand4_sha
```
During benchmark dispatch, the launch command attempted to assign the environment variable using:
`$env:ENGRAM_USE_CANDIDATE_4_2=1; python ...`
In PowerShell, assigning without spaces or quotes failed with:
`=1 : The term '=1' is not recognized as the name of a cmdlet, function, script file...`
As a result, `$env:ENGRAM_USE_CANDIDATE_4_2` remained unset (`None`), falling through line 153 to `active_cand4_bin = cand4_bin`. Arms C and D were dispatched with Candidate 4 (`FC605BC0...`).

---

## 3. Candidate 4 versus Candidate 4.2 Comparison

The differences between the executed Candidate 4 binary and the intended Candidate 4.2 binary are material to understanding the trial's outcome:

| Feature / Capability | Candidate 4 (`FC605BC0...`) [Actually Executed] | Candidate 4.2 (`78B84880...`) [Intended] |
| :--- | :--- | :--- |
| **`engram_inspect_symbol` Sizing** | Bounded (max 16,384 B internal payload) | Bounded (max 16,384 B envelope-aware) |
| **`engram_get_context` Sizing** | **UNBOUNDED** (emits raw AST and full omission manifest; 89–108 KB) | **BOUNDED** ($\le 16,384$ B via `enforce_mcp_envelope_boundary`) |
| **Envelope-Aware Calculation** | No (measures internal payload; risks envelope overflow) | Yes (`compute_envelope_bytes` includes JSON-RPC frame) |
| **Omission Manifest Handling** | Emits all 127+ unselected files (~70 KB raw JSON) | Compacted summary, categorized counts, $\le 2\text{--}4$ sampled files |
| **UTF-8 Slicing Safety** | **UNSAFE** (`&str[..1500]` crashes on multibyte chars like `•` or emojis) | **SAFE** (zero-allocation character boundary slicing via `safe_truncate_str`) |
| **Universal Endpoint Coverage** | Only `engram_inspect_symbol` | Universal (`get_context`, `inspect_symbol`, `preflight`, `postflight`) |
| **Degradation Tiers** | Single tier | 8 Progressive shedding tiers (Level 0 through Level 7) |

---

## 4. Patch and Implementation Audit

### 4.1 Summary of Changes Across Arms

| Arm | Files in Patch | Task Files Modified | Insertions | Deletions | Assessment |
| :--- | :---: | :---: | :---: | :---: | :--- |
| **Arm A (Native)** | 3 | 3 | +7 | -3 | Partial core/auth modification; no UI or effects. |
| **Arm B (Cand 3)** | 1 | 0 | +0 | -19 | Zero task modifications; 1 artifact deletion. |
| **Arm C (Cand 4)** | 7 | 6 | +33 | -29 | 6 target files modified; 1 artifact deletion; typecheck fails. |
| **Arm D (Cand 4 Adapt)** | 1 | 0 | +0 | -19 | Zero task modifications; 1 artifact deletion. |

### 4.2 The `+0 / -19` Discrepancy & `stock-market` Theme File Deletion
Inspection of `changes.patch` in Arms B, C, and D reveals an identical 19-line deletion of:
`projects/angular-ngrx-material-starter/src/app/features/examples/stock-market/components/stock-market-container.component.scss-theme.scss`

**Forensic Explanation:**
1. Event logs confirm that none of the models in Arm B, C, or D ever issued commands touching this file.
2. In `evaluator.py`, `files_changed_count` is derived from `git status --porcelain`, while insertions and deletions are derived from `git diff HEAD --numstat`.
3. The file path length in the temporary sandbox exceeds 247 characters (`C:\Users\Amal P S\AppData\Local\Temp\engram_pilot\runs\task_b_remember_me_tetrad_live_01_arm_b_cand3_always_on_20261119_193837\...`).
4. While `git worktree add` ran with `-c core.longpaths=true`, `evaluator.py` executed `git diff HEAD` without this config, causing git to treat the unreadable long path as deleted in working-tree comparisons.
5. In Arm D, `git status --porcelain` showed no modified files (`files_changed_count: 0`), but `git diff HEAD --numstat` returned `0 19`, producing the `+0 / -19` report artifact.

### 4.3 Arm C Feature Implementation Inspection

Arm C modified 6 intended files:
1. `app.component.html`: Added `<input class="login-username">` and `<input type="checkbox" [(ngModel)]="rememberMe">`.
2. `app.component.ts`: Added `username`, `rememberMe` fields; loaded from `storageService.getItem('AUTH')`; dispatched `authLogin` with credentials.
3. `auth.actions.ts`: Changed `authLogin` action to accept `props<{ username: string; rememberMe: boolean }>()`.
4. `auth.effects.ts`: Stored `username` on login tap if `rememberMe`; preserved `username` on logout tap.
5. `auth.effects.spec.ts`: Updated unit test mocks for the new login/logout signatures.
6. `en.json`: Added i18n key `"anms.menu.remember-me": "Remember me"`.

**Critical Flaw Discovered:**
In `auth.actions.ts`, changing `authLogin` to require `{ username, rememberMe }` broke existing test suites:
- `auth.actions.spec.ts(5,20): error TS2554: Expected 1 arguments, but got 0.`
- `auth.reducer.spec.ts(18,20): error TS2554: Expected 1 arguments, but got 0.`

In turn 7 (item_16), the model ran `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.spec.json --noEmit`, which failed with exit code 1. Before the model could update these two test files, the headroom guard halted execution at Turn 8.

---

## 5. Recalculated Telemetry

Independent parsing and recalculation of `run_telemetry.json` and `events.jsonl` yields the following exact ledger:

| Telemetry Metric | Arm A (Native) | Arm B (Cand 3) | Arm C (Cand 4) | Arm D (Cand 4 Adapt) |
| :--- | :---: | :---: | :---: | :---: |
| **Total Model Tokens** | **310,207** | **338,142** | **307,753** | **293,067** |
| **Prompt Tokens (Total)** | 304,847 | 336,413 | 304,530 | 289,233 |
| Prompt Tokens (Cached) | 266,880 | 289,408 | 265,600 | 241,152 |
| Prompt Tokens (Uncached) | 37,967 | 47,005 | 38,930 | 48,081 |
| **Completion Tokens** | 5,360 | 1,729 | 3,223 | 3,834 |
| Reasoning Tokens | 1,119 | 317 | 416 | 562 |
| **Total Turns** | 8 | 9 | 8 | 7 |
| **Peak Context (Single Turn)** | 47,894 | 52,027 | 48,803 | 58,218 |
| **MCP Calls Count** | 0 | 3 | 4 | 4 |
| **Auxiliary Payload Bytes** | 0 B | 134,361 B | 31,621 B | 90,161 B |
| **Max Individual MCP Response** | 0 B | 282,852 B | 14,491 B | 154,608 B |
| **Remaining Budget at Stop** | 39,793 | 11,858 | 42,247 | 56,933 |
| **Estimated Next Turn Cost** | 47,894 | 52,027 | 48,803 | 58,218 |

### Identity Verification:
The fundamental token identity $\text{Prompt Tokens} + \text{Completion Tokens} = \text{Total Model Tokens}$ holds exactly across all four arms:
- **Arm A:** $304,847 + 5,360 = 310,207$ ($\Delta = 0$)
- **Arm B:** $336,413 + 1,729 = 338,142$ ($\Delta = 0$)
- **Arm C:** $304,530 + 3,223 = 307,753$ ($\Delta = 0$)
- **Arm D:** $289,233 + 3,834 = 293,067$ ($\Delta = 0$)

---

## 6. Headroom Guard Verification

The pre-dispatch headroom guard halts execution when:
$$\text{Remaining Budget} < \max(\text{Estimated Next Turn Cost}, \text{Headroom Reserve})$$
Where:
- $\text{Configured Ceiling} = 350,000$ tokens
- $\text{Headroom Reserve} = 45,000$ tokens
- $\text{Remaining Budget} = \text{Configured Ceiling} - \text{Cumulative Tokens at Stop}$
- $\text{Estimated Next Turn Cost} = \text{Peak Turn Context}$

| Arm | Cumulative Tokens | Remaining Budget | Est Next Turn Cost | Threshold $\max(\text{Est}, 45\text{k})$ | Condition Met? | Actual Decision |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **Arm A** | 310,207 | 39,793 | 47,894 | 47,894 | $39,793 < 47,894$ (**True**) | PREVENTIVE_HALT |
| **Arm B** | 338,142 | 11,858 | 52,027 | 52,027 | $11,858 < 52,027$ (**True**) | PREVENTIVE_HALT |
| **Arm C** | 307,753 | 42,247 | 48,803 | 48,803 | $42,247 < 48,803$ (**True**) | PREVENTIVE_HALT |
| **Arm D** | 293,067 | 56,933 | 58,218 | 58,218 | $56,933 < 58,218$ (**True**) | PREVENTIVE_HALT |

The guard triggered deterministically and accurately in 100% of cases, preventing runaway token expenditure.

---

## 7. Evidence Classification Table

| # | Statement / Claim | Evidence Classification | Empirical Basis |
| :---: | :--- | :---: | :--- |
| **1** | **Candidate 4.2 was executed in Arms C & D** | **CONTRADICTED** | SHA-256 in `run_telemetry.json` is `FC605BC0...` (Candidate 4), not `78B84880...` (Candidate 4.2). |
| **2** | **Candidate 4 demonstrated bounded MCP responses** | **PARTIALLY VERIFIED** | Bounded for `inspect_symbol` in Arm C (max 14,491 B), but unbounded for `get_context` in Arm D (154,608 B formatted). |
| **3** | **76.5% auxiliary payload reduction achieved** | **PARTIALLY VERIFIED** | Verified arithmetically for Arm C vs Arm B ($31,621$ B vs $134,361$ B), but misattributed to engine bounding rather than LLM tool selection. Arm D was 90,161 B. |
| **4** | **Arm C produced a complete 6-file implementation** | **PARTIALLY VERIFIED** | 6 files were modified, but breaking changes in `authLogin` left 2 test files broken at halt. |
| **5** | **Arm C implementation is demonstrably correct** | **CONTRADICTED** | Failed `tsconfig.spec.json` with TS2554. Correctness score is $0.00$ (`L0_typecheck: BLOCKED`). |
| **6** | **Adaptive routing demonstrated an advantage** | **CONTRADICTED** | Arm D routed to `FULL`, modified zero task files, emitted 90 KB of payloads, and halted at Turn 7. |
| **7** | **350,000 budget is demonstrably insufficient** | **VERIFIED** | All 4 arms halted between turns 7 and 9. At ~50k tokens/turn, 305k effective budget allows only 6–8 turns. |
| **8** | **Proposed 500,000 ceiling is sufficient** | **PARTIALLY VERIFIED** | 500k provides ~9–10 turns. Arm C was halted at Turn 8 with broken specs; 11–13 turns are required for full repair and gate passes. |

---

## 8. Corrected Conclusions

1. **Phase 5.4.7 Was an Unintentional Candidate 4 Test:** Because Candidate 4 was run, Phase 5.4.7 provides empirical data on Candidate 4's partial boundary implementation under live LLM load, but **zero empirical data** on Candidate 4.2's universal envelope boundary or UTF-8 safe truncation under live LLM load.
2. **Context Growth Dominates the Budget:** With prompt caching active, cumulative tokens reach ~300k by turn 7–8 simply because each turn carries 45k–58k tokens of conversation history. The model runs out of budget before it can enter the compile-test-repair loop.
3. **Correctness Requires Verification Turns:** A benchmark cannot measure functional superiority if the model is killed immediately upon writing code, before running tests. Task B requires at least 4 verification turns after coding.

---

## 9. Remaining Unknowns

1. **True Behavior of Candidate 4.2 in Live Matched Pair:** How Candidate 4.2 performs when `engram_get_context` is compacted to $\le 16,384$ bytes during a live LLM session remains empirically unmeasured.
2. **Arm C Full Fix Scope:** Does Arm C's implementation pass all Angular unit tests and production build once `auth.actions.spec.ts` and `auth.reducer.spec.ts` are supplied with `{ username: 'test', rememberMe: true }`?
3. **Adaptive Classifier Escalation on Task B:** Under what prompt structure would the adaptive classifier route Task B to `METADATA_ONLY` or `PRUNED` instead of immediately escalating to `FULL`?

---

## 10. Recommended Next Engineering Action

1. **Harness Environment Hardening:**
   - In `benchmarks/matched-pair-harness/src/orchestrator.py`, replace environment-variable toggles with an explicit CLI flag:
     `--candidate-version {cand3,cand4,cand4_1,cand4_2}`
   - Remove fragile reliance on shell-specific environment variable syntax.
2. **Longpaths Configuration in Evaluator:**
   - Update `evaluator.py` line 260 and 286 to include `-c core.longpaths=true` on all `git diff` and `git status` invocations, preventing false-positive deletions of deeply nested theme files.
3. **Operational Budget Re-calibration:**
   - Increase Task B operational ceiling to **600,000 tokens** (effective headroom 555,000 tokens / ~11–12 turns) to allow proper compile-test triage.
4. **Execution Authorization:**
   - Await explicit user authorization before launching any subsequent live benchmark trial (`task_b_remember_me_tetrad_live_02`).
