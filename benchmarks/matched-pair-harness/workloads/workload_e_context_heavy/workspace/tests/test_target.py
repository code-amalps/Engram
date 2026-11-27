import unittest
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from src.target_module import MAX_RETRIES

class TestTarget(unittest.TestCase):
    def test_target_max_retries(self):
        self.assertEqual(MAX_RETRIES, 5)

if __name__ == '__main__':
    unittest.main()
