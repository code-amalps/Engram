# Engram Phase 5.5: Optimization E — Token Budget & Response Bounding Report

**Phase:** 5.5  
**Component:** `src/mcp.rs` (`enforce_mcp_envelope_boundary`, `truncate_tool_payload`), `src/compiler.rs`  
**Scope:** Budget Enforcement, 16 KB Stdio Envelope Guard, Progressive Truncation Ladder  

---

## 1. Technical Problem Statement

AI coding agents crash or drop tool execution context when MCP stdio JSON-RPC payloads exceed 16 KB or 64 KB buffers, or when multi-byte UTF-8 code points are sliced mid-character.

---

## 2. Implementation Details

Engram combines two robust budget-bounding mechanisms:
1. **Compilation-Level Token Budget:** `compiler::context_scoped_optimized` tracks `used` tokens against `limit` (`token_budget`, default 2,000 tokens). Files exceeding the budget are recorded in `omission_manifest` rather than silently truncated.
2. **MCP Envelope-Level Byte Boundary (16,384 Bytes):** `src/mcp.rs` enforces an 8-level progressive degradation ladder:
   - Level 0: Full payload if under `byte_limit` (16,384 bytes).
   - Levels 1–5: Progressive shedding of raw AST signatures, symbol relationships, and docstrings.
   - Level 6: Minimal valid payload with essential target identity and `payload_truncated: true`.
   - Level 7: Structured error response.
3. **UTF-8 Slicing Guard:** `safe_truncate_str` inspects UTF-8 leading bytes (`(b & 0xC0) != 0x80`) to guarantee zero mid-character split panics.

---

## 3. Empirical Verification & Results

- **Test Suite:** `tests/mcp_boundary_test.rs`, `tests/utf8_safety_test.rs`, `tests/inspect_symbol_budget_test.rs`.
- **Results:**
  - 100% of tested paths enforce byte limit $\le 16,384$ bytes.
  - Zero UTF-8 decoding errors or character boundary panics.
  - Zero malformed JSON serialization errors.
- **Verdict:** **PASS**. Complete protocol safety and budget enforcement.
