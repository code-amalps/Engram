"""
Pilot 03A Automated Compatibility & Unit Test Suite.
Validates local runtime harness extensions across all nine required areas:
1. Default cloud command generation.
2. Ollama command generation.
3. LM Studio command generation.
4. Provider and model argument validation.
5. Local reasoning-configuration handling.
6. Missing optional telemetry fields (cached_input_tokens, reasoning_output_tokens, prompt_tokens aliases).
7. Provider metadata serialization and schema compliance.
8. Existing cloud backward compatibility.
9. Control and treatment configuration consistency.
"""
import unittest
import tempfile
import shutil
import json
from pathlib import Path

import sys
HARNESS_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(HARNESS_ROOT / "src"))

from runner import CodexRunner, StreamingTokenTracker, RunExecutionResult, SUPPORTED_PROVIDERS
from orchestrator import Orchestrator
from telemetry import TelemetryParser, TelemetryMetrics, TokenReplacementAccounting
from schema_validator import validate_instance


class TestPilot03ALocalRuntime(unittest.TestCase):
    def setUp(self):
        self.temp_dir = Path(tempfile.mkdtemp(prefix="engram_test_03a_"))
        self.schemas_dir = HARNESS_ROOT / "schemas"
        with open(self.schemas_dir / "run_telemetry.schema.json", "r", encoding="utf-8") as f:
            self.telemetry_schema = json.load(f)
        with open(self.schemas_dir / "matched_pair_summary.schema.json", "r", encoding="utf-8") as f:
            self.pair_schema = json.load(f)

        self.mock_codex = self.temp_dir / "mock_codex.exe"
        self.mock_codex.write_text("mock binary", encoding="utf-8")
        self.mock_engram = self.temp_dir / "mock_engram.exe"
        self.mock_engram.write_text("mock engram", encoding="utf-8")

        self.sandbox = self.temp_dir / "sandbox"
        self.sandbox.mkdir(parents=True)

    def tearDown(self):
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    # -------------------------------------------------------------------------
    # TEST 1: Default Cloud Command Generation
    # -------------------------------------------------------------------------
    def test_default_cloud_command_generation(self):
        runner = CodexRunner(
            codex_executable=self.mock_codex,
            model="gpt-5.6-luna",
            reasoning_effort="medium"
        )
        self.assertEqual(runner.provider, "cloud")
        cmd = runner.build_codex_command(
            sandbox_path=self.sandbox,
            prompt="Test prompt",
            arm="control_A"
        )
        self.assertIn("-m", cmd)
        self.assertIn("gpt-5.6-luna", cmd)
        self.assertIn('-c', cmd)
        self.assertIn('collaboration_mode.settings.reasoning_effort="medium"', cmd)
        self.assertNotIn("--oss", cmd)
        self.assertNotIn("--local-provider", cmd)

    # -------------------------------------------------------------------------
    # TEST 2: Ollama Command Generation
    # -------------------------------------------------------------------------
    def test_ollama_command_generation(self):
        runner = CodexRunner(
            codex_executable=self.mock_codex,
            provider="ollama",
            model="qwen2.5-coder:7b"
        )
        self.assertEqual(runner.provider, "ollama")
        self.assertEqual(runner.model, "qwen2.5-coder:7b")
        cmd = runner.build_codex_command(
            sandbox_path=self.sandbox,
            prompt="Test prompt",
            arm="control_A"
        )
        self.assertIn("--oss", cmd)
        self.assertIn("--local-provider", cmd)
        self.assertIn("ollama", cmd)
        self.assertIn("-m", cmd)
        self.assertIn("qwen2.5-coder:7b", cmd)
        # Reasoning effort must be omitted for local provider
        self.assertNotIn('collaboration_mode.settings.reasoning_effort="medium"', cmd)

    # -------------------------------------------------------------------------
    # TEST 3: LM Studio Command Generation
    # -------------------------------------------------------------------------
    def test_lmstudio_command_generation(self):
        runner = CodexRunner(
            codex_executable=self.mock_codex,
            provider="lmstudio",
            model="qwen2.5-coder:14b"
        )
        self.assertEqual(runner.provider, "lmstudio")
        cmd = runner.build_codex_command(
            sandbox_path=self.sandbox,
            prompt="Test prompt",
            arm="control_A"
        )
        self.assertIn("--oss", cmd)
        self.assertIn("--local-provider", cmd)
        self.assertIn("lmstudio", cmd)
        self.assertIn("-m", cmd)
        self.assertIn("qwen2.5-coder:14b", cmd)
        self.assertNotIn('collaboration_mode.settings.reasoning_effort', " ".join(cmd))

    # -------------------------------------------------------------------------
    # TEST 4: Provider & Model Argument Validation
    # -------------------------------------------------------------------------
    def test_provider_argument_validation(self):
        # Valid providers
        for p in ("cloud", "ollama", "lmstudio"):
            runner = CodexRunner(codex_executable=self.mock_codex, provider=p)
            self.assertEqual(runner.provider, p)

        # Invalid providers
        for invalid_p in ("vllm", "llama_cpp", "openai", "local", "", None):
            with self.assertRaises(ValueError):
                CodexRunner(codex_executable=self.mock_codex, provider=invalid_p)

    # -------------------------------------------------------------------------
    # TEST 5: Local Reasoning-Configuration Handling
    # -------------------------------------------------------------------------
    def test_local_reasoning_configuration_handling(self):
        # Cloud runner includes reasoning override
        cloud_runner = CodexRunner(codex_executable=self.mock_codex, provider="cloud", reasoning_effort="high")
        cloud_cmd = cloud_runner.build_codex_command(sandbox_path=self.sandbox, prompt="P", arm="control_A")
        self.assertIn('collaboration_mode.settings.reasoning_effort="high"', cloud_cmd)

        # Local Ollama runner omits reasoning override
        ollama_runner = CodexRunner(codex_executable=self.mock_codex, provider="ollama", reasoning_effort="high")
        ollama_cmd = ollama_runner.build_codex_command(sandbox_path=self.sandbox, prompt="P", arm="control_A")
        self.assertNotIn('collaboration_mode.settings.reasoning_effort="high"', ollama_cmd)
        self.assertNotIn('reasoning_effort', " ".join(ollama_cmd))

    # -------------------------------------------------------------------------
    # TEST 6: Missing Optional Telemetry Fields (Ollama / Local Provider Simulation)
    # -------------------------------------------------------------------------
    def test_missing_optional_telemetry_fields(self):
        # 1. Test StreamingTokenTracker with missing cached_input_tokens and reasoning_output_tokens
        tracker = StreamingTokenTracker(conservative_reserve=15000)
        local_event = json.dumps({
            "type": "token_usage_record",
            "payload": {
                "thread_token_usage": {
                    "input_tokens": 12000,
                    "output_tokens": 450,
                    "total_tokens": 12450
                    # cached_input_tokens is omitted
                    # reasoning_output_tokens is omitted
                },
                "usage": {
                    "input_tokens": 12000,
                    "output_tokens": 450
                }
            }
        })
        tracker.process_line(local_event)
        self.assertTrue(tracker.telemetry_valid)
        self.assertEqual(tracker.prompt_tokens_total, 12000)
        self.assertEqual(tracker.prompt_tokens_cached, 0)
        self.assertEqual(tracker.prompt_tokens_uncached, 12000)
        self.assertEqual(tracker.completion_tokens, 450)
        self.assertEqual(tracker.reasoning_tokens, 0)
        self.assertEqual(tracker.effective_tokens, 12450)

        # 2. Test TelemetryParser with rollout containing prompt_tokens / completion_tokens alias
        rollout_file = self.temp_dir / "rollout_test.jsonl"
        with open(rollout_file, "w", encoding="utf-8") as f:
            f.write(json.dumps({
                "type": "token_usage_record",
                "payload": {
                    "thread_token_usage": {
                        "prompt_tokens": 8500,
                        "completion_tokens": 300,
                        "total_tokens": 8800
                    },
                    "usage": {
                        "prompt_tokens": 8500,
                        "completion_tokens": 300
                    }
                }
            }) + "\n")

        parser = TelemetryParser(sessions_dir=self.temp_dir)
        metrics = parser.parse_rollout_file(rollout_file)
        self.assertTrue(metrics.telemetry_valid)
        self.assertEqual(metrics.prompt_tokens_total, 8500)
        self.assertEqual(metrics.prompt_tokens_cached, 0)
        self.assertTrue(metrics.prompt_tokens_cached_inferred)
        self.assertEqual(metrics.prompt_tokens_uncached, 8500)
        self.assertTrue(metrics.prompt_tokens_uncached_inferred)
        self.assertEqual(metrics.completion_tokens, 300)
        self.assertEqual(metrics.reasoning_tokens, 0)
        self.assertEqual(metrics.total_model_tokens, 8800)

    # -------------------------------------------------------------------------
    # TEST 7: Provider Metadata Serialization & Schema Compliance
    # -------------------------------------------------------------------------
    def test_provider_metadata_serialization(self):
        task_config_file = self.temp_dir / "task_mock.json"
        with open(task_config_file, "w", encoding="utf-8") as f:
            json.dump({
                "task_id": "task_mock",
                "repository_type": "rust",
                "source_repo": str(self.temp_dir),
                "baseline_commit": "c70845b4d5913457f1b176fb0370b80dd792711f",
                "validation_commands": ["cargo check"]
            }, f)

        # Create dummy Cargo.toml
        (self.temp_dir / "Cargo.toml").write_text('[package]\nname = "test"\nversion = "0.1.0"\n', encoding="utf-8")

        orch = Orchestrator(
            task_config_path=task_config_file,
            pair_id="pilot_test",
            golden_repo=self.temp_dir,
            codex_bin=self.mock_codex,
            engram_bin=self.mock_engram,
            provider="ollama",
            model="qwen2.5-coder:7b",
            is_dry_run=True,
            output_dir=self.temp_dir / "out"
        )
        self.assertEqual(orch.provider, "ollama")
        self.assertEqual(orch.runner.provider, "ollama")

        # Test skipped run serialization schema compliance
        skipped = orch._create_skipped_arm_record(arm="control_A", exec_order=2, reason="Pre-flight skipped")
        self.assertEqual(skipped["model"]["provider"], "ollama")
        self.assertEqual(skipped["model"]["name"], "qwen2.5-coder:7b")
        self.assertEqual(skipped["model"]["reasoning_effort"], "none")
        validate_instance(skipped, self.telemetry_schema, "run_telemetry.schema.json")

    # -------------------------------------------------------------------------
    # TEST 8: Existing Cloud Backward Compatibility
    # -------------------------------------------------------------------------
    def test_existing_cloud_backward_compatibility(self):
        task_config_file = self.temp_dir / "task_mock_cloud.json"
        with open(task_config_file, "w", encoding="utf-8") as f:
            json.dump({
                "task_id": "task_mock_cloud",
                "repository_type": "rust",
                "source_repo": str(self.temp_dir),
                "baseline_commit": "c70845b4d5913457f1b176fb0370b80dd792711f",
                "validation_commands": ["cargo check"]
            }, f)

        # Omit provider parameter: must default to cloud
        orch = Orchestrator(
            task_config_path=task_config_file,
            pair_id="pilot_cloud",
            golden_repo=self.temp_dir,
            codex_bin=self.mock_codex,
            engram_bin=self.mock_engram,
            is_dry_run=True,
            output_dir=self.temp_dir / "out_cloud"
        )
        self.assertEqual(orch.provider, "cloud")
        self.assertEqual(orch.runner.provider, "cloud")
        self.assertEqual(orch.model, "gpt-5.6-luna")

        skipped = orch._create_skipped_arm_record(arm="control_A", exec_order=2, reason="Pre-flight skipped")
        # For cloud, provider must serialize as "openai" for backward compatibility
        self.assertEqual(skipped["model"]["provider"], "openai")
        self.assertEqual(skipped["model"]["reasoning_effort"], "medium")
        validate_instance(skipped, self.telemetry_schema, "run_telemetry.schema.json")

    # -------------------------------------------------------------------------
    # TEST 9: Control and Treatment Configuration Consistency
    # -------------------------------------------------------------------------
    def test_control_and_treatment_configuration_consistency(self):
        runner = CodexRunner(
            codex_executable=self.mock_codex,
            engram_executable=self.mock_engram,
            provider="ollama",
            model="qwen2.5-coder:7b"
        )
        ctrl_cmd = runner.build_codex_command(self.sandbox, "Prompt A", arm="control_A")
        treat_cmd = runner.build_codex_command(self.sandbox, "Prompt B", arm="treatment_B")

        # Both arms must receive identical provider and model
        self.assertIn("--oss", ctrl_cmd)
        self.assertIn("--oss", treat_cmd)
        self.assertIn("ollama", ctrl_cmd)
        self.assertIn("ollama", treat_cmd)
        self.assertIn("qwen2.5-coder:7b", ctrl_cmd)
        self.assertIn("qwen2.5-coder:7b", treat_cmd)


if __name__ == "__main__":
    unittest.main()
