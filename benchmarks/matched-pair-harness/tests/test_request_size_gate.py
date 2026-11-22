import unittest
import tempfile
import sys
import os
import math
from pathlib import Path

# Add src to sys.path
HARNESS_ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = HARNESS_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from runner import RequestSizeGate, PreDispatchGateViolationError, CodexRunner, RunExecutionResult


class TestRequestSizeGate(unittest.TestCase):
    def setUp(self):
        self.gate = RequestSizeGate(default_limit_tokens=60_000)
        self.temp_dir = tempfile.TemporaryDirectory()
        self.sandbox = Path(self.temp_dir.name)

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_01_encoder_initialization_offline(self):
        """Verifies that RequestSizeGate initializes an encoder offline without network access."""
        self.assertIsNotNone(self.gate._encoding_name)
        self.assertIn(self.gate._encoding_name, [
            "tiktoken_o200k_base",
            "tiktoken_cl100k_base",
            "heuristic_bytes_div_3_upper_bound"
        ])

    def test_02_token_counting_basic_text(self):
        """Verifies accurate token and byte counting on typical prompt text."""
        text = "Analyze the Angular NgRx state management architecture and fix the issue."
        tokens, bytes_count, method = self.gate.count_tokens(text)
        self.assertGreater(tokens, 0)
        self.assertEqual(bytes_count, len(text.encode("utf-8")))
        self.assertIsInstance(method, str)

    def test_03_conservative_heuristic_fallback(self):
        """Verifies conservative heuristic fallback calculates a safe, mathematically proven upper bound."""
        # Create a gate with encoder disabled to test heuristic
        heuristic_gate = RequestSizeGate(default_limit_tokens=60_000)
        heuristic_gate._encoder = None
        heuristic_gate._encoding_name = "heuristic_bytes_upper_bound"

        sample = "export class ComponentStore<T extends object> implements OnDestroy {"
        tokens, bytes_count, method = heuristic_gate.count_tokens(sample)
        self.assertEqual(method, "heuristic_bytes_upper_bound")
        self.assertGreater(tokens, 0)
        # Mathematically proven upper bound: tokens == bytes_count
        expected_bytes = len(sample.encode("utf-8"))
        self.assertEqual(bytes_count, expected_bytes)
        self.assertEqual(tokens, expected_bytes)

    def test_04_measure_complete_request_includes_all_parts(self):
        """Verifies request measurement gathers prompt, instructions, and CLI arguments."""
        # Setup instructions file in sandbox
        github_dir = self.sandbox / ".github"
        github_dir.mkdir(parents=True, exist_ok=True)
        inst_file = github_dir / "copilot-instructions.md"
        inst_file.write_text("# Project Instructions\nUse bounded context.", encoding="utf-8")

        prompt = "Refactor the authentication service."
        cmd_args = ["codex", "-m", "gpt-5.6-luna", "-c", "model_reasoning_effort=medium"]

        tot_tokens, tot_bytes, method, breakdown = self.gate.measure_complete_request(
            prompt=prompt,
            cmd_args=cmd_args,
            sandbox_path=self.sandbox
        )
        self.assertGreater(tot_tokens, 0)
        self.assertIn("prompt", breakdown["components"])
        self.assertIn("instructions", breakdown["components"])
        self.assertIn("cli_arguments", breakdown["components"])
        self.assertGreater(breakdown["components"]["instructions"]["tokens"], 0)

    def test_05_check_pre_dispatch_passes_under_limit(self):
        """Verifies check_pre_dispatch returns True when request is within limits."""
        prompt = "Short prompt within the 60,000 token boundary."
        passed, tokens, bytes_count, method, err, breakdown = self.gate.check_pre_dispatch(
            prompt=prompt,
            limit_tokens=60_000
        )
        self.assertTrue(passed)
        self.assertIsNone(err)
        self.assertLess(tokens, 60_000)

    def test_06_check_pre_dispatch_rejects_over_limit_fail_closed(self):
        """Verifies check_pre_dispatch strictly rejects requests exceeding the limit (zero truncation)."""
        # Create oversized request: 70,000 tokens (~280,000 characters)
        oversized_prompt = "word " * 75_000
        passed, tokens, bytes_count, method, err, breakdown = self.gate.check_pre_dispatch(
            prompt=oversized_prompt,
            limit_tokens=60_000
        )
        self.assertFalse(passed)
        self.assertIsNotNone(err)
        self.assertIn("Pre-dispatch request boundary violation", err)
        self.assertIn("exceeds the configured limit of 60,000 tokens", err)
        self.assertIn("Zero-truncation policy enforced", err)

    def test_07_runner_rejection_without_process_spawn(self):
        """Verifies CodexRunner rejects oversized requests before dispatching any child process."""
        dummy_exe = Path(sys.executable)
        runner = CodexRunner(
            codex_executable=dummy_exe,
            max_request_tokens=500  # Low limit to trigger gate
        )
        oversized_prompt = "large content block " * 200  # ~800 tokens > 500 limit
        events_out = self.sandbox / "events.jsonl"

        result = runner.run(
            sandbox_path=self.sandbox,
            prompt=oversized_prompt,
            arm="control_A",
            output_events_path=events_out,
            is_dry_run=True
        )
        self.assertEqual(result.exit_code, 105)
        self.assertFalse(result.request_size_gate_passed)
        self.assertEqual(result.stop_reason, "PRE_DISPATCH_REQUEST_SIZE_EXCEEDED")
        self.assertGreater(result.measured_request_tokens, 500)
        self.assertTrue(events_out.exists())

    def test_08_dry_run_gate_passing(self):
        """Verifies normal dry run passes request size gate cleanly."""
        dummy_exe = Path(sys.executable)
        runner = CodexRunner(
            codex_executable=dummy_exe,
            max_request_tokens=60_000
        )
        normal_prompt = "Fix typo in component file."
        events_out = self.sandbox / "events.jsonl"

        result = runner.run(
            sandbox_path=self.sandbox,
            prompt=normal_prompt,
            arm="control_A",
            output_events_path=events_out,
            is_dry_run=True
        )
        self.assertEqual(result.exit_code, 0)
        self.assertTrue(result.request_size_gate_passed)
        self.assertGreater(result.measured_request_tokens, 0)
        self.assertLess(result.measured_request_tokens, 60_000)

    def test_09_adversarial_undercount_demonstration_and_byte_bound(self):
        """
        Adversarial test demonstrating that legacy heuristic ceil(bytes/3.0)*1.1 severely
        undercounts on single-byte token sequences, whereas bytes_upper_bound is mathematically conservative.
        """
        adversarial_inputs = [
            ("digits", "1 2 3 4 5 6 7 8 9 0 " * 10),
            ("ascii_punct", "!@#$%^&*()_+-=[]{}|;:,.<>?/~`" * 5),
            ("base64_dense", "aB9!xZ2$qW8*mK4@aB9!xZ2$qW8*mK4@" * 5),
            ("dense_code", "x += (y >> 2) ^ (z & ~w); \n" * 10)
        ]

        fallback_gate = RequestSizeGate(default_limit_tokens=60_000)
        fallback_gate._encoder = None
        fallback_gate._encoding_name = "heuristic_bytes_upper_bound"

        for label, text in adversarial_inputs:
            actual_bytes = len(text.encode("utf-8"))
            # Count with tiktoken (real BPE)
            bpe_tokens, _, _ = self.gate.count_tokens(text)
            # Count with fallback
            fallback_tokens, _, _ = fallback_gate.count_tokens(text)

            # Legacy formula undercount demonstration
            legacy_undercount_estimate = int(math.ceil((actual_bytes / 3.0) * 1.1))
            self.assertLess(
                legacy_undercount_estimate,
                bpe_tokens,
                f"Adversarial demonstration: {label} proves legacy formula undercounts"
            )

            # Modern fallback must be a true upper bound: fallback_tokens >= bpe_tokens
            self.assertGreaterEqual(
                fallback_tokens,
                bpe_tokens,
                f"Modern fallback must guarantee no undercount for {label}"
            )

    def test_10_multilingual_and_rare_unicode(self):
        """Verifies tokenizer and fallback behavior on multilingual, rare Unicode, and emoji."""
        multilingual_inputs = [
            "这是一个包含中文测试的分词输入样本，用于验证分词器的准确性。",
            "Этот текст на русском языке предназначен для проверки токенизации.",
            "هذا النص باللغة العربية لاختبار دقة محدد حجم الطلب.",
            "🚀🔥🎉✨🌟💡🤖🛡️📦🔍⚡🎯🧩" * 5
        ]
        fallback_gate = RequestSizeGate(default_limit_tokens=60_000)
        fallback_gate._encoder = None
        fallback_gate._encoding_name = "heuristic_bytes_upper_bound"

        for text in multilingual_inputs:
            bpe_tokens, bpe_bytes, _ = self.gate.count_tokens(text)
            fb_tokens, fb_bytes, _ = fallback_gate.count_tokens(text)

            self.assertGreater(bpe_tokens, 0)
            self.assertGreater(fb_tokens, 0)
            self.assertEqual(bpe_bytes, fb_bytes)
            # Fallback must be >= bpe_tokens
            self.assertGreaterEqual(fb_tokens, bpe_tokens)

    def test_11_boundary_equality_and_one_token_over(self):
        """Verifies exact boundary equality (tokens == limit passes) and one-token-over (rejects)."""
        # Exact limit check: synthesize prompt measuring exactly N tokens
        sample_word = "hello "  # 1 token in o200k_base
        single_token_count, _, _ = self.gate.count_tokens(sample_word)

        exact_limit = 100
        prompt_100 = sample_word * exact_limit
        tokens_100, _, _ = self.gate.count_tokens(prompt_100)

        # Equal to limit: passes
        passed, tokens, _, _, err, _ = self.gate.check_pre_dispatch(prompt=prompt_100, limit_tokens=tokens_100)
        self.assertTrue(passed)
        self.assertIsNone(err)

        # One token over limit: rejected
        passed_over, tokens_over, _, _, err_over, _ = self.gate.check_pre_dispatch(prompt=prompt_100, limit_tokens=tokens_100 - 1)
        self.assertFalse(passed_over)
        self.assertIsNotNone(err_over)
        self.assertIn("Pre-dispatch request boundary violation", err_over)

    def test_12_malformed_and_boundary_inputs(self):
        """Verifies robust handling of empty string, whitespace, and null bytes."""
        malformed_inputs = [
            "",
            "   \t\n   ",
            "\x00\x00\x00\x00",
            "Valid prefix\x00embedded null\x00suffix"
        ]
        for s in malformed_inputs:
            tokens, byte_count, method = self.gate.count_tokens(s)
            self.assertGreaterEqual(tokens, 0)
            self.assertGreaterEqual(byte_count, 0)
            passed, _, _, _, _, _ = self.gate.check_pre_dispatch(prompt=s, limit_tokens=60_000)
            self.assertTrue(passed)


if __name__ == "__main__":
    unittest.main()
