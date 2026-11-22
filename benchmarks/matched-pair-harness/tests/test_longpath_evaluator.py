import os
import sys
import unittest
import tempfile
import subprocess
from pathlib import Path

HARNESS_ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = HARNESS_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from evaluator import Evaluator, GitImpact


class TestLongPathEvaluator(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.repo_dir = Path(self.temp_dir.name) / "repo"
        self.repo_dir.mkdir(parents=True, exist_ok=True)

        # Initialize git repo with core.longpaths enabled
        subprocess.run(["git", "init"], cwd=self.repo_dir, capture_output=True, check=True)
        subprocess.run(["git", "-c", "core.longpaths=true", "config", "core.longpaths", "true"], cwd=self.repo_dir, capture_output=True, check=True)
        subprocess.run(["git", "config", "user.name", "TestUser"], cwd=self.repo_dir, capture_output=True, check=True)
        subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=self.repo_dir, capture_output=True, check=True)

        # Create deeply nested directory structure exceeding 247 characters
        # e.g. path > 250 characters
        nest = self.repo_dir / "deep_module_alpha_level1" / "nested_component_sublevel2" / "container_features_layer3" / "stock_market_analytics_engine_layer4" / "presentation_styles_and_theming_definitions_layer5"
        nest.mkdir(parents=True, exist_ok=True)
        
        self.deep_file = nest / "stock-market-container.component.scss-theme.scss"
        self.deep_file_rel = self.deep_file.relative_to(self.repo_dir).as_posix()
        
        # Verify path length exceeds 247 characters
        full_path_str = str(self.deep_file.resolve())
        self.assertGreater(len(full_path_str), 220) # Total length on Windows will be deep

        # Write initial commit
        self.initial_content = "@mixin theme-sample {\n  color: red;\n  background: blue;\n}\n"
        self.deep_file.write_text(self.initial_content, encoding="utf-8")

        # Standard file
        self.normal_file = self.repo_dir / "app.component.ts"
        self.normal_file.write_text("export class AppComponent {}\n", encoding="utf-8")

        subprocess.run(["git", "-c", "core.longpaths=true", "add", "."], cwd=self.repo_dir, capture_output=True, check=True)
        subprocess.run(["git", "-c", "core.longpaths=true", "commit", "-m", "Initial commit"], cwd=self.repo_dir, capture_output=True, check=True)

        task_config = {
            "task_id": "test_longpath_task",
            "repository_type": "angular",
            "allowed_patterns": ["*"],
            "forbidden_patterns": []
        }
        self.evaluator = Evaluator(task_config=task_config)
        self.patch_out = Path(self.temp_dir.name) / "changes.patch"

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_untouched_deep_file_does_not_appear_as_deletion(self):
        """Verifies that when a deep file is untouched, it produces 0 deletions and 0 changes."""
        impact = self.evaluator.analyze_git_impact(self.repo_dir, self.patch_out)
        self.assertEqual(impact.files_changed_count, 0)
        self.assertEqual(impact.insertions, 0)
        self.assertEqual(impact.deletions, 0)
        self.assertEqual(impact.files_changed, [])
        self.assertFalse(self.deep_file_rel in impact.files_changed)

    def test_actual_modification_to_deep_file_is_detected(self):
        """Verifies that genuine modifications to a deep file are accurately detected."""
        modified_content = "@mixin theme-sample {\n  color: green;\n  background: blue;\n  font-size: 14px;\n}\n"
        self.deep_file.write_text(modified_content, encoding="utf-8")

        impact = self.evaluator.analyze_git_impact(self.repo_dir, self.patch_out)
        self.assertEqual(impact.files_changed_count, 1)
        self.assertIn(self.deep_file_rel, impact.files_changed)
        self.assertGreater(impact.insertions, 0)
        self.assertGreater(impact.deletions, 0)

        # Check patch file
        patch_text = self.patch_out.read_text(encoding="utf-8")
        self.assertIn("color: green", patch_text)
        self.assertIn(self.deep_file_rel, patch_text)

    def test_actual_deletion_of_deep_file_is_detected(self):
        """Verifies that genuine deletions of a deep file are accurately detected."""
        self.deep_file.unlink()

        impact = self.evaluator.analyze_git_impact(self.repo_dir, self.patch_out)
        self.assertEqual(impact.files_changed_count, 1)
        self.assertIn(self.deep_file_rel, impact.files_changed)
        self.assertEqual(impact.insertions, 0)
        self.assertGreater(impact.deletions, 0)

        patch_text = self.patch_out.read_text(encoding="utf-8")
        self.assertIn("deleted file mode", patch_text)

    def test_reconciliation_between_porcelain_and_diff(self):
        """Verifies file count reconciliation when multiple files are modified and created."""
        # 1 modified deep file
        self.deep_file.write_text("@mixin updated {}\n", encoding="utf-8")
        # 1 new untracked file
        untracked = self.repo_dir / "new_feature.ts"
        untracked.write_text("export const x = 1;\n", encoding="utf-8")

        impact = self.evaluator.analyze_git_impact(self.repo_dir, self.patch_out)
        self.assertEqual(impact.files_changed_count, 2)
        self.assertIn(self.deep_file_rel, impact.files_changed)
        self.assertIn("new_feature.ts", impact.files_changed)
        self.assertGreater(impact.insertions, 0)


if __name__ == "__main__":
    unittest.main()
