# Engram Phase 0–2 Measurement Pilot — Protocol Amendment 01

**Amendment ID:** AMENDMENT-20261018-01  
**Subject:** Model Substitution Due to Upstream Unavailability (`Claude 3.5 Sonnet` $\to$ `Claude Sonnet 5.5`)  
**Timestamp:** 2026-10-18T07:10:00Z  
**Status:** **RATIFIED PROTOCOL AMENDMENT**  
**Authorizing Authority:** User Explicit Approval  
**Base Preregistration Document:** [`PREREGISTRATION.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/PREREGISTRATION.md) (Revision 1.3.0, Sealed SHA-256: `db3ab2956a4da40aad8f1f376aa72a50bebc96f3e1ecc3120f405f0a294ea8bb`)  
**Sealed Candidate Binary:** `C:\Users\Amal P S\OneDrive\Projects\Engram\target\release\engram.exe` (SHA-256: `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0`)

---

## 1. Nature of Amendment & Protocol Deviation Notice

> [!CAUTION]
> **Formal Protocol Deviation Notice:**  
> This document records an intentional, ratified **protocol deviation** from Revision 1.3.0 of the sealed preregistration.  
> This substitution **does NOT preserve all aspects of the original experiment's validity**. Specifically, it breaks direct quantitative comparability with historical calibration runs, changes the model parameter scale and tokenization profile, and introduces an updated model capability baseline.

Under Section 2 of `PREREGISTRATION.md` (*Model Governance Rule*), the primary model preference (`Claude 3.5 Sonnet`) was ratified contingent on actual availability at session start, with a mandatory halt rule if unavailable:
> *"If `Claude 3.5 Sonnet` is unavailable, the runner must STOP and report the available model choices before selecting a replacement. The selected model must remain strictly identical across all runs of both Configuration A and Configuration B."*

The pre-launch audit of the local execution environment on 2026-10-18 confirmed that `Claude 3.5 Sonnet` is **unavailable / not present** in the active GitHub Copilot model registry. Execution was halted at Pre-Launch Gate 2 prior to any model calls or token consumption.

This amendment formally ratifies **`Claude Sonnet 5.5`** as the required replacement model for the 20-run pilot.

---

## 2. Empirical Grounding for Substitution

An exhaustive audit of the local VS Code runtime state (`C:\Users\Amal P S\AppData\Roaming\Code\User\globalStorage\state.vscdb`) revealed the following:
1. In `chat.modelsControl`, the upstream Copilot platform has deprecated / demoted older model families and registered newer revisions. Both `"free"` and `"paid"` tiers list `claude-sonnet-5.5` with label `"Claude Sonnet 5.5"` and `"featured": true`.
2. The legacy model identifier `claude-3.5-sonnet` does not exist in the active cached model registry (`chat.cachedLanguageModels.v2` or `chat.modelsControl`).
3. Attempting to force an unlisted model identifier via CLI/API would trigger silent fallback or runtime exception.

Therefore, selecting `Claude Sonnet 5.5` represents the closest contemporary architectural equivalent within the Anthropic Sonnet family available in the intended execution path.

---

## 3. Experimental Consequences & Validity Impact

### A. Impact on Historical Baseline Comparability (Broken)
- The exploratory calibration runs executed on 2026-10-17 (`run_001_A_T1_r1` through `run_020_B_T5_r5` in [`results.csv`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/results.csv)) recorded `model: claude-3.5-sonnet`.
- `Claude Sonnet 5.5` possesses a different tokenizer, different parameter capacity, altered reasoning trajectories, and updated native tool-use characteristics.
- **Consequence:** Absolute token consumption numbers, auxiliary orchestration turns, and elapsed durations from this pilot **cannot be pooled with or directly compared against** the October 17 calibration figures.
- The 20 planned runs under this pilot constitute a **new, independent empirical dataset**.

### B. Impact on Internal Treatment Validity (Preserved Conditionally)
- The primary research hypothesis ($H_1$ vs. $H_0$ and $H_2$) evaluates the differential effect of Engram context and scope enforcement relative to native Copilot exploration under identical conditions.
- Because the experimental design employs **block-randomized matched pairs** where every Treatment B run is paired with a contemporaneous Control A run on the same task, internal relative validity is preserved **if and only if** `Claude Sonnet 5.5` is held strictly constant across all 20 runs.

---

## 4. Invariant Model Parity Rule

To ensure experimental integrity under this amendment:
1. **Mandatory Model Locking:**  
   Every single planned run—all 10 Control (A) runs and all 10 Treatment (B) runs—must execute strictly on **`Claude Sonnet 5.5`**.
2. **Visual UI Verification:**  
   Because headless CLI commands do not support deterministic model flags, the operator must visually verify that **"Claude Sonnet 5.5"** is selected in the VS Code Copilot Chat model dropdown prior to submitting any task prompt.
3. **Zero Mid-Stream Model Changes:**  
   Under no circumstances may model selection be toggled, updated, or alternated between runs or between arms of a matched pair.

---

## 5. Unchanged Experimental Invariants

All other dimensions of the sealed baseline freeze and preregistration remain completely unaltered:

| Protocol Parameter | Sealed Status | Invariant Confirmation |
|---|---|---|
| **Candidate Binary** | `target\release\engram.exe` | Exact SHA-256 `474a1a7...` preserved bit-for-bit. No rebuild. |
| **Sample Size** | Fixed 5 Matched Pairs / Task | Exactly 20 runs (10 Control, 10 Treatment). No runs added based on variance. |
| **Run Order & Seed** | Seed `20261018` | Locked 20-run schedule in [`EXECUTION_CHECKLIST.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/EXECUTION_CHECKLIST.md) remains unchanged. |
| **Golden Repository** | Commit `1c2856086...` | Remains untouched and read-only. |
| **Workspace Isolation** | `%TEMP%\engram_pilot\run_{id}` | Every run executes in a freshly extracted disposable sandbox. |
| **Task A Prompt & Gates** | Verbatim Single-File Typo | `verify_task_a.ps1` exit 0 + `npx tsc` app exit 0. |
| **Task B Prompt & Gates** | Verbatim Remember-Me | `verify_task_b.ps1`, security negative check, dual `tsc` gates, browser reload marked `UNVERIFIED AT RUNTIME`. |
| **Blinded Diff Review** | Anonymized Post-Pilot Diff | Human reviewer evaluates stripped diffs without arm labels. |
| **Telemetry Accounting** | Strict Metric Separation | Measured auxiliary tokens, estimated context tokens, and billing credits recorded in distinct fields. |
| **Replacement Rule** | Infrastructure Abort Only | Max 2 replacements per arm; failed runs never discarded. |

---

## 6. Pre-Execution Attestation

Prior to ratifying this amendment:
- Zero model API calls were initiated.
- Zero Copilot Agent Mode sessions were launched.
- Zero tokens or account billing credits were consumed.
- The original sealed artifacts ([`PREREGISTRATION.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/PREREGISTRATION.md), [`BUILD_MANIFEST.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/BUILD_MANIFEST.md), [`FREEZE_SEAL.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/FREEZE_SEAL.md)) remain untouched and un-resealed.
- The candidate release binary remains untouched.
