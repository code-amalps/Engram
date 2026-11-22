# Verified Launch Command: N=1 Matched-Pair Pilot

**Milestone:** Engram N=1 Matched-Pair Pilot Final Preflight  
**Document ID:** `amendments/draft-05/n1-pilot-preflight/verified-launch-command.md`  
**Evaluation Date:** 2026-10-30  
**Target Trial:** Controlled Matched-Pair Pilot ($N=1$, `pilot_01`)  
**Status:** FULLY PARSED AND VERIFIED AGAINST `argparse`  

---

## 1. Executive Summary

This document presents the exact PowerShell invocation for launching the N=1 matched-pair pilot experiment. Every CLI parameter has been audited and empirically tested against `benchmarks/matched-pair-harness/src/orchestrator.py` (`main()`).

As mandated by experimental rules, any discrepancy between the prompt specification and the harness argument parser is documented transparently. The verified launch command binds the Candidate Phase 1 Engram executable via pre-execution SHA-256 validation while strictly conforming to the orchestrator's argument grammar.

---

## 2. Argument Parser Audit and Discrepancy Analysis

### 2.1 Parameter-by-Parameter Grammar Check
Each parameter required by the experiment specification was cross-referenced with `orchestrator.py`'s `argparse` definitions:

| Specification Parameter | Required Value | CLI Option in `orchestrator.py` | Parser Conformance |
|---|---|---|---|
| Execution Mode | `pair` | `--mode pair` | **Supported** |
| Task Identifier | `task_commit_cache_invalidation` | `--task task_commit_cache_invalidation` | **Supported** |
| Matched Pair ID | `pilot_01` | `--pair pilot_01` | **Supported** |
| Candidate Binary | `candidate-phase1/engram.exe` | `--engram-bin <path>` | **UNSUPPORTED / NOT DEFINED** |
| Expected Treatment SHA-256 | `F56347EF...` | `--expected-treatment-sha256 <hash>` | **Supported** |
| Model Identifier | `gpt-5.6-luna` | `--model gpt-5.6-luna` | **Supported** |
| Reasoning Effort | `medium` | `--reasoning medium` | **Supported** |
| Nominal Token Ceiling | `175000` | `--max-tokens-ceiling 175000` | **Supported** |
| Headroom Reserve | `45000` | `--headroom-reserve 45000` | **Supported** |
| Dependency Strategy | `none` | `--dependency-strategy none` | **Supported** |
| Output Directory | `reports/runs/pilot_01` | `--output-dir <path>` | **Supported** |

### 2.2 Empirical Proof of `--engram-bin` Failure
Invoking `orchestrator.py` with `--engram-bin` produces the following fatal error:
```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py --engram-bin benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe
```
**Output:**
```
usage: orchestrator.py [-h] [--mode {pair,triad,single}] [--task TASK]
                       [--pair PAIR] [--block BLOCK] [--arm-order ARM_ORDER]
                       [--model MODEL] [--reasoning REASONING] [--dry-run]
                       [--dependency-strategy {copy,junction,none}]
                       [--preserve-sandboxes] [--seed SEED]
                       [--expected-treatment-sha256 EXPECTED_TREATMENT_SHA256]
                       [--engram-v151-bin ENGRAM_V151_BIN]
                       [--engram-v154-bin ENGRAM_V154_BIN]
                       [--expected-v154-sha256 EXPECTED_V154_SHA256]
                       [--max-tokens-ceiling MAX_TOKENS_CEILING]
                       [--headroom-reserve HEADROOM_RESERVE]
                       [--golden-repo GOLDEN_REPO] [--output-dir OUTPUT_DIR]
                       [--no-stop-on-failure]
orchestrator.py: error: unrecognized arguments: --engram-bin benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe
```

### 2.3 Verified Candidate Binary Binding Mechanism
In `orchestrator.py` (`lines 135–185`), `Orchestrator.__init__` defines:
```python
DEFAULT_ENGRAM_BIN = Path(r"C:\Users\Amal P S\OneDrive\Projects\Engram\target\release\engram.exe")
```
- Both `target/release/engram.exe` and `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` have the exact same SHA-256 digest: `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`.
- By passing `--expected-treatment-sha256 F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`, the orchestrator verifies `target/release/engram.exe` immediately before executing `treatment_B`.
- This fulfills the specification requirement of verifying and executing the Candidate Phase 1 binary without triggering an `argparse` syntax error.

---

## 3. Verified Launch Command (PowerShell)

> [!CAUTION]
> **DO NOT EXECUTE.** This command is for authorization review only. Live inference must not proceed until explicit owner authorization is granted.

### 3.1 Single-Line Invocation
```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py --mode pair --task task_commit_cache_invalidation --pair pilot_01 --expected-treatment-sha256 "F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE" --model "gpt-5.6-luna" --reasoning "medium" --max-tokens-ceiling 175000 --headroom-reserve 45000 --dependency-strategy "none" --output-dir "benchmarks/matched-pair-harness/reports/runs/pilot_01"
```

### 3.2 Formatted Multi-Line Invocation
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

## 4. Execution Preconditions Summary

Before invoking this command upon owner authorization:
1. Current working directory must be: `C:\Users\Amal P S\OneDrive\Projects\Engram`.
2. Git status must remain clean on `feature/async-precommit-guard` at `c70845b4d5913457f1b176fb0370b80dd792711f`.
3. Output directory `benchmarks/matched-pair-harness/reports/runs/pilot_01` must remain uncreated prior to launch.
4. Codex CLI app-server must be accessible at `C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe`.
