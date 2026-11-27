"""
Headless Codex CLI runner for Matched-Pair Benchmark runs.
Remediated capabilities:
1. Explicitly pins supported model and reasoning effort via CLI flags (-m, -c).
2. Sanitizes child process PATH by stripping any legacy Engram directories.
3. Tracks specific child PID and terminates only that process tree on timeout/exception.
4. Captures thread_id and full command metadata.
"""
from dataclasses import dataclass
from typing import Dict, Any, List, Optional, Tuple, Set
from pathlib import Path
import subprocess
import threading
import datetime
import hashlib
import time
import json
import shutil
import math
import os

from isolation import terminate_pid_tree, SafetyViolationError


class PreDispatchGateViolationError(SafetyViolationError):
    """Raised when a candidate run request exceeds the pre-dispatch token or byte ceiling."""
    pass


class RequestSizeGate:
    """
    Strict offline pre-dispatch request-size protection gate.
    Enforces a hard pre-dispatch token boundary (~60,000 tokens) on outbound model requests.
    Zero-truncation policy: Fails closed immediately if boundary is breached.
    Measures complete prompt text, injected context, instructions files, and CLI parameters.
    """
    def __init__(self, default_limit_tokens: int = 60_000):
        self.default_limit_tokens = default_limit_tokens
        self._encoder = None
        self._encoding_name = None
        self._init_encoder()

    def _init_encoder(self):
        try:
            import tiktoken
            try:
                self._encoder = tiktoken.get_encoding("o200k_base")
                self._encoding_name = "tiktoken_o200k_base"
            except Exception:
                self._encoder = tiktoken.get_encoding("cl100k_base")
                self._encoding_name = "tiktoken_cl100k_base"
        except Exception:
            self._encoder = None
            self._encoding_name = "heuristic_bytes_upper_bound"

    def count_tokens(self, text: str) -> Tuple[int, int, str]:
        """
        Counts tokens and bytes in text using offline tiktoken encoder,
        falling back to a mathematically proven conservative upper-bound heuristic if unavailable.
        Returns: (token_count, byte_count, method_name)
        """
        if not text:
            return (0, 0, self._encoding_name or "empty")

        byte_count = len(text.encode("utf-8", errors="replace"))

        if self._encoder is not None:
            try:
                tokens = len(self._encoder.encode(text))
                return (tokens, byte_count, self._encoding_name)
            except Exception:
                pass

        # Mathematically proven conservative upper-bound heuristic fallback:
        # In BPE tokenizers, every token represents at least 1 UTF-8 byte.
        # Therefore, token_count <= byte_count holds unconditionally for all UTF-8 inputs.
        # Dividing by 3.0 was an empirical approximation for prose, but severely undercounted
        # on code, ASCII, and digits (by up to 63%).
        # Using byte_count guarantees zero undercount under all adversarial inputs.
        tokens = byte_count
        return (tokens, byte_count, "heuristic_bytes_upper_bound")

    def measure_complete_request(
        self,
        prompt: str,
        cmd_args: Optional[List[str]] = None,
        sandbox_path: Optional[Path] = None,
        extra_context: Optional[str] = None
    ) -> Tuple[int, int, str, Dict[str, Any]]:
        """
        Assembles and measures all components of the outbound request:
        - Effective prompt / Turn-0 context
        - Workspace instruction files (.github/copilot-instructions.md, .codex/instructions.md)
        - CLI arguments and configuration flags
        - Extra context (tool definitions, schemas, etc.)
        """
        components: Dict[str, Dict[str, Any]] = {}
        full_text_parts: List[str] = []

        # 1. Main prompt & turn0 context
        p_tokens, p_bytes, p_method = self.count_tokens(prompt or "")
        components["prompt"] = {"tokens": p_tokens, "bytes": p_bytes}
        full_text_parts.append(prompt or "")

        # 2. Instruction files if present in sandbox
        inst_text = ""
        if sandbox_path:
            sb = Path(sandbox_path)
            for rel_path in [
                Path(".github") / "copilot-instructions.md",
                Path(".codex") / "instructions.md",
                Path(".codex") / "config.toml"
            ]:
                target = sb / rel_path
                if target.exists() and target.is_file():
                    try:
                        content = target.read_text(encoding="utf-8", errors="ignore")
                        inst_text += f"\n--- {rel_path} ---\n{content}\n"
                    except Exception:
                        pass
        i_tokens, i_bytes, _ = self.count_tokens(inst_text)
        components["instructions"] = {"tokens": i_tokens, "bytes": i_bytes}
        if inst_text:
            full_text_parts.append(inst_text)

        # 3. CLI arguments
        cmd_text = " ".join(cmd_args) if cmd_args else ""
        c_tokens, c_bytes, _ = self.count_tokens(cmd_text)
        components["cli_arguments"] = {"tokens": c_tokens, "bytes": c_bytes}
        if cmd_text:
            full_text_parts.append(cmd_text)

        # 4. Extra context (tool schemas, metadata)
        if extra_context:
            e_tokens, e_bytes, _ = self.count_tokens(extra_context)
            components["extra_context"] = {"tokens": e_tokens, "bytes": e_bytes}
            full_text_parts.append(extra_context)

        full_request_text = "\n\n".join(full_text_parts)
        tot_tokens, tot_bytes, method = self.count_tokens(full_request_text)

        breakdown = {
            "components": components,
            "total_tokens": tot_tokens,
            "total_bytes": tot_bytes,
            "counting_method": method
        }
        return tot_tokens, tot_bytes, method, breakdown

    def check_pre_dispatch(
        self,
        prompt: str,
        cmd_args: Optional[List[str]] = None,
        sandbox_path: Optional[Path] = None,
        extra_context: Optional[str] = None,
        limit_tokens: Optional[int] = None
    ) -> Tuple[bool, int, int, str, Optional[str], Dict[str, Any]]:
        """
        Strict pre-dispatch boundary check.
        Returns (passed, total_tokens, total_bytes, method, error_message, breakdown).
        """
        effective_limit = limit_tokens if limit_tokens is not None else self.default_limit_tokens
        tokens, bytes_count, method, breakdown = self.measure_complete_request(
            prompt=prompt,
            cmd_args=cmd_args,
            sandbox_path=sandbox_path,
            extra_context=extra_context
        )

        if tokens > effective_limit:
            msg = (
                f"Pre-dispatch request boundary violation: Complete outbound request is {tokens:,} tokens "
                f"({bytes_count:,} bytes, method: {method}), which exceeds the configured limit of "
                f"{effective_limit:,} tokens by {tokens - effective_limit:,} tokens. "
                f"Zero-truncation policy enforced: Execution rejected pre-dispatch."
            )
            return False, tokens, bytes_count, method, msg, breakdown

        return True, tokens, bytes_count, method, None, breakdown


@dataclass
class RequestReservation:
    """
    Formal per-request token reservation.
    R = input_tokens + reserved_output_tokens
    """
    request_id: str
    turn_index: int
    input_tokens: int
    reserved_output_tokens: int
    total_reserved_tokens: int
    created_at: str
    status: str = "ACTIVE"  # "ACTIVE", "RECONCILED", "REJECTED"
    actual_input_tokens: Optional[int] = None
    actual_output_tokens: Optional[int] = None
    actual_total_tokens: Optional[int] = None
    reconciled_at: Optional[str] = None
    rejection_reason: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "request_id": self.request_id,
            "turn_index": self.turn_index,
            "input_tokens": self.input_tokens,
            "reserved_output_tokens": self.reserved_output_tokens,
            "total_reserved_tokens": self.total_reserved_tokens,
            "created_at": self.created_at,
            "status": self.status,
            "actual_input_tokens": self.actual_input_tokens,
            "actual_output_tokens": self.actual_output_tokens,
            "actual_total_tokens": self.actual_total_tokens,
            "reconciled_at": self.reconciled_at,
            "rejection_reason": self.rejection_reason
        }


class RequestReservationCoordinator:
    """
    Synchronous request-level budget reservation and enforcement coordinator.
    Enforces that every model request reserves R = I + O before network dispatch,
    guaranteeing that R <= B_remaining - B_reserve.
    Fails closed immediately (zero truncation) if capacity is insufficient.
    """
    def __init__(
        self,
        aggregate_budget: int = 1_000_000,
        safety_reserve: int = 45_000,
        default_completion_allowance: int = 4_096
    ):
        if aggregate_budget <= 0:
            raise ValueError(f"aggregate_budget must be positive, got {aggregate_budget}")
        if safety_reserve < 0:
            raise ValueError(f"safety_reserve cannot be negative, got {safety_reserve}")
        self.aggregate_budget = aggregate_budget
        self.safety_reserve = safety_reserve
        self.default_completion_allowance = default_completion_allowance
        self.cumulative_reconciled_tokens: int = 0
        self.active_reservations: Dict[str, RequestReservation] = {}
        self.reconciled_history: List[RequestReservation] = []
        self.rejection_history: List[Dict[str, Any]] = []

    @property
    def in_flight_reserved_tokens(self) -> int:
        return sum(res.total_reserved_tokens for res in self.active_reservations.values() if res.status == "ACTIVE")

    @property
    def total_committed_tokens(self) -> int:
        return self.cumulative_reconciled_tokens + self.in_flight_reserved_tokens

    @property
    def remaining_budget(self) -> int:
        return max(0, self.aggregate_budget - self.total_committed_tokens)

    def reserve_request(
        self,
        request_id: str,
        turn_index: int,
        input_tokens: int,
        completion_allowance: Optional[int] = None
    ) -> Tuple[bool, Optional[str], Optional[RequestReservation]]:
        """
        Attempts to reserve capacity for an outbound request.
        Fails closed if R = I + O > B_remaining - B_reserve.
        """
        out_allowance = completion_allowance if completion_allowance is not None else self.default_completion_allowance
        r_total = input_tokens + out_allowance
        avail_for_dispatch = self.remaining_budget - self.safety_reserve

        now_iso = datetime.datetime.now(datetime.timezone.utc).isoformat()

        if r_total > avail_for_dispatch:
            deficit = r_total - avail_for_dispatch
            reason = (
                f"Request budget reservation rejected: Request {request_id} (Turn {turn_index}) requires "
                f"{r_total:,} tokens (Input: {input_tokens:,}, Reserved Output: {out_allowance:,}), but available "
                f"headroom above safety reserve ({self.safety_reserve:,}) is only {avail_for_dispatch:,} tokens "
                f"(Remaining Budget: {self.remaining_budget:,}, In-Flight Reservations: {self.in_flight_reserved_tokens:,}). "
                f"Deficit: {deficit:,} tokens. Zero-truncation policy enforced: Pre-dispatch rejected."
            )
            rejection_event = {
                "request_id": request_id,
                "turn_index": turn_index,
                "estimated_input_tokens": input_tokens,
                "reserved_output_tokens": out_allowance,
                "total_requested_reservation": r_total,
                "aggregate_remaining_budget": self.remaining_budget,
                "safety_reserve": self.safety_reserve,
                "available_headroom": avail_for_dispatch,
                "rejection_reason": reason,
                "enforcement_timestamp": now_iso,
                "provider_dispatch_prevented": True
            }
            self.rejection_history.append(rejection_event)
            return False, reason, None

        reservation = RequestReservation(
            request_id=request_id,
            turn_index=turn_index,
            input_tokens=input_tokens,
            reserved_output_tokens=out_allowance,
            total_reserved_tokens=r_total,
            created_at=now_iso,
            status="ACTIVE"
        )
        self.active_reservations[request_id] = reservation
        return True, None, reservation

    def reconcile_response(
        self,
        request_id: str,
        actual_total_tokens: Optional[int] = None,
        actual_input_tokens: Optional[int] = None,
        actual_output_tokens: Optional[int] = None
    ) -> RequestReservation:
        """
        Reconciles a completed request with actual provider usage.
        If actual usage is missing or None, conservatively charges the full reserved capacity R.
        """
        reservation = self.active_reservations.pop(request_id, None)
        now_iso = datetime.datetime.now(datetime.timezone.utc).isoformat()

        if not reservation:
            raise KeyError(f"No active reservation found for request {request_id}")

        if actual_total_tokens is not None:
            final_charge = actual_total_tokens
        elif actual_input_tokens is not None and actual_output_tokens is not None:
            final_charge = actual_input_tokens + actual_output_tokens
        else:
            # Conservative exposure preservation: missing usage does not release budget
            final_charge = reservation.total_reserved_tokens

        self.cumulative_reconciled_tokens += final_charge

        reservation.status = "RECONCILED"
        reservation.actual_input_tokens = actual_input_tokens
        reservation.actual_output_tokens = actual_output_tokens
        reservation.actual_total_tokens = final_charge
        reservation.reconciled_at = now_iso
        self.reconciled_history.append(reservation)
        return reservation

    def update_streaming_tokens(self, request_id: str, observed_output_tokens: int) -> None:
        """
        Updates in-flight reservation if streaming tokens exceed the initial output allowance.
        """
        reservation = self.active_reservations.get(request_id)
        if not reservation:
            return
        if observed_output_tokens > reservation.reserved_output_tokens:
            extra = observed_output_tokens - reservation.reserved_output_tokens
            reservation.reserved_output_tokens = observed_output_tokens
            reservation.total_reserved_tokens += extra

    def to_state_dict(self) -> Dict[str, Any]:
        return {
            "aggregate_budget": self.aggregate_budget,
            "safety_reserve": self.safety_reserve,
            "default_completion_allowance": self.default_completion_allowance,
            "cumulative_reconciled_tokens": self.cumulative_reconciled_tokens,
            "in_flight_reserved_tokens": self.in_flight_reserved_tokens,
            "remaining_budget": self.remaining_budget,
            "active_reservations": {k: v.to_dict() for k, v in self.active_reservations.items()},
            "reconciled_history": [r.to_dict() for r in self.reconciled_history],
            "rejection_history": list(self.rejection_history)
        }

    @classmethod
    def from_state_dict(cls, data: Dict[str, Any]) -> "RequestReservationCoordinator":
        coord = cls(
            aggregate_budget=data["aggregate_budget"],
            safety_reserve=data["safety_reserve"],
            default_completion_allowance=data.get("default_completion_allowance", 4096)
        )
        coord.cumulative_reconciled_tokens = data.get("cumulative_reconciled_tokens", 0)
        coord.rejection_history = data.get("rejection_history", [])
        for k, v in data.get("active_reservations", {}).items():
            coord.active_reservations[k] = RequestReservation(**v)
        for r in data.get("reconciled_history", []):
            coord.reconciled_history.append(RequestReservation(**r))
        return coord

KNOWN_RIPGREP_CANDIDATES: List[Path] = [
    Path(r"C:\Program Files\Microsoft Visual Studio\18\Community\Common7\IDE\CopilotCli\rg.exe"),
    Path(r"C:\Program Files\Microsoft SQL Server Management Studio 22\Release\Common7\IDE\CopilotCli\rg.exe"),
    Path(r"C:\Users\Amal P S\AppData\Local\Programs\Microsoft VS Code\e646868227\resources\app\node_modules.asar.unpacked\@vscode\ripgrep-universal\bin\win32-x64\rg.exe"),
    Path(r"C:\Users\Amal P S\AppData\Local\Programs\Microsoft VS Code\6d62b1d5da\resources\app\node_modules.asar.unpacked\@vscode\ripgrep-universal\bin\win32-x64\rg.exe"),
]

SHELL_RETRIEVAL_GUIDANCE: str = (
    "\n\n[Context Retrieval Guidance]\n"
    "Always prefer Engram MCP tools (`engram_get_context`, `engram_inspect_symbol`) for code inspection.\n"
    "If inspecting files via shell commands, do NOT dump entire files into the prompt.\n"
    "Always use bounded line ranges (e.g. `Get-Content <file> | Select-Object -First 50` or ripgrep `rg -n -C 3 <symbol> <file>`)."
)


@dataclass
class RunExecutionResult:
    exit_code: int
    elapsed_seconds: float
    started_at: str
    completed_at: str
    events_log_path: str
    thread_id: Optional[str]
    model_name: str
    reasoning_effort: str
    harness_executable: str
    harness_version: str
    path_sanitized: bool
    error_message: Optional[str] = None
    turn0_injected: bool = False
    initial_prompt: Optional[str] = None
    timed_out: bool = False
    token_ceiling_breached: bool = False
    native_rollout_path: Optional[str] = None
    recovered_events_count: int = 0
    fast_path_active: bool = False
    pre_dispatch_guard_triggered: bool = False
    stop_reason: Optional[str] = None
    headroom_reserve: int = 45_000
    cumulative_tokens_at_stop: Optional[int] = None
    estimated_next_turn_cost: Optional[int] = None
    configured_ceiling: Optional[int] = None
    remaining_budget_at_stop: Optional[int] = None
    last_turn_usage_at_stop: Optional[int] = None
    guard_decision: Optional[str] = None
    guard_decision_timestamp: Optional[str] = None
    relevant_event_id: Optional[str] = None
    telemetry_valid: bool = True
    telemetry_status: str = "VALID"
    sandbox_policy: str = "workspace-write"
    provider: str = "cloud"
    adaptive_activation: Optional[Dict[str, Any]] = None
    request_size_gate_passed: bool = True
    measured_request_tokens: int = 0
    measured_request_bytes: int = 0
    request_counting_method: str = "tiktoken_o200k_base"
    configured_request_size_limit: int = 60_000
    request_size_breakdown: Optional[Dict[str, Any]] = None
    reservation_id: Optional[str] = None
    reservation_details: Optional[Dict[str, Any]] = None


class StreamingTokenTracker:
    """
    Real-time token tracker that safely extracts and reconciles tokens from streaming events.
    Invariants:
    1. Distinguishes cumulative thread snapshots (token_usage_record, event_msg token_count)
       from incremental turn deltas (turn.completed).
    2. Snapshots are never summed to each other; they update thread_cumulative_max.
    3. Incremental turn deltas are summed across distinct turns and deduplicated.
    4. Effective cumulative usage is max(thread_cumulative_max, turn_deltas_sum).
    5. Deduplication ensures repeated events or duplicate IDs cannot inflate token counts.
    6. Tracks last turn request tokens and conservative reserve to support pre-dispatch headroom checks.
    7. Tracks pending tools and intermediate turn lifecycle to detect pre-dispatch boundaries
       across Turns 2..N without relying on recurring turn.started events.
    8. Preserves the breakdown between cached input, uncached input, output, and reasoning tokens.
    9. Reasoning tokens are an informative sub-metric of output tokens and are NEVER double-counted.
    10. Invalid, negative, or malformed telemetry marks telemetry_valid=False and fails safe.
    """
    def __init__(self, conservative_reserve: int = 45_000):
        self.thread_cumulative_max: int = 0
        self.turn_deltas_sum: int = 0
        self._seen_turn_events: Set[str] = set()
        self._seen_snapshot_hashes: Set[str] = set()
        self.last_turn_request_tokens: int = 0
        self.last_turn_total_tokens: int = 0
        self.conservative_reserve: int = conservative_reserve
        self.turns_completed: int = 0
        self.has_pending_tool: bool = False

        # Detailed token breakdown
        self.prompt_tokens_total: int = 0
        self.prompt_tokens_cached: int = 0
        self.prompt_tokens_uncached: int = 0
        self.completion_tokens: int = 0
        self.reasoning_tokens: int = 0

        # Telemetry validity tracking
        self.telemetry_valid: bool = True
        self.telemetry_status: str = "VALID"  # "VALID", "INVALID", "UNKNOWN", "UNAVAILABLE"
        self.telemetry_error: Optional[str] = None
        self.partial_records_count: int = 0

        # Decision and event tracking
        self.last_event_id: Optional[str] = None
        self.last_event_type: Optional[str] = None
        self.last_decision: str = "DISPATCH_ALLOWED"  # "DISPATCH_ALLOWED", "PREVENTIVE_HALT", "OVERRUN_HALT"
        self.last_decision_timestamp: Optional[str] = None

    def process_line(self, line: str) -> int:
        """Processes a JSONL event line and returns current effective cumulative tokens."""
        if not line or not line.strip():
            return self.effective_tokens
        try:
            data = json.loads(line)
        except (json.JSONDecodeError, TypeError, ValueError):
            self.partial_records_count += 1
            return self.effective_tokens

        try:
            ev_type = data.get("type", "")
            line_hash = hashlib.sha256(line.strip().encode("utf-8")).hexdigest()

            # Event identifier extraction
            item = data.get("item") if isinstance(data.get("item"), dict) else {}
            ev_id = data.get("id") or item.get("id") or data.get("turn_id") or data.get("call_id")
            if ev_id:
                self.last_event_id = str(ev_id)
            if ev_type:
                self.last_event_type = ev_type

            # 1. Authoritative cumulative thread snapshot: token_usage_record
            if ev_type == "token_usage_record":
                payload = data.get("payload")
                if not isinstance(payload, dict):
                    self.telemetry_valid = False
                    self.telemetry_status = "INVALID"
                    self.telemetry_error = "Malformed token_usage_record: payload is not a dict"
                    return self.effective_tokens

                thread_usage = payload.get("thread_token_usage") or payload.get("usage")
                if not isinstance(thread_usage, dict):
                    self.telemetry_valid = False
                    self.telemetry_status = "INVALID"
                    self.telemetry_error = "Malformed token_usage_record: missing thread_token_usage"
                    return self.effective_tokens

                inp = thread_usage.get("input_tokens") if thread_usage.get("input_tokens") is not None else thread_usage.get("prompt_tokens")
                out = thread_usage.get("output_tokens") if thread_usage.get("output_tokens") is not None else thread_usage.get("completion_tokens", 0)
                tot = thread_usage.get("total_tokens")
                cached = thread_usage.get("cached_input_tokens")
                reasoning = thread_usage.get("reasoning_output_tokens", 0)

                # Validation checks: reject negative tokens or invalid types
                if tot is None and inp is None:
                    self.telemetry_valid = False
                    self.telemetry_status = "INVALID"
                    self.telemetry_error = "Both total_tokens and input_tokens are missing"
                    return self.effective_tokens

                if tot is not None and (not isinstance(tot, int) or tot < 0):
                    self.telemetry_valid = False
                    self.telemetry_status = "INVALID"
                    self.telemetry_error = f"Invalid total_tokens: {tot}"
                    return self.effective_tokens

                if inp is not None and (not isinstance(inp, int) or inp < 0):
                    self.telemetry_valid = False
                    self.telemetry_status = "INVALID"
                    self.telemetry_error = f"Invalid input_tokens: {inp}"
                    return self.effective_tokens

                if out is not None and (not isinstance(out, int) or out < 0):
                    self.telemetry_valid = False
                    self.telemetry_status = "INVALID"
                    self.telemetry_error = f"Invalid output_tokens: {out}"
                    return self.effective_tokens

                # Reconcile if one of (inp, tot) is absent
                if inp is None and tot is not None:
                    inp = max(0, tot - (out if isinstance(out, int) else 0))
                elif tot is None and inp is not None:
                    tot = inp + (out if isinstance(out, int) else 0)

                if cached is not None:
                    if not isinstance(cached, int) or cached < 0 or cached > inp:
                        self.telemetry_valid = False
                        self.telemetry_status = "INVALID"
                        self.telemetry_error = f"Invalid cached_input_tokens ({cached}) relative to input_tokens ({inp})"
                        return self.effective_tokens
                    self.prompt_tokens_cached = cached
                    self.prompt_tokens_uncached = inp - cached
                else:
                    self.prompt_tokens_cached = 0
                    self.prompt_tokens_uncached = inp

                self.prompt_tokens_total = inp
                self.completion_tokens = out if isinstance(out, int) else 0
                self.reasoning_tokens = reasoning if isinstance(reasoning, int) else 0

                # Single-request usage extraction (actual single-turn prompt context cost)
                single_usage = payload.get("usage")
                turn_usage = payload.get("turn_token_usage", {})

                req_inp = None
                if isinstance(single_usage, dict):
                    req_inp = single_usage.get("input_tokens") if single_usage.get("input_tokens") is not None else single_usage.get("prompt_tokens")

                if isinstance(req_inp, int) and req_inp > 0:
                    self.last_turn_request_tokens = req_inp
                elif isinstance(turn_usage, dict):
                    turn_inp = turn_usage.get("input_tokens") if turn_usage.get("input_tokens") is not None else turn_usage.get("prompt_tokens")
                    if isinstance(turn_inp, int) and turn_inp > 0:
                        self.last_turn_request_tokens = turn_inp
                elif isinstance(inp, int) and inp > 0 and self.last_turn_request_tokens == 0:
                    self.last_turn_request_tokens = inp

                if isinstance(turn_usage, dict):
                    turn_tot = turn_usage.get("total_tokens", 0)
                    if isinstance(turn_tot, int) and turn_tot > 0:
                        self.last_turn_total_tokens = turn_tot

                # Snapshot deduplication: do not double-increment turns_completed on duplicate snapshot
                if line_hash not in self._seen_snapshot_hashes:
                    self._seen_snapshot_hashes.add(line_hash)
                    self.turns_completed += 1
                    if tot > self.thread_cumulative_max:
                        self.thread_cumulative_max = tot

            # 2. Authoritative cumulative thread snapshot: event_msg token_count
            elif ev_type == "event_msg":
                payload = data.get("payload")
                if isinstance(payload, dict) and payload.get("type") == "token_count":
                    info = payload.get("info", {})
                    if isinstance(info, dict):
                        tot_usage = info.get("total_token_usage", {})
                        if isinstance(tot_usage, dict):
                            inp = tot_usage.get("input_tokens") if tot_usage.get("input_tokens") is not None else tot_usage.get("prompt_tokens")
                            out = tot_usage.get("output_tokens") if tot_usage.get("output_tokens") is not None else tot_usage.get("completion_tokens", 0)
                            tot = tot_usage.get("total_tokens")
                            cached = tot_usage.get("cached_input_tokens")
                            reasoning = tot_usage.get("reasoning_output_tokens", 0)

                            if isinstance(inp, int) and inp >= 0:
                                self.prompt_tokens_total = inp
                                if isinstance(cached, int) and 0 <= cached <= inp:
                                    self.prompt_tokens_cached = cached
                                    self.prompt_tokens_uncached = inp - cached
                                else:
                                    self.prompt_tokens_cached = 0
                                    self.prompt_tokens_uncached = inp

                                # Extract single-request usage from last_token_usage rather than total cumulative
                                last_usage = info.get("last_token_usage")
                                if isinstance(last_usage, dict):
                                    last_inp = last_usage.get("input_tokens") if last_usage.get("input_tokens") is not None else last_usage.get("prompt_tokens")
                                    if isinstance(last_inp, int) and last_inp > 0:
                                        self.last_turn_request_tokens = last_inp
                                elif inp > 0 and self.last_turn_request_tokens == 0:
                                    self.last_turn_request_tokens = inp

                            if isinstance(out, int) and out >= 0:
                                self.completion_tokens = out
                            if isinstance(reasoning, int) and reasoning >= 0:
                                self.reasoning_tokens = reasoning

                            if tot is None and isinstance(inp, int) and isinstance(out, int):
                                tot = inp + out
                            if isinstance(tot, int) and tot > self.thread_cumulative_max:
                                self.thread_cumulative_max = tot

            # 3. Incremental turn completion: turn.completed
            elif ev_type == "turn.completed":
                turn_key = data.get("turn_id") or line_hash
                if turn_key not in self._seen_turn_events:
                    self._seen_turn_events.add(turn_key)
                    self.turns_completed += 1
                    usage = data.get("usage", {})
                    if isinstance(usage, dict):
                        inp = usage.get("input_tokens", 0)
                        out = usage.get("output_tokens", 0)
                        tot = usage.get("total_tokens")
                        cached = usage.get("cached_input_tokens")
                        reasoning = usage.get("reasoning_output_tokens", 0)

                        if isinstance(inp, int) and inp >= 0:
                            if inp > 0 and self.last_turn_request_tokens == 0:
                                self.last_turn_request_tokens = inp
                            self.prompt_tokens_total += inp
                            if isinstance(cached, int) and 0 <= cached <= inp:
                                self.prompt_tokens_cached += cached
                                self.prompt_tokens_uncached += (inp - cached)
                            else:
                                self.prompt_tokens_uncached += inp
                        if isinstance(out, int) and out >= 0:
                            self.completion_tokens += out
                        if isinstance(reasoning, int) and reasoning >= 0:
                            self.reasoning_tokens += reasoning

                        # Reasoning tokens are inside output_tokens; never add reasoning to total
                        if tot is None and isinstance(inp, int) and isinstance(out, int):
                            tot = inp + out
                        if isinstance(tot, int) and tot > 0:
                            self.last_turn_total_tokens = tot
                            self.turn_deltas_sum += tot

            # 4. Tool lifecycle tracking for multi-turn execution
            if ev_type in ("custom_tool_call", "mcp_tool_call"):
                self.has_pending_tool = True
            elif ev_type == "item.started":
                item_data = data.get("item", {}) if isinstance(data.get("item"), dict) else {}
                item_type = item_data.get("type", "")
                if item_type in ("command_execution", "mcp_tool_call", "file_edit", "web_search", "tool_call"):
                    self.has_pending_tool = True
            elif ev_type == "item.completed":
                item_data = data.get("item", {}) if isinstance(data.get("item"), dict) else {}
                item_type = item_data.get("type", "")
                if item_type in ("command_execution", "mcp_tool_call", "file_edit", "web_search", "tool_call"):
                    # Tool finished executing locally; next model request will be dispatched with tool results
                    self.has_pending_tool = True
            elif ev_type == "custom_tool_call_output":
                self.has_pending_tool = True
            elif ev_type in ("turn.started", "reasoning", "turn.completed"):
                # New model generation has commenced or turn has concluded
                self.has_pending_tool = False

        except Exception as e:
            self.telemetry_valid = False
            self.telemetry_status = "INVALID"
            self.telemetry_error = f"Exception processing event line: {e}"

        return self.effective_tokens

    def evaluate_pre_dispatch_boundary(self, line: str) -> Tuple[bool, bool, str]:
        """
        Comprehensive boundary evaluation returning:
            (is_candidate_trigger: bool, is_guaranteed_pre_dispatch: bool, classification: str)
        Distinguishes poller candidate trigger events from guaranteed pre-dispatch boundaries.
        Under external polling, no event is guaranteed to halt an outbound request before network dispatch.
        """
        if not line or not line.strip():
            return (False, False, "EMPTY_LINE")
        try:
            data = json.loads(line)
        except Exception:
            return (False, False, "MALFORMED_JSON")

        ev_type = data.get("type", "")
        item = data.get("item", {}) if isinstance(data.get("item"), dict) else {}
        item_type = item.get("type", "")

        if ev_type == "item.started" and item_type in ("command_execution", "mcp_tool_call", "file_edit", "web_search", "tool_call"):
            return (True, False, "TOOL_START_CANDIDATE_UNPROVEN")

        if ev_type in ("custom_tool_call", "mcp_tool_call"):
            return (True, False, "TOOL_CALL_INITIATION_UNPROVEN")

        if ev_type == "item.completed" and item_type in ("command_execution", "mcp_tool_call", "file_edit", "web_search", "tool_call"):
            return (True, False, "TOOL_COMPLETED_CANDIDATE_UNPROVEN")

        if ev_type == "custom_tool_call_output":
            return (True, False, "TOOL_OUTPUT_CANDIDATE_UNPROVEN")

        if ev_type == "turn.started":
            return (True, False, "TURN_STARTED_CANDIDATE_UNPROVEN")

        if ev_type in ("token_usage_record", "event_msg") and self.has_pending_tool:
            return (True, False, "SNAPSHOT_PENDING_DISPATCH_UNPROVEN")

        return (False, False, "NOT_PRE_DISPATCH_BOUNDARY")

    def is_guaranteed_pre_dispatch_boundary(self, line: str) -> bool:
        """
        Evaluates whether an event provides a mathematically guaranteed pre-dispatch boundary.
        Under the external polling architecture (50ms polling loop without internal child hooks),
        NO event can guarantee that an outbound request has not already been dispatched.
        Always returns False.
        """
        return False

    def is_pre_dispatch_boundary(self, line: str) -> bool:
        """
        Determines whether a streaming JSONL line marks a candidate pre-dispatch boundary
        where a new model inference request is about to be sent.
        In multi-turn codex exec runs, this occurs:
        1. At initial turn preflight (turn.started).
        2. When a tool call is initiated or completed (signaling tool execution or output packaging).
        3. When a token usage snapshot arrives while a tool call has completed.
        """
        is_candidate, _, _ = self.evaluate_pre_dispatch_boundary(line)
        return is_candidate

    def estimate_next_request_tokens(self) -> int:
        """Estimates the tokens required for the next turn request."""
        reserve = self.conservative_reserve if (self.conservative_reserve and self.conservative_reserve > 0) else 0
        if self.last_turn_request_tokens > 0:
            return max(self.last_turn_request_tokens, reserve)
        return reserve

    def check_pre_dispatch_headroom(self, ceiling: int) -> Tuple[bool, int, int]:
        """
        Evaluates whether sufficient headroom remains to dispatch the next turn.
        Formula:
            remaining = ceiling - cumulative_usage
            estimated_next_turn = max(last_turn_request_tokens, conservative_reserve)
            has_headroom = (remaining >= estimated_next_turn)

        Returns:
            (has_headroom: bool, remaining_tokens: int, estimated_next_turn_cost: int)

        Safety Guard Rules:
        1. If ceiling is None, not an int, or <= 0: fail-safe, has_headroom = False
        2. If conservative_reserve is None, not an int, or <= 0: fail-safe, has_headroom = False
        3. If telemetry is invalid (telemetry_valid is False): fail-safe, has_headroom = False
        4. Equality boundary: remaining == estimated_next_turn -> has_headroom = True (dispatch allowed)
        5. One token below: remaining < estimated_next_turn -> has_headroom = False (preventive stop)
        6. Cumulative usage at or above ceiling: remaining <= 0 -> has_headroom = False (halt)
        """
        # Fail-safe checks for missing or invalid safety configuration
        if ceiling is None or not isinstance(ceiling, int) or ceiling <= 0:
            self.last_decision = "PREVENTIVE_HALT"
            self.last_decision_timestamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
            return (False, 0, 0)

        if self.conservative_reserve is None or not isinstance(self.conservative_reserve, int) or self.conservative_reserve <= 0:
            self.last_decision = "PREVENTIVE_HALT"
            self.last_decision_timestamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
            return (False, 0, 0)

        if not self.telemetry_valid:
            self.last_decision = "PREVENTIVE_HALT"
            self.last_decision_timestamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
            return (False, 0, 0)

        remaining = ceiling - self.effective_tokens
        estimated = self.estimate_next_request_tokens()

        # Cumulative tokens already at or above ceiling
        if remaining <= 0:
            self.last_decision = "OVERRUN_HALT" if self.effective_tokens > ceiling else "PREVENTIVE_HALT"
            self.last_decision_timestamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
            return (False, remaining, estimated)

        # Equality boundary condition: remaining >= estimated
        has_headroom = (remaining >= estimated)
        self.last_decision = "DISPATCH_ALLOWED" if has_headroom else "PREVENTIVE_HALT"
        self.last_decision_timestamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
        return (has_headroom, remaining, estimated)

    @property
    def effective_tokens(self) -> int:
        return max(self.thread_cumulative_max, self.turn_deltas_sum)


def extract_tokens_from_line(line: str) -> Optional[int]:
    """Extracts total tokens from a JSONL event line if present."""
    tracker = StreamingTokenTracker()
    res = tracker.process_line(line)
    return res if res > 0 else None


SUPPORTED_PROVIDERS: Set[str] = {"cloud", "ollama", "lmstudio", "openai", "mock", "controlled"}
SUPPORTED_SANDBOX_POLICIES: Set[str] = {"read-only", "workspace-write", "danger-full-access"}


class CodexRunner:
    def __init__(
        self,
        codex_executable: Path,
        engram_executable: Optional[Path] = None,
        model: str = "gpt-5.6-luna",
        reasoning_effort: str = "medium",
        timeout_seconds: int = 600,
        max_tokens_ceiling: int = 250_000,
        headroom_reserve: int = 45_000,
        sandbox_policy: str = "workspace-write",
        provider: str = "cloud",
        max_request_tokens: int = 60_000
    ):
        if sandbox_policy not in SUPPORTED_SANDBOX_POLICIES:
            raise ValueError(
                f"Unsupported sandbox policy '{sandbox_policy}'. "
                f"Supported values: {sorted(SUPPORTED_SANDBOX_POLICIES)}"
            )
        if provider not in SUPPORTED_PROVIDERS:
            raise ValueError(
                f"Unsupported provider '{provider}'. "
                f"Supported values: {sorted(SUPPORTED_PROVIDERS)}"
            )
        self.sandbox_policy = sandbox_policy
        self.provider = provider
        self.codex_executable = Path(codex_executable)
        self.engram_executable = Path(engram_executable) if engram_executable else None
        self.model = model
        self.reasoning_effort = reasoning_effort
        self.timeout_seconds = timeout_seconds
        self.max_tokens_ceiling = max_tokens_ceiling
        self.headroom_reserve = headroom_reserve
        self.max_request_tokens = max_request_tokens
        self.request_size_gate = RequestSizeGate(default_limit_tokens=max_request_tokens)
        self.reservation_coordinator = RequestReservationCoordinator(
            aggregate_budget=max_tokens_ceiling,
            safety_reserve=headroom_reserve
        )

        # Query version
        self.harness_version = "unknown"
        try:
            ver_proc = subprocess.run(
                [str(self.codex_executable), "--version"],
                capture_output=True,
                text=True,
                check=False
            )
            if ver_proc.returncode == 0:
                self.harness_version = ver_proc.stdout.strip()
        except Exception:
            pass

    def sanitize_path(self, base_env: Optional[Dict[str, str]] = None) -> Tuple[Dict[str, str], bool]:
        """
        Strips any directories containing Engram executables from PATH to ensure
        the control arm cannot discover or execute Engram via system PATH.
        Guarantees that ripgrep ('rg') is discoverable in the sandbox PATH for all arms
        without introducing any Engram executables.
        """
        env = (base_env or os.environ).copy()
        raw_path = env.get("PATH", "")
        paths = raw_path.split(os.pathsep)

        filtered = []
        sanitized = False
        for p in paths:
            norm_p = p.lower()
            if "engram" in norm_p:
                sanitized = True
                continue
            filtered.append(p)

        # FIX-3: Ensure rg (ripgrep) is discoverable across all benchmark sandboxes
        current_path_str = os.pathsep.join(filtered)
        if not shutil.which("rg", path=current_path_str):
            for candidate in KNOWN_RIPGREP_CANDIDATES:
                if candidate.exists():
                    cand_dir = str(candidate.parent)
                    if "engram" not in cand_dir.lower():
                        filtered.append(cand_dir)
                        break

        env["PATH"] = os.pathsep.join(filtered)
        return env, sanitized

    def build_codex_command(
        self,
        sandbox_path: Path,
        prompt: str,
        arm: str,
        engram_executable: Optional[Path] = None,
        fast_path_active: bool = False,
        task_config: Optional[Dict[str, Any]] = None
    ) -> List[str]:
        """
        Constructs the exact CLI invocation for Codex.
        Pins model, reasoning, sandbox isolation, trusted directory flags,
        and dynamically configures Engram MCP server approvals when applicable.
        """
        canonical_sandbox = Path(sandbox_path).resolve()
        sandbox_str = str(canonical_sandbox)
        esc_sandbox = sandbox_str.replace("\\", "\\\\")
        esc_sandbox_lower = sandbox_str.lower().replace("\\", "\\\\")
        fwd_sandbox = sandbox_str.replace("\\", "/")
        fwd_sandbox_lower = sandbox_str.lower().replace("\\", "/")

        cmd = [
            str(self.codex_executable),
            "exec",
            "--json",
        ]

        if self.provider == "cloud":
            cmd.extend([
                "-m", self.model,
                "-c", f'collaboration_mode.settings.reasoning_effort="{self.reasoning_effort}"',
            ])
        elif self.provider in ("ollama", "lmstudio"):
            cmd.extend([
                "--oss",
                "--local-provider", self.provider,
                "-c", f'model_provider="{self.provider}"',
                "-m", self.model,
            ])
            # For local providers, omit unsupported reasoning configuration unless independently verified

        cmd.extend([
            "-s", self.sandbox_policy,
        ])
        if self.sandbox_policy != "read-only":
            cmd.extend(["--add-dir", sandbox_str])
        cmd.extend([
            "-C", sandbox_str,
            "-c", f'projects."{esc_sandbox}".trust_level="trusted"',
            "-c", f'projects."{esc_sandbox_lower}".trust_level="trusted"',
            "-c", f'projects."{fwd_sandbox}".trust_level="trusted"',
            "-c", f'projects."{fwd_sandbox_lower}".trust_level="trusted"',
        ])

        # Dynamic per-invocation MCP configuration
        is_native_arm = arm in ("control_A", "baseline_arm1", "arm_a_native")
        if is_native_arm and engram_executable is not None:
            raise SafetyViolationError(
                f"FATAL: Native arm '{arm}' received an engram_executable ({engram_executable}). "
                f"Native arms must NOT have any MCP server configured."
            )

        is_engram_arm = False
        enabled_tools = None

        if arm in (
            "treatment_B", "v151_arm2", "v154_arm3", "arm_2_v151", "arm_3_v154",
            "arm_b_cand3", "arm_b_cand3_always_on", "arm_c_cand4_always_on",
            "arm_c_cand4_1_always_on", "arm_c_cand4_2_always_on",
            "arm_c_cand42_always_on"
        ):
            is_engram_arm = True
            enabled_tools = ["engram_get_context", "engram_inspect_symbol", "engram_preflight", "engram_postflight"]
        elif arm in (
            "arm_d_cand4_adaptive", "adaptive_cand4",
            "arm_d_cand4_1_adaptive", "arm_d_cand4_2_adaptive",
            "arm_d_cand42_adaptive", "arm_e_cand5_optimized"
        ):
            try:
                from .classifier import TaskComplexityClassifier
            except (ImportError, ValueError):
                from classifier import TaskComplexityClassifier
            override = task_config.get("adaptive_override") if task_config else None
            cl_res = TaskComplexityClassifier.classify(prompt, repo_path=canonical_sandbox, explicit_override=override)
            if cl_res.routed_mode == "BYPASS":
                is_engram_arm = False
            elif cl_res.routed_mode == "LEAN":
                is_engram_arm = True
                enabled_tools = ["engram_get_context", "engram_why"]
            elif cl_res.routed_mode == "FULL":
                is_engram_arm = True
                enabled_tools = ["engram_get_context", "engram_inspect_symbol", "engram_preflight", "engram_postflight"]

        # Always-On arms must retain active MCP tools regardless of fast_path_active
        is_always_on = arm in (
            "arm_b_cand3", "arm_b_cand3_always_on", "arm_c_cand4_always_on",
            "arm_c_cand4_1_always_on", "arm_c_cand4_2_always_on",
            "arm_c_cand42_always_on"
        )
        bypass_mcp = fast_path_active and not is_always_on

        if is_engram_arm and not bypass_mcp:
            active_bin = engram_executable or self.engram_executable
            if not active_bin or not active_bin.exists():
                raise FileNotFoundError(f"Engram executable missing: {active_bin}")

            # Ensure deterministic .codex/config.toml exists in sandbox root
            codex_cfg = canonical_sandbox / ".codex" / "config.toml"
            if not codex_cfg.exists():
                try:
                    from .isolation import configure_sandbox_codex_mcp
                    configure_sandbox_codex_mcp(canonical_sandbox, active_bin)
                except Exception:
                    pass

            fwd_bin = str(active_bin.resolve()).replace("\\", "/")
            cmd.extend([
                "-c", f'mcp_servers.engram.command="{fwd_bin}"',
                "-c", f'mcp_servers.engram.args=["-C", "{fwd_sandbox}", "mcp"]',
                "-c", 'mcp_servers.engram.default_tools_approval_mode="approve"',
            ])
            if enabled_tools:
                tools_json = json.dumps(enabled_tools)
                cmd.extend([
                    "-c", f'mcp_servers.engram.enabled_tools={tools_json}',
                ])
                for t in enabled_tools:
                    cmd.extend([
                        "-c", f'mcp_servers.engram.tools.{t}.approval_mode="approve"',
                    ])

        # Enforce MCP registration invariants
        has_mcp = any("mcp_servers.engram" in arg for arg in cmd)
        if is_native_arm and has_mcp:
            raise SafetyViolationError(
                f"FATAL: Native arm '{arm}' unexpectedly configured with MCP command arguments: {cmd}"
            )
        if is_engram_arm and not bypass_mcp and not has_mcp:
            raise SafetyViolationError(
                f"FATAL: Candidate arm '{arm}' expected active MCP registration, but no MCP configuration was generated."
            )

        cmd.append(prompt)
        return cmd

    def run(
        self,
        sandbox_path: Path,
        prompt: str,
        arm: str,  # "control_A", "treatment_B", "baseline_arm1", "v151_arm2", "v154_arm3"
        output_events_path: Path,
        is_dry_run: bool = False,
        engram_executable: Optional[Path] = None,
        task_config: Optional[Dict[str, Any]] = None
    ) -> RunExecutionResult:
        """
        Executes a single benchmark run arm inside the specified sandbox.
        """
        if self.provider in ("openai", "mock", "controlled"):
            try:
                from agent_runtime import ControlledRunner
            except (ImportError, ValueError):
                from .agent_runtime import ControlledRunner
            c_runner = ControlledRunner(
                model=self.model,
                reasoning_effort=self.reasoning_effort,
                timeout_seconds=self.timeout_seconds,
                max_tokens_ceiling=self.max_tokens_ceiling,
                headroom_reserve=self.headroom_reserve,
                sandbox_policy=self.sandbox_policy,
                provider=self.provider,
                max_request_tokens=self.max_request_tokens,
                engram_executable=self.engram_executable
            )
            return c_runner.run(
                sandbox_path=sandbox_path,
                prompt=prompt,
                arm=arm,
                output_events_path=output_events_path,
                is_dry_run=is_dry_run,
                engram_executable=engram_executable,
                task_config=task_config
            )

        started_dt = datetime.datetime.now(datetime.timezone.utc)
        start_time = time.time()
        started_at = started_dt.isoformat()

        output_events_path.parent.mkdir(parents=True, exist_ok=True)
        child_env, path_sanitized = self.sanitize_path()

        # Resolve sandbox path
        canonical_sandbox = Path(sandbox_path).resolve()
        sandbox_str = str(canonical_sandbox)
        esc_sandbox = sandbox_str.replace("\\", "\\\\")
        esc_sandbox_lower = sandbox_str.lower().replace("\\", "\\\\")
        fwd_sandbox = sandbox_str.replace("\\", "/")
        fwd_sandbox_lower = sandbox_str.lower().replace("\\", "/")

        # Determine whether this is an Engram arm and check for Fast Path eligibility
        is_engram_arm = arm in ("treatment_B", "v151_arm2", "v154_arm3", "arm_2_v151", "arm_3_v154", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive", "adaptive_cand4")
        fast_path_active = False
        adaptive_activation = None
        minimal_contract = None

        if arm in ("arm_d_cand4_adaptive", "adaptive_cand4"):
            try:
                try:
                    from .classifier import TaskComplexityClassifier
                except (ImportError, ValueError):
                    from classifier import TaskComplexityClassifier
                override = task_config.get("adaptive_override") if task_config else None
                cl_res = TaskComplexityClassifier.classify(prompt, repo_path=canonical_sandbox, explicit_override=override)
                adaptive_activation = cl_res.to_dict()
                routed_mode = cl_res.routed_mode
                if routed_mode == "BYPASS":
                    is_engram_arm = False
                    target_files = []
                    if task_config and task_config.get("target_files"):
                        target_files = task_config["target_files"]
                    elif cl_res.signals.get("files_mentioned"):
                        target_files = cl_res.signals["files_mentioned"]
                    target_str = ", ".join(target_files) if target_files else "specified target file"
                    minimal_contract = (
                        "[Engram Contract: BYPASS]\n"
                        f"- Target: {target_str}\n"
                        "- Action: Direct edit. Do not load auxiliary skills, test suites, or package managers.\n"
                        "- Verification: Perform the task's required acceptance checks. Avoid unrelated exploratory testing and unnecessary tool or package-manager usage."
                    )
                elif routed_mode == "LEAN":
                    is_engram_arm = True
                    minimal_contract = (
                        "[Engram Contract: LEAN]\n"
                        "- Directive: Bounded read-only context tools available (`engram_get_context`, `engram_why`).\n"
                        "- Verification: Confine edits strictly to authorized targets; verify with `git diff`."
                    )
                elif routed_mode == "FULL":
                    is_engram_arm = True
            except Exception as e:
                adaptive_activation = {"error": str(e), "routed_mode": "FULL"}

        # Only apply legacy fast path to legacy arms (treatment_B, etc.), not always-on or adaptive arms
        if is_engram_arm and arm not in ("arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive", "adaptive_cand4"):
            try:
                from classifier import classify_task_efficiency
                classification = classify_task_efficiency(
                    prompt=prompt,
                    task_config=task_config,
                    workspace_root=canonical_sandbox
                )
                if classification.is_fast_path:
                    fast_path_active = True
            except Exception:
                fast_path_active = False

        # Turn-0 Context Injection & Decoupled Task Contract
        turn0_injected = False
        turn0_context = None
        turn0_context_bytes = 0
        turn0_context_tokens = 0
        turn0_context_sha256 = None
        effective_prompt = prompt

        active_bin = engram_executable or self.engram_executable

        if minimal_contract:
            turn0_context = minimal_contract
            turn0_injected = True
            turn0_context_bytes = len(turn0_context.encode("utf-8"))
            turn0_context_tokens = max(1, turn0_context_bytes // 4)
            turn0_context_sha256 = hashlib.sha256(turn0_context.encode("utf-8")).hexdigest()
            effective_prompt = f"{turn0_context}\n\nTask Instructions:\n{prompt}"
        elif arm in ("v154_arm3", "arm_3_v154", "treatment_B", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive", "adaptive_cand4") and not fast_path_active:
            if active_bin and active_bin.exists():
                try:
                    inject_prompt = prompt
                    if task_config and task_config.get("target_files"):
                        targets = task_config["target_files"]
                        inject_prompt = f"{prompt} Planned files: {', '.join(targets)}"
                    subprocess.run(
                        [str(active_bin), "inject", "-C", sandbox_str, inject_prompt],
                        cwd=sandbox_str,
                        capture_output=True,
                        text=True,
                        check=False,
                        timeout=30
                    )
                    inst_file = canonical_sandbox / ".github" / "copilot-instructions.md"
                    if inst_file.exists():
                        turn0_context = inst_file.read_text(encoding="utf-8", errors="ignore")
                        turn0_injected = True
                        turn0_context_bytes = len(turn0_context.encode("utf-8"))
                        turn0_context_tokens = max(1, turn0_context_bytes // 4)
                        turn0_context_sha256 = hashlib.sha256(turn0_context.encode("utf-8")).hexdigest()
                except Exception:
                    pass

            if is_dry_run and not turn0_context:
                turn0_context = (
                    "<!-- BEGIN ENGRAM MANAGED BLOCK -->\n"
                    "## Engram Directives\n"
                    "- Active Change Contract: `ctr_dry_run_contract` (locked to `1c285608`)\n"
                    "- Directives: Contract locked. Use `engram_inspect_symbol` for symbol lookups; call `engram_postflight`.\n"
                    "<!-- END ENGRAM MANAGED BLOCK -->"
                )
                turn0_injected = True
                turn0_context_bytes = len(turn0_context.encode("utf-8"))
                turn0_context_tokens = max(1, turn0_context_bytes // 4)
                turn0_context_sha256 = hashlib.sha256(turn0_context.encode("utf-8")).hexdigest()

            if turn0_context:
                effective_prompt = f"{turn0_context}\n\nTask Instructions:\n{prompt}"

            # Append shell retrieval guidance for candidate arm
            effective_prompt = f"{effective_prompt}{SHELL_RETRIEVAL_GUIDANCE}"

        # Build command with explicit model, reasoning, and canonical sandbox settings
        cmd = self.build_codex_command(
            sandbox_path=sandbox_path,
            prompt=effective_prompt,
            arm=arm,
            engram_executable=active_bin if is_engram_arm else None,
            fast_path_active=fast_path_active,
            task_config=task_config
        )

        # Workstream A: Defensible Pre-Dispatch Request-Size Protection Gate
        gate_passed, req_tokens, req_bytes, count_method, gate_err, req_breakdown = (
            self.request_size_gate.check_pre_dispatch(
                prompt=effective_prompt,
                cmd_args=cmd,
                sandbox_path=canonical_sandbox,
                limit_tokens=self.max_request_tokens
            )
        )

        if not gate_passed:
            elapsed = round(time.time() - start_time, 2)
            completed_dt = datetime.datetime.now(datetime.timezone.utc)
            completed_at = completed_dt.isoformat()

            # Write rejection event to events log
            with open(output_events_path, "w", encoding="utf-8") as f:
                f.write(json.dumps({
                    "type": "pre_dispatch_request_gate_violation",
                    "timestamp": completed_at,
                    "measured_request_tokens": req_tokens,
                    "measured_request_bytes": req_bytes,
                    "limit_tokens": self.max_request_tokens,
                    "counting_method": count_method,
                    "breakdown": req_breakdown,
                    "error_message": gate_err
                }) + "\n")

            return RunExecutionResult(
                exit_code=105,
                elapsed_seconds=elapsed,
                started_at=started_at,
                completed_at=completed_at,
                events_log_path=str(output_events_path),
                thread_id=None,
                model_name=self.model,
                reasoning_effort=self.reasoning_effort,
                harness_executable=str(self.codex_executable),
                harness_version=self.harness_version,
                path_sanitized=path_sanitized,
                error_message=gate_err,
                turn0_injected=turn0_injected,
                initial_prompt=effective_prompt,
                fast_path_active=fast_path_active,
                pre_dispatch_guard_triggered=True,
                stop_reason="PRE_DISPATCH_REQUEST_SIZE_EXCEEDED",
                headroom_reserve=self.headroom_reserve,
                cumulative_tokens_at_stop=0,
                estimated_next_turn_cost=req_tokens,
                configured_ceiling=self.max_tokens_ceiling,
                remaining_budget_at_stop=self.max_tokens_ceiling,
                last_turn_usage_at_stop=0,
                guard_decision="REJECT_PRE_DISPATCH_REQUEST_SIZE_EXCEEDED",
                guard_decision_timestamp=completed_at,
                relevant_event_id=None,
                telemetry_valid=True,
                telemetry_status="VALID",
                sandbox_policy=self.sandbox_policy,
                provider=self.provider,
                adaptive_activation=adaptive_activation,
                request_size_gate_passed=False,
                measured_request_tokens=req_tokens,
                measured_request_bytes=req_bytes,
                request_counting_method=count_method,
                configured_request_size_limit=self.max_request_tokens,
                request_size_breakdown=req_breakdown,
                reservation_id=None,
                reservation_details=None
            )

        # Workstream B/D: Synchronous Pre-Dispatch Capacity Reservation Gate
        req_id = f"req-{arm}-{int(start_time)}"
        res_ok, res_err, reservation = self.reservation_coordinator.reserve_request(
            request_id=req_id,
            turn_index=0,
            input_tokens=req_tokens
        )
        if not res_ok and not is_dry_run:
            elapsed = round(time.time() - start_time, 2)
            completed_dt = datetime.datetime.now(datetime.timezone.utc)
            completed_at = completed_dt.isoformat()

            with open(output_events_path, "w", encoding="utf-8") as f:
                f.write(json.dumps({
                    "type": "pre_dispatch_budget_reservation_violation",
                    "timestamp": completed_at,
                    "measured_request_tokens": req_tokens,
                    "measured_request_bytes": req_bytes,
                    "error_message": res_err,
                    "remaining_budget": self.reservation_coordinator.remaining_budget,
                    "safety_reserve": self.reservation_coordinator.safety_reserve
                }) + "\n")

            return RunExecutionResult(
                exit_code=106,
                elapsed_seconds=elapsed,
                started_at=started_at,
                completed_at=completed_at,
                events_log_path=str(output_events_path),
                thread_id=None,
                model_name=self.model,
                reasoning_effort=self.reasoning_effort,
                harness_executable=str(self.codex_executable),
                harness_version=self.harness_version,
                path_sanitized=path_sanitized,
                error_message=res_err,
                turn0_injected=turn0_injected,
                initial_prompt=effective_prompt,
                fast_path_active=fast_path_active,
                pre_dispatch_guard_triggered=True,
                stop_reason="PRE_DISPATCH_BUDGET_RESERVATION_EXCEEDED",
                headroom_reserve=self.headroom_reserve,
                cumulative_tokens_at_stop=self.reservation_coordinator.cumulative_reconciled_tokens,
                estimated_next_turn_cost=req_tokens,
                configured_ceiling=self.max_tokens_ceiling,
                remaining_budget_at_stop=self.reservation_coordinator.remaining_budget,
                last_turn_usage_at_stop=0,
                guard_decision="REJECT_PRE_DISPATCH_BUDGET_RESERVATION_EXCEEDED",
                guard_decision_timestamp=completed_at,
                relevant_event_id=None,
                telemetry_valid=True,
                telemetry_status="VALID",
                sandbox_policy=self.sandbox_policy,
                provider=self.provider,
                adaptive_activation=adaptive_activation,
                request_size_gate_passed=True,
                measured_request_tokens=req_tokens,
                measured_request_bytes=req_bytes,
                request_counting_method=count_method,
                configured_request_size_limit=self.max_request_tokens,
                request_size_breakdown=req_breakdown,
                reservation_id=req_id,
                reservation_details=None
            )

        if is_dry_run:
            time.sleep(0.1)
            elapsed = round(time.time() - start_time, 2)
            completed_dt = datetime.datetime.now(datetime.timezone.utc)
            completed_at = completed_dt.isoformat()

            thread_id = f"dry-run-{arm}-thread"
            self._write_dry_run_events(
                output_events_path,
                arm,
                thread_id,
                initial_prompt=effective_prompt,
                fast_path_active=fast_path_active
            )

            # Reconcile reservation with dry run synthetic tokens (5,744)
            if reservation:
                self.reservation_coordinator.reconcile_response(
                    request_id=reservation.request_id,
                    actual_total_tokens=5744
                )

            return RunExecutionResult(
                exit_code=0,
                elapsed_seconds=elapsed,
                started_at=started_at,
                completed_at=completed_at,
                events_log_path=str(output_events_path),
                thread_id=thread_id,
                model_name=self.model,
                reasoning_effort=self.reasoning_effort,
                harness_executable=str(self.codex_executable),
                harness_version=self.harness_version,
                path_sanitized=path_sanitized,
                error_message=None,
                turn0_injected=turn0_injected,
                initial_prompt=effective_prompt,
                fast_path_active=fast_path_active,
                pre_dispatch_guard_triggered=False,
                stop_reason=None,
                headroom_reserve=self.headroom_reserve,
                cumulative_tokens_at_stop=None,
                estimated_next_turn_cost=None,
                configured_ceiling=self.max_tokens_ceiling,
                remaining_budget_at_stop=None,
                last_turn_usage_at_stop=None,
                guard_decision="DISPATCH_ALLOWED",
                guard_decision_timestamp=completed_at,
                relevant_event_id=None,
                telemetry_valid=True,
                telemetry_status="VALID",
                sandbox_policy=self.sandbox_policy,
                provider=self.provider,
                adaptive_activation=adaptive_activation,
                request_size_gate_passed=True,
                measured_request_tokens=req_tokens,
                measured_request_bytes=req_bytes,
                request_counting_method=count_method,
                configured_request_size_limit=self.max_request_tokens,
                request_size_breakdown=req_breakdown,
                reservation_id=reservation.request_id if reservation else None,
                reservation_details=reservation.to_dict() if reservation else None
            )

        exit_code = -1
        error_msg = None
        thread_id = None
        proc = None
        stdout_lines: List[str] = []
        stderr_lines: List[str] = []
        timed_out = False
        token_ceiling_breached = False
        native_rollout_path = None
        current_cumulative_tokens = 0
        pre_dispatch_guard_triggered = False

        try:
            # stdin=subprocess.DEVNULL is mandatory to prevent codex from waiting for EOF on stdin
            proc = subprocess.Popen(
                cmd,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                encoding="utf-8",
                errors="replace",
                cwd=sandbox_str,
                env=child_env
            )

            def _read_stream(stream, target_list):
                try:
                    for line in iter(stream.readline, ""):
                        if not line:
                            break
                        target_list.append(line)
                except Exception:
                    pass

            t_stdout = threading.Thread(target=_read_stream, args=(proc.stdout, stdout_lines), daemon=True)
            t_stderr = threading.Thread(target=_read_stream, args=(proc.stderr, stderr_lines), daemon=True)
            t_stdout.start()
            t_stderr.start()

            token_tracker = StreamingTokenTracker(conservative_reserve=self.headroom_reserve)
            last_checked_idx = 0
            rollout_fp = None
            active_rollout_path = None
            stop_reason = None

            while proc.poll() is None:
                # 50ms responsive polling interval for child process monitoring.
                # NOTE ON RESIDUAL CONCURRENCY RISK:
                # External polling monitors stdout and rollout events. When an intermediate tool
                # call completes, Codex formats the result and dispatches the subsequent model request.
                # If network dispatch occurs in <50ms before the poll cycle executes, an outbound
                # request could be in flight when the child tree is terminated. Deterministic zero-window
                # interception would require an internal Codex pre-request hook or synchronous middleware.
                time.sleep(0.05)
                elapsed = time.time() - start_time

                # 1. Hard Timeout Enforcement
                if elapsed >= self.timeout_seconds:
                    timed_out = True
                    break

                # 2. Hard Token Ceiling and Pre-Dispatch Headroom Enforcement across streaming stdout lines
                while last_checked_idx < len(stdout_lines):
                    line = stdout_lines[last_checked_idx]
                    last_checked_idx += 1
                    token_tracker.process_line(line)
                    # Extract thread_id early if emitted
                    if not thread_id and ('"type":"thread.started"' in line or '"type": "thread.started"' in line):
                        try:
                            ev = json.loads(line)
                            thread_id = ev.get("thread_id")
                        except Exception:
                            pass
                    # If actual usage already breached ceiling, prioritize hard breach
                    if token_tracker.effective_tokens > self.max_tokens_ceiling:
                        token_ceiling_breached = True
                        break
                    # Pre-dispatch headroom check on pre-dispatch boundaries
                    if token_tracker.is_pre_dispatch_boundary(line):
                        has_headroom, remaining, estimated = token_tracker.check_pre_dispatch_headroom(self.max_tokens_ceiling)
                        if not has_headroom:
                            pre_dispatch_guard_triggered = True
                            break
                if token_ceiling_breached or pre_dispatch_guard_triggered:
                    break

                # Dynamic rollout discovery & live tailing (if running against real Codex sessions)
                if not rollout_fp:
                    try:
                        from telemetry import TelemetryParser, CODEX_SESSIONS_DIR
                        t_parser = TelemetryParser(sessions_dir=CODEX_SESSIONS_DIR)
                        cand = t_parser.find_session_rollout(
                            thread_id=thread_id,
                            sandbox_path=sandbox_path,
                            min_mtime=start_time
                        )
                        if cand and cand.exists():
                            active_rollout_path = cand
                            rollout_fp = open(cand, "r", encoding="utf-8", errors="replace")
                    except Exception:
                        pass

                # Read newly appended lines from active rollout file
                if rollout_fp:
                    try:
                        while True:
                            r_line = rollout_fp.readline()
                            if not r_line:
                                break
                            token_tracker.process_line(r_line)
                            if token_tracker.effective_tokens > self.max_tokens_ceiling:
                                token_ceiling_breached = True
                                break
                            if token_tracker.is_pre_dispatch_boundary(r_line):
                                has_headroom, remaining, estimated = token_tracker.check_pre_dispatch_headroom(self.max_tokens_ceiling)
                                if not has_headroom:
                                    pre_dispatch_guard_triggered = True
                                    break
                    except Exception:
                        pass
                if token_ceiling_breached or pre_dispatch_guard_triggered:
                    break

                current_cumulative_tokens = token_tracker.effective_tokens
                if current_cumulative_tokens > self.max_tokens_ceiling:
                    token_ceiling_breached = True
                    break

            # Close live rollout handle if open
            if rollout_fp:
                try:
                    rollout_fp.close()
                except Exception:
                    pass

            # Terminate child tree if limit breached, guard triggered, or timed out
            if timed_out or pre_dispatch_guard_triggered or token_ceiling_breached:
                if proc:
                    terminate_pid_tree(proc.pid)
                    try:
                        proc.kill()
                        proc.wait(timeout=1.0)
                    except Exception:
                        pass
                    try:
                        if proc.stdout:
                            proc.stdout.close()
                        if proc.stderr:
                            proc.stderr.close()
                    except Exception:
                        pass
                exit_code = -1
                current_cumulative_tokens = token_tracker.effective_tokens
                if pre_dispatch_guard_triggered:
                    stop_reason = "PRE_DISPATCH_HEADROOM_EXHAUSTED"
                    # CRITICAL FIX 2 INVARIANT:
                    # If cumulative tokens at stop are <= ceiling, token_ceiling_breached MUST be False.
                    # Only if cumulative tokens actually exceeded the ceiling is token_ceiling_breached True.
                    token_ceiling_breached = (current_cumulative_tokens > self.max_tokens_ceiling)
                    _, rem, est = token_tracker.check_pre_dispatch_headroom(self.max_tokens_ceiling)
                    error_msg = f"Execution halted by pre-dispatch token headroom guard: {rem:,} tokens remaining < estimated next turn cost of {est:,} tokens (budget: {self.max_tokens_ceiling:,})"
                elif timed_out:
                    stop_reason = "TIMEOUT"
                    error_msg = f"Execution timed out after {self.timeout_seconds} seconds (child tree terminated safely)"
                elif not token_tracker.telemetry_valid:
                    stop_reason = "TELEMETRY_INVALID"
                    error_msg = f"Execution halted: Token telemetry is invalid or corrupted ({token_tracker.telemetry_error})"
                else:
                    stop_reason = "TOKEN_CEILING_BREACHED"
                    token_ceiling_breached = True
                    error_msg = f"Execution terminated: Token ceiling exceeded ({current_cumulative_tokens:,} > {self.max_tokens_ceiling:,} tokens)"
            else:
                exit_code = proc.returncode
                # Check tokens in any trailing lines flushed upon completion
                while last_checked_idx < len(stdout_lines):
                    line = stdout_lines[last_checked_idx]
                    last_checked_idx += 1
                    token_tracker.process_line(line)

                # Check trailing lines in rollout file if available
                if active_rollout_path and active_rollout_path.exists():
                    try:
                        with open(active_rollout_path, "r", encoding="utf-8", errors="replace") as f_tail:
                            for r_line in f_tail:
                                token_tracker.process_line(r_line)
                    except Exception:
                        pass

                current_cumulative_tokens = token_tracker.effective_tokens
                if current_cumulative_tokens > self.max_tokens_ceiling:
                    token_ceiling_breached = True
                    stop_reason = "TOKEN_CEILING_BREACHED"
                    exit_code = -1
                    error_msg = f"Execution completed but breached token ceiling ({current_cumulative_tokens:,} > {self.max_tokens_ceiling:,} tokens)"
                elif not token_tracker.telemetry_valid:
                    stop_reason = "TELEMETRY_INVALID"
                    exit_code = -1
                    error_msg = f"Execution completed with invalid token telemetry ({token_tracker.telemetry_error})"
                elif exit_code != 0:
                    if exit_code < 0 or exit_code in (3221225477, 3221225786, 3221226505):
                        stop_reason = "PROCESS_CRASH"
                        error_msg = f"Codex process crashed with code {exit_code}: {''.join(stderr_lines).strip()}"
                    else:
                        stop_reason = "PROCESS_ERROR"
                        error_msg = f"Codex exited with code {exit_code}: {''.join(stderr_lines).strip()}"
                else:
                    stop_reason = None

            # Wait briefly for reader threads to finish reading remaining buffer
            t_stdout.join(timeout=2.0)
            t_stderr.join(timeout=2.0)

            # Preserve all captured stdout lines to output_events_path (Never drop partial output)
            stdout_data = "".join(stdout_lines)
            if stdout_data:
                with open(output_events_path, "w", encoding="utf-8") as f:
                    f.write(stdout_data)

            # Extract thread_id from stdout if available
            if not thread_id:
                for line in stdout_lines:
                    if '"type":"thread.started"' in line or '"type": "thread.started"' in line:
                        try:
                            ev = json.loads(line)
                            thread_id = ev.get("thread_id")
                            break
                        except Exception:
                            pass

            # Discover and link native session rollout from .codex/sessions
            try:
                from telemetry import TelemetryParser, CODEX_SESSIONS_DIR
                parser = TelemetryParser(sessions_dir=CODEX_SESSIONS_DIR)
                found_rollout = active_rollout_path or parser.find_session_rollout(
                    thread_id=thread_id,
                    sandbox_path=sandbox_path,
                    min_mtime=start_time
                )
                if found_rollout:
                    native_rollout_path = str(found_rollout)
                    # If output_events_path is empty or missing, recover directly from native rollout
                    if not output_events_path.exists() or output_events_path.stat().st_size == 0:
                        shutil.copy2(found_rollout, output_events_path)
            except Exception:
                pass

        except Exception as e:
            if proc:
                terminate_pid_tree(proc.pid)
                try:
                    proc.kill()
                except Exception:
                    pass
            exit_code = -1
            error_msg = str(e)

        elapsed = round(time.time() - start_time, 2)
        completed_dt = datetime.datetime.now(datetime.timezone.utc)
        completed_at = completed_dt.isoformat()

        if reservation:
            actual_toks = current_cumulative_tokens if token_tracker and token_tracker.telemetry_valid else None
            try:
                self.reservation_coordinator.reconcile_response(
                    request_id=reservation.request_id,
                    actual_total_tokens=actual_toks
                )
            except Exception:
                pass

        return RunExecutionResult(
            exit_code=exit_code,
            elapsed_seconds=elapsed,
            started_at=started_at,
            completed_at=completed_at,
            events_log_path=str(output_events_path),
            thread_id=thread_id,
            model_name=self.model,
            reasoning_effort=self.reasoning_effort,
            harness_executable=str(self.codex_executable),
            harness_version=self.harness_version,
            path_sanitized=path_sanitized,
            error_message=error_msg,
            turn0_injected=turn0_injected,
            initial_prompt=effective_prompt,
            timed_out=timed_out,
            token_ceiling_breached=token_ceiling_breached,
            native_rollout_path=native_rollout_path,
            recovered_events_count=len(stdout_lines),
            fast_path_active=fast_path_active,
            pre_dispatch_guard_triggered=pre_dispatch_guard_triggered,
            stop_reason=stop_reason,
            headroom_reserve=self.headroom_reserve,
            cumulative_tokens_at_stop=current_cumulative_tokens if (pre_dispatch_guard_triggered or token_ceiling_breached) else None,
            estimated_next_turn_cost=token_tracker.estimate_next_request_tokens() if pre_dispatch_guard_triggered else None,
            configured_ceiling=self.max_tokens_ceiling,
            remaining_budget_at_stop=(self.max_tokens_ceiling - current_cumulative_tokens) if (pre_dispatch_guard_triggered or token_ceiling_breached) else None,
            last_turn_usage_at_stop=token_tracker.last_turn_request_tokens,
            guard_decision=token_tracker.last_decision,
            guard_decision_timestamp=token_tracker.last_decision_timestamp or completed_at,
            relevant_event_id=token_tracker.last_event_id,
            telemetry_valid=token_tracker.telemetry_valid,
            telemetry_status=token_tracker.telemetry_status,
            sandbox_policy=self.sandbox_policy,
            provider=self.provider,
            adaptive_activation=adaptive_activation,
            request_size_gate_passed=True,
            measured_request_tokens=req_tokens,
            measured_request_bytes=req_bytes,
            request_counting_method=count_method,
            configured_request_size_limit=self.max_request_tokens,
            request_size_breakdown=req_breakdown,
            reservation_id=reservation.request_id if reservation else None,
            reservation_details=reservation.to_dict() if reservation else None
        )

    def _write_dry_run_events(
        self,
        output_path: Path,
        arm: str,
        thread_id: str,
        initial_prompt: Optional[str] = None,
        fast_path_active: bool = False
    ) -> None:
        """Writes synthetic JSONL events for dry-run verification."""
        events = [
            {"type": "thread.started", "thread_id": thread_id},
            {"type": "turn.started"}
        ]
        if initial_prompt:
            events.append({
                "type": "item.completed",
                "item": {
                    "id": "prompt_0",
                    "type": "user_message",
                    "content": initial_prompt
                }
            })

        if fast_path_active:
            events.extend([
                {
                    "type": "item.completed",
                    "item": {
                        "id": "item_1",
                        "type": "agent_message",
                        "text": "Direct micro-edit executed via fast path without MCP overhead."
                    }
                },
                {
                    "type": "turn.completed",
                    "usage": {
                        "input_tokens": 1200,
                        "cached_input_tokens": 900,
                        "cache_write_input_tokens": 0,
                        "output_tokens": 45,
                        "reasoning_output_tokens": 5
                    }
                }
            ])
        elif arm == "arm_e_cand5_optimized":
            events.extend([
                {
                    "type": "item.completed",
                    "item": {
                        "id": "item_1",
                        "type": "mcp_tool_call",
                        "server": "engram",
                        "tool": "engram_get_context",
                        "arguments": {"task": "Task execution", "budget": 2000},
                        "result": {"content": [{"type": "text", "text": "Optimized context compiled. Seven token-efficiency passes active."}]}
                    }
                },
                {
                    "type": "turn.completed",
                    "usage": {
                        "input_tokens": 2850,
                        "cached_input_tokens": 2200,
                        "cache_write_input_tokens": 0,
                        "output_tokens": 60,
                        "reasoning_output_tokens": 8
                    }
                }
            ])
        elif arm in ("v154_arm3", "arm_3_v154", "arm_c_cand42_always_on", "arm_d_cand42_adaptive", "arm_c_cand4_2_always_on", "arm_d_cand4_2_adaptive"):
            events.extend([
                {
                    "type": "item.completed",
                    "item": {
                        "id": "item_1",
                        "type": "mcp_tool_call",
                        "server": "engram",
                        "tool": "engram_prepare_task",
                        "arguments": {"task": "Task execution", "planned_files": []},
                        "result": {"content": [{"type": "text", "text": "Task prepared atomically. Adaptive small-task bypass active."}]}
                    }
                },
                {
                    "type": "turn.completed",
                    "usage": {
                        "input_tokens": 4200,
                        "cached_input_tokens": 3500,
                        "cache_write_input_tokens": 0,
                        "output_tokens": 80,
                        "reasoning_output_tokens": 10
                    }
                }
            ])
        elif arm in ("treatment_B", "v151_arm2", "arm_2_v151", "arm_b_cand3", "arm_b_cand3_always_on"):
            events.extend([
                {
                    "type": "item.completed",
                    "item": {
                        "id": "item_1",
                        "type": "mcp_tool_call",
                        "server": "engram",
                        "tool": "engram_query",
                        "arguments": {"query": "theme configuration"},
                        "result": {"content": [{"type": "text", "text": "Relevant decisions found."}]}
                    }
                },
                {
                    "type": "turn.completed",
                    "usage": {
                        "input_tokens": 12500,
                        "cached_input_tokens": 10000,
                        "cache_write_input_tokens": 0,
                        "output_tokens": 150,
                        "reasoning_output_tokens": 20
                    }
                }
            ])
        else:
            events.extend([
                {
                    "type": "item.completed",
                    "item": {
                        "id": "item_1",
                        "type": "agent_message",
                        "text": "Inspecting repository files manually without MCP..."
                    }
                },
                {
                    "type": "turn.completed",
                    "usage": {
                        "input_tokens": 28400,
                        "cached_input_tokens": 21000,
                        "cache_write_input_tokens": 0,
                        "output_tokens": 320,
                        "reasoning_output_tokens": 45
                    }
                }
            ])

        with open(output_path, "w", encoding="utf-8") as f:
            for ev in events:
                f.write(json.dumps(ev) + "\n")
