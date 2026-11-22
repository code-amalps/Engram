# Engram Phase 5.4: Protocol Overhead & Host Integration Audit

**Document Path:** `docs/phase-5-4/PHASE_5_4_PROTOCOL_OVERHEAD_AUDIT.md`  
**Milestone:** Phase 5.4 — Protocol Overhead, Adaptive Context Optimization & Live Benchmark Validation  
**Date:** November 19, 2026  
**Status:** **PASSED**  

---

## 1. Executive Summary

This audit investigates the host integration architecture between OpenAI Codex CLI (`codex-cli 0.167.0-alpha.4.1`), `gpt-5.6-luna`, and the Model Context Protocol (MCP) server provided by Engram Candidate 3 (`97C5EC47...`).

During Phase 5.3 Task A, Treatment Arm B incurred a -14.22% cumulative token regression (172,512 vs 151,039 tokens). The initial hypothesis attributed this to an estimated ~3,500 token per-turn MCP schema injection tax.

**Independent empirical measurement has disproven that hypothesis:**
1. The 4 exposed Engram MCP tool schemas total **1,789 serialized bytes (~447 tokens)**, not 3,500 tokens.
2. In the initial turn (Request 1), Treatment B input tokens was **17,070** compared to Control A's **17,062** — a net difference of only **8 tokens**.
3. Across Requests 3 through 6, **Treatment B's per-request prompt context was actually 1,487 to 1,881 tokens SMALLER than Control A**, delivering a **4.85% reduction in peak context** and a **3.55% reduction in uncached prompt tokens**.
4. The cumulative token regression in Task A was caused solely by conversational turn count variance: **Treatment B executed 7 request turns, whereas Control A executed 6 request turns**. The single additional turn in Treatment B cost ~27,897 tokens, accounting for the entire observed difference (+21,473 tokens).

---

## 2. Host Integration Architecture (Codex CLI & MCP)

### 2.1 Tool Registration & Schema Injection Behavior
- **Host Configuration:** MCP servers are passed to `codex.exe exec` via CLI flags:
  `-c mcp_servers.engram.command="<path>/engram.exe"`
  `-c mcp_servers.engram.args=["-C", "<sandbox>", "mcp"]`
  `-c mcp_servers.engram.enabled_tools=["engram_get_context", "engram_inspect_symbol", "engram_preflight", "engram_postflight"]`
  `-c mcp_servers.engram.default_tools_approval_mode="approve"`
- **Schema Discovery:** Codex CLI initiates the MCP stdio JSON-RPC handshake (`initialize` followed by `tools/list`) during session startup.
- **Host Schema Format:** Codex CLI formats tools into its internal developer prompt as `<tools> Deferred tool namespaces: ... </tools>` and developer messages (`additional_tools`).
- **Dynamic Registration / Lazy Discovery:** The current Codex CLI does **NOT** support dynamic runtime tool registration or deregistration within an ongoing execution session. All tools declared in `enabled_tools` are negotiated at startup.
- **Conditional Exposure:** Tools cannot be selectively hidden per turn by the client once the session starts; however, the client harness can conditionally configure `enabled_tools` at launch.

### 2.2 Independent Schema Measurements

Measured by running `engram.exe mcp` with standard JSON-RPC `tools/list`:

| Tool Name | Purpose | Serialized JSON Bytes | Estimated Tokens ($\approx 4\text{ B/tok}$) |
| :--- | :--- | :---: | :---: |
| `engram_get_context` | Budget-bounded context compilation | 427 bytes | 106.8 tok |
| `engram_inspect_symbol` | Symbol impact & contract inspection | 342 bytes | 85.5 tok |
| `engram_preflight` | Pre-flight handshake & contract locking | 594 bytes | 148.5 tok |
| `engram_postflight` | Post-flight diff validation | 426 bytes | 106.5 tok |
| **Total MCP Tool Schema Surface** | **All 4 registered tools** | **1,789 bytes** | **447.2 tokens** |

In addition, Turn-0 directives injected into `.github/copilot-instructions.md` by `engram inject` for a single-file task measure **246 bytes (~61.5 tokens)**, and the harness `SHELL_RETRIEVAL_GUIDANCE` string measures **350 bytes (~87.5 tokens)**.

**Total Static Injected Protocol Overhead:**
$$1{,}789\text{ B (tools)} + 246\text{ B (instructions)} + 350\text{ B (guidance)} = 2{,}385\text{ bytes} \approx \mathbf{596 \text{ tokens}}$$

---

## 3. Dissecting the Task A Discrepancy

### 3.1 Per-Request Token Telemetry (Authoritative Session Rollouts)

Extracted from native `.codex/sessions` rollout logs:
- **Treatment B Rollout:** `rollout-2026-11-19T09-49-01-01a1f207-7ccc-7e63-9fd4-4069843f2079.jsonl`
- **Control A Rollout:** `rollout-2026-11-19T09-58-26-01a1f210-1db1-7a81-b1c1-9a8f0609f490.jsonl`

| Request Index | Control A Input Tokens | Treatment B Input Tokens | Delta ($A - B$) | Treatment Context Savings |
| :---: | :---: | :---: | :---: | :---: |
| **Request 1** | 17,062 tok (0 cached) | 17,070 tok (0 cached) | -8 tok | -0.05% (Negligible) |
| **Request 2** | 17,437 tok (16,896 cached) | 17,449 tok (16,896 cached) | -12 tok | -0.07% |
| **Request 3** | 28,599 tok (17,280 cached) | **26,948 tok** (17,280 cached) | **+1,651 tok** | **+5.77% Reduction** |
| **Request 4** | 28,798 tok (28,416 cached) | **27,230 tok** (26,752 cached) | **+1,568 tok** | **+5.45% Reduction** |
| **Request 5** | 28,861 tok (28,672 cached) | **27,374 tok** (27,008 cached) | **+1,487 tok** | **+5.15% Reduction** |
| **Request 6** | 29,318 tok (28,672 cached) | **27,437 tok** (27,136 cached) | **+1,881 tok** | **+6.42% Reduction** |
| **Request 7** | *N/A (Finished)* | 27,897 tok (27,264 cached) | -27,897 tok | Extra Turn Incurred |
| **Peak Context** | **29,318 tokens** | **27,897 tokens** | **+1,421 tokens** | **+4.85% Reduction** |
| **Cumulative Total** | **151,039 tokens** | **172,512 tokens** | **-21,473 tokens** | **-14.22% Regression** |

### 3.2 Key Findings:
1. **Per-Turn Context Efficiency:** On every comparable multi-turn request (Requests 3, 4, 5, 6), Treatment B maintained a strictly smaller prompt context than Control A, reducing prompt size by 1,487 to 1,881 tokens per request.
2. **Turn Count Inflation:** In Treatment B, the model invoked `function_call: wait` during PowerShell command completion checks, resulting in 7 total model requests before emitting the final agent response, compared to 6 in Control A.
3. **Prompt Cache Reuse:** In Request 7 of Treatment B, 27,264 tokens of the 27,897 input tokens (97.7%) were served from provider prompt cache, meaning actual uncached token generation was only 633 tokens.

---

## 4. Protocol Overhead Accounting Table

| Component | Measurement (Task A Treatment B) | Measurement Source |
| :--- | :---: | :--- |
| **Base System & Skills Instructions** | ~16,800 tokens | Provider rollout telemetry (Request 1 baseline) |
| **MCP Tool Schemas (4 tools)** | 1,789 bytes (~447 tokens) | Exact stdio `tools/list` JSON measurement |
| **Turn-0 Contract Directives** | 246 bytes (~61 tokens) | Exact `.github/copilot-instructions.md` measurement |
| **Context Retrieval Guidance** | 350 bytes (~88 tokens) | Exact string length in `runner.py` |
| **Task Prompt** | ~50 tokens | Character count / 4 |
| **Conversation History per Turn** | 17,000 to 27,000 tokens | Provider rollout telemetry (`token_usage_record`) |
| **Cached Input (Peak)** | 27,264 tokens | Provider rollout telemetry |
| **Uncached Input (Peak)** | 17,070 tokens (Turn 1); <1,000 thereafter | Provider rollout telemetry |
| **Model Completion Output** | 1,107 tokens total (39 to 351 per turn) | Provider rollout telemetry |
| **Reasoning Tokens** | 221 tokens total (6 to 104 per turn) | Provider rollout telemetry (`reasoning_output_tokens`) |
| **Provider-Reported Total** | 172,512 tokens | Provider rollout telemetry (`total_token_usage`) |
| **Total Real Latency** | 168.19 seconds | Harness wall-clock measurement |

---

## 5. Architectural Conclusions & Recommendations

1. **Hypothesis Disproven:** The MCP protocol schema is *not* a bloated 3,500 token tax. At 447 tokens, it is compact and accounts for <2.6% of the initial prompt.
2. **True Source of Task A Regression:** The regression in Task A was caused by conversational turn count ($N = 7$ vs $N = 6$), not static schema weight.
3. **Turn-Count Sensitivity:** Because full conversation history is re-sent on every multi-turn request (~27k tokens/turn), any extraneous turn (e.g. redundant polling or verification wait calls) imposes a massive ~27k cumulative token penalty.
4. **Adaptive Small-Task Bypass:** On trivial single-file tasks, zero-turn or 1-turn direct execution must be strongly incentivized to prevent conversational turn bloat.
