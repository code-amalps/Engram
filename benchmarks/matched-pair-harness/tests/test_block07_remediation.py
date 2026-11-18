"""
Unit and integration regression tests for Block 07 root-cause remediation.

Covers:
1. Fix 1: Safe non-interactive MCP approval configuration without global bypass.
2. Fix 1 CLI validation: Offline verification of Codex CLI accepting fine-grained MCP approval flags.
3. Candidate binary tool surface: stdio JSON-RPC handshake verification for 4 consolidated tools.
4. Fix 2: Change contract planned file injection authorizing Task B targets without in-place mutation.
5. Fix 2 fallback: Preservation of prompt behavior when target_files is absent.
6. Fix 3: StreamingTokenTracker pre-dispatch headroom calculations and boundary conditions.
7. Fix 3 runner integration: Pre-dispatch headroom guard termination, ceiling breach flag, and error message.
8. Shell fallback retrieval guidance: Added to candidate arm prompt, excluded from baseline arm prompt.
9. Task B config: Declares the 5 required target files.
"""

import unittest
from unittest.mock import patch
from pathlib import Path
import subprocess
import tempfile
import shutil
import json
import sys
import os

# Harness imports
ROOT_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT_DIR / "src"))

from runner import (
    CodexRunner,
    RunExecutionResult,
    StreamingTokenTracker,
    SHELL_RETRIEVAL_GUIDANCE
)
from orchestrator import Orchestrator
from schema_validator import validate_instance


class TestBlock07Remediation(unittest.TestCase):
    def setUp(self):
        self.temp_dir = Path(tempfile.mkdtemp(prefix="engram_test_b07_"))
        self.candidate_bin = ROOT_DIR / "bin" / "candidate-phase1" / "engram.exe"
        self.codex_bin = Path(r"C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe")
        self.task_b_config_path = ROOT_DIR / "config" / "tasks" / "task_b_remember_me.json"

    def tearDown(self):
        if self.temp_dir.exists():
            shutil.rmtree(self.temp_dir, ignore_errors=True)

    # =========================================================================
    # Fix 1: MCP Approvals Configuration
    # =========================================================================

    def test_build_codex_command_mcp_approvals(self):
        """Verifies that build_codex_command safely enables the 4 allowlisted MCP tools without global bypass."""
        runner = CodexRunner(
            codex_executable=self.codex_bin,
            engram_executable=self.candidate_bin,
            model="gpt-5.6-luna",
            reasoning_effort="medium"
        )
        sandbox = self.temp_dir / "sandbox"
        sandbox.mkdir()

        # 1. Candidate arm: v154_arm3
        cmd = runner.build_codex_command(
            sandbox_path=sandbox,
            prompt="Test prompt",
            arm="v154_arm3"
        )

        cmd_str = " ".join(cmd)

        # Invariant: Sandbox isolation is strictly preserved
        self.assertIn("-s workspace-write", cmd_str)

        # Invariant: Fine-grained auto-approval for engram server
        self.assertIn('mcp_servers.engram.default_tools_approval_mode="approve"', cmd_str)

        # Invariant: Exactly the 4 consolidated tools are enabled
        self.assertIn(
            'mcp_servers.engram.enabled_tools=["engram_get_context", "engram_inspect_symbol", "engram_preflight", "engram_postflight"]',
            cmd_str
        )

        # Invariant: Each of the 4 tools has per-tool approval_mode="approve"
        self.assertIn('mcp_servers.engram.tools.engram_get_context.approval_mode="approve"', cmd_str)
        self.assertIn('mcp_servers.engram.tools.engram_inspect_symbol.approval_mode="approve"', cmd_str)
        self.assertIn('mcp_servers.engram.tools.engram_preflight.approval_mode="approve"', cmd_str)
        self.assertIn('mcp_servers.engram.tools.engram_postflight.approval_mode="approve"', cmd_str)

        # Invariant: Prompt appended at end
        self.assertEqual(cmd[-1], "Test prompt")

        # 2. Baseline arm: baseline_arm1
        baseline_cmd = runner.build_codex_command(
            sandbox_path=sandbox,
            prompt="Baseline prompt",
            arm="baseline_arm1"
        )
        baseline_cmd_str = " ".join(baseline_cmd)

        # Baseline must NEVER have MCP server flags or approvals
        self.assertNotIn("mcp_servers.engram", baseline_cmd_str)
        self.assertIn("-s workspace-write", baseline_cmd_str)
        self.assertEqual(baseline_cmd[-1], "Baseline prompt")

    def test_codex_features_list_with_mcp_config(self):
        """Verifies that installed Codex CLI accepts the 4-tool approval configuration offline."""
        if not self.codex_bin.exists():
            self.skipTest(f"Codex executable not found at {self.codex_bin}")

        runner = CodexRunner(
            codex_executable=self.codex_bin,
            engram_executable=self.candidate_bin,
            model="gpt-5.6-luna"
        )
        sandbox = self.temp_dir / "features_sb"
        sandbox.mkdir()

        cmd = runner.build_codex_command(
            sandbox_path=sandbox,
            prompt="features list",
            arm="v154_arm3"
        )

        # Extract the -c flags for mcp_servers.engram from build_codex_command
        mcp_flags = []
        for i in range(len(cmd)):
            if cmd[i] == "-c" and i + 1 < len(cmd) and "mcp_servers.engram" in cmd[i + 1]:
                mcp_flags.extend(["-c", cmd[i + 1]])

        test_cmd = [str(self.codex_bin)] + mcp_flags + ["features", "list"]

        proc = subprocess.run(
            test_cmd,
            capture_output=True,
            text=True,
            timeout=10,
            check=False
        )

        self.assertEqual(
            proc.returncode, 0,
            f"Codex CLI rejected MCP approval configuration: {proc.stderr}"
        )

    def test_candidate_binary_mcp_tools(self):
        """Verifies that candidate binary exposes exactly the 4 consolidated MCP tools via JSON-RPC."""
        if not self.candidate_bin.exists():
            self.skipTest(f"Candidate binary not found at {self.candidate_bin}")

        sandbox = self.temp_dir / "mcp_sb"
        sandbox.mkdir()

        # Send initialize followed by tools/list
        reqs = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "test", "version": "1.0"}}},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}
        ]
        input_data = "\n".join(json.dumps(r) for r in reqs) + "\n"

        proc = subprocess.run(
            [str(self.candidate_bin), "-C", str(sandbox), "mcp"],
            input=input_data,
            capture_output=True,
            text=True,
            timeout=10,
            check=False
        )

        self.assertEqual(proc.returncode, 0)
        tool_names = []
        for line in proc.stdout.splitlines():
            line = line.strip()
            if not line:
                continue
            try:
                res = json.loads(line)
                if res.get("id") == 2 and "result" in res:
                    tools = res["result"].get("tools", [])
                    tool_names = [t["name"] for t in tools]
            except Exception:
                pass

        expected_tools = ["engram_get_context", "engram_inspect_symbol", "engram_preflight", "engram_postflight"]
        self.assertEqual(sorted(tool_names), sorted(expected_tools))

    # =========================================================================
    # Fix 2: Change Contract Target Files Scope
    # =========================================================================

    def test_task_b_config_contains_five_target_files(self):
        """Verifies that task_b_remember_me.json defines the 5 expected target files."""
        self.assertTrue(self.task_b_config_path.exists())
        with open(self.task_b_config_path, "r", encoding="utf-8") as f:
            cfg = json.load(f)

        self.assertIn("target_files", cfg)
        target_files = cfg["target_files"]
        expected_files = [
            "projects/angular-ngrx-material-starter/src/app/core/auth/auth.actions.ts",
            "projects/angular-ngrx-material-starter/src/app/core/auth/auth.effects.ts",
            "projects/angular-ngrx-material-starter/src/app/app/app.component.ts",
            "projects/angular-ngrx-material-starter/src/app/app/app.component.html",
            "projects/angular-ngrx-material-starter/src/assets/i18n/en.json"
        ]
        self.assertEqual(sorted(target_files), sorted(expected_files))

    def test_inject_with_task_b_target_files(self):
        """Verifies that engram inject with Planned files populates Authorized Targets for all 5 files."""
        if not self.candidate_bin.exists():
            self.skipTest(f"Candidate binary not found at {self.candidate_bin}")

        # Initialize mock git repo
        sb = self.temp_dir / "inject_sb"
        sb.mkdir()
        subprocess.run(["git", "init"], cwd=str(sb), capture_output=True, check=True)
        subprocess.run(["git", "config", "user.name", "Test"], cwd=str(sb), capture_output=True, check=True)
        subprocess.run(["git", "config", "user.email", "test@test.local"], cwd=str(sb), capture_output=True, check=True)

        # Create dummy target files
        with open(self.task_b_config_path, "r", encoding="utf-8") as f:
            task_cfg = json.load(f)
        target_files = task_cfg["target_files"]

        for rel_p in target_files:
            full_p = sb / rel_p
            full_p.parent.mkdir(parents=True, exist_ok=True)
            full_p.write_text(f"// Content of {rel_p}\n", encoding="utf-8")

        subprocess.run(["git", "add", "."], cwd=str(sb), capture_output=True, check=True)
        subprocess.run(["git", "commit", "-m", "Initial commit"], cwd=str(sb), capture_output=True, check=True)

        prompt = task_cfg["prompt"]
        inject_prompt = f"{prompt} Planned files: {', '.join(target_files)}"

        proc = subprocess.run(
            [str(self.candidate_bin), "inject", "-C", str(sb), inject_prompt],
            cwd=str(sb),
            capture_output=True,
            text=True,
            timeout=15,
            check=False
        )
        self.assertEqual(proc.returncode, 0, f"inject failed: {proc.stderr}")

        inst_file = sb / ".github" / "copilot-instructions.md"
        self.assertTrue(inst_file.exists(), "copilot-instructions.md was not generated")
        content = inst_file.read_text(encoding="utf-8")

        # Invariant: Change contract must authorize all 5 target files
        self.assertIn("- Authorized Targets:", content)
        for target in target_files:
            self.assertIn(target, content, f"Authorized Targets missing {target}")

    def test_inject_does_not_mutate_task_config_in_place(self):
        """Verifies that runner.run reads task_config without mutating it in place."""
        runner = CodexRunner(
            codex_executable=self.codex_bin,
            engram_executable=self.candidate_bin,
            model="gpt-5.6-luna"
        )
        sb = self.temp_dir / "sb_nomut"
        sb.mkdir()
        out_ev = self.temp_dir / "events.jsonl"

        original_config = {
            "task_id": "task_b",
            "target_files": ["file1.ts", "file2.ts"],
            "prompt": "Fix something"
        }
        config_copy = json.loads(json.dumps(original_config))

        runner.run(
            sandbox_path=sb,
            prompt="Fix something",
            arm="v154_arm3",
            output_events_path=out_ev,
            is_dry_run=True,
            task_config=config_copy
        )

        self.assertEqual(config_copy, original_config, "task_config was mutated in place!")

    # =========================================================================
    # Fix 3: StreamingTokenTracker Pre-Dispatch Headroom Guard
    # =========================================================================

    def test_streaming_token_tracker_headroom_guard(self):
        """Comprehensive verification of StreamingTokenTracker headroom estimation and boundary logic."""
        tracker = StreamingTokenTracker(conservative_reserve=15_000)

        # 1. Missing / initial usage data (defaults to conservative reserve)
        has_headroom, rem, est = tracker.check_pre_dispatch_headroom(ceiling=250_000)
        self.assertTrue(has_headroom)
        self.assertEqual(rem, 250_000)
        self.assertEqual(est, 15_000)

        # 2. Turn 1 completed with 50,000 tokens (last request tokens: 45,000)
        line_t1 = json.dumps({
            "type": "turn.completed",
            "turn_id": "turn_1",
            "usage": {
                "input_tokens": 45_000,
                "output_tokens": 5_000,
                "total_tokens": 50_000
            }
        })
        tracker.process_line(line_t1)
        self.assertEqual(tracker.effective_tokens, 50_000)
        self.assertEqual(tracker.last_turn_request_tokens, 45_000)
        self.assertEqual(tracker.turns_completed, 1)

        # Next estimate should be max(45_000, 15_000) = 45_000
        self.assertEqual(tracker.estimate_next_request_tokens(), 45_000)
        has_headroom, rem, est = tracker.check_pre_dispatch_headroom(ceiling=250_000)
        self.assertTrue(has_headroom)
        self.assertEqual(rem, 200_000)
        self.assertEqual(est, 45_000)

        # 3. Block 07 exact scenario:
        # Effective tokens: 240,504, last_turn_request_tokens: 29,918, ceiling: 250,000
        b07_tracker = StreamingTokenTracker(conservative_reserve=15_000)
        b07_line = json.dumps({
            "type": "token_usage_record",
            "payload": {
                "thread_token_usage": {
                    "input_tokens": 238_628,
                    "output_tokens": 1_876,
                    "total_tokens": 240_504
                },
                "turn_token_usage": {
                    "input_tokens": 29_918,
                    "output_tokens": 84,
                    "total_tokens": 30_002
                }
            }
        })
        b07_tracker.process_line(b07_line)
        self.assertEqual(b07_tracker.effective_tokens, 240_504)
        self.assertEqual(b07_tracker.last_turn_request_tokens, 29_918)
        self.assertEqual(b07_tracker.estimate_next_request_tokens(), 29_918)

        has_headroom, rem, est = b07_tracker.check_pre_dispatch_headroom(ceiling=250_000)
        self.assertFalse(has_headroom, "Pre-dispatch headroom guard should have flagged insufficient headroom")
        self.assertEqual(rem, 9_496)
        self.assertEqual(est, 29_918)

        # 4. Exact boundary condition (remaining == estimated vs remaining < estimated)
        boundary_tracker = StreamingTokenTracker(conservative_reserve=15_000)
        # Setup: effective_tokens = 85_000, last_turn = 10_000 -> estimate = 15_000
        boundary_line = json.dumps({
            "type": "turn.completed",
            "turn_id": "turn_b",
            "usage": {"input_tokens": 10_000, "total_tokens": 85_000}
        })
        boundary_tracker.process_line(boundary_line)
        self.assertEqual(boundary_tracker.estimate_next_request_tokens(), 15_000)

        # If ceiling is 100_000, remaining = 15_000 == estimated (Allowed)
        has_headroom, rem, est = boundary_tracker.check_pre_dispatch_headroom(ceiling=100_000)
        self.assertTrue(has_headroom)
        self.assertEqual(rem, 15_000)
        self.assertEqual(est, 15_000)

        # If ceiling is 99_999, remaining = 14_999 < estimated (Halted)
        has_headroom, rem, est = boundary_tracker.check_pre_dispatch_headroom(ceiling=99_999)
        self.assertFalse(has_headroom)
        self.assertEqual(rem, 14_999)
        self.assertEqual(est, 15_000)

        # 5. Large previous request consuming majority of budget
        large_tracker = StreamingTokenTracker(conservative_reserve=15_000)
        large_line = json.dumps({
            "type": "turn.completed",
            "turn_id": "turn_large",
            "usage": {"input_tokens": 80_000, "total_tokens": 180_000}
        })
        large_tracker.process_line(large_line)
        has_headroom, rem, est = large_tracker.check_pre_dispatch_headroom(ceiling=250_000)
        self.assertFalse(has_headroom)
        self.assertEqual(rem, 70_000)
        self.assertEqual(est, 80_000)

        # 6. Custom configurable reserve
        custom_tracker = StreamingTokenTracker(conservative_reserve=30_000)
        custom_line = json.dumps({
            "type": "turn.completed",
            "turn_id": "turn_c",
            "usage": {"input_tokens": 20_000, "total_tokens": 20_000}
        })
        custom_tracker.process_line(custom_line)
        self.assertEqual(custom_tracker.estimate_next_request_tokens(), 30_000)

    # =========================================================================
    # Fix 3 Runner Integration: Pre-Dispatch Guard Execution Trigger
    # =========================================================================

    def test_runner_pre_dispatch_guard_triggers(self):
        """Verifies that CodexRunner terminates process tree and records correct error when headroom exhausted."""
        runner = CodexRunner(
            codex_executable=Path(sys.executable),
            engram_executable=self.candidate_bin,
            max_tokens_ceiling=250_000,
            timeout_seconds=10,
            headroom_reserve=15_000
        )
        sandbox = self.temp_dir / "guard_sb"
        sandbox.mkdir()
        events_path = self.temp_dir / "guard_events.jsonl"

        # Mock child process that finishes Turn 1 at 240,504 tokens (request tokens 29,918),
        # then emits turn.started for Turn 2 and blocks
        mock_script = (
            "import sys, time, json\n"
            "sys.stdout.write(json.dumps({'type': 'thread.started', 'thread_id': 'th_guard'}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "t1_complete = {\n"
            "    'type': 'token_usage_record',\n"
            "    'payload': {\n"
            "        'thread_token_usage': {'total_tokens': 240504, 'input_tokens': 238628, 'output_tokens': 1876},\n"
            "        'turn_token_usage': {'total_tokens': 30002, 'input_tokens': 29918, 'output_tokens': 84}\n"
            "    }\n"
            "}\n"
            "sys.stdout.write(json.dumps(t1_complete) + '\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(0.3)\n"
            "sys.stdout.write(json.dumps({'type': 'turn.started'}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(10.0)\n"
        )

        orig_popen = subprocess.Popen
        proc = subprocess.Popen(
            [sys.executable, "-u", "-c", mock_script],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8"
        )

        def mock_popen_filter(*args, **kwargs):
            cmd = args[0] if args else kwargs.get("args")
            cmd_str = str(cmd)
            if "taskkill" in cmd_str or "inject" in cmd_str:
                return orig_popen(*args, **kwargs)
            return proc

        with patch("subprocess.Popen", side_effect=mock_popen_filter):
            res = runner.run(
                sandbox_path=sandbox,
                prompt="test_prompt",
                arm="baseline_arm1",
                output_events_path=events_path,
                is_dry_run=False
            )

        # Invariant checks on RunExecutionResult (Fix 2: Correct classification)
        self.assertEqual(res.exit_code, -1)
        self.assertFalse(res.token_ceiling_breached, "Token ceiling must NOT be marked breached when tokens at stop <= ceiling")
        self.assertTrue(res.pre_dispatch_guard_triggered)
        self.assertEqual(res.stop_reason, "PRE_DISPATCH_HEADROOM_EXHAUSTED")
        self.assertEqual(res.cumulative_tokens_at_stop, 240504)
        self.assertEqual(res.estimated_next_turn_cost, 29918)
        self.assertFalse(res.timed_out)

        # Invariant check on exact error message format
        expected_msg_fragment = "Execution halted by pre-dispatch token headroom guard: 9,496 tokens remaining < estimated next turn cost of 29,918 tokens (budget: 250,000)"
        self.assertIn(expected_msg_fragment, res.error_message or "")

    def test_runner_pre_dispatch_guard_triggers_realistic_multiturn(self):
        """
        Fix 1 & Fix 4: Verifies pre-dispatch guard in a realistic multi-turn lifecycle.
        Codex exec emits 'turn.started' ONLY ONCE at session start.
        Intermediate turns transition via tool events (item.started, item.completed)
        and token_usage_record without recurring 'turn.started'.
        """
        runner = CodexRunner(
            codex_executable=Path(sys.executable),
            engram_executable=self.candidate_bin,
            max_tokens_ceiling=250_000,
            timeout_seconds=10,
            headroom_reserve=15_000
        )
        sandbox = self.temp_dir / "realistic_sb"
        sandbox.mkdir()
        events_path = self.temp_dir / "realistic_events.jsonl"

        # Realistic multi-turn script:
        # Turn 1: Starts with turn.started, executes tool 1, reports 50,000 tokens.
        # Turn 2: Starts tool 2, reports 240,504 cumulative tokens (29,918 input tokens).
        # Tool 2 completes. NO recurring turn.started is emitted. Child attempts to sleep 10s.
        mock_script = (
            "import sys, time, json\n"
            "# Startup (Turn 1)\n"
            "sys.stdout.write(json.dumps({'type': 'thread.started', 'thread_id': 'th_realistic'}) + '\\n')\n"
            "sys.stdout.write(json.dumps({'type': 'turn.started'}) + '\\n')\n"
            "sys.stdout.write(json.dumps({'type': 'item.started', 'item': {'id': 't1', 'type': 'command_execution'}}) + '\\n')\n"
            "sys.stdout.write(json.dumps({'type': 'item.completed', 'item': {'id': 't1', 'type': 'command_execution'}}) + '\\n')\n"
            "sys.stdout.write(json.dumps({'type': 'token_usage_record', 'payload': {'thread_token_usage': {'total_tokens': 50000, 'input_tokens': 45000}}}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(0.1)\n"
            "# Turn 2 (No turn.started emitted! Tool call initiated by model)\n"
            "sys.stdout.write(json.dumps({'type': 'item.started', 'item': {'id': 't2', 'type': 'command_execution'}}) + '\\n')\n"
            "t2_usage = {\n"
            "    'type': 'token_usage_record',\n"
            "    'payload': {\n"
            "        'thread_token_usage': {'total_tokens': 240504, 'input_tokens': 238628, 'output_tokens': 1876},\n"
            "        'turn_token_usage': {'total_tokens': 30002, 'input_tokens': 29918, 'output_tokens': 84}\n"
            "    }\n"
            "}\n"
            "sys.stdout.write(json.dumps(t2_usage) + '\\n')\n"
            "sys.stdout.write(json.dumps({'type': 'item.completed', 'item': {'id': 't2', 'type': 'command_execution'}}) + '\\n')\n"
            "sys.stdout.flush()\n"
            "# Simulates Codex about to dispatch Turn 3 to API\n"
            "time.sleep(10.0)\n"
        )

        orig_popen = subprocess.Popen
        proc = subprocess.Popen(
            [sys.executable, "-u", "-c", mock_script],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8"
        )

        def mock_popen_filter(*args, **kwargs):
            cmd = args[0] if args else kwargs.get("args")
            cmd_str = str(cmd)
            if "taskkill" in cmd_str or "inject" in cmd_str:
                return orig_popen(*args, **kwargs)
            return proc

        with patch("subprocess.Popen", side_effect=mock_popen_filter):
            res = runner.run(
                sandbox_path=sandbox,
                prompt="realistic_prompt",
                arm="v154_arm3",
                output_events_path=events_path,
                is_dry_run=False
            )

        # Invariants: Process halted BEFORE Turn 3 dispatched, classified correctly
        self.assertEqual(res.exit_code, -1)
        self.assertTrue(res.pre_dispatch_guard_triggered)
        self.assertFalse(res.token_ceiling_breached, "Must not report token_ceiling_breached when tokens <= ceiling")
        self.assertEqual(res.stop_reason, "PRE_DISPATCH_HEADROOM_EXHAUSTED")
        self.assertEqual(res.cumulative_tokens_at_stop, 240504)
        self.assertEqual(res.estimated_next_turn_cost, 29918)
        self.assertIn("Execution halted by pre-dispatch token headroom guard", res.error_message or "")

    def test_actual_ceiling_breach_distinct_classification(self):
        """
        Fix 2 & Fix 4: Verifies that a true token ceiling breach (> 250,000)
        is classified as TOKEN_CEILING_BREACHED, with pre_dispatch_guard_triggered = False.
        """
        runner = CodexRunner(
            codex_executable=Path(sys.executable),
            engram_executable=self.candidate_bin,
            max_tokens_ceiling=250_000,
            timeout_seconds=10,
            headroom_reserve=15_000
        )
        sandbox = self.temp_dir / "breach_sb"
        sandbox.mkdir()
        events_path = self.temp_dir / "breach_events.jsonl"

        mock_script = (
            "import sys, time, json\n"
            "sys.stdout.write(json.dumps({'type': 'thread.started', 'thread_id': 'th_breach'}) + '\\n')\n"
            "sys.stdout.write(json.dumps({'type': 'turn.started'}) + '\\n')\n"
            "breach_usage = {\n"
            "    'type': 'token_usage_record',\n"
            "    'payload': {\n"
            "        'thread_token_usage': {'total_tokens': 265000, 'input_tokens': 260000, 'output_tokens': 5000}\n"
            "    }\n"
            "}\n"
            "sys.stdout.write(json.dumps(breach_usage) + '\\n')\n"
            "sys.stdout.flush()\n"
            "time.sleep(10.0)\n"
        )

        orig_popen = subprocess.Popen
        proc = subprocess.Popen(
            [sys.executable, "-u", "-c", mock_script],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8"
        )

        def mock_popen_filter(*args, **kwargs):
            cmd = args[0] if args else kwargs.get("args")
            cmd_str = str(cmd)
            if "taskkill" in cmd_str or "inject" in cmd_str:
                return orig_popen(*args, **kwargs)
            return proc

        with patch("subprocess.Popen", side_effect=mock_popen_filter):
            res = runner.run(
                sandbox_path=sandbox,
                prompt="breach_prompt",
                arm="v154_arm3",
                output_events_path=events_path,
                is_dry_run=False
            )

        self.assertEqual(res.exit_code, -1)
        self.assertTrue(res.token_ceiling_breached, "True breach must set token_ceiling_breached = True")
        self.assertFalse(res.pre_dispatch_guard_triggered, "True breach must not report pre_dispatch_guard_triggered")
        self.assertEqual(res.stop_reason, "TOKEN_CEILING_BREACHED")
        self.assertEqual(res.cumulative_tokens_at_stop, 265000)
        self.assertIn("Token ceiling exceeded", res.error_message or "")

    def test_orchestrator_execution_status_mapping_and_schema(self):
        """
        Fix 2 & Fix 3: Verifies that Orchestrator sets execution_status='PRE_DISPATCH_HEADROOM_EXHAUSTED'
        and serializes a valid headroom_guard block adhering to run_telemetry.schema.json.
        """
        orchestrator = Orchestrator(
            task_config_path=self.task_b_config_path,
            pair_id="pair_test_status",
            is_dry_run=True,
            dependency_strategy="none",
            output_dir=self.temp_dir / "orch_out",
            headroom_reserve=20_000
        )

        # Mock runner return with pre-dispatch guard triggered
        mock_exec_res = RunExecutionResult(
            exit_code=-1,
            elapsed_seconds=12.5,
            started_at="2026-10-29T12:00:00Z",
            completed_at="2026-10-29T12:00:12Z",
            events_log_path=str(self.temp_dir / "events.jsonl"),
            thread_id="th_orch_test",
            model_name="gpt-5.6-luna",
            reasoning_effort="medium",
            harness_executable="codex.exe",
            harness_version="1.0.0",
            path_sanitized=True,
            error_message="Execution halted by pre-dispatch token headroom guard: 9,496 tokens remaining < estimated 29,918",
            pre_dispatch_guard_triggered=True,
            token_ceiling_breached=False,
            stop_reason="PRE_DISPATCH_HEADROOM_EXHAUSTED",
            headroom_reserve=20_000,
            cumulative_tokens_at_stop=240504,
            estimated_next_turn_cost=29918
        )

        # Write dummy events file for telemetry parser
        events_file = orchestrator.output_dir / "v154_arm3" / "events.jsonl"
        events_file.parent.mkdir(parents=True, exist_ok=True)
        events_file.write_text(
            json.dumps({"type": "token_usage_record", "payload": {"thread_token_usage": {"total_tokens": 240504, "input_tokens": 238628, "output_tokens": 1876}}}) + "\n",
            encoding="utf-8"
        )

        with patch.object(orchestrator.runner, "run", return_value=mock_exec_res):
            with patch("isolation.verify_sandbox_identity"), patch("isolation.verify_sandbox_cleanliness"), patch("isolation.teardown_sandbox"), patch("isolation.verify_dependency_integrity"):
                record = orchestrator._execute_single_arm("v154_arm3", 1)

        # Invariant: execution_status must be distinctly PRE_DISPATCH_HEADROOM_EXHAUSTED
        self.assertEqual(record["execution_status"], "PRE_DISPATCH_HEADROOM_EXHAUSTED")
        self.assertIn("headroom_guard", record)
        hg = record["headroom_guard"]
        self.assertEqual(hg["headroom_reserve"], 20_000)
        self.assertTrue(hg["pre_dispatch_guard_triggered"])
        self.assertEqual(hg["stop_reason"], "PRE_DISPATCH_HEADROOM_EXHAUSTED")
        self.assertEqual(hg["cumulative_tokens_at_stop"], 240504)
        self.assertEqual(hg["estimated_next_turn_cost"], 29918)

        # Invariant: Must strictly validate against run_telemetry.schema.json
        validate_instance(record, orchestrator.telemetry_schema)

    def test_headroom_reserve_configurability_and_orchestrator_integration(self):
        """Fix 3: Verifies headroom_reserve is configurable via CLI and Orchestrator."""
        orchestrator = Orchestrator(
            task_config_path=self.task_b_config_path,
            pair_id="pair_cfg_test",
            is_dry_run=True,
            dependency_strategy="none",
            headroom_reserve=35_000
        )
        self.assertEqual(orchestrator.headroom_reserve, 35_000)
        self.assertEqual(orchestrator.runner.headroom_reserve, 35_000)

        # Test skipped arm record carries headroom_reserve
        skipped = orchestrator._create_skipped_arm_record("baseline_arm1", 2, "Test skip")
        self.assertIn("headroom_guard", skipped)
        self.assertEqual(skipped["headroom_guard"]["headroom_reserve"], 35_000)
        self.assertFalse(skipped["headroom_guard"]["pre_dispatch_guard_triggered"])
        validate_instance(skipped, orchestrator.telemetry_schema)

    def test_streaming_token_tracker_is_pre_dispatch_boundary_multiturn(self):
        """Fix 1: Verifies is_pre_dispatch_boundary on individual streaming event lines."""
        tracker = StreamingTokenTracker(conservative_reserve=15_000)

        # 1. turn.started is always a boundary
        self.assertTrue(tracker.is_pre_dispatch_boundary(json.dumps({"type": "turn.started"})))

        # 2. item.started with command_execution or mcp_tool_call is a boundary
        self.assertTrue(tracker.is_pre_dispatch_boundary(json.dumps({
            "type": "item.started", "item": {"id": "c1", "type": "command_execution"}
        })))
        self.assertTrue(tracker.is_pre_dispatch_boundary(json.dumps({
            "type": "item.started", "item": {"id": "c2", "type": "mcp_tool_call"}
        })))

        # 3. item.completed with command_execution is a boundary
        self.assertTrue(tracker.is_pre_dispatch_boundary(json.dumps({
            "type": "item.completed", "item": {"id": "c1", "type": "command_execution"}
        })))

        # 4. custom_tool_call and custom_tool_call_output are boundaries
        self.assertTrue(tracker.is_pre_dispatch_boundary(json.dumps({"type": "custom_tool_call", "call_id": "c3"})))
        self.assertTrue(tracker.is_pre_dispatch_boundary(json.dumps({"type": "custom_tool_call_output", "call_id": "c3"})))

        # 5. Non-tool events before any turns are completed are NOT boundaries
        self.assertFalse(tracker.is_pre_dispatch_boundary(json.dumps({"type": "thread.started", "thread_id": "th1"})))
        self.assertFalse(tracker.is_pre_dispatch_boundary(json.dumps({
            "type": "item.completed", "item": {"id": "msg", "type": "agent_message"}
        })))

    # =========================================================================
    # Additional Guidance: Shell Fallback Guidance
    # =========================================================================

    def test_shell_retrieval_guidance_prompt_injection(self):
        """Verifies that SHELL_RETRIEVAL_GUIDANCE is appended for candidate arm and omitted for baseline arm."""
        runner = CodexRunner(
            codex_executable=self.codex_bin,
            engram_executable=self.candidate_bin,
            model="gpt-5.6-luna"
        )
        sb = self.temp_dir / "guidance_sb"
        sb.mkdir()
        out_ev = self.temp_dir / "events.jsonl"

        # 1. Candidate arm: v154_arm3
        cand_res = runner.run(
            sandbox_path=sb,
            prompt="Candidate task instruction",
            arm="v154_arm3",
            output_events_path=out_ev,
            is_dry_run=True
        )
        self.assertIn(SHELL_RETRIEVAL_GUIDANCE, cand_res.initial_prompt)
        self.assertIn("[Context Retrieval Guidance]", cand_res.initial_prompt)

        # 2. Baseline arm: baseline_arm1
        base_res = runner.run(
            sandbox_path=sb,
            prompt="Baseline task instruction",
            arm="baseline_arm1",
            output_events_path=out_ev,
            is_dry_run=True
        )
        self.assertNotIn(SHELL_RETRIEVAL_GUIDANCE, base_res.initial_prompt)
        self.assertNotIn("[Context Retrieval Guidance]", base_res.initial_prompt)
        self.assertEqual(base_res.initial_prompt, "Baseline task instruction")


if __name__ == "__main__":
    unittest.main()
