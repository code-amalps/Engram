# Engram Phase 5.4.22: Tetrad Order Validation Audit

**Execution Mode:** STRICTLY OFFLINE, READ-ONLY FORENSIC AUDIT  
**Project:** Engram Benchmark Harness  
**Branch:** `feature/phase2-stage5-remediation`  
**Git Baseline HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Authorization Status:** `NOT_AUTHORIZED`  
**Phase Verdict:** `MANIFEST_BLOCKED`

---

## 1. Executive Summary

This audit examined the command-line interface (CLI) parsing, argument validation, and dispatch preparation pathways for matched-pair tetrad execution in `benchmarks/matched-pair-harness/src/orchestrator.py`.

### Critical Finding (Finding `ENGRAM-5422-01`):
**The benchmark harness currently lacks pre-dispatch validation for `--arm-order` in tetrad mode.**

Specifically:
1. In `main()` (`orchestrator.py` lines 2558–2615), input validation is only enforced when `--mode single`. When `--mode tetrad`, the raw parsed `arm_order` list is passed directly to `orchestrator.run_tetrad(block_id=args.block, arm_order=arm_order)` without checking arm count, uniqueness, or registered identity membership.
2. In `run_tetrad()` (`orchestrator.py` lines 2065–2071), `order = arm_order` is assigned directly without upfront validation.
3. As a result:
   - **Omitted arms (e.g. 3 arms passed):** The orchestrator dispatches and executes all 3 arms against live tasks, and only after all 3 finish does it mark the omitted arm as `DEFERRED` (lines 2220–2226).
   - **Duplicate arms (e.g. `A,A,C,D`):** Arm A executes twice sequentially, overwriting its run summary, and omitted Arm B is marked `DEFERRED`.
   - **Unknown arm in late position (e.g. `A,D,C,X`):** Arms A, D, and C are dispatched and executed first. Only when the loop reaches `arm_x` does `_execute_single_arm()` raise `SafetyViolationError`.

Because **malformed order input can dispatch a partial or incorrect tetrad**, the Phase 5.4.22 audit verdict is **`MANIFEST_BLOCKED`**.

---

## 2. Required Tetrad Invariant

For any valid four-arm comparative benchmark block, the execution sequence must satisfy:
1. Exact cardinality: $|\text{order}| = 4$
2. Strict uniqueness: $|\operatorname{unique}(\text{order})| = 4$
3. Exact set equivalence: $\operatorname{set}(\text{order}) = \operatorname{set}(\text{all\_tetrad\_arms})$

Where `all_tetrad_arms` consists strictly of:
- `arm_a_native`
- `arm_b_cand3_always_on`
- `arm_c_cand4_always_on`
- `arm_d_cand4_adaptive`

---

## 3. Input Matrix Evaluation (T01 – T15)

The 15 test inputs were evaluated against the current CLI parsing and execution path. A mocked dispatch boundary was used to verify dispatch behavior without initiating live model calls.

| Test ID | Raw Input | Description | Meets Invariant? | Pre-Dispatch Safety | Prior Arms Dispatched Before Error | Current Behavior / Defect |
|:---:|---|---|:---:|:---:|:---:|---|
| **T01** | `arm_a_native,arm_d_cand4_adaptive,arm_c_cand4_always_on,arm_b_cand3_always_on` | Valid permutation (A,D,C,B) | **YES** | **PASS** | 0 | Correctly executed valid permutation. |
| **T02** | `arm_b_cand3_always_on,arm_d_cand4_adaptive,arm_a_native,arm_c_cand4_always_on` | Valid permutation (B,D,A,C) | **YES** | **PASS** | 0 | Correctly executed valid permutation. |
| **T03** | `arm_a_native,arm_a_native,arm_c_cand4_always_on,arm_d_cand4_adaptive` | Duplicate arm (A,A,C,D) | **NO** | **UNSAFE LEAKAGE** | 0 | Dispatches A twice, executes C and D, marks B as DEFERRED. |
| **T04** | `arm_a_native,arm_d_cand4_adaptive,arm_c_cand4_always_on` | Missing arm (A,D,C) | **NO** | **UNSAFE LEAKAGE** | 0 | Dispatches and executes 3 arms, marks B as DEFERRED. |
| **T05** | `arm_a_native,arm_d_cand4_adaptive,arm_c_cand4_always_on,arm_b_cand3_always_on,arm_x_unknown` | Unknown arm at position 5 | **NO** | **UNSAFE LEAKAGE** | **4 arms dispatched** | Executes A, D, C, B first; raises SafetyViolationError on arm 5. |
| **T06** | `""` (Empty string) | Empty order | **NO** | **DEFAULT FALLBACK** | 0 | Falls back to deterministic seed-shuffled default registered arms. |
| **T07** | `arm_a_native arm_d_cand4_adaptive...` | Space-separated string | **NO** | **FAIL-CLOSED (INCIDENTAL)** | 0 | Parsed as single unknown string; raises SafetyViolationError before arm 1. |
| **T08** | ` arm_a_native , arm_d_cand4_adaptive...` | Whitespace around commas | **YES** | **PASS** | 0 | Stripped by `[a.strip() for a in ...]`; valid. |
| **T09** | `A,D,C,B` | Short aliases | **NO** | **FAIL-CLOSED (INCIDENTAL)** | 0 | Arm 'A' is unregistered; raises SafetyViolationError before arm 1. |
| **T10** | `arm_a_native,arm_b_cand3_always_on,arm_c_cand4_always_on,arm_d_cand4_adaptive` | Canonical order | **YES** | **PASS** | 0 | Correctly executed. |
| **T11** | `arm_a_native,arm_b_cand3_always_on,arm_c_cand4_always_on,arm_a_native` | Same arm repeated | **NO** | **UNSAFE LEAKAGE** | 0 | Executes 4 arms (A twice), overwrites A run, marks D as DEFERRED. |
| **T12** | `arm_a_native,arm_d_cand4_adaptive,arm_c_cand4_always_on,unknown_arm` | Unknown arm in position 4 | **NO** | **UNSAFE LEAKAGE** | **3 arms dispatched** | Dispatches and runs A, D, C; raises SafetyViolationError on arm 4. |
| **T13** | `arm_a_native,arm_d_cand4_adaptive,arm_c_cand4_always_on,invalid!suffix` | Malformed suffix | **NO** | **UNSAFE LEAKAGE** | **3 arms dispatched** | Dispatches and runs A, D, C; raises SafetyViolationError on arm 4. |
| **T14** | `arm_a_native,arm_d_cand4_adaptive,arm_c_cand4_always_on,arm_a_native` | Duplicate in position 4 | **NO** | **UNSAFE LEAKAGE** | 0 | Runs A, D, C, A; marks B as DEFERRED. |
| **T15** | `ARM_A_NATIVE,ARM_D_CAND4_ADAPTIVE...` | Uppercase names | **NO** | **FAIL-CLOSED (INCIDENTAL)** | 0 | Case mismatch raises SafetyViolationError before arm 1. |

---

## 4. Side-Effect Verification & Failure Analysis

When an input such as `T04` (`A,D,C`) or `T12` (`A,D,C,unknown`) is provided:
1. `orchestrator.run_tetrad` begins executing the order loop at line 2094.
2. For each valid arm:
   - Golden repository files are checked.
   - Isolated sandbox directory (`output_dir / arm`) is provisioned on the filesystem.
   - Dependencies are copied / linked.
   - Codex runner child process is launched.
   - LLM model queries are dispatched over the network.
   - Telemetry and correctness records are written to disk.
3. For `T04`, all 3 arms complete live execution. At line 2220, the loop ends, and the omitted Arm B is populated with a synthetic `DEFERRED` record.
4. For `T12`, Arms A, D, and C execute to completion. When arm 4 is evaluated, `arm_identity = self.arm_identities.get(arm)` returns `None`, raising `SafetyViolationError`.

**Conclusion:** Malformed inputs currently create concrete filesystem modifications, launch child processes, and consume LLM tokens before failure occurs.

---

## 5. Valid Order Verification (`A → D → C → B`)

For the approved execution order `arm_a_native,arm_d_cand4_adaptive,arm_c_cand4_always_on,arm_b_cand3_always_on`:
- All four registered arm keys are present.
- Each key maps strictly to its correct candidate identity:
  - `arm_a_native`: Native mode, no MCP, binary `None`.
  - `arm_d_cand4_adaptive`: Candidate 4.2 (`78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2`), adaptive MCP mode.
  - `arm_c_cand4_always_on`: Candidate 4.2 (`78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2`), always-on MCP mode.
  - `arm_b_cand3_always_on`: Candidate 3 (`97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777`), always-on MCP mode.
- Relative position: Candidate 3 (Arm B) is executed last, preventing token exhaustion from aborting downstream arms.

---

## 6. Minimal Future Remediation Specification

To achieve full fail-closed pre-dispatch safety, the following minimal, non-invasive remediation must be implemented in a future authorized implementation phase:

### 1. In `orchestrator.py::main()` (lines 2558–2576):
```python
    if mode == "tetrad" and arm_order is not None:
        allowed_tetrad = {
            "arm_a_native", "arm_b_cand3_always_on",
            "arm_c_cand4_always_on", "arm_d_cand4_adaptive"
        }
        if len(arm_order) != 4 or len(set(arm_order)) != 4 or set(arm_order) != allowed_tetrad:
            print(
                f"Error: Tetrad mode requires exactly the 4 registered arms {sorted(allowed_tetrad)} "
                f"with no duplicates, omissions, or unregistered identifiers. Received: {arm_order}"
            )
            sys.exit(1)
```

### 2. In `orchestrator.py::run_tetrad()` (lines 2065–2072):
```python
        if arm_order:
            if len(arm_order) != 4 or len(set(arm_order)) != 4 or set(arm_order) != set(all_tetrad_arms):
                raise SafetyViolationError(
                    f"FATAL: Malformed tetrad arm_order: {arm_order}. "
                    f"Expected exact permutation of {all_tetrad_arms}."
                )
            order = arm_order
```

This guarantees that any malformed tetrad order is rejected with zero side effects before sandbox creation, process launching, or network dispatch.
