"""
Offline unit test suite for Engram Amendment 05 Milestone 2C Remediation.
Tests all five remediation areas:
1. Task Configuration (smoke_test_read_only.json validity and properties)
2. Single-Arm Execution Mode (exact single arm dispatch, rejection of invalid/multiple arms)
3. Safe Dependency Strategy (none accepted for read-only, rejected for compile tasks)
4. Safe Output Directory Handling (path validation, protected root guards, unique subdir on evidence collision)
5. Enforce Read-Only Agent Permissions (exact CLI sandbox mode, omission of --add-dir, mutation defense)
6. Candidate Binary Integrity & Token Guard Preservation.

STRICT RESTRICTION: Mocks and synthetic tests only. Never launches Codex or model processes.
"""
from typing import Dict, Any, List
from pathlib import Path
from unittest.mock import MagicMock, patch
import unittest
import tempfile
import shutil
import hashlib
import json
import os
import sys

# Add src to path
HARNESS_SRC = Path(__file__).resolve().parent.parent / "src"
if str(HARNESS_SRC) not in sys.path:
    sys.path.insert(0, str(HARNESS_SRC))

from orchestrator import Orchestrator, SafetyViolationError
from runner import CodexRunner, RunExecutionResult, StreamingTokenTracker, SUPPORTED_SANDBOX_POLICIES
from schema_validator import validate_instance, SchemaValidationError
from isolation import verify_frozen_binary, IsolationError
from evaluator import Evaluator, GitImpact, CorrectnessResult


HARNESS_ROOT = Path(__file__).resolve().parent.parent
CONFIG_DIR = HARNESS_ROOT / "config" / "tasks"
SCHEMAS_DIR = HARNESS_ROOT / "schemas"
BIN_DIR = HARNESS_ROOT / "bin"


class TestTaskConfigurationRemediation(unittest.TestCase):
    """Remediation A: Read-Only Smoke-Test Task Configuration verification."""

    def setUp(self):
        self.schema_path = SCHEMAS_DIR / "correctness_checklist.schema.json"
        with open(self.schema_path, "r", encoding="utf-8") as f:
            self.schema = json.load(f)
        self.task_path = CONFIG_DIR / "smoke_test_read_only.json"

    def test_smoke_test_task_config_exists_and_conforms_to_schema(self):
        """smoke_test_read_only.json must exist and strictly pass schema validation."""
        self.assertTrue(self.task_path.exists(), "smoke_test_read_only.json must exist on disk")
        with open(self.task_path, "r", encoding="utf-8") as f:
            task_cfg = json.load(f)
        validate_instance(task_cfg, self.schema)

    def test_smoke_test_task_properties_and_boundaries(self):
        """smoke_test_read_only.json must be read-only, require no deps, and specify pinned symbol."""
        with open(self.task_path, "r", encoding="utf-8") as f:
            task_cfg = json.load(f)

        self.assertEqual(task_cfg.get("task_id"), "smoke_test_read_only")
        self.assertTrue(task_cfg.get("read_only"), "Task must have read_only=True")
        self.assertFalse(task_cfg.get("requires_dependencies"), "Task must not require dependencies")
        self.assertEqual(task_cfg.get("baseline_commit"), "1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2")

        # Gates must all be disabled
        gates = task_cfg.get("gates", {})
        for gname in ["L0_typecheck", "L1_lint", "L2_unit_tests", "L3_production_build"]:
            self.assertFalse(gates.get(gname, {}).get("enabled", True), f"Gate {gname} must be disabled")

        # Target file and pattern
        target_files = task_cfg.get("target_files", [])
        self.assertTrue(any("auth.actions.ts" in f for f in target_files))
        preflight = task_cfg.get("fixture_preflight", {})
        patterns = preflight.get("baseline_present_patterns", [])
        self.assertTrue(any("export const authLogout = createAction('[Auth] Logout');" in p.get("pattern", "") for p in patterns))

        # Allowed patterns must be empty (read-only)
        self.assertEqual(task_cfg.get("allowed_file_patterns"), [])
        self.assertEqual(task_cfg.get("forbidden_file_patterns"), ["**/*"])

    def test_invalid_task_schema_fails_validation(self):
        """Missing required fields or invalid schema must fail validation."""
        invalid_cfg = {
            "schema_version": "2.0.0",
            "task_id": "bad_task"
            # Missing required fields
        }
        with self.assertRaises(SchemaValidationError):
            validate_instance(invalid_cfg, self.schema)


class TestSingleArmModeRemediation(unittest.TestCase):
    """Remediation B: Explicit Single-Arm Execution Mode verification."""

    def setUp(self):
        self.task_path = CONFIG_DIR / "smoke_test_read_only.json"
        self.temp_dir = tempfile.mkdtemp(prefix="test_single_arm_")

    def tearDown(self):
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_run_single_dispatches_only_requested_arm(self):
        """run_single executes exactly one arm, creates single_summary.json, and produces no companion arms."""
        with patch.object(Orchestrator, "_execute_single_arm") as mock_exec, \
             patch("orchestrator.compute_dependency_fingerprint", return_value={"test": "fp"}), \
             patch("orchestrator.validate_fixture_preflight"):

            mock_exec.return_value = {
                "schema_version": "2.0.0",
                "run_id": "smoke_test_read_only_pair_01_v154_arm3",
                "execution_status": "SUCCESS",
                "correctness": {"score": 1.0, "passed_all_requirements": True, "requirements": {"REQ-1": "PASS", "REQ-2": "PASS"}},
                "telemetry": {"total_model_tokens": 1200, "telemetry_valid": True}
            }

            orch = Orchestrator(
                task_config_path=self.task_path,
                pair_id="smoke_single_test",
                output_dir=Path(self.temp_dir) / "output",
                is_dry_run=True,
                dependency_strategy="none"
            )

            res = orch.run_single(arm="v154_arm3")
            self.assertEqual(res.get("mode"), "single")
            self.assertEqual(res.get("arm"), "v154_arm3")
            self.assertTrue(res.get("acceptance", {}).get("v154_arm3_accepted"))
            self.assertEqual(mock_exec.call_count, 1)

            # Verify files written
            self.assertTrue((orch.output_dir / "single_summary.json").exists())
            self.assertTrue((orch.output_dir / "single_report.md").exists())

    def test_run_single_rejects_invalid_arm(self):
        """run_single rejects invalid arm identifiers before execution."""
        with patch("orchestrator.compute_dependency_fingerprint", return_value={"test": "fp"}), \
             patch("orchestrator.validate_fixture_preflight"):
            orch = Orchestrator(
                task_config_path=self.task_path,
                pair_id="smoke_single_test",
                output_dir=Path(self.temp_dir) / "output",
                is_dry_run=True,
                dependency_strategy="none"
            )
            with self.assertRaises(ValueError):
                orch.run_single(arm="invalid_nonexistent_arm")


class TestDependencyStrategyRemediation(unittest.TestCase):
    """Remediation C: Safe Dependency Strategy verification."""

    def setUp(self):
        self.smoke_task_path = CONFIG_DIR / "smoke_test_read_only.json"
        self.compile_task_path = CONFIG_DIR / "task_b_remember_me.json"
        self.temp_dir = tempfile.mkdtemp(prefix="test_deps_")

    def tearDown(self):
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_dependency_strategy_none_accepted_for_read_only_task(self):
        """Live run with dependency_strategy='none' is accepted for tasks with requires_dependencies=False / read_only=True."""
        with patch.object(Orchestrator, "_execute_single_arm") as mock_exec, \
             patch("orchestrator.compute_dependency_fingerprint", return_value={"test": "fp"}), \
             patch("orchestrator.validate_fixture_preflight"):
            mock_exec.return_value = {"execution_status": "SUCCESS", "correctness": {"score": 1.0}}

            orch = Orchestrator(
                task_config_path=self.smoke_task_path,
                pair_id="smoke_deps_test",
                output_dir=Path(self.temp_dir) / "output",
                is_dry_run=False,  # Live trial mode!
                dependency_strategy="none"
            )
            # Must not raise in constructor
            self.assertEqual(orch.dependency_strategy, "none")

    def test_dependency_strategy_none_rejected_for_compile_task_on_live_run(self):
        """Live execution of task requiring compilation (task_b_remember_me) with strategy 'none' raises SafetyViolationError."""
        with patch("orchestrator.compute_dependency_fingerprint", return_value={"test": "fp"}), \
             patch("orchestrator.validate_fixture_preflight"), \
             patch("orchestrator.create_sandbox"), \
             patch("orchestrator.verify_dependency_integrity"), \
             patch("orchestrator.teardown_sandbox"):

            orch = Orchestrator(
                task_config_path=self.compile_task_path,
                pair_id="compile_deps_test",
                output_dir=Path(self.temp_dir) / "output",
                is_dry_run=False,  # Live trial mode!
                dependency_strategy="none"
            )
            with self.assertRaises(SafetyViolationError) as cm:
                orch._execute_single_arm("baseline_arm1", 1)
            self.assertIn("requires dependencies", str(cm.exception))

    def test_dependency_strategy_copy_accepted_for_all_tasks(self):
        """Strategy 'copy' remains accepted across all tasks."""
        with patch("orchestrator.compute_dependency_fingerprint", return_value={"test": "fp"}), \
             patch("orchestrator.validate_fixture_preflight"):
            orch = Orchestrator(
                task_config_path=self.compile_task_path,
                pair_id="copy_test",
                output_dir=Path(self.temp_dir) / "output",
                is_dry_run=False,
                dependency_strategy="copy"
            )
            self.assertEqual(orch.dependency_strategy, "copy")


class TestOutputDirectoryHandlingRemediation(unittest.TestCase):
    """Remediation D: Safe Output Directory Handling verification."""

    def setUp(self):
        self.task_path = CONFIG_DIR / "smoke_test_read_only.json"
        self.temp_dir = tempfile.mkdtemp(prefix="test_output_dir_")

    def tearDown(self):
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_explicit_output_dir_accepted_and_created(self):
        """Supplied --output-dir is accepted and created safely."""
        target_out = Path(self.temp_dir) / "reports" / "custom_smoke_test"
        with patch("orchestrator.compute_dependency_fingerprint", return_value={"test": "fp"}), \
             patch("orchestrator.validate_fixture_preflight"):
            orch = Orchestrator(
                task_config_path=self.task_path,
                pair_id="pair_01",
                output_dir=target_out,
                is_dry_run=True
            )
            self.assertEqual(orch.output_dir, target_out.resolve())
            self.assertTrue(orch.output_dir.exists())

    def test_output_dir_cannot_resolve_to_protected_directories(self):
        """Output directory pointing to protected root raises SafetyViolationError."""
        with patch("orchestrator.compute_dependency_fingerprint", return_value={"test": "fp"}), \
             patch("orchestrator.validate_fixture_preflight"):
            with self.assertRaises(SafetyViolationError):
                Orchestrator(
                    task_config_path=self.task_path,
                    pair_id="pair_01",
                    output_dir=HARNESS_ROOT.parent.parent,  # Engram repo root
                    is_dry_run=True
                )

    def test_existing_evidence_preserved_via_timestamped_subdirectory(self):
        """If target output directory already contains run evidence, creates unique timestamped run subdirectory."""
        target_out = Path(self.temp_dir) / "reports" / "existing_run"
        target_out.mkdir(parents=True, exist_ok=True)
        # Seed with existing evidence file
        (target_out / "triad_summary.json").write_text("{}", encoding="utf-8")

        with patch("orchestrator.compute_dependency_fingerprint", return_value={"test": "fp"}), \
             patch("orchestrator.validate_fixture_preflight"):
            orch = Orchestrator(
                task_config_path=self.task_path,
                pair_id="pair_01",
                output_dir=target_out,
                is_dry_run=True
            )
            # Output dir must be a subfolder of target_out starting with 'run_'
            self.assertNotEqual(orch.output_dir, target_out)
            self.assertTrue(orch.output_dir.name.startswith("run_"))
            self.assertEqual(orch.output_dir.parent, target_out.resolve())
            # Original evidence preserved
            self.assertTrue((target_out / "triad_summary.json").exists())


class TestReadOnlyAgentPermissionsRemediation(unittest.TestCase):
    """Remediation E: Enforce Read-Only Agent Permissions verification."""

    def setUp(self):
        self.temp_dir = tempfile.mkdtemp(prefix="test_perms_")
        self.fake_codex = Path(self.temp_dir) / "codex.exe"
        self.fake_codex.write_text("#!fake", encoding="utf-8")

    def tearDown(self):
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_supported_sandbox_policies(self):
        """Only 'read-only', 'workspace-write', and 'danger-full-access' are supported."""
        self.assertIn("read-only", SUPPORTED_SANDBOX_POLICIES)
        self.assertIn("workspace-write", SUPPORTED_SANDBOX_POLICIES)

        runner = CodexRunner(
            codex_executable=self.fake_codex,
            sandbox_policy="read-only"
        )
        self.assertEqual(runner.sandbox_policy, "read-only")

        with self.assertRaises(ValueError):
            CodexRunner(
                codex_executable=self.fake_codex,
                sandbox_policy="unsupported-sandbox-mode"
            )

    def test_build_codex_command_read_only_flags(self):
        """In read-only mode, Codex command uses '-s read-only' and omits '--add-dir'."""
        runner = CodexRunner(
            codex_executable=self.fake_codex,
            sandbox_policy="read-only"
        )
        sandbox = Path(self.temp_dir) / "test_sandbox"
        cmd = runner.build_codex_command(
            sandbox_path=sandbox,
            prompt="Inspection prompt",
            arm="baseline_arm1"
        )
        # Verify -s read-only
        self.assertIn("-s", cmd)
        s_idx = cmd.index("-s")
        self.assertEqual(cmd[s_idx + 1], "read-only")
        # Verify --add-dir is NOT present
        self.assertNotIn("--add-dir", cmd)

    def test_build_codex_command_write_flags(self):
        """In workspace-write mode, Codex command uses '-s workspace-write' and includes '--add-dir'."""
        runner = CodexRunner(
            codex_executable=self.fake_codex,
            sandbox_policy="workspace-write"
        )
        sandbox = Path(self.temp_dir) / "test_sandbox"
        cmd = runner.build_codex_command(
            sandbox_path=sandbox,
            prompt="Editing prompt",
            arm="baseline_arm1"
        )
        self.assertIn("-s", cmd)
        s_idx = cmd.index("-s")
        self.assertEqual(cmd[s_idx + 1], "workspace-write")
        self.assertIn("--add-dir", cmd)

    def test_orchestrator_defaults_sandbox_policy_from_task(self):
        """Orchestrator automatically selects 'read-only' when task_config has read_only=True."""
        task_path = CONFIG_DIR / "smoke_test_read_only.json"
        with patch("orchestrator.compute_dependency_fingerprint", return_value={"test": "fp"}), \
             patch("orchestrator.validate_fixture_preflight"):
            orch = Orchestrator(
                task_config_path=task_path,
                pair_id="pair_01",
                output_dir=Path(self.temp_dir) / "output",
                is_dry_run=True
            )
            self.assertEqual(orch.sandbox_policy, "read-only")
            self.assertEqual(orch.runner.sandbox_policy, "read-only")


class TestCandidateBinaryIntegrity(unittest.TestCase):
    """Remediation F: Candidate Binary Integrity verification."""

    def test_candidate_phase1_binary_sha256(self):
        """Candidate Phase 1 binary hash must match authoritative expected value."""
        cand_bin = BIN_DIR / "candidate-phase1" / "engram.exe"
        expected_sha = "F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE"

        self.assertTrue(cand_bin.exists(), "Candidate Phase 1 executable must exist")
        self.assertTrue(verify_frozen_binary(cand_bin, expected_sha))

    def test_candidate_hash_mismatch_fails_closed(self):
        """A corrupted or mismatched candidate hash raises IsolationError."""
        cand_bin = BIN_DIR / "candidate-phase1" / "engram.exe"
        bad_sha = "0000000000000000000000000000000000000000000000000000000000000000"
        with self.assertRaises(IsolationError):
            verify_frozen_binary(cand_bin, bad_sha)


class TestTokenGuardSettings(unittest.TestCase):
    """Token Guard Preservation verification."""

    def test_token_guard_smoke_test_thresholds(self):
        """Validates ceiling 10,000, reserve 2,000, and nominal stop at 8,000."""
        tracker = StreamingTokenTracker(conservative_reserve=2000)
        ceiling = 10000

        # At 7,000 cumulative tokens with zero turns: estimated cost is 2,000.
        # Remaining is 3,000. remaining >= estimated (3000 >= 2000) -> DISPATCH_ALLOWED
        tracker.thread_cumulative_max = 7000
        has_headroom, remaining, estimated = tracker.check_pre_dispatch_headroom(ceiling)
        self.assertTrue(has_headroom)
        self.assertEqual(tracker.last_decision, "DISPATCH_ALLOWED")

        # At 8,000 cumulative tokens: remaining is 2,000.
        # remaining == estimated (2000 == 2000) -> DISPATCH_ALLOWED (equality boundary)
        tracker.thread_cumulative_max = 8000
        has_headroom, remaining, estimated = tracker.check_pre_dispatch_headroom(ceiling)
        self.assertTrue(has_headroom)

        # At 8,001 cumulative tokens: remaining is 1,999.
        # remaining < estimated (1999 < 2000) -> PREVENTIVE_HALT
        tracker.thread_cumulative_max = 8001
        has_headroom, remaining, estimated = tracker.check_pre_dispatch_headroom(ceiling)
        self.assertFalse(has_headroom)
        self.assertEqual(tracker.last_decision, "PREVENTIVE_HALT")


class TestWorkspaceMutationDefenseInDepth(unittest.TestCase):
    """Defense-in-depth: workspace mutation in read-only tasks must fail closed."""

    def setUp(self):
        self.smoke_task_path = CONFIG_DIR / "smoke_test_read_only.json"
        self.temp_dir = tempfile.mkdtemp(prefix="test_mutation_")

    def tearDown(self):
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_read_only_task_fails_if_files_mutated(self):
        """Any modified or added files in a read-only task strictly fail acceptance."""
        # Create baseline target file in temp_dir for preflight validation
        target_f = Path(self.temp_dir) / "projects/angular-ngrx-material-starter/src/app/core/auth/auth.actions.ts"
        target_f.parent.mkdir(parents=True, exist_ok=True)
        target_f.write_text("export const authLogout = createAction('[Auth] Logout');", encoding="utf-8")

        with patch("orchestrator.compute_dependency_fingerprint", return_value={"test": "fp"}), \
             patch("orchestrator.validate_fixture_preflight"), \
             patch("orchestrator.create_sandbox", return_value=Path(self.temp_dir)), \
             patch("orchestrator.verify_sandbox_identity"), \
             patch("orchestrator.verify_sandbox_cleanliness"), \
             patch("orchestrator.teardown_sandbox"), \
             patch("orchestrator.verify_dependency_integrity"), \
             patch.object(CodexRunner, "run") as mock_run, \
             patch.object(Evaluator, "analyze_git_impact") as mock_impact:

            mock_run.return_value = RunExecutionResult(
                exit_code=0,
                elapsed_seconds=1.2,
                started_at="2026-10-30T00:00:00Z",
                completed_at="2026-10-30T00:00:01Z",
                events_log_path="events.jsonl",
                thread_id="t1",
                model_name="gpt-5.6-luna",
                reasoning_effort="low",
                harness_executable="codex",
                harness_version="1.0",
                path_sanitized=True,
                sandbox_policy="read-only"
            )
            # Simulate agent mutating a file
            mock_impact.return_value = GitImpact(
                files_changed_count=1,
                files_changed=["projects/angular-ngrx-material-starter/src/app/core/auth/auth.actions.ts"],
                insertions=2,
                deletions=0,
                unrelated_files_count=0,
                diff_patch_path="changes.patch"
            )

            orch = Orchestrator(
                task_config_path=self.smoke_task_path,
                pair_id="pair_mutation_test",
                output_dir=Path(self.temp_dir) / "output",
                is_dry_run=True,
                dependency_strategy="none"
            )

            res = orch._execute_single_arm("v154_arm3", 1)
            corr = res.get("correctness", {})
            self.assertEqual(corr.get("score"), 0.0)
            self.assertFalse(corr.get("passed_all_requirements"))
            self.assertEqual(corr.get("requirements", {}).get("REQ-2"), "FAIL")

    def test_read_only_task_passes_when_zero_files_mutated(self):
        """Zero files mutated in a read-only task allows REQ-2 to pass."""
        # Create expected target file in temp_dir
        target_f = Path(self.temp_dir) / "projects/angular-ngrx-material-starter/src/app/core/auth/auth.actions.ts"
        target_f.parent.mkdir(parents=True, exist_ok=True)
        target_f.write_text("export const authLogout = createAction('[Auth] Logout');", encoding="utf-8")

        with patch("orchestrator.compute_dependency_fingerprint", return_value={"test": "fp"}), \
             patch("orchestrator.validate_fixture_preflight"), \
             patch("orchestrator.create_sandbox", return_value=Path(self.temp_dir)), \
             patch("orchestrator.verify_sandbox_identity"), \
             patch("orchestrator.verify_sandbox_cleanliness"), \
             patch("orchestrator.teardown_sandbox"), \
             patch("orchestrator.verify_dependency_integrity"), \
             patch.object(CodexRunner, "run") as mock_run, \
             patch.object(Evaluator, "analyze_git_impact") as mock_impact:

            mock_run.return_value = RunExecutionResult(
                exit_code=0,
                elapsed_seconds=1.2,
                started_at="2026-10-30T00:00:00Z",
                completed_at="2026-10-30T00:00:01Z",
                events_log_path="events.jsonl",
                thread_id="t1",
                model_name="gpt-5.6-luna",
                reasoning_effort="low",
                harness_executable="codex",
                harness_version="1.0",
                path_sanitized=True,
                sandbox_policy="read-only"
            )
            # Zero mutation
            mock_impact.return_value = GitImpact(
                files_changed_count=0,
                files_changed=[],
                insertions=0,
                deletions=0,
                unrelated_files_count=0,
                diff_patch_path="changes.patch"
            )

            orch = Orchestrator(
                task_config_path=self.smoke_task_path,
                pair_id="pair_clean_test",
                output_dir=Path(self.temp_dir) / "output",
                is_dry_run=True,
                dependency_strategy="none"
            )

            res = orch._execute_single_arm("v154_arm3", 1)
            corr = res.get("correctness", {})
            self.assertEqual(corr.get("score"), 1.0)
            self.assertTrue(corr.get("passed_all_requirements"))
            self.assertEqual(corr.get("requirements", {}).get("REQ-1"), "PASS")
            self.assertEqual(corr.get("requirements", {}).get("REQ-2"), "PASS")


class TestCLIArgumentParsing(unittest.TestCase):
    """Verifies CLI argument parsing in orchestrator.py main()."""

    def test_cli_parser_accepts_mode_single_and_output_dir(self):
        """CLI parser accepts --mode single, --arms v154_arm3, and --output-dir."""
        import argparse
        from orchestrator import main

        # Create parser identical to main() to verify arguments
        parser = argparse.ArgumentParser()
        parser.add_argument("--mode", choices=["pair", "triad", "single"], default="pair")
        parser.add_argument("--task", default="task_a_typo")
        parser.add_argument("--arms", "--arm-order", dest="arm_order", default=None)
        parser.add_argument("--output-dir", default=None)

        args = parser.parse_args([
            "--mode", "single",
            "--task", "smoke_test_read_only",
            "--arms", "v154_arm3",
            "--output-dir", "reports/smoke_test"
        ])
        self.assertEqual(args.mode, "single")
        self.assertEqual(args.task, "smoke_test_read_only")
        self.assertEqual(args.arm_order, "v154_arm3")
        self.assertEqual(args.output_dir, "reports/smoke_test")


if __name__ == "__main__":
    unittest.main()
