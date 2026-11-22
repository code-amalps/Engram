# Engram First-Edit Stopping Rule Feasibility Audit

**Document Identifier:** `ENGRAM_DRAFT05_FIRST_EDIT_RULE_AUDIT`  
**Governing Context:** Amendment 05 Variant B Evaluation (`COMPLETED_FIRST_EDIT`)  
**Inspected Documents:** `amendments/draft-05/amendment-05-draft-v2.md`, historical run rollouts, and harness runner  
**Status:** Strictly Offline — Zero Code or Configuration Mutations  

---

## 1. Audit of Proposed Event Identifiers (Observed vs. Proposed)

Amendment 05 Draft v2 proposed triggering the stop-at-first-edit rule on:
> `- item.type == "file_change" OR tool call is tools.apply_patch / tools.write_file`

A forensic check against raw historical event streams and session rollouts reveals the exact status of each identifier:

| Event Identifier | Real / Observed vs. Proposed | Exact Evidence Source & Syntax | Audit Finding |
| :--- | :---: | :--- | :--- |
| **`item.completed`** | **`OBSERVED`** | Stdout stream ([`baseline_arm1/events.jsonl#L20`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/baseline_arm1/events.jsonl#L20)) and session rollout line L65 (`payload.type: "item_completed"`). | Valid, real event type emitted by Codex CLI. |
| **`file_change`** | **`OBSERVED`** | Stdout stream ([`baseline_arm1/events.jsonl#L20`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/baseline_arm1/events.jsonl#L20)): `{"type":"item.completed","item":{"type":"file_change"}}`. | Real event value in standard output stream (lowercase snake_case). |
| **`FileChange`** | **`OBSERVED`** | Session rollout (`rollout-*.jsonl` line L65): `payload.item.type: "FileChange"`. | Real event value in internal rollout files (PascalCase). |
| **`tools.apply_patch`** | **`OBSERVED`** | Rollout line L63 (`custom_tool_call.input`): `text(await tools.apply_patch(patch));`. | Real JavaScript invocation string inside Codex execution environment. |
| **`tools.write_file`** | **`PROPOSAL — NOT VERIFIED`** | Never observed in any Task B rollout. | Unverified; Codex CLI used `tools.apply_patch` exclusively for code changes. |

---

## 2. Implementability with Existing Harness Hooks

### 2.1 Inspection of Active Runner Code
Inspection of `benchmarks/matched-pair-harness/src/runner.py` (both at commit `HEAD` and in working-tree remediation code) reveals:
1. **Absence of First-Edit Stopping Logic:**  
   The runner contains stopping conditions for:
   - `timed_out` ([`runner.py#L547`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L547))
   - `token_ceiling_breached` ([`runner.py#L564`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L564))
   - `pre_dispatch_guard_triggered` ([`runner.py#L571`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L571))
   There is **no code anywhere in the harness** that inspects `item.type == "file_change"` or terminates the child process upon file mutation.
2. **Lifecycle Mismatch:**  
   In `runner.py#L150`, tool lifecycle tracking checks:
   `if item_type in ("command_execution", "mcp_tool_call", "file_edit", "web_search", "tool_call"):`
   Notice that it checks `"file_edit"`, not `"file_change"` (the actual string emitted by Codex).
3. **Conclusion:**  
   **The first-edit rule is NOT implementable with existing hooks without changing the harness.** Enabling Variant B requires authoring and validating new event-handling code in `runner.py` and `orchestrator.py`.

---

## 3. Concurrency and Interception Timing

### 3.1 Can the Harness Reliably Halt Before Next Request Dispatch?
In Codex CLI's execution flow:
1. The model outputs a JavaScript script containing `await tools.apply_patch(patch)`.
2. The patch modifies files in the sandbox workspace.
3. Codex emits `{"type": "item.completed", "item": {"type": "file_change"}}` to stdout.
4. Codex immediately packages the tool output and, if the turn is complete, formats the prompt for the next turn and calls OpenAI.

With the current 50ms external poller:
- If the poller detects `item.completed` with `item.type == "file_change"`, it can issue `terminate_pid_tree(proc.pid)`.
- However, if the time between patch completion and next-turn network dispatch is $<50\text{ms}$, an outbound HTTP request may already be in flight before the process is killed.
- Therefore, external polling provides a **probabilistic**, not a deterministic zero-window guarantee.

### 3.2 Post-Edit Git Status or Diff Checks
- An external check of `git status` or `git diff` takes 50–200ms on Windows.
- Relying on external Git inspection to detect an edit and terminate the process is **too slow**; Codex will have already dispatched the subsequent model turn before the Git command completes.
- Interception **must** occur via synchronous stream inspection of stdout, not periodic filesystem polling.

---

## 4. Interaction Between Token Headroom Guard and First-Edit Rule

> [!CRITICAL]
> **Headroom Guard Preempts First Edit Under 175k Ceiling:**  
> As demonstrated in `reserve-simulation.md`:
> 1. In Block 06, exploration required **145,999 tokens** across Turns 1–6. First edit was attempted at Turn 7.
> 2. In Block 08, exploration required **151,470 tokens** across Turns 1–8. First edit was attempted at Turn 9.
> 3. Under a 175,000 ceiling with a 30,000–40,000 reserve, the token guard stops execution at:
>    - B06: **145,999 tokens** (Before Turn 7)
>    - B08: **151,470 tokens** (Before Turn 9)
> 
> In **100% of historical cases**, the token headroom guard stops the run **BEFORE the first edit is ever reached**. The first-edit stopping rule would never be triggered because the agent is killed for headroom exhaustion during exploration.

---

## 5. Formal Proposed Event Hook Definition (`PROPOSAL — NOT VERIFIED`)

If the owner decides to adopt Variant B and authorize harness changes in a future pass, the required implementation should be defined as follows:

```python
# PROPOSAL — NOT VERIFIED IN ACTIVE HARNESS
def is_first_edit_boundary(self, line: str) -> bool:
    """Detects first source edit completion in child stdout stream."""
    if not line:
        return False
    try:
        data = json.loads(line)
        if data.get("type") == "item.completed":
            item = data.get("item", {})
            if isinstance(item, dict):
                # Check both stdout snake_case and rollout PascalCase
                if item.get("type") in ("file_change", "FileChange"):
                    return True
        # Also check custom_tool_call script input
        if data.get("type") == "response_item":
            payload = data.get("payload", {})
            if payload.get("type") == "custom_tool_call":
                inp = payload.get("input", "")
                if "tools.apply_patch" in inp or "tools.write_file" in inp:
                    return True
    except Exception:
        pass
    return False
```
