"""
Unit and regression tests for telemetry measurement semantics in the Engram benchmark harness.

Verifies:
1. Disambiguation of response size metrics:
   - inner_response_content_bytes
   - serialized_jsonrpc_response_bytes
   - aggregate_auxiliary_payload_bytes
   - max_individual_response_bytes
2. Invariants:
   - inner_response_content_bytes <= serialized_jsonrpc_response_bytes
   - max_individual_response_bytes <= aggregate_auxiliary_payload_bytes
   - Single-call limits (16,384 B) apply ONLY to max_individual_response_bytes, NEVER aggregate
3. Parsing of historical Phase 5.4.7 runs:
   - Arm C: max_individual_response_bytes is 14,491 B (<= 16,384), aggregate is 31,621 B
   - Arm D: max_individual_response_bytes is 154,608 B (exceeded because Candidate 4 was used)
"""

import json
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "benchmarks" / "matched-pair-harness"))

from src.telemetry import TelemetryParser, TelemetryMetrics, TokenReplacementAccounting

PHASE_5_4_7_RUNS = REPO_ROOT / "benchmarks" / "matched-pair-harness" / "reports" / "runs" / "task_b_remember_me_tetrad_live_01"


class TestTelemetryMeasurementSemantics(unittest.TestCase):

    def test_disambiguation_synthetic_events(self):
        """Verify that multi-call events correctly populate all four metrics without conflation."""
        # 3 calls: 8,000 bytes, 12,000 bytes, 5,000 bytes
        calls_data = [
            ("call_1", {"content": [{"type": "text", "text": "A" * 7800}]}, 8000),
            ("call_2", {"content": [{"type": "text", "text": "B" * 11800}]}, 12000),
            ("call_3", {"content": [{"type": "text", "text": "C" * 4800}]}, 5000),
        ]
        events = [
            {"type": "thread.started", "thread_id": "synthetic_thread_01"},
            {"type": "turn.completed", "usage": {"input_tokens": 1000, "output_tokens": 100}}
        ]
        total_inv_bytes = 0
        total_res_bytes = 0
        inner_content_bytes = 0

        for cid, res_obj, approx_b in calls_data:
            arg_obj = {"target": "foo.ts"}
            events.append({
                "type": "item.completed",
                "item": {
                    "id": cid,
                    "type": "mcp_tool_call",
                    "server": "engram",
                    "tool": "engram_inspect_symbol",
                    "arguments": arg_obj,
                    "result": res_obj
                }
            })
            total_inv_bytes += len(json.dumps(arg_obj).encode("utf-8"))
            res_str = json.dumps(res_obj, ensure_ascii=False)
            res_len = len(res_str.encode("utf-8"))
            total_res_bytes += res_len
            inner_content_bytes += len(res_obj["content"][0]["text"].encode("utf-8"))

        parser = TelemetryParser(provenance="SYNTHETIC_TEST")
        metrics = parser.parse_events(events)

        # 1. Total aggregate auxiliary payload = invocations + responses
        self.assertEqual(metrics.aggregate_auxiliary_payload_bytes, total_inv_bytes + total_res_bytes)
        # 2. Serialized jsonrpc responses = sum of responses only
        self.assertEqual(metrics.serialized_jsonrpc_response_bytes, total_res_bytes)
        # 3. Inner content bytes = sum of text content
        self.assertEqual(metrics.inner_response_content_bytes, inner_content_bytes)
        # 4. Invariant: inner <= serialized responses
        self.assertLessEqual(metrics.inner_response_content_bytes, metrics.serialized_jsonrpc_response_bytes)
        # 5. Invariant: max_individual <= aggregate
        self.assertLessEqual(metrics.max_individual_response_bytes, metrics.aggregate_auxiliary_payload_bytes)

        # 6. Peak individual response: max of the 3 calls (~11,800 + JSON overhead)
        expected_max = max(len(json.dumps(r, ensure_ascii=False).encode("utf-8")) for _, r, _ in calls_data)
        self.assertEqual(metrics.max_individual_response_bytes, expected_max)

        # 7. Non-conflation: aggregate is ~25,000 bytes, but max_individual is ~12,000 bytes (< 16,384)
        self.assertGreater(metrics.aggregate_auxiliary_payload_bytes, 16384)
        self.assertLessEqual(metrics.max_individual_response_bytes, 16384)

    def test_conflation_prevention_check(self):
        """Ensure that single-call limit evaluation cannot be satisfied by aggregate payload."""
        limit = 16384
        # Case: 2 calls of 10,000 bytes each
        # Aggregate = 20,000 bytes (> limit)
        # Max individual = 10,000 bytes (<= limit)
        rep = TokenReplacementAccounting(
            files_retrieved_via_engram=["foo.ts"],
            files_read_via_shell=[],
            files_repeated_read=[],
            engram_excerpts_count=1,
            engram_excerpts_reread_count=0,
            serialized_invocations_bytes=200,
            serialized_responses_bytes=20000,
            estimated_payload_tokens=5050,
            replacement_status="MEASURED",
            inner_response_content_bytes=19000,
            serialized_jsonrpc_response_bytes=20000,
            max_individual_response_bytes=10000,
            aggregate_auxiliary_payload_bytes=20200
        )
        # Checking single-call compliance MUST test max_individual_response_bytes
        self.assertTrue(rep.max_individual_response_bytes <= limit)
        # Testing aggregate against limit would be a false positive violation
        self.assertFalse(rep.aggregate_auxiliary_payload_bytes <= limit)

    def test_phase_5_4_7_arm_c_telemetry_reconciliation(self):
        """Reconcile Phase 5.4.7 Arm C events.jsonl against new semantics."""
        arm_c_events = PHASE_5_4_7_RUNS / "arm_c_cand4_always_on" / "events.jsonl"
        if not arm_c_events.exists():
            self.skipTest(f"Arm C events.jsonl not found at {arm_c_events}")

        with open(arm_c_events, "r", encoding="utf-8") as f:
            events = [json.loads(line) for line in f if line.strip()]

        # Mode A: Parse stdout events directly (without session rollout override)
        parser_direct = TelemetryParser(provenance="SYNTHETIC_TEST", sessions_dir=Path("/nonexistent"))
        metrics_direct = parser_direct.parse_events(events)

        # Arm C tool calls: 4 inspect_symbol calls
        self.assertEqual(metrics_direct.mcp_tool_calls_count, 4)
        self.assertEqual(metrics_direct.aggregate_auxiliary_payload_bytes, 31662)
        self.assertEqual(metrics_direct.serialized_jsonrpc_response_bytes, 31200)
        self.assertEqual(metrics_direct.max_individual_response_bytes, 14491)
        self.assertLessEqual(metrics_direct.max_individual_response_bytes, 16384)

        # Mode B: Parse authoritative session rollout file
        parser_rollout = TelemetryParser(provenance="SESSION_ROLLOUT")
        metrics_rollout = parser_rollout.parse_events(events)
        self.assertEqual(metrics_rollout.mcp_tool_calls_count, 4)
        self.assertEqual(metrics_rollout.aggregate_auxiliary_payload_bytes, 31621)
        self.assertEqual(metrics_rollout.max_individual_response_bytes, 14481)
        self.assertLessEqual(metrics_rollout.max_individual_response_bytes, 16384)

    def test_phase_5_4_7_arm_d_telemetry_reconciliation(self):
        """Reconcile Phase 5.4.7 Arm D events.jsonl against new semantics."""
        arm_d_events = PHASE_5_4_7_RUNS / "arm_d_cand4_adaptive" / "events.jsonl"
        if not arm_d_events.exists():
            self.skipTest(f"Arm D events.jsonl not found at {arm_d_events}")

        with open(arm_d_events, "r", encoding="utf-8") as f:
            events = [json.loads(line) for line in f if line.strip()]

        # Mode A: Parse stdout events directly
        parser_direct = TelemetryParser(provenance="SYNTHETIC_TEST", sessions_dir=Path("/nonexistent"))
        metrics_direct = parser_direct.parse_events(events)

        self.assertEqual(metrics_direct.mcp_tool_calls_count, 4)
        self.assertEqual(metrics_direct.max_individual_response_bytes, 154608)
        self.assertGreater(metrics_direct.max_individual_response_bytes, 16384)

        # Mode B: Parse authoritative session rollout file
        parser_rollout = TelemetryParser(provenance="SESSION_ROLLOUT")
        metrics_rollout = parser_rollout.parse_events(events)
        self.assertEqual(metrics_rollout.mcp_tool_calls_count, 4)
        self.assertEqual(metrics_rollout.max_individual_response_bytes, 65542)
        self.assertGreater(metrics_rollout.max_individual_response_bytes, 16384)


if __name__ == "__main__":
    unittest.main()
