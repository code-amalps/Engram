import unittest
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from src.loader import load_settings

class TestLoader(unittest.TestCase):
    def test_settings_fallback(self):
        cfg = load_settings()
        self.assertEqual(cfg.get('timeout'), 30)
        self.assertEqual(cfg.get('retry'), 3)

if __name__ == '__main__':
    unittest.main()
