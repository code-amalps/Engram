import unittest
import tempfile
import json
import sys
from pathlib import Path

# Add src to sys.path
HARNESS_ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = HARNESS_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from telemetry import (
    TelemetryParser,
    RoundTripEvent,
    TaskRoundTripSummary,
    RoundTripComparison,
    TelemetryMetrics
)
from jsonschema import validate as validate_schema


class TestRoundTripTelemetry(unittest.TestCase):
    def setUp(self):
        self.parser = TelemetryParser(provenance="SYNTHETIC_TEST")
        schema_path = HARNESS_ROOT / "schemas" / "run_telemetry.schema.json"
        with open(schema_path, "r", encoding="utf-8") as f:
            self.schema = json.load(f)

    def test_01_empty_events_handling(self):
        """Verifies extract_round_trips handles empty events cleanly."""
        summary, events = self.parser.extract_round_trips([], turns_count=0)
        self.assertEqual(summary.total_round_trips, 0)
        self.assertEqual(summary.engram_mcp_calls, 0)
        self.assertEqual(summary.standard_tool_calls, 0)
        self.assertEqual(summary.total_retries, 0)
        self.assertEqual(summary.latency_mean_ms, 0.0)
        self.assertEqual(summary.latency_median_ms, 0.0)
        self.assertEqual(summary.latency_p95_ms, 0.0)
        self.assertEqual(len(events), 0)

    def test_02_tool_call_correlation_and_latency(self):
        """Verifies correlation of tool requests and outputs, latency measurement, and bytes accounting."""
        events = [
            {
                "type": "response_item",
                "timestamp": "2026-11-22T10:00:00.000Z",
                "payload": {
                    "type": "custom_tool_call",
                    "call_id": "call_engram_01",
                    "name": "engram_get_context",
                    "server": "engram",
                    "input": {"task": "fix auth bug"}
                }
            },
            {
                "type": "response_item",
                "timestamp": "2026-11-22T10:00:00.050Z",
                "payload": {
                    "type": "custom_tool_call_output",
                    "call_id": "call_engram_01",
                    "output": {"content": [{"type": "text", "text": "context snippet"}]}
                }
            },
            {
                "type": "response_item",
                "timestamp": "2026-11-22T10:00:01.000Z",
                "payload": {
                    "type": "custom_tool_call",
                    "call_id": "call_bash_02",
                    "name": "exec",
                    "input": {"command": "git status"}
                }
            },
            {
                "type": "response_item",
                "timestamp": "2026-11-22T10:00:01.120Z",
                "payload": {
                    "type": "custom_tool_call_output",
                    "call_id": "call_bash_02",
                    "output": "On branch feature/auth"
                }
            }
        ]

        summary, rt_events = self.parser.extract_round_trips(events, turns_count=2)
        self.assertEqual(summary.total_round_trips, 2)
        self.assertEqual(summary.engram_mcp_calls, 1)
        self.assertEqual(summary.standard_tool_calls, 1)
        self.assertEqual(len(rt_events), 2)

        # Engram call: 50ms latency
        engram_ev = rt_events[0]
        self.assertTrue(engram_ev.is_engram)
        self.assertEqual(engram_ev.tool_name, "engram_get_context")
        self.assertAlmostEqual(engram_ev.duration_ms, 50.0, delta=1.0)
        self.assertGreater(engram_ev.request_bytes, 0)
        self.assertGreater(engram_ev.response_bytes, 0)

        # Standard tool: 120ms latency
        exec_ev = rt_events[1]
        self.assertFalse(exec_ev.is_engram)
        self.assertEqual(exec_ev.tool_name, "exec")
        self.assertAlmostEqual(exec_ev.duration_ms, 120.0, delta=1.0)

    def test_03_retry_detection(self):
        """Verifies retry detection when identical tool calls are executed repeatedly."""
        events = [
            # Call 1
            {
                "type": "response_item",
                "payload": {
                    "type": "custom_tool_call",
                    "call_id": "call_1",
                    "name": "exec",
                    "input": {"command": "npm test"}
                }
            },
            {
                "type": "response_item",
                "payload": {
                    "type": "custom_tool_call_output",
                    "call_id": "call_1",
                    "output": "Test failed with error: timeout"
                }
            },
            # Call 2 (Retry of identical command)
            {
                "type": "response_item",
                "payload": {
                    "type": "custom_tool_call",
                    "call_id": "call_2",
                    "name": "exec",
                    "input": {"command": "npm test"}
                }
            },
            {
                "type": "response_item",
                "payload": {
                    "type": "custom_tool_call_output",
                    "call_id": "call_2",
                    "output": "Test passed 10/10"
                }
            }
        ]

        summary, rt_events = self.parser.extract_round_trips(events, turns_count=2)
        self.assertEqual(summary.total_round_trips, 2)
        self.assertEqual(summary.total_retries, 1)
        self.assertFalse(rt_events[0].is_retry)
        self.assertEqual(rt_events[0].attempt_number, 1)
        self.assertTrue(rt_events[1].is_retry)
        self.assertEqual(rt_events[1].attempt_number, 2)

    def test_04_latency_percentiles_calculation(self):
        """Verifies accurate calculation of mean, median (p50), and p95 latency."""
        events = []
        for i in range(10):
            cid = f"call_{i}"
            duration_sec = 0.010 * (i + 1)  # 10ms, 20ms, ..., 100ms
            t0 = f"2026-11-22T10:00:00.{i:03d}Z"
            events.append({
                "type": "response_item",
                "timestamp": "2026-11-22T10:00:00.000Z",
                "payload": {"type": "custom_tool_call", "call_id": cid, "name": "tool", "input": {"idx": i}}
            })
            events.append({
                "type": "response_item",
                "timestamp": f"2026-11-22T10:00:{duration_sec:06.3f}Z",
                "payload": {"type": "custom_tool_call_output", "call_id": cid, "output": "ok"}
            })

        summary, rt_events = self.parser.extract_round_trips(events, turns_count=10)
        self.assertEqual(summary.total_round_trips, 10)
        self.assertGreater(summary.latency_mean_ms, 0)
        self.assertGreater(summary.latency_p95_ms, summary.latency_median_ms)
        self.assertLessEqual(summary.latency_min_ms, summary.latency_median_ms)
        self.assertGreaterEqual(summary.latency_max_ms, summary.latency_p95_ms)

    def test_05_schema_validation_with_round_trip_telemetry(self):
        """Verifies telemetry record with round_trip_summary and round_trip_events validates against schema."""
        events = [
            {
                "type": "item.completed",
                "item": {
                    "id": "item_1",
                    "type": "mcp_tool_call",
                    "server": "engram",
                    "tool": "engram_get_context",
                    "arguments": {"task": "fix issue"},
                    "result": {"content": [{"type": "text", "text": "context snippet"}]}
                }
            },
            {
                "type": "turn.completed",
                "usage": {"input_tokens": 1500, "output_tokens": 100}
            }
        ]
        metrics = self.parser.parse_events(events)
        self.assertIsNotNone(metrics.round_trip_summary)
        self.assertIsNotNone(metrics.round_trip_events)

        # Build full mock record matching schema
        record = {
            "schema_version": "2.0.0",
            "run_id": "test_run_01",
            "pair_id": "pair_01",
            "task_id": "task_a",
            "arm": "arm_e_cand5_optimized",
            "execution_order_in_pair": 1,
            "execution_status": "SUCCESS",
            "error_message": None,
            "timestamps": {
                "started_at": "2026-11-22T10:00:00Z",
                "completed_at": "2026-11-22T10:00:10Z",
                "elapsed_seconds": 10.0
            },
            "model": {
                "provider": "openai",
                "name": "gpt-5.6-luna",
                "mode": "agent",
                "reasoning_effort": "medium",
                "harness_executable": "codex",
                "harness_version": "1.0.0"
            },
            "isolation": {
                "baseline_commit": "1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2",
                "sandbox_path": "C:/tmp/test",
                "cold_start_verified": True,
                "engram_enabled": True,
                "engram_binary_sha256": "E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44",
                "dependency_fingerprint_verified": True,
                "path_sanitized": True
            },
            "telemetry": metrics.to_dict(),
            "git_impact": {
                "files_changed_count": 1,
                "files_changed": ["src/app.ts"],
                "insertions": 2,
                "deletions": 1,
                "unrelated_files_count": 0,
                "diff_patch_path": "C:/tmp/changes.patch"
            },
            "correctness": {
                "score": 1.0,
                "quality_adjusted_cost": 1600.0,
                "passed_all_requirements": True,
                "gates": {
                    "L0_typecheck": "PASS",
                    "L1_lint_prettier": "PASS",
                    "L2_unit_tests": "PASS",
                    "L3_production_build": "PASS"
                },
                "requirements": {"R1": "PASS"}
            },
            "request_size_gate": {
                "gate_passed": True,
                "measured_request_tokens": 1200,
                "measured_request_bytes": 4800,
                "counting_method": "tiktoken_o200k_base",
                "limit_tokens": 60000,
                "breakdown": {}
            }
        }

        # Validate against JSON schema - must not raise
        validate_schema(instance=record, schema=self.schema)

    def test_06_intentional_repeated_calls_not_flagged_as_retries(self):
        """
        Verifies that when an intentional command succeeds, subsequent identical calls
        (e.g., periodic git status or repeated read_file) increment attempt_number but
        are NOT misclassified as failure retries (is_retry=False).
        """
        events = [
            # Call 1: Success
            {
                "type": "response_item",
                "payload": {
                    "type": "custom_tool_call",
                    "call_id": "call_status_1",
                    "name": "exec",
                    "input": {"command": "git status"}
                }
            },
            {
                "type": "response_item",
                "payload": {
                    "type": "custom_tool_call_output",
                    "call_id": "call_status_1",
                    "output": "Clean working directory"
                }
            },
            # Call 2: Intentional repeated check (also succeeds)
            {
                "type": "response_item",
                "payload": {
                    "type": "custom_tool_call",
                    "call_id": "call_status_2",
                    "name": "exec",
                    "input": {"command": "git status"}
                }
            },
            {
                "type": "response_item",
                "payload": {
                    "type": "custom_tool_call_output",
                    "call_id": "call_status_2",
                    "output": "Clean working directory"
                }
            }
        ]

        summary, rt_events = self.parser.extract_round_trips(events, turns_count=2)
        self.assertEqual(summary.total_round_trips, 2)
        self.assertEqual(summary.total_retries, 0, "Intentional repeat after success must NOT be counted as a retry")
        self.assertFalse(rt_events[0].is_retry)
        self.assertEqual(rt_events[0].attempt_number, 1)
        self.assertFalse(rt_events[1].is_retry, "Subsequent successful call must have is_retry=False")
        self.assertEqual(rt_events[1].attempt_number, 2)

    def test_07_latency_percentiles_small_samples_safety(self):
        """Verifies mathematical stability of latency percentiles across N=0, N=1, N=2."""
        # N=0
        s0, _ = self.parser.extract_round_trips([])
        self.assertEqual(s0.latency_mean_ms, 0.0)
        self.assertEqual(s0.latency_median_ms, 0.0)
        self.assertEqual(s0.latency_p95_ms, 0.0)

        # N=1
        ev1 = [
            {"type": "response_item", "timestamp": "2026-11-22T10:00:00.000Z",
             "payload": {"type": "custom_tool_call", "call_id": "c1", "name": "t", "input": {}}},
            {"type": "response_item", "timestamp": "2026-11-22T10:00:00.040Z",
             "payload": {"type": "custom_tool_call_output", "call_id": "c1", "output": "ok"}}
        ]
        s1, _ = self.parser.extract_round_trips(ev1)
        self.assertAlmostEqual(s1.latency_mean_ms, 40.0, delta=1.0)
        self.assertAlmostEqual(s1.latency_median_ms, 40.0, delta=1.0)
        self.assertAlmostEqual(s1.latency_p95_ms, 40.0, delta=1.0)

        # N=2
        ev2 = ev1 + [
            {"type": "response_item", "timestamp": "2026-11-22T10:00:01.000Z",
             "payload": {"type": "custom_tool_call", "call_id": "c2", "name": "t2", "input": {}}},
            {"type": "response_item", "timestamp": "2026-11-22T10:00:01.080Z",
             "payload": {"type": "custom_tool_call_output", "call_id": "c2", "output": "ok"}}
        ]
        s2, _ = self.parser.extract_round_trips(ev2)
        self.assertAlmostEqual(s2.latency_mean_ms, 60.0, delta=1.0)
        self.assertAlmostEqual(s2.latency_min_ms, 40.0, delta=1.0)
        self.assertAlmostEqual(s2.latency_max_ms, 80.0, delta=1.0)


if __name__ == "__main__":
    unittest.main()
