# Engram Benchmark Methodology & Empirical Audit

This document provides a critical audit of Engram's benchmark suite, detailing the methodology, distinguishing empirically measured values from baseline estimates, outlining sample sizes, and documenting experimental limitations.

---

## 1. Executive Summary of Claims & Verification

| Metric Claim | Type | Measured or Estimated? | Sample Size | Scope & Limitations |
|---|---|---|---|---|
| **Context Tokens (210 vs 4,500)** | Comparative | **Measured** Engram context vs. **Estimated** naive scan baseline | 8 synthetic tasks (7 languages) | Engram tokens are measured from actual context payloads; the 4,500 baseline is an estimated discovery model assuming full file reads. |
| **Token Reduction (-95.3%)** | Comparative | Derived from measured vs. estimated above | 8 synthetic tasks | Varies depending on repository size, token budget setting, and task scope. |
| **Tool Calls (1 vs 12)** | Comparative | **Measured** (1 Engram context call) vs. **Estimated** iterative tool-use baseline | 8 synthetic tasks | Compares single-turn MCP context compilation against typical iterative search/grep tool loops. |
| **Grounding File Recall (95.2%)** | Empirical | **Measured** against ground-truth files | 8 synthetic tasks | Ground truth is manually annotated for the benchmark task definitions. |
| **Composite Grounding F1 (0.813)** | Empirical | **Measured** (harmonic mean of recall & precision) | 8 synthetic tasks | Precision is ~61.4% due to conservative multi-hop symbol inclusion; F1 reflects file, symbol, and test recall. |
| **Change Contract Accuracy (100%)** | Synthetic | **Measured** on synthetic mutation suite | 32 synthetic mutation cases | **Synthetic evaluation only.** Tests boundary classification logic against defined mutation fixtures, not real-world developer errors. |
| **Memory Recall (100% vs 0% Control)** | Controlled A/B | **Measured** retrieval on defined follow-up tasks | 8 synthetic tasks | Confirms retrieval logic: unrecorded decisions return 0%; explicitly recorded decisions return 100% in controlled follow-up queries. |

---

## 2. Separation of Concerns: Measured vs. Estimated

A central methodological requirement in Engram is that **empirically measured numbers must never be conflated with simulated baseline estimates**:

### What Engram Actually Measures:
- **`measured_context_tokens`**: The exact number of tokens in the compiled context payload emitted by `engram context`, computed using GPT/Claude tokenizer approximations (`chars / 4` with punctuation weighting).
- **`measured_tool_calls`**: Exactly 1 tool invocation (or 1 CLI execution) delivering the task-grounded context.
- **`classification_accuracy`**: The deterministic accuracy of `engram contract check` and `engram guard` when evaluated against synthetic mutations of known ground-truth category (`EXPECTED`, `EXPANSION`, `UNEXPECTED`, `BOUNDARY_VIOLATION`, `MISSING`).
- **`cold_latency_ms` and `warm_incremental_latency_ms`**: Wall-clock duration measured via `std::time::Instant` across cold cache and warm incremental scans.

### What Is Estimated:
- **`estimated_baseline_tokens`**: An analytical model representing what an AI coding assistant would consume if it performed discovery by reading complete source files relevant to the task (sum of total tokens in un-sliced files).
- **`estimated_baseline_tool_calls`**: An analytical model representing the number of iterative `list_dir`, `grep_search`, and `view_file` calls an agent typically performs to locate relevant symbols in an un-indexed repository.

---

## 3. Benchmark Corpus Structure

The built-in benchmark suite (`engram bench`) evaluates tasks across isolated temporary repositories:

1. **OrderService Task (TypeScript):** E-commerce order lifecycle and CSV export grounding.
2. **CatalogManager Task (JavaScript):** Product catalog CRUD and routing.
3. **WarehouseService Task (Python):** Inventory reservation with FastAPI models.
4. **CustomerService Task (C#):** Clean architecture account management with interfaces.
5. **Worker Task (Rust):** Asynchronous work queue with multi-module traits and enums.
6. **PaymentProcessor Task (Go):** Go module, structural interface satisfaction, and table-driven tests.
7. **BillingService Task (Java):** Maven / Gradle Spring service with records and JUnit tests.
8. **Multi-Turn Compounding Suite:** 4 sequential turns evaluating initial discovery, memory compounding, policy supersession (`ADR-014` $\to$ `ADR-029`), and scoped conflict resolution.

---

## 4. Known Methodological Limitations

When reviewing Engram benchmarks, keep the following context in mind:

1. **Synthetic Corpus $\ne$ Production Generalization:** The 8 tasks in the standard suite are designed to be deterministic, offline, and runnable in sub-30 seconds. They demonstrate that Engram's algorithms function correctly on designed scenarios, but they do not guarantee an identical 95.3% token reduction on arbitrary, messy, 1-million-line monolithic codebases.
2. **Precision vs. Recall Tradeoff:** File recall is high (95.2%), but precision is lower (~61.4%). Engram intentionally errs on the side of including related interface definitions and tests rather than risking omitting necessary context.
3. **Baseline Comparison Model:** The baseline assumes the agent reads whole files. Agents with highly optimized sub-string search or BM25 embeddings might consume fewer baseline tokens than our 4,500 estimate.
4. **Synthetic Mutations:** The 100% contract accuracy reflects that our test cases accurately mirror the rule engine's specification. Real-world coding agent mistakes can be subtle (e.g., semantic logic bugs that do not violate file/symbol boundaries).

---

## 5. Reproducing Benchmarks Locally

You can independently reproduce all benchmark numbers on your own machine:

```bash
# Run the internal benchmark suite with human-readable summary:
engram bench

# Run with per-task breakdown and metric transparency:
engram bench --verbose

# Run with machine-readable JSON output:
engram bench --json

# Run external benchmark across your own repository:
engram bench --external /path/to/your/project
```
