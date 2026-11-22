# Engram Phase 5.5: Optimization D — Adaptive Activation & Trivial Task Bypass Report

**Phase:** 5.5  
**Component:** `src/mcp.rs` (`engram_get_context`), `src/prompt.rs` (`classify_task_complexity`)  
**Scope:** Automated Task Complexity Classification, Negative Net-Benefit Bypass, Zero-Token Fallback  

---

## 1. Technical Problem Statement

Many tasks given to AI coding agents are self-contained or trivial leaf edits (e.g. "fix typo in doc comment in user.rs", "update package version in Cargo.toml"). Running full context retrieval for such tasks yields negative net benefit:
$$\text{Expected Net Benefit} = \text{Expected Context Savings} - \text{Retrieval Overhead} < 0$$
The retrieval overhead (2,000+ tokens) vastly exceeds any context benefit.

---

## 2. Implementation Details

In `src/mcp.rs`, `engram_get_context` now evaluates task complexity when `adaptive: true` (or `bypass_trivial: true`) is passed:
```rust
let adaptive = args["adaptive"].as_bool().unwrap_or(false);
let bypass_trivial = args["bypass_trivial"].as_bool().unwrap_or(adaptive);
let no_bypass = args["no_bypass"].as_bool().unwrap_or(false);

if bypass_trivial && !no_bypass {
    let signals = crate::prompt::classify_task_complexity(task);
    if signals.classification == crate::prompt::TaskComplexity::Trivial {
        return Ok(json!({
            "status": "BYPASS_TRIVIAL",
            "task": task,
            "classification": "TRIVIAL",
            "reason": signals.reason,
            "net_benefit": "NEGATIVE_RETRIEVAL_OVERHEAD_EXCEEDS_SAVINGS",
            "target_files": [],
            "governing_rules": [],
            "cochange_warnings": [],
            "relevant_decisions": [],
            "ambiguity_gaps": [],
            "token_budget": budget,
            "estimated_tokens": 0,
            "confidence": "HIGH",
            "omission_manifest": Value::Null,
            "next_action": "Task classified as trivial leaf edit. Context retrieval bypassed to preserve token budget. Proceed directly with edits."
        }));
    }
}
```

---

## 3. Empirical Verification & Results

- **Test Suite:** `tests/token_efficiency_test.rs::test_adaptive_bypass_trivial_in_mcp`.
- **Results:**
  1. Trivial task query (`"fix typo in doc comment in main.rs"`) with `adaptive: true` returned `status: "BYPASS_TRIVIAL"`, `target_files: []`, and `estimated_tokens: 0`. **Tokens saved: 100%**.
  2. Complex task query (`"refactor database transaction storage and add rollback recovery mechanism"`) with `adaptive: true` correctly bypassed the trivial exit and compiled full, grounded context.
- **Verdict:** **PASS**. Zero regression on complex tasks; total elimination of retrieval tokens on trivial tasks.
