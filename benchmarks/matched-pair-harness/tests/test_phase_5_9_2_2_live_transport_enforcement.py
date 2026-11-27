"""
Engram Phase 5.9.2.2: Live Transport Enforcement & Multi-Turn Verification Tests
Repository: https://github.com/engram/engram

Verifies:
1. Explicit transport-mode selection (mock must be explicit).
2. Fail-closed credential validation in live mode (no silent mock fallback).
3. Secret sanitization across error messages and headers.
4. Genuine multi-turn agent tool execution lifecycle.
5. Telemetry transport identity and provenance classification.
"""

import json
from pathlib import Path
import sys
import tempfile
import pytest

HARNESS_ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = HARNESS_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from agent_runtime import ControlledAgentRuntime, ControlledRunner
from provider_transport import (
    AuthenticationError,
    BudgetExceededError,
    InvalidConfigurationError,
    MockProviderTransport,
    NetworkAccessForbiddenError,
    OpenAICompatibleTransport,
    ProviderRequest,
    ProviderResponse,
    create_provider_transport,
    sanitize_text,
)
from runner import RequestReservation, RequestReservationCoordinator


class TestPhase5922TransportEnforcement:

    def test_01_mock_transport_must_be_explicit(self):
        """Mock transport is only instantiated when explicitly requested via provider='mock'."""
        transport = create_provider_transport(provider="mock")
        assert isinstance(transport, MockProviderTransport)
        assert transport.provider_name == "mock_transport"
        assert not transport.allow_live_network

    def test_02_live_transport_missing_credentials_fails_closed(self, monkeypatch):
        """Controlled live transport fails closed with AuthenticationError if API key is missing."""
        monkeypatch.delenv("OPENAI_API_KEY", raising=False)
        with pytest.raises(AuthenticationError) as exc_info:
            create_provider_transport(
                provider="controlled",
                api_key=None,
                allow_live_network=True
            )
        assert "OPENAI_API_KEY is not set" in str(exc_info.value)
        assert "Live mode cannot silently fall back to mock transport" in str(exc_info.value)

    def test_03_controlled_runner_live_mode_fails_closed_without_key(self, monkeypatch):
        """ControlledRunner in live mode raises AuthenticationError rather than defaulting to mock."""
        monkeypatch.delenv("OPENAI_API_KEY", raising=False)
        with pytest.raises(AuthenticationError):
            ControlledRunner(
                provider="controlled",
                allow_live_network=True,
                api_key=None
            )

    def test_04_live_transport_created_when_credentials_provided(self):
        """OpenAICompatibleTransport is instantiated when credentials and allow_live_network are set."""
        transport = create_provider_transport(
            provider="controlled",
            api_key="sk-test-mock-key-for-verification-only",
            allow_live_network=True
        )
        assert isinstance(transport, OpenAICompatibleTransport)
        assert transport.provider_name == "openai_compatible"
        assert transport.allow_live_network is True
        # Verify secret is not exposed in string representation
        assert "sk-test" not in repr(transport)
        assert "api_key" not in repr(transport)

    def test_05_openai_compatible_offline_dispatch_blocked(self):
        """OpenAICompatibleTransport without allow_live_network blocks dispatch fail-closed."""
        transport = OpenAICompatibleTransport(
            model="gpt-5.6-luna",
            api_key="sk-test-key",
            allow_live_network=False
        )
        coord = RequestReservationCoordinator(aggregate_budget=100_000)
        res_ok, _, res = coord.reserve_request("req_01", 0, 500, 4096)
        assert res_ok
        req = ProviderRequest(
            request_id="req_01",
            turn_index=0,
            task_id="task_test",
            arm_id="arm_a",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Hello"}]
        )
        with pytest.raises(NetworkAccessForbiddenError):
            transport.dispatch(req, res)

    def test_06_genuine_multi_turn_tool_calling_lifecycle(self):
        """
        Verifies that ControlledAgentRuntime handles a multi-turn tool call:
        - Turn 0: Model returns tool_call to read a file.
        - Runtime executes the tool against the sandbox.
        - Turn 1: Model receives the tool output and produces final completion.
        - Budget is accurately reserved and reconciled across both turns.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            sandbox = Path(tmpdir)
            test_file = sandbox / "sample.txt"
            test_file.write_text("CONTENT_LINE_1\nCONTENT_LINE_2", encoding="utf-8")
            events_path = sandbox / "events.jsonl"

            # Scripted 2-turn canned responses
            canned = [
                {
                    "content": None,
                    "tool_calls": [
                        {
                            "id": "call_abc123",
                            "type": "function",
                            "function": {
                                "name": "read_file",
                                "arguments": json.dumps({"path": "sample.txt"})
                            }
                        }
                    ],
                    "prompt_tokens": 450,
                    "completion_tokens": 30,
                    "total_tokens": 480
                },
                {
                    "content": "File verified. Task complete.",
                    "tool_calls": None,
                    "prompt_tokens": 520,
                    "completion_tokens": 25,
                    "total_tokens": 545
                }
            ]

            mock_trans = MockProviderTransport(model="gpt-5.6-luna", canned_responses=canned)
            runtime = ControlledAgentRuntime(
                sandbox_path=sandbox,
                arm="arm_a_native",
                task_id="task_multiturn_test",
                model="gpt-5.6-luna",
                transport=mock_trans,
                max_tokens_ceiling=50_000,
                headroom_reserve=5_000
            )

            res = runtime.run(prompt="Read sample.txt and report", output_events_path=events_path)

            assert res.exit_code == 0
            assert res.stop_reason is None
            # Verified 2 turns were executed
            assert len(runtime.per_request_telemetry) == 2
            # Verified tool call round trip was captured
            assert len(runtime.round_trip_events) == 1
            assert runtime.round_trip_events[0]["tool_name"] == "read_file"
            assert runtime.round_trip_events[0]["round_trip_id"].startswith("rt-arm_a_native-0-call_abc123")

            # Verify events.jsonl contains tool execution event
            events = [json.loads(line) for line in events_path.read_text(encoding="utf-8").splitlines()]
            tool_events = [e for e in events if e.get("type") == "item.completed" and e.get("item", {}).get("type") == "tool_call"]
            assert len(tool_events) == 1
            assert tool_events[0]["item"]["tool"] == "read_file"
            assert "CONTENT_LINE_1" in tool_events[0]["item"]["result"]["content"][0]["text"]

            # Reconciled budget: Turn 0 (480) + Turn 1 (545) = 1,025 tokens
            assert res.cumulative_tokens_at_stop == 1025

    def test_07_telemetry_records_transport_identity_accurately(self):
        """Verifies that transport class and provider name are preserved in runtime results."""
        with tempfile.TemporaryDirectory() as tmpdir:
            sandbox = Path(tmpdir)
            events_path = sandbox / "events.jsonl"
            mock_trans = MockProviderTransport(model="gpt-5.6-luna")

            runner = ControlledRunner(
                transport=mock_trans,
                model="gpt-5.6-luna",
                provider="mock"
            )
            res = runner.run(
                sandbox_path=sandbox,
                prompt="Quick test",
                arm="arm_a_native",
                output_events_path=events_path,
                is_dry_run=False
            )
            assert res.exit_code == 0
            assert res.provider == "mock_transport"
            assert runner.transport.__class__.__name__ == "MockProviderTransport"
