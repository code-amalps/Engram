# Token Control Readiness: N=1 Matched-Pair Pilot

**Milestone:** Engram N=1 Matched-Pair Pilot Final Preflight  
**Document ID:** `amendments/draft-05/n1-pilot-preflight/token-control-readiness.md`  
**Evaluation Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Status:** AUDITED & RIGOROUSLY QUALIFIED  

---

## 1. Configured Token Control Parameters

The pilot experiment relies on the streaming token tracker and pre-dispatch guard implemented in `benchmarks/matched-pair-harness/src/runner.py` (`StreamingTokenTracker`) and parameterized via `orchestrator.py`:

| Parameter | Configured Value | Harness Role |
|---|---|---|
| Per-Arm Nominal Ceiling (`--max-tokens-ceiling`) | `175,000` tokens | Nominal planning budget for each individual arm |
| Pre-Dispatch Reserve (`--headroom-reserve`) | `45,000` tokens | Safety buffer reserved for in-flight turn completion |
| Pre-Dispatch Stop Threshold | `130,000` tokens | Pre-dispatch boundary ($175,000 - 45,000$) |
| Combined Planned Nominal Exposure | `350,000` tokens | Total nominal budget across Control A + Treatment B |

---

## 2. Technical Capabilities of the Local Pre-Dispatch Guard

### 2.1 Pre-Dispatch Evaluation Mechanics
In `runner.py`, the `StreamingTokenTracker` and `CodexRunner` evaluate token expenditure at the boundaries of each conversational turn:
1. `StreamingTokenTracker` monitors live streaming events from the agent runtime (`turn.completed`, `token_usage_record`).
2. Before dispatching Turn $N+1$, the runner computes:
   $$\text{Remaining Headroom} = \text{Nominal Ceiling} (175,000) - \text{Cumulative Usage}$$
3. If $\text{Cumulative Usage} \ge 130,000$ (i.e., $\text{Remaining Headroom} \le 45,000$), the pre-dispatch guard triggers:
   - Further model dispatches are aborted immediately.
   - The arm is marked `PRE_DISPATCH_HEADROOM_EXHAUSTED`.
   - The child Codex process is terminated cleanly.

### 2.2 Mathematical Deduplication & Double-Counting Prevention
- Invariants in `StreamingTokenTracker` distinguish between cumulative thread snapshots and incremental turn deltas.
- Reasoning tokens are treated as an informative sub-metric of completion tokens and are never added twice.
- Cached prompt tokens are tracked separately and not double-counted into net model generation.

---

## 3. Strict Qualifications and Exposure Realities

The token controls described above operate entirely within the client harness. To prevent misrepresentation of financial or operational risk, the following realities are explicitly established:

### 3.1 Nominal Planning Values vs. Provider Hard Limits
> [!IMPORTANT]
> The figures $175,000$ and $350,000$ are **nominal planning values**, NOT provider-enforced hard caps. The local harness cannot guarantee that provider-side usage will strictly terminate at exactly $175,000$ or $350,000$ tokens.

### 3.2 In-Flight Turn Request Overrun Exposure
A request dispatched across the network to the model provider cannot be paused or truncated mid-generation by a pre-dispatch check:
- If cumulative usage stands at $129,500$ tokens immediately prior to Turn 5, the pre-dispatch guard evaluates $129,500 < 130,000$ and permits dispatch.
- If the model generates a comprehensive response consuming $48,000$ tokens during that turn (larger than the $45,000$ reserve), the cumulative turn usage will register at $177,500$ tokens upon completion.
- While the post-turn check will immediately halt further turns with `TOKEN_CEILING_BREACHED`, the terminal turn will have exceeded the nominal $175,000$ budget.
- Historical data from prior Task B runs recorded maximum single-turn deltas of $37,651$ to $47,200$ tokens. While the $45,000$ reserve covers typical turns, worst-case turn bursts can cause nominal budget overshoots.

### 3.3 Asynchronous Remote Generation After Process Termination
When the local Python harness detects a ceiling breach or timeout, it terminates the local Codex process (`ProcessTracker.kill()` / `taskkill`). However:
- Terminating the local client process severs the client-side socket connection.
- Depending on backend API gateway semantics, OpenAI server-side GPU workers may continue generating tokens until output buffers saturate or internal cancellation propagates.
- Provider-side usage telemetry or account allowances may reflect tokens generated during this cancellation window.

### 3.4 ChatGPT Subscription OAuth vs. Metered API Hard Limits
Milestone 5B-Review confirmed that the active Codex environment uses ChatGPT Subscription OAuth (`auth_mode = "chatgpt"`, with `OPENAI_API_KEY: null`):
- Under ChatGPT Plus/Team subscription licensing, there is no direct per-token monetary billing (\$0.00 metered invoice spend).
- However, subscription accounts are governed by rolling message/token rate limits (e.g., dynamic request windows).
- There is **no account-level dollar cap or provider-enforced hard ceiling** that cuts off token consumption at exactly 350,000 tokens. The experiment directly expends the user's finite interactive subscription capacity.

### 3.5 Telemetry Integrity as an Absolute Experimental Precondition
In accordance with `orchestrator.py` (`lines 450–475`):
- Total model tokens and net savings must be derived strictly from provider-reported telemetry in `events.jsonl`.
- If telemetry is missing, unparseable, negative, or demonstrates zero-token reporting anomalies, the harness marks `telemetry_valid = False`.
- Any invalid or missing telemetry strictly **invalidates and suppresses** comparative savings claims, even if functional code acceptance is achieved.

---

## 4. Final Token Exposure Summary

| Dimension | Assessment | Operational Exposure |
|---|---|---|
| Nominal Budget (Arm A) | 175,000 tokens | Expected planning envelope |
| Nominal Budget (Arm B) | 175,000 tokens | Expected planning envelope |
| Combined Nominal Budget | 350,000 tokens | Combined expected planning envelope |
| In-Flight Turn Burst Risk | Present | Terminal turn may exceed reserve by 2,000–10,000 tokens |
| Provider-Enforced Hard Cap | Non-existent | Local client monitor only |
| Financial Liability | $0.00 | Uses ChatGPT subscription allowance |
| Account Liability | Rolling rate limit quota | Depletes subscription request window |
