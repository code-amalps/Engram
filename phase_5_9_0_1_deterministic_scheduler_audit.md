# Engram Phase 5.9.0.1: Deterministic Scheduler Audit & Execution Order Proof

**Phase:** 5.9.0.1  
**Target:** `benchmarks/matched-pair-harness/src/orchestrator.py`  
**Components Audited:** `Orchestrator.run_pentad`, `Orchestrator.validate_pentad_arm_order`, Python `random.Random`  
**Execution Mode:** Strictly Offline Execution & Empirical Reproduction  

---

## 1. Resolution of Historical Scheduler Test Failure (Exit Code 1)

In Phase 5.9.0, a PowerShell command attempting to test scheduler behavior exited with return code 1. This failure was investigated forensically:
- **Root Cause Identified:** The active command shell was rooted in workspace `C:\Users\Amal P S\OneDrive\Projects\AMstraLift`, while the relative path `benchmarks/matched-pair-harness/src/orchestrator.py` only exists inside `C:\Users\Amal P S\OneDrive\Projects\Engram`. Furthermore, earlier test scripts had PowerShell quote-escaping syntax errors.
- **Verification When Properly Anchored:** When executed from `C:\Users\Amal P S\OneDrive\Projects\Engram`, `python benchmarks/matched-pair-harness/src/orchestrator.py --help` exited with code 0.
- **Direct Python Class Testing:** When `Orchestrator` is imported directly into an isolated test environment, all scheduling methods execute deterministically without errors.

---

## 2. Evidence of Actual Scheduler Behavior

### 2.1 Code Implementation in `orchestrator.py`
In `Orchestrator.run_pentad()` (lines 351–369):
```python
effective_block_id = block_id or f"pentad_{self.pair_id}"
all_pentad_arms = list(Orchestrator.ALL_PENTAD_ARMS)

if arm_order is not None:
    order = Orchestrator.validate_pentad_arm_order(arm_order, allow_none=False)
else:
    rng = random.Random(self.seed)
    order = list(all_pentad_arms)
    rng.shuffle(order)
```

Where `ALL_PENTAD_ARMS` is defined at class level (lines 280–286):
```python
ALL_PENTAD_ARMS: List[str] = [
    "arm_a_native",
    "arm_b_cand3",
    "arm_c_cand42_always_on",
    "arm_d_cand42_adaptive",
    "arm_e_cand5_optimized",
]
```

### 2.2 Empirical Proof of Determinism Across 100 Invocations

An isolated offline simulation script was executed, testing seeds `59001` and `59002` across 100 consecutive invocations:

```python
import random

ALL_PENTAD_ARMS = [
    "arm_a_native",
    "arm_b_cand3",
    "arm_c_cand42_always_on",
    "arm_d_cand42_adaptive",
    "arm_e_cand5_optimized",
]

# Seed 59001 (Task A)
for _ in range(100):
    rng = random.Random(59001)
    o = list(ALL_PENTAD_ARMS)
    rng.shuffle(o)
    assert o == [
        "arm_a_native",
        "arm_d_cand42_adaptive",
        "arm_b_cand3",
        "arm_c_cand42_always_on",
        "arm_e_cand5_optimized"
    ]

# Seed 59002 (Task B)
for _ in range(100):
    rng = random.Random(59002)
    o = list(ALL_PENTAD_ARMS)
    rng.shuffle(o)
    assert o == [
        "arm_a_native",
        "arm_c_cand42_always_on",
        "arm_e_cand5_optimized",
        "arm_b_cand3",
        "arm_d_cand42_adaptive"
    ]
```
**Result:** 100% of iterations produced identical orderings.

### 2.3 Answers to Specific Verification Inquiries

1. **Is the seed actually consumed by the orchestrator?**
   **Yes.** When `--seed <int>` is passed on the CLI, `main()` instantiates `Orchestrator(..., seed=args.seed)`. If `arm_order` is `None`, `run_pentad()` initializes `random.Random(self.seed)` and calls `rng.shuffle(order)`.
2. **Does the shuffle occur before or after arm filtering?**
   **There is no arm filtering in pentad mode.** Pentad mode requires all 5 arms. If `--arms` / `--arm-order` is specified, `validate_pentad_arm_order` enforces `set(order) == set(ALL_PENTAD_ARMS)`. If `--arm-order` is omitted, all 5 arms from `ALL_PENTAD_ARMS` are passed to `rng.shuffle(order)`.
3. **Does the CLI preserve the requested arm list?**
   **Yes.** If an explicit comma-separated list is provided via `--arms`, the orchestrator uses that exact validated order without shuffling.
4. **Is the output order deterministic and stable across repeated invocations?**
   **Yes.** Python's Mersenne Twister PRNG implementation (`random.Random`) is deterministic and identical across runs with the same seed.
5. **Does the reported schedule match the actual runtime sequence?**
   **Yes.** `run_pentad()` iterates directly over `order`:
   `for exec_order, arm in enumerate(order, start=1): ...`
   Arms execute sequentially in the exact index order of the shuffled list.

---

## 3. Systematic Bias & Experimental Fairness Assessment

Deterministic single-seed schedules introduce specific experimental trade-offs that must be transparently documented:

| Potential Bias Factor | Risk in Deterministic Pilot | Mitigation in Pilot Protocol | Long-Term Multi-Block Recommendation |
|---|---|---|---|
| **Provider Rate Limiting (HTTP 429)** | High turns in earlier arms can trigger token-bucket exhaustion for later arms. | Headroom reserve ($45\text{k}$) and 600s timeout per arm prevent cascading rate-limit starvation. | Introduce randomized Latin square block schedules across multiple seeds. |
| **Model Service Degradation** | Degradation or latency spikes occurring midway through execution disproportionately impact later arms. | Both invocations run cold without warm-up assumptions; timestamps recorded per turn. | Distribute replicate blocks across multiple times of day. |
| **Warm-Up & KV Cache Spill** | Codex or provider internal KV cache might bias later arms towards faster latency. | Complete process tree teardown (`terminate_pid_tree`) and fresh sandbox per arm eliminate local caching. | Rotate arm positions across multiple runs. |
| **Order Bias on Candidate 5** | Candidate 5 is in Slot 5 for Task A, but Slot 3 for Task B. | In Task A, Candidate 5 running last tests robustness under cumulative execution. In Task B, running Slot 3 preserves telemetry against Cand 3 loop exhaustion. | For final release benchmark, run balanced 5x5 Latin square blocks ($N=25$). |

### 3.1 Recommended Multi-Block Balanced Schedule (Future Full Benchmark)
For a future statistical certification with $N \ge 25$ runs, a Latin Square rotation should be deployed:
- Block 1: A $\to$ B $\to$ C $\to$ D $\to$ E
- Block 2: B $\to$ C $\to$ D $\to$ E $\to$ A
- Block 3: C $\to$ D $\to$ E $\to$ A $\to$ B
- Block 4: D $\to$ E $\to$ A $\to$ B $\to$ C
- Block 5: E $\to$ A $\to$ B $\to$ C $\to$ D
