import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

# Ensure src is on sys.path
HARNESS_ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = HARNESS_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from orchestrator import Orchestrator, SafetyViolationError
from runner import StreamingTokenTracker


class TestTetradOrderValidation(unittest.TestCase):
    """Verifies fail-closed tetrad order validation and zero-side-effect guarantees (Workstream A)."""

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.output_dir = Path(self.temp_dir.name) / "order_val_out"
        self.task_cfg = HARNESS_ROOT / "config" / "tasks" / "task_a_typo.json"

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_t01_valid_order_adcb(self):
        """T01: Accept exact valid order A,D,C,B."""
        raw = "arm_a_native,arm_d_cand4_adaptive,arm_c_cand4_always_on,arm_b_cand3_always_on"
        res = Orchestrator.validate_tetrad_arm_order(raw)
        self.assertEqual(res, ["arm_a_native", "arm_d_cand4_adaptive", "arm_c_cand4_always_on", "arm_b_cand3_always_on"])

    def test_t02_valid_order_bdac(self):
        """T02: Accept exact valid order B,D,A,C."""
        raw = "arm_b_cand3_always_on,arm_d_cand4_adaptive,arm_a_native,arm_c_cand4_always_on"
        res = Orchestrator.validate_tetrad_arm_order(raw)
        self.assertEqual(res, ["arm_b_cand3_always_on", "arm_d_cand4_adaptive", "arm_a_native", "arm_c_cand4_always_on"])

    def test_t03_valid_order_dcba(self):
        """T03: Accept exact valid order D,C,B,A."""
        raw = "arm_d_cand4_adaptive,arm_c_cand4_always_on,arm_b_cand3_always_on,arm_a_native"
        res = Orchestrator.validate_tetrad_arm_order(raw)
        self.assertEqual(res, ["arm_d_cand4_adaptive", "arm_c_cand4_always_on", "arm_b_cand3_always_on", "arm_a_native"])

    def test_t04_reject_duplicate_arm(self):
        """T04: Reject duplicate arm A,A,C,D."""
        raw = "arm_a_native,arm_a_native,arm_c_cand4_always_on,arm_d_cand4_adaptive"
        with self.assertRaises(SafetyViolationError) as ctx:
            Orchestrator.validate_tetrad_arm_order(raw)
        self.assertIn("duplicate", str(ctx.exception).lower())

    def test_t05_reject_missing_arm(self):
        """T05: Reject missing arm A,D,C (only 3 arms)."""
        raw = "arm_a_native,arm_d_cand4_adaptive,arm_c_cand4_always_on"
        with self.assertRaises(SafetyViolationError) as ctx:
            Orchestrator.validate_tetrad_arm_order(raw)
        self.assertIn("expected exactly 4 arms", str(ctx.exception).lower())

    def test_t06_reject_extra_arm(self):
        """T06: Reject 5 arms A,D,C,B,X."""
        raw = "arm_a_native,arm_d_cand4_adaptive,arm_c_cand4_always_on,arm_b_cand3_always_on,arm_x"
        with self.assertRaises(SafetyViolationError) as ctx:
            Orchestrator.validate_tetrad_arm_order(raw)
        self.assertIn("expected exactly 4 arms", str(ctx.exception).lower())

    def test_t07_reject_unknown_arm(self):
        """T07: Reject unknown arm A,D,C,X (4 arms but one unknown)."""
        raw = "arm_a_native,arm_d_cand4_adaptive,arm_c_cand4_always_on,arm_x"
        with self.assertRaises(SafetyViolationError) as ctx:
            Orchestrator.validate_tetrad_arm_order(raw)
        self.assertIn("unregistered arm", str(ctx.exception).lower())

    def test_t08_reject_empty_string(self):
        """T08: Reject explicit empty order string."""
        with self.assertRaises(SafetyViolationError) as ctx:
            Orchestrator.validate_tetrad_arm_order("")
        self.assertIn("cannot be empty", str(ctx.exception).lower())

    def test_t09_omitted_order_preserves_default(self):
        """T09: Omitted order option (None) preserves seeded default ordering."""
        res = Orchestrator.validate_tetrad_arm_order(None, allow_none=True)
        self.assertIsNone(res)

    def test_t10_reject_empty_entry(self):
        """T10: Reject empty entry A,,C,D."""
        raw = "arm_a_native,,arm_c_cand4_always_on,arm_d_cand4_adaptive"
        with self.assertRaises(SafetyViolationError) as ctx:
            Orchestrator.validate_tetrad_arm_order(raw)
        self.assertIn("empty entry", str(ctx.exception).lower())

    def test_t11_reject_semicolon_separator(self):
        """T11: Reject semicolon separator."""
        raw = "arm_a_native;arm_d_cand4_adaptive;arm_c_cand4_always_on;arm_b_cand3_always_on"
        with self.assertRaises(SafetyViolationError) as ctx:
            Orchestrator.validate_tetrad_arm_order(raw)
        self.assertIn("unsupported separator", str(ctx.exception).lower())

    def test_t12_reject_space_separated_composite(self):
        """T12: Reject space-separated composite."""
        raw = "arm_a_native arm_d_cand4_adaptive arm_c_cand4_always_on arm_b_cand3_always_on"
        with self.assertRaises(SafetyViolationError) as ctx:
            Orchestrator.validate_tetrad_arm_order(raw)
        self.assertTrue("expected exactly 4 arms" in str(ctx.exception).lower() or "whitespace" in str(ctx.exception).lower())

    def test_t13_accept_whitespace_around_comma_separated_names(self):
        """T13: Accept surrounding whitespace around valid comma-separated names."""
        raw = "  arm_a_native ,  arm_d_cand4_adaptive , arm_c_cand4_always_on , arm_b_cand3_always_on  "
        res = Orchestrator.validate_tetrad_arm_order(raw)
        self.assertEqual(res, ["arm_a_native", "arm_d_cand4_adaptive", "arm_c_cand4_always_on", "arm_b_cand3_always_on"])

    def test_t14_reject_short_aliases(self):
        """T14: Reject short aliases A,D,C,B."""
        raw = "A,D,C,B"
        with self.assertRaises(SafetyViolationError) as ctx:
            Orchestrator.validate_tetrad_arm_order(raw)
        self.assertIn("unregistered arm", str(ctx.exception).lower())

    def test_t15_reject_case_variation(self):
        """T15: Reject uppercase / case variation."""
        raw = "ARM_A_NATIVE,ARM_D_CAND4_ADAPTIVE,ARM_C_CAND4_ALWAYS_ON,ARM_B_CAND3_ALWAYS_ON"
        with self.assertRaises(SafetyViolationError) as ctx:
            Orchestrator.validate_tetrad_arm_order(raw)
        self.assertIn("unregistered arm", str(ctx.exception).lower())

    def test_t16_reject_duplicate_in_final_position(self):
        """T16: Reject duplicate in final position A,D,C,A."""
        raw = "arm_a_native,arm_d_cand4_adaptive,arm_c_cand4_always_on,arm_a_native"
        with self.assertRaises(SafetyViolationError) as ctx:
            Orchestrator.validate_tetrad_arm_order(raw)
        self.assertIn("duplicate", str(ctx.exception).lower())

    def test_t17_reject_valid_prefix_with_invalid_suffix(self):
        """T17: Reject valid prefix followed by malformed suffix."""
        raw = "arm_a_native,arm_d_cand4_adaptive,arm_c_cand4_always_on,invalid_suffix!"
        with self.assertRaises(SafetyViolationError) as ctx:
            Orchestrator.validate_tetrad_arm_order(raw)
        self.assertIn("unregistered arm", str(ctx.exception).lower())

    def test_t18_direct_run_tetrad_rejection_zero_side_effects(self):
        """T18: Direct run_tetrad invocation with malformed input rejects before any side effect."""
        orch = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="t18_test",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
        )

        with patch.object(orch, "_execute_single_arm") as mock_exec:
            # 1. Test missing arm
            with self.assertRaises(SafetyViolationError):
                orch.run_tetrad(arm_order=["arm_a_native", "arm_d_cand4_adaptive", "arm_c_cand4_always_on"])
            mock_exec.assert_not_called()

            # 2. Test duplicate arm
            with self.assertRaises(SafetyViolationError):
                orch.run_tetrad(arm_order=["arm_a_native", "arm_a_native", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"])
            mock_exec.assert_not_called()

            # 3. Test unknown arm
            with self.assertRaises(SafetyViolationError):
                orch.run_tetrad(arm_order=["arm_a_native", "arm_d_cand4_adaptive", "arm_c_cand4_always_on", "unknown_arm"])
            mock_exec.assert_not_called()

            # 4. Test empty list
            with self.assertRaises(SafetyViolationError):
                orch.run_tetrad(arm_order=[])
            mock_exec.assert_not_called()

    def test_valid_permutations_preserve_identities(self):
        """Verifies that valid orders A->D->C->B and B->D->A->C preserve identities and execution order."""
        orch = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="perm_test",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
        )

        executed_order_1 = []
        def mock_exec_1(arm, exec_order, arm_max_tokens_ceiling=None):
            executed_order_1.append(arm)
            return {
                "schema_version": "2.0.0",
                "run_id": f"test_{arm}",
                "pair_id": "perm_test",
                "task_id": "task_a_typo",
                "arm": arm,
                "execution_order_in_pair": exec_order,
                "execution_status": "SUCCESS",
                "error_message": None,
                "telemetry_valid": True,
                "telemetry": {"total_model_tokens": 1000, "prompt_tokens_total": 800, "completion_tokens": 200, "telemetry_valid": True},
                "correctness": {"score": 1.0, "passed_all_requirements": True},
                "headroom_guard": {"configured_ceiling": 250000}
            }

        order_1 = ["arm_a_native", "arm_d_cand4_adaptive", "arm_c_cand4_always_on", "arm_b_cand3_always_on"]
        with patch.object(orch, "_execute_single_arm", side_effect=mock_exec_1):
            res_1 = orch.run_tetrad(block_id="b1", arm_order=order_1)
        self.assertEqual(executed_order_1, order_1)
        self.assertEqual(res_1["order"], order_1)

        executed_order_2 = []
        def mock_exec_2(arm, exec_order, arm_max_tokens_ceiling=None):
            executed_order_2.append(arm)
            return mock_exec_1(arm, exec_order, arm_max_tokens_ceiling)

        order_2 = ["arm_b_cand3_always_on", "arm_d_cand4_adaptive", "arm_a_native", "arm_c_cand4_always_on"]
        with patch.object(orch, "_execute_single_arm", side_effect=mock_exec_2):
            res_2 = orch.run_tetrad(block_id="b2", arm_order=order_2)
        self.assertEqual(executed_order_2, order_2)
        self.assertEqual(res_2["order"], order_2)


class TestGlobalBudgetOvershootRemediation(unittest.TestCase):
    """Verifies global budget enforcement and Scenario 13 overshoot remediation (Workstream B)."""

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.output_dir = Path(self.temp_dir.name) / "budget_out"
        self.task_cfg = HARNESS_ROOT / "config" / "tasks" / "task_a_typo.json"

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_scenario_13_deterministic_reproduction(self):
        """Scenario 13 reproduction: Runner/mock consuming 120,000 against 100,000 ceiling can NEVER be accepted."""
        orch = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="scen_13",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            global_tokens_ceiling=200_000,
            headroom_reserve=45_000,
        )

        # 1. Direct acceptance check: an arm with 120,000 tokens against a 100,000 ceiling is rejected
        run_record_over_budget = {
            "execution_status": "SUCCESS",
            "telemetry_valid": True,
            "telemetry": {
                "total_model_tokens": 120_000,
                "prompt_tokens_total": 100_000,
                "completion_tokens": 20_000,
                "telemetry_valid": True
            },
            "correctness": {
                "score": 1.0,
                "passed_all_requirements": True
            },
            "headroom_guard": {
                "configured_ceiling": 100_000
            }
        }
        self.assertFalse(
            Orchestrator.is_run_accepted(run_record_over_budget),
            "is_run_accepted MUST return False when total_model_tokens > configured_ceiling"
        )

        # 2. Tetrad orchestration check: an over-ceiling SUCCESS raises SafetyViolationError
        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            return run_record_over_budget

        with patch.object(orch, "_execute_single_arm", side_effect=mock_exec):
            summary = orch.run_tetrad(
                block_id="b_scen_13",
                arm_order=["arm_a_native", "arm_d_cand4_adaptive", "arm_c_cand4_always_on", "arm_b_cand3_always_on"]
            )

        self.assertEqual(summary["block_status"], "ABORTED_SAFETY_FAILURE")
        # Failing arm is marked ABORTED_SAFETY_FAILURE
        self.assertEqual(summary["runs"]["arm_d_cand4_adaptive"]["execution_status"], "ABORTED_SAFETY_FAILURE")
        # Trailing arms must not be executed and marked SKIPPED_SAFETY_FAILURE
        for rem_arm in ["arm_c_cand4_always_on", "arm_b_cand3_always_on"]:
            self.assertEqual(summary["runs"][rem_arm]["execution_status"], "SKIPPED_SAFETY_FAILURE")

    def test_global_ceiling_breach_halts_immediately(self):
        """When cumulative tokens reach or exceed the global ceiling, no further arms are dispatched."""
        orch = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="global_halt",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            global_tokens_ceiling=150_000,
            headroom_reserve=45_000,
            execution_policy="diagnostic_continue"  # Diagnostic mode must NOT bypass global ceiling
        )

        executed_arms = []
        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            executed_arms.append(arm)
            # Arm 1 spends 150,000, exhausting entire global budget
            return {
                "execution_status": "SUCCESS",
                "telemetry_valid": True,
                "telemetry": {
                    "total_model_tokens": 150_000,
                    "prompt_tokens_total": 120_000,
                    "completion_tokens": 30_000,
                    "telemetry_valid": True
                },
                "correctness": {"score": 1.0, "passed_all_requirements": True},
                "headroom_guard": {"configured_ceiling": 150_000}
            }

        with patch.object(orch, "_execute_single_arm", side_effect=mock_exec):
            summary = orch.run_tetrad(
                block_id="b_global_halt",
                arm_order=["arm_a_native", "arm_d_cand4_adaptive", "arm_c_cand4_always_on", "arm_b_cand3_always_on"]
            )

        self.assertEqual(executed_arms, ["arm_a_native"], "No subsequent arms may dispatch once global ceiling is exhausted")
        self.assertEqual(summary["block_status"], "ABORTED_GLOBAL_BUDGET")
        for rem_arm in ["arm_d_cand4_adaptive", "arm_c_cand4_always_on", "arm_b_cand3_always_on"]:
            self.assertEqual(summary["runs"][rem_arm]["execution_status"], "SKIPPED_GLOBAL_BUDGET")

    def test_streaming_tracker_headroom_boundary(self):
        """Streaming token tracker preventive headroom check rejects dispatch when remaining < estimated."""
        tracker = StreamingTokenTracker(conservative_reserve=45_000)

        # Ceiling = 100,000, usage = 60,000 -> remaining = 40,000 (< 45,000 reserve)
        tracker.thread_cumulative_max = 60_000
        has_hr, rem, est = tracker.check_pre_dispatch_headroom(100_000)
        self.assertFalse(has_hr)
        self.assertEqual(rem, 40_000)
        self.assertEqual(est, 45_000)
        self.assertEqual(tracker.last_decision, "PREVENTIVE_HALT")

        # Ceiling = 100,000, usage = 55,000 -> remaining = 45,000 (== 45,000 reserve) -> allowed
        tracker.thread_cumulative_max = 55_000
        has_hr, rem, est = tracker.check_pre_dispatch_headroom(100_000)
        self.assertTrue(has_hr)
        self.assertEqual(rem, 45_000)
        self.assertEqual(tracker.last_decision, "DISPATCH_ALLOWED")

    def test_streaming_tracker_deduplication(self):
        """Streaming token tracker deduplicates repeated snapshot lines without inflating token count."""
        tracker = StreamingTokenTracker(conservative_reserve=45_000)
        snapshot_line = json.dumps({
            "type": "token_usage_record",
            "payload": {
                "thread_token_usage": {
                    "total_tokens": 50_000,
                    "input_tokens": 40_000,
                    "output_tokens": 10_000
                }
            }
        })

        tracker.process_line(snapshot_line)
        self.assertEqual(tracker.effective_tokens, 50_000)
        self.assertEqual(tracker.turns_completed, 1)

        # Process exact same snapshot line again
        tracker.process_line(snapshot_line)
        self.assertEqual(tracker.effective_tokens, 50_000)
        self.assertEqual(tracker.turns_completed, 1, "Duplicate snapshot must not increment turns_completed")


if __name__ == "__main__":
    unittest.main()
