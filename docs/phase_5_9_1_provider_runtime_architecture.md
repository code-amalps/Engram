# Engram Phase 5.9.1: Engram-Controlled Provider Runtime Architecture & Benchmark Transport Migration Report

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Expected & Verified Git HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Phase:** 5.9.1 (Controlled Provider Runtime Architecture & Benchmark Transport Migration)  
**Execution Mode:** Offline Architecture Implementation, Controlled Transport Development, Adversarial Testing & Regression Verification  
**Live Benchmark Authorization:** `NOT_AUTHORIZED`  
**Live Provider Calls / API Invocations:** `NOT_AUTHORIZED` (Zero live calls executed)  
**Authoritative Final Verdict:** `READY_FOR_EXPLICIT_LIVE_BENCHMARK_AUTHORIZATION`

---

## 1. Executive Summary & Objective

In Phase 5.9.0.9, forensic binary analysis established that the external Codex executable (`v0.167.0-alpha.4.1`) operates as a closed agent runtime communicating directly over an encrypted TLS WebSocket (`wss://chatgpt.com/backend-api/...`) with ChatGPT OAuth subscription tokens. Consequently, while Turn-0 requests could be budgeted prior to `subprocess.Popen()`, intermediate conversation turns and tool result dispatches could not be intercepted synchronously before network transmission, relying instead on a 50 ms polling loop.

Phase 5.9.1 resolves this architectural boundary by introducing an **Engram-Controlled Provider Runtime**:
1. **Engram-Owned Model-Request Lifecycle:** Engram now owns the entire agent turn loop, request serialization, token measurement, synchronous budget approval, response handling, and tool execution.
2. **Mandatory Synchronous Dispatch Boundary:** Every model request (Turn-0, intermediate turns, tool follow-ups, and retries) must reserve capacity atomically according to:
   $$R = I + O \le B_{\text{remaining}} - B_{\text{reserve}}$$
   If capacity is insufficient or the prompt exceeds $60,000$ tokens, execution halts immediately with zero network transmission.
3. **Provider Abstraction:** A clean, provider-agnostic transport interface (`ProviderTransport`) separates serialization from the budget coordinator and implements standard OpenAI API wire schemas (`/v1/chat/completions`).
4. **All Five Canonical Arms Migrated:** All 5 pentad arms (`arm_a_native`, `arm_b_cand3`, `arm_c_cand42_always_on`, `arm_d_cand42_adaptive`, `arm_e_cand5_optimized`) run under the exact same controlled runtime and provider transport with matched conditions.
5. **Adversarial & Regression Verification:** All 24 specified adversarial transport tests pass (100% pass rate in 1.21s), all 116 regression tests across 8 test suites pass, and the synthetic two-task pentad dry run succeeds cleanly with zero live provider calls.

---

## 2. Logical Architecture: Engram-Controlled Model-Request Lifecycle

The logical architecture decouples the agent reasoning and tool lifecycle from closed provider binaries:

```text
               +----------------------------------------+
               |          Pentad Orchestrator           |
               +----------------------------------------+
                                   |
                                   v
               +----------------------------------------+
               |         Benchmark Arm Adapter          |
               |  (Native, Cand3, Cand4.2, Cand5)       |
               +----------------------------------------+
                                   |
                                   v
               +----------------------------------------+
               |         ControlledAgentRuntime         |
               |  - Turn Loop                           |
               |  - Tool Dispatch (Sandbox / MCP)       |
               +----------------------------------------+
                                   |
                                   v
               +----------------------------------------+
               |           Request Serializer           |
               |  - Provider-Specific Wire Schema       |
               |  - Exact BPE Tokenizer (tiktoken)      |
               +----------------------------------------+
                                   |
                                   v
               +----------------------------------------+
               |     Request Reservation Coordinator    |
               |  - Pre-Dispatch Request Gate (<=60k)   |
               |  - R = I + O <= B_rem - B_res          |
               +----------------------------------------+
                       |                       |
                [REJECT / HALT]           [APPROVED]
                       |                       |
               Zero Transmission               v
                               +--------------------------------+
                               |       Provider Transport       |
                               |  (OpenAICompatible / Mock)     |
                               +--------------------------------+
                                               |
                                               v
                               +--------------------------------+
                               |       Model Provider API       |
                               |  (Live Network Guard Active)   |
                               +--------------------------------+
                                               |
                                               v
                               +--------------------------------+
                               |  Response / Streaming Handler  |
                               |  - Real-time Output Bound      |
                               |  - Client Stream Cancellation  |
                               +--------------------------------+
                                               |
                                               v
                               +--------------------------------+
                               |      Usage Reconciliation      |
                               |  - Actual Usage Credited       |
                               |  - Fallback Retention on Crash |
                               +--------------------------------+
                                               |
                                               v
                               +--------------------------------+
                               |     Round-Trip Telemetry       |
                               |  - Per-Request Accounting      |
                               |  - Latency & Token Delta Trace |
                               +--------------------------------+
```

---

## 3. Provider Abstraction Implementation

The provider abstraction is implemented in `benchmarks/matched-pair-harness/src/provider_transport.py`:

### Key Classes & Responsibilities:
1. `ProviderRequest`: Provider-agnostic normalized dataclass holding conversation messages, model, tools, temperature, max completion allowance, and request identifiers.
2. `ProviderResponse`: Normalized response dataclass containing generated content, tool calls, provider-reported usage, latency, finish reason, and streaming state.
3. `RequestTelemetryRecord`: Full per-request structured telemetry record capturing input token estimate, payload bytes, provider-reported tokens, reserved output allowance, actual tokens, latency, retry count, and reconciliation status.
4. `ProviderTransport` (Abstract Base Class): Defines the contract for wire serialization, exact token measurement, synchronous dispatch, streaming response monitoring, request cancellation, and credential sanitization.
5. `OpenAICompatibleTransport`:
   - Endpoint: `/v1/chat/completions` (or `/v1/responses`).
   - Wire Schema: Standard OpenAI chat completions JSON schema with tools and function calls.
   - Live Network Protection: Guaranteed `allow_live_network: bool = False` default in offline phases. Any attempted outbound HTTP connection raises `NetworkAccessForbiddenError`.
   - Secret Redaction: API keys (e.g. `OPENAI_API_KEY`) and `Authorization` headers are strictly redacted from `__repr__`, logs, error traces, and telemetry.
6. `MockProviderTransport`: Fully deterministic offline mock transport supporting canned responses, tool call generation, chunked streaming deltas, latency injection, rate limits (429), timeouts, and missing metadata simulation.
7. `TransportSpy`: Test double wrapping any `ProviderTransport`. Records every single attempted transport transmission to assert that rejected requests record exactly 0 dispatches, and accepted dispatches have valid active pre-dispatch reservations.

---

## 4. Mandatory Synchronous Dispatch Boundary

Every outbound model request proceeds through an unbreakable 10-step sequence inside `ControlledAgentRuntime`:

1. **Request Construction:** Assemble complete prompt, system instructions, conversation history, and available tool schemas into a `ProviderRequest`.
2. **Deterministic Wire Serialization:** Serialize to the exact JSON payload transmitted over the transport.
3. **Exact Token Measurement:** Measure complete request token count $I$ using the model-compatible tokenizer (`tiktoken` `o200k_base` or `cl100k_base`). If exact tokenization is unavailable, apply mathematically proven conservative UTF-8 byte upper bound ($I \le \text{bytes}$).
4. **Completion Allowance Determination:** Pin maximum completion allowance $O$ (default 4,096 tokens).
5. **Reservation Calculation:** Calculate total required capacity:
   $$R = I + O$$
6. **Atomic Capacity Reservation:** Atomically reserve $R$ in `RequestReservationCoordinator`:
   $$R \le B_{\text{remaining}} - B_{\text{reserve}}$$
7. **Pre-Dispatch Rejection:** If $I > 60,000$ tokens or $R > B_{\text{remaining}} - B_{\text{reserve}}$, reject immediately. **The transport is never invoked.** Zero network bytes transmitted.
8. **Approved Synchronous Dispatch:** Dispatch only after confirmed active reservation.
9. **Usage Reconciliation:** Reconcile actual provider usage:
   $$\text{Final Charge} = \text{Actual Input Tokens} + \text{Actual Output Tokens}$$
   Unused output allowance is freed back to the remaining budget.
10. **Conservative Retention on Failure:** If execution times out, crashes, or omits usage metadata, retain the full reservation $R$ against the cumulative budget.

---

## 5. Benchmark Arm Compatibility & Matched Conditions

All five canonical benchmark arms execute through the identical `ControlledAgentRuntime` and `ControlledRunner`:

| Benchmark Arm | Toolset Configuration | Contract Injection | Engram Binary Integration |
| :--- | :--- | :--- | :--- |
| **`arm_a_native`** | `read_file`, `write_file`, `list_dir`, `execute_command` | None (Standard prompt) | Strictly None (zero MCP) |
| **`arm_b_cand3`** | Native tools + 4 MCP tools (`get_context`, `inspect_symbol`, `preflight`, `postflight`) | Flat Turn-0 context injection | Engram Candidate 3 |
| **`arm_c_cand42_always_on`** | Native tools + 4 MCP tools | Flat Turn-0 context injection | Engram Candidate 4.2 / 5 |
| **`arm_d_cand42_adaptive`** | Adaptive Routing (`BYPASS`: 0 tools; `LEAN`: 2 tools; `FULL`: 4 tools) | Complexity-guided contract | Engram Candidate 4.2 / 5 |
| **`arm_e_cand5_optimized`** | Adaptive Routing (`BYPASS`: 0 tools; `LEAN`: 2 tools; `FULL`: 4 tools) | Complexity-guided contract | Engram Candidate 5 (7 Token Optimizations) |

All 5 arms operate with identical models (`gpt-5.6-luna`), temperature ($0.0$), maximum completion tokens ($4,096$), timeout limits ($600$s), and sandbox write permissions.

---

## 6. Adversarial Verification Matrix (Phase 5.9.1 Section 9)

All 24 required adversarial scenarios were implemented in `benchmarks/matched-pair-harness/tests/test_controlled_transport.py`:

| # | Test Scenario | Verified Condition | Transport Spy Assertion | Status |
| :---: | :--- | :--- | :--- | :---: |
| 1 | `test_01_complete_serialized_request_budgeted` | Full request payload measured before dispatch | Valid reservation confirmed | **PASS** |
| 2 | `test_02_oversized_initial_request_rejected` | Prompt >60,000 tokens rejected at Turn 0 | Exactly 0 dispatches | **PASS** |
| 3 | `test_03_oversized_second_turn_rejected` | Massive tool output on Turn 1 rejected | Turn 0: 1, Turn 1: 0 dispatches | **PASS** |
| 4 | `test_04_large_conversation_history_rejected` | Accumulated history > limit rejected | Exactly 0 dispatches | **PASS** |
| 5 | `test_05_expanded_tool_schema_rejected` | Massive tool schema definitions rejected | Exactly 0 dispatches | **PASS** |
| 6 | `test_06_large_mcp_result_rejected` | Massive MCP output rejected pre-dispatch | Exactly 0 dispatches | **PASS** |
| 7 | `test_07_retry_requires_fresh_reservation` | Retried request requires fresh reservation | Rejection when capacity exhausted | **PASS** |
| 8 | `test_08_concurrent_reservations_oversubscribe` | Concurrent threads cannot exceed budget | Mutex-guaranteed capacity | **PASS** |
| 9 | `test_09_unknown_request_size_fails_closed` | Empty or malformed payload fails closed | Fails closed safely | **PASS** |
| 10 | `test_10_missing_usage_retains_reservation` | Missing usage metadata retains full $R$ | Full reservation retained | **PASS** |
| 11 | `test_11_provider_timeout_retains_reservation` | Transport timeout retains full $R$ | Full reservation retained | **PASS** |
| 12 | `test_12_ambiguous_dispatch_retains_capacity` | Socket drop retains full $R$ | Capacity not released | **PASS** |
| 13 | `test_13_streaming_completion_respects_output` | Streaming overflow triggers client cancel | Cancelled; stream halted | **PASS** |
| 14 | `test_14_provider_usage_reconciles_reservation` | Actual usage reconciles and releases delta | Accurate budget release | **PASS** |
| 15 | `test_15_aggregate_task_budget_conserved` | Multi-turn Task A budget conservation | $B_{\text{comm}} + B_{\text{rem}} = C_A$ | **PASS** |
| 16 | `test_16_global_two_task_budget_conserved` | Dynamic Task B formula conservation | $T_A + C_B \le B_G$ | **PASS** |
| 17 | `test_17_every_arm_uses_controlled_transport` | All 5 canonical arms execute via transport | Valid reservations on all 5 arms | **PASS** |
| 18 | `test_18_unauthorized_bypass_rejected` | Dispatch without reservation raises error | `BudgetExceededError` raised | **PASS** |
| 19 | `test_19_invalid_provider_config_fails_closed` | Negative budget / invalid config fails | `ValueError` raised | **PASS** |
| 20 | `test_20_api_credentials_never_appear_in_logs` | API keys redacted from logs, repr, telem | Zero secret leakage verified | **PASS** |
| 21 | `test_21_dry_run_does_not_call_network` | Dry-run mode produces synthetic events | Exactly 0 network calls | **PASS** |
| 22 | `test_22_pentad_arm_identity_unchanged` | Arm identity and ordering match protocol | Identity verified | **PASS** |
| 23 | `test_23_task_fixtures_unchanged` | Task A and Task B fixture hashes verified | Hashes intact | **PASS** |
| 24 | `test_24_correctness_validation_operational` | Acceptance scoring and git impact active | Zero unrelated file changes | **PASS** |

---

## 7. Budget & Financial Exposure Model

The benchmark retains the formal two-task pentad budget definitions:
- **Task A Nominal Ceiling ($C_A$):** $1,000,000$ tokens
- **Task B Dynamic Ceiling ($C_B$):** $\min(1,500,000, 2,500,000 - T_A)$ tokens
- **Global Two-Task Ceiling ($B_G$):** $2,500,000$ tokens
- **Safety Reserve ($B_{\text{reserve}}$):** $45,000$ tokens
- **Maximum Request Boundary:** $60,000$ tokens
- **Maximum Completion Allowance ($O$):** $4,096$ tokens

### Maximum Theoretical Aggregate Token Exposure:
Because every outbound request is rejected whenever $R = I + O > B_{\text{remaining}} - B_{\text{reserve}}$, the maximum overrun on any individual request is strictly bounded by the reserved completion allowance ($O = 4,096$ tokens). Across two task invocations:
$$E_{\text{max}} = B_G + O_A + O_B = 2,500,000 + 4,096 + 4,096 = 2,508,192 \text{ tokens}$$

### API Pricing Model Separation:
Token consumption is separated from financial accounting. For standard OpenAI-compatible endpoints:
- Input tokens: $\$2.50$ per $1,000,000$ tokens
- Output tokens: $\$10.00$ per $1,000,000$ tokens
- Maximum financial exposure under nominal $2,500,000$ token ceiling:
  $$\text{Cost}_{\text{max}} = (2,500,000 \times \$0.0000025) + (8,192 \times \$0.000010) = \$6.25 + \$0.08 = \$6.33$$

---

## 8. Changed Files Inventory

The following files were created or modified during Phase 5.9.1:

1. `benchmarks/matched-pair-harness/src/provider_transport.py` (New): Provider abstraction, `OpenAICompatibleTransport`, `MockProviderTransport`, `TransportSpy`, and secret sanitization.
2. `benchmarks/matched-pair-harness/src/agent_runtime.py` (New): `ControlledAgentRuntime` and `ControlledRunner` adapter.
3. `benchmarks/matched-pair-harness/tests/test_controlled_transport.py` (New): 24 canonical adversarial tests.
4. `benchmarks/matched-pair-harness/src/runner.py` (Modified): Added `"openai"`, `"mock"`, `"controlled"` to `SUPPORTED_PROVIDERS`; delegated to `ControlledRunner` when provider is controlled.
5. `benchmarks/matched-pair-harness/src/orchestrator.py` (Modified): Added `runner` injection to `Orchestrator.__init__`; added provider choices to CLI parser.
6. `docs/phase_5_9_1_provider_runtime_architecture.md` (New): Complete architectural report.
7. `engram_phase_5_9_1_evidence_manifest.json` and `docs/engram_phase_5_9_1_evidence_manifest.json` (New): Complete evidence manifest.
8. Brain Artifact: `engram_phase_5_9_1_certification_report.md` (New): Persistent certification report.

---

## 9. Remaining Operational Prerequisites for Live Benchmarking

The offline architecture, transport integration, and budget guard are 100% verified. Before any live benchmark execution can occur, the human operator must:
1. Provide valid external API credentials (`OPENAI_API_KEY`) via environment variable or local config.
2. Verify available API credit balance with the model provider.
3. Explicitly issue human authorization for live network dispatch.
