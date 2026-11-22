# Pilot 01 Closure Report: Final Authorized Trial Accounting

**Document ID:** `amendments/draft-05/milestone-5d/pilot_01/pilot_01-closure-report.md`  
**Milestone:** Engram Amendment 05 — Milestone 5D Live Pilot  
**Experiment Pair:** `pilot_01`  
**Task ID:** `task_commit_cache_invalidation`  
**Trial Date:** 2026-10-30  
**Lead Systems Engineer:** Antigravity  
**Status:** FORMALLY CLOSED & INDEPENDENTLY AUDITED  

---

## 1. Executive Closure Statement

The single authorized $N=1$ matched-pair experiment (`pilot_01`) authorized under Engram Amendment 05 has completed execution under strict compliance with all experimental protocols, isolation constraints, token safeguards, and repository preservation rules.

Execution results are formally recorded as follows:
- **Treatment Arm B:** Executed first. Safely halted by the pre-dispatch token headroom guard at Turn 4 with **117,135 total model tokens** consumed (status: `PRE_DISPATCH_HEADROOM_EXHAUSTED`).
- **Baseline Arm A:** In accordance with the preregistered Amendment 04 stop-on-failure rule, Baseline execution was **SKIPPED** (status: `SKIPPED_STOP_ON_FAILURE`). **0 tokens** consumed.
- **Combined Token Consumption:** **117,135 tokens** (within the 350,000 combined planned budget ceiling).
- **Comparative Token Savings Claim:** **STRICTLY SUPPRESSED AND INVALIDATED** (`MUTUAL_ACCEPTANCE_FAILURE`). Both arms achieved $C = 0.00$.

The experiment is formally concluded. In adherence to Section 13, all benchmarking activity is terminated. No retries, follow-on arms, or $N=2/N=3$ experiments will be initiated.

---

## 2. Comprehensive Metric Accounting

| Parameter | Planned Protocol | Actual Observed Value | Deviation / Notes |
| :--- | :--- | :--- | :--- |
| **Pair Identifier** | `pilot_01` | `pilot_01` | None |
| **Task Identifier** | `task_commit_cache_invalidation` | `task_commit_cache_invalidation` | None |
| **Model** | `gpt-5.6-luna` | `gpt-5.6-luna` | None |
| **Reasoning Effort** | `medium` | `medium` | None |
| **Nominal Per-Arm Ceiling** | 175,000 tokens | 175,000 tokens | Enforced |
| **Headroom Reserve** | 45,000 tokens | 45,000 tokens | Enforced |
| **Combined Planned Exposure**| 350,000 tokens | 117,135 tokens | **-232,865 tokens (-66.5%)** |
| **Treatment Model Tokens** | $\le 175,000$ | 117,135 | Halted at turn boundary |
| **Baseline Model Tokens** | $\le 175,000$ | 0 (Skipped) | Skipped under stop policy |
| **Treatment Correctness ($C$)** | 1.00 (Target) | 0.00 | Failed (premature halt) |
| **Baseline Correctness ($C$)** | 1.00 (Target) | 0.00 | Failed (skipped) |
| **Treatment MCP Calls** | Active | 0 | Model used powershell |
| **Treatment Duration** | $\le 600\text{ s}$ | 71.12 seconds | Within timeout |
| **Baseline Duration** | $\le 600\text{ s}$ | 0.00 seconds | Skipped |
| **Git Working Tree** | Clean / Preserved | Pinned at `c70845...` | Preserved intact |

---

## 3. Key Findings and Scientific Insights

### 3.1 Harness and Safety Infrastructure Validation
1. **Pre-Dispatch Guard Succeeded:** The host-side pre-dispatch token headroom guard functioned flawlessly. By evaluating cumulative usage against the estimated next turn request cost at the pre-dispatch boundary, it prevented a runaway turn that would have exceeded the 175,000 token ceiling.
2. **Stop-on-Failure Rule Succeeded:** The harness immediately skipped the baseline arm upon treatment failure, conserving 175,000 tokens and preventing pointless financial and computational expenditure.
3. **Workspace Isolation Succeeded:** All file operations occurred within an isolated temporary sandbox (`AppData\Local\Temp\engram_pilot\runs\...`). The golden repository dependency fingerprint and git state remained completely untainted.
4. **Telemetry Fidelity Succeeded:** The provider's native `SESSION_ROLLOUT` event stream was captured, parsed, and reconciled with zero double-counting of cached input or reasoning tokens.

### 3.2 Agent Behavior Bottleneck
The critical failure mode observed in `treatment_B` was **agent tool neglect**:
- The model was provided an active Engram MCP server configured with `engram_get_context`, `engram_inspect_symbol`, `engram_preflight`, and `engram_postflight`.
- However, rather than utilizing these structured retrieval tools, the model defaulted to raw PowerShell execution (`Get-ChildItem`, `rg`, `Get-Content`).
- These shell commands returned hundreds of uncompressed lines of Rust source code into the prompt history, inflating context from ~1,000 tokens to 41,011 tokens across 4 turns.
- **Scientific Takeaway:** Providing MCP tools alone is insufficient if the coding agent lacks explicit prompt guidance or reinforcement to prioritize MCP queries over raw terminal file dumps.

---

## 4. Deliverables Index

All deliverables for Milestone 5D Pilot 01 are compiled in `amendments/draft-05/milestone-5d/pilot_01/`:

1. [`execution-summary.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5d/pilot_01/execution-summary.md)
2. [`treatment-arm-results.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5d/pilot_01/treatment-arm-results.md)
3. [`baseline-arm-results.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5d/pilot_01/baseline-arm-results.md)
4. [`token-usage-comparison.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5d/pilot_01/token-usage-comparison.md)
5. [`functional-acceptance-results.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5d/pilot_01/functional-acceptance-results.md)
6. [`experimental-limitations.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5d/pilot_01/experimental-limitations.md)
7. [`pilot_01-closure-report.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5d/pilot_01/pilot_01-closure-report.md)
8. [`SHA256SUMS`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-5d/pilot_01/SHA256SUMS)

Raw event logs, summaries, and telemetry records are preserved in their original format at:
`benchmarks/matched-pair-harness/reports/runs/pilot_01/`

---

## 5. Final Boundary Affirmation

In strict accordance with the repository owner's authorization:
- Exactly ONE live matched-pair invocation was executed.
- Zero retries were attempted.
- Zero follow-on experiments were initiated.
- All golden repository binaries, configurations, and branches remain strictly preserved.
- The engineering team now waits for owner review and guidance.
