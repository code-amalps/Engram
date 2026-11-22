# Engram Milestone 5F — Comprehensive Remediation Report
## Offline Remediation Across Workstreams A, B, C, and D

**Date:** 2026-11-04  
**Milestone:** 5F — Harness & Integration Remediation  
**Status:** COMPLETE (Strictly Offline Verification)  
**Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Active Git Branch:** `feature/async-precommit-guard`  
**HEAD Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`  

---

## 1. Executive Summary

Following the forensic analysis in Milestone 5E of the first authorized live matched-pair pilot (`pilot_01`), Milestone 5F was commissioned to execute offline engineering remediation across four core workstreams:

1. **Workstream A: Codex MCP Integration & Pre-Dispatch Headroom Guard Remediation**
   - Eliminated the headless CLI argument bypass by implementing deterministic workspace-level `.codex/config.toml` MCP configuration.
   - Whitelisted `.codex` in sandbox cleanliness validation.
   - Fixed the variable attribution bug in `StreamingTokenTracker` where cumulative thread tokens (115,992) were mistakenly assigned to single-request input tokens.
   - Restored Turn-0 semantic context injection and bounded shell retrieval guidance for `arm == "treatment_B"`.
   - Remediated skipped-arm telemetry to report `score: null` and `passed_all_requirements: null` instead of false zeroes, and classified skipped-arm verdicts as `TREATMENT_FAILED_CONTROL_SKIPPED` / `CONTROL_FAILED_TREATMENT_SKIPPED`.

2. **Workstream B: Code Excerpt & AST Line-Bounded Knowledge Delivery**
   - Transformed Engram compiler and MCP tools to return exact line-bounded code blocks (`L{start}..L{end}`) rather than bare file paths.
   - Added explicit completeness indicators (`[COMPLETE_BLOCK]` vs `[INCOMPLETE_EXCERPT: showing X of Y lines]`).
   - Integrated uncertainty indicators when confidence is LOW to warn the agent before it attempts speculative file reads.

3. **Workstream C: Empirical Token Replacement Telemetry & Accounting**
   - Implemented `TokenReplacementAccounting` in `telemetry.py` to observe whether Engram actually replaces source reads or merely adds overhead.
   - Extracted `files_retrieved_via_engram`, `files_read_via_shell`, `files_repeated_read`, `engram_excerpts_count`, `engram_excerpts_reread_count`, and `tokens_per_successful_task`.
   - Updated JSON schemas to validate replacement telemetry.

4. **Workstream D: Future Architecture Preservation**
   - Formally documented 12 architectural design pillars in `future-architecture-preservation.md`.

---

## 2. Workstream A: Harness & Integration Remediation

### 2.1 Codex MCP Initialization via Workspace Config
- **Issue Discovered in Milestone 5E**: In headless execution on Windows via `subprocess.Popen`, the Codex CLI (`codex exec`) silently dropped ad-hoc command line overrides passed via `-c mcp_servers.engram...`. The MCP server was never spawned, and the agent had zero awareness of Engram tools.
- **Root Cause**: The Codex CLI prioritizes configuration files in the active workspace root (`<sandbox>/.codex/config.toml`) over command-line override arguments.
- **Implementation**:
  - In `benchmarks/matched-pair-harness/src/isolation.py`, added `configure_sandbox_codex_mcp(sandbox_path, engram_executable)`.
  - Configures `<sandbox>/.codex/config.toml` declaring:
    ```toml
    [mcp_servers.engram]
    command = "<sanitized_engram_path>"
    args = ["-C", "<sandbox_path>", "mcp"]
    default_tools_approval_mode = "approve"
    ```
  - In `isolation.py:verify_sandbox_cleanliness()`, whitelisted `.codex`, `.codex/`, and `.codex/config.toml` in `allowed_untracked` to prevent sandbox rejection.
  - In `runner.py:build_codex_command()`, added automatic verification that `<sandbox>/.codex/config.toml` is present before launching any Engram treatment arm.

### 2.2 Token Guard Headroom Variable Attribution Fix
- **Issue Discovered in Milestone 5E**: In `pilot_01`, the pre-dispatch guard tripped at Turn 2 claiming estimated next turn cost was 115,992 tokens (equal to the entire multi-turn thread cumulative token count), prematurely killing the treatment run with 35,000 tokens of remaining budget.
- **Root Cause**: `StreamingTokenTracker` had an assignment defect:
  ```python
  # BUGGY CODE in runner.py:
  self.last_turn_request_tokens = turn_token_usage.get("input_tokens")  # Held 115,992 (cumulative)
  ```
- **Implementation**:
  - In `runner.py:StreamingTokenTracker.process_line()`:
    - For `token_usage_record`: Extract single-request usage strictly from `payload.usage["input_tokens"]` (41,011), while thread cumulative usage is extracted from `payload.thread_token_usage["total_tokens"]` (115,992).
    - For `event_msg (token_count)`: Extract single-request usage strictly from `info.last_token_usage["input_tokens"]`.
    - For `turn.completed`: Extract single-request usage strictly from `usage["input_tokens"]`.
  - Verified that `estimate_next_request_tokens()` returns $\max(41011, 45000) = 45000$, keeping the projected next turn within the 175,000 ceiling.

### 2.3 Turn-0 Context Injection & Shell Retrieval Guidance
- **Issue**: `runner.py` Turn-0 context injection logic was restricted to legacy arms `if arm in ("v151_arm2", "v154_arm3")` and omitted `"treatment_B"`.
- **Implementation**:
  - Updated `runner.py` line 666 to check `if arm in ("treatment_B", "v151_arm2", "v154_arm3")`.
  - Appended `SHELL_RETRIEVAL_GUIDANCE` to instruct the agent to prefer Engram tools over blind whole-file shell dumps.
  - Computed and recorded `turn0_context_bytes`, `turn0_context_tokens`, and `turn0_context_sha256` in run telemetry.

### 2.4 Skipped Arm Reporting & Comparative Analysis
- **Issue**: Skipped baseline arms were reported as `score: 0.0` and `passed_all_requirements: False`, causing the summary to claim parity failure and mutual failure.
- **Implementation**:
  - In `schemas/run_telemetry.schema.json` and `matched_pair_summary.schema.json`, permitted `null` for `score`, `quality_adjusted_cost`, and `passed_all_requirements`.
  - In `orchestrator.py:_create_skipped_arm_record()`, sets `score=None`, `quality_adjusted_cost=None`, `passed_all_requirements=None`.
  - In `orchestrator.py:compute_comparative_analysis()`, returns `verdict = "TREATMENT_FAILED_CONTROL_SKIPPED"` or `"CONTROL_FAILED_TREATMENT_SKIPPED"` instead of `"MUTUAL_ACCEPTANCE_FAILURE"`.

---

## 3. Workstream B: Code Excerpt & AST Line-Bounded Delivery

### 3.1 Bounded Excerpt Formatting in Compiler & MCP
- **Issue**: Previously, Engram returned bare file paths or entity summaries without source code lines. When an agent queried Engram, it received a file path and was forced to immediately run `cat <file>`, reading the entire file (thousands of tokens) and negating any token savings.
- **Implementation**:
  - In `src/compiler.rs`:
    - Updated entity version tracking to record `start_line` and `end_line`.
    - Formatted code excerpts with explicit line annotations: `[L{start}..L{end}]`.
    - Added completeness indicator: `[COMPLETE_BLOCK]` for whole functions/classes, and `[INCOMPLETE_EXCERPT: showing X of Y lines]` with `// [TRUNCATED to fit token budget]` when truncated.
    - When retrieval confidence is LOW, injected explicit uncertainty warnings in gaps.
  - In `src/mcp.rs`:
    - Enhanced `engram_inspect_symbol` to read source files directly and return bounded code blocks with line ranges and completeness tags.

---

## 4. Workstream C: Token Replacement Telemetry & Accounting

### 4.1 Implementation of TokenReplacementAccounting
- **Dataclass**: Defined in `benchmarks/matched-pair-harness/src/telemetry.py`:
  ```python
  @dataclass
  class TokenReplacementAccounting:
      files_retrieved_via_engram: List[str]
      files_read_via_shell: List[str]
      files_repeated_read: List[str]
      engram_excerpts_count: int
      engram_excerpts_reread_count: int
      serialized_invocations_bytes: int
      serialized_responses_bytes: int
      estimated_payload_tokens: int
      replacement_status: str  # "MEASURED", "ESTIMATED", "UNKNOWN", "NOT_OBSERVABLE"
      tokens_per_successful_task: Optional[float] = None
  ```
- **Parsing Logic**:
  - In `parse_rollout_file()` and `parse_events()`:
    - Extracts file targets and payload bytes from Engram MCP tool calls.
    - Extracts shell file reads (`cat`, `type`, `Get-Content`, `head`, `tail`) from command executions.
    - Correlates whether files retrieved via Engram were still subsequently read in full via shell (`files_repeated_read` and `engram_excerpts_reread_count`).
    - Reports "tokens per successful task" when tasks achieve $C = 1.0$.

---

## 5. Workstream D: Future Architecture Preservation

- **Document**: Created `amendments/draft-05/milestone-5f/future-architecture-preservation.md`.
- **Preserved Pillars**:
  1. Automatic Task Memories & Insight Retention
  2. Evidence-Based Cache Invalidation
  3. Gitless Content-Hash Snapshots
  4. Cross-Repository Frontend / Backend Graph Traversal
  5. Incremental Context Updates & Delta Streaming
  6. Build & Test Log Summarization
  7. Prompt-Cache Stability & Prefix Determinism
  8. Dynamic Retry Budgets & Exploration Cost Caps
  9. No-Progress & Repetitive Action Detection
  10. Strict Resource Governance & Sandboxing Limits
  11. Protocol-Independent Knowledge Adapters
  12. UTCP / Native Tool Execution Runtimes

---

## 6. Verification and Test Results

### 6.1 Targeted Milestone 5F Regression Tests
- **Test File**: `benchmarks/matched-pair-harness/tests/test_milestone_5f_remediation.py`
- **Results**: **7 passed in 1.64s**
  - `test_configure_sandbox_codex_mcp`: PASS
  - `test_streaming_token_tracker_multi_turn_accounting`: PASS
  - `test_streaming_token_tracker_correctly_halts_at_genuine_breach`: PASS
  - `test_turn0_guidance_present_in_runner_constants`: PASS
  - `test_skipped_arm_telemetry_schema_validation`: PASS
  - `test_comparative_analysis_verdict_for_skipped_arm`: PASS
  - `test_telemetry_replacement_accounting_from_events`: PASS

### 6.2 Broader Harness Regression Suite
- **Executed**: All non-live unit, safety, resilience, and evaluation tests.
- **Results**: **297 passed in 220.99s** (100% pass rate).

### 6.3 Rust Engine Tests
- **Executed**:
  - `cargo test --manifest-path Cargo.toml --test mcp_consolidation_test`: **13 passed, 0 failed**
  - `cargo test --manifest-path Cargo.toml --test turn0_injection_test`: **5 passed, 0 failed**
  - `cargo test --manifest-path Cargo.toml --lib`: **17 passed, 0 failed**
  - `cargo test --manifest-path Cargo.toml --bin engram`: **28 passed, 0 failed**
  - `cargo check --manifest-path Cargo.toml --tests`: **CLEAN (0 errors, 0 warnings)**

---

## 7. Integrity Checklist

| Integrity Requirement | Status | Verification Evidence |
| :--- | :---: | :--- |
| Zero live model API requests | VERIFIED | Offline execution only; zero network calls made |
| Frozen binaries intact | VERIFIED | SHA-256 hashes of Candidate Phase 1, v1.5.1, v1.5.4 untouched |
| Golden repository intact | VERIFIED | Golden repo clean; zero mutations or uncommitted changes |
| Git working tree preservation | VERIFIED | Zero commits, tags, resets, or branch switches performed |
| JSON schema validation | VERIFIED | Telemetry and summary schemas strictly valid under Draft 2020-12 |
| Clean compilation | VERIFIED | Rust and Python test suites pass 100% clean |
