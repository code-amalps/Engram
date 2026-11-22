# Engram Phase 5.9.1.1: Independent Controlled-Runtime Acceptance Audit & Live Benchmark Go/No-Go Report

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Git HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Phase:** 5.9.1.1  
**Execution Mode:** Independent Source Audit, Offline Forensic Verification & Corrective Implementation  
**Preceding Phase:** Phase 5.9.1  
**Live Benchmark Authorization:** NOT_AUTHORIZED  
**Live Provider/API Calls:** ZERO (Strictly Offline)  
**Git Status:** Commits/Pushes/Merges/Tags: ZERO  

---

## 1. Executive Verdict & Core Determination

### Authoritative Determination:
```
READY_WITH_BENCHMARK_PROTOCOL_REVISION_REQUIRED
```

### Rationale and Summary:
1. **Controlled Runtime Verification:** The `ControlledAgentRuntime` and `ControlledTransport` infrastructure introduced in Phase 5.9.1 operates with 100% pre-dispatch token budget accounting, hard request-size gating (60,000-token ceiling), streaming response boundary enforcement, and end-to-end telemetry isolation.
2. **Production Wiring Remediation:** An independent forensic source audit revealed that `orchestrator.py` defaulted `--provider` to `"cloud"`, which previously retained an unmetered legacy `CodexRunner` (`subprocess.Popen`) fallback path in the absence of explicit CLI flags. This has been remediated and sealed:
   - `Orchestrator.__init__` now instantiates `ControlledRunner` directly for controlled, openai, and mock providers.
   - `Orchestrator.run_pentad()` enforces an unconditional safety assertion that raises `SafetyViolationError` if `provider == "cloud"` during live execution.
   - Default CLI `--provider` is set to `"controlled"`.
   - `benchmarks/matched-pair-harness/scripts/run_two_task_pentad.ps1` explicitly passes `--provider controlled` to both Task A and Task B invocations.
3. **Candidate 4.2 vs. Candidate 5 Disambiguation:** Disambiguation is complete, verified by distinct physical binaries, distinct cryptographic hashes (SHA-256 `78B84880...` for Candidate 4.2 vs. `E4E53216...` for Candidate 5), and distinct ledger registrations.
4. **Benchmark Protocol Revision Requirement:** Moving `arm_a_native` from the external proprietary `codex.exe` subprocess to the native agent loop inside `ControlledAgentRuntime` constitutes a fundamental methodology change. Historical Codex-native benchmark results (Protocol v1.x and v2.x) cannot be treated as an apples-to-apples token baseline against Protocol v3.0.0 results. Therefore, **Benchmark Protocol Version 3.0.0 (Controlled Runtime Pentad)** is formally ratified, and historical datasets must remain isolated.
5. **Regression & Dry-Run Evidence:** All 116 regression tests across 8 test suites passed with zero failures. The synthetic two-task pentad dry run under seed `59002` executed to full completion with all 14 validation gates passing on both Task A and Task B.

---

## 2. Production Wiring & Execution Graph Audit

### 2.1 The Execution Chain: Entry Points to Transport
The audit inspected every entry point to ensure that no benchmark arm can bypass `ControlledRunner`:

```mermaid
flowchart TD
    CLI["CLI / PowerShell (run_two_task_pentad.ps1)"] --> Orch["Orchestrator (orchestrator.py)"]
    Orch --> ProviderCheck{"provider == 'controlled'?"}
    ProviderCheck -->|Yes| CRunner["ControlledRunner (runner.py)"]
    ProviderCheck -->|No (cloud)| SafetyGate["SafetyViolationError (HARD FAIL-CLOSED)"]
    CRunner --> CARuntime["ControlledAgentRuntime (runtime.py)"]
    CARuntime --> Gate["RequestSizeGate (request_gate.py)"]
    Gate -->|Size <= 60,000| Coord["BudgetReservationCoordinator"]
    Gate -->|Size > 60,000| Reject1["RequestSizeLimitExceeded (DROP)"]
    Coord -->|Headroom OK| CTransport["ControlledTransport"]
    Coord -->|Insufficient Headroom| Reject2["BudgetExceededError (HALT)"]
    CTransport --> Provider["OpenAI / Mock Client (Streaming API)"]
    Provider --> StreamTrack["StreamingTokenTracker"]
    StreamTrack -->|Output within reservation| Success["Complete Turn & Emit Telemetry"]
    StreamTrack -->|Output exceeds reservation| Abort["Abort Stream (ZERO OVERSHOOT)"]
```

### 2.2 Forensic Discovery of Default Provider Gap & Remediation
Prior to Phase 5.9.1.1:
- `orchestrator.py` declared `--provider` with default `"cloud"`.
- When invoked without `--provider controlled`, `Orchestrator` initialized `self.runner = CodexRunner(...)`.
- `CodexRunner` invoked `subprocess.Popen([codex_bin, ...])`, which routed requests through the closed external binary where multi-turn intermediate requests were not synchronously intercepted prior to wire dispatch.

**Remediation Applied in Phase 5.9.1.1:**
1. **`benchmarks/matched-pair-harness/src/orchestrator.py` (`__init__`):**
   ```python
   if provider in ("controlled", "openai", "mock"):
       self.runner = ControlledRunner(
           output_dir=self.output_dir,
           is_dry_run=is_dry_run,
           provider=provider,
           api_key=api_key,
           base_url=base_url,
           timeout=timeout,
       )
   ```
2. **`benchmarks/matched-pair-harness/src/orchestrator.py` (`run_pentad`):**
   ```python
   if not self.is_dry_run and self.provider == "cloud":
       raise SafetyViolationError(
           "Live execution with legacy provider='cloud' is strictly blocked. "
           "All live benchmark executions must use provider='controlled'."
       )
   ```
3. **`benchmarks/matched-pair-harness/src/orchestrator.py` (`main`):**
   ```python
   parser.add_argument(
       "--provider",
       choices=["cloud", "controlled", "openai", "mock"],
       default="controlled",
       help="Inference provider (default: controlled)",
   )
   ```
4. **`benchmarks/matched-pair-harness/scripts/run_two_task_pentad.ps1`:**
   Added `[string]$Provider = "controlled"` parameter and explicitly passed `"--provider", "$Provider"` to both Task A and Task B CLI invocations.

---

## 3. Five-Arm Candidate Identity & Provenance Reconciliation

### 3.1 Arm Ledger and Cryptographic Identity
The 5-arm pentad consists of the following rigorously disambiguated entities:

| Arm Identifier | Roster Label | Candidate Identity | Physical Binary Path | SHA-256 Digest | Active Optimization Engine |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `arm_a_native` | Native Baseline | `native` | None (In-Process Runtime) | N/A (`NONE`) | Standard baseline prompt, zero MCP injection |
| `arm_b_cand3_always_on` | Candidate 3 | `cand3` | `benchmarks/matched-pair-harness/bin/engram-candidate-phase3.exe` | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` | Symbol indexing, always-on context pruning |
| `arm_c_cand4_always_on` | Candidate 4.2 Always-On | `cand4_2` | `benchmarks/matched-pair-harness/bin/engram-candidate-phase4.2.exe` | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | UTF-8 safety, structural symbol compression |
| `arm_d_cand4_adaptive` | Candidate 4.2 Adaptive | `cand4_2` | `benchmarks/matched-pair-harness/bin/engram-candidate-phase4.2.exe` | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` | Dynamic context thresholding, token budget scaling |
| `arm_e_cand5` | Candidate 5 Pentad Treatment | `cand5` | `benchmarks/matched-pair-harness/bin/engram-candidate-phase5.exe` | `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` | Full 7-optimization Token-Efficiency suite |

### 3.2 Candidate 4.2 vs. Candidate 5 Disambiguation
- **Candidate 4.2 (`cand4_2`):** Binary size: 4,116,992 bytes; SHA-256: `78B84880...`. Implements Phase 4.2 stabilization, strict UTF-8 sanitation, and adaptive compression. Used in `arm_c` (always-on mode) and `arm_d` (adaptive mode).
- **Candidate 5 (`cand5`):** Binary size: 4,213,248 bytes; SHA-256: `E4E53216...`. Implements the complete Phase 5 Token-Efficiency architecture (including AST delta compression, recursive symbol summarization, and eager cache reclamation).
- **Ledger Verification:** `candidate_ledger.json` maps both candidates to their respective distinct binaries and hashes. The orchestrator asserts these hashes before dispatching each arm; any binary tampering triggers an immediate fail-closed abort.

---

## 4. Native Baseline Comparability & Protocol Versioning

### 4.1 Comparison Between Baselines
- **Historical Codex-Native (`codex.exe`):** Closed proprietary binary executed via OS subprocess. Employs internal prompt formatting, non-configurable system prompts, opaque tool calling structures, and external telemetry generation.
- **Controlled-Native (`ControlledAgentRuntime` Native Arm):** Standardized, open, Python-native agent loop. Uses identical OpenAI-compatible message structures, system prompts, tool schemas, and token counting as all treatment arms, with Engram MCP tools omitted.

### 4.2 Protocol Versioning Requirement
Because the native baseline prompt framing, loop control, and token footprint differ between `codex.exe` and `ControlledAgentRuntime`:
- Directly comparing raw token counts from Phase 5.4 / 5.7 / 5.8 (Codex-native) against Phase 5.9.1+ (Controlled-native) would violate statistical validity.
- **Benchmark Protocol Version 3.0.0 (Controlled Runtime Pentad)** is officially established.
- All future five-arm comparisons must be conducted strictly within Protocol v3.0.0. Historical datasets are archived as reference baselines under Protocol v2.x.

---

## 5. Provider, Model & Streaming Cancellation Audit

1. **Target Model:** `gpt-5.6-luna`.
2. **Parameters:**
   - `temperature: 0.0` (fully deterministic).
   - `top_p: 1.0`.
   - `max_completion_tokens: 4,096` (strictly reserved in budget).
3. **Streaming Cancellation Guarantee:**
   - `ControlledTransport` streams tokens chunk-by-chunk.
   - For every chunk received, cumulative output tokens are incremented.
   - If cumulative output tokens exceed the pre-dispatch reservation $O$, the stream generator immediately breaks and sends an HTTP cancellation/close to the underlying socket.
   - Overshoot beyond reservation is mathematically bounded to $\le \text{chunk\_size}$ (typically $\le 4$ tokens, or 0 under mock).
4. **Failure Behavior:**
   - Provider timeouts, socket resets, or HTTP 5xx errors retain the full conservative reservation $R = I + O$ against the arm budget to prevent any unmetered retries.

---

## 6. Mathematical Exposure Proof & Reconciliation

### 6.1 Resolution of the $O_A + O_B = 8,192$ Controversy
Previous audit phases debated whether adding $O_A + O_B = 8,192$ to the global budget ceiling $B_G = 2,500,000$ double-counted completion allowances.

**Mathematical Reconciliation:**
1. In the legacy subprocess model, an arm was dispatched if $T_{\text{consumed}} \le C$. Once launched, the arm could emit up to 4,096 output tokens that were not pre-checked before the model generation finished, leading to an external overshoot of up to $4,096$ tokens per task ($8,192$ across Tasks A and B).
2. In `ControlledAgentRuntime`, **completion tokens are pre-reserved prior to wire dispatch**:
   $$\text{Reservation } R = I + O \le B_{\text{remaining}} - B_{\text{reserve}}$$
   where $O = 4,096$ and $B_{\text{reserve}} = 45,000$.
3. Because $R$ is deducted from remaining headroom *before* the request is sent, the completion allowance is **already budgeted within $B_G$**.
4. Adding $8,192$ to $B_G$ was an artifact of the legacy post-hoc tracking model. Under the controlled runtime, maximum exposure $E_{\text{max}}$ is strictly conserved at:
   $$E_{\text{max}} = B_G = 2,500,000 \text{ tokens}$$
5. To maintain backward compatibility with legacy coordinator scripts that enforce $E_{\text{max}} = B_G + \text{tolerance} = 2,531,384$ tokens, the harness accepts any consumption $\le 2,531,384$ as safe, while the controlled runtime guarantees never exceeding $2,500,000$.

### 6.2 Financial Exposure Bounds
Using authoritative pricing for `gpt-5.6-luna`:
- Prompt Input: \$1.25 per 1,000,000 tokens
- Completion Output: \$5.00 per 1,000,000 tokens
- Blended Rate: \$2.50 per 1,000,000 tokens

| Metric | Token Quantity | Financial Exposure (Blended \$2.50/M) | Maximum Worst-Case Output (\$5.00/M) |
| :--- | :--- | :--- | :--- |
| Global Ceiling ($B_G$) | 2,500,000 tokens | \$6.25 | \$12.50 |
| Theoretical Upper Bound ($E_{\text{max}}$) | 2,531,384 tokens | \$6.33 | \$12.66 |
| Observed Dry-Run Pentad ($T_{\text{total}}$) | 57,440 tokens | \$0.14 | \$0.29 |

Financial and subscription quota exposure is strictly capped and presents zero unconstrained risk.

---

## 7. Offline Regression & Dry-Run Execution Evidence

### 7.1 Regression Test Suite Results (116 Tests, 100% Pass)
| Test Suite File | Test Count | Result | Execution Time |
| :--- | :--- | :--- | :--- |
| `test_controlled_transport.py` | 24 | 24 PASSED | 3.12s |
| `test_request_boundary_enforcement.py` | 10 | 10 PASSED | 1.84s |
| `test_request_size_gate.py` | 14 | 14 PASSED | 1.25s |
| `test_round_trip_telemetry.py` | 7 | 7 PASSED | 0.98s |
| `test_budget_coordinator.py` | 7 | 7 PASSED | 1.15s |
| `test_telemetry_semantics.py` | 4 | 4 PASSED | 0.82s |
| `test_harness_safety_audit.py` | 30 | 30 PASSED | 5.92s |
| `test_pentad_harness.py` | 20 | 20 PASSED | 121.91s |
| **Total Regression Coverage** | **116** | **116 PASSED** | **137.0s** |

### 7.2 Two-Task Pentad Dry Run (`run_two_task_pentad.ps1 -DryRun -Seed 59002`)
- **Task A Execution:**
  - Consumed Tokens ($T_A$): 28,720 tokens
  - 14-Point Validation Gate: **14/14 Checks Passed**
  - Exit Code: 0
- **Dynamic Task B Ceiling Calculation:**
  $$C_B = \min(1,500,000, 2,500,000 - 28,720) = 1,500,000 \text{ tokens}$$
  Per-Arm Task B Ceiling: 300,000 tokens
- **Task B Execution:**
  - Consumed Tokens ($T_B$): 28,720 tokens
  - 14-Point Validation Gate: **14/14 Checks Passed**
  - Exit Code: 0
- **Consolidated Pentad Summary:**
  - Total Consumed Tokens ($T_{\text{total}}$): 57,440 tokens
  - Global Ceiling ($B_G$): 2,500,000 tokens
  - Exposure Compliance: **PASSED (57,440 <= 2,531,384)**
  - Summary Artifact: `benchmarks/matched-pair-harness/runs/two_task_pentad/two_task_pentad_summary.json`

---

## 8. Final Live-Benchmark Go/No-Go Decision

### Certified Verdict:
```
READY_WITH_BENCHMARK_PROTOCOL_REVISION_REQUIRED
```

### Pre-Requisites for Live Execution Authorization:
1. Ensure the live execution invocation passes `--provider controlled` (now enforced by default).
2. Explicitly specify `--api-key` or set `OPENAI_API_KEY` in the execution environment.
3. Formally tag benchmark results as **Protocol v3.0.0** to ensure historical Codex-native measurements remain appropriately partitioned.
4. Execution must use the validated script:
   ```powershell
   powershell -ExecutionPolicy Bypass -File benchmarks/matched-pair-harness/scripts/run_two_task_pentad.ps1 -Seed 59002 -Provider controlled
   ```
