# Verified N=1 Launch Command: Post-Remediation Specification

**Milestone:** Engram Amendment 05 — Milestone 5C  
**Document ID:** `amendments/draft-05/milestone-5c/verified-n1-launch-command.md`  
**Evaluation Date:** 2026-10-30  
**Target Trial:** Controlled Matched-Pair Experiment ($N=1$, `pilot_01`)  
**Status:** FULLY PARSED, BOUND TO CANDIDATE HASH & READY FOR OWNER REVIEW  

---

## 1. Executive Summary

This document presents the reconstructed, parser-verified launch command for the $N=1$ matched-pair pilot following the completion of the evaluator remediation. Every CLI argument conforms strictly to `benchmarks/matched-pair-harness/src/orchestrator.py` (`main()`).

The command executes `control_A` and `treatment_B` against the remediated evaluator for `task_commit_cache_invalidation`, binds the Candidate Phase 1 binary via pre-execution SHA-256 validation, and enforces isolated sandbox execution.

---

## 2. Command Parameters and Parser Mapping

| Parameter | Specified Value | CLI Option in `orchestrator.py` | Parser Verification |
|---|---|---|---|
| Execution Mode | `pair` | `--mode pair` | Verified |
| Task Identifier | `task_commit_cache_invalidation` | `--task task_commit_cache_invalidation` | Verified |
| Matched Pair ID | `pilot_01` | `--pair pilot_01` | Verified |
| Model Identifier | `gpt-5.6-luna` | `--model "gpt-5.6-luna"` | Verified |
| Reasoning Effort | `medium` | `--reasoning "medium"` | Verified |
| Nominal Ceiling | `175000` tokens | `--max-tokens-ceiling 175000` | Verified |
| Headroom Reserve | `45000` tokens | `--headroom-reserve 45000` | Verified |
| Dependency Strategy | `none` | `--dependency-strategy "none"` | Verified |
| Output Directory | `reports/runs/pilot_01` | `--output-dir "benchmarks/matched-pair-harness/reports/runs/pilot_01"` | Verified |
| Expected Treatment Hash | `F56347EF...` | `--expected-treatment-sha256 "F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE"` | Verified |

### Confirmation of Selected Executable Hash
`DEFAULT_ENGRAM_BIN` in `orchestrator.py` resolves to:
`C:\Users\Amal P S\OneDrive\Projects\Engram\target\release\engram.exe`
- Physical File SHA-256: `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`
- Treatment Arm Hash: Matches expected hash exactly; pre-execution check succeeds.

---

## 3. Verified PowerShell Launch Invocations

> [!CAUTION]
> **DO NOT EXECUTE.** Live inference has not been authorized by the owner. This command is prepared solely for owner review and authorization.

### 3.1 Single-Line Command
```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py --mode pair --task task_commit_cache_invalidation --pair pilot_01 --expected-treatment-sha256 "F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE" --model "gpt-5.6-luna" --reasoning "medium" --max-tokens-ceiling 175000 --headroom-reserve 45000 --dependency-strategy "none" --output-dir "benchmarks/matched-pair-harness/reports/runs/pilot_01"
```

### 3.2 Formatted Multi-Line Command
```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --mode pair `
  --task task_commit_cache_invalidation `
  --pair pilot_01 `
  --expected-treatment-sha256 "F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE" `
  --model "gpt-5.6-luna" `
  --reasoning "medium" `
  --max-tokens-ceiling 175000 `
  --headroom-reserve 45000 `
  --dependency-strategy "none" `
  --output-dir "benchmarks/matched-pair-harness/reports/runs/pilot_01"
```

---

## 4. Operational Invariants for Execution

When authorized by the owner, execution will:
1. Initialize the pair summary structure and randomize arm order (`control_A` vs `treatment_B`).
2. Verify golden dependency manifest before sandbox setup.
3. Validate candidate binary SHA-256 immediately before running `treatment_B`.
4. Provision ephemeral Git worktrees under `%TEMP%\engram_pilot\runs`.
5. Enforce the pre-dispatch guard (aborting at $\le 45,000$ tokens remaining headroom).
6. Evaluate functional correctness using the remediated `Evaluator` across all four requirements.
7. Compute comparative savings only if both arms achieve $C = 1.00$ with valid telemetry.
8. Safely tear down sandboxes and write run evidence to `benchmarks/matched-pair-harness/reports/runs/pilot_01`.
