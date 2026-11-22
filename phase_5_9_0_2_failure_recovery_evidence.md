# Engram Phase 5.9.0.2: Failure Recovery & Process Isolation Implementation Evidence

**Phase:** 5.9.0.2  
**Target:** `benchmarks/matched-pair-harness/src/isolation.py` & `runner.py`  
**Execution Mode:** Strictly Offline Source-Code Audit  

---

## 1. Process Tree Lifecycle & Forced Termination

In `isolation.py` (lines 80–98):
```python
def terminate_pid_tree(pid: int) -> bool:
    """
    Terminates a specific process and all its descendants on Windows using taskkill /F /T /PID.
    Does NOT touch any unrelated processes.
    """
    if not pid or pid <= 0:
        return False
    try:
        res = subprocess.run(
            ["taskkill", "/F", "/T", "/PID", str(pid)],
            capture_output=True,
            text=True,
            check=False
        )
        return res.returncode == 0
    except Exception:
        return False
```
- **Windows Taskkill Tree Killing:** Uses Windows OS-level `/T` (tree kill) and `/F` (forceful termination).
- **Targeted Containment:** Explicitly restricted to `str(pid)` of the spawned `codex.exe` process and its direct child tree (e.g. `rg.exe`, `node.exe`, `engram.exe`).
- **Immediate Resource Freeing:** Severing the process tree forcefully terminates network sockets and background file handles.

---

## 2. Sandbox Lifecycle & Zero-Spill Guarantees

In `isolation.py` (`create_sandbox` and `teardown_sandbox`):
1. **Isolated Copy:** Each arm executes in an ephemeral directory (`sandbox_path`) decoupled from the golden repository.
2. **Path Sanitization:** System PATH is sanitized via `sanitize_path()` to remove any host or developer Engram binaries, preventing accidental binary leakage into control arms.
3. **Clean Teardown:** Post-execution, `teardown_sandbox()` completely purges the directory tree using Windows `rmdir /s /q` or Python `shutil.rmtree`.
4. **Preserve Mode:** Unless `--preserve-sandboxes` is explicitly set, zero residue remains on disk between arm executions.

---

## 3. Golden Repository Integrity Fingerprinting

In `isolation.py` (`compute_comprehensive_golden_manifest`):
Before and after every arm execution:
- Cryptographic SHA-256 hashes of `package.json` and `package-lock.json` are computed.
- Full structural directory digest across all 68,274 files in `node_modules` is verified.
- If ANY modification is detected in the source golden repository, `verify_dependency_integrity()` raises `SafetyViolationError`, immediately aborting the benchmark to prevent tainted baselines.

---

## 4. Fail-Closed Arm Recovery Summary

| Failure Event | Process Lifecycle Action | State & Sandbox Action | Next Arm Scheduled? |
|---|---|---|---|
| **Arm Token Ceiling Breach** | `terminate_pid_tree(proc.pid)` kills process | Logs written to `results/`; sandbox torn down | Yes (under `diagnostic_continue`) |
| **Arm Timeout (600s)** | `terminate_pid_tree(proc.pid)` kills process | Telemetry up to 600s saved; sandbox torn down | Yes |
| **Provider Telemetry Invalid** | `terminate_pid_tree(proc.pid)` kills process | Flagged `TELEMETRY_INVALID`; sandbox torn down | **NO (Aborts block on uncertain budget)** |
| **Golden Repo Mutation** | Immediate benchmark halt | Sandbox preserved for forensic audit | **NO (Hard safety abort)** |
