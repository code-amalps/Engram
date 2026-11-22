# Engram Phase 5.5: Optimization A — Task-Aware Retrieval Ranking Report

**Phase:** 5.5  
**Component:** `src/compiler.rs` (`context_scoped_optimized`, `compute_symbol_score`)  
**Scope:** Intra-File Symbol Prioritization, Explainable Relevance Scoring, Deterministic Tie-Breaking  

---

## 1. Technical Problem Statement

Previously, when a candidate file was selected for context packing, its symbols were retrieved and formatted strictly in source-file line order:
```sql
SELECT entity_id, name, kind, start_line, end_line, signature
FROM entity_versions
WHERE path=?1 AND invalidated_at_revision IS NULL
ORDER BY start_line
```
In files with dozens of functions, constructors, or imports, early helper routines consumed the available token headroom, causing the actually relevant symbol to be truncated with `[TRUNCATED: Budget Exceeded]`.

---

## 2. Implementation Details

In `src/compiler.rs`, symbol extraction now computes a multi-tiered task-relevance score before ordering:
```rust
let compute_symbol_score = |sym_name: &str, sym_sig: &str| -> f64 {
    let mut sym_score = 0.0;
    let sym_lower = sym_name.to_ascii_lowercase();
    if let Some(targets) = target_symbols {
        if targets.iter().any(|t| t.eq_ignore_ascii_case(sym_name)) {
            sym_score += 5000.0; // Tier 1: Explicitly requested symbol
        }
    }
    if potential_symbols.iter().any(|ps| ps.eq_ignore_ascii_case(sym_name)) {
        sym_score += 2000.0; // Tier 2: Exact query symbol match
    }
    for term in &terms {
        if sym_lower == *term {
            sym_score += 1000.0; // Tier 3: Keyword token match
        } else if sym_lower.contains(term) && term.len() >= 3 {
            sym_score += 300.0;  // Tier 4: Substring match
        } else if sym_sig.to_ascii_lowercase().contains(term) && term.len() >= 3 {
            sym_score += 100.0;  // Tier 5: Signature match
        }
    }
    sym_score
};

syms_meta.sort_by(|a, b| {
    let sa = compute_symbol_score(&a.1, &a.5);
    let sb = compute_symbol_score(&b.1, &b.5);
    sb.total_cmp(&sa).then_with(|| a.3.cmp(&b.3)) // Deterministic start_line tie-break
});
```

---

## 3. Verification & Results

- **Test Suite:** `tests/token_efficiency_test.rs::test_targeted_symbols_filtering`.
- **Result:** The requested symbol `calculate_tax` was ranked at the head of the excerpt list and formatted with full details, while untargeted helper functions were formatted as lightweight 1-line signatures.
- **Verdict:** **PASS**. Zero relevant symbols starved of token budget.
