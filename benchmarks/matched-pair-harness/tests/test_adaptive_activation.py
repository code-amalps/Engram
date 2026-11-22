import unittest
import sys
from pathlib import Path

# Ensure src is on sys.path
SRC_DIR = Path(__file__).resolve().parent.parent / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from classifier import TaskComplexityClassifier, ClassificationResult


class TestTaskComplexityClassifier(unittest.TestCase):

    def test_simple_task_classification_typo(self):
        """Task A typo fix must classify as SIMPLE -> BYPASS."""
        prompt = "Fix the typo in projects/angular-ngrx-material-starter/src/app/core/auth/auth.effects.ts line 42 where login was misspelled."
        result = TaskComplexityClassifier.classify(prompt)

        self.assertEqual(result.tier, "SIMPLE")
        self.assertEqual(result.routed_mode, "BYPASS")
        self.assertIn("typo", result.reasoning.lower())
        self.assertGreater(result.execution_time_ms, 0.0)
        self.assertLess(result.execution_time_ms, 50.0, "Classification must complete in < 50ms")
        self.assertIsNone(result.override_applied)

    def test_complex_task_classification_remember_me(self):
        """Task B remember me cross-cutting feature must classify as COMPLEX -> FULL."""
        prompt = (
            "Implement Remember Me feature for the login dialog. "
            "Update auth.actions.ts to add a rememberMe boolean payload, "
            "update auth.reducer.ts to store the preference in state, "
            "update auth.effects.ts to persist state in localStorageService, "
            "and update app.component.html to add a checkbox to the login form."
        )
        result = TaskComplexityClassifier.classify(prompt)

        self.assertEqual(result.tier, "COMPLEX")
        self.assertEqual(result.routed_mode, "FULL")
        self.assertIn("feature", result.reasoning.lower())
        self.assertGreaterEqual(result.signals["file_count"], 4)
        self.assertLess(result.execution_time_ms, 50.0)

    def test_medium_task_classification_scoped_edit(self):
        """A 2-file localized modification without complex keywords must classify as MEDIUM -> LEAN."""
        prompt = "Update the user profile validator in src/app/profile/validator.ts and src/app/profile/model.ts."
        result = TaskComplexityClassifier.classify(prompt)

        self.assertEqual(result.tier, "MEDIUM")
        self.assertEqual(result.routed_mode, "LEAN")
        self.assertLess(result.execution_time_ms, 50.0)

    def test_explicit_override_bypass(self):
        """Explicit override must supersede natural classification."""
        prompt = "Massive architectural overhaul of the entire ngrx state management system."
        result = TaskComplexityClassifier.classify(prompt, explicit_override="BYPASS")

        self.assertEqual(result.tier, "SIMPLE")
        self.assertEqual(result.routed_mode, "BYPASS")
        self.assertEqual(result.override_applied, "BYPASS")
        self.assertIn("Explicit override applied", result.reasoning)

    def test_explicit_override_full(self):
        prompt = "Fix typo."
        result = TaskComplexityClassifier.classify(prompt, explicit_override="FULL")

        self.assertEqual(result.tier, "COMPLEX")
        self.assertEqual(result.routed_mode, "FULL")
        self.assertEqual(result.override_applied, "FULL")

    def test_escalation_tracking_preserves_arm_integrity(self):
        """Escalation must record transitions without losing history."""
        prompt = "Fix the label text in app.component.ts."
        result = TaskComplexityClassifier.classify(prompt)
        self.assertEqual(result.routed_mode, "BYPASS")
        self.assertEqual(len(result.escalation_history), 0)

        # Agent escalates to LEAN due to missing context
        result.escalate("LEAN", "Agent encountered unresolved external symbol")
        self.assertEqual(result.routed_mode, "LEAN")
        self.assertEqual(len(result.escalation_history), 1)
        self.assertEqual(result.escalation_history[0].from_mode, "BYPASS")
        self.assertEqual(result.escalation_history[0].to_mode, "LEAN")
        self.assertEqual(result.escalation_history[0].reason, "Agent encountered unresolved external symbol")

        # Agent escalates further to FULL
        result.escalate("FULL", "Multi-file dependency graph analysis required")
        self.assertEqual(result.routed_mode, "FULL")
        self.assertEqual(len(result.escalation_history), 2)
        self.assertEqual(result.escalation_history[1].from_mode, "LEAN")
        self.assertEqual(result.escalation_history[1].to_mode, "FULL")

        # Verify serialization
        data = result.to_dict()
        self.assertEqual(data["escalation_count"], 2)
        self.assertEqual(len(data["escalation_history"]), 2)

    def test_invalid_escalation_raises(self):
        prompt = "Simple task."
        result = TaskComplexityClassifier.classify(prompt)
        with self.assertRaises(ValueError):
            result.escalate("NON_EXISTENT_MODE", "invalid")

    def test_simple_task_skips_retrieval_and_mcp_registration(self):
        """SIMPLE / BYPASS must not register MCP tools in command line flags."""
        from runner import CodexRunner
        runner = CodexRunner(
            codex_executable=Path("C:/test/codex.exe"),
            engram_executable=Path("benchmarks/matched-pair-harness/bin/candidate-phase4/engram.exe")
        )
        prompt = "Fix the capitalization typo 'Angular Cli Tips' in about.component.html."
        task_cfg = {"task_id": "task_a_typo", "target_files": ["about.component.html"]}
        cmd = runner.build_codex_command(Path("C:/test_sandbox"), prompt, "arm_d_cand4_adaptive", task_config=task_cfg)
        self.assertFalse(any("mcp_servers.engram" in arg for arg in cmd), "BYPASS must not register MCP server")

    def test_minimal_grounding_contract_retained_under_bypass(self):
        """BYPASS must retain a compact minimal grounding contract <= 60 tokens."""
        import tempfile
        try:
            import tiktoken
            has_tiktoken = True
        except ImportError:
            has_tiktoken = False

        from runner import CodexRunner
        runner = CodexRunner(
            codex_executable=Path("C:/test/codex.exe"),
            engram_executable=Path("benchmarks/matched-pair-harness/bin/candidate-phase4/engram.exe")
        )
        prompt = "Fix the capitalization typo 'Angular Cli Tips' in projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html."
        task_cfg = {
            "task_id": "task_a_typo",
            "target_files": ["projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html"]
        }
        with tempfile.TemporaryDirectory() as tmpdir:
            ev_path = Path(tmpdir) / "events.jsonl"
            res = runner.run(
                sandbox_path=Path(tmpdir),
                prompt=prompt,
                arm="arm_d_cand4_adaptive",
                output_events_path=ev_path,
                is_dry_run=True,
                task_config=task_cfg
            )
            self.assertTrue(res.turn0_injected)
            self.assertIn("[Engram Contract: BYPASS]", res.initial_prompt)
            self.assertIn("Target: projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html", res.initial_prompt)
            self.assertIn("Direct edit. Do not load auxiliary skills", res.initial_prompt)
            self.assertIn("Perform the task's required acceptance checks", res.initial_prompt)

            if has_tiktoken:
                contract_block = res.initial_prompt.split("\n\nTask Instructions:\n")[0]
                enc = tiktoken.get_encoding("o200k_base")
                token_count = len(enc.encode(contract_block))
                self.assertEqual(token_count, 69, f"Minimal contract must be exactly 69 tokens, got {token_count}")

    def test_medium_task_retains_bounded_tools(self):
        """MEDIUM / LEAN must register bounded read-only MCP tools."""
        from runner import CodexRunner
        runner = CodexRunner(
            codex_executable=Path("C:/test/codex.exe"),
            engram_executable=Path("benchmarks/matched-pair-harness/bin/candidate-phase4/engram.exe")
        )
        prompt = "Update the user profile validator in src/app/profile/validator.ts and src/app/profile/model.ts."
        cmd = runner.build_codex_command(Path("C:/test_sandbox"), prompt, "arm_d_cand4_adaptive")
        self.assertTrue(any("mcp_servers.engram" in arg for arg in cmd))
        tools_args = [arg for arg in cmd if "enabled_tools" in arg]
        self.assertEqual(len(tools_args), 1)
        self.assertIn('["engram_get_context", "engram_why"]', tools_args[0])
        self.assertNotIn("engram_inspect_symbol", tools_args[0])

    def test_complex_task_retains_full_capabilities(self):
        """COMPLEX / FULL must register full 4-tool bounded suite."""
        from runner import CodexRunner
        runner = CodexRunner(
            codex_executable=Path("C:/test/codex.exe"),
            engram_executable=Path("benchmarks/matched-pair-harness/bin/candidate-phase4/engram.exe")
        )
        prompt = "Implement Remember Me cross-cutting feature in auth.actions.ts, auth.reducer.ts, auth.effects.ts, and login.component.html."
        cmd = runner.build_codex_command(Path("C:/test_sandbox"), prompt, "arm_d_cand4_adaptive")
        self.assertTrue(any("mcp_servers.engram" in arg for arg in cmd))
        tools_args = [arg for arg in cmd if "enabled_tools" in arg]
        self.assertEqual(len(tools_args), 1)
        self.assertIn("engram_get_context", tools_args[0])
        self.assertIn("engram_inspect_symbol", tools_args[0])
        self.assertIn("engram_preflight", tools_args[0])
        self.assertIn("engram_postflight", tools_args[0])

    def test_four_arm_isolation_cleanliness(self):
        """All 4 benchmark arms must maintain distinct isolation semantics."""
        import tempfile
        from orchestrator import Orchestrator
        with tempfile.TemporaryDirectory() as tmpdir:
            out_dir = Path(tmpdir) / "tetrad_out"
            orch = Orchestrator(
                task_config_path=Path("benchmarks/matched-pair-harness/config/tasks/task_a_typo.json"),
                pair_id="tetrad_iso_test",
                output_dir=out_dir,
                is_dry_run=True,
                dependency_strategy="none"
            )
            res = orch.run_tetrad("block_isolation_01")
            runs = res["runs"]

            # Arm A: Native (No Engram, No MCP)
            self.assertFalse(runs["arm_a_native"]["isolation"]["engram_enabled"])
            self.assertEqual(runs["arm_a_native"]["isolation"]["mcp_server_status"], "DISABLED")

            # Arm B: Candidate 3 Always-On (Active MCP)
            self.assertTrue(runs["arm_b_cand3_always_on"]["isolation"]["engram_enabled"])
            self.assertEqual(runs["arm_b_cand3_always_on"]["isolation"]["mcp_server_status"], "ACTIVE")

            # Arm C: Candidate 4 Always-On (Active MCP)
            self.assertTrue(runs["arm_c_cand4_always_on"]["isolation"]["engram_enabled"])
            self.assertEqual(runs["arm_c_cand4_always_on"]["isolation"]["mcp_server_status"], "ACTIVE")

            # Arm D: Candidate 4 Adaptive (ADAPTIVE_BYPASS on Task A)
            self.assertTrue(runs["arm_d_cand4_adaptive"]["isolation"]["engram_enabled"])
            self.assertEqual(runs["arm_d_cand4_adaptive"]["isolation"]["mcp_server_status"], "ADAPTIVE_BYPASS")
            self.assertEqual(runs["arm_d_cand4_adaptive"]["adaptive_activation"]["routed_mode"], "BYPASS")


if __name__ == "__main__":
    unittest.main()
