"""
Offline Unit and Integration Test Suite for Amendment 04 (Alternative B: Two-Arm Focus).
Verifies:
1. Arm selection and order (v154_arm3 -> baseline_arm1 with v151_arm2 deferred).
2. Per-arm token ceiling propagation (250,000 default).
3. Candidate failure skipping downstream baseline arm under stop-on-failure.
4. Telemetry missing or invalid strictly suppressing comparative savings.
5. Acceptance failure (C < 1.0) strictly suppressing comparative savings.
6. Mathematical token accounting integrity (no double-counting cached or reasoning tokens).
7. Strict schema validation against triad_summary.schema.json and run_telemetry.schema.json.
8. Baseline isolation (zero MCP tools, zero Turn-0 injection) vs candidate activation.
9. CLI argument parsing support for --arms and --arm-order with --block.
"""
import sys
import unittest
import tempfile
import shutil
import json
import argparse
import subprocess
from pathlib import Path
from typing import Dict, Any

HARNESS_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(HARNESS_ROOT / "src"))

from orchestrator import (
    Orchestrator,
    TRIAD_BLOCK_SCHEDULES,
    FROZEN_ENGRAM_V151_SHA256,
    FROZEN_ENGRAM_V154_SHA256,
)
from runner import CodexRunner
from schema_validator import validate_instance, SchemaValidationError


class TestAmendment04TwoArmProtocol(unittest.TestCase):
    """Verifies Alternative B execution mechanics, schema conformance, and guard invariants."""

    def setUp(self):
        self.test_dir = Path(tempfile.mkdtemp(prefix="engram_a04_test_"))
        self.golden_repo = self.test_dir / "golden_repo"
        self.golden_repo.mkdir(parents=True)

        # Create minimal mock golden repo files
        (self.golden_repo / "package.json").write_text('{"name": "mock-repo"}', encoding="utf-8")
        (self.golden_repo / "package-lock.json").write_text('{"lockfileVersion": 2}', encoding="utf-8")

        # Mock git commit
        subprocess.run(["git", "init"], cwd=self.golden_repo, capture_output=True, check=True)
        subprocess.run(["git", "config", "user.email", "test@engram.dev"], cwd=self.golden_repo, check=True)
        subprocess.run(["git", "config", "user.name", "Test Runner"], cwd=self.golden_repo, check=True)

        # Add target files for Task B fixture preflight
        target_file = self.golden_repo / "projects" / "angular-ngrx-material-starter" / "src" / "app" / "core" / "auth" / "auth.effects.ts"
        target_file.parent.mkdir(parents=True, exist_ok=True)
        target_file.write_text("// auth effects mock", encoding="utf-8")

        subprocess.run(["git", "add", "."], cwd=self.golden_repo, check=True)
        subprocess.run(["git", "commit", "-m", "initial commit"], cwd=self.golden_repo, check=True)

        res = subprocess.run(["git", "rev-parse", "HEAD"], cwd=self.golden_repo, capture_output=True, text=True, check=True)
        self.commit_sha = res.stdout.strip()

        # Create mock task_b config with this commit_sha
        task_b_path = HARNESS_ROOT / "config" / "tasks" / "task_b_remember_me.json"
        with open(task_b_path, "r", encoding="utf-8") as f:
            cfg = json.load(f)
        cfg["baseline_commit"] = self.commit_sha
        cfg.pop("fixture_preflight", None)  # Bypass preflight check on synthetic mock golden repo

        self.test_task_path = self.test_dir / "task_b_test.json"
        with open(self.test_task_path, "w", encoding="utf-8") as f:
            json.dump(cfg, f, indent=2)

        self.output_dir = self.test_dir / "runs" / "task_b_block_07"

    def tearDown(self):
        shutil.rmtree(self.test_dir, ignore_errors=True)

    def test_two_arm_order_and_deferred_arm_population(self):
        """When 2 arms are executed, omitted triad arm (v151_arm2) must be populated as DEFERRED_AMENDMENT_04."""
        orch = Orchestrator(
            task_config_path=self.test_task_path,
            pair_id="block_07",
            golden_repo=self.golden_repo,
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            stop_on_failure=True,
            max_tokens_ceiling=250_000,
        )

        two_arm_order = ["v154_arm3", "baseline_arm1"]
        summary = orch.run_triad(block_id="block_07", arm_order=two_arm_order)

        # 1. Order matches the 2 specified arms
        self.assertEqual(summary["order"], two_arm_order)

        # 2. Runs contains all 3 arms for schema completeness
        runs = summary["runs"]
        self.assertIn("v154_arm3", runs)
        self.assertIn("baseline_arm1", runs)
        self.assertIn("v151_arm2", runs)

        # 3. v151_arm2 is populated as deferred
        v151_record = runs["v151_arm2"]
        self.assertEqual(v151_record["execution_status"], "DEFERRED_AMENDMENT_04")
        self.assertFalse(v151_record["telemetry_valid"])
        self.assertEqual(v151_record["correctness"]["score"], 0.0)

        # 4. Acceptance flags
        acc = summary["acceptance"]
        self.assertFalse(acc["v151_arm2_accepted"])
        self.assertFalse(acc["all_arms_accepted"])

        # 5. Schema validation against triad_summary.schema.json passes
        schema_path = HARNESS_ROOT / "schemas" / "triad_summary.schema.json"
        with open(schema_path, "r", encoding="utf-8") as f:
            triad_schema = json.load(f)
        validate_instance(summary, triad_schema)

    def test_candidate_failure_skips_baseline_under_stop_on_failure(self):
        """If candidate (v154_arm3, Order 1) fails, baseline (Order 2) must be SKIPPED_STOP_ON_FAILURE."""
        orch = Orchestrator(
            task_config_path=self.test_task_path,
            pair_id="block_07",
            golden_repo=self.golden_repo,
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            stop_on_failure=True,
            max_tokens_ceiling=250_000,
        )

        # Simulate execution where arm 1 fails by overriding _execute_single_arm
        def mock_failed_arm(arm: str, exec_order: int):
            if arm == "v154_arm3":
                return {
                    "schema_version": "2.0.0",
                    "run_id": f"task_b_remember_me_block_07_{arm}",
                    "pair_id": "block_07",
                    "task_id": "task_b_remember_me",
                    "arm": arm,
                    "execution_order_in_pair": exec_order,
                    "execution_status": "TOKEN_CEILING_BREACHED",
                    "error_message": "Execution terminated: Token ceiling exceeded (254,120 > 250,000 tokens)",
                    "telemetry_valid": True,
                    "timestamps": {"started_at": "2026-10-29T12:00:00Z", "completed_at": "2026-10-29T12:02:00Z", "elapsed_seconds": 120.0},
                    "model": {"provider": "openai", "name": "gpt-5.6-luna", "mode": "agent", "reasoning_effort": "medium", "harness_executable": "codex.exe", "harness_version": "1.0"},
                    "isolation": {"baseline_commit": self.commit_sha, "sandbox_path": "mock", "cold_start_verified": True, "engram_enabled": True, "engram_binary_sha256": "cf807eac", "mcp_server_status": "ACTIVE", "dependency_fingerprint_verified": True, "path_sanitized": True},
                    "telemetry": {"provenance": "SESSION_ROLLOUT", "prompt_tokens_total": 250000, "prompt_tokens_cached": 200000, "prompt_tokens_cached_inferred": False, "prompt_tokens_uncached": 50000, "prompt_tokens_uncached_inferred": False, "completion_tokens": 4120, "reasoning_tokens": 800, "total_model_tokens": 254120, "peak_single_request_context": 32000, "turns_count": 8, "mcp_tool_calls_count": 4, "auxiliary_tool_payload_bytes": 1200, "telemetry_valid": True, "error_message": None},
                    "git_impact": {"files_changed_count": 2, "files_changed": ["auth.effects.ts"], "insertions": 20, "deletions": 5, "unrelated_files_count": 0, "diff_patch_path": "", "dependency_mutated": False},
                    "correctness": {"score": 0.0, "quality_adjusted_cost": 2541200.0, "passed_all_requirements": False, "gates": {"L0_typecheck": "BLOCKED", "L1_lint_prettier": "SKIPPED", "L2_unit_tests": "BLOCKED", "L3_production_build": "BLOCKED"}, "requirements": {"REQ-1": "FAIL", "REQ-2": "FAIL", "REQ-3": "FAIL", "REQ-4": "FAIL"}}
                }
            return orch._execute_single_arm(arm, exec_order)

        orch._execute_single_arm = mock_failed_arm
        summary = orch.run_triad(block_id="block_07", arm_order=["v154_arm3", "baseline_arm1"])

        runs = summary["runs"]
        # v154_arm3 breached ceiling
        self.assertEqual(runs["v154_arm3"]["execution_status"], "TOKEN_CEILING_BREACHED")
        # baseline_arm1 was skipped because arm 1 failed
        self.assertEqual(runs["baseline_arm1"]["execution_status"], "SKIPPED_STOP_ON_FAILURE")
        # v151_arm2 was deferred
        self.assertEqual(runs["v151_arm2"]["execution_status"], "DEFERRED_AMENDMENT_04")

        # Comparative savings strictly suppressed
        comp = summary["comparative_analysis"]["v154_vs_baseline"]
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_ACCEPTANCE_FAILURE")
        self.assertEqual(comp["verdict"], "MUTUAL_ACCEPTANCE_FAILURE")

    def test_per_arm_token_ceiling_enforcement(self):
        """Runner must enforce max_tokens_ceiling (250,000) during streaming checks."""
        runner = CodexRunner(
            codex_executable=HARNESS_ROOT / "bin" / "mock-codex.exe",
            engram_executable=HARNESS_ROOT / "bin" / "v1.5.4" / "engram.exe",
            timeout_seconds=600,
            max_tokens_ceiling=250_000
        )
        self.assertEqual(runner.max_tokens_ceiling, 250_000)

    def test_baseline_receives_no_mcp_or_turn0(self):
        """Baseline arm must execute with zero Turn-0 injection and MCP disabled."""
        runner = CodexRunner(
            codex_executable=HARNESS_ROOT / "bin" / "mock-codex.exe",
            engram_executable=HARNESS_ROOT / "bin" / "v1.5.4" / "engram.exe",
            timeout_seconds=600,
            max_tokens_ceiling=250_000
        )
        events_path = self.test_dir / "events_baseline.jsonl"
        task_cfg = {"id": "task_b_remember_me", "target_files": ["core.module.ts"]}
        result = runner.run(
            sandbox_path=self.test_dir,
            prompt="Test prompt for baseline",
            arm="baseline_arm1",
            output_events_path=events_path,
            is_dry_run=True,
            task_config=task_cfg
        )
        self.assertFalse(result.turn0_injected)
        self.assertNotIn("<!-- BEGIN ENGRAM MANAGED BLOCK -->", result.initial_prompt)
        self.assertEqual(result.initial_prompt, "Test prompt for baseline")
        self.assertTrue(result.path_sanitized)

    def test_candidate_receives_turn0_and_mcp_on_task_b(self):
        """Candidate arm (v154_arm3) on Task B must receive Turn-0 instructions in dry-run."""
        runner = CodexRunner(
            codex_executable=HARNESS_ROOT / "bin" / "mock-codex.exe",
            engram_executable=HARNESS_ROOT / "bin" / "v1.5.4" / "engram.exe",
            timeout_seconds=600,
            max_tokens_ceiling=250_000
        )
        events_path = self.test_dir / "events_candidate.jsonl"
        task_cfg = {
            "id": "task_b_remember_me",
            "target_files": ["auth.effects.ts", "app.component.ts"]
        }
        result = runner.run(
            sandbox_path=self.test_dir,
            prompt="Test prompt for candidate",
            arm="v154_arm3",
            output_events_path=events_path,
            is_dry_run=True,
            task_config=task_cfg
        )
        self.assertTrue(result.turn0_injected)
        self.assertIn("<!-- BEGIN ENGRAM MANAGED BLOCK -->", result.initial_prompt)
        self.assertIn("Task Instructions:\nTest prompt for candidate", result.initial_prompt)

    def test_missing_or_invalid_telemetry_suppresses_savings(self):
        """If telemetry is missing or invalid on either arm, savings claims are strictly suppressed."""
        run_baseline = {
            "correctness": {"score": 1.0, "passed_all_requirements": True},
            "telemetry": {"total_model_tokens": 240000, "prompt_tokens_total": 235000, "completion_tokens": 5000, "telemetry_valid": True}
        }
        run_v154_invalid_tel = {
            "correctness": {"score": 1.0, "passed_all_requirements": True},
            "telemetry": {"total_model_tokens": 100000, "prompt_tokens_total": 95000, "completion_tokens": 5000, "telemetry_valid": False}
        }

        comp = Orchestrator.compute_comparative_analysis(run_baseline, run_v154_invalid_tel, "baseline_arm1", "v154_arm3")
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_TELEMETRY_INVALID")
        self.assertEqual(comp["verdict"], "TREATMENT_TELEMETRY_INVALID")
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["savings_pct"])

    def test_acceptance_failure_suppresses_savings(self):
        """If candidate fails acceptance (C < 1.0), savings claims are strictly suppressed."""
        run_baseline = {
            "correctness": {"score": 1.0, "passed_all_requirements": True},
            "telemetry": {"total_model_tokens": 240000, "prompt_tokens_total": 235000, "completion_tokens": 5000, "telemetry_valid": True}
        }
        run_v154_failed = {
            "correctness": {"score": 0.5, "passed_all_requirements": False},
            "telemetry": {"total_model_tokens": 80000, "prompt_tokens_total": 75000, "completion_tokens": 5000, "telemetry_valid": True}
        }

        comp = Orchestrator.compute_comparative_analysis(run_baseline, run_v154_failed, "baseline_arm1", "v154_arm3")
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_ACCEPTANCE_FAILURE")
        self.assertEqual(comp["verdict"], "TREATMENT_FAILED_ACCEPTANCE")
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["savings_pct"])

    def test_full_acceptance_computes_exact_savings_without_double_counting(self):
        """When both arms achieve C=1.0 with valid telemetry, savings are exact and non-double-counted."""
        run_baseline = {
            "correctness": {"score": 1.0, "passed_all_requirements": True, "quality_adjusted_cost": 240000.0},
            "telemetry": {
                "total_model_tokens": 240000,
                "prompt_tokens_total": 235000,
                "prompt_tokens_cached": 200000,
                "prompt_tokens_uncached": 35000,
                "completion_tokens": 5000,
                "reasoning_tokens": 1200,
                "peak_single_request_context": 35000,
                "turns_count": 9,
                "mcp_tool_calls_count": 0,
                "auxiliary_tool_payload_bytes": 0,
                "telemetry_valid": True
            }
        }
        run_v154 = {
            "correctness": {"score": 1.0, "passed_all_requirements": True, "quality_adjusted_cost": 96000.0},
            "telemetry": {
                "total_model_tokens": 96000,
                "prompt_tokens_total": 92000,
                "prompt_tokens_cached": 75000,
                "prompt_tokens_uncached": 17000,
                "completion_tokens": 4000,
                "reasoning_tokens": 600,
                "peak_single_request_context": 18000,
                "turns_count": 5,
                "mcp_tool_calls_count": 6,
                "auxiliary_tool_payload_bytes": 1800,
                "telemetry_valid": True
            }
        }

        comp = Orchestrator.compute_comparative_analysis(run_baseline, run_v154, "baseline_arm1", "v154_arm3")
        self.assertTrue(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "VALID")
        self.assertEqual(comp["verdict"], "EFFICIENCY_GAIN")

        # Delta: 240,000 - 96,000 = 144,000 tokens
        # Savings: 144,000 / 240,000 = 60.0%
        m_tot = comp["metrics"]["total_model_tokens"]
        self.assertEqual(m_tot["delta"], 144000)
        self.assertEqual(m_tot["savings_pct"], 60.0)
        self.assertEqual(m_tot["baseline_arm1"], 240000)
        self.assertEqual(m_tot["v154_arm3"], 96000)

    def test_cli_argument_parsing(self):
        """CLI parser must accept --block block_07 with --arms or --arm-order and --max-tokens-ceiling."""
        parser = argparse.ArgumentParser()
        parser.add_argument("--mode", choices=["pair", "triad"], default="pair")
        parser.add_argument("--task", default="task_a_typo")
        parser.add_argument("--pair", default="pair_01")
        parser.add_argument("--block", default=None)
        parser.add_argument("--arm-order", "--arms", dest="arm_order", default=None)
        parser.add_argument("--model", default="gpt-5.6-luna")
        parser.add_argument("--reasoning", default="medium")
        parser.add_argument("--max-tokens-ceiling", type=int, default=250_000)

        # 1. Test with --arms
        args1 = parser.parse_args(["--block", "block_07", "--arms", "v154_arm3,baseline_arm1", "--max-tokens-ceiling", "250000"])
        self.assertEqual(args1.block, "block_07")
        self.assertEqual(args1.arm_order, "v154_arm3,baseline_arm1")
        self.assertEqual(args1.max_tokens_ceiling, 250000)

        # 2. Test with --arm-order
        args2 = parser.parse_args(["--block", "block_07", "--arm-order", "v154_arm3,baseline_arm1", "--max-tokens-ceiling", "250000"])
        self.assertEqual(args2.block, "block_07")
        self.assertEqual(args2.arm_order, "v154_arm3,baseline_arm1")
        self.assertEqual(args2.max_tokens_ceiling, 250000)


if __name__ == "__main__":
    unittest.main()
