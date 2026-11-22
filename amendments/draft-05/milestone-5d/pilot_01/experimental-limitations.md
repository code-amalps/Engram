# Experimental Limitations: Pilot 01 Methodology Audit

**Document ID:** `amendments/draft-05/milestone-5d/pilot_01/experimental-limitations.md`  
**Milestone:** Engram Amendment 05 — Pilot 01  
**Scope:** Controlled Token-Efficiency Pilot Analysis  

---

## 1. Sample Size Limitation ($N=1$)

1. **Exploratory Pilot Only:**  
   $N=1$ provides directional and operational evidence regarding harness execution, guard mechanics, and agent behavior. It possesses **zero statistical power** ($p$-values, confidence intervals, and effect sizes cannot be computed).
2. **Non-Generalizability:**  
   The outcome of Pilot 01 applies strictly to this single run of `task_commit_cache_invalidation` under `gpt-5.6-luna`. It cannot be generalized to other tasks, repositories, model architectures, or Engram configurations.
3. **Absence of Paired Comparison:**  
   Because the baseline arm was skipped under the stop-on-failure rule, no paired difference ($T_B - T_A$) exists for this run.

---

## 2. Agent Tool-Selection Failure and Context Inflation

1. **Zero Engram MCP Utilization:**  
   In `treatment_B`, `mcp_tool_calls_count` was exactly `0`. Although the Engram MCP server was registered with full permissions (`engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight`), the model opted entirely for native shell commands (`powershell.exe -Command ...`).
2. **Unassisted Tool Overhead:**  
   Instead of retrieving scoped AST symbols (which Engram was designed to provide), the agent executed broad text greps and bulk file reads (dumping up to 340 lines per slice from `analyzer.rs`, `storage.rs`, etc.). This caused prompt context to expand to 41,011 tokens within 4 turns.
3. **Confounding Variable:**  
   Because the agent did not execute any Engram MCP tools, the 117,135 tokens consumed in Treatment B reflect the cost of unguided shell exploration rather than the efficiency of Engram-guided context retrieval.

---

## 3. Ceiling and Headroom Calibration Dynamics

1. **Ceiling Sizing (175,000 tokens):**  
   The nominal per-arm budget of 175,000 tokens proved insufficient for an agent engaging in iterative bulk source code dumping across multiple large Rust modules (`analyzer.rs` is ~1,000 lines; `storage.rs` is ~400 lines).
2. **Pre-Dispatch Guard Sensitivity:**  
   The pre-dispatch formula:
   $$\text{Remaining} < \max(\text{Last Turn Request Tokens}, \text{Reserve})$$
   Triggered at Turn 4 because `last_turn_request_tokens` reached 115,992 tokens (due to cumulative conversation history). While this safely prevented exceeding the 175,000 ceiling, it curtailed execution before the agent reached code authoring.
3. **Budget Calibration Insight:**  
   For complex Rust engineering tasks involving multi-file AST architectures, a task requiring comprehensive context either demands:
   - Enforced Turn-0 context injection (to prevent the agent from performing manual raw dumps); or
   - Prompt-level system instructions mandating the use of Engram MCP tools over shell dumps; or
   - A recalibrated operational budget ceiling.

---

## 4. Platform and Operational Boundaries

1. **Host-Side vs. Provider-Side Enforcement:**  
   As established in Milestone 5B-Review, the 175,000 token ceiling and 45,000 reserve are host-side harness guardrails enforced via `codex.exe` process termination. OpenAI ChatGPT/Codex accounts operate under monthly/subscription allowances rather than API per-request token hard limits.
2. **In-Flight Exposure Limitation:**  
   Host process termination immediately ends local stdout/stderr streaming and stops future turn dispatches. However, it does not provide remote cryptographic proof of backend GPU cancellation for an in-flight HTTP request. In Pilot 01, termination occurred cleanly at the turn boundary prior to Turn 5 dispatch.
3. **Model Checkpoint Specificity:**  
   Results are specific to `gpt-5.6-luna` (version `codex-cli 0.164.0-alpha.11.1`). Other model families or newer reasoning checkpoints may exhibit different tool-calling proclivities.
