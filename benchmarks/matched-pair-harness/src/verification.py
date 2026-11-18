"""
Tracked Subprocess Verification Workflow for Matched-Pair and Triad Benchmarks.

Provides robust, host-side execution of compilers, linters, and test suites:
1. Executes commands as tracked subprocesses with incremental stdout/stderr capture.
2. Performs host-side bounded waiting with adaptive polling (50ms - 250ms backoff)
   to completely eliminate busy-looping and model-turn token amplification.
3. Automatically terminates the full process tree (taskkill /F /T /PID on Windows)
   on timeout or cancellation to prevent orphaned compiler/node processes.
4. Preserves partial stdout and stderr across timeouts, crashes, and cancellations.
5. Propagates exit codes and maps results into deterministic quality-gate statuses.
"""
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional, Any, Union
import collections
import subprocess
import threading
import datetime
import argparse
import time
import sys
import os

try:
    from isolation import terminate_pid_tree
except ImportError:
    # Standalone / fallback import
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from isolation import terminate_pid_tree


class BoundedStreamBuffer:
    """
    Thread-safe stream buffer that caps memory consumption at max_chars,
    preserving initial output (head) and trailing output (tail) with a clear truncation marker.
    """
    def __init__(self, max_chars: int = 25_000):
        self.max_chars = max(100, max_chars)
        self.head_limit = self.max_chars // 2
        self.tail_limit = self.max_chars - self.head_limit
        self.head_lines: List[str] = []
        self.head_chars: int = 0
        self.tail_lines: collections.deque = collections.deque()
        self.tail_chars: int = 0
        self.total_chars_seen: int = 0
        self.truncated: bool = False
        self.lock = threading.Lock()

    def append(self, line: str) -> None:
        if not line:
            return
        # Cap single oversized chunk (e.g. minified asset or single runaway line)
        if len(line) > self.max_chars:
            line = line[:self.max_chars]
        line_len = len(line)
        with self.lock:
            self.total_chars_seen += line_len
            if self.head_chars + line_len <= self.head_limit:
                self.head_lines.append(line)
                self.head_chars += line_len
            elif self.head_chars < self.head_limit:
                avail = self.head_limit - self.head_chars
                self.head_lines.append(line[:avail])
                self.head_chars += avail
                remainder = line[avail:]
                self._append_tail(remainder)
                self.truncated = True
            else:
                self._append_tail(line)
                self.truncated = True

    def _append_tail(self, chunk: str) -> None:
        self.tail_lines.append(chunk)
        self.tail_chars += len(chunk)
        while self.tail_chars > self.tail_limit and self.tail_lines:
            dropped = self.tail_lines.popleft()
            self.tail_chars -= len(dropped)

    def get_output(self) -> str:
        with self.lock:
            if not self.truncated:
                return "".join(self.head_lines)
            omitted = self.total_chars_seen - (self.head_chars + self.tail_chars)
            return (
                "".join(self.head_lines)
                + f"\n[... OUTPUT TRUNCATED: {omitted} characters omitted ...]\n"
                + "".join(self.tail_lines)
            )


@dataclass
class VerificationResult:
    """Encapsulates the execution result of a verified subprocess command."""
    command: str
    exit_code: int
    stdout: str
    stderr: str
    combined_output: str
    elapsed_seconds: float
    timed_out: bool
    cancelled: bool
    status: str  # "PASS", "FAIL", "TIMEOUT", "CANCELLED", "BLOCKED"
    poll_count: int
    partial_output_preserved: bool
    output_truncated: bool = False
    stdout_truncated: bool = False
    stderr_truncated: bool = False

    def to_dict(self) -> Dict[str, Any]:
        return {
            "command": self.command,
            "exit_code": self.exit_code,
            "stdout": self.stdout,
            "stderr": self.stderr,
            "combined_output": self.combined_output,
            "elapsed_seconds": self.elapsed_seconds,
            "timed_out": self.timed_out,
            "cancelled": self.cancelled,
            "status": self.status,
            "poll_count": self.poll_count,
            "partial_output_preserved": self.partial_output_preserved,
            "output_truncated": self.output_truncated,
            "stdout_truncated": self.stdout_truncated,
            "stderr_truncated": self.stderr_truncated,
        }


class TrackedSubprocessRunner:
    """
    Host-side runner that manages verification commands as tracked subprocesses.
    Executes outside the model's token turn, avoiding repeated model polling.
    """
    def __init__(
        self,
        default_timeout_seconds: int = 300,
        min_poll_interval_s: float = 0.05,
        max_poll_interval_s: float = 0.25,
        backoff_factor: float = 1.25,
        max_output_chars: int = 50_000
    ):
        self.default_timeout_seconds = default_timeout_seconds
        self.min_poll_interval_s = min_poll_interval_s
        self.max_poll_interval_s = max_poll_interval_s
        self.backoff_factor = backoff_factor
        self.max_output_chars = max_output_chars

    def _terminate_proc_tree(self, proc: Optional[subprocess.Popen]) -> None:
        """Robust process-tree termination handling concurrent child exits gracefully."""
        if proc is None or proc.pid is None:
            return
        if proc.poll() is not None:
            return
        try:
            terminate_pid_tree(proc.pid)
        except Exception:
            pass
        try:
            proc.kill()
        except (ProcessLookupError, PermissionError, OSError):
            pass
        try:
            proc.wait(timeout=0.5)
        except Exception:
            pass

    def run_command(
        self,
        command: str,
        cwd: Optional[Union[str, Path]] = None,
        timeout_seconds: Optional[Union[int, float]] = None,
        cancel_event: Optional[threading.Event] = None,
        env: Optional[Dict[str, str]] = None,
        shell: bool = True,
        max_output_chars: Optional[int] = None
    ) -> VerificationResult:
        """
        Runs a command as a tracked subprocess with incremental capture,
        host-side adaptive polling, guaranteed process-tree cleanup,
        clock-independent monotonic timing, and bounded memory buffers.
        """
        timeout = float(timeout_seconds) if timeout_seconds is not None else float(self.default_timeout_seconds)
        resolved_cwd = str(Path(cwd).resolve()) if cwd else None
        exec_env = env or os.environ.copy()

        cap_total = max_output_chars if max_output_chars is not None else self.max_output_chars
        per_stream_cap = max(100, cap_total // 2)

        stdout_buf = BoundedStreamBuffer(max_chars=per_stream_cap)
        stderr_buf = BoundedStreamBuffer(max_chars=per_stream_cap)

        timed_out = False
        cancelled = False
        poll_count = 0
        proc: Optional[subprocess.Popen] = None
        start_time = time.monotonic()

        def _read_pipe(pipe, buffer: BoundedStreamBuffer):
            try:
                for line in iter(pipe.readline, ""):
                    if not line:
                        break
                    buffer.append(line)
            except Exception:
                pass
            finally:
                try:
                    pipe.close()
                except Exception:
                    pass

        try:
            proc = subprocess.Popen(
                command,
                cwd=resolved_cwd,
                shell=shell,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                encoding="utf-8",
                errors="replace",
                env=exec_env
            )

            # Start streaming reader threads for incremental capture into bounded buffers
            t_out = threading.Thread(target=_read_pipe, args=(proc.stdout, stdout_buf), daemon=True)
            t_err = threading.Thread(target=_read_pipe, args=(proc.stderr, stderr_buf), daemon=True)
            t_out.start()
            t_err.start()

            # Adaptive host-side waiting loop with monotonic timing
            current_interval = self.min_poll_interval_s

            while True:
                poll_count += 1
                ret = proc.poll()
                if ret is not None:
                    # Process completed naturally
                    break

                # Check external cancellation
                if cancel_event and cancel_event.is_set():
                    cancelled = True
                    break

                # Check monotonic timeout
                elapsed = time.monotonic() - start_time
                if elapsed >= timeout:
                    timed_out = True
                    break

                # Sleep bounded adaptive interval
                time.sleep(current_interval)
                current_interval = min(self.max_poll_interval_s, current_interval * self.backoff_factor)

            # Termination handling if timed out or cancelled
            if timed_out or cancelled:
                self._terminate_proc_tree(proc)

            # Robust pipe cleanup: if readers are still blocked (e.g. descendant pipe leak), close pipes
            if proc is not None:
                if t_out.is_alive() and proc.stdout:
                    try:
                        proc.stdout.close()
                    except Exception:
                        pass
                if t_err.is_alive() and proc.stderr:
                    try:
                        proc.stderr.close()
                    except Exception:
                        pass

            # Wait briefly for reader threads to consume remaining pipe buffers
            t_out.join(timeout=1.0)
            t_err.join(timeout=1.0)

        except Exception as e:
            if proc:
                self._terminate_proc_tree(proc)
            stderr_buf.append(f"\n[Verification Runner Error]: {str(e)}\n")

        finally:
            if proc is not None:
                if proc.stdout:
                    try:
                        proc.stdout.close()
                    except Exception:
                        pass
                if proc.stderr:
                    try:
                        proc.stderr.close()
                    except Exception:
                        pass

        elapsed_total = round(time.monotonic() - start_time, 3)

        captured_stdout = stdout_buf.get_output()
        captured_stderr = stderr_buf.get_output()
        has_truncated = stdout_buf.truncated or stderr_buf.truncated

        combined = captured_stdout
        if captured_stderr:
            if combined:
                combined += "\n--- STDERR ---\n" + captured_stderr
            else:
                combined = captured_stderr

        # Resolve exit code and status
        if timed_out:
            exit_code = 124  # Standard timeout exit code
            status = "TIMEOUT"
        elif cancelled:
            exit_code = 130  # Standard interrupt/cancellation code
            status = "CANCELLED"
        elif proc is not None and proc.returncode is not None:
            exit_code = proc.returncode
            status = "PASS" if exit_code == 0 else "FAIL"
        else:
            exit_code = -1
            status = "FAIL"

        has_partial = bool(captured_stdout or captured_stderr)

        return VerificationResult(
            command=command,
            exit_code=exit_code,
            stdout=captured_stdout,
            stderr=captured_stderr,
            combined_output=combined,
            elapsed_seconds=elapsed_total,
            timed_out=timed_out,
            cancelled=cancelled,
            status=status,
            poll_count=poll_count,
            partial_output_preserved=has_partial,
            output_truncated=has_truncated,
            stdout_truncated=stdout_buf.truncated,
            stderr_truncated=stderr_buf.truncated
        )

    def run_suite(
        self,
        gates: Dict[str, Dict[str, Any]],
        cwd: Union[str, Path],
        aggregate_timeout_seconds: Optional[float] = None
    ) -> Dict[str, VerificationResult]:
        """
        Executes a sequential suite of verification gates with monotonic aggregate deadline tracking.
        Each gate receives only the remaining time. Gates that cannot start within budget are marked BLOCKED.
        """
        results: Dict[str, VerificationResult] = {}
        suite_start = time.monotonic()
        budget = float(aggregate_timeout_seconds) if aggregate_timeout_seconds is not None else float("inf")

        for name, cfg in gates.items():
            if not cfg or not cfg.get("enabled", False):
                continue
            cmd = cfg.get("command", "")
            gate_timeout = float(cfg.get("timeout_seconds", self.default_timeout_seconds))

            elapsed = time.monotonic() - suite_start
            remaining = budget - elapsed
            if remaining <= 0:
                results[name] = VerificationResult(
                    command=cmd,
                    exit_code=124,
                    stdout="",
                    stderr="Gate blocked: Aggregate verification deadline exhausted before start.",
                    combined_output="Gate blocked: Aggregate verification deadline exhausted before start.",
                    elapsed_seconds=0.0,
                    timed_out=True,
                    cancelled=False,
                    status="BLOCKED",
                    poll_count=0,
                    partial_output_preserved=False,
                    output_truncated=False,
                    stdout_truncated=False,
                    stderr_truncated=False
                )
                break

            effective_timeout = min(gate_timeout, remaining)
            res = self.run_command(cmd, cwd=cwd, timeout_seconds=effective_timeout)
            results[name] = res
            if res.status != "PASS":
                # Halt remaining gates if a mandatory gate fails or times out
                break

        return results


def main():
    parser = argparse.ArgumentParser(description="Host-Side Subprocess Verification CLI Runner")
    parser.add_argument("--cmd", required=True, help="Shell command to execute")
    parser.add_argument("--cwd", default=None, help="Working directory")
    parser.add_argument("--timeout", type=int, default=300, help="Timeout in seconds")
    args = parser.parse_args()

    runner = TrackedSubprocessRunner(default_timeout_seconds=args.timeout)
    res = runner.run_command(args.cmd, cwd=args.cwd, timeout_seconds=args.timeout)

    if res.stdout:
        sys.stdout.write(res.stdout)
    if res.stderr:
        sys.stderr.write(res.stderr)

    if res.timed_out:
        sys.stderr.write(f"\n[Verification Error] Command timed out after {res.elapsed_seconds}s (tree terminated).\n")
    elif res.cancelled:
        sys.stderr.write(f"\n[Verification Error] Command cancelled.\n")

    sys.exit(res.exit_code)


if __name__ == "__main__":
    main()
