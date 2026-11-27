import os
import sys
import unittest
from unittest import mock
import tempfile
import hashlib
from pathlib import Path

HARNESS_ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = HARNESS_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from orchestrator import Orchestrator
from runner import CodexRunner
from isolation import SafetyViolationError


class TestCandidateSelection(unittest.TestCase):
    def setUp(self):
        self.task_config = HARNESS_ROOT / "config" / "tasks" / "task_a_typo.json"
        self.temp_dir = tempfile.TemporaryDirectory()
        self.output_dir = Path(self.temp_dir.name) / "out"
        self.patcher = mock.patch("orchestrator.compute_comprehensive_golden_manifest", return_value="dummy_fingerprint")
        self.patcher.start()

    def tearDown(self):
        self.patcher.stop()
        self.temp_dir.cleanup()

    def test_explicit_candidate_4_2_selection(self):
        """Verifies that --candidate-version cand4_2 explicitly resolves to candidate-phase4.2 binary and hash."""
        orch = Orchestrator(
            task_config_path=self.task_config,
            pair_id="test_cand4_2",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            candidate_version="cand4_2"
        )
        self.assertEqual(orch.resolved_candidate_version, "cand4_2")
        self.assertEqual(orch.requested_candidate_version, "cand4_2")
        self.assertEqual(orch.arm_expected_hashes["arm_c_cand4_always_on"], "78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2")
        self.assertEqual(orch.arm_expected_hashes["arm_d_cand4_adaptive"], "78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2")
        self.assertTrue(str(orch.arm_binaries["arm_c_cand4_always_on"]).endswith("candidate-phase4.2\\engram.exe"))

    def test_missing_binary_fails_closed(self):
        """Verifies that requesting a missing binary fails closed immediately without silent fallback."""
        fake_bin = Path(self.temp_dir.name) / "nonexistent" / "engram.exe"
        with mock.patch.dict(os.environ, {"ENGRAM_USE_CANDIDATE_4_2": "1"}):
            with mock.patch.object(Path, "exists", return_value=False):
                with self.assertRaises(FileNotFoundError):
                    Orchestrator(
                        task_config_path=self.task_config,
                        pair_id="test_missing",
                        output_dir=self.output_dir,
                        is_dry_run=True,
                        dependency_strategy="none",
                        candidate_version="cand4_2"
                    )

    def test_hash_mismatch_fails_closed(self):
        """Verifies that a binary with an unexpected SHA-256 raises SafetyViolationError immediately."""
        fake_bin_dir = Path(self.temp_dir.name) / "cand4_2_fake"
        fake_bin_dir.mkdir(parents=True, exist_ok=True)
        fake_bin = fake_bin_dir / "engram.exe"
        fake_bin.write_bytes(b"corrupted binary content")

        with mock.patch.object(
            Path, "read_bytes", return_value=b"corrupted binary content"
        ):
            with self.assertRaises(SafetyViolationError):
                Orchestrator(
                    task_config_path=self.task_config,
                    pair_id="test_hash_mismatch",
                    output_dir=self.output_dir,
                    is_dry_run=True,
                    dependency_strategy="none",
                    candidate_version="cand4_2"
                )

    def test_invalid_candidate_version_fails_closed(self):
        """Verifies that an unknown version string raises ValueError immediately."""
        with self.assertRaises(ValueError) as ctx:
            Orchestrator(
                task_config_path=self.task_config,
                pair_id="test_invalid",
                output_dir=self.output_dir,
                is_dry_run=True,
                dependency_strategy="none",
                candidate_version="candidate_99"
            )
        self.assertIn("Invalid candidate version", str(ctx.exception))

    def test_conflicting_legacy_env_fails_closed(self):
        """Verifies that conflicting explicit candidate and legacy env vars fail closed with ValueError."""
        with mock.patch.dict(os.environ, {"ENGRAM_USE_CANDIDATE_4_2": "1"}):
            with self.assertRaises(ValueError) as ctx:
                Orchestrator(
                    task_config_path=self.task_config,
                    pair_id="test_conflict",
                    output_dir=self.output_dir,
                    is_dry_run=True,
                    dependency_strategy="none",
                    candidate_version="cand4"
                )
            self.assertIn("Conflicting configuration", str(ctx.exception))

    def test_native_arm_rejects_mcp_registration(self):
        """Verifies that native arm rejects unexpected MCP executable configuration."""
        runner = CodexRunner(codex_executable=HARNESS_ROOT / "bin" / "dummy_codex.exe")
        cand_bin = HARNESS_ROOT / "bin" / "candidate-phase4.2" / "engram.exe"
        with self.assertRaises(SafetyViolationError) as ctx:
            runner.build_codex_command(
                sandbox_path=Path(self.temp_dir.name),
                prompt="test prompt",
                arm="arm_a_native",
                engram_executable=cand_bin
            )
        self.assertIn("Native arm 'arm_a_native' received an engram_executable", str(ctx.exception))

    def test_candidate_arm_verifies_mcp_registration(self):
        """Verifies that candidate arm successfully configures active MCP commands in codex invocation."""
        runner = CodexRunner(codex_executable=HARNESS_ROOT / "bin" / "dummy_codex.exe")
        cand_bin = HARNESS_ROOT / "bin" / "candidate-phase4.2" / "engram.exe"
        cmd = runner.build_codex_command(
            sandbox_path=Path(self.temp_dir.name),
            prompt="test prompt",
            arm="arm_c_cand4_always_on",
            engram_executable=cand_bin
        )
        self.assertTrue(any("mcp_servers.engram.command" in arg for arg in cmd))
        self.assertTrue(any("engram_get_context" in arg for arg in cmd))

    def test_regression_reproduce_phase_5_4_7_failure(self):
        """
        Reproduces the exact defect from Phase 5.4.7:
        When legacy env var assignment syntax fails in PowerShell, ENGRAM_USE_CANDIDATE_4_2 was unset,
        causing legacy orchestrator to silently fall back to Candidate 4.
        With the new interface, explicit --candidate-version cand4_2 guarantees Candidate 4.2.
        """
        # Simulate legacy execution where env var was empty
        with mock.patch.dict(os.environ, {}, clear=True):
            orch_legacy = Orchestrator(
                task_config_path=self.task_config,
                pair_id="test_legacy",
                output_dir=self.output_dir,
                is_dry_run=True,
                dependency_strategy="none",
                candidate_version=None
            )
            # Legacy without flag resolved to cand4
            self.assertEqual(orch_legacy.resolved_candidate_version, "cand4")
            self.assertEqual(orch_legacy.arm_expected_hashes["arm_c_cand4_always_on"], "FC605BC0CF3B6C9C5FDB1E7C83EE666FD7E049FD9AE25C893D478578681699F8")

        # Now with hardened explicit flag:
        orch_hardened = Orchestrator(
            task_config_path=self.task_config,
            pair_id="test_hardened",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            candidate_version="cand4_2"
        )
        self.assertEqual(orch_hardened.resolved_candidate_version, "cand4_2")
        self.assertEqual(orch_hardened.arm_expected_hashes["arm_c_cand4_always_on"], "78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2")


if __name__ == "__main__":
    unittest.main()
