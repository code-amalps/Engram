from src.models import Account

class AccountService:
    def __init__(self):
        self.accounts = {}

    def register(self, account_id: str, owner: str, balance: float):
        acc = Account(account_id, owner, balance)
        self.accounts[account_id] = acc
        return acc

    def get_active_accounts(self):
        return list(self.accounts.values())
