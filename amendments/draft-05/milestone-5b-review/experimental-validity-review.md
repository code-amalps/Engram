# Milestone 5B-Review: Experimental Validity Review

## 1. Executive Summary

This review audits the experimental design and scientific validity of the proposed matched-pair token-efficiency experiment preregistered in Milestone 5A.

The review confirms that:
1. Baseline and treatment definitions maintain strict experimental symmetry.
2. The primary metric is total model token consumption per accepted solution, not merely context package size.
3. Functional acceptance gates are identical across both arms.
4. The mutual acceptance rule prevents invalid efficiency claims.
5. An $N=3$ block trial is explicitly classified as **exploratory**, and cannot guarantee statistical significance.

---

## 2. Experimental Arm Definitions & Symmetry Controls

To ensure that observed token differences reflect the true causal impact of Engram's context compiler rather than confounding tool advantages or prompt asymmetries, the arms are defined as follows:

| Dimension | Control Arm (Baseline Exploration) | Treatment Arm (Engram-Assisted) | Confounding Defense |
|---|---|---|---|
| **Task Prompt** | Standard task description & requirements | Identical task description & requirements | Prompts have identical problem statements |
| **Model & Reasoning** | Exact same model (`gpt-5.6-luna`), same reasoning effort | Exact same model, same reasoning effort | Zero variance in model capability or reasoning depth |
| **Tool Availability** | Standard coding-agent tools (file read, file edit, shell execution) | Standard coding-agent tools + candidate Engram context package | No unregistered secondary MCP tools enabled |
| **Engram Context** | **NONE** (No context package, no Turn-0 injection, no MCP) | **Engram Context Package** generated offline by candidate binary | Isolates context compiler impact |
| **Starting State** | Pristine git worktree at baseline commit | Pristine git worktree at baseline commit | Cold-start parity |
| **Sandbox Isolation** | Independent temp directory in `engram_pilot/runs` | Independent temp directory in `engram_pilot/runs` | Zero cross-contamination |

---

## 3. Primary Outcome Specification

### Central Hypothesis (H1)
**Using Engram-compiled engineering context reduces total AI-agent token consumption per successfully completed engineering task, compared with an otherwise identical coding-agent workflow without Engram.**

### Primary Dependent Metric
$$\mathbf{Tokens}_{\text{total}} = \text{Total Model Tokens (Input + Output) per Accepted Solution}$$

- The experiment **DOES NOT** measure merely the size of the initial prompt or the context package.
- It measures the **entire dialogue lifecycle**, capturing prompt tokens, exploration queries, file reads, tool execution responses, chain-of-thought reasoning tokens, and final patch generation.

---

## 4. Functional Acceptance Equivalence & The Mutual Acceptance Rule

A fundamental vulnerability in AI efficiency benchmarks is "cheap failure": an agent that terminates early or generates superficial edits consumes very few tokens but fails to solve the task.

### The Mutual Acceptance Rule (Strictly Enforced)
A comparative token savings claim is valid **if and only if both the Control arm and the Treatment arm achieve 100% functional acceptance**:
$$\text{Claim Valid} \iff (\text{Score}_{\text{control}} = 1.0 \land \text{Score}_{\text{treatment}} = 1.0)$$

If either arm fails:
- The comparative savings rate is formally marked **`SUPPRESSED [MUTUAL_ACCEPTANCE_FAILURE]`**.
- Quality-Adjusted Cost is computed as:
  $$\text{Effective Cost} = \frac{\text{Total Tokens}}{\text{Score}}$$
  If $\text{Score} = 0.0$, Effective Cost is mathematically infinite (or flagged failed).
- A lower-token run that fails tests **CANNOT** be claimed as an efficiency improvement.

### Evaluation Gates Applied Identically to Both Arms
1. **L0 Typecheck:** `cargo check --offline` (Must exit code 0).
2. **L2 Unit Tests:** `cargo test --offline --lib compiler::tests` (Must exit code 0).
3. **L3 Production Build:** `cargo build --offline` (Must exit code 0).
4. **Independent Checklist Requirements (`REQ-1` to `REQ-4`):**
   - SQLite commit tracking (0.25).
   - AST cache invalidation on commit drift (0.25).
   - `engram clean --cache` subcommand (0.25).
   - Boundary adherence (zero edits to forbidden files or manifests) (0.25).

---

## 5. Statistical Power and Sample Size Qualifications

The Milestone 5A protocol proposes an initial trial of **$N = 3$ matched pairs**.

### Explicit Scientific Qualification:
1. **$N = 3$ is strictly exploratory.**
2. A sample of three pairs cannot establish formal statistical significance ($p < 0.05$) under non-parametric tests (such as the Wilcoxon signed-rank test), because the minimum sample size required for Wilcoxon signed-rank significance at $\alpha = 0.05$ (two-tailed) is $N = 6$.
3. The $N = 3$ exploratory block serves exclusively to:
   - Prove end-to-end execution stability.
   - Measure empirical variance ($\sigma^2$) of token deltas.
   - Establish whether the effect size ($\delta$) is sufficiently large to justify a larger confirmatory trial ($N \ge 10$).
4. The harness and documentation must explicitly disclaim statistical certainty for any $N=3$ trial.

---

## 6. Audit Verdict

The experimental protocol is scientifically rigorous. Confounding variables are well controlled, the primary outcome measures end-to-end task completion tokens, cheap failure is prevented by the mutual acceptance rule, and sample size limitations are transparently qualified.
