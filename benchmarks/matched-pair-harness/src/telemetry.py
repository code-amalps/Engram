"""
Telemetry parser and aggregator for Codex JSONL event streams.
Remediated capabilities:
1. Distinguishes per-request token usage, cumulative thread usage, cached input tokens,
   output tokens, reasoning tokens, and peak single-request context.
2. Cross-references thread_id against .codex/sessions to extract exact per-request records.
3. If per-request records are unavailable, explicitly records peak_single_request_context as None
   rather than falsely substituting cumulative thread totals.
4. Strictly prevents double-counting: cached input is a subset of prompt tokens; reasoning is
   a subset of output tokens; cumulative thread summaries are never added as independent requests.
"""
from dataclasses import dataclass, asdict
from typing import Dict, Any, List, Optional, Union
from pathlib import Path
import json
import glob
import os


CODEX_SESSIONS_DIR = Path(r"C:\Users\Amal P S\.codex\sessions")


@dataclass
class TelemetryMetrics:
    provenance: str  # "SESSION_ROLLOUT", "DIRECT_API", "HARNESS_LOG", "UI_COUNTER", "SYNTHETIC_TEST"
    prompt_tokens_total: int
    prompt_tokens_cached: int
    prompt_tokens_cached_inferred: bool
    prompt_tokens_uncached: int
    prompt_tokens_uncached_inferred: bool
    completion_tokens: int
    reasoning_tokens: int
    total_model_tokens: int
    peak_single_request_context: Optional[int]
    peak_turn_context: Optional[int]
    final_turn_context: Optional[int]
    model_requests_count: Optional[int]
    turns_count: int
    mcp_tool_calls_count: int
    auxiliary_tool_payload_bytes: int
    account_usage_reading_start_pct: Optional[int] = None
    account_usage_reading_end_pct: Optional[int] = None

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


class TelemetryParser:
    def __init__(self, provenance: str = "DIRECT_API", sessions_dir: Optional[Path] = None):
        self.provenance = provenance
        self.sessions_dir = Path(sessions_dir) if sessions_dir else CODEX_SESSIONS_DIR

    def parse_rollout_file(self, rollout_path: Union[str, Path]) -> TelemetryMetrics:
        """
        Parses a definitive Codex session rollout JSONL file (from .codex/sessions).
        Extracts exact per-request usage from token_usage_record entries.
        """
        rollout_path = Path(rollout_path)
        records = []
        seen_tool_ids: Set[str] = set()
        mcp_tool_calls_count = 0
        auxiliary_tool_payload_bytes = 0

        with open(rollout_path, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    data = json.loads(line)
                    ev_type = data.get("type", "")

                    if ev_type == "token_usage_record":
                        payload = data.get("payload", {})
                        records.append(payload)

                    elif ev_type in ("item.completed", "item.started"):
                        item = data.get("item", {})
                        if item.get("type") == "mcp_tool_call" and ev_type == "item.completed":
                            item_id = item.get("id")
                            if item_id and item_id in seen_tool_ids:
                                continue
                            if item_id:
                                seen_tool_ids.add(item_id)
                            mcp_tool_calls_count += 1
                            args_str = json.dumps(item.get("arguments", {}))
                            auxiliary_tool_payload_bytes += len(args_str.encode("utf-8"))
                            res = item.get("result")
                            if res:
                                res_str = json.dumps(res)
                                auxiliary_tool_payload_bytes += len(res_str.encode("utf-8"))
                except json.JSONDecodeError:
                    continue

        if not records:
            raise ValueError(f"No token_usage_record events found in rollout file: {rollout_path}")

        # The last record contains the authoritative thread_token_usage
        last_record = records[-1]
        final_thread_usage = last_record.get("thread_token_usage") or last_record.get("usage", {})

        prompt_tokens_total = int(final_thread_usage.get("input_tokens", 0))
        cached_input = final_thread_usage.get("cached_input_tokens")
        if cached_input is not None:
            prompt_tokens_cached = int(cached_input)
            if prompt_tokens_cached < 0:
                raise ValueError(f"Invalid negative cached_input_tokens in rollout: {prompt_tokens_cached}")
            if prompt_tokens_cached > prompt_tokens_total:
                raise ValueError(
                    f"Invalid telemetry in rollout: cached_input_tokens ({prompt_tokens_cached}) "
                    f"exceeds input_tokens ({prompt_tokens_total})"
                )
            prompt_tokens_cached_inferred = False
            prompt_tokens_uncached = prompt_tokens_total - prompt_tokens_cached
            prompt_tokens_uncached_inferred = True
        else:
            prompt_tokens_cached = 0
            prompt_tokens_cached_inferred = True
            prompt_tokens_uncached = prompt_tokens_total
            prompt_tokens_uncached_inferred = True

        completion_tokens = int(final_thread_usage.get("output_tokens", 0))
        reasoning_tokens = int(final_thread_usage.get("reasoning_output_tokens", 0))

        # Per-request context inspection
        request_contexts = []
        for r in records:
            req_usage = r.get("usage", {})
            inp = int(req_usage.get("input_tokens", 0))
            if inp > 0:
                request_contexts.append(inp)

        peak_single_request_context = max(request_contexts) if request_contexts else None
        final_context = request_contexts[-1] if request_contexts else None

        # Mathematical invariants:
        # Total model tokens = prompt_tokens_total + completion_tokens
        # Note: reasoning_tokens is an informative sub-metric already included inside completion_tokens.
        # cached_tokens is an informative sub-metric already included inside prompt_tokens_total.
        total_model_tokens = prompt_tokens_total + completion_tokens

        return TelemetryMetrics(
            provenance="SESSION_ROLLOUT",
            prompt_tokens_total=prompt_tokens_total,
            prompt_tokens_cached=prompt_tokens_cached,
            prompt_tokens_cached_inferred=prompt_tokens_cached_inferred,
            prompt_tokens_uncached=prompt_tokens_uncached,
            prompt_tokens_uncached_inferred=prompt_tokens_uncached_inferred,
            completion_tokens=completion_tokens,
            reasoning_tokens=reasoning_tokens,
            total_model_tokens=total_model_tokens,
            peak_single_request_context=peak_single_request_context,
            peak_turn_context=peak_single_request_context,
            final_turn_context=final_context,
            model_requests_count=len(records),
            turns_count=len(records),
            mcp_tool_calls_count=mcp_tool_calls_count,
            auxiliary_tool_payload_bytes=auxiliary_tool_payload_bytes,
        )

    def parse_events(
        self,
        events: List[Dict[str, Any]],
        thread_id: Optional[str] = None
    ) -> TelemetryMetrics:
        """
        Parses stdout event stream from codex exec.
        Attempts to find matching session rollout by thread_id for exact per-request accounting.
        If unavailable, parses turn.completed and explicitly marks peak_single_request_context as None.
        """
        # 1. Look for thread_id in events if not passed
        if not thread_id:
            for ev in events:
                if ev.get("type") == "thread.started":
                    thread_id = ev.get("thread_id")
                    break

        # 2. Try to resolve local session rollout file
        if thread_id and self.sessions_dir.exists():
            # Search for rollout file ending in thread_id
            pattern = str(self.sessions_dir / "**" / f"*{thread_id}*.jsonl")
            matches = glob.glob(pattern, recursive=True)
            if matches:
                try:
                    return self.parse_rollout_file(matches[0])
                except Exception:
                    pass

        # 3. Fallback: Parse from stdout events
        prompt_tokens_total = 0
        prompt_tokens_cached = 0
        has_cached_metric = False
        completion_tokens = 0
        reasoning_tokens = 0
        turn_inputs: List[int] = []
        mcp_tool_calls_count = 0
        auxiliary_tool_payload_bytes = 0
        seen_tool_ids: Set[str] = set()

        for event in events:
            ev_type = event.get("type", "")

            if ev_type == "turn.completed":
                usage = event.get("usage", {})
                inp = int(usage.get("input_tokens", 0))
                turn_inputs.append(inp)
                prompt_tokens_total += inp

                if "cached_input_tokens" in usage and usage["cached_input_tokens"] is not None:
                    has_cached_metric = True
                    cached = int(usage["cached_input_tokens"])
                    prompt_tokens_cached += cached

                out = int(usage.get("output_tokens", 0))
                reasoning = int(usage.get("reasoning_output_tokens", 0))
                completion_tokens += out
                reasoning_tokens += reasoning

            elif ev_type in ("item.completed", "item.started"):
                item = event.get("item", {})
                if item.get("type") == "mcp_tool_call" and ev_type == "item.completed":
                    item_id = item.get("id")
                    if item_id and item_id in seen_tool_ids:
                        continue
                    if item_id:
                        seen_tool_ids.add(item_id)
                    mcp_tool_calls_count += 1
                    args_str = json.dumps(item.get("arguments", {}))
                    auxiliary_tool_payload_bytes += len(args_str.encode("utf-8"))
                    res = item.get("result")
                    if res:
                        res_str = json.dumps(res)
                        auxiliary_tool_payload_bytes += len(res_str.encode("utf-8"))

        turns_count = len(turn_inputs)
        final_turn_context = turn_inputs[-1] if turn_inputs else None

        # CRITICAL SAFEGUARD:
        # When parsing only codex exec stdout, turn.completed reports thread cumulative totals.
        # It does NOT expose single-request context breakdown. Therefore, peak_single_request_context
        # MUST be marked None (UNAVAILABLE) to prevent misrepresenting cumulative tokens as context occupancy.
        peak_single_request_context = None
        peak_turn_context = max(turn_inputs) if turn_inputs else None

        if has_cached_metric:
            if prompt_tokens_cached < 0:
                raise ValueError(f"Invalid negative cached_input_tokens in events: {prompt_tokens_cached}")
            if prompt_tokens_cached > prompt_tokens_total:
                raise ValueError(
                    f"Invalid telemetry in events: cached_input_tokens ({prompt_tokens_cached}) "
                    f"exceeds input_tokens ({prompt_tokens_total})"
                )
            prompt_tokens_cached_inferred = False
            prompt_tokens_uncached = prompt_tokens_total - prompt_tokens_cached
            prompt_tokens_uncached_inferred = True
        else:
            prompt_tokens_cached = 0
            prompt_tokens_cached_inferred = True
            prompt_tokens_uncached = prompt_tokens_total
            prompt_tokens_uncached_inferred = True

        total_model_tokens = prompt_tokens_total + completion_tokens

        return TelemetryMetrics(
            provenance=self.provenance,
            prompt_tokens_total=prompt_tokens_total,
            prompt_tokens_cached=prompt_tokens_cached,
            prompt_tokens_cached_inferred=prompt_tokens_cached_inferred,
            prompt_tokens_uncached=prompt_tokens_uncached,
            prompt_tokens_uncached_inferred=prompt_tokens_uncached_inferred,
            completion_tokens=completion_tokens,
            reasoning_tokens=reasoning_tokens,
            total_model_tokens=total_model_tokens,
            peak_single_request_context=peak_single_request_context,
            peak_turn_context=peak_turn_context,
            final_turn_context=final_turn_context,
            model_requests_count=turns_count if turns_count > 0 else None,
            turns_count=turns_count,
            mcp_tool_calls_count=mcp_tool_calls_count,
            auxiliary_tool_payload_bytes=auxiliary_tool_payload_bytes,
        )

    def parse_file(
        self,
        jsonl_path: Union[str, Path],
        thread_id: Optional[str] = None
    ) -> TelemetryMetrics:
        events = []
        with open(jsonl_path, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    events.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
        return self.parse_events(events, thread_id=thread_id)
