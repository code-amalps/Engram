import unittest
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from src.service import AccountService

class TestAccountService(unittest.TestCase):
    def test_active_accounts(self):
        svc = AccountService()
        a1 = svc.register('acc_1', 'Alice', 100.0)
        a2 = svc.register('acc_2', 'Bob', 200.0)
        a2.is_active = False
        active = svc.get_active_accounts()
        self.assertEqual(len(active), 1)
        self.assertEqual(active[0].account_id, 'acc_1')

if __name__ == '__main__':
    unittest.main()
