# Phase 5.4 — Stage 2: Task B Auxiliary Context Payload Forensic Audit

**Document Status:** Sealed Forensic Investigation Report  
**Execution Date:** 2026-11-19  
**Investigator:** Principal Systems Architect & Code Auditor  
**Evaluation Scope:** Live Benchmark Task B (`task_b_remember_me_live_pair_01` & `task_b_remember_me_live_single_ctrl`)  
**Target Repository:** Engram (`feature/phase2-stage5-remediation`, HEAD `7106a2f`)  
**Candidate Evaluated:** Candidate 3 (`97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777`)

---

## 1. Executive Summary

In Phase 5.3 live benchmarking, Task B (`task_b_remember_me`) failed across both arms:
1. **Treatment B (`treatment_B`)** terminated with `TOKEN_CEILING_BREACHED` (`250,102 > 250,000` tokens) on Turn 3.
2. **Control A (`control_A`)** terminated with `PRE_DISPATCH_HEADROOM_EXHAUSTED` (headroom reserve 45,000 exhausted at Turn 5 with 222,063 cumulative tokens).

A preliminary observation in Phase 5.3 noted an ~199 KB payload. This Stage 2 forensic audit conducted deep JSON deserialization and schema-level accounting of all tool calls and context emissions.

### Primary Forensic Findings:
- **Catastrophic Unbounded Payload:** The actual auxiliary context returned to the model in Treatment B across 3 tool calls was **1,353,616 bytes (~338,404 tokens)**, with a single tool call returning **651,703 bytes (~162,925 tokens)**.
- **Root Cause Identified in `src/mcp.rs`:** While `engram_get_context` strictly enforces budget constraints via `BudgetPacker`, `engram_inspect_symbol` (**lines 450–520 of `src/mcp.rs`**) completely **bypasses any token budget guard**. It serializes the raw `ImpactCone` structure and an unconstrained narrative string (`format_impact_explanation`).
- **Hub-Component Graph Explosion:** For hub components such as `app.component.ts` (the Angular application root), depth-2 dependency traversal traversed external modules (e.g., `@angular/core`, `@angular/router`) and pulled in **119 project files** and **hundreds of cross-file import relationships**.
- **Redundancy Ratio:** Over **98.2%** of the payload consisted of transitive, external import relationships and redundant narrative explanations. The task-relevant code and contract data represented less than **1.8%** of the payload.
- **Token Budget Contract Violation:** The unbudgeted return in `engram_inspect_symbol` is a direct violation of the context boundary contracts established in Phase 4A–4D, which mandate bounded serialized outputs.

---

## 2. Audit of Auxiliary Context Injections Across Arms

### 2.1 Treatment B: MCP Tool Call Injections

In Treatment B (`task_b_remember_me_live_pair_01/treatment_B/events.jsonl`), the agent invoked `engram_inspect_symbol` 3 times:

| Call # | Target Symbol / File | Depth | Raw Response Bytes | Estimated Tokens (~4 B/tok) | Direct Entities | Import Dependent Entities | Files in Imports |
|:---:|:---|:---:|:---:|:---:|:---:|:---:|:---:|
| 1 | `auth.effects.ts` | 2 | 228,484 B | ~57,121 | 2 | 78 | 78 |
| 2 | `auth.actions.ts` | 2 | 203,590 B | ~50,897 | 1 | 75 | 75 |
| 3 | `app.component.ts` | 2 | 651,703 B | ~162,926 | 7 | 119 | 119 |
| **Total** | **3 calls** | — | **1,083,777 B** | **~270,944** | **10** | **272** | **272** |

*(Note: Including JSON-RPC envelope and formatting overhead, total serialized payload in events.jsonl exceeded 1.35 MB).*

### 2.2 Detailed Payload Content Breakdown

Deserializing the 651,703-byte payload of Call 3 (`app.component.ts`) reveals the exact compositional breakdown:

```
Total Payload: 651,703 bytes (100.0%)
│
├── 1. Impact Cone Object: 456,070 bytes (70.0%)
│   ├── direct_entities: 7 entities (3,842 bytes / 0.6%)
│   ├── caller_entities: 0 entities (2 bytes)
│   ├── test_entities: 3 entities (1,840 bytes / 0.3%)
│   └── import_dependent_entities: 119 entities (427,044 bytes / 65.5%)
│       ├── External library nodes (@angular/core, @angular/material, @ngrx/store, etc.)
│       └── Transitive project files sharing those external imports (each repeating full relationship arrays)
│
├── 2. Formatted Narrative Explanation: 174,550 bytes (26.8%)
│   └── Text string duplicating the entire graph structure in prose ("format_impact_explanation")
│
├── 3. Authorization Object: 8,966 bytes (1.4%)
│   └── Verification result against active contract
│
├── 4. Active Contract & Excerpt: 12,117 bytes (1.8%)
│   ├── code_excerpt & signature: 9,180 bytes
│   └── active_contract specification: 2,937 bytes
```

### 2.3 Control A: Baseline Shell Injection Analysis

In Control A (`task_b_remember_me_live_single_ctrl`), the agent used native PowerShell and `ripgrep` commands. It did not have Engram MCP tools. Its context accumulation was driven by broad file dumps and agent skill loads:

| Turn | Command Executed | Output Chars | Content Description |
|:---:|:---|:---:|:---|
| 1 | `Get-Content .../SKILL.md` (5 files) | 54,698 chars | Read global developer skills into prompt context |
| 2 | `rg -n "remember\|login\|..."` | 26,091 chars | Full repository regex search across all files |
| 3 | `Get-Content auth.actions.ts ...` | 9,964 chars | Cat auth module core files |
| 4 | `rg -n "authLogin..."` | 12,293 chars | Secondary grep across template files |
| 5 | `Get-Content app.component.ts ...` | 16,210 chars | Cat 6 components and spec files |
| **Total** | **5 shell turns** | **119,256 chars** | **Prompt Tokens Reached: 222,063** |

At Turn 5, Control A's prompt history reached 222,063 tokens. The harness headroom guard (`45,000` token reserve) detected that the next turn would breach the 250,000 token limit and preemptively halted execution (`PRE_DISPATCH_HEADROOM_EXHAUSTED`).

---

## 3. Root Cause Analysis: Why Task B Produced an Explosion

### 3.1 The Algorithmic Flaw in `src/mcp.rs`
In `src/mcp.rs` (lines 450–520), the handler for `engram_inspect_symbol` is written as:
```rust
let conn = db(&root)?;
let cone = crate::impact::compute_impact_cone(&conn, &root, target, depth)?;
let active_contract = crate::contract::get_active_contract(&root).ok().flatten();
let authorization = active_contract
    .as_ref()
    .map(|c| crate::contract::evaluate_impact_against_contract(c, &cone));
let explanation = crate::impact::format_impact_explanation(&cone);
...
Ok(json_result(json!({
    "target": cone.target_symbol,
    "target_file": cone.target_file,
    "line_range": line_range,
    "code_excerpt": code_excerpt,
    "completeness": completeness,
    "impact_cone": cone,              // <--- UNBOUNDED OBJECT
    "active_contract": active_contract,
    "authorization": authorization,
    "explanation": explanation,        // <--- UNBOUNDED PROSE DUPLICATE
    "memories": audit_memories,
    "memory_conflicts": memory_conflicts,
})))
```

Unlike `engram_get_context`, which passes data through `crate::budget::BudgetPacker` with strict token ceilings, truncation manifests, and signature contractions, `engram_inspect_symbol`:
1. Directly embeds the entire `cone: ImpactCone` without any pruning.
2. Directly embeds `explanation: String` which iterates over every entity in `cone` and writes descriptive sentences for each.

### 3.2 Transitive External Node Fan-Out
In `src/impact.rs`, when `depth >= 2`, dependency traversal searches for entities that import the target or entities the target imports.
Because `app.component.ts` imports `@angular/core`, the traversal identifies `@angular/core` as an external dependency at hop 1. At hop 2, the graph queries all files in the repository that also import `@angular/core`.
In an Angular workspace, **nearly every single file imports `@angular/core`**.
Consequently, at depth 2:
- 119 internal project entities were classified as `IMPORT_DEPENDENT`.
- Each entity was serialized with its full array of hop-1 and hop-2 relationships.
- Total size of `import_dependent_entities`: **427,044 bytes**.

---

## 4. Determinism Evaluation

- **Codebase Indexing:** The SQLite database index is strictly deterministic for a fixed Git tree commit. Query results from `compute_impact_cone` return identical entity sets across repeated runs on the same commit.
- **Model Invocation Non-Determinism:** The decision of *which* tool to invoke, *what depth* to request, and *in what order* depends on the LLM's stochastic generation:
  - In Treatment B, the model elected to call `engram_inspect_symbol` with default depth 2 on three files sequentially.
  - Because depth 2 on `app.component.ts` deterministically outputs ~651 KB, whenever an agent inspects a hub component with depth 2, a >600 KB payload is **100% guaranteed** under the current Candidate 3 binary.

---

## 5. Ratio of Task-Relevant to Redundant Context

To complete Task B (implementing the "Remember Me" checkbox on login), the agent needed:
1. State definition in `auth.models.ts` and `auth.reducer.ts` (~80 lines, ~2 KB).
2. Action definition in `auth.actions.ts` (~30 lines, ~1 KB).
3. Effect handling in `auth.effects.ts` (~50 lines, ~1.5 KB).
4. Template markup in `app.component.html` (~20 lines, ~0.8 KB).
5. Component binding in `app.component.ts` (~30 lines, ~1 KB).
Total relevant context needed: **~6.3 KB (~1,600 tokens)**.

Payload Delivered by `engram_inspect_symbol` on `app.component.ts`:
- Total: **651,703 bytes (~162,925 tokens)**.
- Task-relevant excerpt & contract: **12,117 bytes (~3,029 tokens)** (1.86%).
- Redundant / Irrelevant AST graph & prose: **639,586 bytes (~159,896 tokens)** (98.14%).

**Ratio of Redundant to Useful Data: 52.8 to 1.**

---

## 6. Token Budget Contract Violation Assessment

| Contract / Specification | Defined Standard | Actual Behaviour in Candidate 3 | Compliance Status |
|:---|:---|:---|:---:|
| **MCP Tool Result Ceiling** | Single tool response must fit within bounded context (typically $\le 8,000$ tokens). | `engram_inspect_symbol` returned 162,925 tokens in a single JSON-RPC response. | **FAILED (VIOLATED)** |
| **BudgetPacker Bounding** | All context injection must route through deterministic packing with omission manifests. | `engram_inspect_symbol` does not invoke `BudgetPacker`; raw structs are serialized directly. | **FAILED (VIOLATED)** |
| **Transitive Graph Pruning** | External library dependencies should not pull the entire workspace into an impact cone. | Depth-2 search on external module nodes fan out to all importing files in the workspace. | **FAILED (DEFECT)** |
| **Duplication Prohibition** | Information must not be duplicated within the same payload. | Graph is emitted twice: once as structured JSON (`impact_cone`) and once as prose (`explanation`). | **FAILED (DEFECT)** |

---

## 7. Conclusions and Mandatory Remediation

1. **Candidate 3 possesses an unmitigated context-inflation defect in `engram_inspect_symbol`:**
   Any call to `engram_inspect_symbol` on a root or shared component produces a response large enough to consume 65% of the total session token budget in a single turn.
2. **Remediation is technically straightforward and localized:**
   - In `src/mcp.rs`: Apply budget bounding to `engram_inspect_symbol`.
   - In `src/impact.rs`:
     - Cap `import_dependent_entities` (e.g., top $N \le 10$ relevant entities, omit external transitive fan-out).
     - Prune relationship arrays for external nodes.
     - Condense or eliminate narrative `explanation` when structured JSON is present (or gate narrative behind an explicit flag).
   - Enforce an upper bound on single MCP response size (e.g., max 4,000 tokens / 16 KB) with a clear omission manifest.

*Document finalized and ready for Stage 3.*
