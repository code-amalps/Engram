# Engram Amendment 05 — Smoke-Test Execution Checklist

**Governing Protocol:** Engram Amendment 05 (Milestone 2)  
**Document Path:** `amendments/draft-05/milestone-2/smoke-test-execution-checklist.md`  
**Execution Status:** **STRICTLY PENDING OWNER SIGN-OFF — ZERO LIVE INFERENCE WITHOUT AUTHORIZATION**  

---

## 1. Pre-Execution Technical Preconditions

All technical checks must be certified prior to initiating execution:

```text
[   ] Precondition 1: Git Working Tree Cleanliness (relative to harness assets)
      - Engram HEAD: c70845b4d5913457f1b176fb0370b80dd792711f
      - Branch: feature/async-precommit-guard

[   ] Precondition 2: Candidate Binary Cryptographic Integrity
      - Target: benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe
      - Expected SHA-256: F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE
      - Verification Command: Get-FileHash benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe

[   ] Precondition 3: Golden Reference Repository Isolation
      - Target: C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter
      - Baseline Commit: 1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2 (test-branch)
      - State: Exactly 18 modified files (+208/-16) and untracked .engram/ (29 files)
      - Status: Untouched and NOT referenced by the smoke-test command

[   ] Precondition 4: Dedicated Disposable Sandbox Allocation
      - Workspace Path: scratch/smoke_test_sandbox_<timestamp>/
      - Storage Type: Local isolated temporary directory

[   ] Precondition 5: Codex CLI Availability & Version Check
      - Executable Path: C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe
      - Verification: Executable responds to --version without error
```

---

## 2. Mandatory Owner Authorization Gates

Execution cannot proceed if any of the three authorization gates below remain unfulfilled:

```text
[   ] Authorization Gate 1: Smoke-Test Token Budget Approved
      - Selected Ceiling: [   ] Option A (10,000 tokens)   [   ] Option B (15,000 tokens)   [   ] Custom: ______
      - Headroom Reserve: [   ] 2,000 tokens (Option A)    [   ] 3,000 tokens (Option B)    [   ] Custom: ______
      - Nominal Stop Trigger: [   ] 8,000 tokens           [   ] 12,000 tokens              [   ] Custom: ______

[   ] Authorization Gate 2: Subscription Billing & Auto-Purchase Certification
      - Active Plan Tier: ____________________
      - Rolling Quota Window: Verified sufficient for <= 10,000 token test
      - Auto-Purchase / Add-On Billing: VERIFIED OFF in OpenAI/ChatGPT account settings

[   ] Authorization Gate 3: Explicit Owner Execution Sign-off
      - Signed Owner Statement: Formally recorded in milestone-2-authorization-gates.md
```

---

## 3. Step-by-Step Execution Sequence

When and only when all preconditions and authorization gates are signed:

### Step 3.1: Environment Initialization
1. Create isolated disposable scratch sandbox:
   ```powershell
   $SandboxPath = "C:\Users\Amal P S\OneDrive\Projects\Engram\scratch\smoke_test_$(Get-Date -Format 'yyyyMMdd_HHmmss')"
   New-Item -ItemType Directory -Path $SandboxPath -Force
   ```
2. Set up candidate Engram MCP configuration inside the scratch sandbox.

### Step 3.2: Execution Command Invocation
Execute the single-arm smoke-test runner using non-interactive stdin redirection:
```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --task smoke_test_read_only `
  --arms v154_arm3 `
  --max-tokens-ceiling 10000 `
  --headroom-reserve 2000 `
  --model gpt-5.6-luna `
  --reasoning low `
  --output-dir benchmarks/matched-pair-harness/reports/smoke_test
```

### Step 3.3: Real-Time Stream Monitoring
The monitoring harness continuously enforces:
1. **Stdout JSONL Parsing:** Real-time polling via `StreamingTokenTracker` (50 ms sleep loop).
2. **Pre-Dispatch Headroom Calculation:** If $\text{remaining\_tokens} < 2,000$, trigger immediate graceful stop (`PRE_DISPATCH_HEADROOM_EXHAUSTED`).
3. **Hard Ceiling Guard:** If cumulative tokens $\ge 10,000$, execute recursive process termination (`taskkill /F /T /PID`).
4. **Wall-Clock Watchdog:** If elapsed time exceeds 60 seconds, trigger `TIMEOUT` kill.

---

## 4. Immediate Abort Criteria

The execution harness must immediately terminate the child process tree if any of the following occur:

| Abort Condition | Threshold / Trigger | Action | Resulting Status |
|:---|:---|:---|:---:|
| **Cumulative Token Limit** | Tokens $\ge 10,000$ (or owner ceiling) | `terminate_pid_tree(pid)` | `TOKEN_CEILING_BREACHED` |
| **Headroom Exhaustion** | Budget $< 2,000$ tokens prior to turn | Halt turn dispatch | `PRE_DISPATCH_HEADROOM_EXHAUSTED` |
| **Execution Timeout** | Elapsed time $> 60.0\text{ s}$ | `terminate_pid_tree(pid)` | `TIMEOUT` |
| **Interactive Console Prompt** | Subprocess requests stdin input | `terminate_pid_tree(pid)` | `INTERACTIVE_BLOCKED` |
| **Workspace Mutation Attempt** | File creation/edit in repository | `terminate_pid_tree(pid)` | `WRITE_VIOLATION` |
| **Process Crash / Panic** | Candidate binary exits with code $\ne 0$ | Halt execution | `PROCESS_CRASH` |

---

## 5. Evidence Collection & Output Files

Immediately upon run completion, the following 5 evidence artifacts must be verified and saved to `benchmarks/matched-pair-harness/reports/smoke_test/`:

1. **`check1_cli_init.log`:** Captures CLI startup lines, version check, and model acceptance.
2. **`check2_mcp_discovery.jsonl`:** Captures MCP JSON-RPC initialize handshake registering all 4 tools.
3. **`check3_tool_call_correlation.json`:** Correlates Codex `tool_call` requesting Engram with candidate server JSON-RPC audit logs.
4. **`check4_token_telemetry.json`:** Full serialized telemetry including authoritative total tokens and `headroom_guard` metrics.
5. **`check5_sandbox_diff.txt`:** Git status and diff proving zero modifications to the workspace.

---

## 6. Mandatory Rollback & Teardown

1. **Delete Disposable Sandbox:**
   ```powershell
   if (Test-Path $SandboxPath) {
       Remove-Item -Path $SandboxPath -Recurse -Force
       Write-Output "Smoke test scratch sandbox safely discarded."
   }
   ```
2. **Verify Golden Repository State:**
   Execute read-only verification confirming commit `1c2856086f3a...` remains clean of any smoke-test activity.

---

## 7. Pass / Fail Acceptance Matrix

| Verification Gate | Required Pass Criterion | Fail Consequence |
|:---|:---|:---|
| **Gate 1: CLI & Model Acceptance** | Codex initializes headless, accepts `gpt-5.6-luna`, no HTTP 401/403/404. | Immediate abort; report CLI compatibility blocker. |
| **Gate 2: Non-Interactive MCP Discovery** | Exactly 4 tools discovered without console approval prompt. | Immediate abort; report MCP configuration blocker. |
| **Gate 3: Autonomous Tool Selection** | Agent invokes `engram_get_context` or `engram_inspect_symbol` without prompt naming. | Test failed; candidate agent autonomy invalidated. |
| **Gate 4: Strict Token Bounding** | Authoritative model tokens $\le 10,000$ (or approved ceiling). | Test failed; report token guard failure. |
| **Gate 5: Zero Working Tree Mutation** | 0 files created, modified, or deleted in workspace. | Test failed; write isolation invalidated. |

**Overall Result Verdict:**  
$$\text{ACCEPTANCE} = \text{Gate 1} \land \text{Gate 2} \land \text{Gate 3} \land \text{Gate 4} \land \text{Gate 5}$$
If any single gate fails, the smoke test is formally classified as **FAILED / INVALID**.
