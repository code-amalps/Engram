"""
Offline test suite for Stage 1 Core Triad 3-Arm Benchmark Harness.
Tests arm assignment, Latin-square schedule, binary validation, run isolation,
comparative calculations, invalidation guards, schema conformity, and reporting.
"""
from typing import Dict, Any
from pathlib import Path
import unittest
import tempfile
import shutil
import json

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "src"))

from orchestrator import (
    Orchestrator,
    TRIAD_BLOCK_SCHEDULES,
    DEFAULT_ENGRAM_V151_BIN,
    DEFAULT_ENGRAM_V154_BIN,
    FROZEN_ENGRAM_V151_SHA256,
    FROZEN_ENGRAM_V154_SHA256,
)
from schema_validator import validate_instance, SchemaValidationError
from isolation import verify_frozen_binary


HARNESS_ROOT = Path(__file__).resolve().parent.parent
CONFIG_DIR = HARNESS_ROOT / "config" / "tasks"
SCHEMAS_DIR = HARNESS_ROOT / "schemas"


class TestTriadArmAssignment(unittest.TestCase):
    """Verifies that Latin-square schedule blocks and arm order mappings conform strictly to protocol."""

    def test_latin_square_10_blocks_specification(self):
        """All 10 blocks in Amendment 02 must be defined with exact tasks, orders, and seeds."""
        self.assertEqual(len(TRIAD_BLOCK_SCHEDULES), 10)

        # Blocks 01-05 are Task A
        for i in range(1, 6):
            bid = f"block_{i:02d}"
            sched = TRIAD_BLOCK_SCHEDULES[bid]
            self.assertEqual(sched["task"], "task_a_typo")
            self.assertEqual(len(sched["order"]), 3)
            self.assertIn("baseline_arm1", sched["order"])
            self.assertIn("v151_arm2", sched["order"])
            self.assertIn("v154_arm3", sched["order"])

        # Blocks 06-10 are Task B
        for i in range(6, 11):
            bid = f"block_{i:02d}"
            sched = TRIAD_BLOCK_SCHEDULES[bid]
            self.assertEqual(sched["task"], "task_b_remember_me")
            self.assertEqual(len(sched["order"]), 3)
            self.assertIn("baseline_arm1", sched["order"])
            self.assertIn("v151_arm2", sched["order"])
            self.assertIn("v154_arm3", sched["order"])

    def test_latin_square_counterbalancing(self):
        """Verify that across 5 blocks each arm starts at least once."""
        task_a_first_arms = [TRIAD_BLOCK_SCHEDULES[f"block_{i:02d}"]["order"][0] for i in range(1, 6)]
        self.assertIn("baseline_arm1", task_a_first_arms)
        self.assertIn("v151_arm2", task_a_first_arms)
        self.assertIn("v154_arm3", task_a_first_arms)

    def test_binary_verification_hashes(self):
        """Verify that actual v1.5.1 and v1.5.4 executables match their authoritative SHA-256 hashes."""
        self.assertTrue(DEFAULT_ENGRAM_V151_BIN.exists(), f"v1.5.1 binary not found at {DEFAULT_ENGRAM_V151_BIN}")
        self.assertTrue(DEFAULT_ENGRAM_V154_BIN.exists(), f"v1.5.4 binary not found at {DEFAULT_ENGRAM_V154_BIN}")

        self.assertTrue(verify_frozen_binary(DEFAULT_ENGRAM_V151_BIN, FROZEN_ENGRAM_V151_SHA256))
        self.assertTrue(verify_frozen_binary(DEFAULT_ENGRAM_V154_BIN, FROZEN_ENGRAM_V154_SHA256))


class TestTriadExecutionAndSchema(unittest.TestCase):
    """Verifies offline dry-run execution of a 3-arm triad block, JSON schema validation, and reporting."""

    def setUp(self):
        self.test_dir = Path(tempfile.mkdtemp(prefix="engram_triad_test_"))
        self.golden_repo = self.test_dir / "repo"
        self.golden_repo.mkdir(parents=True)
        # Create minimal mock golden repo files
        (self.golden_repo / "package.json").write_text('{"name": "mock-repo"}', encoding="utf-8")
        (self.golden_repo / "package-lock.json").write_text('{"lockfileVersion": 2}', encoding="utf-8")

        # Mock git commit
        import subprocess
        subprocess.run(["git", "init"], cwd=self.golden_repo, capture_output=True, check=True)
        subprocess.run(["git", "config", "user.email", "test@engram.dev"], cwd=self.golden_repo, check=True)
        subprocess.run(["git", "config", "user.name", "Test Runner"], cwd=self.golden_repo, check=True)

        # Add target file for Task A fixture preflight
        target_file = self.golden_repo / "projects" / "angular-ngrx-material-starter" / "src" / "app" / "features" / "about" / "about" / "about.component.html"
        target_file.parent.mkdir(parents=True, exist_ok=True)
        target_file.write_text("<span>Angular Cli Tips</span>", encoding="utf-8")

        subprocess.run(["git", "add", "."], cwd=self.golden_repo, check=True)
        subprocess.run(["git", "commit", "-m", "initial commit"], cwd=self.golden_repo, check=True)

        res = subprocess.run(["git", "rev-parse", "HEAD"], cwd=self.golden_repo, capture_output=True, text=True, check=True)
        self.commit_sha = res.stdout.strip()

    def tearDown(self):
        shutil.rmtree(self.test_dir, ignore_errors=True)

    def test_offline_dry_run_triad_execution(self):
        """Executes a full 3-arm dry-run triad block and validates output schema and reports."""
        task_a_path = CONFIG_DIR / "task_a_typo.json"
        with open(task_a_path, "r", encoding="utf-8") as f:
            cfg = json.load(f)
        cfg["baseline_commit"] = self.commit_sha

        test_task_path = self.test_dir / "task_a_typo.json"
        with open(test_task_path, "w", encoding="utf-8") as f:
            json.dump(cfg, f, indent=2)

        output_dir = self.test_dir / "runs" / "task_a_block_01"

        orchestrator = Orchestrator(
            task_config_path=test_task_path,
            pair_id="block_01",
            golden_repo=self.golden_repo,
            is_dry_run=True,
            dependency_strategy="none",
            output_dir=output_dir,
            seed=2026102801
        )

        triad_summary = orchestrator.run_triad(block_id="block_01")

        # Verify 3 runs executed
        self.assertEqual(len(triad_summary["runs"]), 3)
        self.assertIn("baseline_arm1", triad_summary["runs"])
        self.assertIn("v151_arm2", triad_summary["runs"])
        self.assertIn("v154_arm3", triad_summary["runs"])

        # Check arm ordering matches block_01
        self.assertEqual(triad_summary["order"], ["baseline_arm1", "v151_arm2", "v154_arm3"])

        # Check telemetry schema validation for each individual run
        telemetry_schema_path = SCHEMAS_DIR / "run_telemetry.schema.json"
        with open(telemetry_schema_path, "r", encoding="utf-8") as f:
            tel_schema = json.load(f)

        for arm, run_data in triad_summary["runs"].items():
            validate_instance(run_data, tel_schema)

        # Check triad summary schema validation
        triad_schema_path = SCHEMAS_DIR / "triad_summary.schema.json"
        with open(triad_schema_path, "r", encoding="utf-8") as f:
            triad_schema = json.load(f)
        validate_instance(triad_summary, triad_schema)

        # Verify comparative analysis structure
        comp = triad_summary["comparative_analysis"]
        self.assertIn("v154_vs_baseline", comp)
        self.assertIn("v154_vs_v151", comp)
        self.assertIn("v151_vs_baseline", comp)

        # In dry run with mock unchanged repo, C=0.0 (no edits applied in dry run), so claim status must be INVALIDATED_BY_ACCEPTANCE_FAILURE
        self.assertEqual(comp["v154_vs_baseline"]["savings_claim_status"], "INVALIDATED_BY_ACCEPTANCE_FAILURE")
        self.assertFalse(comp["v154_vs_baseline"]["savings_claim_eligible"])

        # Verify report file exists and contains essential sections
        report_path = output_dir / "triad_report.md"
        self.assertTrue(report_path.exists())
        report_text = report_path.read_text(encoding="utf-8")
        self.assertIn("# Stage 1 Core Triad Benchmark Report", report_text)
        self.assertIn("Acceptance & Correctness Gate", report_text)
        self.assertIn("Telemetry Comparison", report_text)
        self.assertIn("Primary Outcome: Arm 3 (v1.5.4) vs Arm 1 (Baseline)", report_text)


class TestTriadComparativeLogic(unittest.TestCase):
    """Verifies that pairwise comparisons compute exact metrics when accepted and invalidate when failed."""

    def _create_mock_run(self, score: float, passed: bool, total_tokens: int, prompt_tokens: int, completion_tokens: int) -> Dict[str, Any]:
        return {
            "correctness": {
                "score": score,
                "passed_all_requirements": passed,
                "quality_adjusted_cost": float(total_tokens / score) if score > 0 else float(total_tokens)
            },
            "telemetry": {
                "total_model_tokens": total_tokens,
                "prompt_tokens_total": prompt_tokens,
                "completion_tokens": completion_tokens,
                "prompt_tokens_cached": int(prompt_tokens * 0.8),
                "prompt_tokens_cached_inferred": False,
                "prompt_tokens_uncached": prompt_tokens - int(prompt_tokens * 0.8),
                "prompt_tokens_uncached_inferred": False,
                "peak_single_request_context": prompt_tokens,
                "turns_count": 3,
                "mcp_tool_calls_count": 2,
                "auxiliary_tool_payload_bytes": 500
            }
        }

    def test_full_acceptance_computes_exact_savings(self):
        """When all arms achieve C=1.0, pairwise savings must be strictly positive and mathematically exact."""
        run_baseline = self._create_mock_run(score=1.0, passed=True, total_tokens=25000, prompt_tokens=20000, completion_tokens=5000)
        run_v151 = self._create_mock_run(score=1.0, passed=True, total_tokens=15000, prompt_tokens=12000, completion_tokens=3000)
        run_v154 = self._create_mock_run(score=1.0, passed=True, total_tokens=5000, prompt_tokens=4000, completion_tokens=1000)

        # Primary Outcome: Arm 3 vs Arm 1
        comp_primary = Orchestrator.compute_comparative_analysis(run_baseline, run_v154, "baseline_arm1", "v154_arm3")
        self.assertTrue(comp_primary["savings_claim_eligible"])
        self.assertEqual(comp_primary["savings_claim_status"], "VALID")
        self.assertEqual(comp_primary["verdict"], "EFFICIENCY_GAIN")

        # Delta: 25000 - 5000 = 20000 -> 80.0% savings
        m_tot = comp_primary["metrics"]["total_model_tokens"]
        self.assertEqual(m_tot["delta"], 20000)
        self.assertEqual(m_tot["savings_pct"], 80.0)
        self.assertEqual(m_tot["baseline_arm1"], 25000)
        self.assertEqual(m_tot["v154_arm3"], 5000)

        # Architectural Evolution: Arm 3 vs Arm 2
        comp_arch = Orchestrator.compute_comparative_analysis(run_v151, run_v154, "v151_arm2", "v154_arm3")
        self.assertTrue(comp_arch["savings_claim_eligible"])
        # Delta: 15000 - 5000 = 10000 -> 66.67% savings
        m_arch_tot = comp_arch["metrics"]["total_model_tokens"]
        self.assertEqual(m_arch_tot["delta"], 10000)
        self.assertEqual(m_arch_tot["savings_pct"], 66.67)

    def test_failure_on_baseline_suppresses_savings(self):
        """If baseline fails correctness (C < 1.0), comparative savings are invalidated."""
        run_baseline = self._create_mock_run(score=0.5, passed=False, total_tokens=25000, prompt_tokens=20000, completion_tokens=5000)
        run_v154 = self._create_mock_run(score=1.0, passed=True, total_tokens=5000, prompt_tokens=4000, completion_tokens=1000)

        comp = Orchestrator.compute_comparative_analysis(run_baseline, run_v154, "baseline_arm1", "v154_arm3")
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_ACCEPTANCE_FAILURE")
        self.assertEqual(comp["verdict"], "CONTROL_FAILED_ACCEPTANCE")
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["savings_pct"])

    def test_failure_on_v154_suppresses_savings(self):
        """If v1.5.4 fails correctness, savings claims are strictly suppressed."""
        run_baseline = self._create_mock_run(score=1.0, passed=True, total_tokens=25000, prompt_tokens=20000, completion_tokens=5000)
        run_v154 = self._create_mock_run(score=0.0, passed=False, total_tokens=5000, prompt_tokens=4000, completion_tokens=1000)

        comp = Orchestrator.compute_comparative_analysis(run_baseline, run_v154, "baseline_arm1", "v154_arm3")
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_ACCEPTANCE_FAILURE")
        self.assertEqual(comp["verdict"], "TREATMENT_FAILED_ACCEPTANCE")
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["savings_pct"])

    def test_missing_telemetry_suppresses_savings_even_if_accepted(self):
        """Even if all pass C=1.0, missing or malformed telemetry invalidates savings."""
        run_baseline = self._create_mock_run(score=1.0, passed=True, total_tokens=25000, prompt_tokens=20000, completion_tokens=5000)
        run_v154_empty_tel = {
            "correctness": {"score": 1.0, "passed_all_requirements": True},
            "telemetry": {}
        }

        comp = Orchestrator.compute_comparative_analysis(run_baseline, run_v154_empty_tel, "baseline_arm1", "v154_arm3")
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_TELEMETRY_INVALID")
        self.assertEqual(comp["verdict"], "TREATMENT_TELEMETRY_INVALID")


if __name__ == "__main__":
    unittest.main()
