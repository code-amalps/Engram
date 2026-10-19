# Engram Efficiency-First Architecture & Competitive Differentiation Roadmap

**Document Identifier:** `ENGRAM-EFFICIENCY-FIRST-ROADMAP-20261018`
**Current Baseline Commit:** `17fdf4a` (`origin/main`, `origin/Development`)
**Engine Version:** `v1.5.1`
**Architectural Objective:** Make **Measurable End-to-End Task Token Efficiency** a First-Class Competitive Objective (targeting up to 80% reduction in cumulative task-level tokens without sacrificing correctness, scope safety, or test quality).
**Status:** ARCHITECTURE DESIGN & AUDIT (Phase 0 Planning — Implementation Deferred)

---

## 1. Executive Summary & Problem Diagnosis

Engram’s core competitive mission is to provide AI coding agents with **persistent engineering memory and change intelligence**—ensuring agents know what should change, know what actually changed, and remember why.

In `v1.5.1`, Fix A successfully optimized local MCP response payloads by **97.5%** (reducing tool outputs from $\sim 34\text{ KB}$ down to $806\text{--}986\text{ bytes}$), while Fix B and Fix C established SHA-256 preflight fingerprinting and scope-drift enforcement.

However, real-world A/B validation in GitHub Copilot Agent Mode revealed a critical systemic bottleneck:

> [!CAUTION]
> ### The "MCP Interaction Tax" Problem
> Even with compact 850-byte tool responses, running multi-turn interactive tool calls (`get_context` $\to$ `preflight` $\to$ `preflight (dirty)` $\to$ `postflight`) caused:
> 1. **Conversational Turn Inflation:** Task-level agent turns increased by **+43% to +57%** (from 7 to 11 turns on Task B; from 1 to 3 turns on Task A).
> 2. **Cumulative History Multiplication:** In multi-turn LLM chat, the primary model re-reads the entire accumulated dialogue history on every single turn:
>    $$\text{Cumulative Task Input Tokens} = \sum_{t=1}^T \text{Context}_t \approx T \times \overline{\text{Context}}$$
>    Adding 4 extra conversational turns in an 8,000-token context burns **$\sim 32,000$ extra input tokens**, completely overwhelming the 8,000-token savings from compact payloads.
> 3. **Auxiliary Wrapper Surge:** In VS Code Copilot, every tool call triggers a safety/summarization call (`copilotLanguageModelWrapper`, `gpt-4o-mini`) using a 1,634-token template, generating $+5,541$ auxiliary tokens.
> 4. **User-Visible Credit Burn:** Because GitHub Copilot bills per turn/request at the account level, sessions requiring extra tool turns consume billable credits faster.

**Conclusion:** Local wire payload optimization alone cannot deliver end-to-end efficiency. Engram must evolve into an **Efficiency-First Task Planner** that eliminates unnecessary conversational turns entirely through **Turn-0 static context injection**, **hook-based asynchronous validation**, and **adaptive small-task bypass**.

---

## 2. Token & Billing Taxonomy: Separating the Three Tiers

To prevent confusion and false cost claims, Engram establishes strict accounting boundaries across three distinct tiers:

```
┌────────────────────────────────────────────────────────────────────────┐
│ TIER 1: Wire / Tool Response Payload Tokens                            │
│ Local bytes returned by MCP tools (`preflight`, `get_context`).        │
│ Measured: Reduced from 34 KB to 2.6 KB (-97.5%).                       │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ Multiplied across turns
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ TIER 2: Total Task-Level Cumulative Model Tokens                       │
│ Sum of (Prompt + History + Schemas + Tool Results + Output) over all T │
│ turns for both Primary Model (Claude 3.5 / GPT-4o) and Auxiliary Model │
│ (gpt-4o-mini wrapper).                                                 │
│ Measured: Increased from ~11.5k to ~17k auxiliary tokens (+48%) due    │
│ to T increasing from 7 to 11 turns.                                    │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ Billed by provider
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ TIER 3: Account-Level Billed Usage / AI Credits / Premium Requests    │
│ Deductions from user quotas (e.g. 23.7 / 200 used). Withheld by local  │
│ client logs. Billed per turn/request, not by wire bytes.               │
│ Status: UNPROVEN / UNVERIFIED from client logs alone.                  │
└────────────────────────────────────────────────────────────────────────┘
```

### Scientific Rule:
- **Never claim Tier 3 savings based solely on Tier 1 payload reductions.**
- **Never sum auxiliary wrapper tokens (0.00 AIC) with primary orchestrator tokens into a single fictitious number.**
- **Claims must explicitly report Tier 1 (bytes), Tier 2 (cumulative tokens & turns), and Tier 3 (attributable credits) separately.**

---

## 3. Inventory of Existing Engram Capabilities (Reuse Before Rebuilding)

Engram already possesses high-performance Rust components that can be leveraged without architectural rewrites:

| Module | Existing Capability | Efficiency-First Reuse Opportunity | Refactoring Required? |
|---|---|---|:---:|
| [`src/analyzer.rs`](file:///c:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/analyzer.rs) | Tree-sitter AST parsing across 7 languages | Reused for offline AST extraction and symbol dependency indexing. | **No** (Stable) |
| [`src/graph.rs`](file:///c:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/graph.rs) | Entity graph, bi-temporal versions in SQLite WAL | Reused for topological blast-radius and symbol relevance queries. | **No** (Stable) |
| [`src/incremental.rs`](file:///c:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/incremental.rs) | Two-tier invalidation (Tier 1 SHA-256 + Tier 2 relink) | Ensures cold-start cost is paid once; warm turns execute in $\le 2\text{ ms}$. | **No** (Stable) |
| [`src/contract.rs`](file:///c:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/contract.rs) | Contract lifecycle, monorepo prefix stripping, scope-drift | Validates working-tree diff against approved boundaries. | **Minimal** (Export contract to file) |
| [`src/flywheel.rs`](file:///c:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/flywheel.rs) | SHA-256 preflight fingerprinting (`task_goal` + `base_commit` + `constitution` + `forbidden` + `planned_files`) | Reused for sub-millisecond contract re-validation. | **No** (Stable) |
| [`src/guard.rs`](file:///c:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/guard.rs) | Scope-drift detection (bulk i18n $\ge 3$ files, unrequested UI dialogs) | Core boundary guard ensuring agents don't sprawl into unrelated code. | **No** (Stable) |
| [`src/memory.rs`](file:///c:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/memory.rs) & [`src/drift.rs`](file:///c:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/drift.rs) | Scoped staleness tracking, explicit human ratification lifecycle | Prevents stale memory retrieval from poisoning context. | **No** (Stable) |
| [`src/instructions.rs`](file:///c:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/instructions.rs) | Workspace instructions generation (`.github/copilot-instructions.md`) | **Primary leverage point:** Inject pre-session context and rules to eliminate Turn-1 & Turn-2 MCP calls. | **YES (Major)** |
| [`src/prompt.rs`](file:///c:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/prompt.rs) | BM25 lexical grounding + token budget slicing | Enforce strict user-configurable token caps ($\le 500$ tokens for small, $\le 1,200$ for complex). | **YES (Refactor budget slicing)** |
| [`src/mcp.rs`](file:///c:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/mcp.rs) | JSON-RPC 2.0 MCP server with compact default responses | Consolidate `get_context` and `preflight` into an optional atomic call. | **YES (Add single-shot endpoint)** |
| [`src/ci_guard.rs`](file:///c:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/ci_guard.rs) | Pre-merge CLI / CI guard | Extend to serve as a local Git pre-commit hook, shifting postflight out of conversation. | **YES (Hook integration)** |

---

## 4. Efficiency-First Task Planner Architecture

```text
 ┌─────────────────────────────────────────────────────────────────────────────┐
 │                         EFFICIENCY-FIRST TASK PLANNER                       │
 ├─────────────────────────────────────────────────────────────────────────────┤
 │                                                                             │
 │  1. ADAPTIVE TASK CLASSIFIER (Zero-Cost Bypass)                             │
 │     • Evaluates task complexity from initial prompt / staged diff.          │
 │     • Trivial tasks (single file typo/config) → 100% BYPASS of MCP loop.     │
 │                                                                             │
 │  2. TURN-0 STATIC CONTEXT INJECTION (Turn Elimination)                      │
 │     • Pre-computes exact AST symbols, dependencies, and boundary rules.     │
 │     • Injects context into workspace instructions / prompt preamble.        │
 │     • Eliminates `get_context` and initial `preflight` MCP calls (-2 turns). │
 │                                                                             │
 │  3. TOKEN-BUDGETED GRAPH RETRIEVAL                                          │
 │     • Hard token ceiling: 500 tokens (Trivial/Small), 1,200 tokens (Complex)│
 │     • 1-hop direct dependencies prioritized over speculative 2-hop trees.   │
 │                                                                             │
 │  4. CHANGE CONTRACT & SCOPE-DRIFT ENFORCEMENT                               │
 │     • Maintains strict blast-radius containment (e.g. max 18 files).        │
 │     • SHA-256 fingerprinting ensures deterministic contract reuse.          │
 │                                                                             │
 │  5. ASYNCHRONOUS HOOK-BASED POSTFLIGHT                                      │
 │     • Postflight runs as a local Git pre-commit hook or file-save watcher.  │
 │     • If changes match contract → Exit 0 silently (0 model turns burned).   │
 │     • If breach detected → Injects surgical recovery diff only then.        │
 │                                                                             │
 └─────────────────────────────────────────────────────────────────────────────┘
```

### Key Architectural Invariants:
1. **Zero-Turn Context Invariant:** An agent must never be forced to make a conversational tool call solely to learn what the repository rules, boundaries, and primary dependencies are. That knowledge must be provided at Turn 0.
2. **Turn-Count Invariant:** The total conversational turns $T$ for an Engram-guided task must not exceed the unguided baseline by more than $+1$ turn (and on complex tasks, should be equal or fewer due to reduced exploration).
3. **Adaptive Routing Invariant:** Trivial single-file modifications must never run multi-stage graph traversals or handshakes.
4. **Local-First & Free Invariant:** The architecture requires zero paid third-party APIs, zero hosted vector databases, and zero network calls. All graph queries, AST parsing, and fingerprinting execute in local native Rust.

---

## 5. Comparative Evaluation: Engram vs. Graphify

| Capability | Graphify | Engram (Current v1.5.1) | Engram (Efficiency-First) |
|---|---|---|---|
| **Core Paradigm** | Knowledge graph extraction & visual exploration | Bi-temporal engineering memory & change boundary guard | Unified turn-optimized task planner & change guard |
| **Primary Artifacts** | `graph.json`, `graph.html`, god nodes, communities | `.engram/index.db`, `memory.yaml`, append-only audit events | Injected instructions, active contract, `.engram/events` |
| **Context Delivery** | Manual querying via CLI or god-node inspection | Interactive MCP tools (`get_context`, `preflight`) | **Turn-0 static injection + single-shot MCP** |
| **Change Boundaries** | None (Analysis only) | Contract-based blast radius & drift enforcement | Contract-based blast radius & drift enforcement |
| **MCP Interaction Tax** | Depends on agent exploration turns | **High (+4 conversational turns)** | **Near-Zero (0 to 1 turn maximum)** |
| **Local-First & Free** | Yes (Python/AST) | Yes (Rust native binary) | Yes (Rust native binary) |

### Synergy and Demarcation:
- **Demarcation:** Graphify excels at macro-architecture discovery (identifying god nodes, circular clusters, and global topological structure). Engram excels at micro-change governance (binding a specific engineering task to a baseline commit, preventing file sprawl, and verifying AST-level contract boundaries).
- **Synergy:** When `graphify-out/` is present, Engram can consume community boundaries as high-level architectural domains for its constitution rules. However, **Engram does not require Graphify to operate**.

---

## 6. Reproducible 5-Arm Benchmark Protocol

To evaluate the efficiency-first architecture rigorously without bias or manufactured results:

### The 5 Arms
- **Arm 1 (Baseline):** Copilot Agent without Engram.
- **Arm 2 (Current Engram):** `v1.5.1` Release binary (compact responses, interactive MCP).
- **Arm 3 (Optimized Engram):** Efficiency-First Task Planner (Turn-0 injection, hook validation, adaptive bypass).
- **Arm 4 (Graphify Alone):** Copilot Agent supplied with Graphify macro-graph context.
- **Arm 5 (Graphify + Optimized Engram):** Graphify community clustering + Engram boundary enforcement.

### Staged Execution Sequence
- **Stage 1 (Core Triad):** Run Arms 1, 2, and 3 across Task A (trivial typo) and Task B (remember-me feature) with 5 matched repetitions each (30 runs total).
- **Stage 2 (Graphify Integration):** Execute Arms 4 and 5 on equivalent snapshots once Arm 3 demonstrates statistically significant turn and token reductions.

### Metrics Measured & Accounting Rule
| Metric | Measurement Source | Unit | Accounting Treatment |
|---|---|:---:|---|
| **Primary Model Tokens** | Host API (where observable) / Estimated | Tokens | Reported separately. Never mixed with auxiliary. |
| **Auxiliary Tokens** | `copilotLanguageModelWrapper_*.copilotmd` | Tokens | Authoritative measured (`gpt-4o-mini`, 0.00 AIC). |
| **Conversational Turns ($T$)**| Chat transcript | Turns | Direct measure of interactive round-trips. |
| **MCP Invocations** | `.engram/events/*.json` | Calls | Exact count of preflight/context/postflight calls. |
| **Changed Files (Blast Radius)**| `git status --short` | Files | Measure of sprawl containment. |
| **Correctness & Tests** | `verify_task_a.ps1` / `verify_task_b.ps1` | Pass/Fail | Quality gate: zero regressions allowed. |
| **Cold-Start vs Warm Cost** | Wall-clock timer | Seconds | Amortize indexing across multiple runs. |
| **Attributable Billed Credits** | Account billing dashboard (quiet window) | Credits | Reported as observable account deltas. |

---

## 7. Quality Gates: Rejecting Harmful Token Cuts

Token reduction must never be achieved by degrading engineering quality:

> [!IMPORTANT]
> ### The 5 Non-Negotiable Quality Gates
> 1. **Zero Context Starvation:** Context reduction must not remove files or symbols necessary for compilation or test resolution. If the agent fails a task due to missing context, the optimization is rejected.
> 2. **Zero Test Elimination:** Optimizations that skip unit test execution or suppress compiler warnings are strictly prohibited.
> 3. **Zero Security & Safety Bypasses:** Constitution rules, forbidden directories (e.g. `secrets/**`), and scope-drift checks (e.g. bulk i18n, unrequested UI dialogs) remain 100% active.
> 4. **No Stale Invalidation:** Invalidation rules must remain authoritative (`SHA-256` content hash beats `mtime`). Engram will never reuse stale context merely to claim token savings.
> 5. **Traceability Invariant:** Every event must continue to write an append-only SHA-256 audit record in `.engram/events/`.

---

## 8. Ranked Optimization Opportunities (Savings-to-Complexity Ratio)

We rank all proposed efficiency improvements by expected task-level token savings, implementation effort, regression risk, and confidence:

| Rank | Optimization Initiative | Target Mechanism | Expected Token Savings | Implementation Complexity | Regression Risk | Confidence Level |
|:---:|---|---|:---:|:---:|:---:|:---:|
| **1** | **Turn-0 Static Context & Instruction Injection** | Pre-compute graph context & boundaries into `.github/copilot-instructions.md` | **35% to 50% total task tokens** (-2 turns, -3.2k aux tokens, -16k primary history re-reads) | **LOW** (Extend `instructions.rs` + CLI command) | **VERY LOW** | **VERY HIGH** |
| **2** | **Adaptive Small-Task Bypass** | Heuristic classifier detects trivial single-file fixes and bypasses MCP handshakes | **60% to 70% on small tasks** (-2 turns on Task A) | **LOW** (Pattern match in CLI/extension) | **LOW** | **HIGH** |
| **3** | **Asynchronous Hook-Based Postflight** | Shift postflight diff validation from chat tool to local Git pre-commit hook | **15% to 25% total task tokens** (-1 turn on clean runs) | **MEDIUM** (Git hook runner integration) | **LOW** | **HIGH** |
| **4** | **Atomic Tool Consolidation** | Combine `get_context` and `preflight` into an atomic single-shot tool | **10% to 15% tokens** (Saves 1 turn if MCP used) | **LOW** (New MCP schema in `src/mcp.rs`) | **VERY LOW** | **HIGH** |
| **5** | **Strict Context Budget Slicing** | Enforce hard 500-token cap on symbol context in `src/prompt.rs` | **5% to 10% payload tokens** | **LOW** (Truncation tuning) | **MEDIUM** (Risk of context starvation) | **MEDIUM** |
| **6** | **Graphify Community Boundary Ingestion** | Ingest `graphify-out/` clusters to automatically seed module boundaries | **Variable** (Improves boundary accuracy on large repos) | **HIGH** (Cross-system JSON parser & linker) | **MEDIUM** | **MEDIUM** |

---

## 9. Implementation Sequence

The roadmap is structured into four sequential, rollback-safe phases:

```
┌────────────────────────────────────────────────────────────────────────┐
│ PHASE 1: Turn-0 Context Injection & Adaptive Small-Task Bypass        │
│ • Implement `engram inject --task "<prompt>"` CLI command.             │
│ • Update `instructions.rs` to output pre-session context block.        │
│ • Implement adaptive bypass for trivial single-line changes.           │
│ • Benchmark Arm 1 vs Arm 2 vs Arm 3 on Task A & Task B.                │
├────────────────────────────────────────────────────────────────────────┤
│ PHASE 2: Asynchronous Pre-Commit Hook Gating                          │
│ • Package `engram hook pre-commit` for local Git hook installation.    │
│ • Validate postflight silently on commit; surface errors only on breach│
│ • Cut postflight conversational turn out of the chat loop.             │
├────────────────────────────────────────────────────────────────────────┤
│ PHASE 3: Single-Shot MCP Tool Consolidation                           │
│ • Introduce atomic `engram_prepare_task` tool combining context + pre. │
│ • Maintain full backward compatibility for existing MCP consumers.     │
├────────────────────────────────────────────────────────────────────────┤
│ PHASE 4: Optional Graphify Community Ingestion                        │
│ • Detect `graphify-out/` and import module cluster boundaries.         │
│ • Execute benchmark Arms 4 and 5.                                      │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 10. Decision Gate: Recommended Minimum Viable First Iteration

### Recommendation
**Execute Iteration 1: Turn-0 Static Context & Instruction Injection.**

### Justification of Best Savings-to-Complexity Ratio:
1. **Directly Solves the Root Cause:** Eliminates the 2 conversational tool turns that currently trigger Copilot’s "MCP Interaction Tax" at the start of every session.
2. **Immediate Measurable Impact:**
   - Cuts primary model cumulative history re-reads by **$\sim 16,000$ to $24,000$ input tokens** per multi-file task.
   - Cuts auxiliary `copilotLanguageModelWrapper` calls by **2 to 3 calls** (saving $\sim 3,300$ to $4,950$ auxiliary tokens).
   - Reduces billable user turn count by **2 requests** per session.
3. **Minimal Implementation Surface:**
   - Requires zero changes to SQLite storage, Tree-sitter parsers, or AST graph structures.
   - Implemented via a targeted enhancement to [`src/instructions.rs`](file:///c:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/instructions.rs) and a lightweight CLI command (`engram prepare --task "<goal>"`).
4. **Zero-Risk Rollback Strategy:**
   - Does not alter existing MCP tool signatures (`get_context`, `preflight`, `postflight` remain fully operational for legacy or fallback flows).
   - If instructions are omitted, behavior smoothly falls back to standard `v1.5.1` behavior.

### Evidence Required to Proceed to Iteration 2:
Before advancing to Phase 2 (Pre-Commit Hook Gating), Iteration 1 must demonstrate:
1. Exact pass on automated acceptance checks: [`scripts/verify_task_a.ps1`](file:///c:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/scripts/verify_task_a.ps1) and [`scripts/verify_task_b.ps1`](file:///c:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/scripts/verify_task_b.ps1) (100% success rate).
2. Reduction of Task B conversational turns from **11 turns down to $\le 8$ turns**.
3. Measurable reduction of auxiliary wrapper tokens from **17,075 down to $\le 12,500$ tokens**.
4. Zero regressions across existing Rust test suites (`cargo test`).

---

*Planning and audit complete. Awaiting user authorization before initiating implementation of Iteration 1.*
