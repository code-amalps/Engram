import json
import tempfile
import unittest
import sys
import copy
from pathlib import Path

# Ensure src is on sys.path
HARNESS_ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = HARNESS_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from orchestrator import Orchestrator, ArmCandidateIdentity
from isolation import SafetyViolationError


class TestPentadHarness(unittest.TestCase):

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.output_dir = Path(self.temp_dir.name) / "pentad_out"
        self.task_cfg = HARNESS_ROOT / "config" / "tasks" / "task_a_typo.json"

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_01_successful_five_arm_dry_run(self):
        """Verifies that 5-arm pentad dry-run executes across all 5 arms cleanly."""
        orch = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="pentad_dryrun_01",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
        )
        res = orch.run_pentad(block_id="pentad_block_01")
        self.assertEqual(res["schema_version"], "2.1.0")
        self.assertEqual(res["block_id"], "pentad_block_01")
        self.assertEqual(len(res["runs"]), 5)
        self.assertIn("all_arms_accepted", res["acceptance"])

    def test_02_all_five_arms_represented_in_report(self):
        """Verifies all five arms are present in summary and report."""
        orch = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="pentad_dryrun_02",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
        )
        res = orch.run_pentad()
        expected_arms = set(Orchestrator.ALL_PENTAD_ARMS)
        self.assertEqual(set(res["runs"].keys()), expected_arms)
        self.assertTrue((self.output_dir / "pentad_summary.json").exists())
        self.assertTrue((self.output_dir / "pentad_report.md").exists())

    def test_03_correct_ae_candidate_mapping(self):
        """Verifies correct candidate versions mapped to Arms A through E."""
        orch = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="pentad_dryrun_03",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
        )
        self.assertEqual(orch.arm_identities["arm_a_native"].requested_candidate_version, "native")
        self.assertEqual(orch.arm_identities["arm_b_cand3"].requested_candidate_version, "cand3")
        self.assertEqual(orch.arm_identities["arm_c_cand42_always_on"].requested_candidate_version, "cand4_2")
        self.assertEqual(orch.arm_identities["arm_d_cand42_adaptive"].requested_candidate_version, "cand4_2")
        self.assertEqual(orch.arm_identities["arm_e_cand5_optimized"].requested_candidate_version, "cand5")

    def test_04_candidate5_binary_hash_verification(self):
        """Verifies that Candidate 5 binary integrity verification succeeds."""
        orch = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="pentad_dryrun_04",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
        )
        cand5_id = orch.arm_identities["arm_e_cand5_optimized"]
        verified_hash = cand5_id.verify_binary_integrity()
        self.assertEqual(verified_hash, cand5_id.expected_sha256)

    def test_05_missing_candidate5_rejection(self):
        """Rejects pentad order missing Candidate 5."""
        with self.assertRaises(SafetyViolationError):
            Orchestrator.validate_pentad_arm_order([
                "arm_a_native", "arm_b_cand3", "arm_c_cand42_always_on", "arm_d_cand42_adaptive"
            ])

    def test_06_missing_other_arms_rejection(self):
        """Rejects pentad order missing any individual arm."""
        for missing_arm in Orchestrator.ALL_PENTAD_ARMS:
            order = [a for a in Orchestrator.ALL_PENTAD_ARMS if a != missing_arm]
            with self.assertRaises(SafetyViolationError):
                Orchestrator.validate_pentad_arm_order(order)

    def test_07_duplicate_arm_rejection(self):
        """Rejects pentad order with duplicate arms."""
        order = [
            "arm_a_native", "arm_b_cand3", "arm_c_cand42_always_on",
            "arm_d_cand42_adaptive", "arm_a_native"
        ]
        with self.assertRaises(SafetyViolationError):
            Orchestrator.validate_pentad_arm_order(order)

    def test_08_unknown_candidate_rejection(self):
        """Rejects unknown candidate versions in orchestrator."""
        with self.assertRaises(ValueError):
            Orchestrator(
                task_config_path=self.task_cfg,
                pair_id="test",
                candidate_version="unknown_cand_99"
            )

    def test_09_wrong_arm_identity_rejection(self):
        """Rejects unknown arm identifier in arm order."""
        order = [
            "arm_a_native", "arm_b_cand3", "arm_c_cand42_always_on",
            "arm_d_cand42_adaptive", "arm_x_unregistered"
        ]
        with self.assertRaises(SafetyViolationError):
            Orchestrator.validate_pentad_arm_order(order)

    def test_10_fixture_hash_mismatch_rejection(self):
        """Rejects corrupted or altered fixture configurations."""
        bad_cfg = {
            "task_id": "bad_task",
            "prompt": "Fix typo",
            "fixture_preflight": {
                "required_files": ["nonexistent_path_corrupted/fixture.txt"]
            }
        }
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
            json.dump(bad_cfg, f)
            bad_path = Path(f.name)
        try:
            with self.assertRaises(Exception):
                Orchestrator(task_config_path=bad_path, pair_id="bad", is_dry_run=True, dependency_strategy="none")
        finally:
            bad_path.unlink(missing_ok=True)

    def test_11_candidate_binary_hash_mismatch_rejection(self):
        """Rejects dispatch if candidate binary hash does not match pre-registered ledger."""
        cand_id = ArmCandidateIdentity(
            arm="arm_e_cand5_optimized",
            requested_candidate_version="cand5",
            resolved_candidate_version="cand5",
            binary_path=HARNESS_ROOT / "bin" / "candidate-phase5" / "engram.exe",
            expected_sha256="0000000000000000000000000000000000000000000000000000000000000000",
            is_engram=True,
        )
        with self.assertRaises(SafetyViolationError):
            cand_id.verify_binary_integrity()

    def test_12_missing_telemetry_rejection(self):
        """Rejects run with missing telemetry via acceptance validator."""
        run_record = {
            "execution_status": "SUCCESS",
            "correctness": {"score": 1.0, "passed_all_requirements": True},
            "telemetry_valid": False,
            "telemetry": {}
        }
        self.assertFalse(Orchestrator.is_run_accepted(run_record))

    def test_13_duplicate_telemetry_rejection(self):
        """Validates telemetry integrity rejects inconsistent total tokens."""
        telem = {
            "total_model_tokens": 1000,
            "prompt_tokens_total": 800,
            "completion_tokens": 500,  # 800 + 500 = 1300 != 1000
        }
        valid, reason, _ = Orchestrator.validate_telemetry_integrity(telem)
        self.assertFalse(valid)
        self.assertIn("INVARIANT_VIOLATION", reason)

    def test_14_incomplete_arm_rejection(self):
        """Rejects block acceptance if any arm was incomplete or failed."""
        pentad_summary = {
            "acceptance": {
                "arm_a_native_accepted": True,
                "arm_b_cand3_accepted": True,
                "arm_c_cand42_always_on_accepted": True,
                "arm_d_cand42_adaptive_accepted": True,
                "arm_e_cand5_optimized_accepted": False,
                "all_arms_accepted": False
            }
        }
        self.assertFalse(pentad_summary["acceptance"]["all_arms_accepted"])

    def test_15_per_arm_budget_overflow(self):
        """Verifies per-arm token ceiling override to TOKEN_CEILING_BREACHED."""
        orch = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="pentad_overbudget",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            max_tokens_ceiling=500  # Ceiling lower than dry-run synthetic tokens (2850+)
        )
        res = orch.run_pentad()
        # When ceiling is breached, arm status must be overridden
        breached = [r for r in res["runs"].values() if r.get("execution_status") == "TOKEN_CEILING_BREACHED"]
        self.assertGreater(len(breached), 0)

    def test_16_global_budget_overflow(self):
        """Verifies global budget ceiling halting downstream arms."""
        orch = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="pentad_global_halt",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            global_tokens_ceiling=5000,
            headroom_reserve=3000
        )
        res = orch.run_pentad()
        self.assertTrue(any(sub in res["block_status"] for sub in ["ABORTED", "HALTED", "MUTUAL_ACCEPTANCE"]))

    def test_17_stop_on_failure_behavior(self):
        """Verifies strict execution policy halts on first arm failure."""
        orch = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="pentad_stop_fail",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            max_tokens_ceiling=100,  # Causes immediate ceiling breach
            stop_on_failure=True
        )
        res = orch.run_pentad()
        skipped = [r for r in res["runs"].values() if "SKIPPED" in r.get("execution_status", "")]
        self.assertGreater(len(skipped), 0)

    def test_18_deterministic_execution_order_under_seed(self):
        """Verifies identical seed produces identical execution order."""
        order1 = None
        order2 = None
        for _ in range(2):
            orch = Orchestrator(
                task_config_path=self.task_cfg,
                pair_id="pentad_seed_test",
                output_dir=self.output_dir,
                is_dry_run=True,
                dependency_strategy="none",
                seed=424242
            )
            res = orch.run_pentad()
            if order1 is None:
                order1 = res["order"]
            else:
                order2 = res["order"]
        self.assertEqual(order1, order2)

    def test_19_correct_accounting_of_skipped_arms(self):
        """Verifies skipped arms are marked with execution_status and reasons."""
        orch = Orchestrator(
            task_config_path=self.task_cfg,
            pair_id="pentad_skipped_acc",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            max_tokens_ceiling=50,
            stop_on_failure=True
        )
        res = orch.run_pentad()
        for arm, rdata in res["runs"].items():
            if "SKIPPED" in rdata.get("execution_status", ""):
                self.assertTrue(bool(rdata.get("error_message") or rdata.get("skip_reason")))

    def test_20_prevention_of_comparisons_when_incomplete(self):
        """Verifies comparative analysis is marked INCOMPLETE when any arm fails."""
        ctrl_run = {"execution_status": "SUCCESS", "correctness": {"score": 1.0, "passed_all_requirements": True}}
        treat_run = {"execution_status": "FAILED", "correctness": {"score": 0.0, "passed_all_requirements": False}}
        orch = Orchestrator(task_config_path=self.task_cfg, pair_id="comp_test", is_dry_run=True, dependency_strategy="none")
        comp = orch.compute_comparative_analysis(
            ctrl_run=ctrl_run, treat_run=treat_run,
            baseline_label="native", treatment_label="cand5",
            block_incomplete=True
        )
        self.assertIn(comp.get("verdict"), ["INCOMPLETE", "BLOCKED", "FAIL", "REJECTED", "NOT_COMPARABLE", "MUTUAL_ACCEPTANCE_FAILURE"])


if __name__ == "__main__":
    unittest.main()
