# Engram Phase 5.9.0.1: Live Benchmark Explicit Authorization Template

**Document Status:** TEMPLATE SPECIFICATION (NOT AUTHORIZED TO EXECUTE)  
**Governance Requirement:** A human stakeholder must explicitly sign/approve this document before any live provider API calls may be initiated.  

---

## 1. Explicit Scope of Authorization

This authorization covers exclusively the **two-task, five-arm pentad pilot benchmark**:
1. **Task A:** `task_a_typo` (`projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html`)
2. **Task B:** `task_b_remember_me` (`projects/angular-ngrx-material-starter/src/app/...`)

### 1.1 Evaluated Benchmark Arms
- **Arm A (`arm_a_native`):** Native Codex CLI baseline (no Engram hook).
- **Arm B (`arm_b_cand3`):** Candidate 3 binary (`benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe`).
- **Arm C (`arm_c_cand42_always_on`):** Candidate 4.2 binary (`benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe --always-on`).
- **Arm D (`arm_d_cand42_adaptive`):** Candidate 4.2 binary (`benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe --adaptive`).
- **Arm E (`arm_e_cand5_optimized`):** Candidate 5 binary (`benchmarks/matched-pair-harness/bin/candidate-phase5/engram.exe`).

---

## 2. Invariants & Cryptographic Baselines

| Component | Path / Identification | Expected SHA-256 Hash |
|---|---|---|
| **Git Baseline HEAD** | `feature/phase2-stage5-remediation` | `7106a2f369fe79faf5f95449378a900ab8e8eddf` |
| **Golden Repository** | `angular-ngrx-material-starter` | Commit: `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2` |
| **Candidate 3 Binary**| `candidate-phase3/engram.exe` | `97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777` |
| **Candidate 4.2 Binary**| `candidate-phase4.2/engram.exe` | `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2` |
| **Candidate 5 Binary**| `candidate-phase5/engram.exe` | `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` |
| **Task A Fixture** | `config/tasks/task_a_typo.json` | `357D9B4B5777A41A8C5691475DCA8DDAC00134C480C6324E26B60FEA39F95F02` |
| **Task B Fixture** | `config/tasks/task_b_remember_me.json` | `CD55DB0DFDB5758275B262B8B330858E9673533AA6FAA182897EDAE83D69A2B7` |

---

## 3. Approved Budgets, Ceilings & Execution Boundaries

| Operational Parameter | Task A Allocation | Task B Allocation | Combined Limit |
|---|---:|---:|---:|
| **Invocation Global Ceiling** | 1,000,000 tokens | 1,500,000 tokens | **2,500,000 tokens (Hard Cap)** |
| **Per-Arm Ceiling** | 250,000 tokens | 500,000 tokens | Enforced per arm |
| **Headroom Reserve** | 45,000 tokens | 45,000 tokens | 90,000 tokens protected |
| **Max Turn Budget** | 15 turns | 25 turns | Enforced |
| **Deterministic Seed** | `59001` | `59002` | Fixed PRNG order |
| **Execution Policy** | `diagnostic_continue` | `diagnostic_continue` | Complete 5-arm logging |
| **Estimated Commercial Cost** | $\approx \$2.50 - \$4.00$ | $\approx \$5.00 - \$16.00$ | **$\le \$20.26$ max exposure** |

---

## 4. Governance & Telemetry Commitments

1. **Telemetry Integrity Gate:** If any arm reports invalid, negative, or unparseable telemetry (`telemetry_valid == False`), that arm is marked `TELEMETRY_INVALID` and cannot be claimed as an optimization win.
2. **Partial Result Retention:** Under `diagnostic_continue`, if an individual arm halts or fails, its workspace diff, execution logs, and telemetry are preserved in `results/`, and the orchestrator advances to the next scheduled arm.
3. **Fail-Closed Halting:** If available headroom drops below 45,000 tokens before dispatching an arm, all remaining arms in that block are skipped with `SKIPPED_GLOBAL_BUDGET`.
4. **Dynamic Reconciled Enforcement:** The operator agrees to execute Invocation 2 using the dynamic ceiling formula:
   $$\text{Task B Ceiling} = \min(1,500,000, 2,500,000 - \text{Task A Tokens Spent})$$

---

## 5. Stakeholder Authorization Sign-Off

```
[ ] I explicitly authorize the live execution of Task A and Task B pentad benchmark
    under the exact budgets, candidate identities, and seeds specified in this template.

Authorized By: ____________________________________
Date & Timestamp: _________________________________
Authorization Token / Reference: ___________________
```
