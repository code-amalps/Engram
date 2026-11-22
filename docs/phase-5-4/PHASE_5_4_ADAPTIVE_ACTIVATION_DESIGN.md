# Phase 5.4 — Stage 4: Complexity-Aware Adaptive Activation Design

**Document Status:** Sealed Architectural Design Specification  
**Execution Date:** 2026-11-19  
**Architect:** Principal Software Architect & Systems Engineer  
**Target Repository:** Engram (`feature/phase2-stage5-remediation`, HEAD `7106a2f`)  
**Scope:** Adaptive Context Optimization, Complexity Classification, and 3-Arm Benchmark Design

---

## 1. Executive Summary & Core Architectural Dilemma

Live empirical benchmarking across Phase 5.3 and Stages 1–3 revealed the fundamental trade-off of LLM context optimization:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           THE ENGRAM PARADOX                                │
├─────────────────────────────────────────────────────────────────────────────┤
│ 1. SIMPLE TASKS (e.g. Task A: Single-line typo fix)                        │
│    • Context required: < 50 tokens.                                         │
│    • Engram overhead: 447 tokens tool schema + preflight turn requirements. │
│    • Outcome: Net token regression (-14.22%) and turn-count inflation.      │
│                                                                             │
│ 2. COMPLEX TASKS (e.g. Task B: Multi-file Angular feature)                 │
│    • Context required: ~1,600 tokens across 5 files.                        │
│    • Native baseline: 119,000 chars of skill & repo dumps; headroom died.   │
│    • Engram unpruned: 651 KB unbudgeted graph dump; ceiling breached.       │
│    • Target State: Pruned, bounded Engram saves ~40,000 tokens per turn!    │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Conclusion:** A static "Always-On" tool architecture is fundamentally sub-optimal. For simple targeted tasks, any auxiliary tool surface creates turn-count and prompt inflation. For complex architectural tasks, bounded context retrieval is essential to prevent context exhaustion.

**Solution:** Implement **Complexity-Aware Adaptive Activation** that routes tasks to appropriate operating modes before session dispatch.

---

## 2. Task Complexity Classification Model

### 2.1 Complexity Tiers

| Tier | Category | Operational Definition | Example |
|:---:|:---|:---|:---|
| **Tier 1** | `SIMPLE` | Single-file targeted modification, typo fix, isolated renaming, or localized unit-test patch where file path and location are known or narrow. | Task A (`typo in login menu`) |
| **Tier 2** | `MEDIUM` | Scoped multi-file change within a single directory or subsystem (2–3 closely coupled files; e.g. updating an action and its reducer). | Adding an enum member and switch case |
| **Tier 3** | `COMPLEX` | Cross-cutting architectural feature requiring state management, effects, service integration, and UI component template binding across $\ge 4$ files. | Task B (`remember_me feature`) |

### 2.2 Input Signals & Fast Decision Heuristic
The classification runs in $< 5\text{ ms}$ at session initialization (Turn 0) using deterministic lexical and AST signals:

```python
def classify_task_complexity(task_prompt: str, repo_index: Optional[sqlite3.Connection] = None) -> str:
    prompt_lower = task_prompt.lower()
    
    # 1. Negative / Simple Scope Triggers (Tier 1)
    simple_keywords = ["typo", "fix typo", "spelling", "rename symbol", "single line", "off-by-one"]
    has_simple_kw = any(kw in prompt_lower for kw in simple_keywords)
    
    # Extract candidate file paths mentioned in prompt
    import re
    file_matches = re.findall(r'[\w\-\.\/]+\.(?:ts|js|rs|py|html|css|json)', prompt_lower)
    unique_files = set(file_matches)
    
    # Tier 1 Condition: Explicit simple keywords OR exactly 1 file mentioned without feature expansion
    if has_simple_kw or (len(unique_files) == 1 and not any(k in prompt_lower for k in ["feature", "implement", "integrate", "store", "state", "effect"])):
        return "SIMPLE"
        
    # 2. Complex / Cross-Cutting Triggers (Tier 3)
    complex_keywords = [
        "feature", "implement", "architecture", "cross-cutting", "refactor",
        "state management", "ngrx", "redux", "effect", "effects", "store",
        "remember me", "theme toggle", "full-stack", "authentication"
    ]
    has_complex_kw = any(kw in prompt_lower for kw in complex_keywords)
    
    if len(unique_files) >= 3 or has_complex_kw:
        return "COMPLEX"
        
    return "MEDIUM"
```

---

## 3. Activation Policy Formal Specification

### 3.1 Three Operational Modes

```
                    ┌─────────────────────────┐
                    │  Session Task Prompt    │
                    └───────────┬─────────────┘
                                │
                    [ Complexity Classifier ]
                                │
         ┌──────────────────────┼──────────────────────┐
         ▼                      ▼                      ▼
    [ SIMPLE ]             [ MEDIUM ]             [ COMPLEX ]
         │                      │                      │
         ▼                      ▼                      ▼
   ┌───────────┐          ┌───────────┐          ┌───────────┐
   │  BYPASS   │          │   LEAN    │          │   FULL    │
   │   MODE    │          │   MODE    │          │   MODE    │
   └─────┬─────┘          └─────┬─────┘          └─────┬─────┘
         │                      │                      │
         ▼                      ▼                      ▼
  • 0 Engram Tools       • 2 Read-Only Tools    • 4 Tools (Bounded)
  • Pure Native          • Token Budget: 1,200  • Token Budget: 2,000
  • Zero Overhead        • No Impact Cone       • Pruned Impact Cone
  • Min Turns            • Excerpts Only        • Max 4,000 B Response
```

### 3.2 Operating Mode Specifications

#### Mode 1: `BYPASS` (Pure Native Execution)
- **Target:** Tier 1 (`SIMPLE`).
- **MCP Configuration:** Zero Engram MCP servers registered. Zero custom tool schemas exposed.
- **Instruction Injection:** None. The model receives standard repository instructions without mandatory preflight/postflight or symbol lookup directives.
- **Result:** Identical to native baseline. Turn count is minimized ($N \le 6$); protocol overhead is exactly 0 tokens.

#### Mode 2: `LEAN` Mode
- **Target:** Tier 2 (`MEDIUM`).
- **MCP Configuration:** 2 read-only tools registered: `engram_get_context` and `engram_why`.
- **Exclusions:** `engram_inspect_symbol` and preflight contracts are omitted.
- **Budget Ceiling:** Injected context capped at 1,200 tokens. Response size capped at 8 KB.

#### Mode 3: `FULL` Mode (Bounded Production Engine)
- **Target:** Tier 3 (`COMPLEX`).
- **MCP Configuration:** Full suite (4 tools: `engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight`).
- **Remediated Budget Constraints:**
  - `engram_inspect_symbol` strictly bounded: `import_dependent_entities` capped at top 10 project-internal entities (external library transitive fan-out pruned).
  - Narrative prose explanation omitted when JSON is returned.
  - Hard single-response ceiling: 4,000 tokens (16 KB).
  - Mandatory omission manifest returned if items exceed budget.

---

## 4. Benchmark Protocol Redesign: The 3-Arm Evaluation Model

To provide rigorous, unassailable scientific proof of value without confounding variables, the benchmark harness is expanded from a 2-arm design to a **3-arm evaluation model**:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       3-ARM BENCHMARK PROTOCOL                              │
├─────────────────────────────────────────────────────────────────────────────┤
│ ARM A: Native Baseline                                                      │
│ • No Engram MCP tools. Native shell & editor tools only.                     │
│ • Purpose: Establishes true control baseline.                               │
│                                                                             │
│ ARM B: Always-On Engram (Unremediated / Fixed Candidate 3)                  │
│ • Engram tools unconditionally registered for all tasks.                    │
│ • Purpose: Quantifies fixed protocol overhead and verifies failure modes.   │
│                                                                             │
│ ARM C: Adaptive Engram (Remediated Candidate 4 Engine + Classifier)         │
│ • Classifier chooses BYPASS for Tier 1, LEAN for Tier 2, FULL for Tier 3.   │
│ • Purpose: Measures real-world optimization value under intelligent routing.│
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.1 Formal Scientific Distinctions

$$\Delta_{\text{overhead}} = \text{Tokens}(\text{Arm B}) - \text{Tokens}(\text{Arm A}) \quad \text{[Measures overhead penalty on simple tasks]}$$
$$\Delta_{\text{retrieval}} = \text{Tokens}(\text{Arm A}) - \text{Tokens}(\text{Arm C}) \quad \text{[Measures net optimization on complex tasks]}$$
$$\Delta_{\text{routing}} = \text{Tokens}(\text{Arm B}) - \text{Tokens}(\text{Arm C}) \quad \text{[Measures value of adaptive classifier]}$$

### 4.2 Acceptance Gates for Phase 5.4 Live Validation
1. **Correctness Invariant:** Functional correctness score $C = 1.00$ (all verification gates pass) across all arms on completed tasks.
2. **Simple Task Non-Regression:** Arm C on Task A MUST achieve $\text{Tokens}(\text{Arm C}) \le \text{Tokens}(\text{Arm A}) + 1\%$ by routing to `BYPASS`.
3. **Complex Task Bounded Execution:** Arm C on Task B MUST complete execution without breaching the token ceiling or headroom guard.
4. **Deterministic Classification:** Classification latency $< 10\text{ ms}$; routing decision pre-registered in run manifest before dispatch.

---

## 5. Implementation Roadmap for Stages 5–8

1. **Pre-Implementation Decision Gate:** Present Stages 0–4 findings and design specifications to the user for formal approval.
2. **Stage 5 (Controlled Engine Remediation):**
   - Apply bounded packing and import pruning to `engram_inspect_symbol` in `src/mcp.rs` and `src/impact.rs`.
   - Implement `TaskComplexityClassifier` in `benchmarks/matched-pair-harness/src/runner.py`.
   - Recompile and register Candidate 4 binary.
3. **Stage 6 (3-Arm Benchmark Execution):**
   - Execute 3-arm evaluation (Arm A, Arm B, Arm C) across Task A and Task B.
4. **Stage 7 (Regression Testing):**
   - Execute full offline Rust (399 tests) and Python (318 tests) regression suites.
5. **Stage 8 (Final Acceptance Package):**
   - Generate release decision and documentation package.

*Document finalized. Proceeding to User Presentation & Decision Gate.*
