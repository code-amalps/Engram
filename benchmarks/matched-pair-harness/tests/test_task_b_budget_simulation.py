"""
Task B budget requirements and turn-by-turn simulation for Engram benchmark harness.

Models:
1. Ground-truth token consumption trajectories from Phase 5.4.7 (Arms A, B, C, D).
2. Context growth rate and marginal turn cost.
3. Ceiling simulations: 350k, 450k, 500k, 550k, 600k, 650k tokens.
4. Headroom reserve impact: 45,000 vs 50,000 tokens.
5. Projected candidate 4.2 trajectory with compacted get_context (saving ~35k tokens/turn).
"""

import json
import glob
import unittest
from pathlib import Path
from typing import Dict, List, Tuple

REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent


def load_phase_5_4_7_trajectories() -> Dict[str, List[Dict[str, int]]]:
    """Loads actual turn-by-turn token records from Phase 5.4.7 session rollouts."""
    tids = {
        "arm_a_native": "01a1f41e-4e01-7280-ae21-113b239fcdc1",
        "arm_b_cand3_always_on": "01a1f425-7b35-7a83-9471-34123ff9a26e",
        "arm_c_cand4_always_on": "01a1f42a-fee7-7332-abbe-d41da3764401",
        "arm_d_cand4_adaptive": "01a1f42e-dfa5-7253-bec6-707dd12abb86"
    }
    trajectories = {}
    for arm, tid in tids.items():
        matches = glob.glob(f"C:/Users/Amal P S/.codex/sessions/**/*{tid}*.jsonl", recursive=True)
        if not matches:
            continue
        with open(matches[0], "r", encoding="utf-8") as f:
            records = [json.loads(l) for l in f if json.loads(l).get("type") == "token_usage_record"]
        turns = []
        for r in records:
            p = r.get("payload", {})
            u = p.get("usage", {})
            tu = p.get("thread_token_usage", {})
            turns.append({
                "req_input": u.get("input_tokens", 0),
                "req_output": u.get("output_tokens", 0),
                "cum_input": tu.get("input_tokens", 0),
                "cum_output": tu.get("output_tokens", 0),
                "cum_total": tu.get("input_tokens", 0) + tu.get("output_tokens", 0)
            })
        trajectories[arm] = turns
    return trajectories


def simulate_turns_allowed(
    turns: List[Dict[str, int]],
    ceiling: int,
    reserve: int,
    extrapolate_turns: int = 15
) -> Tuple[int, str]:
    """
    Simulates how many turns can execute under a given ceiling and reserve.
    Returns (turns_allowed, halt_reason).
    """
    cum_tokens = 0
    history = list(turns)
    
    # If we need more turns than recorded, extrapolate using the last turn's context growth
    if len(history) < extrapolate_turns and len(history) > 0:
        last_turn = history[-1]
        last_req_input = last_turn["req_input"]
        avg_output = int(sum(t["req_output"] for t in history) / len(history))
        for ext_i in range(len(history), extrapolate_turns):
            # Assume steady-state context with minor growth (+1,500 tokens/turn for new tool outputs)
            next_input = last_req_input + (ext_i - len(history) + 1) * 1500
            cum_tokens_prev = history[-1]["cum_total"]
            history.append({
                "req_input": next_input,
                "req_output": avg_output,
                "cum_input": history[-1]["cum_input"] + next_input,
                "cum_output": history[-1]["cum_output"] + avg_output,
                "cum_total": cum_tokens_prev + next_input + avg_output
            })

    allowed = 0
    cum_total = 0
    for idx, t in enumerate(history):
        req_input = t["req_input"]
        # Pre-dispatch check before turn:
        # Remaining budget = ceiling - cum_total
        # Guard condition: remaining < (req_input + reserve)
        remaining = ceiling - cum_total
        est_next_turn = req_input
        if remaining < reserve or remaining < est_next_turn:
            return allowed, f"HALTED_PRE_DISPATCH_TURN_{idx+1}: remaining={remaining} < required (reserve={reserve}, est_cost={est_next_turn})"
        allowed += 1
        cum_total += (t["req_input"] + t["req_output"])
        if cum_total >= ceiling:
            return allowed, f"HALTED_BUDGET_EXHAUSTED_TURN_{idx+1}: cum_total={cum_total} >= ceiling={ceiling}"

    return allowed, "COMPLETED_WITHOUT_HALT"


class TestTaskBBudgetSimulation(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.trajectories = load_phase_5_4_7_trajectories()

    def test_reproduce_phase_5_4_7_halt(self):
        """Verify that under ceiling=350,000 and reserve=45,000, all arms halt at turn 7-9."""
        for arm, turns in self.trajectories.items():
            allowed, reason = simulate_turns_allowed(turns, ceiling=350000, reserve=45000)
            self.assertIn(allowed, [7, 8, 9], f"Arm {arm} should halt between turns 7-9 under 350k budget")
            self.assertIn("HALTED", reason)

    def test_ceiling_sensitivity_analysis(self):
        """Simulate turn capacity across ceilings: 350k, 450k, 500k, 550k, 600k, 650k."""
        ceilings = [350000, 450000, 500000, 550000, 600000, 650000]
        results = {}

        for ceil in ceilings:
            results[ceil] = {}
            for arm, turns in self.trajectories.items():
                allowed, reason = simulate_turns_allowed(turns, ceiling=ceil, reserve=45000, extrapolate_turns=14)
                results[ceil][arm] = allowed

        # 350k: strictly 7-9 turns
        for arm in self.trajectories:
            self.assertLessEqual(results[350000][arm], 9)

        # 500k: allows 10-12 turns (sufficient for implementation + test fix)
        for arm in self.trajectories:
            self.assertGreaterEqual(results[500000][arm], 10)

        # 550k-600k: allows 12-14 turns (comfortably covers 11-12 turn requirement)
        for arm in self.trajectories:
            self.assertGreaterEqual(results[550000][arm], 11)
            self.assertGreaterEqual(results[600000][arm], 12)

    def test_candidate_4_2_compaction_benefit_simulation(self):
        """
        Simulate Candidate 4.2 benefit in Arm D.
        Candidate 4 returned 154,608 B (~38,650 tokens) on get_context.
        Candidate 4.2 returns 14,599 B (~3,650 tokens) on get_context.
        Delta = 35,000 tokens of context saved per turn starting at turn 3!
        """
        arm_d_turns = self.trajectories.get("arm_d_cand4_adaptive")
        if not arm_d_turns:
            self.skipTest("Arm D trajectory not found")

        # Create simulated Candidate 4.2 trajectory
        cand4_2_turns = []
        cum_in = 0
        cum_out = 0
        for idx, t in enumerate(arm_d_turns):
            req_in = t["req_input"]
            if idx >= 2:  # After turn 2 where get_context happened
                # Context is ~35,000 tokens leaner because of compacted omission manifest
                req_in = max(15000, req_in - 35000)
            req_out = t["req_output"]
            cum_in += req_in
            cum_out += req_out
            cand4_2_turns.append({
                "req_input": req_in,
                "req_output": req_out,
                "cum_input": cum_in,
                "cum_output": cum_out,
                "cum_total": cum_in + cum_out
            })

        # Under 350k budget, uncompacted Candidate 4 allowed only 7 turns
        allowed_c4, _ = simulate_turns_allowed(arm_d_turns, ceiling=350000, reserve=45000)
        # Candidate 4.2 should allow significantly more turns due to lower context accumulation
        allowed_c4_2, _ = simulate_turns_allowed(cand4_2_turns, ceiling=350000, reserve=45000, extrapolate_turns=12)

        self.assertEqual(allowed_c4, 7)
        self.assertGreater(allowed_c4_2, allowed_c4, "Candidate 4.2 compaction must extend available turns under identical ceiling")


if __name__ == "__main__":
    unittest.main()
