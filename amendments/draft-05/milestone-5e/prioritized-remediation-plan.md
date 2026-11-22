# Engram Milestone 5E — Deliverable 7
# Prioritized Remediation Plan for Future Experimental Runs

## 1. Executive Summary

This remediation plan synthesizes the findings of the Milestone 5E forensic investigation into a prioritized engineering roadmap. The objective is to eliminate all technical, architectural, and reporting failure modes identified during `pilot_01` before seeking authorization for any future matched-pair experiments.

The plan is organized into four strict priority tiers:
- **P0: Blocker (Mandatory for Experimental Validity)**
- **P1: Reporting & Integrity (Mandatory for Truth in Advertising)**
- **P2: Prompt & Model Guidance (Recommended for Efficiency Optimization)**
- **P3: Diagnostics & Observability (Recommended for Operational Hardening)**

---

## 2. Priority Matrix

| Priority | Component | Defect / Objective | Remediation Target | Target Files |
| :---: | :--- | :--- | :--- | :--- |
| **P0-1** | Pre-Dispatch Guard | Misattributed cumulative thread prompt tokens to single-turn cost. | Correct `self.last_turn_request_tokens` assignment to use per-request `usage["input_tokens"]`. | `benchmarks/matched-pair-harness/src/runner.py` |
| **P0-2** | MCP Configuration | Codex CLI ignored command-line `-c` MCP server overrides in headless `exec` mode. | Write deterministic `.codex/config.toml` file into sandbox root declaring `[mcp_servers.engram]`. | `benchmarks/matched-pair-harness/src/isolation.py`, `runner.py` |
| **P0-3** | MCP Handshake Preflight | Harness falsely marked MCP status as `ACTIVE` without verification. | Implement active preflight handshake check verifying Engram tools appear in tool registry before dispatch. | `benchmarks/matched-pair-harness/src/isolation.py`, `runner.py` |
| **P0-4** | Turn-0 Context Injection | Task prompt contained zero pre-compiled architectural context. | Wire up Candidate Phase 1 compiler to inject Turn-0 context pack into initial task prompt preamble. | `benchmarks/matched-pair-harness/src/runner.py` |
| **P1-1** | Result Classification | Skipped control arm reported as `score: 0.0` with `MUTUAL_ACCEPTANCE_FAILURE`. | Update classifier to report skipped arms as `SKIPPED` / `null` and verdict as `TREATMENT_FAILED_CONTROL_SKIPPED`. | `benchmarks/matched-pair-harness/src/classifier.py`, `orchestrator.py` |
| **P1-2** | Parity Logic | `pair_acceptance_parity: true` asserted when one arm never executed. | Set `pair_acceptance_parity = null` when any arm is skipped. | `benchmarks/matched-pair-harness/src/orchestrator.py` |
| **P2-1** | Prompt Directives | Agent defaulted to bulk PowerShell dumps due to lack of instructions. | Add explicit system prompt instructing agent to prioritize Engram tools and avoid bulk file dumps. | `benchmarks/matched-pair-harness/config/tasks/` |
| **P3-1** | Real-time Observability | Stdio logs from MCP server were uncaptured in run directory. | Pipe Engram MCP server stderr to dedicated `mcp_server.log` in run directory. | `benchmarks/matched-pair-harness/src/runner.py` |

---

## 3. Detailed Engineering Specifications for P0 Remediations

### 3.1 P0-1: Pre-Dispatch Guard Telemetry Fix
**Root Cause**: `StreamingTokenTracker.on_token_usage()` assigned `self.last_turn_request_tokens = turn_inp` (where `turn_inp` was populated from `payload["turn_token_usage"]["input_tokens"]`, a monotonically increasing thread counter).

**Exact Remediation in `benchmarks/matched-pair-harness/src/runner.py`**:
```python
def on_token_usage(self, payload: dict) -> None:
    usage = payload.get("usage") or {}
    turn_usage = payload.get("turn_token_usage") or {}
    thread_usage = payload.get("thread_token_usage") or {}

    # Extract single-request usage
    inp = usage.get("input_tokens", 0)
    out = usage.get("output_tokens", 0)

    # Extract thread/cumulative usage
    turn_inp = turn_usage.get("input_tokens") or inp
    turn_out = turn_usage.get("output_tokens") or out

    # FIX: Record the actual isolated cost of this single request context:
    self.last_turn_request_tokens = inp  # Fixes premature halt!
```

### 3.2 P0-2: Deterministic Sandbox `.codex/config.toml` Creation
**Root Cause**: `codex.exe exec` ignores command-line `-c` config overrides for nested arrays on Windows.

**Exact Remediation in `benchmarks/matched-pair-harness/src/isolation.py`**:
When initializing a treatment arm sandbox:
1. Create directory `<sandbox>/.codex/`.
2. Write `<sandbox>/.codex/config.toml`:
```toml
[mcp_servers.engram]
command = "C:\\path\\to\\engram.exe"
args = ["serve-mcp"]
```
3. Remove command-line `-c mcp_servers.engram...` flags from the CLI launch arguments.

### 3.3 P0-3: Active MCP Tool Registration Verification Gate
**Root Cause**: The harness blindly trusted CLI parameters without verifying that the Codex runtime registered Engram tools.

**Exact Remediation**:
1. During execution startup, after the Codex process initializes, inspect the first `response_item` (`type: "additional_tools"`).
2. Assert that at least one tool matching namespace `mcp__engram` or `engram` exists in the tools array.
3. If no Engram tools are found:
   - Terminate the run immediately before Request 1 consumes full prompt tokens.
   - Mark status as `MCP_REGISTRATION_FAILED`.
   - Prevent budget burn on an unassisted run masquerading as a treatment run.

### 3.4 P0-4: Turn-0 Context Injection Pipeline
**Root Cause**: Naked MCP execution left the agent blind when MCP failed.

**Exact Remediation**:
1. Prior to launching Codex in treatment mode, invoke the Candidate Phase 1 Engram binary:
   ```cmd
   engram.exe context --format markdown --max-tokens 2500
   ```
2. Prepend the compiled context pack directly into the task prompt given to Codex.
3. This guarantees the agent begins Request 1 with full architectural awareness, regardless of dynamic tool calling.

---

## 4. Verification and Testing Protocol for Remediations

Before any live pilot authorization is requested:
1. **Offline Unit Tests**:
   - Run `tests/test_token_guard_validation.py` with synthetic multi-turn payloads containing `usage` (41k) vs `turn_token_usage` (115k) to verify no premature halt occurs.
   - Run `tests/test_isolation_mcp_config.py` to verify `.codex/config.toml` is written with correct escaping.
2. **Offline Replay Test**:
   - Replay the raw `pilot_01` rollout through the corrected `StreamingTokenTracker` and verify that the guard correctly identifies that Request 5 was dispatchable.
3. **Dry-Run Tool Verification**:
   - Run a 0-token offline preflight verifying that Codex successfully parses `.codex/config.toml` and lists Engram tools.
