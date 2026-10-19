# Engram Cost Metrics Specification

**Document Version:** 1.0.0
**Phase:** Phase 0–2 Measurement Pilot
**Status:** ACTIVE SPECIFICATION

> [!IMPORTANT]
> **Scientific Integrity Rule:** The target of reducing usage by 50–60% is an experimental hypothesis to test, not a verified fact or a target to manufacture. Nothing in this document assumes savings exist before attributable evidence is recorded.
> If per-task billing cannot be isolated from client logs alone, no claim of "AI credit savings" or "cheaper runs" may be made. Claims are strictly restricted to verified secondary proxies (e.g. "fewer files modified", "fewer exploration turns").

---

## 1. Primary Cost Metric

### Chosen Primary Metric
- **[x] GitHub AI Credits / Premium Requests per successful task**

### Data Source and Telemetry Characteristics
- **Source:** GitHub Copilot Account Billing Dashboard & VS Code Status Bar Usage Display (e.g. `23.7 / 200 used`).
- **Granularity:** Account-level aggregated consumption. There is **no** per-request, per-turn, or per-task billing API endpoint accessible to local VS Code extensions or MCP servers.
- **Reporting Delay / Latency:** Account-level credit counters update asynchronously with a propagation delay ranging from 2 to 15 minutes after session completion.
- **Can usage from one benchmark run be isolated from other activity reliably?**
  - **Verdict: NO.**
  - **Reason:** Client debug logs (`copilotLanguageModelWrapper_*.copilotmd`) only expose auxiliary UI summarizer calls (`gpt-4o-mini`), which report `copilotUsage: 0.00 AIC (0 nano-AIU)`. Primary model orchestrator calls (`panel/editAgent`, e.g. Claude 3.5 Sonnet or GPT-4o) run server-side over an opaque stream where exact token consumption and credit deduction per turn are withheld by GitHub Copilot. In an environment with background language server checks, telemetry pings, or concurrent IDE activity, individual fractional credit deltas cannot be isolated with single-run precision.

### Mandatory Accounting Boundary
> [!WARNING]
> Because per-task credit billing cannot be isolated with single-turn granularity from client-side traces alone, this benchmark pilot **relies on secondary proxy metrics** and will **not claim verified AI Credit savings**.
> Claims must be phrased strictly in terms of secondary proxies (e.g. "X% fewer edited files", "Y% reduced MCP payload bytes", "Z additional agent conversational turns").

---

## 2. Secondary Proxies

Each proxy is explicitly tagged with its evidence source and extraction method. **Never mix or add together measured and estimated numbers into a single aggregate.**

| Proxy Metric | Evidence Source | Tag | Unit | Description / Limitations |
|---|---|---|---|---|
| **Primary-Model Input Tokens** | Host API (Withheld) | **Estimated** | Tokens | Context fed to orchestrator model (`Claude 3.5 Sonnet` / `GPT-4o`). Withheld by Copilot client logs. |
| **Primary-Model Output Tokens** | Host API (Withheld) | **Estimated** | Tokens | Orchestrator generation tokens. Withheld by Copilot client logs. |
| **Primary-Model Cached Tokens** | Host API (Withheld) | **Estimated** | Tokens | Prompt cache hits. Withheld by Copilot client logs. |
| **Auxiliary-Model Prompt Tokens** | `copilotLanguageModelWrapper_*.copilotmd` | **Measured** | Tokens | `gpt-4o-mini` UI status summarizer prompt tokens (~1,634 tokens/call, ~1,536 cached). |
| **Auxiliary-Model Output Tokens** | `copilotLanguageModelWrapper_*.copilotmd` | **Measured** | Tokens | `gpt-4o-mini` UI status generation tokens (~8 tokens/call). |
| **Model Turns / Agent Steps** | Chat Transcript / Wrapper count | **Measured** | Turns | Conversational round-trips taken by the agent to complete the task. |
| **Engram Tool Invocations** | `.engram/events/*.json` | **Measured** | Calls | Invocations of `preflight`, `get_context`, and `postflight`. |
| **Other Tool Invocations** | Chat Transcript | **Measured** | Calls | File edits, reads, greps, terminal test executions. |
| **Engram Response Size** | `.engram/events/*.json` / Server Log | **Measured** | Bytes / Chars | Total bytes returned across all Engram tool call responses. |
| **Tool Definition Footprint** | MCP Schema / Tool Definitions | **Measured / Est.** | Bytes / Tokens | Schema characters (~2,800 bytes) and estimated tokens (~650 tokens) declared to the agent. |
| **Agent Exploration Calls** | Chat Transcript | **Measured** | Calls | Non-Engram reads, file searches, greps done to explore the codebase. |
| **Retries / Repeated Errors** | Chat Transcript | **Measured** | Calls | Repeated failed commands, syntax errors, or failing test loops. |
| **Elapsed Time** | Wall Clock / Timestamps | **Measured** | Seconds | Total time from initial prompt submission to final agent completion. |
| **Changed Files (Blast Radius)** | `git status --short` / `git diff --stat` | **Measured** | Files | Total count of modified, added, or deleted files. |
| **Forbidden Scope Sprawl** | `git diff --name-only` | **Measured** | Files | Files modified outside the approved task boundary. |
| **Test Verification** | Test runner output | **Measured** | Pass / Fail | Exit code and test suite assertion results. |

### Tag Meanings:
- **`Measured`**: Directly recorded from an authoritative log, file, event payload, or exit code.
- **`Estimated`**: Calculated using a deterministic heuristic (e.g. character-to-token tokenizer approximation `bytes / 4`). The calculation method must be explicitly documented.
- **`Unobservable`**: Value is withheld by the proprietary environment (e.g. GitHub Copilot's private backend token stream).

---

## 3. Definition of a Successful Task

A benchmark run counts as **SUCCESSFUL** if and only if **ALL** four criteria are satisfied:

1. **Automated Verification:** The project's automated test command passes with exit code `0` (e.g. `npx ng test --watch=false --browsers=ChromeHeadless` or targeted spec).
2. **Deterministic Acceptance Checklist:** Every functional and security criterion specified in the task specification (`benchmarks/cost-baseline/SPEC.md`) passes verification.
3. **Diff-Scope Containment:** Zero modifications occur outside the task's allowed file set. Any edit to forbidden directories (e.g. bulk i18n, unrelated UI dialogs, sample modules) constitutes a blast-radius failure.
4. **Safeguard Compliance:** No security policies or verification steps were bypassed, silenced, or circumvented.

**Cost Reporting Rule:**
Cost and token metrics are reported per **SUCCESSFUL** task:
$$\text{Cost per Successful Task} = \frac{\sum \text{Cost of all runs in group}}{\text{Count of Successful Runs}}$$
Failed runs still contribute to the cumulative expenditure of the group.

---

## 4. Acceptance Rules for Comparing Configurations

To eliminate time-of-day, prompt, or environmental bias:

1. **Identical Inputs:** Both Configuration A (without Engram) and Configuration B (with Engram) must use the identical initial Git commit snapshot, identical prompt wording, identical model (`Claude 3.5 Sonnet` or `GPT-4o`), and identical IDE agent settings.
2. **Repetition Count:** Start with **5 matched repetitions** per task per configuration (total 20 runs for the pilot).
3. **Statistical Reporting:** Report every individual run, the median, and the min-max range $[ \min, \max ]$. Do not report means alone, as outliers skew agent benchmarks.
4. **Task Segregation:** Report Task A (trivial) and Task B (multi-file feature) separately. Never publish a combined or blended average that conceals small-task overhead.

---

## 5. Small-Task Overhead Limit

There is no assumption that Engram "never costs more." In fact, for trivial single-line changes:
- Invocations of multi-step MCP tools (`preflight`, `postflight`) necessarily introduce tool-call round-trips.
- In VS Code Copilot, each tool call adds an agent turn and triggers an auxiliary `copilotLanguageModelWrapper` call.
- **Threshold Policy:** For trivial tasks (Task A), maximum acceptable overhead vs. Configuration A is **$\le 15\%$** in wall-clock time and **$\le 1$** extra agent turn. If Engram adds more than 2 extra conversational turns on trivial tasks without preventing errors, Engram must provide an adaptive bypass mechanism for small changes.

---

## 6. Known Blind Spots and Environmental Dynamics

1. **The "MCP Tax" in VS Code Copilot:**
   Every tool invocation in VS Code Copilot Agent Mode triggers a separate conversational loop iteration. Each turn invokes `copilotLanguageModelWrapper` (`gpt-4o-mini`) with a 1,634-token prompt template to generate UI action summaries, adding ~1,650 auxiliary tokens per tool call.
2. **Primary Orchestrator Token Black Box:**
   VS Code does not log the raw prompt and completion tokens exchanged between `panel/editAgent` and the upstream Copilot API. Primary model context savings from Engram's compact responses (~33 KB saved per preflight call) cannot be directly read from client files.
3. **Asynchronous Account Credit Ledger:**
   Credit deductions on the GitHub backend are batched and lag client actions. Fast repeated runs risk attributing credits to the wrong run if quiet windows are not observed.
4. **Tool Schema Context Injection:**
   All registered MCP tools have their JSON schemas exposed to the agent. In VS Code Copilot, tool schemas consume context space on every model turn where tools are available.

---

## 7. Benchmark Budget

| Configuration | Task A (Trivial) | Task B (Remember-Me) | Total Repetitions | Estimated Time |
|---|---:|---:|---:|---:|
| **Config A (Baseline - Without Engram)** | 5 runs | 5 runs | 10 runs | ~60 min |
| **Config B (Optimized - With Engram)** | 5 runs | 5 runs | 10 runs | ~70 min |
| **Total Pilot Scope** | **10 runs** | **10 runs** | **20 runs** | **~130 min** |
