"""
Headless Codex CLI runner for Matched-Pair Benchmark runs.
Remediated capabilities:
1. Explicitly pins supported model and reasoning effort via CLI flags (-m, -c).
2. Sanitizes child process PATH by stripping any legacy Engram directories.
3. Tracks specific child PID and terminates only that process tree on timeout/exception.
4. Captures thread_id and full command metadata.
"""
from dataclasses import dataclass
from typing import Dict, Any, List, Optional
from pathlib import Path
import subprocess
import datetime
import time
import json
import os

from isolation import terminate_pid_tree


@dataclass
class RunExecutionResult:
    exit_code: int
    elapsed_seconds: float
    started_at: str
    completed_at: str
    events_log_path: str
    thread_id: Optional[str]
    model_name: str
    reasoning_effort: str
    harness_executable: str
    harness_version: str
    path_sanitized: bool
    error_message: Optional[str] = None


class CodexRunner:
    def __init__(
        self,
        codex_executable: Path,
        engram_executable: Optional[Path] = None,
        model: str = "gpt-5.6-luna",
        reasoning_effort: str = "medium",
        timeout_seconds: int = 600
    ):
        self.codex_executable = Path(codex_executable)
        self.engram_executable = Path(engram_executable) if engram_executable else None
        self.model = model
        self.reasoning_effort = reasoning_effort
        self.timeout_seconds = timeout_seconds

        # Query version
        self.harness_version = "unknown"
        try:
            ver_proc = subprocess.run(
                [str(self.codex_executable), "--version"],
                capture_output=True,
                text=True,
                check=False
            )
            if ver_proc.returncode == 0:
                self.harness_version = ver_proc.stdout.strip()
        except Exception:
            pass

    def sanitize_path(self, base_env: Optional[Dict[str, str]] = None) -> Tuple[Dict[str, str], bool]:
        """
        Strips any directories containing Engram executables from PATH to ensure
        the control arm cannot discover or execute Engram via system PATH.
        """
        env = (base_env or os.environ).copy()
        raw_path = env.get("PATH", "")
        paths = raw_path.split(os.pathsep)

        filtered = []
        sanitized = False
        for p in paths:
            norm_p = p.lower()
            if "engram" in norm_p:
                sanitized = True
                continue
            filtered.append(p)

        env["PATH"] = os.pathsep.join(filtered)
        return env, sanitized

    def run(
        self,
        sandbox_path: Path,
        prompt: str,
        arm: str,  # "control_A" or "treatment_B"
        output_events_path: Path,
        is_dry_run: bool = False
    ) -> RunExecutionResult:
        """
        Executes a single benchmark run arm inside the specified sandbox.
        """
        started_dt = datetime.datetime.now(datetime.timezone.utc)
        start_time = time.time()
        started_at = started_dt.isoformat()

        output_events_path.parent.mkdir(parents=True, exist_ok=True)
        child_env, path_sanitized = self.sanitize_path()

        if is_dry_run:
            time.sleep(0.1)
            elapsed = round(time.time() - start_time, 2)
            completed_dt = datetime.datetime.now(datetime.timezone.utc)
            completed_at = completed_dt.isoformat()

            thread_id = f"dry-run-{arm}-thread"
            self._write_dry_run_events(output_events_path, arm, thread_id)

            return RunExecutionResult(
                exit_code=0,
                elapsed_seconds=elapsed,
                started_at=started_at,
                completed_at=completed_at,
                events_log_path=str(output_events_path),
                thread_id=thread_id,
                model_name=self.model,
                reasoning_effort=self.reasoning_effort,
                harness_executable=str(self.codex_executable),
                harness_version=self.harness_version,
                path_sanitized=path_sanitized,
                error_message=None
            )

        # Build command with explicit model, reasoning, and canonical sandbox settings
        canonical_sandbox = Path(sandbox_path).resolve()
        sandbox_str = str(canonical_sandbox)
        esc_sandbox = sandbox_str.replace("\\", "\\\\")
        esc_sandbox_lower = sandbox_str.lower().replace("\\", "\\\\")
        fwd_sandbox = sandbox_str.replace("\\", "/")
        fwd_sandbox_lower = sandbox_str.lower().replace("\\", "/")

        cmd = [
            str(self.codex_executable),
            "exec",
            "--json",
            "-m", self.model,
            "-c", f'collaboration_mode.settings.reasoning_effort="{self.reasoning_effort}"',
            "-s", "workspace-write",
            "--add-dir", sandbox_str,
            "-C", sandbox_str,
            "-c", f'projects."{esc_sandbox}".trust_level="trusted"',
            "-c", f'projects."{esc_sandbox_lower}".trust_level="trusted"',
            "-c", f'projects."{fwd_sandbox}".trust_level="trusted"',
            "-c", f'projects."{fwd_sandbox_lower}".trust_level="trusted"',
        ]

        # Dynamic per-invocation MCP configuration
        if arm == "treatment_B":
            if not self.engram_executable or not self.engram_executable.exists():
                raise FileNotFoundError(f"Engram executable missing: {self.engram_executable}")
            esc_bin = str(self.engram_executable.resolve()).replace("\\", "\\\\")
            cmd.extend([
                "-c", f'mcp_servers.engram.command="{esc_bin}"',
                "-c", f'mcp_servers.engram.args=["-C", "{esc_sandbox}", "mcp"]'
            ])

        cmd.append(prompt)

        exit_code = -1
        error_msg = None
        thread_id = None
        proc = None

        try:
            # stdin=subprocess.DEVNULL is mandatory to prevent codex from waiting for EOF on stdin
            proc = subprocess.Popen(
                cmd,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                encoding="utf-8",
                errors="replace",
                cwd=sandbox_str,
                env=child_env
            )

            stdout_data, stderr_data = proc.communicate(timeout=self.timeout_seconds)
            exit_code = proc.returncode

            # Save full JSONL events
            with open(output_events_path, "w", encoding="utf-8") as f:
                f.write(stdout_data)

            # Extract thread_id from stdout if available
            for line in stdout_data.splitlines():
                if '"type":"thread.started"' in line or '"type": "thread.started"' in line:
                    try:
                        ev = json.loads(line)
                        thread_id = ev.get("thread_id")
                        break
                    except Exception:
                        pass

            if exit_code != 0:
                error_msg = f"Codex exited with code {exit_code}: {stderr_data.strip()}"

        except subprocess.TimeoutExpired:
            if proc:
                terminate_pid_tree(proc.pid)
            exit_code = -1
            error_msg = f"Execution timed out after {self.timeout_seconds} seconds (child tree terminated safely)"
        except Exception as e:
            if proc:
                terminate_pid_tree(proc.pid)
            exit_code = -1
            error_msg = str(e)

        elapsed = round(time.time() - start_time, 2)
        completed_dt = datetime.datetime.now(datetime.timezone.utc)
        completed_at = completed_dt.isoformat()

        return RunExecutionResult(
            exit_code=exit_code,
            elapsed_seconds=elapsed,
            started_at=started_at,
            completed_at=completed_at,
            events_log_path=str(output_events_path),
            thread_id=thread_id,
            model_name=self.model,
            reasoning_effort=self.reasoning_effort,
            harness_executable=str(self.codex_executable),
            harness_version=self.harness_version,
            path_sanitized=path_sanitized,
            error_message=error_msg
        )

    def _write_dry_run_events(self, output_path: Path, arm: str, thread_id: str) -> None:
        """Writes synthetic JSONL events for dry-run verification."""
        events = [
            {"type": "thread.started", "thread_id": thread_id},
            {"type": "turn.started"}
        ]

        if arm == "treatment_B":
            events.extend([
                {
                    "type": "item.completed",
                    "item": {
                        "id": "item_1",
                        "type": "mcp_tool_call",
                        "server": "engram",
                        "tool": "engram_query",
                        "arguments": {"query": "theme configuration"},
                        "result": {"content": [{"type": "text", "text": "Relevant decisions found."}]}
                    }
                },
                {
                    "type": "turn.completed",
                    "usage": {
                        "input_tokens": 12500,
                        "cached_input_tokens": 10000,
                        "cache_write_input_tokens": 0,
                        "output_tokens": 150,
                        "reasoning_output_tokens": 20
                    }
                }
            ])
        else:
            events.extend([
                {
                    "type": "item.completed",
                    "item": {
                        "id": "item_1",
                        "type": "agent_message",
                        "text": "Inspecting repository files manually without MCP..."
                    }
                },
                {
                    "type": "turn.completed",
                    "usage": {
                        "input_tokens": 28400,
                        "cached_input_tokens": 21000,
                        "cache_write_input_tokens": 0,
                        "output_tokens": 320,
                        "reasoning_output_tokens": 45
                    }
                }
            ])

        with open(output_path, "w", encoding="utf-8") as f:
            for ev in events:
                f.write(json.dumps(ev) + "\n")
