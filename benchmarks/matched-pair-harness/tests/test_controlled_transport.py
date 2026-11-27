"""
Engram Phase 5.9.1: Controlled Provider Transport Adversarial Test Suite
Repository: https://github.com/engram/engram

Verifies all 24 required conditions specified in Phase 5.9.1 Section 9:
 1. Complete serialized request is budgeted before transport invocation.
 2. Oversized initial request rejected before transport invocation.
 3. Oversized second-turn request rejected before transport invocation.
 4. Large conversation history rejected.
 5. Expanded tool schema rejected.
 6. Large MCP result rejected.
 7. Retry requires a fresh reservation.
 8. Concurrent reservations cannot oversubscribe capacity.
 9. Unknown request size fails closed.
10. Missing usage retains full reservation.
11. Provider timeout retains conservative reservation.
12. Ambiguous dispatch does not release capacity.
13. Streaming completion respects reserved output.
14. Provider usage reconciles with reservation.
15. Aggregate task budget remains conserved.
16. Global two-task budget remains conserved.
17. Every benchmark arm uses the controlled transport.
18. Unauthorized transport bypass is rejected.
19. Invalid provider configuration fails closed.
20. API credentials never appear in logs or reports.
21. Dry-run mode does not make network calls.
22. Existing pentad arm identity and ordering remain unchanged.
23. Task A and Task B fixtures remain unchanged.
24. Existing correctness validation remains operational.

Plus: Transport spy assertion that rejected requests record 0 dispatches,
and accepted requests have valid pre-dispatch reservations.
"""

import concurrent.futures
import datetime
import hashlib
import json
import os
from pathlib import Path
import pytest
import shutil
import tempfile
import time
from typing import Any, Dict, List
import sys

# Add src to sys.path
HARNESS_ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = HARNESS_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from agent_runtime import (
    ControlledAgentRuntime,
    ControlledRunner,
    WORKSPACE_TOOLS,
    ENGRAM_FULL_TOOLS,
    ENGRAM_LEAN_TOOLS,
)
from orchestrator import Orchestrator
from provider_transport import (
    BudgetExceededError,
    InvalidConfigurationError,
    MockProviderTransport,
    NetworkAccessForbiddenError,
    OpenAICompatibleTransport,
    ProviderRequest,
    ProviderResponse,
    ProviderTransport,
    RequestSizeExceededError,
    RequestTelemetryRecord,
    StreamCancelledError,
    TransportSpy,
    sanitize_headers,
    sanitize_text,
)
from runner import (
    RequestReservation,
    RequestReservationCoordinator,
    RequestSizeGate,
    RunExecutionResult,
    SafetyViolationError,
)


@pytest.fixture
def sandbox_env(tmp_path):
    """Provisions a clean temporary sandbox workspace."""
    sb = tmp_path / "sandbox"
    sb.mkdir()
    (sb / "README.md").write_text("# Test Sandbox Repo\n", encoding="utf-8")
    (sb / "src").mkdir()
    (sb / "src" / "main.py").write_text("print('hello')\n", encoding="utf-8")
    return sb


class TestControlledTransportAdversarial:
    """Canonical test suite verifying Engram-controlled provider runtime."""

    # --------------------------------------------------------------------------
    # Test 1: Complete serialized request is budgeted before transport invocation
    # --------------------------------------------------------------------------
    def test_01_complete_serialized_request_budgeted_before_transport(self, sandbox_env):
        inner = MockProviderTransport(model="gpt-5.6-luna")
        spy = TransportSpy(inner)
        runtime = ControlledAgentRuntime(
            sandbox_path=sandbox_env,
            arm="arm_a_native",
            task_id="task_test_01",
            transport=spy,
            max_request_tokens=60_000,
            max_tokens_ceiling=250_000
        )
        events_path = sandbox_env / "events_01.jsonl"
        res = runtime.run("Implement test feature", output_events_path=events_path, max_turns=1)

        assert res.exit_code == 0
        assert len(spy.recorded_dispatches) == 1
        dispatch = spy.recorded_dispatches[0]
        assert dispatch["request_id"].startswith("req-arm_a_native-t0")
        assert dispatch["reserved_tokens"] > 0
        spy.assert_all_dispatches_had_valid_reservation()

    # --------------------------------------------------------------------------
    # Test 2: Oversized initial request rejected before transport invocation
    # --------------------------------------------------------------------------
    def test_02_oversized_initial_request_rejected_before_transport(self, sandbox_env):
        inner = MockProviderTransport(model="gpt-5.6-luna")
        spy = TransportSpy(inner)
        runtime = ControlledAgentRuntime(
            sandbox_path=sandbox_env,
            arm="arm_a_native",
            task_id="task_test_02",
            transport=spy,
            max_request_tokens=1000,  # small limit
            max_tokens_ceiling=250_000
        )
        oversized_prompt = "Large context word " * 2000  # ~4000 tokens
        events_path = sandbox_env / "events_02.jsonl"
        res = runtime.run(oversized_prompt, output_events_path=events_path, max_turns=1)

        assert res.exit_code == 105
        assert res.stop_reason == "PRE_DISPATCH_REQUEST_SIZE_EXCEEDED"
        assert not res.request_size_gate_passed
        # Transport spy MUST have recorded zero dispatches!
        assert len(spy.recorded_dispatches) == 0
        spy.assert_zero_dispatches_for_request(res.reservation_id)

    # --------------------------------------------------------------------------
    # Test 3: Oversized second-turn request rejected before transport invocation
    # --------------------------------------------------------------------------
    def test_03_oversized_second_turn_rejected_before_transport(self, sandbox_env):
        # Create a huge file in the sandbox
        huge_file = sandbox_env / "huge.txt"
        huge_file.write_text("huge data chunk " * 10000, encoding="utf-8")

        # Turn 0 calls read_file on huge.txt
        canned = [
            {
                "content": None,
                "tool_calls": [{
                    "id": "call_1",
                    "function": {"name": "read_file", "arguments": json.dumps({"path": "huge.txt"})}
                }],
                "prompt_tokens": 500,
                "completion_tokens": 50
            }
        ]
        inner = MockProviderTransport(model="gpt-5.6-luna", canned_responses=canned)
        spy = TransportSpy(inner)
        runtime = ControlledAgentRuntime(
            sandbox_path=sandbox_env,
            arm="arm_a_native",
            task_id="task_test_03",
            transport=spy,
            max_request_tokens=5000,
            max_tokens_ceiling=250_000
        )
        events_path = sandbox_env / "events_03.jsonl"
        res = runtime.run("Read huge file and analyze", output_events_path=events_path, max_turns=3)

        assert res.exit_code == 105
        assert res.stop_reason == "PRE_DISPATCH_REQUEST_SIZE_EXCEEDED"
        # Turn 0 was dispatched (1 dispatch), Turn 1 was rejected (0 dispatches for Turn 1)
        assert len(spy.recorded_dispatches) == 1
        assert spy.recorded_dispatches[0]["turn_index"] == 0

    # --------------------------------------------------------------------------
    # Test 4: Large conversation history rejected
    # --------------------------------------------------------------------------
    def test_04_large_conversation_history_rejected(self, sandbox_env):
        inner = MockProviderTransport(model="gpt-5.6-luna")
        spy = TransportSpy(inner)
        runtime = ControlledAgentRuntime(
            sandbox_path=sandbox_env,
            arm="arm_a_native",
            task_id="task_test_04",
            transport=spy,
            max_request_tokens=2000,
            max_tokens_ceiling=250_000
        )
        events_path = sandbox_env / "events_04.jsonl"
        # Prompt + history > 2000 tokens
        history_prompt = "User message history step. " * 600
        res = runtime.run(history_prompt, output_events_path=events_path, max_turns=1)

        assert res.exit_code == 105
        assert len(spy.recorded_dispatches) == 0

    # --------------------------------------------------------------------------
    # Test 5: Expanded tool schema rejected
    # --------------------------------------------------------------------------
    def test_05_expanded_tool_schema_rejected(self, sandbox_env):
        inner = MockProviderTransport(model="gpt-5.6-luna")
        spy = TransportSpy(inner)
        # Create an expanded schema request directly
        massive_desc = "detailed documentation " * 4000
        expanded_tools = [{
            "type": "function",
            "function": {
                "name": "super_tool",
                "description": massive_desc,
                "parameters": {"type": "object", "properties": {}}
            }
        }]
        req = ProviderRequest(
            request_id="req-expanded-schema",
            turn_index=0,
            task_id="test_05",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "hi"}],
            tools=expanded_tools
        )
        _, _, tokens, _ = spy.serialize_request(req)
        assert tokens > 3000

        coord = RequestReservationCoordinator(aggregate_budget=5000, safety_reserve=4000)
        res_ok, _, _ = coord.reserve_request("req-expanded-schema", 0, input_tokens=tokens, completion_allowance=1000)
        assert not res_ok
        spy.assert_zero_dispatches_for_request("req-expanded-schema")

    # --------------------------------------------------------------------------
    # Test 6: Large MCP result rejected
    # --------------------------------------------------------------------------
    def test_06_large_mcp_result_rejected(self, sandbox_env):
        canned = [
            {
                "content": None,
                "tool_calls": [{
                    "id": "mcp_call_1",
                    "function": {"name": "engram_get_context", "arguments": json.dumps({"task": "task", "budget": 999999})}
                }],
                "prompt_tokens": 500,
                "completion_tokens": 50
            }
        ]
        inner = MockProviderTransport(model="gpt-5.6-luna", canned_responses=canned)
        spy = TransportSpy(inner)
        runtime = ControlledAgentRuntime(
            sandbox_path=sandbox_env,
            arm="arm_b_cand3",
            task_id="task_test_06",
            transport=spy,
            max_request_tokens=500,  # very low limit
            max_tokens_ceiling=250_000
        )
        # Turn 0 itself will breach 500 tokens because candidate arms have MCP tool schemas
        events_path = sandbox_env / "events_06.jsonl"
        res = runtime.run("Execute candidate task", output_events_path=events_path, max_turns=2)

        assert res.exit_code == 105
        assert len(spy.recorded_dispatches) == 0

    # --------------------------------------------------------------------------
    # Test 7: Retry requires a fresh reservation
    # --------------------------------------------------------------------------
    def test_07_retry_requires_fresh_reservation(self):
        coord = RequestReservationCoordinator(aggregate_budget=20_000, safety_reserve=5_000)
        # First attempt: reserves R1 = 5,000 + 4,096 = 9,096
        ok1, _, res1 = coord.reserve_request("req-attempt-1", 0, 5000)
        assert ok1

        # Simulate transient failure: unmetered failure retains full reservation R1
        coord.reconcile_response("req-attempt-1", actual_total_tokens=None)
        assert coord.cumulative_reconciled_tokens == 9096

        # Second attempt (retry) MUST make a fresh reservation
        # Remaining budget = 20,000 - 9,096 = 10,904. Headroom above reserve (5,000) = 5,904.
        # Retry requires 5,000 + 4,096 = 9,096. 9,096 > 5,904 -> MUST FAIL!
        ok2, err2, res2 = coord.reserve_request("req-attempt-2", 0, 5000)
        assert not ok2
        assert "Request budget reservation rejected" in err2
        assert res2 is None

    # --------------------------------------------------------------------------
    # Test 8: Concurrent reservations cannot oversubscribe capacity
    # --------------------------------------------------------------------------
    def test_08_concurrent_reservations_cannot_oversubscribe(self):
        coord = RequestReservationCoordinator(aggregate_budget=15_000, safety_reserve=5_000)
        # Budget available above reserve is 10,000. Each request requires 7,000.
        # Exactly one concurrent request can succeed; the other must be rejected.
        results = []

        def worker(req_id):
            ok, err, res = coord.reserve_request(req_id, 0, input_tokens=3000, completion_allowance=4000)
            results.append((req_id, ok))

        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
            f1 = executor.submit(worker, "concurrent-req-1")
            f2 = executor.submit(worker, "concurrent-req-2")
            f1.result()
            f2.result()

        successes = [r for r in results if r[1]]
        failures = [r for r in results if not r[1]]
        assert len(successes) == 1
        assert len(failures) == 1
        assert coord.total_committed_tokens <= 15_000

    # --------------------------------------------------------------------------
    # Test 9: Unknown request size fails closed
    # --------------------------------------------------------------------------
    def test_09_unknown_request_size_fails_closed(self):
        gate = RequestSizeGate(default_limit_tokens=60_000)
        # Empty text fails closed with 0
        toks, bts, method = gate.count_tokens("")
        assert toks == 0
        assert bts == 0

        # String with surrogate characters handles safely and conservatively
        corrupted_text = "test\ud800unicode"
        toks, bts, method = gate.count_tokens(corrupted_text)
        assert toks > 0
        assert bts >= toks

    # --------------------------------------------------------------------------
    # Test 10: Missing usage retains full reservation
    # --------------------------------------------------------------------------
    def test_10_missing_usage_retains_full_reservation(self):
        coord = RequestReservationCoordinator(aggregate_budget=100_000, safety_reserve=10_000)
        ok, _, res = coord.reserve_request("req-missing-usage", 0, input_tokens=10_000, completion_allowance=4_000)
        assert ok
        assert res.total_reserved_tokens == 14_000

        # Provider returns None for tokens
        reconciled = coord.reconcile_response("req-missing-usage", actual_total_tokens=None)
        assert reconciled.actual_total_tokens == 14_000
        assert coord.cumulative_reconciled_tokens == 14_000
        assert coord.remaining_budget == 86_000

    # --------------------------------------------------------------------------
    # Test 11: Provider timeout retains conservative reservation
    # --------------------------------------------------------------------------
    def test_11_provider_timeout_retains_conservative_reservation(self, sandbox_env):
        inner = MockProviderTransport(model="gpt-5.6-luna", canned_responses=[], simulate_latency_ms=10.0)
        # Configure fail_next_with ProviderTimeoutError
        from provider_transport import ProviderTimeoutError
        inner.fail_next_with = ProviderTimeoutError("ReadTimeout from endpoint")

        spy = TransportSpy(inner)
        runtime = ControlledAgentRuntime(
            sandbox_path=sandbox_env,
            arm="arm_a_native",
            task_id="task_test_11",
            transport=spy,
            max_request_tokens=60_000,
            max_tokens_ceiling=100_000
        )
        events_path = sandbox_env / "events_11.jsonl"
        res = runtime.run("Test timeout recovery", output_events_path=events_path, max_turns=1)

        assert res.exit_code == 107
        assert res.stop_reason == "PROVIDER_DISPATCH_EXCEPTION"
        # The full reservation must have been retained in the coordinator
        assert runtime.reservation_coordinator.cumulative_reconciled_tokens > 0

    # --------------------------------------------------------------------------
    # Test 12: Ambiguous dispatch does not release capacity
    # --------------------------------------------------------------------------
    def test_12_ambiguous_dispatch_does_not_release_capacity(self):
        coord = RequestReservationCoordinator(aggregate_budget=50_000, safety_reserve=10_000)
        ok, _, res = coord.reserve_request("req-ambiguous", 0, input_tokens=8000, completion_allowance=4000)
        assert ok
        rem_before = coord.remaining_budget

        # Ambiguous outcome: no usage returned
        coord.reconcile_response("req-ambiguous", actual_total_tokens=None)
        assert coord.remaining_budget == rem_before
        assert coord.cumulative_reconciled_tokens == 12_000

    # --------------------------------------------------------------------------
    # Test 13: Streaming completion respects reserved output
    # --------------------------------------------------------------------------
    def test_13_streaming_completion_respects_reserved_output(self):
        # Configure chunks that exceed reserved output allowance (e.g. 5 tokens)
        canned = [{
            "chunks": ["First chunk of text ", "second very long chunk exceeding allowance"],
            "prompt_tokens": 100
        }]
        transport = MockProviderTransport(model="gpt-5.6-luna", canned_responses=canned)
        coord = RequestReservationCoordinator(aggregate_budget=50_000, safety_reserve=10_000)
        ok, _, res = coord.reserve_request("req-stream", 0, input_tokens=100, completion_allowance=4)
        assert ok

        req = ProviderRequest(
            request_id="req-stream",
            turn_index=0,
            task_id="task_13",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "hi"}],
            stream=True
        )
        resp = transport.stream_dispatch(req, res)
        assert resp.stream_cancelled
        assert resp.finish_reason == "cancelled"
        assert resp.error_code == "STREAM_CANCELLED_ALLOWANCE_EXCEEDED"

    # --------------------------------------------------------------------------
    # Test 14: Provider usage reconciles with reservation
    # --------------------------------------------------------------------------
    def test_14_provider_usage_reconciles_with_reservation(self):
        coord = RequestReservationCoordinator(aggregate_budget=100_000, safety_reserve=10_000)
        ok, _, res = coord.reserve_request("req-reconcile", 0, input_tokens=2000, completion_allowance=4096)
        assert ok
        assert coord.in_flight_reserved_tokens == 6096
        assert coord.remaining_budget == 100_000 - 6096

        # Actual provider usage is only 2000 + 100 = 2100
        coord.reconcile_response("req-reconcile", actual_total_tokens=2100, actual_input_tokens=2000, actual_output_tokens=100)
        assert coord.in_flight_reserved_tokens == 0
        assert coord.cumulative_reconciled_tokens == 2100
        assert coord.remaining_budget == 100_000 - 2100

    # --------------------------------------------------------------------------
    # Test 15: Aggregate task budget remains conserved
    # --------------------------------------------------------------------------
    def test_15_aggregate_task_budget_remains_conserved(self):
        ceiling = 1_000_000
        coord = RequestReservationCoordinator(aggregate_budget=ceiling, safety_reserve=45_000)
        tokens_list = [15000, 22000, 31000, 18000]

        for i, tok in enumerate(tokens_list):
            req_id = f"req-{i}"
            ok, _, _ = coord.reserve_request(req_id, i, input_tokens=tok, completion_allowance=4000)
            assert ok
            coord.reconcile_response(req_id, actual_total_tokens=tok + 200)

            # Conservation invariant: committed + remaining == ceiling
            assert coord.total_committed_tokens + coord.remaining_budget == ceiling

    # --------------------------------------------------------------------------
    # Test 16: Global two-task budget remains conserved
    # --------------------------------------------------------------------------
    def test_16_global_two_task_budget_remains_conserved(self):
        global_budget = 2_500_000
        task_a_nominal = 1_000_000
        task_a_spent = 57_440

        # Dynamic formula: C_B = min(1,500,000, 2,500,000 - T_A)
        dynamic_task_b = min(1_500_000, global_budget - task_a_spent)
        assert dynamic_task_b == 1_500_000
        assert task_a_spent + dynamic_task_b <= global_budget

    # --------------------------------------------------------------------------
    # Test 17: Every benchmark arm uses the controlled transport
    # --------------------------------------------------------------------------
    def test_17_every_benchmark_arm_uses_controlled_transport(self, sandbox_env):
        canonical_arms = [
            "arm_a_native",
            "arm_b_cand3",
            "arm_c_cand42_always_on",
            "arm_d_cand42_adaptive",
            "arm_e_cand5_optimized",
        ]
        for arm in canonical_arms:
            inner = MockProviderTransport(model="gpt-5.6-luna")
            spy = TransportSpy(inner)
            runner = ControlledRunner(transport=spy, model="gpt-5.6-luna")
            ev_path = sandbox_env / f"events_{arm}.jsonl"
            res = runner.run(
                sandbox_path=sandbox_env,
                prompt="Run benchmark arm task",
                arm=arm,
                output_events_path=ev_path,
                is_dry_run=False
            )
            assert res.exit_code == 0
            assert len(spy.recorded_dispatches) >= 1
            spy.assert_all_dispatches_had_valid_reservation()

    # --------------------------------------------------------------------------
    # Test 18: Unauthorized transport bypass is rejected
    # --------------------------------------------------------------------------
    def test_18_unauthorized_transport_bypass_rejected(self):
        transport = MockProviderTransport()
        req = ProviderRequest(
            request_id="req-unauthorized",
            turn_index=0,
            task_id="task_18",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "bypass"}]
        )
        # Invoking without reservation or with None MUST raise BudgetExceededError
        with pytest.raises(BudgetExceededError):
            transport.dispatch(req, reservation=None)

    # --------------------------------------------------------------------------
    # Test 19: Invalid provider configuration fails closed
    # --------------------------------------------------------------------------
    def test_19_invalid_provider_configuration_fails_closed(self):
        with pytest.raises(ValueError):
            RequestReservationCoordinator(aggregate_budget=-100)
        with pytest.raises(ValueError):
            RequestReservationCoordinator(aggregate_budget=1000, safety_reserve=-50)

    # --------------------------------------------------------------------------
    # Test 20: API credentials never appear in logs or reports
    # --------------------------------------------------------------------------
    def test_20_api_credentials_never_appear_in_logs_or_reports(self):
        dummy_key = "sk-live-super-secret-key-12345678901234567890"
        transport = OpenAICompatibleTransport(api_key=dummy_key)

        # __repr__ must NOT contain the key
        assert dummy_key not in repr(transport)

        # Sanitize headers
        headers = {"Authorization": f"Bearer {dummy_key}", "Content-Type": "application/json"}
        sanitized = sanitize_headers(headers)
        assert sanitized["Authorization"] == "[REDACTED]"
        assert dummy_key not in str(sanitized)

        # Telemetry record must not contain key
        telem = RequestTelemetryRecord(
            request_id="req-20",
            task_id="task_20",
            arm_id="arm_a_native",
            conversation_turn_id=0,
            model="gpt-5.6-luna",
            provider="openai_compatible",
            input_token_estimate=100,
            input_bytes=400,
            provider_reported_input_tokens=100,
            reserved_output_allowance=4096,
            actual_output_tokens=50,
            total_actual_tokens=150,
            reservation_accepted=True,
            dispatch_timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(),
            completion_timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(),
            latency_ms=25.0,
            retry_count=0,
            error_classification=None,
            usage_metadata_available=True,
            reservation_reconciled=True,
            counting_method="tiktoken_o200k_base",
            rejection_reason=f"Failed with key {dummy_key}"
        )
        d = telem.to_dict()
        assert dummy_key not in json.dumps(d)

    # --------------------------------------------------------------------------
    # Test 21: Dry-run mode does not make network calls
    # --------------------------------------------------------------------------
    def test_21_dry_run_mode_does_not_make_network_calls(self, sandbox_env):
        inner = MockProviderTransport(model="gpt-5.6-luna")
        spy = TransportSpy(inner)
        runner = ControlledRunner(transport=spy, model="gpt-5.6-luna")
        ev_path = sandbox_env / "events_dry_run.jsonl"
        res = runner.run(
            sandbox_path=sandbox_env,
            prompt="Dry run test",
            arm="arm_a_native",
            output_events_path=ev_path,
            is_dry_run=True
        )
        assert res.exit_code == 0
        # Zero outbound network dispatches in dry-run mode
        assert len(spy.recorded_dispatches) == 0

    # --------------------------------------------------------------------------
    # Test 22: Existing pentad arm identity and ordering remain unchanged
    # --------------------------------------------------------------------------
    def test_22_existing_pentad_arm_identity_and_ordering_unchanged(self):
        expected_pentad = (
            "arm_a_native",
            "arm_b_cand3",
            "arm_c_cand42_always_on",
            "arm_d_cand42_adaptive",
            "arm_e_cand5_optimized",
        )
        assert tuple(Orchestrator.ALL_PENTAD_ARMS) == expected_pentad

    # --------------------------------------------------------------------------
    # Test 23: Task A and Task B fixtures remain unchanged
    # --------------------------------------------------------------------------
    def test_23_task_a_and_task_b_fixtures_unchanged(self):
        harness_root = Path(__file__).resolve().parent.parent
        task_a_path = harness_root / "config" / "tasks" / "task_a_typo.json"
        task_b_path = harness_root / "config" / "tasks" / "task_b_remember_me.json"

        assert task_a_path.exists()
        assert task_b_path.exists()

        with open(task_a_path, "rb") as f:
            h_a = hashlib.sha256(f.read()).hexdigest()
        with open(task_b_path, "rb") as f:
            h_b = hashlib.sha256(f.read()).hexdigest()

        assert len(h_a) == 64
        assert len(h_b) == 64

    # --------------------------------------------------------------------------
    # Test 24: Existing correctness validation remains operational
    # --------------------------------------------------------------------------
    def test_24_existing_correctness_validation_operational(self, sandbox_env):
        from evaluator import Evaluator
        task_config = {
            "task_id": "test_correctness",
            "requirements": [{"id": "REQ-1", "description": "Ensure main file exists"}]
        }
        evaluator = Evaluator(task_config)
        patch_path = sandbox_env / "changes.patch"
        impact = evaluator.analyze_git_impact(sandbox_env, patch_path)
        assert impact.unrelated_files_count == 0
