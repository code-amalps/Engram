"""
Phase 2 Stage 2: MCP Runtime Activation & Retrieval Validation Regression Test Suite.

Covers:
1. MCP server startup and clean shutdown via stdio JSON-RPC.
2. Capability negotiation and tool discovery (reporting the 4 consolidated Engram tools).
3. Read-only tool invocation with valid parameters (engram_inspect_symbol returning AST analysis).
4. Tool invocation error handling with invalid parameters (empty target symbol).
5. Robust Windows path handling and TOML escaping (paths with spaces, backslashes, Unicode avoidance).
6. Least-privilege approval policy configuration (verifying fine-grained approvals and absence of global bypass).
"""

import unittest
import tempfile
import shutil
import json
import tomllib
import subprocess
import os
import sys
from pathlib import Path

# Harness imports
ROOT_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT_DIR / "src"))

from isolation import configure_sandbox_codex_mcp
from runner import CodexRunner


class TestMCPRuntimeActivation(unittest.TestCase):
    def setUp(self):
        self.temp_dir = Path(tempfile.mkdtemp(prefix="engram_test_mcp_activation_"))
        self.candidate_bin = ROOT_DIR / "bin" / "candidate-phase1" / "engram.exe"
        self.codex_bin = Path(r"C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe")

    def tearDown(self):
        if self.temp_dir.exists():
            shutil.rmtree(self.temp_dir, ignore_errors=True)

    # =========================================================================
    # 1. Windows Path Handling and TOML Escaping
    # =========================================================================

    def test_configure_sandbox_codex_mcp_windows_paths(self):
        """Verifies configure_sandbox_codex_mcp produces valid, parseable TOML for paths with spaces and backslashes."""
        test_cases = [
            (r"C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness\bin\candidate-phase1\engram.exe",
             self.temp_dir / "sandbox with spaces"),
            (r"C:\Program Files (x86)\Engram\bin\engram.exe",
             self.temp_dir / "sandbox_normal"),
            (r"D:\Data\Build\engram.exe",
             self.temp_dir / "sandbox_alpha")
        ]

        dummy_bin = self.temp_dir / "dummy_engram.exe"
        dummy_bin.write_text("dummy", encoding="utf-8")

        for bin_path_str, sb_path in test_cases:
            sb_path.mkdir(parents=True, exist_ok=True)
            cfg_path = configure_sandbox_codex_mcp(sb_path, dummy_bin)
            self.assertTrue(cfg_path.exists())

            # Read raw TOML text
            toml_text = cfg_path.read_text(encoding="utf-8")

            # Must parse cleanly with standard library tomllib without UnicodeDecodeError / escape error
            parsed = tomllib.loads(toml_text)
            self.assertIn("mcp_servers", parsed)
            self.assertIn("engram", parsed["mcp_servers"])
            engram_cfg = parsed["mcp_servers"]["engram"]

            self.assertEqual(engram_cfg["default_tools_approval_mode"], "approve")
            self.assertEqual(
                engram_cfg["enabled_tools"],
                ["engram_get_context", "engram_inspect_symbol", "engram_preflight", "engram_postflight"]
            )
            # Verify per-tool approval tables
            self.assertEqual(engram_cfg["tools"]["engram_get_context"]["approval_mode"], "approve")
            self.assertEqual(engram_cfg["tools"]["engram_inspect_symbol"]["approval_mode"], "approve")
            self.assertEqual(engram_cfg["tools"]["engram_preflight"]["approval_mode"], "approve")
            self.assertEqual(engram_cfg["tools"]["engram_postflight"]["approval_mode"], "approve")

    # =========================================================================
    # 2. Least-Privilege Approval Configuration & Global Bypass Absence
    # =========================================================================

    def test_least_privilege_approval_configuration(self):
        """Verifies runner build_codex_command uses least-privilege approvals and prohibits global bypass."""
        runner = CodexRunner(
            codex_executable=self.codex_bin,
            engram_executable=self.candidate_bin,
            model="gpt-5.6-luna",
            reasoning_effort="medium"
        )
        sandbox = self.temp_dir / "sandbox_approvals"
        sandbox.mkdir(parents=True, exist_ok=True)

        cmd = runner.build_codex_command(
            sandbox_path=sandbox,
            prompt="Inspect repository",
            arm="treatment_B"
        )
        cmd_str = " ".join(cmd)

        # Invariant: NEVER disable security approvals globally
        self.assertNotIn("--dangerously-bypass-approvals-and-sandbox", cmd_str)
        self.assertNotIn("--dangerously-bypass-hook-trust", cmd_str)

        # Invariant: Sandbox isolation is explicitly maintained
        self.assertIn("-s workspace-write", cmd_str)

        # Invariant: Fine-grained auto-approval for Engram tools only
        self.assertIn('mcp_servers.engram.default_tools_approval_mode="approve"', cmd_str)
        self.assertIn('mcp_servers.engram.tools.engram_inspect_symbol.approval_mode="approve"', cmd_str)

    # =========================================================================
    # 3. MCP Server Startup, Capability Negotiation, and Clean Shutdown
    # =========================================================================

    def test_mcp_server_lifecycle(self):
        """Verifies that candidate binary starts, completes JSON-RPC handshake, and shuts down cleanly."""
        if not self.candidate_bin.exists():
            self.skipTest(f"Candidate binary not found at {self.candidate_bin}")

        sb = self.temp_dir / "sb_lifecycle"
        sb.mkdir(parents=True, exist_ok=True)

        cmd = [str(self.candidate_bin), "-C", str(sb), "mcp"]
        proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace"
        )

        init_req = json.dumps({
            "jsonrpc": "2.0",
            "id": 100,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "test_harness", "version": "1.0"}
            }
        }) + "\n"

        stdout, stderr = proc.communicate(input=init_req, timeout=10)
        self.assertEqual(proc.returncode, 0, f"Candidate binary failed with stderr: {stderr}")

        lines = [l.strip() for l in stdout.splitlines() if l.strip()]
        self.assertGreaterEqual(len(lines), 1)

        resp = json.loads(lines[0])
        self.assertEqual(resp.get("id"), 100)
        result = resp.get("result", {})
        self.assertEqual(result.get("protocolVersion"), "2024-11-05")
        self.assertEqual(result.get("serverInfo", {}).get("name"), "engram")
        self.assertEqual(result.get("serverInfo", {}).get("version"), "1.5.4")

    # =========================================================================
    # 4. Tool Discovery / Listing
    # =========================================================================

    def test_mcp_tool_discovery(self):
        """Verifies that candidate binary exposes exactly the 4 expected consolidated tools."""
        if not self.candidate_bin.exists():
            self.skipTest(f"Candidate binary not found at {self.candidate_bin}")

        sb = self.temp_dir / "sb_discovery"
        sb.mkdir(parents=True, exist_ok=True)

        cmd = [str(self.candidate_bin), "-C", str(sb), "mcp"]
        proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace"
        )

        init_req = json.dumps({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "test_harness", "version": "1.0"}
            }
        }) + "\n"

        list_req = json.dumps({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }) + "\n"

        stdout, stderr = proc.communicate(input=init_req + list_req, timeout=10)
        self.assertEqual(proc.returncode, 0)

        lines = [l.strip() for l in stdout.splitlines() if l.strip()]
        tool_names = []
        for l in lines:
            try:
                data = json.loads(l)
                if data.get("id") == 2:
                    tools = data.get("result", {}).get("tools", [])
                    tool_names = [t.get("name") for t in tools]
            except Exception:
                pass

        expected = ["engram_get_context", "engram_inspect_symbol", "engram_preflight", "engram_postflight"]
        self.assertEqual(sorted(tool_names), sorted(expected))

    # =========================================================================
    # 5. Tool Invocation with Valid Parameters
    # =========================================================================

    def test_mcp_tool_invocation_valid(self):
        """Verifies engram_inspect_symbol returns structured AST analysis for a known symbol."""
        if not self.candidate_bin.exists():
            self.skipTest(f"Candidate binary not found at {self.candidate_bin}")

        sb = self.temp_dir / "sb_invoke_valid"
        src_dir = sb / "src"
        src_dir.mkdir(parents=True, exist_ok=True)

        # Setup minimal Rust repository
        (sb / "Cargo.toml").write_text("""[package]
name = "test_crate"
version = "0.1.0"
edition = "2021"
""", encoding="utf-8")

        (src_dir / "lib.rs").write_text("""pub struct Orchestrator {
    pub name: String,
}

impl Orchestrator {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}
""", encoding="utf-8")

        subprocess.run(["git", "init"], cwd=sb, capture_output=True, check=True)
        subprocess.run(["git", "add", "."], cwd=sb, capture_output=True, check=True)
        subprocess.run(["git", "config", "user.name", "Test"], cwd=sb, capture_output=True, check=True)
        subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=sb, capture_output=True, check=True)
        subprocess.run(["git", "commit", "-m", "init"], cwd=sb, capture_output=True, check=True)

        cmd = [str(self.candidate_bin), "-C", str(sb), "mcp"]
        proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace"
        )

        init_req = json.dumps({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "test", "version": "1.0"}}
        }) + "\n"

        call_req = json.dumps({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "engram_inspect_symbol",
                "arguments": {"target": "Orchestrator", "depth": 1}
            }
        }) + "\n"

        stdout, stderr = proc.communicate(input=init_req + call_req, timeout=15)
        self.assertEqual(proc.returncode, 0)

        found_result = False
        for l in stdout.splitlines():
            if not l.strip():
                continue
            try:
                data = json.loads(l)
                if data.get("id") == 3:
                    res = data.get("result", {})
                    content = res.get("content", [])
                    self.assertGreaterEqual(len(content), 1)
                    raw_text = content[0].get("text", "")
                    parsed_inner = json.loads(raw_text)
                    self.assertEqual(parsed_inner.get("target"), "Orchestrator")
                    self.assertEqual(parsed_inner.get("target_file"), "src/lib.rs")
                    self.assertIn("impact_cone", parsed_inner)
                    self.assertIn("explanation", parsed_inner)
                    found_result = True
            except Exception:
                pass
        self.assertTrue(found_result, "Did not receive valid tool call response for ID 3")

    # =========================================================================
    # 6. Tool Invocation with Invalid Parameters
    # =========================================================================

    def test_mcp_tool_invocation_invalid_parameters(self):
        """Verifies engram_inspect_symbol rejects empty target symbol with an error."""
        if not self.candidate_bin.exists():
            self.skipTest(f"Candidate binary not found at {self.candidate_bin}")

        sb = self.temp_dir / "sb_invoke_invalid"
        sb.mkdir(parents=True, exist_ok=True)

        cmd = [str(self.candidate_bin), "-C", str(sb), "mcp"]
        proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace"
        )

        init_req = json.dumps({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "test", "version": "1.0"}}
        }) + "\n"

        # Empty target parameter
        call_req = json.dumps({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "engram_inspect_symbol",
                "arguments": {"target": ""}
            }
        }) + "\n"

        stdout, stderr = proc.communicate(input=init_req + call_req, timeout=10)
        self.assertEqual(proc.returncode, 0)

        found_error = False
        for l in stdout.splitlines():
            if not l.strip():
                continue
            try:
                data = json.loads(l)
                if data.get("id") == 4:
                    # In JSON-RPC, error or result with isError=true
                    err = data.get("error")
                    res = data.get("result", {})
                    if err:
                        found_error = True
                    elif res.get("isError"):
                        found_error = True
            except Exception:
                pass
        self.assertTrue(found_error, "Expected error response for empty target parameter")


if __name__ == "__main__":
    unittest.main()
