# Engram Controlled Token Efficiency Experiment
# Formal Experiment Preregistration Document

**Preregistration Protocol ID:** `ENGRAM-PREREG-2026-M5`  
**Date of Freeze:** 2026-10-30  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Repository Branch:** `feature/async-precommit-guard`  
**Baseline Git HEAD:** `c70845b4d5913457f1b176fb0370b80dd792711f`  
**Candidate Binary SHA-256:** `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`  
**Governance State:** **CONCEPTUALLY FROZEN — AWAITING EXPLICIT OWNER AUTHORIZATION**  

---

### 1. Central Hypothesis

$$\mathbf{H_1}: \text{Providing Engram-compiled engineering context reduces total AI-agent model token consumption per successfully completed engineering task, compared to an identical coding-agent workflow without Engram.}$$

$$\mathbf{H_0}: \text{Total AI-agent model token consumption per successfully completed task with Engram-compiled context is greater than or equal to that of an unassisted coding-agent workflow.}$$

---

### 2. Experimental Task Specification

- **Task Identifier:** `task_commit_cache_invalidation`
- **Task Title:** Commit-Bound AST Cache Invalidation and Stale Index Cleanup
- **Target Repository:** Engram (`C:\Users\Amal P S\OneDrive\Projects\Engram`)
- **Authorized Source Files:**
  - `src/compiler.rs`
  - `src/analyzer.rs`
  - `src/main.rs`
  - `tests/commit_cache_invalidation_test.rs`
- **Task Requirements:**
  1. Record active Git commit HEAD hash in a `meta` table in SQLite `.engram/index.db`.
  2. In `ensure_fresh()`, detect whether active commit HEAD matches the cached hash; invalidate stale AST records on drift.
  3. Implement `engram clean --cache` CLI subcommand to purge AST SQLite caches while preserving user decision memories (`.engram/memories.json`).
  4. Pass all automated verification checks without breaking existing compiler tests.

---

### 3. Experimental Arm Definitions

#### Arm A — Baseline (Control)
- **Prompt:** Task specification and acceptance criteria only.
- **Context:** Zero Engram-compiled context.
- **Environment:** Clean Git worktree, standard shell tools (`Get-Content`, `rg`, `ls`, file editing).
- **Tools:** Standard coding-agent capabilities; **zero Engram MCP tools**; **zero Turn-0 directives**.

#### Arm B — Engram-Assisted (Primary Treatment)
- **Prompt:** Identical task specification, appended with an AST-grounded Engram Context Package compiled via:
  ```powershell
  engram prompt --subpath src/ --language rust --token-budget 2500 "<task_prompt>"
  ```
- **Context Package:** Function signatures, type definitions, SQLite access patterns in `analyzer.rs` and `compiler.rs`, test locations, and context completeness metadata.
- **Environment:** Clean Git worktree in isolated workspace.
- **Tools:** **100% identical to Arm A** (same shell, same editor, same file tools; no background MCP daemon).

---

### 4. Model and Harness Configuration

- **Model Identifier:** `gpt-5.6-luna` (or authorized equivalent model)
- **Reasoning Effort:** `medium`
- **Sampling Temperature:** Default deterministic setting
- **Dependency Strategy:** Isolated workspace clone / worktree
- **Pre-Warming:** SQLite index pre-warmed offline prior to timing

---

### 5. Authoritative Token Accounting Specification

All token metrics are extracted directly from provider-reported `usage` objects:

1. **Total Task Model Tokens ($T_{\text{task}}$):**
   $$T_{\text{task}} = \sum_{t=1}^{K} (\text{prompt\_tokens}_t + \text{completion\_tokens}_t)$$
2. **Double-Counting Invariant:**
   - `prompt_tokens_details.cached_tokens` is a *subset* of `prompt_tokens`, not an addition.
   - `completion_tokens_details.reasoning_tokens` is a *subset* of `completion_tokens`, not an addition.
3. **Local Preprocessing:**
   - Engram local compilation runs 100% offline ($0$ model tokens).
   - CPU time and indexing wall-clock duration are reported in a separate local overhead table.

---

### 6. Token Ceiling, Reserve, and Stopping Policy

| Parameter | Value | Enforcement Mechanism |
| :--- | :---: | :--- |
| **Nominal Stop Threshold** | **140,000 tokens** | Pre-dispatch guard halts next turn locally before API call. |
| **Reserve Headroom** | **35,000 tokens** | Sized to absorb worst-case single-turn context compounding. |
| **Hard Ceiling Per Arm** | **175,000 tokens** | External monitor kills process via Windows Job Object. |
| **Combined Task Budget** | **350,000 tokens** | Maximum theoretical exposure across both arms. |
| **Per-Turn Timeout** | **300 seconds** | Halts hung subprocess. |
| **Per-Arm Total Timeout** | **1,200 seconds** | Wall-clock execution ceiling. |

---

### 7. Five-Gate Functional Acceptance Battery

A run is certified successful ($C = 1.00$) if and only if all five gates pass:
- **Gate 0 (Build):** `cargo check --offline` exits code 0 with 0 errors.
- **Gate 1 (Regression):** `cargo test --offline --lib compiler::tests` and `cargo test --offline --test scoped_query_routing_test` pass 100%.
- **Gate 2 (Functional):** `cargo test --offline --test commit_cache_invalidation_test` passes all requirements.
- **Gate 3 (Boundaries):** Edits strictly confined to authorized files.
- **Gate 4 (Blinded Review):** Anonymized patch approved for architectural idiomaticness and absence of test-gaming hacks.

$$\mathbf{\text{Any run with }} C < 1.00 \mathbf{\text{ is classified as a FAILURE, not a token saving.}}$$

---

### 8. Statistical Analysis Plan

- **Design:** Exploratory matched-pair comparison ($N = 3$ pairs, 6 total executions).
- **Primary Metric:** Paired percentage savings rate on accepted pairs:
  $$\Delta\%_i = \frac{T_{\text{baseline}, i} - T_{\text{engram}, i}}{T_{\text{baseline}, i}} \times 100\%$$
- **Completion Supremacy:** If treatment completion rate is lower than baseline ($\text{CR}_{\text{engram}} < \text{CR}_{\text{baseline}}$), the result is formally classified as a **REGRESSION**.
- **Counterbalancing Schedule:**
  - Pair 1: Arm A (Baseline) $\rightarrow$ Arm B (Engram)
  - Pair 2: Arm B (Engram) $\rightarrow$ Arm A (Baseline)
  - Pair 3: Arm A (Baseline) $\rightarrow$ Arm B (Engram)

---

### 9. Protocol Amendment Policy

This preregistration is frozen as of 2026-10-30. 
- No experimental parameter (thresholds, tasks, prompts, acceptance gates, analysis formulas) may be altered during or after live execution.
- Any necessary post-freeze modification must be recorded as a formal, numbered protocol amendment (`amendments/draft-05/protocol-amendments/`) prior to execution.

---

### 10. Owner Authorization Gate

```
================================================================================
                        MILESTONE 5A AUTHORIZATION GATE
================================================================================

Milestone 5A is strictly an offline preparation and preregistration phase.
Zero live model inference, token-consuming benchmarks, or external API requests
were authorized or executed during this milestone.

Proceeding to live execution (Milestone 5B) requires explicit owner sign-off on:
1. Billing checklist verification.
2. Token exposure ceiling authorization (350,000 tokens maximum).
3. Matched-pair harness adaptation for Cargo repositories.

[ ] LIVE EXPERIMENT (MILESTONE 5B) AUTHORIZED BY REPOSITORY OWNER

Signature: _________________________________________  Date: ____________________
================================================================================
```
