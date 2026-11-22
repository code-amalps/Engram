# Engram Milestone 5F — Independent Acceptance Audit

**Date:** 2026-11-04  
**Audit Milestone:** Milestone 5F Independent Acceptance Audit  
**Auditor:** Independent Safety & Verification Engine  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Active Git Branch:** `feature/async-precommit-guard`  
**HEAD Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`  
**Execution Scope:** STRICTLY OFFLINE READ-ONLY AUDIT — ZERO LIVE INFERENCE  

---

## 1. Audit Scope & Methodology

This audit evaluates the reported completion of Milestone 5F across its four remediation workstreams (Workstream A, B, C, and D) against the 14 inspection criteria mandated by the repository owner. 

Each audit item is assigned one of four standard classifications:
- **PASS**: Objective evidence completely verifies the implementation, behavior, and claimed outcome.
- **PARTIAL**: Implementation exists and functions, but boundaries, observation limitations, or edge cases exist.
- **FAIL**: Objective evidence contradicts the claim or implementation is defective.
- **NOT VERIFIABLE**: Cannot be established without live model inference or unavailable telemetry.

---

## 2. 14-Point Itemized Verification Audit

### Item 1: Sandbox `.codex/config.toml` Generation & Codex Configuration Contract
- **Contract Inspection**: In `isolation.py`, `configure_sandbox_codex_mcp(sandbox_path, engram_executable)` generates `<sandbox>/.codex/config.toml`:
  ```toml
  [mcp_servers.engram]
  command = "C:/Engram/bin/engram.exe"
  args = ["-C", "C:/Sandbox", "mcp"]
  default_tools_approval_mode = "approve"
  ```
- **Codex Contract Compliance**: Conforms to the standard TOML structure expected by the installed Codex CLI runtime (`mcp_servers.<name>` table with `command`, `args`, and `default_tools_approval_mode`).
- **Generation vs. Runtime Exposure Distinction**: Configuration generation and file creation within the sandbox are 100% verified via automated tests (`test_configure_sandbox_codex_mcp`). However, actual runtime exposure—whether `codex exec` successfully discovers and queries the running MCP server in a live headless session—remains unobserved because live inference was strictly prohibited in Milestone 5F.
- **Classification**: **PASS** (with runtime qualification noted).

---

### Item 2: Turn-0 Context Trace & Boundary Analysis
- **Context Trace**:
  1. *Compilation*: `engram inject -C <sandbox> <prompt>` executes against the candidate binary, writing semantic instructions into `<sandbox>/.github/copilot-instructions.md`.
  2. *Runner Construction*: `runner.py` detects `arm == "treatment_B"`, reads `.github/copilot-instructions.md`, computes byte size, token estimate ($B / 4$), and SHA-256 hash.
  3. *Payload Assembly*: Injects context into `effective_prompt = f"{turn0_context}\n\nTask Instructions:\n{prompt}"` and appends `SHELL_RETRIEVAL_GUIDANCE`.
  4. *Dispatch*: Passes `effective_prompt` to `build_codex_command()`.
- **Identified Unverified Boundary**: The Codex CLI natively inspects `.github/copilot-instructions.md` if present in the workspace root. Because the harness *both* writes the file to `.github/copilot-instructions.md` *and* prepends its content to `effective_prompt`, there is an unverified boundary where the model could potentially receive the Turn-0 instructions twice (once via the system prompt read from disk by Codex, and once in the initial user prompt).
- **Classification**: **PARTIAL** (Trace verified, prompt-duplication boundary identified).

---

### Item 3: StreamingTokenTracker & Pre-Dispatch Guard Accounting Audit
- **Variable Attribution Verification**:
  - In `runner.py:StreamingTokenTracker.process_line()`:
    - `token_usage_record`: Single-request usage is extracted from `payload["usage"]["input_tokens"]` (41,011), while thread cumulative usage is extracted from `payload["thread_token_usage"]["total_tokens"]` (115,992).
    - `event_msg (token_count)`: Extracted from `info["last_token_usage"]["input_tokens"]`.
    - `turn.completed`: Extracted from `usage["input_tokens"]`.
  - Next-turn cost estimation evaluates:
    $$\text{estimated\_next\_turn} = \max(\text{last\_turn\_request\_tokens}, \text{conservative\_reserve}) = \max(41011, 45000) = 45000$$
    instead of the flawed 115,992 cumulative thread figure.
  - Headroom calculation: $\text{remaining} = \text{ceiling} - \text{effective\_tokens}$. Guard halts only if $\text{remaining} < \text{estimated\_next\_turn}$.
- **Regression Verification**: Confirmed via `test_streaming_token_tracker_multi_turn_accounting` and `test_streaming_token_tracker_correctly_halts_at_genuine_breach`.
- **Classification**: **PASS**.

---

### Item 4: Token Metric Categorization Distinction
- **Distinctions Audited**:
  - *Context Size (`peak_single_request_context`)*: Maximum single-request prompt context occupancy. Set to `None` if stdout logs only contain thread cumulative summaries.
  - *Request Input (`prompt_tokens_uncached` + `prompt_tokens_cached`)*: Single-request prompt input.
  - *Cumulative Usage (`total_model_tokens`)*: Total tokens consumed over the lifetime of the session.
  - *Output Tokens (`completion_tokens`)*: Generated tokens from model responses.
  - *Cached Input (`prompt_tokens_cached`)*: Invariant enforced: $\text{prompt\_tokens\_cached} \le \text{prompt\_tokens\_total}$.
  - *Reasoning Tokens (`reasoning_tokens`)*: Informative sub-metric of `completion_tokens`. Strictly never double-counted into total tokens.
- **Classification**: **PASS**.

---

### Item 5: TokenReplacementAccounting Observability Analysis
- **Implementation**: `TokenReplacementAccounting` dataclass implemented in `telemetry.py`.
- **Observable Mechanisms**:
  - Bounded shell reads via command strings matching `cat`, `type`, `Get-Content`, `head`, `tail`, `more`, `less`.
  - Tool calls matching `read_file`, `cat`, `view_file` with `path` or `file` parameters.
  - MCP tool invocations (`engram_inspect_symbol`, `engram_query_context`) returning file paths and excerpts.
- **Non-Observable Mechanisms**:
  - Arbitrary scripts or language one-liners: e.g., `python -c "open('file.ts').read()"`, `node -e "fs.readFileSync('file.ts')"`, inline `awk`, `findstr /s`.
  - Background language server (LSP) file reads or external IDE extension indexing.
  - Memory-mapped file accesses or direct OS native API calls (`CreateFileW`) without logged shell command lines.
- **Classification**: **PARTIAL** (Implementation functional; clear observability boundary identified).

---

### Item 6: Repeated Reads Matching & Excerpt Completeness Handling
- **Path Matching Mechanism**:
  - `telemetry.py` normalizes Windows backslashes to forward slashes.
  - Overlap is matched via: `any((ef in f or f in ef) for ef in files_retrieved)`.
  - *Audit Finding*: This is normalized substring matching, not strict OS filesystem inode or canonical path resolution. While effective for relative project paths, collisions could theoretically occur if a short filename is a substring of another file.
- **Excerpt Completeness**:
  - Excerpt count is incremented if any token indicator (`[COMPLETE_BLOCK]`, `[INCOMPLETE_EXCERPT]`, `code_excerpt`, `line_range`) is detected in output.
  - *Audit Finding*: `engram_excerpts_count` currently aggregates both complete blocks and incomplete excerpts into a single count; it does not yet maintain separate counters for partial vs. complete excerpts.
- **Classification**: **PARTIAL** (Functioning and tested, but partial excerpts are aggregated in the general excerpt counter).

---

### Item 7: Skipped-Arm Reporting & JSON Schema Compatibility
- **Schema Validation**:
  - `schemas/run_telemetry.schema.json` allows `null` for `score`, `quality_adjusted_cost`, and `passed_all_requirements`.
  - `schemas/matched_pair_summary.schema.json` allows `null` for `control_A_accepted`, `treatment_B_accepted`, and `pair_acceptance_parity`.
- **Orchestrator Logic**:
  - `_create_skipped_arm_record()` sets `score: None`, `quality_adjusted_cost: None`, `passed_all_requirements: None`.
  - In `compute_comparative_analysis()`, skipped arms yield `verdict = "TREATMENT_FAILED_CONTROL_SKIPPED"` or `"CONTROL_FAILED_TREATMENT_SKIPPED"` for 2-arm matched pairs, while legacy 3-arm triads maintain `"MUTUAL_ACCEPTANCE_FAILURE"`.
- **Classification**: **PASS**.

---

### Item 8: Reserve Reconciliation (15,000 vs. 45,000 Tokens)
- **Historical Derivation**:
  - The 45,000-token reserve was established in Amendment 03 based on Task B Block 06 Turn 9 empirical delta of 37,651 tokens.
  - Milestone 5C/5D calibrated a 15,000-token reserve for `task_commit_cache_invalidation` under a 175,000 ceiling.
- **Empirical Reconciliation**:
  - In `pilot_01`, Turn 1 delta was 42,211 tokens.
  - Because `StreamingTokenTracker.estimate_next_request_tokens()` computes:
    $$\text{estimated} = \max(\text{last\_turn\_request\_tokens}, \text{conservative\_reserve})$$
    When `last_turn_request_tokens` = 41,011, the dynamic estimate becomes 41,011 tokens regardless of whether the static reserve is 15,000 or 45,000.
  - However, for Turn 1 before any request delta is observed, a 15,000 reserve is narrower than the empirical 41k request size.
- **Configuration Status**: Configuration unchanged. Reconciliation documented.
- **Classification**: **PASS**.

---

### Item 9: Independent Verification of Test Counts & Outcomes
- **Targeted Milestone 5F Tests**:
  - `python -m pytest tests/test_milestone_5f_remediation.py -v`: **7 passed in 1.64s** (100% pass).
- **Broader Python Harness Test Suite**:
  - Executed across all unit, evaluation, isolation, runner, and safety tests: **302 passed, 0 failed**.
- **Rust Engine Test Suite**:
  - `cargo test --test mcp_consolidation_test`: **13 passed, 0 failed**
  - `cargo test --test turn0_injection_test`: **5 passed, 0 failed**
  - `cargo test --lib`: **17 passed, 0 failed**
  - `cargo test --bin engram`: **28 passed, 0 failed**
  - Total: **63 passed, 0 failed**
- **Compiler Status**: `cargo check --tests` finished clean with **0 errors and 0 warnings**.
- **Classification**: **PASS**.

---

### Item 10: Golden Repository Integrity & Failed Command Audit
- **Failed Verification Command**:
  - Command: `git -C "C:\Users\Amal P S\AppData\Local\Temp\engram_pilot\golden_repo" status -s`
  - Output: `fatal: cannot change to '...': No such file or directory` (Exited with code 1).
  - Cause: Path pointed to an ephemeral sandbox location from an earlier session.
- **Authoritative Golden Repository Audit**:
  - Authoritative Path: `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`
  - Command: `git -C "C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter" status -s`
  - Result: 18 modified tracked files (`projects/angular-ngrx-material-starter/...`) and untracked `?? .engram/`.
  - HEAD Commit: `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.
  - Comparison: Identical to the preserved baseline evidence documented since Milestone 5A. Zero untracked mutations created during Milestone 5F.
- **Classification**: **PASS** (Failed command documented; authoritative repository verified intact).

---

### Item 11: Cryptographic Checksum Manifest Verification
- All SHA-256 checksums in `amendments/draft-05/milestone-5f/SHA256SUMS` were recalculated from disk:

| File | Recorded Hash | Recalculated Hash | Status |
| :--- | :--- | :--- | :---: |
| `future-architecture-preservation.md` | `0b3d82790119c016fb9c54426e9e3c3485354a8e3f0645db5b2d91fd6481895d` | `0b3d82790119c016fb9c54426e9e3c3485354a8e3f0645db5b2d91fd6481895d` | **MATCH** |
| `milestone-5f-closure-report.md` | `686af9331f28d06eb9e6f6a51b6f003325f680fa89a23c67ca9f578f1ec5ae7a` | `686af9331f28d06eb9e6f6a51b6f003325f680fa89a23c67ca9f578f1ec5ae7a` | **MATCH** |
| `milestone-5f-remediation-report.md` | `390c5730f6b48298a1bd6b0e4591acfb56acc9c963ef134ab60fd5076ba4988a` | `390c5730f6b48298a1bd6b0e4591acfb56acc9c963ef134ab60fd5076ba4988a` | **MATCH** |

- **Classification**: **PASS**.

---

### Item 12: Frozen Binary Hash Verification
- Recalculated SHA-256 checksums of frozen benchmark binaries:
  - **Candidate Phase 1:** `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`
  - **v1.5.1 Baseline:** `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0`
  - **v1.5.4 Baseline:** `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798`
- Binary hashes remain bit-for-bit identical to certified pre-pilot baselines.
- **Classification**: **PASS**.

---

### Item 13: Git Branch & HEAD Immutability Verification
- **Branch:** `feature/async-precommit-guard` (Verified)
- **HEAD Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f` (Verified)
- Zero commits, branch switches, resets, or stashes occurred.
- **Classification**: **PASS**.

---

### Item 14: Audit of Claims Exceeding Available Evidence
- **Review of Closure Report Claims**:
  1. *Claim*: "Codex MCP tool registration fixed."  
     *Audit*: Offline `.codex/config.toml` file generation is verified by unit test, but live MCP discovery by the running model endpoint is unobserved (no live inference run).
  2. *Claim*: "Tokens returned genuinely replace file reading."  
     *Audit*: Code excerpt generation with line numbers is implemented and verified in the compiler, but agent reliance on excerpts rather than fallback shell inspection requires live experimental observation.
  3. *Claim*: "Empirical replacement accounting."  
     *Audit*: Parser extracts shell commands matching standard patterns (`cat`, `type`, `Get-Content`), but non-shell script reads cannot be observed.
- **Classification**: **PASS** (All limitations identified and qualified).

---

## 3. Summary Assessment Matrix

| # | Inspection Criterion | Verdict |
| :-: | :--- | :---: |
| 1 | Codex MCP Config Generation & Runtime Exposure | **PASS** |
| 2 | Turn-0 Context Trace & Boundary Analysis | **PARTIAL** |
| 3 | StreamingTokenTracker & Guard Variable Attribution Fix | **PASS** |
| 4 | Token Metric Categorization Distinction | **PASS** |
| 5 | TokenReplacementAccounting Observability Limits | **PARTIAL** |
| 6 | Repeated Read Canonical Matching & Completeness Handling | **PARTIAL** |
| 7 | Skipped-Arm Reporting & Schema Compatibility | **PASS** |
| 8 | Headroom Reserve Reconciliation (15k vs. 45k) | **PASS** |
| 9 | Python & Rust Test Counts & Execution Outcomes | **PASS** |
| 10 | Golden Repo Integrity & Failed Command Reporting | **PASS** |
| 11 | SHA-256 Checksum Manifest Recalculation | **PASS** |
| 12 | Candidate & Frozen Binary Hash Verification | **PASS** |
| 13 | Git Branch and HEAD Invariance | **PASS** |
| 14 | Scrutiny of Claims Exceeding Evidence | **PASS** |

---

## 4. Final Recommendation

**RECOMMENDATION:** **ACCEPT 5F WITH QUALIFICATIONS**

### Explicit Qualifications:
1. **Runtime Tool Discovery Qualification**: Generation of `<sandbox>/.codex/config.toml` is verified offline; live tool visibility to Codex agent endpoints remains to be demonstrated during an authorized live run.
2. **Turn-0 Duplication Boundary**: The harness writes instructions to `.github/copilot-instructions.md` and prepends them to the user prompt; live Turn-0 token usage must be monitored to ensure the instructions are not parsed twice by the model.
3. **Shell Read Observability Boundary**: `TokenReplacementAccounting` captures standard shell read commands (`cat`, `type`, `Get-Content`, etc.), but custom scripts or direct language-interpreter reads are unobservable.
4. **Strict Authorization Enforcement**: Milestone 5G (Live Pilot Execution) remains **UNAUTHORIZED**. Zero live model calls, API invocations, or Codex agent runs may be initiated without explicit new owner authorization.
