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
from typing import Dict, Any, List, Optional, Union, Set
from pathlib import Path
import json
import glob
import time
import datetime
import os
import re


CODEX_SESSIONS_DIR = Path(r"C:\Users\Amal P S\.codex\sessions")


@dataclass
class TokenReplacementAccounting:
    files_retrieved_via_engram: List[str]
    files_read_via_shell: List[str]
    files_repeated_read: List[str]
    engram_excerpts_count: int
    engram_excerpts_reread_count: int
    serialized_invocations_bytes: int
    serialized_responses_bytes: int
    estimated_payload_tokens: int
    replacement_status: str  # "MEASURED", "ESTIMATED", "UNKNOWN", "NOT_OBSERVABLE"
    tokens_per_successful_task: Optional[float] = None

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


def extract_files_from_shell_command(cmd: str) -> List[str]:
    files = []
    if not isinstance(cmd, str):
        return files
    patterns = [
        r'(?:cat|type|Get-Content|head|tail|more|less)\s+(?:-[a-zA-Z0-9_-]+\s+)*["\']?([^"\'\r\n\t ;&|><]+)["\']?',
    ]
    for p in patterns:
        for m in re.finditer(p, cmd, re.IGNORECASE):
            path_str = m.group(1).strip()
            if path_str and not path_str.startswith("-") and ("/" in path_str or "\\" in path_str or "." in path_str):
                files.append(path_str.replace("\\", "/"))
    return files


def extract_files_from_obj(obj: Any) -> List[str]:
    files = []
    if isinstance(obj, str):
        matches = re.findall(r'(?:[a-zA-Z0-9_\-\./\\]+\.[a-zA-Z0-9_]{1,10})', obj)
        for m in matches:
            if "/" in m or "\\" in m:
                files.append(m.replace("\\", "/"))
    elif isinstance(obj, dict):
        for k, v in obj.items():
            if k in ("path", "file", "file_path", "filename", "target_file") and isinstance(v, str):
                files.append(v.replace("\\", "/"))
            else:
                files.extend(extract_files_from_obj(v))
    elif isinstance(obj, list):
        for item in obj:
            files.extend(extract_files_from_obj(item))
    return files


def process_item_for_replacement(
    item: Dict[str, Any],
    seen_tool_ids: Set[str],
    state: Dict[str, Any]
):
    raw_type = item.get("type", "")
    norm_type = raw_type.lower()
    name = (item.get("name") or "").lower()
    server = (item.get("server") or "").lower()
    input_str = str(item.get("input") or item.get("arguments") or "")

    is_exec = (name == "exec")
    is_engram = ("engram" in norm_type or "engram" in name or server == "engram" or "mcp__engram" in input_str)
    is_mcp = not is_exec and (norm_type in ("mcp_tool_call", "mcptoolcall", "custom_tool_call") or is_engram)

    # MCP / Engram tool calls
    if is_mcp:
        item_id = item.get("id") or item.get("call_id")
        if item_id and item_id in seen_tool_ids:
            return
        if item_id:
            seen_tool_ids.add(item_id)
        state["mcp_tool_calls_count"] += 1

        args = item.get("arguments") if "arguments" in item else item.get("input")
        if args is not None:
            args_str = json.dumps(args) if isinstance(args, (dict, list)) else str(args)
            b = len(args_str.encode("utf-8"))
            state["auxiliary_tool_payload_bytes"] += b
            if is_engram:
                state["serialized_invocations_bytes"] += b
                state["files_retrieved_via_engram"].extend(extract_files_from_obj(args))

        res = item.get("result") if "result" in item else item.get("output")
        if res is not None:
            res_str = json.dumps(res) if isinstance(res, (dict, list)) else str(res)
            b = len(res_str.encode("utf-8"))
            state["auxiliary_tool_payload_bytes"] += b
            if is_engram:
                state["serialized_responses_bytes"] += b
                state["files_retrieved_via_engram"].extend(extract_files_from_obj(res))
                if any(tok in res_str for tok in ("[COMPLETE_BLOCK]", "[INCOMPLETE_EXCERPT]", "code_excerpt", "line_range")):
                    state["engram_excerpts_count"] += 1

    # Shell commands or file reads
    elif norm_type in ("command_execution", "exec_command", "bash", "shell", "function_call", "tool_call") or name in ("exec_command", "bash", "execute_command", "powershell", "read_file", "cat"):
        cmd = item.get("command") or item.get("cmd") or ""
        args = item.get("arguments") if "arguments" in item else item.get("input")
        if isinstance(args, dict):
            cmd = cmd or args.get("command") or args.get("cmd") or ""
            p = args.get("path") or args.get("file_path") or args.get("file")
            if p and isinstance(p, str):
                state["files_read_via_shell"].append(p.replace("\\", "/"))
        elif isinstance(args, str) and not cmd:
            cmd = args
        if cmd:
            state["files_read_via_shell"].extend(extract_files_from_shell_command(cmd))


def finalize_replacement_accounting(state: Dict[str, Any]) -> TokenReplacementAccounting:
    files_retrieved = sorted(list(dict.fromkeys(state["files_retrieved_via_engram"])))
    files_read = sorted(list(dict.fromkeys(state["files_read_via_shell"])))
    reread_after_engram = [f for f in state["files_read_via_shell"] if any((ef in f or f in ef) for ef in files_retrieved)]
    repeated_shell_reads = [f for f in set(state["files_read_via_shell"]) if state["files_read_via_shell"].count(f) > 1]
    files_repeated = sorted(list(set(reread_after_engram + repeated_shell_reads)))
    excerpts_reread = len(reread_after_engram)
    inv_bytes = state["serialized_invocations_bytes"]
    res_bytes = state["serialized_responses_bytes"]
    payload_tokens = (inv_bytes + res_bytes) // 4
    if state["mcp_tool_calls_count"] > 0 or len(files_retrieved) > 0:
        status = "MEASURED"
    elif files_read:
        status = "ESTIMATED"
    else:
        status = "NOT_OBSERVABLE"
    return TokenReplacementAccounting(
        files_retrieved_via_engram=files_retrieved,
        files_read_via_shell=files_read,
        files_repeated_read=files_repeated,
        engram_excerpts_count=state["engram_excerpts_count"],
        engram_excerpts_reread_count=excerpts_reread,
        serialized_invocations_bytes=inv_bytes,
        serialized_responses_bytes=res_bytes,
        estimated_payload_tokens=payload_tokens,
        replacement_status=status,
        tokens_per_successful_task=None
    )


@dataclass
class TelemetryMetrics:
    provenance: str  # "SESSION_ROLLOUT", "DIRECT_API", "HARNESS_LOG", "UI_COUNTER", "SYNTHETIC_TEST", "UNAVAILABLE"
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
    telemetry_valid: bool = True
    error_message: Optional[str] = None
    replacement_accounting: Optional[TokenReplacementAccounting] = None

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


class TelemetryParser:
    def __init__(self, provenance: str = "DIRECT_API", sessions_dir: Optional[Path] = None):
        self.provenance = provenance
        self.sessions_dir = Path(sessions_dir) if sessions_dir else CODEX_SESSIONS_DIR

    def create_unavailable_metrics(self, reason: str = "Telemetry unavailable") -> TelemetryMetrics:
        """Creates a structured unavailable TelemetryMetrics record without fabricating token counts."""
        return TelemetryMetrics(
            provenance="UNAVAILABLE",
            prompt_tokens_total=0,
            prompt_tokens_cached=0,
            prompt_tokens_cached_inferred=False,
            prompt_tokens_uncached=0,
            prompt_tokens_uncached_inferred=False,
            completion_tokens=0,
            reasoning_tokens=0,
            total_model_tokens=0,
            peak_single_request_context=None,
            peak_turn_context=None,
            final_turn_context=None,
            model_requests_count=None,
            turns_count=0,
            mcp_tool_calls_count=0,
            auxiliary_tool_payload_bytes=0,
            telemetry_valid=False,
            error_message=reason,
            replacement_accounting=TokenReplacementAccounting(
                files_retrieved_via_engram=[],
                files_read_via_shell=[],
                files_repeated_read=[],
                engram_excerpts_count=0,
                engram_excerpts_reread_count=0,
                serialized_invocations_bytes=0,
                serialized_responses_bytes=0,
                estimated_payload_tokens=0,
                replacement_status="NOT_OBSERVABLE",
                tokens_per_successful_task=None
            )
        )

    def find_session_rollout(
        self,
        thread_id: Optional[str] = None,
        sandbox_path: Optional[Union[str, Path]] = None,
        min_mtime: Optional[float] = None
    ) -> Optional[Path]:
        """
        Discovers the matching Codex session rollout file from sessions_dir.
        Checks thread_id first; if not found or unavailable, matches by sandbox_path in world_state.
        """
        if not self.sessions_dir.exists():
            return None

        # 1. Search by thread_id in today's folder, then fallback
        if thread_id:
            now_dt = datetime.datetime.now(datetime.timezone.utc)
            today_dir = self.sessions_dir / now_dt.strftime("%Y") / now_dt.strftime("%m") / now_dt.strftime("%d")
            matches = glob.glob(str(today_dir / f"*{thread_id}*.jsonl")) if today_dir.exists() else []
            if not matches:
                pattern = str(self.sessions_dir / "**" / f"*{thread_id}*.jsonl")
                matches = glob.glob(pattern, recursive=True)
            if matches:
                return Path(matches[0])

        # 2. Search by sandbox_path in candidate files (checking today's folder first)
        if sandbox_path:
            norm_sandbox = str(Path(sandbox_path).resolve()).lower()
            now_dt = datetime.datetime.now(datetime.timezone.utc)
            today_dir = self.sessions_dir / now_dt.strftime("%Y") / now_dt.strftime("%m") / now_dt.strftime("%d")
            candidates = glob.glob(str(today_dir / "rollout-*.jsonl")) if today_dir.exists() else []
            if not candidates:
                pattern = str(self.sessions_dir / "**" / "rollout-*.jsonl")
                candidates = glob.glob(pattern, recursive=True)

            candidate_paths = sorted([Path(c) for c in candidates], key=lambda p: p.stat().st_mtime if p.exists() else 0, reverse=True)
            cutoff = (min_mtime - 30.0) if min_mtime else (time.time() - 3600.0)
            for cand in candidate_paths[:25]:
                try:
                    if cand.stat().st_mtime < cutoff:
                        continue
                    with open(cand, "r", encoding="utf-8", errors="replace") as f:
                        for idx, line in enumerate(f):
                            if idx > 25:
                                break
                            if norm_sandbox in line.lower():
                                return cand
                except Exception:
                    continue

        return None

    def parse_rollout_file(self, rollout_path: Union[str, Path]) -> TelemetryMetrics:
        """
        Parses a definitive Codex session rollout JSONL file (from .codex/sessions).
        Extracts exact per-request usage from token_usage_record entries.
        """
        rollout_path = Path(rollout_path)
        if not rollout_path.exists() or rollout_path.stat().st_size == 0:
            return self.create_unavailable_metrics(f"Rollout file missing or empty: {rollout_path}")

        records = []
        seen_tool_ids: Set[str] = set()
        accounting_state = {
            "mcp_tool_calls_count": 0,
            "auxiliary_tool_payload_bytes": 0,
            "serialized_invocations_bytes": 0,
            "serialized_responses_bytes": 0,
            "files_retrieved_via_engram": [],
            "files_read_via_shell": [],
            "engram_excerpts_count": 0
        }

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

                    # 1. item.completed / item.started
                    elif ev_type in ("item.completed", "item.started"):
                        item = data.get("item", {})
                        if ev_type == "item.completed":
                            process_item_for_replacement(item, seen_tool_ids, accounting_state)

                    # 2. event_msg wrapper
                    elif ev_type == "event_msg":
                        payload = data.get("payload", {})
                        p_type = payload.get("type", "")
                        if p_type in ("item_completed", "item.completed"):
                            item = payload.get("item", {})
                            process_item_for_replacement(item, seen_tool_ids, accounting_state)

                    # 3. response_item custom_tool_call
                    elif ev_type == "response_item":
                        payload = data.get("payload", {})
                        p_type = payload.get("type", "")
                        if p_type == "custom_tool_call":
                            process_item_for_replacement(payload, seen_tool_ids, accounting_state)
                        elif p_type == "custom_tool_call_output":
                            call_id = payload.get("call_id") or payload.get("id")
                            if call_id and call_id in seen_tool_ids:
                                out = payload.get("output") or payload.get("result")
                                if out is not None:
                                    out_str = json.dumps(out) if isinstance(out, (dict, list)) else str(out)
                                    b = len(out_str.encode("utf-8"))
                                    accounting_state["auxiliary_tool_payload_bytes"] += b
                                    accounting_state["serialized_responses_bytes"] += b
                                    accounting_state["files_retrieved_via_engram"].extend(extract_files_from_obj(out))
                                    if any(tok in out_str for tok in ("[COMPLETE_BLOCK]", "[INCOMPLETE_EXCERPT]", "code_excerpt", "line_range")):
                                        accounting_state["engram_excerpts_count"] += 1
                except json.JSONDecodeError:
                    continue

        mcp_tool_calls_count = accounting_state["mcp_tool_calls_count"]
        auxiliary_tool_payload_bytes = accounting_state["auxiliary_tool_payload_bytes"]
        replacement_accounting = finalize_replacement_accounting(accounting_state)

        if not records:
            return self.create_unavailable_metrics(f"No token_usage_record events found in rollout file: {rollout_path}")

        # The last record contains the authoritative thread_token_usage
        last_record = records[-1]
        final_thread_usage = last_record.get("thread_token_usage") or last_record.get("usage", {})

        prompt_tokens_total = int(
            final_thread_usage.get("input_tokens")
            if final_thread_usage.get("input_tokens") is not None
            else final_thread_usage.get("prompt_tokens", 0)
        )
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

        completion_tokens = int(
            final_thread_usage.get("output_tokens")
            if final_thread_usage.get("output_tokens") is not None
            else final_thread_usage.get("completion_tokens", 0)
        )
        reasoning_tokens = int(
            final_thread_usage.get("reasoning_output_tokens")
            if final_thread_usage.get("reasoning_output_tokens") is not None
            else final_thread_usage.get("reasoning_tokens", 0)
        )

        # Per-request context inspection
        request_contexts = []
        for r in records:
            req_usage = r.get("usage", {})
            inp = int(
                req_usage.get("input_tokens")
                if req_usage.get("input_tokens") is not None
                else req_usage.get("prompt_tokens", 0)
            )
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
            replacement_accounting=replacement_accounting,
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
        seen_tool_ids: Set[str] = set()
        accounting_state = {
            "mcp_tool_calls_count": 0,
            "auxiliary_tool_payload_bytes": 0,
            "serialized_invocations_bytes": 0,
            "serialized_responses_bytes": 0,
            "files_retrieved_via_engram": [],
            "files_read_via_shell": [],
            "engram_excerpts_count": 0
        }

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
                if ev_type == "item.completed":
                    process_item_for_replacement(item, seen_tool_ids, accounting_state)

            elif ev_type == "event_msg":
                payload = event.get("payload", {})
                p_type = payload.get("type", "")
                if p_type in ("item_completed", "item.completed"):
                    item = payload.get("item", {})
                    process_item_for_replacement(item, seen_tool_ids, accounting_state)

            elif ev_type == "response_item":
                payload = event.get("payload", {})
                p_type = payload.get("type", "")
                if p_type == "custom_tool_call":
                    process_item_for_replacement(payload, seen_tool_ids, accounting_state)
                elif p_type == "custom_tool_call_output":
                    call_id = payload.get("call_id") or payload.get("id")
                    if call_id and call_id in seen_tool_ids:
                        out = payload.get("output") or payload.get("result")
                        if out is not None:
                            out_str = json.dumps(out) if isinstance(out, (dict, list)) else str(out)
                            b = len(out_str.encode("utf-8"))
                            accounting_state["auxiliary_tool_payload_bytes"] += b
                            accounting_state["serialized_responses_bytes"] += b
                            accounting_state["files_retrieved_via_engram"].extend(extract_files_from_obj(out))
                            if any(tok in out_str for tok in ("[COMPLETE_BLOCK]", "[INCOMPLETE_EXCERPT]", "code_excerpt", "line_range")):
                                accounting_state["engram_excerpts_count"] += 1

        turns_count = len(turn_inputs)
        final_turn_context = turn_inputs[-1] if turn_inputs else None
        mcp_tool_calls_count = accounting_state["mcp_tool_calls_count"]
        auxiliary_tool_payload_bytes = accounting_state["auxiliary_tool_payload_bytes"]
        replacement_accounting = finalize_replacement_accounting(accounting_state)

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
        is_valid = (total_model_tokens > 0) or (self.provenance == "SYNTHETIC_TEST")

        return TelemetryMetrics(
            provenance=self.provenance if is_valid else "UNAVAILABLE",
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
            replacement_accounting=replacement_accounting,
            telemetry_valid=is_valid,
            error_message=None if is_valid else "Zero total model tokens accumulated in event stream"
        )

    def parse_file(
        self,
        jsonl_path: Union[str, Path],
        thread_id: Optional[str] = None
    ) -> TelemetryMetrics:
        path = Path(jsonl_path)
        if not path.exists() or path.stat().st_size == 0:
            if thread_id and self.sessions_dir.exists():
                rollout = self.find_session_rollout(thread_id=thread_id)
                if rollout:
                    try:
                        return self.parse_rollout_file(rollout)
                    except Exception:
                        pass
            return self.create_unavailable_metrics(f"Events file not found or empty: {jsonl_path}")

        events = []
        try:
            with open(path, "r", encoding="utf-8", errors="replace") as f:
                for line in f:
                    line = line.strip()
                    if not line:
                        continue
                    try:
                        events.append(json.loads(line))
                    except json.JSONDecodeError:
                        continue
        except Exception as e:
            return self.create_unavailable_metrics(f"Failed to read events file: {e}")

        if not events and self.provenance != "SYNTHETIC_TEST":
            if thread_id and self.sessions_dir.exists():
                rollout = self.find_session_rollout(thread_id=thread_id)
                if rollout:
                    try:
                        return self.parse_rollout_file(rollout)
                    except Exception:
                        pass
            return self.create_unavailable_metrics(f"Zero parseable JSON events in {jsonl_path}")

        return self.parse_events(events, thread_id=thread_id)
