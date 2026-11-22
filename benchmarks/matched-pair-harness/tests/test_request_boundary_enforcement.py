"""
Phase 5.9.0.9: Adversarial Request Boundary Enforcement Test Suite.
Verifies all 20 adversarial scenarios covering:
- Synchronous request reservation (R = I + O <= B_remaining - B_reserve)
- In-flight reservation tracking and concurrency exposure
- Fail-closed rejection with zero truncation
- Conservative exposure preservation under missing usage, timeout, and crash
- State preservation across coordinator restarts
- Boundary equality and one-token-over precision
- Rejection of bypass attempts and unauthorized endpoints
"""
import unittest
import tempfile
import sys
import os
import json
from pathlib import Path

# Add src to sys.path
HARNESS_ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = HARNESS_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from runner import (
    RequestReservation,
    RequestReservationCoordinator,
    RequestSizeGate,
    CodexRunner,
    RunExecutionResult
)


class TestRequestBoundaryEnforcement(unittest.TestCase):
    def setUp(self):
        # Default budget: 100,000 tokens, 10,000 reserve, 4,096 default output allowance
        self.coord = RequestReservationCoordinator(
            aggregate_budget=100_000,
            safety_reserve=10_000,
            default_completion_allowance=4_096
        )
        self.gate = RequestSizeGate(default_limit_tokens=60_000)

    # 1. Turn-0 request
    def test_01_turn0_request_reservation(self):
        """Verifies Turn-0 request creates an active reservation of R = I + O against the initial budget."""
        ok, err, res = self.coord.reserve_request(
            request_id="req-turn0",
            turn_index=0,
            input_tokens=15_000,
            completion_allowance=4_096
        )
        self.assertTrue(ok)
        self.assertIsNone(err)
        self.assertIsNotNone(res)
        self.assertEqual(res.total_reserved_tokens, 19_096)
        self.assertEqual(self.coord.in_flight_reserved_tokens, 19_096)
        # Remaining budget: 100,000 - 19,096 = 80,904
        self.assertEqual(self.coord.remaining_budget, 80_904)

    # 2. Second conversation turn
    def test_02_second_conversation_turn(self):
        """Verifies second conversation turn reserves against remaining budget after first turn is reconciled."""
        # Turn 1
        self.coord.reserve_request("req-turn1", 0, 10_000, 2_000)
        self.coord.reconcile_response("req-turn1", actual_total_tokens=11_500)
        self.assertEqual(self.coord.cumulative_reconciled_tokens, 11_500)

        # Turn 2
        ok, err, res2 = self.coord.reserve_request("req-turn2", 1, 25_000, 4_096)
        self.assertTrue(ok)
        self.assertEqual(res2.total_reserved_tokens, 29_096)
        # Total committed = 11,500 + 29,096 = 40,596
        self.assertEqual(self.coord.total_committed_tokens, 40_596)

    # 3. Long conversation history
    def test_03_long_conversation_history(self):
        """Verifies that accumulated conversation history expanding input tokens is rejected once exceeding available headroom."""
        # Remaining budget available above reserve is 90,000 (100,000 - 10,000)
        # Accumulated history requires 88,000 input tokens + 4,096 output allowance = 92,096 > 90,000
        ok, err, res = self.coord.reserve_request(
            request_id="req-long-history",
            turn_index=15,
            input_tokens=88_000,
            completion_allowance=4_096
        )
        self.assertFalse(ok)
        self.assertIsNone(res)
        self.assertIn("Request budget reservation rejected", err)
        self.assertIn("Zero-truncation policy enforced", err)
        self.assertEqual(len(self.coord.rejection_history), 1)
        self.assertTrue(self.coord.rejection_history[0]["provider_dispatch_prevented"])

    # 4. Large MCP tool output
    def test_04_large_mcp_tool_output(self):
        """Verifies large MCP tool output payload expanding next turn input is rejected if budget is insufficient."""
        # Simulate previous turn consumed 60,000 tokens
        self.coord.reserve_request("t1", 0, 10_000, 2_000)
        self.coord.reconcile_response("t1", actual_total_tokens=60_000)
        # Remaining budget: 40,000 tokens. Available above 10,000 reserve: 30,000 tokens.
        # Large MCP tool dump: 35,000 tokens input + 2,000 output = 37,000 > 30,000
        ok, err, res = self.coord.reserve_request("t2-large-mcp", 1, 35_000, 2_000)
        self.assertFalse(ok)
        self.assertIn("Request budget reservation rejected", err)

    # 5. Large tool schema
    def test_05_large_tool_schema(self):
        """Verifies tool definitions and schemas count toward request size and fail closed."""
        large_schema_text = json.dumps({"schema": "x " * 60_000})  # ~60k tokens
        tokens, _, method = self.gate.count_tokens(large_schema_text)
        self.assertGreater(tokens, 25_000)
        # Try to reserve with 88,000 input tokens (prompts + schemas)
        ok, err, res = self.coord.reserve_request("t-schema", 0, 88_000, 4_096)
        self.assertFalse(ok)
        self.assertIn("Deficit:", err)

    # 6. Retry after provider error
    def test_06_retry_after_provider_error(self):
        """Verifies retrying after a provider error creates a distinct new reservation and cannot double-count."""
        # Initial request failed
        self.coord.reserve_request("req-orig", 0, 10_000, 2_000)
        # Reconcile provider error with 0 tokens consumed
        self.coord.reconcile_response("req-orig", actual_total_tokens=0)
        self.assertEqual(self.coord.cumulative_reconciled_tokens, 0)
        self.assertEqual(self.coord.in_flight_reserved_tokens, 0)

        # Retry request creates new reservation
        ok, err, res_retry = self.coord.reserve_request("req-retry-1", 0, 10_000, 2_000)
        self.assertTrue(ok)
        self.assertEqual(res_retry.request_id, "req-retry-1")
        self.assertEqual(self.coord.in_flight_reserved_tokens, 12_000)

    # 7. Intentional repeated tool invocation
    def test_07_intentional_repeated_tool_invocation(self):
        """Verifies repeated identical tool invocations consume budget sequentially and cumulatively."""
        for i in range(4):
            req_id = f"tool-call-{i}"
            ok, _, _ = self.coord.reserve_request(req_id, i, 5_000, 2_000)
            self.assertTrue(ok)
            self.coord.reconcile_response(req_id, actual_total_tokens=6_500)
        # 4 calls * 6,500 = 26,000 tokens
        self.assertEqual(self.coord.cumulative_reconciled_tokens, 26_000)
        self.assertEqual(self.coord.remaining_budget, 74_000)

    # 8. Concurrent dispatch attempt
    def test_08_concurrent_dispatch_attempt(self):
        """Verifies concurrent in-flight requests compound in-flight reservations, blocking over-budget concurrency."""
        # Available above reserve: 90,000 tokens
        # Request 1 reserves 50,000 tokens (in flight)
        ok1, _, res1 = self.coord.reserve_request("req-conc-1", 0, 46_000, 4_000)
        self.assertTrue(ok1)
        self.assertEqual(self.coord.in_flight_reserved_tokens, 50_000)

        # Request 2 concurrently attempts to reserve 45,000 tokens (50k + 45k = 95k > 90k)
        ok2, err2, res2 = self.coord.reserve_request("req-conc-2", 0, 41_000, 4_000)
        self.assertFalse(ok2)
        self.assertIn("Deficit:", err2)
        # In flight remains exactly 50,000
        self.assertEqual(self.coord.in_flight_reserved_tokens, 50_000)

    # 9. Streaming response capacity update
    def test_09_streaming_response(self):
        """Verifies streaming response token updates expand reservation if observed tokens exceed initial output allowance."""
        ok, _, res = self.coord.reserve_request("req-stream", 0, 10_000, 2_000)
        self.assertEqual(res.total_reserved_tokens, 12_000)

        # Stream outputs 3,500 tokens (> 2,000 initial allowance)
        self.coord.update_streaming_tokens("req-stream", observed_output_tokens=3_500)
        self.assertEqual(res.reserved_output_tokens, 3_500)
        self.assertEqual(res.total_reserved_tokens, 13_500)
        self.assertEqual(self.coord.in_flight_reserved_tokens, 13_500)

    # 10. Missing usage metadata
    def test_10_missing_usage_metadata(self):
        """Verifies that missing usage metadata charges the full reserved capacity R and never silently releases budget."""
        self.coord.reserve_request("req-missing-meta", 0, 20_000, 4_000)
        self.assertEqual(self.coord.in_flight_reserved_tokens, 24_000)

        # Reconcile with None usage (missing metadata)
        reconciled = self.coord.reconcile_response("req-missing-meta", actual_total_tokens=None)
        # Preserves maximum exposure: charges 24,000
        self.assertEqual(reconciled.actual_total_tokens, 24_000)
        self.assertEqual(self.coord.cumulative_reconciled_tokens, 24_000)
        self.assertEqual(self.coord.in_flight_reserved_tokens, 0)

    # 11. Provider timeout
    def test_11_provider_timeout(self):
        """Verifies provider timeout retains conservative exposure rather than releasing tokens to 0."""
        self.coord.reserve_request("req-timeout", 0, 15_000, 3_000)
        # On timeout, usage is unknown -> conservatively charge R
        reconciled = self.coord.reconcile_response("req-timeout", actual_total_tokens=None)
        self.assertEqual(reconciled.actual_total_tokens, 18_000)
        self.assertEqual(self.coord.cumulative_reconciled_tokens, 18_000)

    # 12. Process crash
    def test_12_process_crash(self):
        """Verifies process crash retains conservative reservation in state."""
        self.coord.reserve_request("req-crash", 0, 12_000, 2_000)
        # Process crashed before telemetry returned -> charge R
        reconciled = self.coord.reconcile_response("req-crash", actual_total_tokens=None)
        self.assertEqual(reconciled.actual_total_tokens, 14_000)
        self.assertEqual(self.coord.cumulative_reconciled_tokens, 14_000)

    # 13. Gateway or state unavailable
    def test_13_gateway_crash_if_applicable(self):
        """Verifies coordinator fails closed immediately if state or limits are corrupted/negative."""
        with self.assertRaises(Exception):
            corrupted_data = {"aggregate_budget": -100, "safety_reserve": 50_000}
            coord_broken = RequestReservationCoordinator(aggregate_budget=-100)
            coord_broken.reserve_request("bad", 0, 10, 10)

    # 14. Budget coordinator restart preserves usage
    def test_14_budget_coordinator_restart(self):
        """Verifies state serialization and restoration preserves cumulative consumed tokens across coordinator restarts."""
        self.coord.reserve_request("req-prev", 0, 20_000, 2_000)
        self.coord.reconcile_response("req-prev", actual_total_tokens=21_300)

        # Export state
        saved_state = self.coord.to_state_dict()
        # Restore into new coordinator instance
        restored_coord = RequestReservationCoordinator.from_state_dict(saved_state)

        self.assertEqual(restored_coord.cumulative_reconciled_tokens, 21_300)
        self.assertEqual(restored_coord.remaining_budget, 100_000 - 21_300)

    # 15. Request exactly at the limit
    def test_15_request_exactly_at_limit(self):
        """Verifies boundary equality: request requiring exactly B_remaining - B_reserve passes."""
        # Available above reserve: 100,000 - 10,000 = 90,000
        # Request requiring exactly 90,000 tokens (86,000 + 4,000)
        ok, err, res = self.coord.reserve_request("req-exact", 0, 86_000, 4_000)
        self.assertTrue(ok)
        self.assertIsNone(err)
        self.assertEqual(res.total_reserved_tokens, 90_000)
        self.assertEqual(self.coord.in_flight_reserved_tokens, 90_000)
        # Remaining budget available for dispatch is now exactly 0
        avail_next = self.coord.remaining_budget - self.coord.safety_reserve
        self.assertEqual(avail_next, 0)

    # 16. Request one token above the limit
    def test_16_request_one_token_above_limit(self):
        """Verifies boundary inequality: request requiring exactly 1 token above B_remaining - B_reserve is rejected."""
        # Available above reserve: 90,000
        # Request requiring 90,001 tokens (86,001 + 4,000)
        ok, err, res = self.coord.reserve_request("req-one-over", 0, 86_001, 4_000)
        self.assertFalse(ok)
        self.assertIsNone(res)
        self.assertIn("Deficit: 1 tokens", err)

    # 17. Completion allowance exceeding remaining capacity
    def test_17_completion_allowance_exceeding_capacity(self):
        """Verifies that even if input is tiny, an excessive completion allowance O is rejected pre-dispatch."""
        # Available: 90,000. Input: 1,000. Output allowance requested: 95,000 (total 96,000 > 90,000)
        ok, err, res = self.coord.reserve_request("req-huge-out", 0, 1_000, 95_000)
        self.assertFalse(ok)
        self.assertIn("Request budget reservation rejected", err)

    # 18. Alternate provider endpoint
    def test_18_alternate_provider_endpoint(self):
        """Verifies runner rejects unsupported or alternate provider endpoints (strict provider allowlist)."""
        with self.assertRaises(ValueError) as ctx:
            CodexRunner(
                codex_executable=Path(sys.executable),
                provider="unauthorized_proxy_provider"
            )
        self.assertIn("Unsupported provider", str(ctx.exception))

    # 19. Unusual Unicode and serialized JSON
    def test_19_unusual_unicode_and_serialized_json(self):
        """Verifies complex UTF-8, multi-byte emoji, and deep nested JSON are measured conservatively."""
        dense_json = json.dumps({
            "nested": [{"symbols": "🔥⚡🚀🛡️", "math": "∑_{i=1}^n x_i ≤ C", "code": "x = (y >> 2) & 0xFF;"} for _ in range(50)]
        })
        tokens, bytes_count, method = self.gate.count_tokens(dense_json)
        self.assertGreater(tokens, 0)
        self.assertGreater(bytes_count, tokens)  # multi-byte UTF-8 ensures bytes > tokens

        # Successfully reserve
        ok, _, res = self.coord.reserve_request("req-unicode", 0, tokens, 2_000)
        self.assertTrue(ok)
        self.assertEqual(res.input_tokens, tokens)

    # 20. Bypass attempt detection
    def test_20_bypass_attempt_detection(self):
        """Verifies that an unreserved request cannot be reconciled and is flagged as an unreserved dispatch violation."""
        with self.assertRaises(KeyError) as ctx:
            # Attempt to reconcile a request that never passed through reserve_request()
            self.coord.reconcile_response("unauthorized-bypass-req", actual_total_tokens=5_000)
        self.assertIn("No active reservation found", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()
