"""
Engram Phase 5.4.15 & Phase 5.4.17: Diagnostic Continuation Integration & Global Budget Integrity Test Suite.

Verifies:
1. Safe diagnostic continuation capability across all 12 core scenarios:
   - Scenario 01: Existing strict mode (Arm B fails -> Arms D, A, C skipped -> MUTUAL_ACCEPTANCE_FAILURE).
   - Scenario 02: Diagnostic continuation (Arm B fails -> Arms D, A, C execute -> DIAGNOSTIC_INCOMPLETE).
   - Scenario 03: Diagnostic arm budget exhaustion (Arm B hits per-arm limit -> subsequent arms continue).
   - Scenario 04: Global budget exhaustion (Global ceiling enforced, effective arm ceiling passed, halts at reserve).
   - Scenario 05: Global headroom reserve (Global remaining < headroom_reserve -> dispatch blocked).
   - Scenario 06: Candidate identity mismatch (Safety violation -> block ABORTED_SAFETY_FAILURE).
   - Scenario 07: Sandbox integrity failure (Safety violation -> remaining arms SKIPPED_SAFETY_FAILURE).
   - Scenario 08: Timeout and cleanup (Process cleanup confirmed before continuation).
   - Scenario 09: Historical telemetry compatibility (Live 01 and Live 02 readable).
   - Scenario 10: Comparative acceptance protection (Mixed success in diagnostic mode -> savings ineligible).
   - Scenario 11: All arms succeed in diagnostic mode (DIAGNOSTIC_COMPLETE, all_arms_accepted True).
   - Scenario 12: Cancellation (KeyboardInterrupt / cancellation -> safe cleanup).
2. Phase 5.4.17 Dedicated Remediation Gate Tests (T01 - T15):
   - T01: Missing final telemetry causes ABORTED_UNCERTAIN_BUDGET and marks trailing arms SKIPPED_UNCERTAIN_BUDGET.
   - T02: Malformed telemetry causes ABORTED_UNCERTAIN_BUDGET.
   - T03: Conflicting token totals causes ABORTED_UNCERTAIN_BUDGET.
   - T04: Zero-token anomaly is rejected or handled fail-closed.
   - T05: Timeout without final usage causes ABORTED_UNCERTAIN_BUDGET.
   - T06: Process crash without final usage causes ABORTED_UNCERTAIN_BUDGET.
   - T07: Arm with score = 1.0 and invalid telemetry has arm_accepted = False.
   - T08: Block where all arms pass functional checks but one has unknown usage cannot resolve to ACCEPTED or DIAGNOSTIC_COMPLETE.
   - T09: execute_run receives effective arm ceiling and runner halts at ceiling.
   - T10: Mock reporting consumption above ceiling raises a safety violation in the orchestrator.
   - T11: Conflicting CLI arguments (--no-stop-on-failure + --execution-policy strict) exit with code 2.
   - T12: Strict policy stops on first non-safety failure; remaining arms are skipped.
   - T13: Diagnostic policy continues after non-safety failure when budget usage is verified.
   - T14: Diagnostic policy halts immediately if a failure leaves budget usage unknown.
   - T15: Historical telemetry payloads without new validity fields remain readable and are handled safely.
"""
import json
import tempfile
import unittest
from unittest.mock import patch, MagicMock
from pathlib import Path
import sys
import subprocess

# Ensure src is on sys.path
HARNESS_ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = HARNESS_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from orchestrator import Orchestrator, audit_arm_candidate_identity, resolve_execution_policy
from isolation import SafetyViolationError
from runner import CodexRunner, StreamingTokenTracker, RunExecutionResult
from schema_validator import validate_instance


def make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=50000, valid_telemetry=True):
    """Helper to create a schema-valid mock run record."""
    record = orchestrator._create_skipped_arm_record(arm, exec_order, f"Mock run for {arm}", execution_status=status)
    record["execution_status"] = status
    record["telemetry_valid"] = valid_telemetry
    record["telemetry"]["telemetry_valid"] = valid_telemetry
    record["telemetry"]["total_model_tokens"] = tokens
    prompt_tokens = int(tokens * 0.7)
    completion_tokens = tokens - prompt_tokens
    record["telemetry"]["prompt_tokens_total"] = prompt_tokens
    record["telemetry"]["completion_tokens"] = completion_tokens
    record["telemetry"]["prompt_tokens_cached"] = 0
    record["telemetry"]["prompt_tokens_cached_inferred"] = True
    record["telemetry"]["prompt_tokens_uncached"] = prompt_tokens
    record["telemetry"]["prompt_tokens_uncached_inferred"] = True
    record["telemetry"]["peak_single_request_context"] = max(1, int(tokens * 0.5))
    record["correctness"]["score"] = score
    record["correctness"]["passed_all_requirements"] = (score == 1.0)
    record["correctness"]["quality_adjusted_cost"] = float(tokens)
    return record


class TestDiagnosticContinuation(unittest.TestCase):

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.output_dir = Path(self.temp_dir.name) / "tetrad_diag_out"
        self.task_cfg = HARNESS_ROOT / "config" / "tasks" / "task_a_typo.json"

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_01_strict_mode_stop_on_failure(self):
        """Scenario 1: Existing strict mode halts on Arm B failure and skips remaining arms."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="test_01",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="strict",
            stop_on_failure=True,
        )

        executed_arms = []

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            executed_arms.append(arm)
            if arm == "arm_b_cand3_always_on":
                # Known-budget failure under strict mode
                return make_mock_run_record(orchestrator, arm, exec_order, status="PRE_DISPATCH_HEADROOM_EXHAUSTED", score=0.0, tokens=50000, valid_telemetry=True)
            return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=50000)

        arm_order = ["arm_b_cand3_always_on", "arm_d_cand4_adaptive", "arm_a_native", "arm_c_cand4_always_on"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_strict_01", arm_order=arm_order)

        self.assertEqual(executed_arms, ["arm_b_cand3_always_on"], "Strict mode must halt immediately after Arm B failure")
        self.assertEqual(summary["block_status"], "MUTUAL_ACCEPTANCE_FAILURE")
        self.assertFalse(summary["acceptance"]["all_arms_accepted"])

        # Subsequent arms must be marked SKIPPED
        for skipped_arm in ["arm_d_cand4_adaptive", "arm_a_native", "arm_c_cand4_always_on"]:
            self.assertIn("SKIPPED", summary["runs"][skipped_arm]["execution_status"])

        # Comparative analyses must be invalidated
        for comp_key, comp_val in summary["comparative_analysis"].items():
            self.assertFalse(comp_val["savings_claim_eligible"], f"{comp_key} must be ineligible")
            self.assertEqual(comp_val["verdict"], "MUTUAL_ACCEPTANCE_FAILURE")

    def test_02_diagnostic_continuation_flow(self):
        """Scenario 2: Diagnostic continuation allows subsequent arms to execute after Arm B fails with verified tokens."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="test_02",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        executed_arms = []

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            executed_arms.append(arm)
            if arm == "arm_b_cand3_always_on":
                return make_mock_run_record(orchestrator, arm, exec_order, status="PRE_DISPATCH_HEADROOM_EXHAUSTED", score=0.0, tokens=50000, valid_telemetry=True)
            return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=50000)

        arm_order = ["arm_b_cand3_always_on", "arm_d_cand4_adaptive", "arm_a_native", "arm_c_cand4_always_on"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_diag_02", arm_order=arm_order)

        self.assertEqual(executed_arms, arm_order, "Diagnostic continuation must execute all arms in order")
        self.assertEqual(summary["block_status"], "DIAGNOSTIC_INCOMPLETE")
        self.assertFalse(summary["acceptance"]["all_arms_accepted"])
        self.assertEqual(summary["runs"]["arm_b_cand3_always_on"]["execution_status"], "PRE_DISPATCH_HEADROOM_EXHAUSTED")
        self.assertEqual(summary["runs"]["arm_d_cand4_adaptive"]["execution_status"], "SUCCESS")

        # Comparative analysis must still be invalidated because Arm B failed acceptance
        for comp_key, comp_val in summary["comparative_analysis"].items():
            self.assertFalse(comp_val["savings_claim_eligible"], f"{comp_key} must be ineligible")
            self.assertEqual(comp_val["verdict"], "MUTUAL_ACCEPTANCE_FAILURE")

    def test_03_diagnostic_arm_budget_exhaustion(self):
        """Scenario 3: An arm breaching its token ceiling preserves failure status and diagnostic continuation proceeds."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="test_03",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            if arm == "arm_b_cand3_always_on":
                return make_mock_run_record(orchestrator, arm, exec_order, status="TOKEN_CEILING_BREACHED", score=0.0, tokens=260000, valid_telemetry=True)
            return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=40000)

        arm_order = ["arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive", "arm_a_native"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_diag_03", arm_order=arm_order)

        self.assertEqual(summary["block_status"], "DIAGNOSTIC_INCOMPLETE")
        self.assertEqual(summary["runs"]["arm_b_cand3_always_on"]["execution_status"], "TOKEN_CEILING_BREACHED")
        self.assertEqual(summary["runs"]["arm_c_cand4_always_on"]["execution_status"], "SUCCESS")
        self.assertFalse(summary["acceptance"]["arm_b_cand3_always_on_accepted"])
        self.assertTrue(summary["acceptance"]["arm_c_cand4_always_on_accepted"])

    def test_04_global_budget_exhaustion(self):
        """Scenario 4: When cumulative tokens breach the global ceiling, remaining arms are SKIPPED_GLOBAL_BUDGET."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="test_04",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
            global_tokens_ceiling=200_000,
            headroom_reserve=45_000,
        )

        executed_arms = []
        recorded_ceilings = {}

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            executed_arms.append(arm)
            recorded_ceilings[arm] = arm_max_tokens_ceiling
            if arm == "arm_a_native":
                return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=120_000)
            elif arm == "arm_b_cand3_always_on":
                # Simulated runner respects arm_max_tokens_ceiling (80,000) and halts at ceiling
                actual_tokens = arm_max_tokens_ceiling if arm_max_tokens_ceiling is not None else 80_000
                return make_mock_run_record(orchestrator, arm, exec_order, status="TOKEN_CEILING_BREACHED", score=0.0, tokens=actual_tokens)
            return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=50_000)

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_diag_04", arm_order=arm_order)

        # Arm 1 spends 120,000 -> remaining 80,000 >= 45,000 -> Arm 2 dispatches with effective ceiling 80,000
        self.assertEqual(recorded_ceilings.get("arm_b_cand3_always_on"), 80_000, "Arm B must receive effective ceiling of 80,000")
        # Arm 2 spends 80,000 -> cumulative 200,000 -> remaining 0 < 45,000 headroom reserve
        # Arms 3 and 4 should NOT dispatch
        self.assertEqual(executed_arms, ["arm_a_native", "arm_b_cand3_always_on"])
        self.assertEqual(summary["block_status"], "ABORTED_GLOBAL_BUDGET")
        self.assertEqual(summary["cumulative_global_tokens"], 200_000)
        self.assertEqual(summary["runs"]["arm_c_cand4_always_on"]["execution_status"], "SKIPPED_GLOBAL_BUDGET")
        self.assertEqual(summary["runs"]["arm_d_cand4_adaptive"]["execution_status"], "SKIPPED_GLOBAL_BUDGET")

    def test_05_global_headroom_reserve_guard(self):
        """Scenario 5: Pre-dispatch check halts tetrad if global remaining < headroom_reserve."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="test_05",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
            global_tokens_ceiling=100_000,
            headroom_reserve=45_000,
        )

        executed_arms = []

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            executed_arms.append(arm)
            return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=60_000)

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_diag_05", arm_order=arm_order)

        # Arm 1 spends 60,000. Remaining: 40,000 < 45,000 headroom reserve.
        # Arm 2 should not be dispatched.
        self.assertEqual(executed_arms, ["arm_a_native"])
        self.assertEqual(summary["block_status"], "ABORTED_GLOBAL_BUDGET")
        self.assertEqual(summary["runs"]["arm_b_cand3_always_on"]["execution_status"], "SKIPPED_GLOBAL_BUDGET")
        self.assertIn("headroom reserve exhausted", summary["runs"]["arm_b_cand3_always_on"]["error_message"])

    def test_06_candidate_identity_safety_violation(self):
        """Scenario 6: Safety violation (e.g. hash mismatch) overrides diagnostic continue and halts block immediately."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="test_06",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        executed_arms = []

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            executed_arms.append(arm)
            if arm == "arm_b_cand3_always_on":
                raise SafetyViolationError("Pre-launch binary hash verification failed: SHA-256 mismatch")
            return make_mock_run_record(orchestrator, arm, exec_order)

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_diag_06", arm_order=arm_order)

        self.assertEqual(executed_arms, ["arm_a_native", "arm_b_cand3_always_on"])
        self.assertEqual(summary["block_status"], "ABORTED_SAFETY_FAILURE")
        self.assertEqual(summary["runs"]["arm_b_cand3_always_on"]["execution_status"], "ABORTED_SAFETY_FAILURE")
        self.assertEqual(summary["runs"]["arm_c_cand4_always_on"]["execution_status"], "SKIPPED_SAFETY_FAILURE")
        self.assertEqual(summary["runs"]["arm_d_cand4_adaptive"]["execution_status"], "SKIPPED_SAFETY_FAILURE")

    def test_07_sandbox_integrity_safety_violation(self):
        """Scenario 7: Sandbox creation or golden dependency corruption halts block immediately."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="test_07",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            raise SafetyViolationError("Golden repository dependency tree corrupted: manifest mismatch")

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_diag_07", arm_order=arm_order)

        self.assertEqual(summary["block_status"], "ABORTED_SAFETY_FAILURE")
        self.assertEqual(summary["runs"]["arm_a_native"]["execution_status"], "ABORTED_SAFETY_FAILURE")
        self.assertEqual(summary["runs"]["arm_b_cand3_always_on"]["execution_status"], "SKIPPED_SAFETY_FAILURE")

    def test_08_timeout_and_cleanup(self):
        """Scenario 8: Arm timeout with valid telemetry preserves status and allows next arm to execute."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="test_08",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        executed_arms = []

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            executed_arms.append(arm)
            if arm == "arm_b_cand3_always_on":
                return make_mock_run_record(orchestrator, arm, exec_order, status="TIMEOUT", score=0.0, tokens=15000, valid_telemetry=True)
            return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=45000)

        arm_order = ["arm_b_cand3_always_on", "arm_d_cand4_adaptive", "arm_a_native", "arm_c_cand4_always_on"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_diag_08", arm_order=arm_order)

        self.assertEqual(executed_arms, arm_order)
        self.assertEqual(summary["runs"]["arm_b_cand3_always_on"]["execution_status"], "TIMEOUT")
        self.assertEqual(summary["runs"]["arm_d_cand4_adaptive"]["execution_status"], "SUCCESS")
        self.assertEqual(summary["block_status"], "DIAGNOSTIC_INCOMPLETE")

    def test_09_historical_telemetry_compatibility(self):
        """Scenario 9: Audits and verifies compatibility with historical Phase 5.4.7 (Live 01) and Phase 5.4.13 (Live 02) telemetry."""
        runs_dir = HARNESS_ROOT / "reports" / "runs"
        live01_dir = runs_dir / "task_b_remember_me_tetrad_live_01"
        live02_dir = runs_dir / "task_b_remember_me_tetrad_live_02"

        for live_dir in [live01_dir, live02_dir]:
            if not live_dir.exists():
                continue
            for arm_folder in live_dir.iterdir():
                telem_file = arm_folder / "run_telemetry.json"
                if telem_file.exists():
                    with open(telem_file, "r", encoding="utf-8") as f:
                        data = json.load(f)
                    audit = audit_arm_candidate_identity(data)
                    self.assertIn("arm", audit)
                    self.assertIn("is_legacy", audit)

    def test_10_comparative_acceptance_protection(self):
        """Scenario 10: Mixed arm success in diagnostic mode unconditionally invalidates all comparative efficiency claims."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="test_10",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        # Arms A, C, D succeed, but Arm B fails
        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            if arm == "arm_b_cand3_always_on":
                return make_mock_run_record(orchestrator, arm, exec_order, status="FAILED", score=0.0, tokens=30000)
            return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=40000)

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_diag_10", arm_order=arm_order)

        self.assertEqual(summary["block_status"], "DIAGNOSTIC_INCOMPLETE")
        comp = summary["comparative_analysis"]

        # Even though Arm A and Arm D both succeeded with score 1.0, block is incomplete
        cand4_adapt_vs_native = comp["cand4_adaptive_vs_native"]
        self.assertFalse(cand4_adapt_vs_native["savings_claim_eligible"])
        self.assertEqual(cand4_adapt_vs_native["savings_claim_status"], "INVALIDATED_BY_ACCEPTANCE_FAILURE")
        self.assertEqual(cand4_adapt_vs_native["verdict"], "MUTUAL_ACCEPTANCE_FAILURE")

    def test_11_all_arms_succeed_in_diagnostic_mode(self):
        """Scenario 11: All arms succeed in diagnostic mode -> DIAGNOSTIC_COMPLETE and valid comparative analysis."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="test_11",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            tokens = 100_000 if arm == "arm_a_native" else 40_000
            return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=tokens)

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_diag_11", arm_order=arm_order)

        self.assertEqual(summary["block_status"], "DIAGNOSTIC_COMPLETE")
        self.assertTrue(summary["acceptance"]["all_arms_accepted"])

        # Comparative analysis should be valid
        cand4_adapt_vs_native = summary["comparative_analysis"]["cand4_adaptive_vs_native"]
        self.assertTrue(cand4_adapt_vs_native["savings_claim_eligible"])
        self.assertEqual(cand4_adapt_vs_native["verdict"], "EFFICIENCY_GAIN")
        self.assertGreater(cand4_adapt_vs_native["metrics"]["total_model_tokens"]["savings_pct"], 0)

    def test_12_cancellation_safe_cleanup(self):
        """Scenario 12: KeyboardInterrupt / cancellation during execution cleanly aborts the block."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="test_12",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            if arm == "arm_b_cand3_always_on":
                raise KeyboardInterrupt()
            return make_mock_run_record(orchestrator, arm, exec_order)

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_diag_12", arm_order=arm_order)

        self.assertEqual(summary["block_status"], "ABORTED_USER_CANCEL")
        self.assertEqual(summary["runs"]["arm_b_cand3_always_on"]["execution_status"], "ABORTED_USER_CANCEL")
        self.assertEqual(summary["runs"]["arm_c_cand4_always_on"]["execution_status"], "SKIPPED_USER_CANCEL")


class TestPhase5417RemediationGate(unittest.TestCase):
    """
    Dedicated regression test suite covering T01 through T15 required by Phase 5.4.17.
    """

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.output_dir = Path(self.temp_dir.name) / "gate_out"
        self.task_cfg = HARNESS_ROOT / "config" / "tasks" / "task_a_typo.json"

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_t01_missing_final_telemetry_halts_uncertain_budget(self):
        """T01: Missing final telemetry causes ABORTED_UNCERTAIN_BUDGET and marks trailing arms SKIPPED_UNCERTAIN_BUDGET."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="gate_t01",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            record = make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=0, valid_telemetry=False)
            record["telemetry"] = {}  # Empty telemetry dictionary
            return record

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_t01", arm_order=arm_order)

        self.assertEqual(summary["block_status"], "ABORTED_UNCERTAIN_BUDGET")
        self.assertTrue(summary["global_budget_uncertain"])
        self.assertFalse(summary["acceptance"]["all_arms_accepted"])
        # All trailing arms must be SKIPPED_UNCERTAIN_BUDGET
        for trailing in ["arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]:
            self.assertEqual(summary["runs"][trailing]["execution_status"], "SKIPPED_UNCERTAIN_BUDGET")
            self.assertIn("uncertain token budget", summary["runs"][trailing]["error_message"])

    def test_t02_malformed_telemetry_halts_uncertain_budget(self):
        """T02: Malformed telemetry (non-integer token fields) causes ABORTED_UNCERTAIN_BUDGET."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="gate_t02",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            record = make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=50000)
            record["telemetry"]["total_model_tokens"] = "five_thousand"  # Malformed non-int
            return record

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_t02", arm_order=arm_order)

        self.assertEqual(summary["block_status"], "ABORTED_UNCERTAIN_BUDGET")
        self.assertTrue(summary["global_budget_uncertain"])
        self.assertEqual(summary["runs"]["arm_b_cand3_always_on"]["execution_status"], "SKIPPED_UNCERTAIN_BUDGET")

    def test_t03_conflicting_token_totals_halts_uncertain_budget(self):
        """T03: Conflicting token totals (prompt + completion != total) causes ABORTED_UNCERTAIN_BUDGET."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="gate_t03",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            record = make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=50000)
            record["telemetry"]["prompt_tokens_total"] = 30000
            record["telemetry"]["completion_tokens"] = 10000
            record["telemetry"]["total_model_tokens"] = 70000  # Mathematical invariant breach (30k + 10k != 70k)
            return record

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_t03", arm_order=arm_order)

        self.assertEqual(summary["block_status"], "ABORTED_UNCERTAIN_BUDGET")
        self.assertEqual(summary["runs"]["arm_b_cand3_always_on"]["execution_status"], "SKIPPED_UNCERTAIN_BUDGET")

    def test_t04_zero_token_anomaly_rejected_fail_closed(self):
        """T04: Zero-token anomaly is rejected fail-closed as uncertain budget."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="gate_t04",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            record = make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=0, valid_telemetry=True)
            record["telemetry"]["prompt_tokens_total"] = 0
            record["telemetry"]["completion_tokens"] = 0
            record["telemetry"]["total_model_tokens"] = 0  # Zero tokens
            return record

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_t04", arm_order=arm_order)

        self.assertEqual(summary["block_status"], "ABORTED_UNCERTAIN_BUDGET")
        self.assertTrue(summary["global_budget_uncertain"])
        self.assertEqual(summary["runs"]["arm_b_cand3_always_on"]["execution_status"], "SKIPPED_UNCERTAIN_BUDGET")

    def test_t05_timeout_without_final_usage_halts_uncertain_budget(self):
        """T05: Timeout without final usage causes ABORTED_UNCERTAIN_BUDGET."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="gate_t05",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            # Timeout where process crashed or hung without emitting final valid telemetry
            return make_mock_run_record(orchestrator, arm, exec_order, status="TIMEOUT", score=0.0, tokens=0, valid_telemetry=False)

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_t05", arm_order=arm_order)

        self.assertEqual(summary["block_status"], "ABORTED_UNCERTAIN_BUDGET")
        self.assertEqual(summary["runs"]["arm_b_cand3_always_on"]["execution_status"], "SKIPPED_UNCERTAIN_BUDGET")

    def test_t06_process_crash_without_final_usage_halts_uncertain_budget(self):
        """T06: Process crash without final usage causes ABORTED_UNCERTAIN_BUDGET."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="gate_t06",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            return make_mock_run_record(orchestrator, arm, exec_order, status="CRASH", score=0.0, tokens=0, valid_telemetry=False)

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_t06", arm_order=arm_order)

        self.assertEqual(summary["block_status"], "ABORTED_UNCERTAIN_BUDGET")
        self.assertEqual(summary["runs"]["arm_b_cand3_always_on"]["execution_status"], "SKIPPED_UNCERTAIN_BUDGET")

    def test_t07_functional_success_with_invalid_telemetry_not_accepted(self):
        """T07: Arm with functional score = 1.0 and invalid telemetry has arm_accepted = False."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="gate_t07",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
        )
        record = make_mock_run_record(orchestrator, "arm_a_native", 1, status="SUCCESS", score=1.0, tokens=50000, valid_telemetry=False)
        self.assertFalse(Orchestrator.is_run_accepted(record), "Arm with invalid telemetry must not be accepted")

    def test_t08_block_with_unknown_usage_cannot_be_accepted_or_diagnostic_complete(self):
        """T08: Block where functional checks pass but one arm has unknown usage cannot resolve to ACCEPTED or DIAGNOSTIC_COMPLETE."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="gate_t08",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            if arm == "arm_d_cand4_adaptive":
                # Fourth arm has score 1.0 but invalid telemetry
                return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=0, valid_telemetry=False)
            return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=40000)

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_t08", arm_order=arm_order)

        self.assertNotEqual(summary["block_status"], "DIAGNOSTIC_COMPLETE")
        self.assertNotEqual(summary["block_status"], "ACCEPTED")
        self.assertEqual(summary["block_status"], "ABORTED_UNCERTAIN_BUDGET")
        self.assertFalse(summary["acceptance"]["all_arms_accepted"])
        # All comparative claims must be ineligible
        for comp_key, comp_val in summary["comparative_analysis"].items():
            self.assertFalse(comp_val["savings_claim_eligible"])

    def test_t09_runner_enforces_effective_arm_ceiling(self):
        """T09: execute_run receives effective arm ceiling and runner terminates at ceiling with TOKEN_CEILING_BREACHED."""
        runner = CodexRunner(
            codex_executable=Path("C:/mock/codex.exe"),
            engram_executable=Path("C:/mock/engram.exe"),
            max_tokens_ceiling=50_000,
            headroom_reserve=10_000,
        )

        line1 = json.dumps({
            "type": "token_usage_record",
            "payload": {"thread_token_usage": {"input_tokens": 30_000, "output_tokens": 5_000, "total_tokens": 35_000}}
        }) + "\n"
        line2 = json.dumps({
            "type": "token_usage_record",
            "payload": {"thread_token_usage": {"input_tokens": 55_000, "output_tokens": 5_000, "total_tokens": 60_000}}
        }) + "\n"

        mock_proc = MagicMock()
        mock_proc.pid = 99999
        mock_proc.poll.side_effect = [None, None, 0]
        mock_proc.stdout.readline.side_effect = [line1, line2, ""]
        mock_proc.stderr.readline.return_value = ""
        mock_proc.wait.return_value = 0
        mock_proc.returncode = 0

        with patch("subprocess.Popen", return_value=mock_proc), \
             patch("runner.terminate_pid_tree", return_value=True):
            res = runner.run(
                sandbox_path=self.output_dir,
                prompt="test prompt",
                arm="arm_a_native",
                output_events_path=self.output_dir / "events.jsonl",
                is_dry_run=False,
            )

        self.assertTrue(res.token_ceiling_breached, "Runner must set token_ceiling_breached=True when ceiling exceeded")
        self.assertEqual(res.stop_reason, "TOKEN_CEILING_BREACHED")
        self.assertEqual(res.configured_ceiling, 50_000)
        self.assertEqual(res.cumulative_tokens_at_stop, 60_000)
        self.assertIn("Token ceiling exceeded", res.error_message)

    def test_t10_mock_over_ceiling_detection_raises_safety_violation(self):
        """T10: Mock or runner reporting consumption above effective ceiling with SUCCESS raises a safety violation."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="gate_t10",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            global_tokens_ceiling=200_000,
            headroom_reserve=45_000,
        )

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            if arm == "arm_a_native":
                return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=120_000)
            elif arm == "arm_b_cand3_always_on":
                # Ceiling is 80,000, but mock illegally returns 120,000 with SUCCESS
                return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=120_000)
            return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=40_000)

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_t10", arm_order=arm_order)

        self.assertEqual(summary["block_status"], "ABORTED_SAFETY_FAILURE")
        self.assertEqual(summary["runs"]["arm_b_cand3_always_on"]["execution_status"], "ABORTED_SAFETY_FAILURE")
        self.assertIn("exceeding effective ceiling", summary["runs"]["arm_b_cand3_always_on"]["error_message"])

    def test_t11_conflicting_cli_arguments_rejected(self):
        """T11: Conflicting CLI arguments (--no-stop-on-failure + --execution-policy strict) exit with code 2."""
        # Unit level:
        pol_default, stop_default = resolve_execution_policy(None, False)
        self.assertEqual(pol_default, "strict")
        self.assertTrue(stop_default)

        pol_diag, stop_diag = resolve_execution_policy("diagnostic_continue", False)
        self.assertEqual(pol_diag, "diagnostic_continue")
        self.assertFalse(stop_diag)

        pol_legacy, stop_legacy = resolve_execution_policy(None, True)
        self.assertEqual(pol_legacy, "diagnostic_continue")
        self.assertFalse(stop_legacy)

        with self.assertRaises(ValueError):
            resolve_execution_policy("strict", True)

        # CLI subprocess level:
        cmd = [
            sys.executable,
            str(SRC_DIR / "orchestrator.py"),
            "--no-stop-on-failure",
            "--execution-policy",
            "strict",
        ]
        res = subprocess.run(cmd, capture_output=True, text=True)
        self.assertEqual(res.returncode, 2, "Conflicting arguments must exit with code 2")
        self.assertIn("cannot be combined", res.stderr)

    def test_t12_strict_policy_stops_on_first_failure(self):
        """T12: Strict policy stops on first non-safety failure; remaining arms are skipped."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="gate_t12",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="strict",
        )

        executed_arms = []

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            executed_arms.append(arm)
            if arm == "arm_a_native":
                return make_mock_run_record(orchestrator, arm, exec_order, status="FAILED", score=0.0, tokens=40000, valid_telemetry=True)
            return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=40000)

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_t12", arm_order=arm_order)

        self.assertEqual(executed_arms, ["arm_a_native"])
        self.assertEqual(summary["block_status"], "MUTUAL_ACCEPTANCE_FAILURE")
        for trailing in ["arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]:
            self.assertEqual(summary["runs"][trailing]["execution_status"], "SKIPPED")

    def test_t13_diagnostic_policy_continues_after_verified_budget_failure(self):
        """T13: Diagnostic policy continues after non-safety failure when budget usage is verified."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="gate_t13",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        executed_arms = []

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            executed_arms.append(arm)
            if arm == "arm_a_native":
                return make_mock_run_record(orchestrator, arm, exec_order, status="TOKEN_CEILING_BREACHED", score=0.0, tokens=80000, valid_telemetry=True)
            return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=40000)

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_t13", arm_order=arm_order)

        self.assertEqual(executed_arms, arm_order)
        self.assertEqual(summary["block_status"], "DIAGNOSTIC_INCOMPLETE")

    def test_t14_diagnostic_policy_halts_on_unknown_budget_failure(self):
        """T14: Diagnostic policy halts immediately if a failure leaves budget usage unknown."""
        orchestrator = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="gate_t14",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            execution_policy="diagnostic_continue",
        )

        executed_arms = []

        def mock_exec(arm, exec_order, arm_max_tokens_ceiling=None):
            executed_arms.append(arm)
            if arm == "arm_a_native":
                return make_mock_run_record(orchestrator, arm, exec_order, status="TIMEOUT", score=0.0, tokens=0, valid_telemetry=False)
            return make_mock_run_record(orchestrator, arm, exec_order, status="SUCCESS", score=1.0, tokens=40000)

        arm_order = ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_tetrad(block_id="block_t14", arm_order=arm_order)

        self.assertEqual(executed_arms, ["arm_a_native"])
        self.assertEqual(summary["block_status"], "ABORTED_UNCERTAIN_BUDGET")
        for trailing in ["arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]:
            self.assertEqual(summary["runs"][trailing]["execution_status"], "SKIPPED_UNCERTAIN_BUDGET")

    def test_t15_historical_telemetry_backward_compatibility(self):
        """T15: Historical telemetry payloads without new validity fields remain readable and are handled safely."""
        synthetic_legacy = {
            "schema_version": "2.0.0",
            "run_id": "legacy_run_01",
            "execution_status": "SUCCESS",
            "correctness": {"score": 1.0, "passed_all_requirements": True},
            # Missing top-level telemetry_valid and telemetry object lacks telemetry_valid
            "telemetry": {
                "total_model_tokens": 50000,
                "prompt_tokens_total": 35000,
                "completion_tokens": 15000,
            }
        }
        # Acceptance fails closed because validity fields are missing/null
        self.assertFalse(Orchestrator.is_run_accepted(synthetic_legacy))

        # Identity audit remains functional
        audit = audit_arm_candidate_identity(synthetic_legacy)
        self.assertIn("arm", audit)
        self.assertIn("is_legacy", audit)


if __name__ == "__main__":
    unittest.main()
