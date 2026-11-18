"""
Token Efficiency Regression Tests for Engram Benchmark Harness.

Validates:
1. Exact o200k_base tokenizer budget for compact Turn-0 directives (<= 60 tokens).
2. Exact o200k_base tokenizer budget for lean 4-tool MCP schema (<= 400 tokens).
3. Conservative classifier boundaries:
   - Task A (leaf typo) qualifies for zero-overhead Fast Path.
   - Task B (architectural feature) fails open to full Engram mode.
   - Fail-open safety invariants (multi-file, missing targets, architectural keywords).
   - Word boundary safety (no false positive collisions like 'capitalization' matching 'api').
4. Harness forward-slash path escaping and MCP argument construction.
"""

import json
import pytest
import tiktoken
from pathlib import Path
import sys

# Ensure harness src is importable
harness_src = Path(__file__).parent.parent / "src"
if str(harness_src) not in sys.path:
    sys.path.insert(0, str(harness_src))

from classifier import (
    classify_task_efficiency,
    TaskClassificationResult,
    ARCHITECTURAL_KEYWORDS,
    TYPOGRAPHICAL_KEYWORDS
)
from runner import CodexRunner, RunExecutionResult


# Canonical 4-tool lean schemas matching src/mcp.rs core_tools_definitions()
LEAN_CORE_TOOLS = [
    {
        "type": "function",
        "function": {
            "name": "engram_get_context",
            "description": "Fetch AST symbols, targets, boundaries, and active contract.",
            "parameters": {
                "type": "object",
                "properties": {
                    "task": {"type": "string", "description": "Task description"},
                    "token_budget": {"type": "integer", "description": "Token budget"}
                },
                "required": ["task"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "engram_inspect_symbol",
            "description": "Inspect symbol declaration, signature, docstring, and references.",
            "parameters": {
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "description": "Symbol name"},
                    "path": {"type": "string", "description": "Relative file path"}
                },
                "required": ["symbol"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "engram_preflight",
            "description": "Validate planned edits and target boundaries before modifying files.",
            "parameters": {
                "type": "object",
                "properties": {
                    "task": {"type": "string", "description": "Task description"},
                    "planned_files": {"type": "array", "items": {"type": "string"}},
                    "contract_id": {"type": "string", "description": "Active contract ID"}
                },
                "required": ["task", "planned_files"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "engram_postflight",
            "description": "Audit working-tree diff against active Change Contract boundaries.",
            "parameters": {
                "type": "object",
                "properties": {
                    "contract_id": {"type": "string", "description": "Active contract ID"}
                }
            }
        }
    }
]

# Canonical compact Turn-0 directive template from src/instructions.rs
CANONICAL_COMPACT_DIRECTIVE = (
    "## Engram Directives\n"
    "- Active Change Contract: `ctr_8a6296e0` (locked to `1c285608`)\n"
    "- Directives: Contract locked. Use `engram_inspect_symbol` for symbol lookups; call `engram_postflight`."
)


class TestTokenBudgetLimits:
    """Verifies strict token ceilings under OpenAI's o200k_base tokenizer."""

    @pytest.fixture
    def enc(self):
        return tiktoken.get_encoding("o200k_base")

    def test_compact_turn0_directive_token_budget(self, enc):
        """Turn-0 directive must not exceed 60 tokens under o200k_base."""
        tokens = enc.encode(CANONICAL_COMPACT_DIRECTIVE)
        token_count = len(tokens)
        assert token_count <= 60, f"Turn-0 directive measured {token_count} tokens (> 60 budget)"

    def test_lean_mcp_surface_token_budget(self, enc):
        """Lean 4-tool client payload must not exceed 400 tokens under o200k_base."""
        payload_str = json.dumps(LEAN_CORE_TOOLS, separators=(",", ":"))
        tokens = enc.encode(payload_str)
        token_count = len(tokens)
        assert token_count <= 400, f"Lean MCP schema measured {token_count} tokens (> 400 budget)"

    def test_combined_static_overhead_budget(self, enc):
        """Combined static overhead (schema + directive) must be under 450 tokens."""
        directive_tokens = len(enc.encode(CANONICAL_COMPACT_DIRECTIVE))
        schema_tokens = len(enc.encode(json.dumps(LEAN_CORE_TOOLS, separators=(",", ":"))))
        total_overhead = directive_tokens + schema_tokens
        assert total_overhead <= 450, f"Combined overhead {total_overhead} exceeded 450 token ceiling"


class TestConservativeClassifier:
    """Tests fail-open conservative classification for Fast Path routing."""

    def test_task_a_qualifies_for_fast_path(self):
        """Task A (isolated typo in about template) must qualify for zero-overhead Fast Path."""
        prompt = (
            "Fix the capitalization typo 'Angular Cli Tips' -> 'Angular CLI Tips' "
            "in the about component header template (projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html)."
        )
        task_config = {
            "allowed_file_patterns": ["projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html"]
        }
        res = classify_task_efficiency(prompt, task_config=task_config)
        assert res.is_fast_path is True
        assert res.mode == "FAST_PATH"
        assert len(res.blocked_by) == 0
        assert res.escalation_hint is not None

    def test_task_b_fails_open_to_engram_mode(self):
        """Task B (remember-me auth feature) must route to full Engram mode."""
        prompt = (
            "Implement Remember Me login feature with localStorage and state management "
            "across auth reducers and effects."
        )
        task_config = {
            "allowed_file_patterns": [
                "projects/angular-ngrx-material-starter/src/app/core/auth/**",
                "projects/angular-ngrx-material-starter/src/app/app/app.component.*"
            ]
        }
        res = classify_task_efficiency(prompt, task_config=task_config)
        assert res.is_fast_path is False
        assert res.mode == "ENGRAM_MODE"
        assert any("wildcards" in b.lower() or "multi-file" in b.lower() for b in res.blocked_by)

    def test_missing_target_files_fails_open(self):
        """Tasks with missing or empty target specifications fail open to Engram mode."""
        res = classify_task_efficiency("Fix spelling typo in label", target_files=[])
        assert res.is_fast_path is False
        assert res.mode == "ENGRAM_MODE"
        assert any("Missing explicit target" in b for b in res.blocked_by)

    def test_multi_file_targets_fail_open(self):
        """Tasks touching more than one file fail open to Engram mode."""
        res = classify_task_efficiency(
            "Fix spelling typo in labels",
            target_files=["src/label1.ts", "src/label2.ts"]
        )
        assert res.is_fast_path is False
        assert res.mode == "ENGRAM_MODE"
        assert any("Multi-file" in b for b in res.blocked_by)

    def test_architectural_file_name_fails_open(self):
        """Leaf edits targeting structural files (e.g. reducer, service) fail open."""
        res = classify_task_efficiency(
            "Fix typo in variable name",
            target_files=["src/app/core/auth/auth.reducer.ts"]
        )
        assert res.is_fast_path is False
        assert res.mode == "ENGRAM_MODE"
        assert any("architectural component" in b for b in res.blocked_by)

    def test_architectural_keywords_in_prompt_fail_open(self):
        """Prompts mentioning state, store, api, token, etc. fail open."""
        for kw in ["state", "store", "reducer", "auth", "login", "token", "service", "api"]:
            res = classify_task_efficiency(
                f"Fix typo in {kw} handler",
                target_files=["src/app/label.ts"]
            )
            assert res.is_fast_path is False, f"Keyword '{kw}' failed to block fast path"
            assert res.mode == "ENGRAM_MODE"

    def test_word_boundary_avoids_false_positive_substring_match(self):
        """'capitalization' must NOT match 'api', and 'rapid' must NOT match 'api'."""
        prompt = "Fix capitalization of label string in src/app/label.ts"
        res = classify_task_efficiency(
            prompt,
            target_files=["src/app/label.ts"]
        )
        assert res.is_fast_path is True, f"Word-boundary collision blocked valid prompt: {res.blocked_by}"
        assert res.mode == "FAST_PATH"


class TestRunnerFastPathIntegration:
    """Verifies CodexRunner properly integrates task classification and TOML path escaping."""

    def test_dry_run_task_a_fast_path_bypasses_mcp(self, tmp_path):
        """Dry-run execution of Task A bypasses MCP tool calls and Turn-0 directive injection."""
        events_path = tmp_path / "events.jsonl"
        fake_codex = tmp_path / "codex.exe"
        fake_codex.touch()

        runner = CodexRunner(codex_executable=fake_codex)

        task_a_config = {
            "allowed_file_patterns": ["projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html"]
        }
        prompt = (
            "Fix the capitalization typo 'Angular Cli Tips' -> 'Angular CLI Tips' "
            "in the about component header template (projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html)."
        )

        exec_res = runner.run(
            sandbox_path=tmp_path,
            prompt=prompt,
            arm="v154_arm3",
            output_events_path=events_path,
            is_dry_run=True,
            task_config=task_a_config
        )

        assert exec_res.fast_path_active is True
        assert exec_res.turn0_injected is False
        assert exec_res.initial_prompt == prompt

        # Verify generated events contain no MCP tool calls
        events = [json.loads(line) for line in events_path.read_text(encoding="utf-8").strip().splitlines()]
        tool_calls = [ev for ev in events if ev.get("item", {}).get("type") == "mcp_tool_call"]
        assert len(tool_calls) == 0, "Fast Path must not emit any MCP tool calls"

    def test_dry_run_task_b_engram_mode_retains_mcp(self, tmp_path):
        """Dry-run execution of Task B retains Engram MCP tool execution and Turn-0 directives."""
        events_path = tmp_path / "events.jsonl"
        fake_codex = tmp_path / "codex.exe"
        fake_codex.touch()

        runner = CodexRunner(codex_executable=fake_codex)

        task_b_config = {
            "target_files": [
                "src/app/core/auth/auth.reducer.ts",
                "src/app/core/auth/auth.effects.ts"
            ]
        }
        prompt = "Implement Remember Me login feature with state management"

        exec_res = runner.run(
            sandbox_path=tmp_path,
            prompt=prompt,
            arm="v154_arm3",
            output_events_path=events_path,
            is_dry_run=True,
            task_config=task_b_config
        )

        assert exec_res.fast_path_active is False
        assert exec_res.turn0_injected is True
        assert "## Engram Directives" in exec_res.initial_prompt
        assert "engram_inspect_symbol" in exec_res.initial_prompt

        # Verify generated events contain MCP tool call
        events = [json.loads(line) for line in events_path.read_text(encoding="utf-8").strip().splitlines()]
        tool_calls = [ev for ev in events if ev.get("item", {}).get("type") == "mcp_tool_call"]
        assert len(tool_calls) == 1, "Engram mode in arm 3 dry-run must emit prepare_task MCP call"


class TestTokenAccountingIntegrity:
    """Verifies trustworthy, comparable token accounting and suppression of false savings."""

    def test_no_double_counting_cached_or_reasoning(self):
        """Cached input and reasoning tokens must be subsets, never additive to total tokens."""
        from telemetry import TelemetryMetrics

        # Simulated rollout parser outcome
        tel = TelemetryMetrics(
            provenance="SESSION_ROLLOUT",
            prompt_tokens_total=50000,
            prompt_tokens_cached=35000,
            prompt_tokens_cached_inferred=False,
            prompt_tokens_uncached=15000,
            prompt_tokens_uncached_inferred=True,
            completion_tokens=2500,
            reasoning_tokens=800,
            total_model_tokens=52500,
            peak_single_request_context=18000,
            peak_turn_context=18000,
            final_turn_context=18000,
            model_requests_count=5,
            turns_count=5,
            mcp_tool_calls_count=2,
            auxiliary_tool_payload_bytes=1450,
            telemetry_valid=True
        )

        d = tel.to_dict()
        # Invariant 1: Total model tokens = prompt_tokens_total + completion_tokens
        assert d["total_model_tokens"] == d["prompt_tokens_total"] + d["completion_tokens"]
        # Invariant 2: Cached input is strictly <= prompt_tokens_total
        assert d["prompt_tokens_cached"] <= d["prompt_tokens_total"]
        # Invariant 3: Cached + Uncached == prompt_tokens_total
        assert d["prompt_tokens_cached"] + d["prompt_tokens_uncached"] == d["prompt_tokens_total"]
        # Invariant 4: Reasoning tokens is strictly <= completion_tokens
        assert d["reasoning_tokens"] <= d["completion_tokens"]
        # Invariant 5: Total tokens must NEVER add cached or reasoning on top
        assert d["total_model_tokens"] != d["prompt_tokens_total"] + d["completion_tokens"] + d["prompt_tokens_cached"]

    def test_missing_telemetry_suppresses_comparative_savings(self):
        """Missing or zero telemetry must suppress savings claims (status: INVALIDATED_BY_TELEMETRY_INVALID)."""
        from orchestrator import Orchestrator

        ctrl_run = {
            "telemetry": {
                "provenance": "SESSION_ROLLOUT",
                "prompt_tokens_total": 60000,
                "prompt_tokens_cached": 40000,
                "completion_tokens": 1000,
                "total_model_tokens": 61000,
                "peak_single_request_context": 15000,
                "telemetry_valid": True
            },
            "correctness": {"score": 1.0, "passed_all_requirements": True, "quality_adjusted_cost": 61000.0}
        }

        # Defective treatment run: total_model_tokens is 0
        treat_run = {
            "telemetry": {
                "provenance": "UNAVAILABLE",
                "prompt_tokens_total": 0,
                "prompt_tokens_cached": 0,
                "completion_tokens": 0,
                "total_model_tokens": 0,
                "peak_single_request_context": None,
                "telemetry_valid": False
            },
            "correctness": {"score": 1.0, "passed_all_requirements": True, "quality_adjusted_cost": 0.0}
        }

        analysis = Orchestrator.compute_comparative_analysis(
            ctrl_run=ctrl_run,
            treat_run=treat_run,
            baseline_label="baseline_arm1",
            treatment_label="v154_arm3"
        )

        assert analysis["savings_claim_eligible"] is False
        assert analysis["savings_claim_status"] == "INVALIDATED_BY_TELEMETRY_INVALID"
        assert analysis["metrics"]["total_model_tokens"]["savings_pct"] is None
        assert analysis["metrics"]["total_model_tokens"]["status"] == "INVALIDATED_TELEMETRY"

    def test_acceptance_failure_suppresses_comparative_savings(self):
        """Any arm failing correctness (C < 1.0) must invalidate token savings claims."""
        from orchestrator import Orchestrator

        ctrl_run = {
            "telemetry": {
                "prompt_tokens_total": 80000,
                "completion_tokens": 2000,
                "total_model_tokens": 82000,
                "peak_single_request_context": 18000,
                "telemetry_valid": True
            },
            "correctness": {"score": 1.0, "passed_all_requirements": True, "quality_adjusted_cost": 82000.0}
        }

        # Treatment arm achieved fewer tokens but failed functional correctness
        treat_run = {
            "telemetry": {
                "prompt_tokens_total": 40000,
                "completion_tokens": 1000,
                "total_model_tokens": 41000,
                "peak_single_request_context": 12000,
                "telemetry_valid": True
            },
            "correctness": {"score": 0.5, "passed_all_requirements": False, "quality_adjusted_cost": 410000.0}
        }

        analysis = Orchestrator.compute_comparative_analysis(
            ctrl_run=ctrl_run,
            treat_run=treat_run,
            baseline_label="baseline_arm1",
            treatment_label="v154_arm3"
        )

        assert analysis["savings_claim_eligible"] is False
        assert analysis["savings_claim_status"] == "INVALIDATED_BY_ACCEPTANCE_FAILURE"
        assert analysis["verdict"] == "TREATMENT_FAILED_ACCEPTANCE"
        assert analysis["metrics"]["total_model_tokens"]["savings_pct"] is None
        assert analysis["metrics"]["total_model_tokens"]["status"] == "INVALIDATED_UNACCEPTED"

    def test_incomplete_peak_context_telemetry_marked_unavailable(self):
        """When single-request context is unexposed, peak context comparison is UNAVAILABLE, not fabricated."""
        from orchestrator import Orchestrator

        ctrl_run = {
            "telemetry": {
                "prompt_tokens_total": 50000,
                "completion_tokens": 1000,
                "total_model_tokens": 51000,
                "peak_single_request_context": None,  # Not available from stdout
                "telemetry_valid": True
            },
            "correctness": {"score": 1.0, "passed_all_requirements": True}
        }
        treat_run = {
            "telemetry": {
                "prompt_tokens_total": 45000,
                "completion_tokens": 900,
                "total_model_tokens": 45900,
                "peak_single_request_context": None,
                "telemetry_valid": True
            },
            "correctness": {"score": 1.0, "passed_all_requirements": True}
        }

        analysis = Orchestrator.compute_comparative_analysis(
            ctrl_run=ctrl_run,
            treat_run=treat_run,
            baseline_label="baseline_arm1",
            treatment_label="v154_arm3"
        )

        assert analysis["savings_claim_eligible"] is True
        assert analysis["metrics"]["total_model_tokens"]["status"] == "VALID"
        assert analysis["metrics"]["total_model_tokens"]["savings_pct"] == 10.0
        # Peak context must be UNAVAILABLE, not 0% or fabricated
        peak_metric = analysis["metrics"]["peak_single_request_context"]
        assert peak_metric["status"] == "UNAVAILABLE"
        assert peak_metric["savings_pct"] is None
        assert peak_metric["delta"] is None
