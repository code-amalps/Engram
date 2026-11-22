# Engram Phase 5.9.0.5: Request Boundary & Client-Side Gate Design Audit

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Phase:** 5.9.0.5 Workstream D  
**Execution Mode:** Strictly Offline, Read-Only Adversarial Audit  
**Audit Target:** Request Serialization, Boundary Classifications & Safe Gate Specifications  
**Audit Status:** `FULLY_SPECIFIED_AND_CLASSIFIED`

---

## 1. Purpose

This deliverable provides an adversarial architectural audit of the request lifecycle between the client runner (`benchmarks/matched-pair-harness/src/runner.py`) and the provider process (`codex exec`). It traces request serialization, distinguishes four distinct token boundaries, audits what the client can and cannot measure pre-dispatch, and defines the exact software requirements for an enforceable $60,000$-token pre-dispatch prompt gate.

---

## 2. Request Serialization & Token Measurement Trace

We traced the complete request construction pipeline in `runner.py`:
1. **Client Serialization:**
   - The user task prompt (`task_b_remember_me.json`) is assembled as a Python string.
   - For Turn 0, it includes the task description and `SHELL_RETRIEVAL_GUIDANCE`.
   - The command line arguments (`-m`, `-c`, `--sandbox`) are assembled and passed via `subprocess.Popen([codex_executable, "exec", ...])`.
   - In subsequent turns (Turns $1..N$), Codex manages the conversational thread internally in its SQLite database (`rollout.db`), appending tool call results and file reads into its context window.
2. **Local Token Measurement (Pre-Dispatch):**
   - **None.** The client harness performs zero pre-dispatch string tokenization.
   - `StreamingTokenTracker` relies on `last_turn_request_tokens` parsed from the *previous* turn's post-hoc response to estimate the *next* turn's prompt cost.
3. **Provider Measurement (Post-Hoc):**
   - The provider tokenizer processes the full serialized payload: system instructions, developer instructions, tool schemas, conversation history, and user input.
   - Token counts are emitted on stdout via `token_usage_record` or `turn.completed` events.
   - All input tokens, including cached prefixes and tool schemas, are reported in `input_tokens`.

---

## 3. Classification of the Four Request Boundaries

Pursuant to Section 7.1 of the directive, we formally distinguish and classify the four operational boundaries:

| Boundary # | Boundary Description | Scope & Mechanism | Current Enforcement Status | Formal Classification |
|---|---|---|---|---|
| **Boundary 1** | **Client-Side Pre-Dispatch Request Limit** | Local harness checks assembled prompt before sending to subprocess | No prompt token check or length limit exists in `runner.py` | **`NOT_ENFORCED`** (`ESTIMATED_ONLY` via last turn) |
| **Boundary 2** | **Provider Model Context-Window Limit** | Physical attention limit of model architecture ($258,400$ tokens for `gpt-5.6-luna`) | Remote provider generation engine rejects prompts exceeding window | **`PROVIDER_ENFORCED`** |
| **Boundary 3** | **Provider Maximum Completion Limit** | Output burst ceiling ($16,384$ tokens for standard output) | Remote provider generation stops generating at length cap | **`PROVIDER_ENFORCED`** |
| **Boundary 4** | **Actual Billable / Quota Usage** | Post-hoc accounting of input + output + reasoning tokens | Emitted via streaming JSONL; parsed by `StreamingTokenTracker` | **`POST_HOC_ONLY`** |

---

## 4. Architectural Specification for a Safe 60,000-Token Pre-Dispatch Gate

Pursuant to Section 7.2 of the directive, we specify the exact architectural design required to implement a robust, safe client-side gate without risking truncation defects or bypassing:

### 4.1 Measurement Point
- **Location:** Inside `runner.py:execute_turn()` immediately before invoking the subprocess, and inside the streaming event hook when an `item.completed` event signals a pending tool result turn.
- **Payload Inspected:** For Turn 0, the initial raw prompt string. For Turns $1..N$, because Codex manages conversational thread history internally in its local session rollout, the runner must inspect either:
  1. The accumulated length of stdout/stdin tool payload logs passed to the child process.
  2. The `last_turn_request_tokens` reported by the immediate predecessor turn plus the serialized byte length of the current tool execution output.

### 4.2 Token Estimation Methodology
- To remain strictly offline without remote API dependencies, use a local offline tokenizer library (e.g. `tiktoken` with `cl100k_base` / `o200k_base` offline weights).
- If offline BPE tokenization is unavailable, enforce a conservative character ratio:
  $$\text{Estimated Tokens} = \frac{\text{Serialized Characters}}{3.5}$$
  A $60,000$-token ceiling corresponds to $\le 210,000$ characters of accumulated context.

### 4.3 Handling Large Requests: Reject vs Truncate
- **Strict Rule: REJECT, DO NOT TRUNCATE.**
- Arbitrary string truncation of prompt payloads can corrupt JSON tool schemas, break TypeScript syntax, or strip required system instructions, causing undefined agent execution or silent hallucination.
- When an assembled turn payload exceeds $60,000$ tokens, the harness must **reject the dispatch**, raising `SafetyViolationError("PROMPT_INPUT_EXCEEDS_60K_GATE")` and cleanly terminating the arm with status `PRE_DISPATCH_PROMPT_CEILING_BREACHED`.

### 4.4 Exception & Error Handling
- If local token estimation fails (e.g. non-UTF-8 characters, tokenizer exception), the gate must **fail closed**, returning `is_budget_uncertain = True` and halting execution.
- Retries and continuation turns must re-evaluate the gate before dispatch. A retry request that inflates context cannot bypass the check.

---

## 5. Summary Conclusion on Request Boundaries

1. Client-side pre-dispatch request gating is currently **`ESTIMATED_ONLY`** based on previous-turn history; no hard pre-flight token barrier exists.
2. The model architectural context window ($258,400$ tokens) and completion burst limit ($16,384$ tokens) are **`PROVIDER_ENFORCED`**.
3. Telemetry accounting is **`POST_HOC_ONLY`**.
4. To prove an unconditional theoretical single-turn overshoot bound of $31,384$ tokens, the system must implement the reject-based client-side gate specified above.
