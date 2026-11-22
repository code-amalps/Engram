# Engram Milestone 5G — Owner Authorization Checklist
## Formal Authorization Protocol for Matched-Pair Live Pilot

**Date:** 2026-11-04  
**Milestone:** 5G — Live Pilot Readiness Package  
**Status:** DRAFT / PENDING EXPLICIT OWNER AUTHORIZATION  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Git Branch:** `feature/async-precommit-guard`  
**Git HEAD:** `c70845b4d5913457f1b176fb0370b80dd792711f`  

---

## 1. Explicit Authorization Boundary Notice

> [!IMPORTANT]
> **PRIOR AUTHORIZATION IS FULLY CONSUMED AND VOID.**  
> The single live experiment authorization granted by the repository owner for `pilot_01` expired upon the conclusion of that run.  
> **This Milestone (5G) does NOT authorize live model execution.** No agent inference, OpenAI API call, network request, or token consumption may occur without a new, explicit, unambiguous authorization directive from the repository owner.

---

## 2. The 10-Point Live Authorization Checklist

Before the repository owner issues authorization for a live pilot, every point below must be verified as satisfied:

| # | Inspection Item | Required Specification | Status / Verification Method | Verified |
| :-: | :--- | :--- | :--- | :---: |
| **1** | **Task ID & Scope** | `task_commit_cache_invalidation` | Bounded Cargo/Rust AST edits to `src/compiler.rs`, `src/analyzer.rs`, `src/main.rs`, and `tests/commit_cache_invalidation_test.rs`; fully supported by `evaluator.py`. | [X] READY |
| **2** | **Candidate Binary** | `candidate-phase1/engram.exe` | Verified SHA-256: `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` (at `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` and `target/release/engram.exe`). | [X] READY |
| **3** | **Experimental Design** | 2-Arm Matched Pair | Arm B (Treatment) runs first; Arm A (Control) runs second conditionally upon Treatment functional acceptance ($C_B = 1.0$). | [X] READY |
| **4** | **Model & Runtime Target** | `gpt-5-codex` via `codex exec` | Runtime alias `gpt-5.6-luna`. Marked `NOT VERIFIABLE` offline; live exposure observed at Checkpoint 1. | [X] READY (QUALIFIED) |
| **5** | **Token Ceiling & Guard** | 175k Hard / 160k Pre-Dispatch | Cumulative ceiling `--max-tokens-ceiling 175000`; headroom reserve `--headroom-reserve 15000` (derived threshold: 160,000). | [X] READY |
| **6** | **Operational Timeouts** | 600s / 25 Turns Max | Strict wall-clock cutoff (10 min) and 25-turn loop ceiling enforced by runner. | [X] READY |
| **7** | **Strict Arm Cap** | Exactly 2 Arms ($N=1$ Pair) | Zero retries, zero unapproved arms, zero triads, zero reruns permitted under this authorization. | [X] READY |
| **8** | **Conditional Execution** | Auto-skip Control on Treatment Failure | If Arm B fails ($C_B < 1.0$) or halts, Arm A is skipped to conserve user quota. | [X] READY |
| **9** | **Estimated Quota Spend** | 60,000–120,000 tokens per arm | Absolute worst-case ceiling 350,000 tokens (if both arms run to ceiling). Expected total spend: < 180,000 tokens. | [X] READY |
| **10** | **Evidence Vault & Isolation** | `results/pilot_02/` & Isolated Worktrees | Detached Git worktrees provisioned from `c70845b4...` in `%TEMP%\engram_pilot\runs\`. Telemetry and `SHA256SUMS` retained in vault. | [X] READY |

---

## 3. Required Owner Authorization Directive Template

To authorize execution of the live pilot, the repository owner must provide a statement matching or equivalent to the following:

```markdown
# OWNER AUTHORIZATION FOR ENGRAM N=1 LIVE PILOT

I, the repository owner, hereby authorize execution of exactly ONE matched-pair live experiment:
- Task: task_commit_cache_invalidation
- Candidate Binary SHA-256: F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE
- Model: gpt-5-codex (or gpt-5.6-luna)
- Maximum Tokens Ceiling: 175,000 provider tokens per arm (--max-tokens-ceiling 175000)
- Headroom Reserve: 15,000 tokens (--headroom-reserve 15000)
- Design: Sequential 2-arm (Treatment first; Control conditionally executed upon Treatment acceptance)
- Max Arms: 2 arms maximum

Execution authorization applies strictly to this single pilot run. No subsequent experiments are authorized.
```

---

## 4. Verified Launch Command Specification

The following exact PowerShell command conforms to `orchestrator.py` CLI argument grammar:

```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --mode pair `
  --task task_commit_cache_invalidation `
  --pair pilot_02 `
  --expected-treatment-sha256 "F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE" `
  --model "gpt-5.6-luna" `
  --reasoning "medium" `
  --max-tokens-ceiling 175000 `
  --headroom-reserve 15000 `
  --dependency-strategy "none" `
  --output-dir "benchmarks/matched-pair-harness/results/pilot_02"
```

### Explanatory Notes on Command Flags:
1. **Candidate Binary Binding:** In `orchestrator.py`, the CLI parser does not define a separate `--treatment-bin` flag; it binds `DEFAULT_ENGRAM_BIN` (`target/release/engram.exe`) and validates its digest against `--expected-treatment-sha256`. Both `target/release/engram.exe` and `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` share the identical verified SHA-256 (`F56347EF...`).
2. **Headroom Reserve:** Passing `--headroom-reserve 15000` overrides the orchestrator's default `45_000` reserve, establishing the verified 160,000 pre-dispatch threshold against the 175,000 ceiling ($175\text{k} - 15\text{k} = 160\text{k}$).
3. **Dependency Strategy:** `--dependency-strategy "none"` is used because `task_commit_cache_invalidation` is a self-contained Cargo repository task with dependencies handled offline via local Rust tooling (`requires_dependencies: false`).

*(Note: The above command is documented for pre-authorization verification only. Do not execute without explicit authorization.)*
