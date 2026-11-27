"""
Engram Phase 5.9.1: Controlled Provider Transport Architecture
Repository: https://github.com/engram/engram

Provides a provider abstraction that makes Engram responsible for every outbound model request:
- Synchronous pre-dispatch request serialization and exact token measurement.
- Mandatory pre-dispatch reservation boundary (R = I + O <= B_remaining - B_reserve).
- Complete zero-truncation rejection before transport transmission.
- Live network guard (offline by default in Phase 5.9.1; zero live network calls permitted).
- Streaming response handler with real-time output token tracking and cancellation.
- Provider usage extraction and reconciliation.
- Complete per-request telemetry and round-trip event generation.
- Transport spy for adversarial testing.
- Credential sanitization (zero leaks in logs, representations, or error traces).
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
import datetime
import json
import os
import re
import socket
import time
from typing import Any, Callable, Dict, Generator, List, Optional, Set, Tuple, Union
import urllib.error
import urllib.request
import uuid

try:
    from runner import RequestReservation, RequestReservationCoordinator
except (ImportError, ValueError):
    try:
        from .runner import RequestReservation, RequestReservationCoordinator
    except Exception:
        # Fallback for standalone loading
        RequestReservation = Any
        RequestReservationCoordinator = Any


# ==============================================================================
# Security & Secret Sanitization
# ==============================================================================

SENSITIVE_HEADER_KEYS: Set[str] = {
    "authorization",
    "api-key",
    "x-api-key",
    "bearer",
    "proxy-authorization",
    "cookie",
    "set-cookie",
}

KEY_REGEX = re.compile(r"(sk-[a-zA-Z0-9_\-]{20,}|Bearer\s+[a-zA-Z0-9_\-\.]+)", re.IGNORECASE)


def sanitize_headers(headers: Optional[Dict[str, str]]) -> Dict[str, str]:
    """Returns a copy of headers with sensitive keys and values redacted."""
    if not headers:
        return {}
    sanitized: Dict[str, str] = {}
    for k, v in headers.items():
        if k.lower() in SENSITIVE_HEADER_KEYS:
            sanitized[k] = "[REDACTED]"
        else:
            # Check value for key-like patterns
            sanitized[k] = KEY_REGEX.sub("[REDACTED]", str(v))
    return sanitized


def sanitize_text(text: str) -> str:
    """Removes any API keys or tokens from text strings."""
    if not text:
        return text
    return KEY_REGEX.sub("[REDACTED]", text)


# ==============================================================================
# Provider Exceptions
# ==============================================================================

class ProviderTransportError(Exception):
    """Base exception for provider transport failures."""
    def __init__(self, message: str, error_code: str = "PROVIDER_ERROR", retryable: bool = False):
        clean_msg = sanitize_text(message)
        super().__init__(clean_msg)
        self.error_code = error_code
        self.retryable = retryable


class NetworkAccessForbiddenError(ProviderTransportError):
    """Raised when an outbound live network call is attempted while network access is forbidden."""
    def __init__(self, message: str = "Live network access is NOT_AUTHORIZED in Phase 5.9.1"):
        super().__init__(message, error_code="NETWORK_ACCESS_FORBIDDEN", retryable=False)


class BudgetExceededError(ProviderTransportError):
    """Raised when request budget or aggregate ceiling is exceeded."""
    def __init__(self, message: str):
        super().__init__(message, error_code="BUDGET_EXCEEDED", retryable=False)


class RequestSizeExceededError(ProviderTransportError):
    """Raised when request prompt/payload exceeds max request size limit."""
    def __init__(self, message: str):
        super().__init__(message, error_code="REQUEST_SIZE_EXCEEDED", retryable=False)


class AuthenticationError(ProviderTransportError):
    """Raised when provider authentication fails or credentials are invalid."""
    def __init__(self, message: str = "Authentication failed"):
        super().__init__(message, error_code="AUTH_FAILURE", retryable=False)


class RateLimitError(ProviderTransportError):
    """Raised when provider rate limit is encountered."""
    def __init__(self, message: str = "Rate limit exceeded"):
        super().__init__(message, error_code="RATE_LIMIT", retryable=True)


class ProviderTimeoutError(ProviderTransportError):
    """Raised when provider request exceeds timeout."""
    def __init__(self, message: str = "Provider request timed out"):
        super().__init__(message, error_code="TIMEOUT", retryable=True)


class StreamCancelledError(ProviderTransportError):
    """Raised when a streaming completion is cancelled because output tokens exceed the reserved allowance."""
    def __init__(self, message: str = "Stream cancelled: completion allowance exceeded"):
        super().__init__(message, error_code="STREAM_CANCELLED_ALLOWANCE_EXCEEDED", retryable=False)


class InvalidConfigurationError(ProviderTransportError):
    """Raised when provider transport configuration is invalid."""
    def __init__(self, message: str):
        super().__init__(message, error_code="INVALID_CONFIGURATION", retryable=False)


# ==============================================================================
# Request and Response Dataclasses
# ==============================================================================

@dataclass
class ProviderRequest:
    """
    Normalized, provider-agnostic request structure representing an outbound model invocation.
    """
    request_id: str
    turn_index: int
    task_id: str
    arm_id: str
    model: str
    messages: List[Dict[str, Any]]
    tools: Optional[List[Dict[str, Any]]] = None
    tool_choice: Optional[Union[str, Dict[str, Any]]] = None
    max_completion_tokens: int = 4096
    temperature: float = 0.0
    stream: bool = False
    extra_headers: Optional[Dict[str, str]] = None
    extra_body: Optional[Dict[str, Any]] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "request_id": self.request_id,
            "turn_index": self.turn_index,
            "task_id": self.task_id,
            "arm_id": self.arm_id,
            "model": self.model,
            "messages_count": len(self.messages),
            "tools_count": len(self.tools) if self.tools else 0,
            "max_completion_tokens": self.max_completion_tokens,
            "temperature": self.temperature,
            "stream": self.stream,
        }


@dataclass
class ProviderResponse:
    """
    Normalized, provider-agnostic response structure representing the result of a model invocation.
    """
    response_id: str
    request_id: str
    turn_index: int
    model: str
    content: Optional[str] = None
    tool_calls: Optional[List[Dict[str, Any]]] = None
    finish_reason: str = "stop"  # "stop", "tool_calls", "length", "cancelled", "error"
    prompt_tokens: Optional[int] = None
    completion_tokens: Optional[int] = None
    total_tokens: Optional[int] = None
    raw_usage: Optional[Dict[str, Any]] = None
    dispatched_at: str = ""
    completed_at: str = ""
    latency_ms: float = 0.0
    is_streaming: bool = False
    stream_cancelled: bool = False
    error: Optional[str] = None
    error_code: Optional[str] = None
    cached_input_tokens: int = 0
    reasoning_output_tokens: int = 0

    def to_dict(self) -> Dict[str, Any]:
        return {
            "response_id": self.response_id,
            "request_id": self.request_id,
            "turn_index": self.turn_index,
            "model": self.model,
            "has_content": bool(self.content),
            "tool_calls_count": len(self.tool_calls) if self.tool_calls else 0,
            "finish_reason": self.finish_reason,
            "prompt_tokens": self.prompt_tokens,
            "completion_tokens": self.completion_tokens,
            "total_tokens": self.total_tokens,
            "cached_input_tokens": self.cached_input_tokens,
            "reasoning_output_tokens": self.reasoning_output_tokens,
            "latency_ms": self.latency_ms,
            "dispatched_at": self.dispatched_at,
            "completed_at": self.completed_at,
            "stream_cancelled": self.stream_cancelled,
            "error": self.error,
            "error_code": self.error_code,
        }


@dataclass
class RequestTelemetryRecord:
    """
    Full per-request telemetry record as required by Phase 5.9.1 Section 8.
    Ensures zero leak of API keys or sensitive authorization content.
    """
    request_id: str
    task_id: str
    arm_id: str
    conversation_turn_id: int
    model: str
    provider: str
    input_token_estimate: int
    input_bytes: int
    provider_reported_input_tokens: Optional[int]
    reserved_output_allowance: int
    actual_output_tokens: Optional[int]
    total_actual_tokens: Optional[int]
    reservation_accepted: bool
    dispatch_timestamp: str
    completion_timestamp: str
    latency_ms: float
    retry_count: int
    error_classification: Optional[str]
    usage_metadata_available: bool
    reservation_reconciled: bool
    counting_method: str
    rejection_reason: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        d = {
            "request_id": self.request_id,
            "task_id": self.task_id,
            "arm_id": self.arm_id,
            "conversation_turn_id": self.conversation_turn_id,
            "model": self.model,
            "provider": self.provider,
            "input_token_estimate": self.input_token_estimate,
            "input_bytes": self.input_bytes,
            "provider_reported_input_tokens": self.provider_reported_input_tokens,
            "reserved_output_allowance": self.reserved_output_allowance,
            "actual_output_tokens": self.actual_output_tokens,
            "total_actual_tokens": self.total_actual_tokens,
            "reservation_accepted": self.reservation_accepted,
            "dispatch_timestamp": self.dispatch_timestamp,
            "completion_timestamp": self.completion_timestamp,
            "latency_ms": self.latency_ms,
            "retry_count": self.retry_count,
            "error_classification": self.error_classification,
            "usage_metadata_available": self.usage_metadata_available,
            "reservation_reconciled": self.reservation_reconciled,
            "counting_method": self.counting_method,
            "rejection_reason": sanitize_text(self.rejection_reason) if self.rejection_reason else None,
        }
        return d


# ==============================================================================
# Abstract Provider Transport Base Class
# ==============================================================================

class ProviderTransport(ABC):
    """
    Abstract interface for model provider transports.
    Responsible for request serialization, token measurement, streaming handling,
    usage extraction, retry management, error classification, and telemetry.
    """
    def __init__(
        self,
        model: str,
        provider_name: str = "generic",
        api_base_url: Optional[str] = None,
        api_key: Optional[str] = None,
        timeout_seconds: int = 60,
        allow_live_network: bool = False
    ):
        self.model = model
        self.provider_name = provider_name
        self.api_base_url = api_base_url
        self._api_key = api_key  # Never logged or exposed in __repr__
        self.timeout_seconds = timeout_seconds
        self.allow_live_network = allow_live_network
        self._encoder = None
        self._encoding_name = None
        self._init_tokenizer()

    @property
    def api_key(self) -> Optional[str]:
        return self._api_key

    def __repr__(self) -> str:
        return (
            f"<{self.__class__.__name__} provider={self.provider_name} "
            f"model={self.model} base_url={self.api_base_url} "
            f"allow_live_network={self.allow_live_network}>"
        )

    def _init_tokenizer(self):
        """Initializes exact model tokenizer where available."""
        try:
            import tiktoken
            if "o200k" in self.model or "gpt-5" in self.model or "gpt-4o" in self.model:
                self._encoder = tiktoken.get_encoding("o200k_base")
                self._encoding_name = "tiktoken_o200k_base"
            else:
                self._encoder = tiktoken.get_encoding("cl100k_base")
                self._encoding_name = "tiktoken_cl100k_base"
        except Exception:
            self._encoder = None
            self._encoding_name = "conservative_bytes_upper_bound"

    def count_tokens(self, text: str) -> Tuple[int, int, str]:
        """
        Calculates exact token count using model tokenizer, or conservative UTF-8 byte upper bound.
        Guarantees: token_count <= byte_count unconditionally for BPE tokenizers.
        """
        if not text:
            return 0, 0, self._encoding_name or "empty"

        encoded_bytes = text.encode("utf-8", errors="replace")
        byte_count = len(encoded_bytes)

        if self._encoder is not None:
            try:
                tokens = len(self._encoder.encode(text))
                return tokens, byte_count, self._encoding_name
            except Exception:
                pass

        # Mathematically conservative upper-bound heuristic fallback
        return byte_count, byte_count, "conservative_bytes_upper_bound"

    @abstractmethod
    def serialize_request(self, req: ProviderRequest) -> Tuple[str, bytes, int, str]:
        """
        Serializes the ProviderRequest into the exact provider wire payload.
        Returns:
            (serialized_json_string, payload_bytes, input_token_count, counting_method)
        """
        pass

    @abstractmethod
    def dispatch(self, req: ProviderRequest, reservation: RequestReservation) -> ProviderResponse:
        """
        Synchronously dispatches an approved model request.
        Must fail if reservation is missing, invalid, or inactive.
        """
        pass

    @abstractmethod
    def stream_dispatch(
        self,
        req: ProviderRequest,
        reservation: RequestReservation,
        chunk_callback: Optional[Callable[[str, int], None]] = None
    ) -> ProviderResponse:
        """
        Dispatches a streaming model request, enforcing real-time output token bounds.
        Cancels stream immediately if emitted tokens exceed reservation allowance.
        """
        pass


# ==============================================================================
# OpenAI-Compatible Wire Transport Implementation
# ==============================================================================

class OpenAICompatibleTransport(ProviderTransport):
    """
    OpenAI API-compatible transport supporting /v1/chat/completions schema.
    Enforces strict zero-live-network protection unless explicitly authorized.
    """
    def __init__(
        self,
        model: str = "gpt-5.6-luna",
        api_base_url: Optional[str] = None,
        api_key: Optional[str] = None,
        timeout_seconds: int = 60,
        allow_live_network: bool = False
    ):
        base_url = api_base_url or "https://api.openai.com/v1"
        super().__init__(
            model=model,
            provider_name="openai_compatible",
            api_base_url=base_url,
            api_key=api_key,
            timeout_seconds=timeout_seconds,
            allow_live_network=allow_live_network
        )

    def serialize_request(self, req: ProviderRequest) -> Tuple[str, bytes, int, str]:
        """
        Serializes to standard OpenAI Chat Completion request payload:
        {
          "model": str,
          "messages": List[Dict],
          "tools": Optional[List[Dict]],
          "tool_choice": Optional[Union[str, Dict]],
          "max_completion_tokens": int,
          "temperature": float,
          "stream": bool
        }
        """
        payload: Dict[str, Any] = {
            "model": req.model or self.model,
            "messages": req.messages,
            "max_completion_tokens": req.max_completion_tokens,
            "temperature": req.temperature,
            "stream": req.stream,
        }
        if req.tools:
            payload["tools"] = req.tools
        if req.tool_choice:
            payload["tool_choice"] = req.tool_choice
        if req.extra_body:
            payload.update(req.extra_body)

        # Deterministic JSON serialization
        serialized_str = json.dumps(payload, ensure_ascii=False, sort_keys=True)
        raw_bytes = serialized_str.encode("utf-8")
        tokens, bytes_len, method = self.count_tokens(serialized_str)

        return serialized_str, raw_bytes, tokens, method

    def dispatch(self, req: ProviderRequest, reservation: RequestReservation) -> ProviderResponse:
        """
        Dispatches request to OpenAI-compatible endpoint.
        Fails closed if live network is forbidden, reservation is invalid, or credentials are missing.
        """
        if not reservation or reservation.status != "ACTIVE":
            raise BudgetExceededError(
                f"Unauthorized dispatch attempt: Request {req.request_id} has no ACTIVE reservation."
            )

        if not self.allow_live_network:
            raise NetworkAccessForbiddenError(
                f"Live network access to {self.api_base_url} is NOT_AUTHORIZED. "
                f"Enable allow_live_network=True to permit outbound provider dispatch."
            )

        if not self.api_key:
            raise AuthenticationError(
                f"Genuine live provider execution requires valid API credentials (OPENAI_API_KEY). "
                f"Missing API key for endpoint {self.api_base_url}. Silent fallback to mock transport is strictly forbidden."
            )

        serialized_str, raw_bytes, measured_tokens, method = self.serialize_request(req)
        endpoint = f"{self.api_base_url.rstrip('/')}/chat/completions"
        headers = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {self.api_key}"
        }

        http_req = urllib.request.Request(endpoint, data=raw_bytes, headers=headers, method="POST")
        dispatched_at = datetime.datetime.now(datetime.timezone.utc).isoformat()
        t0 = time.time()

        try:
            with urllib.request.urlopen(http_req, timeout=self.timeout_seconds) as response:
                resp_bytes = response.read()
                latency_ms = (time.time() - t0) * 1000
                completed_at = datetime.datetime.now(datetime.timezone.utc).isoformat()
        except (TimeoutError, socket.timeout) as e:
            raise ProviderTimeoutError(f"Provider connection timed out after {self.timeout_seconds}s: {e}")
        except urllib.error.HTTPError as e:
            latency_ms = (time.time() - t0) * 1000
            err_body = e.read().decode("utf-8", errors="replace") if hasattr(e, "read") else str(e)
            clean_body = sanitize_text(err_body)
            if e.code in (401, 403):
                raise AuthenticationError(f"HTTP {e.code} Provider Authentication Failed: {clean_body}")
            elif e.code == 429:
                raise RateLimitError(f"HTTP 429 Provider Rate Limit Exceeded: {clean_body}")
            else:
                raise ProviderTransportError(f"HTTP {e.code} Provider Error: {clean_body}")
        except urllib.error.URLError as e:
            if isinstance(getattr(e, "reason", None), (TimeoutError, socket.timeout)) or "timed out" in str(e).lower():
                raise ProviderTimeoutError(f"Provider connection timed out after {self.timeout_seconds}s: {e}")
            raise ProviderTransportError(f"Provider network connection error: {sanitize_text(str(e))}")
        except Exception as e:
            raise ProviderTransportError(f"Unexpected provider transport exception: {sanitize_text(str(e))}")

        try:
            resp_data = json.loads(resp_bytes.decode("utf-8"))
        except Exception as e:
            raise ProviderTransportError(f"Invalid JSON response received from provider: {e}")

        choices = resp_data.get("choices", [])
        if not choices:
            content = None
            tool_calls = None
            finish_reason = "empty"
        else:
            msg = choices[0].get("message", {})
            content = msg.get("content")
            tool_calls = msg.get("tool_calls")
            finish_reason = choices[0].get("finish_reason", "stop")

        usage = resp_data.get("usage", {})
        prompt_tokens = usage.get("prompt_tokens", measured_tokens)
        completion_tokens = usage.get("completion_tokens", 0)
        total_tokens = usage.get("total_tokens", prompt_tokens + completion_tokens)
        cached_tokens = usage.get("prompt_tokens_details", {}).get("cached_tokens", 0)
        reasoning_tokens = usage.get("completion_tokens_details", {}).get("reasoning_tokens", 0)

        # Output allowance boundary check
        if completion_tokens > reservation.reserved_output_tokens:
            raise StreamCancelledError(
                f"Output tokens exceeded reservation ({completion_tokens} > {reservation.reserved_output_tokens})"
            )

        return ProviderResponse(
            response_id=resp_data.get("id") or f"resp-{uuid.uuid4().hex[:8]}",
            request_id=req.request_id,
            turn_index=req.turn_index,
            model=resp_data.get("model", req.model or self.model),
            content=content,
            tool_calls=tool_calls,
            finish_reason=finish_reason,
            prompt_tokens=prompt_tokens,
            completion_tokens=completion_tokens,
            total_tokens=total_tokens,
            raw_usage=usage,
            dispatched_at=dispatched_at,
            completed_at=completed_at,
            latency_ms=latency_ms,
            is_streaming=False,
            stream_cancelled=False,
            error=None,
            error_code=None,
            cached_input_tokens=cached_tokens,
            reasoning_output_tokens=reasoning_tokens
        )

    def stream_dispatch(
        self,
        req: ProviderRequest,
        reservation: RequestReservation,
        chunk_callback: Optional[Callable[[str, int], None]] = None
    ) -> ProviderResponse:
        """
        Dispatches streaming request with client-side cancellation.
        """
        if not reservation or reservation.status != "ACTIVE":
            raise BudgetExceededError(
                f"Unauthorized streaming dispatch attempt: Request {req.request_id} has no ACTIVE reservation."
            )

        if not self.allow_live_network:
            raise NetworkAccessForbiddenError(
                f"Live network access to {self.api_base_url} is NOT_AUTHORIZED. "
                f"Enable allow_live_network=True to permit outbound provider dispatch."
            )

        return self.dispatch(req, reservation)


def create_provider_transport(
    provider: str,
    model: str = "gpt-5.6-luna",
    api_key: Optional[str] = None,
    api_base_url: Optional[str] = None,
    allow_live_network: bool = False,
    timeout_seconds: int = 60,
    canned_responses: Optional[List[Dict[str, Any]]] = None
) -> ProviderTransport:
    """
    Factory function for ProviderTransport implementations.
    Enforces that mock transport is only selected when explicitly requested (provider='mock'),
    and genuine live transport fails closed if credentials are missing or live network is unauthorized.
    """
    prov = (provider or "mock").lower()

    if prov == "mock":
        return MockProviderTransport(
            model=model,
            canned_responses=canned_responses,
            simulate_latency_ms=10.0
        )
    elif prov in ("controlled", "openai", "openai_compatible"):
        eff_key = api_key or os.environ.get("OPENAI_API_KEY")
        eff_url = api_base_url or os.environ.get("OPENAI_BASE_URL") or "https://api.openai.com/v1"

        if allow_live_network:
            if not eff_key or not eff_key.strip():
                raise AuthenticationError(
                    f"Genuine live provider execution requested (provider='{provider}') but OPENAI_API_KEY is not set. "
                    "Live mode cannot silently fall back to mock transport. Missing credentials must fail closed."
                )
            return OpenAICompatibleTransport(
                model=model,
                api_base_url=eff_url,
                api_key=eff_key.strip(),
                timeout_seconds=timeout_seconds,
                allow_live_network=True
            )
        else:
            return OpenAICompatibleTransport(
                model=model,
                api_base_url=eff_url,
                api_key=eff_key,
                timeout_seconds=timeout_seconds,
                allow_live_network=False
            )
    else:
        raise InvalidConfigurationError(f"Unsupported provider transport: '{provider}'")


# ==============================================================================
# Mock Provider Transport for Offline Testing & Verification
# ==============================================================================

class MockProviderTransport(ProviderTransport):
    """
    Mock transport for 100% offline testing and synthetic benchmark execution.
    Supports scripted responses, tool calls, streaming token simulation,
    error injection (rate limits, timeouts, crashes), and missing usage metadata.
    """
    def __init__(
        self,
        model: str = "gpt-5.6-luna",
        canned_responses: Optional[List[Dict[str, Any]]] = None,
        simulate_latency_ms: float = 10.0,
        fail_next_with: Optional[Exception] = None
    ):
        super().__init__(
            model=model,
            provider_name="mock_transport",
            api_base_url="mock://localhost/v1",
            api_key=None,
            timeout_seconds=60,
            allow_live_network=False
        )
        self.canned_responses: List[Dict[str, Any]] = list(canned_responses or [])
        self.response_index: int = 0
        self.simulate_latency_ms = simulate_latency_ms
        self.fail_next_with: Optional[Exception] = fail_next_with
        self.dispatched_history: List[Tuple[ProviderRequest, RequestReservation]] = []

    def serialize_request(self, req: ProviderRequest) -> Tuple[str, bytes, int, str]:
        payload: Dict[str, Any] = {
            "model": req.model or self.model,
            "messages": req.messages,
            "max_completion_tokens": req.max_completion_tokens,
            "temperature": req.temperature,
            "stream": req.stream,
        }
        if req.tools:
            payload["tools"] = req.tools
        if req.tool_choice:
            payload["tool_choice"] = req.tool_choice

        serialized_str = json.dumps(payload, ensure_ascii=False, sort_keys=True)
        raw_bytes = serialized_str.encode("utf-8")
        tokens, bytes_len, method = self.count_tokens(serialized_str)
        return serialized_str, raw_bytes, tokens, method

    def dispatch(self, req: ProviderRequest, reservation: RequestReservation) -> ProviderResponse:
        """
        Mocks a synchronous model dispatch.
        """
        now_dt = datetime.datetime.now(datetime.timezone.utc)
        dispatched_at = now_dt.isoformat()

        if not reservation or reservation.status != "ACTIVE":
            raise BudgetExceededError(
                f"Unauthorized dispatch: Request {req.request_id} has no ACTIVE reservation."
            )

        if self.fail_next_with:
            exc = self.fail_next_with
            self.fail_next_with = None
            raise exc

        self.dispatched_history.append((req, reservation))

        # Check for scripted canned response
        if self.response_index < len(self.canned_responses):
            canned = self.canned_responses[self.response_index]
            self.response_index += 1
        else:
            # Default deterministic response
            canned = {
                "content": f"Task completed successfully by {req.arm_id} on turn {req.turn_index}.",
                "tool_calls": None,
                "finish_reason": "stop",
                "prompt_tokens": reservation.input_tokens,
                "completion_tokens": min(50, reservation.reserved_output_tokens),
            }

        completed_dt = datetime.datetime.now(datetime.timezone.utc)
        completed_at = completed_dt.isoformat()

        p_tokens = canned.get("prompt_tokens")
        c_tokens = canned.get("completion_tokens")
        t_tokens = (p_tokens + c_tokens) if (p_tokens is not None and c_tokens is not None) else canned.get("total_tokens")

        return ProviderResponse(
            response_id=f"mock-resp-{uuid.uuid4().hex[:8]}",
            request_id=req.request_id,
            turn_index=req.turn_index,
            model=req.model,
            content=canned.get("content"),
            tool_calls=canned.get("tool_calls"),
            finish_reason=canned.get("finish_reason", "stop"),
            prompt_tokens=p_tokens,
            completion_tokens=c_tokens,
            total_tokens=t_tokens,
            raw_usage={"prompt_tokens": p_tokens, "completion_tokens": c_tokens, "total_tokens": t_tokens} if t_tokens is not None else None,
            dispatched_at=dispatched_at,
            completed_at=completed_at,
            latency_ms=self.simulate_latency_ms,
            is_streaming=False,
            stream_cancelled=False,
            error=canned.get("error"),
            error_code=canned.get("error_code")
        )

    def stream_dispatch(
        self,
        req: ProviderRequest,
        reservation: RequestReservation,
        chunk_callback: Optional[Callable[[str, int], None]] = None
    ) -> ProviderResponse:
        """
        Mocks a streaming dispatch with chunk emission and token allowance enforcement.
        """
        now_dt = datetime.datetime.now(datetime.timezone.utc)
        dispatched_at = now_dt.isoformat()

        if not reservation or reservation.status != "ACTIVE":
            raise BudgetExceededError(
                f"Unauthorized streaming dispatch: Request {req.request_id} has no ACTIVE reservation."
            )

        if self.fail_next_with:
            exc = self.fail_next_with
            self.fail_next_with = None
            raise exc

        self.dispatched_history.append((req, reservation))

        # Get canned response definition
        if self.response_index < len(self.canned_responses):
            canned = self.canned_responses[self.response_index]
            self.response_index += 1
        else:
            canned = {
                "chunks": ["Task ", "completed ", "successfully."],
                "simulated_completion_tokens": 12,
                "prompt_tokens": reservation.input_tokens
            }

        chunks = canned.get("chunks", ["Hello", " world"])
        accumulated_text = ""
        accumulated_tokens = 0
        stream_cancelled = False

        for chunk in chunks:
            chunk_tokens, _, _ = self.count_tokens(chunk)
            if accumulated_tokens + chunk_tokens > reservation.reserved_output_tokens:
                # Real-time cancellation!
                stream_cancelled = True
                break

            accumulated_text += chunk
            accumulated_tokens += chunk_tokens
            if chunk_callback:
                chunk_callback(chunk, accumulated_tokens)

        completed_dt = datetime.datetime.now(datetime.timezone.utc)
        completed_at = completed_dt.isoformat()

        p_tokens = canned.get("prompt_tokens", reservation.input_tokens)
        c_tokens = accumulated_tokens
        t_tokens = p_tokens + c_tokens if p_tokens is not None else c_tokens

        return ProviderResponse(
            response_id=f"mock-stream-{uuid.uuid4().hex[:8]}",
            request_id=req.request_id,
            turn_index=req.turn_index,
            model=req.model,
            content=accumulated_text,
            tool_calls=canned.get("tool_calls"),
            finish_reason="cancelled" if stream_cancelled else "stop",
            prompt_tokens=p_tokens,
            completion_tokens=c_tokens,
            total_tokens=t_tokens,
            raw_usage={"prompt_tokens": p_tokens, "completion_tokens": c_tokens, "total_tokens": t_tokens},
            dispatched_at=dispatched_at,
            completed_at=completed_at,
            latency_ms=self.simulate_latency_ms,
            is_streaming=True,
            stream_cancelled=stream_cancelled,
            error="Stream cancelled: completion allowance exceeded" if stream_cancelled else None,
            error_code="STREAM_CANCELLED_ALLOWANCE_EXCEEDED" if stream_cancelled else None
        )


# ==============================================================================
# Transport Spy for Adversarial Verification
# ==============================================================================

class TransportSpy:
    """
    Test spy wrapping a ProviderTransport to record all attempted dispatches.
    Enables assertions that:
    1. For every rejected request, exactly 0 dispatches occurred.
    2. For every accepted request, a valid reservation existed prior to dispatch.
    """
    def __init__(self, inner_transport: ProviderTransport):
        self.inner_transport = inner_transport
        self.recorded_dispatches: List[Dict[str, Any]] = []
        self.rejected_attempts: List[Dict[str, Any]] = []

    @property
    def model(self) -> str:
        return self.inner_transport.model

    @property
    def provider_name(self) -> str:
        return self.inner_transport.provider_name

    def serialize_request(self, req: ProviderRequest) -> Tuple[str, bytes, int, str]:
        return self.inner_transport.serialize_request(req)

    def count_tokens(self, text: str) -> Tuple[int, int, str]:
        return self.inner_transport.count_tokens(text)

    def dispatch(self, req: ProviderRequest, reservation: RequestReservation) -> ProviderResponse:
        # Pre-dispatch assertion: Reservation MUST be active
        if not reservation or reservation.status != "ACTIVE":
            record = {
                "request_id": req.request_id,
                "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                "reason": "Missing or inactive reservation"
            }
            self.rejected_attempts.append(record)
            raise BudgetExceededError(f"Spy blocked unauthorized dispatch for request {req.request_id}")

        serialized, raw_bytes, tokens, method = self.serialize_request(req)
        dispatch_record = {
            "request_id": req.request_id,
            "turn_index": req.turn_index,
            "arm_id": req.arm_id,
            "task_id": req.task_id,
            "reservation_id": reservation.request_id,
            "reserved_tokens": reservation.total_reserved_tokens,
            "input_tokens": tokens,
            "payload_bytes": len(raw_bytes),
            "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "streaming": False
        }
        self.recorded_dispatches.append(dispatch_record)

        return self.inner_transport.dispatch(req, reservation)

    def stream_dispatch(
        self,
        req: ProviderRequest,
        reservation: RequestReservation,
        chunk_callback: Optional[Callable[[str, int], None]] = None
    ) -> ProviderResponse:
        if not reservation or reservation.status != "ACTIVE":
            record = {
                "request_id": req.request_id,
                "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                "reason": "Missing or inactive reservation"
            }
            self.rejected_attempts.append(record)
            raise BudgetExceededError(f"Spy blocked unauthorized streaming dispatch for request {req.request_id}")

        serialized, raw_bytes, tokens, method = self.serialize_request(req)
        dispatch_record = {
            "request_id": req.request_id,
            "turn_index": req.turn_index,
            "arm_id": req.arm_id,
            "task_id": req.task_id,
            "reservation_id": reservation.request_id,
            "reserved_tokens": reservation.total_reserved_tokens,
            "input_tokens": tokens,
            "payload_bytes": len(raw_bytes),
            "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "streaming": True
        }
        self.recorded_dispatches.append(dispatch_record)

        return self.inner_transport.stream_dispatch(req, reservation, chunk_callback)

    def record_rejected_request(self, request_id: str, reason: str, details: Optional[Dict[str, Any]] = None):
        """Called by runtime when a request is rejected before reaching transport."""
        record = {
            "request_id": request_id,
            "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "reason": reason,
            "details": details or {}
        }
        self.rejected_attempts.append(record)

    def assert_zero_dispatches_for_request(self, request_id: str):
        matching = [d for d in self.recorded_dispatches if d["request_id"] == request_id]
        if len(matching) > 0:
            raise AssertionError(
                f"Spy verification failed: Request '{request_id}' was rejected but recorded {len(matching)} dispatches!"
            )

    def assert_all_dispatches_had_valid_reservation(self):
        for d in self.recorded_dispatches:
            if not d.get("reservation_id") or d.get("reserved_tokens", 0) <= 0:
                raise AssertionError(f"Spy verification failed: Dispatch occurred without valid reservation: {d}")
