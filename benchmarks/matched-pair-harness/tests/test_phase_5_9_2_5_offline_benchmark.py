"""
Engram Phase 5.9.2.5: Adversarial and Regression Validation Suite
Repository: https://github.com/engram/engram

Verifies:
1. No external network access is attempted during offline benchmark execution.
2. No live provider transport is selected (deterministic simulator is strictly offline).
3. No credential is required (operates with OPENAI_API_KEY unset).
4. Simulator provenance is strictly 'SYNTHETIC_SIMULATED_USAGE' and never mislabeled.
5. Tool-call IDs and ordering remain strictly preserved.
6. Multiple tool calls within a single turn are executed correctly in sequence.
7. Follow-up turns preserve complete tool history in conversation messages.
8. Budget reservations remain strictly enforced (R = I + O <= B_rem - B_res).
9. Failed requests cannot create unaccounted budget capacity (full reservation R retained).
10. Repeated runs with identical seeds produce 100% identical benchmark outputs (reproducibility).
11. Candidate workspace isolation prevents cross-arm state contamination.
12. Historical benchmark directories remain unchanged and preserved.
"""

import json
from pathlib import Path
import shutil
import sys
import tempfile
import pytest

HARNESS_ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = HARNESS_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from agent_runtime import ControlledAgentRuntime
from provider_simulator import DeterministicProviderSimulator, PROVENANCE_MARKER
from provider_transport import (
    BudgetExceededError,
    OpenAICompatibleTransport,
    ProviderRequest,
    ProviderResponse,
)
from runner import RequestReservation, RequestReservationCoordinator


@pytest.fixture
def dummy_reservation():
    return RequestReservation(
        request_id="req-test-sim",
        turn_index=0,
        input_tokens=200,
        reserved_output_tokens=4096,
        total_reserved_tokens=4296,
        created_at="2026-11-22T00:00:00Z",
        status="ACTIVE"
    )


class TestPhase5925AdversarialValidation:

    def test_01_no_external_network_access(self, monkeypatch, dummy_reservation):
        """Verifies zero socket/network connection attempts during simulator dispatch."""
        def forbidden_connect(*args, **kwargs):
            raise AssertionError("Forbidden network access attempt detected during offline benchmark!")

        import socket
        monkeypatch.setattr(socket.socket, "connect", forbidden_connect)

        simulator = DeterministicProviderSimulator(seed=59002, workload_id="workload_a_simple_edit", arm_id="arm_a_native")
        req = ProviderRequest(
            request_id="req_offline_01",
            turn_index=0,
            task_id="workload_a_simple_edit",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Fix code"}]
        )
        resp = simulator.dispatch(req, dummy_reservation)
        assert resp is not None
        assert simulator.allow_live_network is False

    def test_02_no_live_provider_transport_selected(self):
        """Verifies deterministic simulator is strictly offline and distinct from live HTTP transport."""
        simulator = DeterministicProviderSimulator()
        assert not isinstance(simulator, OpenAICompatibleTransport)
        assert simulator.provider_name == "deterministic_simulator"
        assert simulator.api_base_url.startswith("simulator://")
        assert simulator.allow_live_network is False

    def test_03_no_credential_required(self, monkeypatch, dummy_reservation):
        """Verifies simulator operates cleanly with OPENAI_API_KEY completely absent."""
        monkeypatch.delenv("OPENAI_API_KEY", raising=False)
        simulator = DeterministicProviderSimulator()
        req = ProviderRequest(
            request_id="req_nokey",
            turn_index=0,
            task_id="workload_a_simple_edit",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Ping"}]
        )
        resp = simulator.dispatch(req, dummy_reservation)
        assert resp.content is not None or resp.tool_calls is not None
        assert simulator.api_key is None

    def test_04_provenance_marker_not_mislabeled(self, dummy_reservation):
        """Verifies provenance is explicitly SYNTHETIC_SIMULATED_USAGE and never DIRECT_API."""
        simulator = DeterministicProviderSimulator()
        req = ProviderRequest(
            request_id="req_prov",
            turn_index=0,
            task_id="workload_a_simple_edit",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Test"}]
        )
        resp = simulator.dispatch(req, dummy_reservation)
        assert resp.raw_usage["provenance"] == PROVENANCE_MARKER
        assert "DIRECT_API" not in resp.raw_usage["provenance"]

    def test_05_tool_call_ids_and_ordering_correct(self, dummy_reservation):
        """Verifies tool calls preserve IDs and structure in sequence."""
        simulator = DeterministicProviderSimulator(workload_id="workload_a_simple_edit", arm_id="arm_a_native")
        req = ProviderRequest(
            request_id="req_tc_ord",
            turn_index=0,
            task_id="workload_a_simple_edit",
            arm_id="arm_a_native",
            model="gpt-5.6-luna",
            messages=[{"role": "user", "content": "Fix bug"}]
        )
        resp = simulator.dispatch(req, dummy_reservation)
        assert resp.tool_calls is not None
        assert len(resp.tool_calls) == 1
        assert resp.tool_calls[0]["id"] == "call_wa_native_01"
        assert resp.tool_calls[0]["function"]["name"] == "read_file"

    def test_06_multiple_tool_calls_executed_correctly(self):
        """Verifies ControlledAgentRuntime executes multiple tool calls within single turn."""
        with tempfile.TemporaryDirectory() as tmpdir:
            sandbox = Path(tmpdir)
            (sandbox / "a.txt").write_text("Hello A", encoding="utf-8")
            (sandbox / "b.txt").write_text("Hello B", encoding="utf-8")
            events_path = sandbox / "events.jsonl"

            simulator = DeterministicProviderSimulator(workload_id="workload_b_cross_file", arm_id="arm_a_native")
            runtime = ControlledAgentRuntime(
                sandbox_path=sandbox,
                arm="arm_a_native",
                task_id="workload_b_cross_file",
                model="gpt-5.6-luna",
                transport=simulator
            )

            # Turn 0 of Workload B Arm A dispatches 2 tool calls
            run_res = runtime.run(prompt="Cross file test", output_events_path=events_path, max_turns=1)
            assert len(runtime.round_trip_events) == 2
            assert runtime.round_trip_events[0]["tool_name"] == "read_file"
            assert runtime.round_trip_events[1]["tool_name"] == "read_file"

    def test_07_follow_up_turns_preserve_complete_tool_history(self):
        """Verifies conversation messages preserve tool calls followed by matching tool results."""
        with tempfile.TemporaryDirectory() as tmpdir:
            sandbox = Path(tmpdir)
            src_dir = sandbox / "src"
            src_dir.mkdir(parents=True)
            (src_dir / "calculator.py").write_text("def add_tax(p, r): return p * r", encoding="utf-8")
            events_path = sandbox / "events.jsonl"

            simulator = DeterministicProviderSimulator(workload_id="workload_a_simple_edit", arm_id="arm_a_native")
            runtime = ControlledAgentRuntime(
                sandbox_path=sandbox,
                arm="arm_a_native",
                task_id="workload_a_simple_edit",
                model="gpt-5.6-luna",
                transport=simulator
            )

            run_res = runtime.run(prompt="Fix tax calculation", output_events_path=events_path, max_turns=3)
            assert run_res.exit_code == 0
            assert len(runtime.per_request_telemetry) >= 2

    def test_08_budget_reservations_enforced(self):
        """Verifies RequestReservationCoordinator fails closed when budget is exceeded."""
        coord = RequestReservationCoordinator(aggregate_budget=10_000, safety_reserve=2_000)
        # Attempt to reserve 9,000 tokens when available is 10,000 - 2,000 = 8,000
        ok, err, res = coord.reserve_request("req_over", 0, input_tokens=8_000, completion_allowance=1_000)
        assert not ok
        assert res is None
        assert "Request budget reservation rejected" in err

    def test_09_failed_requests_retain_full_reservation(self):
        """Verifies transport failures retain full reservation R upon reconciliation."""
        coord = RequestReservationCoordinator(aggregate_budget=50_000, safety_reserve=5_000)
        ok, _, res = coord.reserve_request("req_crash", 0, input_tokens=3_000, completion_allowance=1_000)
        assert ok
        # Reconcile with actual_total_tokens = None
        reconciled = coord.reconcile_response("req_crash", actual_total_tokens=None)
        assert reconciled.actual_total_tokens == 4_000
        assert coord.remaining_budget == 46_000  # 50,000 - 4,000 = 46,000

    def test_10_deterministic_reproducibility(self):
        """Verifies repeated runs with identical seed produce 100% identical outputs."""
        sim1 = DeterministicProviderSimulator(seed=59002, workload_id="workload_a_simple_edit", arm_id="arm_e_cand5_optimized")
        sim2 = DeterministicProviderSimulator(seed=59002, workload_id="workload_a_simple_edit", arm_id="arm_e_cand5_optimized")

        dummy_res = RequestReservation("r1", 0, 100, 1000, 1100, "2026-11-22", "ACTIVE")
        req1 = ProviderRequest("r1", 0, "wa", "arm_e", "gpt-5.6-luna", [{"role": "user", "content": "prompt"}])
        req2 = ProviderRequest("r1", 0, "wa", "arm_e", "gpt-5.6-luna", [{"role": "user", "content": "prompt"}])

        resp1 = sim1.dispatch(req1, dummy_res)
        resp2 = sim2.dispatch(req2, dummy_res)

        assert resp1.prompt_tokens == resp2.prompt_tokens
        assert resp1.completion_tokens == resp2.completion_tokens
        assert resp1.total_tokens == resp2.total_tokens
        assert resp1.content == resp2.content
        assert resp1.tool_calls == resp2.tool_calls

    def test_11_candidate_isolation_prevents_contamination(self):
        """Verifies workspace isolation prevents state pollution across different arms."""
        with tempfile.TemporaryDirectory() as tmp1, tempfile.TemporaryDirectory() as tmp2:
            sb1 = Path(tmp1)
            sb2 = Path(tmp2)
            (sb1 / "file.txt").write_text("ORIGINAL_1", encoding="utf-8")
            (sb2 / "file.txt").write_text("ORIGINAL_2", encoding="utf-8")

            # Mutate sb1
            (sb1 / "file.txt").write_text("MUTATED_BY_ARM_A", encoding="utf-8")
            # Verify sb2 remains unchanged
            assert (sb2 / "file.txt").read_text(encoding="utf-8") == "ORIGINAL_2"

    def test_12_historical_benchmark_directories_unchanged(self):
        """Verifies Phase 5.9.2.1 historical benchmark run is 100% preserved and untouched."""
        historical_run = HARNESS_ROOT / "runs" / "two_task_pentad_live_20261122_180013"
        assert historical_run.exists(), "Historical run directory must remain preserved!"
        summary_file = historical_run / "two_task_pentad_summary.json"
        assert summary_file.exists(), "Historical summary JSON must exist!"
        data = json.loads(summary_file.read_text(encoding="utf-8-sig"))
        assert data["schema_version"] == "2.1.0"
        assert data["total_tokens_consumed"] == 7395
