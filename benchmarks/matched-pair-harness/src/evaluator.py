"""
Evaluator module for the Matched-Pair Harness.
Performs independent correctness assessment:
- Git diff impact (files changed, insertions, deletions, unrelated file count, blast radius)
- Automated verification quality gates (L0 typecheck, L1 lint, L2 tests, L3 build)
- Strict dependency integrity protection: Any mutation of package manifests or node_modules fails immediately.
- Independent requirement checklist scoring (C in [0.0, 1.0])
- Quality-adjusted efficiency calculation (Effective Cost = Total Tokens / C)
"""
from dataclasses import dataclass, asdict
from typing import Dict, Any, List, Optional, Tuple, Union
from pathlib import Path
import subprocess
import tempfile
import sqlite3
import fnmatch
import json
import time
import re
import os

from verification import TrackedSubprocessRunner, VerificationResult


class FixturePreflightError(Exception):
    """Raised when a task fixture fails preflight checks before running the model."""
    pass


def resolve_safe_target_path(root_path: Union[str, Path], relative_target: str) -> Path:
    """
    Safely resolves a relative target file within a repository or sandbox root.
    Strictly prevents path traversal and external escapes:
    1. Rejects null, empty, or non-string paths.
    2. Rejects absolute paths (both POSIX '/' and Windows drive 'C:\\' or UNC '\\\\').
    3. Rejects Windows drive prefix specifications (e.g. 'c:foo').
    4. Canonicalizes both root_path and candidate target_path using resolve().
    5. Enforces that resolved candidate is strictly contained within resolved root_path.
    6. Normalizes Windows case and path separators to prevent case-insensitive bypass.
    Raises FixturePreflightError if any boundary check fails.
    """
    if not relative_target or not isinstance(relative_target, str):
        raise FixturePreflightError("Target file path must be a non-empty string.")

    target_p = Path(relative_target)
    if target_p.is_absolute():
        raise FixturePreflightError(
            f"Path traversal security violation: Target path '{relative_target}' must be relative, not absolute."
        )

    if os.path.splitdrive(relative_target)[0]:
        raise FixturePreflightError(
            f"Path traversal security violation: Target path '{relative_target}' contains a drive specification."
        )

    resolved_root = Path(root_path).resolve()
    candidate = (resolved_root / target_p).resolve()

    try:
        candidate.relative_to(resolved_root)
    except ValueError:
        raise FixturePreflightError(
            f"Path traversal security violation: Target path '{relative_target}' escapes repository root: {resolved_root}"
        )

    root_str = os.path.normcase(os.path.abspath(str(resolved_root)))
    cand_str = os.path.normcase(os.path.abspath(str(candidate)))

    sep = os.path.normcase(os.sep)
    if not cand_str.startswith(root_str.rstrip(sep) + sep):
        raise FixturePreflightError(
            f"Path traversal security violation: Target path '{relative_target}' escapes repository root: {resolved_root}"
        )

    return candidate


def validate_fixture_preflight(repo_or_sandbox_path: Union[str, Path], task_config: Dict[str, Any]) -> None:
    """
    Validates that a repository or sandbox fixture copy contains the required baseline preconditions:
    1. Multi-path existence verification ('required_files').
    2. Baseline patterns required to be present ('baseline_present_patterns').
    3. Baseline patterns forbidden from being present ('baseline_absent_patterns').
    4. Single-file typo / exact-match assertions ('target_file', 'original_text', 'expected_text').
    Fails fast with clear diagnostic if any precondition is not satisfied.
    """
    task_id = task_config.get("task_id", "unknown_task")
    preflight_cfg = task_config.get("fixture_preflight")
    if not preflight_cfg:
        return

    # 1. Multi-path existence assertions
    required_files = preflight_cfg.get("required_files", [])
    for rel_f in required_files:
        target_path = resolve_safe_target_path(repo_or_sandbox_path, rel_f)
        if not target_path.exists() or not target_path.is_file():
            raise FixturePreflightError(
                f"Preflight failure for task '{task_id}': Required baseline file does not exist: {rel_f}"
            )

    # 2. Baseline patterns required to be present
    present_patterns = preflight_cfg.get("baseline_present_patterns", [])
    for check in present_patterns:
        rel_f = check.get("file")
        pat = check.get("pattern")
        if rel_f and pat:
            target_path = resolve_safe_target_path(repo_or_sandbox_path, rel_f)
            if not target_path.exists() or not target_path.is_file():
                raise FixturePreflightError(
                    f"Preflight failure for task '{task_id}': File does not exist for pattern check: {rel_f}"
                )
            content = target_path.read_text(encoding="utf-8", errors="ignore")
            if pat not in content:
                raise FixturePreflightError(
                    f"Preflight failure for task '{task_id}': Expected baseline pattern '{pat}' missing from '{rel_f}'."
                )

    # 3. Baseline patterns forbidden to be present (dirty fixture prevention)
    absent_patterns = preflight_cfg.get("baseline_absent_patterns", [])
    for check in absent_patterns:
        rel_f = check.get("file")
        pat = check.get("pattern")
        if rel_f and pat:
            target_path = resolve_safe_target_path(repo_or_sandbox_path, rel_f)
            if target_path.exists() and target_path.is_file():
                content = target_path.read_text(encoding="utf-8", errors="ignore")
                if pat in content:
                    raise FixturePreflightError(
                        f"Preflight failure for task '{task_id}': Baseline fixture is dirty or already modified! "
                        f"Forbidden baseline pattern '{pat}' found in '{rel_f}'."
                    )

    # 4. Single-file exact match / typo check (legacy/task_a compatibility)
    target_file = preflight_cfg.get("target_file")
    if target_file:
        original_text = preflight_cfg.get("original_text")
        if original_text is None or original_text == "":
            raise FixturePreflightError(f"Task '{task_id}' fixture_preflight missing 'original_text'.")

        expected_text = preflight_cfg.get("expected_text")
        if expected_text is None or expected_text == "":
            raise FixturePreflightError(f"Task '{task_id}' fixture_preflight missing 'expected_text'.")

        if original_text == expected_text:
            raise FixturePreflightError(
                f"Task '{task_id}' fixture_preflight original_text and expected_text are identical ('{original_text}'). No-op repair."
            )

        expected_occurrences = preflight_cfg.get("expected_occurrences", 1)

        target_path = resolve_safe_target_path(repo_or_sandbox_path, target_file)
        if not target_path.exists() or not target_path.is_file():
            raise FixturePreflightError(
                f"Preflight failure for task '{task_id}': Target file does not exist: {target_file}"
            )

        content = target_path.read_text(encoding="utf-8", errors="ignore")
        occurrences = content.count(original_text)

        # 1. Target string missing
        if occurrences == 0:
            if expected_text in content:
                raise FixturePreflightError(
                    f"Preflight failure for task '{task_id}': Original target text '{original_text}' "
                    f"not found in target file: {target_file} (expected text '{expected_text}' is already present; fixture is already corrected or dirty)."
                )
            raise FixturePreflightError(
                f"Preflight failure for task '{task_id}': Original target text '{original_text}' "
                f"not found in target file: {target_file}"
            )

        # 2. Reject dirty fixture where expected text is already present alongside original text
        if expected_text in content:
            raise FixturePreflightError(
                f"Preflight failure for task '{task_id}': Baseline fixture is dirty or already corrected! "
                f"Expected text '{expected_text}' is already present in '{target_file}'."
            )

        # 3. Reject ambiguous fixtures (more or fewer occurrences than required)
        if occurrences != expected_occurrences:
            raise FixturePreflightError(
                f"Preflight failure for task '{task_id}': Ambiguous fixture! Target text '{original_text}' "
                f"occurs {occurrences} times in '{target_file}'. Exactly {expected_occurrences} occurrence(s) required."
            )


@dataclass
class GitImpact:
    files_changed_count: int
    files_changed: List[str]
    insertions: int
    deletions: int
    unrelated_files_count: int
    diff_patch_path: str
    dependency_mutated: bool = False

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


@dataclass
class CorrectnessResult:
    score: Optional[float]
    quality_adjusted_cost: Optional[float]
    passed_all_requirements: Optional[bool]
    gates: Dict[str, str]  # L0_typecheck, L1_lint_prettier, L2_unit_tests, L3_production_build
    requirements: Dict[str, str]  # REQ-1: PASS/FAIL/PARTIAL/NOT_EVALUATED

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


class Evaluator:
    def __init__(self, task_config: Dict[str, Any]):
        self.task_config = task_config
        self.task_id = task_config.get("task_id", "")
        self.allowed_patterns = task_config.get("allowed_file_patterns", [])
        self.forbidden_patterns = task_config.get("forbidden_file_patterns", [])
        self.gates_cfg = task_config.get("gates", {})
        self.requirements_cfg = task_config.get("requirements", [])

    def analyze_git_impact(self, sandbox_path: Path, patch_output_path: Path) -> GitImpact:
        """
        Inspects git diff and status against sandbox baseline.
        Accurately captures:
        - Staged new files, staged modifications, and staged deletions
        - Unstaged modifications and unstaged deletions
        - Untracked files created by the agent
        Uses baseline-to-working-tree diff (`git diff HEAD`) so that changes are not missed
        if an agent runs `git add`.
        """
        # 1. Get changed files from git status --porcelain
        cmd_status = ["git", "-c", "core.longpaths=true", "-c", "core.quotepath=false", "status", "--porcelain"]
        res_status = subprocess.run(cmd_status, cwd=sandbox_path, stdin=subprocess.DEVNULL, capture_output=True, text=True)
        changed_files = []
        untracked_files = []
        for raw_line in res_status.stdout.splitlines():
            if not raw_line or len(raw_line) < 3:
                continue
            status_code = raw_line[:2]
            fpath_raw = raw_line[3:].strip()
            if fpath_raw.startswith('"') and fpath_raw.endswith('"'):
                fpath_raw = fpath_raw[1:-1]
            if " -> " in fpath_raw:
                fpath_raw = fpath_raw.split(" -> ")[-1]
            norm_fpath = fpath_raw.replace("\\", "/")
            # Ignore pre-injected Turn-0 infrastructure files from blast-radius accounting
            if (
                norm_fpath.startswith(".engram/")
                or norm_fpath == ".engram"
                or norm_fpath.startswith(".github/")
                or norm_fpath == ".github"
            ):
                continue
            changed_files.append(norm_fpath)
            if status_code == "??":
                untracked_files.append(norm_fpath)

        # 2. Get numstat covering BOTH staged and unstaged changes against HEAD
        cmd_diff = ["git", "-c", "core.longpaths=true", "-c", "core.quotepath=false", "diff", "HEAD", "--numstat"]
        res_diff = subprocess.run(cmd_diff, cwd=sandbox_path, stdin=subprocess.DEVNULL, capture_output=True, text=True)
        if res_diff.returncode != 0:
            # Fallback if HEAD does not exist (e.g. bare initialized repo in synthetic tests)
            cmd_diff = ["git", "-c", "core.longpaths=true", "-c", "core.quotepath=false", "diff", "--numstat"]
            res_diff = subprocess.run(cmd_diff, cwd=sandbox_path, stdin=subprocess.DEVNULL, capture_output=True, text=True)

        insertions = 0
        deletions = 0
        spurious_deleted_files = set()

        for line in res_diff.stdout.splitlines():
            parts = line.split(maxsplit=2)
            if len(parts) >= 3 and parts[0].isdigit() and parts[1].isdigit():
                file_ins = int(parts[0])
                file_del = int(parts[1])
                fpath_diff = parts[2].strip().strip('"').replace("\\", "/")
                if (
                    fpath_diff.startswith(".engram/")
                    or fpath_diff == ".engram"
                    or fpath_diff.startswith(".github/")
                    or fpath_diff == ".github"
                ):
                    continue

                # Distinguish actual modifications from evaluator-generated path failures:
                # If git diff reports all lines deleted, verify if the file still exists on disk
                # and matches HEAD.
                upath = sandbox_path / fpath_diff
                if file_ins == 0 and file_del > 0 and upath.is_file():
                    cmd_show = ["git", "-c", "core.longpaths=true", "-c", "core.quotepath=false", "show", f"HEAD:{fpath_diff}"]
                    res_show = subprocess.run(cmd_show, cwd=sandbox_path, stdin=subprocess.DEVNULL, capture_output=True)
                    if res_show.returncode == 0:
                        try:
                            if upath.read_bytes() == res_show.stdout:
                                # Spurious deletion caused by git working-tree path failure: ignore
                                spurious_deleted_files.add(fpath_diff)
                                if fpath_diff in changed_files:
                                    changed_files.remove(fpath_diff)
                                continue
                        except Exception:
                            pass

                insertions += file_ins
                deletions += file_del
                if fpath_diff not in changed_files and fpath_diff not in spurious_deleted_files:
                    changed_files.append(fpath_diff)

        # Account for insertions in untracked files
        for uf in untracked_files:
            upath = sandbox_path / uf
            if upath.is_file():
                try:
                    lines = upath.read_text(encoding="utf-8", errors="ignore").splitlines()
                    insertions += len(lines)
                except Exception:
                    pass

        # 3. Export full patch covering BOTH staged and unstaged changes against HEAD
        cmd_patch = ["git", "-c", "core.longpaths=true", "-c", "core.quotepath=false", "diff", "HEAD"]
        res_patch = subprocess.run(cmd_patch, cwd=sandbox_path, stdin=subprocess.DEVNULL, capture_output=True, text=True)
        if res_patch.returncode != 0:
            cmd_patch = ["git", "-c", "core.longpaths=true", "-c", "core.quotepath=false", "diff"]
            res_patch = subprocess.run(cmd_patch, cwd=sandbox_path, stdin=subprocess.DEVNULL, capture_output=True, text=True)

        patch_text = res_patch.stdout

        # Filter out spurious deletion diff chunks if any were generated
        if spurious_deleted_files:
            filtered_chunks = []
            current_chunk = []
            skip_chunk = False
            for line in patch_text.splitlines(keepends=True):
                if line.startswith("diff --git"):
                    if current_chunk and not skip_chunk:
                        filtered_chunks.extend(current_chunk)
                    current_chunk = [line]
                    skip_chunk = any(f"b/{sp_f}" in line or f"a/{sp_f}" in line for sp_f in spurious_deleted_files)
                else:
                    current_chunk.append(line)
            if current_chunk and not skip_chunk:
                filtered_chunks.extend(current_chunk)
            patch_text = "".join(filtered_chunks)

        for uf in untracked_files:
            upath = sandbox_path / uf
            if upath.is_file():
                try:
                    ucontent = upath.read_text(encoding="utf-8", errors="ignore")
                    patch_text += f"\n--- /dev/null\n+++ b/{uf}\n"
                    for uline in ucontent.splitlines():
                        patch_text += f"+{uline}\n"
                except Exception:
                    pass

        patch_output_path.parent.mkdir(parents=True, exist_ok=True)
        with open(patch_output_path, "w", encoding="utf-8") as f:
            f.write(patch_text)

        # 4. Check allowed vs forbidden vs unrelated vs dependency mutations
        repo_type = self.task_config.get("repository_type", "angular")
        unrelated_count = 0
        dependency_mutated = False
        for fpath in changed_files:
            # Check dependency mutation based on repository type
            if repo_type in ("cargo", "rust"):
                if fpath in ("Cargo.toml", "Cargo.lock") and not any(fnmatch.fnmatch(fpath, p) for p in self.allowed_patterns):
                    dependency_mutated = True
                    unrelated_count += 1
                    continue
            else:
                if fpath in ("package.json", "package-lock.json") or fpath.startswith("node_modules/"):
                    dependency_mutated = True
                    unrelated_count += 1
                    continue

            # Check forbidden
            is_forbidden = any(fnmatch.fnmatch(fpath, p) for p in self.forbidden_patterns)
            # Check allowed
            is_allowed = any(fnmatch.fnmatch(fpath, p) for p in self.allowed_patterns)
            if is_forbidden or not is_allowed:
                unrelated_count += 1

        return GitImpact(
            files_changed_count=len(changed_files),
            files_changed=changed_files,
            insertions=insertions,
            deletions=deletions,
            unrelated_files_count=unrelated_count,
            diff_patch_path=str(patch_output_path),
            dependency_mutated=dependency_mutated,
        )

    def run_automated_gates(
        self,
        sandbox_path: Path,
        git_impact: Optional[GitImpact] = None,
        is_dry_run: bool = False,
        runner: Optional[TrackedSubprocessRunner] = None,
        aggregate_timeout_seconds: Optional[float] = None
    ) -> Dict[str, str]:
        """
        Runs L0-L3 gates or produces deterministic simulated gates in dry run.
        Enforces monotonic aggregate verification deadline across sequential gates.
        """
        results = {}
        gate_map = {
            "L0_typecheck": self.gates_cfg.get("L0_typecheck"),
            "L1_lint_prettier": self.gates_cfg.get("L1_lint"),
            "L2_unit_tests": self.gates_cfg.get("L2_unit_tests"),
            "L3_production_build": self.gates_cfg.get("L3_production_build"),
        }

        # If shared dependencies were mutated, L0 gate fails immediately
        if git_impact and git_impact.dependency_mutated:
            results["L0_typecheck"] = "FAIL"
            results["L1_lint_prettier"] = "FAIL"
            results["L2_unit_tests"] = "BLOCKED"
            results["L3_production_build"] = "FAIL"
            return results

        active_runner = runner or TrackedSubprocessRunner(default_timeout_seconds=300)
        self.last_gate_details: Dict[str, Any] = {}

        # Aggregate verification timeout resolution
        if aggregate_timeout_seconds is not None:
            budget = float(aggregate_timeout_seconds)
        else:
            budget = float(self.task_config.get("verification_timeout_seconds", 300))

        start_time = time.monotonic()

        for gate_name, gate_info in gate_map.items():
            if not gate_info or not gate_info.get("enabled", False):
                results[gate_name] = "SKIPPED"
                continue

            if is_dry_run:
                results[gate_name] = "PASS"
                continue

            elapsed = time.monotonic() - start_time
            remaining = budget - elapsed
            cmd = gate_info.get("command", "")
            gate_timeout = float(gate_info.get("timeout_seconds", 300))

            if remaining <= 0:
                results[gate_name] = "BLOCKED"
                self.last_gate_details[gate_name] = {
                    "command": cmd,
                    "exit_code": 124,
                    "stdout": "",
                    "stderr": "Gate blocked: Aggregate verification deadline exhausted before start.",
                    "combined_output": "Gate blocked: Aggregate verification deadline exhausted before start.",
                    "elapsed_seconds": 0.0,
                    "timed_out": True,
                    "cancelled": False,
                    "status": "BLOCKED",
                    "poll_count": 0,
                    "partial_output_preserved": False,
                    "output_truncated": False,
                    "stdout_truncated": False,
                    "stderr_truncated": False
                }
                continue

            effective_timeout = min(gate_timeout, remaining)
            res = active_runner.run_command(cmd, cwd=sandbox_path, timeout_seconds=effective_timeout)
            self.last_gate_details[gate_name] = res.to_dict()

            if res.timed_out:
                results[gate_name] = "BLOCKED"
            elif res.exit_code == 0:
                results[gate_name] = "PASS"
            else:
                results[gate_name] = "FAIL"

        return results

    def evaluate_requirements(
        self,
        sandbox_path: Path,
        git_impact: GitImpact,
        gate_results: Dict[str, str]
    ) -> Dict[str, str]:
        """Evaluates checklist requirements based on code inspection, gates, and git impact."""
        req_results = {}

        if git_impact.dependency_mutated:
            for req in self.requirements_cfg:
                req_results[req.get("id", "")] = "FAIL"
            return req_results

        for req in self.requirements_cfg:
            req_id = req.get("id", "")
            eval_type = req.get("evaluation_type", "")

            if eval_type == "AUTOMATED_DIFF":
                if git_impact.unrelated_files_count > 0:
                    req_results[req_id] = "FAIL"
                elif git_impact.files_changed_count == 0:
                    req_results[req_id] = "FAIL"
                else:
                    # Specific scope drift check for Task B: bulk i18n sprawl
                    if self.task_id == "task_b_remember_me":
                        i18n_files = [f for f in git_impact.files_changed if re.search(r'assets/i18n/.+\.json$', f)]
                        if len(i18n_files) >= 3:
                            req_results[req_id] = "FAIL"
                            continue
                    req_results[req_id] = "PASS"

            elif eval_type == "UNIT_TEST":
                l2 = gate_results.get("L2_unit_tests", "SKIPPED")
                if self.task_id == "task_commit_cache_invalidation" and req_id == "REQ-2":
                    drift_ok = self._check_commit_cache_req2_drift_detection(sandbox_path)
                    req_results[req_id] = "PASS" if (l2 == "PASS" and drift_ok) else "FAIL"
                else:
                    req_results[req_id] = "PASS" if l2 == "PASS" else "FAIL"

            elif eval_type == "STATIC_CHECK":
                passed = self._check_static_requirement(sandbox_path, req_id)
                req_results[req_id] = "PASS" if passed else "FAIL"

            else:
                req_results[req_id] = "NOT_EVALUATED"

        return req_results

    def validate_preflight(self, sandbox_path: Path) -> None:
        """Validates baseline fixture preconditions against the sandbox path."""
        validate_fixture_preflight(sandbox_path, self.task_config)

    def _check_static_requirement(self, sandbox_path: Path, req_id: str) -> bool:
        """Domain-specific static checks based on requirement ID and task."""
        if self.task_id == "task_a_typo":
            preflight = self.task_config.get("fixture_preflight", {})
            rel_file = preflight.get("target_file", "projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html")
            orig_text = preflight.get("original_text", "Angular Cli Tips")
            exp_text = preflight.get("expected_text", "Angular CLI Tips")

            if req_id == "REQ-1":
                try:
                    target_path = resolve_safe_target_path(sandbox_path, rel_file)
                except FixturePreflightError:
                    return False
                if not target_path.exists() or not target_path.is_file():
                    return False
                content = target_path.read_text(encoding="utf-8", errors="ignore")

                # Strip HTML comments to prevent false positives from commented-out code
                uncommented = re.sub(r'<!--.*?-->', '', content, flags=re.DOTALL)

                # Normalize whitespace across words to accommodate harmless formatting adjustments
                norm_content = re.sub(r'\s+', ' ', uncommented)
                norm_exp = re.sub(r'\s+', ' ', exp_text)
                norm_orig = re.sub(r'\s+', ' ', orig_text)

                return (norm_exp in norm_content) and (norm_orig not in norm_content)
            return False

        if self.task_id == "task_b_remember_me":
            app_html = sandbox_path / "projects/angular-ngrx-material-starter/src/app/app/app.component.html"
            app_ts = sandbox_path / "projects/angular-ngrx-material-starter/src/app/app/app.component.ts"
            auth_effects = sandbox_path / "projects/angular-ngrx-material-starter/src/app/core/auth/auth.effects.ts"
            settings_model = sandbox_path / "projects/angular-ngrx-material-starter/src/app/core/settings/settings.model.ts"
            settings_reducer = sandbox_path / "projects/angular-ngrx-material-starter/src/app/core/settings/settings.reducer.ts"

            req_cfg = next((r for r in self.requirements_cfg if r.get("id") == req_id), {})
            desc_and_crit = (req_cfg.get("description", "") + " " + req_cfg.get("criteria", "")).lower()

            if req_id == "REQ-1":
                # Check 1A: Auth UI layout (app.component.html has rememberMe checkbox and username input)
                if "ui" in desc_and_crit or "checkbox" in desc_and_crit or "component" in desc_and_crit or "layout" in desc_and_crit:
                    if not app_html.exists() or not app_html.is_file():
                        return False
                    html_content = app_html.read_text(encoding="utf-8", errors="ignore")
                    uncommented = re.sub(r'<!--.*?-->', '', html_content, flags=re.DOTALL)
                    has_checkbox = bool(re.search(r'rememberMe|remember|mat-checkbox', uncommented, re.IGNORECASE))
                    has_username = bool(re.search(r'username|user-input|matInput', uncommented, re.IGNORECASE))
                    return has_checkbox and has_username

                # Check 1B: Settings model fallback
                if "settings" in desc_and_crit or "model" in desc_and_crit:
                    if not settings_model.exists() or not settings_model.is_file():
                        return False
                    model_content = settings_model.read_text(encoding="utf-8", errors="ignore")
                    return bool(re.search(r'rememberMe\s*\??:\s*boolean', model_content, re.IGNORECASE))

                return False

            elif req_id == "REQ-2":
                # Check 2A: Auth persistence restoration logic (app.component.ts or auth.effects.ts)
                if "auth" in desc_and_crit or "persistence" in desc_and_crit or "restor" in desc_and_crit or "flow" in desc_and_crit:
                    found_restoration = False
                    for target_f in [app_ts, auth_effects]:
                        if target_f.exists() and target_f.is_file():
                            ts_content = target_f.read_text(encoding="utf-8", errors="ignore")
                            if "localStorage" in ts_content and re.search(r'getItem|AUTH_USERNAME|remember', ts_content, re.IGNORECASE):
                                found_restoration = True
                                break
                    return found_restoration

                # Check 2B: Settings reducer fallback
                if "reducer" in desc_and_crit or "settings" in desc_and_crit:
                    if not settings_reducer.exists() or not settings_reducer.is_file():
                        return False
                    red_content = settings_reducer.read_text(encoding="utf-8", errors="ignore")
                    has_init = bool(re.search(r'rememberMe\s*:\s*false', red_content, re.IGNORECASE))
                    has_action = bool(re.search(r'actionSettingsChangeRememberMe|rememberMe', red_content, re.IGNORECASE))
                    return has_init and has_action

                return False

            elif req_id == "REQ-3":
                # Check 3A: Security negative check (zero password/token/secret/jwt persistence to localStorage)
                if "security" in desc_and_crit or "password" in desc_and_crit or "token" in desc_and_crit or "negative" in desc_and_crit:
                    security_violation = False
                    for target_f in [app_ts, auth_effects]:
                        if target_f.exists() and target_f.is_file():
                            ts_content = target_f.read_text(encoding="utf-8", errors="ignore")
                            if re.search(r'localStorage\s*\.\s*setItem\s*\([^)]*(?:password|token|secret|jwt)', ts_content, re.IGNORECASE):
                                security_violation = True
                                break
                    return not security_violation

                return False

            return False

        if self.task_id == "task_c_theme_toggle":
            app_html = sandbox_path / "projects/angular-ngrx-material-starter/src/app/app/app.component.html"
            app_ts = sandbox_path / "projects/angular-ngrx-material-starter/src/app/app/app.component.ts"
            core_mod = sandbox_path / "projects/angular-ngrx-material-starter/src/app/core/core.module.ts"

            if req_id == "REQ-1":
                if app_html.exists():
                    txt = app_html.read_text(encoding="utf-8", errors="ignore")
                    return ("mat-icon-button" in txt or "button" in txt) and ("theme" in txt.lower() or "night" in txt.lower() or "mode" in txt.lower())
                return False

            elif req_id == "REQ-2":
                if app_ts.exists():
                    txt = app_ts.read_text(encoding="utf-8", errors="ignore")
                    return "actionThemeChange" in txt or "actionSettingsChangeTheme" in txt or "theme" in txt.lower()
                return False

            elif req_id == "REQ-3":
                if app_ts.exists():
                    txt = app_ts.read_text(encoding="utf-8", errors="ignore")
                    return "store" in txt.lower() or "settings" in txt.lower()
                return False

            elif req_id == "REQ-4":
                return True

            elif req_id == "REQ-5":
                if core_mod.exists():
                    txt = core_mod.read_text(encoding="utf-8", errors="ignore")
                    return "faSun" in txt or "faMoon" in txt or "faLightbulb" in txt
                return False

            elif req_id == "REQ-6":
                if app_ts.exists():
                    txt = app_ts.read_text(encoding="utf-8", errors="ignore")
                    return "autoNightMode" in txt or "actionSettingsChangeAutoNightMode" in txt
                return False

            elif req_id == "REQ-7":
                return True

            return False

        if self.task_id == "task_commit_cache_invalidation":
            if req_id == "REQ-1":
                return self._check_commit_cache_req1_meta_persistence(sandbox_path)
            elif req_id == "REQ-3":
                return self._check_commit_cache_req3_clean_subcommand(sandbox_path)
            return False

        return False

    def _check_commit_cache_req1_meta_persistence(self, sandbox_path: Path) -> bool:
        """
        REQ-1: Record active Git commit HEAD in SQLite meta table.
        Criteria: index.db contains meta table recording git_commit_head.
        Verifies both physical SQLite index databases and source code persistence logic.
        """
        # 1. Executable check: inspect SQLite database file if it exists in .engram/
        candidate_db_names = [
            sandbox_path / ".engram" / "index.db",
            sandbox_path / ".engram" / "derived_symbols.db",
            sandbox_path / "index.db"
        ]
        for db_path in candidate_db_names:
            if db_path.exists() and db_path.is_file():
                try:
                    conn = sqlite3.connect(str(db_path))
                    cur = conn.cursor()
                    cur.execute("SELECT name FROM sqlite_master WHERE type='table' AND (lower(name)='meta' OR lower(name)='metadata')")
                    table_row = cur.fetchone()
                    if table_row:
                        table_name = table_row[0]
                        cur.execute(f"PRAGMA table_info({table_name})")
                        cols = [row[1].lower() for row in cur.fetchall()]
                        if "git_commit_head" in cols:
                            cur.execute(f"SELECT git_commit_head FROM {table_name} WHERE git_commit_head IS NOT NULL")
                            val_row = cur.fetchone()
                            if val_row and val_row[0]:
                                conn.close()
                                return True
                        if "key" in cols and ("value" in cols or "val" in cols):
                            val_col = "value" if "value" in cols else "val"
                            cur.execute(f"SELECT {val_col} FROM {table_name} WHERE lower(key)='git_commit_head' OR lower(key) LIKE '%commit%'")
                            val_row = cur.fetchone()
                            if val_row and val_row[0]:
                                conn.close()
                                return True
                    conn.close()
                except Exception:
                    pass

        # 2. Source code static check: verify SQL and metadata recording in source files
        target_files = [
            sandbox_path / "src" / "analyzer.rs",
            sandbox_path / "src" / "compiler.rs",
            sandbox_path / "src" / "storage.rs",
            sandbox_path / "src" / "main.rs",
            sandbox_path / "tests" / "commit_cache_invalidation_test.rs"
        ]
        has_meta_table_def = False
        has_commit_head_record = False

        for tf in target_files:
            if tf.exists() and tf.is_file():
                try:
                    content = tf.read_text(encoding="utf-8", errors="ignore")
                    if re.search(r'(?i)CREATE\s+TABLE\s+IF\s+NOT\s+EXISTS\s+(?:meta|metadata)|(?:meta|metadata)\s*\([^)]*(?:key|git_commit)', content) or \
                       re.search(r'(?i)\b(?:meta|metadata)\b.*git_commit_head', content, re.DOTALL):
                        has_meta_table_def = True

                    if re.search(r'git_commit_head', content, re.IGNORECASE) or \
                       ("meta" in content.lower() and re.search(r'commit.*(?:head|hash|rev)', content, re.IGNORECASE)) or \
                       re.search(r'(?i)(?:INSERT|REPLACE|UPDATE)\s+INTO\s+(?:meta|metadata)', content):
                        if re.search(r'git_commit_head', content, re.IGNORECASE):
                            has_commit_head_record = True
                except Exception:
                    pass

        return has_meta_table_def and has_commit_head_record

    def _check_commit_cache_req2_drift_detection(self, sandbox_path: Path) -> bool:
        """
        REQ-2: Commit drift detection invalidates stale cache entries.
        Criteria: ensure_fresh triggers re-index when repository HEAD diverges from cached commit.
        """
        target_files = [
            sandbox_path / "src" / "analyzer.rs",
            sandbox_path / "src" / "compiler.rs",
            sandbox_path / "tests" / "commit_cache_invalidation_test.rs"
        ]
        has_drift_detection = False

        for tf in target_files:
            if tf.exists() and tf.is_file():
                try:
                    content = tf.read_text(encoding="utf-8", errors="ignore")
                    if "ensure_fresh" in content:
                        if re.search(r'(?:commit|head|rev|git_commit_head).*!=|!=\s*(?:commit|head|rev|cached_commit)', content, re.IGNORECASE) or \
                           re.search(r'(?:meta|metadata).*commit.*(?:stale|drift|analyze|invalidate)', content, re.IGNORECASE | re.DOTALL):
                            has_drift_detection = True
                    if tf.name == "commit_cache_invalidation_test.rs" and "ensure_fresh" in content and re.search(r'drift|commit|stale', content, re.IGNORECASE):
                        has_drift_detection = True
                except Exception:
                    pass

        return has_drift_detection

    def _check_commit_cache_req3_clean_subcommand(self, sandbox_path: Path) -> bool:
        """
        REQ-3: Add engram clean --cache subcommand preserving memories.
        Criteria: clean subcommand removes SQLite cache without deleting memories.json.
        """
        # 1. Executable check: if a compiled engram binary exists in sandbox target/
        for bin_subpath in ["target/debug/engram.exe", "target/release/engram.exe"]:
            bin_path = sandbox_path / bin_subpath
            if bin_path.exists() and bin_path.is_file():
                try:
                    with tempfile.TemporaryDirectory(prefix="test_engram_clean_") as tmp_dir:
                        fixture_dir = Path(tmp_dir)
                        engram_dir = fixture_dir / ".engram"
                        engram_dir.mkdir(parents=True, exist_ok=True)
                        dummy_db = engram_dir / "index.db"
                        dummy_db.write_text("SENTINEL_INDEX_CACHE", encoding="utf-8")
                        mem_file = engram_dir / "memories.json"
                        sentinel_mem_content = '{"memories": ["SENTINEL_PRESERVED_USER_MEMORY_DO_NOT_DELETE"]}'
                        mem_file.write_text(sentinel_mem_content, encoding="utf-8")

                        res = subprocess.run(
                            [str(bin_path), "clean", "--cache"],
                            cwd=str(fixture_dir),
                            capture_output=True,
                            text=True,
                            timeout=15,
                            check=False
                        )
                        if res.returncode == 0:
                            cache_deleted = not dummy_db.exists()
                            memories_intact = (mem_file.exists() and mem_file.read_text(encoding="utf-8") == sentinel_mem_content)
                            if cache_deleted and memories_intact:
                                return True
                except Exception:
                    pass

        # 2. Source code static check: verify CommandLine enum and clean handler in src/main.rs
        main_rs = sandbox_path / "src" / "main.rs"
        if main_rs.exists() and main_rs.is_file():
            try:
                content = main_rs.read_text(encoding="utf-8", errors="ignore")
                has_clean_cmd = bool(
                    re.search(r'(?i)Clean\s*\{[^}]*cache\s*:\s*bool', content) or
                    re.search(r'(?i)#\[command\(.*alias.*clean.*\)\].*Clean', content) or
                    re.search(r'(?i)enum\s+CommandLine\s*\{[^}]*\bClean\b', content) or
                    re.search(r'(?i)\bCleanCache\b', content)
                )

                has_cache_arg = bool(
                    re.search(r'(?i)cache\s*:\s*bool', content) or
                    re.search(r'(?i)#\[arg\([^)]*cache[^)]*\)\]', content) or
                    re.search(r'(?i)"--cache"', content)
                )

                has_cache_removal = bool(
                    re.search(r'(?i)(?:remove_file|unlink|remove_dir)[^;]*(?:index\.db|derived_symbols|cache)', content) or
                    re.search(r'(?i)clean.*cache.*(?:remove_file|fs::remove)', content, re.DOTALL)
                )

                deletes_memories = bool(re.search(r'(?i)remove_file[^;]*memories\.json', content))

                if has_clean_cmd and has_cache_arg and has_cache_removal and not deletes_memories:
                    return True
            except Exception:
                pass

        return False

    def calculate_score(
        self,
        req_results: Dict[str, str],
        total_model_tokens: int,
        gate_results: Dict[str, str],
        dependency_mutated: bool = False
    ) -> CorrectnessResult:
        """Calculates correctness score C and quality-adjusted effective token cost."""
        if dependency_mutated:
            return CorrectnessResult(
                score=0.0,
                quality_adjusted_cost=total_model_tokens * 100.0,
                passed_all_requirements=False,
                gates=gate_results,
                requirements={req.get("id", ""): "FAIL" for req in self.requirements_cfg},
            )

        total_weight = 0.0
        earned_weight = 0.0
        all_passed = True

        for req in self.requirements_cfg:
            req_id = req.get("id", "")
            w = float(req.get("weight", 0.0))
            total_weight += w

            res = req_results.get(req_id, "FAIL")
            if res == "PASS":
                earned_weight += w
            elif res == "PARTIAL":
                earned_weight += (w * 0.5)
                all_passed = False
            else:
                all_passed = False

        # Mandatory quality gate verification:
        # Every enabled gate must complete with PASS.
        # BLOCKED (timeout), FAIL, ERROR, SKIPPED (when enabled), or missing results fail the mandatory gate check.
        gate_map = {
            "L0_typecheck": self.gates_cfg.get("L0_typecheck"),
            "L1_lint_prettier": self.gates_cfg.get("L1_lint") or self.gates_cfg.get("L1_lint_prettier"),
            "L2_unit_tests": self.gates_cfg.get("L2_unit_tests"),
            "L3_production_build": self.gates_cfg.get("L3_production_build"),
        }

        mandatory_gates_passed = True
        for gate_key, gate_info in gate_map.items():
            if gate_info and gate_info.get("enabled", False):
                status = gate_results.get(gate_key)
                if status is None and gate_key == "L1_lint_prettier":
                    status = gate_results.get("L1_lint")
                if status != "PASS":
                    mandatory_gates_passed = False
                    all_passed = False
                    break

        if not mandatory_gates_passed:
            all_passed = False
            score = 0.0
        else:
            score = round(earned_weight / total_weight, 4) if total_weight > 0 else 0.0
            score = min(1.0, max(0.0, score))

        eff_divisor = max(score, 0.01)
        quality_adjusted_cost = round(total_model_tokens / eff_divisor, 2)

        return CorrectnessResult(
            score=score,
            quality_adjusted_cost=quality_adjusted_cost,
            passed_all_requirements=all_passed and (score == 1.0),
            gates=gate_results,
            requirements=req_results,
        )
