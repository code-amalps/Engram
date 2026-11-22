# Engram Amendment 05 — Milestone 2: Controlled Smoke-Test Readiness Report

## Executive Summary

This report documents the completion of **Milestone 2: Controlled Smoke-Test Readiness** under Engram Amendment 05. The primary objective is to complete all offline architectural, procedural, budgeting, and safety preparations required for a controlled pre-benchmark smoke test without executing live model inference or consuming inference credits.

All engineering and verification were conducted **strictly offline** with zero model inference, zero API calls, zero network activity, zero package installations, and zero spend (\$0.00).

---

## 1. Repository & Baseline Artifact Verification

A comprehensive read-only audit was conducted to confirm the integrity of all baseline assets prior to establishing smoke-test readiness:

### 1.1 Milestone 1 and 1A Closure Artifacts
The following five closure artifacts exist in `amendments/draft-05/` and are fully validated:
1. [`milestone-1-guard-validation-report.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-1-guard-validation-report.md)
2. [`milestone-1-test-results.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-1-test-results.md)
3. [`milestone-1-safety-limitations.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-1-safety-limitations.md)
4. [`milestone-1a-final-reconciliation.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-1a-final-reconciliation.md)
5. [`milestone-1a-final-reconciliation-summary.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-1a-final-reconciliation-summary.md)

### 1.2 Amendment 05 Protocol & Procedure Assets
- **Draft Protocol:** [`amendments/draft-05/amendment-05.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/amendment-05.md) is present and incorporates Owner Decisions 1 & 2 (175k ceiling inclusive of 15k reserve; stop trigger at 160k; overrun runs analyzed as censored data and ineligible for savings claims).
- **Smoke-Test Procedure:** [`amendments/draft-05/smoke-test-procedure.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/smoke-test-procedure.md) is present, specifying the 5-gate preflight protocol.

### 1.3 Canonical Binary Hashes Verification
All executable binaries under `benchmarks/matched-pair-harness/bin/` were queried via PowerShell `Get-FileHash -Algorithm SHA256`:

| Binary Target | File Path | Actual On-Disk SHA-256 | Canonical Reference SHA-256 | Status |
|:---|:---|:---:|:---:|:---:|
| **v1.5.1 Frozen** | `benchmarks/matched-pair-harness/bin/v1.5.1/engram.exe` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | **MATCH (100% PRESERVED)** |
| **v1.5.4 Frozen** | `benchmarks/matched-pair-harness/bin/v1.5.4/engram.exe` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | **MATCH (100% PRESERVED)** |
| **Candidate Phase 1** | `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | **MATCH (100% PRESERVED)** |

No binaries were rebuilt, altered, or replaced.

### 1.4 Golden Repository Integrity
- **Path:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`
- **Branch:** `test-branch` at commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`
- **Working Tree:** Exactly 18 modified files (`+208 / -16` lines) and untracked `.engram/` (29 files).
- **Preservation Status:** Preserved untouched and unmutated.

### 1.5 Historical Benchmark Evidence
All historical reports for Blocks 06, 07, 08 and Amendment 04 in `benchmarks/` remain bit-for-bit intact.

---

## 2. Billing & Quota Readiness Inspection

In accordance with strict authorization boundaries, local Codex configuration was inspected offline without exposing secrets, credentials, or API tokens.

### 2.1 Locally Established Authentication Configuration
Forensic inspection of `C:\Users\Amal P S\.codex\auth.json` reveals:
- **`auth_mode`:** `"chatgpt"` (ChatGPT Web / Subscription OAuth).
- **`OPENAI_API_KEY`:** `None` (no developer platform API key is configured).
- **OAuth Tokens:** Present in session store (`id_token`, `access_token`, `refresh_token`, `account_id`).

### 2.2 Critical Distinction: Subscription vs. Metered API Quota
Because authentication mode is `"chatgpt"`:
1. **No Metered USD API Invoicing:** Model calls do not draw down prepaid credit balances on `platform.openai.com`.
2. **Rolling Subscription Window Quota:** Model calls consume the user's active ChatGPT subscription usage quota (e.g. 3-hour sliding rate limit windows for advanced models like `gpt-5.6-luna`).
3. **What Local Configuration CANNOT Establish:**
   - **Subscription Plan Tier:** Unspecified locally (could be Plus, Team, Enterprise, or Pro).
   - **Remaining Window Quota:** Not cached locally.
   - **Usage Limits:** Specific token/message counts per 3-hour cycle are enforced dynamically by OpenAI servers.
   - **Automatic Additional Purchasing:** Local configuration files contain zero settings governing automatic credit add-ons or overdraft billing.
4. **Classification:**
   $$\mathbf{Subscription\ Details\ \&\ Auto-Purchase\ Status:\ OWNER\ VERIFICATION\ REQUIRED}$$
   The project owner must inspect the ChatGPT web interface settings and certify that auto-purchase / add-on billing is **strictly disabled** prior to authorizing live inference.

---

## 3. Smoke-Test Budget Design

### 3.1 Pilot Protocol vs. Smoke-Test Budget Distinction
- **Full Pilot Protocol Budget (Amendment 05):**
  - Per-arm ceiling: `175,000 tokens`
  - Headroom reserve: `15,000 tokens`
  - Nominal stop trigger: `160,000 tokens`
  - Two-arm maximum planned exposure: `350,000 tokens`
- **Smoke-Test Budget Requirement:**
  - A smoke test is a narrowly targeted single-arm diagnostic probe, **not a full pilot block**.
  - Allocating the full 175,000-token ceiling for a smoke test is completely unnecessary and exposes the user's subscription window to avoidable risk.
  - An independent, dedicated smoke-test budget must be established.

### 3.2 Smoke-Test Scope & Expected Token Consumption
The smoke test prompt is trivial and read-only:
> *"Locate and state the file path and exact action creator name for authLogout in the core auth module. Do not edit, format, or create any files."*

Based on verified empirical metrics from historical runs:
1. **Turn 0 Setup:** Initial system prompt injection + candidate MCP server tool manifest (4 tools) consumes **~1,500 to 2,000 prompt tokens**.
2. **Tool Invocation (Turn 1):** Agent autonomously calls `engram_get_context` or `engram_inspect_symbol`. The candidate MCP server returns bounded symbol metadata consuming **~1,000 to 2,500 prompt tokens**.
3. **Agent Synthesis (Turn 2):** Agent generates the concise answer and concludes, consuming **~100 to 300 completion tokens**.
4. **Total Expected Consumption:** **~3,000 to 6,000 tokens** across 1 or 2 turns.

### 3.3 Proposed Smoke-Test Budget Recommendation

To provide adequate operational margin while strictly preventing runaway loops:

| Budget Parameter | Option A (Recommended) | Option B (Conservative) | Rationale |
|:---|:---:|:---:|:---|
| **Smoke-Test Operational Ceiling** | **`10,000 tokens`** | `15,000 tokens` | Provides a 2x–3x buffer over expected usage (~4,000 tokens) while capping maximum possible loss. |
| **Headroom Reserve** | **`2,000 tokens`** | `3,000 tokens` | Sized to absorb a full final turn during process termination latency. |
| **Nominal Stop Threshold** | **`8,000 tokens`** | `12,000 tokens` | Pre-dispatch guard halts execution before Turn $N+1$ if remaining budget $< 2,000$. |
| **Wall-Clock Timeout** | **`60 seconds`** | `90 seconds` | Prevents hanging subprocesses or interactive prompts. |

$$\mathbf{Status:\ AWAITING\ OWNER\ APPROVAL\ (Gate\ 1)}$$
The agent will not adopt or execute this budget unilaterally. The owner must select and approve the cap.

### 3.4 Token Accounting & Abort Conditions
- **Real-Time Accounting:** Handled by `StreamingTokenTracker` via stdout polling (50 ms loop). Cumulative usage is calculated as $\max(\text{thread\_snapshots}, \text{turn\_deltas})$.
- **Reasoning Invariant:** Reasoning tokens are tracked as a completion sub-metric and **never double-counted**.
- **External Polling Latency Absorption:** As proven in Milestone 1/1A, Windows process termination (`taskkill /F /T`) requires **197 ms to 284 ms (median 229 ms)**. The 2,000-token headroom reserve ensures that even if Codex initiates an outbound request during the termination window, the final turn cannot push total consumption past the 10,000-token ceiling.

### 3.5 Abort & Termination Triggers:
1. **Token Ceiling Breach:** If cumulative tokens exceed `10,000`, process tree is terminated immediately (`TOKEN_CEILING_BREACHED`).
2. **Headroom Exhaustion:** If remaining budget is less than the 2,000-token reserve prior to next turn dispatch, execution halts gracefully (`PRE_DISPATCH_HEADROOM_EXHAUSTED`).
3. **Timeout:** If wall-clock time exceeds `60` seconds, process tree is forcefully terminated (`TIMEOUT`).
4. **Interactive Prompt:** If Codex prompts for console input on stdin, process is terminated (`INTERACTIVE_BLOCKED`).
5. **Write Attempt:** If child process attempts file modification on disk, execution is aborted (`WRITE_VIOLATION`).
6. **Process Crash:** If candidate binary panics, execution halts immediately (`PROCESS_CRASH`).

### 3.6 Success vs. Inconclusive/Invalid Criteria
- **SUCCESSFUL SMOKE TEST:** All 5 checks in `smoke-test-procedure.md` pass:
  1. CLI initializes and model ID `gpt-5.6-luna` is accepted.
  2. Candidate Engram binary discovers non-interactively with exactly 4 tools.
  3. Agent autonomously selects and invokes an Engram MCP tool without prompting.
  4. Authoritative model tokens $\le 10,000$ (or owner-approved cap).
  5. Zero files created, modified, or deleted in the workspace.
- **INCONCLUSIVE RESULT:**
  - Upstream model API rate-limit (HTTP 429) or transient network timeout prior to tool execution. Requires window cooldown without invalidating harness code.
- **INVALID / FAILED RESULT:**
  - Agent answers without calling Engram MCP tools (bypass failure).
  - Fewer than 4 tools registered by candidate binary.
  - Interactive approval prompt blocks headless execution.
  - Token cap exceeded.
  - Workspace mutation detected.

---

## 4. Execution Safety & Isolation Review

A rigorous safety review confirms that the smoke-test procedure adheres to all containment requirements:

1. **Disposable Scratch Sandbox:**  
   The smoke test runs in `scratch/smoke_test_sandbox_<timestamp>/`, isolated from repository roots.
2. **Zero Golden Repo Access:**  
   The golden repository (`Projects/Test/angular-ngrx-material-starter`) is not referenced in the smoke test command.
3. **Binary Protection:**  
   The candidate binary at `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` is executed strictly read-only.
4. **Evidence Segregation:**  
   Smoke test artifacts are saved exclusively to a dedicated directory:  
   `benchmarks/matched-pair-harness/reports/smoke_test/`. Historical benchmark reports (Blocks 06, 07, 08) cannot be overwritten.
5. **Non-Interactive MCP Policy:**  
   Codex is invoked with `-c mcp_servers.engram...` and `stdin=subprocess.DEVNULL`, preventing interactive hangs.
6. **Network Containment:**  
   No external packages or Git remotes are contacted. Network access is restricted to the established HTTPS model API endpoint.

---

## 5. Technical Blockers & Final Readiness Verdict

### Technical Blocker Audit:
- **Harness Code Status:** 212 / 212 offline tests pass with exit code 0.
- **Schema Validation:** `run_telemetry.schema.json` is validated.
- **Process Termination:** Verified clean with 0 orphans across 20 empirical trials.
- **Remaining Technical Blockers:** **NONE.**

### Governance Gating Audit:
While technical preparation is 100% complete, **execution is strictly blocked pending owner sign-off** on the three authorization gates defined in `milestone-2-authorization-gates.md`.

$$\mathbf{Milestone\ 2\ Readiness\ Classification:\ READY\ FOR\ OWNER\ AUTHORIZATION}$$

*(Execution remains strictly suspended until the project owner approves the token budget, certifies subscription billing, and signs the execution authorization.)*
