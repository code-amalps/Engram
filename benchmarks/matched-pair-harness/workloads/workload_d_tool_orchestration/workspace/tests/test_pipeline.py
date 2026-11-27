import unittest
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from src.pipeline import load_pipeline_config

class TestPipeline(unittest.TestCase):
    def test_orchestration_config(self):
        cfg = load_pipeline_config()
        self.assertEqual(cfg.get('batch_size'), 64)
        self.assertEqual(cfg.get('workers'), 4)
        self.assertTrue(cfg.get('validated'))

if __name__ == '__main__':
    unittest.main()
