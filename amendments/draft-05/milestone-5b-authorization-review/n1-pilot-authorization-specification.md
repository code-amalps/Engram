# Milestone 5B Authorization Review: N=1 Pilot Authorization Specification

## 1. Governance Preamble

This document specifies the exact, immutable technical parameters for a single exploratory matched-pair trial ($N=1$) under Engram Amendment 05 Milestone 5B.

> [!WARNING]
> **THIS SPECIFICATION DOES NOT AUTHORIZE EXECUTION.**
> Preparing this specification establishes technical clarity. Actual execution requires human owner review, completion of the billing checklist, and an explicit owner execution prompt.

---

## 2. Immutable Trial Parameters

| Parameter | Specification / Value | Enforcement Mechanism |
|---|---|---|
| **Task Identifier** | `task_commit_cache_invalidation` | Validated in `config/tasks/task_commit_cache_invalidation.json` |
| **Repository Type** | `cargo` (Rust) | Checked via `isolation.compute_cargo_manifest` |
| **Target Baseline Commit** | `c70845b4d5913457f1b176fb0370b80dd792711f` | Checked via `git rev-parse HEAD` on detached worktree |
| **Candidate Binary Path** | `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe` | Explicit CLI override flag `--engram-bin` |
| **Candidate Binary Hash** | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | Verified via `--expected-treatment-sha256` |
| **Agent Executable** | `C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe` | Standard Codex binary |
| **Model Identifier** | `gpt-5.6-luna` | Enforced via `--model gpt-5.6-luna` |
| **Reasoning Effort** | `medium` | Enforced via `--reasoning medium` |
| **Trial Structure** | 1 Matched Pair ($N=1$ block) | Latin-square order randomized by explicit seed |
| **Arm Presentation Order** | Seed-determined (`control_A` then `treatment_B` or vice-versa) | Logged in `matched_pair_summary.json` |

---

## 3. Budget, Ceilings & Exposure Boundaries

> [!IMPORTANT]
> **THE 175,000-TOKEN CEILING IS A NOMINAL GOVERNANCE THRESHOLD, NOT A ZERO-LEAKAGE HARD CAP.**
> Because the local monitor operates via 50ms external polling, overruns on the single terminal turn can occur.

- **Nominal Per-Arm Token Ceiling ($C_{\text{nom}}$):** `175,000 tokens`
- **Headroom Planning Reserve ($R$):** `45,000 tokens`
- **Pre-Dispatch Stop Threshold ($T_{\text{stop}} = C_{\text{nom}} - R$):** `130,000 tokens`
- **Combined Planned Exposure (2 Arms):** `350,000 tokens`
- **Known Historical Jump Tolerance:** Up to `45,000 tokens` per turn (historical maximum was `37,651 tokens`).
- **Accepted Potential Overrun:** If an agent initiates an anomalous single turn consuming $50,000$ to $60,000$ tokens starting from $129,999$ tokens, final post-turn usage will reach **~180,000 to 190,000 tokens** before the ceiling breach terminates the process. The owner must accept this potential overrun.
- **Verified Financial Limit:** Local tooling operates via ChatGPT Subscription OAuth (`auth_mode: "chatgpt"`). No dollar hard cap is configured in local files. Financial quota availability requires **`OWNER VERIFICATION REQUIRED`**.
- **Wall-Clock Execution Timeout:** `600 seconds` (10 minutes) per arm.

---

## 4. Execution Stop Conditions

The harness runner (`runner.py`) will terminate the agent process tree immediately upon encountering any of the following conditions:

1. **Pre-Dispatch Headroom Exhausted:** Cumulative usage at turn boundary reaches $> 130,000$ tokens (`stop_reason = "PRE_DISPATCH_HEADROOM_EXHAUSTED"`).
2. **Ceiling Breached:** Post-turn cumulative usage exceeds $175,000$ tokens (`stop_reason = "TOKEN_CEILING_BREACHED"`).
3. **Execution Timeout:** Elapsed time exceeds $600$ seconds (`stop_reason = "TIMEOUT"`).
4. **Invalid Telemetry:** Malformed JSON, missing usage fields, or negative token values (`stop_reason = "TELEMETRY_INVALID"`).
5. **Sandbox Contamination:** Unauthorized modification of `Cargo.toml`, `Cargo.lock`, or untracked file generation (`dependency_mutated = True`).

---

## 5. Functional Acceptance & Evaluation Gates

Both arms must satisfy identical evaluation gates. A lower-token run that fails acceptance is treated as a failed trial:

1. **L0 Typecheck:** `cargo check --offline` must pass with exit code 0.
2. **L2 Unit Tests:** `cargo test --offline --lib compiler::tests` must pass with exit code 0.
3. **L3 Production Build:** `cargo build --offline` must pass with exit code 0.
4. **Requirement Checklist Scoring ($C \in [0.0, 1.0]$):**
   - **REQ-1 (Weight 0.25):** SQLite `.engram/index.db` records active Git commit HEAD.
   - **REQ-2 (Weight 0.25):** `ensure_fresh()` triggers re-index on commit drift.
   - **REQ-3 (Weight 0.25):** `engram clean --cache` purges index while preserving memories.
   - **REQ-4 (Weight 0.25):** Blast-radius adherence; zero changes to `Cargo.toml` or unrelated modules.
5. **The Mutual Acceptance Rule:** Comparative token efficiency claims are strictly invalidated unless **both** arms achieve $\text{Score} = 1.0$.

---

## 6. Exact Authorized Invocation Command

If and when authorized by the owner, the exact invocation command is:

```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --mode pair `
  --task task_commit_cache_invalidation `
  --pair pilot_01 `
  --engram-bin "benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe" `
  --expected-treatment-sha256 "F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE" `
  --model gpt-5.6-luna `
  --reasoning medium `
  --max-tokens-ceiling 175000 `
  --headroom-reserve 45000 `
  --dependency-strategy none `
  --output-dir "benchmarks/matched-pair-harness/reports/runs/pilot_01"
```
*(Do not execute without explicit authorization prompt)*.
