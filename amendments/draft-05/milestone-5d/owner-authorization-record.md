# Owner Authorization Record: Engram N=1 Matched-Pair Pilot

**Milestone:** Engram Amendment 05 — Milestone 5D  
**Document ID:** `amendments/draft-05/milestone-5d/owner-authorization-record.md`  
**Evaluation Date:** 2026-10-30  
**Target Trial:** Matched-Pair Pilot ($N=1$, `pilot_01`)  
**Status:** PREPARED FOR OWNER AUTHORIZATION  

---

## 1. Experiment Specification Summary

| Dimension | Specification Value | Verification Reference |
|---|---|---|
| **Task Identifier** | `task_commit_cache_invalidation` | [`task_commit_cache_invalidation.json`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/config/tasks/task_commit_cache_invalidation.json) |
| **Repository Path** | `C:\Users\Amal P S\OneDrive\Projects\Engram` | Golden repository root |
| **Active Git Branch** | `feature/async-precommit-guard` | Pinned at commit `c70845b4d5913457f1b176fb0370b80dd792711f` |
| **Pair Identifier** | `pilot_01` | Trial identifier for matched pair $N=1$ |
| **Model Identifier** | `gpt-5.6-luna` | Configured across task, runner, and orchestrator |
| **Reasoning Effort** | `medium` | Standardized across Control A and Treatment B |
| **Candidate Binary** | `target/release/engram.exe` | Compiled from Candidate Phase 1 source code |
| **Candidate SHA-256** | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | Cryptographically validated prior to execution |
| **Output Directory** | `benchmarks/matched-pair-harness/reports/runs/pilot_01` | Tested; confirmed pristine and collision-free |

---

## 2. Resource, Quota & Budget Terms

| Parameter | Value | Operational Reality |
|---|---|---|
| **Nominal Per-Arm Budget** | 175,000 tokens | Client-side planning threshold, not provider hard limit |
| **Pre-Dispatch Headroom Reserve** | 45,000 tokens | Safety buffer for in-flight turn completion |
| **Pre-Dispatch Stop Threshold** | 130,000 tokens | Remaining turns aborted if cumulative tokens $\ge 130,000$ |
| **Combined Nominal Exposure** | 350,000 tokens | Total planning budget across Control A + Treatment B |
| **Financial Spend** | $0.00 | Consumes ChatGPT Subscription OAuth quota |
| **Potential Overrun** | ~2,000–10,000 tokens | In-flight terminal turn burst may exceed reserve |

---

## 3. Scientific Invariants Enforced by Harness

1. **Mutual Acceptance Requirement:** Savings claims are strictly suppressed if either arm achieves $C < 1.00$.
2. **Telemetry Validity Requirement:** Missing, unparseable, or negative token telemetry strictly invalidates savings claims.
3. **Double-Counting Prevention:** Cached tokens and reasoning tokens are tracked as informative breakdowns and never double-counted into total model tokens.
4. **Physical Isolation:** Each arm executes inside a detached Git worktree under `%TEMP%\engram_pilot\runs` with zero shared build artifacts.
5. **Fail-Closed Binary Validation:** Any byte divergence from SHA-256 `F56347EF...` immediately halts execution prior to sandbox setup.

---

## 4. Verified PowerShell Launch Command

```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --mode pair `
  --task task_commit_cache_invalidation `
  --pair pilot_01 `
  --expected-treatment-sha256 "F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE" `
  --model "gpt-5.6-luna" `
  --reasoning "medium" `
  --max-tokens-ceiling 175000 `
  --headroom-reserve 45000 `
  --dependency-strategy "none" `
  --output-dir "benchmarks/matched-pair-harness/reports/runs/pilot_01"
```

---

## 5. Owner Authorization Sign-Off Checklist

To authorize live execution of the first controlled Engram token-efficiency experiment, please confirm:

- [ ] **Subscription Quota:** My ChatGPT subscription is active with sufficient message capacity in the current rolling window.
- [ ] **Model Access:** My subscription account has verified access to `gpt-5.6-luna`.
- [ ] **Budget Exposure:** I accept the nominal planning budget of 175,000 tokens per arm (350,000 total) and acknowledge that in-flight turn bursts may cause nominal budget overruns.
- [ ] **Execution Command:** I authorize execution of the verified launch command for pair `pilot_01`.
