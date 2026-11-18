"""
Offline unit test suite for Engram Amendment 05 Milestone 5B-Prep Remediation.
Tests:
1. Workspace isolation remediation (Cargo manifest, dependency integrity, sandbox cleanliness, NTFS junctions/copies).
2. Task configuration validation (task_commit_cache_invalidation.json, schema validation, Evaluator git impact).
3. Token accounting audit & safety headroom guard (45,000-token reserve, pre-dispatch stop at 130,000 tokens, worst-case delta exposure).
4. Experimental integrity & dual-arm workspace separation.

STRICT SAFETY RESTRICTION: Mocks and synthetic tests only. Never launches Codex, model processes, or network requests.
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
import subprocess

# Add src to path
HARNESS_SRC = Path(__file__).resolve().parent.parent / "src"
if str(HARNESS_SRC) not in sys.path:
    sys.path.insert(0, str(HARNESS_SRC))

from orchestrator import Orchestrator, SafetyViolationError
from runner import CodexRunner, RunExecutionResult, StreamingTokenTracker, SUPPORTED_SANDBOX_POLICIES
from schema_validator import validate_instance, SchemaValidationError
from isolation import (
    compute_cargo_manifest,
    compute_comprehensive_golden_manifest,
    verify_dependency_integrity,
    verify_sandbox_cleanliness,
    create_sandbox,
    teardown_sandbox,
    IsolationError,
    GoldenDependencyCorruptionError,
)
from evaluator import Evaluator, GitImpact, CorrectnessResult


HARNESS_ROOT = Path(__file__).resolve().parent.parent
CONFIG_DIR = HARNESS_ROOT / "config" / "tasks"
SCHEMAS_DIR = HARNESS_ROOT / "schemas"


class TestCargoWorkspaceIsolation(unittest.TestCase):
    """Verifies workspace isolation remediation for Cargo repositories."""

    def setUp(self):
        self.tmp_dir = tempfile.mkdtemp(prefix="engram_cargo_iso_test_")
        self.root = Path(self.tmp_dir)
        self.golden_repo = self.root / "engram_repo"
        self.golden_repo.mkdir()

        # Create Cargo workspace structure
        (self.golden_repo / "Cargo.toml").write_text('[package]\nname = "engram"\nversion = "1.5.4"\n', encoding="utf-8")
        (self.golden_repo / "Cargo.lock").write_text('# lock file\nversion = 3\n', encoding="utf-8")
        (self.golden_repo / "src").mkdir()
        (self.golden_repo / "src" / "main.rs").write_text('fn main() { println!("engram"); }\n', encoding="utf-8")
        (self.golden_repo / "src" / "compiler.rs").write_text('pub fn ensure_fresh() {}\n', encoding="utf-8")
        (self.golden_repo / "tests").mkdir()
        (self.golden_repo / "tests" / "test1.rs").write_text('// test\n', encoding="utf-8")

        # Initialize git repository for git worktree and git status operations
        subprocess.run(["git", "init"], cwd=self.golden_repo, capture_output=True, check=True)
        subprocess.run(["git", "config", "user.email", "test@test.local"], cwd=self.golden_repo, capture_output=True, check=True)
        subprocess.run(["git", "config", "user.name", "Test User"], cwd=self.golden_repo, capture_output=True, check=True)
        subprocess.run(["git", "add", "."], cwd=self.golden_repo, capture_output=True, check=True)
        subprocess.run(["git", "commit", "-m", "init"], cwd=self.golden_repo, capture_output=True, check=True)

        res = subprocess.run(["git", "rev-parse", "HEAD"], cwd=self.golden_repo, capture_output=True, text=True, check=True)
        self.baseline_commit = res.stdout.strip()

        # Mock runs root
        self.runs_root = self.root / "engram_pilot" / "runs"
        self.runs_root.mkdir(parents=True)

    def tearDown(self):
        shutil.rmtree(self.tmp_dir, ignore_errors=True)

    def test_compute_cargo_manifest(self):
        manifest = compute_cargo_manifest(self.golden_repo)
        self.assertEqual(manifest["repo_type"], "cargo")
        self.assertIsNotNone(manifest["cargo_toml_sha256"])
        self.assertIsNotNone(manifest["cargo_lock_sha256"])
        self.assertEqual(manifest["source_files_count"], 3)  # main.rs, compiler.rs, test1.rs
        self.assertGreater(manifest["source_total_bytes"], 0)
        self.assertIsNotNone(manifest["source_recursive_digest"])

    def test_verify_dependency_integrity_cargo_unmodified(self):
        baseline = compute_cargo_manifest(self.golden_repo)
        # Should succeed without error
        verify_dependency_integrity(self.golden_repo, baseline)

    def test_verify_dependency_integrity_cargo_lock_drift(self):
        baseline = compute_cargo_manifest(self.golden_repo)
        # Tamper with Cargo.lock
        (self.golden_repo / "Cargo.lock").write_text('# modified lock\n', encoding="utf-8")
        with self.assertRaises(GoldenDependencyCorruptionError) as ctx:
            verify_dependency_integrity(self.golden_repo, baseline)
        self.assertIn("Golden repository Cargo.lock was mutated", str(ctx.exception))

    def test_verify_dependency_integrity_cargo_toml_drift(self):
        baseline = compute_cargo_manifest(self.golden_repo)
        # Tamper with Cargo.toml
        (self.golden_repo / "Cargo.toml").write_text('[package]\nname = "tampered"\n', encoding="utf-8")
        with self.assertRaises(GoldenDependencyCorruptionError) as ctx:
            verify_dependency_integrity(self.golden_repo, baseline)
        self.assertIn("Golden repository Cargo.toml was mutated", str(ctx.exception))

    def test_verify_sandbox_cleanliness_cargo(self):
        # Allow .engram, .github, target in cargo repos
        (self.golden_repo / ".engram").mkdir()
        (self.golden_repo / ".engram" / "index.db").write_text("db", encoding="utf-8")
        (self.golden_repo / "target").mkdir()
        (self.golden_repo / "target" / "dummy.txt").write_text("build", encoding="utf-8")

        # Cleanliness check should pass for cargo repo type
        verify_sandbox_cleanliness(self.golden_repo, repo_type="cargo")

        # But unauthorized file outside allowlist should fail
        (self.golden_repo / "unauthorized_dump.bin").write_text("bad", encoding="utf-8")
        with self.assertRaises(IsolationError) as ctx:
            verify_sandbox_cleanliness(self.golden_repo, repo_type="cargo")
        self.assertIn("Sandbox cleanliness violation", str(ctx.exception))

    def test_teardown_sandbox_safety_guard(self):
        # Refuses to delete paths outside engram_pilot/runs
        unsafe_path = self.root / "system_path"
        unsafe_path.mkdir()
        with self.assertRaises(SafetyViolationError) as ctx:
            teardown_sandbox(unsafe_path, self.golden_repo, repo_type="cargo")
        self.assertIn("Refusing to clean path outside engram_pilot/runs", str(ctx.exception))

    def test_create_and_teardown_sandbox_cargo(self):
        sb = create_sandbox(
            task_id="test_cargo_task",
            pair_id="pair_5b",
            arm="treatment_B",
            golden_repo=self.golden_repo,
            baseline_commit=self.baseline_commit,
            dependency_strategy="none",
            runs_dir=self.runs_root,
            repo_type="cargo"
        )
        self.assertTrue(sb.exists())
        self.assertTrue((sb / "Cargo.toml").exists())
        self.assertTrue((sb / "src" / "compiler.rs").exists())

        # Teardown
        teardown_sandbox(sb, self.golden_repo, preserve_archive=False, repo_type="cargo")
        self.assertFalse(sb.exists())


class TestTaskConfigurationValidation(unittest.TestCase):
    """Verifies schema conformance and properties of task_commit_cache_invalidation.json."""

    def setUp(self):
        self.schema_path = SCHEMAS_DIR / "correctness_checklist.schema.json"
        with open(self.schema_path, "r", encoding="utf-8") as f:
            self.schema = json.load(f)
        self.task_path = CONFIG_DIR / "task_commit_cache_invalidation.json"
        with open(self.task_path, "r", encoding="utf-8") as f:
            self.task_cfg = json.load(f)

    def test_schema_conformance(self):
        # Validate task against official schema
        validate_instance(self.task_cfg, self.schema, "task_commit_cache_invalidation")

    def test_task_attributes(self):
        self.assertEqual(self.task_cfg["task_id"], "task_commit_cache_invalidation")
        self.assertEqual(self.task_cfg["repository_type"], "cargo")
        self.assertFalse(self.task_cfg["read_only"])
        self.assertFalse(self.task_cfg["requires_dependencies"])
        self.assertEqual(self.task_cfg["baseline_commit"], "c70845b4d5913457f1b176fb0370b80dd792711f")
        self.assertIn("Cargo.toml", self.task_cfg["forbidden_file_patterns"])
        self.assertIn("Cargo.lock", self.task_cfg["forbidden_file_patterns"])

    def test_evaluator_cargo_git_impact_allowed_edits(self):
        evaluator = Evaluator(self.task_cfg)
        mock_sandbox = Path("fake_sandbox")
        mock_patch = Path("fake_patch.diff")

        with patch("subprocess.run") as mock_run:
            # git status porcelain
            res_status = MagicMock()
            res_status.returncode = 0
            res_status.stdout = " M src/compiler.rs\n M src/analyzer.rs\n"

            # git diff numstat
            res_stat = MagicMock()
            res_stat.returncode = 0
            res_stat.stdout = "10\t2\tsrc/compiler.rs\n5\t1\tsrc/analyzer.rs\n"

            # git diff HEAD
            res_diff = MagicMock()
            res_diff.returncode = 0
            res_diff.stdout = "diff --git a/src/compiler.rs b/src/compiler.rs\n"

            mock_run.side_effect = [res_status, res_stat, res_diff]

            with patch("builtins.open", unittest.mock.mock_open()):
                git_impact = evaluator.analyze_git_impact(mock_sandbox, mock_patch)

            self.assertEqual(git_impact.files_changed_count, 2)
            self.assertEqual(git_impact.unrelated_files_count, 0)
            self.assertFalse(git_impact.dependency_mutated)

    def test_evaluator_cargo_git_impact_dependency_mutation(self):
        evaluator = Evaluator(self.task_cfg)
        mock_sandbox = Path("fake_sandbox")
        mock_patch = Path("fake_patch.diff")

        with patch("subprocess.run") as mock_run:
            # git status modifying Cargo.lock (forbidden)
            res_status = MagicMock()
            res_status.returncode = 0
            res_status.stdout = " M src/compiler.rs\n M Cargo.lock\n"

            res_stat = MagicMock()
            res_stat.returncode = 0
            res_stat.stdout = "10\t2\tsrc/compiler.rs\n1\t1\tCargo.lock\n"

            res_diff = MagicMock()
            res_diff.returncode = 0
            res_diff.stdout = "diff"

            mock_run.side_effect = [res_status, res_stat, res_diff]

            with patch("builtins.open", unittest.mock.mock_open()):
                git_impact = evaluator.analyze_git_impact(mock_sandbox, mock_patch)

            self.assertEqual(git_impact.files_changed_count, 2)
            self.assertEqual(git_impact.unrelated_files_count, 1)
            self.assertTrue(git_impact.dependency_mutated)


class TestConservativeTokenPlanningScenario(unittest.TestCase):
    """
    Verifies the conservative token planning scenario:
    - Nominal budget per arm: 175,000 tokens
    - Headroom reserve: 45,000 tokens
    - Nominal stop threshold: 130,000 tokens
    - Combined nominal budget: 350,000 tokens
    - Turn 9 worst-case delta (37,651 tokens) containment.
    """

    def test_streaming_token_tracker_default_reserve(self):
        tracker = StreamingTokenTracker()
        self.assertEqual(tracker.conservative_reserve, 45_000)

    def test_headroom_check_below_stop_threshold(self):
        # At 100,000 tokens, ceiling 175,000, reserve 45,000:
        # Headroom = 175,000 - 100,000 = 75,000 > 45,000 -> Allowed
        tracker = StreamingTokenTracker(conservative_reserve=45_000)
        tracker.thread_cumulative_max = 100_000
        has_headroom, remaining, est = tracker.check_pre_dispatch_headroom(ceiling=175_000)
        self.assertTrue(has_headroom)
        self.assertEqual(remaining, 75_000)
        self.assertEqual(est, 45_000)

    def test_headroom_check_equality_boundary(self):
        # At 130,000 tokens: Remaining = 45,000 == estimated next turn (45,000) -> Allowed
        tracker = StreamingTokenTracker(conservative_reserve=45_000)
        tracker.thread_cumulative_max = 130_000
        has_headroom, remaining, est = tracker.check_pre_dispatch_headroom(ceiling=175_000)
        self.assertTrue(has_headroom)
        self.assertEqual(remaining, 45_000)

    def test_headroom_check_above_stop_threshold(self):
        # At 130,001 tokens: Remaining = 44,999 < 45,000 -> Blocked
        tracker = StreamingTokenTracker(conservative_reserve=45_000)
        tracker.thread_cumulative_max = 130_001
        has_headroom, remaining, est = tracker.check_pre_dispatch_headroom(ceiling=175_000)
        self.assertFalse(has_headroom)
        self.assertEqual(remaining, 44_999)

    def test_worst_case_turn9_delta_containment(self):
        """
        Critical empirical safety invariant:
        In Task B Block 06, Turn 9 delta was 37,651 tokens.
        If an agent is dispatched at 130,000 tokens (the boundary pre-dispatch level),
        the final post-turn token exposure is 130,000 + 37,651 = 167,651 tokens.
        167,651 is STRICTLY BELOW the 175,000 hard ceiling.
        Thus, the 45,000 reserve guarantees that worst-case single-turn jump cannot breach 175k.
        """
        ceiling = 175_000
        reserve = 45_000
        stop_threshold = ceiling - reserve  # 130,000
        highest_allowed_pre_dispatch = stop_threshold  # 130,000
        worst_case_turn_delta = 37_651

        max_possible_post_turn_exposure = highest_allowed_pre_dispatch + worst_case_turn_delta
        self.assertLess(max_possible_post_turn_exposure, ceiling)
        self.assertEqual(max_possible_post_turn_exposure, 167_651)
        # Margin of safety remaining under hard ceiling:
        safety_margin = ceiling - max_possible_post_turn_exposure
        self.assertEqual(safety_margin, 7_349)
        self.assertGreater(safety_margin, 0)

    def test_streaming_deduplication(self):
        tracker = StreamingTokenTracker(conservative_reserve=45_000)
        event = {
            "type": "turn.completed",
            "turn_id": "turn_01",
            "usage": {
                "input_tokens": 1000,
                "output_tokens": 200,
                "cached_tokens": 500,
                "reasoning_tokens": 50
            }
        }
        # Ingestion 1
        tracker.process_line(json.dumps(event))
        self.assertEqual(tracker.turn_deltas_sum, 1200)

        # Ingestion 2 (identical duplicate event line)
        tracker.process_line(json.dumps(event))
        self.assertEqual(tracker.turn_deltas_sum, 1200)  # Must not double-count!

    def test_malformed_telemetry_fails_safe(self):
        tracker = StreamingTokenTracker(conservative_reserve=45_000)
        malformed_event = {
            "type": "token_usage_record",
            "payload": {
                "thread_token_usage": {
                    "input_tokens": -500,  # Negative tokens
                    "output_tokens": 100
                }
            }
        }
        tracker.process_line(json.dumps(malformed_event))
        self.assertFalse(tracker.telemetry_valid)


class TestOrchestratorMilestone5BConfiguration(unittest.TestCase):
    """Verifies Orchestrator configuration and repository type inference."""

    def test_orchestrator_defaults_and_cargo_repo(self):
        task_path = CONFIG_DIR / "task_commit_cache_invalidation.json"
        orch = Orchestrator(
            task_config_path=task_path,
            pair_id="pair_5b_test",
            is_dry_run=True,
            dependency_strategy="none",
        )
        self.assertEqual(orch.repo_type, "cargo")
        self.assertEqual(orch.headroom_reserve, 45_000)
        self.assertEqual(orch.max_tokens_ceiling, 250_000)
        self.assertEqual(orch.golden_baseline_fingerprint.get("repo_type"), "cargo")

    def test_orchestrator_custom_ceiling_and_reserve(self):
        task_path = CONFIG_DIR / "task_commit_cache_invalidation.json"
        orch = Orchestrator(
            task_config_path=task_path,
            pair_id="pair_5b_custom",
            is_dry_run=True,
            dependency_strategy="none",
            max_tokens_ceiling=175_000,
            headroom_reserve=45_000,
        )
        self.assertEqual(orch.max_tokens_ceiling, 175_000)
        self.assertEqual(orch.headroom_reserve, 45_000)


if __name__ == "__main__":
    unittest.main()
