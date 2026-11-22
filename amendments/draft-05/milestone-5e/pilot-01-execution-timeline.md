# Engram Milestone 5E — Deliverable 1
# pilot_01 Execution Timeline and Turn-by-Turn Forensic Chronology

## 1. Executive Summary

This document provides a forensic, millisecond-accurate timeline and turn-by-turn analysis of the authorized $N=1$ matched-pair pilot experiment (`pilot_01`), executed on **2026-10-30**.

The experiment was configured as a two-arm matched pair:
- **Treatment Arm (`treatment_B`)**: Candidate Phase 1 Engram binary (`v0.1.0-alpha.1`, SHA-256: `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`), MCP enabled, configured token ceiling: 175,000 tokens, pre-dispatch headroom reserve: 45,000 tokens.
- **Control Arm (`control_A`)**: Unassisted baseline, configured to execute second in sequence.

The treatment arm ran for **71.12 seconds**, issued **4 model requests**, consumed **117,135 total model tokens** without editing any project files, and was terminated by the harness pre-dispatch headroom guard before request 5 could be dispatched. In accordance with the orchestrator's fail-fast policy, the control arm was skipped.

---

## 2. High-Level Run Metadata

| Attribute | Treatment Arm (`treatment_B`) | Control Arm (`control_A`) |
| :--- | :--- | :--- |
| **Run ID** | `task_commit_cache_invalidation_pilot_01_treatment_B` | `task_commit_cache_invalidation_pilot_01_control_A` |
| **Execution Order** | 1 (Executed first) | 2 (Skipped) |
| **Execution Status** | `PRE_DISPATCH_HEADROOM_EXHAUSTED` | `SKIPPED_STOP_ON_FAILURE` |
| **Start Timestamp** | `2026-10-30T10:42:13.540506+00:00` | `2026-10-30T10:43:26.576312+00:00` |
| **End Timestamp** | `2026-10-30T10:43:24.657728+00:00` | `2026-10-30T10:43:26.576312+00:00` |
| **Wall Clock Elapsed** | **71.12 seconds** | 0.0 seconds |
| **Model** | `openai/gpt-5.6-luna` | `openai/gpt-5.6-luna` |
| **Harness CLI** | `codex-cli 0.164.0-alpha.11.1` | N/A (skipped) |
| **Sandbox Path** | `AppData\Local\Temp\engram_pilot\runs\...treatment_B_20261030_161210` | N/A |
| **Total Model Tokens** | **117,135** | 0 |
| **Prompt Tokens** | **115,992** | 0 |
| **Completion Tokens** | **1,143** (Reasoning: 500) | 0 |
| **Peak Single Request Context**| **41,011 tokens** | N/A |
| **MCP Tool Calls** | **0** | 0 |
| **Files Modified** | **0** (0 lines inserted, 0 deleted) | 0 |
| **Functional Score** | **0.00 / 1.00 (FAIL)** | **0.00 / 1.00 (NOT_EVALUATED)** |

---

## 3. Turn-by-Turn Execution Matrix

The treatment arm executed 4 distinct model interaction turns (requests) within a single Codex session thread (`01a18c67-2e83-7b51-9326-bf2dcf886752`). The table below presents the verified provider-reported metrics extracted directly from the raw Codex rollout log (`rollout-2026-10-30T16-12-15-01a18c67-2e83-7b51-9326-bf2dcf886752.jsonl`).

| Request # | Rollout Event | Request Input Tokens | Cached Input Tokens | Uncached Input Tokens | Output Tokens | Reasoning Tokens | Total Request Tokens | Cumulative Prompt Tokens | Cumulative Total Tokens | Primary Agent Action |
| :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :--- |
| **Req 1** | Line 16 | 14,648 | 0 | 14,648 | 183 | 26 | 14,831 | 14,648 | 14,831 | Initial repository survey via PowerShell `rg` |
| **Req 2** | Line 23 | 25,521 | 14,464 | 11,057 | 167 | 11 | 25,688 | 40,169 | 40,519 | Multi-file dump (6 files, 220 lines each) |
| **Req 3** | Line 30 | 34,812 | 25,344 | 9,468 | 210 | 37 | 35,022 | 74,981 | 75,541 | Deep file slice (`analyzer.rs`, `storage.rs`) |
| **Req 4** | Line 37 | 41,011 | 34,688 | 6,323 | 583 | 427 | 41,594 | 115,992 | 117,135 | Engram dir inspect + `main.rs` lines 35-1685 |
| **Pre-5** | Harness | — | — | — | — | — | — | — | — | **Harness Pre-Dispatch Guard Halt** |

---

## 4. Millisecond Forensic Chronology

### Phase 1: Initialization & Environment Setup (10:42:13 – 10:42:25)
- **10:42:13.540**: Orchestrator initiates `treatment_B`. Clean temporary sandbox created at `AppData\Local\Temp\engram_pilot\runs\task_commit_cache_invalidation_pilot_01_treatment_B_20261030_161210`.
- **10:42:13.620**: Golden repository checked out at baseline commit `c70845b4d5913457f1b176fb0370b80dd792711f`. Cold-start state verified.
- **10:42:14.210**: Codex CLI spawned with command arguments:
  ```text
  codex.exe exec --json --model gpt-5.6-luna -c mcp_servers.engram.command=... -c mcp_servers.engram.args=...
  ```
- **10:42:24.469**: Codex session initialized (Session ID: `01a18c67-2e83-7b51-9326-bf2dcf886752`).
- **10:42:25.500 (Line 2)**: Codex runtime declares `additional_tools`. The declared tools belong exclusively to namespaces `functions` (`exec`, `wait`, `request_user_input`) and `mcp__cua_repl` (`js`, `js_reset`). **Zero Engram tools are declared or registered.**
- **10:42:25.659 (Line 9)**: Task prompt injected into conversation. Prompt contains task instructions for `task_commit_cache_invalidation`. No compiled Engram context is embedded.

### Phase 2: Request 1 — Broad Repository Reconnaissance (10:42:27 – 10:42:50)
- **10:42:27.239 (Line 12)**: Model enters reasoning (26 reasoning tokens).
- **10:42:28.156 (Line 15)**: Agent invokes `functions.exec` to run PowerShell command:
  ```powershell
  Get-ChildItem -Force; rg -n "ensure_fresh|index\.db|memories\.json|clean|meta|HEAD|git" src tests -S
  ```
- **10:42:28.512 (Line 16)**: Provider records Request 1 token usage:
  - Input: 14,648 tokens (0 cached, 14,648 uncached).
  - Output: 183 tokens (26 reasoning).
  - Total turn tokens: 14,831.
- **10:42:50.570 (Line 17–18)**: Command finishes (wall time 22.0s). Output size: 2,752 characters.

### Phase 3: Request 2 — Catastrophic Multi-File Dump (10:42:51 – 10:43:04)
- **10:42:51.577 (Line 21)**: Model reasons (11 reasoning tokens).
- **10:42:52.343 (Line 22)**: Agent invokes `functions.exec` with bulk dump script:
  ```powershell
  $files=@('src/analyzer.rs','src/compiler.rs','src/main.rs','src/storage.rs','src/incremental.rs','Cargo.toml');
  foreach($f in $files){
      Write-Output "--- $f";
      if(Test-Path $f){ Get-Content $f | Select-Object -First 220 }
  }
  ```
- **10:42:52.815 (Line 23)**: Provider records Request 2 token usage:
  - Single request input: 25,521 tokens (14,464 cached, 11,057 uncached).
  - Single request output: 167 tokens (11 reasoning).
  - Cumulative thread usage: 40,169 input tokens, 350 output tokens (Total: 40,519).
- **10:43:04.762 (Line 24–25)**: Tool execution completes (wall time 12.0s).
  - Output size: **42,576 characters**, 1,146 lines.
  - Codex injects truncation warning: `Warning: truncated output (original token count: 10094)`.

### Phase 4: Request 3 — Deep Source Code Slicing (10:43:05 – 10:43:18)
- **10:43:05.869 (Line 28)**: Model reasons (37 reasoning tokens).
- **10:43:06.792 (Line 29)**: Agent invokes `functions.exec` to inspect middle slices of large files:
  ```powershell
  $a=Get-Content src/analyzer.rs; $a[560..900];
  Write-Output '--- STORAGE REST';
  $s=Get-Content src/storage.rs; $s[180..330];
  Write-Output '--- MAIN COMMANDS/MATCH';
  rg -n "CommandLine::|match command|cache_dir|derived_symbols|memories" src/main.rs src/storage.rs src/analyzer.rs -S
  ```
- **10:43:07.236 (Line 30)**: Provider records Request 3 token usage:
  - Single request input: 34,812 tokens (25,344 cached, 9,468 uncached).
  - Single request output: 210 tokens (37 reasoning).
  - Cumulative thread usage: 74,981 input tokens, 560 output tokens (Total: 75,541).
- **10:43:18.644 (Line 31–32)**: Tool completes (wall time 11.5s). Output size: **25,513 characters**.

### Phase 5: Request 4 & Headroom Guard Interception (10:43:22 – 10:43:24)
- **10:43:22.915 (Line 35)**: Model reasons extensively (427 reasoning tokens).
- **10:43:23.784 (Line 36)**: Agent invokes `functions.exec` to examine `.engram` cache directory and `main.rs`:
  ```powershell
  Get-ChildItem .engram -Force | Format-Table Name,Length,Mode;
  $m=Get-Content src/main.rs; $m[35..95]; $m[570..680]; $m[1610..1685];
  rg -n "fn main|fn init|cache" src/main.rs -S
  ```
- **10:43:24.259 (Line 37)**: Provider records Request 4 token usage:
  - Single request input: 41,011 tokens (34,688 cached, 6,323 uncached).
  - Single request output: 583 tokens (427 reasoning).
  - Cumulative thread usage: **115,992 input tokens**, 1,143 output tokens (Total: **117,135**).
- **10:43:24.651**: Harness `StreamingTokenTracker` processes Line 37 event.
  - Due to a variable substitution bug in `runner.py`, `last_turn_request_tokens` is populated with `payload["turn_token_usage"]["input_tokens"]` (115,992) rather than `payload["usage"]["input_tokens"]` (41,011).
  - Pre-dispatch guard calculates estimated next turn cost:
    $$\text{estimated\_next\_turn} = \max(115992, 45000) = 115,992 \text{ tokens}$$
  - Guard compares against remaining budget:
    $$\text{remaining\_budget} = 175,000 - 117,135 = 57,865 \text{ tokens}$$
  - Guard checks condition: $57,865 < 115,992 \implies \mathbf{TRUE}$.
  - Guard triggers `PRE_DISPATCH_HEADROOM_EXHAUSTED`!
- **10:43:24.657**: Harness kills Codex child process tree. Run status recorded as `PRE_DISPATCH_HEADROOM_EXHAUSTED`.

### Phase 6: Post-Run Evaluation & Pair Cessation (10:43:24 – 10:43:26)
- **10:43:24.700**: Evaluator inspects sandbox. Git working tree is completely clean (`changes.patch` is 0 bytes).
- **10:43:24.750**: Evaluator records 0.00 score across all requirements (REQ-1 to REQ-4: `FAIL`).
- **10:43:26.576**: Orchestrator encounters stop-on-failure condition. Control arm (`control_A`) is marked `SKIPPED_STOP_ON_FAILURE`. Pilot completes.

---

## 5. Timeline Synthesis & Observations

```mermaid
sequenceDiagram
    autonumber
    participant H as Harness Orchestrator
    participant G as Pre-Dispatch Guard
    participant C as Codex CLI / Agent
    participant M as Model (gpt-5.6-luna)

    H->>C: Spawn codex exec with -c mcp_servers.engram...
    Note over C: MCP Server NOT initialized into additional_tools
    C->>M: Request 1 (Prompt only, 14,648 tokens)
    M->>C: Tool Call 1: exec (rg "ensure_fresh...")
    C->>M: Request 2 (+2.7KB output, 25,521 tokens)
    M->>C: Tool Call 2: exec (bulk dump 6 files x 220 lines)
    C->>M: Request 3 (+42.5KB output, 34,812 tokens)
    M->>C: Tool Call 3: exec (analyzer.rs 560..900, storage.rs)
    C->>M: Request 4 (+25.5KB output, 41,011 tokens)
    M->>C: Tool Call 4: exec (Get-ChildItem .engram, main.rs)
    C->>G: Usage Event (Req 4: 41,011; Thread: 115,992)
    Note over G: Tracker misassigns last_turn = 115,992!<br/>Budget: 57,865 < 115,992
    G->>H: PRE_DISPATCH_HEADROOM_EXHAUSTED
    H->>C: Terminate Process Tree (Kill)
    H->>H: Skip control_A (Stop-on-failure)
```

1. **Complete Absence of Engram Tools**: At no point did the agent call an Engram tool, because no Engram tools were registered in the session.
2. **Context Growth Velocity**: Context grew by an average of 8,787 tokens per turn (14,648 $\to$ 25,521 $\to$ 34,812 $\to$ 41,011) solely through large PowerShell file dumps.
3. **Execution Deadlock**: The agent spent 100% of its runtime performing discovery via text dumping, leaving zero tokens or turns for actual code modification before being halted.
