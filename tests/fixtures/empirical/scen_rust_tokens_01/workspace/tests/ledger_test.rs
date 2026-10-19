use fixture_ledger::account::Account;
use fixture_ledger::currency::Currency;
use fixture_ledger::ledger::LedgerEngine;

#[test]
fn test_transfer_invariants() {
    let mut acc1 = Account::new(1, 1000);
    let mut acc2 = Account::new(2, 500);
    assert!(LedgerEngine::transfer(&mut acc1, &mut acc2, 300).is_ok());
    assert_eq!(acc1.balance, 700);
    assert_eq!(acc2.balance, 800);
}

#[test]
fn test_calculate_fx_fee() {
    let fee = LedgerEngine::calculate_fx_fee(10_000, Currency::USD, Currency::EUR);
    assert_eq!(fee, 250);
}
