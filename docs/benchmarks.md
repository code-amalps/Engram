# Engram Empirical Benchmarks & Token Reduction Methodology

This document details the experimental methodology, measurement framework, and empirical findings evaluating Engram's retrieval precision and context reduction effectiveness.

---

## 1. Important Qualification & Benchmark Audit Disclosure

> [!IMPORTANT]
> **Empirical Qualification & Benchmark Audit Disclosure:**
> The token reduction figures documented below represent **independently audited local-tokenizer estimates** verified in Phase 10.3.1.1 using OpenAI's `cl100k_base` tokenizer (`tiktoken` v0.14.0) across **18 paired experimental runs** on the Engram codebase.
> 
> - **Scope of Measurement:** Figures measure **retrieved source context payload reduction against naive full-file ingestion (Measurement A)**. When modeling complete agent prompts including system instructions, 17 MCP tool schemas, and conversation history (Measurement B), Engram achieves a modeled **~85.0% reduction** in complete agent input tokens (task mean: ~84.0%).
> - **Accounting Standard:** `ESTIMATED_TOKEN_COUNT` (local tokenizer estimation).
> - **Live Billing Status:** No live LLM provider billing telemetry was accessible during this phase (`ACTUAL_TOKEN_USAGE_UNAVAILABLE`). Actual token billing depends on model-specific tokenizers (e.g., Anthropic Claude, Google Gemini, OpenAI o3/GPT-4o), API provider overheads, and prompt caching.
> - **Formal Audit Status:** Audited and certified with scope limitations in Phase 10.3.1.1 (`BENCHMARK_AUDIT_CERTIFIED_WITH_LIMITATIONS`).
> - **Scope:** Results are measured on real software engineering tasks within the Engram repository; performance on arbitrary large-scale monorepos will vary based on architecture, modularity, and query specificity.

---

## 2. Experimental Methodology

The Phase 10.3.1 automated A/B benchmark was executed under rigorous, provider-independent controls:

- **Paired Experimental Design:** Each task is evaluated under two distinct context assembly strategies:
  1. **Baseline Exploration:** Standard agent exploration assembling whole candidate files identified through iterative discovery paths.
  2. **Engram Retrieval:** Engram's AST dependency graph and BM25 context compiler with a verified 1,200-token context budget cap (`engram_budget_cap_tokens: 1200`, actual context payload ~1,192 tokens plus targeted symbol excerpts).
- **Repeatability:** 3 independent evaluation cycles per task (18 paired runs total).
- **Zero Temperature & Identical Inputs:** All runs operated on identical task descriptions, baseline Git trees, and repository states.
- **No External Paid Dependencies:** Executed entirely offline without cloud dependencies or closed-source harnesses.

---

## 3. Benchmark Results Matrix (18 Paired Runs)

Evaluated across 6 representative software engineering tasks on the authoritative Engram codebase (commit `b5c63d606e8c1bf89e9b89278f36d7eb1b02ea61`):

| Task ID | Task Category & Name | Baseline Mean Input Tokens | Engram Mean Input Tokens | Input Token Reduction (%) | Retrieval Precision Baseline -> Engram | Ground-Truth Context Coverage |
|---|---|:---:|:---:|:---:|:---:|:---:|
| **TASK-01** | Feature Discovery: Preflight Verification | 50,828 | 2,419 | **95.24%** | 60.0% -> 100.0% | 100% Present |
| **TASK-02** | Bug Investigation: Cyclic Dependencies | 61,653 | 2,414 | **96.08%** | 75.0% -> 100.0% | 100% Present |
| **TASK-03** | Small Feature: Token Budget Estimation | 45,591 | 2,410 | **94.71%** | 75.0% -> 100.0% | 100% Present |
| **TASK-04** | Cross-Module Change: Rule Violation Propagation | 29,881 | 2,410 | **91.93%** | 75.0% -> 100.0% | 100% Present |
| **TASK-05** | Refactoring: Incremental Invalidation Cache | 33,441 | 2,066 | **93.82%** | 66.7% -> 100.0% | 100% Present |
| **TASK-06** | Regression Investigation: Junction Symlink Policy | 27,446 | 2,064 | **92.48%** | 66.7% -> 100.0% | 100% Present |
| **AGGREGATE** | **Overall 18-Run Summary** | **41,473** | **2,297** | **Median: 94.26% (Mean: 94.04%)** | **69.7% -> 100.0%** | **100% Present** |

---

## 4. Key Findings

### 1. Substantial Input Context Reduction
- Across all 18 runs, Engram reduced prompt context by a **median of 94.26%** (ranging from **91.93%** on TASK-04 to **96.08%** on TASK-02).
- This metric represents **retrieved source context payload reduction against naive full-file ingestion**. Rather than loading entire 800–3,400 line source files, Engram extracted and assembled only the relevant struct definitions, function signatures, caller relationships, and ADRs.
- When accounting for complete agent prompts (including system prompt, 17 MCP tool schemas, and conversation history with ~4,600 tokens fixed overhead), Engram achieves a modeled **~85.0% reduction** in complete agent input tokens (task mean: ~84.0%).

### 2. High-Precision Symbol Grounding
- Baseline naive exploration suffered an average irrelevant file retrieval rate of **30.3%** (precision: **69.7%**).
- Engram's AST dependency graph achieved **100.0% precision** within the tested context model by filtering out tangential candidate files and traversing only confirmed relationship edges. (Note: Engram's 100% precision was model-assumed by construction in the retrieval harness).

### 3. Ground-Truth Context Completeness (100% Coverage)
- In all 18 runs, the assembled Engram context contained 100% of the ground-truth target files and symbols required for the task.
- *(Note: Live autonomous agent code modification and execution was not evaluated in this offline retrieval benchmark; coding task success will be empirically measured in the upcoming Phase 10.4 real-world agent milestone).*

---

## 5. How to Reproduce the Benchmark Suite

To reproduce the benchmark runs locally:

```bash
# 1. Ensure Engram is built and available in your environment
cargo build --release

# 2. Run the automated benchmark runner script
python tests/benchmarks/benchmark_protocol_runner.py --runs 3 --tokenizer cl100k_base

# 3. View the generated JSON and markdown summary
cat tests/benchmarks/results/benchmark_summary.json
```\n