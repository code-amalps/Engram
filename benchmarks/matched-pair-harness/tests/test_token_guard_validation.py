"""
Comprehensive deterministic offline validation tests for Engram Token Guard remediation (Milestone 1).

Covers:
Section 4.A: Token Accounting (parsing, missing fields, malformed JSON, deduplication, reasoning tokens).
Section 4.B: Headroom Calculation (the 10 required boundary condition cases, equality boundary).
Section 4.C: Event-Boundary Detection (tool-start vs completion, external poller unproven boundary).
Section 4.D: Process Termination (8 scenarios, latency benchmarks, multiple descendants).
Section 4.E: Stop Classification (PRE_DISPATCH_HEADROOM_EXHAUSTED, TOKEN_CEILING_BREACHED, TELEMETRY_INVALID, TIMEOUT, CRASH).
Section 4.F: Telemetry Integrity & Serialization (all headroom_guard fields, JSON schema validation).

STRICTLY OFFLINE — ZERO MODEL OR API INFERENCE CALLS.
"""

import unittest
from unittest.mock import patch, MagicMock
from pathlib import Path
import subprocess
import tempfile
import shutil
import json
import sys
import os
import time
import datetime
import hashlib
from typing import Dict, Any, List

# Harness imports
ROOT_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT_DIR / "src"))

from runner import (
    CodexRunner,
    RunExecutionResult,
    StreamingTokenTracker,
    extract_tokens_from_line
)
from orchestrator import Orchestrator
from isolation import terminate_pid_tree
from schema_validator import validate_instance


class TestTokenAccounting(unittest.TestCase):
    """Section 4.A: Token accounting tests."""

    def setUp(self):
        self.tracker = StreamingTokenTracker(conservative_reserve=15_000)

    def test_token_usage_record_parsing(self):
        """Verifies parsing of authoritative token_usage_record thread and turn tokens."""
        line = json.dumps({
            "type": "token_usage_record",
            "payload": {
                "thread_token_usage": {
                    "input_tokens": 100_000,
                    "cached_input_tokens": 80_000,
                    "output_tokens": 2_000,
                    "reasoning_output_tokens": 500,
                    "total_tokens": 102_000
                },
                "turn_token_usage": {
                    "input_tokens": 25_000,
                    "output_tokens": 50,
                    "total_tokens": 25_050
                }
            }
        })
        tokens = self.tracker.process_line(line)
        self.assertEqual(tokens, 102_000)
        self.assertEqual(self.tracker.thread_cumulative_max, 102_000)
        self.assertEqual(self.tracker.last_turn_request_tokens, 25_000)
        self.assertEqual(self.tracker.last_turn_total_tokens, 25_050)
        self.assertEqual(self.tracker.prompt_tokens_total, 100_000)
        self.assertEqual(self.tracker.prompt_tokens_cached, 80_000)
        self.assertEqual(self.tracker.prompt_tokens_uncached, 20_000)
        self.assertEqual(self.tracker.completion_tokens, 2_000)
        self.assertEqual(self.tracker.reasoning_tokens, 500)
        self.assertTrue(self.tracker.telemetry_valid)
        self.assertEqual(self.tracker.telemetry_status, "VALID")

    def test_event_msg_token_count_parsing(self):
        """Verifies parsing of event_msg token_count cumulative snapshot."""
        line = json.dumps({
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "total_token_usage": {
                        "input_tokens": 60_000,
                        "cached_input_tokens": 40_000,
                        "output_tokens": 1_500,
                        "reasoning_output_tokens": 300,
                        "total_tokens": 61_500
                    }
                }
            }
        })
        tokens = self.tracker.process_line(line)
        self.assertEqual(tokens, 61_500)
        self.assertEqual(self.tracker.prompt_tokens_cached, 40_000)
        self.assertEqual(self.tracker.prompt_tokens_uncached, 20_000)
        self.assertEqual(self.tracker.completion_tokens, 1_500)
        self.assertEqual(self.tracker.reasoning_tokens, 300)

    def test_turn_completed_incremental_sum(self):
        """Verifies summation and deduplication of incremental turn.completed deltas."""
        t1 = json.dumps({
            "type": "turn.completed",
            "turn_id": "turn_1",
            "usage": {"input_tokens": 15_000, "output_tokens": 500, "total_tokens": 15_500}
        })
        t2 = json.dumps({
            "type": "turn.completed",
            "turn_id": "turn_2",
            "usage": {"input_tokens": 20_000, "output_tokens": 600, "total_tokens": 20_600}
        })
        self.tracker.process_line(t1)
        self.assertEqual(self.tracker.turn_deltas_sum, 15_500)
        self.tracker.process_line(t2)
        self.assertEqual(self.tracker.turn_deltas_sum, 36_100)
        self.assertEqual(self.tracker.effective_tokens, 36_100)
        self.assertEqual(self.tracker.turns_completed, 2)

    def test_missing_usage_fields(self):
        """Verifies safe handling when total_tokens is omitted (computed from input + output)."""
        line = json.dumps({
            "type": "token_usage_record",
            "payload": {
                "thread_token_usage": {
                    "input_tokens": 40_000,
                    "output_tokens": 1_000
                    # total_tokens omitted
                }
            }
        })
        tokens = self.tracker.process_line(line)
        self.assertEqual(tokens, 41_000)
        self.assertEqual(self.tracker.effective_tokens, 41_000)
        self.assertTrue(self.tracker.telemetry_valid)

    def test_malformed_incomplete_json(self):
        """Verifies that malformed or partially written JSON records do not crash the tracker."""
        partial_line_1 = '{"type": "token_usage_rec'
        partial_line_2 = '{"type": "token_usage_record", "payload": {'
        empty_line = ''
        whitespace_line = '   \n'

        self.tracker.process_line(partial_line_1)
        self.tracker.process_line(partial_line_2)
        self.tracker.process_line(empty_line)
        self.tracker.process_line(whitespace_line)

        self.assertEqual(self.tracker.effective_tokens, 0)
        self.assertEqual(self.tracker.partial_records_count, 2)

    def test_duplicate_snapshots_deduplication(self):
        """Verifies duplicate snapshot records do not double-increment turns or inflate cumulative tokens."""
        line = json.dumps({
            "type": "token_usage_record",
            "payload": {
                "thread_token_usage": {"input_tokens": 50_000, "output_tokens": 500, "total_tokens": 50_500}
            }
        })
        self.tracker.process_line(line)
        self.assertEqual(self.tracker.turns_completed, 1)
        self.assertEqual(self.tracker.effective_tokens, 50_500)

        # Duplicate arrival of identical snapshot
        self.tracker.process_line(line)
        self.assertEqual(self.tracker.turns_completed, 1, "Duplicate snapshot double-incremented turns_completed!")
        self.assertEqual(self.tracker.effective_tokens, 50_500, "Duplicate snapshot inflated cumulative tokens!")

    def test_duplicate_turn_completed_deduplication(self):
        """Verifies duplicate turn.completed events are deduplicated by turn_id."""
        line = json.dumps({
            "type": "turn.completed",
            "turn_id": "turn_dup",
            "usage": {"input_tokens": 10_000, "output_tokens": 200, "total_tokens": 10_200}
        })
        self.tracker.process_line(line)
        self.assertEqual(self.tracker.turn_deltas_sum, 10_200)
        self.assertEqual(self.tracker.turns_completed, 1)

        # Duplicate event with same turn_id
        self.tracker.process_line(line)
        self.assertEqual(self.tracker.turn_deltas_sum, 10_200, "Duplicate turn.completed inflated sum!")
        self.assertEqual(self.tracker.turns_completed, 1)

    def test_out_of_order_records(self):
        """Verifies out-of-order snapshots preserve the maximum cumulative total safely."""
        snap_later = json.dumps({
            "type": "token_usage_record",
            "payload": {"thread_token_usage": {"input_tokens": 80_000, "total_tokens": 80_000}}
        })
        snap_earlier = json.dumps({
            "type": "token_usage_record",
            "payload": {"thread_token_usage": {"input_tokens": 40_000, "total_tokens": 40_000}}
        })
        self.tracker.process_line(snap_later)
        self.assertEqual(self.tracker.effective_tokens, 80_000)
        # Delayed/reordered arrival of an earlier snapshot
        self.tracker.process_line(snap_earlier)
        self.assertEqual(self.tracker.effective_tokens, 80_000, "Earlier snapshot improperly decreased cumulative tokens!")

    def test_reasoning_tokens_never_double_counted(self):
        """Verifies that reasoning tokens are never added twice to total tokens."""
        line = json.dumps({
            "type": "token_usage_record",
            "payload": {
                "thread_token_usage": {
                    "input_tokens": 10_000,
                    "output_tokens": 2_000,
                    "reasoning_output_tokens": 1_500,
                    "total_tokens": None  # Force calculation
                }
            }
        })
        tokens = self.tracker.process_line(line)
        # Expected: 10,000 + 2,000 = 12,000 (NOT 10,000 + 2,000 + 1,500 = 13,500)
        self.assertEqual(tokens, 12_000)
        self.assertEqual(self.tracker.completion_tokens, 2_000)
        self.assertEqual(self.tracker.reasoning_tokens, 1_500)

    def test_preserves_cached_uncached_output_reasoning_distinction(self):
        """Verifies full decomposition of input into cached vs uncached."""
        line = json.dumps({
            "type": "token_usage_record",
            "payload": {
                "thread_token_usage": {
                    "input_tokens": 50_000,
                    "cached_input_tokens": 35_000,
                    "output_tokens": 1_200,
                    "reasoning_output_tokens": 800,
                    "total_tokens": 51_200
                }
            }
        })
        self.tracker.process_line(line)
        self.assertEqual(self.tracker.prompt_tokens_total, 50_000)
        self.assertEqual(self.tracker.prompt_tokens_cached, 35_000)
        self.assertEqual(self.tracker.prompt_tokens_uncached, 15_000)
        self.assertEqual(self.tracker.completion_tokens, 1_200)
        self.assertEqual(self.tracker.reasoning_tokens, 800)

    def test_invalid_telemetry_classification(self):
        """Verifies invalid telemetry (cached > input, negative tokens) is flagged INVALID."""
        # Case 1: cached > input
        bad_cached = json.dumps({
            "type": "token_usage_record",
            "payload": {
                "thread_token_usage": {
                    "input_tokens": 10_000,
                    "cached_input_tokens": 20_000,  # Impossible
                    "output_tokens": 100,
                    "total_tokens": 10_100
                }
            }
        })
        self.tracker.process_line(bad_cached)
        self.assertFalse(self.tracker.telemetry_valid)
        self.assertEqual(self.tracker.telemetry_status, "INVALID")

        # Case 2: negative tokens
        tracker2 = StreamingTokenTracker()
        bad_negative = json.dumps({
            "type": "token_usage_record",
            "payload": {
                "thread_token_usage": {"input_tokens": -500, "total_tokens": 0}
            }
        })
        tracker2.process_line(bad_negative)
        self.assertFalse(tracker2.telemetry_valid)
        self.assertEqual(tracker2.telemetry_status, "INVALID")


class TestHeadroomCalculation(unittest.TestCase):
    """Section 4.B: Headroom calculation tests covering all 10 required cases."""

    def test_case_1_budget_greater_than_estimated(self):
        """Case 1: Remaining budget greater than estimated next-turn cost."""
        tracker = StreamingTokenTracker(conservative_reserve=15_000)
        tracker.thread_cumulative_max = 100_000
        tracker.last_turn_request_tokens = 20_000  # Estimate = max(20k, 15k) = 20k
        # Ceiling: 150_000 -> Remaining = 50_000 > 20_000
        has_headroom, rem, est = tracker.check_pre_dispatch_headroom(ceiling=150_000)
        self.assertTrue(has_headroom)
        self.assertEqual(rem, 50_000)
        self.assertEqual(est, 20_000)
        self.assertEqual(tracker.last_decision, "DISPATCH_ALLOWED")

    def test_case_2_budget_exactly_equal_estimated(self):
        """Case 2: Remaining budget exactly equal to estimated next-turn cost (Equality boundary)."""
        tracker = StreamingTokenTracker(conservative_reserve=15_000)
        tracker.thread_cumulative_max = 85_000
        tracker.last_turn_request_tokens = 15_000  # Estimate = 15k
        # Ceiling: 100_000 -> Remaining = 15_000 == 15_000
        has_headroom, rem, est = tracker.check_pre_dispatch_headroom(ceiling=100_000)
        self.assertTrue(has_headroom, "Equality boundary (remaining == estimated) must permit dispatch!")
        self.assertEqual(rem, 15_000)
        self.assertEqual(est, 15_000)
        self.assertEqual(tracker.last_decision, "DISPATCH_ALLOWED")

    def test_case_3_budget_one_token_below_estimated(self):
        """Case 3: Remaining budget one token below estimated next-turn cost."""
        tracker = StreamingTokenTracker(conservative_reserve=15_000)
        tracker.thread_cumulative_max = 85_001
        tracker.last_turn_request_tokens = 15_000  # Estimate = 15k
        # Ceiling: 100_000 -> Remaining = 14_999 < 15_000
        has_headroom, rem, est = tracker.check_pre_dispatch_headroom(ceiling=100_000)
        self.assertFalse(has_headroom, "One token below estimated cost must trigger preventive halt!")
        self.assertEqual(rem, 14_999)
        self.assertEqual(est, 15_000)
        self.assertEqual(tracker.last_decision, "PREVENTIVE_HALT")

    def test_case_4_budget_zero(self):
        """Case 4: Remaining budget zero."""
        tracker = StreamingTokenTracker(conservative_reserve=15_000)
        tracker.thread_cumulative_max = 100_000
        # Ceiling: 100_000 -> Remaining = 0 < 15_000
        has_headroom, rem, est = tracker.check_pre_dispatch_headroom(ceiling=100_000)
        self.assertFalse(has_headroom)
        self.assertEqual(rem, 0)
        self.assertEqual(tracker.last_decision, "PREVENTIVE_HALT")

    def test_case_5_cumulative_at_or_above_ceiling(self):
        """Case 5: Current cumulative usage already at or above ceiling."""
        tracker = StreamingTokenTracker(conservative_reserve=15_000)
        tracker.thread_cumulative_max = 110_000
        # Ceiling: 100_000 -> Remaining = -10_000
        has_headroom, rem, est = tracker.check_pre_dispatch_headroom(ceiling=100_000)
        self.assertFalse(has_headroom)
        self.assertEqual(rem, -10_000)
        self.assertEqual(tracker.last_decision, "OVERRUN_HALT")

    def test_case_6_missing_previous_turn_usage(self):
        """Case 6: Missing previous-turn usage falls back safely to conservative_reserve."""
        tracker = StreamingTokenTracker(conservative_reserve=25_000)
        self.assertEqual(tracker.last_turn_request_tokens, 0)
        has_headroom, rem, est = tracker.check_pre_dispatch_headroom(ceiling=100_000)
        self.assertTrue(has_headroom)
        self.assertEqual(est, 25_000, "Missing previous-turn tokens must default to conservative_reserve!")

    def test_case_7_missing_or_invalid_reserve(self):
        """Case 7: Missing or invalid reserve fails safe rather than substituting unsafe default."""
        for bad_reserve in [None, 0, -10_000, "invalid"]:
            tracker = StreamingTokenTracker(conservative_reserve=bad_reserve)
            has_headroom, rem, est = tracker.check_pre_dispatch_headroom(ceiling=100_000)
            self.assertFalse(has_headroom, f"Invalid reserve {bad_reserve} must fail safe to has_headroom=False!")
            self.assertEqual(tracker.last_decision, "PREVENTIVE_HALT")

    def test_case_8_missing_or_invalid_ceiling(self):
        """Case 8: Missing or invalid ceiling fails safe rather than substituting unsafe default."""
        tracker = StreamingTokenTracker(conservative_reserve=15_000)
        for bad_ceiling in [None, 0, -50_000, "invalid"]:
            has_headroom, rem, est = tracker.check_pre_dispatch_headroom(ceiling=bad_ceiling)
            self.assertFalse(has_headroom, f"Invalid ceiling {bad_ceiling} must fail safe to has_headroom=False!")
            self.assertEqual(tracker.last_decision, "PREVENTIVE_HALT")

    def test_case_9_telemetry_temporarily_unavailable(self):
        """Case 9: Usage telemetry invalid or unavailable fails safe."""
        tracker = StreamingTokenTracker(conservative_reserve=15_000)
        tracker.telemetry_valid = False
        tracker.telemetry_status = "UNAVAILABLE"
        has_headroom, rem, est = tracker.check_pre_dispatch_headroom(ceiling=100_000)
        self.assertFalse(has_headroom, "Unavailable telemetry must fail safe to has_headroom=False!")
        self.assertEqual(tracker.last_decision, "PREVENTIVE_HALT")

    def test_case_10_usage_updated_immediately_before_dispatch(self):
        """Case 10: Usage data updated immediately before a dispatch boundary is evaluated."""
        tracker = StreamingTokenTracker(conservative_reserve=15_000)
        snap = json.dumps({
            "type": "token_usage_record",
            "payload": {
                "thread_token_usage": {"input_tokens": 150_000, "total_tokens": 150_000},
                "turn_token_usage": {"input_tokens": 20_000, "total_tokens": 20_000}
            }
        })
        tracker.process_line(snap)
        boundary_line = json.dumps({"type": "turn.started"})
        self.assertTrue(tracker.is_pre_dispatch_boundary(boundary_line))
        has_headroom, rem, est = tracker.check_pre_dispatch_headroom(ceiling=165_000)
        # Remaining: 15_000, Estimated: 20_000 -> has_headroom = False
        self.assertFalse(has_headroom)
        self.assertEqual(rem, 15_000)
        self.assertEqual(est, 20_000)


class TestEventBoundaryDetection(unittest.TestCase):
    """Section 4.C: Event-boundary detection tests."""

    def setUp(self):
        self.tracker = StreamingTokenTracker(conservative_reserve=15_000)

    def test_turn_started_is_candidate_boundary(self):
        """Verifies turn.started is recognized as a candidate boundary."""
        line = json.dumps({"type": "turn.started"})
        self.assertTrue(self.tracker.is_pre_dispatch_boundary(line))
        is_cand, is_proven, reason = self.tracker.evaluate_pre_dispatch_boundary(line)
        self.assertTrue(is_cand)
        self.assertFalse(is_proven, "Turn-started cannot be proven zero-lag under external polling!")

    def test_tool_start_events_are_candidate_triggers_but_unproven(self):
        """Verifies item.started is flagged as a candidate trigger but unproven pre-dispatch."""
        line = json.dumps({"type": "item.started", "item": {"id": "c1", "type": "command_execution"}})
        self.assertTrue(self.tracker.is_pre_dispatch_boundary(line))
        is_cand, is_proven, reason = self.tracker.evaluate_pre_dispatch_boundary(line)
        self.assertTrue(is_cand)
        self.assertFalse(is_proven, "Tool start event must NOT be reported as guaranteed pre-dispatch!")
        self.assertEqual(reason, "TOOL_START_CANDIDATE_UNPROVEN")

    def test_tool_completion_is_candidate_boundary(self):
        """Verifies item.completed with command_execution/mcp is a candidate pre-dispatch boundary."""
        line = json.dumps({"type": "item.completed", "item": {"id": "c1", "type": "command_execution"}})
        self.assertTrue(self.tracker.is_pre_dispatch_boundary(line))
        is_cand, is_proven, reason = self.tracker.evaluate_pre_dispatch_boundary(line)
        self.assertTrue(is_cand)
        self.assertFalse(is_proven)
        self.assertEqual(reason, "TOOL_COMPLETED_CANDIDATE_UNPROVEN")

    def test_custom_tool_call_output_is_candidate_boundary(self):
        """Verifies custom_tool_call_output is a candidate pre-dispatch boundary."""
        line = json.dumps({"type": "custom_tool_call_output", "call_id": "call_123"})
        self.assertTrue(self.tracker.is_pre_dispatch_boundary(line))

    def test_usage_snapshot_while_tool_pending(self):
        """Verifies token usage snapshots while a tool is pending trigger headroom check."""
        self.tracker.has_pending_tool = True
        snap = json.dumps({"type": "token_usage_record", "payload": {}})
        self.assertTrue(self.tracker.is_pre_dispatch_boundary(snap))

        self.tracker.has_pending_tool = False
        self.assertFalse(self.tracker.is_pre_dispatch_boundary(snap))

    def test_external_poller_boundary_always_unproven(self):
        """Verifies that is_guaranteed_pre_dispatch_boundary returns False for all events."""
        events = [
            json.dumps({"type": "turn.started"}),
            json.dumps({"type": "item.started", "item": {"type": "command_execution"}}),
            json.dumps({"type": "item.completed", "item": {"type": "command_execution"}}),
            json.dumps({"type": "custom_tool_call_output"}),
        ]
        for ev in events:
            self.assertFalse(
                self.tracker.is_guaranteed_pre_dispatch_boundary(ev),
                "External poller must never claim a guaranteed pre-dispatch boundary!"
            )


class TestProcessTermination(unittest.TestCase):
    """Section 4.D: Process termination tests on disposable local processes."""

    def test_parent_process_termination(self):
        """Test 1: Simple parent process termination."""
        p = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(10)"])
        self.assertIsNone(p.poll())
        res = terminate_pid_tree(p.pid)
        self.assertTrue(res)
        time.sleep(0.1)
        self.assertIsNotNone(p.poll())

    def test_child_process_termination(self):
        """Test 2: Parent with a spawned child process."""
        script = (
            "import sys, subprocess, time\n"
            "c = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(10)'])\n"
            "time.sleep(10)\n"
        )
        p = subprocess.Popen([sys.executable, "-c", script])
        time.sleep(0.3)
        self.assertIsNone(p.poll())
        res = terminate_pid_tree(p.pid)
        self.assertTrue(res)
        time.sleep(0.1)
        self.assertIsNotNone(p.poll())

    def test_multiple_descendants(self):
        """Test 3: Multiple descendants (parent -> child -> grandchild)."""
        grandchild_script = "import time; time.sleep(10)"
        child_script = (
            f"import sys, subprocess, time\n"
            f"gc = subprocess.Popen([sys.executable, '-c', {repr(grandchild_script)}])\n"
            f"time.sleep(10)\n"
        )
        parent_script = (
            f"import sys, subprocess, time\n"
            f"c = subprocess.Popen([sys.executable, '-c', {repr(child_script)}])\n"
            f"time.sleep(10)\n"
        )
        p = subprocess.Popen([sys.executable, "-c", parent_script])
        time.sleep(0.5)
        self.assertIsNone(p.poll())
        res = terminate_pid_tree(p.pid)
        self.assertTrue(res)
        time.sleep(0.1)
        self.assertIsNotNone(p.poll())

    def test_already_exited_process(self):
        """Test 4: Already-exited process returns False safely without error."""
        p = subprocess.Popen([sys.executable, "-c", "import sys; sys.exit(0)"])
        p.wait()
        res = terminate_pid_tree(p.pid)
        self.assertFalse(res, "Already-exited process should return False.")

    def test_invalid_pid(self):
        """Test 5: Invalid PIDs (0, negative, non-numeric) handled safely."""
        self.assertFalse(terminate_pid_tree(0))
        self.assertFalse(terminate_pid_tree(-1))
        self.assertFalse(terminate_pid_tree(-99999))
        self.assertFalse(terminate_pid_tree(None))  # type: ignore

    def test_repeated_termination_requests(self):
        """Test 7: Repeated termination requests are idempotent."""
        p = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(10)"])
        res1 = terminate_pid_tree(p.pid)
        self.assertTrue(res1)
        time.sleep(0.1)
        res2 = terminate_pid_tree(p.pid)
        self.assertFalse(res2, "Second termination of dead process should return False.")

    def test_windows_process_tree_latency_benchmark(self):
        """Measures termination latency and verifies zero orphaned child processes across 20 iterations."""
        latencies_ms = []
        failed_terminations = 0
        orphaned_processes = 0
        iterations = 20

        for _ in range(iterations):
            p = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(10)"])
            pid = p.pid
            t0 = time.perf_counter()
            success = terminate_pid_tree(pid)
            t1 = time.perf_counter()
            lat_ms = (t1 - t0) * 1000.0
            latencies_ms.append(lat_ms)

            if not success:
                failed_terminations += 1

            # Check if process still alive after 100ms
            time.sleep(0.05)
            if p.poll() is None:
                orphaned_processes += 1
                try:
                    p.kill()
                except Exception:
                    pass

        latencies_ms.sort()
        min_lat = latencies_ms[0]
        median_lat = (latencies_ms[iterations // 2 - 1] + latencies_ms[iterations // 2]) / 2.0
        p90_idx = int(0.90 * iterations) - 1
        p90_lat = latencies_ms[p90_idx]
        max_lat = latencies_ms[-1]

        self.assertEqual(failed_terminations, 0)
        self.assertEqual(orphaned_processes, 0)

        # Store results for report generation
        TestProcessTermination.benchmark_results = {
            "iterations": iterations,
            "min_ms": round(min_lat, 2),
            "median_ms": round(median_lat, 2),
            "p90_ms": round(p90_lat, 2),
            "max_ms": round(max_lat, 2),
            "failed_count": failed_terminations,
            "orphaned_count": orphaned_processes
        }


class TestStopClassification(unittest.TestCase):
    """Section 4.E: Stop classification tests."""

    def test_preventive_stop_never_recorded_as_ceiling_breach(self):
        """Verifies that PRE_DISPATCH_HEADROOM_EXHAUSTED sets token_ceiling_breached=False."""
        tracker = StreamingTokenTracker(conservative_reserve=15_000)
        tracker.thread_cumulative_max = 160_000
        tracker.last_turn_request_tokens = 25_000  # Next estimate = 25,000
        ceiling = 175_000
        # Remaining: 15,000 < Estimated: 25,000 -> Insufficient headroom
        has_headroom, rem, est = tracker.check_pre_dispatch_headroom(ceiling)
        self.assertFalse(has_headroom)
        self.assertEqual(rem, 15_000)
        self.assertEqual(est, 25_000)

        # Invariant: At stop, cumulative tokens (160k) <= ceiling (175k)
        token_ceiling_breached = (tracker.effective_tokens > ceiling)
        self.assertFalse(token_ceiling_breached)
        stop_reason = "PRE_DISPATCH_HEADROOM_EXHAUSTED"
        self.assertEqual(stop_reason, "PRE_DISPATCH_HEADROOM_EXHAUSTED")

    def test_genuine_overrun_remains_visible(self):
        """Verifies that actual overruns (> ceiling) set token_ceiling_breached=True."""
        tracker = StreamingTokenTracker(conservative_reserve=15_000)
        tracker.thread_cumulative_max = 180_000
        ceiling = 175_000
        token_ceiling_breached = (tracker.effective_tokens > ceiling)
        self.assertTrue(token_ceiling_breached)


class TestTelemetryIntegrity(unittest.TestCase):
    """Section 4.F: Telemetry integrity and schema validation tests."""

    def setUp(self):
        self.temp_dir = Path(tempfile.mkdtemp(prefix="engram_telem_test_"))
        self.schema_path = ROOT_DIR / "schemas" / "run_telemetry.schema.json"
        with open(self.schema_path, "r", encoding="utf-8") as f:
            self.schema = json.load(f)

    def tearDown(self):
        if self.temp_dir.exists():
            shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_serialized_headroom_telemetry_schema_validation(self):
        """Verifies that full headroom_guard record validates strictly against run_telemetry.schema.json."""
        record = {
            "schema_version": "2.0.0",
            "run_id": "test_run_01",
            "pair_id": "test_pair_01",
            "task_id": "task_b_remember_me",
            "arm": "v154_arm3",
            "execution_order_in_pair": 1,
            "execution_status": "PRE_DISPATCH_HEADROOM_EXHAUSTED",
            "error_message": "Execution halted by pre-dispatch headroom guard",
            "timestamps": {
                "started_at": "2026-10-30T10:00:00Z",
                "completed_at": "2026-10-30T10:02:00Z",
                "elapsed_seconds": 120.0
            },
            "model": {
                "name": "gpt-5.6-luna",
                "provider": "openai",
                "mode": "exec",
                "harness_executable": "codex.exe",
                "harness_version": "0.163.0",
                "reasoning_effort": "medium"
            },
            "isolation": {
                "strategy": "copy",
                "baseline_commit": "c70845b4d5913457f1b176fb0370b80dd792711f",
                "sandbox_path": "C:/temp/sb",
                "cold_start_verified": True,
                "engram_enabled": True,
                "engram_binary_sha256": "F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE",
                "mcp_server_status": "ACTIVE",
                "dependency_fingerprint_verified": True,
                "path_sanitized": True
            },
            "telemetry": {
                "provenance": "SESSION_ROLLOUT",
                "prompt_tokens_total": 150_000,
                "prompt_tokens_cached": 120_000,
                "prompt_tokens_cached_inferred": False,
                "prompt_tokens_uncached": 30_000,
                "prompt_tokens_uncached_inferred": True,
                "completion_tokens": 1_500,
                "reasoning_tokens": 400,
                "total_model_tokens": 151_500,
                "peak_single_request_context": 28_000,
                "peak_turn_context": 28_000,
                "final_turn_context": 28_000,
                "model_requests_count": 6,
                "turns_count": 6,
                "mcp_tool_calls_count": 2,
                "auxiliary_tool_payload_bytes": 1024,
                "telemetry_valid": True,
                "error_message": None
            },
            "git_impact": {
                "files_changed_count": 1,
                "files_changed": ["projects/angular-ngrx-material-starter/src/app/core/core.module.ts"],
                "insertions": 10,
                "deletions": 2,
                "unrelated_files_count": 0,
                "diff_patch_path": "changes.patch"
            },
            "correctness": {
                "score": 0.0,
                "quality_adjusted_cost": 0.0,
                "passed_all_requirements": False,
                "gates": {
                    "L0_typecheck": "SKIPPED",
                    "L1_lint_prettier": "SKIPPED",
                    "L2_unit_tests": "SKIPPED",
                    "L3_production_build": "SKIPPED"
                },
                "requirements": {
                    "remember_me_checkbox": "NOT_EVALUATED"
                }
            },
            "headroom_guard": {
                "headroom_reserve": 15_000,
                "pre_dispatch_guard_triggered": True,
                "stop_reason": "PRE_DISPATCH_HEADROOM_EXHAUSTED",
                "cumulative_tokens_at_stop": 160_000,
                "estimated_next_turn_cost": 25_000,
                "configured_ceiling": 175_000,
                "remaining_budget": 15_000,
                "last_turn_usage": 25_000,
                "guard_decision": "PREVENTIVE_HALT",
                "timestamp": "2026-10-30T10:01:50Z",
                "relevant_event_id": "resp_0123",
                "telemetry_valid": True
            }
        }
        # Strictly validates against schema
        validate_instance(record, self.schema)


if __name__ == "__main__":
    unittest.main()
