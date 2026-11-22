# Engram Phase 5.9.0.4: Prompt-Input Bound Forensic Audit ($P_k \le 60,000$)

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Phase:** 5.9.0.4 Workstream A  
**Execution Mode:** Strictly Offline, Read-Only Adversarial Forensic Audit  
**Target Parameter:** $P_{\max} = 60,000$ Prompt Input Tokens  
**Classification:** `HISTORICAL_OBSERVED_ESTIMATE_ONLY` (Not an Enforceable Software Boundary)  
**Theoretical Single-Turn Overshoot Status:** `NOT_PROVEN_AS_HARD_LIMIT` (`CONDITIONAL_BOUND_ONLY`)

---

## 1. Purpose

This deliverable provides an exhaustive adversarial source-code audit to determine whether the claimed $60,000$-token prompt input ceiling ($P_k \le 60,000$) represents a hard, enforceable client-side or provider-side software boundary, or merely an empirical estimate. It audits the complete request construction pipeline, token measurement mechanics, truncation behavior, and the resultant impact on single-turn overshoot bounds.

---

## 2. Source Identification & Origin of the 60,000 Value

An exhaustive search across the entire Engram codebase, benchmark harness, and historical audit manifests reveals the exact origin of the $60,000$-token figure:

- **Historical Origin:** The $60,000$ value originated in **Phase 5.4.24** (`PHASE_5_4_24_OVERSHOOT_BOUND_ANALYSIS.md`) and was subsequently audited in **Phase 5.4.25** (`PHASE_5_4_25_PROMPT_CONTEXT_AND_COMPLETION_LIMIT_AUDIT.md` and `phase5_4_25_budget_guarantee_findings.json`).
- **Empirical Context:** It was derived as an empirical upper bound observed during Stage 1 Block 06/07 exploratory runs on `task_b_remember_me`, where multi-turn conversational exploration across Angular components (`auth.effects.ts`, `app.component.ts`, `local-storage.service.ts`) generated single-turn prompt payloads ranging between $35,000$ and $58,000$ tokens.
- **Formal Classification:** Pursuant to Section 4.1 of the Phase 5.9.0.4 directive, the $60,000$ value is classified as:
  # `HISTORICAL_OBSERVED_MAXIMUM` (`OBSERVED_VALUE_ONLY`)

**Critical Finding:** There is **zero code** in `benchmarks/matched-pair-harness/src/runner.py`, `orchestrator.py`, `telemetry.py`, `isolation.py`, or any CLI invocation that defines, checks, asserts, truncates, or restricts prompt input to $60,000$ tokens.

---

## 3. Complete Request Input Model & Token Accounting

We audited the serialized provider request to identify every component constituting the turn's prompt input ($P_k$):

| Request Component | Description / Origin | Counted Locally Pre-Dispatch? | Reported by Provider Post-Hoc? | Bound Enforced? |
|---|---|---|---|---|
| **System Instructions** | Codex built-in system prompt and agent operational instructions | **NO** | YES (in `input_tokens`) | Unbounded (model context) |
| **Developer Instructions** | Custom project instructions (`.codex/instructions.md` / AGY rules) | **NO** | YES (in `input_tokens`) | Unbounded |
| **Task Prompt** | User-supplied task description and goal (`task_b_remember_me.json`) | **NO** | YES (in `input_tokens`) | Unbounded |
| **Retrieved Context** | Engram MCP context payloads (symbol graphs, entities, snippets) | **NO** | YES (in `input_tokens`) | Bounded only by MCP query limit |
| **Repository Snippets** | File contents read via shell (`cat`, `Get-Content`, `rg`) | **NO** | YES (in `input_tokens`) | **Unbounded** (file size limit only) |
| **Tool Definitions** | Complete JSON schemas for all registered MCP & system tools | **NO** | YES (in `input_tokens`) | Fixed schema overhead (~1.7k) |
| **Tool Call Arguments** | Serialized arguments of previously executed tools | **NO** | YES (in `input_tokens`) | Unbounded |
| **Conversation History** | All preceding user messages, assistant responses, and tool outputs | **NO** | YES (in `input_tokens`) | Accumulates across turns |
| **Cached Input Tokens** | Prompt prefix tokens matching provider caching blocks | **NO** | YES (`cached_input_tokens`) | Subset of total input tokens |
| **Provider Metadata** | Model catalog hints, session hashes, permissions manifests | **NO** | YES (in `input_tokens`) | Fixed internal overhead |

### 3.1 Local vs Provider Accounting
1. **Local Pre-Dispatch Counting:** The benchmark harness performs **zero token estimation or pre-flight tokenization**. Prompts are assembled as raw strings and passed directly to the headless `codex exec` subprocess via stdin or CLI parameters.
2. **Provider Post-Hoc Reporting:** Input token consumption is discovered **exclusively post-hoc**, when the provider emits streaming JSONL events (`token_usage_record`, `event_msg token_count`, or `turn.completed`) which are parsed by `StreamingTokenTracker.process_line()` in `runner.py`.

---

## 4. Tracing Enforcement from Configuration to Submission

We traced the command construction in `runner.py:build_codex_command()`:
```python
cmd = [
    str(codex_bin),
    "exec",
    "--sandbox", sandbox_mode,
    "-m", model,
    "-c", f"model_reasoning_effort={reasoning_effort}"
]
if full_auto:
    cmd.append("--full-auto")
```
- **CLI Flags:** No `--max-prompt-tokens`, `--max-input-tokens`, or `--context-window` flags are supported or passed.
- **Pre-Dispatch Rejection:** If a prompt context reaches $80,000$ or $120,000$ tokens, the harness does **not** reject it.
- **Truncation / Degradation:** The harness implements **no truncation or degradation mechanism**. The full string is dispatched to the provider.
- **Provider Architectural Context Window:** The actual boundary is the frontier model's architectural context window, which is **$258,400$ tokens** for `gpt-5.6-luna` (as recorded in `real_session_rollout.jsonl:17`: `"model_context_window": 258400`).

---

## 5. Impact on the Theoretical Overshoot Formula

Phase 5.9.0.3 derived the single-turn overshoot bound:
$$\Omega_{\text{turn}} = P_{\max} + C_{\max} - R_{\min} = 60,000 + 16,384 - 45,000 = 31,384 \text{ tokens}$$

Because this formula assumes $P_{\max} = 60,000$, and $P_{\max}$ is an observed historical quantity rather than an enforced software constraint:
1. If an agent inspecting `angular-ngrx-material-starter` reads a large bundle or multiple source files, $P_k$ can reach $100,000$ tokens.
2. Under $P_k = 100,000$, $C_k = 16,384$, and $R_{\min} = 45,000$:
   $$\Omega_{\text{turn}} = 100,000 + 16,384 - 45,000 = 71,384 \text{ tokens}$$
3. Therefore, $31,384$ tokens is **NOT a hard mathematical upper bound**. It is valid **only conditionally**, subject to the explicit assumption that prompt context does not exceed $60,000$ tokens.

---

## 6. Formal Audit Conclusions

Pursuant to Section 4.4 of the Phase 5.9.0.4 specification:
1. Because input consumption cannot be bounded by current code, the theoretical single-turn overshoot bound of $31,384$ tokens is formally classified as **`NOT_PROVEN_AS_HARD_LIMIT`** (`CONDITIONAL_UPPER_BOUND`).
2. The empirical historical single-turn overshoot demonstrated across extensive Phase 5.4 offline runs remains **$20,000$ tokens**.
3. To transform the conditional upper bound into a guaranteed hard limit, the system requires an explicit remediation: adding a pre-dispatch prompt length/token gate in `runner.py` before subprocess submission.
