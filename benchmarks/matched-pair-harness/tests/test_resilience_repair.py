"""
Offline regression suite for Engram Stage 1 harness resilience repair.
Validates:
1. Subprocess timeout handling without losing partial output.
2. Hard token ceiling enforcement mid-run.
3. Structured failed-arm records on missing/corrupted events.jsonl.
4. Telemetry invalidation and savings suppression without token fabrication.
5. Stop-on-failure triad behavior and skipped arm record generation.
6. Guaranteed sandbox teardown and golden integrity preservation.
"""
import unittest
import tempfile
import shutil
import json
import time
import sys
from pathlib import Path
from unittest.mock import patch, MagicMock

# Add src to sys.path
HARNESS_SRC = Path(__file__).resolve().parent.parent / "src"
if str(HARNESS_SRC) not in sys.path:
    sys.path.insert(0, str(HARNESS_SRC))

from runner import CodexRunner, RunExecutionResult, extract_tokens_from_line
from telemetry import TelemetryParser, TelemetryMetrics
from orchestrator import Orchestrator
from isolation import compute_dependency_fingerprint, teardown_sandbox


class TestResilienceRepair(unittest.TestCase):
    def setUp(self):
        self.temp_dir = Path(tempfile.mkdtemp(prefix="engram_resilience_test_"))
        self.task_config_path = self.temp_dir / "task_b_dummy.json"
        self.task_config = {
            "schema_version": "2.0.0",
            "task_id": "task_b_dummy",
            "prompt": "Test prompt",
            "baseline_commit": "1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2",
            "requirements": [
                {"id": "REQ-1", "weight": 0.5, "criteria": "test"},
                {"id": "REQ-2", "weight": 0.5, "criteria": "test"}
            ]
        }
        with open(self.task_config_path, "w", encoding="utf-8") as f:
            json.dump(self.task_config, f)

    def tearDown(self):
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_extract_tokens_from_line(self):
        """Verifies extraction of cumulative tokens across all supported event shapes."""
        # turn.completed
        line1 = json.dumps({"type": "turn.completed", "usage": {"input_tokens": 1000, "output_tokens": 50}})
        self.assertEqual(extract_tokens_from_line(line1), 1050)

        # token_usage_record
        line2 = json.dumps({"type": "token_usage_record", "payload": {"thread_token_usage": {"total_tokens": 55000}}})
        self.assertEqual(extract_tokens_from_line(line2), 55000)

        # event_msg token_count
        line3 = json.dumps({"type": "event_msg", "payload": {"type": "token_count", "info": {"total_token_usage": {"total_tokens": 125000}}}})
        self.assertEqual(extract_tokens_from_line(line3), 125000)

        # Non-token line
        line4 = json.dumps({"type": "item.started", "item": {"type": "agent_message"}})
        self.assertIsNone(extract_tokens_from_line(line4))

    def test_runner_timeout_preserves_partial_output(self):
        """Verifies that when a process times out, partial stdout is preserved and process is killed."""
        runner = CodexRunner(
            codex_executable=Path(sys.executable),
            timeout_seconds=1,
            max_tokens_ceiling=250_000
        )
        sandbox = self.temp_dir / "sandbox"
        sandbox.mkdir(parents=True)
        events_path = self.temp_dir / "events.jsonl"

        # Mock codex executable command to run a script that emits partial events then sleeps
        py_cmd = (
            "import sys, time, json\n"
            "sys.stdout.write(json.dumps({'type': 'thread.started', 'thread_id': 'test_th_1'}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "sys.stdout.write(json.dumps({'type': 'turn.started'}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(10)\n"
        )
        with patch.object(runner, "codex_executable", Path(sys.executable)):
            # Patch Popen in runner to run the python snippet
            orig_popen = runner.run
            # We run python -c py_cmd by mocking the command construction
            def mock_run(*args, **kwargs):
                return runner.run(
                    sandbox_path=sandbox,
                    prompt="test",
                    arm="baseline_arm1",
                    output_events_path=events_path,
                    is_dry_run=False
                )

        # Test the direct streaming reader with a real timeout on python
        import subprocess
        proc = subprocess.Popen(
            [sys.executable, "-u", "-c", py_cmd],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8"
        )

        orig_popen = subprocess.Popen
        def mock_popen_1(*args, **kwargs):
            cmd = args[0] if args else kwargs.get("args")
            if cmd and "taskkill" in str(cmd):
                return orig_popen(*args, **kwargs)
            return proc

        # Verify runner's timeout handling logic directly
        with patch("subprocess.Popen", side_effect=mock_popen_1):
            res = runner.run(
                sandbox_path=sandbox,
                prompt="test",
                arm="baseline_arm1",
                output_events_path=events_path,
                is_dry_run=False
            )

        self.assertTrue(res.timed_out)
        self.assertEqual(res.exit_code, -1)
        self.assertIn("timed out", res.error_message.lower())
        self.assertTrue(events_path.exists())
        content = events_path.read_text(encoding="utf-8")
        self.assertIn("thread.started", content)
        self.assertIn("test_th_1", content)
        self.assertGreaterEqual(res.recovered_events_count, 1)

    def test_runner_token_ceiling_enforcement(self):
        """Verifies that if tokens exceed max_tokens_ceiling, execution is terminated immediately."""
        runner = CodexRunner(
            codex_executable=Path(sys.executable),
            timeout_seconds=30,
            max_tokens_ceiling=5000  # Low ceiling for testing
        )
        sandbox = self.temp_dir / "sandbox2"
        sandbox.mkdir(parents=True)
        events_path = self.temp_dir / "events_ceiling.jsonl"

        # Python script that emits events with tokens exceeding 5000, then sleeps
        py_cmd = (
            "import sys, time, json\n"
            "sys.stdout.write(json.dumps({'type': 'thread.started', 'thread_id': 'test_th_ceil'}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "sys.stdout.write(json.dumps({'type': 'turn.completed', 'usage': {'input_tokens': 6000, 'output_tokens': 200}}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "for _ in range(300):\n"
            "    time.sleep(0.1)\n"
        )
        import subprocess
        proc = subprocess.Popen(
            [sys.executable, "-u", "-c", py_cmd],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8"
        )

        orig_popen = subprocess.Popen
        def mock_popen_2(*args, **kwargs):
            cmd = args[0] if args else kwargs.get("args")
            if cmd and "taskkill" in str(cmd):
                return orig_popen(*args, **kwargs)
            return proc

        with patch("subprocess.Popen", side_effect=mock_popen_2):
            start = time.time()
            res = runner.run(
                sandbox_path=sandbox,
                prompt="test",
                arm="baseline_arm1",
                output_events_path=events_path,
                is_dry_run=False
            )
            elapsed = time.time() - start

        self.assertTrue(res.token_ceiling_breached)
        self.assertEqual(res.exit_code, -1)
        self.assertIn("Token ceiling exceeded", res.error_message)
        self.assertLess(elapsed, 8.0)  # Must terminate promptly, not wait for 30s child loop
        self.assertTrue(events_path.exists())
        self.assertIn("turn.completed", events_path.read_text(encoding="utf-8"))

    def test_telemetry_missing_file_creates_unavailable_metrics(self):
        """Verifies that missing events.jsonl returns structured unavailable metrics without raising."""
        parser = TelemetryParser(provenance="DIRECT_API", sessions_dir=self.temp_dir / "sessions")
        metrics = parser.parse_file(self.temp_dir / "does_not_exist.jsonl")

        self.assertFalse(metrics.telemetry_valid)
        self.assertEqual(metrics.provenance, "UNAVAILABLE")
        self.assertEqual(metrics.total_model_tokens, 0)
        self.assertEqual(metrics.prompt_tokens_total, 0)
        self.assertEqual(metrics.completion_tokens, 0)
        self.assertIsNotNone(metrics.error_message)

    def test_telemetry_corrupted_jsonl_handled(self):
        """Verifies that malformed/partial JSONL lines return structured unavailable metrics without raising."""
        bad_file = self.temp_dir / "corrupted.jsonl"
        bad_file.write_text("NOT_VALID_JSON\n{truncated_json\n", encoding="utf-8")

        parser = TelemetryParser(provenance="DIRECT_API", sessions_dir=self.temp_dir / "sessions")
        metrics = parser.parse_file(bad_file)

        self.assertFalse(metrics.telemetry_valid)
        self.assertEqual(metrics.provenance, "UNAVAILABLE")
        self.assertEqual(metrics.total_model_tokens, 0)

    @patch("orchestrator.verify_dependency_integrity")
    @patch("orchestrator.compute_dependency_fingerprint", return_value={"package_json_sha256": "fake", "package_lock_sha256": "fake", "node_modules_file_count": 10, "node_modules_total_bytes": 100, "node_modules_recursive_digest": "fake"})
    def test_orchestrator_stop_on_failure_triad(self, mock_fingerprint, mock_verify):
        """Verifies that when Arm 1 fails, Arm 2 and Arm 3 are skipped, records validate, and savings suppressed."""
        out_dir = self.temp_dir / "triad_runs"
        with patch("evaluator.Evaluator.validate_preflight"):
            orchestrator = Orchestrator(
                task_config_path=self.task_config_path,
                pair_id="block_01",
                output_dir=out_dir,
                is_dry_run=True,
                stop_on_failure=True
            )

        # Mock _execute_single_arm so that the first call fails with TIMEOUT
        call_count = [0]
        orig_exec = orchestrator._execute_single_arm

        def mock_exec_fail_first(arm, exec_order):
            call_count[0] += 1
            if exec_order == 1:
                # Return failed arm record
                record = orchestrator._create_skipped_arm_record(arm, exec_order, "Execution timed out after 600s")
                record["execution_status"] = "TIMEOUT"
                return record
            return orig_exec(arm, exec_order)

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec_fail_first):
            summary = orchestrator.run_triad(block_id="block_01", arm_order=["baseline_arm1", "v151_arm2", "v154_arm3"])

        # Exactly 1 arm was executed before stop-on-failure triggered
        self.assertEqual(call_count[0], 1)

        # All 3 arms are present in runs
        self.assertIn("baseline_arm1", summary["runs"])
        self.assertIn("v151_arm2", summary["runs"])
        self.assertIn("v154_arm3", summary["runs"])

        # Check statuses
        self.assertEqual(summary["runs"]["baseline_arm1"]["execution_status"], "TIMEOUT")
        self.assertEqual(summary["runs"]["v151_arm2"]["execution_status"], "SKIPPED_STOP_ON_FAILURE")
        self.assertEqual(summary["runs"]["v154_arm3"]["execution_status"], "SKIPPED_STOP_ON_FAILURE")

        # Check acceptance
        self.assertFalse(summary["acceptance"]["baseline_arm1_accepted"])
        self.assertFalse(summary["acceptance"]["v151_arm2_accepted"])
        self.assertFalse(summary["acceptance"]["v154_arm3_accepted"])
        self.assertFalse(summary["acceptance"]["all_arms_accepted"])

        # Check comparative savings suppression
        p_res = summary["comparative_analysis"]["v154_vs_baseline"]
        self.assertFalse(p_res["savings_claim_eligible"])
        self.assertEqual(p_res["savings_claim_status"], "INVALIDATED_BY_ACCEPTANCE_FAILURE")
        self.assertEqual(p_res["verdict"], "MUTUAL_ACCEPTANCE_FAILURE")

        # Check triad report and summary files exist on disk
        self.assertTrue((out_dir / "triad_summary.json").exists())
        self.assertTrue((out_dir / "triad_report.md").exists())

    @patch("orchestrator.verify_dependency_integrity")
    @patch("orchestrator.compute_dependency_fingerprint", return_value={"package_json_sha256": "fake", "package_lock_sha256": "fake", "node_modules_file_count": 10, "node_modules_total_bytes": 100, "node_modules_recursive_digest": "fake"})
    def test_orchestrator_timed_out_arm_produces_valid_record(self, mock_fingerprint, mock_verify):
        """Verifies _execute_single_arm handles a timed-out run without FileNotFoundError and outputs schema-valid json."""
        out_dir = self.temp_dir / "arm_run"
        with patch("evaluator.Evaluator.validate_preflight"):
            orchestrator = Orchestrator(
                task_config_path=self.task_config_path,
                pair_id="block_01",
                output_dir=out_dir,
                is_dry_run=True,
                stop_on_failure=True
            )

        mock_res = RunExecutionResult(
            exit_code=-1,
            elapsed_seconds=600.0,
            started_at="2026-10-28T15:00:00Z",
            completed_at="2026-10-28T15:10:00Z",
            events_log_path=str(out_dir / "baseline_arm1" / "events.jsonl"),
            thread_id="test_th_timeout",
            model_name="gpt-5.6-luna",
            reasoning_effort="medium",
            harness_executable="codex.exe",
            harness_version="1.0",
            path_sanitized=True,
            error_message="Execution timed out after 600 seconds",
            timed_out=True
        )

        # Mock runner.run to return the timed out result
        with patch.object(orchestrator.runner, "run", return_value=mock_res):
            with patch("orchestrator.create_sandbox", return_value=self.temp_dir / "fake_sandbox"):
                with patch("orchestrator.teardown_sandbox") as mock_teardown:
                    with patch.object(orchestrator.evaluator, "validate_preflight"):
                        record = orchestrator._execute_single_arm("baseline_arm1", 1)
                        mock_teardown.assert_called_once()

        self.assertEqual(record["execution_status"], "TIMEOUT")
        self.assertEqual(record["correctness"]["score"], 0.0)
        self.assertFalse(record["correctness"]["passed_all_requirements"])
        self.assertFalse(record["telemetry"]["telemetry_valid"])
        self.assertTrue((out_dir / "baseline_arm1" / "run_telemetry.json").exists())


if __name__ == "__main__":
    unittest.main()
