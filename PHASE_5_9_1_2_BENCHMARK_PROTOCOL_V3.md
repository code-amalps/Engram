# Engram Benchmark Protocol Specification: Version 3.0.0 (Controlled Runtime Pentad)

**Status:** Ratified for Offline Verification & Pending Explicit Live Authorization  
**Protocol Version:** 3.0.0  
**Phase Integration:** Phase 5.9.1.2  
**Baseline Model:** `gpt-5.6-luna` (Temperature: 0.0, Top_P: 1.0, Max Completion: 4096)  
**Execution Engine:** `ControlledAgentRuntime` via `ControlledRunner` and `ProviderTransport`  

---

## 1. Protocol Architecture & Historical Partitioning

### 1.1 Why Protocol Version 3.0.0 Is Required
In Protocol v1.x and v2.x (Phases 5.3 through 5.8), the benchmark baseline arm (`control_A` / `baseline_arm1`) executed by spawning the external, closed-source `codex.exe` binary via an OS subprocess (`subprocess.Popen`). In that architecture:
- Codex injected proprietary, non-configurable system prompts and turn framing.
- Codex handled tool invocation through internal unmetered loops.
- Intermediate requests were not synchronously intercepted prior to wire dispatch.

Under **Protocol Version 3.0.0**, all five benchmark arms execute through **`ControlledAgentRuntime`**:
- A standardized, open Python agent loop manages message serialization, request sizing, tool schemas, and turn execution.
- Baseline `arm_a_native` runs identical agent-loop logic as the treatment arms, with only the Engram MCP toolset omitted.
- Because system prompts, turn sequencing, and message envelopes differ from the external `codex.exe` executable, **historical Codex-native token measurements cannot be merged directly or compared as apples-to-apples baseline figures against Protocol v3.0.0 measurements**.
- All historical datasets (Protocol v1.x / v2.x) remain permanently archived as isolated reference series.

---

## 2. Five-Arm Pentad Specification

The 5-arm matched pentad evaluates the sequential evolution of the Engram optimization engine:

| Arm Identifier | Formal Label | Candidate Identity | Physical Binary Path | SHA-256 Digest | Active Capabilities & Configuration |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `arm_a_native` | Native Baseline | `native` | None (In-Process Agent Loop) | `NONE` | Standard prompt, zero MCP injection, standard file tools only |
| `arm_b_cand3` | Candidate 3 | `cand3` | `benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe` | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | Static symbol indexing & context pruning (`ENGRAM_FULL_TOOLS`, flat routing) |
| `arm_c_cand42_always_on` | Candidate 4.2 Always-On | `cand4_2` | `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe` | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | UTF-8 safety, symbol compression, always-on context compilation (`routed_mode="FULL"`) |
| `arm_d_cand42_adaptive` | Candidate 4.2 Adaptive | `cand4_2` | `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe` | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | `TaskComplexityClassifier` routing (BYPASS, LEAN, or FULL based on task heuristics) |
| `arm_e_cand5_optimized` | Candidate 5 Optimized | `cand5` | `benchmarks/matched-pair-harness/bin/candidate-phase5/engram.exe` | `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` | Full 7-optimization Token-Efficiency suite (AST delta compression, recursive summarization, eager cache reclamation) |

---

## 3. Strict Pre-Dispatch Boundaries & Budget Governance

1. **Request Size Boundary (`RequestSizeGate`):**
   - Outbound requests (messages + tool schemas + parameters) are serialized and measured before wire dispatch.
   - If total serialized input tokens exceed **60,000 tokens**, the request is immediately dropped (`PRE_DISPATCH_REQUEST_SIZE_EXCEEDED`).
   - Zero-truncation policy: The request is never truncated or compressed on-the-fly; execution fails closed with exit code 105.

2. **Budget Reservation Coordinator (`RequestReservationCoordinator`):**
   - Every model request (Turn-0, intermediate turns, tool follow-ups, retries) must acquire an atomic pre-dispatch reservation:
     $$R = I + O \le B_{\text{remaining}} - B_{\text{reserve}}$$
     where $O = 4,096$ tokens and $B_{\text{reserve}} = 45,000$ tokens.
   - If capacity is insufficient, wire dispatch is blocked and execution halts immediately.

3. **Two-Task Aggregate Budget Conservation:**
   - Global Budget Ceiling: $B_G = 2,500,000$ tokens.
   - Task A Nominal Ceiling: $C_A = 1,000,000$ tokens (per-arm: 200,000 tokens).
   - Dynamic Task B Ceiling: $C_B = \min(1,500,000, B_G - T_A)$ (per-arm: $\lfloor C_B / 5 \rfloor$).
   - Sum Conservation: $T_A + C_B \le B_G = 2,500,000$ tokens.
   - Output allowances $O$ are deducted *before* dispatch, guaranteeing that output generation cannot cause an unbudgeted overrun.

4. **Streaming Response Tracking & Cancellation:**
   - Responses are streamed chunk-by-chunk through `StreamingTokenTracker`.
   - If locally received output tokens exceed the pre-reserved allowance $O$, the streaming connection is aborted immediately.
   - Server-side completion cap is permanently set to `max_completion_tokens: 4096`.

---

## 4. Execution Invariants & Fail-Closed Policies

- **Live Provider Guard:** Live benchmark execution strictly requires `provider in ('controlled', 'openai')`. Any attempt to dispatch live benchmarks with `provider == 'cloud'` triggers an immediate fail-closed `SafetyViolationError`.
- **Candidate Integrity Assertion:** The physical SHA-256 digest of every candidate binary is verified immediately before sandbox launch. Any hash discrepancy triggers a fail-closed abort.
- **Dependency Hygiene:** Live runs enforce `dependency_strategy="copy"` from verified golden baselines. Read-only tasks permit `dependency_strategy="none"`.
- **Stop-on-Failure:** Under `execution_policy="strict"`, any arm failure or correctness rejection halts the block immediately, preventing unmetered token consumption on doomed tasks.
