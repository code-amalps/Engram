"""
Comprehensive unit test suite for the Reproducible Matched-Pair Benchmark Harness.
Includes regression tests for all 4 preflight remediation items:
1. Dependency isolation & golden integrity verification.
2. Targeted PID tree termination (verifying unrelated processes are untouched).
3. Telemetry accounting (per-request rollout parsing, cumulative distinction, no double-counting).
4. Reproducibility & control isolation (PATH sanitation, explicit model/reasoning pinning).
"""
import unittest
import sys
import os
import json
import time
import subprocess
from pathlib import Path

# Add src to path
SRC_DIR = Path(__file__).resolve().parent.parent / "src"
sys.path.insert(0, str(SRC_DIR))

from schema_validator import validate_instance, SchemaValidationError
from telemetry import TelemetryParser, TelemetryMetrics
from evaluator import Evaluator, GitImpact
from isolation import (
    verify_frozen_binary,
    compute_dependency_fingerprint,
    verify_dependency_integrity,
    terminate_pid_tree,
    ProcessTracker,
    IsolationError,
    SafetyViolationError,
    GoldenDependencyCorruptionError,
    teardown_sandbox,
    FROZEN_ENGRAM_SHA256,
)
from runner import CodexRunner
from orchestrator import Orchestrator


FIXTURES_DIR = Path(__file__).resolve().parent / "fixtures"
SCHEMAS_DIR = Path(__file__).resolve().parent.parent / "schemas"
TASKS_DIR = Path(__file__).resolve().parent.parent / "config" / "tasks"
ENGRAM_BIN = Path(r"C:\Users\Amal P S\OneDrive\Projects\Engram\target\release\engram.exe")
GOLDEN_REPO = Path(r"C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter")
CODEX_BIN = Path(r"C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe")


class TestSchemaValidator(unittest.TestCase):
    def setUp(self):
        with open(SCHEMAS_DIR / "run_telemetry.schema.json", "r", encoding="utf-8") as f:
            self.telemetry_schema = json.load(f)
        with open(SCHEMAS_DIR / "correctness_checklist.schema.json", "r", encoding="utf-8") as f:
            self.task_schema = json.load(f)

    def test_all_tasks_valid(self):
        for task_file in TASKS_DIR.glob("*.json"):
            with open(task_file, "r", encoding="utf-8") as f:
                data = json.load(f)
            validate_instance(data, self.task_schema)

    def test_invalid_task_missing_field(self):
        bad_task = {
            "schema_version": "2.0.0",
            "task_id": "bad_task"
        }
        with self.assertRaises(SchemaValidationError):
            validate_instance(bad_task, self.task_schema)


class TestDependencyIsolationAndIntegrity(unittest.TestCase):
    def test_golden_dependency_fingerprint_integrity(self):
        # Baseline comprehensive manifest should verify against itself
        fp = compute_dependency_fingerprint(GOLDEN_REPO)
        self.assertIsNotNone(fp["package_json_sha256"])
        self.assertIsNotNone(fp["package_lock_sha256"])
        self.assertGreater(fp["node_modules_file_count"], 50000)
        self.assertGreater(fp["node_modules_total_bytes"], 100000000)
        self.assertIsNotNone(fp["node_modules_recursive_digest"])

        # Verification passes on clean repo
        verify_dependency_integrity(GOLDEN_REPO, fp)

    def test_mutation_detection_fails_on_tampering(self):
        fp = compute_dependency_fingerprint(GOLDEN_REPO)
        corrupted_fp = dict(fp)
        corrupted_fp["node_modules_recursive_digest"] = "corrupted_digest_000000"

        with self.assertRaises(GoldenDependencyCorruptionError):
            verify_dependency_integrity(GOLDEN_REPO, corrupted_fp)

    def test_evaluator_fails_on_dependency_mutation(self):
        with open(TASKS_DIR / "task_c_theme_toggle.json", "r", encoding="utf-8") as f:
            task_cfg = json.load(f)
        evaluator = Evaluator(task_cfg)

        # Simulated git impact where agent modified package.json
        bad_impact = GitImpact(
            files_changed_count=1,
            files_changed=["package.json"],
            insertions=2,
            deletions=1,
            unrelated_files_count=1,
            diff_patch_path="dummy.patch",
            dependency_mutated=True
        )

        gates = evaluator.run_automated_gates(GOLDEN_REPO, git_impact=bad_impact, is_dry_run=True)
        self.assertEqual(gates["L0_typecheck"], "FAIL")

        req_res = evaluator.evaluate_requirements(GOLDEN_REPO, bad_impact, gates)
        for req_id, status in req_res.items():
            self.assertEqual(status, "FAIL")

        score_res = evaluator.calculate_score(req_res, 10000, gates, dependency_mutated=True)
        self.assertEqual(score_res.score, 0.0)
        self.assertFalse(score_res.passed_all_requirements)


class TestProcessLifecycleSafety(unittest.TestCase):
    def test_terminate_pid_tree_does_not_touch_unrelated_process(self):
        # Spawn two independent dummy processes
        p1 = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"])
        p2 = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"])

        try:
            self.assertIsNone(p1.poll(), "p1 should be running")
            self.assertIsNone(p2.poll(), "p2 should be running")

            # Terminate only p1
            success = terminate_pid_tree(p1.pid)
            self.assertTrue(success)

            # Wait briefly for process reap
            time.sleep(0.5)

            # p1 must be dead
            self.assertIsNotNone(p1.poll(), "Target process p1 must be terminated")

            # CRITICAL SAFETY CHECK: p2 must STILL BE RUNNING (unrelated process untouched)
            self.assertIsNone(p2.poll(), "Unrelated process p2 MUST remain running")

        finally:
            # Clean up p2 safely
            terminate_pid_tree(p2.pid)
            p1.kill()
            p2.kill()

    def test_process_tracker(self):
        tracker = ProcessTracker()
        p = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"])
        tracker.track(p.pid)
        self.assertIn(p.pid, tracker.tracked_pids)

        terminated_count = tracker.terminate_all()
        self.assertEqual(terminated_count, 1)
        time.sleep(0.5)
        self.assertIsNotNone(p.poll())


class TestTelemetryAccounting(unittest.TestCase):
    def setUp(self):
        self.parser = TelemetryParser(provenance="DIRECT_API")

    def test_real_session_rollout_fixture(self):
        # Test against real captured rollout log with 3 internal model calls
        rollout_file = FIXTURES_DIR / "real_session_rollout.jsonl"
        metrics = self.parser.parse_rollout_file(rollout_file)

        # Total prompt tokens (sum of the 3 calls: 15684 + 15835 + 16022 = 47541)
        self.assertEqual(metrics.prompt_tokens_total, 47541)
        self.assertEqual(metrics.prompt_tokens_cached, 39296)
        self.assertFalse(metrics.prompt_tokens_cached_inferred)
        self.assertEqual(metrics.prompt_tokens_uncached, 8245)
        self.assertTrue(metrics.prompt_tokens_uncached_inferred)
        self.assertEqual(metrics.completion_tokens, 83)
        self.assertEqual(metrics.reasoning_tokens, 9)
        self.assertEqual(metrics.total_model_tokens, 47624)

        # EXACT SINGLE-REQUEST CONTEXT: Should be 16022, NOT 47541!
        self.assertEqual(metrics.peak_single_request_context, 16022)
        self.assertEqual(metrics.final_turn_context, 16022)
        self.assertEqual(metrics.model_requests_count, 3)

        # Invariant checks: uncached + cached == total
        self.assertEqual(metrics.prompt_tokens_uncached + metrics.prompt_tokens_cached, metrics.prompt_tokens_total)
        # Total model tokens == prompt total + completion (no double-counting of reasoning or cached)
        self.assertEqual(metrics.total_model_tokens, metrics.prompt_tokens_total + metrics.completion_tokens)

    def test_stdout_events_marks_single_context_unavailable(self):
        # When parsing only stdout events without rollout, single-request context is marked None
        real_stdout_file = FIXTURES_DIR / "real_handshake.jsonl"
        metrics = self.parser.parse_file(real_stdout_file, thread_id="nonexistent-thread-id")

        self.assertEqual(metrics.prompt_tokens_total, 47541)
        self.assertEqual(metrics.prompt_tokens_cached, 39296)
        self.assertFalse(metrics.prompt_tokens_cached_inferred)
        self.assertEqual(metrics.prompt_tokens_uncached, 8245)
        self.assertTrue(metrics.prompt_tokens_uncached_inferred)
        # Peak single request context must be None (UNAVAILABLE) to prevent mislabeling cumulative as context
        self.assertIsNone(metrics.peak_single_request_context)
        # But peak turn context snapshot is available
        self.assertEqual(metrics.peak_turn_context, 47541)

    def test_tool_call_deduplication(self):
        # Synthetic events containing duplicate tool call events for the same item ID
        dup_events = [
            {"type": "item.completed", "item": {"id": "tool_1", "type": "mcp_tool_call", "arguments": {"q": "test"}, "result": {"res": "ok"}}},
            {"type": "item.completed", "item": {"id": "tool_1", "type": "mcp_tool_call", "arguments": {"q": "test"}, "result": {"res": "ok"}}},  # DUPLICATE
            {"type": "item.completed", "item": {"id": "tool_2", "type": "mcp_tool_call", "arguments": {"q": "second"}, "result": {"res": "ok"}}}
        ]
        metrics = self.parser.parse_events(dup_events)
        # Must count exactly 2 tool calls, ignoring duplicate
        self.assertEqual(metrics.mcp_tool_calls_count, 2)


class TestReproducibilityAndControlIsolation(unittest.TestCase):
    def setUp(self):
        self.runner = CodexRunner(
            codex_executable=CODEX_BIN,
            engram_executable=ENGRAM_BIN,
            model="gpt-5.6-luna",
            reasoning_effort="medium"
        )

    def test_path_sanitation_removes_engram(self):
        fake_path = r"C:\Windows\System32;C:\Users\Amal P S\AppData\Local\Programs\Engram\bin;C:\Users\Amal P S\AppData\Local\Programs\Python\Python314"
        test_env = {"PATH": fake_path}

        sanitized_env, was_sanitized = self.runner.sanitize_path(test_env)
        self.assertTrue(was_sanitized)

        sanitized_parts = sanitized_env["PATH"].split(os.pathsep)
        self.assertIn(r"C:\Windows\System32", sanitized_parts)
        self.assertIn(r"C:\Users\Amal P S\AppData\Local\Programs\Python\Python314", sanitized_parts)
        for p in sanitized_parts:
            self.assertNotIn("engram", p.lower())

    def test_frozen_binary_verification(self):
        self.assertTrue(verify_frozen_binary(ENGRAM_BIN, FROZEN_ENGRAM_SHA256))

    def test_safety_guard_prevents_arbitrary_deletion(self):
        unsafe_path = Path(r"C:\Users\Amal P S\OneDrive\Projects\Engram")
        with self.assertRaises(SafetyViolationError):
            teardown_sandbox(unsafe_path, unsafe_path)


class TestComparativeSavingsGuard(unittest.TestCase):
    def setUp(self):
        with open(SCHEMAS_DIR / "matched_pair_summary.schema.json", "r", encoding="utf-8") as f:
            self.summary_schema = json.load(f)

    def _make_dummy_run(
        self,
        score: float,
        passed: bool,
        total_tokens: int,
        prompt_tokens: Optional[int] = None,
        peak_single: Optional[int] = None,
        cached_inferred: bool = False,
        uncached_inferred: bool = True
    ):
        if prompt_tokens is None:
            prompt_tokens = int(total_tokens * 0.8)
        completion_tokens = max(0, total_tokens - prompt_tokens)
        cached = int(prompt_tokens * 0.5)
        uncached = prompt_tokens - cached
        return {
            "correctness": {
                "score": score,
                "passed_all_requirements": passed,
                "quality_adjusted_cost": round(total_tokens / max(score, 0.01), 2)
            },
            "telemetry": {
                "total_model_tokens": total_tokens,
                "prompt_tokens_total": prompt_tokens,
                "prompt_tokens_cached": cached,
                "prompt_tokens_cached_inferred": cached_inferred,
                "prompt_tokens_uncached": uncached,
                "prompt_tokens_uncached_inferred": uncached_inferred,
                "completion_tokens": completion_tokens,
                "peak_single_request_context": peak_single
            }
        }

    def test_savings_invalidated_on_mutual_failure(self):
        ctrl = self._make_dummy_run(score=0.70, passed=False, total_tokens=28720)
        treat = self._make_dummy_run(score=0.70, passed=False, total_tokens=12650)

        comp = Orchestrator.compute_comparative_analysis(ctrl, treat)

        # Invariant 1: Savings claim is not eligible
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_ACCEPTANCE_FAILURE")
        self.assertEqual(comp["verdict"], "MUTUAL_ACCEPTANCE_FAILURE")

        # Invariant 2: Token percentages and deltas are strictly suppressed (None)
        tot_metric = comp["metrics"]["total_model_tokens"]
        self.assertIsNone(tot_metric["savings_pct"])
        self.assertIsNone(tot_metric["delta"])
        self.assertEqual(tot_metric["status"], "INVALIDATED_UNACCEPTED")

        # Invariant 3: Effective cost savings percentage is also strictly suppressed (None)
        qac_metric = comp["metrics"]["quality_adjusted_cost"]
        self.assertIsNone(qac_metric["savings_pct"])
        self.assertIsNone(qac_metric["delta"])
        self.assertEqual(qac_metric["status"], "INVALIDATED_UNACCEPTED")

        # Invariant 4: Markdown report displays INVALIDATED banner and table entries
        pair_summary = {
            "schema_version": "2.0.0",
            "pair_id": "pair_test",
            "task_id": "task_c_theme_toggle",
            "seed": 1234,
            "order": ["control_A", "treatment_B"],
            "model": "gpt-5.6-luna",
            "reasoning_effort": "medium",
            "golden_dependency_fingerprint_verified": True,
            "acceptance": {
                "control_A_accepted": False,
                "treatment_B_accepted": False,
                "pair_acceptance_parity": True
            },
            "comparative_analysis": comp,
            "runs": {"control_A": ctrl, "treatment_B": treat}
        }
        validate_instance(pair_summary, self.summary_schema)

        md = Orchestrator.generate_markdown_report(pair_summary)
        self.assertIn("COMPARATIVE TOKEN SAVINGS CLAIM INVALIDATED", md)
        self.assertIn("**INVALIDATED ($C < 1.0$)**", md)
        self.assertNotIn("-56.0%", md)

    def test_savings_invalidated_when_treatment_fails(self):
        # Treatment cuts corners, uses 70% fewer tokens, but fails requirements
        ctrl = self._make_dummy_run(score=1.0, passed=True, total_tokens=30000)
        treat = self._make_dummy_run(score=0.70, passed=False, total_tokens=9000)

        comp = Orchestrator.compute_comparative_analysis(ctrl, treat)

        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_ACCEPTANCE_FAILURE")
        self.assertEqual(comp["verdict"], "TREATMENT_FAILED_ACCEPTANCE")
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["savings_pct"])
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["delta"])
        self.assertEqual(comp["metrics"]["total_model_tokens"]["status"], "INVALIDATED_UNACCEPTED")

    def test_savings_invalidated_when_control_fails(self):
        ctrl = self._make_dummy_run(score=0.50, passed=False, total_tokens=20000)
        treat = self._make_dummy_run(score=1.0, passed=True, total_tokens=15000)

        comp = Orchestrator.compute_comparative_analysis(ctrl, treat)

        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_ACCEPTANCE_FAILURE")
        self.assertEqual(comp["verdict"], "CONTROL_FAILED_ACCEPTANCE")
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["savings_pct"])
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["delta"])

    def test_savings_valid_when_both_pass(self):
        # Legitimate success: both pass 100% of requirements
        ctrl = self._make_dummy_run(score=1.0, passed=True, total_tokens=40000, prompt_tokens=35000, peak_single=15000)
        treat = self._make_dummy_run(score=1.0, passed=True, total_tokens=20000, prompt_tokens=18000, peak_single=10000)

        comp = Orchestrator.compute_comparative_analysis(ctrl, treat)

        self.assertTrue(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "VALID")
        self.assertEqual(comp["verdict"], "EFFICIENCY_GAIN")

        tot_metric = comp["metrics"]["total_model_tokens"]
        self.assertEqual(tot_metric["delta"], 20000)
        self.assertEqual(tot_metric["savings_pct"], 50.0)
        self.assertEqual(tot_metric["status"], "VALID")

        peak_metric = comp["metrics"]["peak_single_request_context"]
        self.assertEqual(peak_metric["delta"], 5000)
        self.assertEqual(peak_metric["savings_pct"], 33.33)
        self.assertEqual(peak_metric["status"], "VALID")

        pair_summary = {
            "schema_version": "2.0.0",
            "pair_id": "pair_valid",
            "task_id": "task_c_theme_toggle",
            "seed": 5678,
            "order": ["control_A", "treatment_B"],
            "model": "gpt-5.6-luna",
            "reasoning_effort": "medium",
            "golden_dependency_fingerprint_verified": True,
            "acceptance": {
                "control_A_accepted": True,
                "treatment_B_accepted": True,
                "pair_acceptance_parity": True
            },
            "comparative_analysis": comp,
            "runs": {"control_A": ctrl, "treatment_B": treat}
        }
        validate_instance(pair_summary, self.summary_schema)

        md = Orchestrator.generate_markdown_report(pair_summary)
        self.assertIn("COMPARATIVE TOKEN SAVINGS CLAIM VALID", md)
        self.assertIn("**+50.0%** (+20,000)", md)

    def test_savings_ineligible_when_treatment_telemetry_missing_even_if_accepted(self):
        """Even if both pass C=1.0, missing Treatment telemetry must suppress savings claims."""
        ctrl = self._make_dummy_run(score=1.0, passed=True, total_tokens=30000)
        treat = self._make_dummy_run(score=1.0, passed=True, total_tokens=20000)
        treat["telemetry"] = {}  # Missing telemetry

        comp = Orchestrator.compute_comparative_analysis(ctrl, treat)
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_TELEMETRY_INVALID")
        self.assertEqual(comp["verdict"], "TREATMENT_TELEMETRY_INVALID")
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["savings_pct"])
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["treatment_B"])
        self.assertEqual(comp["metrics"]["total_model_tokens"]["status"], "INVALIDATED_TELEMETRY")

        # Validate against schema
        pair_summary = {
            "schema_version": "2.0.0",
            "pair_id": "pair_missing_treat",
            "task_id": "task_c_theme_toggle",
            "seed": 9999,
            "order": ["control_A", "treatment_B"],
            "model": "gpt-5.6-luna",
            "reasoning_effort": "medium",
            "golden_dependency_fingerprint_verified": True,
            "acceptance": {
                "control_A_accepted": True,
                "treatment_B_accepted": True,
                "pair_acceptance_parity": True
            },
            "comparative_analysis": comp,
            "runs": {"control_A": ctrl, "treatment_B": treat}
        }
        validate_instance(pair_summary, self.summary_schema)

    def test_savings_ineligible_on_zero_token_anomaly_even_if_accepted(self):
        """A 0-token Treatment run with C=1.0 must NOT claim 100% token savings."""
        ctrl = self._make_dummy_run(score=1.0, passed=True, total_tokens=30000)
        treat = self._make_dummy_run(score=1.0, passed=True, total_tokens=0, prompt_tokens=0)

        comp = Orchestrator.compute_comparative_analysis(ctrl, treat)
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_TELEMETRY_INVALID")
        self.assertEqual(comp["verdict"], "TREATMENT_TELEMETRY_INVALID")
        self.assertIn("ZERO_TOKEN_ANOMALY", comp["explanation"])
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["savings_pct"])
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["delta"])

    def test_savings_ineligible_on_control_telemetry_missing(self):
        """Missing Control telemetry prevents establishing baseline cost."""
        ctrl = self._make_dummy_run(score=1.0, passed=True, total_tokens=30000)
        ctrl["telemetry"]["total_model_tokens"] = None
        treat = self._make_dummy_run(score=1.0, passed=True, total_tokens=15000)

        comp = Orchestrator.compute_comparative_analysis(ctrl, treat)
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_TELEMETRY_INVALID")
        self.assertEqual(comp["verdict"], "CONTROL_TELEMETRY_INVALID")
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["savings_pct"])
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["control_A"])

    def test_savings_ineligible_when_both_telemetry_missing(self):
        """When both arms have missing telemetry, mutual telemetry invalidation is reported."""
        ctrl = self._make_dummy_run(score=1.0, passed=True, total_tokens=30000)
        ctrl["telemetry"] = {}
        treat = self._make_dummy_run(score=1.0, passed=True, total_tokens=15000)
        treat["telemetry"] = {}

        comp = Orchestrator.compute_comparative_analysis(ctrl, treat)
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_TELEMETRY_INVALID")
        self.assertEqual(comp["verdict"], "MUTUAL_TELEMETRY_INVALID")
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["savings_pct"])

    def test_savings_ineligible_on_telemetry_invariant_violation(self):
        """Token counts violating prompt + completion == total must be rejected."""
        ctrl = self._make_dummy_run(score=1.0, passed=True, total_tokens=30000)
        treat = self._make_dummy_run(score=1.0, passed=True, total_tokens=15000)
        treat["telemetry"]["completion_tokens"] = 99999  # Corrupt invariant

        comp = Orchestrator.compute_comparative_analysis(ctrl, treat)
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertIn("TELEMETRY_INVARIANT_VIOLATION", comp["explanation"])
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["savings_pct"])

    def test_savings_ineligible_on_boolean_token_counts(self):
        """Booleans must be strictly rejected as token counts (issubclass(bool, int) is True)."""
        ctrl = self._make_dummy_run(score=1.0, passed=True, total_tokens=30000)
        treat = self._make_dummy_run(score=1.0, passed=True, total_tokens=15000)
        treat["telemetry"]["total_model_tokens"] = True  # Boolean injection

        comp = Orchestrator.compute_comparative_analysis(ctrl, treat)
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_TELEMETRY_INVALID")
        self.assertIn("TELEMETRY_MALFORMED", comp["explanation"])
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["savings_pct"])

        # Test boolean in cached tokens
        treat2 = self._make_dummy_run(score=1.0, passed=True, total_tokens=15000)
        treat2["telemetry"]["prompt_tokens_cached"] = True
        comp2 = Orchestrator.compute_comparative_analysis(ctrl, treat2)
        self.assertFalse(comp2["savings_claim_eligible"])
        self.assertIn("TELEMETRY_MALFORMED", comp2["explanation"])

        # Test boolean in uncached tokens
        treat3 = self._make_dummy_run(score=1.0, passed=True, total_tokens=15000)
        treat3["telemetry"]["prompt_tokens_uncached"] = False
        comp3 = Orchestrator.compute_comparative_analysis(ctrl, treat3)
        self.assertFalse(comp3["savings_claim_eligible"])
        self.assertIn("TELEMETRY_MALFORMED", comp3["explanation"])

    def test_savings_ineligible_on_malformed_cached_uncached(self):
        """Non-integer, negative, or inconsistent cached/uncached values must be rejected."""
        ctrl = self._make_dummy_run(score=1.0, passed=True, total_tokens=30000)

        # 1. Negative cached tokens
        treat_neg = self._make_dummy_run(score=1.0, passed=True, total_tokens=15000)
        treat_neg["telemetry"]["prompt_tokens_cached"] = -50
        comp_neg = Orchestrator.compute_comparative_analysis(ctrl, treat_neg)
        self.assertFalse(comp_neg["savings_claim_eligible"])
        self.assertIn("TELEMETRY_MALFORMED", comp_neg["explanation"])

        # 2. String cached tokens
        treat_str = self._make_dummy_run(score=1.0, passed=True, total_tokens=15000)
        treat_str["telemetry"]["prompt_tokens_cached"] = "1000"
        comp_str = Orchestrator.compute_comparative_analysis(ctrl, treat_str)
        self.assertFalse(comp_str["savings_claim_eligible"])
        self.assertIn("TELEMETRY_MALFORMED", comp_str["explanation"])

        # 3. Cached + uncached != prompt total
        treat_inconsistent = self._make_dummy_run(score=1.0, passed=True, total_tokens=15000)
        treat_inconsistent["telemetry"]["prompt_tokens_cached"] = 8000
        treat_inconsistent["telemetry"]["prompt_tokens_uncached"] = 3000  # Sum 11000 != prompt (12000)
        comp_inconsistent = Orchestrator.compute_comparative_analysis(ctrl, treat_inconsistent)
        self.assertFalse(comp_inconsistent["savings_claim_eligible"])
        self.assertIn("TELEMETRY_INVARIANT_VIOLATION", comp_inconsistent["explanation"])

        # 4. Malformed peak single request context (boolean, string, negative)
        treat_peak = self._make_dummy_run(score=1.0, passed=True, total_tokens=15000)
        treat_peak["telemetry"]["peak_single_request_context"] = True
        comp_peak = Orchestrator.compute_comparative_analysis(ctrl, treat_peak)
        self.assertFalse(comp_peak["savings_claim_eligible"])
        self.assertIn("TELEMETRY_MALFORMED", comp_peak["explanation"])


class TestTokenProvenanceAndCacheIntegrity(unittest.TestCase):
    def setUp(self):
        with open(SCHEMAS_DIR / "run_telemetry.schema.json", "r", encoding="utf-8") as f:
            self.telemetry_schema = json.load(f)
        with open(SCHEMAS_DIR / "matched_pair_summary.schema.json", "r", encoding="utf-8") as f:
            self.summary_schema = json.load(f)

    def test_case_1_provider_supplies_cached_uncached_derived(self):
        """1. When provider supplies cached tokens, uncached is derived and marked inferred."""
        tel = {
            "total_model_tokens": 1000,
            "prompt_tokens_total": 800,
            "completion_tokens": 200,
            "prompt_tokens_cached": 500,
            "prompt_tokens_cached_inferred": False
        }
        valid, reason, parsed = Orchestrator.validate_telemetry_integrity(tel)
        self.assertTrue(valid, reason)
        self.assertEqual(parsed["prompt_tokens_cached"], 500)
        self.assertFalse(parsed["prompt_tokens_cached_inferred"])
        self.assertEqual(parsed["prompt_tokens_uncached"], 300)
        self.assertTrue(parsed["prompt_tokens_uncached_inferred"])

    def test_case_2_cached_missing_inferred_from_uncached(self):
        """2. When cached is missing, it is inferred from uncached and marked inferred."""
        tel = {
            "total_model_tokens": 1000,
            "prompt_tokens_total": 800,
            "completion_tokens": 200,
            "prompt_tokens_uncached": 300,
            "prompt_tokens_uncached_inferred": False
        }
        valid, reason, parsed = Orchestrator.validate_telemetry_integrity(tel)
        self.assertTrue(valid, reason)
        self.assertEqual(parsed["prompt_tokens_cached"], 500)
        self.assertTrue(parsed["prompt_tokens_cached_inferred"])
        self.assertEqual(parsed["prompt_tokens_uncached"], 300)
        self.assertFalse(parsed["prompt_tokens_uncached_inferred"])

    def test_case_3_uncached_missing_inferred_from_cached(self):
        """3. When uncached is missing, it is inferred from cached and marked inferred."""
        tel = {
            "total_model_tokens": 1000,
            "prompt_tokens_total": 800,
            "completion_tokens": 200,
            "prompt_tokens_cached": 600,
            "prompt_tokens_cached_inferred": False
        }
        valid, reason, parsed = Orchestrator.validate_telemetry_integrity(tel)
        self.assertTrue(valid, reason)
        self.assertEqual(parsed["prompt_tokens_cached"], 600)
        self.assertFalse(parsed["prompt_tokens_cached_inferred"])
        self.assertEqual(parsed["prompt_tokens_uncached"], 200)
        self.assertTrue(parsed["prompt_tokens_uncached_inferred"])

    def test_case_4_both_cache_counts_missing_both_marked_inferred(self):
        """4. When both cache counts are missing, neither is falsely represented as measured."""
        tel = {
            "total_model_tokens": 1000,
            "prompt_tokens_total": 800,
            "completion_tokens": 200
        }
        valid, reason, parsed = Orchestrator.validate_telemetry_integrity(tel)
        self.assertTrue(valid, reason)
        self.assertEqual(parsed["prompt_tokens_cached"], 0)
        self.assertTrue(parsed["prompt_tokens_cached_inferred"])
        self.assertEqual(parsed["prompt_tokens_uncached"], 800)
        self.assertTrue(parsed["prompt_tokens_uncached_inferred"])

    def test_case_5_existing_inference_flags_survive_repeated_validation(self):
        """5. Existing inference flags survive repeated validation and normalization."""
        tel = {
            "total_model_tokens": 1000,
            "prompt_tokens_total": 800,
            "completion_tokens": 200,
            "prompt_tokens_cached": 400,
            "prompt_tokens_cached_inferred": True,
            "prompt_tokens_uncached": 400,
            "prompt_tokens_uncached_inferred": True
        }
        valid1, reason1, parsed1 = Orchestrator.validate_telemetry_integrity(tel)
        self.assertTrue(valid1, reason1)
        self.assertTrue(parsed1["prompt_tokens_cached_inferred"])
        self.assertTrue(parsed1["prompt_tokens_uncached_inferred"])

        # Second pass: Ensure True is never overwritten with False
        valid2, reason2, parsed2 = Orchestrator.validate_telemetry_integrity(parsed1)
        self.assertTrue(valid2, reason2)
        self.assertTrue(parsed2["prompt_tokens_cached_inferred"])
        self.assertTrue(parsed2["prompt_tokens_uncached_inferred"])

    def test_case_6_missing_provenance_flag_defaults_to_inferred(self):
        """6. A missing provenance flag is defaulted to inferred (True) to prevent unproven measured claims."""
        tel = {
            "total_model_tokens": 1000,
            "prompt_tokens_total": 800,
            "completion_tokens": 200,
            "prompt_tokens_cached": 500,
            "prompt_tokens_uncached": 300
        }
        valid, reason, parsed = Orchestrator.validate_telemetry_integrity(tel)
        self.assertTrue(valid, reason)
        self.assertTrue(parsed["prompt_tokens_cached_inferred"])
        self.assertTrue(parsed["prompt_tokens_uncached_inferred"])

    def test_case_7_non_boolean_provenance_flags_rejected(self):
        """7. Non-boolean provenance flags (strings, integers, lists) are rejected as TELEMETRY_MALFORMED."""
        base_tel = {
            "total_model_tokens": 1000,
            "prompt_tokens_total": 800,
            "completion_tokens": 200,
            "prompt_tokens_cached": 500,
            "prompt_tokens_uncached": 300
        }
        # String instead of bool
        tel_str = dict(base_tel, prompt_tokens_cached_inferred="false")
        v, r, _ = Orchestrator.validate_telemetry_integrity(tel_str)
        self.assertFalse(v)
        self.assertIn("TELEMETRY_MALFORMED", r)

        # Integer 1 instead of bool
        tel_int = dict(base_tel, prompt_tokens_uncached_inferred=1)
        v, r, _ = Orchestrator.validate_telemetry_integrity(tel_int)
        self.assertFalse(v)
        self.assertIn("TELEMETRY_MALFORMED", r)

        # List instead of bool
        tel_list = dict(base_tel, prompt_tokens_cached_inferred=[True])
        v, r, _ = Orchestrator.validate_telemetry_integrity(tel_list)
        self.assertFalse(v)
        self.assertIn("TELEMETRY_MALFORMED", r)

    def test_case_8_negative_inconsistent_cache_counts_remain_invalid(self):
        """8. Negative, inconsistent or malformed cache counts remain strictly invalid."""
        parser = TelemetryParser()
        bad_events = [
            {
                "type": "turn.completed",
                "usage": {
                    "input_tokens": 100,
                    "cached_input_tokens": 200,  # cached > total
                    "output_tokens": 10
                }
            }
        ]
        with self.assertRaises(ValueError) as ctx:
            parser.parse_events(bad_events)
        self.assertIn("exceeds input_tokens", str(ctx.exception))

        bad_neg_events = [
            {
                "type": "turn.completed",
                "usage": {
                    "input_tokens": 100,
                    "cached_input_tokens": -5,  # negative
                    "output_tokens": 10
                }
            }
        ]
        with self.assertRaises(ValueError) as ctx:
            parser.parse_events(bad_neg_events)
        self.assertIn("negative", str(ctx.exception))

        # Validator rejects cached > prompt_tokens
        tel_exceed = {
            "total_model_tokens": 1000,
            "prompt_tokens_total": 800,
            "completion_tokens": 200,
            "prompt_tokens_cached": 900
        }
        v, r, _ = Orchestrator.validate_telemetry_integrity(tel_exceed)
        self.assertFalse(v)
        self.assertIn("TELEMETRY_INVARIANT_VIOLATION", r)

    def test_case_9_serialization_preserves_provenance_flags(self):
        """9. Serialized run_telemetry.json and matched_pair_summary.json preserve the provenance flags."""
        record = {
            "schema_version": "2.0.0",
            "run_id": "test_run_prov",
            "pair_id": "test_pair",
            "task_id": "test_task",
            "arm": "treatment_B",
            "execution_order_in_pair": 2,
            "timestamps": {
                "started_at": "2026-10-18T12:00:00Z",
                "completed_at": "2026-10-18T12:01:00Z",
                "elapsed_seconds": 60.0
            },
            "model": {
                "provider": "openai",
                "name": "gpt-5.6-luna",
                "mode": "agent",
                "reasoning_effort": "medium",
                "harness_executable": "codex.exe",
                "harness_version": "0.163.0"
            },
            "isolation": {
                "baseline_commit": "1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2",
                "sandbox_path": "C:\\temp\\sandbox",
                "cold_start_verified": True,
                "engram_enabled": True,
                "engram_binary_sha256": "474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0",
                "mcp_server_status": "ACTIVE",
                "dependency_fingerprint_verified": True,
                "path_sanitized": True
            },
            "telemetry": {
                "provenance": "SESSION_ROLLOUT",
                "prompt_tokens_total": 10000,
                "prompt_tokens_cached": 7000,
                "prompt_tokens_cached_inferred": False,
                "prompt_tokens_uncached": 3000,
                "prompt_tokens_uncached_inferred": True,
                "completion_tokens": 500,
                "reasoning_tokens": 50,
                "total_model_tokens": 10500,
                "peak_single_request_context": 5000,
                "peak_turn_context": 10000,
                "final_turn_context": 5000,
                "model_requests_count": 2,
                "turns_count": 2,
                "mcp_tool_calls_count": 4,
                "auxiliary_tool_payload_bytes": 1024,
                "account_usage_reading_start_pct": None,
                "account_usage_reading_end_pct": None
            },
            "git_impact": {
                "files_changed_count": 1,
                "files_changed": ["file.ts"],
                "insertions": 5,
                "deletions": 2,
                "unrelated_files_count": 0,
                "diff_patch_path": "changes.patch"
            },
            "correctness": {
                "score": 1.0,
                "quality_adjusted_cost": 10500.0,
                "passed_all_requirements": True,
                "gates": {
                    "L0_typecheck": "PASS",
                    "L1_lint_prettier": "PASS",
                    "L2_unit_tests": "PASS",
                    "L3_production_build": "PASS"
                },
                "requirements": {
                    "REQ-1": "PASS"
                }
            }
        }
        # Schema validation
        validate_instance(record, self.telemetry_schema)

        # JSON Roundtrip serialization
        serialized = json.dumps(record)
        deserialized = json.loads(serialized)
        self.assertIs(deserialized["telemetry"]["prompt_tokens_cached_inferred"], False)
        self.assertIs(deserialized["telemetry"]["prompt_tokens_uncached_inferred"], True)
        validate_instance(deserialized, self.telemetry_schema)


if __name__ == "__main__":
    unittest.main()


