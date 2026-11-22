"""
Engram Phase 5.9.2.5: Deterministic Provider Simulator
Repository: https://github.com/engram/engram

Provides a 100% offline, deterministic model provider simulator reproducing
controlled multi-turn agent interactions across five pentad arms:
- Arm A: arm_a_native (Native Baseline, zero Engram tools)
- Arm B: arm_b_cand3 (Candidate 3, static symbol indexing & flat context)
- Arm C: arm_c_cand42_always_on (Candidate 4.2 Always-On, structural symbol compression)
- Arm D: arm_d_cand42_adaptive (Candidate 4.2 Adaptive, dynamic complexity routing)
- Arm E: arm_e_cand5_optimized (Candidate 5 Optimized, 7 Token-Efficiency Optimizations)

Provenance Marker: SYNTHETIC_SIMULATED_USAGE
Execution Mode: Strictly Offline, Zero Network Calls, Seeded Deterministic Reproducibility.
"""

from dataclasses import dataclass, field
import datetime
import json
from pathlib import Path
import sys
from typing import Any, Callable, Dict, List, Optional, Tuple
import uuid

from provider_transport import (
    BudgetExceededError,
    ProviderRequest,
    ProviderResponse,
    ProviderTransport,
)
from runner import RequestReservation


PROVENANCE_MARKER = "SYNTHETIC_SIMULATED_USAGE"


@dataclass
class SimulatedTurn:
    """Represents a single deterministic model turn."""
    turn_index: int
    content: Optional[str] = None
    tool_calls: Optional[List[Dict[str, Any]]] = None
    finish_reason: str = "stop"
    synthetic_prompt_tokens: int = 150
    synthetic_completion_tokens: int = 40
    cached_tokens: int = 0
    reasoning_tokens: int = 0


class DeterministicProviderSimulator(ProviderTransport):
    """
    Deterministic provider simulator implementing reproducible multi-turn agent workflows.
    Ensures that offline benchmarking produces mathematically consistent, reproducible
    observations without requiring external API access or credentials.
    """
    def __init__(
        self,
        model: str = "gpt-5.6-luna",
        seed: int = 59002,
        workload_id: str = "workload_a_simple_edit",
        arm_id: str = "arm_a_native",
        simulate_latency_ms: float = 5.0
    ):
        super().__init__(
            model=model,
            provider_name="deterministic_simulator",
            api_base_url="simulator://offline/v1",
            api_key=None,
            timeout_seconds=60,
            allow_live_network=False
        )
        self.seed = seed
        self.workload_id = workload_id
        self.arm_id = arm_id
        self.simulate_latency_ms = simulate_latency_ms
        self.current_turn = 0
        self.dispatched_history: List[Tuple[ProviderRequest, RequestReservation]] = []
        self.provenance = PROVENANCE_MARKER
        self._workload_plans = self._build_workload_plans()

    def serialize_request(self, req: ProviderRequest) -> Tuple[str, bytes, int, str]:
        """Serializes request and counts synthetic tokens deterministically."""
        payload = {
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
        return serialized_str, raw_bytes, tokens, f"simulator_{method}"

    def dispatch(self, req: ProviderRequest, reservation: RequestReservation) -> ProviderResponse:
        """
        Dispatches request to deterministic simulation plan.
        Fails closed if reservation is invalid.
        """
        if not reservation or reservation.status != "ACTIVE":
            raise BudgetExceededError(
                f"Unauthorized dispatch attempt: Request {req.request_id} has no ACTIVE reservation."
            )

        self.dispatched_history.append((req, reservation))
        now_dt = datetime.datetime.now(datetime.timezone.utc)
        dispatched_at = now_dt.isoformat()

        plan = self._workload_plans.get(self.workload_id, {}).get(self.arm_id, [])
        if self.current_turn < len(plan):
            sim_turn = plan[self.current_turn]
            self.current_turn += 1
        else:
            # Fallback completion turn
            sim_turn = SimulatedTurn(
                turn_index=self.current_turn,
                content=f"Workload {self.workload_id} completed by {self.arm_id}.",
                tool_calls=None,
                finish_reason="stop",
                synthetic_prompt_tokens=reservation.input_tokens,
                synthetic_completion_tokens=25
            )
            self.current_turn += 1

        completed_dt = datetime.datetime.now(datetime.timezone.utc)
        completed_at = completed_dt.isoformat()

        p_tok = sim_turn.synthetic_prompt_tokens
        c_tok = sim_turn.synthetic_completion_tokens
        t_tok = p_tok + c_tok

        return ProviderResponse(
            response_id=f"sim-resp-{uuid.uuid4().hex[:8]}",
            request_id=req.request_id,
            turn_index=req.turn_index,
            model=req.model or self.model,
            content=sim_turn.content,
            tool_calls=sim_turn.tool_calls,
            finish_reason=sim_turn.finish_reason,
            prompt_tokens=p_tok,
            completion_tokens=c_tok,
            total_tokens=t_tok,
            raw_usage={
                "prompt_tokens": p_tok,
                "completion_tokens": c_tok,
                "total_tokens": t_tok,
                "provenance": PROVENANCE_MARKER,
                "cached_tokens": sim_turn.cached_tokens,
                "reasoning_tokens": sim_turn.reasoning_tokens
            },
            dispatched_at=dispatched_at,
            completed_at=completed_at,
            latency_ms=self.simulate_latency_ms,
            is_streaming=False,
            stream_cancelled=False,
            error=None,
            error_code=None,
            cached_input_tokens=sim_turn.cached_tokens,
            reasoning_output_tokens=sim_turn.reasoning_tokens
        )

    def stream_dispatch(
        self,
        req: ProviderRequest,
        reservation: RequestReservation,
        chunk_callback: Optional[Callable[[str, int], None]] = None
    ) -> ProviderResponse:
        """Streaming dispatch returns simulated response."""
        return self.dispatch(req, reservation)

    def _build_workload_plans(self) -> Dict[str, Dict[str, List[SimulatedTurn]]]:
        """
        Builds the deterministic multi-turn simulation trees for all 6 workloads
        and all 5 pentad arms.
        """
        plans: Dict[str, Dict[str, List[SimulatedTurn]]] = {}

        # ======================================================================
        # WORKLOAD A: Simple Coding Change (src/calculator.py tax calculation bug)
        # ======================================================================
        plans["workload_a_simple_edit"] = {
            # Arm A (Native): Inspects calculator.py, writes fix, concludes.
            "arm_a_native": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{
                        "id": "call_wa_native_01",
                        "type": "function",
                        "function": {"name": "read_file", "arguments": json.dumps({"path": "src/calculator.py"})}
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=220,
                    synthetic_completion_tokens=35
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{
                        "id": "call_wa_native_02",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/calculator.py",
                                "content": "def add_tax(price: float, rate: float) -> float:\n    return round(price * (1.0 + rate), 2)\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=310,
                    synthetic_completion_tokens=50
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Tax calculation bug corrected in src/calculator.py. add_tax returns price * (1 + rate).",
                    finish_reason="stop",
                    synthetic_prompt_tokens=390,
                    synthetic_completion_tokens=25
                )
            ],
            # Arm B (Candidate 3): Uses engram_get_context, reads file, edits, concludes.
            "arm_b_cand3": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{
                        "id": "call_wa_cand3_01",
                        "type": "function",
                        "function": {"name": "engram_get_context", "arguments": "{}"}
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=260,
                    synthetic_completion_tokens=30
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{
                        "id": "call_wa_cand3_02",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/calculator.py",
                                "content": "def add_tax(price: float, rate: float) -> float:\n    return round(price * (1.0 + rate), 2)\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=340,
                    synthetic_completion_tokens=50
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Tax calculation bug fixed with Candidate 3 context.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=410,
                    synthetic_completion_tokens=20
                )
            ],
            # Arm C (Candidate 4.2 Always-On): Uses compressed context.
            "arm_c_cand42_always_on": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{
                        "id": "call_wa_cand42_01",
                        "type": "function",
                        "function": {"name": "engram_get_context", "arguments": "{}"}
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=230,  # Compressed symbol table
                    synthetic_completion_tokens=25
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{
                        "id": "call_wa_cand42_02",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/calculator.py",
                                "content": "def add_tax(price: float, rate: float) -> float:\n    return round(price * (1.0 + rate), 2)\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=295,
                    synthetic_completion_tokens=45
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Tax calculation bug fixed.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=350,
                    synthetic_completion_tokens=18
                )
            ],
            # Arm D (Candidate 4.2 Adaptive): Heuristic detects SIMPLE task -> BYPASS route!
            # Skips MCP overhead entirely, directly writes fix in 1 turn!
            "arm_d_cand42_adaptive": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{
                        "id": "call_wa_adaptive_01",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/calculator.py",
                                "content": "def add_tax(price: float, rate: float) -> float:\n    return round(price * (1.0 + rate), 2)\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=180,  # Zero MCP tool prompt overhead
                    synthetic_completion_tokens=45
                ),
                SimulatedTurn(
                    turn_index=1,
                    content="Adaptive classifier bypassed MCP tools. File fixed directly.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=240,
                    synthetic_completion_tokens=15
                )
            ],
            # Arm E (Candidate 5 Optimized): Adaptive bypass + AST delta compression + minimal turn.
            "arm_e_cand5_optimized": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{
                        "id": "call_wa_cand5_01",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/calculator.py",
                                "content": "def add_tax(price: float, rate: float) -> float:\n    return round(price * (1.0 + rate), 2)\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=160,  # AST delta token savings
                    synthetic_completion_tokens=40,
                    cached_tokens=40
                ),
                SimulatedTurn(
                    turn_index=1,
                    content="Task A completed with Candidate 5 optimized execution.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=210,
                    synthetic_completion_tokens=15,
                    cached_tokens=50
                )
            ]
        }

        # ======================================================================
        # WORKLOAD B: Cross-File Implementation (src/models.py & src/service.py)
        # ======================================================================
        plans["workload_b_cross_file"] = {
            # Arm A (Native): Discovers files, reads models, reads service, edits models, edits service.
            "arm_a_native": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[
                        {"id": "call_wb_a1", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "src/models.py"})}},
                        {"id": "call_wb_a2", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "src/service.py"})}}
                    ],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=320,
                    synthetic_completion_tokens=60
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{
                        "id": "call_wb_a3",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/models.py",
                                "content": "from dataclasses import dataclass\n\n@dataclass\nclass Account:\n    account_id: str\n    owner_name: str\n    balance: float\n    is_active: bool = True\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=510,
                    synthetic_completion_tokens=55
                ),
                SimulatedTurn(
                    turn_index=2,
                    tool_calls=[{
                        "id": "call_wb_a4",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/service.py",
                                "content": "from src.models import Account\n\nclass AccountService:\n    def __init__(self):\n        self.accounts = {}\n\n    def register(self, account_id: str, owner: str, balance: float):\n        acc = Account(account_id, owner, balance)\n        self.accounts[account_id] = acc\n        return acc\n\n    def get_active_accounts(self):\n        return [a for a in self.accounts.values() if getattr(a, 'is_active', True)]\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=680,
                    synthetic_completion_tokens=70
                ),
                SimulatedTurn(
                    turn_index=3,
                    content="Cross-file Account and AccountService modifications complete.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=820,
                    synthetic_completion_tokens=20
                )
            ],
            # Arm B (Candidate 3): Inspects symbol, edits models, edits service.
            "arm_b_cand3": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{"id": "call_wb_b1", "type": "function", "function": {"name": "engram_inspect_symbol", "arguments": json.dumps({"symbol": "Account"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=290,
                    synthetic_completion_tokens=40
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[
                        {
                            "id": "call_wb_b2",
                            "type": "function",
                            "function": {
                                "name": "write_file",
                                "arguments": json.dumps({
                                    "path": "src/models.py",
                                    "content": "from dataclasses import dataclass\n\n@dataclass\nclass Account:\n    account_id: str\n    owner_name: str\n    balance: float\n    is_active: bool = True\n"
                                })
                            }
                        },
                        {
                            "id": "call_wb_b3",
                            "type": "function",
                            "function": {
                                "name": "write_file",
                                "arguments": json.dumps({
                                    "path": "src/service.py",
                                    "content": "from src.models import Account\n\nclass AccountService:\n    def __init__(self):\n        self.accounts = {}\n\n    def register(self, account_id: str, owner: str, balance: float):\n        acc = Account(account_id, owner, balance)\n        self.accounts[account_id] = acc\n        return acc\n\n    def get_active_accounts(self):\n        return [a for a in self.accounts.values() if getattr(a, 'is_active', True)]\n"
                                })
                            }
                        }
                    ],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=480,
                    synthetic_completion_tokens=110
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Candidate 3 cross-file update completed.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=650,
                    synthetic_completion_tokens=20
                )
            ],
            # Arm C (Cand 4.2 Always-On): Compressed symbol tables.
            "arm_c_cand42_always_on": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{"id": "call_wb_c1", "type": "function", "function": {"name": "engram_inspect_symbol", "arguments": json.dumps({"symbol": "Account"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=250,
                    synthetic_completion_tokens=35
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[
                        {
                            "id": "call_wb_c2",
                            "type": "function",
                            "function": {
                                "name": "write_file",
                                "arguments": json.dumps({
                                    "path": "src/models.py",
                                    "content": "from dataclasses import dataclass\n\n@dataclass\nclass Account:\n    account_id: str\n    owner_name: str\n    balance: float\n    is_active: bool = True\n"
                                })
                            }
                        },
                        {
                            "id": "call_wb_c3",
                            "type": "function",
                            "function": {
                                "name": "write_file",
                                "arguments": json.dumps({
                                    "path": "src/service.py",
                                    "content": "from src.models import Account\n\nclass AccountService:\n    def __init__(self):\n        self.accounts = {}\n\n    def register(self, account_id: str, owner: str, balance: float):\n        acc = Account(account_id, owner, balance)\n        self.accounts[account_id] = acc\n        return acc\n\n    def get_active_accounts(self):\n        return [a for a in self.accounts.values() if getattr(a, 'is_active', True)]\n"
                                })
                            }
                        }
                    ],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=420,
                    synthetic_completion_tokens=110
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Candidate 4.2 cross-file update completed.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=580,
                    synthetic_completion_tokens=20
                )
            ],
            # Arm D (Cand 4.2 Adaptive): LEAN mode routing.
            "arm_d_cand42_adaptive": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{"id": "call_wb_d1", "type": "function", "function": {"name": "engram_inspect_symbol", "arguments": json.dumps({"symbol": "Account"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=240,
                    synthetic_completion_tokens=35
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[
                        {
                            "id": "call_wb_d2",
                            "type": "function",
                            "function": {
                                "name": "write_file",
                                "arguments": json.dumps({
                                    "path": "src/models.py",
                                    "content": "from dataclasses import dataclass\n\n@dataclass\nclass Account:\n    account_id: str\n    owner_name: str\n    balance: float\n    is_active: bool = True\n"
                                })
                            }
                        },
                        {
                            "id": "call_wb_d3",
                            "type": "function",
                            "function": {
                                "name": "write_file",
                                "arguments": json.dumps({
                                    "path": "src/service.py",
                                    "content": "from src.models import Account\n\nclass AccountService:\n    def __init__(self):\n        self.accounts = {}\n\n    def register(self, account_id: str, owner: str, balance: float):\n        acc = Account(account_id, owner, balance)\n        self.accounts[account_id] = acc\n        return acc\n\n    def get_active_accounts(self):\n        return [a for a in self.accounts.values() if getattr(a, 'is_active', True)]\n"
                                })
                            }
                        }
                    ],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=400,
                    synthetic_completion_tokens=110
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Adaptive cross-file update complete.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=550,
                    synthetic_completion_tokens=20
                )
            ],
            # Arm E (Candidate 5 Optimized): AST delta compression + Preflight contract cache.
            "arm_e_cand5_optimized": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[
                        {
                            "id": "call_wb_e1",
                            "type": "function",
                            "function": {
                                "name": "write_file",
                                "arguments": json.dumps({
                                    "path": "src/models.py",
                                    "content": "from dataclasses import dataclass\n\n@dataclass\nclass Account:\n    account_id: str\n    owner_name: str\n    balance: float\n    is_active: bool = True\n"
                                })
                            }
                        },
                        {
                            "id": "call_wb_e2",
                            "type": "function",
                            "function": {
                                "name": "write_file",
                                "arguments": json.dumps({
                                    "path": "src/service.py",
                                    "content": "from src.models import Account\n\nclass AccountService:\n    def __init__(self):\n        self.accounts = {}\n\n    def register(self, account_id: str, owner: str, balance: float):\n        acc = Account(account_id, owner, balance)\n        self.accounts[account_id] = acc\n        return acc\n\n    def get_active_accounts(self):\n        return [a for a in self.accounts.values() if getattr(a, 'is_active', True)]\n"
                                })
                            }
                        }
                    ],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=220,  # Cached symbol contracts
                    synthetic_completion_tokens=100,
                    cached_tokens=80
                ),
                SimulatedTurn(
                    turn_index=1,
                    content="Candidate 5 cross-file execution complete in 2 turns.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=350,
                    synthetic_completion_tokens=15,
                    cached_tokens=120
                )
            ]
        }

        # ======================================================================
        # WORKLOAD C: Debugging (src/indexer.py safe_divide)
        # ======================================================================
        plans["workload_c_debugging"] = {
            "arm_a_native": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{"id": "call_wc_a1", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "src/indexer.py"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=210,
                    synthetic_completion_tokens=30
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{
                        "id": "call_wc_a2",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/indexer.py",
                                "content": "def safe_divide(values):\n    if not values:\n        return []\n    total = sum(values)\n    if total == 0:\n        return [0.0] * len(values)\n    return [v / total for v in values]\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=310,
                    synthetic_completion_tokens=55
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="safe_divide bug corrected with zero sum and empty checks.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=400,
                    synthetic_completion_tokens=20
                )
            ],
            "arm_b_cand3": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{"id": "call_wc_b1", "type": "function", "function": {"name": "engram_inspect_symbol", "arguments": json.dumps({"symbol": "safe_divide"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=240,
                    synthetic_completion_tokens=35
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{
                        "id": "call_wc_b2",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/indexer.py",
                                "content": "def safe_divide(values):\n    if not values:\n        return []\n    total = sum(values)\n    if total == 0:\n        return [0.0] * len(values)\n    return [v / total for v in values]\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=350,
                    synthetic_completion_tokens=55
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Bug fixed.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=430,
                    synthetic_completion_tokens=20
                )
            ],
            "arm_c_cand42_always_on": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{"id": "call_wc_c1", "type": "function", "function": {"name": "engram_inspect_symbol", "arguments": json.dumps({"symbol": "safe_divide"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=220,
                    synthetic_completion_tokens=30
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{
                        "id": "call_wc_c2",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/indexer.py",
                                "content": "def safe_divide(values):\n    if not values:\n        return []\n    total = sum(values)\n    if total == 0:\n        return [0.0] * len(values)\n    return [v / total for v in values]\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=315,
                    synthetic_completion_tokens=55
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Bug fixed.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=390,
                    synthetic_completion_tokens=15
                )
            ],
            "arm_d_cand42_adaptive": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{"id": "call_wc_d1", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "src/indexer.py"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=195,
                    synthetic_completion_tokens=30
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{
                        "id": "call_wc_d2",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/indexer.py",
                                "content": "def safe_divide(values):\n    if not values:\n        return []\n    total = sum(values)\n    if total == 0:\n        return [0.0] * len(values)\n    return [v / total for v in values]\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=290,
                    synthetic_completion_tokens=55
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Bug fixed.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=360,
                    synthetic_completion_tokens=15
                )
            ],
            "arm_e_cand5_optimized": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{
                        "id": "call_wc_e1",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/indexer.py",
                                "content": "def safe_divide(values):\n    if not values:\n        return []\n    total = sum(values)\n    if total == 0:\n        return [0.0] * len(values)\n    return [v / total for v in values]\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=175,
                    synthetic_completion_tokens=50,
                    cached_tokens=50
                ),
                SimulatedTurn(
                    turn_index=1,
                    content="Bug corrected with single surgical AST edit.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=235,
                    synthetic_completion_tokens=15,
                    cached_tokens=60
                )
            ]
        }

        # ======================================================================
        # WORKLOAD D: Multi-turn Tool Orchestration (config.json batch_size/workers)
        # ======================================================================
        plans["workload_d_tool_orchestration"] = {
            "arm_a_native": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{"id": "call_wd_a1", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "config.json"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=230,
                    synthetic_completion_tokens=25
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{"id": "call_wd_a2", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "src/pipeline.py"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=320,
                    synthetic_completion_tokens=30
                ),
                SimulatedTurn(
                    turn_index=2,
                    tool_calls=[{
                        "id": "call_wd_a3",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "config.json",
                                "content": json.dumps({"batch_size": 64, "workers": 4, "validated": True}, indent=2)
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=430,
                    synthetic_completion_tokens=45
                ),
                SimulatedTurn(
                    turn_index=3,
                    content="Pipeline configuration updated.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=520,
                    synthetic_completion_tokens=20
                )
            ],
            "arm_b_cand3": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[
                        {"id": "call_wd_b1", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "config.json"})}},
                        {"id": "call_wd_b2", "type": "function", "function": {"name": "engram_get_context", "arguments": "{}"}}
                    ],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=280,
                    synthetic_completion_tokens=45
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{
                        "id": "call_wd_b3",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "config.json",
                                "content": json.dumps({"batch_size": 64, "workers": 4, "validated": True}, indent=2)
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=410,
                    synthetic_completion_tokens=45
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Config updated.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=490,
                    synthetic_completion_tokens=20
                )
            ],
            "arm_c_cand42_always_on": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[
                        {"id": "call_wd_c1", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "config.json"})}},
                        {"id": "call_wd_c2", "type": "function", "function": {"name": "engram_get_context", "arguments": "{}"}}
                    ],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=250,
                    synthetic_completion_tokens=45
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{
                        "id": "call_wd_c3",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "config.json",
                                "content": json.dumps({"batch_size": 64, "workers": 4, "validated": True}, indent=2)
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=370,
                    synthetic_completion_tokens=45
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Config updated.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=440,
                    synthetic_completion_tokens=20
                )
            ],
            "arm_d_cand42_adaptive": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{"id": "call_wd_d1", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "config.json"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=220,
                    synthetic_completion_tokens=25
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{
                        "id": "call_wd_d2",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "config.json",
                                "content": json.dumps({"batch_size": 64, "workers": 4, "validated": True}, indent=2)
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=330,
                    synthetic_completion_tokens=45
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Config updated.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=410,
                    synthetic_completion_tokens=20
                )
            ],
            "arm_e_cand5_optimized": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[
                        {"id": "call_wd_e1", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "config.json"})}},
                        {
                            "id": "call_wd_e2",
                            "type": "function",
                            "function": {
                                "name": "write_file",
                                "arguments": json.dumps({
                                    "path": "config.json",
                                    "content": json.dumps({"batch_size": 64, "workers": 4, "validated": True}, indent=2)
                                })
                            }
                        }
                    ],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=210,
                    synthetic_completion_tokens=60,
                    cached_tokens=50
                ),
                SimulatedTurn(
                    turn_index=1,
                    content="Config read and updated in consolidated turn.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=310,
                    synthetic_completion_tokens=15,
                    cached_tokens=70
                )
            ]
        }

        # ======================================================================
        # WORKLOAD E: Context-Heavy Task (target_module.py MAX_RETRIES = 5)
        # ======================================================================
        plans["workload_e_context_heavy"] = {
            # Arm A (Native): Reads all files blindly (architecture, legacy_v1, legacy_v2, target_module)
            # Accumulates massive context (~2800 tokens)
            "arm_a_native": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[
                        {"id": "call_we_a1", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "docs/architecture.md"})}},
                        {"id": "call_we_a2", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "src/legacy_v1.py"})}}
                    ],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=550,
                    synthetic_completion_tokens=45
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[
                        {"id": "call_we_a3", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "src/legacy_v2.py"})}},
                        {"id": "call_we_a4", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "src/target_module.py"})}}
                    ],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=1100,
                    synthetic_completion_tokens=50
                ),
                SimulatedTurn(
                    turn_index=2,
                    tool_calls=[{
                        "id": "call_we_a5",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/target_module.py",
                                "content": "MAX_RETRIES = 5\nTIMEOUT_SECONDS = 30\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=1650,
                    synthetic_completion_tokens=40
                ),
                SimulatedTurn(
                    turn_index=3,
                    content="Context-heavy task completed after reading entire workspace.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=1950,
                    synthetic_completion_tokens=25
                )
            ],
            # Arm B (Candidate 3): Uses engram_get_context to find target module
            "arm_b_cand3": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[
                        {"id": "call_we_b1", "type": "function", "function": {"name": "engram_get_context", "arguments": "{}"}},
                        {"id": "call_we_b2", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "src/target_module.py"})}}
                    ],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=420,
                    synthetic_completion_tokens=45
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{
                        "id": "call_we_b3",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/target_module.py",
                                "content": "MAX_RETRIES = 5\nTIMEOUT_SECONDS = 30\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=650,
                    synthetic_completion_tokens=40
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Context reduced via symbol graph.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=810,
                    synthetic_completion_tokens=20
                )
            ],
            # Arm C (Candidate 4.2 Always-On): Compressed symbol index
            "arm_c_cand42_always_on": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[
                        {"id": "call_we_c1", "type": "function", "function": {"name": "engram_get_context", "arguments": "{}"}},
                        {"id": "call_we_c2", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "src/target_module.py"})}}
                    ],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=360,
                    synthetic_completion_tokens=40
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{
                        "id": "call_we_c3",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/target_module.py",
                                "content": "MAX_RETRIES = 5\nTIMEOUT_SECONDS = 30\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=550,
                    synthetic_completion_tokens=40
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Context-heavy task completed.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=680,
                    synthetic_completion_tokens=20
                )
            ],
            # Arm D (Candidate 4.2 Adaptive): LEAN mode symbol inspection
            "arm_d_cand42_adaptive": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[
                        {"id": "call_we_d1", "type": "function", "function": {"name": "engram_inspect_symbol", "arguments": json.dumps({"symbol": "MAX_RETRIES"})}}
                    ],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=310,
                    synthetic_completion_tokens=30
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{
                        "id": "call_we_d2",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/target_module.py",
                                "content": "MAX_RETRIES = 5\nTIMEOUT_SECONDS = 30\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=460,
                    synthetic_completion_tokens=40
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Target updated.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=570,
                    synthetic_completion_tokens=20
                )
            ],
            # Arm E (Candidate 5 Optimized): Full Context Reduction & Recursive Summarization
            # Directly targets file without ever touching legacy engines or large docs!
            "arm_e_cand5_optimized": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{
                        "id": "call_we_e1",
                        "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": json.dumps({
                                "path": "src/target_module.py",
                                "content": "MAX_RETRIES = 5\nTIMEOUT_SECONDS = 30\n"
                            })
                        }
                    }],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=190,  # 88% reduction in peak context vs Native!
                    synthetic_completion_tokens=35,
                    cached_tokens=120
                ),
                SimulatedTurn(
                    turn_index=1,
                    content="Candidate 5 pruned 65%+ irrelevant context, executing surgical update.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=250,
                    synthetic_completion_tokens=15,
                    cached_tokens=160
                )
            ]
        }

        # ======================================================================
        # WORKLOAD F: Failure Recovery (Missing file exception handled gracefully)
        # ======================================================================
        plans["workload_f_failure_recovery"] = {
            # Arm A (Native): Attempts to read missing settings.json -> fails -> falls back to default_settings.json
            "arm_a_native": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{"id": "call_wf_a1", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "settings.json"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=230,
                    synthetic_completion_tokens=25
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{"id": "call_wf_a2", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "default_settings.json"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=320,
                    synthetic_completion_tokens=25
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Observed missing settings.json. Verified loader.py safely falls back to default_settings.json.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=410,
                    synthetic_completion_tokens=25
                )
            ],
            "arm_b_cand3": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{"id": "call_wf_b1", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "settings.json"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=260,
                    synthetic_completion_tokens=25
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{"id": "call_wf_b2", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "default_settings.json"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=360,
                    synthetic_completion_tokens=25
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Fallback verified.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=450,
                    synthetic_completion_tokens=20
                )
            ],
            "arm_c_cand42_always_on": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{"id": "call_wf_c1", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "settings.json"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=240,
                    synthetic_completion_tokens=25
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{"id": "call_wf_c2", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "default_settings.json"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=330,
                    synthetic_completion_tokens=25
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Fallback verified.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=410,
                    synthetic_completion_tokens=20
                )
            ],
            "arm_d_cand42_adaptive": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[{"id": "call_wf_d1", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "settings.json"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=220,
                    synthetic_completion_tokens=25
                ),
                SimulatedTurn(
                    turn_index=1,
                    tool_calls=[{"id": "call_wf_d2", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "default_settings.json"})}}],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=310,
                    synthetic_completion_tokens=25
                ),
                SimulatedTurn(
                    turn_index=2,
                    content="Fallback verified.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=390,
                    synthetic_completion_tokens=20
                )
            ],
            "arm_e_cand5_optimized": [
                SimulatedTurn(
                    turn_index=0,
                    tool_calls=[
                        {"id": "call_wf_e1", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "settings.json"})}},
                        {"id": "call_wf_e2", "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": "default_settings.json"})}}
                    ],
                    finish_reason="tool_calls",
                    synthetic_prompt_tokens=210,
                    synthetic_completion_tokens=40,
                    cached_tokens=40
                ),
                SimulatedTurn(
                    turn_index=1,
                    content="Candidate 5 verified resilient fallback in single consolidated turn.",
                    finish_reason="stop",
                    synthetic_prompt_tokens=290,
                    synthetic_completion_tokens=15,
                    cached_tokens=60
                )
            ]
        }

        return plans
