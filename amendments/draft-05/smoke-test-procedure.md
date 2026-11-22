# Engram Pre-Benchmark Smoke Test Procedure (Specification Only)

**Document Path:** `amendments/draft-05/smoke-test-procedure.md`  
**Status:** SPECIFICATION ONLY — **STRICTLY NOT EXECUTED — REQUIRES SEPARATE EXPLICIT AUTHORIZATION**  
**Purpose:** Precise five-gate preflight protocol to validate CLI compatibility, non-interactive MCP handshake, genuine unprompted tool invocation, and token bounding on a disposable workspace before any full pilot block under Proposed Amendment 05.  
**Mandatory Safety Rule:** **Preparing this procedure does NOT constitute authorization to run it. Zero live inference or model API requests may be initiated without separate, explicit human authorization.**

---

## 1. Scope, Constraints, and Environment Setup

1. **Isolation Mandate:**
   - The test must execute exclusively within an isolated disposable temporary directory created under `scratch/smoke_test_sandbox_<timestamp>/`.
   - The golden repository (`C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`) and its `.engram/` state must **NEVER** be accessed or targeted directly.
2. **Binary Verification Gate:**
   - The candidate executable must match the ratified SHA-256 hash prior to execution:
   - Path: `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`
   - SHA-256: `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE`
3. **Execution Parameter Bounds:**
   - **Operational Token Cap:** **`[ OWNER TO SET ]`** (Placeholder: Recommended 5,000 to 10,000 tokens maximum).
   - **Wall-Clock Timeout:** `60` seconds maximum.
   - **Model / Reasoning:** `gpt-5.6-luna` / `low` or `medium`.
   - **Trivial Read-Only Prompt (Unprompted):**
     `"Locate and state the file path and exact action creator name for authLogout in the core auth module. Do not edit, format, or create any files."`

---

## 2. The Five Verification Checks

The smoke test evaluates five strict sequential verification checks:

### Check 1: Model ID Acceptance & CLI Version
- **Objective:** Verify that Codex CLI executes headless, recognizes `gpt-5.6-luna`, and starts without deprecation or authentication rejection.
- **Pass Criteria:**
  - Subprocess initializes within $< 10$ seconds.
  - Returns valid JSONL initialization events (`turn.started` or `thread.started`).
  - No HTTP 401 (Unauthorized), 403 (Forbidden), 404 (Model Not Found), or invalid flag errors.
- **Fail Criteria:** Non-zero exit code, error payload indicating unrecognized model, or missing authorization.
- **Evidence File to Save:** `benchmarks/matched-pair-harness/reports/smoke_test/check1_cli_init.log`

### Check 2: Non-Interactive MCP Handshake & Tool Discovery
- **Objective:** Verify that Codex CLI discovers the candidate Engram MCP server non-interactively without approval hangs.
- **Pass Criteria:**
  - Subprocess launches `candidate-phase1/engram.exe` via `-c mcp_servers.engram...`.
  - JSON-RPC initialize handshake completes cleanly.
  - Exactly the 4 consolidated Phase 1 tools are registered:
    `engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, `engram_postflight`.
  - Zero interactive approval prompts on stdin (`stdin=subprocess.DEVNULL`).
- **Fail Criteria:** Handshake times out (>15s), candidate process crashes, or fewer than 4 tools discovered.
- **Evidence File to Save:** `benchmarks/matched-pair-harness/reports/smoke_test/check2_mcp_discovery.jsonl`

### Check 3: Genuine Autonomous MCP Tool Selection (Unprompted)
- **Objective:** Verify that the agent autonomously invokes an Engram MCP tool without being instructed to use tools.
- **Pass Criteria:**
  - The test prompt contains **zero tool names** or directives to call MCP.
  - Codex session rollout (`rollout-*.jsonl`) records a valid `tool_call` requesting an Engram tool (e.g. `engram_get_context` or `engram_inspect_symbol`).
  - Engram candidate server log records an incoming JSON-RPC call matching the request ID and timestamp from Codex.
- **Fail Criteria:** Agent fails to invoke any Engram tool, or requires explicit prompt naming of the tool.
- **Evidence File to Save:** `benchmarks/matched-pair-harness/reports/smoke_test/check3_tool_call_correlation.json`

### Check 4: Strict Token Cap Bounding
- **Objective:** Verify that token consumption is bounded strictly below the owner-designated cap.
- **Pass Criteria:**
  - Authoritative total model tokens ($T_{\text{model}}$) extracted from session rollout $\le \mathbf{[ OWNER\ TO\ SET ]}$ (e.g. $\le 10,000$ tokens).
  - Pre-dispatch guard remains armed and active throughout execution.
- **Fail Criteria:** $T_{\text{model}} > \mathbf{[ OWNER\ TO\ SET ]}$, or missing token telemetry.
- **Evidence File to Save:** `benchmarks/matched-pair-harness/reports/smoke_test/check4_token_telemetry.json`

### Check 5: Zero Working-Tree and Golden-Repo Mutation
- **Objective:** Verify complete write isolation and non-destructive read-only operation.
- **Pass Criteria:**
  - `git status --porcelain` on the disposable sandbox is completely empty (0 files modified, added, or deleted).
  - Original Golden Repository commit remains `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` with 18 modified files and identical `.engram/` hashes.
- **Fail Criteria:** Any file created, modified, or deleted within the repository workspace.
- **Evidence File to Save:** `benchmarks/matched-pair-harness/reports/smoke_test/check5_sandbox_diff.txt`

---

## 3. Immediate Abort Criteria

Execution must be forcibly terminated via process-tree kill (`taskkill /F /T /PID`) if:
1. Cumulative tokens exceed the designated **`[ OWNER TO SET ]`** cap at any point.
2. Execution elapsed time exceeds `60` seconds.
3. Subprocess prompts for interactive console input.
4. Codex child process attempts to execute file modification commands (`touch`, `git checkout`, `sed`, `write_file`).
5. Candidate binary crashes or terminates with a panic.

---

## 4. Rollback and Cleanup Procedure

Immediately upon completion of the smoke test (whether PASS or FAIL):
1. **Evidence Preservation:**
   All 5 evidence files must be saved to `benchmarks/matched-pair-harness/reports/smoke_test/`.
2. **Sandbox Discard (Mandatory Rollback):**
   The disposable temporary directory must be completely deleted:
   ```powershell
   # Mandatory rollback: Discard disposable sandbox
   if (Test-Path $DisposableSandboxPath) {
       Remove-Item -Path $DisposableSandboxPath -Recurse -Force
       Write-Output "Disposable sandbox safely discarded."
   }
   ```
3. **Golden Repository Verification:**
   Re-verify `git status --short` and `git rev-parse HEAD` on the golden repository to confirm zero contamination.

---

## 5. Execution Template (Strictly Awaiting Authorization)

```powershell
# TEMPLATE ONLY — DO NOT EXECUTE WITHOUT EXPLICIT OWNER SIGN-OFF AND ALLOCATED TOKEN CAP
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --task smoke_test_read_only `
  --arms v154_arm3 `
  --max-tokens-ceiling [ OWNER TO SET: e.g. 10000 ] `
  --headroom-reserve 2000 `
  --model gpt-5.6-luna `
  --reasoning low
```

---

## 6. Pre-Execution Sign-Off Checklist

```
[   ] Authorization Gate: Explicit owner sign-off received
[   ] Operational Token Cap set: [ OWNER TO SET: ____________ tokens ]
[   ] Candidate binary hash verified (F56347EF...23D85BEE)
[   ] Disposable scratch sandbox path allocated
[   ] Golden repository integrity verified
```
