import json
import tempfile
import unittest
import sys
from pathlib import Path

# Ensure src is on sys.path
HARNESS_ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = HARNESS_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from orchestrator import Orchestrator


class TestTetradHarness(unittest.TestCase):

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.output_dir = Path(self.temp_dir.name) / "tetrad_out"

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_tetrad_dry_run_execution_and_reports(self):
        """Verifies that 4-arm tetrad dry-run executes across all 4 arms and produces valid summary and report."""
        task_cfg = HARNESS_ROOT / "config" / "tasks" / "task_a_typo.json"
        orchestrator = Orchestrator(
            task_config_path=task_cfg,
            pair_id="tetrad_dryrun_01",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
        )

        res = orchestrator.run_tetrad(block_id="tetrad_block_01")

        self.assertIn("schema_version", res)
        self.assertEqual(res["schema_version"], "2.0.0")
        self.assertEqual(res["block_id"], "tetrad_block_01")
        self.assertEqual(len(res["runs"]), 4)

        # Verify all 4 arms are present in runs
        expected_arms = {
            "arm_a_native",
            "arm_b_cand3_always_on",
            "arm_c_cand4_always_on",
            "arm_d_cand4_adaptive"
        }
        self.assertEqual(set(res["runs"].keys()), expected_arms)

        # Verify comparative analysis keys
        comp = res.get("comparative_analysis", {})
        self.assertIn("cand4_adaptive_vs_native", comp)
        self.assertIn("cand4_always_on_vs_native", comp)
        self.assertIn("cand3_always_on_vs_native", comp)
        self.assertIn("cand4_vs_cand3", comp)
        self.assertIn("adaptive_vs_always_on", comp)

        # Verify disk artifacts
        summary_file = self.output_dir / "tetrad_summary.json"
        report_file = self.output_dir / "tetrad_report.md"
        self.assertTrue(summary_file.exists(), "tetrad_summary.json must exist")
        self.assertTrue(report_file.exists(), "tetrad_report.md must exist")

        with open(summary_file, "r", encoding="utf-8") as f:
            disk_summary = json.load(f)
        self.assertEqual(disk_summary["block_id"], "tetrad_block_01")

        with open(report_file, "r", encoding="utf-8") as f:
            md_text = f.read()
        self.assertIn("Phase 5.4 4-Arm Benchmark Report", md_text)
        for arm in expected_arms:
            self.assertIn(arm, md_text)


if __name__ == "__main__":
    unittest.main()
