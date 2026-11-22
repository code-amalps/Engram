import os
import sys
import unittest
import subprocess
import json
import hashlib
from pathlib import Path

HARNESS_ROOT = Path(__file__).resolve().parent.parent
GOLDEN_REPO = Path(r"C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter")
CAND_4_2_BIN = HARNESS_ROOT / "bin" / "candidate-phase4.2" / "engram.exe"
EXPECTED_SHA = "78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2"
MAX_BYTES_LIMIT = 16384


class TestCandidate42OfflineCertification(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # 1. Verify sealed binary presence and SHA-256
        if not CAND_4_2_BIN.exists():
            raise FileNotFoundError(f"Sealed Candidate 4.2 binary missing at {CAND_4_2_BIN}")
        actual_sha = hashlib.sha256(CAND_4_2_BIN.read_bytes()).hexdigest().upper()
        if actual_sha != EXPECTED_SHA:
            raise AssertionError(f"Candidate 4.2 SHA-256 mismatch: expected {EXPECTED_SHA}, got {actual_sha}")

    def call_mcp_raw(self, method: str, params: dict, proc=None, req_id: int = 1):
        """Helper to call MCP via stdio and return raw line bytes and parsed JSON."""
        close_proc = False
        if proc is None:
            close_proc = True
            proc = subprocess.Popen(
                [str(CAND_4_2_BIN), "-C", str(GOLDEN_REPO), "mcp"],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1
            )
        req = {
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method,
            "params": params
        }
        proc.stdin.write(json.dumps(req) + "\n")
        proc.stdin.flush()
        line = proc.stdout.readline()
        raw_bytes = len(line.encode("utf-8"))
        data = json.loads(line)
        if close_proc:
            proc.terminate()
            proc.wait(timeout=5)
        return raw_bytes, data

    def test_01_binary_hash_and_integrity(self):
        """Certifies that the Candidate 4.2 binary matches exact sealed SHA-256."""
        actual_sha = hashlib.sha256(CAND_4_2_BIN.read_bytes()).hexdigest().upper()
        self.assertEqual(actual_sha, EXPECTED_SHA)

    def test_02_tools_list_contract(self):
        """Certifies that all 4 tools are registered with max_bytes boundary support."""
        raw_b, data = self.call_mcp_raw("tools/list", {})
        self.assertIn("result", data)
        tools = {t["name"]: t for t in data["result"]["tools"]}
        expected_tools = {"engram_get_context", "engram_inspect_symbol", "engram_preflight", "engram_postflight"}
        self.assertTrue(expected_tools.issubset(set(tools.keys())))
        for tname in expected_tools:
            schema = tools[tname]["inputSchema"]["properties"]
            self.assertIn("max_bytes", schema, f"Tool {tname} must expose max_bytes")

    def test_03_get_context_large_omission_manifest_bounded(self):
        """Certifies that engram_get_context on the 100+ file repo never exceeds 16,384 bytes."""
        params = {
            "name": "engram_get_context",
            "arguments": {
                "task": "Add remember-me option to authentication flow so username persists across sessions",
                "token_budget": 4000
            }
        }
        raw_b, data = self.call_mcp_raw("tools/call", params)
        self.assertLessEqual(raw_b, MAX_BYTES_LIMIT, f"Response size {raw_b} exceeds {MAX_BYTES_LIMIT}")
        self.assertFalse(data.get("result", {}).get("isError", True))
        
        # Verify omission manifest is compacted
        content_text = data["result"]["content"][0]["text"]
        inner = json.loads(content_text)
        self.assertEqual(inner.get("status"), "READY")
        self.assertTrue(inner.get("omission_manifest", {}).get("manifest_truncated", False))
        self.assertGreater(inner.get("omission_manifest", {}).get("omitted_items_count", 0), 50)

    def test_04_inspect_symbol_with_multibyte_and_unicode_safety(self):
        """Certifies that engram_inspect_symbol on auth.effects.ts handles Unicode without panic and stays <= 16,384 B."""
        params = {
            "name": "engram_inspect_symbol",
            "arguments": {
                "target": "auth.effects.ts",
                "depth": 2
            }
        }
        raw_b, data = self.call_mcp_raw("tools/call", params)
        self.assertLessEqual(raw_b, MAX_BYTES_LIMIT, f"Response size {raw_b} exceeds {MAX_BYTES_LIMIT}")
        self.assertFalse(data.get("result", {}).get("isError", True))

    def test_05_preflight_bounded(self):
        """Certifies that engram_preflight stays <= 16,384 bytes."""
        params = {
            "name": "engram_preflight",
            "arguments": {
                "task": "Implement remember-me checkbox in login component",
                "planned_files": [
                    "projects/angular-ngrx-material-starter/src/app/core/auth/auth.effects.ts",
                    "projects/angular-ngrx-material-starter/src/app/core/auth/auth.actions.ts",
                    "projects/angular-ngrx-material-starter/src/app/app/app.component.ts"
                ]
            }
        }
        raw_b, data = self.call_mcp_raw("tools/call", params)
        self.assertLessEqual(raw_b, MAX_BYTES_LIMIT)
        self.assertFalse(data.get("result", {}).get("isError", True))

    def test_06_postflight_bounded(self):
        """Certifies that engram_postflight stays <= 16,384 bytes."""
        params = {
            "name": "engram_postflight",
            "arguments": {
                "strict": True,
                "test_evidence": "npm run test passed with 156 specs"
            }
        }
        raw_b, data = self.call_mcp_raw("tools/call", params)
        self.assertLessEqual(raw_b, MAX_BYTES_LIMIT)
        self.assertFalse(data.get("result", {}).get("isError", True))

    def test_07_boundary_exact_one_byte_below_and_above(self):
        """Certifies clamping and enforcement when max_bytes is requested at 16,383, 16,384, and 16,385."""
        for requested_limit in (16383, 16384, 16385):
            params = {
                "name": "engram_get_context",
                "arguments": {
                    "task": "Perform comprehensive architecture audit",
                    "max_bytes": requested_limit
                }
            }
            raw_b, data = self.call_mcp_raw("tools/call", params)
            self.assertLessEqual(raw_b, MAX_BYTES_LIMIT, f"When max_bytes={requested_limit}, response {raw_b} B must be <= 16384")

    def test_08_adversarial_unicode_inputs_and_excerpts(self):
        """Certifies adversarial Unicode queries (emojis, Chinese, Cyrillic, RTL Arabic, bullets) do not trigger slicing panics."""
        adversarial_targets = [
            "🔐_auth_login_🔥_🚀",
            "认证模块_auth_effects",
            "аутентификация_компонент",
            "مصادقة_auth_actions",
            "• bullet • point • test •"
        ]
        for adv in adversarial_targets:
            params = {
                "name": "engram_inspect_symbol",
                "arguments": {
                    "target": adv,
                    "depth": 1
                }
            }
            raw_b, data = self.call_mcp_raw("tools/call", params)
            self.assertLessEqual(raw_b, MAX_BYTES_LIMIT)
            self.assertIn("result", data, f"Adversarial input '{adv}' must not crash MCP server")

    def test_09_malformed_and_missing_parameters_graceful_fallback(self):
        """Certifies error fallback behavior on malformed or missing required parameters."""
        # Missing required 'task' in get_context
        params = {
            "name": "engram_get_context",
            "arguments": {}
        }
        raw_b, data = self.call_mcp_raw("tools/call", params)
        self.assertLessEqual(raw_b, MAX_BYTES_LIMIT)
        self.assertTrue(data.get("result", {}).get("isError", False) or "error" in data)

    def test_10_persistent_stdio_session_idempotency(self):
        """Certifies that multiple sequential requests across a persistent stdio session remain bounded and deterministic."""
        proc = subprocess.Popen(
            [str(CAND_4_2_BIN), "-C", str(GOLDEN_REPO), "mcp"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1
        )
        try:
            req_params = {
                "name": "engram_inspect_symbol",
                "arguments": {"target": "auth.actions.ts", "depth": 1}
            }
            sizes = []
            for i in range(1, 6):
                raw_b, data = self.call_mcp_raw("tools/call", req_params, proc=proc, req_id=i)
                self.assertLessEqual(raw_b, MAX_BYTES_LIMIT)
                sizes.append(raw_b)
            # Idempotency check: repeated calls must return exact same length
            self.assertEqual(len(set(sizes)), 1, f"Responses across persistent session varied in size: {sizes}")
        finally:
            proc.terminate()
            proc.wait(timeout=5)


if __name__ == "__main__":
    unittest.main()
