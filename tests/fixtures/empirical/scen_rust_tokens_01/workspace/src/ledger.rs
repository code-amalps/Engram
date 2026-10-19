use crate::account::Account;
use crate::currency::{fx_rate_bps, Currency};

pub struct LedgerEngine;

impl LedgerEngine {
    pub fn transfer(from: &mut Account, to: &mut Account, amount: i64) -> Result<(), &'static str> {
        if from.balance < amount {
            return Err("Insufficient funds");
        }
        from.balance -= amount;
        to.balance += amount;
        Ok(())
    }

    pub fn calculate_fx_fee(amount: i64, from: Currency, to: Currency) -> i64 {
        if from == to {
            0
        } else {
            let rate = fx_rate_bps(from, to) as i64;
            (amount * (rate - 10_000)) / 10_000
        }
    }
}
