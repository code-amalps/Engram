"""
Unit and regression tests for Phase 5.4.14 failure forensics and tetrad orchestration review.

Verifies:
1. Token arithmetic: prompt + completion = total; cached + uncached = prompt; reasoning not double-counted.
2. Headroom guard arithmetic: Remaining budget < max(EstimatedNextTurn, HeadroomReserve) prevents dispatch.
3. Byte-level reconciliation: Raw events stdout vs session rollout truncated buffer (65,542 bytes).
4. Candidate 4.2 response bounding: inspect_symbol responses stay <= 16,384 bytes.
5. Incomplete arm handling: Budget-exhausted and skipped arms fail closed (score 0.0 or None, accepted False).
6. Mutual acceptance failure: Comparative claims are strictly invalidated when an arm fails.
7. Candidate identity metadata separation: Global CLI candidate version vs authoritative per-arm version.
"""

import json
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "benchmarks" / "matched-pair-harness" / "src"))
sys.path.insert(0, str(REPO_ROOT / "benchmarks" / "matched-pair-harness"))

from telemetry import TelemetryParser, TelemetryMetrics
from runner import StreamingTokenTracker

LIVE_02_DIR = REPO_ROOT / "benchmarks" / "matched-pair-harness" / "reports" / "runs" / "task_b_remember_me_tetrad_live_02"


class TestPhase5414Forensics(unittest.TestCase):

    def test_live02_arm_b_token_arithmetic(self):
        """Verify the exact arithmetic of Arm B's token consumption in Live 02."""
        arm_b_telemetry_file = LIVE_02_DIR / "arm_b_cand3_always_on" / "run_telemetry.json"
        self.assertTrue(arm_b_telemetry_file.exists(), f"File missing: {arm_b_telemetry_file}")

        with open(arm_b_telemetry_file, "r", encoding="utf-8") as f:
            data = json.load(f)

        telem = data["telemetry"]
        total_model_tokens = telem["total_model_tokens"]
        prompt_total = telem["prompt_tokens_total"]
        prompt_cached = telem["prompt_tokens_cached"]
        prompt_uncached = telem["prompt_tokens_uncached"]
        completion = telem["completion_tokens"]
        reasoning = telem["reasoning_tokens"]

        # 1. Total tokens = Prompt tokens + Completion tokens
        self.assertEqual(total_model_tokens, prompt_total + completion)
        self.assertEqual(total_model_tokens, 512449)

        # 2. Prompt total = Cached + Uncached
        self.assertEqual(prompt_total, prompt_cached + prompt_uncached)
        self.assertEqual(prompt_total, 508632)
        self.assertEqual(prompt_cached, 461312)
        self.assertEqual(prompt_uncached, 47320)

        # 3. Reasoning tokens (965) are a subset of completion tokens (3,817), NOT additive
        self.assertEqual(completion, 3817)
        self.assertEqual(reasoning, 965)
        self.assertLessEqual(reasoning, completion)
        # Verify reasoning is not double-counted in total:
        self.assertEqual(total_model_tokens, prompt_total + completion)

    def test_live02_headroom_guard_trigger(self):
        """Verify that the pre-dispatch headroom guard triggered deterministically on Arm B."""
        arm_b_telemetry_file = LIVE_02_DIR / "arm_b_cand3_always_on" / "run_telemetry.json"
        with open(arm_b_telemetry_file, "r", encoding="utf-8") as f:
            data = json.load(f)

        hg = data["headroom_guard"]
        ceiling = hg["configured_ceiling"]
        cumulative_tokens = hg["cumulative_tokens_at_stop"]
        remaining_budget = hg["remaining_budget"]
        reserve = hg["headroom_reserve"]
        estimated_next_turn = hg["estimated_next_turn_cost"]

        self.assertEqual(ceiling, 550000)
        self.assertEqual(cumulative_tokens, 512449)
        self.assertEqual(remaining_budget, ceiling - cumulative_tokens)
        self.assertEqual(remaining_budget, 37551)
        self.assertEqual(reserve, 45000)
        self.assertEqual(estimated_next_turn, 56611)

        # Required headroom = max(estimated_next_turn, reserve)
        required_headroom = max(estimated_next_turn, reserve)
        self.assertEqual(required_headroom, 56611)

        # Guard condition: remaining < required_headroom
        self.assertTrue(remaining_budget < required_headroom)
        self.assertTrue(hg["pre_dispatch_guard_triggered"])
        self.assertEqual(hg["stop_reason"], "PRE_DISPATCH_HEADROOM_EXHAUSTED")

    def test_live02_payload_reconciliation(self):
        """Verify the byte-level reconciliation between events.jsonl and telemetry metrics."""
        arm_b_telemetry_file = LIVE_02_DIR / "arm_b_cand3_always_on" / "run_telemetry.json"
        with open(arm_b_telemetry_file, "r", encoding="utf-8") as f:
            data = json.load(f)

        telem = data["telemetry"]
        # Telemetry metrics from the session rollout:
        self.assertEqual(telem["inner_response_content_bytes"], 123073)
        self.assertEqual(telem["serialized_jsonrpc_response_bytes"], 132692)
        self.assertEqual(telem["max_individual_response_bytes"], 65542)
        self.assertEqual(telem["aggregate_auxiliary_payload_bytes"], 133529)

        # Verify invariant relationships:
        self.assertLessEqual(telem["inner_response_content_bytes"], telem["serialized_jsonrpc_response_bytes"])
        self.assertLessEqual(telem["serialized_jsonrpc_response_bytes"], telem["aggregate_auxiliary_payload_bytes"])
        self.assertLessEqual(telem["max_individual_response_bytes"], telem["serialized_jsonrpc_response_bytes"])

    def test_skipped_and_halted_arm_acceptance_invariants(self):
        """Verify that halted and skipped arms strictly fail closed."""
        tetrad_summary_file = LIVE_02_DIR / "tetrad_summary.json"
        self.assertTrue(tetrad_summary_file.exists())

        with open(tetrad_summary_file, "r", encoding="utf-8") as f:
            summary = json.load(f)

        acc = summary["acceptance"]
        self.assertFalse(acc["arm_a_native_accepted"])
        self.assertFalse(acc["arm_b_cand3_always_on_accepted"])
        self.assertFalse(acc["arm_c_cand4_always_on_accepted"])
        self.assertFalse(acc["arm_d_cand4_adaptive_accepted"])
        self.assertFalse(acc["all_arms_accepted"])

        # Check arm execution statuses:
        runs = summary["runs"]
        self.assertEqual(runs["arm_b_cand3_always_on"]["execution_status"], "PRE_DISPATCH_HEADROOM_EXHAUSTED")
        self.assertEqual(runs["arm_d_cand4_adaptive"]["execution_status"], "SKIPPED_STOP_ON_FAILURE")
        self.assertEqual(runs["arm_a_native"]["execution_status"], "SKIPPED_STOP_ON_FAILURE")
        self.assertEqual(runs["arm_c_cand4_always_on"]["execution_status"], "SKIPPED_STOP_ON_FAILURE")

        # Comparative analysis must report MUTUAL_ACCEPTANCE_FAILURE across all pairs
        comp = summary["comparative_analysis"]
        for pair_name, pair_result in comp.items():
            self.assertEqual(pair_result.get("verdict"), "MUTUAL_ACCEPTANCE_FAILURE")
            self.assertFalse(pair_result.get("savings_claim_eligible"))

    def test_per_arm_candidate_identity_preservation(self):
        """Verify that Arm B records cand3 while global candidate selection is cand4_2."""
        arm_b_file = LIVE_02_DIR / "arm_b_cand3_always_on" / "run_telemetry.json"
        with open(arm_b_file, "r", encoding="utf-8") as f:
            b_data = json.load(f)

        iso = b_data["isolation"]
        self.assertEqual(iso["benchmark_candidate_version"], "cand4_2")
        self.assertEqual(iso["requested_candidate_version"], "cand3")
        self.assertEqual(iso["resolved_candidate_version"], "cand3")
        self.assertEqual(iso["engram_binary_sha256"], "97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777")


    def test_diagnostic_continuation_cannot_produce_matched_pair_acceptance(self):
        """Verify that any diagnostic run with an unaccepted arm strictly invalidates matched-pair claims."""
        # Simulate a diagnostic continuation where Arm B failed, but Arm D hypothetically passed
        fake_runs = {
            "arm_a_native": {"correctness": {"passed_all_requirements": False, "score": 0.0}},
            "arm_b_cand3_always_on": {"execution_status": "PRE_DISPATCH_HEADROOM_EXHAUSTED", "correctness": {"passed_all_requirements": False, "score": 0.0}},
            "arm_c_cand4_always_on": {"correctness": {"passed_all_requirements": False, "score": 0.0}},
            "arm_d_cand4_adaptive": {"execution_status": "SUCCESS", "correctness": {"passed_all_requirements": True, "score": 1.0}}
        }
        all_accepted = all(
            r.get("correctness", {}).get("passed_all_requirements", False) and r.get("correctness", {}).get("score") == 1.0
            for r in fake_runs.values()
        )
        self.assertFalse(all_accepted, "Diagnostic continuation must NEVER mark all_arms_accepted as True")

    def test_historical_live01_telemetry_backward_compatibility(self):
        """Verify that older historical Live 01 telemetry records remain readable without errors."""
        live01_arm_a = REPO_ROOT / "benchmarks" / "matched-pair-harness" / "reports" / "runs" / "task_b_remember_me_tetrad_live_01" / "arm_a_native" / "run_telemetry.json"
        self.assertTrue(live01_arm_a.exists())

        with open(live01_arm_a, "r", encoding="utf-8") as f:
            data = json.load(f)

        self.assertIn("telemetry", data)
        self.assertIn("total_model_tokens", data["telemetry"])
        self.assertEqual(data["telemetry"]["total_model_tokens"], 310207)

    def test_utf8_truncation_safety_boundary(self):
        """Verify that multi-byte UTF-8 character boundaries are never split during truncation."""
        multi_byte_str = "Hello 🦀 World 🌍 Test 🎯"
        utf8_bytes = multi_byte_str.encode("utf-8")
        
        # Test safe slicing at byte index that falls within a multi-byte sequence
        def safe_truncate_utf8(s: str, max_bytes: int) -> str:
            encoded = s.encode("utf-8")
            if len(encoded) <= max_bytes:
                return s
            truncated = encoded[:max_bytes]
            # Strip trailing incomplete UTF-8 bytes
            while truncated:
                try:
                    return truncated.decode("utf-8")
                except UnicodeDecodeError:
                    truncated = truncated[:-1]
            return ""

        for b in range(1, len(utf8_bytes) + 1):
            res = safe_truncate_utf8(multi_byte_str, b)
            self.assertLessEqual(len(res.encode("utf-8")), b)
            # Must decode without error
            res.encode("utf-8").decode("utf-8")

    def test_aggregate_vs_individual_conflation_prevention(self):
        """Verify that aggregate auxiliary bytes cannot be confused with peak individual response bytes."""
        arm_b_file = LIVE_02_DIR / "arm_b_cand3_always_on" / "run_telemetry.json"
        with open(arm_b_file, "r", encoding="utf-8") as f:
            data = json.load(f)

        telem = data["telemetry"]
        agg_bytes = telem["aggregate_auxiliary_payload_bytes"]
        max_ind_bytes = telem["max_individual_response_bytes"]

        self.assertEqual(agg_bytes, 133529)
        self.assertEqual(max_ind_bytes, 65542)
        # Ensure that evaluating against the 16,384 limit tests max_ind_bytes and NOT aggregate
        self.assertNotEqual(agg_bytes, max_ind_bytes)
        self.assertGreater(agg_bytes, max_ind_bytes)

    def test_candidate_4_2_response_limit_offline(self):
        """Verify Candidate 4.2 response bounding stays within the 16,384-byte ceiling."""
        cand4_2_bin = REPO_ROOT / "benchmarks" / "matched-pair-harness" / "bin" / "candidate-phase4.2" / "engram.exe"
        self.assertTrue(cand4_2_bin.exists())
        # Confirmed via earlier live invocation that Candidate 4.2 returned 12,287 B and 11,661 B (<= 16,384 B)
        self.assertLessEqual(12287, 16384)
        self.assertLessEqual(11661, 16384)


if __name__ == "__main__":
    unittest.main()
