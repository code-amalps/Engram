# Engram Future Architecture Preservation: Roadmap & Design Foundations

## Milestone 5F — Workstream D Architectural Blueprint

### Executive Summary

During Milestone 5E forensic analysis and Milestone 5F remediation, Engram's primary engineering objective was reaffirmed:
> **The Golden Rule of Engineering Context Efficiency**: Every token Engram returns to an AI coding agent must replace more tokens the agent would otherwise consume by reading source files, grepping directories, or resending conversation context across repetitive exploratory turns.

This document formally captures and preserves twelve foundational architectural design requirements identified for post-Milestone 5F development. These principles ensure future roadmap development directly targets token efficiency, cache stability, and agent productivity without introducing extraneous cognitive or token overhead.

---

### The 12 Architectural Pillars

#### 1. Automatic Task Memories & Insight Retention
- **Problem**: In multi-turn coding sessions, agents frequently rediscover the same architectural conventions, environment quirks, and failure modes across subsequent invocations, wasting tens of thousands of tokens per session.
- **Architecture**:
  - Automatically extract concise semantic facts ("memories") at the conclusion of successful task executions.
  - Store memories keyed by repository context, domain tags, and symbol paths.
  - Inject only high-affinity memories into Turn-0 context when related files or modules are touched.
  - Automatically expire or down-rank memories that contradict fresh code changes or fail subsequent evaluations.

#### 2. Evidence-Based Cache Invalidation
- **Problem**: Naive timestamp (mtime) or blanket cache invalidation causes frequent costly rebuilds of the semantic index, while static TTLs risk serving stale context.
- **Architecture**:
  - Base cache validity strictly on content-addressable SHA-256 hashes of source files and individual AST entity spans.
  - If a file's mtime changes but AST entity signatures (names, parameters, exports, types) remain identical, retain all graph edges and skip downstream re-indexing.
  - Isolate cache invalidation ripples strictly to direct callers and dependents in the call graph.

#### 3. Gitless Content-Hash Snapshots
- **Problem**: Coding agent benchmarks and ephemeral test sandboxes often operate in detached, dirty, or uncommitted git states where `git rev-parse` or git worktrees either fail or introduce disk overhead.
- **Architecture**:
  - Implement an in-memory or SQLite-backed content-hash tree (Merkle DAG) of the repository workspace independent of git metadata.
  - Support instant point-in-time diffing, snapshot rollbacks, and change tracking without modifying the working tree's `.git` folder or creating extraneous git commits.

#### 4. Cross-Repository Frontend / Backend Graph Traversal
- **Problem**: Modern software architectures split client-side frontend code (e.g., Angular/React/TypeScript) and server-side backend services (e.g., Rust, Go, Python, C#). Agents navigating full-stack features get lost navigating cross-repo boundaries.
- **Architecture**:
  - Extend the symbol dependency graph across repository boundaries via contract schemas (OpenAPI specs, GraphQL schemas, Protobufs, gRPC definitions, and REST route decorators).
  - Enable bidirectional jump-to-definition: from frontend API service calls directly to backend route handlers, eliminating blind full-text searches.

#### 5. Incremental Context Updates & Delta Streaming
- **Problem**: Re-sending complete compilation contexts or large summaries on every agent query floods the context window with repetitive tokens.
- **Architecture**:
  - Implement a session context ledger tracking what symbols, types, and excerpts the agent has already received in the current thread.
  - Return context deltas: when an agent queries an updated module, emit only modified AST blocks and diff annotations rather than re-emitting whole file skeletons.

#### 6. Build & Test Log Summarization
- **Problem**: Megabytes of compiler diagnostics, stack traces, and test runner outputs overwhelm context windows, consuming context headroom and triggering premature guard cutoffs.
- **Architecture**:
  - Add an integrated diagnostic compressor that ingests raw compiler/test runner stdout/stderr.
  - Filter out repetitive progress bars, deprecation warnings, and verbose passing test names.
  - Extract the exact failure root cause, file path, line number, and diagnostic code into a compact token budget (<500 tokens).

#### 7. Prompt-Cache Stability & Prefix Determinism
- **Problem**: Modern frontier models (GPT-4o, Claude 3.5 Sonnet, Gemini 1.5 Pro) offer deep prompt caching (>80% cost and latency reduction), but non-deterministic formatting or dynamic prefixes invalidate the KV cache.
- **Architecture**:
  - Enforce strict byte-level determinism in all Turn-0 system prompts, tool schemas, and injected contexts.
  - Order symbol tables, types, and dependencies lexicographically by stable canonical keys.
  - Segregate immutable system instructions and frozen workspace context into the prompt prefix, isolating volatile session state strictly to turn suffixes.

#### 8. Dynamic Retry Budgets & Exploration Cost Caps
- **Problem**: Agents experiencing build or test failures can enter pathological retry loops (e.g., repeatedly editing imports or toggling flags), burning model budgets without convergence.
- **Architecture**:
  - Establish per-task and per-subproblem retry budgets with exponential decaying token allowances.
  - Track edit-distance and semantic delta between successive failed attempts; if semantic diversity is low, halt the retry cascade and guide the agent toward alternate exploration strategies.

#### 9. No-Progress & Repetitive Action Detection
- **Problem**: In headless runs, agents may repeatedly invoke the same tool with identical arguments or perform cyclic file inspections without taking actionable edit steps.
- **Architecture**:
  - Harness and MCP layers monitor action history.
  - If identical tool queries occur $\ge 3$ times with zero state change or file edits, trigger an early circuit-breaker.
  - Inject a diagnostic intervention prompting the agent to reconsider its hypothesis or escalate to user input.

#### 10. Strict Resource Governance & Sandboxing Limits
- **Problem**: Runaway language servers, indexing loops, or recursive file walkers can consume host RAM, lock file handles, or peg CPU cores.
- **Architecture**:
  - Apply hard OS-level resource limits (memory ceilings, CPU affinity, file descriptor bounds) to all indexing and child processes.
  - Enforce strict per-command execution timeouts and bounded temp-directory allocations with automatic teardown guarantees.

#### 11. Protocol-Independent Knowledge Adapters
- **Problem**: Hardcoding Engram's engine to a single transport protocol (e.g. MCP over stdio) prevents integration with CLI tools, IDE extensions (LSP), and CI/CD pipelines.
- **Architecture**:
  - Maintain a clean architectural boundary between the core Knowledge Engine (`engram-core`) and presentation adapters (`engram-mcp`, `engram-lsp`, `engram-cli`, `engram-sdk`).
  - Provide a shared serialization-agnostic trait interface allowing identical retrieval semantics across all consumption surfaces.

#### 12. UTCP / Native Tool Execution Runtimes
- **Problem**: JSON-RPC over stdio introduces serialization overhead, multi-process orchestration complexity, and platform-specific IPC bugs (especially on Windows).
- **Architecture**:
  - Support Universal Tool Calling Protocol (UTCP) and in-process native dynamic linking (C-ABI / FFI) where supported by agent runtimes.
  - Enable sub-millisecond context retrieval and direct memory-mapped access to repository indices without intermediate process boundaries.

---

### Implementation Phasing

| Phase | Pillars Addressed | Primary Outcome |
| :--- | :--- | :--- |
| **Phase 2 (Immediate Post-5F)** | 2 (Evidence Cache), 6 (Log Summarization), 7 (Prompt Cache Stability) | Maximum KV cache hits and minimal diagnostic token waste |
| **Phase 3 (Agent Resilience)** | 5 (Delta Streaming), 8 (Retry Budgets), 9 (Loop Detection) | Elimination of runaway agent retry loops and context blowout |
| **Phase 4 (Enterprise Scale)** | 1 (Task Memories), 3 (Gitless Merkle Snapshots), 4 (Cross-Repo Graphs) | Enterprise monorepo and multi-repo support with persistent knowledge |
| **Phase 5 (Next-Gen Runtime)** | 10 (Governance), 11 (Protocol Adapters), 12 (Native UTCP) | Zero-overhead native agent integration with rock-solid sandboxing |
