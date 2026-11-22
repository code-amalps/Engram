# Engram MCP Usage and Autonomy Forensic Analysis (Blocks 06, 07, 08)

**Document Identifier:** `ENGRAM_DRAFT05_ENGRAM_USAGE_ANALYSIS`  
**Governing Context:** Engram Amendment 05 Pass 4 Offline Feasibility Investigation  
**Authoritative Evidence Sources:**  
- Block 06 Baseline Arm 1: [`benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/baseline_arm1/run_telemetry.json`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/baseline_arm1/run_telemetry.json)  
- Block 07 Candidate Arm 3: [`benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/v154_arm3/events.jsonl`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/v154_arm3/events.jsonl) (lines 6–7)  
- Block 08 Candidate Arm 3: [`benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_08/v154_arm3/events.jsonl`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_08/v154_arm3/events.jsonl)  

---

## 1. Executive Summary

This forensic investigation traces every attempted, approved, executed, and completed Engram MCP tool invocation across the three historical live Task B executions (Blocks 06, 07, and 08).

### Primary Findings:
1. **Total Engram MCP Invocations Across All Runs:** Exactly **1 attempted call** across 36 total model turns.
2. **Autonomous Dispatch:** In Block 07, the agent autonomously decided to invoke Engram (`engram_inspect_symbol`) on Turn 2 without human prompting.
3. **Approval Failure:** In Block 07, the call was blocked locally because the runner configured the Codex client approval policy to `"never"`. The call never reached the Engram MCP server process.
4. **Tool Abandonment & Exploration Regression:** In Block 07, following the approval failure, the agent immediately abandoned Engram MCP tools and reverted to exhaustive native shell file reads and recursive `rg` queries. In Block 08, the agent never attempted to invoke Engram at all.

---

## 2. Block-by-Block Engram Invocation Forensic Record

### 2.1 Block 06: Baseline Arm 1 (`baseline_arm1`)
- **MCP Server State:** `DISABLED` (Baseline unassisted arm).
- **Number of Engram MCP Tool Calls:** **0** (`NONE`).
- **Turn & Cumulative Tokens at First Call:** `NONE`.
- **Autonomous Invocation:** **No** (MCP server was not provided to the agent).
- **Response Size:** `N/A`.
- **Subsequent Action:** Agent executed native shell commands (`Get-Content`, `rg`, `apply_patch`).

---

### 2.2 Block 07: Candidate Arm 3 (`v154_arm3`)
- **MCP Server State:** `ACTIVE` (Engram Phase 1 binary loaded).
- **Number of Engram MCP Tool Calls:** **1 Attempted** / **0 Executed** / **0 Successful**.
- **Turn & Cumulative Tokens at First Call:** **Turn 2**, cumulative tokens: **32,397 tokens**.
- **Event Evidence:**
  - Dispatched at `events.jsonl` line 6:
    ```json
    {
      "type": "item.started",
      "item": {
        "id": "item_3",
        "type": "mcp_tool_call",
        "server": "engram",
        "tool": "engram_inspect_symbol",
        "arguments": {
          "target": "projects/angular-ngrx-material-starter/src/app/core/core.module.ts",
          "depth": 2
        },
        "status": "in_progress"
      }
    }
    ```
  - Failed at `events.jsonl` line 7:
    ```json
    {
      "type": "item.completed",
      "item": {
        "id": "item_3",
        "type": "mcp_tool_call",
        "server": "engram",
        "tool": "engram_inspect_symbol",
        "result": null,
        "error": {
          "message": "MCP tool call requires approval, but approval policy is never"
        },
        "status": "failed"
      }
    }
    ```
- **Call Lifecycle Classification:**
  - **Attempted:** `YES` (Dispatched by agent).
  - **Approved:** `NO` (Approval policy was `"never"`).
  - **Executed:** `NO` (Harness runner blocked execution before dispatch to MCP server).
  - **Successful:** `NO` (Execution failed).
- **Serialized Response Size:**
  - `result`: `null` (0 bytes).
  - `error.message`: `"MCP tool call requires approval, but approval policy is never"` (**65 UTF-8 bytes**; approximately 16 tokens under OpenAI `o200k_base` tokenization).
- **Agent Behavior Post-Failure:**
  - The agent completely abandoned Engram MCP tools.
  - In Turn 3 (item_8), it executed a broad recursive PowerShell search:
    `git status --short; rg -n "Login|login|username|remember" projects/.../src/assets projects/.../src/app`
  - It proceeded across Turns 4–11 to read entire files via `Get-Content` (`core.module.ts`, `app.component.ts`, `auth.effects.ts`, `core.state.ts`).
  - **Did it read/list files it could have skipped?** **Yes.** Had `engram_inspect_symbol` returned symbol definitions, the agent would not have needed to manually inspect multiple sibling files.
- **Autonomous Invocation:** **YES**. The agent initiated the MCP call autonomously based solely on system prompt tool definitions.

---

### 2.3 Block 08: Candidate Arm 3 (`v154_arm3`)
- **MCP Server State:** `ACTIVE` (Engram Phase 1 binary loaded; approval policy remediated).
- **Number of Engram MCP Tool Calls:** **0** (`NONE`).
- **Turn & Cumulative Tokens at First Call:** `NONE`.
- **Autonomous Invocation:** **No**. The agent was provided the Engram MCP server but chose not to invoke it.
- **Response Size:** `N/A`.
- **Observed Behavior:**
  - From Turn 1, the agent opted exclusively for native shell commands (`Get-Content` on `core.module.ts`, `rg` on `projects/angular-ngrx-material-starter/src/app`).
  - Across Turns 1–8, the agent executed 8 consecutive PowerShell commands, consuming **151,470 tokens** in manual codebase exploration before authoring its first patch at Turn 9.

---

## 3. Comparative Summary Table

| Metric | Block 06 Baseline Arm 1 | Block 07 Candidate Arm 3 | Block 08 Candidate Arm 3 |
| :--- | :---: | :---: | :---: |
| **Engram Server Provisioned** | No (`DISABLED`) | Yes (`ACTIVE`) | Yes (`ACTIVE`) |
| **Attempted MCP Calls** | 0 | **1** | 0 |
| **Approved MCP Calls** | 0 | **0** | 0 |
| **Executed MCP Calls** | 0 | **0** | 0 |
| **Successful MCP Calls** | 0 | **0** | 0 |
| **Turn at First MCP Attempt** | `NONE` | **Turn 2** | `NONE` |
| **Cumulative Tokens at First Attempt**| `NONE` | **32,397 tokens** | `NONE` |
| **Response Payload Size** | `N/A` | **65 bytes** (0 result, 65-byte error) | `N/A` |
| **Post-Call Regression to Shell** | `N/A` | **Immediate (Turn 3 rg & file reads)** | `N/A` (Shell used from Turn 1) |
| **Autonomous Model Intent** | No | **YES (Autonomous decision to call)** | No (Agent ignored MCP tools) |
