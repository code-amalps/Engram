import unittest
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from src.calculator import add_tax

class TestCalculator(unittest.TestCase):
    def test_tax_calculation(self):
        self.assertEqual(add_tax(100.0, 0.05), 105.0)
        self.assertEqual(add_tax(50.0, 0.10), 55.0)

if __name__ == '__main__':
    unittest.main()
