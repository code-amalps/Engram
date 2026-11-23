# Engram Phase 8.1.1.2: Evidence & Claims Forensic Audit

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**GitHub Target:** `code-amalps/Engram` (`origin`), branch `feature/phase2-stage5-remediation`  
**Audit Target:** Phase 8.1.1 Reports (`phase_8_1_1_benchmark_analysis.md`, `phase_8_1_1_final_certification_report.md`)  
**Auditor:** Independent Acceptance Verification  
**Date:** 23 November 2026  

---

## 1. Audit Taxonomy & Standard

Every technical claim made in Phase 8.1.1 documentation is evaluated and classified according to the following strict evidentiary standard:

| Classification | Definition |
|---|---|
| **Verified by Actual Live-Pipe Benchmark** | Directly supported by empirical measurements collected over OS stdio pipes (`ChildStdin` / `ChildStdout`). |
| **Verified by Automated Test** | Directly asserted and passed by an automated test in `tests/`. |
| **Verified by Source Inspection** | Directly confirmed by examining the committed Rust source code. |
| **Derived Statistical Result** | Mathematically derived from the underlying raw JSON records. |
| **Inference (Documented Limitation)** | Logical deduction that is not directly measured (must be labeled as inference). |
| **Unverified / Corrected Claim** | Claim that was unsupported, inaccurate, or conflated and is formally corrected herein. |

---

## 2. Claims Audit & Classification Matrix

| # | Phase 8.1.1 Claim | Phase 8.1.1 Status | Phase 8.1.1.2 Re-Classification | Forensic Audit Findings & Corrections |
|---|---|---|---|---|
| **C1** | `is_git_clean` was fail-open and now fails closed | Claimed Verified | **Verified by Automated Test** & **Verified by Source Inspection** | Confirmed: `src/git.rs` implements `GitStateError`, `GitWorkingTreeState`, and bounded timeouts. Verified by `tests/git_state_reliability_test.rs` (8/8 tests pass). |
| **C2** | Fresh contract creation in `preflight_task` checked boundaries against constitution | Claimed Incomplete in 8.1 | **Verified by Automated Test** & **Verified by Source Inspection** | Confirmed: Extracted shared `crate::contract::validate_planned_boundaries` in `src/contract.rs` and integrated into both `flywheel.rs` and `engage.rs`. |
| **C3** | Phase 8.1.1 150-run benchmark measured actual OS-pipe wire traffic | Claimed Verified | **Unverified / Corrected Claim** (Corrected in 8.1.1.2) | **CORRECTION:** Phase 8.1.1 calculated wire framing sizes accurately but invoked execution via in-process `mcp_call_raw`. Real OS-pipe benchmark execution was implemented in Phase 8.1.1.2 via `LiveMcpWireClient` in `tests/engage_bench_test.rs`. |
| **C4** | JSON-RPC wire-framing byte counting is exact on UTF-8, newlines, and escaping | Claimed Verified | **Verified by Automated Test** | Confirmed: `tests/wire_measurement_test.rs` explicitly tests ASCII, multi-byte UTF-8, escaped strings, and stdio pipes. |
| **C5** | Category 1 Equivalent-Work reduces wire payloads by ~31-45% | Claimed Verified | **Verified by Actual Live-Pipe Benchmark** & **Derived Statistical Result** | Confirmed: Live OS-pipe benchmark shows 45.6% on EQ-01, 44.5% on EQ-02, 20.9% on EQ-03, 12.9% on EQ-04, and 31.0% mean paired savings overall. |
| **C6** | Category 1 Equivalent-Work reduces total execution latency | Disclaimed in 8.1.1 | **Verified by Actual Live-Pipe Benchmark** (Penalty Confirmed) | Confirmed: Consolidating 7 operations into a single pass takes longer (~542 ms vs ~345 ms in local debug stdio) due to synchronous SQLite setup and comprehensive verification. Latency penalty is clearly retained and disclosed. |
| **C7** | Eliminating one MCP tool call guarantees latency reduction | Disclaimed in 8.1.1 | **Inference (Documented Limitation)** (Negative Finding) | Confirmed: Eliminating a local stdio call does NOT reduce execution latency when the single consolidated call executes more comprehensive verification. Network round-trip benefits cannot be inferred from local stdio pipes. |
| **C8** | Category 2 Trivial Bypass saves ~47-53% latency | Claimed Verified | **Verified by Actual Live-Pipe Benchmark** & **Derived Statistical Result** | Confirmed: Live OS-pipe measurements demonstrate 419.6 ms $\rightarrow$ 218.4 ms (+46.8% paired savings / +47.9% aggregate change) by safely skipping AST parsing and contract generation. |
| **C9** | Local stdio pipe results prove network / SSE MCP performance | Never Claimed | **Inference (Documented Limitation)** | Confirmed limitation: Local OS pipe benchmarks do not measure network latency, TLS overhead, or SSE transport dynamics. |
| **C10**| Engram Engage reduces LLM provider billing tokens or costs | Never Claimed | **Inference (Documented Limitation)** | Confirmed limitation: Wire byte reductions measure MCP client-server protocol payload bytes, not external LLM API token counts or billing consumption. |
| **C11**| Phase 8.1.1 latency savings table numbers (`-63.0%`, etc.) match aggregate means | Ambiguous in 8.1.1 | **Derived Statistical Result** (Mathematically Reconciled) | **RECONCILIATION:** Clarified that `-63.0%` was the Mean Paired Latency Savings ($\frac{1}{N}\sum \frac{B_i - E_i}{B_i}$), not the ratio of sample means (-55.6% in 8.1.1, -47.7% in 8.1.1.2). Both metrics are now explicitly defined and presented side-by-side. |
| **C12**| `704.1 ms` and `1,093.5 ms` were the baseline and engage durations for EQ-01 | Ambiguous in 8.1.1 | **Derived Statistical Result** (Mathematically Reconciled) | **RECONCILIATION:** Clarified that `704.1 ms` and `1,093.5 ms` were the cross-workload Category 1 aggregate means, while `701.7 ms` and `1,092.0 ms` were the EQ-01 specific means. |

---

## 3. Mandatory Epistemic Disclaimers

1. **No Production Latency Inferences:**
   Local Windows stdio pipe measurements reflect operating system process scheduling, unoptimized debug compilation, and local SQLite I/O. They do not simulate remote network latency, cloud proxy overhead, or distributed server environments.
2. **No Billing Token Inferences:**
   Wire payload bytes represent JSON-RPC 2.0 messages over stdin/stdout. They do not represent OpenAI, Anthropic, or Google API token counts.
3. **Local Debug Latency Penalty Retained:**
   In local debug mode, consolidating AST extraction, boundary validation, Git state checking, and contract creation into a single pass takes approximately 50-70% longer than two sequential, simpler tool calls. This latency penalty is permanent in local execution and must never be minimized or obscured.
