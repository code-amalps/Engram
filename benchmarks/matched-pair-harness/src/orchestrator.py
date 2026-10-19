"""
Matched-Pair Benchmark Orchestrator.
Coordinates isolated cold-start execution of paired benchmark arms (Control A vs Treatment B),
enforces pre-registered coin-flip execution ordering, verifies golden dependency tree integrity
before and after every run, runs automated verification gates, parses exact telemetry,
calculates independent correctness, and validates against JSON schemas.
"""
from typing import Dict, Any, List, Optional
from pathlib import Path
import argparse
import random
import json
import sys
import os

from isolation import (
    verify_frozen_binary,
    create_sandbox,
    teardown_sandbox,
    compute_dependency_fingerprint,
    verify_dependency_integrity,
    ProcessTracker,
    SafetyViolationError,
    FROZEN_ENGRAM_SHA256,
)
from telemetry import TelemetryParser, TelemetryMetrics
from evaluator import (
    Evaluator,
    GitImpact,
    CorrectnessResult,
    FixturePreflightError,
    validate_fixture_preflight,
)
from runner import CodexRunner, RunExecutionResult
from schema_validator import validate_instance, SchemaValidationError


DEFAULT_GOLDEN_REPO = Path(r"C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter")
DEFAULT_CODEX_BIN = Path(r"C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe")
DEFAULT_ENGRAM_BIN = Path(r"C:\Users\Amal P S\OneDrive\Projects\Engram\target\release\engram.exe")
HARNESS_ROOT = Path(__file__).resolve().parent.parent


class Orchestrator:
    def __init__(
        self,
        task_config_path: Path,
        pair_id: str,
        golden_repo: Path = DEFAULT_GOLDEN_REPO,
        codex_bin: Path = DEFAULT_CODEX_BIN,
        engram_bin: Path = DEFAULT_ENGRAM_BIN,
        model: str = "gpt-5.6-luna",
        reasoning_effort: str = "medium",
        output_dir: Optional[Path] = None,
        is_dry_run: bool = False,
        dependency_strategy: str = "copy",
        preserve_sandboxes: bool = False,
        seed: Optional[int] = None,
        expected_treatment_sha256: Optional[str] = None,
    ):
        self.task_config_path = Path(task_config_path)
        with open(self.task_config_path, "r", encoding="utf-8") as f:
            self.task_config = json.load(f)

        self.pair_id = pair_id
        self.task_id = self.task_config.get("task_id", "unknown_task")
        self.golden_repo = Path(golden_repo)
        self.codex_bin = Path(codex_bin)
        self.engram_bin = Path(engram_bin)
        self.expected_treatment_sha256 = expected_treatment_sha256 or FROZEN_ENGRAM_SHA256
        self.model = model
        self.reasoning_effort = reasoning_effort
        self.output_dir = output_dir or (HARNESS_ROOT / "reports" / "runs" / f"{self.task_id}_{self.pair_id}")
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.is_dry_run = is_dry_run
        self.dependency_strategy = dependency_strategy
        self.preserve_sandboxes = preserve_sandboxes
        self.seed = seed if seed is not None else random.randint(1000, 9999)

        # Load schema for validation
        self.telemetry_schema_path = HARNESS_ROOT / "schemas" / "run_telemetry.schema.json"
        with open(self.telemetry_schema_path, "r", encoding="utf-8") as f:
            self.telemetry_schema = json.load(f)

        self.summary_schema_path = HARNESS_ROOT / "schemas" / "matched_pair_summary.schema.json"
        with open(self.summary_schema_path, "r", encoding="utf-8") as f:
            self.summary_schema = json.load(f)

        self.evaluator = Evaluator(self.task_config)
        self.runner = CodexRunner(
            codex_executable=self.codex_bin,
            engram_executable=self.engram_bin,
            model=self.model,
            reasoning_effort=self.reasoning_effort
        )
        self.process_tracker = ProcessTracker()

        # Capture baseline golden dependency fingerprint
        self.golden_baseline_fingerprint = compute_dependency_fingerprint(self.golden_repo)

        # Validate task fixture preflight against baseline golden repository
        self.evaluator.validate_preflight(self.golden_repo)

    def run_pair(self) -> Dict[str, Any]:
        """Executes a matched pair with randomized arm order and pre/post dependency verification."""
        rng = random.Random(self.seed)
        coin = rng.choice([0, 1])

        if coin == 0:
            order = [("control_A", 1), ("treatment_B", 2)]
        else:
            order = [("treatment_B", 1), ("control_A", 2)]

        pair_summary = {
            "schema_version": "2.0.0",
            "pair_id": self.pair_id,
            "task_id": self.task_id,
            "seed": self.seed,
            "order": [arm for arm, _ in order],
            "model": self.model,
            "reasoning_effort": self.reasoning_effort,
            "golden_dependency_fingerprint_verified": True,
            "acceptance": {
                "control_A_accepted": False,
                "treatment_B_accepted": False,
                "pair_acceptance_parity": False
            },
            "comparative_analysis": {},
            "runs": {}
        }

        for arm, exec_order in order:
            run_data = self._execute_single_arm(arm=arm, exec_order=exec_order)
            pair_summary["runs"][arm] = run_data

        # Update acceptance summary
        ctrl = pair_summary["runs"].get("control_A", {})
        treat = pair_summary["runs"].get("treatment_B", {})

        ctrl_corr = ctrl.get("correctness", {})
        treat_corr = treat.get("correctness", {})

        ctrl_acc = bool(ctrl_corr.get("passed_all_requirements", False) and (ctrl_corr.get("score", 0.0) == 1.0))
        treat_acc = bool(treat_corr.get("passed_all_requirements", False) and (treat_corr.get("score", 0.0) == 1.0))

        pair_summary["acceptance"]["control_A_accepted"] = ctrl_acc
        pair_summary["acceptance"]["treatment_B_accepted"] = treat_acc
        pair_summary["acceptance"]["pair_acceptance_parity"] = (ctrl_acc == treat_acc)

        # Compute comparative analysis with strict invalidation guards
        comp_analysis = self.compute_comparative_analysis(ctrl, treat)
        pair_summary["comparative_analysis"] = comp_analysis

        # Validate against matched_pair_summary schema
        validate_instance(pair_summary, self.summary_schema)

        # Write comparative summary JSON
        summary_path = self.output_dir / "matched_pair_summary.json"
        with open(summary_path, "w", encoding="utf-8") as f:
            json.dump(pair_summary, f, indent=2)

        # Write comparative markdown report
        md_content = self.generate_markdown_report(pair_summary)
        report_path = self.output_dir / "matched_pair_report.md"
        with open(report_path, "w", encoding="utf-8") as f:
            f.write(md_content)

        # Print human-readable verdict banner
        self._print_pair_verdict(pair_summary)

        return pair_summary

    @staticmethod
    def validate_telemetry_integrity(tel: Any) -> Tuple[bool, str, Optional[Dict[str, Any]]]:
        """
        Validates telemetry completeness and mathematical integrity.
        Distinguishes missing telemetry from zero-token anomalies.
        Returns (is_valid, reason, parsed_metrics_dict).
        """
        if not isinstance(tel, dict) or not tel:
            return False, "TELEMETRY_MISSING: Telemetry dictionary is missing or empty.", None

        # Check for presence of required token fields (must not be None or absent)
        required_keys = ["total_model_tokens", "prompt_tokens_total", "completion_tokens"]
        missing = [k for k in required_keys if k not in tel or tel[k] is None]
        if missing:
            return False, f"TELEMETRY_MISSING: Required token fields missing or null: {missing}.", None

        for k in required_keys:
            if not isinstance(tel[k], int) or isinstance(tel[k], bool):
                return False, f"TELEMETRY_MALFORMED: Field '{k}' is not an integer (got {type(tel[k]).__name__}).", None

        total_tokens = tel["total_model_tokens"]
        prompt_tokens = tel["prompt_tokens_total"]
        completion_tokens = tel["completion_tokens"]

        # Distinct check: zero-token run is not valid model execution
        if total_tokens == 0:
            return False, "ZERO_TOKEN_ANOMALY: total_model_tokens is 0. Coding agent execution requires model tokens; 0 tokens indicates failed extraction or skipped model calls.", None

        if total_tokens < 0 or prompt_tokens <= 0 or completion_tokens < 0:
            return False, f"TELEMETRY_INVALID: Token counts must be positive (total={total_tokens}, prompt={prompt_tokens}, completion={completion_tokens}).", None

        # Mathematical invariant checks
        if prompt_tokens + completion_tokens != total_tokens:
            return False, f"TELEMETRY_INVARIANT_VIOLATION: prompt_tokens_total ({prompt_tokens}) + completion_tokens ({completion_tokens}) != total_model_tokens ({total_tokens}).", None

        cached_inferred_in = tel.get("prompt_tokens_cached_inferred")
        if cached_inferred_in is not None:
            if not isinstance(cached_inferred_in, bool):
                return False, f"TELEMETRY_MALFORMED: 'prompt_tokens_cached_inferred' must be a boolean (got {type(cached_inferred_in).__name__}: {cached_inferred_in!r}).", None

        uncached_inferred_in = tel.get("prompt_tokens_uncached_inferred")
        if uncached_inferred_in is not None:
            if not isinstance(uncached_inferred_in, bool):
                return False, f"TELEMETRY_MALFORMED: 'prompt_tokens_uncached_inferred' must be a boolean (got {type(uncached_inferred_in).__name__}: {uncached_inferred_in!r}).", None

        cached = tel.get("prompt_tokens_cached")
        if cached is not None:
            if isinstance(cached, bool) or not isinstance(cached, int) or cached < 0:
                return False, f"TELEMETRY_MALFORMED: 'prompt_tokens_cached' must be a non-negative integer (got {cached!r}).", None
            if cached > prompt_tokens:
                return False, f"TELEMETRY_INVARIANT_VIOLATION: cached tokens ({cached}) exceeds prompt_tokens_total ({prompt_tokens}).", None

        uncached = tel.get("prompt_tokens_uncached")
        if uncached is not None:
            if isinstance(uncached, bool) or not isinstance(uncached, int) or uncached < 0:
                return False, f"TELEMETRY_MALFORMED: 'prompt_tokens_uncached' must be a non-negative integer (got {uncached!r}).", None
            if uncached > prompt_tokens:
                return False, f"TELEMETRY_INVARIANT_VIOLATION: uncached tokens ({uncached}) exceeds prompt_tokens_total ({prompt_tokens}).", None

        if cached is not None and uncached is not None:
            if cached + uncached != prompt_tokens:
                return False, f"TELEMETRY_INVARIANT_VIOLATION: prompt cached ({cached}) + uncached ({uncached}) != prompt total ({prompt_tokens}).", None

        # Provenance and Normalization:
        # Rule 1: A count may be marked False only when the source establishes it was directly measured.
        # Rule 2: Whenever a count is inferred or missing during normalization, its flag MUST be True.
        # Rule 3: An existing True inference flag is preserved; never overwritten with False.
        if cached is None and uncached is None:
            # Both missing: apply fallback (0 cached, all uncached) but mark BOTH as inferred.
            # Never present cached = 0 as a measured fact.
            final_cached = 0
            final_cached_inferred = True
            final_uncached = prompt_tokens
            final_uncached_inferred = True
        elif cached is None:
            # Cached missing, inferred from prompt_tokens - uncached
            final_cached = prompt_tokens - uncached
            final_cached_inferred = True
            final_uncached = uncached
            final_uncached_inferred = uncached_inferred_in if uncached_inferred_in is not None else True
        elif uncached is None:
            # Uncached missing, inferred from prompt_tokens - cached
            final_cached = cached
            final_cached_inferred = cached_inferred_in if cached_inferred_in is not None else True
            final_uncached = prompt_tokens - cached
            final_uncached_inferred = True
        else:
            # Both present
            final_cached = cached
            final_cached_inferred = cached_inferred_in if cached_inferred_in is not None else True
            final_uncached = uncached
            final_uncached_inferred = uncached_inferred_in if uncached_inferred_in is not None else True

        peak_single = tel.get("peak_single_request_context")
        if peak_single is not None:
            if isinstance(peak_single, bool) or not isinstance(peak_single, int) or peak_single <= 0:
                return False, f"TELEMETRY_MALFORMED: 'peak_single_request_context' must be a positive integer or None (got {peak_single!r}).", None
            if peak_single > total_tokens:
                return False, f"TELEMETRY_INVARIANT_VIOLATION: peak_single_request_context ({peak_single}) cannot exceed total_model_tokens ({total_tokens}).", None

        parsed = {
            "total_model_tokens": total_tokens,
            "prompt_tokens_total": prompt_tokens,
            "completion_tokens": completion_tokens,
            "prompt_tokens_cached": final_cached,
            "prompt_tokens_cached_inferred": final_cached_inferred,
            "prompt_tokens_uncached": final_uncached,
            "prompt_tokens_uncached_inferred": final_uncached_inferred,
            "peak_single_request_context": peak_single
        }
        return True, "VALID", parsed

    @staticmethod
    def compute_comparative_analysis(
        ctrl_run: Dict[str, Any],
        treat_run: Dict[str, Any]
    ) -> Dict[str, Any]:
        """
        Computes rigorous matched-pair comparative metrics.
        CRITICAL SCIENTIFIC INVARIANTS:
        1. Telemetry Integrity Guard: If telemetry is missing, zero, or mathematically inconsistent
           on either arm, comparative savings are STRICTLY INELIGIBLE even if functional acceptance passed.
        2. Acceptance Guard: Token savings, context reductions, and efficiency percentages are
           STRICTLY INVALIDATED whenever either arm fails functional correctness (C < 1.0 or passed_all_requirements == False).
        """
        # 1. Validate Telemetry Integrity on both arms
        ctrl_tel_valid, ctrl_tel_reason, ctrl_tel_data = Orchestrator.validate_telemetry_integrity(ctrl_run.get("telemetry"))
        treat_tel_valid, treat_tel_reason, treat_tel_data = Orchestrator.validate_telemetry_integrity(treat_run.get("telemetry"))

        # 2. Extract and validate Correctness Acceptance
        ctrl_corr = ctrl_run.get("correctness", {})
        treat_corr = treat_run.get("correctness", {})

        ctrl_score = ctrl_corr.get("score", 0.0)
        treat_score = treat_corr.get("score", 0.0)

        ctrl_passed = bool(ctrl_corr.get("passed_all_requirements", False) and (ctrl_score == 1.0))
        treat_passed = bool(treat_corr.get("passed_all_requirements", False) and (treat_score == 1.0))

        both_accepted = ctrl_passed and treat_passed

        # 3. Guard against functional acceptance failure (Primary Gate)
        if not both_accepted:
            if not ctrl_passed and not treat_passed:
                verdict = "MUTUAL_ACCEPTANCE_FAILURE"
                explanation = (
                    f"Both arms failed functional correctness requirements "
                    f"(Control A score: {ctrl_score:.2f}, Treatment B score: {treat_score:.2f}). "
                    f"Comparative token savings and efficiency claims are STRICTLY INVALIDATED."
                )
            elif not treat_passed:
                verdict = "TREATMENT_FAILED_ACCEPTANCE"
                explanation = (
                    f"Treatment arm failed functional correctness requirements "
                    f"(Treatment B score: {treat_score:.2f}, Control A score: {ctrl_score:.2f}). "
                    f"A defective or incomplete patch cannot claim token efficiency savings."
                )
            else:
                verdict = "CONTROL_FAILED_ACCEPTANCE"
                explanation = (
                    f"Control arm failed functional correctness requirements "
                    f"(Control A score: {ctrl_score:.2f}, Treatment B score: {treat_score:.2f}). "
                    f"Baseline implementation was not established; comparative savings cannot be computed."
                )

            return {
                "savings_claim_eligible": False,
                "savings_claim_status": "INVALIDATED_BY_ACCEPTANCE_FAILURE",
                "verdict": verdict,
                "explanation": explanation,
                "metrics": {
                    "total_model_tokens": {
                        "control_A": ctrl_tel_data["total_model_tokens"] if ctrl_tel_data else None,
                        "treatment_B": treat_tel_data["total_model_tokens"] if treat_tel_data else None,
                        "delta": None,
                        "savings_pct": None,
                        "status": "INVALIDATED_UNACCEPTED"
                    },
                    "prompt_tokens_total": {
                        "control_A": ctrl_tel_data["prompt_tokens_total"] if ctrl_tel_data else None,
                        "treatment_B": treat_tel_data["prompt_tokens_total"] if treat_tel_data else None,
                        "delta": None,
                        "savings_pct": None,
                        "status": "INVALIDATED_UNACCEPTED"
                    },
                    "completion_tokens": {
                        "control_A": ctrl_tel_data["completion_tokens"] if ctrl_tel_data else None,
                        "treatment_B": treat_tel_data["completion_tokens"] if treat_tel_data else None,
                        "delta": None,
                        "savings_pct": None,
                        "status": "INVALIDATED_UNACCEPTED"
                    },
                    "peak_single_request_context": {
                        "control_A": ctrl_tel_data["peak_single_request_context"] if ctrl_tel_data else None,
                        "treatment_B": treat_tel_data["peak_single_request_context"] if treat_tel_data else None,
                        "delta": None,
                        "savings_pct": None,
                        "status": "INVALIDATED_UNACCEPTED"
                    },
                    "quality_adjusted_cost": {
                        "control_A": float(ctrl_corr.get("quality_adjusted_cost")) if (ctrl_tel_data and "quality_adjusted_cost" in ctrl_corr) else None,
                        "treatment_B": float(treat_corr.get("quality_adjusted_cost")) if (treat_tel_data and "quality_adjusted_cost" in treat_corr) else None,
                        "delta": None,
                        "savings_pct": None,
                        "status": "INVALIDATED_UNACCEPTED"
                    }
                }
            }

        # 4. Guard against invalid, missing, or zero-token telemetry EVEN IF BOTH PASSED CORRECTNESS
        if not (ctrl_tel_valid and treat_tel_valid):
            if not ctrl_tel_valid and not treat_tel_valid:
                verdict = "MUTUAL_TELEMETRY_INVALID"
                explanation = f"Both arms passed functional correctness (C = 1.0), BUT token telemetry is invalid or missing. Control: {ctrl_tel_reason}; Treatment: {treat_tel_reason}. Comparative savings claim is strictly ineligible."
            elif not treat_tel_valid:
                verdict = "TREATMENT_TELEMETRY_INVALID"
                explanation = f"Both arms passed functional correctness (C = 1.0), BUT Treatment arm telemetry is invalid or missing: {treat_tel_reason}. Savings claims cannot be made without verified treatment telemetry."
            else:
                verdict = "CONTROL_TELEMETRY_INVALID"
                explanation = f"Both arms passed functional correctness (C = 1.0), BUT Control arm telemetry is invalid or missing: {ctrl_tel_reason}. Baseline token count cannot be established; savings claim is ineligible."

            return {
                "savings_claim_eligible": False,
                "savings_claim_status": "INVALIDATED_BY_TELEMETRY_INVALID",
                "verdict": verdict,
                "explanation": explanation,
                "metrics": {
                    "total_model_tokens": {
                        "control_A": ctrl_tel_data["total_model_tokens"] if ctrl_tel_data else None,
                        "treatment_B": treat_tel_data["total_model_tokens"] if treat_tel_data else None,
                        "delta": None,
                        "savings_pct": None,
                        "status": "INVALIDATED_TELEMETRY"
                    },
                    "prompt_tokens_total": {
                        "control_A": ctrl_tel_data["prompt_tokens_total"] if ctrl_tel_data else None,
                        "treatment_B": treat_tel_data["prompt_tokens_total"] if treat_tel_data else None,
                        "delta": None,
                        "savings_pct": None,
                        "status": "INVALIDATED_TELEMETRY"
                    },
                    "completion_tokens": {
                        "control_A": ctrl_tel_data["completion_tokens"] if ctrl_tel_data else None,
                        "treatment_B": treat_tel_data["completion_tokens"] if treat_tel_data else None,
                        "delta": None,
                        "savings_pct": None,
                        "status": "INVALIDATED_TELEMETRY"
                    },
                    "peak_single_request_context": {
                        "control_A": ctrl_tel_data["peak_single_request_context"] if ctrl_tel_data else None,
                        "treatment_B": treat_tel_data["peak_single_request_context"] if treat_tel_data else None,
                        "delta": None,
                        "savings_pct": None,
                        "status": "INVALIDATED_TELEMETRY"
                    },
                    "quality_adjusted_cost": {
                        "control_A": float(ctrl_corr.get("quality_adjusted_cost")) if (ctrl_tel_data and "quality_adjusted_cost" in ctrl_corr) else None,
                        "treatment_B": float(treat_corr.get("quality_adjusted_cost")) if (treat_tel_data and "quality_adjusted_cost" in treat_corr) else None,
                        "delta": None,
                        "savings_pct": None,
                        "status": "INVALIDATED_TELEMETRY"
                    }
                }
            }

        # 5. BOTH ACCEPTED AND BOTH TELEMETRY VALID: Compute genuine savings
        ctrl_total = ctrl_tel_data["total_model_tokens"]
        treat_total = treat_tel_data["total_model_tokens"]
        ctrl_prompt = ctrl_tel_data["prompt_tokens_total"]
        treat_prompt = treat_tel_data["prompt_tokens_total"]
        ctrl_completion = ctrl_tel_data["completion_tokens"]
        treat_completion = treat_tel_data["completion_tokens"]
        ctrl_peak = ctrl_tel_data["peak_single_request_context"]
        treat_peak = treat_tel_data["peak_single_request_context"]
        ctrl_qac = float(ctrl_corr.get("quality_adjusted_cost", ctrl_total))
        treat_qac = float(treat_corr.get("quality_adjusted_cost", treat_total))

        total_delta = ctrl_total - treat_total
        total_savings_pct = round((total_delta / ctrl_total) * 100.0, 2)

        prompt_delta = ctrl_prompt - treat_prompt
        prompt_savings_pct = round((prompt_delta / ctrl_prompt) * 100.0, 2)

        completion_delta = ctrl_completion - treat_completion
        completion_savings_pct = round((completion_delta / ctrl_completion) * 100.0, 2) if ctrl_completion > 0 else 0.0

        if ctrl_peak is not None and treat_peak is not None and ctrl_peak > 0:
            peak_delta = ctrl_peak - treat_peak
            peak_savings_pct = round((peak_delta / ctrl_peak) * 100.0, 2)
            peak_status = "VALID"
        else:
            peak_delta = None
            peak_savings_pct = None
            peak_status = "UNAVAILABLE"

        qac_delta = round(ctrl_qac - treat_qac, 2)
        qac_savings_pct = round((qac_delta / ctrl_qac) * 100.0, 2)

        if total_savings_pct > 0:
            verdict = "EFFICIENCY_GAIN"
            explanation = f"Treatment B achieved full acceptance with {total_savings_pct}% total token savings ({total_delta:,} fewer tokens)."
        elif total_savings_pct < 0:
            verdict = "EFFICIENCY_REGRESSION"
            explanation = f"Treatment B achieved full acceptance but incurred a {abs(total_savings_pct)}% token regression ({abs(total_delta):,} more tokens)."
        else:
            verdict = "EFFICIENCY_NEUTRAL"
            explanation = "Treatment B and Control A used identical total token counts."

        return {
            "savings_claim_eligible": True,
            "savings_claim_status": "VALID",
            "verdict": verdict,
            "explanation": explanation,
            "metrics": {
                "total_model_tokens": {
                    "control_A": ctrl_total,
                    "treatment_B": treat_total,
                    "delta": total_delta,
                    "savings_pct": total_savings_pct,
                    "status": "VALID"
                },
                "prompt_tokens_total": {
                    "control_A": ctrl_prompt,
                    "treatment_B": treat_prompt,
                    "delta": prompt_delta,
                    "savings_pct": prompt_savings_pct,
                    "status": "VALID"
                },
                "completion_tokens": {
                    "control_A": ctrl_completion,
                    "treatment_B": treat_completion,
                    "delta": completion_delta,
                    "savings_pct": completion_savings_pct,
                    "status": "VALID"
                },
                "peak_single_request_context": {
                    "control_A": ctrl_peak,
                    "treatment_B": treat_peak,
                    "delta": peak_delta,
                    "savings_pct": peak_savings_pct,
                    "status": peak_status
                },
                "quality_adjusted_cost": {
                    "control_A": ctrl_qac,
                    "treatment_B": treat_qac,
                    "delta": qac_delta,
                    "savings_pct": qac_savings_pct,
                    "status": "VALID"
                }
            }
        }

    @staticmethod
    def generate_markdown_report(pair_summary: Dict[str, Any]) -> str:
        """Generates a human-readable matched-pair markdown report with explicit savings guards."""
        task_id = pair_summary.get("task_id", "")
        pair_id = pair_summary.get("pair_id", "")
        seed = pair_summary.get("seed", "")
        order = " -> ".join(pair_summary.get("order", []))
        model = pair_summary.get("model", "")
        reasoning = pair_summary.get("reasoning_effort", "")

        comp = pair_summary.get("comparative_analysis", {})
        eligible = comp.get("savings_claim_eligible", False)
        verdict = comp.get("verdict", "UNKNOWN")
        explanation = comp.get("explanation", "")
        metrics = comp.get("metrics", {})

        ctrl = pair_summary.get("runs", {}).get("control_A", {})
        treat = pair_summary.get("runs", {}).get("treatment_B", {})

        ctrl_corr = ctrl.get("correctness", {})
        treat_corr = treat.get("correctness", {})

        ctrl_score = ctrl_corr.get("score", 0.0)
        treat_score = treat_corr.get("score", 0.0)

        ctrl_pass = ctrl_corr.get("passed_all_requirements", False)
        treat_pass = treat_corr.get("passed_all_requirements", False)

        lines = [
            f"# Matched-Pair Benchmark Run Report: `{task_id}` (`{pair_id}`)",
            "",
            f"- **Model:** `{model}` (Reasoning: `{reasoning}`)",
            f"- **Execution Order:** {order} (Seed: `{seed}`)",
            f"- **Golden Dependency Fingerprint:** Verified Intact",
            "",
            "---",
            "",
            "## 1. Acceptance & Correctness Gate",
            "",
            "| Arm | Engram Status | Correctness Score ($C$) | Passed All Requirements | Functional Acceptance |",
            "| :--- | :---: | :---: | :---: | :---: |",
            f"| **Control Arm A** | DISABLED | {ctrl_score:.2f} | {ctrl_pass} | {'PASS' if ctrl_pass and ctrl_score == 1.0 else 'FAIL'} |",
            f"| **Treatment Arm B** | ACTIVE | {treat_score:.2f} | {treat_pass} | {'PASS' if treat_pass and treat_score == 1.0 else 'FAIL'} |",
            "",
        ]

        if not eligible:
            lines.extend([
                "> [!WARNING]",
                "> **COMPARATIVE TOKEN SAVINGS CLAIM INVALIDATED**",
                f"> - **Verdict:** `{verdict}`",
                f"> - **Explanation:** {explanation}",
                "> - **Scientific Safeguard:** An agent run that fails functional requirements ($C < 1.0$) cannot claim token savings. All comparative percentage metrics are strictly suppressed (`null`).",
                "",
            ])
        else:
            lines.extend([
                "> [!NOTE]",
                "> **COMPARATIVE TOKEN SAVINGS CLAIM VALID**",
                f"> - **Verdict:** `{verdict}`",
                f"> - **Explanation:** {explanation}",
                "> Both arms successfully fulfilled all requirements ($C = 1.0$).",
                "",
            ])

        lines.extend([
            "## 2. Telemetry Comparison",
            "",
            "| Telemetry Metric | Control Arm A (Engram OFF) | Treatment Arm B (Engram ON) | Comparative Delta / Savings |",
            "| :--- | :---: | :---: | :---: |",
        ])

        for mkey, mlabel in [
            ("total_model_tokens", "Total Model Tokens"),
            ("prompt_tokens_total", "Prompt Tokens (Total)"),
            ("completion_tokens", "Completion Tokens"),
            ("peak_single_request_context", "Peak Single-Request Context"),
            ("quality_adjusted_cost", "Quality-Adjusted Effective Cost"),
        ]:
            m = metrics.get(mkey, {})
            c_val = m.get("control_A")
            t_val = m.get("treatment_B")
            c_str = f"{c_val:,}" if isinstance(c_val, (int, float)) else str(c_val)
            t_str = f"{t_val:,}" if isinstance(t_val, (int, float)) else str(t_val)

            if m.get("status") == "INVALIDATED_UNACCEPTED":
                delta_str = "**INVALIDATED ($C < 1.0$)**"
            elif m.get("status") == "INVALIDATED_TELEMETRY":
                delta_str = "**INVALIDATED (Telemetry Invalid/Missing)**"
            elif m.get("status") == "UNAVAILABLE":
                delta_str = "UNAVAILABLE"
            else:
                pct = m.get("savings_pct")
                delta = m.get("delta")
                delta_str = f"**{pct:+.1f}%** ({delta:+,})" if pct is not None else "N/A"

            lines.append(f"| **{mlabel}** | {c_str} | {t_str} | {delta_str} |")

        lines.append("")
        return "\n".join(lines)

    @staticmethod
    def _print_pair_verdict(pair_summary: Dict[str, Any]) -> None:
        comp = pair_summary.get("comparative_analysis", {})
        eligible = comp.get("savings_claim_eligible", False)
        verdict = comp.get("verdict", "")
        m = comp.get("metrics", {})
        tot = m.get("total_model_tokens", {})

        print("\n=======================================================")
        if eligible:
            pct = tot.get("savings_pct")
            delta = tot.get("delta")
            print(f"Matched Pair Result: [VALID EFFICIENCY CLAIM - {verdict}]")
            print(f"  - Control A Tokens:   {tot.get('control_A'):,}")
            print(f"  - Treatment B Tokens: {tot.get('treatment_B'):,}")
            print(f"  - Claimable Savings:  {pct:+.2f}% ({delta:+,} tokens)")
            print(f"  - Acceptance:         Both arms achieved 100% (C = 1.0)")
        else:
            print(f"Matched Pair Result: [SAVINGS INVALIDATED - {verdict}]")
            print(f"  - Control A Accepted:   {pair_summary['acceptance']['control_A_accepted']}")
            print(f"  - Treatment B Accepted: {pair_summary['acceptance']['treatment_B_accepted']}")
            print(f"  - Token Savings Claim:  SUPPRESSED (null)")
            print(f"  - Reason: {comp.get('explanation')}")
        print("=======================================================\n")


    def _execute_single_arm(self, arm: str, exec_order: int) -> Dict[str, Any]:
        """Executes, evaluates, and verifies a single arm in an isolated sandbox."""
        print(f"\n=======================================================")
        print(f"Executing Arm: {arm} (Order: {exec_order}) [Task: {self.task_id}]")
        print(f"=======================================================")

        run_id = f"{self.task_id}_{self.pair_id}_{arm}"
        run_vault = self.output_dir / arm
        # 1. Verify Engram binary immediately before treatment execution
        if arm == "treatment_B":
            verify_frozen_binary(self.engram_bin, self.expected_treatment_sha256)

        # 2. Strict Live-Run Guard: Junction mode is forbidden for live trials
        if not self.is_dry_run and self.dependency_strategy != "copy":
            raise SafetyViolationError(
                f"FATAL: Live benchmark runs must use dependency_strategy='copy' to guarantee 100% write isolation. "
                f"Attempted to run live arm with '{self.dependency_strategy}'."
            )

        # 3. Verify golden dependency tree integrity prior to sandbox setup
        verify_dependency_integrity(self.golden_repo, self.golden_baseline_fingerprint)

        # 4. Provision sandbox
        baseline_commit = self.task_config.get("baseline_commit", "1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2")
        sandbox_path = create_sandbox(
            task_id=self.task_id,
            pair_id=self.pair_id,
            arm=arm,
            golden_repo=self.golden_repo,
            baseline_commit=baseline_commit,
            dependency_strategy=self.dependency_strategy
        )
        print(f"Sandbox created at: {sandbox_path}")

        try:
            # 4.1 Validate fixture preflight in the actual provisioned sandbox
            self.evaluator.validate_preflight(sandbox_path)

            # 4.2 Execute agent
            prompt = self.task_config.get("prompt", "")
            events_jsonl_path = run_vault / "events.jsonl"
            exec_res = self.runner.run(
                sandbox_path=sandbox_path,
                prompt=prompt,
                arm=arm,
                output_events_path=events_jsonl_path,
                is_dry_run=self.is_dry_run
            )

            # 5. Parse Telemetry
            provenance = "SYNTHETIC_TEST" if self.is_dry_run else "DIRECT_API"
            parser = TelemetryParser(provenance=provenance)
            telemetry = parser.parse_file(events_jsonl_path, thread_id=exec_res.thread_id)

            # 6. Evaluate Git Impact & Check for Dependency Mutations
            patch_path = run_vault / "changes.patch"
            git_impact = self.evaluator.analyze_git_impact(sandbox_path, patch_path)

            # 7. Automated Gates
            gate_results = self.evaluator.run_automated_gates(
                sandbox_path,
                git_impact=git_impact,
                is_dry_run=self.is_dry_run
            )

            # 8. Requirements & Correctness Score
            req_results = self.evaluator.evaluate_requirements(sandbox_path, git_impact, gate_results)
            correctness = self.evaluator.calculate_score(
                req_results=req_results,
                total_model_tokens=telemetry.total_model_tokens,
                gate_results=gate_results,
                dependency_mutated=git_impact.dependency_mutated
            )

            # 9. Build Telemetry Record adhering strictly to schema
            record = {
                "schema_version": "2.0.0",
                "run_id": run_id,
                "pair_id": self.pair_id,
                "task_id": self.task_id,
                "arm": arm,
                "execution_order_in_pair": exec_order,
                "timestamps": {
                    "started_at": exec_res.started_at,
                    "completed_at": exec_res.completed_at,
                    "elapsed_seconds": exec_res.elapsed_seconds,
                },
                "model": {
                    "provider": "openai",
                    "name": exec_res.model_name,
                    "mode": "agent",
                    "reasoning_effort": exec_res.reasoning_effort,
                    "harness_executable": exec_res.harness_executable,
                    "harness_version": exec_res.harness_version,
                },
                "isolation": {
                    "baseline_commit": baseline_commit,
                    "sandbox_path": str(sandbox_path),
                    "cold_start_verified": True,
                    "engram_enabled": (arm == "treatment_B"),
                    "engram_binary_sha256": self.expected_treatment_sha256,
                    "mcp_server_status": "ACTIVE" if arm == "treatment_B" else "DISABLED",
                    "dependency_fingerprint_verified": True,
                    "path_sanitized": exec_res.path_sanitized,
                },
                "telemetry": telemetry.to_dict(),
                "git_impact": git_impact.to_dict(),
                "correctness": correctness.to_dict(),
            }

            # 10. Validate strictly against schema
            validate_instance(record, self.telemetry_schema)

            # 11. Write sealed telemetry file
            telemetry_out = run_vault / "run_telemetry.json"
            with open(telemetry_out, "w", encoding="utf-8") as f:
                json.dump(record, f, indent=2)

            print(f"Run {run_id} completed successfully.")
            print(f"  - Total Model Tokens: {telemetry.total_model_tokens}")
            print(f"  - Peak Context:       {telemetry.peak_single_request_context or telemetry.peak_turn_context}")
            print(f"  - Correctness Score:  {correctness.score} (Accepted: {correctness.passed_all_requirements})")
            print(f"  - Effective Cost:     {correctness.quality_adjusted_cost}")

            return record

        finally:
            # 12. Teardown sandbox safely
            teardown_sandbox(sandbox_path, self.golden_repo, preserve_archive=self.preserve_sandboxes)
            # 13. Re-verify golden dependency tree integrity post teardown
            verify_dependency_integrity(self.golden_repo, self.golden_baseline_fingerprint)
            print(f"Sandbox cleaned up safely; golden dependency integrity verified.")


def main():
    parser = argparse.ArgumentParser(description="Reproducible Matched-Pair Benchmark Orchestrator")
    parser.add_argument("--task", default="task_c_theme_toggle", help="Task ID or path to task JSON")
    parser.add_argument("--pair", default="pair_01", help="Matched pair identifier (e.g. pair_01)")
    parser.add_argument("--model", default="gpt-5.6-luna", help="Explicit model identifier")
    parser.add_argument("--reasoning", default="medium", help="Explicit reasoning effort (low/medium/high/xhigh/max)")
    parser.add_argument("--dry-run", action="store_true", help="Simulate execution without model API consumption")
    parser.add_argument("--dependency-strategy", choices=["copy", "junction", "none"], default="copy", help="Dependency isolation mode (default: copy for full write isolation)")
    parser.add_argument("--preserve-sandboxes", action="store_true", help="Preserve sandbox folders")
    parser.add_argument("--seed", type=int, default=None, help="Randomization seed")
    parser.add_argument("--expected-treatment-sha256", default=None, help="Explicit expected SHA-256 for treatment Engram binary (defaults to FROZEN_ENGRAM_SHA256)")
    args = parser.parse_args()

    task_path = Path(args.task)
    if not task_path.exists():
        task_path = HARNESS_ROOT / "config" / "tasks" / f"{args.task}.json"
    if not task_path.exists():
        print(f"Error: Task configuration not found: {args.task}")
        sys.exit(1)

    orchestrator = Orchestrator(
        task_config_path=task_path,
        pair_id=args.pair,
        model=args.model,
        reasoning_effort=args.reasoning,
        is_dry_run=args.dry_run,
        dependency_strategy=args.dependency_strategy,
        preserve_sandboxes=args.preserve_sandboxes,
        seed=args.seed,
        expected_treatment_sha256=args.expected_treatment_sha256
    )

    results = orchestrator.run_pair()
    print("\nMatched Pair Execution Finished!")
    print(f"Summary saved to: {orchestrator.output_dir / 'matched_pair_summary.json'}")


if __name__ == "__main__":
    main()
