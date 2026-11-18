"""
Unit tests for Task B (Remember-Me) acceptance evaluation and static checks.
Verifies that Evaluator rejects invalid implementations and accepts valid ones,
and that missing/unsupported static checks return False.
"""
from typing import Dict, Any
from pathlib import Path
import tempfile
import unittest
import shutil
import json
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "src"))

from evaluator import Evaluator, GitImpact, CorrectnessResult


class TestTaskBEvaluator(unittest.TestCase):
    def setUp(self):
        self.temp_dir = Path(tempfile.mkdtemp(prefix="test_task_b_eval_"))
        self.config_path = Path(__file__).resolve().parent.parent / "config" / "tasks" / "task_b_remember_me.json"
        with open(self.config_path, "r", encoding="utf-8") as f:
            self.task_cfg = json.load(f)
        self.evaluator = Evaluator(self.task_cfg)

        # Setup standard mock workspace layout
        self.app_dir = self.temp_dir / "projects" / "angular-ngrx-material-starter" / "src" / "app" / "app"
        self.auth_dir = self.temp_dir / "projects" / "angular-ngrx-material-starter" / "src" / "app" / "core" / "auth"
        self.i18n_dir = self.temp_dir / "projects" / "angular-ngrx-material-starter" / "src" / "assets" / "i18n"
        self.app_dir.mkdir(parents=True, exist_ok=True)
        self.auth_dir.mkdir(parents=True, exist_ok=True)
        self.i18n_dir.mkdir(parents=True, exist_ok=True)

    def tearDown(self):
        if self.temp_dir.exists():
            shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_req1_ui_rejects_missing_file(self):
        """REQ-1 fails if app.component.html does not exist."""
        passed = self.evaluator._check_static_requirement(self.temp_dir, "REQ-1")
        self.assertFalse(passed)

    def test_req1_ui_rejects_missing_checkbox(self):
        """REQ-1 fails if app.component.html contains username input but lacks remember-me checkbox."""
        html_file = self.app_dir / "app.component.html"
        html_file.write_text('<input matInput placeholder="Username">', encoding="utf-8")
        passed = self.evaluator._check_static_requirement(self.temp_dir, "REQ-1")
        self.assertFalse(passed)

    def test_req1_ui_rejects_commented_out_checkbox(self):
        """REQ-1 fails if remember-me checkbox is commented out."""
        html_file = self.app_dir / "app.component.html"
        html_file.write_text(
            '<!-- <mat-checkbox [(ngModel)]="rememberMe">Remember</mat-checkbox> -->\n'
            '<input matInput placeholder="Username">',
            encoding="utf-8"
        )
        passed = self.evaluator._check_static_requirement(self.temp_dir, "REQ-1")
        self.assertFalse(passed)

    def test_req1_ui_accepts_valid_implementation(self):
        """REQ-1 passes when both checkbox and username input are present in markup."""
        html_file = self.app_dir / "app.component.html"
        html_file.write_text(
            '<div class="login-row">\n'
            '  <input matInput placeholder="Username" [(ngModel)]="username">\n'
            '  <mat-checkbox [(ngModel)]="rememberMe">Remember Me</mat-checkbox>\n'
            '</div>',
            encoding="utf-8"
        )
        passed = self.evaluator._check_static_requirement(self.temp_dir, "REQ-1")
        self.assertTrue(passed)

    def test_req2_persistence_rejects_missing_logic(self):
        """REQ-2 fails if neither app.component.ts nor auth.effects.ts implement localStorage restoration."""
        app_ts = self.app_dir / "app.component.ts"
        app_ts.write_text('export class AppComponent { title = "app"; }', encoding="utf-8")
        passed = self.evaluator._check_static_requirement(self.temp_dir, "REQ-2")
        self.assertFalse(passed)

    def test_req2_persistence_accepts_valid_app_component_restoration(self):
        """REQ-2 passes if app.component.ts restores username from localStorage."""
        app_ts = self.app_dir / "app.component.ts"
        app_ts.write_text(
            'export class AppComponent implements OnInit {\n'
            '  ngOnInit() {\n'
            '    const saved = localStorage.getItem("AUTH_USERNAME");\n'
            '    if (saved) { this.username = saved; }\n'
            '  }\n'
            '}',
            encoding="utf-8"
        )
        passed = self.evaluator._check_static_requirement(self.temp_dir, "REQ-2")
        self.assertTrue(passed)

    def test_req2_persistence_accepts_valid_auth_effects_restoration(self):
        """REQ-2 passes if auth.effects.ts implements localStorage remember logic."""
        auth_eff = self.auth_dir / "auth.effects.ts"
        auth_eff.write_text(
            'export class AuthEffects {\n'
            '  init$ = createEffect(() => {\n'
            '    const remember = localStorage.getItem("remember");\n'
            '  });\n'
            '}',
            encoding="utf-8"
        )
        passed = self.evaluator._check_static_requirement(self.temp_dir, "REQ-2")
        self.assertTrue(passed)

    def test_req3_security_negative_rejects_password_persistence(self):
        """REQ-3 security check fails if password is saved to localStorage."""
        app_ts = self.app_dir / "app.component.ts"
        app_ts.write_text(
            'localStorage.setItem("password", userPassword);',
            encoding="utf-8"
        )
        passed = self.evaluator._check_static_requirement(self.temp_dir, "REQ-3")
        self.assertFalse(passed)

    def test_req3_security_negative_rejects_token_persistence(self):
        """REQ-3 security check fails if jwt token is saved to localStorage."""
        auth_eff = self.auth_dir / "auth.effects.ts"
        auth_eff.write_text(
            'localStorage.setItem("token", jwtToken);',
            encoding="utf-8"
        )
        passed = self.evaluator._check_static_requirement(self.temp_dir, "REQ-3")
        self.assertFalse(passed)

    def test_req3_security_negative_accepts_clean_persistence(self):
        """REQ-3 passes when only non-sensitive username is stored."""
        app_ts = self.app_dir / "app.component.ts"
        app_ts.write_text(
            'if (rememberMe) {\n'
            '  localStorage.setItem("AUTH_USERNAME", this.username);\n'
            '}',
            encoding="utf-8"
        )
        passed = self.evaluator._check_static_requirement(self.temp_dir, "REQ-3")
        self.assertTrue(passed)

    def test_req4_boundary_rejects_bulk_i18n_sprawl(self):
        """REQ-4 fails if 3 or more translation files are modified (sprawl)."""
        git_impact = GitImpact(
            files_changed_count=3,
            files_changed=[
                "projects/angular-ngrx-material-starter/src/assets/i18n/ar.json",
                "projects/angular-ngrx-material-starter/src/assets/i18n/de.json",
                "projects/angular-ngrx-material-starter/src/assets/i18n/es.json",
            ],
            insertions=10,
            deletions=2,
            unrelated_files_count=0,
            diff_patch_path=str(self.temp_dir / "diff.patch"),
            dependency_mutated=False
        )
        results = self.evaluator.evaluate_requirements(
            self.temp_dir,
            git_impact=git_impact,
            gate_results={"L0_typecheck": "PASS", "L2_unit_tests": "PASS", "L3_production_build": "PASS"}
        )
        self.assertEqual(results.get("REQ-4"), "FAIL")

    def test_req4_boundary_accepts_valid_scope(self):
        """REQ-4 passes when only allowed auth/app files and en.json are modified."""
        git_impact = GitImpact(
            files_changed_count=2,
            files_changed=[
                "projects/angular-ngrx-material-starter/src/app/app/app.component.html",
                "projects/angular-ngrx-material-starter/src/assets/i18n/en.json",
            ],
            insertions=5,
            deletions=1,
            unrelated_files_count=0,
            diff_patch_path=str(self.temp_dir / "diff.patch"),
            dependency_mutated=False
        )
        results = self.evaluator.evaluate_requirements(
            self.temp_dir,
            git_impact=git_impact,
            gate_results={"L0_typecheck": "PASS", "L2_unit_tests": "PASS", "L3_production_build": "PASS"}
        )
        self.assertEqual(results.get("REQ-4"), "PASS")

    def test_missing_unsupported_requirement_strictly_returns_false(self):
        """Unsupported requirement ID returns False, not True."""
        passed = self.evaluator._check_static_requirement(self.temp_dir, "REQ-999")
        self.assertFalse(passed)

    def test_unsupported_task_strictly_returns_false(self):
        """Unsupported task ID returns False, not True."""
        bogus_cfg = dict(self.task_cfg)
        bogus_cfg["task_id"] = "unknown_task_xyz"
        bogus_eval = Evaluator(bogus_cfg)
        passed = bogus_eval._check_static_requirement(self.temp_dir, "REQ-1")
        self.assertFalse(passed)

    # =========================================================================
    # Task B Fixture Preflight & Acceptance Coverage Boundary Tests
    # =========================================================================

    def _setup_pristine_baseline_fixture(self):
        """Populates mock workspace with pristine baseline files matching commit 1c285608."""
        html_f = self.app_dir / "app.component.html"
        html_f.write_text('<mat-toolbar color="primary">\n<button (click)="onLoginClick()">Login</button>\n</mat-toolbar>', encoding="utf-8")

        ts_f = self.app_dir / "app.component.ts"
        ts_f.write_text('export class AppComponent { onLoginClick() {} }', encoding="utf-8")

        auth_actions = self.auth_dir / "auth.actions.ts"
        auth_actions.write_text('import { createAction } from "@ngrx/store";\nexport const authLogin = createAction("[Auth] Login");\nexport const authLogout = createAction("[Auth] Logout");', encoding="utf-8")

        auth_effects = self.auth_dir / "auth.effects.ts"
        auth_effects.write_text('export class AuthEffects {}', encoding="utf-8")

        en_json = self.i18n_dir / "en.json"
        en_json.write_text('{"anms.menu.login": "Sign in", "anms.menu.logout": "Logout"}', encoding="utf-8")

    def test_fixture_preflight_task_b_accepts_pristine_baseline(self):
        """Task B preflight succeeds on a pristine fixture matching baseline commit preconditions."""
        self._setup_pristine_baseline_fixture()
        # Should not raise
        self.evaluator.validate_preflight(self.temp_dir)

    def test_fixture_preflight_task_b_rejects_missing_required_file(self):
        """Task B preflight fails closed if an expected baseline fixture path is missing."""
        self._setup_pristine_baseline_fixture()
        # Remove a required baseline file
        (self.auth_dir / "auth.actions.ts").unlink()
        from evaluator import FixturePreflightError
        with self.assertRaises(FixturePreflightError) as ctx:
            self.evaluator.validate_preflight(self.temp_dir)
        self.assertIn("Required baseline file does not exist", str(ctx.exception))

    def test_fixture_preflight_task_b_rejects_missing_baseline_pattern(self):
        """Task B preflight fails closed if an expected baseline hook pattern is missing."""
        self._setup_pristine_baseline_fixture()
        # Overwrite auth.actions.ts without authLogin
        (self.auth_dir / "auth.actions.ts").write_text('export const different = 1;', encoding="utf-8")
        from evaluator import FixturePreflightError
        with self.assertRaises(FixturePreflightError) as ctx:
            self.evaluator.validate_preflight(self.temp_dir)
        self.assertIn("Expected baseline pattern", str(ctx.exception))

    def test_coverage_detection_username_prefill_after_reload(self):
        """
        COVERAGE AUDIT: Evaluator detects static presence of localStorage.getItem,
        but cannot verify runtime browser reload or component state rehydration.
        """
        # Code with static string passes REQ-2
        app_ts = self.app_dir / "app.component.ts"
        app_ts.write_text('const x = localStorage.getItem("AUTH_USERNAME");', encoding="utf-8")
        passed = self.evaluator._check_static_requirement(self.temp_dir, "REQ-2")
        self.assertTrue(passed)

        # Limitation: A static string without actual ngOnInit lifecycle rehydration passes
        app_ts.write_text('// dormant dead code: localStorage.getItem("AUTH_USERNAME")', encoding="utf-8")
        dormant_passed = self.evaluator._check_static_requirement(self.temp_dir, "REQ-2")
        # Demonstrates static regex accepts the pattern without runtime proof of reload prefill
        self.assertTrue(dormant_passed)

    def test_coverage_detection_persistence_only_when_remember_me_enabled(self):
        """
        COVERAGE AUDIT: Evaluator does not verify conditional branching.
        Unconditional persistence (without if (rememberMe)) passes REQ-2 and REQ-3.
        """
        app_ts = self.app_dir / "app.component.ts"
        # Persisting unconditionally without checking rememberMe
        app_ts.write_text(
            'export class AppComponent {\n'
            '  login(user: string) {\n'
            '    localStorage.setItem("AUTH_USERNAME", user);\n'
            '  }\n'
            '}',
            encoding="utf-8"
        )
        req2_passed = self.evaluator._check_static_requirement(self.temp_dir, "REQ-2")
        req3_passed = self.evaluator._check_static_requirement(self.temp_dir, "REQ-3")
        # Limitation: Evaluator accepts unconditional persistence because REQ-2 checks for getItem/AUTH_USERNAME
        # and REQ-3 only rejects password/token/secret
        self.assertTrue(req2_passed)
        self.assertTrue(req3_passed)

    def test_coverage_detection_removal_when_unchecked(self):
        """
        COVERAGE AUDIT: Evaluator has NO requirement checking removal when unchecked.
        An implementation omitting uncheck cleanup is scored PASS across all requirements.
        """
        html_file = self.app_dir / "app.component.html"
        html_file.write_text('<input matInput placeholder="Username"><mat-checkbox [(ngModel)]="rememberMe">Remember</mat-checkbox>', encoding="utf-8")
        app_ts = self.app_dir / "app.component.ts"
        app_ts.write_text('export class AppComponent { ngOnInit() { localStorage.getItem("AUTH_USERNAME"); } }', encoding="utf-8")

        git_impact = GitImpact(
            files_changed_count=2,
            files_changed=[
                "projects/angular-ngrx-material-starter/src/app/app/app.component.html",
                "projects/angular-ngrx-material-starter/src/app/app/app.component.ts"
            ],
            insertions=10,
            deletions=0,
            unrelated_files_count=0,
            diff_patch_path=str(self.temp_dir / "diff.patch"),
            dependency_mutated=False
        )
        results = self.evaluator.evaluate_requirements(
            self.temp_dir,
            git_impact=git_impact,
            gate_results={"L0_typecheck": "PASS", "L2_unit_tests": "PASS", "L3_production_build": "PASS"}
        )
        # Even with zero uncheck removal logic, all requirements pass
        self.assertEqual(results.get("REQ-1"), "PASS")
        self.assertEqual(results.get("REQ-2"), "PASS")
        self.assertEqual(results.get("REQ-3"), "PASS")
        self.assertEqual(results.get("REQ-4"), "PASS")

    def test_coverage_detection_removal_on_logout(self):
        """
        COVERAGE AUDIT: Evaluator has NO check verifying removal on logout.
        Omitting logout removal logic passes all declared evaluator checks.
        """
        # Auth effects has no logout clearing logic
        auth_eff = self.auth_dir / "auth.effects.ts"
        auth_eff.write_text('export class AuthEffects {}', encoding="utf-8")
        app_ts = self.app_dir / "app.component.ts"
        app_ts.write_text('export class AppComponent { ngOnInit() { localStorage.getItem("AUTH_USERNAME"); } }', encoding="utf-8")

        passed_req2 = self.evaluator._check_static_requirement(self.temp_dir, "REQ-2")
        self.assertTrue(passed_req2)
        # Confirms no requirement in self.task_cfg covers logout cleanup
        req_ids = [r["id"] for r in self.task_cfg.get("requirements", [])]
        self.assertNotIn("REQ-LOGOUT", req_ids)

    def test_coverage_detection_no_persistence_of_passwords_tokens_secrets(self):
        """
        COVERAGE AUDIT: Evaluator actively and correctly catches password/token/secret persistence.
        """
        app_ts = self.app_dir / "app.component.ts"
        app_ts.write_text('localStorage.setItem("authToken", token);', encoding="utf-8")
        passed = self.evaluator._check_static_requirement(self.temp_dir, "REQ-3")
        self.assertFalse(passed)

    def test_coverage_detection_preservation_of_existing_auth_session_semantics(self):
        """
        COVERAGE AUDIT: Evaluator does not execute runtime session tests.
        Modifying auth session behavior without breaking TypeScript typechecks is not detected.
        """
        auth_actions = self.auth_dir / "auth.actions.ts"
        # Modify session payload semantics
        auth_actions.write_text('import { createAction, props } from "@ngrx/store";\nexport const authLogin = createAction("[Auth] Login", props<{ customSession: any }>());', encoding="utf-8")
        # Evaluator does not have a session semantics validator
        req_ids = [r["id"] for r in self.task_cfg.get("requirements", [])]
        self.assertNotIn("REQ-SESSION-SEMANTICS", req_ids)


if __name__ == "__main__":
    unittest.main()
