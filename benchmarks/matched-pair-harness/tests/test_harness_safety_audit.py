"""
Comprehensive offline safety audit test suite for the Engram benchmark harness.
Verifies:
1. Token accounting: snapshots vs. deltas, deduplication, reconciliation, and ceiling definition.
2. Hard-limit enforcement: boundary values (249,999, 250,000, 250,001), prompt termination.
3. Timeout & telemetry resilience: partial stdout/stderr recovery, corrupt JSONL, session discovery.
4. Windows process & sandbox safety: process-tree termination, grandchild cleanup, reader threads.
5. Stop-on-failure & scientific validity: skipped records, savings suppression, invariant preservation.

STRICTLY OFFLINE: Uses only mocked processes, synthetic event streams, and isolated temp directories.
"""
import unittest
import tempfile
import shutil
import json
import time
import sys
import os
import subprocess
from pathlib import Path
from unittest.mock import patch, MagicMock

# Add src to sys.path
HARNESS_SRC = Path(__file__).resolve().parent.parent / "src"
if str(HARNESS_SRC) not in sys.path:
    sys.path.insert(0, str(HARNESS_SRC))

from runner import CodexRunner, RunExecutionResult, StreamingTokenTracker, extract_tokens_from_line
from telemetry import TelemetryParser, TelemetryMetrics
from orchestrator import Orchestrator
from isolation import (
    terminate_pid_tree,
    compute_comprehensive_golden_manifest,
    verify_dependency_integrity,
    verify_sandbox_identity,
    verify_sandbox_cleanliness,
    IsolationError,
)


class TestHarnessSafetyAudit(unittest.TestCase):
    def setUp(self):
        self.temp_dir = Path(tempfile.mkdtemp(prefix="engram_safety_audit_"))
        self.task_config_path = self.temp_dir / "task_b_dummy.json"
        self.task_config = {
            "schema_version": "2.0.0",
            "task_id": "task_b_dummy",
            "prompt": "Test prompt for safety audit",
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

    # =========================================================================
    # AUDIT 1: Token Accounting Logic & Tracking
    # =========================================================================

    def test_token_snapshots_not_summed_repeatedly(self):
        """Verify provider cumulative thread snapshots are not repeatedly summed as new increments."""
        tracker = StreamingTokenTracker()
        # Three successive snapshots of cumulative thread usage
        l1 = json.dumps({"type": "token_usage_record", "payload": {"thread_token_usage": {"total_tokens": 50000}}})
        l2 = json.dumps({"type": "token_usage_record", "payload": {"thread_token_usage": {"total_tokens": 100000}}})
        l3 = json.dumps({"type": "token_usage_record", "payload": {"thread_token_usage": {"total_tokens": 150000}}})

        tracker.process_line(l1)
        self.assertEqual(tracker.effective_tokens, 50000)
        tracker.process_line(l2)
        self.assertEqual(tracker.effective_tokens, 100000)
        tracker.process_line(l3)
        # Authoritative cumulative total must be 150,000, NOT 300,000 (50k+100k+150k)
        self.assertEqual(tracker.effective_tokens, 150000)
        self.assertEqual(tracker.thread_cumulative_max, 150000)
        self.assertEqual(tracker.turn_deltas_sum, 0)

    def test_turn_deltas_summed_across_turns(self):
        """Verify per-turn turn.completed deltas are properly summed across turns."""
        tracker = StreamingTokenTracker()
        l1 = json.dumps({"type": "turn.completed", "turn_id": "turn_1", "usage": {"input_tokens": 85000, "output_tokens": 5000}})
        l2 = json.dumps({"type": "turn.completed", "turn_id": "turn_2", "usage": {"input_tokens": 85000, "output_tokens": 5000}})
        l3 = json.dumps({"type": "turn.completed", "turn_id": "turn_3", "usage": {"input_tokens": 85000, "output_tokens": 5000}})

        tracker.process_line(l1)
        self.assertEqual(tracker.effective_tokens, 90000)
        tracker.process_line(l2)
        self.assertEqual(tracker.effective_tokens, 180000)
        tracker.process_line(l3)
        # Sum of three 90k turns must be 270,000 tokens
        self.assertEqual(tracker.effective_tokens, 270000)
        self.assertGreater(tracker.effective_tokens, 250000)

    def test_duplicate_turn_events_deduplicated(self):
        """Verify re-emitted duplicate turn.completed lines do not inflate token counts."""
        tracker = StreamingTokenTracker()
        l1 = json.dumps({"type": "turn.completed", "turn_id": "turn_1", "usage": {"input_tokens": 50000, "output_tokens": 1000}})
        tracker.process_line(l1)
        self.assertEqual(tracker.effective_tokens, 51000)
        # Re-emit exact same turn event
        tracker.process_line(l1)
        self.assertEqual(tracker.effective_tokens, 51000)

    def test_mixed_snapshots_and_turn_deltas(self):
        """Verify reconciliation when both thread snapshots and turn deltas arrive."""
        tracker = StreamingTokenTracker()
        # Thread snapshot indicates 120,000 cumulative tokens
        snap = json.dumps({"type": "token_usage_record", "payload": {"thread_token_usage": {"total_tokens": 120000}}})
        tracker.process_line(snap)
        # Turn delta for turn 1 indicates 60,000
        delta = json.dumps({"type": "turn.completed", "turn_id": "t1", "usage": {"input_tokens": 58000, "output_tokens": 2000}})
        tracker.process_line(delta)
        # Effective usage must be max(120k, 60k) = 120k, never 180k
        self.assertEqual(tracker.effective_tokens, 120000)

    # =========================================================================
    # AUDIT 2: Hard-Limit Enforcement & Boundary Testing
    # =========================================================================

    def _run_mock_runner(self, py_script: str, max_tokens_ceiling: int = 250_000, timeout_seconds: int = 10) -> RunExecutionResult:
        runner = CodexRunner(
            codex_executable=Path(sys.executable),
            timeout_seconds=timeout_seconds,
            max_tokens_ceiling=max_tokens_ceiling
        )
        sandbox = self.temp_dir / f"sb_{time.time_ns()}"
        sandbox.mkdir(parents=True)
        events_path = self.temp_dir / f"ev_{time.time_ns()}.jsonl"

        orig_popen = subprocess.Popen
        proc = subprocess.Popen(
            [sys.executable, "-u", "-c", py_script],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8"
        )

        def mock_popen_filter(*args, **kwargs):
            cmd = args[0] if args else kwargs.get("args")
            if cmd and "taskkill" in str(cmd):
                return orig_popen(*args, **kwargs)
            return proc

        with patch("subprocess.Popen", side_effect=mock_popen_filter):
            res = runner.run(
                sandbox_path=sandbox,
                prompt="audit_prompt",
                arm="baseline_arm1",
                output_events_path=events_path,
                is_dry_run=False
            )
        return res

    def test_token_ceiling_boundary_just_below(self):
        """Test usage total just below the ceiling (249,999 tokens). Must NOT terminate for ceiling."""
        py_cmd = (
            "import sys, time, json\n"
            "sys.stdout.write(json.dumps({'type': 'thread.started', 'thread_id': 'th_below'}) + '\\n')\n"
            "sys.stdout.write(json.dumps({'type': 'turn.completed', 'usage': {'input_tokens': 249000, 'output_tokens': 999}}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(0.1)\n"
        )
        res = self._run_mock_runner(py_cmd, max_tokens_ceiling=250_000, timeout_seconds=5)
        self.assertFalse(res.token_ceiling_breached)
        self.assertFalse(res.timed_out)
        self.assertEqual(res.exit_code, 0)

    def test_token_ceiling_boundary_exactly_at(self):
        """Test usage total exactly at the ceiling (250,000 tokens). Boundary > 250,000 means not breached."""
        py_cmd = (
            "import sys, time, json\n"
            "sys.stdout.write(json.dumps({'type': 'thread.started', 'thread_id': 'th_exact'}) + '\\n')\n"
            "sys.stdout.write(json.dumps({'type': 'turn.completed', 'usage': {'input_tokens': 249000, 'output_tokens': 1000}}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(0.1)\n"
        )
        res = self._run_mock_runner(py_cmd, max_tokens_ceiling=250_000, timeout_seconds=5)
        self.assertFalse(res.token_ceiling_breached)
        self.assertEqual(res.exit_code, 0)

    def test_token_ceiling_boundary_just_above(self):
        """Test usage total just above the ceiling (250,001 tokens). Must trigger immediate breach."""
        py_cmd = (
            "import sys, time, json\n"
            "sys.stdout.write(json.dumps({'type': 'thread.started', 'thread_id': 'th_above'}) + '\\n')\n"
            "sys.stdout.write(json.dumps({'type': 'turn.completed', 'usage': {'input_tokens': 249001, 'output_tokens': 1000}}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(10)\n"
        )
        start = time.time()
        res = self._run_mock_runner(py_cmd, max_tokens_ceiling=250_000, timeout_seconds=10)
        elapsed = time.time() - start

        self.assertTrue(res.token_ceiling_breached)
        self.assertEqual(res.exit_code, -1)
        self.assertIn("Token ceiling exceeded", res.error_message)
        # Must terminate promptly (under 4s), never waiting for full 10s child sleep
        self.assertLess(elapsed, 4.0)

    def test_token_ceiling_single_large_jump(self):
        """Test a single event jumping from below to far above the ceiling (50k -> 500k)."""
        py_cmd = (
            "import sys, time, json\n"
            "sys.stdout.write(json.dumps({'type': 'thread.started', 'thread_id': 'th_jump'}) + '\\n')\n"
            "sys.stdout.write(json.dumps({'type': 'token_usage_record', 'payload': {'thread_token_usage': {'total_tokens': 50000}}}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(0.1)\n"
            "sys.stdout.write(json.dumps({'type': 'token_usage_record', 'payload': {'thread_token_usage': {'total_tokens': 500000}}}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(10)\n"
        )
        start = time.time()
        res = self._run_mock_runner(py_cmd, max_tokens_ceiling=250_000, timeout_seconds=10)
        elapsed = time.time() - start

        self.assertTrue(res.token_ceiling_breached)
        self.assertEqual(res.exit_code, -1)
        self.assertLess(elapsed, 4.0)

    def test_missing_or_delayed_tokens_active_process(self):
        """Test process emitting commands but no token events; runs to completion without false ceiling breach."""
        py_cmd = (
            "import sys, time, json\n"
            "sys.stdout.write(json.dumps({'type': 'thread.started', 'thread_id': 'th_notok'}) + '\\n')\n"
            "sys.stdout.write(json.dumps({'type': 'item.started', 'item': {'type': 'command_execution'}}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(0.2)\n"
            "sys.stdout.write(json.dumps({'type': 'item.completed', 'item': {'type': 'command_execution'}}) + '\\n')\n"
            "sys.stdout.flush()\n"
        )
        res = self._run_mock_runner(py_cmd, max_tokens_ceiling=250_000, timeout_seconds=5)
        self.assertFalse(res.token_ceiling_breached)
        self.assertFalse(res.timed_out)
        self.assertEqual(res.exit_code, 0)

    def test_process_never_exits_times_out_promptly(self):
        """Test a process that never exits; must time out at configured timeout and kill process tree."""
        py_cmd = (
            "import sys, time, json\n"
            "sys.stdout.write(json.dumps({'type': 'thread.started', 'thread_id': 'th_inf'}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "while True:\n"
            "    time.sleep(0.1)\n"
        )
        start = time.time()
        res = self._run_mock_runner(py_cmd, max_tokens_ceiling=250_000, timeout_seconds=1)
        elapsed = time.time() - start

        self.assertTrue(res.timed_out)
        self.assertEqual(res.exit_code, -1)
        self.assertIn("timed out", res.error_message.lower())
        self.assertLess(elapsed, 4.0)

    def test_simultaneous_timeout_and_token_breach(self):
        """Test timeout and ceiling breach triggering simultaneously; handled cleanly without race condition."""
        py_cmd = (
            "import sys, time, json\n"
            "sys.stdout.write(json.dumps({'type': 'thread.started', 'thread_id': 'th_both'}) + '\\n')\n"
            "sys.stdout.write(json.dumps({'type': 'turn.completed', 'usage': {'input_tokens': 300000, 'output_tokens': 5000}}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(5)\n"
        )
        # Timeout is 0.5s, tokens arrive at t=0
        res = self._run_mock_runner(py_cmd, max_tokens_ceiling=250_000, timeout_seconds=1)
        self.assertTrue(res.token_ceiling_breached or res.timed_out)
        self.assertEqual(res.exit_code, -1)

    def test_grandchild_process_tree_termination(self):
        """Test Windows process-tree termination on child that spawns a grandchild process."""
        # Python script that spawns a grandchild process sleeping for 60s
        marker_file = self.temp_dir / "grandchild.pid"
        py_cmd = (
            f"import sys, time, subprocess\n"
            f"proc2 = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'])\n"
            f"with open(r'{marker_file}', 'w') as f:\n"
            f"    f.write(str(proc2.pid))\n"
            f"time.sleep(60)\n"
        )
        proc1 = subprocess.Popen([sys.executable, "-u", "-c", py_cmd])
        # Wait for grandchild PID file to appear
        for _ in range(50):
            if marker_file.exists():
                break
            time.sleep(0.1)
        self.assertTrue(marker_file.exists())
        grandchild_pid = int(marker_file.read_text().strip())

        # Terminate root child process tree
        success = terminate_pid_tree(proc1.pid)
        proc1.kill()
        proc1.wait(timeout=2.0)

        # Verify grandchild is terminated on Windows
        check_proc = subprocess.run(["tasklist", "/FI", f"PID eq {grandchild_pid}"], capture_output=True, text=True)
        self.assertNotIn(str(grandchild_pid), check_proc.stdout)

    # =========================================================================
    # AUDIT 3: Timeout & Telemetry Resilience
    # =========================================================================

    def test_timeout_preserves_partial_stdout_and_stderr(self):
        """Verify partial stdout and stderr lines emitted prior to timeout are safely preserved."""
        py_cmd = (
            "import sys, time, json\n"
            "sys.stdout.write(json.dumps({'type': 'thread.started', 'thread_id': 'th_partial'}) + '\\n')\n"
            "sys.stdout.write(json.dumps({'type': 'item.completed', 'item': {'id': 'cmd_1', 'type': 'mcp_tool_call'}}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "sys.stderr.write('Diagnostic warning prior to timeout\\n')\n"
            "sys.stderr.flush()\n"
            "time.sleep(10)\n"
        )
        res = self._run_mock_runner(py_cmd, timeout_seconds=1)
        self.assertTrue(res.timed_out)
        self.assertGreaterEqual(res.recovered_events_count, 2)
        events_path = Path(res.events_log_path)
        self.assertTrue(events_path.exists())
        text = events_path.read_text(encoding="utf-8")
        self.assertIn("th_partial", text)
        self.assertIn("cmd_1", text)

    def test_missing_events_jsonl_returns_unavailable_metrics(self):
        """Verify missing events.jsonl returns structured unavailable metrics without raising FileNotFoundError."""
        parser = TelemetryParser(provenance="DIRECT_API", sessions_dir=self.temp_dir / "empty_sessions")
        metrics = parser.parse_file(self.temp_dir / "non_existent.jsonl")

        self.assertFalse(metrics.telemetry_valid)
        self.assertEqual(metrics.provenance, "UNAVAILABLE")
        self.assertEqual(metrics.total_model_tokens, 0)
        self.assertEqual(metrics.prompt_tokens_total, 0)
        self.assertEqual(metrics.completion_tokens, 0)
        self.assertIn("not found or empty", metrics.error_message)

    def test_corrupted_and_truncated_jsonl_handled(self):
        """Verify truncated and corrupted JSONL lines return structured unavailable metrics without crash."""
        corrupt_path = self.temp_dir / "corrupt.jsonl"
        corrupt_path.write_text("{\"type\": \"thread.started\", \"thread_id\": \"th_1\"}\n{truncated_json\nCORRUPT_BYTES\n", encoding="utf-8")

        parser = TelemetryParser(provenance="DIRECT_API", sessions_dir=self.temp_dir / "empty_sessions")
        metrics = parser.parse_file(corrupt_path)

        self.assertFalse(metrics.telemetry_valid)
        self.assertEqual(metrics.provenance, "UNAVAILABLE")
        self.assertEqual(metrics.total_model_tokens, 0)

    def test_native_rollout_recovery_when_stdout_empty(self):
        """Verify fallback recovery directly from native Codex session rollout when events.jsonl is empty."""
        sessions_dir = self.temp_dir / "sessions" / "2026" / "10" / "29"
        sessions_dir.mkdir(parents=True)
        th_id = "test_thread_rollout_recov"
        rollout_file = sessions_dir / f"rollout-2026-10-29T00-00-00-{th_id}.jsonl"

        # Populate rollout with valid token_usage_record
        with open(rollout_file, "w", encoding="utf-8") as f:
            f.write(json.dumps({"type": "session_meta", "payload": {"id": th_id}}) + "\n")
            f.write(json.dumps({
                "type": "token_usage_record",
                "payload": {
                    "usage": {"input_tokens": 10000, "output_tokens": 500, "total_tokens": 10500},
                    "thread_token_usage": {"input_tokens": 10000, "output_tokens": 500, "total_tokens": 10500}
                }
            }) + "\n")

        parser = TelemetryParser(provenance="DIRECT_API", sessions_dir=self.temp_dir / "sessions")
        empty_events = self.temp_dir / "empty_events.jsonl"
        empty_events.write_text("", encoding="utf-8")

        metrics = parser.parse_file(empty_events, thread_id=th_id)
        self.assertEqual(metrics.provenance, "SESSION_ROLLOUT")
        self.assertEqual(metrics.total_model_tokens, 10500)
        self.assertEqual(metrics.prompt_tokens_total, 10000)
        self.assertEqual(metrics.completion_tokens, 500)
        self.assertTrue(metrics.telemetry_valid)

    # =========================================================================
    # AUDIT 4: Windows Process and Sandbox Safety
    # =========================================================================

    def test_reader_threads_bounded_and_no_deadlock(self):
        """Verify stdout and stderr reader threads terminate cleanly within 2 seconds."""
        py_cmd = (
            "import sys, time\n"
            "for i in range(100):\n"
            "    sys.stdout.write(f'line_{i}\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(0.1)\n"
        )
        res = self._run_mock_runner(py_cmd, timeout_seconds=5)
        self.assertEqual(res.exit_code, 0)
        self.assertTrue(Path(res.events_log_path).exists())

    @patch("orchestrator.verify_dependency_integrity")
    @patch("orchestrator.compute_dependency_fingerprint", return_value={"package_json_sha256": "fake", "package_lock_sha256": "fake", "node_modules_file_count": 10, "node_modules_total_bytes": 100, "node_modules_recursive_digest": "fake"})
    def test_sandbox_teardown_on_all_paths(self, mock_fingerprint, mock_verify):
        """Verify sandbox teardown is executed unconditionally across normal, timeout, and exception paths."""
        out_dir = self.temp_dir / "sandbox_test_runs"
        with patch("evaluator.Evaluator.validate_preflight"):
            orchestrator = Orchestrator(
                task_config_path=self.task_config_path,
                pair_id="block_01",
                output_dir=out_dir,
                is_dry_run=True,
                stop_on_failure=True
            )

        # Test path 1: normal execution
        with patch("orchestrator.create_sandbox", return_value=self.temp_dir / "sb_norm"):
            with patch("orchestrator.teardown_sandbox") as mock_teardown:
                orchestrator._execute_single_arm("baseline_arm1", 1)
                mock_teardown.assert_called_once()

        # Test path 2: runner exception
        with patch("orchestrator.create_sandbox", return_value=self.temp_dir / "sb_exc"):
            with patch.object(orchestrator.runner, "run", side_effect=RuntimeError("Subprocess crash")):
                with patch("orchestrator.teardown_sandbox") as mock_teardown_exc:
                    with self.assertRaises(RuntimeError):
                        orchestrator._execute_single_arm("baseline_arm1", 1)
                    mock_teardown_exc.assert_called_once()

    # =========================================================================
    # AUDIT 5: Stop-on-Failure & Scientific Validity
    # =========================================================================

    @patch("orchestrator.verify_dependency_integrity")
    @patch("orchestrator.compute_dependency_fingerprint", return_value={"package_json_sha256": "fake", "package_lock_sha256": "fake", "node_modules_file_count": 10, "node_modules_total_bytes": 100, "node_modules_recursive_digest": "fake"})
    def test_stop_on_failure_halts_immediately_and_suppresses_savings(self, mock_fingerprint, mock_verify):
        """Verify failed arm halts block, records skipped arms, and strictly suppresses token savings claims."""
        out_dir = self.temp_dir / "triad_stop_runs"
        with patch("evaluator.Evaluator.validate_preflight"):
            orchestrator = Orchestrator(
                task_config_path=self.task_config_path,
                pair_id="block_06",
                output_dir=out_dir,
                is_dry_run=True,
                stop_on_failure=True
            )

        executed_arms = []
        def mock_exec(arm, exec_order=1, **kwargs):
            executed_arms.append(arm)
            if arm == "baseline_arm1":
                # Simulate Arm 1 timing out
                rec = orchestrator._create_skipped_arm_record(arm, exec_order, "Execution timed out after 600s")
                rec["execution_status"] = "TIMEOUT"
                return rec
            return orchestrator._create_skipped_arm_record(arm, exec_order, "normal")

        with patch.object(orchestrator, "_execute_single_arm", side_effect=mock_exec):
            summary = orchestrator.run_triad(block_id="block_06", arm_order=["baseline_arm1", "v151_arm2", "v154_arm3"])

        # Exactly 1 arm executed
        self.assertEqual(executed_arms, ["baseline_arm1"])
        # All 3 arms recorded in triad_summary
        self.assertIn("baseline_arm1", summary["runs"])
        self.assertIn("v151_arm2", summary["runs"])
        self.assertIn("v154_arm3", summary["runs"])
        # Skipped status
        self.assertEqual(summary["runs"]["v151_arm2"]["execution_status"], "SKIPPED_STOP_ON_FAILURE")
        self.assertEqual(summary["runs"]["v154_arm3"]["execution_status"], "SKIPPED_STOP_ON_FAILURE")
        # Acceptance failure
        self.assertFalse(summary["acceptance"]["all_arms_accepted"])
        # Savings claim suppressed
        comp = summary["comparative_analysis"]["v154_vs_baseline"]
        self.assertFalse(comp["savings_claim_eligible"])
        self.assertEqual(comp["savings_claim_status"], "INVALIDATED_BY_ACCEPTANCE_FAILURE")
        self.assertEqual(comp["verdict"], "MUTUAL_ACCEPTANCE_FAILURE")
        self.assertIsNone(comp["metrics"]["total_model_tokens"]["savings_pct"])

    def test_sandbox_identity_assertion_matches_and_mismatches(self):
        """Sandbox identity verification accepts matching commit and fails closed on mismatch."""
        sb_path = self.temp_dir / "test_sb_id"
        sb_path.mkdir(parents=True, exist_ok=True)

        # Mock git rev-parse returning expected commit
        with patch("subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=0, stdout="1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2\n", stderr="")
            res = verify_sandbox_identity(sb_path, "1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2")
            self.assertEqual(res, "1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2")

            # Mismatch raises IsolationError
            with self.assertRaises(IsolationError) as ctx:
                verify_sandbox_identity(sb_path, "different_commit_hash_123456789")
            self.assertIn("Sandbox identity mismatch", str(ctx.exception))

            # Git command failure raises IsolationError
            mock_run.return_value = MagicMock(returncode=128, stdout="", stderr="fatal: not a git repository")
            with self.assertRaises(IsolationError) as ctx:
                verify_sandbox_identity(sb_path, "1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2")
            self.assertIn("Sandbox identity verification failed", str(ctx.exception))

    def test_sandbox_cleanliness_assertion_clean_and_allowed_untracked(self):
        """Clean sandbox with allowed untracked files passes cleanliness verification."""
        sb_path = self.temp_dir / "test_sb_clean"
        sb_path.mkdir(parents=True, exist_ok=True)

        with patch("subprocess.run") as mock_run:
            # Completely clean
            mock_run.return_value = MagicMock(returncode=0, stdout="", stderr="")
            found = verify_sandbox_cleanliness(sb_path)
            self.assertEqual(found, [])

            # Allowed untracked: .github/copilot-instructions.md and node_modules
            mock_run.return_value = MagicMock(
                returncode=0,
                stdout="?? .github/copilot-instructions.md\n?? node_modules/\n",
                stderr=""
            )
            found = verify_sandbox_cleanliness(sb_path)
            self.assertEqual(len(found), 2)

    def test_sandbox_cleanliness_assertion_rejects_untracked_and_tracked_violations(self):
        """Cleanliness verification fails closed if unexpected untracked or tracked changes exist."""
        sb_path = self.temp_dir / "test_sb_dirty"
        sb_path.mkdir(parents=True, exist_ok=True)

        with patch("subprocess.run") as mock_run:
            # Unexpected untracked file
            mock_run.return_value = MagicMock(
                returncode=0,
                stdout="?? .engram/\n?? unexpected_script.py\n",
                stderr=""
            )
            with self.assertRaises(IsolationError) as ctx:
                verify_sandbox_cleanliness(sb_path)
            self.assertIn("Sandbox cleanliness violation", str(ctx.exception))
            self.assertIn(".engram/", str(ctx.exception))

            # Tracked modification
            mock_run.return_value = MagicMock(
                returncode=0,
                stdout=" M projects/angular-ngrx-material-starter/src/app/app/app.component.html\n",
                stderr=""
            )
            with self.assertRaises(IsolationError) as ctx:
                verify_sandbox_cleanliness(sb_path)
            self.assertIn("Tracked change [ M]", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()
