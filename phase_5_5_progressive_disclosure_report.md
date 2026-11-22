# Engram Phase 5.5: Optimization B — Progressive Context Disclosure Report

**Phase:** 5.5  
**Component:** `src/budget.rs` (`DetailLevel::Compact`), `src/compiler.rs`, `src/mcp.rs`  
**Scope:** Three-Tier Progressive Disclosure, Compact Metadata Emission, Targeted Symbol Excerpts  

---

## 1. Technical Problem Statement

AI coding agents often execute exploratory Turn-0 queries to discover which files and types exist in a subsystem. Returning full function bodies or even contracted structural excerpts on initial discovery wastes massive context headroom on code the agent never edits or inspects.

---

## 2. Implementation Details

Engram now supports a formal 3-stage progressive disclosure architecture:

| Disclosure Stage | Target Consumer | DetailLevel | Contents Emitted | Typical Token Footprint per File |
|---|---|---|---|---|
| **Stage 1: Compact Metadata** | Initial exploration / tool routing | `DetailLevel::Compact` | File path, language, symbol name, kind, line numbers, and signature only. | 80–150 tokens |
| **Stage 2: Targeted Excerpts** | Focused implementation planning | `DetailLevel::Contracted` | Signatures + bounded excerpts for targeted symbols; compact lines for others. | 250–500 tokens |
| **Stage 3: Full Source** | In-depth implementation / debugging | `DetailLevel::Full` | Complete implementation blocks for targeted symbols. | 600–1,500 tokens |

### Code Implementation:
1. `src/budget.rs`: Added `DetailLevel::Compact` variant to enum.
2. `src/compiler.rs`: When `detail_level == DetailLevel::Compact`, emits:
   ```text
   FILE src/user.rs [rust]
   SYMBOLS (COMPACT METADATA):
   - struct UserAccount (L2..L7): pub struct UserAccount
   - fn new (L10..L12): pub fn new(id: u64, name: String, email: String) -> Self
   - fn calculate_user_tax (L23..L28): pub fn calculate_user_tax(account: &UserAccount, rate: f64) -> f64
   ```
3. `src/mcp.rs`: Exposes `compact: bool` and `detail_level: "compact"` on `engram_get_context`.

---

## 3. Empirical Verification & Results

- **Test Suite:** `tests/token_efficiency_test.rs::test_progressive_disclosure_compact_vs_contracted_vs_full`.
- **Measured Result:**
  - Full Mode: 538 tokens
  - Compact Mode: 124 tokens
  - **Measured Reduction: 77.0%**
- **Verdict:** **PASS**. Delivers full symbol discovery while saving 77% of context tokens.
