# Phase 2 Overhead Profile: Where Does the Cost Go?

**Document Version:** 1.0.0  
**Phase:** Phase 2 Measurement Pilot  
**Status:** ACTIVE PROFILE SPECIFICATION & EMPIRICAL AUDIT  

> [!IMPORTANT]
> **Accounting Rule:** Do not add overlapping token categories together and present them as a single cumulative total.
> In particular:
> 1. Auxiliary wrapper tokens (`gpt-4o-mini`, 0.00 AIC) must **not** be added to primary orchestrator tokens.
> 2. Tool schema tokens must **not** be multiplied by total turns unless verified that the host re-injects schemas on every model turn without prompt caching.
> 3. Account-level credit deltas must **not** be attributed to MCP response character counts.

---

## 1. Overhead Bucket Definitions

| Bucket # | Category | What Counts | Measurement Method | Telemetry Tag |
|---|---|---|---|:---:|
| **1** | **Tool Definitions & Schemas** | Schema tokens of Engram tools (`preflight`, `get_context`, `postflight`) declared to the model | Exact character count of MCP tool schemas in `src/mcp.rs` converted to tokens (~650 tokens, 2,800 bytes). Record whether the host re-injects or caches. | **Measured (Schema) / Estimated (Tokens)** |
| **2** | **Engram Tool Results** | Response payload bytes and tokens returned to the agent from Engram tool executions | Exact payload bytes read from `.engram/events/*.json` and server logs. Tokenized at 4 chars/token. | **Measured (Bytes) / Estimated (Tokens)** |
| **3** | **Engram-Caused Conversational Turns** | Model turns that exist solely to perform an Engram handshake (e.g. initial preflight, incremental preflight, postflight) | Count from transcript: turns where the only action executed was an Engram call or its verification follow-up. | **Measured** |
| **4** | **Agent Exploration** | Non-Engram searches, directory listings, file reads, and ripgrep searches | Count and payload characters of `find_files`, `grep_search`, `read_file` in transcript. | **Measured (Count) / Estimated (Tokens)** |
| **5** | **Retries & Error Correction** | Repeated identical failed tool calls, compiler error loops, or repeated failing test runs | Count of repeated executions following a rejection or failure. | **Measured** |
| **6** | **Hook / Test / CI Output** | stdout/stderr of automated test executions returned into agent context | Output bytes captured in transcript from `run_in_terminal` or test runners. | **Measured** |
| **7** | **Model Reasoning & Generation** | Code generation tokens and planning output produced by the primary model | Orchestrator output tokens (withheld by Copilot client logs; estimated from diff size). | **Estimated** |
| **8** | **Auxiliary UI Summarizer Calls** | `copilotLanguageModelWrapper` calls run by VS Code to generate action status summaries | Read directly from `copilotLanguageModelWrapper_*.copilotmd` trace files. | **Measured** |

---

## 2. Per-Operation Classification of Engram Calls

| Operation | Analysis Nature | Execution Model | Can Run Locally or in Hook? | Output Returned to Model? | Overhead Classification |
|---|---|---|---|:---:|---|
| **`get_context`** | Deterministic symbol & dependency graph retrieval | Local offline graph traversal | Yes (CLI or pre-session prompt file) | Yes | **Informational** (~450 bytes) |
| **`preflight` (initial)** | Deterministic boundary validation & SHA-256 fingerprinting | Local git diff & AST parsing | Yes (CLI / pre-prompt hook) | Yes | **Guardrail / Handshake** (~853 bytes) |
| **`preflight` (incremental)** | Incremental SHA-256 cache match | Sub-millisecond fingerprint check | Yes (CLI / background daemon) | Yes | **Cache Re-validation** (~806–986 bytes) |
| **`postflight`** | Deterministic diff validation & contract verification | Local git diff against contract boundaries | Yes (Pre-commit hook / git hook) | Yes | **Enforcement** (~500–750 bytes) |

---

## 3. Profile Results: Task B (Remember-Me Feature)

### Run Profile B (Current Optimized Engram)
- **Task:** Task B (Remember-Me Feature)
- **Snapshot Commit:** `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`
- **Configuration:** Configuration B (Current Engram Deployed)

| Bucket | Measured Value | Est. Tokens | Turns | Share of Turns | Notes & Evidence Source |
|---|---|---|---|---|---|
| **1. Tool Definitions** | 2,814 bytes | ~650 | 0 | — | 3 MCP tools registered in Copilot Language Model Tools API |
| **2. Engram Responses** | 2,645 bytes (853 + 986 + 806) | ~660 | 0 | — | Compact payloads in `.engram/events/` (97.5% smaller than baseline) |
| **3. Engram-Caused Turns** | 3 preflight calls | ~4,950 (Aux) | **3** | **27.3%** | 3 preflight tool calls each required 1 conversational turn |
| **4. Agent Exploration** | 3 file reads | ~1,800 | 2 | 18.2% | Reads on `auth.actions.ts`, `auth.effects.ts`, `app.component.html` |
| **5. Retries & Fixes** | 1 spec failure retry | ~2,200 | 2 | 18.2% | Updated `auth.reducer.spec.ts` after action signature update |
| **6. Test Output** | Karma test output | ~1,200 | 1 | 9.1% | Ran unit tests |
| **7. Reasoning / Output** | 18 files changed | ~2,500 | 3 | 27.2% | Code edits across auth core, app component, and translations |
| **8. Auxiliary Summarizer** | 11 wrapper calls | **17,075** | — | — | `copilotLanguageModelWrapper` (`gpt-4o-mini`, 0.00 AIC) |
| **Total** | — | — | **11** | **100%** | **Elapsed Time: ~600 seconds** |

---

## 4. A vs B Empirical Comparison: Task B

| Metric / Bucket | Config A (Without Engram) | Config B (With Engram) | Absolute Delta (B - A) | Relative Impact | Evidence Status |
|---|---:|---:|---:|---:|:---:|
| **Agent Conversational Turns** | **7** turns | **11** turns | **+4 turns** | **+57%** | **Measured** (Transcript) |
| **Auxiliary Wrapper Calls** | **7** calls | **11** calls | **+4 calls** | **+57%** | **Measured** (`test-out`) |
| **Auxiliary Wrapper Tokens** | 11,534 tokens | 17,075 tokens | **+5,541 tokens** | **+48%** | **Measured** (`gpt-4o-mini`) |
| **Engram Tool Calls** | 0 | 3 | **+3 calls** | — | **Measured** (`.engram/events`) |
| **Engram Response Bytes** | 0 | 2,645 bytes | **+2,645 bytes** | — | **Measured** (Compact payloads) |
| **Repository Exploration Reads** | 5 reads | 3 reads | **-2 reads** | **-40%** | **Measured** (Engram targeted) |
| **Files Modified (Blast Radius)** | 19–36 files | **18 files** | **-1 to -18 files** | **Sprawl Contained** | **Measured** (`git status`) |
| **Unrequested Modal Dialogs** | Created in baseline | **Blocked** | **Prevented** | **Safeguard Verified** | **Measured** (Scope guard) |
| **Elapsed Wall-Clock Time** | ~540 s | ~600 s | **+60 s** | **+11%** | **Measured** |
| **Direct AI Credit Deductions** | Account-level delta | Account-level delta | Not isolated | Unproven | **Unobservable** (Host) |

---

## 5. Profile Analysis: Task A (Trivial Branding Typo)

| Metric | Config A (Without Engram) | Config B (With Engram) | Delta (B - A) | Analysis |
|---|---:|---:|---:|---|
| **Agent Turns** | **1 turn** | **3 turns** | **+2 turns (+200%)** | Preflight and postflight add 2 turns on a single-line fix |
| **Auxiliary Tokens** | ~1,640 tokens | ~4,930 tokens | **+3,290 tokens (+200%)** | 3 wrapper calls vs 1 wrapper call |
| **Changed Files** | 1 file | 1 file | 0 | Both strictly modified `environment.ts` |
| **Elapsed Time** | ~30 s | ~65 s | **+35 s (+116%)** | Handshake overhead dominates trivial task |

---

## 6. Decision Gate & Root-Cause Analysis

### Dominant Overhead Category Identified
> [!CAUTION]
> **The Dominant Source of Cost Overhead is the "MCP Interaction Tax" (Bucket 3 & Bucket 8).**
> In VS Code Copilot Agent Mode, tool invocations are **not** free auxiliary functions. Every tool call:
> 1. Requires a full round-trip turn in the primary orchestrator conversation.
> 2. Triggers an auxiliary safety and UI status summarizer call (`gpt-4o-mini`) with a 1,634-token prompt template.
> 3. Consumes billable agent iterations at the account level.

Even though Fix A successfully reduced Engram payload bytes by **97.5%** (from 34 KB to 2.6 KB), the **number of conversational turns** increased from 7 to 11. To the user, credit usage and turn counts increased because Copilot had to "talk" to Engram 3 extra times.

### Prioritized Optimization Roadmap for Phase 3
1. **Prompt-Based Context Injection (Eliminate Interactive Preflight Turns):**
   Inject pre-computed architectural boundaries, rules, and targeted context directly into workspace instructions (`.github/copilot-instructions.md`) or system prompts prior to the session. This enables zero-tool-turn enforcement.
2. **Pre-Commit / Save Hook Gating (Shift Postflight Out of Conversation):**
   Move postflight diff validation from conversational tool calls to a local Git pre-commit hook or file-save watcher. If validation fails, surface the error only then, saving 1–2 model turns on successful runs.
3. **Single-Shot Tool Execution:**
   Combine `get_context` and `preflight` into an atomic single-shot tool to cut tool turns by 50%.
4. **Adaptive Small-Task Bypass:**
   Automatically bypass Engram MCP handshakes for trivial single-file / single-line changes where blast-radius risk is near zero.
