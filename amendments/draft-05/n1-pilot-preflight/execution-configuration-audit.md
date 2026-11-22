# Execution Configuration Audit: N=1 Matched-Pair Pilot

**Milestone:** Engram N=1 Matched-Pair Pilot Final Preflight  
**Document ID:** `amendments/draft-05/n1-pilot-preflight/execution-configuration-audit.md`  
**Evaluation Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Active Branch:** `feature/async-precommit-guard`  
**Baseline Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`  
**Status:** AUDITED & VERIFIED WITH DOCUMENTED CLI PARSER DISCREPANCY  

---

## 1. Executive Summary

This audit inspects the physical implementation of the benchmark orchestrator (`benchmarks/matched-pair-harness/src/orchestrator.py`), runner (`runner.py`), and isolation layer (`isolation.py`) against the immutable experimental specification for the first controlled token-efficiency pilot ($N=1$).

The audit confirms that the harness enforces strict execution symmetry, complete workspace isolation, pre-execution cryptographic binary validation, and genuine unassisted baseline execution. A technical discrepancy between the experimental command specification and the orchestrator's CLI argument parser was identified, empirically verified, and fully documented.

---

## 2. Experimental Mode & Arm Selection

In `orchestrator.py` (`lines 730–736`, `lines 394–420`):

```python
if args.mode == "single":
    mode = "single"
elif args.block or args.mode == "triad":
    mode = "triad"
else:
    mode = "pair"
```

When invoked with `--mode pair`, the orchestrator executes `run_pair()`, which establishes a two-arm trial:
1. `control_A`: Unassisted baseline arm.
2. `treatment_B`: Candidate Engram Phase 1 assisted arm.

### Arm Execution Ordering
The execution order between `control_A` and `treatment_B` is randomized using a pre-registered pseudo-random coin flip:
```python
rng = random.Random(self.seed)
coin = rng.choice([0, 1])
if coin == 0:
    order = [("control_A", 1), ("treatment_B", 2)]
else:
    order = [("treatment_B", 1), ("control_A", 2)]
```
If a fixed seed is passed (`--seed <int>`), the ordering is deterministic and reproducible; otherwise, system entropy determines the order.

---

## 3. Arm Symmetry and Isolation Controls

### 3.1 Control A (Genuinely Unassisted Baseline)
The execution path for `control_A` in `orchestrator.py` and `runner.py` enforces absolute isolation from Engram:
- **Binary Mapping:** `self.arm_binaries["control_A"] = None`. No Engram binary is loaded, validated, or executed.
- **MCP Tool Registration:** In `runner.py` (`lines 572–595`), `is_engram_arm` evaluates to `False` for `control_A`. Consequently, `-c mcp_servers.engram...` is completely omitted from the Codex command line.
- **Turn-0 Context Injection:** In `runner.py` (`lines 647–670`), Turn-0 instruction injection is bypassed for `control_A`. No `.github/copilot-instructions.md` file is created, and no prompt prefixes or templates are prepended.
- **Prompt Delivery:** The task prompt from `task_commit_cache_invalidation.json` is delivered verbatim to the Codex process without augmentation.
- **Tooling Parity:** Codex operates exclusively with its standard built-in file editing, terminal execution, and inspection tools.

### 3.2 Treatment B (Candidate Phase 1 Assisted)
The execution path for `treatment_B` configures Engram assistance:
- **Binary Mapping:** `self.arm_binaries["treatment_B"] = self.engram_bin`.
- **Preflight Cryptographic Verification:** Immediately before spawning the treatment arm sandbox, `_execute_single_arm` calls:
  ```python
  if is_engram:
      if not binary or not binary.exists():
          raise FileNotFoundError(f"Engram binary missing for arm '{arm}': {binary}")
      if expected_sha:
          verify_frozen_binary(binary, expected_sha)
  ```
  Execution will abort immediately with `SafetyViolationError` if the binary hash does not match `self.expected_treatment_sha256`.
- **MCP Server Registration:** In `runner.py`, `treatment_B` is registered with the isolated sandbox path:
  ```
  -c mcp_servers.engram.command="<engram_bin>"
  -c mcp_servers.engram.args=["-C", "<sandbox_path>", "mcp"]
  -c mcp_servers.engram.default_tools_approval_mode="approve"
  -c mcp_servers.engram.enabled_tools=["engram_get_context", "engram_inspect_symbol", "engram_preflight", "engram_postflight"]
  ```
- **Context Boundaries:** The treatment agent receives context only through explicit MCP tool queries directed to the Candidate Engram binary. No unauthorized external files or pre-seeded manual context packages are introduced.

### 3.3 Common Symmetry Controls Across Arms
Both arms operate under strictly matched parameters:
| Parameter | Control A | Treatment B | Parity Status |
|---|---|---|---|
| Target Repository | `C:\Users\Amal P S\OneDrive\Projects\Engram` | `C:\Users\Amal P S\OneDrive\Projects\Engram` | Identical |
| Baseline Commit | `c70845b4d5913457f1b176fb0370b80dd792711f` | `c70845b4d5913457f1b176fb0370b80dd792711f` | Identical |
| Task Configuration | `task_commit_cache_invalidation` | `task_commit_cache_invalidation` | Identical |
| Task Prompt | Verbatim task prompt | Verbatim task prompt | Identical |
| Model Identifier | `gpt-5.6-luna` | `gpt-5.6-luna` | Identical |
| Reasoning Effort | `medium` | `medium` | Identical |
| Nominal Ceiling | 175,000 tokens | 175,000 tokens | Identical |
| Headroom Reserve | 45,000 tokens | 45,000 tokens | Identical |
| Dependency Strategy | `none` (Cargo offline) | `none` (Cargo offline) | Identical |
| Acceptance Gates | L0, L2, L3 + REQ-1..4 | L0, L2, L3 + REQ-1..4 | Identical |
| Workspace Lifecycle | Fresh Git worktree sandbox | Fresh Git worktree sandbox | Independent |

---

## 4. Binary Mapping and CLI Argument Parser Discrepancy

### 4.1 Candidate Binary Hashes
Static inspection of the Candidate Phase 1 binary across physical locations yielded identical cryptographic SHA-256 digests:
- Location 1: `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`
  - SHA-256: `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`
- Location 2: `target/release/engram.exe`
  - SHA-256: `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`

Both physical executables are compiled from the Candidate Phase 1 source code (`feature/async-precommit-guard`) and are byte-for-byte identical.

### 4.2 CLI Argument Parser Discrepancy Analysis
The experiment specification dictates passing the candidate binary explicitly via:
`--engram-bin benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`

Inspection of `orchestrator.py` (`lines 700–728`) reveals the following argument definitions:
- `--expected-treatment-sha256` (supported)
- `--engram-v151-bin` (supported)
- `--engram-v154-bin` (supported)
- `--expected-v154-sha256` (supported)
- `--engram-bin` (**NOT DEFINED in `argparse`**)

### 4.3 Empirical Verification of Parser Discrepancy
Executing `orchestrator.py` with `--engram-bin` was empirically tested:
```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py --engram-bin benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe
```
**Result:**
- Exit code: `2`
- Stderr: `orchestrator.py: error: unrecognized arguments: --engram-bin benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`

### 4.4 Default Binary Resolution Path
In `Orchestrator.__init__` (`lines 135–185`), `engram_bin` defaults to:
```python
DEFAULT_ENGRAM_BIN = Path(r"C:\Users\Amal P S\OneDrive\Projects\Engram\target\release\engram.exe")
```
Furthermore, `Orchestrator.__init__` handles expected hashes as:
```python
self.expected_treatment_sha256 = expected_treatment_sha256 or FROZEN_ENGRAM_SHA256
```
Because `DEFAULT_ENGRAM_BIN` has SHA-256 `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`, omitting `--engram-bin` and passing:
```powershell
--expected-treatment-sha256 F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE
```
causes the orchestrator to automatically bind the Candidate Phase 1 binary from `target/release/engram.exe` and strictly verify its cryptographic digest before running `treatment_B`.

In accordance with strict experimental guidelines, this parser discrepancy is documented explicitly rather than being silently bypassed.

---

## 5. Verification Verdict

The execution configuration for the N=1 matched-pair pilot satisfies all structural symmetry, workspace independence, and binary verification requirements. The CLI argument parser accepts all necessary experimental flags with the sole qualification that `--engram-bin` must be omitted in favor of the orchestrator's verified default binary path.
