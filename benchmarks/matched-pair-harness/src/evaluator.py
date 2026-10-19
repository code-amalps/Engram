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
import fnmatch
import json
import re
import os


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
    1. Confirm the configured target file exists in the pristine fixture within sandbox boundaries.
    2. Confirm the expected original text is present and unambiguous (exactly expected_occurrences, default 1).
    3. Confirm the expected corrected text is not already present (fixture not dirty / already fixed).
    4. Confirm the expected change is non-trivial (original_text != expected_text).
    Fails fast with clear diagnostic if any precondition is not satisfied.
    """
    task_id = task_config.get("task_id", "unknown_task")
    preflight_cfg = task_config.get("fixture_preflight")
    if not preflight_cfg:
        return

    target_file = preflight_cfg.get("target_file")
    if not target_file:
        raise FixturePreflightError(f"Task '{task_id}' fixture_preflight missing 'target_file'.")

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
    score: float
    quality_adjusted_cost: float
    passed_all_requirements: bool
    gates: Dict[str, str]  # L0_typecheck, L1_lint_prettier, L2_unit_tests, L3_production_build
    requirements: Dict[str, str]  # REQ-1: PASS/FAIL/PARTIAL

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
        cmd_status = ["git", "status", "--porcelain"]
        res_status = subprocess.run(cmd_status, cwd=sandbox_path, capture_output=True, text=True)
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
            changed_files.append(norm_fpath)
            if status_code == "??":
                untracked_files.append(norm_fpath)

        # 2. Get numstat covering BOTH staged and unstaged changes against HEAD
        cmd_diff = ["git", "diff", "HEAD", "--numstat"]
        res_diff = subprocess.run(cmd_diff, cwd=sandbox_path, capture_output=True, text=True)
        if res_diff.returncode != 0:
            # Fallback if HEAD does not exist (e.g. bare initialized repo in synthetic tests)
            cmd_diff = ["git", "diff", "--numstat"]
            res_diff = subprocess.run(cmd_diff, cwd=sandbox_path, capture_output=True, text=True)

        insertions = 0
        deletions = 0
        for line in res_diff.stdout.splitlines():
            parts = line.split()
            if len(parts) >= 3 and parts[0].isdigit() and parts[1].isdigit():
                insertions += int(parts[0])
                deletions += int(parts[1])

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
        cmd_patch = ["git", "diff", "HEAD"]
        res_patch = subprocess.run(cmd_patch, cwd=sandbox_path, capture_output=True, text=True)
        if res_patch.returncode != 0:
            cmd_patch = ["git", "diff"]
            res_patch = subprocess.run(cmd_patch, cwd=sandbox_path, capture_output=True, text=True)

        patch_text = res_patch.stdout
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
        unrelated_count = 0
        dependency_mutated = False
        for fpath in changed_files:
            # Check dependency mutation
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
        is_dry_run: bool = False
    ) -> Dict[str, str]:
        """Runs L0-L3 gates or produces deterministic simulated gates in dry run."""
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

        for gate_name, gate_info in gate_map.items():
            if not gate_info or not gate_info.get("enabled", False):
                results[gate_name] = "SKIPPED"
                continue

            if is_dry_run:
                results[gate_name] = "PASS"
                continue

            cmd = gate_info.get("command", "")
            try:
                proc = subprocess.run(
                    cmd,
                    cwd=sandbox_path,
                    shell=True,
                    capture_output=True,
                    text=True,
                    timeout=300
                )
                results[gate_name] = "PASS" if proc.returncode == 0 else "FAIL"
            except subprocess.TimeoutExpired:
                results[gate_name] = "BLOCKED"
            except Exception:
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
                    req_results[req_id] = "PASS"

            elif eval_type == "UNIT_TEST":
                l2 = gate_results.get("L2_unit_tests", "SKIPPED")
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
            return True

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

        return True

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
