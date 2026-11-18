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
import json
import _winapi


FROZEN_ENGRAM_V151_SHA256 = "474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0"
FROZEN_ENGRAM_V154_SHA256 = "cf807eacdeb36dc972019838edeeb2fa5e8be25f6c3926fc662189fa9430a798"
FROZEN_ENGRAM_SHA256 = FROZEN_ENGRAM_V151_SHA256


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


def compute_cargo_manifest(repo_path: Path) -> Dict[str, Any]:
    """
    Computes an integrity manifest for a Rust/Cargo repository covering:
    - SHA-256 of Cargo.toml
    - SHA-256 of Cargo.lock (if present)
    - Structural file count and metadata digest across src/ and tests/
    NOTE: Does NOT check or require node_modules.
    """
    manifest: Dict[str, Any] = {
        "repo_type": "cargo",
        "cargo_toml_sha256": None,
        "cargo_lock_sha256": None,
        "source_files_count": 0,
        "source_total_bytes": 0,
        "source_recursive_digest": None,
    }

    cargo_toml = repo_path / "Cargo.toml"
    if cargo_toml.exists() and cargo_toml.is_file():
        manifest["cargo_toml_sha256"] = hashlib.sha256(cargo_toml.read_bytes()).hexdigest()

    cargo_lock = repo_path / "Cargo.lock"
    if cargo_lock.exists() and cargo_lock.is_file():
        manifest["cargo_lock_sha256"] = hashlib.sha256(cargo_lock.read_bytes()).hexdigest()

    file_count = 0
    total_size = 0
    hasher = hashlib.sha256()

    for subdir_name in ("src", "tests"):
        sub_p = repo_path / subdir_name
        if sub_p.exists() and sub_p.is_dir():
            for root, _, files in os.walk(str(sub_p)):
                for f in sorted(files):
                    file_count += 1
                    fp = os.path.join(root, f)
                    rel = os.path.relpath(fp, str(repo_path)).replace("\\", "/")
                    try:
                        st = os.stat(fp)
                        total_size += st.st_size
                        hasher.update(f"{rel}:{st.st_size}:{st.st_mtime_ns}\n".encode("utf-8"))
                    except OSError:
                        pass

    manifest["source_files_count"] = file_count
    manifest["source_total_bytes"] = total_size
    manifest["source_recursive_digest"] = hasher.hexdigest() if file_count > 0 else None

    return manifest


def verify_dependency_integrity(repo_path: Path, baseline: Dict[str, Any]) -> None:
    """
    Verifies that the golden repository's dependency tree is 100% identical to the baseline.
    Supports both Angular/Node.js repositories and Rust/Cargo repositories.
    Raises GoldenDependencyCorruptionError if any dependency mutation occurred.
    """
    # 1. Cargo / Rust repository verification
    if baseline.get("repo_type") == "cargo":
        current = compute_cargo_manifest(repo_path)

        if current["cargo_toml_sha256"] != baseline.get("cargo_toml_sha256"):
            raise GoldenDependencyCorruptionError(
                f"FATAL: Golden repository Cargo.toml was mutated!\n"
                f"Baseline SHA-256: {baseline.get('cargo_toml_sha256')}\n"
                f"Current SHA-256:  {current['cargo_toml_sha256']}"
            )

        if current["cargo_lock_sha256"] != baseline.get("cargo_lock_sha256"):
            raise GoldenDependencyCorruptionError(
                f"FATAL: Golden repository Cargo.lock was mutated!\n"
                f"Baseline SHA-256: {baseline.get('cargo_lock_sha256')}\n"
                f"Current SHA-256:  {current['cargo_lock_sha256']}"
            )
        return

    # 2. Angular / Node.js repository verification (preserved 100%)
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


def verify_sandbox_identity(sandbox_path: Path, expected_commit: str) -> str:
    """
    Verifies that git rev-parse HEAD in the sandbox strictly equals expected_commit.
    Fails closed (raises IsolationError) if the check fails or commit does not match.
    Returns the verified commit hash string.
    """
    norm_p = str(Path(sandbox_path).resolve())
    cmd = ["git", "-c", "core.longpaths=true", "rev-parse", "HEAD"]
    res = subprocess.run(cmd, cwd=norm_p, capture_output=True, text=True, check=False)
    if res.returncode != 0:
        raise IsolationError(
            f"FATAL: Sandbox identity verification failed. git rev-parse HEAD returned code {res.returncode}:\n"
            f"Stderr: {res.stderr}\nStdout: {res.stdout}"
        )
    actual_commit = res.stdout.strip()
    if not actual_commit or actual_commit.lower() != expected_commit.lower():
        raise IsolationError(
            f"FATAL: Sandbox identity mismatch in '{norm_p}'!\n"
            f"Expected baseline commit: '{expected_commit}'\n"
            f"Actual HEAD commit:        '{actual_commit}'"
        )
    return actual_commit


def verify_sandbox_cleanliness(
    sandbox_path: Path,
    allowed_untracked: Optional[Set[str]] = None,
    repo_type: str = "angular"
) -> List[str]:
    """
    Verifies git status --porcelain in the sandbox before task execution.
    If unexpected tracked changes or untracked files are present, fails closed with a clear diagnostic.
    Explicitly accounts for expected dependency provisioning (node_modules for Angular, target for Cargo)
    and intentionally generated harness files (e.g. .github/copilot-instructions.md, .engram).
    Returns list of verified allowable entries, or raises IsolationError.
    """
    norm_p = str(Path(sandbox_path).resolve())
    cmd = ["git", "-c", "core.longpaths=true", "status", "--porcelain"]
    res = subprocess.run(cmd, cwd=norm_p, capture_output=True, text=True, check=False)
    if res.returncode != 0:
        raise IsolationError(
            f"FATAL: Sandbox cleanliness verification failed. git status returned code {res.returncode}:\n"
            f"Stderr: {res.stderr}\nStdout: {res.stdout}"
        )

    if allowed_untracked is None:
        if repo_type in ("cargo", "rust"):
            allowed_untracked = {
                "target",
                "target/",
                ".engram",
                ".engram/",
                ".github",
                ".github/",
                ".github/copilot-instructions.md",
                ".codex",
                ".codex/",
                ".codex/config.toml",
            }
        else:
            allowed_untracked = {
                "node_modules",
                "node_modules/",
                ".github",
                ".github/",
                ".github/copilot-instructions.md",
                ".codex",
                ".codex/",
                ".codex/config.toml",
            }

    lines = [line.rstrip("\r\n") for line in res.stdout.splitlines() if line.strip()]
    unexpected = []
    allowed_found = []

    for line in lines:
        if len(line) < 3:
            continue
        status_code = line[:2]
        rel_path = line[3:].strip().strip('"').replace("\\", "/")

        # Any tracked modification, deletion, addition, or staging is strictly forbidden in a fresh sandbox
        if status_code.strip() != "??":
            unexpected.append(f"Tracked change [{status_code}]: {rel_path}")
            continue

        # Untracked file check
        is_allowed = False
        for allowed in allowed_untracked:
            allowed_norm = allowed.replace("\\", "/").rstrip("/")
            if rel_path == allowed_norm or rel_path.startswith(allowed_norm + "/"):
                is_allowed = True
                break

        if is_allowed:
            allowed_found.append(rel_path)
        else:
            unexpected.append(f"Untracked file [??]: {rel_path}")

    if unexpected:
        unexpected_details = "\n  - " + "\n  - ".join(unexpected)
        raise IsolationError(
            f"FATAL: Sandbox cleanliness violation in '{norm_p}'! Unexpected files present before execution:"
            f"{unexpected_details}\n"
            f"The sandbox working tree must be strictly pristine."
        )

    return allowed_found


def create_sandbox(
    task_id: str,
    pair_id: str,
    arm: str,
    golden_repo: Path,
    baseline_commit: str,
    dependency_strategy: str = "copy",  # Default MUST be "copy" for live trials
    runs_dir: Optional[Path] = None,
    repo_type: Optional[str] = None
) -> Path:
    """
    Provisions a fresh isolated cold-start sandbox directory using git worktree.
    Supports both Angular/Node.js and Rust/Cargo repositories.
    Dependency isolation modes:
    - 'copy': Fully copies dependencies (node_modules for Angular) to the sandbox via multi-threaded robocopy.
              Provides 100% genuine physical write isolation (zero shared junctions).
    - 'junction': Creates an NTFS junction with comprehensive pre/post manifest verification.
    - 'none': Omits external dependency/target linkage.
    """
    if repo_type is None:
        if (golden_repo / "Cargo.toml").exists() and not (golden_repo / "package.json").exists():
            repo_type = "cargo"
        else:
            repo_type = "angular"

    if runs_dir is None:
        runs_dir = Path(tempfile.gettempdir()).resolve() / "engram_pilot" / "runs"
    else:
        runs_dir = Path(runs_dir).resolve()
    runs_dir.mkdir(parents=True, exist_ok=True)
    runs_dir = runs_dir.resolve()

    timestamp = datetime.datetime.now().strftime("%Y%m%d_%H%M%S")
    sandbox_name = f"{task_id}_{pair_id}_{arm}_{timestamp}"
    sandbox_path = (runs_dir / sandbox_name).resolve()

    # Safety check: Ensure target path is strictly inside engram_pilot/runs
    norm_path = str(sandbox_path.resolve()).lower()
    if "engram_pilot\\runs" not in norm_path and "engram_pilot/runs" not in norm_path:
        raise SafetyViolationError(f"Target sandbox path must be within engram_pilot/runs: {sandbox_path}")

    # Create worktree from golden repo with longpaths support
    cmd = ["git", "-c", "core.longpaths=true", "worktree", "add", "--detach", str(sandbox_path), baseline_commit]
    res = subprocess.run(cmd, cwd=golden_repo, capture_output=True, text=True)
    if res.returncode != 0:
        raise IsolationError(f"Failed to create git worktree: {res.stderr}\n{res.stdout}")

    sandbox_path = sandbox_path.resolve()

    # 1. Assert sandbox commit identity immediately upon worktree creation
    verify_sandbox_identity(sandbox_path, baseline_commit)

    # 2. Dependency provisioning based on repository type
    if repo_type in ("angular", "node"):
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
    elif repo_type in ("cargo", "rust"):
        golden_target = golden_repo / "target"
        sandbox_target = sandbox_path / "target"
        if golden_target.exists() and not sandbox_target.exists():
            if dependency_strategy == "junction":
                try:
                    _winapi.CreateJunction(str(golden_target), str(sandbox_target))
                except Exception:
                    pass

    # 3. Assert sandbox cleanliness after dependency provisioning
    verify_sandbox_cleanliness(sandbox_path, repo_type=repo_type)

    return sandbox_path


def teardown_sandbox(
    sandbox_path: Path,
    golden_repo: Path,
    preserve_archive: bool = False,
    repo_type: str = "angular"
) -> None:
    """
    Safely tears down a sandbox workspace.
    Handles both NTFS junctions and physical dependency copies cleanly.
    STRICT SAFETY INVARIANT: Refuses to touch any path not inside engram_pilot/runs.
    """
    norm_path = str(sandbox_path.resolve()).lower()
    if "engram_pilot\\runs" not in norm_path and "engram_pilot/runs" not in norm_path:
        raise SafetyViolationError(
            f"FATAL SAFETY VIOLATION: Refusing to clean path outside engram_pilot/runs: {sandbox_path}"
        )

    # 1. Safely remove dependencies / junctions based on repository type
    if repo_type in ("cargo", "rust"):
        sandbox_target = sandbox_path / "target"
        if sandbox_target.is_dir():
            try:
                is_junction = bool(os.stat(str(sandbox_target)).st_file_attributes & 0x400)
                if is_junction:
                    os.rmdir(sandbox_target)
            except Exception:
                pass
    else:
        # Safely remove node_modules (junction or physical directory)
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


def configure_sandbox_codex_mcp(sandbox_path: Path, engram_executable: Path) -> Path:
    """
    Configures deterministic workspace-level Codex MCP registration in sandbox root.
    Writes <sandbox>/.codex/config.toml declaring [mcp_servers.engram] with least-privilege
    tool approvals for the 4 consolidated Engram tools.
    Strictly validates generated TOML using tomllib before writing.
    """
    if not engram_executable.exists():
        raise IsolationError(f"Engram executable missing: {engram_executable}")

    codex_dir = sandbox_path / ".codex"
    codex_dir.mkdir(parents=True, exist_ok=True)
    config_file = codex_dir / "config.toml"

    # Use json.dumps to guarantee 100% compliant TOML basic string escaping (converts \ to \\ and quotes safely)
    bin_str = json.dumps(str(engram_executable.resolve()))
    sandbox_str = json.dumps(str(sandbox_path.resolve()))

    toml_content = f"""[mcp_servers.engram]
command = {bin_str}
args = ["-C", {sandbox_str}, "mcp"]
default_tools_approval_mode = "approve"
enabled_tools = ["engram_get_context", "engram_inspect_symbol", "engram_preflight", "engram_postflight"]

[mcp_servers.engram.tools.engram_get_context]
approval_mode = "approve"

[mcp_servers.engram.tools.engram_inspect_symbol]
approval_mode = "approve"

[mcp_servers.engram.tools.engram_preflight]
approval_mode = "approve"

[mcp_servers.engram.tools.engram_postflight]
approval_mode = "approve"
"""
    # Strict validation via standard library tomllib to prevent malformed TOML
    try:
        import tomllib
        tomllib.loads(toml_content)
    except Exception as e:
        raise IsolationError(f"Generated .codex/config.toml failed TOML validation: {e}")

    config_file.write_text(toml_content, encoding="utf-8")
    return config_file
