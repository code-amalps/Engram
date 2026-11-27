"""
Per-Arm Candidate Identity Metadata Regression Suite.
Engram Phase 5.4.11: Tests A through J for per-arm candidate identity resolution,
schema validation, fail-closed safety, and historical telemetry compatibility.
"""
import os
import sys
import json
import tempfile
import hashlib
import subprocess
import unittest
from pathlib import Path
from unittest import mock

# Ensure src is on sys.path
HARNESS_ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = HARNESS_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from orchestrator import (
    Orchestrator,
    ArmCandidateIdentity,
    audit_arm_candidate_identity,
)
from runner import CodexRunner, RunExecutionResult
from isolation import SafetyViolationError
from schema_validator import validate_instance

CAND3_SHA = "97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777"
CAND4_SHA = "FC605BC0CF3B6C9C5FDB1E7C83EE666FD7E049FD9AE25C893D478578681699F8"
CAND4_1_SHA = "300ACE35A31E8E4AF6DD531E8B83B0A5BA9F03E70B27F5E7823E3650CE494015"
CAND4_2_SHA = "78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2"

CAND3_BIN = HARNESS_ROOT / "bin" / "candidate-phase3" / "engram.exe"
CAND4_BIN = HARNESS_ROOT / "bin" / "candidate-phase4" / "engram.exe"
CAND4_1_BIN = HARNESS_ROOT / "bin" / "candidate-phase4.1" / "engram.exe"
CAND4_2_BIN = HARNESS_ROOT / "bin" / "candidate-phase4.2" / "engram.exe"


class TestPerArmCandidateIdentity(unittest.TestCase):
    def setUp(self):
        self.task_config = HARNESS_ROOT / "config" / "tasks" / "task_a_typo.json"
        self.temp_dir = tempfile.TemporaryDirectory()
        self.output_dir = Path(self.temp_dir.name) / "out"
        self.schema_path = HARNESS_ROOT / "schemas" / "run_telemetry.schema.json"
        with open(self.schema_path, "r", encoding="utf-8") as f:
            self.schema = json.load(f)

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_a_native_arm_identity(self):
        """Test A: Verify that all candidate identity fields accurately represent a native arm with no Engram binary or MCP registration."""
        orch = Orchestrator(
            task_config_path=self.task_config,
            pair_id="test_native",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            candidate_version="cand4_2",
        )
        identity = orch.arm_identities["arm_a_native"]
        self.assertEqual(identity.arm, "arm_a_native")
        self.assertEqual(identity.requested_candidate_version, "native")
        self.assertEqual(identity.resolved_candidate_version, "native")
        self.assertIsNone(identity.binary_path)
        self.assertIsNone(identity.expected_sha256)
        self.assertFalse(identity.is_engram)

        # Execute single arm dry-run and inspect emitted isolation metadata
        record = orch._execute_single_arm("arm_a_native", 1)
        iso = record["isolation"]
        self.assertEqual(iso["benchmark_candidate_version"], "cand4_2")
        self.assertEqual(iso["requested_candidate_version"], "native")
        self.assertEqual(iso["resolved_candidate_version"], "native")
        self.assertIsNone(iso["engram_binary_path"])
        self.assertIsNone(iso["engram_binary_expected_sha256"])
        self.assertIsNone(iso["engram_binary_actual_sha256"])
        self.assertEqual(iso["candidate_verification_status"], "NOT_APPLICABLE")
        self.assertFalse(iso["mcp_registration_verified"])
        self.assertEqual(iso["mcp_server_status"], "DISABLED")

    def test_b_candidate_3_arm_identity(self):
        """Test B: Verify that Arm B records benchmark=cand4_2, requested=cand3, resolved=cand3, Candidate 3 binary and hash."""
        orch = Orchestrator(
            task_config_path=self.task_config,
            pair_id="test_cand3",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            candidate_version="cand4_2",
        )
        identity = orch.arm_identities["arm_b_cand3_always_on"]
        self.assertEqual(identity.arm, "arm_b_cand3_always_on")
        self.assertEqual(identity.requested_candidate_version, "cand3")
        self.assertEqual(identity.resolved_candidate_version, "cand3")
        self.assertEqual(identity.binary_path.resolve(), CAND3_BIN.resolve())
        self.assertEqual(identity.expected_sha256, CAND3_SHA)
        self.assertTrue(identity.is_engram)

        record = orch._execute_single_arm("arm_b_cand3_always_on", 1)
        iso = record["isolation"]
        self.assertEqual(iso["benchmark_candidate_version"], "cand4_2")
        self.assertEqual(iso["requested_candidate_version"], "cand3")
        self.assertEqual(iso["resolved_candidate_version"], "cand3")
        self.assertEqual(Path(iso["engram_binary_path"]).resolve(), CAND3_BIN.resolve())
        self.assertEqual(iso["engram_binary_expected_sha256"], CAND3_SHA)
        self.assertEqual(iso["engram_binary_actual_sha256"], CAND3_SHA)
        self.assertEqual(iso["candidate_verification_status"], "VERIFIED")
        self.assertTrue(iso["mcp_registration_verified"])
        self.assertEqual(iso["mcp_server_status"], "ACTIVE")

    def test_c_candidate_4_2_always_on_arm_identity(self):
        """Test C: Verify that Arm C records benchmark=cand4_2, requested=cand4_2, resolved=cand4_2, Candidate 4.2 binary and hash."""
        orch = Orchestrator(
            task_config_path=self.task_config,
            pair_id="test_cand4_2_arm_c",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            candidate_version="cand4_2",
        )
        identity = orch.arm_identities["arm_c_cand4_always_on"]
        self.assertEqual(identity.arm, "arm_c_cand4_always_on")
        self.assertEqual(identity.requested_candidate_version, "cand4_2")
        self.assertEqual(identity.resolved_candidate_version, "cand4_2")
        self.assertEqual(identity.binary_path.resolve(), CAND4_2_BIN.resolve())
        self.assertEqual(identity.expected_sha256, CAND4_2_SHA)
        self.assertTrue(identity.is_engram)

        record = orch._execute_single_arm("arm_c_cand4_always_on", 1)
        iso = record["isolation"]
        self.assertEqual(iso["benchmark_candidate_version"], "cand4_2")
        self.assertEqual(iso["requested_candidate_version"], "cand4_2")
        self.assertEqual(iso["resolved_candidate_version"], "cand4_2")
        self.assertEqual(Path(iso["engram_binary_path"]).resolve(), CAND4_2_BIN.resolve())
        self.assertEqual(iso["engram_binary_expected_sha256"], CAND4_2_SHA)
        self.assertEqual(iso["engram_binary_actual_sha256"], CAND4_2_SHA)
        self.assertEqual(iso["candidate_verification_status"], "VERIFIED")
        self.assertTrue(iso["mcp_registration_verified"])
        self.assertEqual(iso["mcp_server_status"], "ACTIVE")

    def test_d_candidate_4_2_adaptive_arm_identity(self):
        """Test D: Verify that Arm D records the same binary identity as Arm C while retaining correct adaptive metadata."""
        orch = Orchestrator(
            task_config_path=self.task_config,
            pair_id="test_cand4_2_arm_d",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            candidate_version="cand4_2",
        )
        identity = orch.arm_identities["arm_d_cand4_adaptive"]
        self.assertEqual(identity.arm, "arm_d_cand4_adaptive")
        self.assertEqual(identity.requested_candidate_version, "cand4_2")
        self.assertEqual(identity.resolved_candidate_version, "cand4_2")
        self.assertEqual(identity.binary_path.resolve(), CAND4_2_BIN.resolve())
        self.assertEqual(identity.expected_sha256, CAND4_2_SHA)
        self.assertTrue(identity.is_engram)

        record = orch._execute_single_arm("arm_d_cand4_adaptive", 1)
        iso = record["isolation"]
        self.assertEqual(iso["benchmark_candidate_version"], "cand4_2")
        self.assertEqual(iso["requested_candidate_version"], "cand4_2")
        self.assertEqual(iso["resolved_candidate_version"], "cand4_2")
        self.assertEqual(Path(iso["engram_binary_path"]).resolve(), CAND4_2_BIN.resolve())
        self.assertEqual(iso["engram_binary_expected_sha256"], CAND4_2_SHA)
        self.assertEqual(iso["engram_binary_actual_sha256"], CAND4_2_SHA)
        self.assertEqual(iso["candidate_verification_status"], "VERIFIED")
        self.assertTrue(iso["mcp_registration_verified"])

    def test_e_binary_identity_mismatch_fails_closed(self):
        """Test E: Simulate an actual binary hash mismatch and verify fail-closed behavior."""
        corrupted_bin = Path(self.temp_dir.name) / "corrupted_cand3.exe"
        corrupted_bin.write_bytes(b"corrupted binary bytes")

        identity = ArmCandidateIdentity(
            arm="arm_b_cand3_always_on",
            requested_candidate_version="cand3",
            resolved_candidate_version="cand3",
            binary_path=corrupted_bin,
            expected_sha256=CAND3_SHA,
            is_engram=True,
        )

        with self.assertRaises(SafetyViolationError) as ctx:
            identity.verify_binary_integrity()
        self.assertIn("Pre-launch binary hash verification failed", str(ctx.exception))

        # In telemetry assembly, verify that status reports FAILED if hash mismatch occurs
        orch = Orchestrator(
            task_config_path=self.task_config,
            pair_id="test_mismatch",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            candidate_version="cand4_2",
        )
        orch.arm_identities["arm_b_cand3_always_on"] = identity
        with self.assertRaises(SafetyViolationError):
            orch._execute_single_arm("arm_b_cand3_always_on", 1)

    def test_f_path_version_inconsistency_rejected(self):
        """Test F: Simulate Candidate 3 metadata paired with Candidate 4.2 path or hash, and verify rejection."""
        orch = Orchestrator(
            task_config_path=self.task_config,
            pair_id="test_inconsistency",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            candidate_version="cand4_2",
        )

        # Inconsistency 1: Cand 3 version paired with Cand 4.2 binary path
        inconsistent_path_id = ArmCandidateIdentity(
            arm="arm_b_cand3_always_on",
            requested_candidate_version="cand3",
            resolved_candidate_version="cand3",
            binary_path=CAND4_2_BIN,
            expected_sha256=CAND3_SHA,
            is_engram=True,
        )
        with self.assertRaises(SafetyViolationError) as ctx:
            inconsistent_path_id.validate_against_ledger(orch.candidate_ledger)
        self.assertIn("Binary path mismatch", str(ctx.exception))

        # Inconsistency 2: Cand 3 version paired with Cand 4.2 expected SHA
        inconsistent_sha_id = ArmCandidateIdentity(
            arm="arm_b_cand3_always_on",
            requested_candidate_version="cand3",
            resolved_candidate_version="cand3",
            binary_path=CAND3_BIN,
            expected_sha256=CAND4_2_SHA,
            is_engram=True,
        )
        with self.assertRaises(SafetyViolationError) as ctx:
            inconsistent_sha_id.validate_against_ledger(orch.candidate_ledger)
        self.assertIn("Expected SHA-256 mismatch", str(ctx.exception))

        # Inconsistency 3: Native arm paired with an Engram binary
        inconsistent_native_id = ArmCandidateIdentity(
            arm="arm_a_native",
            requested_candidate_version="native",
            resolved_candidate_version="native",
            binary_path=CAND3_BIN,
            expected_sha256=CAND3_SHA,
            is_engram=False,
        )
        with self.assertRaises(SafetyViolationError) as ctx:
            inconsistent_native_id.validate_against_ledger(orch.candidate_ledger)
        self.assertIn("Native arm 'arm_a_native' cannot specify binary path or hash", str(ctx.exception))

    def test_g_missing_candidate_binary_fails_closed(self):
        """Test G: Verify that a missing executable cannot produce a successful resolved identity."""
        missing_bin = Path(self.temp_dir.name) / "nonexistent" / "engram.exe"
        identity = ArmCandidateIdentity(
            arm="arm_b_cand3_always_on",
            requested_candidate_version="cand3",
            resolved_candidate_version="cand3",
            binary_path=missing_bin,
            expected_sha256=CAND3_SHA,
            is_engram=True,
        )
        with self.assertRaises(FileNotFoundError) as ctx:
            identity.verify_binary_integrity()
        self.assertIn("Candidate binary missing", str(ctx.exception))

    def test_h_schema_validation(self):
        """Test H: Verify that all new and existing identity fields satisfy the telemetry JSON schema."""
        orch = Orchestrator(
            task_config_path=self.task_config,
            pair_id="test_schema",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            candidate_version="cand4_2",
        )

        for arm in ["arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"]:
            record = orch._execute_single_arm(arm, 1)
            # Must strictly validate with zero SchemaValidationErrors
            validate_instance(record, self.schema)

            # Check skipped arm schema compliance as well
            skipped = orch._create_skipped_arm_record(arm, 2, "Test skip reason")
            validate_instance(skipped, self.schema)

    def test_i_historical_telemetry_compatibility(self):
        """Test I: Verify historical telemetry records remain valid against schema and are not mutated."""
        reports_runs = HARNESS_ROOT / "reports" / "runs"
        historical_files = [
            reports_runs / "task_b_remember_me_tetrad_live_01" / "arm_a_native" / "run_telemetry.json",
            reports_runs / "task_b_remember_me_tetrad_live_01" / "arm_b_cand3_always_on" / "run_telemetry.json",
            reports_runs / "task_b_remember_me_tetrad_live_01" / "arm_c_cand4_always_on" / "run_telemetry.json",
            reports_runs / "task_b_remember_me_tetrad_live_01" / "arm_d_cand4_adaptive" / "run_telemetry.json",
            reports_runs / "task_b_remember_me_block_06" / "baseline_arm1" / "run_telemetry.json",
        ]

        for p in historical_files:
            if not p.exists():
                continue
            original_bytes = p.read_bytes()
            original_sha = hashlib.sha256(original_bytes).hexdigest()

            data = json.loads(original_bytes.decode("utf-8"))
            # Schema must validate historical files cleanly without error
            validate_instance(data, self.schema)

            # Audit record using forensic helper
            audit = audit_arm_candidate_identity(data)
            self.assertIn("arm", audit)
            self.assertTrue(audit["is_legacy"])
            # Historical records lack new fields; verify they are exposed as None/unavailable rather than guessed
            self.assertIsNone(audit["benchmark_candidate_version"])
            self.assertIsNone(audit["requested_candidate_version"])
            self.assertIsNone(audit["resolved_candidate_version"])

            # Verify historical file was NOT mutated on disk
            after_bytes = p.read_bytes()
            self.assertEqual(hashlib.sha256(after_bytes).hexdigest(), original_sha)

        # Verify audit detects historical metadata defect when simulated
        simulated_defective_record = {
            "arm": "arm_b_cand3_always_on",
            "isolation": {
                "benchmark_candidate_version": "cand4_2",
                "requested_candidate_version": "cand4_2",
                "resolved_candidate_version": "cand4_2",  # Historical bug
                "engram_binary_actual_sha256": CAND3_SHA,
                "engram_binary_expected_sha256": CAND3_SHA,
            }
        }
        defect_audit = audit_arm_candidate_identity(simulated_defective_record)
        self.assertFalse(defect_audit["is_consistent"])
        self.assertTrue(any("resolved_candidate_version='cand4_2'" in inc for inc in defect_audit["inconsistencies"]))

    def test_j_four_arm_integration(self):
        """Test J: Construct one offline tetrad configuration and verify all four arm identities simultaneously."""
        orch = Orchestrator(
            task_config_path=self.task_config,
            pair_id="tetrad_integration_01",
            output_dir=self.output_dir,
            is_dry_run=True,
            dependency_strategy="none",
            candidate_version="cand4_2",
        )

        expected_table = {
            "arm_a_native": {
                "requested": "native",
                "resolved": "native",
                "binary": None,
                "expected_sha": None,
                "is_engram": False,
                "mcp": False,
            },
            "arm_b_cand3_always_on": {
                "requested": "cand3",
                "resolved": "cand3",
                "binary": CAND3_BIN,
                "expected_sha": CAND3_SHA,
                "is_engram": True,
                "mcp": True,
            },
            "arm_c_cand4_always_on": {
                "requested": "cand4_2",
                "resolved": "cand4_2",
                "binary": CAND4_2_BIN,
                "expected_sha": CAND4_2_SHA,
                "is_engram": True,
                "mcp": True,
            },
            "arm_d_cand4_adaptive": {
                "requested": "cand4_2",
                "resolved": "cand4_2",
                "binary": CAND4_2_BIN,
                "expected_sha": CAND4_2_SHA,
                "is_engram": True,
                "mcp": True,
            },
        }

        # 1. Verify orchestrator mapping before execution
        for arm_key, exp in expected_table.items():
            identity = orch.arm_identities[arm_key]
            self.assertEqual(identity.requested_candidate_version, exp["requested"])
            self.assertEqual(identity.resolved_candidate_version, exp["resolved"])
            if exp["binary"]:
                self.assertEqual(identity.binary_path.resolve(), exp["binary"].resolve())
            else:
                self.assertIsNone(identity.binary_path)
            self.assertEqual(identity.expected_sha256, exp["expected_sha"])
            self.assertEqual(identity.is_engram, exp["is_engram"])

        # 2. Run full offline tetrad execution (deterministic dry-run, zero live model calls)
        # Narrowly scoped offline assertion: strictly prevents any external model dispatch
        orig_popen = subprocess.Popen
        def guarded_popen(*args, **kwargs):
            cmd = args[0] if args else kwargs.get("args", [])
            cmd_str = " ".join(str(c) for c in (cmd if isinstance(cmd, (list, tuple)) else [cmd]))
            if "codex" in cmd_str.lower() or "gpt" in cmd_str.lower():
                raise AssertionError(f"Live model dispatch attempted during offline Test J: {cmd_str}")
            return orig_popen(*args, **kwargs)

        with mock.patch("subprocess.Popen", side_effect=guarded_popen):
            tetrad_summary = orch.run_tetrad(block_id="tetrad_integ_block_01")
        runs = tetrad_summary["runs"]
        self.assertEqual(len(runs), 4)

        for arm_key, exp in expected_table.items():
            arm_run = runs[arm_key]
            iso = arm_run["isolation"]
            self.assertEqual(iso["benchmark_candidate_version"], "cand4_2")
            self.assertEqual(iso["requested_candidate_version"], exp["requested"])
            self.assertEqual(iso["resolved_candidate_version"], exp["resolved"])
            if exp["binary"]:
                self.assertEqual(Path(iso["engram_binary_path"]).resolve(), exp["binary"].resolve())
                self.assertEqual(iso["engram_binary_expected_sha256"], exp["expected_sha"])
                self.assertEqual(iso["engram_binary_actual_sha256"], exp["expected_sha"])
                self.assertEqual(iso["candidate_verification_status"], "VERIFIED")
                self.assertTrue(iso["mcp_registration_verified"])
            else:
                self.assertIsNone(iso["engram_binary_path"])
                self.assertIsNone(iso["engram_binary_expected_sha256"])
                self.assertIsNone(iso["engram_binary_actual_sha256"])
                self.assertEqual(iso["candidate_verification_status"], "NOT_APPLICABLE")
                self.assertFalse(iso["mcp_registration_verified"])

        # 3. Verify tetrad report includes dynamic Candidate 4.2 and Candidate 3 labels
        report_file = self.output_dir / "tetrad_report.md"
        self.assertTrue(report_file.exists())
        report_text = report_file.read_text(encoding="utf-8")
        self.assertIn("Candidate 3", report_text)
        self.assertIn("Candidate 4.2", report_text)
        self.assertIn("Native (None)", report_text)


if __name__ == "__main__":
    unittest.main()
