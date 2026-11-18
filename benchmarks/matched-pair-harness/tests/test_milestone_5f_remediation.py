"""
Milestone 5F Targeted Offline Regression Test Suite.
Validates all Milestone 5F remediations across Workstreams A, B, and C:
1. Workstream A: MCP initialization (.codex/config.toml generation and sandbox whitelist).
2. Workstream A: Pre-dispatch token guard accounting (preventing cumulative-for-request attribution bug).
3. Workstream A: Turn-0 context injection & shell retrieval guidance for treatment_B.
4. Workstream A: Skipped-arm telemetry serialization, null acceptance parity, and verdict classification.
5. Workstream C: Token replacement accounting from Codex rollout/stdout events and schema validation.
"""
import unittest
import tempfile
import shutil
import json
import subprocess
from pathlib import Path

# Harness imports
import sys
HARNESS_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(HARNESS_ROOT / "src"))

from isolation import configure_sandbox_codex_mcp, verify_sandbox_cleanliness
from runner import StreamingTokenTracker, CodexRunner, SHELL_RETRIEVAL_GUIDANCE
from orchestrator import Orchestrator
from telemetry import TelemetryParser, TelemetryMetrics, TokenReplacementAccounting
from evaluator import CorrectnessResult, GitImpact
from schema_validator import validate_instance


class TestMilestone5FRemediation(unittest.TestCase):
    def setUp(self):
        self.temp_dir = Path(tempfile.mkdtemp(prefix="engram_test_5f_"))
        self.schemas_dir = HARNESS_ROOT / "schemas"
        with open(self.schemas_dir / "run_telemetry.schema.json", "r", encoding="utf-8") as f:
            self.telemetry_schema = json.load(f)
        with open(self.schemas_dir / "matched_pair_summary.schema.json", "r", encoding="utf-8") as f:
            self.pair_schema = json.load(f)

    def tearDown(self):
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    # -------------------------------------------------------------------------
    # TEST 1: Workstream A - Sandbox .codex/config.toml Generation & Cleanliness
    # -------------------------------------------------------------------------
    def test_configure_sandbox_codex_mcp(self):
        sandbox = self.temp_dir / "sandbox_test"
        sandbox.mkdir(parents=True)
        # Initialize git repo in sandbox so git status check succeeds
        subprocess.run(["git", "init"], cwd=str(sandbox), capture_output=True, check=True)

        engram_bin = self.temp_dir / "engram.exe"
        engram_bin.write_text("binary placeholder", encoding="utf-8")

        config_path = configure_sandbox_codex_mcp(sandbox, engram_bin)
        self.assertTrue(config_path.exists())
        self.assertEqual(config_path, sandbox / ".codex" / "config.toml")

        content = config_path.read_text(encoding="utf-8")
        self.assertIn("[mcp_servers.engram]", content)
        self.assertIn('command =', content)
        self.assertIn('args = ["-C",', content)
        self.assertIn('"mcp"]', content)
        self.assertIn('default_tools_approval_mode = "approve"', content)

        # Verify sandbox cleanliness check permits .codex and .codex/config.toml
        allowed_found = verify_sandbox_cleanliness(sandbox)
        normalized_allowed = [p.rstrip("/") for p in allowed_found]
        self.assertTrue(any(item in normalized_allowed for item in (".codex", ".codex/config.toml")))

    # -------------------------------------------------------------------------
    # TEST 2: Workstream A - Token Guard Multi-Turn Variable Attribution Fix
    # -------------------------------------------------------------------------
    def test_streaming_token_tracker_multi_turn_accounting(self):
        """
        Validates that token_usage_record events extract single-request usage from
        'usage' and thread cumulative from 'thread_token_usage', avoiding the 115k bug.
        """
        tracker = StreamingTokenTracker(conservative_reserve=45000)

        # Turn 1: request input_tokens = 41011, cumulative = 42211
        turn1_line = json.dumps({
            "type": "token_usage_record",
            "payload": {
                "usage": {
                    "input_tokens": 41011,
                    "output_tokens": 1200,
                    "total_tokens": 42211
                },
                "thread_token_usage": {
                    "input_tokens": 41011,
                    "output_tokens": 1200,
                    "total_tokens": 42211
                }
            }
        })
        tracker.process_line(turn1_line)

        # Verify tracker attributes single turn usage, not thread cumulative
        self.assertEqual(tracker.last_turn_request_tokens, 41011)
        self.assertEqual(tracker.effective_tokens, 42211)
        # Next request estimate should be max(41011, 45000) = 45000
        estimated = tracker.estimate_next_request_tokens()
        self.assertEqual(estimated, 45000)

        # Turn 2: request input_tokens = 38000, cumulative now 115992
        turn2_line = json.dumps({
            "type": "token_usage_record",
            "payload": {
                "usage": {
                    "input_tokens": 38000,
                    "output_tokens": 1200,
                    "total_tokens": 39200
                },
                "thread_token_usage": {
                    "input_tokens": 112992,
                    "output_tokens": 3000,
                    "total_tokens": 115992
                }
            }
        })
        tracker.process_line(turn2_line)

        # Cumulative is 115,992. Last request is 38,000 (NOT 112,992 or 115,992!)
        self.assertEqual(tracker.last_turn_request_tokens, 38000)
        self.assertEqual(tracker.effective_tokens, 115992)
        # Next turn estimate: max(38000, 45000) = 45000
        # Projected total: 115992 + 45000 = 160992 <= 175000 ceiling. Headroom remains True!
        has_headroom, remaining, est_cost = tracker.check_pre_dispatch_headroom(175000)
        self.assertTrue(has_headroom)
        self.assertEqual(est_cost, 45000)
        self.assertEqual(remaining, 175000 - 115992)

    def test_streaming_token_tracker_correctly_halts_at_genuine_breach(self):
        """
        Verify guard halts when cumulative usage genuinely exceeds effective ceiling.
        """
        tracker = StreamingTokenTracker(conservative_reserve=45000)
        # Turn with cumulative usage near ceiling
        near_ceiling_line = json.dumps({
            "type": "token_usage_record",
            "payload": {
                "usage": {
                    "input_tokens": 50000,
                    "output_tokens": 2000,
                    "total_tokens": 52000
                },
                "thread_token_usage": {
                    "input_tokens": 130000,
                    "output_tokens": 5000,
                    "total_tokens": 135000
                }
            }
        })
        tracker.process_line(near_ceiling_line)
        # Ceiling: 175,000. Cumulative: 135,000. Remaining: 40,000.
        # Estimated next turn: max(50000, 45000) = 50,000.
        # 40,000 < 50,000 -> has_headroom MUST be False!
        has_headroom, remaining, est_cost = tracker.check_pre_dispatch_headroom(175000)
        self.assertFalse(has_headroom)
        self.assertEqual(remaining, 40000)
        self.assertEqual(est_cost, 50000)

    # -------------------------------------------------------------------------
    # TEST 3: Workstream A - Turn-0 Context Injection for treatment_B
    # -------------------------------------------------------------------------
    def test_turn0_guidance_present_in_runner_constants(self):
        self.assertIn("Always prefer Engram MCP tools", SHELL_RETRIEVAL_GUIDANCE)
        self.assertIn("engram_inspect_symbol", SHELL_RETRIEVAL_GUIDANCE)
        self.assertIn("do NOT dump entire files into the prompt", SHELL_RETRIEVAL_GUIDANCE)

    # -------------------------------------------------------------------------
    # TEST 4: Workstream A - Skipped Arm Telemetry & Parity Schema Compatibility
    # -------------------------------------------------------------------------
    def test_skipped_arm_telemetry_schema_validation(self):
        task_config = HARNESS_ROOT / "config" / "tasks" / "task_commit_cache_invalidation.json"
        orch = Orchestrator(
            task_config_path=task_config,
            pair_id="pair_5f_test",
            dependency_strategy="none",
            output_dir=self.temp_dir / "orch_out",
            is_dry_run=True
        )

        record = orch._create_skipped_arm_record(
            arm="control_A",
            exec_order=1,
            reason="Pre-dispatch guard halted execution",
            execution_status="SKIPPED_STOP_ON_FAILURE"
        )

        # Check null attributes
        self.assertIsNone(record["correctness"]["score"])
        self.assertIsNone(record["correctness"]["quality_adjusted_cost"])
        self.assertIsNone(record["correctness"]["passed_all_requirements"])
        self.assertEqual(record["execution_status"], "SKIPPED_STOP_ON_FAILURE")

        # Validate strictly against run_telemetry schema
        validate_instance(record, self.telemetry_schema)

    def test_comparative_analysis_verdict_for_skipped_arm(self):
        telemetry_parser = TelemetryParser(provenance="SYNTHETIC_TEST")
        unavail = telemetry_parser.create_unavailable_metrics("Skipped")

        treatment_run = {
            "execution_status": "FAILED",
            "telemetry": unavail.to_dict(),
            "correctness": {
                "score": 0.0,
                "quality_adjusted_cost": 0.0,
                "passed_all_requirements": False
            }
        }
        control_run = {
            "execution_status": "SKIPPED_STOP_ON_FAILURE",
            "telemetry": unavail.to_dict(),
            "correctness": {
                "score": None,
                "quality_adjusted_cost": None,
                "passed_all_requirements": None
            }
        }

        analysis = Orchestrator.compute_comparative_analysis(
            ctrl_run=control_run,
            treat_run=treatment_run
        )

        self.assertEqual(analysis["verdict"], "TREATMENT_FAILED_CONTROL_SKIPPED")
        self.assertFalse(analysis["savings_claim_eligible"])
        self.assertIn("skipped", analysis["explanation"].lower())

    # -------------------------------------------------------------------------
    # TEST 5: Workstream C - Token Replacement Accounting & Event Parsing
    # -------------------------------------------------------------------------
    def test_telemetry_replacement_accounting_from_events(self):
        events = [
            {
                "type": "turn.completed",
                "usage": {
                    "input_tokens": 12000,
                    "output_tokens": 300,
                    "total_tokens": 12300
                }
            },
            # Engram MCP tool call
            {
                "type": "item.completed",
                "item": {
                    "id": "tool_engram_1",
                    "type": "mcp_tool_call",
                    "name": "engram_inspect_symbol",
                    "arguments": {"name": "authLogout", "path": "src/app/core/auth/auth.actions.ts"},
                    "result": {
                        "symbol": "authLogout",
                        "line_range": "L25..L30",
                        "completeness": "COMPLETE_BLOCK",
                        "code_excerpt": "export const authLogout = createAction('[Auth] Logout');"
                    }
                }
            },
            # Subsequent shell read of the same file
            {
                "type": "item.completed",
                "item": {
                    "id": "cmd_cat_1",
                    "type": "command_execution",
                    "name": "bash",
                    "command": "cat src/app/core/auth/auth.actions.ts"
                }
            }
        ]

        parser = TelemetryParser(provenance="SYNTHETIC_TEST")
        metrics = parser.parse_events(events)

        self.assertTrue(metrics.telemetry_valid)
        self.assertEqual(metrics.mcp_tool_calls_count, 1)
        self.assertIsNotNone(metrics.replacement_accounting)

        acct = metrics.replacement_accounting
        self.assertIn("src/app/core/auth/auth.actions.ts", acct.files_retrieved_via_engram)
        self.assertIn("src/app/core/auth/auth.actions.ts", acct.files_read_via_shell)
        self.assertIn("src/app/core/auth/auth.actions.ts", acct.files_repeated_read)
        self.assertEqual(acct.engram_excerpts_count, 1)
        self.assertEqual(acct.engram_excerpts_reread_count, 1)
        self.assertGreater(acct.serialized_invocations_bytes, 0)
        self.assertGreater(acct.serialized_responses_bytes, 0)
        self.assertGreater(acct.estimated_payload_tokens, 0)
        self.assertEqual(acct.replacement_status, "MEASURED")

        # Validate metrics serialize cleanly and validate against schema
        run_record = {
            "schema_version": "2.0.0",
            "run_id": "run_test_acct",
            "pair_id": "pair_test",
            "task_id": "task_commit_cache_invalidation",
            "arm": "treatment_B",
            "execution_order_in_pair": 1,
            "execution_status": "SUCCESS",
            "error_message": None,
            "timestamps": {
                "started_at": "2026-11-04T12:00:00Z",
                "completed_at": "2026-11-04T12:01:00Z",
                "elapsed_seconds": 60.0
            },
            "model": {
                "provider": "openai",
                "name": "gpt-5-codex",
                "mode": "agent",
                "reasoning_effort": "high",
                "harness_executable": "codex.cmd",
                "harness_version": "1.0.0"
            },
            "isolation": {
                "baseline_commit": "c70845b4d5913457f1b176fb0370b80dd792711f",
                "sandbox_path": str(self.temp_dir),
                "cold_start_verified": True,
                "engram_enabled": True,
                "engram_binary_sha256": "F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE",
                "dependency_fingerprint_verified": True,
                "path_sanitized": True
            },
            "telemetry": metrics.to_dict(),
            "git_impact": {
                "files_changed_count": 0,
                "files_changed": [],
                "insertions": 0,
                "deletions": 0,
                "unrelated_files_count": 0
            },
            "correctness": {
                "score": 1.0,
                "quality_adjusted_cost": float(metrics.total_model_tokens),
                "passed_all_requirements": True,
                "gates": {"L0_typecheck": "PASS", "L1_lint_prettier": "PASS", "L2_unit_tests": "PASS", "L3_production_build": "PASS"},
                "requirements": {"REQ-1": "PASS"}
            }
        }
        validate_instance(run_record, self.telemetry_schema)


if __name__ == "__main__":
    unittest.main()
