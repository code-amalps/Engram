"""
Regression tests for Stage 1 Task B Readiness Fixes (FIX-1 through FIX-4):
1. FIX-1: Turn-0 context injection for Arm 3 (v1.5.4)
2. FIX-2: Balanced, non-coercive MCP tool discovery guidance
3. FIX-3: Sandbox PATH hygiene providing discoverable 'rg' without Engram leaks
4. FIX-4: Telemetry parsing for both mcp_tool_call and custom_tool_call shapes
"""
import unittest
import tempfile
import subprocess
import shutil
import json
import sys
import os
from pathlib import Path

# Setup paths
TESTS_DIR = Path(__file__).resolve().parent
HARNESS_ROOT = TESTS_DIR.parent
SRC_DIR = HARNESS_ROOT / "src"
CONFIG_DIR = HARNESS_ROOT / "config"
FIXTURES_DIR = TESTS_DIR / "fixtures"
ENGRAM_V154_BIN = HARNESS_ROOT / "bin" / "v1.5.4" / "engram.exe"

sys.path.insert(0, str(SRC_DIR))

from runner import CodexRunner, RunExecutionResult
from telemetry import TelemetryParser, TelemetryMetrics
from evaluator import Evaluator, GitImpact
from isolation import create_sandbox, teardown_sandbox


class TestTaskBReadinessFixes(unittest.TestCase):
    def setUp(self):
        self.runner = CodexRunner(
            codex_executable=Path("codex"),
            engram_executable=ENGRAM_V154_BIN,
            model="gpt-5.6-luna",
            reasoning_effort="medium"
        )
        self.parser = TelemetryParser(provenance="SYNTHETIC_TEST")

    # -------------------------------------------------------------------------
    # FIX-1: Turn-0 Context Injection Tests
    # -------------------------------------------------------------------------
    def test_fix1_turn0_injection_deterministic_offline_dry_run(self):
        """Verifies Turn-0 context injection for Arm 3 in a deterministic offline dry run."""
        with tempfile.TemporaryDirectory() as td:
            sandbox_path = Path(td)
            output_events = sandbox_path / "events.jsonl"
            prompt = "Add a remember-me option to the authentication flow"

            # Execute Arm 3 (v1.5.4) in dry-run mode
            res_arm3 = self.runner.run(
                sandbox_path=sandbox_path,
                prompt=prompt,
                arm="v154_arm3",
                output_events_path=output_events,
                is_dry_run=True,
                engram_executable=ENGRAM_V154_BIN
            )

            # 1. Arm 3 must flag Turn-0 as injected
            self.assertTrue(res_arm3.turn0_injected)
            self.assertIsNotNone(res_arm3.initial_prompt)

            # 2. Initial prompt must contain Turn-0 architectural intelligence directives
            self.assertIn("## Engram Turn-0 Task Intelligence & Directives", res_arm3.initial_prompt)
            self.assertIn("Active Change Contract:", res_arm3.initial_prompt)
            self.assertIn("Mandatory AI Agent Directives", res_arm3.initial_prompt)
            self.assertIn("ZERO-TURN EXECUTION:", res_arm3.initial_prompt)
            self.assertIn(prompt, res_arm3.initial_prompt)

            # 3. Dry-run events log must capture the injected initial prompt
            events_content = output_events.read_text(encoding="utf-8")
            self.assertIn("Engram Turn-0 Task Intelligence & Directives", events_content)

    def test_fix1_turn0_injection_omitted_for_baseline_and_v151(self):
        """Verifies Turn-0 context injection is strictly omitted for Arm 1 (Baseline) and Arm 2 (v1.5.1)."""
        with tempfile.TemporaryDirectory() as td:
            sandbox_path = Path(td)
            prompt = "Add a remember-me option"

            # Arm 1 (Baseline)
            res_arm1 = self.runner.run(
                sandbox_path=sandbox_path,
                prompt=prompt,
                arm="baseline_arm1",
                output_events_path=sandbox_path / "events_arm1.jsonl",
                is_dry_run=True
            )
            self.assertFalse(res_arm1.turn0_injected)
            self.assertEqual(res_arm1.initial_prompt, prompt)

            # Arm 2 (v1.5.1)
            res_arm2 = self.runner.run(
                sandbox_path=sandbox_path,
                prompt=prompt,
                arm="v151_arm2",
                output_events_path=sandbox_path / "events_arm2.jsonl",
                is_dry_run=True,
                engram_executable=HARNESS_ROOT / "bin" / "v1.5.1" / "engram.exe"
            )
            self.assertFalse(res_arm2.turn0_injected)
            self.assertEqual(res_arm2.initial_prompt, prompt)

    def test_fix1_engram_binary_inject_produces_contract_directives(self):
        """Executes the actual frozen engram.exe v1.5.4 binary with 'inject' in an isolated git repo."""
        if not ENGRAM_V154_BIN.exists():
            self.skipTest(f"Engram v1.5.4 binary not found at {ENGRAM_V154_BIN}")

        with tempfile.TemporaryDirectory() as td:
            repo_path = Path(td)
            subprocess.run(["git", "init"], cwd=repo_path, check=True, capture_output=True)
            subprocess.run(["git", "config", "user.email", "test@test.com"], cwd=repo_path, check=True)
            subprocess.run(["git", "config", "user.name", "Test Runner"], cwd=repo_path, check=True)
            (repo_path / "sample.txt").write_text("initial commit")
            subprocess.run(["git", "add", "."], cwd=repo_path, check=True)
            subprocess.run(["git", "commit", "-m", "init"], cwd=repo_path, check=True)

            task_desc = "Add remember-me option"
            cmd = [str(ENGRAM_V154_BIN), "inject", "-C", str(repo_path), task_desc]
            res = subprocess.run(cmd, cwd=repo_path, capture_output=True, text=True)
            self.assertEqual(res.returncode, 0, f"engram inject failed: {res.stderr}")

            # Verify instruction file was created
            inst_file = repo_path / ".github" / "copilot-instructions.md"
            self.assertTrue(inst_file.exists())
            content = inst_file.read_text(encoding="utf-8")
            self.assertIn("Engram Turn-0 Task Intelligence & Directives", content)
            self.assertIn("Active Change Contract:", content)
            self.assertIn("ZERO-TURN EXECUTION:", content)

    def test_fix1_evaluator_ignores_injected_turn0_files(self):
        """Verifies that analyze_git_impact ignores .github/copilot-instructions.md and .engram/."""
        with tempfile.TemporaryDirectory() as td:
            repo_path = Path(td)
            subprocess.run(["git", "init"], cwd=repo_path, check=True, capture_output=True)
            subprocess.run(["git", "config", "user.email", "test@test.com"], cwd=repo_path, check=True)
            subprocess.run(["git", "config", "user.name", "Test"], cwd=repo_path, check=True)
            (repo_path / "allowed.ts").write_text("export const x = 1;")
            subprocess.run(["git", "add", "."], cwd=repo_path, check=True)
            subprocess.run(["git", "commit", "-m", "init"], cwd=repo_path, check=True)

            # Injected infrastructure files created by Engram / harness
            (repo_path / ".github").mkdir(parents=True, exist_ok=True)
            (repo_path / ".github" / "copilot-instructions.md").write_text("turn-0 instructions")
            (repo_path / ".engram").mkdir(parents=True, exist_ok=True)
            (repo_path / ".engram" / "contract.yaml").write_text("contract: 1")

            # Agent makes allowed modification
            (repo_path / "allowed.ts").write_text("export const x = 2;")

            evaluator = Evaluator({
                "task_id": "test_task",
                "allowed_file_patterns": ["allowed.ts"],
                "forbidden_file_patterns": ["forbidden.ts"]
            })

            patch_path = repo_path / "test.patch"
            impact = evaluator.analyze_git_impact(repo_path, patch_path)

            # Changed files must ONLY include allowed.ts (infrastructure files must be ignored)
            self.assertEqual(impact.files_changed, ["allowed.ts"])
            self.assertEqual(impact.unrelated_files_count, 0)
            self.assertNotIn(".github/copilot-instructions.md", impact.files_changed)

    # -------------------------------------------------------------------------
    # FIX-2: MCP Tool Discovery Guidance Tests
    # -------------------------------------------------------------------------
    def test_fix2_task_b_prompt_discovery_guidance(self):
        """Verifies Task B prompt includes balanced, non-coercive tool awareness guidance."""
        task_b_cfg_path = CONFIG_DIR / "tasks" / "task_b_remember_me.json"
        with open(task_b_cfg_path, "r", encoding="utf-8") as f:
            cfg = json.load(f)

        prompt = cfg.get("prompt", "")

        # 1. Must convey core task requirement
        self.assertIn("Add a remember-me option to the authentication flow", prompt)
        self.assertIn("persists the username across reloads", prompt)

        # 2. Must provide permissive, non-coercive tool awareness
        self.assertIn("available tools", prompt)
        self.assertIn("registered Engram MCP tools if enabled", prompt)
        self.assertIn("standard search commands", prompt)

        # 3. Must NOT force mandatory tool usage
        self.assertNotIn("you must call engram", prompt.lower())
        self.assertNotIn("mandatory: call engram", prompt.lower())
        self.assertNotIn("required to use engram", prompt.lower())

        # 4. Critical safety instruction preserved
        self.assertIn("CRITICAL SAFETY INSTRUCTION: Do NOT run npm install", prompt)

    # -------------------------------------------------------------------------
    # FIX-3: Sandbox PATH Ripgrep Discoverability Tests
    # -------------------------------------------------------------------------
    def test_fix3_sandbox_path_rg_discoverable(self):
        """Verifies that ripgrep ('rg') is discoverable in child process PATH for all arms."""
        sanitized_env, was_sanitized = self.runner.sanitize_path()

        # 1. Engram must NEVER be present in PATH
        for p in sanitized_env["PATH"].split(os.pathsep):
            self.assertNotIn("engram", p.lower(), f"Engram directory leaked in PATH: {p}")

        # 2. 'rg' must be discoverable in the sanitized PATH
        rg_path = shutil.which("rg", path=sanitized_env["PATH"])
        self.assertIsNotNone(rg_path, "ripgrep ('rg') could not be discovered in sanitized PATH!")

        # 3. Verify 'rg' executes successfully
        proc = subprocess.run([rg_path, "--version"], capture_output=True, text=True)
        self.assertEqual(proc.returncode, 0)
        self.assertIn("ripgrep", proc.stdout.lower())

    def test_fix3_path_sanitization_removes_engram_while_retaining_rg(self):
        """Verifies that dirty PATH containing Engram is sanitized while maintaining rg access."""
        fake_dirty_path = (
            r"C:\Users\Amal P S\AppData\Local\Programs\Engram\bin"
            + os.pathsep
            + r"C:\Windows\System32"
            + os.pathsep
            + r"C:\Program Files\Engram"
        )
        test_env = {"PATH": fake_dirty_path}
        sanitized_env, was_sanitized = self.runner.sanitize_path(test_env)

        self.assertTrue(was_sanitized)
        for p in sanitized_env["PATH"].split(os.pathsep):
            self.assertNotIn("engram", p.lower())

        # rg must still be present
        rg_path = shutil.which("rg", path=sanitized_env["PATH"])
        self.assertIsNotNone(rg_path)

    # -------------------------------------------------------------------------
    # FIX-4: Telemetry Parsing for Both Event Shapes Tests
    # -------------------------------------------------------------------------
    def test_fix4_parse_events_mcp_tool_call_shape(self):
        """Verifies parse_events parses standard mcp_tool_call shape."""
        events = [
            {"type": "thread.started", "thread_id": "test_mcp"},
            {"type": "turn.started"},
            {
                "type": "item.completed",
                "item": {
                    "id": "call_mcp_1",
                    "type": "mcp_tool_call",
                    "server": "engram",
                    "tool": "engram_prepare_task",
                    "arguments": {"task": "test task"},
                    "result": {"content": [{"type": "text", "text": "prepared"}]}
                }
            },
            {"type": "turn.completed", "usage": {"input_tokens": 100, "output_tokens": 20}}
        ]
        metrics = self.parser.parse_events(events)
        self.assertEqual(metrics.mcp_tool_calls_count, 1)
        self.assertGreater(metrics.auxiliary_tool_payload_bytes, 0)

    def test_fix4_parse_events_custom_tool_call_shape(self):
        """Verifies parse_events parses custom_tool_call shape."""
        events = [
            {"type": "thread.started", "thread_id": "test_custom"},
            {"type": "turn.started"},
            {
                "type": "item.completed",
                "item": {
                    "id": "call_custom_1",
                    "type": "custom_tool_call",
                    "name": "engram_doctor",
                    "input": {"mode": "deep"},
                    "output": {"status": "ok", "memories": 12}
                }
            },
            {"type": "turn.completed", "usage": {"input_tokens": 120, "output_tokens": 30}}
        ]
        metrics = self.parser.parse_events(events)
        self.assertEqual(metrics.mcp_tool_calls_count, 1)
        self.assertGreater(metrics.auxiliary_tool_payload_bytes, 0)

    def test_fix4_parse_events_deduplication(self):
        """Verifies tool calls with duplicate IDs are strictly counted once."""
        events = [
            {"type": "item.completed", "item": {"id": "tool_x", "type": "mcp_tool_call", "arguments": {"q": 1}}},
            {"type": "item.completed", "item": {"id": "tool_x", "type": "mcp_tool_call", "arguments": {"q": 1}}},
            {"type": "item.completed", "item": {"id": "tool_y", "type": "custom_tool_call", "input": {"q": 2}}},
            {"type": "item.completed", "item": {"id": "tool_y", "type": "custom_tool_call", "input": {"q": 2}}}
        ]
        metrics = self.parser.parse_events(events)
        self.assertEqual(metrics.mcp_tool_calls_count, 2)

    def test_fix4_parse_rollout_file_real_session(self):
        """Verifies parse_rollout_file extracts the MCP tool call from real_session_rollout.jsonl."""
        rollout_file = FIXTURES_DIR / "real_session_rollout.jsonl"
        metrics = self.parser.parse_rollout_file(rollout_file)

        # In real_session_rollout.jsonl, engram_doctor is called via McpToolCall
        self.assertEqual(metrics.mcp_tool_calls_count, 1)
        self.assertGreater(metrics.auxiliary_tool_payload_bytes, 0)
        self.assertEqual(metrics.prompt_tokens_total, 47541)
        self.assertEqual(metrics.completion_tokens, 83)
        self.assertEqual(metrics.peak_single_request_context, 16022)

    def test_fix4_parse_file_real_handshake_and_synthetic(self):
        """Verifies parse_file retains full parity on existing fixtures."""
        m_handshake = self.parser.parse_file(FIXTURES_DIR / "real_handshake.jsonl")
        self.assertEqual(m_handshake.mcp_tool_calls_count, 1)
        self.assertEqual(m_handshake.auxiliary_tool_payload_bytes, 431)

        m_synthetic = self.parser.parse_file(FIXTURES_DIR / "synthetic_multiturn.jsonl")
        self.assertEqual(m_synthetic.mcp_tool_calls_count, 2)
        self.assertEqual(m_synthetic.auxiliary_tool_payload_bytes, 386)


if __name__ == "__main__":
    unittest.main()
