# Phase 8.0: Turn-0 Static Context Injection Report

**Document Identifier:** `ENGRAM-PHASE-8-0-TURN0-20261122`  
**Execution Date:** 2026-11-22  
**Module Path:** `src/instructions.rs`, `src/setup.rs`, `src/main.rs`  
**Execution Gate:** Gate D Completed (Turn-0 Static Context Injection Operational)  

---

## 1. Executive Summary & Problem Diagnosis

In traditional AI coding agent workflows, the agent begins every user prompt with zero repository knowledge, triggering the **"MCP Interaction Tax"**:
1. **Turn 1 (Exploration):** The agent calls `engram_get_context` to discover relevant symbols (~3,300 bytes wire payload).
2. **Turn 2 (Handshake):** The agent calls `engram_preflight` to validate boundaries and obtain a Change Contract (~1,060 bytes wire payload).
3. **Cumulative Context Inflation:** Because multi-turn LLMs re-read the entire dialogue history on every subsequent turn, these 2 exploratory tool calls add 2 extra conversational turns that multiply cumulative context tokens across the entire session.

Phase 8.0 implements **Turn-0 Static Context Injection** to export task context, AST symbol signatures, and pre-locked Change Contracts directly into agent instruction files before the agent's first turn.

---

## 2. Architecture & Instruction-File Integration

Engram injects context using non-destructive, managed delimiter blocks:

```markdown
<!-- BEGIN ENGRAM MANAGED BLOCK -->
## Engram Architecture Context & Task-Aware Intelligence
- **Active Change Contract:** `ctr_8ca0e1ae544046bda495de631cac502b`
- **Authorized Expected Files:** `src/calculator.py`, `tests/test_calculator.py`
- **Forbidden Boundaries:** `config/**`, `.engram/**`
- **Directives:** Proceed directly to implementation within authorized files. Do not call get_context or preflight.
<!-- END ENGRAM MANAGED BLOCK -->
```

### Supported Target Ecosystems:
| Client Identifier | Target Instruction File | Detection Priority |
| :--- | :--- | :---: |
| `vscode` | `.github/copilot-instructions.md` | Primary default fallback |
| `cursor` | `.cursorrules` / `.cursor/rules/` | Auto-detected if present |
| `claude` | `CLAUDE.md` | Auto-detected if present |
| `windsurf` | `.windsurfrules` | Auto-detected if present |
| `all` | All detected agent files in workspace | Multi-target broadcast |

### Strict Non-Destructive Invariants:
1. **Existing User Content Preservation:** Surrounding user-authored instructions outside the managed block are preserved 100% byte-for-byte.
2. **Line-Ending Fidelity:** `detect_line_ending` automatically preserves CRLF (`\r\n`) on Windows and LF (`\n`) on Unix.
3. **Atomic File Updates:** Updates write to sibling temporary files before atomic rename/replacement, eliminating partial-write corruptions.
4. **Malformed & Duplicate Block Rejection:** Files with mismatched markers or multiple blocks trigger a safety `Err` rather than risking user data loss.

---

## 3. CLI User Controls

Turn-0 injection is strictly opt-in and controlled via the `engram inject` command:

- **Inject Task Context:**
  ```bash
  engram inject "Add multiply method to calculator" --token-budget 1200
  ```
- **Dry-Run Inspection (No file modification):**
  ```bash
  engram inject "Refactor auth middleware" --dry-run
  ```
- **Target Selection:**
  ```bash
  engram inject "Fix typo" --client cursor
  engram inject "Update schema" --target ./custom-rules.md
  ```
- **Revert to Standard Rules:**
  ```bash
  engram inject --clear
  ```
- **Strip Managed Block Completely:**
  ```bash
  engram inject --strip
  ```

---

## 4. Empirical Efficiency Comparison

Controlled measurement on `sample_workspace` comparing baseline agent execution against Turn-0 static context injection:

| Metric | Baseline (Interactive MCP) | Treatment (Turn-0 Injection) | Net Delta |
| :--- | :---: | :---: | :---: |
| **Exploratory Tool Calls** | 2 (`get_context`, `preflight`) | 0 (Contract pre-locked) | **-2 calls (-100%)** |
| **Exploratory Conversational Turns** | 2 turns | 0 turns | **-2 turns (-100%)** |
| **Interactive Wire Bytes Exchanged** | 4,365 bytes | 0 bytes | **-4,365 bytes (-100%)** |
| **Static Turn-0 Injection Payload** | 0 bytes | ~337 tokens (~1,100 bytes) | Injected statically |
| **Context Retrieval Correctness** | 100% | 100% | Identical AST symbols |
| **Scope Drift Containment** | Enforced at postflight | Enforced at postflight | Identical boundaries |

---

## 5. Test Suite Verification

All 13 Turn-0 tests executed and passed:
- `tests/turn0_injection_test.rs`: 5 / 5 tests passed
  - `test_turn0_inject_cli_adaptive_bypass` (PASS)
  - `test_turn0_inject_clear_and_strip_lifecycle` (PASS)
  - `test_turn0_inject_cli_standard_task` (PASS)
  - `test_turn0_prepare_cli_inject_integration_and_json` (PASS)
  - `test_turn0_inject_cli_dry_run_and_target_flags` (PASS)
- `tests/turn0_phase1_test.rs`: 8 / 8 tests passed
  - `test_prepare_cli_missing_task_error` (PASS)
  - `test_prepare_cli_unresolved_context` (PASS)
  - `test_mcp_and_existing_commands_compatibility` (PASS)
  - `test_malformed_instruction_file_safeguard` (PASS)
  - `test_instruction_idempotency_and_preservation` (PASS)
  - `test_prepare_cli_dry_run` (PASS)
  - `test_prepare_cli_with_task_flag_and_positional` (PASS)
  - `test_adaptive_bypass_comprehensive_matrix` (PASS)

---

## 6. Certification Verdict

### `TURN0_STATIC_CONTEXT_INJECTION_CERTIFIED`

Gate D is complete. Turn-0 static context injection is fully operational, verified, and ready for integration into the v1.6 efficiency-first architecture.
