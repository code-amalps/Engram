"""
Engram Phase 5.9.2.3: In-Process HTTP Transport Contract Verification Suite
Repository: https://github.com/engram/engram

Verifies:
1. Request payload JSON structure (model, messages, max_completion_tokens, temperature, stream, tools).
2. Authorization header construction (Bearer <token>) & credential sanitization.
3. Model identifier (gpt-5.6-luna) and output token limit (4,096) contract.
4. Valid text completion response parsing (content, finish_reason).
5. Single tool-call response parsing (id, type, function name, arguments).
6. Multiple tool-call response parsing in a single turn.
7. Follow-up tool-result request serialization (role: 'tool', tool_call_id, content).
8. Usage metadata extraction (prompt_tokens, completion_tokens, total_tokens, cached_tokens, reasoning_tokens).
9. HTTP 401 & 403 handling -> AuthenticationError.
10. HTTP 429 handling -> RateLimitError.
11. Provider timeout handling -> ProviderTimeoutError.
12. Malformed JSON response handling -> ProviderTransportError.
13. Output token boundary enforcement (O > R_reserved -> StreamCancelledError).
14. Budget & reservation integrity under shared RequestReservationCoordinator (R = I + O <= B_rem - B_res).
15. Failure preservation (exceptions retain full reservation R without unsafe budget release).

Execution Mode: Strictly Offline, In-Process Local HTTP Test Server (127.0.0.1:0).
Zero External Network Calls. Synthetic Test Credentials Only.
"""

from http.server import BaseHTTPRequestHandler, HTTPServer
import json
from pathlib import Path
import sys
import threading
import time
from typing import Any, Callable, Dict, List, Optional, Tuple
import pytest

HARNESS_ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = HARNESS_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from agent_runtime import ControlledAgentRuntime, WORKSPACE_TOOLS
from provider_transport import (
    AuthenticationError,
    BudgetExceededError,
    InvalidConfigurationError,
    NetworkAccessForbiddenError,
    OpenAICompatibleTransport,
    ProviderRequest,
    ProviderResponse,
    ProviderTimeoutError,
    ProviderTransportError,
    RateLimitError,
    StreamCancelledError,
    create_provider_transport,
    sanitize_text,
)
from runner import RequestReservation, RequestReservationCoordinator


class MockHttpServer(HTTPServer):
    """Local in-process test HTTP server for contract verification."""
    def __init__(self, server_address, RequestHandlerClass):
        super().__init__(server_address, RequestHandlerClass)
        self.received_requests: List[Dict[str, Any]] = []
        self.custom_handler: Optional[Callable[[str, Dict[str, str], bytes], Tuple[int, Dict[str, str], bytes]]] = None
        self.delay_seconds: float = 0.0

    def record_request(self, path: str, headers: Dict[str, str], body: bytes) -> Tuple[int, Dict[str, str], bytes]:
        req_entry = {
            "path": path,
            "headers": dict(headers),
            "body": body,
            "json": None
        }
        try:
            req_entry["json"] = json.loads(body.decode("utf-8"))
        except Exception:
            pass
        self.received_requests.append(req_entry)

        if self.delay_seconds > 0:
            time.sleep(self.delay_seconds)

        if self.custom_handler:
            return self.custom_handler(path, headers, body)

        # Default standard OpenAI completion response
        default_resp = {
            "id": "chatcmpl-test-default",
            "object": "chat.completion",
            "created": int(time.time()),
            "model": "gpt-5.6-luna",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Contract test response completed successfully."
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 120,
                "completion_tokens": 25,
                "total_tokens": 145,
                "prompt_tokens_details": {"cached_tokens": 40},
                "completion_tokens_details": {"reasoning_tokens": 10}
            }
        }
        resp_bytes = json.dumps(default_resp).encode("utf-8")
        return 200, {"Content-Type": "application/json"}, resp_bytes


class MockHttpHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        # Suppress server access logging to preserve clean test output
        pass

    def do_POST(self):
        content_len = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_len)
        server: MockHttpServer = self.server
        status, headers, resp_body = server.record_request(self.path, self.headers, body)
        self.send_response(status)
        for k, v in headers.items():
            self.send_header(k, v)
        self.end_headers()
        if resp_body:
            self.wfile.write(resp_body)


@pytest.fixture
def mock_http_server():
    """Spawns an in-process local HTTP server on 127.0.0.1:0 (ephemeral port)."""
    server = MockHttpServer(("127.0.0.1", 0), MockHttpHandler)
    server_thread = threading.Thread(target=server.serve_forever, daemon=True)
    server_thread.start()
    port = server.server_address[1]
    server.base_url = f"http://127.0.0.1:{port}"
    try:
        yield server
    finally:
        server.shutdown()
        server.server_close()
        server_thread.join(timeout=2.0)


@pytest.fixture
def dummy_reservation():
    """Provides a valid active RequestReservation for contract dispatch."""
    return RequestReservation(
        request_id="req-contract-test",
        turn_index=0,
        input_tokens=500,
        reserved_output_tokens=4096,
        total_reserved_tokens=4596,
        created_at="2026-11-22T00:00:00Z",
        status="ACTIVE"
    )


class TestPhase5923HttpContract:

    def test_01_request_payload_json_structure(self, mock_http_server, dummy_reservation):
        """Verifies full request JSON structure: model, messages, tools, max_completion_tokens, etc."""
        transport = OpenAICompatibleTransport(
            model="gpt-5.6-luna",
            api_base_url=mock_http_server.base_url,
            api_key="sk-test-mock-contract-key-only",
            allow_live_network=True
        )

        tools = [
            {
                "type": "function",
                "function": {
                    "name": "read_file",
                    "description": "Reads file content",
                    "parameters": {
                        "type": "object",
                        "properties": {"path": {"type": "string"}},
                        "required": ["path"]
                    }
                }
            }
        ]

        req = ProviderRequest(
            request_id="req_01",
            turn_index=0,
            task_id="task_contract_test",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Explain project architecture"}],
            tools=tools,
            max_completion_tokens=4096,
            temperature=0.0,
            stream=False
        )

        resp = transport.dispatch(req, dummy_reservation)
        assert resp is not None
        assert len(mock_http_server.received_requests) == 1

        wire = mock_http_server.received_requests[0]
        assert wire["path"] == "/chat/completions"
        assert wire["json"]["model"] == "gpt-5.6-luna"
        assert wire["json"]["max_completion_tokens"] == 4096
        assert wire["json"]["temperature"] == 0.0
        assert wire["json"]["stream"] is False
        assert len(wire["json"]["messages"]) == 1
        assert wire["json"]["messages"][0]["role"] == "user"
        assert wire["json"]["messages"][0]["content"] == "Explain project architecture"
        assert len(wire["json"]["tools"]) == 1
        assert wire["json"]["tools"][0]["function"]["name"] == "read_file"

    def test_02_authorization_header_and_redaction(self, mock_http_server, dummy_reservation):
        """Verifies Authorization: Bearer header is sent and credentials remain redacted from logs/repr."""
        test_key = "sk-test-mock-contract-key-only"
        transport = OpenAICompatibleTransport(
            model="gpt-5.6-luna",
            api_base_url=mock_http_server.base_url,
            api_key=test_key,
            allow_live_network=True
        )

        req = ProviderRequest(
            request_id="req_02",
            turn_index=0,
            task_id="task_contract_test",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Ping"}]
        )

        resp = transport.dispatch(req, dummy_reservation)
        assert resp is not None

        wire = mock_http_server.received_requests[0]
        auth_header = wire["headers"].get("Authorization")
        assert auth_header == f"Bearer {test_key}"

        # Credential sanitization check: key must never appear in repr or str
        assert test_key not in repr(transport)
        assert test_key not in str(transport)
        assert "api_key" not in repr(transport)

    def test_03_model_identifier_and_output_limit(self, mock_http_server, dummy_reservation):
        """Verifies model identifier gpt-5.6-luna and output token limit contract."""
        transport = OpenAICompatibleTransport(
            model="gpt-5.6-luna",
            api_base_url=mock_http_server.base_url,
            api_key="sk-test-mock-contract-key-only",
            allow_live_network=True
        )

        req = ProviderRequest(
            request_id="req_03",
            turn_index=0,
            task_id="task_contract_test",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Verify limits"}],
            max_completion_tokens=4096
        )

        transport.dispatch(req, dummy_reservation)
        wire_json = mock_http_server.received_requests[0]["json"]
        assert wire_json["model"] == "gpt-5.6-luna"
        assert wire_json["max_completion_tokens"] == 4096

    def test_04_valid_text_completion_response_parsing(self, mock_http_server, dummy_reservation):
        """Verifies parsing of standard assistant text response."""
        def handle_custom(path, headers, body):
            data = {
                "id": "chatcmpl-text-01",
                "object": "chat.completion",
                "model": "gpt-5.6-luna",
                "choices": [
                    {
                        "index": 0,
                        "message": {"role": "assistant", "content": "Analysis completed successfully."},
                        "finish_reason": "stop"
                    }
                ],
                "usage": {"prompt_tokens": 300, "completion_tokens": 40, "total_tokens": 340}
            }
            return 200, {"Content-Type": "application/json"}, json.dumps(data).encode("utf-8")

        mock_http_server.custom_handler = handle_custom
        transport = OpenAICompatibleTransport(
            model="gpt-5.6-luna",
            api_base_url=mock_http_server.base_url,
            api_key="sk-test-mock-contract-key-only",
            allow_live_network=True
        )

        req = ProviderRequest(
            request_id="req_04",
            turn_index=0,
            task_id="task_contract_test",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Analyze"}]
        )

        resp = transport.dispatch(req, dummy_reservation)
        assert resp.content == "Analysis completed successfully."
        assert resp.tool_calls is None
        assert resp.finish_reason == "stop"
        assert resp.prompt_tokens == 300
        assert resp.completion_tokens == 40
        assert resp.total_tokens == 340

    def test_05_single_tool_call_response_parsing(self, mock_http_server, dummy_reservation):
        """Verifies parsing of response with single tool_call."""
        def handle_tool(path, headers, body):
            data = {
                "id": "chatcmpl-tc-01",
                "object": "chat.completion",
                "model": "gpt-5.6-luna",
                "choices": [
                    {
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": None,
                            "tool_calls": [
                                {
                                    "id": "call_read_01",
                                    "type": "function",
                                    "function": {
                                        "name": "read_file",
                                        "arguments": json.dumps({"path": "src/main.rs"})
                                    }
                                }
                            ]
                        },
                        "finish_reason": "tool_calls"
                    }
                ],
                "usage": {"prompt_tokens": 250, "completion_tokens": 35, "total_tokens": 285}
            }
            return 200, {"Content-Type": "application/json"}, json.dumps(data).encode("utf-8")

        mock_http_server.custom_handler = handle_tool
        transport = OpenAICompatibleTransport(
            model="gpt-5.6-luna",
            api_base_url=mock_http_server.base_url,
            api_key="sk-test-mock-contract-key-only",
            allow_live_network=True
        )

        req = ProviderRequest(
            request_id="req_05",
            turn_index=0,
            task_id="task_contract_test",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Inspect main.rs"}]
        )

        resp = transport.dispatch(req, dummy_reservation)
        assert resp.content is None
        assert resp.finish_reason == "tool_calls"
        assert resp.tool_calls is not None
        assert len(resp.tool_calls) == 1
        assert resp.tool_calls[0]["id"] == "call_read_01"
        assert resp.tool_calls[0]["function"]["name"] == "read_file"
        args = json.loads(resp.tool_calls[0]["function"]["arguments"])
        assert args["path"] == "src/main.rs"

    def test_06_multiple_tool_calls_response_parsing(self, mock_http_server, dummy_reservation):
        """Verifies parsing of response with multiple tool_calls in a single turn."""
        def handle_multi_tool(path, headers, body):
            data = {
                "id": "chatcmpl-multi-tc",
                "object": "chat.completion",
                "model": "gpt-5.6-luna",
                "choices": [
                    {
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": None,
                            "tool_calls": [
                                {
                                    "id": "call_multi_01",
                                    "type": "function",
                                    "function": {
                                        "name": "read_file",
                                        "arguments": json.dumps({"path": "Cargo.toml"})
                                    }
                                },
                                {
                                    "id": "call_multi_02",
                                    "type": "function",
                                    "function": {
                                        "name": "list_dir",
                                        "arguments": json.dumps({"path": "src"})
                                    }
                                }
                            ]
                        },
                        "finish_reason": "tool_calls"
                    }
                ],
                "usage": {"prompt_tokens": 400, "completion_tokens": 60, "total_tokens": 460}
            }
            return 200, {"Content-Type": "application/json"}, json.dumps(data).encode("utf-8")

        mock_http_server.custom_handler = handle_multi_tool
        transport = OpenAICompatibleTransport(
            model="gpt-5.6-luna",
            api_base_url=mock_http_server.base_url,
            api_key="sk-test-mock-contract-key-only",
            allow_live_network=True
        )

        req = ProviderRequest(
            request_id="req_06",
            turn_index=0,
            task_id="task_contract_test",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Check project files"}]
        )

        resp = transport.dispatch(req, dummy_reservation)
        assert resp.finish_reason == "tool_calls"
        assert resp.tool_calls is not None
        assert len(resp.tool_calls) == 2
        assert resp.tool_calls[0]["id"] == "call_multi_01"
        assert resp.tool_calls[0]["function"]["name"] == "read_file"
        assert resp.tool_calls[1]["id"] == "call_multi_02"
        assert resp.tool_calls[1]["function"]["name"] == "list_dir"

    def test_07_follow_up_tool_result_request_serialization(self, mock_http_server, dummy_reservation):
        """Verifies subsequent request correctly serializes role: 'tool' and tool_call_id."""
        transport = OpenAICompatibleTransport(
            model="gpt-5.6-luna",
            api_base_url=mock_http_server.base_url,
            api_key="sk-test-mock-contract-key-only",
            allow_live_network=True
        )

        messages = [
            {"role": "user", "content": "Read Cargo.toml"},
            {
                "role": "assistant",
                "content": None,
                "tool_calls": [
                    {
                        "id": "call_cargo_01",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": json.dumps({"path": "Cargo.toml"})
                        }
                    }
                ]
            },
            {
                "role": "tool",
                "tool_call_id": "call_cargo_01",
                "content": "[package]\nname = 'engram'\nversion = '0.5.9'"
            }
        ]

        req = ProviderRequest(
            request_id="req_07",
            turn_index=1,
            task_id="task_contract_test",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=messages
        )

        transport.dispatch(req, dummy_reservation)
        wire_messages = mock_http_server.received_requests[0]["json"]["messages"]
        assert len(wire_messages) == 3
        tool_msg = wire_messages[2]
        assert tool_msg["role"] == "tool"
        assert tool_msg["tool_call_id"] == "call_cargo_01"
        assert "version = '0.5.9'" in tool_msg["content"]

    def test_08_usage_metadata_extraction_including_cached_and_reasoning(self, mock_http_server, dummy_reservation):
        """Verifies accurate extraction of usage breakdown: cached prompt tokens and reasoning tokens."""
        def handle_usage(path, headers, body):
            data = {
                "id": "chatcmpl-usage-01",
                "object": "chat.completion",
                "model": "gpt-5.6-luna",
                "choices": [
                    {"index": 0, "message": {"role": "assistant", "content": "Summary done"}, "finish_reason": "stop"}
                ],
                "usage": {
                    "prompt_tokens": 1500,
                    "completion_tokens": 200,
                    "total_tokens": 1700,
                    "prompt_tokens_details": {
                        "cached_tokens": 1200
                    },
                    "completion_tokens_details": {
                        "reasoning_tokens": 150
                    }
                }
            }
            return 200, {"Content-Type": "application/json"}, json.dumps(data).encode("utf-8")

        mock_http_server.custom_handler = handle_usage
        transport = OpenAICompatibleTransport(
            model="gpt-5.6-luna",
            api_base_url=mock_http_server.base_url,
            api_key="sk-test-mock-contract-key-only",
            allow_live_network=True
        )

        req = ProviderRequest(
            request_id="req_08",
            turn_index=0,
            task_id="task_contract_test",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Provide detailed summary"}]
        )

        resp = transport.dispatch(req, dummy_reservation)
        assert resp.prompt_tokens == 1500
        assert resp.completion_tokens == 200
        assert resp.total_tokens == 1700
        assert resp.cached_input_tokens == 1200
        assert resp.reasoning_output_tokens == 150

    def test_09_http_401_and_403_authentication_errors(self, mock_http_server, dummy_reservation):
        """Verifies HTTP 401 Unauthorized and 403 Forbidden map to AuthenticationError without leaking keys."""
        transport = OpenAICompatibleTransport(
            model="gpt-5.6-luna",
            api_base_url=mock_http_server.base_url,
            api_key="sk-test-mock-contract-key-only",
            allow_live_network=True
        )

        req = ProviderRequest(
            request_id="req_09",
            turn_index=0,
            task_id="task_contract_test",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Auth test"}]
        )

        # Test HTTP 401
        mock_http_server.custom_handler = lambda p, h, b: (
            401,
            {"Content-Type": "application/json"},
            b'{"error": {"message": "Invalid authentication token", "type": "invalid_request_error"}}'
        )
        with pytest.raises(AuthenticationError) as exc_401:
            transport.dispatch(req, dummy_reservation)
        assert "HTTP 401" in str(exc_401.value)
        assert "Invalid authentication token" in str(exc_401.value)

        # Test HTTP 403
        mock_http_server.custom_handler = lambda p, h, b: (
            403,
            {"Content-Type": "application/json"},
            b'{"error": {"message": "Forbidden access to model resource", "type": "forbidden"}}'
        )
        with pytest.raises(AuthenticationError) as exc_403:
            transport.dispatch(req, dummy_reservation)
        assert "HTTP 403" in str(exc_403.value)
        assert "Forbidden access to model resource" in str(exc_403.value)

    def test_10_http_429_rate_limit_error(self, mock_http_server, dummy_reservation):
        """Verifies HTTP 429 Too Many Requests maps to RateLimitError."""
        mock_http_server.custom_handler = lambda p, h, b: (
            429,
            {"Content-Type": "application/json"},
            b'{"error": {"message": "Rate limit exceeded: TPM quota reached", "type": "tokens"}}'
        )

        transport = OpenAICompatibleTransport(
            model="gpt-5.6-luna",
            api_base_url=mock_http_server.base_url,
            api_key="sk-test-mock-contract-key-only",
            allow_live_network=True
        )

        req = ProviderRequest(
            request_id="req_10",
            turn_index=0,
            task_id="task_contract_test",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Heavy throughput request"}]
        )

        with pytest.raises(RateLimitError) as exc_429:
            transport.dispatch(req, dummy_reservation)
        assert "HTTP 429" in str(exc_429.value)
        assert "TPM quota reached" in str(exc_429.value)

    def test_11_timeout_handling(self, mock_http_server, dummy_reservation):
        """Verifies provider connection timeout maps to ProviderTimeoutError."""
        mock_http_server.delay_seconds = 0.5  # Delay longer than transport timeout

        transport = OpenAICompatibleTransport(
            model="gpt-5.6-luna",
            api_base_url=mock_http_server.base_url,
            api_key="sk-test-mock-contract-key-only",
            timeout_seconds=0.1,  # 100ms timeout
            allow_live_network=True
        )

        req = ProviderRequest(
            request_id="req_11",
            turn_index=0,
            task_id="task_contract_test",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Slow request"}]
        )

        with pytest.raises(ProviderTimeoutError) as exc_timeout:
            transport.dispatch(req, dummy_reservation)
        assert "timed out after 0.1s" in str(exc_timeout.value) or "timeout" in str(exc_timeout.value).lower()

    def test_12_malformed_json_response_handling(self, mock_http_server, dummy_reservation):
        """Verifies non-JSON or corrupted response maps to ProviderTransportError."""
        mock_http_server.custom_handler = lambda p, h, b: (
            200,
            {"Content-Type": "text/html"},
            b"<html><body>502 Bad Gateway - Upstream Load Balancer Error</body></html>"
        )

        transport = OpenAICompatibleTransport(
            model="gpt-5.6-luna",
            api_base_url=mock_http_server.base_url,
            api_key="sk-test-mock-contract-key-only",
            allow_live_network=True
        )

        req = ProviderRequest(
            request_id="req_12",
            turn_index=0,
            task_id="task_contract_test",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Corrupted response test"}]
        )

        with pytest.raises(ProviderTransportError) as exc_malformed:
            transport.dispatch(req, dummy_reservation)
        assert "Invalid JSON response" in str(exc_malformed.value)

    def test_13_output_token_boundary_enforcement(self, mock_http_server):
        """Verifies output token boundary check: completion_tokens > reserved_output raises StreamCancelledError."""
        def handle_huge_output(path, headers, body):
            data = {
                "id": "chatcmpl-huge",
                "object": "chat.completion",
                "model": "gpt-5.6-luna",
                "choices": [
                    {"index": 0, "message": {"role": "assistant", "content": "Very long output"}, "finish_reason": "length"}
                ],
                "usage": {"prompt_tokens": 100, "completion_tokens": 500, "total_tokens": 600}
            }
            return 200, {"Content-Type": "application/json"}, json.dumps(data).encode("utf-8")

        mock_http_server.custom_handler = handle_huge_output
        transport = OpenAICompatibleTransport(
            model="gpt-5.6-luna",
            api_base_url=mock_http_server.base_url,
            api_key="sk-test-mock-contract-key-only",
            allow_live_network=True
        )

        # Reservation explicitly capped at 200 output tokens
        small_out_res = RequestReservation(
            request_id="req_13",
            turn_index=0,
            input_tokens=100,
            reserved_output_tokens=200,
            total_reserved_tokens=300,
            created_at="2026-11-22T00:00:00Z",
            status="ACTIVE"
        )

        req = ProviderRequest(
            request_id="req_13",
            turn_index=0,
            task_id="task_contract_test",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Generate verbose output"}]
        )

        with pytest.raises(StreamCancelledError) as exc_boundary:
            transport.dispatch(req, small_out_res)
        assert "Output tokens exceeded reservation (500 > 200)" in str(exc_boundary.value)

    def test_14_budget_coordinator_reservation_integrity(self):
        """
        Verifies RequestReservationCoordinator enforces R = I + O <= B_rem - B_res.
        With B_total = 1,000,000, B_res = 45,000, max dispatchable capacity is 955,000 tokens.
        """
        coord = RequestReservationCoordinator(aggregate_budget=1_000_000, safety_reserve=45_000)
        assert coord.remaining_budget == 1_000_000
        assert coord.in_flight_reserved_tokens == 0
        assert coord.total_committed_tokens == 0

        # Attempt to reserve 955,001 tokens (exceeds headroom above safety reserve)
        ok, err, res = coord.reserve_request(
            request_id="req_overshoot",
            turn_index=0,
            input_tokens=950_905,
            completion_allowance=4096  # total = 955,001
        )
        assert not ok
        assert res is None
        assert "Request budget reservation rejected" in err
        assert "Deficit: 1 tokens" in err
        assert len(coord.rejection_history) == 1

        # Attempt to reserve exactly 955,000 tokens (valid dispatchable maximum)
        ok_valid, err_valid, res_valid = coord.reserve_request(
            request_id="req_valid",
            turn_index=0,
            input_tokens=950_904,
            completion_allowance=4096  # total = 955,000
        )
        assert ok_valid
        assert err_valid is None
        assert res_valid is not None
        assert res_valid.total_reserved_tokens == 955_000
        assert coord.in_flight_reserved_tokens == 955_000
        assert coord.remaining_budget == 45_000  # Exactly the protected safety reserve!

    def test_15_failure_retains_full_reservation_without_leakage(self, mock_http_server):
        """
        Verifies that transport failures (timeout, 401, 429, malformed response)
        conservatively retain the full reserved capacity R without unsafe budget release.
        """
        coord = RequestReservationCoordinator(aggregate_budget=100_000, safety_reserve=10_000)

        # Turn 0: Reserve 5,000 tokens (4,000 input + 1,000 output)
        ok, _, res = coord.reserve_request(
            request_id="req_fail_01",
            turn_index=0,
            input_tokens=4_000,
            completion_allowance=1_000
        )
        assert ok
        assert coord.in_flight_reserved_tokens == 5_000

        # Simulate transport failure where actual usage is None
        reconciled = coord.reconcile_response("req_fail_01", actual_total_tokens=None)
        assert reconciled.status == "RECONCILED"
        # Reconciled usage must equal the full reserved capacity R = 5,000
        assert reconciled.actual_total_tokens == 5_000
        assert coord.cumulative_reconciled_tokens == 5_000
        assert coord.in_flight_reserved_tokens == 0
        assert coord.remaining_budget == 95_000
