"""
Unit and Integration Test Suite for Milestone 5C.
Verifies:
1. Phase C: Binary selection integrity, expected hash enforcement, and fail-closed security.
2. Phase D: Task-specific acceptance checks for task_commit_cache_invalidation (REQ-1..4).
3. Positive and negative test cases for SQLite metadata, drift detection, and clean --cache.
4. Correctness scoring C=1.0 for valid implementations and C<1.0 for invalid implementations.
5. Phase E: Mutual acceptance rule invariants (baseline failure, treatment failure, invalid telemetry).

STRICT OFFLINE TEST: Mocks and synthetic fixtures only. Never launches model inference or network calls.
"""
from typing import Dict, Any
from pathlib import Path
from unittest.mock import patch
import unittest
import tempfile
import sqlite3
import shutil
import hashlib
import json
import sys
import os

HARNESS_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(HARNESS_ROOT / "src"))

from evaluator import Evaluator, GitImpact, CorrectnessResult
from isolation import (
    verify_frozen_binary,
    IsolationError,
    FROZEN_ENGRAM_V151_SHA256,
    FROZEN_ENGRAM_V154_SHA256,
)
from orchestrator import (
    Orchestrator,
    DEFAULT_ENGRAM_BIN,
)


class TestBinarySelectionIntegrity(unittest.TestCase):
    """Phase C: Verifies Candidate binary selection, SHA-256 verification, and fail-closed behavior."""

    def setUp(self):
        self.temp_dir = Path(tempfile.mkdtemp(prefix="engram_m5c_bin_"))
        self.candidate_bin = self.temp_dir / "candidate_engram.exe"
        self.candidate_content = b"CANDIDATE_PHASE_1_BINARY_SIMULATION_BYTES_v20261030"
        self.candidate_bin.write_bytes(self.candidate_content)
        self.candidate_sha = hashlib.sha256(self.candidate_content).hexdigest()

        self.frozen_v154_bin = self.temp_dir / "frozen_v154_engram.exe"
        self.frozen_v154_content = b"FROZEN_V154_BINARY_SIMULATION_BYTES_REFERENCE_RELEASE"
        self.frozen_v154_bin.write_bytes(self.frozen_v154_content)
        self.frozen_v154_sha = hashlib.sha256(self.frozen_v154_content).hexdigest()

    def tearDown(self):
        if self.temp_dir.exists():
            shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_correct_candidate_hash_passes(self):
        """Validating the candidate binary against its exact SHA-256 succeeds."""
        result = verify_frozen_binary(self.candidate_bin, self.candidate_sha)
        self.assertTrue(result)

    def test_incorrect_expected_hash_fails_closed(self):
        """Passing an incorrect expected hash raises IsolationError and fails closed."""
        wrong_sha = "0000000000000000000000000000000000000000000000000000000000000000"
        with self.assertRaises(IsolationError) as ctx:
            verify_frozen_binary(self.candidate_bin, wrong_sha)
        self.assertIn("Frozen binary hash mismatch", str(ctx.exception))

    def test_missing_executable_fails_closed(self):
        """Validating a non-existent binary path raises IsolationError."""
        missing_bin = self.temp_dir / "non_existent_engram.exe"
        with self.assertRaises(IsolationError) as ctx:
            verify_frozen_binary(missing_bin, self.candidate_sha)
        self.assertIn("Frozen binary not found", str(ctx.exception))

    def test_wrong_binary_path_fails_closed(self):
        """Validating a wrong binary against candidate hash fails closed."""
        arbitrary_bin = self.temp_dir / "unrelated_tool.exe"
        arbitrary_bin.write_bytes(b"RANDOM_TOOL_BYTES")
        with self.assertRaises(IsolationError) as ctx:
            verify_frozen_binary(arbitrary_bin, self.candidate_sha)
        self.assertIn("Frozen binary hash mismatch", str(ctx.exception))

    def test_accidental_frozen_v154_selection_fails_closed(self):
        """Accidentally supplying frozen v1.5.4 when candidate hash is expected fails closed."""
        with self.assertRaises(IsolationError) as ctx:
            verify_frozen_binary(self.frozen_v154_bin, self.candidate_sha)
        self.assertIn("Frozen binary hash mismatch", str(ctx.exception))

    def test_orchestrator_default_binary_mapping(self):
        """Verifies that DEFAULT_ENGRAM_BIN maps to target/release/engram.exe and resolves for Treatment B."""
        self.assertTrue(DEFAULT_ENGRAM_BIN.is_absolute())
        self.assertEqual(DEFAULT_ENGRAM_BIN.name, "engram.exe")


class TestCommitCacheEvaluatorRequirements(unittest.TestCase):
    """Phase D: Task-specific acceptance checks for task_commit_cache_invalidation."""

    def setUp(self):
        self.temp_dir = Path(tempfile.mkdtemp(prefix="engram_m5c_eval_"))
        self.config_path = HARNESS_ROOT / "config" / "tasks" / "task_commit_cache_invalidation.json"
        with open(self.config_path, "r", encoding="utf-8") as f:
            self.task_cfg = json.load(f)
        self.evaluator = Evaluator(self.task_cfg)

        # Create standard cargo repository layout in temp sandbox
        self.src_dir = self.temp_dir / "src"
        self.tests_dir = self.temp_dir / "tests"
        self.engram_dir = self.temp_dir / ".engram"
        self.src_dir.mkdir(parents=True, exist_ok=True)
        self.tests_dir.mkdir(parents=True, exist_ok=True)
        self.engram_dir.mkdir(parents=True, exist_ok=True)

    def tearDown(self):
        if self.temp_dir.exists():
            shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_req1_sqlite_meta_table_positive(self):
        """REQ-1: Physical SQLite index.db with meta table and git_commit_head passes."""
        db_path = self.engram_dir / "index.db"
        conn = sqlite3.connect(str(db_path))
        conn.execute("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT)")
        conn.execute("INSERT INTO meta (key, value) VALUES ('git_commit_head', 'c70845b4d5913457f1b176fb0370b80dd792711f')")
        conn.commit()
        conn.close()

        passed = self.evaluator._check_commit_cache_req1_meta_persistence(self.temp_dir)
        self.assertTrue(passed)

    def test_req1_source_pattern_positive(self):
        """REQ-1: Source code creating meta table and persisting git_commit_head passes."""
        analyzer_rs = self.src_dir / "analyzer.rs"
        analyzer_rs.write_text(
            'conn.execute("CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT)", [])?;\n'
            'conn.execute("INSERT OR REPLACE INTO meta (key, value) VALUES (\'git_commit_head\', ?1)", params![head])?;\n',
            encoding="utf-8"
        )
        passed = self.evaluator._check_commit_cache_req1_meta_persistence(self.temp_dir)
        self.assertTrue(passed)

    def test_req1_missing_meta_negative(self):
        """REQ-1: Missing meta table in both SQLite and source code returns False."""
        passed = self.evaluator._check_commit_cache_req1_meta_persistence(self.temp_dir)
        self.assertFalse(passed)

    def test_req1_malformed_meta_negative(self):
        """REQ-1: SQLite database without git_commit_head record returns False."""
        db_path = self.engram_dir / "index.db"
        conn = sqlite3.connect(str(db_path))
        conn.execute("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT)")
        conn.execute("INSERT INTO meta (key, value) VALUES ('unrelated_key', 'some_val')")
        conn.commit()
        conn.close()

        passed = self.evaluator._check_commit_cache_req1_meta_persistence(self.temp_dir)
        self.assertFalse(passed)

    def test_req2_drift_detection_positive(self):
        """REQ-2: ensure_fresh inspecting commit divergence and triggering invalidation passes."""
        analyzer_rs = self.src_dir / "analyzer.rs"
        analyzer_rs.write_text(
            'pub fn ensure_fresh(root: &Path) -> Result<()> {\n'
            '    let cached_commit = get_meta_commit(&conn)?;\n'
            '    let current_commit = git::head(&root)?;\n'
            '    if cached_commit != current_commit {\n'
            '        stale = true;\n'
            '    }\n'
            '    if !stale && hashes.len() == scanned.len() { return Ok(()); }\n'
            '    let _ = analyze(&root)?;\n'
            '    Ok(())\n'
            '}\n',
            encoding="utf-8"
        )
        passed = self.evaluator._check_commit_cache_req2_drift_detection(self.temp_dir)
        self.assertTrue(passed)

    def test_req2_missing_drift_negative(self):
        """REQ-2: ensure_fresh without commit comparison returns False."""
        analyzer_rs = self.src_dir / "analyzer.rs"
        analyzer_rs.write_text(
            'pub fn ensure_fresh(root: &Path) -> Result<()> {\n'
            '    // Only checks file content hashes without comparing commit HEAD\n'
            '    if !stale && hashes.len() == scanned.len() { return Ok(()); }\n'
            '    Ok(())\n'
            '}\n',
            encoding="utf-8"
        )
        passed = self.evaluator._check_commit_cache_req2_drift_detection(self.temp_dir)
        self.assertFalse(passed)

    def test_req3_clean_subcommand_source_positive(self):
        """REQ-3: main.rs defining Clean { cache: bool } and safely unlinking cache passes."""
        main_rs = self.src_dir / "main.rs"
        main_rs.write_text(
            'enum CommandLine {\n'
            '    Clean {\n'
            '        #[arg(long)]\n'
            '        cache: bool,\n'
            '    },\n'
            '}\n'
            'fn handle_clean(cache: bool) -> Result<()> {\n'
            '    if cache {\n'
            '        let cache_db = Path::new(".engram/index.db");\n'
            '        if cache_db.exists() { fs::remove_file(cache_db)?; }\n'
            '    }\n'
            '    Ok(())\n'
            '}\n',
            encoding="utf-8"
        )
        passed = self.evaluator._check_commit_cache_req3_clean_subcommand(self.temp_dir)
        self.assertTrue(passed)

    def test_req3_clean_missing_cache_flag_negative(self):
        """REQ-3: Clean subcommand lacking --cache flag returns False."""
        main_rs = self.src_dir / "main.rs"
        main_rs.write_text(
            'enum CommandLine {\n'
            '    Clean,\n'
            '}\n',
            encoding="utf-8"
        )
        passed = self.evaluator._check_commit_cache_req3_clean_subcommand(self.temp_dir)
        self.assertFalse(passed)

    def test_req3_clean_deletes_memories_negative(self):
        """REQ-3: Subcommand that removes memories.json fails negative safety check."""
        main_rs = self.src_dir / "main.rs"
        main_rs.write_text(
            'enum CommandLine { Clean { #[arg(long)] cache: bool } }\n'
            'fn run() { fs::remove_file(".engram/index.db")?; fs::remove_file(".engram/memories.json")?; }\n',
            encoding="utf-8"
        )
        passed = self.evaluator._check_commit_cache_req3_clean_subcommand(self.temp_dir)
        self.assertFalse(passed)

    def test_req4_boundary_adherence_positive(self):
        """REQ-4: Edits confined strictly to allowed files pass AUTOMATED_DIFF."""
        impact = GitImpact(
            files_changed=["src/compiler.rs", "src/analyzer.rs"],
            insertions=25,
            deletions=4,
            unrelated_files_count=0,
            diff_patch_path="mock_patch.diff",
            dependency_mutated=False,
            files_changed_count=2
        )
        gates = {"L0_typecheck": "PASS", "L2_unit_tests": "PASS", "L3_production_build": "PASS"}
        # Provide positive implementations for REQ-1, REQ-2, REQ-3
        self.test_req1_source_pattern_positive()
        self.test_req2_drift_detection_positive()
        self.test_req3_clean_subcommand_source_positive()

        results = self.evaluator.evaluate_requirements(self.temp_dir, impact, gates)
        self.assertEqual(results["REQ-4"], "PASS")

    def test_req4_unauthorized_files_negative(self):
        """REQ-4: Edits touching unauthorized files fail AUTOMATED_DIFF."""
        impact = GitImpact(
            files_changed=["src/compiler.rs", "src/mcp.rs"],  # src/mcp.rs is forbidden
            insertions=20,
            deletions=0,
            unrelated_files_count=1,
            diff_patch_path="mock_patch.diff",
            dependency_mutated=False,
            files_changed_count=2
        )
        gates = {"L0_typecheck": "PASS", "L2_unit_tests": "PASS", "L3_production_build": "PASS"}
        results = self.evaluator.evaluate_requirements(self.temp_dir, impact, gates)
        self.assertEqual(results["REQ-4"], "FAIL")

    def test_req4_dependency_mutation_negative(self):
        """REQ-4: Cargo.toml or Cargo.lock drift fails all requirements and forces score C=0.0."""
        impact = GitImpact(
            files_changed=["src/compiler.rs", "Cargo.toml"],
            insertions=10,
            deletions=1,
            unrelated_files_count=1,
            diff_patch_path="mock_patch.diff",
            dependency_mutated=True,
            files_changed_count=2
        )
        gates = {"L0_typecheck": "FAIL", "L2_unit_tests": "BLOCKED", "L3_production_build": "FAIL"}
        results = self.evaluator.evaluate_requirements(self.temp_dir, impact, gates)
        for req_id in ["REQ-1", "REQ-2", "REQ-3", "REQ-4"]:
            self.assertEqual(results[req_id], "FAIL")

        score_res = self.evaluator.calculate_score(results, 50000, gates, dependency_mutated=True)
        self.assertEqual(score_res.score, 0.0)
        self.assertFalse(score_res.passed_all_requirements)

    def test_full_acceptance_valid_fixture_achieves_c10(self):
        """End-to-end acceptance: Valid fixture with all 4 requirements achieves C=1.0."""
        # 1. Setup valid analyzer.rs with BOTH REQ-1 (meta/git_commit_head) and REQ-2 (ensure_fresh drift)
        analyzer_rs = self.src_dir / "analyzer.rs"
        analyzer_rs.write_text(
            'pub fn record_meta(conn: &Connection, root: &Path) -> Result<()> {\n'
            '    conn.execute("CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT)", [])?;\n'
            '    let head = git_rev_parse(root)?;\n'
            '    conn.execute("INSERT OR REPLACE INTO meta (key, value) VALUES (\'git_commit_head\', ?1)", params![head])?;\n'
            '    Ok(())\n'
            '}\n'
            'pub fn ensure_fresh(root: &Path) -> Result<()> {\n'
            '    let cached_commit = get_meta_commit(&conn)?;\n'
            '    let current_commit = git::head(&root)?;\n'
            '    if cached_commit != current_commit {\n'
            '        stale = true;\n'
            '    }\n'
            '    if !stale && hashes.len() == scanned.len() { return Ok(()); }\n'
            '    let _ = analyze(&root)?;\n'
            '    Ok(())\n'
            '}\n',
            encoding="utf-8"
        )
        # 2. Setup valid REQ-3 (clean --cache in main.rs)
        self.test_req3_clean_subcommand_source_positive()
        # 3. Setup valid REQ-4 (clean impact on allowed files)
        impact = GitImpact(
            files_changed=["src/compiler.rs", "src/analyzer.rs", "src/main.rs"],
            insertions=45,
            deletions=8,
            unrelated_files_count=0,
            diff_patch_path="mock_patch.diff",
            dependency_mutated=False,
            files_changed_count=3
        )
        gates = {"L0_typecheck": "PASS", "L2_unit_tests": "PASS", "L3_production_build": "PASS"}

        req_results = self.evaluator.evaluate_requirements(self.temp_dir, impact, gates)
        self.assertEqual(req_results["REQ-1"], "PASS")
        self.assertEqual(req_results["REQ-2"], "PASS")
        self.assertEqual(req_results["REQ-3"], "PASS")
        self.assertEqual(req_results["REQ-4"], "PASS")

        score_res = self.evaluator.calculate_score(req_results, 65000, gates, dependency_mutated=False)
        self.assertEqual(score_res.score, 1.0)
        self.assertTrue(score_res.passed_all_requirements)

    def test_invalid_fixture_fails_acceptance(self):
        """Invalid fixture failing REQ-1 yields C=0.75 and passed_all_requirements=False."""
        # REQ-1 omitted
        self.test_req2_drift_detection_positive()
        self.test_req3_clean_subcommand_source_positive()
        impact = GitImpact(
            files_changed=["src/compiler.rs"],
            insertions=10,
            deletions=0,
            unrelated_files_count=0,
            diff_patch_path="mock_patch.diff",
            dependency_mutated=False,
            files_changed_count=1
        )
        gates = {"L0_typecheck": "PASS", "L2_unit_tests": "PASS", "L3_production_build": "PASS"}

        req_results = self.evaluator.evaluate_requirements(self.temp_dir, impact, gates)
        self.assertEqual(req_results["REQ-1"], "FAIL")
        self.assertEqual(req_results["REQ-2"], "PASS")
        self.assertEqual(req_results["REQ-3"], "PASS")
        self.assertEqual(req_results["REQ-4"], "PASS")

        score_res = self.evaluator.calculate_score(req_results, 65000, gates, dependency_mutated=False)
        self.assertEqual(score_res.score, 0.75)
        self.assertFalse(score_res.passed_all_requirements)


class TestMutualAcceptanceIntegrity(unittest.TestCase):
    """Phase E: Verifies mutual acceptance invariants across both arms."""

    def setUp(self):
        self.temp_dir = Path(tempfile.mkdtemp(prefix="engram_m5c_mutual_"))
        self.config_path = HARNESS_ROOT / "config" / "tasks" / "task_commit_cache_invalidation.json"
        self.orchestrator = Orchestrator(
            task_config_path=self.config_path,
            pair_id="pilot_m5c_test",
            is_dry_run=True,
            dependency_strategy="none",
            output_dir=self.temp_dir
        )

    def tearDown(self):
        if self.temp_dir.exists():
            shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_failed_baseline_suppresses_savings(self):
        """If baseline fails functional acceptance (C < 1.0), savings are strictly suppressed."""
        ctrl = {
            "correctness": {"score": 0.5, "passed_all_requirements": False},
            "telemetry": {"total_model_tokens": 120000, "telemetry_valid": True, "prompt_tokens_total": 110000, "completion_tokens": 10000}
        }
        treat = {
            "correctness": {"score": 1.0, "passed_all_requirements": True},
            "telemetry": {"total_model_tokens": 80000, "telemetry_valid": True, "prompt_tokens_total": 70000, "completion_tokens": 10000}
        }
        comp = self.orchestrator.compute_comparative_analysis(ctrl, treat)
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["verdict"], "CONTROL_FAILED_ACCEPTANCE")
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_ACCEPTANCE_FAILURE")

    def test_mutual_acceptance_failure_suppresses_savings(self):
        """If both arms fail functional acceptance (C < 1.0), savings are strictly suppressed."""
        ctrl = {
            "correctness": {"score": 0.5, "passed_all_requirements": False},
            "telemetry": {"total_model_tokens": 120000, "telemetry_valid": True, "prompt_tokens_total": 110000, "completion_tokens": 10000}
        }
        treat = {
            "correctness": {"score": 0.5, "passed_all_requirements": False},
            "telemetry": {"total_model_tokens": 80000, "telemetry_valid": True, "prompt_tokens_total": 70000, "completion_tokens": 10000}
        }
        comp = self.orchestrator.compute_comparative_analysis(ctrl, treat)
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["verdict"], "MUTUAL_ACCEPTANCE_FAILURE")
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_ACCEPTANCE_FAILURE")

    def test_failed_treatment_suppresses_savings(self):
        """If treatment fails functional acceptance (C < 1.0), savings are strictly suppressed."""
        ctrl = {
            "correctness": {"score": 1.0, "passed_all_requirements": True},
            "telemetry": {"total_model_tokens": 120000, "telemetry_valid": True, "prompt_tokens_total": 110000, "completion_tokens": 10000}
        }
        treat = {
            "correctness": {"score": 0.75, "passed_all_requirements": False},
            "telemetry": {"total_model_tokens": 70000, "telemetry_valid": True, "prompt_tokens_total": 60000, "completion_tokens": 10000}
        }
        comp = self.orchestrator.compute_comparative_analysis(ctrl, treat)
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["verdict"], "TREATMENT_FAILED_ACCEPTANCE")
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_ACCEPTANCE_FAILURE")

    def test_missing_telemetry_suppresses_savings(self):
        """If telemetry is missing or marked invalid, savings are suppressed even if both arms pass C=1.0."""
        ctrl = {
            "correctness": {"score": 1.0, "passed_all_requirements": True},
            "telemetry": {"telemetry_valid": False, "status": "MALFORMED"}
        }
        treat = {
            "correctness": {"score": 1.0, "passed_all_requirements": True},
            "telemetry": {"total_model_tokens": 70000, "telemetry_valid": True, "prompt_tokens_total": 60000, "completion_tokens": 10000}
        }
        comp = self.orchestrator.compute_comparative_analysis(ctrl, treat)
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_TELEMETRY_INVALID")

    def test_full_acceptance_computes_exact_savings(self):
        """When both arms achieve C=1.0 with valid telemetry, exact savings are computed."""
        ctrl = {
            "correctness": {"score": 1.0, "passed_all_requirements": True},
            "telemetry": {
                "total_model_tokens": 150000,
                "telemetry_valid": True,
                "prompt_tokens_total": 140000,
                "prompt_tokens_cached": 100000,
                "prompt_tokens_uncached": 40000,
                "completion_tokens": 10000,
                "peak_single_request_context": 120000
            }
        }
        treat = {
            "correctness": {"score": 1.0, "passed_all_requirements": True},
            "telemetry": {
                "total_model_tokens": 90000,
                "telemetry_valid": True,
                "prompt_tokens_total": 80000,
                "prompt_tokens_cached": 60000,
                "prompt_tokens_uncached": 20000,
                "completion_tokens": 10000,
                "peak_single_request_context": 75000
            }
        }
        comp = self.orchestrator.compute_comparative_analysis(ctrl, treat)
        self.assertTrue(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "VALID")
        self.assertEqual(comp["verdict"], "EFFICIENCY_GAIN")
        token_metric = comp["metrics"]["total_model_tokens"]
        self.assertEqual(token_metric["delta"], 60000)
        self.assertAlmostEqual(token_metric["savings_pct"], 40.0, places=1)

    def test_orchestrator_pair_mode_arm_order_treatment_first(self):
        """Verifies Orchestrator.run_pair defaults to treatment_B first and respects explicit arm_order."""
        calls = []

        def mock_execute(arm, exec_order):
            calls.append((arm, exec_order))
            return {
                "execution_status": "SUCCESS",
                "telemetry": {"telemetry_valid": True, "total_model_tokens": 1000},
                "correctness": {"passed_all_requirements": True, "score": 1.0}
            }

        with patch.object(self.orchestrator, "_execute_single_arm", side_effect=mock_execute):
            # Default order
            self.orchestrator.run_pair()
            self.assertEqual(calls, [("treatment_B", 1), ("control_A", 2)])

            # Explicit order
            calls.clear()
            self.orchestrator.run_pair(arm_order=["treatment_B", "control_A"])
            self.assertEqual(calls, [("treatment_B", 1), ("control_A", 2)])

    def test_task_commit_cache_req2_test_mapping(self):
        """Verifies task_commit_cache_invalidation L2 gate command exercises commit drift test."""
        task_cfg_path = HARNESS_ROOT / "config" / "tasks" / "task_commit_cache_invalidation.json"
        with open(task_cfg_path, "r", encoding="utf-8") as f:
            cfg = json.load(f)
        l2_cmd = cfg.get("gates", {}).get("L2_unit_tests", {}).get("command", "")
        self.assertIn("commit_cache_invalidation_test", l2_cmd)


if __name__ == "__main__":
    unittest.main()
