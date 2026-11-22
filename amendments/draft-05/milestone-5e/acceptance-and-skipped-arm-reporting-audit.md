# Engram Milestone 5E — Deliverable 5
# Acceptance and Skipped-Arm Reporting Classification Forensic Audit

## 1. Executive Summary

This audit evaluates the reporting, classification, and statistical claims generated in `benchmarks/matched-pair-harness/reports/runs/pilot_01/` following the halting of `treatment_B`.

While the harness correctly invalidated comparative token-savings claims (`savings_claim_eligible: false`), the audit identified **severe semantic and classification defects** in how the skipped control arm (`control_A`) was reported:
1. `control_A` was recorded with `score: 0.0` and `control_A_accepted: false`, falsely indicating that it ran and failed functional tests.
2. The orchestrator declared a verdict of `MUTUAL_ACCEPTANCE_FAILURE` and asserted `pair_acceptance_parity: true`, distorting the experimental reality that `control_A` was never executed.
3. The report conflated **avoided execution tokens** with experimental cost control.

---

## 2. Identified Reporting and Classification Defects

### Defect 1: Falsified "Mutual Failure" and "Parity" Verdict
In `matched_pair_summary.json`:
```json
"acceptance": {
  "control_A_accepted": false,
  "treatment_B_accepted": false,
  "pair_acceptance_parity": true
},
"comparative_analysis": {
  "savings_claim_eligible": false,
  "savings_claim_status": "INVALIDATED_BY_ACCEPTANCE_FAILURE",
  "verdict": "MUTUAL_ACCEPTANCE_FAILURE",
  "explanation": "Both arms failed functional correctness requirements (control_A score: 0.00, treatment_B score: 0.00). Comparative token savings and efficiency claims are STRICTLY INVALIDATED."
}
```

#### Forensic Critique:
- **`control_A` never executed a single line of code, never spawned an agent, and never consumed a single token.**
- Assigning `control_A score: 0.00` and `control_A_accepted: false` creates an entirely spurious equivalence between an arm that ran, consumed 117k tokens, and exhausted headroom (`treatment_B`) and an arm that was intentionally withheld by the harness's fail-fast policy (`control_A`).
- Asserting `pair_acceptance_parity: true` is methodologically invalid. Parity can only exist if both arms were evaluated under the acceptance checklist. A skipped arm cannot be compared for parity.

### Defect 2: Score Zero vs Null / Not Evaluated
In `control_A/run_telemetry.json`:
```json
"correctness": {
  "score": 0.0,
  "quality_adjusted_cost": 0.0,
  "passed_all_requirements": false,
  "gates": {
    "L0_typecheck": "SKIPPED",
    "L1_lint_prettier": "SKIPPED",
    "L2_unit_tests": "SKIPPED",
    "L3_production_build": "SKIPPED"
  },
  "requirements": {
    "REQ-1": "NOT_EVALUATED",
    "REQ-2": "NOT_EVALUATED",
    "REQ-3": "NOT_EVALUATED",
    "REQ-4": "NOT_EVALUATED"
  }
}
```

#### Forensic Critique:
- While gates and individual requirements were accurately marked `SKIPPED` and `NOT_EVALUATED`, the aggregate score was forced to float `0.0`, and `passed_all_requirements` was set to `false`.
- In statistical reporting, a skipped observation must have score `null` (or `None`). Setting `score: 0.0` treats unattempted tasks identically to catastrophic code corruption.

### Defect 3: Conflation of Avoided Tokens with Savings
In experimental summaries and reports, halting early is an essential safety and cost-containment boundary. However, narratives that present avoiding control arm execution as "saving 175,000 baseline tokens" are logically invalid.
- Savings only occur when a task is completed successfully with fewer tokens than an unassisted run.
- When an experiment halts due to tool or agent failure, the baseline tokens were not "saved"; the experiment was simply **aborted early**.

---

## 3. Comparative Classification Truth Table

The following truth table defines the valid classification taxonomy that must be enforced across all future harness summaries:

| Arm A Status | Arm B Status | Arm A Accepted? | Arm B Accepted? | Parity? | Valid Verdict Identifier | Permitted Explanation |
| :---: | :---: | :---: | :---: | :---: | :--- | :--- |
| `SUCCESS` | `SUCCESS` | True | True | True | `VALID_COMPARISON_BOTH_ACCEPTED` | Both arms completed and met functional acceptance. Savings eligible. |
| `SUCCESS` | `SUCCESS` | False | False | True | `MUTUAL_EVALUATION_FAILURE` | Both arms executed and evaluated, but neither met functional checklist. |
| `SUCCESS` | `SUCCESS` | True | False | False | `ASYMMETRIC_FAILURE` | Arm A passed; Arm B failed functional evaluation. |
| `HALTED` | `SKIPPED` | N/A (`SKIPPED`) | False | **False** | **`ARM_HALTED_PAIR_INCOMPLETE`** | Treatment B halted early; Control A skipped by fail-fast. No comparison possible. |
| `SKIPPED` | `HALTED` | False | N/A (`SKIPPED`) | **False** | **`ARM_HALTED_PAIR_INCOMPLETE`** | Control A halted early; Treatment B skipped. |

---

## 4. Required Schema and Classifier Remediation

1. **Schema Updates (`run_telemetry.schema.json` and `triad_summary.schema.json`)**:
   - Allow `correctness.score` to accept `null` for unexecuted runs.
   - Add enum values for `acceptance.status`: `["ACCEPTED", "REJECTED", "SKIPPED", "NOT_EVALUATED"]`.
   - Update `pair_acceptance_parity` to be `null` if any arm in the pair was skipped.
2. **Orchestrator Classifier Logic (`classifier.py`)**:
   - Replace indiscriminate assignment of `verdict = "MUTUAL_ACCEPTANCE_FAILURE"` with state-aware verdict routing.
   - If one arm is `SKIPPED_STOP_ON_FAILURE`, the pair verdict must be strictly set to `RUN_ABORTED_INCOMPLETE_PAIR`.
3. **Report Generation (`orchestrator.py`)**:
   - Explicitly display `SKIPPED (0 tokens consumed, unexecuted)` in all Markdown summary tables.
   - Forbid any calculation or display of token savings percentages ($\Delta\%$) when an arm is skipped.
