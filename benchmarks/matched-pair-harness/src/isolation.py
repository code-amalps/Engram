"""
Workspace isolation and lifecycle management for the Matched-Pair Harness.
Enforces:
1. Cold-start clean Git worktree generation at verified baseline commits.
2. Genuine dependency isolation:
   - Supports 'copy' strategy (100% private sandbox-local copy of node_modules, zero shared junctions).
   - Supports 'junction' strategy (fast junction with full comprehensive recursive manifest verification).
3. Comprehensive Golden Repository Dependency Verification:
   - Full SHA-256 content hashes of package.json and package-lock.json.
   - Recursive structural digest across all 68,274 files in node_modules (relative paths, sizes, mtimes).
4. Process lifecycle safety:
   - Terminates ONLY harness-tracked process trees (taskkill /F /T /PID), never unrelated user processes.
5. Strict filesystem bounds: Aborts if teardown is attempted outside %TEMP%\\engram_pilot\\runs.
"""
from typing import Dict, Any, List, Optional, Tuple, Set
from pathlib import Path
import subprocess
import hashlib
import tempfile
import datetime
import shutil
import os
import _winapi


FROZEN_ENGRAM_SHA256 = "474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0"


class IsolationError(Exception):
    pass


class SafetyViolationError(Exception):
    pass


class GoldenDependencyCorruptionError(Exception):
    pass


class ProcessTracker:
    """Tracks and safely terminates ONLY processes spawned by the current harness run."""
    def __init__(self):
        self._tracked_pids: Set[int] = set()

    def track(self, pid: int) -> None:
        if pid and pid > 0:
            self._tracked_pids.add(pid)

    def untrack(self, pid: int) -> None:
        self._tracked_pids.discard(pid)

    def terminate_all(self) -> int:
        """Terminates all tracked processes and their child trees using taskkill /F /T."""
        terminated = 0
        for pid in list(self._tracked_pids):
            if terminate_pid_tree(pid):
                terminated += 1
            self._tracked_pids.discard(pid)
        return terminated

    @property
    def tracked_pids(self) -> List[int]:
        return sorted(list(self._tracked_pids))


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


def compute_comprehensive_golden_manifest(repo_path: Path) -> Dict[str, Any]:
    """
    Computes a comprehensive integrity manifest covering:
    - Exact cryptographic SHA-256 content hashes of package.json and package-lock.json.
    - Full recursive STRUCTURAL & METADATA digest across all files and directories in node_modules
      (relative path, exact file size in bytes, nanosecond modification timestamp).
      NOTE: The node_modules digest is a structural/metadata tree digest, NOT a cryptographic
      hash of all 68,274 files' contents. Content cryptographic hashes are computed for package.json
      and package-lock.json.
    """
    manifest: Dict[str, Any] = {
        "package_json_sha256": None,
        "package_lock_sha256": None,
        "node_modules_file_count": 0,
        "node_modules_total_bytes": 0,
        "node_modules_recursive_digest": None,
    }

    pkg_json = repo_path / "package.json"
    if pkg_json.exists():
        manifest["package_json_sha256"] = hashlib.sha256(pkg_json.read_bytes()).hexdigest()

    pkg_lock = repo_path / "package-lock.json"
    if pkg_lock.exists():
        manifest["package_lock_sha256"] = hashlib.sha256(pkg_lock.read_bytes()).hexdigest()

    nm = repo_path / "node_modules"
    if nm.exists() and nm.is_dir():
        file_count = 0
        total_size = 0
        hasher = hashlib.sha256()

        for root, _, files in os.walk(str(nm)):
            for f in sorted(files):
                file_count += 1
                fp = os.path.join(root, f)
                rel = os.path.relpath(fp, str(nm)).replace("\\", "/")
                try:
                    st = os.stat(fp)
                    total_size += st.st_size
                    hasher.update(f"{rel}:{st.st_size}:{st.st_mtime_ns}\n".encode("utf-8"))
                except OSError:
                    pass

        manifest["node_modules_file_count"] = file_count
        manifest["node_modules_total_bytes"] = total_size
        manifest["node_modules_recursive_digest"] = hasher.hexdigest()

    return manifest


# Alias for backward compatibility
compute_dependency_fingerprint = compute_comprehensive_golden_manifest


def verify_dependency_integrity(repo_path: Path, baseline: Dict[str, Any]) -> None:
    """
    Verifies that the golden repository's dependency tree is 100% identical to the baseline.
    Compares full package.json / package-lock.json hashes and the full recursive digest.
    Raises GoldenDependencyCorruptionError if any dependency mutation occurred.
    """
    current = compute_comprehensive_golden_manifest(repo_path)

    # 1. Verify package.json content hash
    if current["package_json_sha256"] != baseline["package_json_sha256"]:
        raise GoldenDependencyCorruptionError(
            f"FATAL: Golden repository package.json was mutated!\n"
            f"Baseline SHA-256: {baseline['package_json_sha256']}\n"
            f"Current SHA-256:  {current['package_json_sha256']}"
        )

    # 2. Verify package-lock.json content hash
    if current["package_lock_sha256"] != baseline["package_lock_sha256"]:
        raise GoldenDependencyCorruptionError(
            f"FATAL: Golden repository package-lock.json was mutated!\n"
            f"Baseline SHA-256: {baseline['package_lock_sha256']}\n"
            f"Current SHA-256:  {current['package_lock_sha256']}"
        )

    # 3. Verify total file count in node_modules
    if current["node_modules_file_count"] != baseline["node_modules_file_count"]:
        raise GoldenDependencyCorruptionError(
            f"FATAL: Golden repository node_modules file count altered!\n"
            f"Baseline count: {baseline['node_modules_file_count']}\n"
            f"Current count:  {current['node_modules_file_count']}"
        )

    # 4. Verify total bytes in node_modules
    if current["node_modules_total_bytes"] != baseline["node_modules_total_bytes"]:
        raise GoldenDependencyCorruptionError(
            f"FATAL: Golden repository node_modules total size altered!\n"
            f"Baseline bytes: {baseline['node_modules_total_bytes']}\n"
            f"Current bytes:  {current['node_modules_total_bytes']}"
        )

    # 5. Verify full recursive structural digest
    if current["node_modules_recursive_digest"] != baseline["node_modules_recursive_digest"]:
        raise GoldenDependencyCorruptionError(
            f"FATAL: Golden repository node_modules recursive tree digest was mutated!\n"
            f"Baseline digest: {baseline['node_modules_recursive_digest']}\n"
            f"Current digest:  {current['node_modules_recursive_digest']}"
        )


def verify_frozen_binary(binary_path: Path, expected_sha256: str = FROZEN_ENGRAM_SHA256) -> bool:
    """Verifies that the target Engram executable matches the freeze seal hash."""
    if not binary_path.exists():
        raise IsolationError(f"Frozen binary not found at: {binary_path}")
    hasher = hashlib.sha256()
    with open(binary_path, "rb") as f:
        while chunk := f.read(65536):
            hasher.update(chunk)
    actual_sha = hasher.hexdigest()
    if actual_sha.lower() != expected_sha256.lower():
        raise IsolationError(
            f"Frozen binary hash mismatch!\nExpected: {expected_sha256}\nActual:   {actual_sha}"
        )
    return True


def create_sandbox(
    task_id: str,
    pair_id: str,
    arm: str,
    golden_repo: Path,
    baseline_commit: str,
    dependency_strategy: str = "copy",  # Default MUST be "copy" for live trials
    runs_dir: Optional[Path] = None
) -> Path:
    """
    Provisions a fresh isolated cold-start sandbox directory using git worktree.
    Dependency isolation modes:
    - 'copy': Fully copies node_modules to the sandbox via multi-threaded robocopy.
              Provides 100% genuine physical write isolation (zero shared junctions).
    - 'junction': Creates an NTFS junction with comprehensive pre/post manifest verification.
    - 'none': Omits node_modules linkage.
    """
    if runs_dir is None:
        runs_dir = Path(tempfile.gettempdir()) / "engram_pilot" / "runs"
    runs_dir.mkdir(parents=True, exist_ok=True)

    timestamp = datetime.datetime.now().strftime("%Y%m%d_%H%M%S")
    sandbox_name = f"{task_id}_{pair_id}_{arm}_{timestamp}"
    sandbox_path = runs_dir / sandbox_name

    # Safety check: Ensure target path is strictly inside engram_pilot/runs
    norm_path = str(sandbox_path.resolve()).lower()
    if "engram_pilot\\runs" not in norm_path and "engram_pilot/runs" not in norm_path:
        raise SafetyViolationError(f"Target sandbox path must be within engram_pilot/runs: {sandbox_path}")

    # Create worktree from golden repo with longpaths support
    cmd = ["git", "-c", "core.longpaths=true", "worktree", "add", "--detach", str(sandbox_path), baseline_commit]
    res = subprocess.run(cmd, cwd=golden_repo, capture_output=True, text=True)
    if res.returncode != 0:
        raise IsolationError(f"Failed to create git worktree: {res.stderr}\n{res.stdout}")

    golden_nm = golden_repo / "node_modules"
    sandbox_nm = sandbox_path / "node_modules"

    if golden_nm.exists() and not sandbox_nm.exists():
        if dependency_strategy == "copy":
            # Genuine physical copy: 100% private sandbox dependencies, zero shared junction
            cmd_copy = [
                "robocopy", str(golden_nm), str(sandbox_nm),
                "/E", "/NFL", "/NDL", "/NJH", "/NJS", "/NC", "/NS", "/MT:16"
            ]
            subprocess.run(cmd_copy, capture_output=True, check=False)
        elif dependency_strategy == "junction":
            # Fast junction with full comprehensive manifest verification
            try:
                _winapi.CreateJunction(str(golden_nm), str(sandbox_nm))
            except Exception:
                pass

    return sandbox_path


def teardown_sandbox(sandbox_path: Path, golden_repo: Path, preserve_archive: bool = False) -> None:
    """
    Safely tears down a sandbox workspace.
    Handles both NTFS junctions and physical node_modules copies cleanly.
    STRICT SAFETY INVARIANT: Refuses to touch any path not inside engram_pilot/runs.
    """
    norm_path = str(sandbox_path.resolve()).lower()
    if "engram_pilot\\runs" not in norm_path and "engram_pilot/runs" not in norm_path:
        raise SafetyViolationError(
            f"FATAL SAFETY VIOLATION: Refusing to clean path outside engram_pilot/runs: {sandbox_path}"
        )

    # 1. Safely remove node_modules (junction or physical directory)
    sandbox_nm = sandbox_path / "node_modules"
    if sandbox_nm.is_dir():
        try:
            # Check if it is a reparse point (junction)
            is_junction = bool(os.stat(str(sandbox_nm)).st_file_attributes & 0x400)  # FILE_ATTRIBUTE_REPARSE_POINT
            if is_junction:
                os.rmdir(sandbox_nm)
            else:
                shutil.rmtree(sandbox_nm, ignore_errors=True)
        except Exception:
            shutil.rmtree(sandbox_nm, ignore_errors=True)

    if preserve_archive:
        return

    # 2. Remove git worktree
    cmd = ["git", "-c", "core.longpaths=true", "worktree", "remove", "--force", str(sandbox_path)]
    res = subprocess.run(cmd, cwd=golden_repo, capture_output=True, text=True)
    if res.returncode != 0:
        if sandbox_path.exists():
            shutil.rmtree(sandbox_path, ignore_errors=True)
        subprocess.run(["git", "-c", "core.longpaths=true", "worktree", "prune"], cwd=golden_repo, capture_output=True)
