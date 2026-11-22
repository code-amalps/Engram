# Engram Milestone 5E — Deliverable 8
# Milestone 5E Closure Summary: pilot_01 Forensic Analysis

## 1. Milestone Mission and Status

**Milestone**: Engram Amendment 05 — Milestone 5E: `pilot_01` Forensic Analysis and Agent Integration Failure Investigation  
**Status**: **COMPLETE (OFFLINE-ONLY)**  
**Authorization Compliance**: Zero live inference calls executed; zero tokens consumed; zero git modifications or branch changes performed.

The mission of Milestone 5E was to conduct a comprehensive, independent offline forensic investigation into the authorized $N=1$ pilot run (`pilot_01`, executed 2026-10-30), which consumed 117,135 tokens in treatment arm `treatment_B` without editing project files, utilized 0 Engram MCP tools, and terminated at the pre-dispatch token headroom guard.

---

## 2. Core Forensic Breakthroughs and Conclusions

Across the six targeted investigation areas (A through F), the milestone achieved decisive, evidence-backed conclusions:

### Breakthrough 1 (Investigation A: MCP Integration Lifecycle)
- **Root Failure**: Codex CLI in non-interactive batch mode (`codex exec`) did not load or initialize the Engram MCP server when specified via command-line configuration overrides (`-c mcp_servers.engram.command=...`).
- **Telemetry Evidence**: Line 2 of the raw session rollout (`rollout-2026-10-30T16-12-15-01a18c67-2e83-7b51-9326-bf2dcf886752.jsonl`) confirms that the model was provided only `functions` (`exec`, `wait`, `request_user_input`) and `mcp__cua_repl`. **Zero Engram tools were ever registered or exposed to the model.**
- **Harness Flaw**: The harness falsely asserted `"mcp_server_status": "ACTIVE"` based purely on CLI argument presence without performing an active handshake or tool discovery audit.

### Breakthrough 2 (Investigation B: Agent Exploration and Token Explosion)
- **Root Cause**: Left with only generic PowerShell access (`functions.exec`), the agent conducted brute-force reconnaissance.
- **Catastrophic Dump**: In Request 2, the agent issued a PowerShell script dumping the first 220 lines of 6 core source files (`analyzer.rs`, `compiler.rs`, `main.rs`, `storage.rs`, `incremental.rs`, `Cargo.toml`), flooding the context with **10,094 tokens** in a single turn.
- **Context Escalation**: Context grew from 14,648 tokens in Turn 1 to 41,011 tokens in Turn 4, locking the agent into exploratory paralysis.

### Breakthrough 3 (Investigation C: Pre-Dispatch Guard Defect)
- **Root Cause**: A variable misattribution in `benchmarks/matched-pair-harness/src/runner.py` assigned `self.last_turn_request_tokens` to `turn_token_usage.get("input_tokens")`.
- **The Miscalculation**: In `codex exec`, `turn_token_usage` is a cumulative thread counter across all requests (`115,992 tokens`), whereas the actual single-request input cost for Request 4 was `41,011 tokens`.
- **Premature Halt**: The guard evaluated estimated next turn cost as $\max(115,992, 45,000) = 115,992$, compared it to remaining budget ($57,865$), and prematurely killed the run, despite Request 5 having sufficient budget ($57,865 \ge 45,000$).

### Breakthrough 4 (Investigation D: Acceptance and Reporting Audit)
- **Root Cause**: When `treatment_B` halted, the harness skipped `control_A` via stop-on-failure. However, `matched_pair_summary.json` assigned `control_A` a score of `0.00`, declared `pair_acceptance_parity: true`, and asserted a verdict of `MUTUAL_ACCEPTANCE_FAILURE`.
- **Remediation**: Skipped arms must be recorded with `score: null`, requirements `NOT_EVALUATED`, acceptance `SKIPPED`, and verdict `TREATMENT_FAILED_CONTROL_SKIPPED`.

### Breakthrough 5 (Investigation E: Task and Tool Alignment)
- **Root Cause**: Naked MCP execution without pre-compiled Turn-0 context left the system fragile.
- **Strategic Direction**: Implement a hybrid model where Engram compiles a Turn-0 context pack directly into the initial task prompt, supplemented by dynamic MCP tools and explicit prompt instructions.

### Breakthrough 6 (Investigation F: Prioritized Remediation Plan)
- Formulated an exact, prioritized engineering roadmap across P0 (Guard fix, `.codex/config.toml` creation, active handshake preflight, Turn-0 injection), P1 (classifier and reporting integrity), P2 (prompt hardening), and P3 (diagnostics).

---

## 3. Milestone Deliverables Inventory

All eight required analytical deliverables have been authored and placed into `amendments/draft-05/milestone-5e/`:

| Deliverable ID | Filename | Topic | Status |
| :---: | :--- | :--- | :---: |
| **D1** | `pilot-01-execution-timeline.md` | Chronological turn-by-turn event matrix and timeline | **COMPLETE** |
| **D2** | `mcp-integration-forensic-analysis.md` | 6-stage MCP lifecycle analysis and configuration failure | **COMPLETE** |
| **D3** | `agent-token-consumption-analysis.md` | PowerShell source dumping and prompt context explosion | **COMPLETE** |
| **D4** | `token-guard-forensic-analysis.md` | Cumulative vs single-turn token tracker defect analysis | **COMPLETE** |
| **D5** | `acceptance-and-skipped-arm-reporting-audit.md` | Audit of skipped arm scoring and false parity claims | **COMPLETE** |
| **D6** | `task-and-tool-suitability-review.md` | Architectural evaluation of task, tools, and prompts | **COMPLETE** |
| **D7** | `prioritized-remediation-plan.md` | P0–P3 prioritized engineering remediation roadmap | **COMPLETE** |
| **D8** | `milestone-5e-closure-summary.md` | Executive synthesis and milestone closure report | **COMPLETE** |
| **D9** | `SHA256SUMS` | Cryptographic integrity manifest for all deliverables | **PENDING** |

---

## 4. Strict Operational Boundary Compliance

1. **Inference Boundary**: Zero API calls or model invocations were made during this milestone.
2. **Historical Integrity**: The original `pilot_01` artifact and evidence files in `benchmarks/matched-pair-harness/reports/runs/pilot_01/` remain 100% immutable and unedited.
3. **Repository Integrity**: No git commits, resets, stashes, or branch switches were executed. The working tree remains cleanly preserved at commit `c70845b4d5913457f1b176fb0370b80dd792711f` on branch `feature/async-precommit-guard`.
