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
from typing import Dict, Any, List, Optional, Tuple
from pathlib import Path
import subprocess
import fnmatch
import json
import re


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
        """Inspects git diff and status against sandbox baseline."""
        # 1. Get changed files
        cmd_status = ["git", "status", "--porcelain"]
        res_status = subprocess.run(cmd_status, cwd=sandbox_path, capture_output=True, text=True)
        changed_files = []
        for line in res_status.stdout.splitlines():
            line = line.strip()
            if line:
                parts = line.split(maxsplit=1)
                if len(parts) == 2:
                    changed_files.append(parts[1].replace("\\", "/"))

        # 2. Get numstat (insertions/deletions)
        cmd_diff = ["git", "diff", "--numstat"]
        res_diff = subprocess.run(cmd_diff, cwd=sandbox_path, capture_output=True, text=True)
        insertions = 0
        deletions = 0
        for line in res_diff.stdout.splitlines():
            parts = line.split()
            if len(parts) >= 3 and parts[0].isdigit() and parts[1].isdigit():
                insertions += int(parts[0])
                deletions += int(parts[1])

        # 3. Export full patch
        cmd_patch = ["git", "diff"]
        res_patch = subprocess.run(cmd_patch, cwd=sandbox_path, capture_output=True, text=True)
        patch_output_path.parent.mkdir(parents=True, exist_ok=True)
        with open(patch_output_path, "w", encoding="utf-8") as f:
            f.write(res_patch.stdout)

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

    def _check_static_requirement(self, sandbox_path: Path, req_id: str) -> bool:
        """Domain-specific static checks based on requirement ID and task."""
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

        if gate_results.get("L0_typecheck") == "FAIL" or gate_results.get("L3_production_build") == "FAIL":
            all_passed = False

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
