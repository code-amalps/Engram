import unittest, subprocess, os, json, tempfile, shutil
from pathlib import Path

class TestActualMcpIntegration(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Prefer the packaged candidate binary; fallback to clean worktree or development binary
        cand_paths = [
            Path(r"C:\Users\Amal P S\OneDrive\Projects\Engram\releases\engram-preview-v1.5.4-cand5-windows-x64\bin\engram.exe"),
            Path(r"C:\Users\Amal P S\AppData\Local\Temp\engram_clean_worktree\target\release\engram.exe"),
            Path(r"C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness\bin\candidate-phase5\engram.exe")
        ]
        cls.binary_path = None
        for p in cand_paths:
            if p.exists():
                cls.binary_path = str(p)
                break
        assert cls.binary_path is not None, "No Candidate 5 executable found for integration test"

    def setUp(self):
        self.workspace = tempfile.mkdtemp(prefix="engram_pytest_mcp_")
        src_dir = os.path.join(self.workspace, "src")
        tests_dir = os.path.join(self.workspace, "tests")
        os.makedirs(src_dir, exist_ok=True)
        os.makedirs(tests_dir, exist_ok=True)

        with open(os.path.join(src_dir, "calculator.py"), "w", encoding="utf-8") as f:
            f.write("class Calculator:\n    def add(self, a: int, b: int) -> int:\n        return a + b\n")
        open(os.path.join(src_dir, "__init__.py"), "w").close()

        with open(os.path.join(tests_dir, "test_calculator.py"), "w", encoding="utf-8") as f:
            f.write("from src.calculator import Calculator\ndef test_add():\n    assert Calculator().add(1, 2) == 3\n")
        open(os.path.join(tests_dir, "__init__.py"), "w").close()

        # Run analyze
        subprocess.run([self.binary_path, "-C", self.workspace, "analyze"], check=True, capture_output=True)

    def tearDown(self):
        shutil.rmtree(self.workspace, ignore_errors=True)

    def test_mcp_lifecycle_and_tools(self):
        proc = subprocess.Popen(
            [self.binary_path, "-C", self.workspace, "mcp", "--all-tools"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1
        )

        try:
            # 1. Initialize
            proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}}) + "\n")
            proc.stdin.flush()
            init_resp = json.loads(proc.stdout.readline())
            self.assertEqual(init_resp.get("id"), 1)
            self.assertEqual(init_resp.get("result", {}).get("serverInfo", {}).get("name"), "engram")

            # Notification
            proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
            proc.stdin.flush()

            # 2. List tools
            proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}) + "\n")
            proc.stdin.flush()
            tools_resp = json.loads(proc.stdout.readline())
            tools = [t["name"] for t in tools_resp.get("result", {}).get("tools", [])]
            self.assertEqual(len(tools), 16)
            self.assertIn("engram_get_context", tools)
            self.assertIn("engram_inspect_symbol", tools)

            # 3. Context retrieval
            proc.stdin.write(json.dumps({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {"name": "engram_get_context", "arguments": {"task": "test add", "token_budget": 500}}
            }) + "\n")
            proc.stdin.flush()
            ctx_resp = json.loads(proc.stdout.readline())
            self.assertEqual(ctx_resp.get("id"), 3)
            self.assertIn("content", ctx_resp.get("result", {}))

            # 4. Symbol inspection
            proc.stdin.write(json.dumps({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {"name": "engram_inspect_symbol", "arguments": {"target": "Calculator"}}
            }) + "\n")
            proc.stdin.flush()
            insp_resp = json.loads(proc.stdout.readline())
            self.assertEqual(insp_resp.get("id"), 4)

            # 5. Unknown tool
            proc.stdin.write(json.dumps({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "tools/call",
                "params": {"name": "nonexistent", "arguments": {}}
            }) + "\n")
            proc.stdin.flush()
            err_resp = json.loads(proc.stdout.readline())
            self.assertEqual(err_resp.get("id"), 5)
            self.assertTrue(err_resp.get("result", {}).get("isError") or "error" in err_resp)

            # 6. Graceful shutdown
            proc.stdin.close()
            code = proc.wait(timeout=5)
            self.assertEqual(code, 0)
        finally:
            if proc.poll() is None:
                proc.kill()

if __name__ == "__main__":
    unittest.main()
