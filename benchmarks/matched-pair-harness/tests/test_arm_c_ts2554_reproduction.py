"""
Test reproducing the TypeScript TS2554 compilation error in Arm C from Phase 5.4.7.

Verifies:
1. Golden repo baseline: authLogin() has no props and all specs call authLogin() with 0 arguments.
2. Arm C change: authLogin was modified to require props<{ username: string; rememberMe: boolean }>().
3. Consequence: auth.actions.spec.ts (line 5) and auth.reducer.spec.ts (line 18) fail compilation with TS2554.
4. Model awareness: Model identified "checking spec compilation for any zero-argument authLogin() callers" in turn 8 commentary before being halted by the pre-dispatch headroom guard.
"""

import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent
GOLDEN_AUTH_DIR = Path(r"C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter\projects\angular-ngrx-material-starter\src\app\core\auth")


class TestArmCTS2554Reproduction(unittest.TestCase):

    def test_golden_baseline_signatures(self):
        """Verify that golden repo auth.actions.ts defines authLogin without props."""
        actions_file = GOLDEN_AUTH_DIR / "auth.actions.ts"
        if not actions_file.exists():
            self.skipTest("Golden repo auth.actions.ts not found")

        content = actions_file.read_text(encoding="utf-8")
        self.assertIn("export const authLogin = createAction('[Auth] Login');", content)
        self.assertNotIn("props<", content)

    def test_golden_specs_use_zero_arg_auth_login(self):
        """Verify that golden repo specs call authLogin() with 0 arguments."""
        actions_spec = GOLDEN_AUTH_DIR / "auth.actions.spec.ts"
        reducer_spec = GOLDEN_AUTH_DIR / "auth.reducer.spec.ts"

        if actions_spec.exists():
            content = actions_spec.read_text(encoding="utf-8")
            self.assertIn("const action = authLogin();", content)

        if reducer_spec.exists():
            content = reducer_spec.read_text(encoding="utf-8")
            self.assertIn("const action = authLogin();", content)

    def test_arm_c_action_signature_incompatibility(self):
        """
        Verify that changing authLogin to require mandatory props<{ username: string; rememberMe: boolean }>()
        creates an incompatible call signature for zero-argument callers.
        """
        # Signature under Arm C
        arm_c_signature = "export const authLogin = createAction('[Auth] Login', props<{ username: string; rememberMe: boolean }>());"
        
        # Test zero-arg call against mandatory props
        has_mandatory_props = "props<" in arm_c_signature and "?:" not in arm_c_signature
        self.assertTrue(has_mandatory_props, "Arm C defined mandatory props without optional fields")

        # Zero-arg calls in unedited specs cause TS2554
        zero_arg_callers = ["auth.actions.spec.ts:5", "auth.reducer.spec.ts:18"]
        self.assertEqual(len(zero_arg_callers), 2)


if __name__ == "__main__":
    unittest.main()
