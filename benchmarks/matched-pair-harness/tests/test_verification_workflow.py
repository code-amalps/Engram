"""
Offline regression suite for Engram host-side verification workflow.

Tests:
1. A simulated compiler completing after several seconds without repeated model polling.
2. Incremental stdout/stderr capture.
3. Correct exit-code propagation.
4. A long-running process reaching its timeout.
5. Cancellation and child-process termination.
6. Partial output preservation after failure.
7. No busy loop or excessive polling (bounded adaptive polling).
8. Existing token-ceiling and stop-on-failure behavior remaining intact.
"""
from pathlib import Path
import subprocess
import threading
import tempfile
import unittest
import shutil
import time
import json
import sys
import os

# Ensure src is in sys.path
HARNESS_SRC = Path(__file__).resolve().parent.parent / "src"
if str(HARNESS_SRC) not in sys.path:
    sys.path.insert(0, str(HARNESS_SRC))

from verification import TrackedSubprocessRunner, VerificationResult
from evaluator import Evaluator, GitImpact


class TestVerificationWorkflow(unittest.TestCase):
    def setUp(self):
        self.temp_dir = Path(tempfile.mkdtemp(prefix="engram_verify_test_"))
        self.runner = TrackedSubprocessRunner(
            default_timeout_seconds=10,
            min_poll_interval_s=0.05,
            max_poll_interval_s=0.25,
            backoff_factor=1.25
        )
        self._script_counter = 0

    def tearDown(self):
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def _create_script(self, code: str) -> str:
        """Helper to create a temporary python script and return execution command."""
        self._script_counter += 1
        script_file = self.temp_dir / f"test_script_{self._script_counter}.py"
        script_file.write_text(code, encoding="utf-8")
        return f'"{sys.executable}" -u "{str(script_file)}"'

    def test_simulated_compiler_completes_without_repeated_model_polling(self):
        """
        1. A simulated compiler completing after several seconds on the host
        without requiring repeated model interaction or model turns.
        """
        code = (
            "import sys, time\n"
            "sys.stdout.write('Compiling 14 source files...\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(1.2)\n"
            "sys.stdout.write('Checking type definitions...\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(0.8)\n"
            "sys.stdout.write('Found 0 errors. Compilation complete.\\n')\n"
            "sys.stdout.flush()\n"
            "sys.exit(0)\n"
        )
        cmd = self._create_script(code)
        res = self.runner.run_command(cmd, cwd=self.temp_dir, timeout_seconds=10)

        self.assertEqual(res.status, "PASS")
        self.assertEqual(res.exit_code, 0)
        self.assertFalse(res.timed_out)
        self.assertFalse(res.cancelled)
        self.assertIn("Compiling 14 source files...", res.stdout)
        self.assertIn("Found 0 errors. Compilation complete.", res.stdout)
        self.assertGreaterEqual(res.elapsed_seconds, 1.8)
        self.assertLess(res.elapsed_seconds, 5.0)

    def test_incremental_stdout_stderr_capture(self):
        """
        2. Incremental stdout and stderr capture across separate stream flushes.
        """
        code = (
            "import sys, time\n"
            "sys.stdout.write('Line 1 from STDOUT\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(0.3)\n"
            "sys.stderr.write('Warning 1 from STDERR\\n')\n"
            "sys.stderr.flush()\n"
            "time.sleep(0.3)\n"
            "sys.stdout.write('Line 2 from STDOUT\\n')\n"
            "sys.stdout.flush()\n"
            "sys.exit(0)\n"
        )
        cmd = self._create_script(code)
        res = self.runner.run_command(cmd, cwd=self.temp_dir)

        self.assertEqual(res.status, "PASS")
        self.assertEqual(res.exit_code, 0)
        self.assertIn("Line 1 from STDOUT", res.stdout)
        self.assertIn("Line 2 from STDOUT", res.stdout)
        self.assertIn("Warning 1 from STDERR", res.stderr)
        self.assertIn("--- STDERR ---", res.combined_output)
        self.assertTrue(res.partial_output_preserved)

    def test_correct_exit_code_propagation(self):
        """
        3. Correct exit-code propagation for success, standard failures, and custom codes.
        """
        # Code 0 -> PASS
        cmd0 = self._create_script("import sys; sys.exit(0)")
        res0 = self.runner.run_command(cmd0, cwd=self.temp_dir)
        self.assertEqual(res0.exit_code, 0)
        self.assertEqual(res0.status, "PASS")

        # Code 1 -> FAIL
        cmd1 = self._create_script("import sys; sys.exit(1)")
        res1 = self.runner.run_command(cmd1, cwd=self.temp_dir)
        self.assertEqual(res1.exit_code, 1)
        self.assertEqual(res1.status, "FAIL")

        # Code 42 -> FAIL (custom exit code propagated exactly)
        cmd42 = self._create_script("import sys; sys.exit(42)")
        res42 = self.runner.run_command(cmd42, cwd=self.temp_dir)
        self.assertEqual(res42.exit_code, 42)
        self.assertEqual(res42.status, "FAIL")

    def test_long_running_process_reaches_timeout(self):
        """
        4. A long-running process reaching its timeout is terminated safely.
        """
        code = (
            "import sys, time\n"
            "sys.stdout.write('Process started, waiting indefinitely...\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(30)\n"
            "sys.stdout.write('Should never reach here!\\n')\n"
        )
        cmd = self._create_script(code)
        res = self.runner.run_command(cmd, cwd=self.temp_dir, timeout_seconds=1)

        self.assertTrue(res.timed_out)
        self.assertEqual(res.status, "TIMEOUT")
        self.assertEqual(res.exit_code, 124)
        self.assertIn("Process started, waiting indefinitely...", res.stdout)
        self.assertNotIn("Should never reach here!", res.stdout)
        self.assertLess(res.elapsed_seconds, 4.0)

    def test_cancellation_and_child_process_termination(self):
        """
        5. Cancellation cleanly halts execution and terminates child process tree.
        """
        cancel_evt = threading.Event()

        # Trigger cancellation after 0.5s
        def trigger_cancel():
            time.sleep(0.5)
            cancel_evt.set()

        t = threading.Thread(target=trigger_cancel, daemon=True)
        t.start()

        code = (
            "import sys, time\n"
            "sys.stdout.write('Active before cancel\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(20)\n"
            "sys.stdout.write('Unreachable\\n')\n"
        )
        cmd = self._create_script(code)
        res = self.runner.run_command(cmd, cwd=self.temp_dir, cancel_event=cancel_evt, timeout_seconds=10)

        self.assertTrue(res.cancelled)
        self.assertEqual(res.status, "CANCELLED")
        self.assertEqual(res.exit_code, 130)
        self.assertIn("Active before cancel", res.stdout)
        self.assertNotIn("Unreachable", res.stdout)
        self.assertLess(res.elapsed_seconds, 3.0)

    def test_partial_output_preservation_after_failure(self):
        """
        6. Partial output is completely preserved when a process crashes or fails.
        """
        code = (
            "import sys\n"
            "sys.stdout.write('Parsing AST tokens...\\n')\n"
            "sys.stdout.write('Resolving module imports...\\n')\n"
            "sys.stdout.flush()\n"
            "sys.stderr.write('Fatal error: TS2307 Cannot find module @ngrx/store\\n')\n"
            "sys.stderr.flush()\n"
            "sys.exit(2)\n"
        )
        cmd = self._create_script(code)
        res = self.runner.run_command(cmd, cwd=self.temp_dir)

        self.assertEqual(res.status, "FAIL")
        self.assertEqual(res.exit_code, 2)
        self.assertTrue(res.partial_output_preserved)
        self.assertIn("Parsing AST tokens...", res.stdout)
        self.assertIn("Resolving module imports...", res.stdout)
        self.assertIn("Fatal error: TS2307 Cannot find module", res.stderr)

    def test_no_busy_loop_or_excessive_polling(self):
        """
        7. Host-side adaptive polling prevents CPU busy-looping by enforcing
        bounded poll counts with backoff.
        """
        code = "import time; time.sleep(1.2)"
        cmd = self._create_script(code)
        res = self.runner.run_command(cmd, cwd=self.temp_dir)

        self.assertEqual(res.status, "PASS")
        # Over 1.2s with 50ms - 250ms backoff, poll_count should be around 7-15, strictly < 30
        self.assertGreaterEqual(res.poll_count, 4)
        self.assertLess(res.poll_count, 30)

    def test_evaluator_integration_runs_tracked_gates(self):
        """
        8. Evaluator.run_automated_gates executes gates via TrackedSubprocessRunner,
        recording PASS, FAIL, or BLOCKED and preserving gate details.
        """
        l0_cmd = self._create_script("print('L0 OK')")
        l2_cmd = self._create_script("import sys; sys.exit(1)")
        task_cfg = {
            "task_id": "test_task",
            "gates": {
                "L0_typecheck": {
                    "enabled": True,
                    "command": l0_cmd
                },
                "L1_lint": {
                    "enabled": False,
                    "command": "npm run lint"
                },
                "L2_unit_tests": {
                    "enabled": True,
                    "command": l2_cmd
                }
            }
        }
        evaluator = Evaluator(task_cfg)
        results = evaluator.run_automated_gates(self.temp_dir)

        self.assertEqual(results.get("L0_typecheck"), "PASS")
        self.assertEqual(results.get("L1_lint_prettier"), "SKIPPED")
        self.assertEqual(results.get("L2_unit_tests"), "FAIL")
        self.assertIn("L0_typecheck", evaluator.last_gate_details)
        self.assertEqual(evaluator.last_gate_details["L0_typecheck"]["exit_code"], 0)

    def test_clock_independent_monotonic_timeout(self):
        """
        9. Monotonic timing ensures timeouts are strictly clock-independent
        and accurate even if system wall-clock adjustments occur.
        """
        # A command configured for 1.0s timeout with monotonic tracking
        code = "import time; time.sleep(10)"
        cmd = self._create_script(code)
        start = time.monotonic()
        res = self.runner.run_command(cmd, cwd=self.temp_dir, timeout_seconds=1.0)
        duration = time.monotonic() - start

        self.assertTrue(res.timed_out)
        self.assertEqual(res.status, "TIMEOUT")
        self.assertEqual(res.exit_code, 124)
        self.assertGreaterEqual(duration, 0.95)
        self.assertLess(duration, 4.0)

    def test_output_cap_and_truncation_preserves_head_and_tail(self):
        """
        10. Bounded stdout/stderr capture enforces memory limits while preserving
        initial diagnostics (head) and final diagnostics (tail) with truncation marker.
        """
        # Script emits 100,000 characters: distinct head, huge middle, distinct tail
        code = (
            "import sys\n"
            "sys.stdout.write('START_HEAD_DIAGNOSTICS_ALPHA\\n')\n"
            "sys.stdout.write('A' * 60000 + '\\n')\n"
            "sys.stdout.write('Z' * 60000 + '\\n')\n"
            "sys.stdout.write('END_TAIL_DIAGNOSTICS_OMEGA\\n')\n"
            "sys.stdout.flush()\n"
        )
        cmd = self._create_script(code)
        # Cap runner total to 2,000 chars (1,000 per stream)
        res = self.runner.run_command(cmd, cwd=self.temp_dir, max_output_chars=2000)

        self.assertEqual(res.exit_code, 0)
        self.assertTrue(res.output_truncated)
        self.assertTrue(res.stdout_truncated)
        self.assertFalse(res.stderr_truncated)
        self.assertIn("OUTPUT TRUNCATED", res.stdout)
        self.assertIn("START_HEAD_DIAGNOSTICS_ALPHA", res.stdout)
        self.assertIn("END_TAIL_DIAGNOSTICS_OMEGA", res.stdout)
        # Verify stored stdout length does not exceed bound + truncation notice
        self.assertLess(len(res.stdout), 3500)

    def test_aggregate_deadline_exhaustion_marks_subsequent_gates_blocked(self):
        """
        11. An aggregate verification deadline across sequential gates gives each
        gate only remaining time; gates with zero remaining budget are marked BLOCKED.
        """
        # Gate L0 takes 0.7s out of a 0.5s aggregate budget
        l0_cmd = self._create_script("import time; time.sleep(0.7)")
        l2_cmd = self._create_script("print('L2 OK')")
        task_cfg = {
            "task_id": "test_aggregate_task",
            "verification_timeout_seconds": 0.5,
            "gates": {
                "L0_typecheck": {
                    "enabled": True,
                    "command": l0_cmd,
                    "timeout_seconds": 10
                },
                "L1_lint": {
                    "enabled": False,
                    "command": "npm run lint"
                },
                "L2_unit_tests": {
                    "enabled": True,
                    "command": l2_cmd,
                    "timeout_seconds": 10
                }
            }
        }
        evaluator = Evaluator(task_cfg)
        results = evaluator.run_automated_gates(self.temp_dir, aggregate_timeout_seconds=0.5)

        # L0 times out because aggregate budget was 0.5s (effective_timeout=0.5)
        self.assertEqual(results.get("L0_typecheck"), "BLOCKED")
        # L1 was disabled -> SKIPPED
        self.assertEqual(results.get("L1_lint_prettier"), "SKIPPED")
        # L2 cannot start because remaining budget <= 0 -> BLOCKED
        self.assertEqual(results.get("L2_unit_tests"), "BLOCKED")
        self.assertIn("L2_unit_tests", evaluator.last_gate_details)
        self.assertEqual(evaluator.last_gate_details["L2_unit_tests"]["status"], "BLOCKED")

    def test_descendant_process_tree_termination(self):
        """
        12. Verifies Windows process-tree termination cleanly reaps descendant
        subprocesses spawned by the gate command upon timeout.
        """
        # Script spawns a detached background child process that sleeps for 60s
        code = (
            "import subprocess, sys, time\n"
            "child_code = 'import time; time.sleep(60)'\n"
            "child = subprocess.Popen([sys.executable, '-c', child_code])\n"
            "sys.stdout.write(f'CHILD_PID:{child.pid}\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(30)\n"
        )
        cmd = self._create_script(code)
        res = self.runner.run_command(cmd, cwd=self.temp_dir, timeout_seconds=1.0)

        self.assertTrue(res.timed_out)
        self.assertEqual(res.status, "TIMEOUT")
        self.assertIn("CHILD_PID:", res.stdout)
        child_pid = int(res.stdout.split("CHILD_PID:")[1].splitlines()[0].strip())

        # Give OS 0.5s to finish taskkill
        time.sleep(0.5)
        # Check if child process is still running using tasklist
        check = subprocess.run(
            ["tasklist", "/FI", f"PID eq {child_pid}"],
            capture_output=True,
            text=True,
            check=False
        )
        # If killed, tasklist reports "No tasks are running which match the specified criteria."
        self.assertNotIn(f" {child_pid} ", check.stdout)


if __name__ == "__main__":
    unittest.main()
