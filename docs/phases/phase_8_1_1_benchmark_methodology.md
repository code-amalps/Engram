# Engram Phase 8.1.1: Benchmark Methodology Specification

**Execution Mode:** Experimental Methodology Correction & Measurement Protocol  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Milestone:** Phase 8.1.1  
**Status:** `METHODOLOGY_CERTIFIED`  
**Date:** 23 November 2026  

---

## 1. Limitations of the Previous Phase 8.1 Benchmark

The initial benchmark in Phase 8.1 established that consolidating context and preflight into `engram_engage` reduced MCP tool calls. However, an independent methodological review identified several significant limitations in the Phase 8.1 experimental design:

1. **Execution Order Bias & Cache Warmth:**
   In Phase 8.1, the baseline path (`engram_get_context` followed by `engram_preflight`) was always executed immediately before `engram_engage` on the same workspace. The baseline potentially primed OS disk caches, database file buffers, and working tree state.
2. **Contract State Contamination:**
   In Phase 8.1, the baseline `engram_preflight` created and locked an active contract in `.engram/contracts/active.json`. While tests attempted to isolate execution, any residual contract state risked allowing the Engage endpoint to perform idempotent reuse rather than fresh contract generation.
3. **Flawed Aggregation of Trivial Bypass:**
   The Phase 8.1 report averaged the trivial documentation task (`TASK-01`) together with standard tasks. Because `TASK-01` exercised adaptive bypass (`BYPASS_TRIVIAL`, where AST compilation and contract generation are intentionally skipped), it did **not** perform equivalent logical work. Combining skipped work with full execution artificially distorted the overall headline latency improvement.
4. **Serialization Boundary vs Actual Wire Bytes:**
   Phase 8.1 measured internal serialized `serde_json::Value` sizes rather than the actual line-delimited JSON-RPC 2.0 messages exchanged over the stdio transport pipe.
5. **Statistical Insufficiency:**
   Each task was measured across only a few runs, lacking sufficient sample size to establish standard deviations, medians, or p95 distributions.

---

## 2. Phase 8.1.1 Corrected Methodology

Phase 8.1.1 introduces a rigorous, reproducible, and statistically defensible benchmark protocol:

### 2.1 Separation of Workload Categories
Workloads are split into two strictly separated categories:

#### Category 1: Equivalent-Work Comparison
Both the baseline path and the Engage path perform identical logical work on identical repository states:
- Baseline: Sequential execution of `engram_get_context` followed by `engram_preflight`.
- Engage: Single-call execution of `engram_engage` with `no_bypass: true` (forcing full AST context compilation, Project Constitution verification, and Change Contract generation).
- Workloads evaluated:
  - **`EQ-01`**: Single-File Refactor (`src/auth.rs`)
  - **`EQ-02`**: Multi-File Feature (`src/auth.rs` + `src/user.rs`)
  - **`EQ-03`**: Subsystem Impact Query (`src/auth.rs`)
  - **`EQ-04`**: Constitutional Boundary Violation (`config/secrets.json`)

#### Category 2: Trivial-Task Bypass Optimization
Evaluates the efficiency gain from avoided work when an agent performs a trivial leaf edit:
- Baseline: Two MCP tool calls (`engram_get_context` + `engram_preflight`).
- Engage: Adaptive bypass (`BYPASS_TRIVIAL`, zero context tokens, contract generation skipped).
- Workload evaluated:
  - **`BYPASS-01`**: Trivial Documentation Leaf (`README.md`)
- **Reporting Rule:** Category 2 is reported strictly as an optimization through avoided work and is **never** averaged into Category 1 equivalent-work metrics.

### 2.2 Prevention of State Contamination
To eliminate cache and contract contamination:
1. **Isolated Workspace State:** Before every individual tool invocation, the benchmark harness completely purges `.engram/contracts/` and `.engram/sessions/`.
2. **Alternating Execution Order:**
   - Odd iterations (1, 3, 5, ...): Baseline path executed first; state reset; then Engage path executed.
   - Even iterations (2, 4, 6, ...): Engage path executed first; state reset; then Baseline path executed.
   This guarantees that neither execution path systematically benefits from cache warmth or execution order.

### 2.3 Sample Size & Statistical Rigor
- **30 paired iterations** per workload (4 equivalent workloads = 120 paired runs; 1 trivial bypass workload = 30 paired runs; **150 paired iterations / 300 total executions**).
- For each metric, the following statistics are calculated and recorded:
  - Count ($N = 30$)
  - Arithmetic Mean ($\mu$)
  - Median ($M$)
  - Standard Deviation ($\sigma$)
  - Minimum and Maximum values
  - 95th Percentile ($p_{95}$)
- **Zero Discarding of Outliers:** Every measured run is preserved in the raw dataset.

### 2.4 Actual JSON-RPC Wire-Level Framing
Measurements capture the exact wire-level protocol exchanges:
- **Request Wire Bytes:** Exact UTF-8 byte count of the serialized JSON-RPC 2.0 request line including the trailing `\n` delimiter (`{"jsonrpc":"2.0","id":N,"method":"tools/call",...}\n`).
- **Response Wire Bytes:** Exact UTF-8 byte count of the serialized JSON-RPC 2.0 response line including the trailing `\n` delimiter (`{"jsonrpc":"2.0","id":N,"result":{...}}\n`).
- **Total Wire Bytes:** Sum of Request Wire Bytes and Response Wire Bytes.
- **Client-Observed Latency:** Wall-clock duration in milliseconds from immediately prior to request serialization until response line deserialization completes.

---

## 3. Standardized Terminology

To prevent misleading claims, Phase 8.1.1 adheres strictly to the following terminology:

| Term | Meaning & Scope |
|---|---|
| **MCP Tool-Call Count** | Number of discrete JSON-RPC `tools/call` request-response round trips over the transport. |
| **Request Wire Bytes** | Actual UTF-8 bytes sent over the OS stdio pipe for JSON-RPC request framing and arguments. |
| **Response Wire Bytes** | Actual UTF-8 bytes received over the OS stdio pipe for JSON-RPC response framing and content. |
| **Total Wire Bytes** | Sum of Request Wire Bytes and Response Wire Bytes exchanged across all calls in the workflow. |
| **Tool Arguments Bytes** | Byte size of the inner `params.arguments` JSON payload. |
| **Tool Result Bytes** | Byte size of the inner `result` JSON payload. |
| **Client-Observed Duration** | Total elapsed client wall-clock time in milliseconds across the tool call(s). |

**Terminology Disclaimer:** Local synthetic benchmark results reflect transport and engine processing on Windows x64. They do **not** represent live LLM provider billing token savings, API egress fees, or production agent network round-trip latencies over remote networks.
