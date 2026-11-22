import unittest
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from src.indexer import safe_divide

class TestIndexer(unittest.TestCase):
    def test_safe_divide_normal(self):
        self.assertEqual(safe_divide([1, 3]), [0.25, 0.75])

    def test_safe_divide_zeros(self):
        self.assertEqual(safe_divide([0, 0]), [0.0, 0.0])
        self.assertEqual(safe_divide([]), [])

if __name__ == '__main__':
    unittest.main()
