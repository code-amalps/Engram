# Configuration Change Feasibility and Frozen Boundary Audit

**Document Identifier:** `ENGRAM_DRAFT05_CONFIG_CHANGE_FEASIBILITY`  
**Parent Document:** [`amendments/draft-05/amendment-05.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/amendment-05.md)  
**Harness Source Files Audited:**  
- [`benchmarks/matched-pair-harness/src/orchestrator.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py)  
- [`benchmarks/matched-pair-harness/src/runner.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py)  

---

## 1. Executive Summary

This feasibility audit establishes how the Amendment 05 parameters (175,000 token ceiling and 15,000 headroom reserve) can be executed within the Engram matched-pair harness without violating any frozen boundary or modifying protected artifacts.

### Key Conclusions:
1. **Zero-Code-Change Execution Feasible:**  
   The harness CLI fully exposes `--max-tokens-ceiling` and `--headroom-reserve`. Running Amendment 05 can be performed with **zero modifications to any tracked file**.
2. **Impact on Frozen Artifacts: Exactly Zero:**  
   No frozen binary (`v1.5.1`, `v1.5.4`, or `candidate-phase1`), no Rust source code, and no golden repository file needs to be modified, recompiled, or touched.
3. **Permanent Defaults Location:**  
   If permanent default updates are authorized in the future, only two Python files require updates (`orchestrator.py` and `runner.py`).

---

## 2. Dynamic CLI Override Capability (No File Edits Required)

The orchestrator CLI parses ceiling and reserve flags at runtime:
```bash
python benchmarks/matched-pair-harness/src/orchestrator.py \
    --block block_07 \
    --max-tokens-ceiling 175000 \
    --headroom-reserve 15000
```

### Code Verification:
In [`benchmarks/matched-pair-harness/src/orchestrator.py` lines 1318–1321](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L1318-L1321):
```python
1318:     parser.add_argument("--max-tokens-ceiling", type=int, default=250_000, help="Per-arm token ceiling limit (default: 250,000)")
1319:     parser.add_argument("--headroom-reserve", type=int, default=15_000, help="Pre-dispatch token headroom reserve in tokens (default: 15,000)")
```
And in `orchestrator.py` lines 1341–1355:
```python
orchestrator = TriadOrchestrator(
    ...
    max_tokens_ceiling=args.max_tokens_ceiling,
    headroom_reserve=args.headroom_reserve,
)
```
The parsed integers are passed directly to `TriadOrchestrator` and forwarded to `StreamingCodexRunner(max_tokens_ceiling=..., headroom_reserve=...)`.
Therefore, **no file modification is necessary** to test or execute an Amendment 05 pilot.

---

## 3. Location of Permanent Code Defaults

If the project owner decides to commit Amendment 05 defaults to the repository rather than relying on CLI overrides, the exact lines to modify are:

| File | Line Numbers | Current Value (Amendment 04) | Proposed Value (Amendment 05) |
| :--- | :--- | :--- | :--- |
| [`orchestrator.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L85-L86) | lines 85–86 | `max_tokens_ceiling: int = 250_000,`<br>`headroom_reserve: int = 15_000,` | `max_tokens_ceiling: int = 175_000,`<br>`headroom_reserve: int = 15_000,` |
| [`orchestrator.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/orchestrator.py#L1318-L1319) | lines 1318–1319 | `default=250_000` | `default=175_000` |
| [`runner.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L235-L236) | lines 235–236 | `max_tokens_ceiling: int = 250_000,`<br>`headroom_reserve: int = 15_000` | `max_tokens_ceiling: int = 175_000,`<br>`headroom_reserve: int = 15_000` |

---

## 4. Preservation & Frozen Boundary Verification

1. **Frozen Binaries Intact:**  
   The binary executables in `benchmarks/matched-pair-harness/bin/` (`v1.5.1/engram.exe`, `v1.5.4/engram.exe`, `candidate-phase1/engram.exe`) contain only the MCP server and retrieval engines. They do not contain token ceiling or runner logic. Their SHA-256 hashes are unaffected by any ceiling or harness configuration change.
2. **Golden Repository Untouched:**  
   The golden repository (`C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`) at baseline commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` and its 18 modified files are completely isolated from harness ceiling parameters.
3. **Historical Evidence Protected:**  
   Block 06, Block 07, and Block 08 evidence artifacts in `benchmarks/` and `amendments/` remain strictly immutable and read-only.
